//! Figma REST-shaped `nodes.json` (the plugin's export) built from a `.fig`:
//! kiwi fields are renamed and reshaped into the REST fields design-to-code reads.

use std::cell::Cell;
use std::collections::HashMap;

use serde_json::{Map, Value as Json, json};

use crate::expand::{
    Expansion, Mat, Vec2, abs_matrix, check_depth, expand_instances, expand_instances_within,
    plugin_type,
};
use crate::render::{self, Rect};
use crate::{Tree, to_json};

mod annotations;
pub mod compact;
mod handoff;
mod semantics;

type Obj = Map<String, Json>;
/// Variables and variable sets by guid and by library key; a key can name several imported copies.
pub(crate) type Vars = HashMap<String, Vec<Json>>;
/// Variable set (library key, or guid when local) → the mode a node resolves it in.
type Modes = HashMap<String, String>;

/// `{"document": ...}` for the frame `root`, with instances expanded, plus the
/// `components`, `componentSets` and `styles` the document references.
pub fn nodes(tree: &Tree, root: &str) -> Json {
    build(tree, root, false, None).unwrap_or_else(|e| panic!("{e}"))
}

/// `nodes` with each node's render input under `_r`, for `render::Svg`.
pub fn render_nodes(tree: &Tree, root: &str) -> Json {
    build(tree, root, true, None).unwrap_or_else(|e| panic!("{e}"))
}

/// Select an exact stored or expanded ID, retaining its instance overrides and geometry.
pub fn resolve_nodes(tree: &Tree, id: &str) -> Result<Json, Box<dyn std::error::Error>> {
    resolve_target(tree, id, false, None, false, None)
}

/// `resolve_nodes`, or `resolve_render_nodes` when `render`, plus the node's `ancestor_path`;
/// an instance sublayer's path continues through its expanded parents.
pub fn resolve_nodes_with_path(
    tree: &Tree,
    id: &str,
    render: bool,
) -> Result<(Json, Json), Box<dyn std::error::Error>> {
    let mut expanded = Vec::new();
    let doc = resolve_target(tree, id, render, None, false, Some(&mut expanded))?;
    let mut path = ancestor_path(tree, crate::export::root_of(id));
    path.extend(expanded);
    Ok((doc, Json::Array(path)))
}

/// A stored node's ancestors from the document down to the node itself, as `[{id, name}]`.
pub fn ancestor_path(tree: &Tree, id: &str) -> Vec<Json> {
    let mut path = Vec::new();
    let mut at = Some(id.to_owned());
    while let Some(id) = at {
        let Some(node) = tree.nodes.get(&id) else {
            break;
        };
        let name = node.get("name").map(|v| v.as_string()).unwrap_or("");
        path.push(json!({"id": id, "name": name}));
        at = parent_id(tree, &id);
    }
    path.reverse();
    path
}

/// `resolve_nodes` that stops building, with `ResponseBudget`, once the selection's JSON exceeds `bytes`.
pub fn resolve_nodes_within(
    tree: &Tree,
    id: &str,
    bytes: usize,
) -> Result<Json, Box<dyn std::error::Error>> {
    resolve_target(tree, id, false, Some(bytes), false, None)
}

/// `resolve_nodes` with renderer input, for selected-node image inspection and export.
pub fn resolve_render_nodes(tree: &Tree, id: &str) -> Result<Json, Box<dyn std::error::Error>> {
    resolve_target(tree, id, true, None, false, None)
}

/// Opt-in lossless sharing, with a separate finite construction guard before encoding.
/// `bytes` limits the compact JSON, not the larger REST representation it resolves to.
pub fn resolve_compact_nodes(
    tree: &Tree,
    id: &str,
    subtrees: bool,
    bytes: Option<usize>,
) -> Result<Json, Box<dyn std::error::Error>> {
    let data = resolve_target(tree, id, false, Some(64 * 1024 * 1024), true, None)?;
    let result = compact::encode(data, subtrees);
    if bytes.is_some_and(|limit| compact::serialized_size(&result, limit) > limit) {
        return Err(crate::FigError::new(
            crate::ErrorKind::ResponseBudget,
            "compact nodes exceeds response budget; select a smaller node or use CLI nodes",
        )
        .into());
    }
    Ok(result)
}

fn resolve_target(
    tree: &Tree,
    id: &str,
    render: bool,
    bytes: Option<usize>,
    compact: bool,
    path: Option<&mut Vec<Json>>,
) -> Result<Json, Box<dyn std::error::Error>> {
    let root = crate::export::root_of(id);
    if !tree.nodes.contains_key(root) {
        return Err(crate::FigError::new(
            crate::ErrorKind::NodeNotFound,
            format!("node {id} not found; list ids with `figlens tree <file.fig> --depth 2`"),
        )
        .into());
    }
    let budget = bytes.map(|bytes| Budget {
        target: id,
        bytes,
        used: Cell::new(0),
        inside: Cell::new(false),
        compact,
    });
    let mut doc = build(tree, root, render, budget.as_ref())?;
    if let Some(b) = budget.filter(Budget::exceeded) {
        if compact {
            return Err(crate::FigError::new(
                crate::ErrorKind::ResponseBudget,
                "compact nodes construction exceeds 64 MiB; select a smaller node",
            )
            .into());
        }
        return Err(crate::FigError::new(crate::ErrorKind::ResponseBudget, format!(
            "response exceeds {} bytes; use inspect, a scoped bundle, or CLI nodes instead of a larger MCP reply", b.bytes
        ))
        .into());
    }
    if root != id {
        // Expand the outer instance first; selecting the component's raw child loses context.
        let mut chain = Vec::new();
        if !render::find_path(&doc["document"], id, &mut chain) {
            return Err(crate::FigError::new(crate::ErrorKind::NodeNotFound, format!(
                "node {id} not found in expanded instance {root}; inspect {root} for available sublayer IDs"
            ))
            .into());
        }
        if let Some(path) = path {
            path.extend(
                chain[1..]
                    .iter()
                    .map(|n| json!({"id": n["id"], "name": n["name"].as_str().unwrap_or("")})),
            );
        }
        let selected = chain[chain.len() - 1].clone();
        doc["document"] = selected;
        handoff::select(&mut doc);
    }
    Ok(doc)
}

/// Counts each selected node's compact fields without children, `_r` or later bounds:
/// a lower bound of the selection's JSON, so exceeding it never rejects a reply that would fit.
struct Budget<'a> {
    target: &'a str,
    bytes: usize,
    used: Cell<usize>,
    /// Inside the selected node, which may be an expanded sublayer of the built root.
    inside: Cell<bool>,
    compact: bool,
}

impl Budget<'_> {
    fn exceeded(&self) -> bool {
        self.used.get() > self.bytes
    }
}

