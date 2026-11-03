//! Geometry and SVG rendering of REST-shaped nodes built with render info (`rest::render_nodes`).
//!
//! Paths come straight from the `.fig`: shapes carry `fillGeometry` / `strokeGeometry` and text
//! carries glyph outlines, all as command blobs, so no font files are needed. Everything is
//! written in page coordinates; the SVG's viewBox picks the region.

use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value as Json;

use crate::Tree;
use crate::expand::{Mat, Vec2};

/// Expanded render input and original image bytes for one selected export node.
pub(crate) struct Prepared {
    pub document: Json,
    pub images: HashMap<String, Vec<u8>>,
}

impl Prepared {
    /// Expand the containing instance before selecting the exact target and its images.
    pub fn new(tree: &Tree, fig: &Path, id: &str) -> Result<Self, Box<dyn Error>> {
        let document = crate::rest::resolve_render_nodes(tree, id)?;
        let images = images(fig, &document["document"])?;
        Ok(Self { document, images })
    }

    /// Read paints from the same immutable bytes that supplied the decoded tree.
    pub fn from_snapshot(tree: &Tree, bytes: &[u8], id: &str) -> Result<Self, Box<dyn Error>> {
        let document = crate::rest::resolve_render_nodes(tree, id)?;
        let mut refs = Vec::new();
        collect_refs(&document["document"], &mut refs);
        let images = if refs.is_empty() {
            HashMap::new()
        } else {
            read_images(crate::open_zip_bytes(bytes)?, refs)?
        };
        Ok(Self { document, images })
    }

    pub fn node(&self) -> &Json {
        &self.document["document"]
    }

    /// A raster export includes page/section backdrops and uses export bounds, not asset bounds.
    pub fn export_svg(&self, tree: &Tree, scale: f64) -> Option<String> {
        Some(self.export_svg_in(tree, export_area(self.node())?, scale, false))
    }

    /// Raster-ready SVG of the node over the page region `area`, cut to it when `clip`.
    pub fn export_svg_in(&self, tree: &Tree, area: Rect, scale: f64, clip: bool) -> String {
        let n = self.node();
        let lookup = |h: &str| self.images.get(h).cloned();
        Svg::new(tree, &lookup)
            .for_raster()
            .clipped(clip)
            .backdrop(n, area)
            .document(n, area, scale)
    }
}

pub(crate) fn find<'j>(n: &'j Json, id: &str) -> Option<&'j Json> {
    if n["id"] == id {
        return Some(n);
    }
    n["children"].as_array()?.iter().find_map(|c| find(c, id))
}

pub(crate) fn find_mut<'j>(n: &'j mut Json, id: &str) -> Option<&'j mut Json> {
    if n["id"] == id {
        return Some(n);
    }
    n.get_mut("children")?
        .as_array_mut()?
        .iter_mut()
        .find_map(|c| find_mut(c, id))
}

/// The `images/<sha1>` bytes of every image fill under `document`, by imageRef.
pub(crate) fn images(
    fig: &Path,
    document: &Json,
) -> Result<HashMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut refs = Vec::new();
    collect_refs(document, &mut refs);
    if refs.is_empty() {
        return Ok(HashMap::new());
    }
    read_images(crate::open_zip(fig)?, refs)
}

fn read_images<R: std::io::Read + std::io::Seek>(
    mut zip: zip::ZipArchive<R>,
    refs: Vec<String>,
) -> Result<HashMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut images = HashMap::new();
    let mut remaining = crate::parse::IMAGE_TOTAL_BYTES;
    for r in refs {
        if let Ok(f) = zip.by_name(&format!("images/{r}")) {
            let size = f.size();
            let bytes = crate::read_image(f, size, &mut remaining)?;
            images.insert(r, bytes);
        }
    }
    Ok(images)
}

fn collect_refs(v: &Json, out: &mut Vec<String>) {
    match v {
        Json::Object(o) => {
            if let Some(r) = o.get("imageRef").and_then(Json::as_str)
                && !out.iter().any(|x| x == r)
            {
                out.push(r.to_owned());
            }
            o.values().for_each(|x| collect_refs(x, out));
        }
        Json::Array(a) => a.iter().for_each(|x| collect_refs(x, out)),
        _ => {}
    }
}

/// `[x0, y0, x1, y1]` in page coordinates.
pub type Rect = [f64; 4];

pub fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some([
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]),
        (a, b) => a.or(b),
    }
}

/// The overlap of `a` with `b`; a zero-width or zero-height `a` (a hairline) can still overlap.
pub fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let r = [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ];
    let open = |lo: f64, hi: f64, flat: bool| lo < hi || (flat && lo == hi);
    (open(r[0], r[2], a[0] == a[2]) && open(r[1], r[3], a[1] == a[3])).then_some(r)
}

pub fn rect_json(r: Option<Rect>) -> Json {
    r.map_or(Json::Null, |r| {
        serde_json::json!({ "x": r[0], "y": r[1], "width": r[2] - r[0], "height": r[3] - r[1] })
    })
}

pub fn rect_of(v: &Json) -> Option<Rect> {
    let f = |k: &str| v[k].as_f64();
    let (x, y) = (f("x")?, f("y")?);
    Some([x, y, x + f("width")?, y + f("height")?])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    M(Vec2),
    L(Vec2),
    Q(Vec2, Vec2),
    C(Vec2, Vec2, Vec2),
    Z,
}

/// Decodes a command blob: a command byte (0 close, 1 move, 2 line, 3 quad, 4 cubic)
/// followed by its points as little-endian f32 pairs.
pub fn commands(blob: &[u8]) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut i = 0;
    let pt = |i: &mut usize| -> Option<Vec2> {
        let f = |o: usize| Some(f32::from_le_bytes(blob.get(o..o + 4)?.try_into().ok()?) as f64);
        let p = Vec2 {
            x: f(*i)?,
            y: f(*i + 4)?,
        };
        *i += 8;
        Some(p)
    };
    while i < blob.len() {
        let c = blob[i];
        i += 1;
        let seg = match c {
            0 => Some(Seg::Z),
            1 => pt(&mut i).map(Seg::M),
            2 => pt(&mut i).map(Seg::L),
            3 => (|| Some(Seg::Q(pt(&mut i)?, pt(&mut i)?)))(),
            4 => (|| Some(Seg::C(pt(&mut i)?, pt(&mut i)?, pt(&mut i)?)))(),
            _ => None,
        };
        match seg {
            Some(s) => out.push(s),
            None => break,
        }
    }
    out
}

