//! Instance expansion: a `.fig` stores an instance as one node, so its children
//! are rebuilt from the main component plus the instance's overrides, with the
//! `I<instance>;<path>` ids the Figma plugin reports.

use std::collections::HashMap;

use kiwi_schema::Value;
use serde_json::{Map, Value as Json};

use crate::{Tree, guid_key, to_json};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    fn times(self, k: f64) -> Vec2 {
        Vec2 {
            x: self.x * k,
            y: self.y * k,
        }
    }
}

/// A node transform: linear part `a b c d` plus translation `tx ty`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Mat {
    pub const IDENTITY: Mat = Mat {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// The transform with its translation times `k`; the linear part keeps rotation and flips.
    fn moved(self, k: f64) -> Mat {
        Mat {
            tx: self.tx * k,
            ty: self.ty * k,
            ..self
        }
    }

    pub fn from_value(v: &Value) -> Mat {
        let f = |name: &str| v.get(name).map_or(0.0, |x| x.as_float() as f64);
        Mat {
            a: f("m00"),
            b: f("m10"),
            c: f("m01"),
            d: f("m11"),
            tx: f("m02"),
            ty: f("m12"),
        }
    }

    pub fn apply(self, p: Vec2) -> Vec2 {
        Vec2 {
            x: self.a * p.x + self.c * p.y + self.tx,
            y: self.b * p.x + self.d * p.y + self.ty,
        }
    }

    pub fn inverse(self) -> Mat {
        let det = self.a * self.d - self.b * self.c;
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Mat {
            a,
            b,
            c,
            d,
            tx: -(a * self.tx + c * self.ty),
            ty: -(b * self.tx + d * self.ty),
        }
    }

    /// Top-left corner of the axis-aligned box around a `size` box after this transform.
    pub fn origin(self, s: Vec2) -> Vec2 {
        Vec2 {
            x: self.tx + (self.a * s.x).min(0.0) + (self.c * s.y).min(0.0),
            y: self.ty + (self.b * s.x).min(0.0) + (self.d * s.y).min(0.0),
        }
    }

    /// Size of the axis-aligned box around a `size` box after this transform.
    pub fn bbox(self, s: Vec2) -> Vec2 {
        Vec2 {
            x: self.a.abs() * s.x + self.c.abs() * s.y,
            y: self.b.abs() * s.x + self.d.abs() * s.y,
        }
    }
}

/// `self * t`: `t` is the child's transform inside this space.
impl std::ops::Mul for Mat {
    type Output = Mat;

    fn mul(self, t: Mat) -> Mat {
        Mat {
            a: self.a * t.a + self.c * t.b,
            b: self.b * t.a + self.d * t.b,
            c: self.a * t.c + self.c * t.d,
            d: self.b * t.c + self.d * t.d,
            tx: self.a * t.tx + self.c * t.ty + self.tx,
            ty: self.b * t.tx + self.d * t.ty + self.ty,
        }
    }
}

/// A node produced by expanding an instance.
#[derive(Debug)]
pub struct Expanded {
    /// The main component's node this one copies.
    pub source: String,
    pub name: String,
    pub kind: String,
    pub size: Option<Vec2>,
    pub matrix: Mat,
    /// Effective placement and overridden kiwi fields, applied over `source`'s own.
    pub props: Map<String, Json>,
    pub(crate) prototype_source: Option<crate::prototype::Source>,
    /// For an INSTANCE: the main component expansion followed, present or not, and whether a
    /// swap (`swap`) or the copied definition's binding (`definition`) named it.
    pub(crate) main: Option<(String, &'static str)>,
    /// The composed `uniformScaleFactor` `source`'s own lengths were read at; 1 outside
    /// K-scaled instances. A text style the node links takes it too.
    pub scale: f64,
}

/// Most layers a scope may hold below its root, stored and expanded together. Measured real
/// files reach 21 stored levels; a release build aborts near 900 on Windows' 1 MiB main-thread
/// stack, so a deeper crafted file fails with an error instead of ending the process.
pub const MAX_DEPTH: usize = 256;

/// `parse budget exceeded: layer depth` once a node `children` reaches lies more than
/// `MAX_DEPTH` layers below `root`. It walks without recursion, so run it before a walk that
/// recurses once per layer.
pub fn check_depth<'a>(
    root: &'a str,
    children: impl Fn(&str) -> Option<&'a Vec<String>>,
) -> Result<(), crate::FigError> {
    let mut stack = vec![(root, 0)];
    while let Some((id, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(crate::parse::exceeded("layer depth", MAX_DEPTH));
        }
        stack.extend(
            children(id)
                .into_iter()
                .flatten()
                .map(|c| (c.as_str(), depth + 1)),
        );
    }
    Ok(())
}

/// Expanded nodes plus their child order (keys: instance or expanded ids).
pub struct Expansion {
    pub nodes: HashMap<String, Expanded>,
    pub children: HashMap<String, Vec<String>>,
}

/// Fields an instance can override or Figma can derive for a nested node.
#[derive(Clone, Default)]
struct Patch {
    name: Option<String>,
    visible: Option<bool>,
    size: Option<Vec2>,
    transform: Option<Mat>,
    swap: Option<String>,
    props: Map<String, Json>,
    prototype_source: Option<crate::prototype::Source>,
}

impl Patch {
    fn from_value(v: &Value) -> Patch {
        Patch {
            name: v.get("name").map(|n| n.as_string().to_owned()),
            visible: v.get("visible").map(Value::as_bool),
            size: v.get("size").map(vec2),
            transform: v.get("transform").map(Mat::from_value),
            swap: v.get("overriddenSymbolID").and_then(guid_key),
            props: match to_json(v) {
                Json::Object(mut m) => {
                    m.remove("guidPath");
                    m
                }
                _ => Map::new(),
            },
            prototype_source: None,
        }
    }

    /// Field-wise merge where `self` wins over `inner`.
    fn over(self, inner: Patch) -> Patch {
        Patch {
            name: self.name.or(inner.name),
            visible: self.visible.or(inner.visible),
            size: self.size.or(inner.size),
            transform: self.transform.or(inner.transform),
            swap: self.swap.or(inner.swap),
            props: {
                // An outer outline was drawn at its own scale, not at the inner one's.
                let mut props: Map<String, Json> = inner.props;
                for (f, _, mark) in GEOMETRY {
                    if self.props.contains_key(f) {
                        props.remove(mark);
                    }
                }
                props.extend(self.props);
                props
            },
            prototype_source: self.prototype_source.or(inner.prototype_source),
        }
    }
}

