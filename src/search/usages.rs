//! Exact relations of one index and the `usages` query over them (issue #91). Edges are
//! extracted while the search index is built, from the same expansion: every edge names its
//! location record, relation, target, resolution state, basis and evidence. Similarity, names
//! and coordinates never make an edge.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;

use serde_json::{Map, Value as Json, json};

use super::{Index, Record, kiwi_name, short_hash};
use crate::expand::{Bounds, Expanded, json_bytes, kiwi_bytes, parent_of, plugin_type, sum_while};
use crate::{ErrorKind, FigError, Tree, guid_key, to_json};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Extraction version: a stored generation built with another one is rebuilt. It also versions
/// the stored fingerprints (`fingerprint`).
pub(crate) const RELATIONS: &str = "relations-7";
/// Relations extracted, in key order; `LIBRARY` is stored apart and not listed here.
pub(crate) const KINDS: [&str; 7] = [
    "binds_variable",
    "contains",
    "instance_of",
    "prototype_target",
    "uses_image",
    "uses_style",
    "variant_of",
];
/// A library reference: the namespace (`sourceLibraryKey`) and key a cached library copy records
/// for the component, style or variable a record uses. Read by the workspace library query
/// (`workspace::library`) and, per use, by outgoing usages (`library_of`); it is not in `KINDS`
/// and no query, default or coverage lists it as a relation.
pub(crate) const LIBRARY: &str = "uses_library";
/// Every relation a stored generation holds: `KINDS` and `LIBRARY`.
pub(crate) const STORED: [&str; 8] = [
    "binds_variable",
    "contains",
    "instance_of",
    "prototype_target",
    "uses_image",
    LIBRARY,
    "uses_style",
    "variant_of",
];
/// `resolved`: the target exists in this file with the expected type. `missing`: a recorded id
/// absent from the file. `unresolved`: no usable target id. `unsupported`: a target of the wrong
/// type or a field figlens cannot read. `external`: a library identity this file only names.
/// `recorded`: an exact identity figlens does not check further (an image hash).
pub(crate) const STATES: [&str; 6] = [
    "resolved",
    "missing",
    "unresolved",
    "unsupported",
    "external",
    "recorded",
];
/// `cached_copy`: read from the file's cached copy of a library definition, not proven against
/// the library itself.
pub(crate) const BASES: [&str; 3] = ["cached_copy", "recorded", "resolved"];
/// The kinds a text-only build (ordinary stateless search) still extracts.
/// Kinds read from binding fields, which a field too large to read leaves partial.
pub(crate) const FIELDED: [&str; 3] = ["binds_variable", "uses_style", "uses_image"];
pub(crate) const TEXT_ONLY: [&str; 3] = ["contains", "instance_of", "variant_of"];
pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 200;
/// Most component-set members one query lists and follows.
pub(crate) const MAX_MEMBERS: usize = 200;
const MAX_ID_BYTES: usize = 1024;
/// Visits one field size estimate may take.
const VISITS: usize = 1 << 16;
const FIELDS: [&str; 5] = [
    "fillPaints",
    "strokePaints",
    "effects",
    "parameterConsumptionMap",
    "variableConsumptionMap",
];
/// Fields whose `styleOverrideTable` holds per-range (text) or per-region (vector) paints.
const STYLED: [&str; 2] = ["textData", "vectorData"];
const STYLES: [&str; 5] = [
    "styleIdForText",
    "styleIdForFill",
    "styleIdForStrokeFill",
    "styleIdForEffect",
    "styleIdForGrid",
];

/// One relation from a location record to its target.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Edge {
    pub relation: &'static str,
    /// Orders several edges of one relation on one record.
    pub seq: u32,
    pub target: Option<String>,
    pub state: &'static str,
    pub basis: &'static str,
    /// The field or binding that established it.
    pub evidence: String,
}

/// What an index knows about an edge target that may lie outside its scope.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Definition {
    pub name: String,
    pub kind: String,
    /// The COMPONENT_SET a COMPONENT belongs to.
    pub set: Option<String>,
    pub page: Option<String>,
}

// ---- Extraction ----

