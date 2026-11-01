//! Compare a screenshot of an implemented screen with the frame figlens renders, at the
//! screenshot's scale, and name the layers under the regions that differ.
//!
//! Pixels are compared over white with a per-channel threshold. A pixel only differs when no
//! pixel within `radius` of it in the other image is within the threshold, both ways, so
//! anti-aliasing and sub-pixel edge offsets do not count. Differing pixels are grouped on a
//! grid of cells into regions; tiny regions are dropped as noise.

use std::error::Error;
use std::io::Read as _;
use std::path::Path;

use resvg::tiny_skia::Pixmap;
use serde_json::{Value as Json, json};

use crate::render::{self, Prepared, Rect};
use crate::{ErrorKind, FigError, Tree};

/// Encoded screenshot bytes read at most; checked before decoding.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Decoded pixels of the screenshot, and of the frame rendered at its scale (100 MB RGBA each).
pub const MAX_PIXELS: u64 = 25_000_000;
/// Default per-channel difference (of 255) above which two pixels differ.
pub const THRESHOLD: u8 = 48;
/// Grid cell side in frame (CSS) pixels.
const CELL: f64 = 8.0;
/// A region with fewer differing pixels than this many frame px² is noise.
const MIN_REGION: f64 = 12.0;
/// Most layers named per region.
const MAX_NODES: usize = 3;

fn invalid(message: impl Into<String>) -> Box<dyn Error> {
    FigError::new(ErrorKind::InvalidArguments, message).into()
}

/// Reads a screenshot file, refusing one over `MAX_BYTES` without reading it all.
pub fn read(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(invalid("screenshot exceeds 64 MiB; use a PNG of the frame"));
    }
    Ok(bytes)
}

/// Decodes a PNG screenshot after checking its declared size against `MAX_PIXELS`.
pub fn decode(bytes: &[u8]) -> Result<Pixmap, Box<dyn Error>> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(invalid("screenshot must be a PNG"));
    }
    let declared = imagesize::blob_size(bytes).map_err(|_| invalid("invalid PNG screenshot"))?;
    let pixels = (declared.width as u64).checked_mul(declared.height as u64);
    if declared.width == 0 || declared.height == 0 || pixels.is_none_or(|n| n > MAX_PIXELS) {
        return Err(invalid(format!(
            "screenshot is {}x{} px, over {MAX_PIXELS} pixels; capture at a lower device pixel ratio or crop it",
            declared.width, declared.height
        )));
    }
    let shot = Pixmap::decode_png(bytes).map_err(|_| invalid("invalid PNG screenshot"))?;
    if shot.width() as usize != declared.width || shot.height() as usize != declared.height {
        return Err(invalid("screenshot dimensions changed during decode"));
    }
    Ok(shot)
}

/// How to compare a screenshot.
#[derive(Debug)]
pub struct Options {
    /// Screenshot pixels per node pixel; `None` derives it from the width.
    pub scale: Option<f64>,
    /// Most regions listed.
    pub limit: usize,
    /// Also build the diff image.
    pub diff: bool,
    /// The screenshot is the whole page: a shorter one is compared over its height and the
    /// frame rows it lacks count as one differing region.
    pub full_page: bool,
    /// Layers inside the node whose render boxes are left out, such as content from data.
    pub ignore: Vec<String>,
    /// Per-channel difference (of 255) above which two pixels differ.
    pub threshold: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scale: None,
            limit: 10,
            diff: false,
            full_page: false,
            ignore: Vec::new(),
            threshold: THRESHOLD,
        }
    }
}