/// Outline fields, the size their paths were drawn at, and the internal mark of the factor a
/// scaled row's outline was not drawn at.
const GEOMETRY: [(&str, &str, &str); 2] = [
    ("fillGeometry", "fillGeometrySize", "fillGeometryScale"),
    (
        "strokeGeometry",
        "strokeGeometrySize",
        "strokeGeometryScale",
    ),
];

impl Patch {
    /// The patch as read at `k` times its stored scale: an instance's own rows sit at the
    /// scale of the main holding it, which a K-scaled outer instance multiplies.
    fn scaled(mut self, k: f64) -> Patch {
        if k == 1.0 {
            return self;
        }
        self.size = self.size.map(|s| s.times(k));
        self.transform = self.transform.map(|t| t.moved(k));
        for (f, v) in self.props.iter_mut() {
            if let Some(s) = scale_value(f, v, k) {
                *v = s;
            }
        }
        // Outline paths stay as drawn; `node_props` sizes them from the node's size over `k`.
        for (f, _, mark) in GEOMETRY {
            if self.props.contains_key(f) {
                self.props.insert(mark.into(), k.into());
            }
        }
        self
    }
}

type Patches = HashMap<String, Patch>;
type Resize = Option<(Vec2, Vec2)>;

/// Factors outside this range are ignored, and composed factors are clamped to it.
const SCALE_RANGE: std::ops::RangeInclusive<f64> = 1e-3..=1e3;

/// Lengths the Scale tool (K) multiplies. A K-scaled instance keeps its own size and stores
/// `symbolData.uniformScaleFactor`; Figma draws its main's lengths times that factor
/// (checked against the values Figma saved in `derivedSymbolData`).
const SCALED: [&str; 23] = [
    "cornerRadius",
    "rectangleTopLeftCornerRadius",
    "rectangleTopRightCornerRadius",
    "rectangleBottomLeftCornerRadius",
    "rectangleBottomRightCornerRadius",
    "strokeWeight",
    "borderTopWeight",
    "borderRightWeight",
    "borderBottomWeight",
    "borderLeftWeight",
    "stackSpacing",
    "stackPadding",
    "stackCounterSpacing",
    "stackHorizontalPadding",
    "stackVerticalPadding",
    "stackPaddingRight",
    "stackPaddingBottom",
    "gridRowGap",
    "gridColumnGap",
    "fontSize",
    "paragraphSpacing",
    "paragraphIndent",
    "listSpacing",
];

/// Structured fields holding lengths: dashes, effect radii and offsets, pixel letter spacing
/// and line height, min/max sizes, and the text ranges' own lengths.
const SCALED_PARTS: [&str; 7] = [
    "dashPattern",
    "effects",
    "letterSpacing",
    "lineHeight",
    "minSize",
    "maxSize",
    "textData",
];

// The helpers below stay out of line so `Expander::expand`, which recurses once per level up
// to `MAX_DEPTH`, keeps its frame small.

/// The `props` of an expanded copy of `child` that looks like `source` (itself, or the main
/// a swap names), with override `o` and derived row `d`, reading `source`'s lengths at `k`,
/// for a copy of `size`.
#[inline(never)]
fn node_props(
    child: &Value,
    source: &Value,
    o: &Patch,
    d: &Patch,
    k: f64,
    size: Option<Vec2>,
) -> Map<String, Json> {
    // Keep placement complete for semantic readers, including fields absent on the child.
    let mut props: Map<String, Json> = PLACEMENT
        .into_iter()
        .chain(GRID_PLACEMENT)
        .filter_map(|f| Some((f.to_owned(), to_json(child.get(f)?))))
        .collect();
    props.extend(scaled_lengths(source, k));
    props.extend(o.props.clone());
    // Figma re-derives the outlines of resized and re-texted nodes, and the lengths of K-scaled
    // ones; without them the source's outlines are scaled from the size they were drawn at.
    // Size and transform are placed through `Expanded::size` and `matrix`.
    for (f, v) in &d.props {
        if !placed(f) {
            props.insert(f.clone(), v.clone());
        }
    }
    for (f, at, mark) in GEOMETRY {
        // The mark is internal: it never reaches the emitted props.
        props.remove(mark);
        let drawn = d.props.get(mark).and_then(Json::as_f64);
        if !d.props.contains_key(f) {
            if let Some(s) = source.get("size") {
                props.insert(at.into(), to_json(s));
            }
        } else if let (Some(k), Some(s)) = (drawn, size) {
            // A nested instance's own derived outline was drawn before the outer factor.
            let s = s.times(1.0 / k);
            props.insert(at.into(), serde_json::json!({"x": s.x, "y": s.y}));
        }
    }
    props
}

/// `node`'s own lengths at `k` times their scale; none at 1.
#[inline(never)]
fn scaled_lengths(node: &Value, k: f64) -> Map<String, Json> {
    if k == 1.0 {
        return Map::new();
    }
    SCALED
        .iter()
        .chain(&SCALED_PARTS)
        .filter_map(|f| Some(((*f).to_owned(), scale_value(f, &to_json(node.get(f)?), k)?)))
        .collect()
}

/// Estimated bytes of `scaled_lengths(node, k)`, charged as `expand` charges copied fields.
#[inline(never)]
fn lengths_bytes(node: &Value, k: f64, left: &mut usize) -> usize {
    if k == 1.0 {
        return 0;
    }
    let present = SCALED
        .iter()
        .chain(&SCALED_PARTS)
        .filter_map(|f| Some((*f, node.get(f)?)));
    sum_while(present, left, |(f, v), left| {
        f.len() + 24 + kiwi_bytes(v, left)
    })
}

/// A nested instance's own rows, read at the scale `k` of the main holding it.
#[inline(never)]
fn own_rows(list: Option<&Value>, source: Option<&str>, k: f64) -> Patches {
    if k == 1.0 {
        return entries(list, source);
    }
    entries(list, source)
        .into_iter()
        .map(|(key, p)| (key, p.scaled(k)))
        .collect()
}

/// Derived fields `expand` places through `Expanded::size` and `matrix` instead of `props`.
fn placed(field: &str) -> bool {
    matches!(field, "size" | "transform")
}