/// The namespace each library style and variable key has in this file, read from the cached
/// copies that carry the key. `None` when a copy names no namespace or two copies disagree: a
/// reference to that key keeps no namespace rather than borrowing one.
pub(crate) type Libraries = HashMap<(&'static str, String), Option<String>>;

/// `Libraries` for `tree`, every node visit and kept key charged first; `None` when `bounds`
/// ran out.
pub(crate) fn libraries(tree: &Tree, bounds: &mut Bounds) -> Option<Libraries> {
    if !bounds.charge(tree.nodes.len(), 0) {
        return None;
    }
    let mut out = Libraries::new();
    for node in tree.nodes.values() {
        let kind = match node.get("styleType") {
            Some(_) => "style",
            None if plugin_type(node) == "VARIABLE" => "variable",
            None => continue,
        };
        let Some(key) = text(node, "key") else {
            continue;
        };
        let ns = text(node, "sourceLibraryKey");
        if !bounds.charge(1, 64 + key.len() + ns.map_or(0, str::len)) {
            return None;
        }
        out.entry((kind, key.to_owned()))
            .and_modify(|v| {
                if v.as_deref() != ns {
                    *v = None;
                }
            })
            .or_insert(ns.map(str::to_owned));
    }
    Some(out)
}

fn text<'a>(node: &'a kiwi_schema::Value<'_>, field: &str) -> Option<&'a str> {
    Some(node.get(field)?.as_string()).filter(|s| !s.is_empty())
}

/// The edges of one record, `contains` excluded (each record's `parent` is that edge). `None`
/// when `bounds` ran out first. Prototype links the record inherits from its component
/// definition are not verified to apply, so they are counted in `gaps.0` (unverified), not
/// kept; a binding field too large to read is counted in `gaps.1` (unread).
pub(crate) fn extract(
    tree: &Tree,
    id: &str,
    kind: &str,
    e: Option<&Expanded>,
    bounds: &mut Bounds,
    gaps: Option<(&mut usize, &mut usize)>,
    libraries: Option<&Libraries>,
) -> Option<Vec<Edge>> {
    let mut out: Vec<Edge> = Vec::new();
    // Library references are pushed where each use is read, between other relations' edges.
    let mut library_seq = 0;
    let mut push = |bounds: &mut Bounds,
                    relation: &'static str,
                    target: Option<(&str, &str)>,
                    state: &'static str,
                    basis: &'static str,
                    evidence: String| {
        // A target longer than any stored id cannot be kept exactly; it stays explicit. Its
        // prefix and borrowed body are measured first, so only a kept, charged one is copied.
        let (target, state) = match target {
            Some((p, b)) if p.len() + b.len() > super::store::ID_BYTES => (None, "unsupported"),
            t => (t, state),
        };
        let bytes = 96 + target.map_or(0, |(p, b)| p.len() + b.len()) + evidence.len();
        if !bounds.charge(1, bytes) {
            return false;
        }
        // Each other relation's edges are pushed together, so its seq follows the last edge's.
        let seq = match relation {
            LIBRARY => {
                library_seq += 1;
                library_seq - 1
            }
            _ => out
                .iter()
                .rfind(|x| x.relation != LIBRARY)
                .filter(|x| x.relation == relation)
                .map_or(0, |x| x.seq + 1),
        };
        out.push(Edge {
            relation,
            seq,
            target: target.map(|(p, b)| format!("{p}{b}")),
            state,
            basis,
            evidence,
        });
        true
    };
    let mut main = None;
    if kind == "INSTANCE" {
        let recorded: Option<String>;
        let (target, basis, evidence) = match e {
            None => {
                recorded = tree
                    .nodes
                    .get(id)
                    .and_then(|n| n.get("symbolData")?.get("symbolID").and_then(guid_key));
                (recorded.as_deref(), "recorded", "symbolData")
            }
            Some(e) => (
                e.main.as_ref().map(|m| m.0.as_str()),
                "resolved",
                e.main.as_ref().map_or("definition", |m| m.1),
            ),
        };
        let state = node_state(tree, target, "COMPONENT");
        main = target.map(str::to_owned);
        let target = target.map(|t| ("", t));
        if !push(bounds, "instance_of", target, state, basis, evidence.into()) {
            return None;
        }
    }
    if kind == "COMPONENT"
        && e.is_none()
        && let Some(set) = tree.nodes.get(id).and_then(|n| parent_of(n))
        && tree
            .nodes
            .get(&set)
            .is_some_and(|n| plugin_type(n) == "COMPONENT_SET")
        && !push(
            bounds,
            "variant_of",
            Some(("", &set)),
            "resolved",
            "recorded",
            "parentIndex".into(),
        )
    {
        return None;
    }
    // A text-only build (`gaps` is `None`) keeps the structural relations; field-borne ones
    // need a full build.
    let Some((unverified, unread)) = gaps else {
        return Some(out);
    };
    // A library reference is (kind, namespace, key) as a JSON array, which keeps them apart
    // whatever characters they hold; with no namespace it stays unresolved. The namespace comes
    // from a cached copy: a main's own, or for a style or variable the copies with its key.
    let reference = |kind: &str, ns: Option<&str>, key: &str| {
        // Measured on the borrowed strings first, so an overlong reference is never copied.
        if kind.len() + ns.map_or(0, str::len) + key.len() > super::store::ID_BYTES {
            return (None, "unsupported");
        }
        let state = if ns.is_some() {
            "external"
        } else {
            "unresolved"
        };
        (Some(json!([kind, ns, key]).to_string()), state)
    };
    let version = |field: &str, v: Option<&str>| {
        // An implausibly long version is left out rather than stored.
        json!({"field": field, "version": v.filter(|v| v.len() <= 128)}).to_string()
    };
    let namespace = |kind: &'static str, key: &str| {
        // An overlong key keeps no reference, so it is not copied for the lookup either.
        let libraries = libraries.filter(|_| key.len() <= super::store::ID_BYTES);
        libraries.and_then(|l| l.get(&(kind, key.to_owned())).cloned().flatten())
    };
    if libraries.is_some()
        && let Some((node, copy)) = main.and_then(|m| Some((tree.nodes.get(&m).copied()?, m)))
        && let Some(key) = text(node, "componentKey")
    {
        let (target, state) = reference("component", text(node, "sourceLibraryKey"), key);
        let evidence = json!({"copy": copy}).to_string();
        if !push(
            bounds,
            LIBRARY,
            target.as_deref().map(|t| ("", t)),
            state,
            "cached_copy",
            evidence,
        ) {
            return None;
        }
    }
    let mut fields = effective(tree, id, e, bounds, unread, false)?;
    // A text range row applies where a character uses its styleID. Its style reference is the
    // last row of that styleID, as in `rest::text_overrides`; its paints are the first row of
    // that styleID with fill paints, as `render` draws them. Each row keeps only what it supplies
    // and the rest are blanked in place, so every relation read from the rows keeps its index.
    // Both passes are charged per row first.
    if let Some(text) = fields.get_mut("textData") {
        let uses = text["characterStyleIDs"].take();
        let uses = uses.as_array().map_or(&[][..], Vec::as_slice);
        let active = |sid: Option<u64>| {
            sid.is_some_and(|sid| {
                uses.binary_search_by(|u| u.as_u64().cmp(&Some(sid)))
                    .is_ok()
            })
        };
        if let Some(table) = text["styleOverrideTable"].as_array_mut() {
            if !bounds.charge(2 * table.len(), 33 * table.len()) {
                return None;
            }
            let mut painted = BTreeSet::new();
            let paints: Vec<bool> = table
                .iter()
                .map(|row| {
                    let sid = row["styleID"].as_u64();
                    row["fillPaints"].is_array() && active(sid) && painted.insert(sid)
                })
                .collect();
            let mut seen = BTreeSet::new();
            for (row, paints) in table.iter_mut().zip(paints).rev() {
                let sid = row["styleID"].as_u64();
                let last = seen.insert(sid) && active(sid);
                match (last, paints) {
                    (true, true) => {}
                    (true, false) => {
                        row.as_object_mut().map(|r| r.remove("fillPaints"));
                    }
                    (false, true) => *row = json!({"fillPaints": row["fillPaints"].take()}),
                    (false, false) => *row = Json::Null,
                }
            }
        }
    }
    // A vector region row applies where a fill geometry paints its styleID; as in
    // `render::region_fills`, the first row of one styleID that has fill paints wins.
    if let Some(vector) = fields.get_mut("vectorData") {
        let painted = vector["fillGeometryStyleIDs"].take();
        let painted = painted.as_array().map_or(&[][..], Vec::as_slice);
        if let Some(table) = vector["styleOverrideTable"].as_array_mut() {
            if !bounds.charge(table.len(), 16 * table.len()) {
                return None;
            }
            let mut seen = BTreeSet::new();
            for row in table.iter_mut() {
                let sid = row["styleID"].as_u64();
                let used = row["fillPaints"].is_array()
                    && sid.is_some_and(|sid| {
                        painted
                            .binary_search_by(|u| u.as_u64().cmp(&Some(sid)))
                            .is_ok()
                            && seen.insert(sid)
                    });
                if !used {
                    *row = Json::Null;
                }
            }
        }
    }
    // The row paints copied below and by `rest::bound_variables` are charged first; each field
    // was read within `VISITS`, so two fields' estimates fit twice that.
    let table = |k: &str| {
        let rows = fields
            .get(k)
            .and_then(|t| t["styleOverrideTable"].as_array());
        rows.map_or(&[][..], Vec::as_slice)
    };
    // The walk stops at what the side has left: running out there stops the side, visits kept.
    let cap = (2 * VISITS).min(bounds.work);
    let mut left = cap;
    let (copies, cut) = row_paints([table("vectorData"), table("textData")], &mut left);
    if !bounds.charge(cap - left, 0) {
        return None;
    }
    if (cut || left == 0 && cap > 0) && cap < 2 * VISITS {
        bounds.stop = Some(crate::expand::Stop::Work);
        return None;
    }
    if !bounds.charge(0, copies) {
        return None;
    }
    // REST lists no vector-region fills; usages reads their bindings like top-level fills.
    let regions: Json = fields
        .get("vectorData")
        .and_then(|v| v["styleOverrideTable"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|s| s["fillPaints"].as_array())
        .flatten()
        .cloned()
        .collect();
    if !prepay_aliases(&fields, bounds) {
        return None;
    }
    let mut bound = crate::rest::bound_variables(&fields);
    if let Some(list) =
        crate::rest::bound_variables(&Map::from_iter([("fillPaints".into(), regions)]))
            .remove("fills")
    {
        bound.insert("vectorRegionFills".into(), list);
    }
    // A local text style's font-size and weight bindings apply to the text that has none of its
    // own, as the REST node output inherits them; only the style's binding maps are read. A
    // library style (`assetRef`) is external, so none is inherited from it.
    if let Some(style) = fields
        .get("styleIdForText")
        .filter(|_| !(bound.contains_key("fontSize") && bound.contains_key("fontWeight")))
        .filter(|r| r["assetRef"]["key"].is_null())
        .and_then(|r| crate::rest::guid(&r["guid"]))
        .filter(|g| tree.nodes.contains_key(g))
    {
        let maps = effective(tree, &style, None, bounds, unread, true)?;
        if !prepay_aliases(&maps, bounds) {
            return None;
        }
        let inherited = crate::rest::bound_variables(&maps);
        for f in ["fontSize", "fontWeight"] {
            if let (Some(v), false) = (inherited.get(f), bound.contains_key(f)) {
                bound.insert(format!("styleIdForText.{f}"), v.clone());
            }
        }
    }
    // Each corner's binding keeps its own path, so a variable moved to another corner differs.
    if let Some(Json::Object(corners)) = bound.remove("rectangleCornerRadii") {
        for (corner, a) in corners {
            bound.insert(format!("rectangleCornerRadii.{corner}"), a);
        }
    }
    for (field, aliases) in bound {
        let mut ids = Vec::new();
        crate::variables::aliases(&aliases, &mut ids);
        for variable in ids {
            if let Some((key, v)) = variable
                .strip_prefix("VariableID:")
                .and_then(|v| v.split_once('/'))
                .filter(|_| libraries.is_some())
            {
                let ns = namespace("variable", key);
                let (target, state) = reference("variable", ns.as_deref(), key);
                let evidence = version(&field, Some(v));
                if !push(
                    bounds,
                    LIBRARY,
                    target.as_deref().map(|t| ("", t)),
                    state,
                    "cached_copy",
                    evidence,
                ) {
                    return None;
                }
            }
            let state = match variable.strip_prefix("VariableID:") {
                Some(v) if v.contains('/') => "external",
                Some(v) => node_state(tree, Some(v), "VARIABLE"),
                None => "unsupported",
            };
            if !push(
                bounds,
                "binds_variable",
                Some(("", &variable)),
                state,
                "recorded",
                field.clone(),
            ) {
                return None;
            }
        }
    }
    let rows = fields
        .get("textData")
        .and_then(|t| t["styleOverrideTable"].as_array())
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, row)| {
            Some((
                row.get("styleIdForText")?,
                format!("textData.styleOverrideTable[{i}].styleIdForText"),
            ))
        });
    let top = STYLES
        .into_iter()
        .filter_map(|k| Some((fields.get(k)?, k.to_owned())));
    for (r, evidence) in top.chain(rows) {
        if let Some(key) = r["assetRef"]["key"]
            .as_str()
            .filter(|_| libraries.is_some())
        {
            let ns = namespace("style", key);
            let (target, state) = reference("style", ns.as_deref(), key);
            let evidence = version(&evidence, r["assetRef"]["version"].as_str());
            if !push(
                bounds,
                LIBRARY,
                target.as_deref().map(|t| ("", t)),
                state,
                "cached_copy",
                evidence,
            ) {
                return None;
            }
        }
        let guid = crate::rest::guid(&r["guid"]);
        let (target, state) = match (r["assetRef"]["key"].as_str(), guid.as_deref()) {
            (Some(k), _) => (("StyleKey:", k), "external"),
            (None, Some(g)) if tree.nodes.contains_key(g) => (("", g), "resolved"),
            (None, Some(g)) => (("", g), "missing"),
            (None, None) => continue,
        };
        if !push(
            bounds,
            "uses_style",
            Some(target),
            state,
            "recorded",
            evidence,
        ) {
            return None;
        }
    }
    // Top-level paints, and the per-range text and per-region vector fills the renderer selects,
    // streamed from the fields `effective` already charged; only a kept edge names its path.
    let top = ["fillPaints", "strokePaints"]
        .into_iter()
        .map(|k| (None, k, fields.get(k)));
    let styled = STYLED.into_iter().flat_map(|table| {
        let rows = fields.get(table).map(|t| &t["styleOverrideTable"]);
        rows.and_then(Json::as_array)
            .into_iter()
            .flatten()
            .enumerate()
            .map(move |(i, row)| (Some((table, i)), "fillPaints", row.get("fillPaints")))
    });
    for (row, key, paints) in top.chain(styled) {
        let paints = paints.and_then(Json::as_array).into_iter().flatten();
        for (i, p) in paints.enumerate() {
            let Some(hash) = p["image"]["hash"].as_str().filter(|_| p["type"] == "IMAGE") else {
                continue;
            };
            let target = Some(("image:", hash));
            if !push(
                bounds,
                "uses_image",
                target,
                "recorded",
                "recorded",
                match row {
                    Some((table, r)) => format!("{table}.styleOverrideTable[{r}].{key}[{i}]"),
                    None => format!("{key}[{i}]"),
                },
            ) {
                return None;
            }
        }
    }
    // An instance can inherit its component's links without carrying any of its own.
    let has_prototype = kind == "INSTANCE"
        || e.is_some_and(|e| e.prototype_source.is_some())
        || tree
            .nodes
            .get(e.map_or(id, |e| e.source.as_str()))
            .is_some_and(|n| n.get("prototypeInteractions").is_some());
    // The reader reserves each projection at its caps from the build bounds before reading
    // it; only kept edges are retained, each charged before it is.
    let p = match has_prototype {
        true => {
            crate::prototype::node_with(tree, id, e, kind, &mut |w, b| bounds.charge(w, b)).ok()?
        }
        false => None,
    };
    if let Some(p) = p {
        // Interactions a component defines are not verified to apply to its instances.
        let inherited = p["basis"] == "component-definition";
        // Actions past the reader's caps are not read, so their targets are not verified absent.
        // Actions whose precedence over a swapped definition is unverified stay, but so does
        // that uncertainty.
        if p["truncated"] == true || p["precedenceState"] == "unsupported" {
            *unverified += 1;
        }
        let interactions = p["interactions"].as_array().into_iter().flatten();
        for action in interactions.flat_map(|i| i["actions"].as_array().into_iter().flatten()) {
            let d = &action["destination"];
            let state = match d["state"].as_str() {
                Some("resolved") => "resolved",
                Some("unresolved") => "missing",
                Some("unsupported") => "unsupported",
                _ if d["reason"].is_string() => "unresolved",
                _ => continue,
            };
            if inherited {
                *unverified += 1;
                continue;
            }
            let target = d["nodeId"].as_str().map(|t| ("", t));
            let evidence = format!(
                "{}:{}",
                p["basis"].as_str().unwrap_or(""),
                d["sourceField"].as_str().unwrap_or("")
            );
            if !push(
                bounds,
                "prototype_target",
                target,
                state,
                "recorded",
                evidence,
            ) {
                return None;
            }
        }
    }
    Some(out)
}