fn build(
    tree: &Tree,
    root: &str,
    render: bool,
    budget: Option<&Budget>,
) -> Result<Json, crate::FigError> {
    let mut styles: HashMap<String, Vec<String>> = HashMap::new();
    let mut dev_status = HashMap::new();
    let mut categories = HashMap::new();
    for (id, node) in &tree.nodes {
        if let (Some(_), Some(key)) = (node.get("styleType"), node.get("key")) {
            styles
                .entry(key.as_string().to_owned())
                .or_default()
                .push(id.clone());
        }
        // Each page records the Ready for dev status of its frames and sections.
        if let Some(kiwi_schema::Value::Array(entries)) =
            node.get("handoffStatusMap").and_then(|m| m.get("entries"))
        {
            for entry in entries {
                if let (Some(id), Some(status)) = (
                    entry.get("guid").and_then(crate::guid_key),
                    entry
                        .get("handoffStatus")
                        .and_then(|s| annotations::dev_status(&to_json(s))),
                ) {
                    dev_status.insert(id, status);
                }
            }
        }
        // The document lists its annotation categories.
        if let Some(stored) = node.get("annotationCategories") {
            categories.extend(annotations::categories(&to_json(stored)));
        }
    }
    // Every expansion under a selected stored root is emitted, so expansion can stop early too.
    let ex = if budget.is_some_and(|b| b.compact) {
        let (ex, bounds) = crate::expand::expand_instances_bounded(
            tree,
            root,
            crate::expand::Bounds {
                work: 2_000_000,
                bytes: 64 * 1024 * 1024,
                depth: crate::expand::MAX_DEPTH,
                deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
                stop: None,
            },
        );
        if bounds.stop.is_some() {
            return Err(crate::FigError::new(
                crate::ErrorKind::ResponseBudget,
                "compact nodes expansion limit reached; select a smaller node",
            ));
        }
        ex
    } else {
        match budget.filter(|b| b.target == root) {
            Some(b) => match expand_instances_within(tree, root, b.bytes) {
                Some(ex) => ex,
                None => {
                    b.used.set(b.bytes.saturating_add(1));
                    return Ok(Json::Null);
                }
            },
            None => expand_instances(tree, root),
        }
    };
    // Building, rendering and dropping the result all recurse once per layer.
    check_depth(root, |id| {
        tree.children.get(id).or_else(|| ex.children.get(id))
    })?;
    let b = Builder {
        tree,
        ex,
        // Expanded matrices are relative to `root`'s own space.
        base: abs_matrix(tree, root, ""),
        vars: variables(tree),
        styles,
        dev_status,
        render,
        budget,
    };
    // Explicit variable modes are inherited from ancestors above `root` too.
    let modes = parent_id(tree, root).map_or_else(Modes::new, |p| context(tree, &b.vars, &p));
    let document = b.node(root, None, &modes, true, None);
    if budget.is_some_and(Budget::exceeded) {
        return Ok(Json::Null);
    }
    let (mut components, mut sets, mut styles) = (Obj::new(), Obj::new(), Obj::new());
    let mut used_categories = Obj::new();
    let mut handoff = handoff::Inventory::default();
    let mut stack = vec![&document];
    while let Some(n) = stack.pop() {
        handoff.record(n);
        stack.extend(n["children"].as_array().into_iter().flatten());
        for id in n["annotations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| a["categoryId"].as_str())
        {
            if let (false, Some(category)) = (used_categories.contains_key(id), categories.get(id))
            {
                used_categories.insert(id.to_owned(), category.clone());
            }
        }
        if let Some(main) = n["componentId"]
            .as_str()
            .filter(|m| tree.nodes.contains_key(*m) && !components.contains_key(*m))
        {
            let meta = component_meta(tree, main);
            if let Some(set) = meta["componentSetId"]
                .as_str()
                .filter(|set| !sets.contains_key(*set))
            {
                let mut s = component_meta(tree, set);
                s.as_object_mut().map(|s| s.remove("documentationLinks"));
                sets.insert(set.to_owned(), s);
            }
            components.insert(main.to_owned(), meta);
        }
        for id in n["styles"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(_, v)| v.as_str())
        {
            let s = to_json(tree.nodes[id]);
            let mut meta = json!({
                "name": s["name"],
                "styleType": s["styleType"],
                // Only library copies carry a key; a local style's key is not stored in the .fig.
                "remote": s["key"].is_string(),
                "description": s["description"].as_str().unwrap_or(""),
            });
            if s["key"].is_string() {
                meta["key"] = s["key"].clone();
            }
            styles.insert(id.to_owned(), meta);
        }
    }
    let mut result = json!({
        "document": document,
        "components": components,
        "componentSets": sets,
        "styles": styles,
        "schemaVersion": 0,
    });
    // Like `styles`: the categories the emitted annotations name, as the Plugin API reads them.
    if !used_categories.is_empty() {
        result["annotationCategories"] = used_categories.into();
    }
    // Stated once here rather than on every node's `_figlens.semantics`.
    result["_figlens"] = json!({"semantics":{"verification":"not-compared-with-figma"}});
    if let Some(summary) = handoff.finish() {
        result["_figlens"]["handoffDiagnostics"] = summary;
    }
    Ok(result)
}

/// REST `components` / `componentSets` entry of a main component or component set.
fn component_meta(tree: &Tree, id: &str) -> Json {
    let n = to_json(tree.nodes[id]);
    let description = n["description"]
        .as_str()
        .or(n["symbolDescription"].as_str())
        .unwrap_or("");
    let mut meta = json!({
        "key": n["componentKey"],
        "name": n["name"],
        "description": description,
        "remote": n["sourceLibraryKey"].is_string(),
        "documentationLinks": [],
    });
    if let Some(set) = parent_id(tree, id)
        .filter(|p| tree.nodes[p].get("isStateGroup").map(to_json) == Some(json!(true)))
    {
        meta["componentSetId"] = json!(set);
    }
    let mut evidence = json!({});
    semantics::definitions(
        tree,
        id,
        n.as_object().unwrap(),
        meta.as_object_mut().unwrap(),
        &mut evidence,
    );
    if evidence.as_object().is_some_and(|e| !e.is_empty()) {
        meta["_figlens"] = json!({"semantics":evidence});
    }
    meta
}

/// Variable modes in effect at `id`: explicit modes of the node and its ancestors, inner ones winning.
fn context(tree: &Tree, vars: &Vars, id: &str) -> Modes {
    let mut chain = vec![id.to_owned()];
    while let Some(p) = parent_id(tree, &chain[chain.len() - 1]) {
        chain.push(p);
    }
    chain.iter().rev().fold(Modes::new(), |m, id| {
        let map = tree.nodes[id].get("variableModeBySetMap").map(to_json);
        with_modes(vars, map.as_ref().unwrap_or(&Json::Null), &m)
    })
}

fn parent_id(tree: &Tree, id: &str) -> Option<String> {
    let p = tree.nodes.get(id)?.get("parentIndex")?.get("guid")?;
    crate::guid_key(p).filter(|p| tree.nodes.contains_key(p))
}

/// Figma keeps variables and their sets, local or imported from libraries, at the top of a canvas.
pub(crate) fn variables(tree: &Tree) -> Vars {
    let mut out = Vars::new();
    for canvas in tree.children.get("0:0").into_iter().flatten() {
        for id in tree.children.get(canvas).into_iter().flatten() {
            let node = tree.nodes[id];
            let kind = node.get("type").map(to_json);
            if !kind
                .as_ref()
                .and_then(Json::as_str)
                .is_some_and(|t| t.starts_with("VARIABLE"))
            {
                continue;
            }
            let node = to_json(node);
            if let Some(key) = node["key"].as_str() {
                out.entry(key.to_owned()).or_default().push(node.clone());
            }
            out.entry(id.clone()).or_default().push(node);
        }
    }
    out
}

/// A local guid or library key reference, as a `Vars` key.
fn ref_id(r: &Json) -> Option<String> {
    r["assetRef"]["key"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| guid(&r["guid"]))
}

/// The copies `r` names, the copy of `r`'s library version first.
pub(crate) fn copies<'v>(vars: &'v Vars, r: &Json) -> Vec<&'v Json> {
    let mut list: Vec<&Json> = ref_id(r)
        .and_then(|id| vars.get(&id))
        .into_iter()
        .flatten()
        .collect();
    list.sort_by_key(|v| v["version"] != r["assetRef"]["version"]);
    list
}

/// Sets are matched by library key when they have one, since nodes may reference them either way.
fn set_id(vars: &Vars, r: &Json) -> Option<String> {
    let id = ref_id(r)?;
    let key = vars.get(&id).and_then(|s| s[0]["key"].as_str());
    Some(key.map_or(id, str::to_owned))
}

/// `modes` with a node's own explicit variable modes (`variableModeBySetMap`) on top.
fn with_modes(vars: &Vars, map: &Json, modes: &Modes) -> Modes {
    let mut out = modes.clone();
    for e in map["entries"].as_array().into_iter().flatten() {
        if let (Some(set), Some(mode)) = (
            set_id(vars, &e["variableSetID"]),
            guid(&e["variableModeID"]),
        ) {
            out.insert(set, mode);
        }
    }
    out
}

/// The color a variable binding (`colorVar`) takes in `modes`, following aliases; a set without
/// an explicit mode uses its first one by sort position. `None` keeps the cached color.
fn resolve(vars: &Vars, binding: &Json, modes: &Modes, depth: u8) -> Option<Json> {
    if depth > 16 {
        return None;
    }
    copies(vars, &binding["value"]["alias"])
        .into_iter()
        .find_map(|var| {
            let set = set_id(vars, &var["variableSetID"])?;
            let mode = modes.get(&set).cloned().or_else(|| {
                let first = copies(vars, &var["variableSetID"]).first()?["variableSetModes"]
                    .as_array()?
                    .iter()
                    .min_by(|a, b| a["sortPosition"].as_str().cmp(&b["sortPosition"].as_str()))?;
                guid(&first["id"])
            })?;
            let data = &var["variableDataValues"]["entries"]
                .as_array()?
                .iter()
                .find(|e| guid(&e["modeID"]).as_ref() == Some(&mode))?["variableData"];
            match data["dataType"].as_str()? {
                "ALIAS" => resolve(vars, data, modes, depth + 1),
                "COLOR" => Some(data["value"]["colorValue"].clone()),
                _ => None,
            }
        })
}

/// Replaces the cached colors of variable-bound paints with the colors they resolve to in `modes`.
fn bind_paints(vars: &Vars, paints: Option<&mut Json>, modes: &Modes) {
    for p in paints.and_then(Json::as_array_mut).into_iter().flatten() {
        if let Some(c) = resolve(vars, &p["colorVar"], modes, 0) {
            // A solid paint carries its variable's alpha as the paint opacity.
            p["opacity"] = c["a"].clone();
            for ch in ["r", "g", "b"] {
                p["color"][ch] = c[ch].clone();
            }
        }
        let stops: Vec<_> = p["stopsVar"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| resolve(vars, &s["colorVar"], modes, 0))
            .collect();
        for (i, c) in stops.into_iter().enumerate() {
            if let (Some(c), Some(stop)) = (c, p["stops"].get_mut(i)) {
                stop["color"] = c;
            }
        }
    }
}

struct Builder<'t, 'a> {
    tree: &'t Tree<'a>,
    ex: Expansion,
    base: Mat,
    vars: Vars,
    /// Library style key → the ids of its copies in this file.
    styles: HashMap<String, Vec<String>>,
    /// Keep each node's render input (`_r`) in the output.
    render: bool,
    budget: Option<&'t Budget<'t>>,
    /// Stored node id → REST `devStatus`.
    dev_status: HashMap<String, Json>,
}

