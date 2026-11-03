//! Bounded discovery output; full design data stays in `nodes` and `bundle`.

use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use figlens::expand::{plugin_type, subtree};
use figlens::{Tree, guid_key};
use serde_json::{Value, json};

const MAX_NODES: usize = 200;
const TEXT_CHARS: usize = 200;
/// Per-text cap with `fullText`; a longer text still sets `textTruncated`.
const FULL_TEXT_CHARS: usize = 10_000;

/// Opt-in extras for `inspect` and `inspect --flat`.
#[derive(Default)]
struct Detail {
    full_text: bool,
    /// Variable id -> name, set when solid fill colors are requested.
    colors: Option<HashMap<String, String>>,
}

impl Detail {
    fn new(tree: &Tree, document: &Value, full_text: bool, colors: bool) -> Self {
        Self {
            full_text,
            colors: colors.then(|| variable_names(tree, document)),
        }
    }

    fn text_chars(&self) -> usize {
        if self.full_text {
            FULL_TEXT_CHARS
        } else {
            TEXT_CHARS
        }
    }

    /// The bound color variable of a paint as `(id, name)`, name empty when not resolved.
    fn bound<'a>(&'a self, paint: &'a Value) -> Option<(&'a str, &'a str)> {
        let id = paint["boundVariables"]["color"]["id"].as_str()?;
        let name = self.colors.as_ref()?.get(id).map_or("", String::as_str);
        Some((id, name))
    }
}

/// `text` cut to `limit` characters, and whether anything was cut.
fn cut(text: &str, limit: usize) -> (String, bool) {
    let mut chars = text.chars();
    let kept = chars.by_ref().take(limit).collect();
    (kept, chars.next().is_some())
}

fn text_preview(summary: &mut Value, text: &str, limit: usize) {
    let (kept, truncated) = cut(text, limit);
    summary["characters"] = kept.into();
    if truncated {
        summary["textTruncated"] = true.into();
    }
}

/// An annotation with its plain `label` cut like a text preview and at most 20 `properties`;
/// `labelMarkdown` stays in nodes.
fn annotation_preview(annotation: &Value, limit: usize, truncated: &mut bool) -> Value {
    let mut preview = json!({});
    if let Some(label) = annotation["label"].as_str() {
        let (kept, cut) = cut(label, limit);
        preview["label"] = kept.into();
        if cut {
            preview["labelTruncated"] = true.into();
        }
    }
    if let Some(list) = annotation["properties"].as_array() {
        preview["properties"] = list.iter().take(20).cloned().collect();
        *truncated |= list.len() > 20;
    }
    if let Some(id) = annotation.get("categoryId") {
        preview["categoryId"] = id.clone();
    }
    preview
}

/// The response's `annotationCategories` that the kept annotations name, labels cut like text
/// previews; none when no kept annotation has a named category.
fn kept_categories(summary: &Value, table: &Value, limit: usize) -> Option<Value> {
    let table = table.as_object()?;
    let mut kept = serde_json::Map::new();
    let mut stack = vec![summary];
    while let Some(node) = stack.pop() {
        stack.extend(node["children"].as_array().into_iter().flatten());
        for id in node["annotations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| a["categoryId"].as_str())
        {
            let Some(category) = table.get(id).filter(|_| !kept.contains_key(id)) else {
                continue;
            };
            let mut category = category.clone();
            if let Some(label) = category["label"].as_str() {
                let (label, cut) = cut(label, limit);
                category["label"] = label.into();
                if cut {
                    category["labelTruncated"] = true.into();
                }
            }
            kept.insert(id.to_owned(), category);
        }
    }
    (!kept.is_empty()).then(|| kept.into())
}

/// Visible SOLID fills as `{hex, opacity?, variableId?, variableName?}`: the paint's own alpha
/// and opacity, not ancestor or node opacity.
fn solid_fills(node: &Value, detail: &Detail) -> Vec<Value> {
    node["fills"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["type"] == "SOLID" && figlens::render::visible(p))
        .map(|p| {
            let mut fill = json!({"hex": figlens::render::hex(&p["color"])});
            let a = p["color"]["a"].as_f64().unwrap_or(1.0) * p["opacity"].as_f64().unwrap_or(1.0);
            if a < 1.0 {
                fill["opacity"] = json!((a * 1000.0).round() / 1000.0);
            }
            if let Some((id, name)) = detail.bound(p) {
                fill["variableId"] = id.into();
                if !name.is_empty() {
                    fill["variableName"] = name.into();
                }
            }
            fill
        })
        .collect()
}

/// A stored node's local pre-rotation size, kept as f32 so output digits match `nodes`.
pub(super) fn local_size(node: &kiwi_schema::Value) -> Option<[f32; 2]> {
    let size = node.get("size")?;
    let axis = |k| size.get(k).map_or(0.0, |v| v.as_float());
    Some([axis("x"), axis("y")])
}

pub fn find(
    tree: &Tree,
    query: &str,
    root: &str,
    kind: Option<&str>,
    limit: usize,
) -> Result<Value, Box<dyn Error>> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Err("find needs a non-empty query".into());
    }
    if !(1..=MAX_NODES).contains(&limit) {
        return Err("--limit takes 1-200".into());
    }
    let mut results = Vec::new();
    let mut truncated = false;
    for id in subtree(tree, root) {
        let node = tree.nodes[&id];
        let node_type = plugin_type(node);
        if kind.is_some_and(|kind| kind != node_type) {
            continue;
        }
        let name = node.get("name").map(|v| v.as_string()).unwrap_or("");
        let text = node
            .get("textData")
            .and_then(|v| v.get("characters"))
            .map(|v| v.as_string())
            .unwrap_or("");
        let name_match = name.to_lowercase().contains(&query);
        let text_match = text.to_lowercase().contains(&query);
        if !name_match && !text_match {
            continue;
        }
        if results.len() == limit {
            truncated = true;
            break;
        }
        let mut path = Vec::new();
        let mut ancestor = Some(id.clone());
        while let Some(id) = ancestor {
            let Some(node) = tree.nodes.get(&id) else {
                break;
            };
            path.push(
                json!({"id": id, "name": node.get("name").map(|v| v.as_string()).unwrap_or("")}),
            );
            ancestor = node
                .get("parentIndex")
                .and_then(|p| p.get("guid"))
                .and_then(guid_key);
        }
        path.reverse();
        let mut summary = json!({
            "id": id, "type": node_type, "name": name, "path": path,
            "childCount": tree.children.get(&id).map_or(0, Vec::len),
            "matchedOn": {"name": name_match, "text": text_match},
        });
        if let Some([width, height]) = local_size(node) {
            summary["size"] = json!({ "width": width, "height": height });
        }
        if node_type == "TEXT" {
            text_preview(&mut summary, text, TEXT_CHARS);
        }
        results.push(summary);
    }
    Ok(json!({"root": root, "results": results, "truncated": truncated}))
}