/// `node`'s valid `uniformScaleFactor`, else 1.
fn scale_factor(node: &Value) -> f64 {
    let f = node
        .get("symbolData")
        .and_then(|s| s.get("uniformScaleFactor"))
        .map_or(1.0, |v| v.as_float() as f64);
    if SCALE_RANGE.contains(&f) { f } else { 1.0 }
}

/// `field`'s value `v` at `k` times its scale, or `None` when it holds no length. Units in
/// percent, unit-box positions (progressive blur) and counts stay as they are.
pub(crate) fn scale_value(field: &str, v: &Json, k: f64) -> Option<Json> {
    let num = |v: &Json| v.as_f64().map(|x| x * k).filter(|x| x.is_finite());
    let vector = |v: &Json| -> Option<Json> {
        let mut o = v.as_object()?.clone();
        for axis in ["x", "y"] {
            if let Some(x) = o.get(axis).and_then(num) {
                o.insert(axis.into(), x.into());
            }
        }
        Some(Json::Object(o))
    };
    match field {
        f if SCALED.contains(&f) => num(v).map(Json::from),
        "size" => vector(v),
        "transform" => {
            let mut o = v.as_object()?.clone();
            for t in ["m02", "m12"] {
                if let Some(x) = o.get(t).and_then(num) {
                    o.insert(t.into(), x.into());
                }
            }
            Some(Json::Object(o))
        }
        "dashPattern" => Some(Json::Array(
            v.as_array()?
                .iter()
                .map(|d| num(d).map_or_else(|| d.clone(), Json::from))
                .collect(),
        )),
        "effects" => Some(Json::Array(
            v.as_array()?
                .iter()
                .map(|e| {
                    let Some(mut o) = e.as_object().cloned() else {
                        return e.clone();
                    };
                    for f in ["radius", "spread", "startRadius"] {
                        if let Some(x) = o.get(f).and_then(num) {
                            o.insert(f.into(), x.into());
                        }
                    }
                    for f in ["offset", "noiseSize"] {
                        if let Some(x) = o.get(f).and_then(vector) {
                            o.insert(f.into(), x);
                        }
                    }
                    Json::Object(o)
                })
                .collect(),
        )),
        "letterSpacing" | "lineHeight" if v["units"] == "PIXELS" => {
            let mut o = v.as_object()?.clone();
            o.insert("value".into(), num(&v["value"])?.into());
            Some(Json::Object(o))
        }
        "minSize" | "maxSize" => {
            let mut o = v.as_object()?.clone();
            o.insert("value".into(), vector(&v["value"])?);
            Some(Json::Object(o))
        }
        // Each text range's override entry holds the same lengths as the node.
        "textData" => {
            let mut o = v.as_object()?.clone();
            let ranges = o.get_mut("styleOverrideTable")?.as_array_mut()?;
            for entry in ranges.iter_mut().filter_map(Json::as_object_mut) {
                for (f, v) in entry.iter_mut().filter(|(f, _)| *f != "textData") {
                    if let Some(s) = scale_value(f, v, k) {
                        *v = s;
                    }
                }
            }
            Some(Json::Object(o))
        }
        _ => None,
    }
}

/// What stopped a bounded run first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Work,
    Bytes,
    Depth,
    Deadline,
}

/// Work left for a bounded run, spent before the visits, clones and allocations it pays for.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    /// Node visits plus override entries converted, merged or walked.
    pub work: usize,
    /// Estimated bytes converted, copied or retained, including a caller's own records.
    pub bytes: usize,
    /// Nested expansion depth.
    pub depth: usize,
    pub deadline: std::time::Instant,
    pub stop: Option<Stop>,
}

impl Bounds {
    /// Spends `work` and `bytes`; false, with `stop` set, once any bound has run out.
    pub fn charge(&mut self, work: usize, bytes: usize) -> bool {
        if self.stop.is_none() {
            self.stop = if std::time::Instant::now() >= self.deadline {
                Some(Stop::Deadline)
            } else if work > self.work {
                Some(Stop::Work)
            } else if bytes > self.bytes {
                Some(Stop::Bytes)
            } else {
                self.work -= work;
                self.bytes -= bytes;
                return true;
            };
        }
        false
    }
}

/// Sums `f` over `items` until `left` runs out, without enumerating the rest of a collection.
pub(crate) fn sum_while<T>(
    items: impl IntoIterator<Item = T>,
    left: &mut usize,
    mut f: impl FnMut(T, &mut usize) -> usize,
) -> usize {
    let mut total = 0;
    for item in items {
        if *left == 0 {
            break;
        }
        total += f(item, left);
    }
    total
}

/// A cheap upper estimate of `v`'s retained size, computed without allocating; it visits at
/// most `left` values, so a payload larger than the remaining work stops early.
pub(crate) fn json_bytes(v: &Json, left: &mut usize) -> usize {
    if *left == 0 {
        return 0;
    }
    *left -= 1;
    match v {
        Json::String(s) => s.len() + 24,
        Json::Array(a) => 24 + sum_while(a, left, json_bytes),
        Json::Object(o) => {
            48 + sum_while(o, left, |(k, v), left| k.len() + 24 + json_bytes(v, left))
        }
        _ => 16,
    }
}

/// `json_bytes` of `v` once converted, estimated from the kiwi value without converting it.
pub(crate) fn kiwi_bytes(v: &Value, left: &mut usize) -> usize {
    if *left == 0 {
        return 0;
    }
    *left -= 1;
    match v {
        // Enum members convert to an owned string of their name.
        Value::String(s) => s.len() + 24,
        Value::Enum(_, name) => name.len() + 24,
        Value::Bytes(b) => 24 + 4 * b.len(),
        Value::Array(a) => 24 + sum_while(a, left, kiwi_bytes),
        Value::Object(_, o) => {
            48 + sum_while(o, left, |(k, v), left| k.len() + 24 + kiwi_bytes(v, left))
        }
        _ => 16,
    }
}

fn patches_bytes(patches: &Patches, left: &mut usize) -> usize {
    sum_while(patches, left, |(k, p), left| {
        let owned = [p.name.as_ref(), p.swap.as_ref()]
            .into_iter()
            .flatten()
            .chain(p.prototype_source.as_ref().map(|s| &s.node_id))
            .map(String::len)
            .sum::<usize>();
        // Field names come from the file's schema, so a clone copies them at any length.
        let props = sum_while(&p.props, left, |(f, v), left| {
            f.len() + 24 + json_bytes(v, left)
        });
        128 + k.len() + owned + props
    })
}