impl Builder<'_, '_> {
    /// REST `styles`: style kind → the id of the style node (a library style's local copy).
    fn style_refs(&self, k: &Obj) -> Obj {
        let mut out = Obj::new();
        for (kiwi, rest) in [
            ("styleIdForText", "text"),
            ("styleIdForFill", "fill"),
            ("styleIdForStrokeFill", "stroke"),
            ("styleIdForEffect", "effect"),
            ("styleIdForGrid", "grid"),
        ] {
            let r = at(k, kiwi);
            let id = match r["assetRef"]["key"].as_str() {
                Some(key) => self.styles.get(key).and_then(|ids| {
                    let version = r["assetRef"]["version"].as_str();
                    ids.iter()
                        .find(|id| {
                            self.tree.nodes[*id].get("version").map(|v| v.as_string()) == version
                        })
                        .or(ids.first())
                        .cloned()
                }),
                None => guid(&r["guid"]).filter(|id| self.tree.nodes.contains_key(id)),
            };
            if let Some(id) = id {
                out.insert(rest.into(), json!(id));
            }
        }
        out
    }

    /// Copies the font fields of the text style `r` applies into `k`: a node's own copies can
    /// be stale, and the plugin reports the style's.
    fn apply_text_style(&self, k: &mut Obj, r: &Obj) {
        let Some(style) = self
            .style_refs(r)
            .get("text")
            .and_then(Json::as_str)
            .map(|id| self.tree.nodes[id])
        else {
            return;
        };
        for f in ["fontName", "fontSize", "letterSpacing", "lineHeight"] {
            if let Some(v) = style.get(f) {
                k.insert(f.into(), to_json(v));
            }
        }
    }

    /// `characterStyleOverrides`, `styleOverrideTable`, `lineTypes` and `lineIndentations`.
    /// An override entry lists the fields its kiwi entry sets; one that applies a text style
    /// lists the style's fields too.
    fn text_overrides(&self, k: &Obj, out: &mut Obj) {
        let td = at(k, "textData");
        let ids = td["characterStyleIDs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut table = Obj::new();
        for e in td["styleOverrideTable"].as_array().into_iter().flatten() {
            let (Some(sid), Some(fields)) = (e["styleID"].as_u64(), e.as_object()) else {
                continue;
            };
            if !ids.iter().any(|i| i.as_u64() == Some(sid)) {
                continue;
            }
            let mut merged = k.clone();
            let mut set: Vec<&str> = Vec::new();
            let style = self
                .style_refs(fields)
                .get("text")
                .and_then(Json::as_str)
                .map(str::to_owned);
            if let Some(id) = &style {
                for (f, v) in object(to_json(self.tree.nodes[id.as_str()])) {
                    if !override_fields(&f).is_empty() {
                        set.extend(override_fields(&f));
                        merged.insert(f, v);
                    }
                }
            }
            for (f, v) in fields {
                set.extend(override_fields(f));
                merged.insert(f.clone(), v.clone());
            }
            let full = text_style(&merged);
            let mut entry: Obj = set
                .into_iter()
                .filter_map(|f| Some((f.to_owned(), full.get(f)?.clone())))
                .collect();
            if let Some(p) = fields.get("fillPaints") {
                entry.insert("fills".into(), paints(Some(p)));
            }
            if let Some(v) = fields.get("isOverrideOverTextStyle") {
                entry.insert("isOverrideOverTextStyle".into(), v.clone());
            }
            if let Some(id) = style {
                entry.insert("inheritTextStyleId".into(), json!(id));
            }
            table.insert(sid.to_string(), Json::Object(entry));
        }
        let lines = td["lines"].as_array().cloned().unwrap_or_default();
        let line_type = |l: &Json| match l["lineType"].as_str() {
            Some("ORDERED_LIST") => json!("ORDERED"),
            Some("UNORDERED_LIST") => json!("UNORDERED"),
            _ => json!("NONE"),
        };
        out.insert("characterStyleOverrides".into(), Json::Array(ids));
        out.insert("styleOverrideTable".into(), Json::Object(table));
        out.insert("lineTypes".into(), lines.iter().map(line_type).collect());
        out.insert(
            "lineIndentations".into(),
            lines
                .iter()
                .map(|l| {
                    l["indentationLevel"]
                        .as_u64()
                        .map_or(json!(0), |v| json!(v))
                })
                .collect(),
        );
    }

    /// `rendered`: no ancestor hides this node or consumes it as a boolean operand;
    /// `clip`: the page region its clipping ancestors leave visible.
    fn node(
        &self,
        id: &str,
        parent: Option<&Obj>,
        modes: &Modes,
        rendered: bool,
        clip: Option<Rect>,
    ) -> Json {
        if self.budget.is_some_and(Budget::exceeded) {
            return Json::Null;
        }
        let entered = self
            .budget
            .is_some_and(|b| b.target == id && !b.inside.replace(true));
        let (kiwi, kind, name, matrix) = match self.ex.nodes.get(id) {
            Some(e) => {
                let mut k = object(to_json(self.tree.nodes[&e.source]));
                k.extend(e.props.clone());
                // The text style wins over cached font fields, except on hidden sublayers,
                // which keep their overridden values (checked against the plugin sweep).
                if e.props.get("visible") != Some(&json!(false)) {
                    let style_ref = k.clone();
                    self.apply_text_style(&mut k, &style_ref);
                }
                if let Some(s) = e.size {
                    k.insert("size".into(), json!({ "x": s.x, "y": s.y }));
                }
                (k, e.kind.clone(), e.name.clone(), self.base * e.matrix)
            }
            None => {
                let node = self.tree.nodes[id];
                let mut k = object(to_json(node));
                let style_ref = k.clone();
                self.apply_text_style(&mut k, &style_ref);
                let name = at(&k, "name").as_str().unwrap_or("").to_owned();
                (k, plugin_type(node), name, abs_matrix(self.tree, id, ""))
            }
        };
        let mut kiwi = kiwi;
        let modes = with_modes(&self.vars, at(&kiwi, "variableModeBySetMap"), modes);
        // A direct node's cached colors are already resolved in its own context; inside an
        // instance they are the main component's, so they are resolved again here.
        if self.ex.nodes.contains_key(id) {
            for f in ["fillPaints", "strokePaints"] {
                bind_paints(&self.vars, kiwi.get_mut(f), &modes);
            }
        }
        let mut out = fields(&kiwi, &kind, parent);
        let root_parent = parent
            .is_none()
            .then(|| parent_id(self.tree, id))
            .flatten()
            .map(|p| object(to_json(self.tree.nodes[&p])));
        if !matches!(kind.as_str(), "DOCUMENT" | "CANVAS" | "SECTION") {
            // A selected stored root still has a parent; don't alter the legacy sizing path.
            let placement = self.ex.nodes.get(id).map_or(&kiwi, |e| &e.props);
            let mut evidence =
                semantics::layout(placement, parent.or(root_parent.as_ref()), &mut out);
            // A grid frame's tracks are its own (merged kiwi); a child's span and alignment are
            // placement, kept from before a swap.
            let tracks = semantics::grid(
                &kiwi,
                placement,
                parent.or(root_parent.as_ref()),
                &mut out,
                &mut evidence,
            );
            if let Some(expanded) = self.ex.nodes.get(id) {
                evidence["instanceContext"] = json!({"instanceId": crate::export::root_of(id), "sourceNodeId": expanded.source, "state":"effective"});
            }
            if matches!(kind.as_str(), "COMPONENT" | "COMPONENT_SET") {
                semantics::definitions(self.tree, id, &kiwi, &mut out, &mut evidence);
            }
            if evidence.as_object().is_some_and(|e| !e.is_empty()) {
                out.insert("_figlens".into(), json!({"semantics":evidence}));
            }
            if let Some(tracks) = tracks {
                out.entry("_figlens".to_owned())
                    .or_insert_with(|| json!({}))["gridTracks"] = tracks;
            }
        }
        handoff::fields(&kiwi, &kind, parent.or(root_parent.as_ref()), &mut out);
        annotations::node(&kiwi, &mut out);
        if let Some(status) = self.dev_status.get(id) {
            out.insert("devStatus".into(), status.clone());
        }
        if let Some(prototype) = crate::prototype::node(self.tree, id, self.ex.nodes.get(id), &kind)
        {
            out.entry("_figlens".to_owned())
                .or_insert_with(|| json!({}))["prototype"] = prototype;
        }
        for list in out
            .get_mut("boundVariables")
            .and_then(Json::as_object_mut)
            .into_iter()
            .flat_map(|b| b.values_mut())
            .filter_map(Json::as_array_mut)
        {
            list.sort_by_cached_key(|a| variable_order(a["id"].as_str().unwrap_or("")));
        }
        if let Some(main) = out.get("componentId").and_then(Json::as_str) {
            let props = PropDefs::read(self.tree, main).map_or_else(Obj::new, |d| {
                d.resolve(at(&kiwi, "componentPropAssignments"))
            });
            if !props.is_empty() {
                out.insert("componentProperties".into(), Json::Object(props));
            }
        }
        if kind == "TEXT" {
            self.text_overrides(&kiwi, &mut out);
        }
        let mut styles = self.style_refs(&kiwi);
        // A text style's own font-size and weight bindings show on the nodes that use it.
        if let Some(style) = styles.get("text").and_then(Json::as_str) {
            let style_bound = bound_variables(&object(to_json(self.tree.nodes[style])));
            let bound = out
                .entry("boundVariables")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .unwrap();
            for f in ["fontSize", "fontWeight"] {
                if let (Some(v), false) = (style_bound.get(f), bound.contains_key(f)) {
                    bound.insert(f.into(), v.clone());
                }
            }
            if bound.is_empty() {
                out.remove("boundVariables");
            }
        }
        // Frame-like nodes repeat their fill style under the legacy `fills` key.
        if let Some(fill) = styles.get("fill").filter(|_| {
            matches!(
                kind.as_str(),
                "FRAME" | "INSTANCE" | "COMPONENT" | "COMPONENT_SET"
            )
        }) {
            styles.insert("fills".into(), fill.clone());
        }
        if !styles.is_empty() {
            out.insert("styles".into(), Json::Object(styles));
        }
        out.insert("id".into(), json!(id));
        out.insert("name".into(), json!(name));
        out.insert("type".into(), json!(kind));
        if at(&kiwi, "size").is_object() {
            let s = vec2(at(&kiwi, "size"));
            let (o, b) = (matrix.origin(s), matrix.bbox(s));
            out.insert(
                "absoluteBoundingBox".into(),
                json!({ "x": o.x, "y": o.y, "width": b.x, "height": b.y }),
            );
        }
        if let Some(b) = self.budget.filter(|b| b.inside.get()) {
            b.used
                .set(b.used.get() + serde_json::to_vec(&out).map_or(0, |v| v.len()));
        }
        out.insert("_r".into(), render_input(&kiwi, matrix));
        let mut node = Json::Object(out);
        let shown = rendered
            && node["visible"] != json!(false)
            && kiwi.get("opacity").and_then(Json::as_f64) != Some(0.0);
        let base = shown
            .then(|| render::base_bounds(self.tree, &node))
            .flatten();
        let own = base.map(|b| render::grow_by_effects(&node, b));
        let (clips, own_box) = (
            node["clipsContent"] == json!(true),
            render::box_bounds(&node),
        );
        // Children of a clipping box see only the part of it their ancestors leave visible.
        let (kids_clip, clipped_away) = match (clips, own_box) {
            (true, Some(b)) => match clip {
                Some(c) => render::intersect(c, b).map_or((None, true), |r| (Some(r), false)),
                None => (Some(b), false),
            },
            _ => (clip, false),
        };
        let children = self
            .tree
            .children
            .get(id)
            .or_else(|| self.ex.children.get(id));
        let mut bounds = own;
        // Bounds no ancestor clips: a filter must see content its ancestors cut away.
        let mut kids_ub = None;
        if let Some(children) = children {
            let kids_shown = shown && kind != "BOOLEAN_OPERATION" && !clipped_away;
            let mut kids_clip = kids_clip;
            let mut list: Vec<Json> = Vec::new();
            for c in children {
                let mut child = self.node(c, Some(&kiwi), &modes, kids_shown, kids_clip);
                // A mask draws nothing itself and limits the siblings above it to its shape.
                if let Some(m) = child.as_object_mut().and_then(|o| o.remove("_mask")) {
                    let m = render::render_bounds(&json!({ "absoluteRenderBounds": m }));
                    kids_clip = match (kids_clip, m) {
                        (Some(c), Some(m)) => render::intersect(c, m).or(Some([0.0; 4])),
                        (c, m) => m.or(c),
                    };
                }
                list.push(child);
            }
            for c in &list {
                bounds = render::union(bounds, render::render_bounds(c));
                kids_ub = render::union(kids_ub, render::rect_of(&c["_r"]["ub"]));
            }
            node["children"] = Json::Array(list);
        }
        if entered {
            self.budget.unwrap().inside.set(false);
        }
        // Only rendering reads these; `_r` is dropped otherwise.
        if self.render {
            if clips {
                kids_ub = kids_ub
                    .zip(own_box)
                    .and_then(|(k, b)| render::intersect(k, b));
            }
            node["_r"]["fb"] = render::rect_json(render::union(base, kids_ub));
            node["_r"]["ub"] = render::rect_json(render::union(own, kids_ub));
        }
        let bounds = match clip {
            Some(c) => bounds.and_then(|b| render::intersect(b, c)),
            None => bounds,
        };
        if kind == "CANVAS" {
            node.as_object_mut()
                .map(|o| o.remove("absoluteRenderBounds"));
        } else if node["isMask"] == json!(true) {
            node["_mask"] = render::rect_json(own.or(bounds));
            node["absoluteRenderBounds"] = Json::Null;
        } else {
            node["absoluteRenderBounds"] = render::rect_json(bounds);
        }
        if shown && bounds.is_some() {
            let diagnostics = render::diagnostics(self.tree, &node);
            if !diagnostics.is_empty() {
                if node.get("_figlens").is_none() {
                    node["_figlens"] = json!({});
                }
                node["_figlens"]["renderDiagnostics"] = json!(diagnostics);
            }
        }
        if !self.render {
            node.as_object_mut().map(|o| o.remove("_r"));
        }
        node
    }
}

/// Per-side stroke weights in the REST shape, when the node sets them independently. A side
/// the `.fig` leaves out is 0: its saved stroke geometry draws nothing there.
fn individual_stroke_weights(k: &Obj) -> Option<Json> {
    (k.get("borderStrokeWeightsIndependent") == Some(&json!(true))).then(|| {
        let side = |s: &str| {
            k.get(&format!("border{s}Weight"))
                .and_then(Json::as_f64)
                .unwrap_or(0.0)
        };
        json!({ "top": side("Top"), "right": side("Right"), "bottom": side("Bottom"), "left": side("Left") })
    })
}

/// What `render` needs of a node beyond its REST fields, with its page transform.
fn render_input(k: &Obj, matrix: Mat) -> Json {
    let text = at(k, "textData");
    json!({
        "m": render::mat_json(matrix),
        "size": at(k, "size"),
        "fillGeometrySize": k.get("fillGeometrySize").unwrap_or(at(k, "size")),
        "strokeGeometrySize": k.get("strokeGeometrySize").unwrap_or(at(k, "size")),
        "fillPaints": at(k, "fillPaints"),
        "strokePaints": at(k, "strokePaints"),
        "fillGeometry": at(k, "fillGeometry"),
        "vectorStyles": at(k, "vectorData")["styleOverrideTable"],
        "strokeGeometry": at(k, "strokeGeometry"),
        "strokeWeight": at(k, "strokeWeight"),
        "strokeAlign": at(k, "strokeAlign"),
        "dashPattern": at(k, "dashPattern"),
        "effects": at(k, "effects"),
        "glyphs": at(k, "derivedTextData")["glyphs"],
        "truncationStartIndex": at(k, "derivedTextData")["truncationStartIndex"],
        "characterStyleIDs": text["characterStyleIDs"],
        "styleOverrideTable": text["styleOverrideTable"],
    })
}

fn object(v: Json) -> Obj {
    match v {
        Json::Object(m) => m,
        _ => Obj::new(),
    }
}

/// `k[field]` without the panic `Map` indexing has on a missing key.
fn at<'a>(k: &'a Obj, field: &str) -> &'a Json {
    k.get(field).unwrap_or(&Json::Null)
}