#[allow(clippy::too_many_arguments)]
pub fn inspect(
    tree: &Tree,
    id: &str,
    depth: usize,
    fig: &Path,
    images: bool,
    states: bool,
    compare_variant: Option<&str>,
    compare: Option<&str>,
    full_text: bool,
    colors: bool,
) -> Result<Value, Box<dyn Error>> {
    if depth > 10 {
        return Err("--depth takes 0-10 for inspect".into());
    }
    if compare_variant.is_some() && compare.is_some() {
        return Err("--compare-variant and --compare cannot be combined".into());
    }
    let general = compare
        .map(|other| figlens::states::compare_nodes(tree, id, other, fig))
        .transpose()?;
    let comparison = compare_variant
        .map(|other| figlens::states::compare_variants(tree, id, other, fig))
        .transpose()?;
    let doc = if images {
        figlens::rest::resolve_render_nodes(tree, id)?
    } else {
        figlens::rest::resolve_nodes(tree, id)?
    };
    let detail = Detail::new(tree, &doc["document"], full_text, colors);
    let mut result = inspect_document(&doc["document"], depth, &detail);
    if let Some(semantics) = doc["_figlens"].get("semantics") {
        result["_figlens"] = json!({"semantics":semantics});
    }
    if let Some(diagnostics) = doc["_figlens"].get("handoffDiagnostics") {
        result["handoffDiagnostics"] = diagnostics.clone();
    }
    if let Some(categories) = kept_categories(
        &result["node"],
        &doc["annotationCategories"],
        detail.text_chars(),
    ) {
        result["annotationCategories"] = categories;
    }
    if images {
        result["images"] = figlens::images::inspect(fig, &doc["document"])?;
    }
    if states {
        result["states"] = figlens::states::inspect(tree, &doc["document"], fig);
    }
    if let Some(comparison) = comparison {
        result["variantComparison"] = comparison;
    }
    if let Some(comparison) = general {
        result["comparison"] = comparison;
    }
    Ok(result)
}

/// `inspect --flat`: one line per node, within the same depth and node limits as `inspect`.
/// Returns the lines and whether nodes were left out.
pub fn inspect_flat(
    tree: &Tree,
    id: &str,
    depth: usize,
    full_text: bool,
    colors: bool,
) -> Result<(String, bool), Box<dyn Error>> {
    if depth > 10 {
        return Err("--depth takes 0-10 for inspect".into());
    }
    let doc = figlens::rest::resolve_nodes(tree, id)?;
    let detail = Detail::new(tree, &doc["document"], full_text, colors);
    let root = &doc["document"]["absoluteBoundingBox"];
    let origin = (
        root["x"].as_f64().unwrap_or(0.0),
        root["y"].as_f64().unwrap_or(0.0),
    );
    let (mut out, mut remaining, mut truncated) = (String::new(), MAX_NODES, false);
    flat_lines(
        &mut out,
        &doc["document"],
        origin,
        0,
        depth,
        &detail,
        &mut remaining,
        &mut truncated,
    );
    Ok((out, truncated))
}

/// Names of the variables the subtree binds, by REST id.
fn variable_names(tree: &Tree, document: &Value) -> HashMap<String, String> {
    figlens::variables::variables(tree, Some(document))["variables"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| Some((v["id"].as_str()?.to_owned(), v["name"].as_str()?.to_owned())))
        .collect()
}

