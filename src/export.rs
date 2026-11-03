//! Export one node as SVG or a raster image, with render bounds so shadows are included, or
//! with its layout box on request. Raster formats support a quality setting and a size budget.

use std::error::Error;
use std::path::{Path, PathBuf};

use resvg::tiny_skia::Pixmap;
use serde_json::{Map, Value as Json, json};

use crate::Tree;
use crate::render::{self, Prepared};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Png,
    Jpg,
    Webp,
    Avif,
    Svg,
}

impl Format {
    /// The format an output path's extension names.
    pub fn of(path: &Path) -> Option<Format> {
        Format::from_ext(path.extension()?.to_str()?)
    }

    /// The format a bare extension (`png`, `JPEG`, ...) names.
    pub fn from_ext(ext: &str) -> Option<Format> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "png" => Format::Png,
            "jpg" | "jpeg" => Format::Jpg,
            "webp" => Format::Webp,
            "avif" => Format::Avif,
            "svg" => Format::Svg,
            _ => return None,
        })
    }
}

/// What the source lacks for visible layers (a copied selection carries no image bytes, and may
/// leave out components): the renderer draws without them, so the export must not read as complete.
fn missing_inputs(
    tree: &Tree,
    n: &Json,
    images: &std::collections::HashMap<String, Vec<u8>>,
    out: &mut Vec<String>,
) {
    // Null render bounds mark what draws nothing: zero opacity, clipped away or a boolean
    // operand (as `rest` decides for render diagnostics); masks shape what they draw.
    if n["visible"] == false
        || (n.get("absoluteRenderBounds") == Some(&Json::Null) && n["isMask"] != true)
    {
        return;
    }
    let id = n["id"].as_str().unwrap_or("");
    let mut push = |warning: String| {
        if !out.contains(&warning) {
            out.push(warning);
        }
    };
    // The paints the renderer draws, including selected text and vector style overrides.
    for (list, _) in crate::render::paint_lists(n) {
        for paint in list.as_array().into_iter().flatten() {
            if paint["type"] == "IMAGE"
                && crate::render::visible(paint)
                && let Some(key) = paint["image"]["hash"]
                    .as_str()
                    .filter(|key| !key.is_empty())
                && !images.contains_key(key)
            {
                push(format!(
                    "Image {key} at node {id} is not in the .fig; it is drawn empty"
                ));
            }
        }
    }
    if let Some(component) = n["componentId"].as_str()
        && !tree.nodes.contains_key(component)
    {
        push(format!(
            "Component {component} of instance {id} is not in the .fig; only the instance's stored layers are drawn"
        ));
    }
    for child in n["children"].as_array().into_iter().flatten() {
        missing_inputs(tree, child, images, out);
    }
}

/// An encoded image and what it ended up as.
pub struct Image {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Raster quality used (lowered to meet a budget); the requested value for SVG, which ignores it.
    pub quality: u8,
}

/// Renders node `id` over its render bounds, or its layout box when `layout`, and encodes it.
/// SVG keeps its original dimensions and ignores quality. Also returns `layoutBox`, `renderBox`
/// and `warnings`: the drawn area past the layout box, then the subtree's render diagnostics.
/// Layers in `hide` draw as if hidden in Figma, over the same area, so the two exports compare
/// pixel for pixel; `hidden` lists them.
#[allow(clippy::too_many_arguments)]
pub fn export(
    tree: &Tree,
    fig: &Path,
    id: &str,
    format: Format,
    layout: bool,
    scale: f64,
    quality: u8,
    max_bytes: Option<usize>,
    max_px: Option<u32>,
    hide: &[String],
) -> Result<(Image, Map<String, Json>), Box<dyn Error>> {
    let mut prepared = Prepared::new(tree, fig, id)?;
    hide_layers(&mut prepared.document["document"], id, hide)?;
    let (image, mut report) = prepared_export(
        tree, &prepared, id, format, layout, scale, quality, max_bytes, max_px,
    )?;
    // Name what the renderer skipped or approximated, as bundle warnings do.
    let mut skipped = Vec::new();
    crate::bundle::render_warnings(prepared.node(), &mut skipped);
    missing_inputs(tree, prepared.node(), &prepared.images, &mut skipped);
    if let Some(Json::Array(warnings)) = report.get_mut("warnings") {
        warnings.extend(skipped.into_iter().map(Json::from));
    }
    if !hide.is_empty() {
        report.insert("hidden".into(), hide.into());
    }
    Ok((image, report))
}

/// Marks each layer in `hide`, strictly inside `node` (exported as `id`), hidden.
fn hide_layers(node: &mut Json, id: &str, hide: &[String]) -> Result<(), crate::FigError> {
    let invalid = |m: String| crate::FigError::new(crate::ErrorKind::InvalidArguments, m);
    for layer in hide {
        let found = (layer != id)
            .then(|| render::find_mut(node, layer))
            .flatten()
            .ok_or_else(|| invalid(format!("hide {layer} is not a layer inside node {id}")))?;
        // Diagnostics were collected with the mask clipping; what it uncovers has none.
        if found["isMask"] == true {
            return Err(invalid(format!(
                "hide {layer} is a mask; hiding it would uncover layers whose rendering warnings were not collected"
            )));
        }
        found["visible"] = Json::Bool(false);
    }
    Ok(())
}