/// `kiwi_bytes` of override or derived `list` plus the strings `entries` keeps outside each
/// patch's props: its name again, its swap id and the `source` id of a prototype override.
fn entries_bytes(list: Option<&Value>, source: &str, left: &mut usize) -> usize {
    let Some(Value::Array(items)) = list else {
        return 0;
    };
    24 + sum_while(items, left, |entry, left| {
        let name = entry.get("name").map_or(0, |n| n.as_string().len());
        let prototype = entry
            .get("prototypeInteractions")
            .map_or(0, |_| source.len());
        // A swap's guid key prints as at most "4294967295:4294967295".
        kiwi_bytes(entry, left) + name + 21 + prototype
    })
}

/// Bytes `bind_properties` converts and copies for property assignments: each converts once,
/// and each property reference may copy the largest one into its layer.
fn assignment_bytes(
    own: Option<&Value>,
    inherited: Option<&Json>,
    refs: usize,
    left: &mut usize,
) -> usize {
    let mut largest = 0;
    let own = match own {
        Some(Value::Array(items)) => &items[..],
        _ => &[],
    };
    let mut total = sum_while(own, left, |v, left| {
        let n = kiwi_bytes(v, left);
        largest = largest.max(n);
        n
    });
    let inherited = inherited.and_then(Json::as_array).into_iter().flatten();
    total += sum_while(inherited, left, |v, left| {
        let n = json_bytes(v, left);
        largest = largest.max(n);
        n
    });
    total.saturating_add(largest.saturating_mul(refs))
}

fn list_len(v: Option<&Value>) -> usize {
    match v {
        Some(Value::Array(items)) => items.len(),
        _ => 0,
    }
}

/// A stored `{x, y}` vector; a missing field is 0.
pub fn vec2(v: &Value) -> Vec2 {
    let f = |name: &str| v.get(name).map_or(0.0, |x| x.as_float() as f64);
    Vec2 {
        x: f("x"),
        y: f("y"),
    }
}

fn enum_name<'a>(v: &Value<'a>, field: &str) -> Option<&'a str> {
    match v.get(field) {
        Some(Value::Enum(_, name)) => Some(*name),
        _ => None,
    }
}

fn flag(v: &Value, field: &str) -> Option<bool> {
    v.get(field).map(Value::as_bool)
}

/// Plugin type names, including component-set and resize-to-fit frames.
pub fn plugin_type(node: &Value) -> String {
    match enum_name(node, "type").unwrap_or("?") {
        "FRAME" if flag(node, "isStateGroup") == Some(true) => "COMPONENT_SET",
        "FRAME" if flag(node, "resizeToFit") == Some(true) => "GROUP",
        "ROUNDED_RECTANGLE" => "RECTANGLE",
        "SYMBOL" => "COMPONENT",
        other => other,
    }
    .to_owned()
}

/// Placement fields an expanded node keeps even when its child lacks them.
pub(crate) const PLACEMENT: [&str; 5] = [
    "stackChildPrimaryGrow",
    "stackChildAlignSelf",
    "stackPositioning",
    "horizontalConstraint",
    "verticalConstraint",
];

/// How a grid child sits in its cell: kept across swaps like `PLACEMENT`, but not part of the
/// search fingerprint's placement (changing that would rebuild every index).
pub(crate) const GRID_PLACEMENT: [&str; 4] = [
    "gridRowSpan",
    "gridColumnSpan",
    "gridChildHorizontalAlign",
    "gridChildVerticalAlign",
];

/// `a:b;c:d` key of an override's `guidPath`.
fn path_key(entry: &Value) -> String {
    let guids = entry.get("guidPath").and_then(|p| p.get("guids"));
    match guids {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(guid_key)
            .collect::<Vec<_>>()
            .join(";"),
        _ => String::new(),
    }
}

/// One patch per guidPath; a path can have several entries (e.g. a swap, then a size), merged with the later one winning per field.
fn entries(list: Option<&Value>, source: Option<&str>) -> Patches {
    let mut patches = Patches::new();
    if let Some(Value::Array(items)) = list {
        for (index, entry) in items.iter().enumerate() {
            let key = path_key(entry);
            let previous = patches.remove(&key).unwrap_or_default();
            let mut patch = Patch::from_value(entry);
            if entry.get("prototypeInteractions").is_some() {
                patch.prototype_source = source.map(|node_id| crate::prototype::Source {
                    node_id: node_id.into(),
                    override_index: Some(index),
                });
            }
            patches.insert(key, patch.over(previous));
        }
    }
    patches
}

/// Adds a nested instance's own patches (relative to it, at `prefix`) under the outer ones.
/// Merges field by field: an outer entry that only carries other fields (e.g. fillGeometry) must not hide the inner size.
fn nest(outer: &Patches, prefix: &str, inner: Patches) -> Patches {
    let mut merged = outer.clone();
    for (k, patch) in inner {
        let full = format!("{prefix};{k}");
        let outer_patch = outer.get(&full).cloned().unwrap_or_default();
        merged.insert(full, outer_patch.over(patch));
    }
    merged
}