fn budget() -> Box<dyn Error> {
    FigError::new(
        ErrorKind::ResponseBudget,
        format!(
            "usages results would copy more than {} bytes of names and paths; lower limit",
            super::REPLY_BYTES
        ),
    )
    .into()
}

fn node_state(tree: &Tree, target: Option<&str>, kind: &str) -> &'static str {
    match target.map(|t| tree.nodes.get(t)) {
        None => "unresolved",
        Some(None) => "missing",
        Some(Some(n)) if plugin_type(n) == kind => "resolved",
        Some(Some(_)) => "unsupported",
    }
}

/// Charges what `rest::bound_variables` and `variables::aliases` copy from `fields`' alias
/// bindings, before they run: each id is held by the projection, its dedup key, the alias set,
/// the id list and an inherited copy, whether or not its edge can keep it.
fn prepay_aliases(fields: &Map<String, Json>, bounds: &mut Bounds) -> bool {
    // One visit past the side's work: a walk cut short there fails its charge, a stop for work.
    let cap = bounds.work.saturating_add(1);
    let mut left = cap;
    let bytes = sum_while(fields.values(), &mut left, alias_bytes);
    bounds.charge(cap - left, 0) && bounds.charge(0, bytes)
}

fn alias_bytes(v: &Json, left: &mut usize) -> usize {
    if *left == 0 {
        return 0;
    }
    *left -= 1;
    match v {
        Json::Array(a) => sum_while(a, left, alias_bytes),
        Json::Object(o) => {
            let own = if v["dataType"] == "ALIAS" {
                let r = &v["value"]["alias"];
                let key = [&r["assetRef"]["key"], &r["assetRef"]["version"]]
                    .iter()
                    .filter_map(|s| s.as_str())
                    .map(str::len)
                    .sum::<usize>();
                // Four raw copies of `VariableID:<guid or key/version>`, plus the serialized dedup
                // key, which escapes each byte to at most six.
                10 * key + 1024
            } else {
                0
            };
            own + sum_while(o.values(), left, alias_bytes)
        }
        _ => 0,
    }
}

/// Estimated bytes of the fill paints in `tables`' rows, walking at most `left`: a visit for
/// each row, a blanked inactive one too, then its paints'. True when a row was left unwalked.
fn row_paints(tables: [&[Json]; 2], left: &mut usize) -> (usize, bool) {
    let mut bytes = 0;
    for row in tables.into_iter().flatten() {
        if *left == 0 {
            return (bytes, true);
        }
        *left -= 1;
        if let Some(p) = row.get("fillPaints") {
            bytes += json_bytes(p, left);
        }
    }
    (bytes, false)
}