/// Tight bounds of `segs` after `m`; curves are sampled, which is exact to well under a pixel.
fn path_bounds(segs: &[Seg], m: Mat) -> Option<Rect> {
    let mut b: Option<Rect> = None;
    let mut add = |p: Vec2| {
        let p = m.apply(p);
        b = union(b, Some([p.x, p.y, p.x, p.y]));
    };
    let mut cur = Vec2 { x: 0.0, y: 0.0 };
    for s in segs {
        match *s {
            Seg::M(p) | Seg::L(p) => {
                add(p);
                cur = p;
            }
            Seg::Q(c, p) => {
                for t in (1..=16).map(|i| i as f64 / 16.0) {
                    let u = 1.0 - t;
                    add(Vec2 {
                        x: u * u * cur.x + 2.0 * u * t * c.x + t * t * p.x,
                        y: u * u * cur.y + 2.0 * u * t * c.y + t * t * p.y,
                    });
                }
                cur = p;
            }
            Seg::C(c1, c2, p) => {
                for t in (1..=16).map(|i| i as f64 / 16.0) {
                    let u = 1.0 - t;
                    let (a, bb, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    add(Vec2 {
                        x: a * cur.x + bb * c1.x + c * c2.x + d * p.x,
                        y: a * cur.y + bb * c1.y + c * c2.y + d * p.y,
                    });
                }
                cur = p;
            }
            Seg::Z => {}
        }
    }
    b
}

fn path_d(segs: &[Seg], m: Mat) -> String {
    let mut d = String::new();
    let p = |v: Vec2| {
        let v = m.apply(v);
        format!("{} {}", num(v.x), num(v.y))
    };
    // Glyph outlines can open with a close command, which SVG path data rejects.
    let start = segs
        .iter()
        .position(|s| matches!(s, Seg::M(_)))
        .unwrap_or(segs.len());
    for s in &segs[start..] {
        let _ = match *s {
            Seg::M(a) => write!(d, "M{}", p(a)),
            Seg::L(a) => write!(d, "L{}", p(a)),
            Seg::Q(a, b) => write!(d, "Q{} {}", p(a), p(b)),
            Seg::C(a, b, c) => write!(d, "C{} {} {}", p(a), p(b), p(c)),
            Seg::Z => write!(d, "Z"),
        };
    }
    d
}

/// `x` with at most four decimals and no trailing zeros.
pub fn num(x: f64) -> String {
    let s = format!("{:.4}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// A rectangle with corner radii `[tl, tr, br, bl]`, as path segments.
fn rect_segs(x: f64, y: f64, w: f64, h: f64, radii: [f64; 4]) -> Vec<Seg> {
    let max = (w.min(h) / 2.0).max(0.0);
    let [tl, tr, br, bl] = radii.map(|r| r.clamp(0.0, max));
    let v = |x, y| Vec2 { x, y };
    // Circular arcs as cubics.
    let k = 1.0 - 0.552_284_75;
    vec![
        Seg::M(v(x + tl, y)),
        Seg::L(v(x + w - tr, y)),
        Seg::C(v(x + w - tr * k, y), v(x + w, y + tr * k), v(x + w, y + tr)),
        Seg::L(v(x + w, y + h - br)),
        Seg::C(
            v(x + w, y + h - br * k),
            v(x + w - br * k, y + h),
            v(x + w - br, y + h),
        ),
        Seg::L(v(x + bl, y + h)),
        Seg::C(v(x + bl * k, y + h), v(x, y + h - bl * k), v(x, y + h - bl)),
        Seg::L(v(x, y + tl)),
        Seg::C(v(x, y + tl * k), v(x + tl * k, y), v(x + tl, y)),
        Seg::Z,
    ]
}

fn mat(v: &Json) -> Mat {
    let f = |i: usize| v[i].as_f64().unwrap_or(0.0);
    Mat {
        a: f(0),
        b: f(1),
        c: f(2),
        d: f(3),
        tx: f(4),
        ty: f(5),
    }
}

pub fn mat_json(m: Mat) -> Json {
    serde_json::json!([m.a, m.b, m.c, m.d, m.tx, m.ty])
}

fn kiwi_mat(t: &Json) -> Mat {
    let f = |k: &str| t[k].as_f64().unwrap_or(0.0);
    Mat {
        a: f("m00"),
        b: f("m10"),
        c: f("m01"),
        d: f("m11"),
        tx: f("m02"),
        ty: f("m12"),
    }
}

pub fn visible(p: &Json) -> bool {
    p["visible"] != Json::Bool(false) && p["opacity"].as_f64().unwrap_or(1.0) > 0.0
}

/// The paint lists the renderer draws for `n`, with their Kiwi field: the fills of the selected
/// text or vector style overrides, the base fills when some part uses them, and drawn strokes.
/// Unused styles are left out.
pub(crate) fn paint_lists(n: &Json) -> Vec<(&Json, String)> {
    let r = &n["_r"];
    let mut lists = Vec::new();
    // Match the renderer's per-region / per-character paint selection; unused styles
    // must not turn into warnings on the node that happens to store them.
    let text = n["type"] == "TEXT" && r["styleOverrideTable"].is_array();
    let (table, ids, table_name) = if text {
        let keep = r["truncationStartIndex"]
            .as_i64()
            .filter(|i| *i >= 0)
            .map_or(usize::MAX, |i| i as usize);
        let ids = r["glyphs"]
            .as_array()
            .into_iter()
            .flatten()
            .take(keep)
            .map(|g| {
                r["characterStyleIDs"][g["firstCharacter"].as_u64().unwrap_or(0) as usize]
                    .as_u64()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        (&r["styleOverrideTable"], ids, "textData.styleOverrideTable")
    } else {
        let ids = r["fillGeometry"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|g| g["styleID"].as_u64().unwrap_or(0))
            .collect::<Vec<_>>();
        (&r["vectorStyles"], ids, "vectorData.styleOverrideTable")
    };
    let mut base = !text && ids.is_empty();
    for id in ids {
        let selected = table
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .find(|(_, s)| s["styleID"].as_u64() == Some(id) && s["fillPaints"].is_array());
        if let Some((index, style)) = selected {
            let field = format!("{table_name}[{index}].fillPaints");
            if !lists.iter().any(|(_, path)| path == &field) {
                lists.push((&style["fillPaints"], field));
            }
        } else {
            base = true;
        }
    }
    if base {
        lists.push((&r["fillPaints"], "fillPaints".into()));
    }
    if has_stroke(n) {
        lists.push((&r["strokePaints"], "strokePaints".into()));
    }
    lists
}

/// Known renderer fallbacks in the effective paints of one visible node.
/// This is deliberately not a completeness or pixel-fidelity claim.
pub(crate) fn diagnostics(tree: &Tree, n: &Json) -> Vec<Json> {
    let mut out = Vec::new();
    let mut drawn_strokes = None;
    let mut report = |feature: &str, field: String, state: &str, reason: &str| {
        // Decode stroke outlines only when a paint would otherwise produce a warning. An
        // undrawn paint type draws nothing with or without outlines, so it is always reported.
        if field.starts_with("strokePaints[")
            && feature != "paint"
            && !*drawn_strokes.get_or_insert_with(|| !stroke_shapes(tree, n).is_empty())
        {
            return;
        }
        out.push(serde_json::json!({
            "nodeId": n["id"], "feature": feature, "field": field,
            "state": state, "reason": reason,
        }));
    };
    let r = &n["_r"];
    // A copied selection stores no outlines, only the vector network; an empty list is not this.
    let outlined = outlined(n["type"].as_str().unwrap_or(""));
    if outlined && n["type"] != "LINE" && r["fillGeometry"].is_null() && has_paint(&r["fillPaints"])
    {
        report(
            "geometry",
            "fillGeometry".into(),
            "approximate",
            "No fill outline is stored, so the fill is drawn as the node's box",
        );
    }
    if outlined && r["strokeGeometry"].is_null() && has_stroke(n) {
        report(
            "geometry",
            "strokeGeometry".into(),
            "approximate",
            "No stroke outline is stored, so the stroke is drawn around the node's box",
        );
    }
    for (list, path) in paint_lists(n) {
        for (i, paint) in list
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(_, p)| visible(p))
        {
            let field = format!("{path}[{i}]");
            match paint["type"].as_str() {
                Some("IMAGE") => {
                    if paint["imageScaleMode"] == "TILE" {
                        report(
                            "imageTile",
                            format!("{field}.imageScaleMode"),
                            "approximate",
                            "TILE is drawn unrotated and unverified against Figma exports",
                        );
                    }
                    for adjustment in ["contrast", "temperature", "tint", "highlights", "shadows"] {
                        if paint["paintFilter"][adjustment]
                            .as_f64()
                            .is_some_and(|v| v != 0.0)
                        {
                            report(
                                "imageAdjustment",
                                format!("{field}.paintFilter.{adjustment}"),
                                "skipped",
                                "This image adjustment is not rendered",
                            );
                        }
                    }
                }
                Some("GRADIENT_ANGULAR" | "GRADIENT_DIAMOND") => {
                    report(
                        "gradient",
                        format!("{field}.type"),
                        "approximate",
                        "Angular and diamond gradients are rendered as radial gradients",
                    );
                }
                // Drawn as `paint_path` draws them.
                Some("SOLID") | None => {}
                Some(kind) if kind.starts_with("GRADIENT_") => {}
                // shortcut: CUSTOM (shader), NOISE, PATTERN, VIDEO and EMOJI paints draw nothing; no sample uses them yet.
                Some(_) => {
                    report(
                        "paint",
                        format!("{field}.type"),
                        "skipped",
                        "This paint type is not rendered",
                    );
                }
            }
        }
    }
    let mut blur_drawn = n["isMask"] == Json::Bool(true);
    for (i, effect) in r["effects"].as_array().into_iter().flatten().enumerate() {
        if !visible(effect) || is_filter_effect(effect) {
            continue;
        }
        if effect["type"] == "BACKGROUND_BLUR" {
            if !effect["radius"].as_f64().is_some_and(|v| v > 0.0) {
                continue;
            }
            if blur_drawn {
                report(
                    "backgroundBlur",
                    format!("effects[{i}]"),
                    "skipped",
                    "Only a node's first background blur is drawn, and none on a mask",
                );
            } else {
                report(
                    "backgroundBlur",
                    format!("effects[{i}]"),
                    "approximate",
                    "Background blur sees only what the same export draws behind the node; past 64 blurs or 32 MiB of backdrop per export, later ones are skipped",
                );
            }
            blur_drawn = true;
        } else {
            report(
                "effect",
                format!("effects[{i}].type"),
                "skipped",
                "This effect type is not rendered",
            );
        }
    }
    out
}

fn size_of(r: &Json) -> Vec2 {
    Vec2 {
        x: r["size"]["x"].as_f64().unwrap_or(0.0),
        y: r["size"]["y"].as_f64().unwrap_or(0.0),
    }
}

/// One geometry list (`fillGeometry` / `strokeGeometry`) as `(segments, evenodd)` per path.
fn geometry(tree: &Tree, list: &Json) -> Vec<(Vec<Seg>, bool)> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let blob = tree.blob(g["commandsBlob"].as_u64()?)?;
            Some((
                commands(&blob),
                matches!(g["windingRule"].as_str(), Some("EVENODD" | "ODD")),
            ))
        })
        .collect()
}

/// Corner radii `[tl, tr, br, bl]` of a REST node.
fn radii(n: &Json) -> [f64; 4] {
    match n["rectangleCornerRadii"].as_array() {
        Some(r) => [0, 1, 2, 3].map(|i| r.get(i).and_then(Json::as_f64).unwrap_or(0.0)),
        None => [n["cornerRadius"].as_f64().unwrap_or(0.0); 4],
    }
}

/// Fill outlines of a node in its own space: its fill geometry, its glyphs, or its box.
fn fill_shapes(tree: &Tree, n: &Json) -> Vec<(Vec<Seg>, bool, Mat)> {
    let r = &n["_r"];
    let m = mat(&r["m"]);
    if n["type"] == "TEXT" {
        return glyphs(tree, r)
            .into_iter()
            .map(|(segs, gm, _)| (segs, false, m * gm))
            .collect();
    }
    let geo = geometry(tree, &r["fillGeometry"]);
    if !geo.is_empty() {
        let gm = m * geometry_scale(r, "fillGeometrySize");
        return geo.into_iter().map(|(s, odd)| (s, odd, gm)).collect();
    }
    let s = size_of(r);
    vec![(rect_segs(0.0, 0.0, s.x, s.y, radii(n)), false, m)]
}

/// Glyph outlines with their transform inside the text box and their character index.
fn glyphs(tree: &Tree, r: &Json) -> Vec<(Vec<Seg>, Mat, usize)> {
    // Truncated text ends with an ellipsis glyph; the glyphs from this list index on are cut.
    let keep = r["truncationStartIndex"]
        .as_i64()
        .filter(|i| *i >= 0)
        .map_or(usize::MAX, |i| i as usize);
    r["glyphs"]
        .as_array()
        .into_iter()
        .flatten()
        .take(keep)
        .filter_map(|g| {
            let blob = tree.blob(g["commandsBlob"].as_u64()?)?;
            let fs = g["fontSize"].as_f64().unwrap_or(0.0);
            let p = &g["position"];
            // Glyphs are in em units with y up.
            let gm = Mat {
                a: fs,
                b: 0.0,
                c: 0.0,
                d: -fs,
                tx: p["x"].as_f64().unwrap_or(0.0),
                ty: p["y"].as_f64().unwrap_or(0.0),
            };
            Some((
                commands(&blob),
                gm,
                g["firstCharacter"].as_u64().unwrap_or(0) as usize,
            ))
        })
        .collect()
}

/// The box a stroke without geometry covers, grown outward and shrunk inward by each side's
/// weight as the alignment places it, in node space: `(outer, inner)` outlines. Weights past
/// the box shrink the inner outline to nothing rather than flip it.
fn stroke_band(n: &Json) -> (Vec<Seg>, Vec<Seg>) {
    let r = &n["_r"];
    let s = size_of(r);
    let out = match r["strokeAlign"].as_str() {
        Some("OUTSIDE") => 1.0,
        Some("CENTER") => 0.5,
        _ => 0.0,
    };
    let [t, rt, b, l] = stroke_weights(n);
    let rect = |k: f64| {
        let [t, rt, b, l] = [t * k, rt * k, b * k, l * k];
        let (w, h) = ((s.x + (l + rt)).max(0.0), (s.y + (t + b)).max(0.0));
        rect_segs(-l, -t, w, h, [0.0; 4])
    };
    (rect(out), rect(out - 1.0))
}

/// Stroke outlines in page space: stroke geometry when the file has it, else the box's outline
/// (`None` geometry) to be stroked with `stroke-width`.
fn stroke_shapes(tree: &Tree, n: &Json) -> Vec<(Vec<Seg>, bool, Mat)> {
    let r = &n["_r"];
    let gm = mat(&r["m"]) * geometry_scale(r, "strokeGeometrySize");
    geometry(tree, &r["strokeGeometry"])
        .into_iter()
        .map(|(s, odd)| (s, odd, gm))
        .collect()
}

/// Page-space outlines a pointer can hit on a vector-like node (fill and stroke geometry), as SVG
/// path data with its even-odd flag. Read-only: the same decoding rendering uses, for preview picking.
pub fn hit_paths(tree: &Tree, n: &Json) -> Vec<(String, bool)> {
    if !outlined(n["type"].as_str().unwrap_or("")) {
        return Vec::new();
    }
    let geometry = |list: &Json| !geometry(tree, list).is_empty();
    let mut shapes = if geometry(&n["_r"]["fillGeometry"]) {
        fill_shapes(tree, n)
    } else {
        Vec::new()
    };
    shapes.extend(stroke_shapes(tree, n));
    shapes
        .into_iter()
        .map(|(segs, odd, m)| (path_d(&segs, m), odd))
        .filter(|(d, _)| !d.is_empty())
        .collect()
}

/// Node types drawn from stored outlines (`fillGeometry` / `strokeGeometry`), not from their box.
pub(crate) fn outlined(kind: &str) -> bool {
    matches!(
        kind,
        "VECTOR" | "BOOLEAN_OPERATION" | "STAR" | "REGULAR_POLYGON" | "ELLIPSE" | "LINE"
    )
}

/// Scales outlines drawn at the size under `field` (a resized instance child's source) to the node's size.
fn geometry_scale(r: &Json, field: &str) -> Mat {
    let (s, g) = (
        size_of(r),
        Vec2 {
            x: r[field]["x"].as_f64().unwrap_or(0.0),
            y: r[field]["y"].as_f64().unwrap_or(0.0),
        },
    );
    let k = |a: f64, b: f64| if b > 0.0 { a / b } else { 1.0 };
    Mat {
        a: k(s.x, g.x),
        b: 0.0,
        c: 0.0,
        d: k(s.y, g.y),
        tx: 0.0,
        ty: 0.0,
    }
}

/// Stroke weight per side (top, right, bottom, left) of REST node `n`: its
/// `individualStrokeWeights` when set, else `strokeWeight`.
pub(crate) fn stroke_weights(n: &Json) -> [f64; 4] {
    let sides = &n["individualStrokeWeights"];
    if sides.is_object() {
        ["top", "right", "bottom", "left"].map(|s| sides[s].as_f64().unwrap_or(0.0))
    } else {
        [n["_r"]["strokeWeight"].as_f64().unwrap_or(0.0); 4]
    }
}

/// Whether `n` draws a stroke: a visible stroke paint and a side with weight.
pub(crate) fn has_stroke(n: &Json) -> bool {
    has_paint(&n["_r"]["strokePaints"]) && stroke_weights(n).iter().any(|w| *w > 0.0)
}

pub(crate) fn has_paint(list: &Json) -> bool {
    list.as_array().into_iter().flatten().any(visible)
}

/// `base` (what a node paints before its effects, `base_bounds`) grown by its shadows and blurs:
/// what the node paints itself, not its children, in page space.
pub fn grow_by_effects(n: &Json, base: Rect) -> Rect {
    let r = &n["_r"];
    let mut out = Some(base);
    for e in r["effects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| visible(e))
    {
        let radius = e["radius"].as_f64().unwrap_or(0.0);
        let spread = e["spread"].as_f64().unwrap_or(0.0);
        let (dx, dy) = (
            e["offset"]["x"].as_f64().unwrap_or(0.0),
            e["offset"]["y"].as_f64().unwrap_or(0.0),
        );
        let grown = match e["type"].as_str() {
            Some("DROP_SHADOW") => {
                let g = radius + spread;
                [
                    base[0] + dx - g,
                    base[1] + dy - g,
                    base[2] + dx + g,
                    base[3] + dy + g,
                ]
            }
            Some("FOREGROUND_BLUR") | Some("LAYER_BLUR") => [
                base[0] - radius,
                base[1] - radius,
                base[2] + radius,
                base[3] + radius,
            ],
            _ => continue,
        };
        out = union(out, Some(grown));
    }
    out.unwrap_or(base)
}

/// What a node paints itself (not its children) before its effects, in page space; `None` when
/// it paints nothing.
pub fn base_bounds(tree: &Tree, n: &Json) -> Option<Rect> {
    let r = &n["_r"];
    let m = mat(&r["m"]);
    let mut b = None;
    // A background blur paints its outline even without fills.
    if has_paint(&r["fillPaints"]) || active_background_blur(n).is_some() {
        for (segs, _, sm) in fill_shapes(tree, n) {
            b = union(b, path_bounds(&segs, sm));
        }
    }
    let fill_box = fill_shapes(tree, n)
        .iter()
        .fold(None, |b, (segs, _, sm)| union(b, path_bounds(segs, *sm)));
    if has_stroke(n) {
        let shapes = stroke_shapes(tree, n);
        if shapes.is_empty() && n["type"] != "TEXT" {
            let (outer, _) = stroke_band(n);
            b = union(b, path_bounds(&outer, m));
        }
        // Stroke geometry is a double-width centered outline; an inside stroke keeps the part within the shape.
        let inside = r["strokeAlign"] == "INSIDE";
        for (segs, _, sm) in shapes {
            let sb = path_bounds(&segs, sm);
            b = union(
                b,
                if inside {
                    sb.zip(fill_box).and_then(|(s, f)| intersect(s, f))
                } else {
                    sb
                },
            );
        }
    }
    // Containers always cover their own box, painted or not.
    if matches!(
        n["type"].as_str(),
        Some("FRAME" | "INSTANCE" | "COMPONENT" | "COMPONENT_SET" | "GROUP")
    ) {
        b = union(b, box_bounds(n));
    }
    b
}

/// The node's own box in page space, used to clip its children.
pub fn box_bounds(n: &Json) -> Option<Rect> {
    let r = &n["_r"];
    let s = size_of(r);
    path_bounds(&rect_segs(0.0, 0.0, s.x, s.y, [0.0; 4]), mat(&r["m"]))
}

/// Pixel size from the encoded image header, without decoding the image.
fn image_size(b: &[u8]) -> Option<(f64, f64)> {
    let size = imagesize::blob_size(b).ok()?;
    (size.width > 0 && size.height > 0).then_some((size.width as f64, size.height as f64))
}

/// Half the size, each pixel the mean of a 2x2 block (premultiplied, so edges stay right).
fn halve(p: &resvg::tiny_skia::Pixmap) -> resvg::tiny_skia::Pixmap {
    let (w, h) = ((p.width() / 2).max(1), (p.height() / 2).max(1));
    let mut out = resvg::tiny_skia::Pixmap::new(w, h).expect("nonzero size");
    let src = p.data();
    let stride = p.width() as usize * 4;
    let dst = out.data_mut();
    for y in 0..h as usize {
        for x in 0..w as usize {
            for c in 0..4 {
                let at = |dx: usize, dy: usize| {
                    let (sx, sy) = (
                        (2 * x + dx).min(p.width() as usize - 1),
                        (2 * y + dy).min(p.height() as usize - 1),
                    );
                    u32::from(src[sy * stride + sx * 4 + c])
                };
                dst[(y * w as usize + x) * 4 + c] =
                    ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
            }
        }
    }
    out
}

/// The area a node exports as, like Figma's `exportAsync`: its render bounds (else its box);
/// a page is the union of its children. Sections get 40 px of empty margin.
pub fn export_area(n: &Json) -> Option<Rect> {
    let area = if n["type"] == "CANVAS" {
        n["children"]
            .as_array()
            .into_iter()
            .flatten()
            .fold(None, |a, c| union(a, render_bounds(c)))
    } else {
        render_bounds(n).or_else(|| box_bounds(n))
    }?;
    let pad = if n["type"] == "SECTION" { 40.0 } else { 0.0 };
    Some([area[0] - pad, area[1] - pad, area[2] + pad, area[3] + pad])
}

/// Writes SVG for render-built nodes.
pub struct Svg<'t, 'a> {
    tree: &'t Tree<'a>,
    images: &'t dyn Fn(&str) -> Option<Vec<u8>>,
    defs: String,
    ids: usize,
    id_prefix: String,
    /// Drawn under the node across the whole export area.
    backdrop: String,
    /// What each open ancestor has drawn so far, outermost first: the content a background
    /// blur sees, with its markup size once references are expanded. Tracked only when the
    /// export has one (`None` otherwise).
    behind: Option<Vec<(String, usize)>>,
    /// Markup size of each shared body with its references expanded, as a renderer expands them.
    expanded: std::collections::HashMap<String, usize>,
    /// Replacement references that leave background blurs out of a backdrop: empty for a blur,
    /// a blur-free copy for a body that contains one.
    plain: std::collections::HashMap<String, String>,
    /// Outlines of the background blurs drawn so far, with their bounds: each blur replaces the
    /// backdrop inside its outline, so content drawn before it is cut there.
    holes: Vec<(String, Rect, usize)>,
    /// Cost of the cuts made so far, against the same budget as blurs: each cut is another layer
    /// that redraws the outlines it cuts.
    cuts: usize,
    /// Set while drawing what is never seen itself: mask content, which only lends its alpha or
    /// luminance, or a zero-opacity subtree. A blur there is not drawn: it would not show, would
    /// use up the budget, and masks are referenced outside `<use>`, beyond blur-free copies.
    unseen: bool,
    /// Expanded markup drawn by background blurs so far in this export.
    blurred: usize,
    /// Output pixels per user unit, set by `document`.
    scale: f64,
    /// The export area, set by `document`.
    area: Rect,
    /// The outermost clipping frame being drawn: content behind a blur ends there.
    edge: Option<Rect>,
    /// Downscaled images by hash and halving count; `None` keeps full images (SVG output).
    mips: Option<std::collections::HashMap<(String, u32), Vec<u8>>>,
    /// The `<image>` in `defs` for each hash and halving count, so a reused image is embedded once.
    embedded: std::collections::HashMap<(String, u32), String>,
    /// Cut the drawing to the export area, which the output rounds up to whole pixels.
    clip: bool,
}

impl<'t, 'a> Svg<'t, 'a> {
    pub fn new(tree: &'t Tree<'a>, images: &'t dyn Fn(&str) -> Option<Vec<u8>>) -> Self {
        Svg {
            tree,
            images,
            defs: String::new(),
            ids: 0,
            id_prefix: String::new(),
            backdrop: String::new(),
            behind: None,
            expanded: Default::default(),
            plain: Default::default(),
            unseen: false,
            holes: Vec::new(),
            cuts: 0,
            blurred: 0,
            scale: 1.0,
            area: [0.0; 4],
            edge: None,
            mips: None,
            embedded: Default::default(),
            clip: false,
        }
    }

    /// With `clip`, draws nothing outside the export area, even in the part of the last pixel
    /// row and column that whole-pixel rounding adds past it.
    pub fn clipped(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    /// For output that is rasterized at the document's scale: large images are downscaled
    /// first, and pure blurs skip unused shadow intermediates. An SVG kept as a file keeps
    /// full images and the original filter graph, since it may be drawn at any size.
    pub fn for_raster(mut self) -> Self {
        self.mips = Some(std::collections::HashMap::new());
        self
    }

    /// What Figma draws under an exported page or section across the whole area: the page's
    /// background, or the fills of the sections a section is nested in (margin included).
    pub fn backdrop(mut self, n: &Json, area: Rect) -> Self {
        let mut layers = Vec::new();
        let mut id = n["id"].as_str().unwrap_or("").to_owned();
        if n["type"] == "CANVAS" {
            let page = self
                .tree
                .nodes
                .get(&id)
                .map(|k| crate::to_json(k))
                .unwrap_or_default();
            if page["backgroundEnabled"] == true {
                let mut color = page["backgroundColor"].clone();
                color["a"] = serde_json::json!(
                    color["a"].as_f64().unwrap_or(1.0)
                        * page["backgroundOpacity"].as_f64().unwrap_or(1.0)
                );
                layers.push(serde_json::json!([{ "type": "SOLID", "color": color }]));
            }
        } else if n["type"] != "SECTION" {
            return self;
        }
        while n["type"] == "SECTION"
            && let Some(parent) = self
                .tree
                .nodes
                .get(&id)
                .and_then(|k| k.get("parentIndex")?.get("guid"))
                .and_then(crate::guid_key)
        {
            let Some(node) = self.tree.nodes.get(&parent) else {
                break;
            };
            if !matches!(
                node.get("type"),
                Some(kiwi_schema::Value::Enum(_, "SECTION"))
            ) {
                break;
            }
            layers.push(crate::to_json(node)["fillPaints"].clone());
            id = parent;
        }
        for p in layers
            .iter()
            .rev()
            .flat_map(|l| l.as_array().into_iter().flatten())
        {
            if p["type"] == "SOLID" && p["visible"] != false {
                let a =
                    p["color"]["a"].as_f64().unwrap_or(1.0) * p["opacity"].as_f64().unwrap_or(1.0);
                let _ = write!(
                    self.backdrop,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{}/>",
                    num(area[0]),
                    num(area[1]),
                    num(area[2] - area[0]),
                    num(area[3] - area[1]),
                    hex(&p["color"]),
                    alpha_attr("fill-opacity", a)
                );
            }
        }
        self
    }

    /// A ` filter="..."` attribute for an image paint's adjustments, set on the painted shape
    /// (inside a pattern resvg filters at the tile's own size), fitted to the plugin's
    /// renders: exposure scales linear light by 2^(5e), saturation is SVG `saturate` in linear RGB.
    // shortcut: contrast, temperature, tint, highlights and shadows are not drawn; fit them against renders when designs rely on them.
    fn image_filter(&mut self, f: &Json) -> String {
        let get = |k: &str| f[k].as_f64().filter(|v| *v != 0.0);
        let (exposure, saturation) = (get("exposure"), get("vibrance"));
        if exposure.is_none() && saturation.is_none() {
            return String::new();
        }
        let id = self.id("a");
        let mut body = String::new();
        if let Some(e) = exposure {
            let slope = num(2f64.powf(5.0 * e));
            let _ = write!(
                body,
                "<feComponentTransfer><feFuncR type=\"linear\" slope=\"{slope}\"/><feFuncG type=\"linear\" slope=\"{slope}\"/><feFuncB type=\"linear\" slope=\"{slope}\"/></feComponentTransfer>"
            );
        }
        if let Some(s) = saturation {
            let _ = write!(
                body,
                "<feColorMatrix type=\"saturate\" values=\"{}\"/>",
                num((1.0 + s).max(0.0))
            );
        }
        let _ = write!(
            self.defs,
            "<filter id=\"{id}\" color-interpolation-filters=\"linearRGB\">{body}</filter>"
        );
        format!(" filter=\"url(#{id})\"")
    }

    /// The image halved (2x2 averages) while it stays at least twice the pixels it covers:
    /// resvg samples a large downscale without averaging, which aliases; Figma's does not.
    // shortcut: only PNGs are downscaled; decode JPEGs too if photos alias.
    /// The image halved while it stays at least `covered` device pixels, and the halving count.
    fn mip(&mut self, hash: &str, bytes: Vec<u8>, covered: (f64, f64)) -> (Vec<u8>, u32) {
        let Some((w, h)) = image_size(&bytes).filter(|_| self.mips.is_some()) else {
            return (bytes, 0);
        };
        let k = (w / covered.0).min(h / covered.1);
        let levels = if k >= 2.0 { k.log2().floor() as u32 } else { 0 };
        if levels == 0 {
            return (bytes, 0);
        }
        let key = (hash.to_owned(), levels);
        if let Some(b) = self.mips.as_ref().and_then(|m| m.get(&key)) {
            return (b.clone(), levels);
        }
        let Ok(mut img) = resvg::tiny_skia::Pixmap::decode_png(&bytes) else {
            return (bytes, 0);
        };
        for _ in 0..levels {
            img = halve(&img);
        }
        let Ok(out) = img.encode_png() else {
            return (bytes, 0);
        };
        if let Some(m) = self.mips.as_mut() {
            m.insert(key, out.clone());
        }
        (out, levels)
    }

    fn id(&mut self, prefix: &str) -> String {
        self.ids += 1;
        format!("{}{prefix}{}", self.id_prefix, self.ids)
    }

    /// A standalone SVG document of `n` and its children over the page region `area`.
    pub fn document(mut self, n: &Json, area: Rect, scale: f64) -> String {
        self.scale = scale;
        self.area = area;
        self.id_prefix.push('n');
        // Escape bytes injectively, including underscores, to keep instance paths NCName-safe.
        for b in n["id"].as_str().unwrap_or("").bytes() {
            if b.is_ascii_alphanumeric() {
                self.id_prefix.push(char::from(b));
            } else {
                let _ = write!(self.id_prefix, "_{b:02x}");
            }
        }
        self.id_prefix.push('-');
        if has_background_blur(n) {
            self.behind = Some(Vec::new());
        }
        let body = self.node(n);
        let backdrop = match self.erase(0, None) {
            Some(m) => format!("<g mask=\"url(#{m})\">{}</g>", self.backdrop),
            None => std::mem::take(&mut self.backdrop),
        };
        let mut drawing = backdrop + &body;
        if self.clip {
            let id = self.id("area");
            let _ = write!(
                self.defs,
                "<clipPath id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                num(area[0]),
                num(area[1]),
                num(area[2] - area[0]),
                num(area[3] - area[1])
            );
            drawing = format!("<g clip-path=\"url(#{id})\">{drawing}</g>");
        }
        // Exports are sized to the scaled box, rounded up, as Figma sizes them.
        // Float noise (26.0000001) must not round up a whole pixel.
        let px = |len: f64| (len * scale - 1e-3).ceil().max(1.0);
        let (w, h) = (px(area[2] - area[0]) / scale, px(area[3] - area[1]) / scale);
        // The origin moves in a transform, not the viewBox: resvg draws filters wrong under a
        // fractional viewBox origin (drop shadows came out a fifth as strong).
        format!(
            "<svg width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">\n<defs>{}</defs>\n<g transform=\"translate({} {})\">{}</g></svg>\n",
            num(w * scale),
            num(h * scale),
            num(w),
            num(h),
            self.defs,
            num(-area[0]),
            num(-area[1]),
            drawing
        )
    }

    fn node(&mut self, n: &Json) -> String {
        let outer = self.unseen;
        self.unseen |= n["opacity"].as_f64() == Some(0.0);
        let body = self.draw(n);
        self.unseen = outer;
        body
    }

    fn draw(&mut self, n: &Json) -> String {
        if n["visible"] == Json::Bool(false) || n["_r"].is_null() {
            return String::new();
        }
        let mut inner = self.background_blur(n).unwrap_or_default();
        let own_holes = self.holes.len();
        let sets_edge =
            self.behind.is_some() && self.edge.is_none() && n["clipsContent"] == Json::Bool(true);
        if sets_edge {
            self.edge = box_bounds(n);
        }
        let r = &n["_r"];
        let fills = match self.region_fills(n) {
            Some(f) => f,
            None => self.fills(n, &r["fillPaints"], fill_shapes(self.tree, n)),
        };
        let fills = self.shared(fills);
        inner += &fills;
        // A boolean operation's children are its operands, already merged into its geometry.
        if n["type"] != "BOOLEAN_OPERATION" {
            let len = self.expanded_len(&fills);
            if let Some(b) = self.behind.as_mut() {
                b.push((fills.clone(), len));
            }
            let mut kids = String::new();
            let mut masked: Option<String> = None;
            // Each drawn child with the holes cut before it.
            let mut drawn = Vec::new();
            for c in n["children"].as_array().into_iter().flatten() {
                if c["isMask"] == Json::Bool(true) && c["visible"] != Json::Bool(false) {
                    if let Some(m) = masked.take() {
                        kids += &m;
                        kids += "</g>";
                    }
                    let id = self.id("m");
                    let alpha = c["maskType"] != "LUMINANCE";
                    let outer = std::mem::replace(&mut self.unseen, true);
                    let body = self.node(c);
                    self.unseen = outer;
                    let _ = write!(
                        self.defs,
                        "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"-1e6\" y=\"-1e6\" width=\"2e6\" height=\"2e6\" style=\"mask-type:{}\">{body}</mask>",
                        if alpha { "alpha" } else { "luminance" }
                    );
                    masked = Some(format!("<g mask=\"url(#{id})\">"));
                    continue;
                }
                let body = self.node(c);
                let body = self.shared(body);
                // shortcut: a backdrop ignores ancestors' masks, clips, opacity and blend modes; add them when a design blurs through one.
                if self.behind.is_some() {
                    drawn.push((body.clone(), self.holes.len(), render_bounds(c)));
                    let plain = self.without_blurs(&body);
                    let len = self.expanded_len(&plain);
                    if let Some(level) = self.behind.as_mut().and_then(|b| b.last_mut()) {
                        level.0 += &plain;
                        level.1 = level.1.saturating_add(len);
                    }
                }
                match masked.as_mut() {
                    Some(m) => m.push_str(&body),
                    None => kids += &body,
                }
            }
            if let Some(b) = self.behind.as_mut() {
                b.pop();
            }
            if let Some(m) = masked {
                kids += &m;
                kids += "</g>";
            }
            // Later blurs replace what they cover, here and in this node's own blur and fills.
            let mut cuts = std::collections::HashMap::new();
            for (body, from, bounds) in drawn {
                let id = body
                    .strip_prefix("<use href=\"#")
                    .and_then(|b| b.strip_suffix("\"/>"));
                if let (Some(id), Some(cut)) = (id, self.cut(&body, from, bounds)) {
                    cuts.insert(id.to_string(), cut);
                }
            }
            if !cuts.is_empty() {
                kids = swap_refs(&kids, &cuts);
            }
            // `inner` holds only this node's own blur and fills so far.
            if let Some(cut) = self.cut(&inner, own_holes, render_bounds(n)) {
                inner = cut;
            }
            if n["clipsContent"] == Json::Bool(true) && !kids.is_empty() {
                let id = self.id("c");
                let s = size_of(r);
                let d = path_d(&rect_segs(0.0, 0.0, s.x, s.y, radii(n)), mat(&r["m"]));
                let _ = write!(
                    self.defs,
                    "<clipPath id=\"{id}\"><path d=\"{d}\"/></clipPath>"
                );
                let _ = write!(inner, "<g clip-path=\"url(#{id})\">{kids}</g>");
            } else {
                inner += &kids;
            }
        }
        if sets_edge {
            self.edge = None;
        }
        inner += &self.strokes(n);
        if inner.is_empty() {
            return inner;
        }
        let mut attrs = String::new();
        if let Some(o) = n["opacity"].as_f64() {
            let _ = write!(attrs, " opacity=\"{}\"", num(o));
        }
        // Only the known modes: the enum's names come from the file's own schema, so an
        // arbitrary one could break out of the attribute.
        if let Some(b) = n["blendMode"].as_str().filter(|b| {
            matches!(
                *b,
                "DARKEN"
                    | "MULTIPLY"
                    | "LINEAR_BURN"
                    | "COLOR_BURN"
                    | "LIGHTEN"
                    | "SCREEN"
                    | "LINEAR_DODGE"
                    | "COLOR_DODGE"
                    | "OVERLAY"
                    | "SOFT_LIGHT"
                    | "HARD_LIGHT"
                    | "DIFFERENCE"
                    | "EXCLUSION"
                    | "HUE"
                    | "SATURATION"
                    | "COLOR"
                    | "LUMINOSITY"
            )
        }) {
            let _ = write!(
                attrs,
                " style=\"mix-blend-mode:{}\"",
                b.to_lowercase().replace('_', "-")
            );
        }
        if let Some(f) = self.effects(n) {
            let _ = write!(attrs, " filter=\"url(#{f})\"");
        }
        format!("<g{attrs}>{inner}</g>\n")
    }

    /// Vector regions with their own fills (`vectorData.styleOverrideTable`), each path painted
    /// with its region's paints; `None` when the node has a single fill.
    fn region_fills(&mut self, n: &Json) -> Option<String> {
        let r = &n["_r"];
        let table = r["vectorStyles"].as_array()?;
        let paints_of = |id: u64| {
            table
                .iter()
                .find(|s| s["styleID"].as_u64() == Some(id) && s["fillPaints"].is_array())
        };
        let list = r["fillGeometry"].as_array()?;
        if !list
            .iter()
            .any(|g| paints_of(g["styleID"].as_u64().unwrap_or(0)).is_some())
        {
            return None;
        }
        let m = mat(&r["m"]) * geometry_scale(r, "fillGeometrySize");
        let mut out = String::new();
        for g in list {
            let shapes: Vec<_> = geometry(self.tree, &Json::Array(vec![g.clone()]))
                .into_iter()
                .map(|(segs, odd)| (segs, odd, m))
                .collect();
            let style = g["styleID"].as_u64().unwrap_or(0);
            let paints = paints_of(style)
                .map_or(&r["fillPaints"], |s| &s["fillPaints"])
                .clone();
            out += &self.fills(n, &paints, shapes);
        }
        Some(out)
    }

    /// Paints `list` (bottom first) over `shapes`.
    fn fills(&mut self, n: &Json, list: &Json, shapes: Vec<(Vec<Seg>, bool, Mat)>) -> String {
        let mut out = String::new();
        if shapes.is_empty() {
            return out;
        }
        let r = &n["_r"];
        let m = mat(&r["m"]);
        let size = size_of(r);
        // Text ranges can override the fill per glyph.
        let per_glyph = n["type"] == "TEXT" && r["styleOverrideTable"].is_array();
        if per_glyph {
            let ids = &r["characterStyleIDs"];
            for (segs, gm, ch) in glyphs(self.tree, r) {
                let style = ids[ch].as_u64().unwrap_or(0);
                let over = r["styleOverrideTable"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|s| s["styleID"].as_u64() == Some(style) && s["fillPaints"].is_array());
                let paints = over.map_or(list, |s| &s["fillPaints"]);
                let d = path_d(&segs, m * gm);
                out += &self.paint_path(paints, &d, false, m, size);
            }
            return out;
        }
        let d: String = shapes.iter().map(|(s, _, sm)| path_d(s, *sm)).collect();
        let odd = shapes.iter().any(|(_, odd, _)| *odd);
        self.paint_path(list, &d, odd, m, size)
    }

    fn paint_path(&mut self, list: &Json, d: &str, odd: bool, m: Mat, size: Vec2) -> String {
        let mut out = String::new();
        let rule = if odd { " fill-rule=\"evenodd\"" } else { "" };
        for p in list.as_array().into_iter().flatten().filter(|p| visible(p)) {
            let opacity = p["opacity"].as_f64().unwrap_or(1.0);
            match p["type"].as_str().unwrap_or("") {
                "SOLID" => {
                    let a = p["color"]["a"].as_f64().unwrap_or(1.0) * opacity;
                    let _ = write!(
                        out,
                        "<path d=\"{d}\"{rule} fill=\"{}\"{}/>",
                        hex(&p["color"]),
                        alpha_attr("fill-opacity", a)
                    );
                }
                "IMAGE" => {
                    let Some(bytes) = p["image"]["hash"].as_str().and_then(|h| (self.images)(h))
                    else {
                        continue;
                    };
                    // A pattern keeps the painted bounds to the shape: an <image> clipped to it keeps
                    // the whole image's bounds, and resvg then cuts it off under an effect filter.
                    let mode = p["imageScaleMode"].as_str();
                    // Tiles repeat from the node's origin at the image's natural size times `scale`;
                    // an unknown size falls back to FILL.
                    let tile = (mode == Some("TILE"))
                        .then(|| {
                            let s = p["scale"].as_f64().unwrap_or(1.0);
                            match (
                                p["originalImageWidth"].as_f64(),
                                p["originalImageHeight"].as_f64(),
                            ) {
                                (Some(w), Some(h)) => Some((w, h)),
                                _ => image_size(&bytes),
                            }
                            .map(|(w, h)| (w * s, h * s))
                            .filter(|(w, h)| w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0)
                        })
                        .flatten();
                    // STRETCH's paint transform maps the node's 0..1 box to image coordinates.
                    let uv = kiwi_mat(&p["transform"]);
                    let ((w, h), t, aspect) = match (mode, tile) {
                        (Some("STRETCH"), _) => (
                            (1.0, 1.0),
                            m * Mat {
                                a: size.x,
                                b: 0.0,
                                c: 0.0,
                                d: size.y,
                                tx: 0.0,
                                ty: 0.0,
                            } * uv.inverse(),
                            "none",
                        ),
                        // shortcut: TILE ignores the paint's rotation; rotate the pattern when a design needs it.
                        (_, Some(tile)) => (tile, m, "none"),
                        (mode, None) => (
                            (size.x, size.y),
                            m,
                            if mode == Some("FIT") {
                                "xMidYMid meet"
                            } else {
                                "xMidYMid slice"
                            },
                        ),
                    };
                    // Device pixels the image covers along each of its axes.
                    let scale = self.scale;
                    let len = |x: f64, y: f64| x.hypot(y) * scale;
                    let covered = match p["imageScaleMode"].as_str() {
                        Some("STRETCH") => Some((len(t.a, t.b), len(t.c, t.d))),
                        _ => image_size(&bytes).map(|(iw, ih)| {
                            let fit = (w / iw, h / ih);
                            let s = if aspect.ends_with("meet") {
                                fit.0.min(fit.1)
                            } else {
                                fit.0.max(fit.1)
                            };
                            (iw * s * len(t.a, t.b), ih * s * len(t.c, t.d))
                        }),
                    };
                    let hash = p["image"]["hash"].as_str().unwrap_or("").to_owned();
                    let (bytes, levels) = match covered {
                        Some(c) => self.mip(&hash, bytes, c),
                        None => (bytes, 0),
                    };
                    // The pattern already clips its tile. Explicit cover geometry avoids usvg's
                    // extra slice clip, which resvg truncates to the outer canvas inside patterns;
                    // explicit fit geometry lets a fitted image share its embedded copy.
                    let (iw, ih, x, y, aspect) = if aspect != "none" {
                        match image_size(&bytes) {
                            Some((iw, ih)) => {
                                let s = if aspect.ends_with("slice") {
                                    (w / iw).max(h / ih)
                                } else {
                                    (w / iw).min(h / ih)
                                };
                                (
                                    iw * s,
                                    ih * s,
                                    (w - iw * s) / 2.0,
                                    (h - ih * s) / 2.0,
                                    "none",
                                )
                            }
                            None => (w, h, 0.0, 0.0, aspect),
                        }
                    } else {
                        (w, h, 0.0, 0.0, aspect)
                    };
                    // resvg allocates a whole pattern tile in device pixels: a tile over MAX_TILE_PX² is
                    // cut to the node's box, bounded like FILL's, so a crafted `scale` cannot allocate a huge one.
                    // shortcut: that cut shifts the period under strokes outside the box; only tiles
                    // over MAX_TILE_PX² get there.
                    let (w, h) = match tile {
                        Some(_)
                            if w * len(t.a, t.b) * h * len(t.c, t.d)
                                > MAX_TILE_PX * MAX_TILE_PX =>
                        {
                            (w.min(size.x), h.min(size.y))
                        }
                        _ => (w, h),
                    };
                    // STRETCH draws the image once: the tile spans every image coordinate the
                    // node's box samples, with the image at 0..1 and the rest empty, so a transform
                    // past the image's edges leaves them empty instead of repeating it. A tile over
                    // 4x the image's or node's device area (a transform shifted far away, or a crop
                    // of a very different shape) spans only what the node samples instead.
                    // shortcut: a skewed transform whose sampled range still exceeds that keeps the
                    // one-image tile, which repeats; strokes outside the box can reach the next tile.
                    let (px, py, w, h) = match mode {
                        Some("STRETCH") => {
                            let unit = Vec2 { x: 1.0, y: 1.0 };
                            let (o, b) = (uv.origin(unit), uv.bbox(unit));
                            let (x0, y0) = (o.x.min(0.0), o.y.min(0.0));
                            let (x1, y1) = ((o.x + b.x).max(1.0), (o.y + b.y).max(1.0));
                            let one = len(t.a, t.b) * len(t.c, t.d);
                            let node =
                                len(m.a * size.x, m.b * size.x) * len(m.c * size.y, m.d * size.y);
                            // resvg allocates the whole tile at the pattern transform's scale.
                            let fits = |w: f64, h: f64| w * h * one <= 4.0 * one.max(node);
                            if fits(x1 - x0, y1 - y0) {
                                (x0, y0, x1 - x0, y1 - y0)
                            } else if fits(b.x, b.y) {
                                (o.x, o.y, b.x, b.y)
                            } else {
                                (0.0, 0.0, w, h)
                            }
                        }
                        _ => (0.0, 0.0, w, h),
                    };
                    // Pattern content is placed from the tile's corner, so the image moves back to 0.
                    let origin = if px != 0.0 || py != 0.0 {
                        format!(" x=\"{}\" y=\"{}\"", num(px), num(py))
                    } else {
                        String::new()
                    };
                    let (x, y) = (x - px, y - py);
                    let adjust = self.image_filter(&p["paintFilter"]);
                    // An image drawn without its own aspect rule is embedded once as a unit square
                    // and scaled by each pattern that paints it; a unit keeps every scale in num()'s
                    // precision, where a reciprocal of the pixel size would not be.
                    let image = match aspect {
                        "none" => {
                            let key = (hash, levels);
                            let id = match self.embedded.get(&key) {
                                Some(id) => id.clone(),
                                None => {
                                    let id = self.id("i");
                                    let _ = write!(
                                        self.defs,
                                        "<image id=\"{id}\" width=\"1\" height=\"1\" preserveAspectRatio=\"none\" href=\"data:{};base64,{}\"/>",
                                        media_type(&bytes),
                                        base64(&bytes),
                                    );
                                    self.embedded.insert(key, id.clone());
                                    id
                                }
                            };
                            let place = Mat {
                                a: iw,
                                b: 0.0,
                                c: 0.0,
                                d: ih,
                                tx: x,
                                ty: y,
                            };
                            format!("<use href=\"#{id}\" transform=\"{}\"/>", matrix(place))
                        }
                        _ => format!(
                            "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{aspect}\" href=\"data:{};base64,{}\"/>",
                            num(x),
                            num(y),
                            num(iw),
                            num(ih),
                            media_type(&bytes),
                            base64(&bytes),
                        ),
                    };
                    let pattern = self.id("p");
                    let _ = write!(
                        self.defs,
                        "<pattern id=\"{pattern}\" patternUnits=\"userSpaceOnUse\"{origin} width=\"{w}\" height=\"{h}\" patternTransform=\"{}\">{image}</pattern>",
                        matrix(t),
                        w = num(w),
                        h = num(h),
                    );
                    let _ = write!(
                        out,
                        "<path d=\"{d}\"{rule} fill=\"url(#{pattern})\"{}{adjust}/>",
                        alpha_attr("fill-opacity", opacity)
                    );
                }
                kind if kind.starts_with("GRADIENT_") => {
                    let id = self.id("g");
                    let t =
                        m * Mat {
                            a: size.x,
                            b: 0.0,
                            c: 0.0,
                            d: size.y,
                            tx: 0.0,
                            ty: 0.0,
                        } * kiwi_mat(&p["transform"]).inverse();
                    let stops: String = p["stops"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|s| {
                            format!(
                                "<stop offset=\"{}\" stop-color=\"{}\"{}/>",
                                num(s["position"].as_f64().unwrap_or(0.0)),
                                hex(&s["color"]),
                                alpha_attr("stop-opacity", s["color"]["a"].as_f64().unwrap_or(1.0))
                            )
                        })
                        .collect();
                    // shortcut: angular and diamond gradients are drawn as radial ones; needs a mesh or shader to match exactly.
                    let (tag, geom) = if kind == "GRADIENT_LINEAR" {
                        ("linearGradient", "x1=\"0\" y1=\"0.5\" x2=\"1\" y2=\"0.5\"")
                    } else {
                        ("radialGradient", "cx=\"0.5\" cy=\"0.5\" r=\"0.5\"")
                    };
                    let _ = write!(
                        self.defs,
                        "<{tag} id=\"{id}\" gradientUnits=\"userSpaceOnUse\" {geom} gradientTransform=\"{}\">{stops}</{tag}>",
                        matrix(t)
                    );
                    let _ = write!(
                        out,
                        "<path d=\"{d}\"{rule} fill=\"url(#{id})\"{}/>",
                        alpha_attr("fill-opacity", opacity)
                    );
                }
                _ => {}
            }
        }
        out
    }

    fn strokes(&mut self, n: &Json) -> String {
        let r = &n["_r"];
        if !has_stroke(n) {
            return String::new();
        }
        let weights = stroke_weights(n);
        let weight = weights[0];
        let shapes = stroke_shapes(self.tree, n);
        if !shapes.is_empty() {
            let body = self.fills(&strip_text(n), &r["strokePaints"], shapes);
            let shape: String = fill_shapes(self.tree, n)
                .iter()
                .map(|(s, _, sm)| path_d(s, *sm))
                .collect();
            return match r["strokeAlign"].as_str() {
                Some("INSIDE") => {
                    let id = self.id("c");
                    let _ = write!(
                        self.defs,
                        "<clipPath id=\"{id}\"><path d=\"{shape}\"/></clipPath>"
                    );
                    format!("<g clip-path=\"url(#{id})\">{body}</g>")
                }
                Some("OUTSIDE") => {
                    let id = self.id("m");
                    let _ = write!(
                        self.defs,
                        "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"-1e6\" y=\"-1e6\" width=\"2e6\" height=\"2e6\"><rect x=\"-1e6\" y=\"-1e6\" width=\"2e6\" height=\"2e6\" fill=\"white\"/><path d=\"{shape}\" fill=\"black\"/></mask>"
                    );
                    format!("<g mask=\"url(#{id})\">{body}</g>")
                }
                _ => body,
            };
        }
        if n["type"] == "TEXT" {
            return String::new();
        }
        // Per-side weights without stroke geometry: fill the band between the grown and shrunk box.
        // shortcut: square corners and no dashes, as no sample had such strokes without geometry.
        if weights.iter().any(|w| *w != weight) {
            let (mut band, inner) = stroke_band(n);
            band.extend(inner);
            return self.fills(n, &r["strokePaints"], vec![(band, true, mat(&r["m"]))]);
        }
        // Boxes without stroke geometry: stroke the box outline, moved by the alignment.
        let m = mat(&r["m"]);
        let s = size_of(r);
        let shift = match r["strokeAlign"].as_str() {
            Some("OUTSIDE") => weight / 2.0,
            Some("CENTER") => 0.0,
            _ => -weight / 2.0,
        };
        let rad = radii(n).map(|x| (x + shift).max(0.0));
        let segs = rect_segs(-shift, -shift, s.x + 2.0 * shift, s.y + 2.0 * shift, rad);
        let d = path_d(&segs, m);
        let scale = (m.a * m.d - m.b * m.c).abs().sqrt();
        let mut out = String::new();
        for p in r["strokePaints"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| visible(p))
        {
            if p["type"] != "SOLID" {
                continue;
            }
            let a = p["color"]["a"].as_f64().unwrap_or(1.0) * p["opacity"].as_f64().unwrap_or(1.0);
            let dash = r["dashPattern"]
                .as_array()
                .filter(|d| !d.is_empty())
                .map(|d| {
                    let list: Vec<String> = d
                        .iter()
                        .map(|x| num(x.as_f64().unwrap_or(0.0) * scale))
                        .collect();
                    format!(" stroke-dasharray=\"{}\"", list.join(" "))
                })
                .unwrap_or_default();
            let _ = write!(
                out,
                "<path d=\"{d}\" stroke=\"{}\" stroke-width=\"{}\"{}{dash}/>",
                hex(&p["color"]),
                num(weight * scale),
                alpha_attr("stroke-opacity", a)
            );
        }
        out
    }

    /// While backdrops are tracked, drawn content is defined once and referenced, so each
    /// background blur repeats references rather than copies (copies of copies grow
    /// exponentially with blurred siblings).
    fn shared(&mut self, body: String) -> String {
        if self.behind.is_none() || body.is_empty() {
            return body;
        }
        let plain = self.without_blurs(&body);
        if plain != body {
            let plain = format!("<use href=\"#{}\"/>", self.define(plain));
            let id = self.define(body);
            self.plain.insert(id.clone(), plain);
            return format!("<use href=\"#{id}\"/>");
        }
        format!("<use href=\"#{}\"/>", self.define(body))
    }

    fn define(&mut self, body: String) -> String {
        let id = self.id("b");
        self.expanded.insert(id.clone(), self.expanded_len(&body));
        let _ = write!(self.defs, "<g id=\"{id}\">{body}</g>");
        id
    }

    /// `body` with references to background blurs dropped and references to bodies containing
    /// them swapped for blur-free copies. Backdrops are drawn from these, so a blurred sibling is
    /// not blurred again into the next one: that nesting grows exponentially.
    fn without_blurs(&self, body: &str) -> String {
        swap_refs(body, &self.plain)
    }

    /// A mask that cuts the outlines of the blurs drawn since hole `from` (those overlapping
    /// `bounds`, or all of them) out of what it covers.
    fn erase(&mut self, from: usize, bounds: Option<Rect>) -> Option<String> {
        let mut holes = String::new();
        let mut cost = 0usize;
        for (id, r, len) in &self.holes[from..] {
            if bounds.is_none_or(|b| intersect(b, *r).is_some()) {
                let _ = write!(holes, "<use href=\"#{id}\" fill=\"#000\"/>");
                cost = cost.saturating_add(*len);
            }
        }
        if holes.is_empty() {
            return None;
        }
        // Past the budget the blur is painted over what it covers, which differs only for a
        // translucent backdrop.
        let cost = cost.max(BLUR_BUDGET / MAX_CUTS);
        if self.cuts.saturating_add(cost) > BLUR_BUDGET {
            return None;
        }
        self.cuts += cost;
        let id = self.id("e");
        let _ = write!(
            self.defs,
            "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"-1e6\" y=\"-1e6\" width=\"2e6\" height=\"2e6\"><rect x=\"-1e6\" y=\"-1e6\" width=\"2e6\" height=\"2e6\" fill=\"#fff\"/>{holes}</mask>"
        );
        Some(id)
    }

    /// The shared `body` with later blurs' outlines cut out, or `None` when none overlap it.
    /// Backdrops still see it uncut, as they see earlier blurs unblurred.
    fn cut(&mut self, body: &str, from: usize, bounds: Option<Rect>) -> Option<String> {
        if body.is_empty() {
            return None;
        }
        let mask = self.erase(from, bounds)?;
        let plain = self.without_blurs(body);
        let id = self.define(format!("<g mask=\"url(#{mask})\">{body}</g>"));
        self.plain.insert(id.clone(), plain);
        Some(format!("<use href=\"#{id}\"/>"))
    }

    /// `body`'s length with every shared reference in it expanded.
    fn expanded_len(&self, body: &str) -> usize {
        body.split("<use href=\"#")
            .skip(1)
            .filter_map(|s| self.expanded.get(s.split('"').next()?))
            .fold(body.len(), |sum, len| sum.saturating_add(*len))
    }

    /// What the export has drawn behind `n`, blurred and cut to `n`'s fill outline, for its
    /// first active background blur. Paths are in page space, so the drawn content is reused
    /// as is.
    // shortcut: content outside the export (an ancestor when only `n` is exported) is not seen, and an earlier blur is seen unblurred; render the ancestors when a design needs them.
    fn background_blur(&mut self, n: &Json) -> Option<String> {
        if self.unseen {
            return None;
        }
        let radius = active_background_blur(n)?;
        let [x0, y0, x1, y1] = rect_of(&n["_r"]["fb"]).or_else(|| box_bounds(n))?;
        // The blur reads 3 sigma (sigma = radius / 2) beyond the outline, as layer blurs do.
        let reach = 1.5 * radius;
        // resvg clears a filter, or crops clipped layers inside it, that reaches past its canvas
        // limit (`max_filter_bbox`) counted from the filter's own origin, so a large radius in a
        // small export reads no further than one export size past each side.
        let e = self.area;
        let (w, h) = (e[2] - e[0], e[3] - e[1]);
        let fr = intersect(
            [x0 - reach, y0 - reach, x1 + reach, y1 + reach],
            [e[0] - w, e[1] - h, e[2] + w, e[3] + h],
        )?;
        // Past the content's edge (the outermost clipping frame, else the export) Figma blurs as
        // if the edge continued: measured on a card near a frame's sides. Left empty, the blur
        // darkens there. Tiles of the backdrop, flipped at every edge, cover what the blur reads.
        // shortcut: at most MAX_TILES tiles past each side, enough for a clipping frame of half
        // the export or more; raise it for designs blurring past smaller ones.
        let a = self.edge.unwrap_or(self.area);
        let tiles = |lo: f64, hi: f64, a0: f64, a1: f64| {
            let w = a1 - a0;
            if w <= 0.0 {
                return 0..=0;
            }
            let k = |v: f64| ((v - a0) / w).floor().clamp(-MAX_TILES, MAX_TILES) as i32;
            k(lo).min(0)..=k(hi).max(0)
        };
        let (xs, ys) = (
            tiles(fr[0], fr[2], a[0], a[2]),
            tiles(fr[1], fr[3], a[1], a[3]),
        );
        let copies = xs.clone().count() * ys.clone().count();
        let levels = self.behind.as_ref()?;
        // Each blur redraws its backdrop, so a crafted file with many blurs over large content
        // must not exhaust the renderer. A blur costs at least 1/MAX_BLURS of the budget: its
        // Gaussian pass and any images it redraws are not counted in markup.
        let len = levels
            .iter()
            .fold(self.backdrop.len(), |sum, (_, len)| {
                sum.saturating_add(*len)
            })
            .saturating_mul(copies)
            .max(BLUR_BUDGET / MAX_BLURS);
        if self.blurred.saturating_add(len) > BLUR_BUDGET {
            return None;
        }
        self.blurred += len;
        let behind: String = levels.iter().map(|(markup, _)| markup.as_str()).collect();
        let behind = self.backdrop.clone() + &behind;
        let inside = self.id("c");
        let _ = write!(
            self.defs,
            "<clipPath id=\"{inside}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
            num(a[0]),
            num(a[1]),
            num(a[2] - a[0]),
            num(a[3] - a[1])
        );
        let backdrop = self.define(format!("<g clip-path=\"url(#{inside})\">{behind}</g>"));
        // Tile k along an axis is the backdrop shifted k widths, mirrored when k is odd.
        let place = |k: i32, a0: f64, a1: f64| {
            let k = f64::from(k);
            if k.rem_euclid(2.0) == 0.0 {
                (1, k * (a1 - a0))
            } else {
                (-1, 2.0 * a0 + (k + 1.0) * (a1 - a0))
            }
        };
        let mut behind = String::new();
        for kx in xs {
            for ky in ys.clone() {
                let ((sx, tx), (sy, ty)) = (place(kx, a[0], a[2]), place(ky, a[1], a[3]));
                let flip = if (kx, ky) == (0, 0) {
                    String::new()
                } else {
                    format!(
                        " transform=\"matrix({sx} 0 0 {sy} {} {})\"",
                        num(tx),
                        num(ty)
                    )
                };
                let _ = write!(behind, "<use href=\"#{backdrop}\"{flip}/>");
            }
        }
        // One outline, as `fills` paints it, so a hole in the fill is a hole in the blur.
        let shapes = fill_shapes(self.tree, n);
        let d: String = shapes.iter().map(|(s, _, m)| path_d(s, *m)).collect();
        let rule = if shapes.iter().any(|(_, odd, _)| *odd) {
            " clip-rule=\"evenodd\""
        } else {
            ""
        };
        let clip = self.id("c");
        let _ = write!(
            self.defs,
            "<clipPath id=\"{clip}\"><path d=\"{d}\"{rule}/></clipPath>"
        );
        let filter = self.id("f");
        let _ = write!(
            self.defs,
            "<filter id=\"{filter}\" filterUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" color-interpolation-filters=\"sRGB\"><feGaussianBlur stdDeviation=\"{}\"/></filter>",
            num(fr[0]),
            num(fr[1]),
            num(fr[2] - fr[0]),
            num(fr[3] - fr[1]),
            num(radius / 2.0)
        );
        let id = self.define(format!(
            "<g clip-path=\"url(#{clip})\"><g filter=\"url(#{filter})\">{behind}</g></g>"
        ));
        self.plain.insert(id.clone(), String::new());
        let hole = self.id("h");
        let rule = if rule.is_empty() {
            ""
        } else {
            " fill-rule=\"evenodd\""
        };
        let _ = write!(self.defs, "<path id=\"{hole}\" d=\"{d}\"{rule}/>");
        self.holes.push((hole, [x0, y0, x1, y1], d.len()));
        Some(format!("<use href=\"#{id}\"/>"))
    }

    /// A filter for the node's visible shadows and blurs; background blur is drawn separately.
    fn effects(&mut self, n: &Json) -> Option<String> {
        let list: Vec<&Json> = n["_r"]["effects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| visible(e) && is_filter_effect(e))
            .collect();
        if list.is_empty() {
            return None;
        }
        let id = self.id("f");
        let mut f = String::from(
            "<feColorMatrix in=\"SourceAlpha\" type=\"matrix\" values=\"0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0\" result=\"hard\"/>",
        );
        let mut under = Vec::new();
        let mut over = Vec::new();
        let mut blur = None;
        for (i, e) in list.iter().enumerate() {
            let sd = num(e["radius"].as_f64().unwrap_or(0.0) / 2.0);
            let (dx, dy) = (
                num(e["offset"]["x"].as_f64().unwrap_or(0.0)),
                num(e["offset"]["y"].as_f64().unwrap_or(0.0)),
            );
            let spread = e["spread"].as_f64().unwrap_or(0.0);
            // Figma's own SVG export: shadows start from the node's alpha made hard (any
            // coverage counts as full), and are colored by a matrix.
            let c = &e["color"];
            let channel = |k: &str| num(c[k].as_f64().unwrap_or(0.0));
            let tint = format!(
                "<feColorMatrix type=\"matrix\" values=\"0 0 0 0 {} 0 0 0 0 {} 0 0 0 0 {} 0 0 0 {} 0\" result=\"e{i}\"/>",
                channel("r"),
                channel("g"),
                channel("b"),
                num(c["a"].as_f64().unwrap_or(1.0))
            );
            match e["type"].as_str() {
                Some("DROP_SHADOW") => {
                    let src = if spread > 0.0 {
                        let _ = write!(
                            f,
                            "<feMorphology in=\"SourceAlpha\" operator=\"dilate\" radius=\"{}\" result=\"s{i}\"/>",
                            num(spread)
                        );
                        format!("s{i}")
                    } else {
                        "hard".into()
                    };
                    let _ = write!(
                        f,
                        "<feOffset in=\"{src}\" dx=\"{dx}\" dy=\"{dy}\"/><feGaussianBlur stdDeviation=\"{sd}\"/>"
                    );
                    // By default the shadow is cut out wherever the node covers it, so it never
                    // shows through translucent fills.
                    if e["showShadowBehindNode"] != true {
                        f += "<feComposite in2=\"hard\" operator=\"out\"/>";
                    }
                    f += &tint;
                    under.push(format!("e{i}"));
                }
                Some("INNER_SHADOW") => {
                    let _ = write!(
                        f,
                        "<feOffset in=\"hard\" dx=\"{dx}\" dy=\"{dy}\"/><feGaussianBlur stdDeviation=\"{sd}\"/><feComposite in2=\"hard\" operator=\"arithmetic\" k2=\"-1\" k3=\"1\"/>{tint}"
                    );
                    over.push(format!("e{i}"));
                }
                _ => blur = Some(sd),
            }
        }
        let input = if self.mips.is_some() && under.is_empty() && over.is_empty() {
            // Skip pure-blur intermediates for rasterization, keeping SVG asset bytes stable.
            f.clear();
            "SourceGraphic"
        } else {
            let merge: String = under
                .iter()
                .map(String::as_str)
                .chain(["SourceGraphic"])
                .chain(over.iter().map(String::as_str))
                .map(|r| format!("<feMergeNode in=\"{r}\"/>"))
                .collect();
            let _ = write!(f, "<feMerge result=\"all\">{merge}</feMerge>");
            "all"
        };
        if let Some(sd) = blur {
            let _ = write!(f, "<feGaussianBlur in=\"{input}\" stdDeviation=\"{sd}\"/>");
        }
        // The filter canvas is what the node draws, uncut by its ancestors' clips, plus each
        // effect's reach: 3 sigma (sigma = radius / 2) of blur, spread and offset. Filter cost
        // grows with this area, so it must not also count the reach that render bounds hold.
        let reach = list
            .iter()
            .map(|e| {
                let o = &e["offset"];
                1.5 * e["radius"].as_f64().unwrap_or(0.0)
                    + e["spread"].as_f64().unwrap_or(0.0).abs()
                    + o["x"]
                        .as_f64()
                        .unwrap_or(0.0)
                        .abs()
                        .max(o["y"].as_f64().unwrap_or(0.0).abs())
            })
            .fold(1.0, f64::max);
        let [x0, y0, x1, y1] = rect_of(&n["_r"]["fb"])
            .or_else(|| box_bounds(n))
            .unwrap_or([0.0; 4]);
        let (x0, y0, x1, y1) = (x0 - reach, y0 - reach, x1 + reach, y1 + reach);
        let _ = write!(
            self.defs,
            "<filter id=\"{id}\" filterUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" color-interpolation-filters=\"sRGB\">{f}</filter>",
            num(x0),
            num(y0),
            num(x1 - x0),
            num(y1 - y0)
        );
        Some(id)
    }
}

/// Expanded markup all background blurs in one export may draw before later ones are skipped.
const BLUR_BUDGET: usize = 32 << 20;
/// Background blurs one export may draw.
const MAX_BLURS: usize = 64;
/// A TILE image fill keeps its whole tile in the pattern up to MAX_TILE_PX² device pixels.
const MAX_TILE_PX: f64 = 2048.0;

/// Mirrored backdrop tiles drawn past each side of the content's edge for a background blur.
const MAX_TILES: f64 = 2.0;
/// Layers one export may cut background blur outlines from.
const MAX_CUTS: usize = 4096;

/// `body` with each `<use>` reference whose id `map` has replaced by its value, in one pass.
fn swap_refs(body: &str, map: &std::collections::HashMap<String, String>) -> String {
    let mut parts = body.split("<use href=\"#");
    let mut out = parts.next().unwrap_or_default().to_string();
    for part in parts {
        match part
            .split_once("\"/>")
            .filter(|(id, _)| map.contains_key(*id))
        {
            Some((id, rest)) => {
                out += &map[id];
                out += rest;
            }
            None => {
                out += "<use href=\"#";
                out += part;
            }
        }
    }
    out
}

/// Shadows and layer blurs, which the node's own filter draws. Background blur is drawn
/// separately; other types (such as GLASS) are skipped and reported, not drawn as blurs.
// shortcut: GLASS is skipped, not modelled; model it once a second file uses it.
fn is_filter_effect(e: &Json) -> bool {
    matches!(
        e["type"].as_str(),
        Some("DROP_SHADOW" | "INNER_SHADOW" | "FOREGROUND_BLUR" | "LAYER_BLUR")
    )
}

/// The radius of `n`'s first visible, nonzero background blur.
fn active_background_blur(n: &Json) -> Option<f64> {
    n["_r"]["effects"]
        .as_array()?
        .iter()
        .filter(|e| visible(e) && e["type"] == "BACKGROUND_BLUR")
        .find_map(|e| e["radius"].as_f64().filter(|r| *r > 0.0))
}

/// Whether `node` would draw a background blur somewhere in `n`.
fn has_background_blur(n: &Json) -> bool {
    n["visible"] != Json::Bool(false)
        && !n["_r"].is_null()
        && n["isMask"] != Json::Bool(true)
        && n["opacity"].as_f64() != Some(0.0)
        && (active_background_blur(n).is_some()
            || n["type"] != "BOOLEAN_OPERATION"
                && n["children"]
                    .as_array()
                    .is_some_and(|c| c.iter().any(has_background_blur)))
}

/// A copy of `n` that is not text, so stroke outlines are painted as plain shapes.
fn strip_text(n: &Json) -> Json {
    let mut c = n.clone();
    c["type"] = Json::from("VECTOR");
    c
}

fn matrix(m: Mat) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        num(m.a),
        num(m.b),
        num(m.c),
        num(m.d),
        num(m.tx),
        num(m.ty)
    )
}

fn alpha_attr(name: &str, a: f64) -> String {
    if a < 1.0 {
        format!(" {name}=\"{}\"", num(a))
    } else {
        String::new()
    }
}

/// A REST color as `#rrggbb`, alpha ignored.
pub fn hex(c: &Json) -> String {
    let ch = |k: &str| (c[k].as_f64().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", ch("r"), ch("g"), ch("b"))
}

/// Image type from its signature, as the bundle names files.
pub fn media_type(b: &[u8]) -> &'static str {
    match b {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xff, 0xd8, 0xff, ..] => "image/jpeg",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "image/webp",
        _ => "application/octet-stream",
    }
}

pub fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() {
                T[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/// Losslessly optimize delivery PNGs, keeping the original encoding when it is not smaller.
pub fn encode_png(p: &resvg::tiny_skia::Pixmap) -> Result<Vec<u8>, String> {
    let original = p.encode_png().map_err(|e| e.to_string())?;
    // Bound added optimization work to 4,194,304 pixels; larger rasters keep their encoding.
    if p.data().len() > 16 * 1024 * 1024 {
        return Ok(original);
    }
    let options = oxipng::Options {
        optimize_alpha: false,
        max_decompressed_size: Some(p.data().len() + p.height() as usize),
        ..oxipng::Options::from_preset(0)
    };
    let optimized = oxipng::optimize_from_memory(&original, &options)
        .map_err(|e| format!("PNG optimization failed: {e}"))?;
    Ok(if optimized.len() < original.len() {
        optimized
    } else {
        original
    })
}

/// Rasterizes an SVG document to PNG bytes.
pub fn png(svg: &str) -> Result<Vec<u8>, String> {
    encode_png(&pixmap(svg)?)
}

/// Rasterizes an SVG document to premultiplied RGBA pixels.
pub fn pixmap(svg: &str) -> Result<resvg::tiny_skia::Pixmap, String> {
    // Images are embedded as data: URLs; never let an href read a file from disk.
    let mut options = resvg::usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty image")?;
    let s = |v: u32, f: f32| v as f32 / f;
    let transform = resvg::tiny_skia::Transform::from_scale(
        s(size.width(), tree.size().width()),
        s(size.height(), tree.size().height()),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Layout box from a REST node (`absoluteBoundingBox`).
pub fn layout_bounds(n: &Json) -> Option<Rect> {
    rect_of(&n["absoluteBoundingBox"])
}

/// Render bounds from a REST node (`absoluteRenderBounds`).
pub fn render_bounds(n: &Json) -> Option<Rect> {
    rect_of(&n["absoluteRenderBounds"])
}

#[cfg(test)]
mod tests;