/// Resolves visibility, text and instance-swap component properties within one instance.
fn bind_properties(
    tree: &Tree,
    symbol: &str,
    prefix: &str,
    mut overrides: Patches,
    instance: &Value,
    patch: &Patch,
) -> Patches {
    // Only a list is read, so anything else is not converted.
    let own = match instance.get("componentPropAssignments") {
        Some(list @ Value::Array(_)) => to_json(list),
        _ => Json::Null,
    };
    let assignments: Vec<_> = own
        .as_array()
        .into_iter()
        .flatten()
        .chain(
            patch
                .props
                .get("componentPropAssignments")
                .and_then(Json::as_array)
                .into_iter()
                .flatten(),
        )
        .collect();
    if assignments.is_empty() {
        return overrides;
    }
    for id in subtree(tree, symbol).into_iter().skip(1) {
        let child = tree.nodes[&id];
        let key = child.get("overrideKey").and_then(guid_key).unwrap_or(id);
        let key = if prefix.is_empty() {
            key
        } else {
            format!("{prefix};{key}")
        };
        if let Some(Value::Array(refs)) = child.get("componentPropRefs") {
            for reference in refs {
                let Some(def) = reference.get("defID").map(to_json) else {
                    continue;
                };
                let Some(assignment) = assignments.iter().rev().find(|a| a["defID"] == def) else {
                    continue;
                };
                let value = assignment
                    .get("varValue")
                    .and_then(|v| v.get("value"))
                    .or_else(|| assignment.get("value"));
                let patch = overrides.entry(key.clone()).or_default();
                if enum_name(reference, "componentPropNodeField") == Some("TEXT_DATA")
                    && !patch.props.contains_key("textData")
                    && let Some(text) = [&assignment["varValue"]["value"], &assignment["value"]]
                        .into_iter()
                        .flat_map(|v| [&v["textDataValue"], &v["textValue"]])
                        .find(|v| v["characters"].is_string() || v.is_string())
                {
                    let mut data = match child.get("textData").map(to_json) {
                        Some(Json::Object(data)) => data,
                        _ => Map::new(),
                    };
                    if let Some(fields) = text.as_object() {
                        data.extend(fields.clone());
                    } else {
                        data.insert("characters".into(), text.clone());
                    }
                    patch.props.insert("textData".into(), Json::Object(data));
                }
                if enum_name(reference, "componentPropNodeField") == Some("VISIBLE")
                    && patch.visible.is_none()
                    && let Some(visible) = value.and_then(|v| v["boolValue"].as_bool())
                {
                    patch.visible = Some(visible);
                    patch.props.insert("visible".into(), Json::Bool(visible));
                }
                if enum_name(reference, "componentPropNodeField") == Some("OVERRIDDEN_SYMBOL_ID")
                    && patch.swap.is_none()
                    && let Some(guid) = value
                        .and_then(|v| v.get("symbolIdValue"))
                        .and_then(|s| s.get("guid"))
                    && let (Some(session), Some(local)) =
                        (guid["sessionID"].as_u64(), guid["localID"].as_u64())
                {
                    patch.swap = Some(format!("{session}:{local}"));
                }
            }
        }
    }
    overrides
}

/// Resizes `size` by the child's constraints when its container changed size.
fn constrain(size: Option<Vec2>, h: Option<&str>, v: Option<&str>, resize: Resize) -> Option<Vec2> {
    let (Some(s), Some((from, to))) = (size, resize) else {
        return size;
    };
    let axis = |c: Option<&str>, len: f64, from: f64, to: f64| match c {
        Some("SCALE") => len * (to / from),
        Some("STRETCH") => len + (to - from),
        _ => len,
    };
    Some(Vec2 {
        x: axis(h, s.x, from.x, to.x),
        y: axis(v, s.y, from.y, to.y),
    })
}

/// A resize from `from` to `to`, or none when they are equal. A K-scaled size is an f32 factor
/// times an f32 length, a few ulps off the size Figma stored for an unresized instance, so
/// sizes within a millionth of each other are equal.
fn resized(from: Option<Vec2>, to: Option<Vec2>) -> Resize {
    let near = |a: f64, b: f64| (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0);
    match (from, to) {
        (Some(f), Some(t)) if !(near(f.x, t.x) && near(f.y, t.y)) => Some((f, t)),
        _ => None,
    }
}

/// The id of `node`'s stored parent.
pub fn parent_of(node: &Value) -> Option<String> {
    node.get("parentIndex")?.get("guid").and_then(guid_key)
}

/// `root` and every descendant, in tree order.
pub fn subtree(tree: &Tree, root: &str) -> Vec<String> {
    subtree_while(tree, root, || true)
}

/// `subtree`, stopping before the first node `more` refuses.
fn subtree_while(tree: &Tree, root: &str, mut more: impl FnMut() -> bool) -> Vec<String> {
    let mut out = vec![root.to_owned()];
    // A crafted file can make a node its own descendant; visit each id once.
    let mut seen: std::collections::HashSet<String> = out.iter().cloned().collect();
    let mut i = 0;
    while i < out.len() {
        for c in tree.children.get(&out[i]).into_iter().flatten() {
            if !seen.contains(c) && !more() {
                return out;
            }
            if seen.insert(c.clone()) {
                out.push(c.clone());
            }
        }
        i += 1;
    }
    out
}

/// Product of transforms from `id` up to (excluding) `root`.
pub fn abs_matrix(tree: &Tree, id: &str, root: &str) -> Mat {
    abs_matrix_while(tree, id, root, || true).unwrap_or(Mat::IDENTITY)
}

/// `abs_matrix`, asking `more` before each ancestor step; None once it says stop.
fn abs_matrix_while(
    tree: &Tree,
    id: &str,
    root: &str,
    mut more: impl FnMut() -> bool,
) -> Option<Mat> {
    let mut m = Mat::IDENTITY;
    let mut cur = Some(id.to_owned());
    while let Some(node) = cur
        .as_deref()
        .filter(|c| *c != root)
        .and_then(|c| tree.nodes.get(c))
    {
        if !more() {
            return None;
        }
        if let Some(t) = node.get("transform") {
            m = Mat::from_value(t) * m;
        }
        cur = parent_of(node);
    }
    Some(m)
}

pub(crate) fn main_of<'t>(tree: &Tree<'t>, instance: &Value) -> Option<String> {
    instance
        .get("symbolData")?
        .get("symbolID")
        .and_then(guid_key)
        .filter(|id| tree.nodes.contains_key(id))
}

/// What `bind_properties` walks and may copy for one component: stored nodes, text bytes,
/// property references and the bytes of the layer keys it builds, once per layer and once
/// more per reference on it.
#[derive(Clone, Copy, Default)]
struct Cost {
    nodes: usize,
    text: usize,
    refs: usize,
    keys: usize,
    /// Converted bytes of every reference's `defID`, which the file's schema can make any size.
    defs: usize,
}

impl Cost {
    /// Bytes binding assignments under `prefix` builds: a key per walked layer, kept for each
    /// reference it fills.
    fn binding(&self, assignments: usize, prefix: &str) -> usize {
        if assignments == 0 {
            return 0;
        }
        (self.nodes + self.refs)
            .saturating_mul(prefix.len() + 1)
            .saturating_add(self.keys)
    }

    /// Work converting each reference id once per binding and comparing it with `assignments`
    /// assignments: one visit per 64 bytes, since an equality check reads at most the smaller
    /// id. Each converted id is dropped before the next, so it is work, not retained bytes.
    fn comparisons(&self, assignments: usize) -> usize {
        if assignments == 0 {
            return 0;
        }
        assignments.saturating_add(1).saturating_mul(self.defs / 64)
    }
}