/// The record's effective binding fields as JSON: an expanded sublayer's override wins over
/// its source's own field. Each is charged at its estimated size before it is converted.
fn effective(
    tree: &Tree,
    id: &str,
    e: Option<&Expanded>,
    bounds: &mut Bounds,
    unread: &mut usize,
    maps_only: bool,
) -> Option<Map<String, Json>> {
    let node = tree.nodes.get(e.map_or(id, |e| e.source.as_str()));
    let mut out = Map::new();
    for key in FIELDS
        .iter()
        .chain(&STYLES)
        .chain(["textData", "vectorData"].iter())
        .filter(|k| !maps_only || k.ends_with("ConsumptionMap"))
    {
        let own = e.and_then(|e| e.props.get(*key));
        let value = match (own, node.and_then(|n| n.get(key))) {
            (Some(_), _) | (None, None) => None,
            // Only the per-range / per-region style table of text and vectors is read.
            (None, Some(v)) if STYLED.contains(key) => match v.get("styleOverrideTable") {
                Some(t) => Some(t),
                None => continue,
            },
            (None, Some(v)) => Some(v),
        };
        // The prefilter's visits are charged whether or not it finds a link, never past the
        // remaining work, and only after the deadline is checked.
        if own.is_none() && value.is_none() {
            continue;
        }
        if !bounds.charge(0, 0) {
            return None;
        }
        let cap = VISITS.min(bounds.work);
        let mut scan = cap;
        let link = match (own, value) {
            (Some(v), _) => linked_json(v, &mut scan),
            (None, Some(v)) => linked(v, &mut scan),
            (None, None) => continue,
        };
        if !bounds.charge(cap - scan, 0) {
            return None;
        }
        match link {
            Some(true) => {}
            Some(false) => continue,
            None if cap < VISITS => {
                bounds.stop = Some(crate::expand::Stop::Work);
                return None;
            }
            // A field too large to scan is left out and counted, like one too large to read.
            None => {
                *unread += 1;
                continue;
            }
        }
        let cap = VISITS.min(bounds.work);
        let mut left = cap;
        let bytes = match (own, value) {
            (Some(v), _) => json_bytes(v, &mut left),
            (None, Some(v)) => kiwi_bytes(v, &mut left),
            (None, None) => continue,
        };
        // The visits made are charged either way. One field past the per-value cap is left out
        // and counted, not the rest of the index; running out of the side's work stops it.
        if !bounds.charge(cap - left, 0) {
            return None;
        }
        if left == 0 {
            if cap < VISITS {
                bounds.stop = Some(crate::expand::Stop::Work);
                return None;
            }
            *unread += 1;
            continue;
        }
        if !bounds.charge(0, bytes) {
            return None;
        }
        let mut converted = match (own, value) {
            (Some(v), _) if STYLED.contains(key) => {
                json!({"styleOverrideTable": v["styleOverrideTable"]})
            }
            (Some(v), _) => v.clone(),
            (None, Some(v)) if STYLED.contains(key) => json!({"styleOverrideTable": to_json(v)}),
            (None, Some(v)) => to_json(v),
            (None, None) => continue,
        };
        // Only the rows a character uses apply, as `rest::text_overrides` selects them.
        if *key == "textData" {
            use kiwi_schema::Value as K;
            let uses = match own {
                Some(v) => match v["characterStyleIDs"].as_array() {
                    Some(ids) => active(ids.iter().map(Json::as_u64), bounds, unread),
                    None => Some(json!([])),
                },
                None => match node.and_then(|n| n.get("textData")?.get("characterStyleIDs")) {
                    Some(K::Array(ids)) => active(
                        ids.iter().map(|i| match i {
                            K::UInt(u) => Some(u64::from(*u)),
                            K::UInt64(u) => Some(*u),
                            _ => None,
                        }),
                        bounds,
                        unread,
                    ),
                    _ => Some(json!([])),
                },
            }?;
            converted["characterStyleIDs"] = uses;
        }
        // Only the regions a fill geometry paints apply, as `render::region_fills` selects them;
        // a geometry without a styleID paints style 0.
        if *key == "vectorData" {
            use kiwi_schema::Value as K;
            let geometry = match e.and_then(|e| e.props.get("fillGeometry")) {
                Some(g) => match g.as_array() {
                    Some(list) => active(
                        list.iter()
                            .map(|g| Some(g["styleID"].as_u64().unwrap_or(0))),
                        bounds,
                        unread,
                    ),
                    None => Some(json!([])),
                },
                None => match node.and_then(|n| n.get("fillGeometry")) {
                    Some(K::Array(list)) => active(
                        list.iter().map(|g| match g.get("styleID") {
                            Some(K::UInt(u)) => Some(u64::from(*u)),
                            Some(K::UInt64(u)) => Some(*u),
                            _ => Some(0),
                        }),
                        bounds,
                        unread,
                    ),
                    _ => Some(json!([])),
                },
            }?;
            converted["fillGeometryStyleIDs"] = geometry;
        }
        out.insert((*key).into(), converted);
    }
    Some(out)
}

/// The sorted style row ids a text's characters use, charged one visit and one set and JSON
/// entry per character before they are collected; a list past one field's visit cap is left
/// out and counted, so none of its rows' styles is claimed.
fn active(
    ids: impl ExactSizeIterator<Item = Option<u64>>,
    bounds: &mut Bounds,
    unread: &mut usize,
) -> Option<Json> {
    if ids.len() > VISITS {
        *unread += 1;
        return Some(json!([]));
    }
    if !bounds.charge(ids.len(), 48 * ids.len()) {
        return None;
    }
    Some(json!(ids.flatten().collect::<BTreeSet<u64>>()))
}

/// Whether a stored field can yield an edge: a paint list (or text style table) with a bound
/// color, gradient stop or image; any other field is kept whole. Skips converting the plain
/// paints most layers carry. Each paint and stop takes one of `left` visits; `None` when they
/// run out first.
fn linked(v: &kiwi_schema::Value, left: &mut usize) -> Option<bool> {
    use kiwi_schema::Value;
    // Plain paints carry an unbound colorVar too; only an alias names a variable.
    fn alias(c: &Value) -> bool {
        matches!(c.get("dataType"), Some(Value::Enum(_, "ALIAS")))
    }
    let Value::Array(items) = v else {
        return Some(true);
    };
    for p in items {
        *left = left.checked_sub(1)?;
        // A text style row links through its own style.
        if p.get("colorVar").is_some_and(alias)
            || matches!(p.get("type"), Some(Value::Enum(_, "IMAGE")))
            || p.get("styleIdForText").is_some()
        {
            return Some(true);
        }
        if let Some(Value::Array(stops)) = p.get("stopsVar") {
            for stop in stops {
                *left = left.checked_sub(1)?;
                if stop.get("colorVar").is_some_and(alias) {
                    return Some(true);
                }
            }
        }
        if let Some(f) = p.get("fillPaints")
            && linked(f, left)?
        {
            return Some(true);
        }
    }
    Some(false)
}

/// `linked` for an override already in JSON.
fn linked_json(v: &Json, left: &mut usize) -> Option<bool> {
    let Json::Array(items) = v else {
        return Some(true);
    };
    for p in items {
        *left = left.checked_sub(1)?;
        if p["colorVar"]["dataType"] == "ALIAS"
            || p["type"] == "IMAGE"
            || p.get("styleIdForText").is_some()
        {
            return Some(true);
        }
        for stop in p["stopsVar"].as_array().into_iter().flatten() {
            *left = left.checked_sub(1)?;
            if stop["colorVar"]["dataType"] == "ALIAS" {
                return Some(true);
            }
        }
        if let Some(f) = p.get("fillPaints")
            && linked_json(f, left)?
        {
            return Some(true);
        }
    }
    Some(false)
}