fn vec2(v: &Json) -> Vec2 {
    Vec2 {
        x: v["x"].as_f64().unwrap_or(0.0),
        y: v["y"].as_f64().unwrap_or(0.0),
    }
}

pub(crate) fn guid(v: &Json) -> Option<String> {
    Some(format!(
        "{}:{}",
        v["sessionID"].as_u64()?,
        v["localID"].as_u64()?
    ))
}

/// The definition fields REST `componentProperties` reads for instances of one main, converted
/// once: its own `componentPropDefs` and `variantPropSpecs`, and its component set's
/// `componentPropDefs` (variants define their properties there and point at them).
pub(crate) struct PropDefs {
    own: Json,
    specs: Json,
    set: Json,
}

impl PropDefs {
    /// The fields of `main`, `None` when it is not in the file.
    pub(crate) fn read(tree: &Tree, main: &str) -> Option<PropDefs> {
        let m = tree.nodes.get(main)?;
        let field = |n: &kiwi_schema::Value, f: &str| n.get(f).map_or(Json::Null, to_json);
        Some(PropDefs {
            own: field(m, "componentPropDefs"),
            specs: field(m, "variantPropSpecs"),
            set: parent_id(tree, main)
                .map_or(Json::Null, |p| field(tree.nodes[&p], "componentPropDefs")),
        })
    }

    /// `crate::expand::kiwi_bytes` of what `read` converts, estimated without converting it.
    pub(crate) fn bytes(tree: &Tree, main: &str, left: &mut usize) -> usize {
        use crate::expand::kiwi_bytes;
        let field = |n: &kiwi_schema::Value, f: &str, left: &mut usize| {
            n.get(f).map_or(0, |v| kiwi_bytes(v, left))
        };
        tree.nodes.get(main).map_or(0, |m| {
            field(m, "componentPropDefs", left)
                + field(m, "variantPropSpecs", left)
                + parent_id(tree, main)
                    .map_or(0, |p| field(tree.nodes[&p], "componentPropDefs", left))
        })
    }

    /// REST `componentProperties` of an instance whose `componentPropAssignments` are
    /// `assignments` (a later entry wins per definition): variant values from the main's name
    /// specs, other properties from the assignments or the definition's default.
    pub(crate) fn resolve(&self, assignments: &Json) -> Obj {
        self.resolve_with(assignments, |_, _| true)
            .expect("an unbounded resolution never stops")
    }