/// Shared renderer path for file exports and immutable snapshot thumbnails.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepared_export(
    tree: &Tree,
    prepared: &Prepared,
    id: &str,
    format: Format,
    layout: bool,
    scale: f64,
    quality: u8,
    max_bytes: Option<usize>,
    max_px: Option<u32>,
) -> Result<(Image, Map<String, Json>), Box<dyn Error>> {
    let unsupported = |reason: &str| {
        crate::FigError::new(
            crate::ErrorKind::UnsupportedNode,
            format!("node {id} is unsupported for export: {reason}"),
        )
    };
    let unbounded =
        || unsupported("no drawable bounds; inspect the node and select a sized drawable layer");
    // A hidden layer draws nothing: say so instead of writing an empty raster.
    if prepared.node()["visible"] == false {
        return Err(unsupported(
            "the layer is hidden; show it in Figma and save a new local copy, or export a visible layer",
        )
        .into());
    }
    let n = prepared.node();
    // What a render-bounds export covers: raster adds a SECTION's margin and frames a page.
    let drawn = if format == Format::Svg {
        render::render_bounds(n)
    } else {
        render::export_area(n)
    };
    let area = if layout {
        render::layout_bounds(n).ok_or_else(|| {
            unsupported("it has no layout box (a page has none); export it without bounds layout")
        })?
    } else {
        drawn.ok_or_else(unbounded)?
    };
    let report = bounds_report(n, drawn, layout);
    if format == Format::Svg {
        let lookup = |h: &str| prepared.images.get(h).cloned();
        // Keep original image fills embedded; raster export's downsampling would lose detail.
        let bytes = render::Svg::new(tree, &lookup)
            .clipped(layout)
            .document(n, area, 1.0)
            .into_bytes();
        if max_bytes.is_some_and(|max| bytes.len() > max) {
            return Err(format!(
                "SVG comes to {} KB; a size budget needs jpg, webp or avif",
                bytes.len().div_ceil(1024)
            )
            .into());
        }
        let px = |len: f64| (len - 1e-3).ceil().max(1.0) as u32;
        let image = Image {
            bytes,
            width: px(area[2] - area[0]),
            height: px(area[3] - area[1]),
            quality,
        };
        return Ok((image, report));
    }
    // Fit the longest side into `max_px`, so an agent's inline image stays small.
    let longest = (area[2] - area[0]).max(area[3] - area[1]) * scale;
    let scale = match max_px {
        Some(max) if longest > f64::from(max) => scale * f64::from(max) / longest,
        _ => scale,
    };
    let pixmap = render::pixmap(&prepared.export_svg_in(tree, area, scale, layout))?;
    Ok((encode(&pixmap, format, quality, max_bytes)?, report))
}

/// Both boxes of REST node `n`, and a warning naming what is drawn past the layout box when
/// `drawn` (what a render-bounds export covers) differs from it, since such an asset does not fit
/// the layout slot. Causes follow the renderer's reach: shadows and layer blurs, outside or
/// centered strokes, children past an unclipped box, and a SECTION's margin.
fn bounds_report(n: &Json, drawn: Option<render::Rect>, layout: bool) -> Map<String, Json> {
    let mut warnings = Vec::new();
    if let (Some(l), Some(d)) = (render::layout_bounds(n), drawn) {
        // How far the drawn area reaches past each side of the layout box; negative is inside.
        let past = [l[1] - d[1], d[2] - l[2], d[3] - l[3], l[0] - d[0]];
        if past.iter().any(|d| d.abs() >= 0.01) {
            let r = &n["_r"];
            let mut causes = Vec::new();
            let reaches = |e: &Json| {
                render::visible(e)
                    && matches!(
                        e["type"].as_str(),
                        Some("DROP_SHADOW" | "FOREGROUND_BLUR" | "LAYER_BLUR")
                    )
            };
            if r["effects"]
                .as_array()
                .is_some_and(|e| e.iter().any(reaches))
            {
                causes.push("effects");
            }
            if render::has_stroke(n)
                && matches!(r["strokeAlign"].as_str(), Some("OUTSIDE" | "CENTER"))
            {
                causes.push("stroke");
            }
            let outside = |c: &Json| {
                render::visible(c)
                    && render::render_bounds(c)
                        .is_some_and(|b| b[0] < l[0] || b[1] < l[1] || b[2] > l[2] || b[3] > l[3])
            };
            if n["clipsContent"] != true
                && n["children"]
                    .as_array()
                    .is_some_and(|c| c.iter().any(outside))
            {
                causes.push("child geometry");
            }
            if n["type"] == "SECTION" && drawn != render::render_bounds(n) {
                causes.push("section margin");
            }
            let sides = ["top", "right", "bottom", "left"]
                .iter()
                .zip(past)
                .map(|(side, d)| format!("{side} {}", (d * 100.0).round() / 100.0 + 0.0))
                .collect::<Vec<_>>()
                .join(", ");
            let from = if past.iter().any(|&d| d >= 0.01) && !causes.is_empty() {
                format!(" from {}", causes.join(", "))
            } else {
                String::new()
            };
            let used = if layout {
                "exported at the layout box, cutting off what is drawn past it"
            } else {
                "exported over what is drawn; bounds layout exports the layout box"
            };
            warnings.push(format!(
                "the drawn area differs from the layout box by {sides} px (positive reaches past it){from}; {used}"
            ));
        }
    }
    let mut report = Map::new();
    report.insert("layoutBox".into(), n["absoluteBoundingBox"].clone());
    report.insert("renderBox".into(), n["absoluteRenderBounds"].clone());
    report.insert("warnings".into(), json!(warnings));
    report
}