struct Expander<'t, 'a> {
    tree: &'t Tree<'a>,
    out: Expansion,
    /// The components being expanded, outermost first: one that contains itself stops here.
    symbols: Vec<String>,
    /// Bytes left for the `id`, `name` and `type` every expanded node emits; `None` is unbounded.
    budget: Option<usize>,
    /// Shared work bounds of `expand_instances_bounded`; `None` is unbounded.
    bounds: Option<Bounds>,
    depth: usize,
    /// `symbol_cost` per component.
    sizes: HashMap<String, Cost>,
    exceeded: bool,
}

/// Expands every visible instance under `root` into `I<instance>;<path>` nodes.
pub fn expand_instances(tree: &Tree, root: &str) -> Expansion {
    expand_all(tree, root, None, None).0
}

/// `expand_instances`, or `None` once the expanded nodes' emitted `id`, `name` and `type`
/// alone exceed `bytes`: a lower bound of `root`'s REST JSON, so a fitting scope still expands.
pub fn expand_instances_within(tree: &Tree, root: &str, bytes: usize) -> Option<Expansion> {
    let (ex, _, exceeded) = expand_all(tree, root, Some(bytes), None);
    (!exceeded).then_some(ex)
}

/// `expand_instances` that spends `bounds` before each stored visit, override conversion or
/// merge, expanded node and nesting level, and keeps what it expanded when they run out
/// (`Bounds::stop`). Stored children still win over an expansion, as in REST output.
pub fn expand_instances_bounded(tree: &Tree, root: &str, bounds: Bounds) -> (Expansion, Bounds) {
    let (ex, bounds, _) = expand_all(tree, root, None, Some(bounds));
    (ex, bounds.unwrap())
}

fn expand_all(
    tree: &Tree,
    root: &str,
    budget: Option<usize>,
    bounds: Option<Bounds>,
) -> (Expansion, Option<Bounds>, bool) {
    let mut ex = Expander {
        tree,
        out: Expansion {
            nodes: HashMap::new(),
            children: HashMap::new(),
        },
        symbols: Vec::new(),
        budget,
        bounds,
        depth: 0,
        sizes: HashMap::new(),
        exceeded: false,
    };
    let stored = subtree_while(tree, root, || ex.spend(1, 0));
    // Parents come before their children, so visibility is inherited in one pass.
    let mut hidden = std::collections::HashSet::new();
    for id in stored {
        if ex.exceeded {
            break;
        }
        let inst = tree.nodes[&id];
        if flag(inst, "visible") == Some(false)
            || (id != root && parent_of(inst).is_some_and(|p| hidden.contains(&p)))
        {
            hidden.insert(id);
            continue;
        }
        if enum_name(inst, "type") != Some("INSTANCE") {
            continue;
        }
        // REST output keeps stored children over an expansion, so a bounded run skips it unread.
        if (budget.is_some() || ex.bounds.is_some()) && tree.children.contains_key(&id) {
            continue;
        }
        let Some(main) = main_of(tree, inst) else {
            continue;
        };
        let sd = inst.get("symbolData");
        if ex.bounds.is_some() {
            let cost = ex.symbol_cost(&main);
            let Cost {
                nodes, text, refs, ..
            } = cost;
            let (overrides, derived, assignments) = (
                sd.and_then(|s| s.get("symbolOverrides")),
                inst.get("derivedSymbolData"),
                inst.get("componentPropAssignments"),
            );
            let work = [
                nodes,
                list_len(overrides),
                list_len(derived),
                refs.saturating_mul(list_len(assignments)),
                cost.comparisons(list_len(assignments)),
            ]
            .into_iter()
            .fold(0, usize::saturating_add);
            let estimate = |left: &mut usize| {
                entries_bytes(overrides, &id, left)
                    + entries_bytes(derived, "", left)
                    + assignment_bytes(assignments, None, refs, left)
            };
            let binding = cost.binding(list_len(assignments), "");
            if ex
                .spend_estimated(work, text.saturating_add(binding), estimate)
                .is_none()
            {
                break;
            }
        }
        let ov = bind_properties(
            tree,
            &main,
            "",
            entries(sd.and_then(|s| s.get("symbolOverrides")), Some(&id)),
            inst,
            &Patch::default(),
        );
        let dv = entries(inst.get("derivedSymbolData"), None);
        // Each instance walks its ancestors for placement; bounded runs pay per step.
        let Some(matrix) = abs_matrix_while(tree, &id, root, || ex.spend(1, 0)) else {
            break;
        };
        // A K-scaled instance fits its main from the main's scaled size, not its stored one.
        let scale = scale_factor(inst);
        let resize = resized(
            tree.nodes[&main].get("size").map(|s| vec2(s).times(scale)),
            inst.get("size").map(vec2),
        );
        ex.symbols = vec![main.clone()];
        ex.expand(&id, &id, &main, &[], &ov, &dv, matrix, resize, scale);
    }
    (ex.out, ex.bounds, ex.exceeded)
}

impl Expander<'_, '_> {
    /// Charges the shared bounds, if any; false once they ran out.
    fn spend(&mut self, work: usize, bytes: usize) -> bool {
        if let Some(b) = &mut self.bounds
            && !b.charge(work, bytes)
        {
            self.exceeded = true;
            return false;
        }
        true
    }

    /// Enters one more nesting level; false once a bounded run is too deep.
    fn descend(&mut self) -> bool {
        if let Some(b) = &mut self.bounds
            && self.depth >= b.depth
        {
            b.stop.get_or_insert(Stop::Depth);
            self.exceeded = true;
            return false;
        }
        // Past the limit `rest::build` rejects what was inserted, unless stored children win.
        if self.depth >= MAX_DEPTH {
            return false;
        }
        self.depth += 1;
        true
    }

    /// Spends `work` and `bytes` plus what `estimate` measures; the estimate's value visits
    /// are charged as work and stop at the remaining work. The estimated bytes, or None.
    fn spend_estimated(
        &mut self,
        work: usize,
        bytes: usize,
        estimate: impl FnOnce(&mut usize) -> usize,
    ) -> Option<usize> {
        let start = self.bounds.as_ref()?.work.saturating_add(1);
        let mut left = start;
        let estimated = estimate(&mut left);
        self.spend(
            work.saturating_add(start - left),
            bytes.saturating_add(estimated),
        )
        .then_some(estimated)
    }