    /// `resolve`, showing `before` each lookup GUID, then each resolved definition with its
    /// chosen assignment (or variant spec), before copying them; `None` once it refuses.
    /// Many definitions can share one parent, so each copy is paid where it is made.
    pub(crate) fn resolve_with(
        &self,
        assignments: &Json,
        mut before: impl FnMut(&Json, Option<&Json>) -> bool,
    ) -> Option<Obj> {
        let mut out = Obj::new();
        let mut set_defs = HashMap::new();
        for d in self.set.as_array().into_iter().flatten() {
            if !before(&d["id"], None) {
                return None;
            }
            if let Some(id) = guid(&d["id"]) {
                set_defs.insert(id, d);
            }
        }
        let mut assigned = HashMap::new();
        for a in assignments.as_array().into_iter().flatten() {
            if !before(&a["defID"], None) {
                return None;
            }
            if let Some(id) = guid(&a["defID"]) {
                assigned.insert(id, a);
            }
        }
        for own in self.own.as_array().into_iter().flatten() {
            if !before(&own["id"], Some(&own["parentPropDefId"])) {
                return None;
            }
            let def = semantics::resolve_definition(own, &set_defs);
            let (Some(def_id), Some(own_id)) = (guid(&def["id"]), guid(&own["id"])) else {
                continue;
            };
            let kind = match def["type"].as_str() {
                Some("BOOL") => "BOOLEAN",
                Some("VARIANT") | None => continue,
                Some(other) => other,
            };
            let chosen = assigned.get(&own_id).or(assigned.get(&def_id)).copied();
            if !before(def, chosen) {
                return None;
            }
            let value = chosen.and_then(prop_value).or_else(|| prop_value(def));
            let mut prop = json!({ "type": kind, "value": value });
            if let Some(list) = def["preferredValues"]["instanceSwapValues"].as_array() {
                prop["preferredValues"] = list
                    .iter()
                    .map(|v| json!({ "type": v["type"], "key": v["key"] }))
                    .collect();
            }
            out.insert(
                format!("{}#{def_id}", def["name"].as_str().unwrap_or("")),
                prop,
            );
        }
        for spec in self.specs.as_array().into_iter().flatten() {
            if !before(&spec["propDefId"], None) {
                return None;
            }
            if let Some(def) = guid(&spec["propDefId"]).and_then(|id| set_defs.get(&id)) {
                if !before(def, Some(spec)) {
                    return None;
                }
                out.insert(
                    def["name"].as_str().unwrap_or("").to_owned(),
                    json!({ "value": spec["value"], "type": "VARIANT", "boundVariables": {} }),
                );
            }
        }
        Some(out)
    }
}

/// Visual, text and auto-layout fields of one node; `parent` is the parent's kiwi object.
fn fields(k: &Obj, kind: &str, parent: Option<&Obj>) -> Obj {
    let mut out = Obj::new();
    let mut put = |name: &str, v: Json| {
        out.insert(name.into(), v);
    };
    let num = |name: &str| k.get(name).and_then(Json::as_f64);
    let text = |name: &str| k.get(name).and_then(Json::as_str);

    if k.get("visible") == Some(&json!(false)) {
        put("visible", json!(false));
    }
    if let Some(o) = num("opacity").filter(|o| *o < 1.0) {
        put("opacity", json!(o));
    }
    put("fills", paints(k.get("fillPaints")));
    put("strokes", paints(k.get("strokePaints")));
    // Pages and sections are not composited, so REST gives them no blend mode or effects.
    if !matches!(kind, "SECTION" | "CANVAS") {
        put(
            "blendMode",
            k.get("blendMode").cloned().unwrap_or(json!("PASS_THROUGH")),
        );
        put(
            "effects",
            k.get("effects")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .map(effect)
                .collect(),
        );
    }
    let t = at(k, "transform");
    let rotation = t["m10"]
        .as_f64()
        .unwrap_or(0.0)
        .atan2(t["m00"].as_f64().unwrap_or(1.0));
    if rotation != 0.0 {
        put("rotation", json!(rotation));
    }
    if k.get("mask") == Some(&json!(true)) {
        put("isMask", json!(true));
        let outline = text("maskType") == Some("OUTLINE");
        put("maskType", json!(if outline { "VECTOR" } else { "ALPHA" }));
    }
    let bound = bound_variables(k);
    if !bound.is_empty() {
        put("boundVariables", Json::Object(bound));
    }
    for name in ["strokeWeight", "strokeAlign"] {
        if let Some(v) = k.get(name) {
            put(name, v.clone());
        }
    }
    if let Some(sides) = individual_stroke_weights(k) {
        put("individualStrokeWeights", sides);
    }
    // With independent corners the four radii win over cornerRadius; equal ones are reported as one.
    let mut radius = num("cornerRadius");
    if k.get("rectangleCornerRadiiIndependent") == Some(&json!(true)) {
        let radii: Vec<f64> = ["TopLeft", "TopRight", "BottomRight", "BottomLeft"]
            .iter()
            .map(|c| num(&format!("rectangle{c}CornerRadius")).unwrap_or(0.0))
            .collect();
        if radii.iter().any(|x| *x != radii[0]) {
            put("rectangleCornerRadii", json!(radii));
            radius = None;
        } else {
            radius = Some(radii[0]);
        }
    }
    let rounds = !matches!(kind, "ELLIPSE" | "BOOLEAN_OPERATION");
    if let Some(r) = radius.filter(|r| *r > 0.0 && rounds) {
        put("cornerRadius", json!(r));
    }
    if let Some(masked) = k.get("frameMaskDisabled").and_then(Json::as_bool) {
        put("clipsContent", json!(!masked && kind != "GROUP"));
    }
    if kind == "INSTANCE" {
        let main = k
            .get("overriddenSymbolID")
            .and_then(guid)
            .or_else(|| guid(&k.get("symbolData")?["symbolID"]));
        put("componentId", json!(main));
    }

    let mode = text("stackMode").filter(|m| *m != "NONE");
    let parent_mode = parent
        .and_then(|p| p.get("stackMode")?.as_str())
        .filter(|m| *m != "NONE");
    if let Some(mode) = mode {
        put("layoutMode", json!(mode));
        if mode != "GRID" {
            let wrap = text("stackWrap") == Some("WRAP");
            put("layoutWrap", json!(if wrap { "WRAP" } else { "NO_WRAP" }));
        }
    }
    // Defaults (AUTO sizing, MIN alignment) are left out, as the plugin does; grids have no flex fields.
    if mode.is_some_and(|m| m != "GRID") {
        if let Some(v) = num("stackSpacing").filter(|v| *v != 0.0) {
            put("itemSpacing", json!(v));
        }
        for (kiwi, rest) in [
            ("stackPrimaryAlignItems", "primaryAxisAlignItems"),
            ("stackCounterAlignItems", "counterAxisAlignItems"),
        ] {
            let align = match text(kiwi) {
                Some("SPACE_EVENLY") => "SPACE_BETWEEN",
                Some(other) if other != "MIN" => other,
                _ => continue,
            };
            put(rest, json!(align));
        }
    }
    if mode.is_some() {
        // The plugin leaves out zero padding.
        for (kiwi, rest) in [
            ("stackHorizontalPadding", "paddingLeft"),
            ("stackPaddingRight", "paddingRight"),
            ("stackVerticalPadding", "paddingTop"),
            ("stackPaddingBottom", "paddingBottom"),
        ] {
            if let Some(v) = num(kiwi).filter(|v| *v != 0.0) {
                put(rest, json!(v));
            }
        }
        if !hugs_primary(text("stackPrimarySizing")) {
            put("primaryAxisSizingMode", json!("FIXED"));
        }
        if !hugs_counter(text("stackCounterSizing")) {
            put("counterAxisSizingMode", json!("FIXED"));
        }
    }
    if mode.is_some() || parent_mode.is_some() {
        let sizing = |horizontal: bool| {
            // On the parent's primary axis a child fills by growing, on the counter axis by stretching.
            if let Some(pm) = parent_mode {
                let fills = if (pm == "HORIZONTAL") == horizontal {
                    num("stackChildPrimaryGrow").unwrap_or(0.0) > 0.0
                } else {
                    text("stackChildAlignSelf") == Some("STRETCH")
                };
                if fills {
                    return "FILL";
                }
            }
            let hug = match mode {
                // A grid's primary axis is horizontal.
                Some(m) if (m != "VERTICAL") == horizontal => {
                    hugs_primary(text("stackPrimarySizing"))
                }
                Some(_) => hugs_counter(text("stackCounterSizing")),
                None => match text("textAutoResize") {
                    Some("WIDTH_AND_HEIGHT") => true,
                    Some("HEIGHT") => !horizontal,
                    _ => false,
                },
            };
            if hug { "HUG" } else { "FIXED" }
        };
        put("layoutSizingHorizontal", json!(sizing(true)));
        put("layoutSizingVertical", json!(sizing(false)));
    }
    // The plugin writes only the non-default value.
    if parent_mode.is_some() && text("stackPositioning") == Some("ABSOLUTE") {
        put("layoutPositioning", json!("ABSOLUTE"));
    }

    if kind == "TEXT" {
        put("characters", at(k, "textData")["characters"].clone());
        put("style", text_style(k));
    }
    out
}

/// Kiwi leaves out the default sizing: hug on the primary axis, fixed on the counter axis.
fn hugs_primary(sizing: Option<&str>) -> bool {
    sizing != Some("FIXED")
}