/// What the index knows about each node or variable its edges point at, and about every
/// component-set member it indexes, wherever the node lies; `None` when `bounds` ran out.
pub(crate) fn definitions(
    tree: &Tree,
    records: &[Record],
    bounds: &mut Bounds,
) -> Option<BTreeMap<String, Definition>> {
    let mut wanted = BTreeSet::new();
    for r in records {
        if !bounds.charge(1 + r.edges.len(), 0) {
            return None;
        }
        for e in &r.edges {
            if let Some(t) = &e.target
                && e.state == "resolved"
            {
                // One set entry per target, and one more per variant member, charged first.
                let entries = 1 + usize::from(e.relation == "variant_of");
                if !bounds.charge(0, 64 * entries) {
                    return None;
                }
                wanted.insert(t.as_str());
                if e.relation == "variant_of" {
                    wanted.insert(r.id.as_str());
                }
            }
        }
    }
    // Variables are direct children of a page; each is a definition even when nothing in the
    // scope binds it, so an unused one answers as such rather than as unknown.
    let mut variables = Vec::new();
    for page in tree.children.get("0:0").into_iter().flatten() {
        // A page pays its lookup, even one with no children.
        if !bounds.charge(1, 0) {
            return None;
        }
        for id in tree.children.get(page).into_iter().flatten() {
            if !bounds.charge(1, 0) {
                return None;
            }
            let Some(node) = tree.nodes.get(id).filter(|n| plugin_type(n) == "VARIABLE") else {
                continue;
            };
            if !bounds.charge(0, 64 + 2 * id.len()) {
                return None;
            }
            variables.push((format!("VariableID:{id}"), id.as_str()));
            // A library's cached copy is also bound as `VariableID:<key>/<version>`.
            if let (Some(key), Some(version)) = (text(node, "key"), text(node, "version")) {
                if !bounds.charge(0, 64 + key.len() + version.len() + id.len()) {
                    return None;
                }
                variables.push((format!("VariableID:{key}/{version}"), id.as_str()));
            }
        }
    }
    let mut out = BTreeMap::new();
    let mut sets = Vec::new();
    for (target, node_id) in wanted
        .into_iter()
        .map(|t| (t, t.strip_prefix("VariableID:").unwrap_or(t)))
        .chain(variables.iter().map(|(t, id)| (t.as_str(), *id)))
    {
        let Some(node) = tree.nodes.get(node_id) else {
            continue;
        };
        let kind = plugin_type(node);
        let set = parent_of(node).filter(|p| {
            kind == "COMPONENT"
                && tree
                    .nodes
                    .get(p)
                    .is_some_and(|n| plugin_type(n) == "COMPONENT_SET")
        });
        // The page: the nearest CANVAS ancestor, one charged step per level.
        let mut page = None;
        let mut cur = parent_of(node);
        let mut steps = 0;
        while let Some(p) = cur {
            let Some(n) = tree.nodes.get(&p) else { break };
            steps += 1;
            if steps > crate::expand::MAX_DEPTH || !bounds.charge(1, 0) {
                break;
            }
            if plugin_type(n) == "CANVAS" {
                page = Some(p);
                break;
            }
            cur = parent_of(n);
        }
        let name = kiwi_name(node);
        // The reverse lookups visit every definition once more, and copy a set member's id and
        // set again into the member lists (estimated storage).
        let member = set.as_ref().map_or(0, |s| 48 + 2 * s.len() + target.len());
        if !bounds.charge(2, 160 + target.len() + name.len() + kind.len() + member) {
            return None;
        }
        let name = name.to_owned();
        if let Some(s) = &set {
            sets.push(s.clone());
        }
        out.insert(
            target.to_owned(),
            Definition {
                name,
                kind,
                set,
                page,
            },
        );
    }
    // A member's set is described too, so a set query names it even outside the scope.
    for s in sets {
        if out.contains_key(&s) {
            continue;
        }
        let node = tree.nodes[&s];
        let name = kiwi_name(node);
        // One more visit for the reverse lookups' pass over every definition.
        if !bounds.charge(2, 160 + s.len() + name.len()) {
            return None;
        }
        let name = name.to_owned();
        let page = parent_of(node).filter(|p| {
            tree.nodes
                .get(p)
                .is_some_and(|n| plugin_type(n) == "CANVAS")
        });
        out.insert(
            s,
            Definition {
                name,
                kind: "COMPONENT_SET".into(),
                set: None,
                page,
            },
        );
    }
    Some(out)
}

// ---- Query ----

/// One `usages` call: an exact node or resource of one source and scope, its relations,
/// direction and page.
#[derive(Clone, Debug)]
pub struct Request {
    /// A node id (stored or expanded `I…;…`) or a resource id (`VariableID:…`, `StyleKey:…`,
    /// `image:…`).
    pub target: String,
    pub resource: bool,
    pub scope: String,
    pub relations: Vec<String>,
    pub outgoing: bool,
    /// The snapshot the caller's reference came from; another served snapshot is `source_stale`.
    pub snapshot: Option<String>,
    pub include_hidden: bool,
    pub limit: usize,
    pub cursor: Option<String>,
}

impl Request {
    /// The caller's question as tool arguments, for a recovery that repeats it elsewhere; the
    /// cursor is left out, since it is bound to the source it came from.
    pub(crate) fn arguments(&self) -> Json {
        let target = if self.resource { "resource" } else { "node" };
        let mut asked = json!({target: self.target, "scope": self.scope, "limit": self.limit});
        if self.outgoing {
            asked["direction"] = json!("out");
        }
        if !self.relations.is_empty() {
            asked["relation"] = json!(self.relations.join(","));
        }
        if self.include_hidden {
            asked["includeHidden"] = json!(true);
        }
        if let Some(snapshot) = &self.snapshot {
            asked["snapshotId"] = json!(snapshot);
        }
        asked
    }
}

/// The ordering key of one result: location record, relation and sequence.
pub(crate) type Key = (u32, String, u32);

/// Matching locations in key order, and the smallest key of a hidden one passed over.
pub(crate) type Inbound = (Vec<(u32, Edge)>, Option<Key>);

/// Keeps the smaller of `left` and `key`.
pub(crate) fn lowest(left: &mut Option<Key>, key: Key) {
    if left.as_ref().is_none_or(|l| key < *l) {
        *left = Some(key);
    }
}

/// A request that passed validation, before any source is read.
pub struct Prepared {
    pub(crate) relations: Vec<&'static str>,
    pub(crate) after: Option<Key>,
    cursor: Option<(String, String, i64)>,
}

impl Prepared {
    /// The generation a workspace cursor paged, `0` for a stateless cursor.
    pub(crate) fn cursor_gen(&self) -> Option<i64> {
        self.cursor.as_ref().map(|c| c.2)
    }
}

fn invalid(message: impl Into<String>) -> Box<dyn Error> {
    FigError::new(ErrorKind::InvalidArguments, message).into()
}

impl Request {
    /// Checks ids, relations, limit and cursor syntax; cheap, so callers run it before I/O.
    pub fn validate(&self) -> Result<Prepared> {
        // Any id an index can store, the deepest expanded sublayer included.
        let max = super::store::ID_BYTES;
        for (what, v) in [("node", &self.target), ("scope", &self.scope)] {
            if v.is_empty() || v.len() > max {
                return Err(invalid(format!("usages {what} takes 1 to {max} bytes")));
            }
        }
        let resource = ["VariableID:", "StyleKey:", "image:"]
            .iter()
            .any(|p| self.target.starts_with(p));
        if resource != self.resource {
            return Err(invalid(if self.resource {
                "resource takes a VariableID:…, StyleKey:… or image:… id; pass a layer id as node"
            } else {
                "node takes a layer id; pass VariableID:…, StyleKey:… or image:… as resource"
            }));
        }
        if !(1..=MAX_LIMIT).contains(&self.limit) {
            return Err(invalid(format!("usages limit takes 1 to {MAX_LIMIT}")));
        }
        let mut relations = Vec::new();
        for r in &self.relations {
            let Some(k) = KINDS.iter().find(|k| **k == r) else {
                return Err(invalid(format!(
                    "unknown relation {r:?}; use {}",
                    KINDS.join(", ")
                )));
            };
            if !relations.contains(k) {
                relations.push(*k);
            }
        }
        if relations.is_empty() {
            // Containment is a usage only when asked for; outgoing, a node's parent is listed.
            relations = KINDS
                .iter()
                .copied()
                .filter(|k| self.outgoing || *k != "contains")
                .collect();
        }
        relations.sort_unstable();
        let mut cursor = None;
        let mut after = None;
        if let Some(c) = &self.cursor {
            let bad = || invalid("invalid usages cursor; pass nextCursor from the previous reply");
            let parts: Vec<&str> = c.split('.').collect();
            let [tag, fp, snap, gen_id, rid, relation, seq] = parts[..] else {
                return Err(bad());
            };
            let relation = KINDS.iter().find(|k| **k == relation).ok_or_else(bad)?;
            if tag != "u1" || fp.len() != 16 || snap.len() != 16 {
                return Err(bad());
            }
            let num = |s: &str| s.parse::<u32>().map_err(|_| bad());
            after = Some((num(rid)?, (*relation).to_owned(), num(seq)?));
            cursor = Some((
                fp.to_owned(),
                snap.to_owned(),
                gen_id.parse::<i64>().map_err(|_| bad())?,
            ));
        }
        if self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.len() > MAX_ID_BYTES)
        {
            return Err(invalid("snapshotId is too long"));
        }
        Ok(Prepared {
            relations,
            after,
            cursor,
        })
    }
}

