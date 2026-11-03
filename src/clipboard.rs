//! A selection copied in Figma, saved from the clipboard's HTML, as a `.fig` the other commands read.
//!
//! Figma writes `<!--(figma)BASE64(/figma)-->` into the clipboard HTML: the same `fig-kiwi`
//! container as a `.fig`'s `canvas.fig`, holding the copied layers and the components their
//! instances use. It carries no image bytes and no stored vector outlines, so the report names
//! what is missing and exports warn where they draw without it.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use serde_json::{Value as Json, json};

use crate::{Document, ErrorKind, FigError, Tree, guid_key, parse};

const START: &str = "(figma)";
const END: &str = "(/figma)";

/// Converts the saved clipboard HTML at `input` into a `.fig` at `out` and reports what it lacks.
/// Never replaces an existing file: a full design saved over with a partial copy is lost.
pub fn convert(input: &Path, out: &Path) -> Result<Json, Box<dyn Error>> {
    // Names a missing output folder as other writes do.
    crate::output::destination(out)
        .map_err(|e| FigError::new(ErrorKind::OutputWrite, e.to_string()))?;
    // Claim the path atomically: a file that appears while this runs is never replaced either.
    match OpenOptions::new().write(true).create_new(true).open(out) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(FigError::new(
                ErrorKind::InvalidArguments,
                format!(
                    "{} exists; choose a new path for the copied selection",
                    out.display()
                ),
            )
            .into());
        }
        Err(error) => return Err(FigError::new(ErrorKind::OutputWrite, error.to_string()).into()),
    }
    let result = write(input, out);
    if result.is_err() {
        // Remove only the empty claim, never a file another process put there meanwhile.
        if fs::metadata(out).is_ok_and(|m| m.len() == 0) {
            let _ = fs::remove_file(out);
        }
    }
    result
}

fn write(input: &Path, out: &Path) -> Result<Json, Box<dyn Error>> {
    // Base64 is 4/3 of the canvas, plus the HTML text Figma adds around it.
    let limit = parse::CANVAS_BYTES / 3 * 4 + (16 << 20);
    let html = crate::read_limited(fs::File::open(input)?, limit, "clipboard bytes")?;
    let canvas = canvas(&html)?;
    // Hold one copy of the payload, not the HTML as well, through decoding and writing.
    drop(html);
    let mut doc = Document::from_canvas(&canvas)?;
    let message = doc.take_message()?;
    let mut report = report(&message, &Tree::new(&message));
    crate::output::write(input, out, |file| {
        let mut zip = zip::ZipWriter::new(file);
        // The chunks inside are already compressed.
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("canvas.fig", options)?;
        zip.write_all(&canvas)?;
        zip.finish()?;
        Ok(())
    })?;
    report["path"] = json!(out);
    Ok(report)
}

/// The `fig-kiwi` bytes between Figma's clipboard markers.
fn canvas(html: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let invalid = |message: &str| FigError::new(ErrorKind::InvalidData, message.to_owned());
    let find = |needle: &str, from: usize| {
        html[from..]
            .windows(needle.len())
            .position(|w| w == needle.as_bytes())
            .map(|i| i + from)
    };
    let start = find(START, 0).ok_or_else(|| {
        invalid(
            "no Figma selection in this file: copy layers in Figma, then save the clipboard's HTML",
        )
    })? + START.len();
    let end = find(END, start).ok_or_else(|| invalid("the Figma clipboard data is cut off"))?;
    base64(&html[start..end])
        .ok_or_else(|| invalid("the Figma clipboard data is not base64").into())
}

/// Standard-alphabet base64, ignoring whitespace; `None` on any other byte.
fn base64(text: &[u8]) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut bits, mut count) = (0u32, 0);
    let mut padding = false;
    for &c in text {
        if c.is_ascii_whitespace() {
            continue;
        }
        if c == b'=' {
            padding = true;
            continue;
        }
        if padding {
            return None;
        }
        bits = bits << 6 | u32::from(value(c)?);
        count += 1;
        if count == 4 {
            out.extend_from_slice(&bits.to_be_bytes()[1..]);
            (bits, count) = (0, 0);
        }
    }
    match count {
        0 => {}
        2 => out.push((bits >> 4) as u8),
        3 => out.extend_from_slice(&((bits >> 2) as u16).to_be_bytes()),
        _ => return None,
    }
    Some(out)
}

/// The copied layers, the source file and what the payload lacks.
fn report(message: &kiwi_schema::Value, tree: &Tree) -> Json {
    use kiwi_schema::Value as K;
    fn kind<'a>(node: &K<'a>) -> &'a str {
        match node.get("type") {
            Some(K::Enum(_, kind)) => kind,
            _ => "",
        }
    }
    let (mut images, mut components, mut outlines) = (BTreeSet::new(), Vec::new(), 0);
    let mut ids: Vec<&String> = tree.nodes.keys().collect();
    ids.sort();
    for &id in &ids {
        let node = tree.nodes[id];
        collect_images(&crate::to_json(node), &mut images);
        // The component an instance draws, as REST `componentId` picks it: a swap first.
        if kind(node) == "INSTANCE"
            && let Some(component) = node
                .get("overriddenSymbolID")
                .and_then(guid_key)
                .or_else(|| node.get("symbolData")?.get("symbolID").and_then(guid_key))
            && !tree.nodes.contains_key(&component)
        {
            components.push(json!({"instanceId": id, "componentId": component}));
        }
        if crate::render::outlined(kind(node))
            && (node.get("fillGeometry").is_none() || node.get("strokeGeometry").is_none())
        {
            outlines += 1;
        }
    }
    // Copied layers sit directly on a page that is not the internal one holding components.
    let layers: Vec<&String> = ids
        .iter()
        .filter(|id| {
            let page = tree.nodes[**id];
            kind(page) == "CANVAS" && !matches!(page.get("internalOnly"), Some(K::Bool(true)))
        })
        .flat_map(|page| tree.children.get(*page).into_iter().flatten())
        .collect();
    let mut warnings = Vec::new();
    if !images.is_empty() {
        warnings.push(format!(
            "{} image(s) are referenced but a copied selection carries no image bytes; exports draw them empty and bundles list them as missing assets",
            images.len()
        ));
    }
    if outlines > 0 {
        warnings.push(format!(
            "{outlines} vector layer(s) lack a stored fill or stroke outline; exports draw such fills as boxes and such strokes around boxes, and warn for each painted one"
        ));
    }
    if !components.is_empty() {
        warnings.push(format!(
            "{} instance(s) use a component the copy does not include; they show only their stored layers and exports warn for each",
            components.len()
        ));
    }
    let file_key = match message.get("pasteFileKey") {
        Some(kiwi_schema::Value::String(key)) => json!(key),
        _ => Json::Null,
    };
    json!({
        "sourceFileKey": file_key,
        "layers": layers,
        "nodeCount": tree.nodes.len(),
        "missingImages": images,
        "missingComponents": components,
        "layersWithoutOutlines": outlines,
        "warnings": warnings,
    })
}

fn collect_images(value: &Json, out: &mut BTreeSet<String>) {
    match value {
        Json::Object(fields) => {
            if let Some(hash) = fields
                .get("image")
                .and_then(|i| i.get("hash"))
                .and_then(Json::as_str)
                .filter(|h| !h.is_empty())
            {
                out.insert(hash.to_owned());
            }
            fields.values().for_each(|v| collect_images(v, out));
        }
        Json::Array(items) => items.iter().for_each(|v| collect_images(v, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests;