fn hugs_counter(sizing: Option<&str>) -> bool {
    sizing.is_some_and(|s| s != "FIXED")
}

fn text_style(k: &Obj) -> Json {
    let size = at(k, "fontSize").as_f64().unwrap_or(0.0);
    let font = at(k, "fontName");
    let font_style = font["style"].as_str().unwrap_or("");
    let mut style = json!({
        "fontFamily": font["family"],
        "fontPostScriptName": font["postscript"],
        "fontStyle": font_style,
        "fontWeight": font_weight(font_style),
        "fontSize": size,
        "textAlignHorizontal": k.get("textAlignHorizontal").unwrap_or(&json!("LEFT")),
        "textAlignVertical": k.get("textAlignVertical").unwrap_or(&json!("TOP")),
        "letterSpacing": 0.0,
    });
    if let Some(r) = k.get("textAutoResize").filter(|r| *r != "NONE") {
        style["textAutoResize"] = r.clone();
    }
    if let Some(d) = k.get("textDecoration").filter(|d| *d != "NONE") {
        style["textDecoration"] = d.clone();
    }
    if at(k, "textTruncation") == "ENDING" {
        style["textTruncation"] = json!("ENDING");
        if let Some(m) = k.get("maxLines") {
            style["maxLines"] = m.clone();
        }
    }
    let px = |v: &Json, percent_of_size: bool| {
        let value = v["value"].as_f64().unwrap_or(0.0);
        match v["units"].as_str() {
            Some("PIXELS") => Some(value),
            Some("PERCENT") if percent_of_size => Some(value * size / 100.0),
            Some("RAW") => Some(value * size),
            _ => None,
        }
    };
    if let Some(ls) = px(at(k, "letterSpacing"), true) {
        style["letterSpacing"] = json!(ls);
    }
    // Percent line heights are of the font's own line height, which the layout stores.
    let font_line = at(k, "derivedTextData")["fontMetaData"][0]["fontLineHeight"]
        .as_f64()
        .map(|f| f * size);
    let lh = at(k, "lineHeight");
    let px = match lh["units"].as_str() {
        Some("PERCENT") => font_line.map(|f| f * lh["value"].as_f64().unwrap_or(100.0) / 100.0),
        _ => px(lh, false),
    };
    if let Some(px) = px {
        style["lineHeightPx"] = json!(px);
        // Figma works these out in f32; matching its rounding keeps the values identical.
        let percent = |ratio: f64| f64::from(ratio as f32 * 100.0);
        if lh["units"] == "PERCENT" {
            style["lineHeightPercent"] = lh["value"].clone();
            style["lineHeightUnit"] = json!("INTRINSIC_%");
        } else {
            if let Some(f) = font_line {
                style["lineHeightPercent"] = json!(percent(px / f));
            }
            if size > 0.0 {
                style["lineHeightPercentFontSize"] = json!(percent(px / size));
            }
            style["lineHeightUnit"] = json!("PIXELS");
        }
    }
    let flags: Obj = [
        ("fontVariantNumericFigure", "LINING", "LNUM"),
        ("fontVariantNumericFigure", "OLDSTYLE", "ONUM"),
        ("fontVariantNumericSpacing", "TABULAR", "TNUM"),
        ("fontVariantNumericSpacing", "PROPORTIONAL", "PNUM"),
    ]
    .into_iter()
    .filter(|(field, value, _)| at(k, field) == *value)
    .map(|(_, _, flag)| (flag.to_owned(), json!(1.0)))
    .collect();
    if !flags.is_empty() {
        style["opentypeFlags"] = Json::Object(flags);
    }
    style
}

/// REST fields that a kiwi text field sets, as the plugin lists them in `styleOverrideTable`.
fn override_fields(kiwi: &str) -> &'static [&'static str] {
    const LINE: [&str; 5] = [
        "fontSize",
        "lineHeightPx",
        "lineHeightPercent",
        "lineHeightPercentFontSize",
        "lineHeightUnit",
    ];
    match kiwi {
        "fontName" => &[
            "fontFamily",
            "fontPostScriptName",
            "fontStyle",
            "fontWeight",
        ],
        "fontSize" => &LINE,
        "lineHeight" => &LINE[1..],
        "letterSpacing" => &["letterSpacing"],
        "textDecoration" => &["textDecoration"],
        "textAutoResize" => &["textAutoResize"],
        "textTruncation" => &["textTruncation"],
        "maxLines" => &["maxLines"],
        _ => &[],
    }
}

/// CSS-style weight from a font style name such as "SemiBold Italic".
fn font_weight(style: &str) -> u32 {
    let s = style.to_lowercase().replace([' ', '-'], "");
    [
        ("thin", 100),
        ("extralight", 200),
        ("ultralight", 200),
        ("light", 300),
        ("medium", 500),
        ("semibold", 600),
        ("demibold", 600),
        ("extrabold", 800),
        ("ultrabold", 800),
        ("bold", 700),
        ("black", 900),
        ("heavy", 900),
    ]
    .into_iter()
    .find(|(name, _)| s.contains(name))
    .map_or(400, |(_, w)| w)
}

pub(crate) fn paints(list: Option<&Json>) -> Json {
    list.and_then(Json::as_array)
        .into_iter()
        .flatten()
        .map(paint)
        .collect()
}

/// REST id of a variable reference: `VariableID:<guid>` when local, `VariableID:<key>/<version>` from a library.
pub(crate) fn variable_id(r: &Json) -> Option<String> {
    let lib = &r["assetRef"];
    match (lib["key"].as_str(), lib["version"].as_str()) {
        (Some(key), Some(version)) => Some(format!("VariableID:{key}/{version}")),
        _ => guid(&r["guid"]).map(|g| format!("VariableID:{g}")),
    }
}

/// The plugin's order of bound variables: library ones by key, then local ones by numeric guid.
fn variable_order(id: &str) -> (bool, String, u64, u64) {
    let id = id.trim_start_matches("VariableID:");
    if let Some((key, _)) = id.split_once('/') {
        return (false, key.to_owned(), 0, 0);
    }
    let (s, l) = id.split_once(':').unwrap_or(("", ""));
    (
        true,
        String::new(),
        s.parse().unwrap_or(0),
        l.parse().unwrap_or(0),
    )
}

/// `{type: VARIABLE_ALIAS, id}` for a kiwi variable binding (`colorVar`, `variableData`).
pub(crate) fn alias(binding: &Json) -> Option<Json> {
    if binding["dataType"] != "ALIAS" {
        return None;
    }
    let id = variable_id(&binding["value"]["alias"])?;
    Some(json!({ "type": "VARIABLE_ALIAS", "id": id }))
}

/// Aliases bound anywhere in a paint list (solid colors and gradient stops), each once.
fn paint_aliases(list: Option<&Json>) -> Vec<Json> {
    let mut out: Vec<Json> = Vec::new();
    // A set keeps thousands of distinct aliases linear; `out` keeps first-seen order.
    let mut seen = std::collections::HashSet::new();
    for p in list.and_then(Json::as_array).into_iter().flatten() {
        let stops = p["stopsVar"].as_array().into_iter().flatten();
        for a in std::iter::once(&p["colorVar"])
            .chain(stops.map(|s| &s["colorVar"]))
            .filter_map(alias)
        {
            if seen.insert(a.to_string()) {
                out.push(a);
            }
        }
    }
    out
}

/// The value of a property assignment or definition: its typed `varValue`, else `value` / `initialValue`.
fn prop_value(a: &Json) -> Option<Json> {
    prop_value_source(a).map(|(value, _)| value)
}

fn prop_value_source(a: &Json) -> Option<(Json, &'static str)> {
    let typed = &a["varValue"]["value"];
    let plain = if a["value"].is_object() {
        &a["value"]
    } else {
        &a["initialValue"]
    };
    for (v, source) in [
        (typed, "varValue.value"),
        (
            plain,
            if a["value"].is_object() {
                "value"
            } else {
                "initialValue"
            },
        ),
    ] {
        if let Some(b) = v["boolValue"].as_bool() {
            return Some((json!(b), source));
        }
        if let Some(s) = v["textDataValue"]["characters"].as_str() {
            return Some((json!(s), source));
        }
        if let Some(s) = v["textValue"]["characters"]
            .as_str()
            .or(v["textValue"].as_str())
        {
            return Some((json!(s), source));
        }
        if let Some(id) = guid(&v["symbolIdValue"]["guid"]).or_else(|| guid(&v["guidValue"])) {
            return Some((json!(id), source));
        }
    }
    None
}