/// The real node an id lives in: an instance sublayer `I<instance>;…` exists only once
/// `<instance>` is expanded.
pub fn root_of(id: &str) -> &str {
    id.strip_prefix('I')
        .and_then(|rest| rest.split(';').next())
        .unwrap_or(id)
}

/// Encodes at `quality`, or at the highest lower quality that fits in `max_bytes`.
pub fn encode(
    p: &Pixmap,
    format: Format,
    quality: u8,
    max_bytes: Option<usize>,
) -> Result<Image, String> {
    let image = |bytes, quality| Image {
        bytes,
        width: p.width(),
        height: p.height(),
        quality,
    };
    let first = encode_at(p, format, quality)?;
    let Some(max) = max_bytes.filter(|&max| first.len() > max) else {
        return Ok(image(first, quality));
    };
    if format == Format::Png {
        return Err(format!(
            "PNG is lossless and comes to {} KB; a size budget needs jpg, webp or avif",
            first.len().div_ceil(1024)
        ));
    }
    // Size falls as quality falls, so binary-search the highest quality that fits.
    let (mut lo, mut hi, mut fit) = (1, quality - 1, None);
    while lo <= hi {
        let q = lo + (hi - lo) / 2;
        let bytes = encode_at(p, format, q)?;
        if bytes.len() <= max {
            fit = Some(image(bytes, q));
            lo = q + 1;
        } else {
            hi = q - 1;
        }
    }
    fit.ok_or_else(|| format!("does not fit in {} KB even at quality 1", max / 1024))
}

fn encode_at(p: &Pixmap, format: Format, quality: u8) -> Result<Vec<u8>, String> {
    let (w, h) = (p.width(), p.height());
    // Encoders take straight (not premultiplied) alpha.
    let rgba = || p.pixels().iter().map(|c| c.demultiply());
    match format {
        Format::Svg => Err("SVG requires vector input; use export instead of encode".into()),
        Format::Png => render::encode_png(p),
        Format::Jpg => {
            // JPG has no alpha: composite over white, as Figma does. Premultiplied makes it c + (255 - a).
            let rgb: Vec<u8> = p
                .pixels()
                .iter()
                .flat_map(|c| {
                    let white = 255 - c.alpha();
                    [c.red() + white, c.green() + white, c.blue() + white]
                })
                .collect();
            let size = |v: u32| u16::try_from(v).map_err(|_| "JPG is limited to 65535 px a side");
            let mut out = Vec::new();
            jpeg_encoder::Encoder::new(&mut out, quality)
                .encode(&rgb, size(w)?, size(h)?, jpeg_encoder::ColorType::Rgb)
                .map_err(|e| e.to_string())?;
            Ok(out)
        }
        Format::Webp => {
            let bytes: Vec<u8> = rgba()
                .flat_map(|c| [c.red(), c.green(), c.blue(), c.alpha()])
                .collect();
            Ok(webp::Encoder::from_rgba(&bytes, w, h)
                .encode(f32::from(quality))
                .to_vec())
        }
        Format::Avif => {
            let pixels: Vec<ravif::RGBA8> = rgba()
                .map(|c| ravif::RGBA8::new(c.red(), c.green(), c.blue(), c.alpha()))
                .collect();
            ravif::Encoder::new()
                .with_quality(f32::from(quality))
                .with_speed(6)
                .encode_rgba(ravif::Img::new(&pixels[..], w as usize, h as usize))
                .map(|e| e.avif_file)
                .map_err(|e| e.to_string())
        }
    }
}

/// SVG files a folder holds, parsed once to compare exports against.
pub struct SvgIndex {
    /// Path relative to the folder (`/`-separated), canonical path and drawing.
    files: Vec<(String, PathBuf, Shape)>,
}

/// A folder needing more than this is too broad to search: SVG files, SVG bytes read, and
/// directory entries visited.
const MAX_SVG_FILES: usize = 10_000;
const MAX_SVG_BYTES: u64 = 64 << 20;
const MAX_ENTRIES: usize = 100_000;
/// Lowest mask overlap a similar drawing reports, and how many are reported.
const SIMILAR: f64 = 0.8;
const MAX_SIMILAR: usize = 10;

impl SvgIndex {
    /// Parses the `.svg` files in `dir` and its subfolders. Links are not followed, so a search
    /// stays inside the folder; files over 1 MiB and files that do not parse are skipped.
    pub fn load(dir: &Path) -> Result<Self, Box<dyn Error>> {
        Self::load_within(dir, MAX_SVG_BYTES)
    }