    /// `Cost` of `symbol`, measured once per component by walks charged as they go. Only
    /// bounded runs measure.
    fn symbol_cost(&mut self, symbol: &str) -> Cost {
        if self.bounds.is_none() {
            return Cost::default();
        }
        if let Some(cost) = self.sizes.get(symbol) {
            return *cost;
        }
        let tree = self.tree;
        let ids = subtree_while(tree, symbol, || self.spend(1, 0));
        // A nested instance may name a component the file lacks; it expands to nothing.
        let present = || {
            ids.iter()
                .filter_map(|id| tree.nodes.get(id).map(|n| (id, n)))
        };
        let texts = present().filter_map(|(_, n)| n.get("textData"));
        let text = self
            .spend_estimated(0, 0, |left| texts.map(|t| kiwi_bytes(t, left)).sum())
            .unwrap_or(0);
        let refs = present()
            .map(|(_, n)| list_len(n.get("componentPropRefs")))
            .sum();
        // An overrideKey guid prints as at most "4294967295:4294967295".
        let keys = present()
            .map(|(id, n)| {
                let key = if n.get("overrideKey").is_some() {
                    21
                } else {
                    id.len()
                };
                key.saturating_mul(1 + list_len(n.get("componentPropRefs")))
            })
            .fold(0usize, usize::saturating_add);
        let defs = present()
            .filter_map(|(_, n)| match n.get("componentPropRefs") {
                Some(Value::Array(refs)) => Some(refs),
                _ => None,
            })
            .flatten()
            .filter_map(|r| r.get("defID"));
        // Measured, not kept: only the walk is charged.
        let start = self.bounds.as_ref().map_or(0, |b| b.work).saturating_add(1);
        let mut left = start;
        let defs = sum_while(defs, &mut left, kiwi_bytes);
        self.spend(start - left, 0);
        let cost = Cost {
            nodes: ids.len(),
            text,
            refs,
            keys,
            defs,
        };
        self.sizes.insert(symbol.to_owned(), cost);
        cost
    }