/// REST `boundVariables` of a node: paint, effect and field (`variableConsumptionMap`) bindings.
pub(crate) fn bound_variables(k: &Obj) -> Obj {
    let mut out = Obj::new();
    let text_fills: Json = at(k, "textData")["styleOverrideTable"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["fillPaints"].as_array())
        .flatten()
        .cloned()
        .collect();
    let lists = [
        ("fills", paint_aliases(k.get("fillPaints"))),
        ("strokes", paint_aliases(k.get("strokePaints"))),
        ("effects", paint_aliases(k.get("effects"))),
        ("textRangeFills", paint_aliases(Some(&text_fills))),
    ];
    for (name, list) in lists {
        if !list.is_empty() {
            out.insert(name.into(), Json::Array(list));
        }
    }
    // Newer files keep field bindings in parameterConsumptionMap; it supersedes the older map.
    let map = ["parameterConsumptionMap", "variableConsumptionMap"]
        .into_iter()
        .find_map(|m| k.get(m))
        .unwrap_or(&Json::Null);
    let entries = map["entries"].as_array().into_iter().flatten();
    for e in entries {
        let data = &e["variableData"];
        // A font-style binding wraps its weight variable.
        let a = alias(data).or_else(|| alias(&data["value"]["fontStyleValue"]["asFloat"]));
        let (Some(field), Some(a)) = (e["variableField"].as_str(), a) else {
            continue;
        };
        let (name, value) = match field {
            "STACK_SPACING" => ("itemSpacing", a),
            "STACK_PADDING_TOP" => ("paddingTop", a),
            "STACK_PADDING_BOTTOM" => ("paddingBottom", a),
            "STACK_PADDING_LEFT" => ("paddingLeft", a),
            "STACK_PADDING_RIGHT" => ("paddingRight", a),
            "OPACITY" => ("opacity", a),
            // Text fields can vary by character range, so REST lists them.
            "FONT_SIZE" => ("fontSize", json!([a])),
            "FONT_STYLE" => ("fontWeight", json!([a])),
            corner if corner.starts_with("RECTANGLE_") => {
                out.entry("rectangleCornerRadii")
                    .or_insert_with(|| json!({}))[corner] = a;
                continue;
            }
            _ => continue,
        };
        out.insert(name.into(), value);
    }
    out
}

fn effect(e: &Json) -> Json {
    let kind = match e["type"].as_str().unwrap_or("") {
        "FOREGROUND_BLUR" => "LAYER_BLUR",
        other => other,
    };
    let mut out =
        json!({ "type": kind, "visible": e["visible"] != json!(false), "radius": e["radius"] });
    if kind.ends_with("_BLUR") {
        return out;
    }
    for f in ["color", "blendMode", "offset"] {
        out[f] = e[f].clone();
    }
    if e["spread"].as_f64().is_some_and(|s| s != 0.0) {
        out["spread"] = e["spread"].clone();
    }
    if kind == "DROP_SHADOW" {
        out["showShadowBehindNode"] = json!(e["showShadowBehindNode"] == json!(true));
    }
    if let Some(a) = alias(&e["colorVar"]) {
        out["boundVariables"] = json!({ "color": a });
    }
    out
}

fn paint(p: &Json) -> Json {
    let mut out = json!({ "type": p["type"], "blendMode": p["blendMode"] });
    if p["visible"] == json!(false) {
        out["visible"] = json!(false);
    }
    if let Some(o) = p["opacity"].as_f64().filter(|o| *o < 1.0) {
        out["opacity"] = json!(o);
    }
    if let Some(a) = alias(&p["colorVar"]) {
        out["boundVariables"] = json!({ "color": a });
    }
    match p["type"].as_str().unwrap_or("") {
        "SOLID" => out["color"] = p["color"].clone(),
        "IMAGE" => {
            out["scaleMode"] = p["imageScaleMode"].clone();
            out["imageRef"] = p["image"]["hash"].clone();
            let filters: Obj = p["paintFilter"]
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, v)| v.as_f64().is_some_and(|x| x != 0.0))
                .map(|(k, v)| match k.as_str() {
                    "vibrance" => ("saturation".to_owned(), v.clone()),
                    _ => (k.clone(), v.clone()),
                })
                .collect();
            if !filters.is_empty() {
                out["filters"] = Json::Object(filters);
            }
            // Only crop (STRETCH) images carry their transform in REST.
            if p["imageScaleMode"] == json!("STRETCH") {
                let t = &p["transform"];
                out["imageTransform"] = json!([
                    [t["m00"], t["m01"], t["m02"]],
                    [t["m10"], t["m11"], t["m12"]]
                ]);
            }
        }
        kind if kind.starts_with("GRADIENT_") => {
            out["gradientStops"] = p["stops"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(i, s)| {
                    let mut stop = json!({ "color": s["color"], "position": s["position"] });
                    if let Some(a) = alias(&p["stopsVar"][i]["colorVar"]) {
                        stop["boundVariables"] = json!({ "color": a });
                    }
                    stop
                })
                .collect();
            out["gradientHandlePositions"] = handles(kind, &p["transform"]);
        }
        _ => {}
    }
    out
}