/// The relations one query reads, behind either a whole in-memory index or a stored generation.
pub(crate) trait Graph {
    fn index(&self) -> &Index;
    /// The record of `node`, if the index has it.
    fn rid_of(&mut self, node: &str) -> Result<Option<u32>>;
    fn definition(&mut self, id: &str) -> Result<Option<Definition>>;
    /// Up to `limit` known members of `set`, in id order.
    fn members(&mut self, set: &str, limit: usize) -> Result<Vec<String>>;
    /// The state of any edge that points at `target`.
    fn targeted(&mut self, target: &str) -> Result<Option<&'static str>>;
    /// Up to `limit` edges after `after` whose target is in `targets`, in key order, with
    /// hidden locations left out unless `hidden`, with the smallest key of a matching hidden
    /// location the same scan passed over. A `contains` relation matches children of `parent`.
    fn inbound(
        &mut self,
        targets: &[String],
        relations: &[&'static str],
        parent: Option<u32>,
        hidden: bool,
        after: Option<&Key>,
        limit: usize,
    ) -> Result<Inbound>;
    /// The edges of record `rid`, its parent as `contains`, in key order.
    fn outbound(&mut self, rid: u32) -> Result<Vec<Edge>>;
    /// The library references record `rid` keeps apart from its edges, in seq order.
    fn library(&mut self, rid: u32) -> Result<Vec<Edge>>;
    /// Makes records `rids` and their ancestors readable through `index`.
    fn load(&mut self, rids: &[u32]) -> Result<()>;
}

/// The in-memory index of a stateless call: every record is loaded.
/// Edges or children one page may pass over before it is refused, so a page costs what it
/// returns plus what its filters skip, never every earlier match.
pub(crate) const SCAN: usize = 50_000;

pub(crate) fn scan_limit() -> Box<dyn Error> {
    FigError::new(
        ErrorKind::QueryWorkLimit,
        format!("one usages page would pass over more than {SCAN} filtered-out edges; pass includeHidden or fewer relations"),
    )
    .into()
}

/// Lookups from a target to what points at it, over a whole built index.
#[derive(Default)]
pub(crate) struct Reverse {
    /// `(rid, edge)` positions per target, in key order.
    by_target: HashMap<String, Vec<(u32, u32)>>,
    /// Child rids per parent rid, ascending.
    children: HashMap<u32, Vec<u32>>,
    rids: HashMap<String, u32>,
    /// Members per component set, in id order.
    members: HashMap<String, Vec<String>>,
}

impl Reverse {
    // A full build prepays these lookups' visits and estimated copies with each record, edge
    // and definition, so they stay within the build's bounds; `prepare` pays for any other.
    fn of(index: &Index) -> &Reverse {
        index.reverse.get_or_init(|| {
            let mut out = Reverse::default();
            for (rid, r) in index.records.iter().enumerate() {
                let rid = rid as u32;
                out.rids.insert(r.id.clone(), rid);
                if let Some(p) = r.parent {
                    out.children.entry(p).or_default().push(rid);
                }
                for (i, e) in r.edges.iter().enumerate() {
                    if let Some(t) = &e.target {
                        out.by_target
                            .entry(t.clone())
                            .or_default()
                            .push((rid, i as u32));
                    }
                }
            }
            for (id, d) in &index.definitions {
                if let Some(set) = &d.set {
                    out.members.entry(set.clone()).or_default().push(id.clone());
                }
            }
            out
        })
    }
}

/// Builds `index`'s reverse map for `pointing_at` and `rid_in`, first paying `bounds` what a
/// relations build estimates for it: a loaded index never paid that, and a fresh build that
/// did is charged again, since an unbuilt map cannot tell which it is. False, with nothing
/// built, when it does not fit; an index whose map is already built costs nothing more.
pub(crate) fn prepare(index: &Index, bounds: &mut Bounds) -> bool {
    if index.reverse.get().is_none() {
        // Visits are paid from lengths before each scan, twice: this estimate and the build.
        for r in &index.records {
            if !bounds.charge(2 * (1 + r.edges.len()), 0) {
                return false;
            }
            let targets = r.edges.iter().filter_map(|e| e.target.as_ref());
            let bytes = 64 + r.id.len() + targets.map(|t| 56 + t.len()).sum::<usize>();
            if !bounds.charge(0, bytes) {
                return false;
            }
        }
        for (id, d) in &index.definitions {
            if !bounds.charge(2, 0) {
                return false;
            }
            if let Some(set) = &d.set
                && !bounds.charge(0, 64 + id.len() + set.len())
            {
                return false;
            }
        }
    }
    Reverse::of(index);
    true
}

/// The `(rid, edge)` positions pointing at `target`, from the reverse map `prepare` built.
pub(crate) fn pointing_at<'a>(index: &'a Index, target: &str) -> &'a [(u32, u32)] {
    Reverse::of(index)
        .by_target
        .get(target)
        .map_or(&[], Vec::as_slice)
}

/// The rid of `node` in `index`, from the same map.
pub(crate) fn rid_in(index: &Index, node: &str) -> Option<u32> {
    Reverse::of(index).rids.get(node).copied()
}

pub(crate) struct Whole<'a>(pub &'a Index);

impl Graph for Whole<'_> {
    fn index(&self) -> &Index {
        self.0
    }
    fn rid_of(&mut self, node: &str) -> Result<Option<u32>> {
        Ok(Reverse::of(self.0).rids.get(node).copied())
    }
    fn definition(&mut self, id: &str) -> Result<Option<Definition>> {
        Ok(self.0.definitions.get(id).cloned())
    }
    fn members(&mut self, set: &str, limit: usize) -> Result<Vec<String>> {
        let members = Reverse::of(self.0).members.get(set);
        Ok(members.into_iter().flatten().take(limit).cloned().collect())
    }
    fn targeted(&mut self, target: &str) -> Result<Option<&'static str>> {
        let first = Reverse::of(self.0)
            .by_target
            .get(target)
            .and_then(|l| l.first());
        Ok(first.map(|&(rid, i)| self.0.record(rid).edges[i as usize].state))
    }
    fn inbound(
        &mut self,
        targets: &[String],
        relations: &[&'static str],
        parent: Option<u32>,
        hidden: bool,
        after: Option<&Key>,
        limit: usize,
    ) -> Result<Inbound> {
        let index = self.0;
        let rev = Reverse::of(index);
        let past = |rid: u32, relation: &str, seq: u32| {
            after.is_none_or(|a| (rid, relation, seq) > (a.0, a.1.as_str(), a.2))
        };
        let mut out = Vec::new();
        let (mut scanned, mut left) = (0, None);
        for t in targets {
            let list = rev.by_target.get(t).map_or(&[][..], Vec::as_slice);
            let edge = |&(rid, i): &(u32, u32)| (rid, &index.record(rid).edges[i as usize]);
            let start = list.partition_point(|p| {
                let (rid, e) = edge(p);
                !past(rid, e.relation, e.seq)
            });
            let mut kept = 0;
            for p in &list[start..] {
                if kept == limit {
                    break;
                }
                scanned += 1;
                if scanned > SCAN {
                    return Err(scan_limit());
                }
                let (rid, e) = edge(p);
                if !relations.contains(&e.relation) {
                    continue;
                }
                if index.record(rid).hidden && !hidden {
                    lowest(&mut left, (rid, e.relation.into(), e.seq));
                    continue;
                }
                out.push((rid, e.clone()));
                kept += 1;
            }
        }
        if let Some(parent) = parent.filter(|_| relations.contains(&"contains")) {
            let kids = rev.children.get(&parent).map_or(&[][..], Vec::as_slice);
            let start = kids.partition_point(|&rid| !past(rid, "contains", 0));
            let mut kept = 0;
            for &rid in &kids[start..] {
                if kept == limit {
                    break;
                }
                scanned += 1;
                if scanned > SCAN {
                    return Err(scan_limit());
                }
                let r = index.record(rid);
                if r.hidden && !hidden {
                    lowest(&mut left, (rid, "contains".into(), 0));
                } else {
                    out.push((rid, contains(r)));
                    kept += 1;
                }
            }
        }
        out.sort_by(|a, b| (a.0, a.1.relation, a.1.seq).cmp(&(b.0, b.1.relation, b.1.seq)));
        out.truncate(limit);
        Ok((out, left))
    }
    fn outbound(&mut self, rid: u32) -> Result<Vec<Edge>> {
        let r = self.0.record(rid);
        let mut out: Vec<Edge> = r
            .parent
            .map(|p| parent_edge(self.0, r, p))
            .into_iter()
            .collect();
        out.extend(r.edges.iter().cloned());
        out.sort_by(|a, b| (a.relation, a.seq).cmp(&(b.relation, b.seq)));
        Ok(out)
    }
    fn library(&mut self, rid: u32) -> Result<Vec<Edge>> {
        Ok(self.0.record(rid).library.clone())
    }
    fn load(&mut self, _: &[u32]) -> Result<()> {
        Ok(())
    }
}