    /// `path` holds only instance boundaries: Figma ids skip plain frames in between.
    /// `scale` multiplies `symbol`'s own lengths; `ov` and `dv` are already at the outer
    /// instance's scale.
    #[allow(clippy::too_many_arguments)]
    fn expand(
        &mut self,
        outer: &str,
        parent: &str,
        symbol: &str,
        path: &[String],
        ov: &Patches,
        dv: &Patches,
        m: Mat,
        resize: Resize,
        scale: f64,
    ) {
        let tree = self.tree;
        for child_id in tree.children.get(symbol).into_iter().flatten() {
            if self.exceeded {
                return;
            }
            let child = tree.nodes[child_id];
            // Library component copies keep the library's ids in overrideKey, and paths use those.
            let key = child
                .get("overrideKey")
                .and_then(guid_key)
                .unwrap_or_else(|| child_id.clone());
            let p: Vec<String> = path.iter().cloned().chain([key]).collect();
            let pk = p.join(";");
            let none = Patch::default();
            let o = ov.get(&pk).unwrap_or(&none);
            let d = dv.get(&pk).unwrap_or(&none);
            // Swaps reset the inherited name; variants use their component set's name.
            let swapped = o
                .swap
                .as_ref()
                .filter(|_| o.name.is_none())
                .and_then(|id| tree.nodes.get(id))
                .and_then(|main| {
                    let set = parent_of(main)
                        .and_then(|id| tree.nodes.get(&id).copied())
                        .filter(|p| flag(p, "isStateGroup") == Some(true));
                    set.unwrap_or(main).get("name")
                });
            let name = o
                .name
                .as_deref()
                .or(swapped.map(Value::as_string))
                .unwrap_or_else(|| child.get("name").map_or("", Value::as_string));
            if self.bounds.is_some() {
                let source = o.swap.as_ref().filter(|s| tree.nodes.contains_key(*s));
                let derived = d.props.iter().filter(|(f, _)| !placed(f));
                let own = tree.nodes[source.unwrap_or(child_id)];
                let placement = PLACEMENT
                    .iter()
                    .chain(&GRID_PLACEMENT)
                    .filter_map(|f| Some((*f, child.get(f)?)));
                // A geometry-size entry copies the source size only where Figma derived no outline.
                let copies = GEOMETRY
                    .iter()
                    .filter(|(f, _, mark)| !d.props.contains_key(*f) || d.props.contains_key(*mark))
                    .count();
                let size = tree.nodes[source.unwrap_or(child_id)]
                    .get("size")
                    .filter(|_| copies > 0);
                // Field names and enum members come from the file's schema, so every converted
                // value and copied key is charged at its length.
                let estimate = |left: &mut usize| {
                    sum_while(&o.props, left, |(f, v), left| {
                        f.len() + 24 + json_bytes(v, left)
                    }) + sum_while(derived, left, |(f, v), left| {
                        f.len() + 24 + json_bytes(v, left)
                    }) + sum_while(placement, left, |(f, v), left| {
                        f.len() + 24 + kiwi_bytes(v, left)
                    }) + lengths_bytes(own, scale, left)
                        + size.map_or(0, |s| copies * (48 + kiwi_bytes(s, left)))
                };
                // Kept per node: the `I{outer};{pk}` id twice (child list and node key), the
                // name and kind, the source and prototype source ids, and the parent's
                // child-list key.
                let prototype = o.prototype_source.as_ref().map(|s| &s.node_id);
                let parent_key = if self.out.children.contains_key(parent) {
                    0
                } else {
                    parent.len()
                };
                let kept = 2 * (outer.len() + pk.len() + 2)
                    + name.len()
                    + enum_name(child, "type").map_or(1, str::len)
                    + source.unwrap_or(child_id).len()
                    + prototype.unwrap_or(child_id).len()
                    + parent_key;
                if self.spend_estimated(1, 200 + kept, estimate).is_none() {
                    return;
                }
            }
            let kind = enum_name(child, "type");
            let own_size = child.get("size").map(|s| vec2(s).times(scale));
            // Auto-layout owns normal-flow child geometry; constraints only apply to absolute children.
            let resize = if matches!(
                enum_name(tree.nodes[symbol], "stackMode"),
                Some("HORIZONTAL" | "VERTICAL")
            ) && enum_name(child, "stackPositioning") != Some("ABSOLUTE")
            {
                None
            } else {
                resize
            };
            // Groups and boolean ops have no constraints of their own: children resolve against the outer container.
            // shortcut: the container takes its first child's constraints instead of re-fitting the children's bounds; fit bounds once positions are tracked.
            let transparent = kind == Some("BOOLEAN_OPERATION")
                || (kind == Some("FRAME") && flag(child, "resizeToFit") == Some(true));
            let size = d.size.or(o.size).or_else(|| {
                let first = tree
                    .children
                    .get(child_id)
                    .and_then(|c| c.first())
                    .filter(|_| transparent);
                let src = first.map_or(child, |f| tree.nodes[f]);
                constrain(
                    own_size,
                    enum_name(src, "horizontalConstraint"),
                    enum_name(src, "verticalConstraint"),
                    resize,
                )
            });
            let child_resize = if transparent {
                resize
            } else {
                resized(own_size, size)
            };

            let name = name.to_owned();

            let matrix = m
                * (d.transform.or(o.transform).unwrap_or_else(|| {
                    child
                        .get("transform")
                        .map_or(Mat::IDENTITY, Mat::from_value)
                        .moved(scale)
                }));
            // A swapped instance looks like its new main; only how it sits in its parent stays its own.
            let source = o
                .swap
                .as_ref()
                .filter(|s| tree.nodes.contains_key(*s))
                .unwrap_or(child_id)
                .clone();
            let props = node_props(child, tree.nodes[&source], o, d, scale, size);
            let visible = o.visible.or_else(|| flag(tree.nodes[&source], "visible"));
            let id = format!("I{outer};{pk}");
            // Figma ids are unique; a repeat would make the tree contain itself, so skip it.
            if self.out.nodes.contains_key(&id) {
                continue;
            }
            let rest_type = plugin_type(child);
            if let Some(left) = self.budget {
                let emitted = serde_json::to_vec(
                    &serde_json::json!({"id": id, "name": name, "type": rest_type}),
                )
                .map_or(0, |v| v.len() - 2);
                match left.checked_sub(emitted) {
                    Some(left) => self.budget = Some(left),
                    None => {
                        self.exceeded = true;
                        return;
                    }
                }
            }
            self.out
                .children
                .entry(parent.to_owned())
                .or_default()
                .push(id.clone());
            let prototype_source = o.prototype_source.clone().or_else(|| {
                (source != *child_id
                    && kind == Some("INSTANCE")
                    && child.get("prototypeInteractions").is_some())
                .then(|| crate::prototype::Source {
                    node_id: child_id.clone(),
                    override_index: None,
                })
            });
            let main = (kind == Some("INSTANCE"))
                .then(|| match &o.swap {
                    Some(swap) => Some((swap.clone(), "swap")),
                    None => child
                        .get("symbolData")
                        .and_then(|s| s.get("symbolID"))
                        .and_then(guid_key)
                        .map(|id| (id, "definition")),
                })
                .flatten();
            self.out.nodes.insert(
                id.clone(),
                Expanded {
                    source,
                    name,
                    kind: rest_type,
                    size,
                    matrix,
                    props,
                    prototype_source,
                    main,
                    scale,
                },
            );
            // Inside an expanded instance, hidden nodes keep the node but omit its children.
            if visible == Some(false) {
                continue;
            }
            if kind == Some("INSTANCE") {
                let sd = child.get("symbolData");
                let Some(nested) = o
                    .swap
                    .clone()
                    .or_else(|| sd.and_then(|s| s.get("symbolID")).and_then(guid_key))
                else {
                    continue;
                };
                // Figma forbids a component containing itself; a crafted file would recurse forever.
                if self.symbols.contains(&nested) {
                    continue;
                }
                if !self.descend() {
                    return;
                }
                // Converting and merging the nested overrides copies the outer ones too.
                if self.bounds.is_some() {
                    let cost = self.symbol_cost(&nested);
                    let Cost {
                        nodes, text, refs, ..
                    } = cost;
                    let (overrides, derived, own, inherited) = (
                        sd.and_then(|s| s.get("symbolOverrides")),
                        child.get("derivedSymbolData"),
                        child.get("componentPropAssignments"),
                        o.props.get("componentPropAssignments"),
                    );
                    let assignments =
                        list_len(own) + inherited.and_then(Json::as_array).map_or(0, Vec::len);
                    let work = [
                        nodes,
                        ov.len(),
                        dv.len(),
                        list_len(overrides),
                        list_len(derived),
                        refs.saturating_mul(assignments),
                        cost.comparisons(assignments),
                    ]
                    .into_iter()
                    .fold(0, usize::saturating_add);
                    let estimate = |left: &mut usize| {
                        patches_bytes(ov, left)
                            + patches_bytes(dv, left)
                            + entries_bytes(overrides, child_id, left)
                            + entries_bytes(derived, "", left)
                            + assignment_bytes(own, inherited, refs, left)
                    };
                    // `nest` writes `pk;` in front of every inner key it merges.
                    let prefixes = (list_len(overrides) + list_len(derived))
                        .saturating_mul(pk.len() + 1)
                        .saturating_add(cost.binding(assignments, &pk));
                    if self
                        .spend_estimated(work, text.saturating_add(prefixes), estimate)
                        .is_none()
                    {
                        return;
                    }
                }
                // The nested instance's own rows sit at the scale of the main holding it.
                let overrides = own_rows(
                    sd.and_then(|s| s.get("symbolOverrides")),
                    Some(child_id),
                    scale,
                );
                let merged =
                    bind_properties(tree, &nested, &pk, nest(ov, &pk, overrides), child, o);
                let derived = own_rows(child.get("derivedSymbolData"), None, scale);
                let merged_derived = nest(dv, &pk, derived);
                let nested_scale =
                    (scale * scale_factor(child)).clamp(*SCALE_RANGE.start(), *SCALE_RANGE.end());
                let main_size = tree
                    .nodes
                    .get(&nested)
                    .and_then(|n| n.get("size"))
                    .map(|s| vec2(s).times(nested_scale));
                self.symbols.push(nested.clone());
                self.expand(
                    outer,
                    &id,
                    &nested,
                    &p,
                    &merged,
                    &merged_derived,
                    matrix,
                    resized(main_size, size),
                    nested_scale,
                );
                self.symbols.pop();
                self.depth -= 1;
            } else {
                if !self.descend() {
                    return;
                }
                self.expand(
                    outer,
                    &id,
                    child_id,
                    path,
                    ov,
                    dv,
                    matrix,
                    child_resize,
                    scale,
                );
                self.depth -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests;