/// REST handles are the gradient's unit-space anchors mapped back into the node's unit box.
fn handles(kind: &str, transform: &Json) -> Json {
    let f = |name: &str| transform[name].as_f64().unwrap_or(0.0);
    let inv = Mat {
        a: f("m00"),
        b: f("m10"),
        c: f("m01"),
        d: f("m11"),
        tx: f("m02"),
        ty: f("m12"),
    }
    .inverse();
    let anchors = if kind == "GRADIENT_LINEAR" {
        [(0.0, 0.5), (1.0, 0.5), (0.0, 1.0)]
    } else {
        [(0.5, 0.5), (1.0, 0.5), (0.5, 1.0)]
    };
    anchors
        .into_iter()
        .map(|(x, y)| {
            let p = inv.apply(Vec2 { x, y });
            json!({ "x": p.x, "y": p.y })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(v: Json) -> Obj {
        object(v)
    }

    #[test]
    fn diagnostics_follow_effective_node_visibility() {
        use kiwi_schema::Value as K;
        let object = |fields: Vec<(&'static str, K<'static>)>| {
            K::Object("Test", fields.into_iter().collect())
        };
        let size = || object(vec![("x", K::Float(20.0)), ("y", K::Float(20.0))]);
        let paint = || {
            K::Array(vec![object(vec![
                ("type", K::Enum("PaintType", "IMAGE")),
                ("imageScaleMode", K::Enum("ScaleMode", "TILE")),
            ])])
        };
        for (visible, opacity, kind, expected) in [
            (true, 1.0, "FRAME", true),
            (false, 1.0, "FRAME", false),
            (true, 0.0, "FRAME", false),
            (true, 1.0, "BOOLEAN_OPERATION", false),
        ] {
            let parent = object(vec![
                ("type", K::Enum("NodeType", kind)),
                ("visible", K::Bool(visible)),
                ("opacity", K::Float(opacity)),
                ("size", size()),
            ]);
            let child = object(vec![
                ("type", K::Enum("NodeType", "RECTANGLE")),
                ("size", size()),
                ("fillPaints", paint()),
            ]);
            let tree = Tree {
                nodes: HashMap::from([("1:1".into(), &parent), ("1:2".into(), &child)]),
                children: HashMap::from([("1:1".into(), vec!["1:2".into()])]),
                blobs: Vec::new(),
            };
            let doc = nodes(&tree, "1:1");
            let diagnostics = &doc["document"]["children"][0]["_figlens"]["renderDiagnostics"];
            assert_eq!(
                diagnostics.is_array(),
                expected,
                "{visible} {opacity} {kind}: {doc}"
            );
            if expected {
                assert_eq!(diagnostics[0]["nodeId"], "1:2");
            }
        }
    }

    #[test]
    fn library_style_reference_resolves_to_its_key() {
        use kiwi_schema::Value as K;
        let object = |fields: Vec<(&'static str, K<'static>)>| {
            K::Object("Test", fields.into_iter().collect())
        };
        let style = object(vec![
            ("name", K::String("Brand/Primary".into())),
            ("styleType", K::Enum("StyleType", "FILL")),
            ("key", K::String("abc123".into())),
        ]);
        let frame = object(vec![
            ("type", K::Enum("NodeType", "FRAME")),
            (
                "styleIdForFill",
                object(vec![(
                    "assetRef",
                    object(vec![("key", K::String("abc123".into()))]),
                )]),
            ),
        ]);
        let tree = Tree {
            nodes: HashMap::from([("1:1".into(), &frame), ("2:1".into(), &style)]),
            children: HashMap::new(),
            blobs: Vec::new(),
        };
        assert_eq!(
            nodes(&tree, "1:1")["styles"]["2:1"],
            json!({"name": "Brand/Primary", "styleType": "FILL", "remote": true, "description": "", "key": "abc123"})
        );
    }

    #[test]
    fn auto_layout_fields_use_rest_names_and_skip_defaults() {
        let k = obj(json!({
            "stackMode": "HORIZONTAL",
            "stackSpacing": 2.0,
            "stackHorizontalPadding": 12.0,
            "stackPaddingRight": 4.0,
            "stackVerticalPadding": 0.0,
            "stackPrimarySizing": "FIXED",
            "stackCounterSizing": "RESIZE_TO_FIT_WITH_IMPLICIT_SIZE",
            "stackPrimaryAlignItems": "CENTER",
        }));
        let out = fields(&k, "FRAME", None);
        let got: Vec<_> = [
            "itemSpacing",
            "paddingLeft",
            "paddingRight",
            "paddingTop",
            "primaryAxisSizingMode",
            "counterAxisSizingMode",
            "primaryAxisAlignItems",
            "counterAxisAlignItems",
            "layoutSizingVertical",
        ]
        .iter()
        .map(|f| out.get(*f).cloned())
        .collect();
        assert_eq!(
            got,
            [
                Some(json!(2.0)),
                Some(json!(12.0)),
                Some(json!(4.0)),
                None,
                Some(json!("FIXED")),
                None,
                Some(json!("CENTER")),
                None,
                Some(json!("HUG")),
            ]
        );
    }

    #[test]
    fn per_side_stroke_weights_use_the_rest_shape_and_leave_out_sides_as_zero() {
        let k = obj(json!({
            "strokeWeight": 0.0,
            "borderStrokeWeightsIndependent": true,
            "borderBottomWeight": 3.0,
        }));
        assert_eq!(
            fields(&k, "FRAME", None)["individualStrokeWeights"],
            json!({ "top": 0.0, "right": 0.0, "bottom": 3.0, "left": 0.0 })
        );
        let uniform = obj(json!({ "strokeWeight": 1.0, "borderBottomWeight": 3.0 }));
        assert!(!fields(&uniform, "FRAME", None).contains_key("individualStrokeWeights"));
    }

    #[test]
    fn child_grows_on_parent_primary_axis_and_stretches_on_counter_axis() {
        let parent = obj(json!({ "stackMode": "VERTICAL" }));
        let k = obj(json!({ "stackChildPrimaryGrow": 1.0, "stackChildAlignSelf": "STRETCH" }));
        let out = fields(&k, "FRAME", Some(&parent));
        assert_eq!(
            (&out["layoutSizingHorizontal"], &out["layoutSizingVertical"]),
            (&json!("FILL"), &json!("FILL"))
        );
    }

    #[test]
    fn absolute_child_of_auto_layout_is_marked() {
        let parent = obj(json!({ "stackMode": "VERTICAL" }));
        let k = obj(json!({ "stackPositioning": "ABSOLUTE" }));
        let out = fields(&k, "FRAME", Some(&parent));
        assert_eq!(out["layoutPositioning"], json!("ABSOLUTE"));
    }

    #[test]
    fn letter_spacing_percent_is_relative_to_font_size() {
        let k = obj(json!({
            "fontSize": 20.0,
            "fontName": { "family": "Inter", "postscript": "Inter-SemiBold", "style": "Semi Bold" },
            "letterSpacing": { "units": "PERCENT", "value": 10.0 },
            "lineHeight": { "units": "PIXELS", "value": 28.0 },
        }));
        let style = text_style(&k);
        assert_eq!(
            (
                &style["letterSpacing"],
                &style["lineHeightPx"],
                &style["fontWeight"]
            ),
            (&json!(2.0), &json!(28.0), &json!(600))
        );
    }

    fn noto(line_height: Json) -> Obj {
        obj(json!({
            "fontSize": 14.0,
            "fontName": { "family": "Noto Sans SC", "postscript": "NotoSansSC-Medium", "style": "Medium" },
            "lineHeight": line_height,
            "derivedTextData": { "fontMetaData": [{ "fontLineHeight": 1.4479999542236328 }] },
        }))
    }

    #[test]
    fn auto_line_height_is_the_fonts_own() {
        let style = text_style(&noto(json!({ "units": "PERCENT", "value": 100.0 })));
        assert_eq!(
            (
                style["lineHeightPx"].as_f64().map(|v| (v * 1e3).round()),
                &style["lineHeightUnit"],
                style.get("lineHeightPercentFontSize")
            ),
            (Some(20272.0), &json!("INTRINSIC_%"), None)
        );
    }

    #[test]
    fn pixel_line_height_percents_round_like_figma() {
        let style = text_style(&noto(json!({ "units": "PIXELS", "value": 20.0 })));
        assert_eq!(
            (
                &style["lineHeightPercent"],
                &style["lineHeightPercentFontSize"]
            ),
            (&json!(98.65824890136719), &json!(142.85714721679688))
        );
    }

    #[test]
    fn a_font_size_override_also_lists_the_line_height_it_moves() {
        assert_eq!(
            override_fields("fontSize"),
            [
                "fontSize",
                "lineHeightPx",
                "lineHeightPercent",
                "lineHeightPercentFontSize",
                "lineHeightUnit"
            ]
        );
    }

    #[test]
    fn bound_paint_takes_the_inherited_mode_through_an_alias() {
        let mode = |s: u64| json!({ "sessionID": s, "localID": 0 });
        let set = |key: &str, modes: &[u64]| {
            let modes: Vec<_> = modes.iter().map(|m| json!({ "id": mode(*m) })).collect();
            json!({ "key": key, "variableSetModes": modes })
        };
        let value = |m: u64, value: Json| json!({ "modeID": mode(m), "variableData": value });
        let color = |r: f64, a: f64| json!({ "dataType": "COLOR", "value": { "colorValue": { "r": r, "g": 0.0, "b": 0.0, "a": a } } });
        let alias = |key: &str| json!({ "dataType": "ALIAS", "value": { "alias": { "assetRef": { "key": key } } } });
        // A themed library variable whose dark mode points at a single-mode primitive.
        let vars = Vars::from([
            ("theme".into(), vec![set("theme", &[1, 2])]),
            ("base".into(), vec![set("base", &[9])]),
            (
                "accent".into(),
                vec![json!({
                    "variableSetID": { "assetRef": { "key": "theme" } },
                    "variableDataValues": { "entries": [value(1, color(0.1, 1.0)), value(2, alias("red"))] },
                })],
            ),
            (
                "red".into(),
                vec![json!({
                    "variableSetID": { "assetRef": { "key": "base" } },
                    "variableDataValues": { "entries": [value(9, color(0.9, 0.4))] },
                })],
            ),
        ]);
        let frame = json!({ "entries": [
            { "variableSetID": { "assetRef": { "key": "theme" } }, "variableModeID": mode(2) },
        ] });
        let modes = with_modes(&vars, &frame, &Modes::new());
        let mut fills = json!([{
            "type": "SOLID",
            "color": { "r": 0.1, "g": 0.0, "b": 0.0, "a": 1.0 },
            "colorVar": alias("accent"),
        }]);
        bind_paints(&vars, Some(&mut fills), &modes);
        assert_eq!(
            (&fills[0]["color"], &fills[0]["opacity"]),
            (
                &json!({ "r": 0.9, "g": 0.0, "b": 0.0, "a": 1.0 }),
                &json!(0.4)
            )
        );
    }

    #[test]
    fn foreground_blur_is_a_layer_blur_and_zero_spread_is_left_out() {
        let blur = effect(&json!({ "type": "FOREGROUND_BLUR", "radius": 4.0, "visible": true }));
        let shadow = effect(
            &json!({ "type": "INNER_SHADOW", "radius": 2.0, "spread": 0.0, "color": {}, "offset": {} }),
        );
        assert_eq!(
            (
                &blur["type"],
                shadow.get("spread"),
                shadow.get("showShadowBehindNode")
            ),
            (&json!("LAYER_BLUR"), None, None)
        );
    }

    #[test]
    fn newer_consumption_map_wins_and_font_style_binds_weight() {
        let lib = |key: &str| json!({ "dataType": "ALIAS", "value": { "alias": { "assetRef": { "key": key, "version": "1:2" } } } });
        let k = obj(json!({
            "variableConsumptionMap": { "entries": [{ "variableField": "STACK_PADDING_TOP", "variableData": lib("old") }] },
            "parameterConsumptionMap": { "entries": [
                { "variableField": "STACK_SPACING", "variableData": lib("gap") },
                { "variableField": "FONT_STYLE", "variableData": { "dataType": "FONT_STYLE", "value": { "fontStyleValue": { "asFloat": lib("weight") } } } },
            ] },
        }));
        let b = bound_variables(&k);
        assert_eq!(
            (
                b.get("paddingTop"),
                &b["itemSpacing"]["id"],
                &b["fontWeight"][0]["id"]
            ),
            (
                None,
                &json!("VariableID:gap/1:2"),
                &json!("VariableID:weight/1:2")
            )
        );
    }

    #[test]
    fn paint_aliases_list_each_color_and_stop_alias_once_in_first_seen_order() {
        let var = |l: u32| json!({ "dataType": "ALIAS", "value": { "alias": { "guid": { "sessionID": 1, "localID": l } } } });
        let paints = json!([
            { "colorVar": var(3), "stopsVar": [{ "colorVar": var(1) }, { "colorVar": var(3) }] },
            { "colorVar": var(1) },
            { "stopsVar": [{ "colorVar": var(2) }] },
        ]);
        let ids: Vec<Json> = paint_aliases(Some(&paints))
            .into_iter()
            .map(|a| a["id"].clone())
            .collect();
        assert_eq!(
            ids,
            [
                json!("VariableID:1:3"),
                json!("VariableID:1:1"),
                json!("VariableID:1:2")
            ]
        );
    }

    #[test]
    fn bound_variables_list_library_ones_by_key_then_local_ones_by_guid() {
        let mut ids = [
            "VariableID:6:14",
            "VariableID:6:4",
            "VariableID:b/9:9",
            "VariableID:a/1:1",
        ];
        ids.sort_by_key(|id| variable_order(id));
        assert_eq!(
            ids,
            [
                "VariableID:a/1:1",
                "VariableID:b/9:9",
                "VariableID:6:4",
                "VariableID:6:14"
            ]
        );
    }

    #[test]
    fn vertical_linear_gradient_handles_run_top_to_bottom() {
        // Kiwi stores the node-to-gradient transform; this one is a quarter turn.
        let t = json!({ "m00": 0.0, "m01": 1.0, "m02": 0.0, "m10": -1.0, "m11": 0.0, "m12": 1.0 });
        let h = handles("GRADIENT_LINEAR", &t);
        let points: Vec<(f64, f64)> = h
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap()))
            .collect();
        assert_eq!(points, [(0.5, 0.0), (0.5, 1.0), (0.0, 0.0)]);
    }
}