/// `id TYPE "name" @x,y WxH` (position relative to the inspected root), then whichever of
/// hidden, rotation, component, Ready for dev status, annotation count, text, font, fill, stroke,
/// radius, effects, auto-layout,
/// alignment, sizing and opacity apply.
/// `children=N` marks children cut by depth; a `… N more` line marks those cut by the node limit.
#[allow(clippy::too_many_arguments)]
fn flat_lines(
    out: &mut String,
    node: &Value,
    origin: (f64, f64),
    level: usize,
    depth: usize,
    detail: &Detail,
    remaining: &mut usize,
    truncated: &mut bool,
) {
    use figlens::render::{hex, num};
    use std::fmt::Write as _;
    *remaining -= 1;
    let indent = "  ".repeat(level);
    let quote = |s: &str| serde_json::to_string(s).unwrap();
    let mut line = format!(
        "{indent}{} {} {}",
        node["id"].as_str().unwrap_or(""),
        node["type"].as_str().unwrap_or(""),
        quote(node["name"].as_str().unwrap_or(""))
    );
    let mut add = |token: String| {
        line.push(' ');
        line.push_str(&token);
    };
    let b = &node["absoluteBoundingBox"];
    if let (Some(x), Some(y), Some(w), Some(h)) = (
        b["x"].as_f64(),
        b["y"].as_f64(),
        b["width"].as_f64(),
        b["height"].as_f64(),
    ) {
        add(format!(
            "@{},{} {}x{}",
            num(x - origin.0),
            num(y - origin.1),
            num(w),
            num(h)
        ));
    }
    if node["visible"] == false {
        add("hidden".into());
    }
    if let Some(r) = node["rotation"].as_f64() {
        add(format!("rot={}deg", num(r.to_degrees())));
    }
    if let Some(main) = node["componentId"].as_str() {
        add(format!("component={main}"));
    }
    if let Some(status) = node["devStatus"]["type"].as_str() {
        add(format!("devStatus={status}"));
    }
    // Only a count: the notes themselves are in inspect's JSON form and nodes.
    if let Some(list) = node["annotations"].as_array() {
        add(format!("annotations={}", list.len()));
    }
    if let Some(text) = node["characters"].as_str() {
        let limit = detail.text_chars();
        add(quote(&text.chars().take(limit).collect::<String>()));
        if text.chars().count() > limit {
            add("textTruncated".into());
        }
        let style = &node["style"];
        let n = |k: &str| style[k].as_f64().map_or("?".into(), num);
        add(format!(
            "font={} {}/{}/{}",
            quote(style["fontFamily"].as_str().unwrap_or("?")),
            n("fontSize"),
            n("fontWeight"),
            n("lineHeightPx")
        ));
    }
    // A color as hex, with /alpha below 1.
    let color = |c: &Value, opacity: f64| {
        let a = c["a"].as_f64().unwrap_or(1.0) * opacity;
        let alpha = if a < 1.0 {
            format!("/{}", num(a))
        } else {
            String::new()
        };
        format!("{}{alpha}", hex(c))
    };
    // Visible paints: solid colors as hex, others by type.
    let paints = |list: &Value| {
        list.as_array()
            .into_iter()
            .flatten()
            .filter(|p| figlens::render::visible(p))
            .map(|p| match p["type"].as_str() {
                Some("SOLID") => {
                    let var = match detail.bound(p) {
                        Some((id, "")) => format!("${id}"),
                        Some((_, name)) => format!("${name}"),
                        None => String::new(),
                    };
                    format!(
                        "{}{var}",
                        color(&p["color"], p["opacity"].as_f64().unwrap_or(1.0))
                    )
                }
                kind => kind.unwrap_or("?").to_owned(),
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let fill = paints(&node["fills"]);
    if !fill.is_empty() {
        add(format!("fill={fill}"));
    }
    let stroke = paints(&node["strokes"]);
    // Per-side weights win: `strokeWeight` can be 0 when only some sides are set.
    let sides = &node["individualStrokeWeights"];
    let weights: Vec<f64> = if sides.is_object() {
        ["top", "right", "bottom", "left"]
            .iter()
            .map(|s| sides[s].as_f64().unwrap_or(0.0))
            .collect()
    } else {
        vec![node["strokeWeight"].as_f64().unwrap_or(0.0)]
    };
    if !stroke.is_empty() && weights.iter().any(|w| *w > 0.0) {
        let weights: Vec<_> = weights.into_iter().map(num).collect();
        add(format!("stroke={stroke}/{}", weights.join(",")));
    }
    if let Some(radii) = node["rectangleCornerRadii"].as_array() {
        let radii: Vec<_> = radii
            .iter()
            .map(|r| num(r.as_f64().unwrap_or(0.0)))
            .collect();
        add(format!("r={}", radii.join(",")));
    } else if let Some(r) = node["cornerRadius"].as_f64() {
        add(format!("r={}", num(r)));
    }
    // Visible effects: shadow=x,y,blur[,spread],color; blurs by radius.
    for e in node["effects"].as_array().into_iter().flatten() {
        if e["visible"] == false {
            continue;
        }
        let n = |k: &Value| num(k.as_f64().unwrap_or(0.0));
        let kind = e["type"].as_str().unwrap_or("?");
        add(match kind {
            "DROP_SHADOW" | "INNER_SHADOW" => {
                let spread = e["spread"]
                    .as_f64()
                    .map_or(String::new(), |s| format!(",{}", num(s)));
                format!(
                    "{}={},{},{}{spread},{}",
                    if kind == "DROP_SHADOW" {
                        "shadow"
                    } else {
                        "inner-shadow"
                    },
                    n(&e["offset"]["x"]),
                    n(&e["offset"]["y"]),
                    n(&e["radius"]),
                    color(&e["color"], 1.0)
                )
            }
            "LAYER_BLUR" => format!("blur={}", n(&e["radius"])),
            "BACKGROUND_BLUR" => format!("bg-blur={}", n(&e["radius"])),
            other => other.to_lowercase(),
        });
    }
    if let Some(mode) = node["layoutMode"].as_str() {
        let n = |k: &str| num(node[k].as_f64().unwrap_or(0.0));
        let gap = match mode {
            "GRID" => {
                // Track sizes as stored: FLEX in fractions, FIXED in px, HUG, min..max when the
                // two differ; `?` when a track has no stored size.
                fn size(t: &Value) -> String {
                    match (t["type"].as_str(), t["value"].as_f64()) {
                        (Some("FLEX"), v) => format!("flex{}", v.map(num).unwrap_or_default()),
                        (Some("FIXED"), Some(v)) => num(v),
                        (Some("HUG"), _) => "hug".into(),
                        _ if t.get("min").is_some() => {
                            format!("{}..{}", size(&t["min"]), size(&t["max"]))
                        }
                        _ => "?".into(),
                    }
                }
                let mut out = String::new();
                if let (Some(r), Some(c)) = (
                    node["gridRowCount"].as_u64(),
                    node["gridColumnCount"].as_u64(),
                ) {
                    out = format!(" {r}x{c}");
                }
                out += &format!(" gap={},{}", n("gridRowGap"), n("gridColumnGap"));
                for (axis, label) in [("rows", "rows"), ("columns", "cols")] {
                    if let Some(list) = node["_figlens"]["gridTracks"][axis].as_array() {
                        let list: Vec<_> = list.iter().map(size).collect();
                        out += &format!(" {label}={}", list.join(","));
                    }
                }
                out
            }
            _ => format!(" gap={}", n("itemSpacing")),
        };
        add(format!(
            "{}{gap} pad={},{},{},{}",
            match mode {
                "HORIZONTAL" => "H",
                "VERTICAL" => "V",
                other => other,
            },
            n("paddingTop"),
            n("paddingRight"),
            n("paddingBottom"),
            n("paddingLeft")
        ));
    }
    // REST leaves out MIN alignment and HUG sizing, so only other values print.
    let (main, cross) = (
        &node["primaryAxisAlignItems"],
        &node["counterAxisAlignItems"],
    );
    if !main.is_null() || !cross.is_null() {
        let a = |v: &Value| v.as_str().unwrap_or("MIN").to_lowercase().replace('_', "-");
        add(format!("align={}/{}", a(main), a(cross)));
    }
    let (w, h) = (
        &node["layoutSizingHorizontal"],
        &node["layoutSizingVertical"],
    );
    if [w, h].iter().any(|v| v.is_string() && *v != "HUG") {
        let s = |v: &Value| v.as_str().unwrap_or("?").to_lowercase();
        add(format!("size={}/{}", s(w), s(h)));
    }
    let span = |k: &str| node[k].as_u64().unwrap_or(1);
    if (span("gridRowSpan"), span("gridColumnSpan")) != (1, 1) {
        add(format!(
            "span={},{}",
            span("gridRowSpan"),
            span("gridColumnSpan")
        ));
    }
    if derived_spacing(node) {
        add("spacing=derived".into());
    }
    if let Some(o) = node["opacity"].as_f64() {
        add(format!("opacity={}", num(o)));
    }
    let children = node["children"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if depth == 0 && !children.is_empty() {
        add(format!("children={}", children.len()));
        *truncated = true;
    }
    writeln!(out, "{line}").unwrap();
    if depth == 0 {
        return;
    }
    for (i, child) in children.iter().enumerate() {
        if *remaining == 0 {
            writeln!(out, "{indent}  … {} more", children.len() - i).unwrap();
            *truncated = true;
            break;
        }
        flat_lines(
            out,
            child,
            origin,
            level + 1,
            depth - 1,
            detail,
            remaining,
            truncated,
        );
    }
}

fn inspect_document(document: &Value, depth: usize, detail: &Detail) -> Value {
    let mut remaining = MAX_NODES;
    let mut truncated = false;
    let node = summarize(document, depth, detail, &mut remaining, &mut truncated);
    json!({"node": node, "truncated": truncated})
}

/// Without auto-layout, gaps between children are coordinate differences, not a rule.
/// Boolean operands and pages are not laid-out children. REST omits layoutMode NONE.
fn derived_spacing(node: &Value) -> bool {
    node["children"].as_array().is_some_and(|c| !c.is_empty())
        && node.get("layoutMode").is_none()
        && !matches!(
            node["type"].as_str(),
            Some("BOOLEAN_OPERATION" | "DOCUMENT")
        )
}

fn summarize(
    node: &Value,
    depth: usize,
    detail: &Detail,
    remaining: &mut usize,
    truncated: &mut bool,
) -> Value {
    *remaining -= 1;
    let mut summary = json!({});
    for key in [
        "id",
        "type",
        "name",
        "visible",
        "absoluteBoundingBox",
        "rotation",
        "componentId",
        "layoutMode",
        "layoutWrap",
        "layoutPositioning",
        "constraints",
        "layoutGrow",
        "layoutAlign",
        "layoutSizingHorizontal",
        "layoutSizingVertical",
        "itemSpacing",
        "counterAxisSpacing",
        "gridRowCount",
        "gridColumnCount",
        "gridRowGap",
        "gridColumnGap",
        "gridRowSpan",
        "gridColumnSpan",
        "gridChildHorizontalAlign",
        "gridChildVerticalAlign",
        "paddingLeft",
        "paddingRight",
        "paddingTop",
        "paddingBottom",
        "primaryAxisAlignItems",
        "counterAxisAlignItems",
        "individualStrokeWeights",
    ] {
        if let Some(value) = node.get(key) {
            summary[key] = value.clone();
        }
    }
    if let Some(semantics) = node["_figlens"]["semantics"].as_object() {
        let mut evidence = serde_json::Map::new();
        for (key, value) in semantics {
            if key != "componentPropertyDefinitions" {
                evidence.insert(key.clone(), value.clone());
            }
        }
        if let Some(definitions) = semantics.get("componentPropertyDefinitions") {
            let mut compact = json!({"source":definitions["source"], "state":definitions["state"]});
            if let Some(entries) = definitions["entries"].as_array() {
                compact["entries"] = entries.iter().take(20).cloned().collect();
                compact["count"] = json!(entries.len());
                compact["truncated"] = json!(entries.len() > 20);
                *truncated |= entries.len() > 20;
            }
            evidence.insert("componentPropertyDefinitions".into(), compact);
        }
        summary["_figlens"] = json!({"semantics":evidence});
    }
    if let Some(tracks) = node["_figlens"].get("gridTracks") {
        summary["_figlens"]["gridTracks"] = tracks.clone();
    }
    if let Some(definitions) = node["componentPropertyDefinitions"].as_object() {
        let compact: serde_json::Map<String, Value> = definitions
            .iter()
            .take(20)
            .map(|(name, definition)| (name.clone(), json!({"type":definition["type"]})))
            .collect();
        summary["componentPropertyDefinitions"] = compact.into();
        summary["componentPropertyDefinitionCount"] = json!(definitions.len());
        *truncated |= definitions.len() > 20;
    }
    if let Some(items) = node["_figlens"]["renderDiagnostics"].as_array() {
        if summary.get("_figlens").is_none() {
            summary["_figlens"] = json!({});
        }
        summary["_figlens"]["renderDiagnostics"] = json!(items.iter().take(20).collect::<Vec<_>>());
        summary["_figlens"]["renderDiagnosticsTruncated"] = json!(items.len() > 20);
        *truncated |= items.len() > 20;
    }
    if let Some(text) = node["characters"].as_str() {
        text_preview(&mut summary, text, detail.text_chars());
    }
    if let Some(status) = node.get("devStatus") {
        summary["devStatus"] = json!({"type": status["type"]});
        if let Some(description) = status["description"].as_str() {
            let (kept, cut) = cut(description, detail.text_chars());
            summary["devStatus"]["description"] = kept.into();
            if cut {
                summary["devStatus"]["descriptionTruncated"] = true.into();
            }
        }
    }
    if let Some(list) = node["annotations"].as_array() {
        summary["annotations"] = list
            .iter()
            .take(20)
            .map(|a| annotation_preview(a, detail.text_chars(), truncated))
            .collect();
        *truncated |= list.len() > 20;
    }
    if detail.colors.is_some() {
        let fills = solid_fills(node, detail);
        if !fills.is_empty() {
            summary["solidFills"] = fills.into();
        }
    }
    if let Some(style) = node["style"].as_object() {
        for key in [
            "fontFamily",
            "fontWeight",
            "fontSize",
            "lineHeightPx",
            "letterSpacing",
            "textAlignHorizontal",
            "textAlignVertical",
            "textAutoResize",
            "textTruncation",
            "maxLines",
            "textDecoration",
        ] {
            if let Some(value) = style.get(key) {
                if summary.get("style").is_none() {
                    summary["style"] = json!({});
                }
                summary["style"][key] = value.clone();
            }
        }
    }
    let children = node["children"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    summary["childCount"] = children.len().into();
    if derived_spacing(node) {
        summary["childSpacing"] = "derived".into();
    }
    if depth > 0 && !children.is_empty() {
        let mut included = Vec::new();
        for child in children {
            if *remaining == 0 {
                break;
            }
            included.push(summarize(child, depth - 1, detail, remaining, truncated));
        }
        if included.len() < children.len() {
            *truncated = true;
        }
        summary["children"] = included.into();
    } else if !children.is_empty() {
        *truncated = true;
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiwi_schema::Value as Kiwi;

    #[test]
    fn compact_typography_preserves_recorded_resize_truncation_and_decoration() {
        let style = json!({"textAutoResize":"HEIGHT","textTruncation":"ENDING","maxLines":2,"textDecoration":"UNDERLINE"});
        let result = inspect_document(
            &json!({"id":"1:1","type":"TEXT","characters":"Title","style":style}),
            0,
            &Detail::default(),
        );
        assert_eq!(result["node"]["style"], style);
        let absent = inspect_document(
            &json!({"id":"1:2","type":"TEXT","characters":"Title"}),
            0,
            &Detail::default(),
        );
        assert!(absent["node"].get("style").is_none());
    }

    #[test]
    fn inspect_bounds_annotations_and_names_only_kept_categories() {
        let long = "x".repeat(250);
        let note = json!({"label": long, "labelMarkdown": long, "categoryId": "1:2",
                          "properties": vec![json!({"type": "fills"}); 21]});
        let mut notes = vec![json!({"label": "first", "categoryId": "1:3"})];
        notes.extend(vec![note; 20]);
        let node = json!({"id": "1:1", "annotations": notes,
                          "devStatus": {"type": "READY_FOR_DEV", "description": long}});
        let result = inspect_document(&node, 0, &Detail::default());
        let summary = &result["node"];
        assert_eq!(result["truncated"], true);
        assert_eq!(summary["annotations"].as_array().unwrap().len(), 20);
        let preview = &summary["annotations"][1];
        assert_eq!(preview["label"].as_str().unwrap().len(), 200);
        assert_eq!(preview["labelTruncated"], true);
        assert!(preview.get("labelMarkdown").is_none());
        assert_eq!(preview["properties"].as_array().unwrap().len(), 20);
        assert_eq!(
            summary["devStatus"]["description"].as_str().unwrap().len(),
            200
        );
        assert_eq!(summary["devStatus"]["descriptionTruncated"], true);
        let table = json!({"1:2": {"label": long, "isPreset": false}, "1:3": {"label": "Kept", "isPreset": true},
                           "1:4": {"label": "Unused", "isPreset": true}});
        let kept = kept_categories(summary, &table, 200).unwrap();
        assert_eq!(kept["1:3"], json!({"label": "Kept", "isPreset": true}));
        assert_eq!(kept["1:2"]["labelTruncated"], true);
        assert!(kept.get("1:4").is_none());
    }

    #[test]
    fn inspect_bounds_definition_summaries_and_keeps_diagnostics() {
        let definitions: serde_json::Map<String, Value> = (0..21)
            .map(|i| {
                (
                    format!("Label#{i}"),
                    json!({"type":"TEXT","defaultValue":"full default"}),
                )
            })
            .collect();
        let node = json!({"id":"1:1","individualStrokeWeights":{"top":0,"right":0,"bottom":3,"left":0},"componentPropertyDefinitions":definitions,"_figlens":{"semantics":{"componentPropertyDefinitions":{"source":"componentPropDefs","state":"recorded","entries":vec![json!({"state":"recorded"});21]}},"renderDiagnostics":[{"state":"skipped"}]}});
        let result = inspect_document(&node, 0, &Detail::default());
        let summary = &result["node"];
        assert_eq!(result["truncated"], true);
        assert_eq!(summary["componentPropertyDefinitionCount"], 21);
        assert_eq!(summary["individualStrokeWeights"]["bottom"], 3);
        assert_eq!(
            summary["componentPropertyDefinitions"]
                .as_object()
                .unwrap()
                .len(),
            20
        );
        assert!(
            summary["componentPropertyDefinitions"]
                .as_object()
                .unwrap()
                .values()
                .all(|v| v.get("defaultValue").is_none())
        );
        let evidence = &summary["_figlens"]["semantics"]["componentPropertyDefinitions"];
        assert_eq!(evidence["count"], 21);
        assert_eq!(evidence["entries"].as_array().unwrap().len(), 20);
        assert_eq!(
            summary["_figlens"]["renderDiagnostics"][0]["state"],
            "skipped"
        );
    }

    #[test]
    fn inspect_marks_spacing_derived_only_under_parents_without_auto_layout() {
        let child = json!({"id":"1:2","type":"RECTANGLE"});
        let spacing = |parent: Value| {
            inspect_document(&parent, 0, &Detail::default())["node"]["childSpacing"].clone()
        };
        assert_eq!(
            spacing(json!({"id":"1:1","type":"FRAME","children":[child]})),
            "derived"
        );
        assert_eq!(
            spacing(json!({"id":"1:1","type":"GROUP","children":[child]})),
            "derived"
        );
        assert_eq!(
            spacing(
                json!({"id":"1:1","type":"FRAME","layoutMode":"VERTICAL","itemSpacing":8,"children":[child]})
            ),
            Value::Null
        );
        assert_eq!(
            spacing(json!({"id":"1:1","type":"BOOLEAN_OPERATION","children":[child]})),
            Value::Null
        );
        assert_eq!(
            spacing(json!({"id":"0:0","type":"DOCUMENT","children":[child]})),
            Value::Null
        );
        assert_eq!(spacing(json!({"id":"1:1","type":"FRAME"})), Value::Null);
    }

    #[test]
    fn flat_grid_lines_show_counts_gaps_and_stored_track_sizes() {
        let node = json!({"id":"1:1","type":"FRAME","name":"g","layoutMode":"GRID","paddingTop":4,
            "gridRowCount":2,"gridColumnCount":3,"gridRowGap":8,"gridRowSpan":2,
            "_figlens":{"gridTracks":{"rows":[{"type":"HUG"},{"min":{"type":"FIXED","value":40},"max":{"type":"FLEX","value":2}}],
                "columns":[{"type":"FLEX","value":1},{"type":"FLEX"},null]}}});
        let (mut out, mut truncated) = (String::new(), false);
        flat_lines(
            &mut out,
            &node,
            (0.0, 0.0),
            0,
            0,
            &Detail::default(),
            &mut 1,
            &mut truncated,
        );
        assert_eq!(
            out,
            "1:1 FRAME \"g\" GRID 2x3 gap=8,0 rows=hug,40..flex2 cols=flex1,flex,? pad=4,0,0,0 span=2,1\n"
        );
    }

    #[test]
    fn flat_lines_show_alignment_sizing_and_visible_effects() {
        let node = json!({"id":"1:1","type":"FRAME","name":"bar","layoutMode":"HORIZONTAL","itemSpacing":20,
            "primaryAxisAlignItems":"SPACE_BETWEEN","counterAxisAlignItems":"CENTER",
            "layoutSizingHorizontal":"FIXED","layoutSizingVertical":"HUG",
            "effects":[
                {"type":"DROP_SHADOW","visible":true,"radius":24,"offset":{"x":0,"y":8},"color":{"r":0.1176,"g":0.3373,"b":0.902,"a":0.1}},
                {"type":"INNER_SHADOW","visible":true,"radius":2,"spread":1,"offset":{"x":0,"y":1},"color":{"r":0,"g":0,"b":0,"a":1}},
                {"type":"LAYER_BLUR","visible":false,"radius":4},
                {"type":"BACKGROUND_BLUR","visible":true,"radius":12}],
            "children":[{"id":"1:2","type":"TEXT","name":"t","layoutSizingHorizontal":"HUG","layoutSizingVertical":"HUG",
                "counterAxisAlignItems":null}]});
        let (mut out, mut truncated) = (String::new(), false);
        flat_lines(
            &mut out,
            &node,
            (0.0, 0.0),
            0,
            1,
            &Detail::default(),
            &mut 3,
            &mut truncated,
        );
        assert_eq!(
            out,
            concat!(
                "1:1 FRAME \"bar\" shadow=0,8,24,#1e56e6/0.1 inner-shadow=0,1,2,1,#000000 bg-blur=12 ",
                "H gap=20 pad=0,0,0,0 align=space-between/center size=fixed/hug\n",
                "  1:2 TEXT \"t\"\n",
            )
        );
    }

    #[test]
    fn flat_lines_are_relative_to_the_root_and_mark_omitted_children() {
        let b = |x, y, w, h| json!({"x":x,"y":y,"width":w,"height":h});
        let leaf = |id: &str| json!({"id":id,"type":"RECTANGLE","name":"r","absoluteBoundingBox":b(110,60,4,4)});
        let node = json!({"id":"1:1","type":"FRAME","name":"Card \"A\"","absoluteBoundingBox":b(100,50,280,111),
        "fills":[{"type":"SOLID","color":{"r":1,"g":1,"b":1,"a":1},"opacity":0.5},{"type":"IMAGE","visible":false},{"type":"SOLID","color":{"r":0,"g":0,"b":0,"a":1},"opacity":0}],
        "strokes":[{"type":"SOLID","color":{"r":0,"g":0,"b":0,"a":1}}],"strokeWeight":0,
        "individualStrokeWeights":{"top":0,"right":0,"bottom":1,"left":0},"cornerRadius":8,
        "layoutMode":"VERTICAL","itemSpacing":10,"paddingTop":1,"paddingRight":2,"paddingBottom":3,"paddingLeft":4,
        "children":[
            {"id":"1:2","type":"TEXT","name":"Title","visible":false,"absoluteBoundingBox":b(112,60,56,21),"characters":"Cancel\nnow",
                "style":{"fontFamily":"Inter","fontSize":14,"fontWeight":400,"lineHeightPx":21},"children":[leaf("1:5")]},
            leaf("1:3"), leaf("1:4"),
        ]});
        let (mut out, mut truncated) = (String::new(), false);
        flat_lines(
            &mut out,
            &node,
            (100.0, 50.0),
            0,
            1,
            &Detail::default(),
            &mut 3,
            &mut truncated,
        );
        assert_eq!(
            out,
            concat!(
                "1:1 FRAME \"Card \\\"A\\\"\" @0,0 280x111 fill=#ffffff/0.5 stroke=#000000/0,0,1,0 r=8 V gap=10 pad=1,2,3,4\n",
                "  1:2 TEXT \"Title\" @12,10 56x21 hidden \"Cancel\\nnow\" font=\"Inter\" 14/400/21 spacing=derived children=1\n",
                "  1:3 RECTANGLE \"r\" @10,10 4x4\n",
                "  … 1 more\n",
            )
        );
        assert!(truncated);
    }

    #[test]
    fn inspect_bounds_render_diagnostics_and_marks_truncation() {
        let node = json!({"id":"1:1", "_figlens":{"renderDiagnostics":vec![json!({"state":"skipped"});21]}});
        let mut truncated = false;
        let summary = summarize(&node, 1, &Detail::default(), &mut 200, &mut truncated);
        assert_eq!(
            summary["_figlens"]["renderDiagnostics"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        assert_eq!(summary["_figlens"]["renderDiagnosticsTruncated"], true);
        assert!(truncated);
    }

    #[test]
    fn full_text_and_solid_fills_are_opt_in() {
        let long = "x".repeat(TEXT_CHARS + 1);
        let fill = json!({"type":"SOLID","color":{"r":1.0,"g":0.0,"b":0.0,"a":1.0},"opacity":0.5,
            "boundVariables":{"color":{"type":"VARIABLE_ALIAS","id":"VariableID:1:2"}}});
        let hidden =
            json!({"type":"SOLID","visible":false,"color":{"r":0.0,"g":0.0,"b":0.0,"a":1.0}});
        let gradient = json!({"type":"GRADIENT_LINEAR"});
        let node =
            json!({"id":"1:1","type":"TEXT","characters":long,"fills":[fill,hidden,gradient]});
        let plain = inspect_document(&node, 0, &Detail::default())["node"].clone();
        assert_eq!(plain["textTruncated"], true);
        assert!(plain.get("solidFills").is_none());
        let names = HashMap::from([("VariableID:1:2".to_owned(), "Brand/Red".to_owned())]);
        let detail = Detail {
            full_text: true,
            colors: Some(names),
        };
        let full = inspect_document(&node, 0, &detail)["node"].clone();
        assert_eq!(full["characters"].as_str().unwrap().len(), TEXT_CHARS + 1);
        assert!(full.get("textTruncated").is_none());
        assert_eq!(
            full["solidFills"],
            json!([{"hex":"#ff0000","opacity":0.5,"variableId":"VariableID:1:2","variableName":"Brand/Red"}])
        );
        let (mut out, mut truncated) = (String::new(), false);
        flat_lines(
            &mut out,
            &node,
            (0.0, 0.0),
            0,
            0,
            &detail,
            &mut 1,
            &mut truncated,
        );
        assert!(
            out.contains("fill=#ff0000/0.5$Brand/Red,GRADIENT_LINEAR"),
            "{out}"
        );
        assert!(out.contains(&long), "{out}");
    }

    fn obj(fields: Vec<(&'static str, Kiwi<'static>)>) -> Kiwi<'static> {
        Kiwi::Object("Test", fields.into_iter().collect())
    }

    fn guid(id: u32) -> Kiwi<'static> {
        obj(vec![
            ("sessionID", Kiwi::UInt(0)),
            ("localID", Kiwi::UInt(id)),
        ])
    }

    fn message() -> Kiwi<'static> {
        let node = |id, parent, kind, name: &str, extra: Vec<_>| {
            let mut fields = vec![
                ("guid", guid(id)),
                ("type", Kiwi::Enum("NodeType", kind)),
                ("name", Kiwi::String(name.into())),
            ];
            if let Some(parent) = parent {
                fields.push((
                    "parentIndex",
                    obj(vec![
                        ("guid", guid(parent)),
                        ("position", Kiwi::String(format!("{id:03}"))),
                    ]),
                ));
            }
            fields.extend(extra);
            obj(fields)
        };
        let text = |s: String| vec![("textData", obj(vec![("characters", Kiwi::String(s))]))];
        obj(vec![(
            "nodeChanges",
            Kiwi::Array(vec![
                node(0, None, "DOCUMENT", "Document", vec![]),
                node(1, Some(0), "CANVAS", "Page", vec![]),
                node(2, Some(1), "FRAME", "Login", vec![]),
                node(
                    3,
                    Some(2),
                    "TEXT",
                    "Label",
                    text(format!("Welcome ÉCOLE {}", "貓".repeat(210))),
                ),
                node(4, Some(1), "FRAME", "Login", vec![]),
                node(
                    5,
                    Some(4),
                    "TEXT",
                    "Login text",
                    text("Other page text".into()),
                ),
                node(
                    6,
                    Some(1),
                    "FRAME",
                    "Login group",
                    vec![("resizeToFit", Kiwi::Bool(true))],
                ),
                node(7, Some(1), "SYMBOL", "Button", vec![]),
                node(8, Some(7), "TEXT", "Button label", text("Click".into())),
                node(
                    9,
                    Some(2),
                    "INSTANCE",
                    "Button instance",
                    vec![("symbolData", obj(vec![("symbolID", guid(7))]))],
                ),
            ]),
        )])
    }

    #[test]
    fn search_matches_text_and_names_with_scope_types_and_real_truncation() {
        let message = message();
        let tree = Tree::new(&message);
        let found = find(&tree, "école", "0:2", Some("TEXT"), 20).unwrap();
        assert_eq!(found["results"][0]["id"], "0:3");
        assert_eq!(
            found["results"][0]["matchedOn"],
            json!({"name": false, "text": true})
        );
        assert_eq!(
            found["results"][0]["path"][2],
            json!({"id": "0:2", "name": "Login"})
        );
        assert_eq!(
            found["results"][0]["characters"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            TEXT_CHARS
        );
        assert_eq!(found["results"][0]["textTruncated"], true);
        assert_eq!(found["truncated"], false);
        let first = find(&tree, "LOGIN", "0:0", Some("FRAME"), 1).unwrap();
        assert_eq!(first["results"][0]["id"], "0:2");
        assert_eq!(first["truncated"], true);
        let all = find(&tree, "LOGIN", "0:0", Some("FRAME"), 2).unwrap();
        assert_eq!(all["results"][1]["id"], "0:4");
        assert_eq!(all["truncated"], false);
        assert_eq!(
            find(&tree, "LOGIN", "0:0", Some("GROUP"), 20).unwrap()["results"][0]["id"],
            "0:6"
        );
        assert!(
            find(&tree, "other page", "0:2", None, 20).unwrap()["results"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        // Stored-layer search does not pretend to search expanded component copies.
        assert!(
            find(&tree, "click", "0:2", None, 20).unwrap()["results"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn inspect_projects_rest_fields_and_expands_instances_with_depth_limits() {
        let message = message();
        let tree = Tree::new(&message);
        let full = figlens::rest::nodes(&tree, "0:2");
        let shallow = inspect(
            &tree,
            "0:2",
            0,
            Path::new("test.fig"),
            false,
            false,
            None,
            None,
            false,
            false,
        )
        .unwrap();
        assert_eq!(shallow["node"]["childCount"], 2);
        assert!(shallow["node"].get("children").is_none());
        assert_eq!(shallow["truncated"], true);
        let summary = inspect(
            &tree,
            "0:2",
            2,
            Path::new("test.fig"),
            false,
            false,
            None,
            None,
            false,
            false,
        )
        .unwrap();
        assert_eq!(summary["truncated"], false);
        assert_eq!(
            summary["node"]["absoluteBoundingBox"],
            full["document"]["absoluteBoundingBox"]
        );
        let child = &summary["node"]["children"][1]["children"][0];
        assert_eq!(child["id"], "I0:9;0:8");
        assert_eq!(child["characters"], "Click");
        assert!(child.get("fills").is_none());
    }

    #[test]
    fn summary_budget_is_shared_across_siblings() {
        let doc = json!({"id": "root", "children": (0..250).map(|i| json!({"id": i})).collect::<Vec<_>>()});
        let mut remaining = MAX_NODES;
        let mut truncated = false;
        let summary = summarize(&doc, 1, &Detail::default(), &mut remaining, &mut truncated);
        assert_eq!(summary["childCount"], 250);
        assert_eq!(summary["children"].as_array().unwrap().len(), 199);
        assert_eq!(remaining, 0);
        assert!(truncated);
    }
}