    fn load_within(dir: &Path, max_bytes: u64) -> Result<Self, Box<dyn Error>> {
        use std::io::Read as _;
        let fail = |path: &Path| {
            let path = path.to_owned();
            move |e: std::io::Error| format!("--match-dir {}: {e}", path.display())
        };
        let too_broad = |what: &str| {
            format!(
                "--match-dir {} holds more than {what}; pass the icon folder",
                dir.display()
            )
        };
        let (mut files, mut entries, mut bytes) = (Vec::new(), 0, 0);
        let mut pending = vec![dir.to_path_buf()];
        while let Some(folder) = pending.pop() {
            for entry in std::fs::read_dir(&folder).map_err(fail(&folder))? {
                entries += 1;
                if entries > MAX_ENTRIES {
                    return Err(too_broad(&format!("{MAX_ENTRIES} entries")).into());
                }
                let entry = entry.map_err(fail(&folder))?;
                let path = entry.path();
                let kind = entry.file_type().map_err(fail(&path))?;
                if kind.is_dir() {
                    pending.push(path);
                    continue;
                }
                let is_svg = path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
                if !kind.is_file() || !is_svg {
                    continue;
                }
                if files.len() == MAX_SVG_FILES {
                    return Err(too_broad(&format!("{MAX_SVG_FILES} SVG files")).into());
                }
                // Read one byte past the limit, so a file that grew after listing is still skipped.
                let mut data = Vec::new();
                std::fs::File::open(&path)
                    .and_then(|f| f.take((1 << 20) + 1).read_to_end(&mut data))
                    .map_err(fail(&path))?;
                // Skipped oversized files still cost their read.
                bytes += data.len() as u64;
                if bytes > max_bytes {
                    return Err(too_broad(&format!("{} MiB of SVG", max_bytes >> 20)).into());
                }
                if data.len() > 1 << 20 {
                    continue;
                }
                let Some(shape) = shape(&data) else {
                    continue;
                };
                let name = path.strip_prefix(dir).unwrap_or(&path);
                let name = name
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                files.push((
                    name,
                    std::fs::canonicalize(&path).map_err(fail(&path))?,
                    shape,
                ));
            }
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Self { files })
    }

    /// The files drawing the same outlines as `svg` once both are moved to the origin (paint,
    /// ids, path syntax and `<path>` splits ignored), and up to ten others whose coverage, scaled
    /// to the same size, overlaps it by at least 0.8 as `{file, score}`, best first. `skip` (the
    /// export itself) is never reported.
    pub fn compare(&self, svg: &[u8], skip: Option<&Path>) -> (Vec<String>, Vec<Json>) {
        let (mut matches, mut similar) = (Vec::new(), Vec::new());
        let Some(want) = shape(svg) else {
            return (matches, Vec::new());
        };
        for (name, path, got) in &self.files {
            if Some(path.as_path()) == skip {
                continue;
            }
            // A clip or mask hides part of the outline; only the rendered masks can compare it.
            // The same segments can paint differently (fill vs stroke, even-odd vs nonzero),
            // so they count only when they also cover the same pixels.
            let outlines = !want.clipped
                && !got.clipped
                && (want.coverage == got.coverage
                    || overlap(&want.coverage, &got.coverage) >= 0.95);
            if outlines && same_outline(&want.outline, &got.outline, 1.0) {
                matches.push(name.clone());
                continue;
            }
            // The same outlines at another size are the best similar result.
            let scale = want.extent / got.extent.max(f32::EPSILON);
            let score = if outlines && same_outline(&want.outline, &got.outline, scale) {
                1.0
            } else {
                overlap(&want.mask, &got.mask)
            };
            if score >= SIMILAR {
                similar.push((name, score));
            }
        }
        similar.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        let similar = similar
            .into_iter()
            .take(MAX_SIMILAR)
            .map(|(file, score)| json!({ "file": file, "score": (score * 100.0).round() / 100.0 }))
            .collect();
        (matches, similar)
    }
}

/// What an SVG draws: its path segments in page space, moved so the points start at 0,0, as
/// (verb, coordinates), and its coverage scaled to fit 64×64, one bit per pixel.
struct Shape {
    outline: Vec<(u8, Vec<f32>)>,
    /// The outline's larger side, to compare outlines drawn at another size.
    extent: f32,
    /// Drawn through a clip path or mask, so the outline overstates what shows.
    clipped: bool,
    mask: [u64; 64],
    /// Every drawn pixel whatever its paint, to check that equal outlines cover the same area.
    coverage: [u64; 64],
}

/// Same segments with points within 0.01 once `b` is scaled by `scale`.
fn same_outline(a: &[(u8, Vec<f32>)], b: &[(u8, Vec<f32>)], scale: f32) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.0 == b.0
                && a.1
                    .iter()
                    .zip(&b.1)
                    .all(|(p, q)| (p - q * scale).abs() <= 0.01)
        })
}

/// How well two masks cover each other within two pixels, so line weight and anti-aliasing
/// matter less than shape: the smaller of the share of `a` near `b` and of `b` near `a`.
fn overlap(a: &[u64; 64], b: &[u64; 64]) -> f64 {
    let near = |m: &[u64; 64], other: &[u64; 64]| {
        let grown = grow(other);
        let total: u32 = m.iter().map(|r| r.count_ones()).sum();
        let hit: u32 = m
            .iter()
            .zip(&grown)
            .map(|(r, g)| (r & g).count_ones())
            .sum();
        if total == 0 {
            0.0
        } else {
            f64::from(hit) / f64::from(total)
        }
    };
    near(a, b).min(near(b, a))
}

/// `m` grown by two pixels in every direction.
fn grow(m: &[u64; 64]) -> [u64; 64] {
    let mut out = [0u64; 64];
    for (y, row) in m.iter().enumerate() {
        let wide = (-2..=2).fold(0, |acc, d: i32| {
            acc | if d < 0 { row >> -d } else { row << d }
        });
        for dy in -2..=2_i32 {
            if let Some(o) = out.get_mut((y as i32 + dy) as usize) {
                *o |= wide;
            }
        }
    }
    out
}