/// A child's `contains` edge: its parent contains it.
fn contains(r: &Record) -> Edge {
    Edge {
        relation: "contains",
        seq: 0,
        target: None,
        state: "resolved",
        basis: if r.source.is_some() {
            "resolved"
        } else {
            "recorded"
        },
        evidence: "parent".into(),
    }
}

fn parent_edge(index: &Index, r: &Record, parent: u32) -> Edge {
    Edge {
        target: Some(index.record(parent).id.clone()),
        ..contains(r)
    }
}

/// The served source: its id, snapshot and, for a workspace, the generation paged.
pub(crate) struct Served<'a> {
    pub source: &'a Json,
    pub gen_id: i64,
}

/// The usages reply for `req` over `graph`.
pub(crate) fn query(
    graph: &mut dyn Graph,
    q: &Prepared,
    req: &Request,
    served: Served,
) -> Result<Json> {
    let snapshot = served.source["snapshotId"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    if let Some(asked) = &req.snapshot
        && *asked != snapshot
    {
        // The recovery repeats the caller's whole question against the current snapshot.
        let mut retry = req.arguments();
        retry["snapshotId"] = json!(snapshot);
        return Err(FigError::new(
            ErrorKind::SourceStale,
            format!("{} was read from snapshot {asked}, but the source now serves {snapshot}; query again with the current snapshotId, and reopen the node in it",
                req.target),
        )
        .with_recovery(json!([{"action": "use_current_snapshot", "requires": "none", "tool": "usages",
            "arguments": retry,
            "detail": "keep the file or workspaceId and sourceId you sent; the same id may name another layer in the new snapshot, so reopen it there first"}]))
        .into());
    }
    let index_generation = graph.index().generation.clone();
    let fingerprint = short_hash(
        json!([
            served.source["sourceId"],
            snapshot,
            served.gen_id,
            index_generation,
            req.scope,
            req.target,
            q.relations,
            req.outgoing,
            req.include_hidden,
            RELATIONS
        ])
        .to_string(),
    );
    // A rebuilt index of the same snapshot can stop elsewhere, so cursors bind to the build too.
    let snap = short_hash(format!("{snapshot}.{index_generation}"));
    if let Some((fp, s, gen_id)) = &q.cursor {
        if *s != snap || *gen_id != served.gen_id {
            return Err(FigError::new(
                ErrorKind::CursorStale,
                "the source was reindexed or changed since this cursor; query usages again without cursor",
            )
            .into());
        }
        if *fp != fingerprint {
            return Err(invalid(
                "usages cursor belongs to another query; query again without cursor",
            ));
        }
    }
    let target = req.target.as_str();
    let rid = if req.resource {
        None
    } else {
        graph.rid_of(target)?
    };
    let definition = match (graph.definition(target)?, rid) {
        (Some(d), _) => Some(d),
        (None, Some(rid)) => {
            graph.load(&[rid])?;
            let r = graph.index().record(rid);
            Some(Definition {
                name: r.name.clone(),
                kind: r.kind.clone(),
                set: None,
                page: None,
            })
        }
        (None, None) => None,
    };
    let state = match (&rid, &definition) {
        (Some(_), _) => "indexed",
        // A library variable named from its cached copy is still the library's.
        (None, Some(_)) if target.starts_with("VariableID:") && target.contains('/') => {
            graph.targeted(target)?.unwrap_or("external")
        }
        (None, Some(_)) => "outside_scope",
        (None, None) => match graph.targeted(target)? {
            Some(s) => s,
            None => {
                return Err(FigError::new(
                    ErrorKind::NodeNotFound,
                    format!("{target} is neither indexed in scope {} nor the target of any relation it holds; check the id and snapshot, or index a scope that has it", req.scope),
                )
                .into());
            }
        },
    };
    let kind = definition.as_ref().map(|d| d.kind.as_str()).unwrap_or("");
    let mut members = Vec::new();
    let mut members_complete = true;
    let mut members_truncated = false;
    if kind == "COMPONENT_SET" && !req.outgoing {
        members = graph.members(target, MAX_MEMBERS + 1)?;
        // Every member is known only when the set itself was indexed whole; a set outside the
        // scope is known by the members the scope uses.
        members_truncated = members.len() > MAX_MEMBERS;
        members_complete = !members_truncated && rid.is_some() && graph.index().stop.is_none();
        members.truncate(MAX_MEMBERS);
    }
    // Every reply copies the queried id, its definition and the members, so they start each
    // page's budget and are checked even when no row follows.
    let head = 256
        + target.len()
        + definition.as_ref().map_or(0, |d| {
            d.name.len()
                + d.kind.len()
                + d.set.as_ref().map_or(0, String::len)
                + d.page.as_ref().map_or(0, String::len)
        })
        + members.iter().map(|m| 8 + m.len()).sum::<usize>();
    if head > super::REPLY_BYTES {
        return Err(budget());
    }
    let after = q.after.as_ref();
    let (hits, more, results): (Vec<(u32, Edge)>, bool, Vec<Json>);
    let mut hidden_excluded = false;
    let mut library_unread = false;
    let source = served.source;
    if req.outgoing {
        let Some(rid) = rid else {
            return Err(invalid(format!(
                "outgoing usages need a node indexed in scope {}; {target} is not",
                req.scope
            )));
        };
        let mut all: Vec<(u32, Edge)> = graph
            .outbound(rid)?
            .into_iter()
            .filter(|e| q.relations.contains(&e.relation))
            .filter(|e| after.is_none_or(|a| (rid, e.relation, e.seq) > (a.0, a.1.as_str(), a.2)))
            .map(|e| (rid, e))
            .collect();
        more = all.len() > req.limit;
        all.truncate(req.limit);
        hits = all;
        // Each target's definition is read once, and the reply budget is checked as it grows,
        // so repeated or large definitions never pile up copies past the budget.
        let mut defs: HashMap<&str, Option<Definition>> = HashMap::new();
        // The library reference each cached-copy use names, read only when one could.
        let named_use = hits.iter().any(|(_, e)| LIBRARY_OF.contains(&e.relation));
        let refs = match named_use {
            true => named(graph.library(rid)?),
            false => Vec::new(),
        };
        library_unread = named_use && graph.index().library_unread;
        // Only the references this page names are charged, each as it is sent.
        let libraries: Vec<Option<Json>> = hits.iter().map(|(_, e)| library_of(e, &refs)).collect();
        let mut bytes = head
            + libraries
                .iter()
                .flatten()
                .map(|l| l.to_string().len())
                .sum::<usize>();
        for (_, e) in &hits {
            let name = match e.target.as_deref() {
                Some(t) => {
                    if !defs.contains_key(t) {
                        let d = graph.definition(t)?;
                        defs.insert(t, d);
                    }
                    defs[t].as_ref().map_or(0, |d| d.name.len())
                }
                None => 0,
            };
            bytes += 256 + e.target.as_ref().map_or(0, String::len) + e.evidence.len() + name;
            if bytes > super::REPLY_BYTES {
                return Err(budget());
            }
        }
        results = hits
            .iter()
            .zip(libraries)
            .map(|((_, e), l)| {
                let d = e.target.as_deref().and_then(|t| defs[t].as_ref());
                let mut out = outgoing(e, d, source);
                if let Some(l) = l {
                    out["library"] = l;
                }
                out
            })
            .collect();
    } else {
        let mut targets = vec![target.to_owned()];
        targets.extend(members.iter().cloned());
        let (mut found, left) = graph.inbound(
            &targets,
            &q.relations,
            rid,
            req.include_hidden,
            after,
            req.limit + 1,
        )?;
        more = found.len() > req.limit;
        found.truncate(req.limit);
        // A hidden match the scan passed over inside this page's range, or anywhere after the
        // cursor on the last page, so a reader of all pages sees every exclusion at any limit.
        hidden_excluded = left.is_some_and(|h| {
            !more
                || found
                    .last()
                    .is_some_and(|(r, e)| h < (*r, e.relation.into(), e.seq))
        });
        let rids: Vec<u32> = found.iter().map(|h| h.0).collect();
        graph.load(&rids)?;
        let index = graph.index();
        let bytes: usize = head
            + found
                .iter()
                .map(|(rid, _)| super::location_bytes(index, *rid))
                .sum::<usize>();
        if bytes > super::REPLY_BYTES {
            return Err(budget());
        }
        results = found
            .iter()
            .map(|(rid, e)| {
                let mut out = super::location(index, *rid, source);
                edge_fields(&mut out, e);
                if let Some(t) = e
                    .target
                    .as_ref()
                    .filter(|t| *t != target && e.relation != "contains")
                {
                    out["via"] = json!(t);
                }
                out
            })
            .collect();
        hits = found;
    }
    let index = graph.index();
    let last = hits
        .last()
        .map(|(rid, e)| format!("{rid}.{}.{}", e.relation, e.seq));
    let mut reasons = Vec::new();
    if let Some(stop) = index.stop {
        reasons.push(json!({"code": stop}));
    }
    if index.hidden_content > 0 {
        reasons.push(json!({"code": "hidden_content_not_expanded",
            "counts": {"hiddenLayers": index.hidden_content}}));
    }
    if hidden_excluded {
        reasons.push(json!({"code": "hidden_locations_excluded",
            "detail": "pass includeHidden to list usages on hidden layers"}));
    }
    if library_unread {
        reasons.push(json!({"code": "library_reference_unread",
            "detail": "the library scan did not fit this build, so no result names a library; ask with a narrower scope"}));
    }
    if members_truncated {
        reasons.push(json!({"code": "members_truncated", "limits": {"members": MAX_MEMBERS}}));
    } else if !members_complete && index.stop.is_none() {
        reasons.push(json!({"code": "members_outside_scope",
            "detail": "the set lies outside the scope; members are the ones the scope uses"}));
    }
    let fielded = FIELDED;
    let unread = index.unread_fields > 0 && q.relations.iter().any(|r| fielded.contains(r));
    if unread {
        reasons.push(json!({"code": "relation_unverified", "relations": fielded,
            "counts": {"fields": index.unread_fields},
            "detail": "binding fields too large to read were left out"}));
    }
    let unverified = index.unverified_links > 0 && q.relations.contains(&"prototype_target");
    if unverified {
        reasons.push(
            json!({"code": "relation_unverified", "relations": ["prototype_target"],
            "counts": {"links": index.unverified_links},
            "detail": "prototype links a component defines are not attributed to its instances"}),
        );
    }
    let complete = index.stop.is_none() && members_complete && !unverified && !unread;
    let mut layer = super::relations(if complete { "ready" } else { "partial" });
    if index.unverified_links > 0 {
        layer["kinds"]["prototype_target"] = json!("partial");
    }
    if index.unread_fields > 0 {
        for r in fielded {
            layer["kinds"][r] = json!("partial");
        }
    }
    if let Some(r) = layer["reasons"].as_array() {
        reasons.extend(r.iter().cloned());
    }
    let definition_json = json!({
        "ref": if req.resource { Json::Null } else {
            json!({"sourceId": source["sourceId"], "snapshotId": source["snapshotId"], "nodeId": target})
        },
        "resource": if req.resource { json!(target) } else { Json::Null },
        "state": state,
        "name": definition.as_ref().map(|d| &d.name),
        "type": definition.as_ref().map(|d| &d.kind),
        "setId": definition.as_ref().and_then(|d| d.set.as_ref()),
        "page": definition.as_ref().and_then(|d| d.page.as_ref()),
    });
    let mut out = json!({
        "definition": definition_json,
        "direction": if req.outgoing { "out" } else { "in" },
        "relations": q.relations,
        "scope": {"nodeId": req.scope},
        "results": results,
        "coverage": {"complete": complete, "layers": {"relations": layer}, "reasons": reasons},
        "hasMore": more,
        "nextCursor": more.then(|| last.map(|l| format!("u1.{fingerprint}.{snap}.{}.{l}", served.gen_id))).flatten(),
    });
    if kind == "COMPONENT_SET" && !req.outgoing {
        out["members"] = json!({"nodeIds": members, "complete": members_complete,
            "basis": "the set's members this index describes: every member in scope and each one an edge points at"});
    }
    Ok(out)
}

/// Relations whose target a cached library copy can name.
const LIBRARY_OF: [&str; 3] = ["instance_of", "uses_style", "binds_variable"];

/// One of a record's library references, parsed once: kind, namespace and key (no key when the
/// reference was too long to keep), and the copy or field it was read from.
struct Named {
    kind: Option<String>,
    namespace: Option<String>,
    key: Option<String>,
    copy: Json,
    field: Json,
}

fn named(refs: Vec<Edge>) -> Vec<Named> {
    refs.into_iter()
        .filter_map(|r| {
            let evidence: Json = serde_json::from_str(&r.evidence).ok()?;
            let (kind, namespace, key) = match r.target.as_deref() {
                Some(t) => {
                    let (k, ns, key): (String, Option<String>, String) =
                        serde_json::from_str(t).ok()?;
                    (Some(k), ns, Some(key))
                }
                None => (None, None, None),
            };
            Some(Named {
                kind,
                namespace,
                key,
                copy: evidence["copy"].clone(),
                field: evidence["field"].clone(),
            })
        })
        .collect()
}

/// The library reference `e` uses, from the record's own `uses_library` edge for the same copy
/// (a component) or field and key (a style or variable). A namespace no copy records, or one
/// two copies disagree on, stays null; a reference too long to keep names no key. Nothing is
/// inferred.
fn library_of(e: &Edge, refs: &[Named]) -> Option<Json> {
    let target = e.target.as_deref()?;
    let (kind, key) = match e.relation {
        "instance_of" => ("component", None),
        "uses_style" => ("style", Some(target.strip_prefix("StyleKey:")?)),
        "binds_variable" => (
            "variable",
            Some(target.strip_prefix("VariableID:")?.split_once('/')?.0),
        ),
        _ => return None,
    };
    let same = |r: &&Named| match key {
        None => r.copy == target,
        Some(_) => r.field == e.evidence.as_str(),
    };
    let found = refs
        .iter()
        .filter(same)
        .find(|r| r.kind.as_deref() == Some(kind) && (key.is_none() || r.key.as_deref() == key));
    let mut out = json!({"kind": kind, "namespace": null, "key": null, "basis": "cached_copy"});
    match found {
        Some(r) => {
            out["namespace"] = json!(r.namespace);
            out["key"] = json!(r.key);
            if r.namespace.is_none() {
                out["unknownReason"] = json!("namespace_unconfirmed");
            }
        }
        None if refs.iter().filter(same).any(|r| r.key.is_none()) => {
            out["unknownReason"] = json!("reference_not_kept");
        }
        None => return None,
    }
    Some(out)
}

fn edge_fields(out: &mut Json, e: &Edge) {
    out["relation"] = json!(e.relation);
    out["state"] = json!(e.state);
    out["basis"] = json!(e.basis);
    out["evidence"] = json!(e.evidence);
}

fn outgoing(e: &Edge, d: Option<&Definition>, source: &Json) -> Json {
    let resource = e.target.as_ref().is_some_and(|t| {
        ["VariableID:", "StyleKey:", "image:"]
            .iter()
            .any(|p| t.starts_with(p))
    });
    let mut out = json!({
        "target": match (&e.target, resource) {
            (None, _) => Json::Null,
            (Some(t), true) => json!({"resource": t}),
            (Some(t), false) => json!({"ref": {"sourceId": source["sourceId"],
                "snapshotId": source["snapshotId"], "nodeId": t}}),
        },
        "definition": d.map(|d| json!({"name": d.name, "type": d.kind, "setId": d.set, "page": d.page})),
    });
    edge_fields(&mut out, e);
    out
}

#[cfg(test)]
#[path = "usages_tests.rs"]
mod tests;