/// Compares `shot` with node `id` rendered over its layout box at `scale`, or at the scale that
/// fits the screenshot's width. Returns the report (regions in frame pixels from the frame's
/// top-left) and, with `diff`, a diff image the size of the compared area.
pub fn compare(
    tree: &Tree,
    fig: &Path,
    id: &str,
    shot: &Pixmap,
    opts: &Options,
) -> Result<(Json, Option<Pixmap>), Box<dyn Error>> {
    let (limit, full_page, threshold) = (opts.limit, opts.full_page, opts.threshold);
    let prepared = Prepared::new(tree, fig, id)?;
    let n = prepared.node();
    let unsupported = |reason: &str| -> Box<dyn Error> {
        FigError::new(
            ErrorKind::UnsupportedNode,
            format!("node {id} cannot be compared: {reason}"),
        )
        .into()
    };
    if n["visible"] == false {
        return Err(unsupported("the layer is hidden"));
    }
    // A screenshot shows the frame's own box, not shadows drawn past it.
    let area = render::layout_bounds(n)
        .or_else(|| render::export_area(n))
        .ok_or_else(|| unsupported("it has no drawable bounds"))?;
    let (fw, fh) = (area[2] - area[0], area[3] - area[1]);
    // Each ignored layer and its render box, checked before rendering. A layer without one
    // draws nothing here; its layout box could cover content that does draw.
    let mut ignored: Vec<(&Json, Rect)> = Vec::new();
    for layer in &opts.ignore {
        let found = (layer != id)
            .then(|| render::find(n, layer))
            .flatten()
            .ok_or_else(|| invalid(format!("ignore {layer} is not a layer inside node {id}")))?;
        let rect = render::render_bounds(found).ok_or_else(|| {
            invalid(format!(
                "ignore {layer} draws nothing in node {id} (hidden, a mask or clipped away)"
            ))
        })?;
        if !ignored.iter().any(|(l, _)| l["id"] == found["id"]) {
            ignored.push((found, rect));
        }
    }
    let (sw, sh) = (shot.width(), shot.height());
    let size = format!("{}x{}", num(fw), num(fh));
    let scale = match opts.scale {
        Some(s) if ((fw * s).round() - f64::from(sw)).abs() > 1.0 => {
            return Err(invalid(format!(
                "screenshot is {sw} px wide but node {id} is {size} px, {} px at scale {s}; omit scale to derive it from the width, or capture the frame alone at that width",
                (fw * s).round()
            )));
        }
        Some(s) => s,
        // A narrower capture is a narrower viewport, which lays the page out again rather
        // than scaling it (a phone screenshot of a desktop frame).
        None if f64::from(sw) < fw - 1.0 => {
            return Err(invalid(format!(
                "screenshot is {sw} px wide, narrower than node {id} ({size} px); capture it at a viewport of the node's width, or pass scale if the image was scaled down on purpose"
            )));
        }
        None => f64::from(sw) / fw,
    };
    let (rw, rh) = ((fw * scale).round(), (fh * scale).round());
    if rw * rh > MAX_PIXELS as f64 {
        return Err(invalid(format!(
            "node {id} at scale {} is {rw}x{rh} px, over {MAX_PIXELS} pixels; compare a smaller frame or a lower-scale screenshot",
            num(scale)
        )));
    }
    // A slightly shorter capture (rounding, a scrollbar) still compares; a viewport capture of
    // a taller frame does not. A stated whole page is short past rounding: its rows are missing.
    let slack = if full_page { 1.0 } else { (rh * 0.01).max(2.0) };
    let short = f64::from(sh) < rh - slack;
    if short && !full_page {
        return Err(invalid(format!(
            "screenshot is {sw}x{sh} px but node {id} ({size}) is {rw}x{rh} px at scale {}; capture the whole frame (a full-page screenshot at the frame's width) or crop the screenshot to the frame; if it already is the whole page, pass --full-page (MCP fullPage) to compare it and report the missing rows",
            num(scale)
        )));
    }
    let render = render::pixmap(&prepared.export_svg_in(tree, area, scale, true))?;
    let (w, h) = (render.width().min(sw), render.height().min(sh));
    // Scored rows: the overlap, plus the frame rows a short full-page capture lacks.
    let full = if short { render.height() } else { h };
    // One CSS pixel; capped so a tiny node in a large screenshot cannot make each pixel's
    // neighbourhood search huge.
    let radius = scale.ceil().clamp(1.0, 4.0) as u32;
    // Ignored pixels neither differ nor count as compared.
    let mut skip = Vec::new();
    if !ignored.is_empty() {
        skip = vec![false; (w * full) as usize];
        let to_px =
            |v: f64, origin: f64, max: u32| ((v - origin) * scale).clamp(0.0, f64::from(max));
        for (_, r) in &ignored {
            let (x0, x1) = (
                to_px(r[0], area[0], w).floor() as u32,
                to_px(r[2], area[0], w).ceil() as u32,
            );
            let (y0, y1) = (
                to_px(r[1], area[1], full).floor() as u32,
                to_px(r[3], area[1], full).ceil() as u32,
            );
            for y in y0..y1 {
                skip[(y * w + x0) as usize..(y * w + x1) as usize].fill(true);
            }
        }
    }
    let skipped = skip.iter().filter(|&&s| s).count() as u64;
    let mut mask = differing(shot, &render, w, h, radius, threshold, &skip);
    // Missing rows all differ, unless ignored.
    mask.extend((w * h..w * full).map(|i| skip.get(i as usize) != Some(&true)));

    // Group differing pixels by grid cell: count and tight bounds per cell. Missing rows start
    // their own cell rows, so they form a region of their own.
    let cell = (CELL * scale).round().max(2.0) as u32;
    let overlap_rows = h.div_ceil(cell);
    let row = |y: u32| {
        if y < h {
            y / cell
        } else {
            overlap_rows + (y - h) / cell
        }
    };
    let (cols, rows) = (w.div_ceil(cell), overlap_rows + (full - h).div_ceil(cell));
    let mut cells: Vec<Cell> = vec![(0, u32::MAX, u32::MAX, 0, 0); (cols * rows) as usize];
    let mut total = 0u64;
    for y in 0..full {
        for x in 0..w {
            if mask[(y * w + x) as usize] {
                total += 1;
                let c = &mut cells[(row(y) * cols + x / cell) as usize];
                *c = (c.0 + 1, c.1.min(x), c.2.min(y), c.3.max(x), c.4.max(y));
            }
        }
    }
    let layers = layers(n, area, &opts.ignore);
    // A cell's tight bounds in page coordinates.
    let page = |c: &Cell| {
        [
            f64::from(c.1) / scale + area[0],
            f64::from(c.2) / scale + area[1],
            f64::from(c.3 + 1) / scale + area[0],
            f64::from(c.4 + 1) / scale + area[1],
        ]
    };
    let min_pixels = (MIN_REGION * scale * scale).ceil() as u64;
    let mut seen = vec![false; cells.len()];
    let mut regions = Vec::new();
    for start in 0..cells.len() {
        if seen[start] || cells[start].0 == 0 {
            continue;
        }
        // Flood the 8-connected cells that hold differing pixels. Each cell is blamed on the
        // smallest of the deepest layers under its differing pixels: a ring around a frame is
        // not blamed on everything inside it, nor an icon on the background rectangle under it.
        let (mut pixels, mut text, mut image) = (0u64, 0u64, 0u64);
        let mut b = (u32::MAX, u32::MAX, 0u32, 0u32);
        let mut weights: Vec<(usize, u64)> = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            let c = cells[i];
            pixels += c.0;
            b = (b.0.min(c.1), b.1.min(c.2), b.2.max(c.3), b.3.max(c.4));
            let under = attribute(&layers, page(&c));
            let missing = i as u32 / cols >= overlap_rows;
            if !missing && under.iter().any(|&k| layers[k].kind == "TEXT") {
                text += c.0;
            }
            // Only image fills under it: content such as a banner, not layout.
            if !missing && !under.is_empty() && under.iter().all(|&k| layers[k].image) {
                image += c.0;
            }
            let area = |k: &usize| {
                let r = layers[*k].rect;
                (r[2] - r[0]) * (r[3] - r[1])
            };
            if let Some(k) = under.iter().min_by(|a, b| area(a).total_cmp(&area(b))) {
                match weights.iter_mut().find(|w| w.0 == *k) {
                    Some(w) => w.1 += c.0,
                    None => weights.push((*k, c.0)),
                }
            }
            let (cx, cy) = ((i as u32 % cols) as i64, (i as u32 / cols) as i64);
            for (dx, dy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 || x >= cols as i64 || y >= rows as i64 {
                    continue;
                }
                if (y >= i64::from(overlap_rows)) != missing {
                    continue;
                }
                let j = (y as u32 * cols + x as u32) as usize;
                if !seen[j] && cells[j].0 > 0 {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        // Missing rows are a known height difference, never noise; the flood stays on one side.
        if pixels >= min_pixels || start as u32 / cols >= overlap_rows {
            weights.sort_by_key(|w| std::cmp::Reverse(w.1));
            weights.truncate(MAX_NODES);
            regions.push((pixels, text, image, b, weights));
        }
    }
    regions.sort_by_key(|r| std::cmp::Reverse(r.0));
    let compared = (u64::from(w) * u64::from(full) - skipped).max(1);
    let ratio = |p: u64| (p as f64 / compared as f64 * 1e5).round() / 1e5;
    let (mut text_pixels, mut image_pixels, mut other_regions) = (0u64, 0u64, 0usize);
    let mut listed = Vec::new();
    for (i, (pixels, text, image, (x0, y0, x1, y1), weights)) in regions.iter().enumerate() {
        text_pixels += text;
        image_pixels += image;
        // Mostly over text: font fallback and line breaks, not layout.
        let over_text = text * 10 >= pixels * 9;
        let over_image = image * 10 >= pixels * 9;
        other_regions += usize::from(!over_text);
        if i < limit {
            // Frame pixels, from the frame's top-left.
            let at = |v: u32| num(f64::from(v) / scale);
            listed.push(json!({
                "x": at(*x0), "y": at(*y0),
                "width": num(f64::from(x1 + 1 - x0) / scale), "height": num(f64::from(y1 + 1 - y0) / scale),
                "pixels": pixels,
                "text": over_text,
                "image": over_image,
                "nodes": weights.iter().map(|&(k, _)| json!({
                    "id": layers[k].id, "name": layers[k].name, "type": layers[k].kind,
                })).collect::<Vec<_>>(),
            }));
        }
    }
    let region_pixels: u64 = regions.iter().map(|r| r.0).sum();
    let mut warnings = Vec::new();
    crate::bundle::render_warnings(n, &mut warnings);
    let mut report = json!({
        "node": {"id": id, "name": n["name"], "type": n["type"]},
        "frame": {"width": num(fw), "height": num(fh)},
        "screenshot": {"width": sw, "height": sh},
        "scale": (scale * 1e4).round() / 1e4,
        "compared": {"width": w, "height": h},
        "threshold": threshold,
        "mismatch": ratio(total),
        "nonTextMismatch": ratio(region_pixels - text_pixels),
        "textMismatch": ratio(text_pixels),
        "nonImageMismatch": ratio(region_pixels - image_pixels),
        "imageMismatch": ratio(image_pixels),
        "regionCount": regions.len(),
        "nonTextRegions": other_regions,
        "regions": listed,
        "warnings": warnings,
    });
    if sh > h {
        report["uncompared"] = json!({"below": sh - h});
    }
    if full > h {
        report["uncompared"] = json!({"missing": full - h});
    }
    if !ignored.is_empty() {
        let nodes: Vec<Json> = ignored
            .iter()
            .map(|(l, _)| {
                let name: String = l["name"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(80)
                    .collect();
                json!({"id": l["id"], "name": name, "type": l["type"]})
            })
            .collect();
        report["ignored"] = json!({"nodes": nodes, "pixels": skipped});
    }
    let diff = opts
        .diff
        .then(|| diff_image(shot, &mask, &skip, w, full, &report["regions"], scale));
    Ok((report, diff))
}

/// Differing pixels in a grid cell and their bounds: count, min x, min y, max x, max y.
type Cell = (u64, u32, u32, u32, u32);

/// Rounds to two decimals for the report.
fn num(v: f64) -> f64 {
    (v * 100.0).round() / 100.0 + 0.0
}

/// Pixel `i` of premultiplied RGBA over white.
fn over_white(d: &[u8], i: usize) -> [u8; 3] {
    let a = 255 - d[i + 3];
    [d[i] + a, d[i + 1] + a, d[i + 2] + a]
}

fn close(a: [u8; 3], b: [u8; 3], threshold: u8) -> bool {
    (0..3).all(|k| a[k].abs_diff(b[k]) <= threshold)
}

/// Whether `p` matches some pixel of `img` within `radius` of (x, y).
#[allow(clippy::too_many_arguments)]
fn near(img: &Pixmap, p: [u8; 3], x: u32, y: u32, radius: u32, w: u32, h: u32, t: u8) -> bool {
    let d = img.data();
    let stride = img.width() as usize;
    for yy in y.saturating_sub(radius)..=(y + radius).min(h - 1) {
        for xx in x.saturating_sub(radius)..=(x + radius).min(w - 1) {
            if close(
                p,
                over_white(d, (yy as usize * stride + xx as usize) * 4),
                t,
            ) {
                return true;
            }
        }
    }
    false
}

/// Differing pixels over the top-left `w`x`h` of both images, row-major; pixels in `skip` are
/// not compared.
fn differing(
    a: &Pixmap,
    b: &Pixmap,
    w: u32,
    h: u32,
    radius: u32,
    t: u8,
    skip: &[bool],
) -> Vec<bool> {
    let mut mask = vec![false; (w * h) as usize];
    let (da, db) = (a.data(), b.data());
    for y in 0..h {
        for x in 0..w {
            if skip.get((y * w + x) as usize) == Some(&true) {
                continue;
            }
            let pa = over_white(da, (y as usize * a.width() as usize + x as usize) * 4);
            let pb = over_white(db, (y as usize * b.width() as usize + x as usize) * 4);
            mask[(y * w + x) as usize] = !close(pa, pb, t)
                && (!near(b, pa, x, y, radius, w, h, t) || !near(a, pb, x, y, radius, w, h, t));
        }
    }
    mask
}

/// A visible layer's box cut by its clipping ancestors, in page coordinates.
struct Layer {
    id: String,
    name: String,
    kind: String,
    /// Has a visible image fill.
    image: bool,
    rect: Rect,
    parent: Option<usize>,
}

/// The visible layers of `n` in pre-order, within `clip`, without the `ignore` subtrees.
fn layers(n: &Json, clip: Rect, ignore: &[String]) -> Vec<Layer> {
    // The tree passed `rest::build`'s depth check, so recursion is bounded.
    fn walk(n: &Json, clip: Rect, parent: Option<usize>, ignore: &[String], out: &mut Vec<Layer>) {
        if !render::visible(n)
            || n["id"]
                .as_str()
                .is_some_and(|id| ignore.iter().any(|i| i == id))
        {
            return;
        }
        let clips = n["clipsContent"] == true;
        let Some(rect) = render::layout_bounds(n).and_then(|b| render::intersect(b, clip)) else {
            // Outside the clip, an unclipped parent's children may still be drawn inside it.
            if !clips {
                for child in n["children"].as_array().into_iter().flatten() {
                    walk(child, clip, parent, ignore, out);
                }
            }
            return;
        };
        let index = out.len();
        out.push(Layer {
            id: n["id"].as_str().unwrap_or_default().to_owned(),
            // A text layer is often named after its whole text; keep the reply small.
            name: n["name"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
            kind: n["type"].as_str().unwrap_or_default().to_owned(),
            image: n["fills"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|f| f["type"] == "IMAGE" && render::visible(f)),
            rect,
            parent,
        });
        let inner = if clips { rect } else { clip };
        for child in n["children"].as_array().into_iter().flatten() {
            walk(child, inner, Some(index), ignore, out);
        }
    }
    let mut out = Vec::new();
    walk(n, clip, None, ignore, &mut out);
    out
}

/// The deepest layers overlapping `r`: none of their descendants does.
fn attribute(layers: &[Layer], r: Rect) -> Vec<usize> {
    let overlap =
        |k: usize| render::intersect(layers[k].rect, r).map(|o| (o[2] - o[0]) * (o[3] - o[1]));
    let hits: Vec<usize> = (0..layers.len())
        .filter(|&k| overlap(k).is_some_and(|a| a > 0.0))
        .collect();
    let mut covered = vec![false; layers.len()];
    for &k in &hits {
        let mut p = layers[k].parent;
        while let Some(i) = p {
            covered[i] = true;
            p = layers[i].parent;
        }
    }
    hits.into_iter().filter(|&k| !covered[k]).collect()
}

/// The screenshot washed out, differing pixels red, ignored pixels flat gray, and each listed
/// region outlined: red over layout, blue over text, purple over images.
fn diff_image(
    shot: &Pixmap,
    mask: &[bool],
    skip: &[bool],
    w: u32,
    h: u32,
    regions: &Json,
    scale: f64,
) -> Pixmap {
    let mut out = Pixmap::new(w, h).expect("compared area is non-empty");
    let d = shot.data();
    let o = out.data_mut();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            // Missing rows past the screenshot are all differing, so it is read only above them.
            let px = if mask[i] {
                [230, 0, 0]
            } else if skip.get(i) == Some(&true) {
                [200, 200, 200]
            } else {
                let p = over_white(d, (y as usize * shot.width() as usize + x as usize) * 4);
                let l = (u32::from(p[0]) * 3 + u32::from(p[1]) * 6 + u32::from(p[2])) / 10;
                let g = (255 - (255 - l) / 4) as u8;
                [g, g, g]
            };
            o[i * 4..i * 4 + 4].copy_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    for r in regions.as_array().into_iter().flatten() {
        let color = if r["text"] == true {
            [0, 90, 230]
        } else if r["image"] == true {
            [150, 0, 200]
        } else {
            [230, 0, 0]
        };
        let at = |k: &str| (r[k].as_f64().unwrap_or(0.0) * scale) as i64;
        let (x0, y0) = (at("x") - 1, at("y") - 1);
        let (x1, y1) = (x0 + at("width") + 1, y0 + at("height") + 1);
        let mut put = |x: i64, y: i64| {
            if (0..i64::from(w)).contains(&x) && (0..i64::from(h)).contains(&y) {
                let i = (y as usize * w as usize + x as usize) * 4;
                o[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        };
        for x in x0..=x1 {
            put(x, y0);
            put(x, y1);
        }
        for y in y0..=y1 {
            put(x0, y);
            put(x1, y);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    const FIG: &str = "examples/first-success/design.fig";

    /// Runs `f` with the public fixture's tree and its frame `0:1` rendered at `scale`.
    fn with_render(scale: f64, f: impl FnOnce(&Tree, Pixmap)) {
        let mut doc = Document::open(Path::new(FIG)).unwrap();
        let message = doc.take_message().unwrap();
        let tree = Tree::new(&message);
        let prepared = Prepared::new(&tree, Path::new(FIG), "0:1").unwrap();
        let area = render::layout_bounds(prepared.node()).unwrap();
        let shot = render::pixmap(&prepared.export_svg_in(&tree, area, scale, true)).unwrap();
        f(&tree, shot);
    }

    fn run(tree: &Tree, shot: &Pixmap, scale: Option<f64>) -> Result<Json, Box<dyn Error>> {
        let opts = Options {
            scale,
            ..Options::default()
        };
        compare(tree, Path::new(FIG), "0:1", shot, &opts).map(|r| r.0)
    }

    fn full_page() -> Options {
        Options {
            full_page: true,
            ..Options::default()
        }
    }

    fn ids(region: &Json) -> Vec<&str> {
        region["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn a_layer_inside_the_frame_counts_under_an_unclipped_parent_outside_it() {
        let bounds = |x: f64, w: f64| json!({"x": x, "y": 0.0, "width": w, "height": 50.0});
        let child =
            json!({"id": "1:3", "type": "RECTANGLE", "absoluteBoundingBox": bounds(10.0, 20.0)});
        let mut outside = json!({"id": "1:2", "type": "FRAME", "absoluteBoundingBox": bounds(150.0, 40.0), "children": [child]});
        let frame = |inner: &Json| json!({"id": "1:1", "type": "FRAME", "absoluteBoundingBox": bounds(0.0, 100.0), "children": [inner]});
        let ids = |frame: &Json| -> Vec<String> {
            layers(frame, [0.0, 0.0, 100.0, 50.0], &[])
                .into_iter()
                .map(|l| l.id)
                .collect()
        };
        assert_eq!(ids(&frame(&outside)), ["1:1", "1:3"]);
        outside["clipsContent"] = true.into();
        assert_eq!(ids(&frame(&outside)), ["1:1"]);
    }

    #[test]
    fn an_identical_render_has_no_differences_at_any_scale() {
        for scale in [1.0, 2.0] {
            with_render(scale, |tree, shot| {
                let report = run(tree, &shot, None).unwrap();
                assert_eq!(report["scale"], scale);
                assert_eq!(report["mismatch"], 0.0);
                assert_eq!(report["regionCount"], 0);
            });
        }
    }

    #[test]
    fn a_moved_shape_is_reported_over_its_layer() {
        with_render(1.0, |tree, mut shot| {
            // Move the triangle (248,116 24x24) 12 px left over the white card.
            let w = shot.width() as usize;
            let d = shot.data_mut();
            for y in 116..140 {
                for x in 236..272 {
                    let from = if x + 12 < 272 {
                        (y * w + x + 12) * 4
                    } else {
                        (y * w + 230) * 4
                    };
                    let px: [u8; 4] = d[from..from + 4].try_into().unwrap();
                    d[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&px);
                }
            }
            let report = run(tree, &shot, None).unwrap();
            let region = &report["regions"][0];
            assert!(ids(region).contains(&"0:6"), "{report}");
            assert_eq!(region["text"], false);
            assert_eq!(report["nonTextRegions"], 1);
        });
    }

    fn run_with(tree: &Tree, shot: &Pixmap, opts: Options) -> Result<Json, Box<dyn Error>> {
        compare(tree, Path::new(FIG), "0:1", shot, &opts).map(|r| r.0)
    }

    #[test]
    fn an_ignored_layer_leaves_its_box_out_and_is_listed() {
        with_render(1.0, |tree, mut shot| {
            // Invert the triangle's box (248,116 24x24), as content from other data would differ.
            let w = shot.width() as usize;
            let d = shot.data_mut();
            for y in 116..140 {
                for x in 248..272 {
                    let i = (y * w + x) * 4;
                    let inverted = [255 - d[i], 255 - d[i + 1], 255 - d[i + 2]];
                    d[i..i + 3].copy_from_slice(&inverted);
                }
            }
            let ignore = vec!["0:6".to_owned()];
            let report = run_with(
                tree,
                &shot,
                Options {
                    ignore,
                    ..Options::default()
                },
            )
            .unwrap();
            assert_eq!(
                [
                    &report["mismatch"],
                    &report["ignored"]["nodes"][0]["id"],
                    &report["ignored"]["pixels"]
                ],
                [&json!(0.0), &json!("0:6"), &json!(576)],
                "{report}"
            );
        });
    }

    #[test]
    fn ignore_refuses_the_node_itself_and_layers_outside_it() {
        with_render(1.0, |tree, shot| {
            for id in ["0:1", "9:9"] {
                let ignore = vec![id.to_owned()];
                let error = run_with(
                    tree,
                    &shot,
                    Options {
                        ignore,
                        ..Options::default()
                    },
                );
                assert!(
                    error
                        .unwrap_err()
                        .to_string()
                        .contains("not a layer inside")
                );
            }
        });
    }

    #[test]
    fn a_changed_image_fill_is_marked_image() {
        with_render(1.0, |tree, mut shot| {
            // Invert the image tile (32,32 32x32), as a banner from other data would differ.
            let w = shot.width() as usize;
            let d = shot.data_mut();
            for y in 32..64 {
                for x in 32..64 {
                    let i = (y * w + x) * 4;
                    let inverted = [255 - d[i], 255 - d[i + 1], 255 - d[i + 2]];
                    d[i..i + 3].copy_from_slice(&inverted);
                }
            }
            let report = run(tree, &shot, None).unwrap();
            assert_eq!(
                [&report["regions"][0]["image"], &report["nonImageMismatch"]],
                [&json!(true), &json!(0.0)],
                "{report}"
            );
        });
    }

    #[test]
    fn a_lower_threshold_sees_a_light_divider() {
        with_render(1.0, |tree, mut shot| {
            // A 1 px #e4edff divider across the white card, under the default threshold.
            let w = shot.width() as usize;
            let d = shot.data_mut();
            for x in 100..220 {
                let i = (150 * w + x) * 4;
                d[i..i + 4].copy_from_slice(&[0xe4, 0xed, 0xff, 255]);
            }
            let default = run(tree, &shot, None).unwrap();
            let low = run_with(
                tree,
                &shot,
                Options {
                    threshold: 16,
                    ..Options::default()
                },
            )
            .unwrap();
            assert_eq!(
                [&default["regionCount"], &low["regionCount"]],
                [&json!(0), &json!(1)],
                "{low}"
            );
        });
    }

    #[test]
    fn a_changed_text_is_marked_text() {
        with_render(1.0, |tree, mut shot| {
            // Invert the dark glyphs of READY (80,32 120x28).
            let w = shot.width() as usize;
            let d = shot.data_mut();
            for y in 32..60 {
                for x in 80..200 {
                    let i = (y * w + x) * 4;
                    if d[i] < 128 {
                        let inverted = [255 - d[i], 255 - d[i + 1], 255 - d[i + 2]];
                        d[i..i + 3].copy_from_slice(&inverted);
                    }
                }
            }
            let report = run(tree, &shot, None).unwrap();
            assert_eq!(ids(&report["regions"][0]), ["0:4"], "{report}");
            assert_eq!(report["regions"][0]["text"], true);
            assert_eq!(report["nonTextMismatch"], 0.0);
        });
    }

    #[test]
    fn a_taller_capture_compares_the_frame_and_reports_the_rest() {
        with_render(1.0, |tree, shot| {
            let mut tall = Pixmap::new(320, 260).unwrap();
            tall.fill(resvg::tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            let n = shot.data().len();
            tall.data_mut()[..n].copy_from_slice(shot.data());
            let report = run(tree, &tall, None).unwrap();
            assert_eq!(report["uncompared"]["below"], 80);
            assert_eq!(report["mismatch"], 0.0);
        });
    }

    #[test]
    fn a_short_full_page_capture_reports_the_missing_rows_as_a_region() {
        with_render(1.0, |tree, shot| {
            // The first 100 of the frame's 180 rows, as a page that came out shorter.
            let mut short = Pixmap::new(320, 100).unwrap();
            let n = short.data().len();
            short.data_mut().copy_from_slice(&shot.data()[..n]);
            let (report, diff) = compare(
                tree,
                Path::new(FIG),
                "0:1",
                &short,
                &Options {
                    diff: true,
                    full_page: true,
                    ..Options::default()
                },
            )
            .unwrap();
            assert_eq!(report["uncompared"]["missing"], 80);
            assert_eq!(report["mismatch"], 0.44444);
            assert_eq!(report["nonTextMismatch"], 0.44444);
            let region = &report["regions"][0];
            assert_eq!(
                [&region["y"], &region["height"], &region["text"]],
                [&json!(100.0), &json!(80.0), &json!(false)],
                "{report}"
            );
            assert!(!ids(region).is_empty(), "{report}");
            assert_eq!(diff.unwrap().height(), 180);
        });
    }

    #[test]
    fn a_missing_band_under_the_noise_floor_is_still_a_region() {
        // The 24 px triangle at scale 5: two missing rows are 240 px, under 12 * 5² = 300.
        let mut doc = Document::open(Path::new(FIG)).unwrap();
        let message = doc.take_message().unwrap();
        let tree = Tree::new(&message);
        let prepared = Prepared::new(&tree, Path::new(FIG), "0:6").unwrap();
        let area = render::layout_bounds(prepared.node()).unwrap();
        let full = render::pixmap(&prepared.export_svg_in(&tree, area, 5.0, true)).unwrap();
        let mut short = Pixmap::new(full.width(), full.height() - 2).unwrap();
        let n = short.data().len();
        short.data_mut().copy_from_slice(&full.data()[..n]);
        let report = compare(&tree, Path::new(FIG), "0:6", &short, &full_page())
            .unwrap()
            .0;
        assert_eq!(report["regionCount"], 1, "{report}");
    }

    #[test]
    fn a_full_page_capture_two_rows_short_reports_them() {
        with_render(1.0, |tree, shot| {
            // Within the viewport slack (2 px of 180), but a stated whole page lacks them.
            let mut short = Pixmap::new(320, 178).unwrap();
            let n = short.data().len();
            short.data_mut().copy_from_slice(&shot.data()[..n]);
            let report = compare(tree, Path::new(FIG), "0:1", &short, &full_page())
                .unwrap()
                .0;
            assert_eq!(report["uncompared"]["missing"], 2, "{report}");
        });
    }

    #[test]
    fn a_short_capture_or_a_width_off_scale_is_refused() {
        with_render(1.0, |tree, _| {
            let short = Pixmap::new(320, 100).unwrap();
            let error = run(tree, &short, None).unwrap_err().to_string();
            assert!(error.contains("--full-page"), "{error}");
            let wide = Pixmap::new(320, 180).unwrap();
            let error = run(tree, &wide, Some(2.0)).unwrap_err().to_string();
            assert!(error.contains("omit scale"), "{error}");
            let phone = Pixmap::new(160, 400).unwrap();
            let error = run(tree, &phone, None).unwrap_err().to_string();
            assert!(error.contains("narrower than node"), "{error}");
        });
    }

    #[test]
    fn decode_checks_the_format_and_declared_size_first() {
        assert!(decode(b"GIF89a").unwrap_err().to_string().contains("PNG"));
        // A header declaring 6000x6000 pixels with no image data behind it.
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(6000u32.to_be_bytes());
        png.extend(6000u32.to_be_bytes());
        png.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
        let error = decode(&png).unwrap_err().to_string();
        assert!(error.contains("over 25000000 pixels"), "{error}");
    }
}