/// None when the SVG does not parse or draws no path.
fn shape(svg: &[u8]) -> Option<Shape> {
    use resvg::tiny_skia::PathSegment as S;
    use resvg::usvg::{Group, Node, Options, Tree};
    fn clipped(group: &Group) -> bool {
        group.clip_path().is_some()
            || group.mask().is_some()
            || group
                .children()
                .iter()
                .any(|n| matches!(n, Node::Group(g) if clipped(g)))
    }
    fn walk(group: &Group, out: &mut Vec<(u8, Vec<f32>)>) {
        for node in group.children() {
            match node {
                Node::Group(g) => walk(g, out),
                Node::Path(p) => {
                    let Some(data) = p.data().clone().transform(p.abs_transform()) else {
                        continue;
                    };
                    let mut start = vec![];
                    for segment in data.segments() {
                        let item = match segment {
                            S::MoveTo(a) => {
                                start = vec![a.x, a.y];
                                (0, start.clone())
                            }
                            S::LineTo(a) => (1, vec![a.x, a.y]),
                            S::QuadTo(a, b) => (2, vec![a.x, a.y, b.x, b.y]),
                            S::CubicTo(a, b, c) => (3, vec![a.x, a.y, b.x, b.y, c.x, c.y]),
                            S::Close => {
                                // A line back to the start before closing draws nothing; optimizers drop it.
                                if out.last().is_some_and(|l| l.0 == 1 && l.1 == start) {
                                    out.pop();
                                }
                                (4, vec![])
                            }
                        };
                        out.push(item);
                    }
                }
                _ => {}
            }
        }
    }
    // Never let an href read a file from disk.
    let mut options = Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = Tree::from_data(svg, &options).ok()?;
    let mut outline = Vec::new();
    walk(tree.root(), &mut outline);
    if outline.is_empty() {
        return None;
    }
    let min = |axis: usize| {
        outline
            .iter()
            .flat_map(|s| s.1.iter().skip(axis).step_by(2))
            .fold(f32::INFINITY, |m, v| m.min(*v))
    };
    let (x, y) = (min(0), min(1));
    for (_, points) in &mut outline {
        for (i, v) in points.iter_mut().enumerate() {
            *v -= if i % 2 == 0 { x } else { y };
        }
    }
    let extent = outline
        .iter()
        .flat_map(|s| s.1.iter())
        .fold(0.0_f32, |m, v| m.max(*v));
    // Strokes count as what they cover, so a stroked icon compares with its outlined copy.
    let b = tree.root().abs_stroke_bounding_box();
    let scale = 64.0 / b.width().max(b.height()).max(f32::EPSILON);
    let transform = resvg::tiny_skia::Transform::from_row(
        scale,
        0.0,
        0.0,
        scale,
        (64.0 - b.width() * scale) / 2.0 - b.x() * scale,
        (64.0 - b.height() * scale) / 2.0 - b.y() * scale,
    );
    let mut pixmap = Pixmap::new(64, 64)?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // Only the main color counts, so a white glyph on a colored badge is a hole, not ink.
    let color = |px: &resvg::tiny_skia::PremultipliedColorU8| {
        let c = px.demultiply();
        [c.red(), c.green(), c.blue()]
    };
    // Ink is relative to the most opaque pixel, so a translucent paint covers what it draws.
    let ink = pixmap
        .pixels()
        .iter()
        .map(|px| px.alpha())
        .max()
        .unwrap_or(0)
        .div_ceil(4)
        .max(1);
    let mut counts = std::collections::HashMap::new();
    for px in pixmap.pixels().iter().filter(|px| px.alpha() >= ink) {
        *counts.entry(color(px).map(|c| c >> 4)).or_insert(0) += 1;
    }
    // A shape too thin to cover a pixel keeps an empty mask; its outline still matches.
    let main = counts.into_iter().max_by_key(|e| (e.1, e.0)).map(|e| e.0);
    let (mut mask, mut coverage) = ([0u64; 64], [0u64; 64]);
    for (i, px) in pixmap.pixels().iter().enumerate() {
        // Any opacity counts as coverage, so paths with different opacities all register.
        if px.alpha() > 0 {
            coverage[i / 64] |= 1 << (i % 64);
        }
        let near = main.is_some_and(|main| {
            color(px)
                .iter()
                .zip(main)
                .all(|(c, m)| c.abs_diff(m << 4 | 8) <= 40)
        });
        if px.alpha() >= ink && near {
            mask[i / 64] |= 1 << (i % 64);
        }
    }
    Some(Shape {
        outline,
        extent,
        clipped: clipped(tree.root()),
        coverage,
        mask,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_inputs_name_visible_images_and_components_once() {
        let tree = Tree {
            nodes: Default::default(),
            children: Default::default(),
            blobs: Vec::new(),
        };
        let image = |key: &str| json!({"type": "IMAGE", "image": {"hash": key}});
        let node = json!({"id": "1:1", "children": [
            {"id": "1:2", "_r": {"fillPaints": [image("a"), image("a")], "strokePaints": [image("b")], "strokeWeight": 1.0}},
            {"id": "1:3", "_r": {"fillPaints": [{"type": "IMAGE", "image": {"hash": "c"}, "visible": false}]}},
            {"id": "1:4", "visible": false, "_r": {"fillPaints": [image("d")]}, "componentId": "9:9"},
            {"id": "1:5", "type": "INSTANCE", "componentId": "9:9", "_r": {"fillPaints": [image("")]}},
            // A stroke with no weight is not drawn.
            {"id": "1:6", "_r": {"strokePaints": [image("h")], "strokeWeight": 0.0}},
            // Null render bounds: a boolean operand, a zero-opacity or clipped-away layer.
            {"id": "1:8", "type": "BOOLEAN_OPERATION", "children": [
                {"id": "1:9", "absoluteRenderBounds": null, "_r": {"fillPaints": [image("operand")]}},
            ]},
            {"id": "1:10", "opacity": 0.0, "absoluteRenderBounds": null, "children": [
                {"id": "1:11", "absoluteRenderBounds": null, "_r": {"fillPaints": [image("faded")]}},
            ]},
            // Only the vector style a region uses draws; the base fill is unused.
            {"id": "1:7", "type": "VECTOR", "_r": {
                "fillPaints": [image("base")],
                "fillGeometry": [{"styleID": 1}],
                "vectorStyles": [
                    {"styleID": 1, "fillPaints": [image("used")]},
                    {"styleID": 2, "fillPaints": [image("unused")]},
                ],
            }},
        ]});
        let images = std::collections::HashMap::from([("e".to_owned(), Vec::new())]);
        let mut out = Vec::new();
        missing_inputs(&tree, &node, &images, &mut out);
        assert_eq!(
            out,
            [
                "Image a at node 1:2 is not in the .fig; it is drawn empty",
                "Image b at node 1:2 is not in the .fig; it is drawn empty",
                "Component 9:9 of instance 1:5 is not in the .fig; only the instance's stored layers are drawn",
                "Image used at node 1:7 is not in the .fig; it is drawn empty",
            ]
        );
    }

    #[test]
    fn hide_marks_inner_layers_and_refuses_masks_the_node_and_outsiders() {
        let mut node = serde_json::json!({"id": "1:1", "children": [
            {"id": "1:2", "children": [{"id": "1:3"}]},
            {"id": "1:4", "isMask": true},
        ]});
        hide_layers(&mut node, "1:1", &["1:3".into()]).unwrap();
        assert_eq!(node["children"][0]["children"][0]["visible"], false);
        for layer in ["1:4", "1:1", "9:9"] {
            assert!(
                hide_layers(&mut node, "1:1", &[layer.into()]).is_err(),
                "{layer}"
            );
        }
        assert!(node["children"][1].get("visible").is_none());
    }

    /// A noisy, half-transparent image, so lossy sizes depend on quality.
    fn noise() -> Pixmap {
        let mut p = Pixmap::new(64, 64).unwrap();
        let mut x: u32 = 1;
        for px in p.pixels_mut() {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let [r, g, b, _] = x.to_le_bytes();
            *px = resvg::tiny_skia::ColorU8::from_rgba(r, g, b, 128).premultiply();
        }
        p
    }

    #[test]
    fn size_budget_lowers_quality_until_it_fits() {
        let p = noise();
        let full = encode(&p, Format::Jpg, 90, None).unwrap().bytes.len();
        let fit = encode(&p, Format::Jpg, 90, Some(full / 2)).unwrap();
        assert!(fit.bytes.len() <= full / 2 && fit.quality < 90);
    }

    #[test]
    fn export_warnings_name_render_diagnostics_of_the_subtree() {
        use kiwi_schema::Value as K;
        let object = |fields: Vec<(&'static str, K<'static>)>| {
            K::Object("Test", fields.into_iter().collect())
        };
        let size = || object(vec![("x", K::Float(20.0)), ("y", K::Float(20.0))]);
        let frame = object(vec![
            ("type", K::Enum("NodeType", "FRAME")),
            ("size", size()),
        ]);
        let tiled = object(vec![
            ("type", K::Enum("NodeType", "RECTANGLE")),
            ("size", size()),
            (
                "fillPaints",
                K::Array(vec![object(vec![
                    ("type", K::Enum("PaintType", "IMAGE")),
                    ("imageScaleMode", K::Enum("ScaleMode", "TILE")),
                ])]),
            ),
        ]);
        let plain = object(vec![
            ("type", K::Enum("NodeType", "RECTANGLE")),
            ("size", size()),
            (
                "fillPaints",
                K::Array(vec![object(vec![
                    ("type", K::Enum("PaintType", "SOLID")),
                    (
                        "color",
                        object(vec![("r", K::Float(0.0)), ("a", K::Float(1.0))]),
                    ),
                ])]),
            ),
        ]);
        let tree = crate::Tree {
            nodes: std::collections::HashMap::from([
                ("1:1".into(), &frame),
                ("1:2".into(), &tiled),
                ("1:3".into(), &plain),
            ]),
            children: std::collections::HashMap::from([(
                "1:1".into(),
                vec!["1:2".into(), "1:3".into()],
            )]),
            blobs: Vec::new(),
        };
        // No image hash, so nothing is read from the file.
        let warnings = |id: &str| {
            let fig = Path::new("unread.fig");
            let (_, report) =
                export(&tree, fig, id, Format::Svg, false, 1.0, 80, None, None, &[]).unwrap();
            report["warnings"].clone()
        };
        let frame = warnings("1:1");
        assert_eq!(frame.as_array().unwrap().len(), 1, "{frame}");
        assert!(
            frame[0]
                .as_str()
                .unwrap()
                .starts_with("Render approximate at node 1:2 (fillPaints[0]"),
            "{frame}"
        );
        assert_eq!(warnings("1:3"), json!([]));
    }

    #[test]
    fn bounds_report_names_only_what_the_renderer_draws_past_the_box() {
        let warning = |n: Json, drawn: render::Rect| {
            bounds_report(&n, Some(drawn), false)["warnings"][0]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let node = |kind: &str, r: Json, children: Json| {
            json!({ "type": kind, "_r": r, "children": children,
                "absoluteBoundingBox": { "x": 0, "y": 0, "width": 10, "height": 10 },
                "absoluteRenderBounds": { "x": 0, "y": 0, "width": 12, "height": 12 } })
        };
        let shadow = node(
            "FRAME",
            json!({ "effects": [{ "type": "DROP_SHADOW" }, { "type": "INNER_SHADOW" }] }),
            json!([]),
        );
        assert!(warning(shadow, [0.0, 0.0, 12.0, 12.0]).contains("from effects;"));
        // An inner shadow and a stroke with no outward alignment stay inside the box.
        let overflow = node(
            "FRAME",
            json!({ "effects": [{ "type": "INNER_SHADOW" }], "strokeWeight": 2,
                "strokePaints": [{ "type": "SOLID" }] }),
            json!([{ "absoluteRenderBounds": { "x": 5, "y": 5, "width": 7, "height": 7 } }]),
        );
        assert!(warning(overflow, [0.0, 0.0, 12.0, 12.0]).contains("from child geometry;"));
        let mut underline = node(
            "FRAME",
            json!({ "strokeWeight": 0, "strokeAlign": "OUTSIDE", "strokePaints": [{ "type": "SOLID" }] }),
            json!([]),
        );
        underline["individualStrokeWeights"] =
            json!({ "top": 0, "right": 0, "bottom": 2, "left": 0 });
        assert!(warning(underline, [0.0, 0.0, 10.0, 12.0]).contains("from stroke;"));
        let section = node("SECTION", json!({}), json!([]));
        let margin = warning(section, [-40.0, -40.0, 52.0, 52.0]);
        assert!(
            margin.contains("top 40, right 42, bottom 42, left 40 px"),
            "{margin}"
        );
        assert!(margin.contains("from section margin;"), "{margin}");
    }

    #[test]
    fn png_cannot_meet_a_budget_it_exceeds() {
        assert!(encode(&noise(), Format::Png, 80, Some(100)).is_err());
    }

    #[test]
    fn png_budget_uses_the_losslessly_optimized_size() {
        let mut p = Pixmap::new(128, 128).unwrap();
        p.fill(resvg::tiny_skia::Color::from_rgba8(30, 120, 180, 255));
        let optimized = encode(&p, Format::Png, 80, None).unwrap();
        let budget = optimized.bytes.len();
        assert!(p.encode_png().unwrap().len() > budget);
        assert_eq!(
            encode(&p, Format::Png, 1, Some(budget)).unwrap().bytes,
            optimized.bytes
        );
        assert!(encode(&p, Format::Png, 100, Some(budget - 1)).is_err());
    }

    #[test]
    fn every_format_encodes() {
        let p = noise();
        let magic = |f| encode(&p, f, 80, None).unwrap().bytes[..12].to_vec();
        assert_eq!(
            (
                &magic(Format::Png)[1..4],
                &magic(Format::Jpg)[..2],
                &magic(Format::Webp)[8..12],
                &magic(Format::Avif)[4..12]
            ),
            (
                &b"PNG"[..],
                &[0xff, 0xd8][..],
                &b"WEBP"[..],
                &b"ftypavif"[..]
            )
        );
    }

    #[test]
    fn svg_index_matches_exact_outlines_in_subfolders_and_ranks_similar_shapes() {
        let dir = crate::test_support::TestDir::new();
        let file = |name: &str, body: &str| {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(
                path,
                format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{body}</svg>"#),
            )
            .unwrap();
        };
        // The export: a 20px square ring drawn as an outline, away from the origin.
        let ring = r#"<path fill-rule="evenodd" d="M100 50H120V70H100ZM102 52V68H118V52Z"/>"#;
        file(
            "deep/same.svg",
            r#"<path fill="red" d="m0 0h20v20h-20zm2 2v16h16v-16z"/>"#,
        );
        file(
            "double.svg",
            r#"<path fill-rule="evenodd" d="M0 0H40V40H0ZM4 4V36H36V4Z"/>"#,
        );
        file(
            "stroked.svg",
            r#"<rect x="1" y="1" width="18" height="18" fill="none" stroke="black" stroke-width="2"/>"#,
        );
        file("dot.svg", r#"<circle cx="10" cy="10" r="10"/>"#);
        file("broken.svg", "<path");
        let index = SvgIndex::load(&dir.join("")).unwrap();
        let export = format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{ring}</svg>"#);
        let (matches, similar) = index.compare(export.as_bytes(), None);
        assert_eq!(matches, ["deep/same.svg"]);
        let files: Vec<_> = similar
            .iter()
            .map(|s| s["file"].as_str().unwrap())
            .collect();
        // The same outlines at twice the size score 1; the stroked copy covers the same pixels.
        assert_eq!(files, ["double.svg", "stroked.svg"]);
        assert_eq!(similar[0]["score"], 1.0);
        let own = std::fs::canonicalize(dir.join("deep/same.svg")).unwrap();
        assert!(index.compare(export.as_bytes(), Some(&own)).0.is_empty());
    }

    #[test]
    fn svg_index_keeps_exact_matching_for_shapes_too_thin_to_rasterize() {
        let dir = crate::test_support::TestDir::new();
        let rule =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1000" height="1"/></svg>"#;
        std::fs::write(dir.join("rule.svg"), rule).unwrap();
        let index = SvgIndex::load(&dir.join("")).unwrap();
        let moved = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect x="5" y="9" width="1000" height="1"/></svg>"#;
        assert_eq!(index.compare(moved.as_bytes(), None).0, ["rule.svg"]);
    }

    #[test]
    fn svg_index_does_not_call_clipped_geometry_an_exact_match() {
        let dir = crate::test_support::TestDir::new();
        let svg = |body: &str| format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{body}</svg>"#);
        std::fs::write(
            dir.join("circle.svg"),
            svg(r#"<circle cx="10" cy="10" r="10"/>"#),
        )
        .unwrap();
        let index = SvgIndex::load(&dir.join("")).unwrap();
        // A layout-bounds export clips the overflowing half of the same circle.
        let half = svg(
            r#"<defs><clipPath id="a"><rect width="10" height="20"/></clipPath></defs><g clip-path="url(#a)"><circle cx="10" cy="10" r="10"/></g>"#,
        );
        assert!(index.compare(half.as_bytes(), None).0.is_empty());
    }

    #[test]
    fn svg_index_charges_skipped_oversized_files_to_the_byte_budget() {
        let dir = crate::test_support::TestDir::new();
        for i in 0..3 {
            std::fs::write(dir.join(&format!("big{i}.svg")), vec![b' '; (1 << 20) + 10]).unwrap();
        }
        let err = SvgIndex::load_within(&dir.join(""), 2 << 20).err().unwrap();
        assert!(err.to_string().contains("more than"), "{err}");
    }

    #[test]
    fn svg_index_keeps_even_odd_holes_apart_from_nonzero_fills() {
        let dir = crate::test_support::TestDir::new();
        let rings = "M0 0H20V20H0Z M5 5H15V15H5Z";
        let svg = |rule: &str| {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg"><path fill-rule="{rule}" d="{rings}"/></svg>"#
            )
        };
        std::fs::write(dir.join("solid.svg"), svg("nonzero")).unwrap();
        let index = SvgIndex::load(&dir.join("")).unwrap();
        assert!(index.compare(svg("evenodd").as_bytes(), None).0.is_empty());
        assert_eq!(
            index.compare(svg("nonzero").as_bytes(), None).0,
            ["solid.svg"]
        );
    }

    #[test]
    fn svg_index_keeps_a_stroked_border_apart_from_the_filled_square() {
        let dir = crate::test_support::TestDir::new();
        let svg = |paint: &str| {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg"><path {paint} d="M0 0H20V20H0Z"/></svg>"#
            )
        };
        std::fs::write(
            dir.join("border.svg"),
            svg(r#"fill="none" stroke="black" stroke-width="2""#),
        )
        .unwrap();
        let index = SvgIndex::load(&dir.join("")).unwrap();
        assert!(index.compare(svg("").as_bytes(), None).0.is_empty());
    }

    #[test]
    fn svg_index_matches_a_gradient_fill_with_the_same_solid_outline() {
        let dir = crate::test_support::TestDir::new();
        let square = r#"<path d="M0 0H20V20H0Z" fill="black"/>"#;
        std::fs::write(
            dir.join("solid.svg"),
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{square}</svg>"#),
        )
        .unwrap();
        let index = SvgIndex::load(&dir.join("")).unwrap();
        let gradient = r#"<svg xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="g"><stop offset="0"/><stop offset="1" stop-color="white"/></linearGradient></defs><path d="M0 0H20V20H0Z" fill="url(#g)"/></svg>"#;
        assert_eq!(index.compare(gradient.as_bytes(), None).0, ["solid.svg"]);
    }

    #[test]
    fn svg_index_matches_a_translucent_export_with_the_same_opaque_outline() {
        let dir = crate::test_support::TestDir::new();
        let svg = |paint: &str| {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg"><path {paint} d="M0 0H20V20H0ZM4 4V16H16V4Z"/></svg>"#
            )
        };
        std::fs::write(dir.join("opaque.svg"), svg("")).unwrap();
        let index = SvgIndex::load(&dir.join("")).unwrap();
        assert_eq!(
            index
                .compare(svg(r#"fill-opacity="0.1""#).as_bytes(), None)
                .0,
            ["opaque.svg"]
        );
    }

    #[test]
    fn svg_index_matches_mixed_opacity_paths_with_the_same_opaque_outlines() {
        let dir = crate::test_support::TestDir::new();
        let svg = |paint: &str| {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0H20V20H0Z"/><path {paint} d="M40 0H60V20H40Z"/></svg>"#
            )
        };
        std::fs::write(dir.join("opaque.svg"), svg("")).unwrap();
        let index = SvgIndex::load(&dir.join("")).unwrap();
        assert_eq!(
            index
                .compare(svg(r#"fill-opacity="0.1""#).as_bytes(), None)
                .0,
            ["opaque.svg"]
        );
    }
}
