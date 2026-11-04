//! Binary-private command semantics shared by the CLI and MCP adapters.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use figlens::expand::plugin_type;
use figlens::export::SvgIndex;
use figlens::rest::resolve_nodes;
use figlens::semantic::Mode;
use figlens::variables::variables;
use figlens::{Tree, normalize_id, to_json};
use serde_json::{Value, json};

use crate::USAGE;

mod discover;
pub mod freshness;
pub mod index;
#[cfg(test)]
mod instance_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tokens_tests;

pub enum Command {
    Tree {
        root: String,
        depth: usize,
        json: bool,
        top_frames: bool,
    },
    Node(String),
    /// The node, and an MCP reply budget that stops oversized JSON before it is built.
    Nodes(String, Option<usize>, bool, bool),
    Find {
        query: String,
        root: String,
        kind: Option<String>,
        limit: usize,
    },
    Inspect {
        node: String,
        depth: usize,
        images: bool,
        states: bool,
        compare_variant: Option<String>,
        compare: Option<String>,
        /// One line per node instead of JSON.
        flat: bool,
        full_text: bool,
        colors: bool,
    },
    Variables(Option<String>),
    Tokens {
        collection: String,
        mode: String,
        out: PathBuf,
        /// The mode to read each aliased collection in, by collection ID.
        alias_modes: BTreeMap<String, String>,
    },
    Bundle {
        node: String,
        out: PathBuf,
        file_key: Option<String>,
        name: Option<String>,
        states: bool,
        /// Record the source's sha256 in the manifest (`--hash-source`).
        hash_source: bool,
        /// The source's stamp read before the tree was decoded; see `bundle::write`.
        decoded: Option<figlens::search::Stamp>,
    },
    Export {
        node: String,
        out: PathBuf,
        format: figlens::export::Format,
        /// Export the layout box (`absoluteBoundingBox`) instead of the render bounds.
        layout: bool,
        scale: f64,
        quality: u8,
        max_kb: Option<usize>,
        /// Longest side in pixels; raster formats are scaled down to fit.
        max_size: Option<u32>,
        /// SVGs to report the files drawing the same or a similar shape.
        match_index: Option<Arc<SvgIndex>>,
        /// Layers inside the node to draw as hidden.
        hide: Vec<String>,
    },
}

pub enum Output {
    Text(String),
    /// Text for people and grep, with the same content as JSON for MCP structured output.
    Lines {
        text: String,
        value: Value,
    },
    Json {
        value: Value,
        pretty: bool,
    },
    Image {
        value: Value,
        bytes: Vec<u8>,
        mime: &'static str,
    },
}

impl Output {
    pub fn text(&self) -> Result<String, serde_json::Error> {
        match self {
            Self::Text(text) | Self::Lines { text, .. } => Ok(text.clone()),
            Self::Image { value, .. } => Ok(format!("{}\n", serde_json::to_string(value)?)),
            Self::Json { value, pretty } => Ok(format!(
                "{}\n",
                if *pretty {
                    serde_json::to_string_pretty(value)?
                } else {
                    serde_json::to_string(value)?
                }
            )),
        }
    }

    pub fn json(&self) -> Option<&Value> {
        match self {
            Self::Json { value, .. } | Self::Image { value, .. } | Self::Lines { value, .. } => {
                Some(value)
            }
            Self::Text(_) => None,
        }
    }
}

fn require_node(tree: &Tree, id: &str) -> Result<(), Box<dyn Error>> {
    if !tree.nodes.contains_key(id) {
        if id.starts_with('I') {
            resolve_nodes(tree, id)?;
            return Err(figlens::FigError::new(figlens::ErrorKind::UnsupportedNode, format!(
                "node {id} is an instance sublayer; this operation supports stored nodes only; use inspect, nodes, export or bundle with the full ID"
            )).into());
        }
        return Err(figlens::FigError::new(
            figlens::ErrorKind::NodeNotFound,
            format!("node {id} not found; list ids with `figlens tree <file.fig> --depth 2`"),
        )
        .into());
    }
    Ok(())
}

// Only a cheap root check during argument parsing; the shared resolver verifies the full ID.
fn require_target_root(tree: &Tree, id: &str) -> Result<(), Box<dyn Error>> {
    if !tree.nodes.contains_key(figlens::export::root_of(id)) {
        return Err(figlens::FigError::new(
            figlens::ErrorKind::NodeNotFound,
            format!("node {id} not found; list ids with `figlens tree <file.fig> --depth 2`"),
        )
        .into());
    }
    Ok(())
}

/// MCP `aliasModes`, already validated as a map of non-empty strings.
pub fn alias_modes(call: &crate::contract::PreparedCall) -> BTreeMap<String, String> {
    let modes = call
        .argument("aliasModes")
        .as_object()
        .into_iter()
        .flatten();
    modes
        .filter_map(|(c, m)| Some((c.clone(), m.as_str()?.to_owned())))
        .collect()
}

impl Command {
    /// Validate the opt-in export shape before source admission in MCP.
    pub fn variables(
        node: Option<String>,
        format: Option<&str>,
        collection: Option<String>,
        mode: Option<String>,
        out: Option<PathBuf>,
        alias_modes: BTreeMap<String, String>,
    ) -> Result<Self, Box<dyn Error>> {
        match format.unwrap_or("rest") {
            "rest"
                if collection.is_none()
                    && mode.is_none()
                    && out.is_none()
                    && alias_modes.is_empty() =>
            {
                Ok(Self::Variables(node))
            }
            "rest" => Err("collection, mode, out and alias modes require format dtcg".into()),
            "dtcg" if node.is_some() => {
                Err("DTCG export selects a collection and mode, not a node".into())
            }
            "dtcg" => Ok(Self::Tokens {
                collection: collection
                    .filter(|s| !s.is_empty())
                    .ok_or("DTCG export requires collection ID")?,
                mode: mode
                    .filter(|s| !s.is_empty())
                    .ok_or("DTCG export requires mode ID")?,
                out: out
                    .filter(|p| !p.as_os_str().is_empty())
                    .ok_or("DTCG export requires out")?,
                alias_modes,
            }),
            _ => Err("variables format must be rest or dtcg".into()),
        }
    }

    /// CLI parsing stays after document loading, including node checks before trailing options.
    pub fn from_cli(name: &str, args: &[String], tree: &Tree) -> Result<Self, Box<dyn Error>> {
        let node = || args.first().cloned().ok_or(USAGE);
        Ok(match name {
            "tree" => {
                let (mut root, mut depth) = ("0:0".to_owned(), usize::MAX);
                let (mut json, mut top_frames) = (false, false);
                let mut rest = args.iter();
                while let Some(arg) = rest.next() {
                    if arg == "--depth" {
                        depth = rest.next().ok_or("--depth needs a number")?.parse()?;
                    } else if arg == "--json" {
                        json = true;
                    } else if arg == "--top-frames" {
                        top_frames = true;
                    } else {
                        root = normalize_id(arg);
                    }
                }
                Self::Tree {
                    root,
                    depth,
                    json,
                    top_frames,
                }
            }
            "node" => Self::Node(node()?),
            "nodes" => {
                let (ids, compact, subtrees) = nodes_options(args)?;
                if ids.len() != 1 || ids[0].starts_with("--") {
                    return Err(
                        "nodes needs one node id and optional --compact [--subtrees]".into(),
                    );
                }
                Self::Nodes(ids[0].clone(), None, compact, subtrees)
            }
            "find" => {
                let query = node()?;
                let (mut root, mut kind, mut limit) = ("0:0".to_owned(), None, 20);
                let mut rest = args[1..].iter();
                while let Some(arg) = rest.next() {
                    let value = rest.next().ok_or(format!("{arg} needs a value"))?;
                    match arg.as_str() {
                        "--node" => root = normalize_id(value),
                        "--type" => kind = Some(value.to_uppercase()),
                        "--limit" => limit = value.parse()?,
                        _ => return Err(USAGE.into()),
                    }
                }
                Self::Find {
                    query,
                    root,
                    kind,
                    limit,
                }
            }
            "inspect" => {
                let node = node()?;
                let (mut depth, mut images, mut states, mut flat) = (1, false, false, false);
                let (mut full_text, mut colors) = (false, false);
                let (mut compare_variant, mut compare) = (None, None);
                let mut rest = args[1..].iter();
                while let Some(flag) = rest.next() {
                    match flag.as_str() {
                        "--depth" => {
                            depth = rest.next().ok_or("--depth needs a number")?.parse()?
                        }
                        "--images" => images = true,
                        "--states" => states = true,
                        "--flat" => flat = true,
                        "--full-text" => full_text = true,
                        "--colors" => colors = true,
                        "--compare-variant" => {
                            compare_variant = Some(
                                rest.next()
                                    .ok_or("--compare-variant needs a node ID")?
                                    .clone(),
                            )
                        }
                        "--compare" => {
                            compare = Some(rest.next().ok_or("--compare needs a node ID")?.clone())
                        }
                        _ => return Err(USAGE.into()),
                    }
                }
                Self::Inspect {
                    node,
                    depth,
                    images,
                    states,
                    compare_variant,
                    compare,
                    flat,
                    full_text,
                    colors,
                }
            }
            "variables" => {
                let (mut node, mut format, mut collection, mut mode, mut out) =
                    (None, None, None, None, None);
                let mut alias_modes = BTreeMap::new();
                let mut rest = args.iter();
                while let Some(arg) = rest.next() {
                    if arg.starts_with("--") {
                        let value = rest.next().ok_or(format!("{arg} needs a value"))?.clone();
                        match arg.as_str() {
                            "--format" => format = Some(value),
                            "--collection" => collection = Some(value),
                            "--mode" => mode = Some(value),
                            "--out" => out = Some(value),
                            "--alias-mode" => {
                                let (target, mode) = value
                                    .split_once('=')
                                    .filter(|(c, m)| !c.is_empty() && !m.is_empty())
                                    .ok_or("--alias-mode takes COLLECTION=MODE")?;
                                if alias_modes
                                    .insert(target.to_owned(), mode.to_owned())
                                    .is_some()
                                {
                                    return Err(format!("--alias-mode names {target} twice").into());
                                }
                            }
                            _ => return Err(USAGE.into()),
                        }
                    } else if node.is_none() {
                        node = Some(arg.clone());
                    } else {
                        return Err(USAGE.into());
                    }
                }
                Self::variables(
                    node,
                    format.as_deref(),
                    collection,
                    mode,
                    out.map(Into::into),
                    alias_modes,
                )?
            }
            "bundle" => {
                let node = node()?;
                let out = args.get(1).cloned().ok_or(USAGE)?;
                require_target_root(tree, &normalize_id(&node))?;
                let (mut file_key, mut name, mut states, mut hash_source) =
                    (None, None, false, false);
                let mut rest = args[2..].iter();
                while let Some(arg) = rest.next() {
                    let mut value = || rest.next().cloned().ok_or(format!("{arg} needs a value"));
                    match arg.as_str() {
                        "--file-key" => file_key = Some(value()?),
                        "--name" => name = Some(value()?),
                        "--states" => states = true,
                        "--hash-source" => hash_source = true,
                        _ => return Err(USAGE.into()),
                    }
                }
                Self::Bundle {
                    node,
                    out: out.into(),
                    file_key,
                    name,
                    states,
                    hash_source,
                    decoded: None,
                }
            }
            "export" => {
                let node = node()?;
                let out = args.get(1).cloned().ok_or(USAGE)?;
                let id = normalize_id(&node);
                require_target_root(tree, &id)?;
                let format = export_format(&out)?;
                let (layout, scale, quality, max_kb, max_size, match_dir, hide) =
                    export_options(&args[2..])?;
                Self::Export {
                    node,
                    out: out.into(),
                    format,
                    layout,
                    scale,
                    quality,
                    max_kb,
                    max_size,
                    match_index: match_index(match_dir, format)?,
                    hide,
                }
            }
            _ => return Err(USAGE.into()),
        })
    }

    /// Schema-shaped MCP arguments enter directly, without inventing CLI positionals or flags.
    pub fn from_tool(
        call: &crate::contract::PreparedCall,
        tree: &Tree,
        output: Option<&Path>,
        match_dir: Option<&Path>,
    ) -> Result<Self, Box<dyn Error>> {
        let string = |key: &str| call.string(key).map(str::to_owned);
        let node = || string("node").ok_or(USAGE);
        Ok(match call.name() {
            "tree" => Self::Tree {
                root: normalize_id(call.string("node").unwrap()),
                depth: call.argument("depth").as_u64().unwrap() as usize,
                json: call.argument("json") == true,
                top_frames: call.argument("topFrames") == true,
            },
            "nodes" => {
                let compact = call.argument("compact") == true;
                let subtrees = call.argument("subtrees") == true;
                if subtrees && !compact {
                    return Err("nodes subtrees requires compact: true".into());
                }
                Self::Nodes(
                    node()?,
                    Some(crate::mcp::MAX_RESPONSE_BYTES),
                    compact,
                    subtrees,
                )
            }
            "find" => Self::Find {
                query: string("query").ok_or(USAGE)?,
                root: normalize_id(call.string("node").unwrap()),
                kind: string("type").map(|s| s.to_uppercase()),
                limit: call.argument("limit").as_u64().unwrap() as usize,
            },
            "inspect" => Self::Inspect {
                node: node()?,
                depth: call.argument("depth").as_u64().unwrap() as usize,
                images: call.argument("images") == true,
                states: call.argument("states") == true,
                compare_variant: string("compareVariant"),
                compare: string("compare"),
                flat: call.argument("flat") == true,
                full_text: call.argument("fullText") == true,
                colors: call.argument("colors") == true,
            },
            "variables" => Self::variables(
                string("node"),
                call.string("format"),
                string("collection"),
                string("mode"),
                output.map(Path::to_owned),
                alias_modes(call),
            )?,
            "bundle" => {
                let node = node()?;
                require_target_root(tree, &normalize_id(&node))?;
                Self::Bundle {
                    node,
                    out: output.ok_or(USAGE)?.to_owned(),
                    file_key: string("fileKey"),
                    name: string("name"),
                    states: call.argument("states") == true,
                    hash_source: call.argument("hashSource") == true,
                    decoded: None,
                }
            }
            "export" => {
                let node = node()?;
                let out = output.ok_or(USAGE)?.to_owned();
                let id = normalize_id(&node);
                require_target_root(tree, &id)?;
                Self::Export {
                    node,
                    out,
                    // Encoding follows the requested extension, not a symlink target's name.
                    format: export_format(call.string("out").unwrap())?,
                    layout: call.string("bounds") == Some("layout"),
                    scale: call.argument("scale").as_f64().unwrap(),
                    quality: call.argument("quality").as_u64().unwrap() as u8,
                    max_kb: call.argument("maxKb").as_u64().map(|n| n as usize),
                    // Tool calls return the image inline, so cap it unless the agent says otherwise.
                    max_size: Some(call.argument("maxSize").as_u64().unwrap() as u32),
                    match_index: match_index(
                        match_dir.map(Path::to_owned),
                        export_format(call.string("out").unwrap())?,
                    )?,
                    hide: call
                        .string("hide")
                        .map(|ids| id_list("hide", ids))
                        .transpose()?
                        .unwrap_or_default(),
                }
            }
            _ => return Err(USAGE.into()),
        })
    }

    /// Sets a bundle's `decoded` stamp; other commands ignore it.
    pub fn decoded(mut self, stamp: Option<figlens::search::Stamp>) -> Self {
        if let Self::Bundle { decoded, .. } = &mut self {
            *decoded = stamp;
        }
        self
    }

    pub fn execute(self, tree: &Tree, fig: &Path) -> Result<Output, Box<dyn Error>> {
        let pretty = |value| Output::Json {
            value,
            pretty: true,
        };
        let compact = |value| Output::Json {
            value,
            pretty: false,
        };
        Ok(match self {
            Self::Tree {
                root,
                top_frames: true,
                json,
                ..
            } => {
                require_node(tree, &root)?;
                // A section root still reports its page; the walk only sees pages below it.
                let mut ancestors =
                    std::iter::successors(Some(root.as_str()), |id| parent(tree, id));
                let page = ancestors.find(|id| plugin_type(tree.nodes[*id]) == "CANVAS");
                let section =
                    (plugin_type(tree.nodes[&root]) == "SECTION").then_some(root.as_str());
                // Only what `top_frames` walks: the document, sections and pages it does not skip.
                figlens::expand::check_depth(&root, |id| {
                    let node = tree.nodes[id];
                    let walked = match plugin_type(node).as_str() {
                        "DOCUMENT" | "SECTION" => true,
                        "CANVAS" => {
                            id == root
                                || node.get("internalOnly").map(|v| v.as_bool()) != Some(true)
                        }
                        _ => false,
                    };
                    tree.children.get(id).filter(|_| walked)
                })?;
                let mut frames = Vec::new();
                top_frames(tree, &root, page, section, &mut frames);
                if json {
                    compact(Value::Array(
                        frames
                            .into_iter()
                            .map(|(id, page, section)| {
                                let mut frame = tree_json(tree, id, 0, 0, true);
                                if let Some(page) = page {
                                    frame["page"] = json!(page);
                                }
                                if let Some(section) = section {
                                    frame["section"] = json!(section);
                                }
                                frame
                            })
                            .collect(),
                    ))
                } else {
                    // The name is JSON-escaped so a newline in it cannot start another id line.
                    let mut out = String::new();
                    for (id, ..) in frames {
                        let (kind, name) = label(tree, id);
                        writeln!(out, "{id} {kind} {}", json!(name))?;
                    }
                    Output::Text(out)
                }
            }
            Self::Tree {
                root, depth, json, ..
            } => {
                require_node(tree, &root)?;
                // Listing recurses once per level; the default depth lists every level.
                if depth > figlens::expand::MAX_DEPTH {
                    figlens::expand::check_depth(&root, |id| tree.children.get(id))?;
                }
                if json {
                    // A root on a page or section keeps the firstText it has in its parent's listing.
                    let top = parent(tree, &root).is_some_and(|p| {
                        matches!(plugin_type(tree.nodes[p]).as_str(), "CANVAS" | "SECTION")
                    });
                    compact(tree_json(tree, &root, 0, depth, top))
                } else {
                    let mut out = String::new();
                    print_tree(&mut out, tree, &root, 0, depth)?;
                    Output::Text(out)
                }
            }
            Self::Node(node) => {
                let id = normalize_id(&node);
                require_node(tree, &id)?;
                pretty(to_json(tree.nodes[&id]))
            }
            Self::Nodes(node, budget, shared, subtrees) => {
                let id = normalize_id(&node);
                let value = if shared {
                    figlens::rest::resolve_compact_nodes(tree, &id, subtrees, budget)?
                } else {
                    match budget {
                        Some(bytes) => figlens::rest::resolve_nodes_within(tree, &id, bytes)?,
                        None => resolve_nodes(tree, &id)?,
                    }
                };
                if shared {
                    compact(value)
                } else {
                    pretty(value)
                }
            }
            Self::Find {
                query,
                root,
                kind,
                limit,
            } => {
                require_node(tree, &root)?;
                compact(discover::find(tree, &query, &root, kind.as_deref(), limit)?)
            }
            Self::Inspect {
                node,
                depth,
                images,
                states,
                compare_variant,
                compare,
                flat,
                full_text,
                colors,
            } => {
                let id = normalize_id(&node);
                if flat {
                    if images || states || compare_variant.is_some() || compare.is_some() {
                        return Err(
                            "--flat cannot be combined with --images, --states, --compare-variant or --compare"
                                .into(),
                        );
                    }
                    let (text, truncated) =
                        discover::inspect_flat(tree, &id, depth, full_text, colors)?;
                    return Ok(Output::Lines {
                        value: json!({ "flat": text, "truncated": truncated }),
                        text,
                    });
                }
                let compare_variant = compare_variant.as_deref().map(normalize_id);
                let compare = compare.as_deref().map(normalize_id);
                compact(discover::inspect(
                    tree,
                    &id,
                    depth,
                    fig,
                    images,
                    states,
                    compare_variant.as_deref(),
                    compare.as_deref(),
                    full_text,
                    colors,
                )?)
            }
            Self::Variables(node) => {
                let doc = match node {
                    Some(node) => {
                        let id = normalize_id(&node);
                        Some(resolve_nodes(tree, &id)?)
                    }
                    None => None,
                };
                pretty(variables(tree, doc.as_ref().map(|d| &d["document"])))
            }
            Self::Tokens {
                collection,
                mode,
                out,
                alias_modes,
            } => {
                // A library collection id carries `<key>/<version>`, a local one a guid. Presence in
                // the local snapshot cannot tell them apart: it also holds what local aliases reach.
                let local = !collection.contains('/');
                let snapshot = if local {
                    variables(tree, None)
                } else {
                    // From the library's copies cached in this file.
                    figlens::variables::in_collection(tree, &collection)
                };
                let mut tokens =
                    figlens::tokens::convert_mapped(&snapshot, &collection, &mode, &alias_modes)?;
                tokens["$extensions"]["org.figlens"]["source"] =
                    serde_json::to_value(fig.canonicalize()?)?;
                if !local {
                    tokens["$extensions"]["org.figlens"]["libraryCopy"] = json!(true);
                }
                figlens::output::write(fig, &out, |file| {
                    use std::io::{BufWriter, Write};
                    let mut writer = BufWriter::new(file);
                    serde_json::to_writer_pretty(&mut writer, &tokens)?;
                    writer.write_all(b"\n")?;
                    writer.flush()?;
                    Ok(())
                })?;
                let count = snapshot["variables"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|v| v["variableCollectionId"] == collection)
                    .count();
                compact(json!({
                    "path": out.to_string_lossy(), "collectionId": collection, "modeId": mode,
                    "tokenCount": count, "diagnostics": []
                }))
            }
            Self::Bundle {
                node,
                out,
                file_key,
                name,
                states,
                hash_source,
                decoded,
            } => {
                let id = normalize_id(&node);
                require_target_root(tree, &id)?;
                // A URL supplies the manifest key when --file-key is absent.
                let key = file_key.as_deref().or_else(|| {
                    node.split_once("/design/")
                        .or_else(|| node.split_once("/file/"))
                        .and_then(|(_, rest)| rest.split('/').next())
                });
                let warnings = figlens::bundle::write_with_states(
                    tree,
                    fig,
                    decoded.filter(|_| hash_source),
                    key,
                    name.as_deref(),
                    &id,
                    &out,
                    states,
                )?;
                compact(
                    json!({ "nodeId": id, "path": out.to_string_lossy(), "warnings": warnings }),
                )
            }
            Self::Export {
                node,
                out,
                format,
                layout,
                scale,
                quality,
                max_kb,
                max_size,
                match_index,
                hide,
            } => {
                let id = normalize_id(&node);
                require_target_root(tree, &id)?;
                let max_bytes = check_export(scale, quality, max_kb)?;
                let (image, report) = figlens::export::export(
                    tree,
                    fig,
                    &id,
                    format,
                    layout,
                    scale,
                    quality,
                    max_bytes,
                    max_size.filter(|&px| px > 0),
                    &hide,
                )?;
                figlens::output::write(fig, &out, |file| {
                    std::io::Write::write_all(file, &image.bytes)?;
                    Ok(())
                })?;
                let mut value = json!({ "path": out.to_string_lossy(), "width": image.width, "height": image.height,
                    "bytes": image.bytes.len(), "quality": image.quality });
                value.as_object_mut().unwrap().extend(report);
                if let Some(index) = match_index {
                    // The export itself may sit in the folder; it is not a match.
                    let own = std::fs::canonicalize(&out).ok();
                    let (matches, similar) = index.compare(&image.bytes, own.as_deref());
                    value["matches"] = json!(matches);
                    value["similar"] = json!(similar);
                }
                let mime = match format {
                    // Model image inputs commonly accept raster formats, not SVG attachments.
                    figlens::export::Format::Svg => return Ok(compact(value)),
                    figlens::export::Format::Png => "image/png",
                    figlens::export::Format::Jpg => "image/jpeg",
                    figlens::export::Format::Webp => "image/webp",
                    figlens::export::Format::Avif => "image/avif",
                };
                Output::Image {
                    value,
                    bytes: image.bytes,
                    mime,
                }
            }
        })
    }
}

/// Most items one batch export takes.
pub const MAX_EXPORT_ITEMS: usize = 64;
/// Written bytes after which a batch export stops starting items.
const MAX_BATCH_BYTES: usize = 256 << 20;

/// One validated batch item; `out` is the caller's label for the file.
pub struct ExportItem {
    node: String,
    out: String,
    format: figlens::export::Format,
    layout: bool,
    scale: f64,
    quality: u8,
    max_kb: Option<usize>,
    max_size: Option<u32>,
    hide: Vec<String>,
}

impl ExportItem {
    pub fn out(&self) -> &str {
        &self.out
    }
}

/// Checks a batch's items (MCP `items`, CLI `--batch` file) before any source is read or file
/// written; `default_max_size` applies to items that give no `maxSize`.
pub fn export_items(items: &Value, default_max_size: u32) -> Result<Vec<ExportItem>, String> {
    let items = items.as_array().ok_or("items must be an array")?;
    if items.is_empty() || items.len() > MAX_EXPORT_ITEMS {
        return Err(format!(
            "items takes 1-{MAX_EXPORT_ITEMS} entries, got {}",
            items.len()
        ));
    }
    let keys = crate::contract::EXPORT_ITEM_KEYS;
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let bad = |what: &str| format!("items[{i}]: {what}");
            let item = item.as_object().ok_or_else(|| bad("must be an object"))?;
            if let Some(key) = item.keys().find(|k| !keys.contains(&k.as_str())) {
                return Err(bad(&format!(
                    "unknown field {key}; accepted: {}",
                    keys.join(", ")
                )));
            }
            let text = |key: &str| {
                item.get(key)
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| bad(&format!("{key} must be a non-empty string")))
            };
            let number = |key: &str, max: u64| match item.get(key) {
                None => Ok(None),
                Some(v) => crate::contract::integer(v)
                    .filter(|&n| n <= max)
                    .map(Some)
                    .ok_or_else(|| bad(&format!("{key} must be an integer up to {max}"))),
            };
            let (node, out) = (text("node")?, text("out")?);
            let quality = number("quality", 100)?.unwrap_or(80);
            let scale = item.get("scale").map_or(Some(1.0), Value::as_f64);
            let scale = scale
                .filter(|s| s.is_finite() && *s > 0.0)
                .filter(|_| quality > 0)
                .ok_or_else(|| bad("quality takes 1-100 and scale a positive number"))?;
            let layout = match item.get("bounds").map(Value::as_str) {
                None | Some(Some("render")) => false,
                Some(Some("layout")) => true,
                _ => return Err(bad("bounds takes layout or render")),
            };
            let hide = match item.get("hide") {
                None => Vec::new(),
                Some(_) => id_list("hide", text("hide")?).map_err(|e| bad(&e.to_string()))?,
            };
            let max_kb = number("maxKb", (1 << 53) - 1)?.map(|n| n as usize);
            let max_size =
                number("maxSize", u32::MAX.into())?.map_or(default_max_size, |n| n as u32);
            check_export(scale, quality as u8, max_kb).map_err(|e| bad(&e.to_string()))?;
            Ok(ExportItem {
                node: node.to_owned(),
                out: out.to_owned(),
                format: export_format(out).map_err(|e| bad(&e.to_string()))?,
                layout,
                scale,
                quality: quality as u8,
                max_kb,
                max_size: Some(max_size),
                hide,
            })
        })
        .collect()
}

/// CLI `compare-image` arguments after the file: node, screenshot, diff path and options.
#[allow(clippy::type_complexity)]
pub fn compare_image_args(
    args: &[String],
) -> Result<
    (
        String,
        String,
        Option<String>,
        figlens::compare_image::Options,
    ),
    Box<dyn Error>,
> {
    let (mut positional, mut out) = (Vec::new(), None);
    let mut opts = figlens::compare_image::Options::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = || rest.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--scale" => {
                let s: f64 = value()?.parse()?;
                if !s.is_finite() || s <= 0.0 {
                    return Err("--scale must be a positive number".into());
                }
                opts.scale = Some(s);
            }
            "--limit" => {
                opts.limit = value()?.parse()?;
                if !(1..=50).contains(&opts.limit) {
                    return Err("--limit must be 1-50".into());
                }
            }
            "--threshold" => {
                let t: u32 = value()?.parse()?;
                if !(1..=254).contains(&t) {
                    return Err("--threshold must be 1-254".into());
                }
                opts.threshold = t as u8;
            }
            "--ignore" => opts.ignore.extend(id_list("ignore", value()?)?),
            "--out" => {
                let path = value()?.clone();
                if !path.to_ascii_lowercase().ends_with(".png") {
                    return Err("--out must end in .png".into());
                }
                out = Some(path);
            }
            "--full-page" => opts.full_page = true,
            _ if !arg.starts_with("--") => positional.push(arg.clone()),
            _ => return Err(USAGE.into()),
        }
    }
    let [node, shot] = <[String; 2]>::try_from(positional).map_err(|_| USAGE)?;
    Ok((node, shot, out, opts))
}

/// Compares a screenshot with a node for CLI and MCP; with `out`, also writes the diff PNG and
/// names it as `path`.
pub fn compare_image(
    tree: &Tree,
    fig: &Path,
    node: &str,
    shot: &Path,
    out: Option<&Path>,
    mut opts: figlens::compare_image::Options,
) -> Result<Value, Box<dyn Error>> {
    let id = normalize_id(node);
    require_target_root(tree, &id)?;
    if let Some(out) = out
        && std::fs::canonicalize(shot).ok() == figlens::output::destination(out).ok()
    {
        return Err(figlens::FigError::new(
            figlens::ErrorKind::InvalidArguments,
            "out must not overwrite the screenshot",
        )
        .into());
    }
    let shot = figlens::compare_image::decode(&figlens::compare_image::read(shot)?)?;
    opts.diff = out.is_some();
    let (mut report, diff) = figlens::compare_image::compare(tree, fig, &id, &shot, &opts)?;
    if let (Some(out), Some(diff)) = (out, diff) {
        let bytes = figlens::render::encode_png(&diff)?;
        figlens::output::write(fig, out, |file| {
            std::io::Write::write_all(file, &bytes)?;
            Ok(())
        })?;
        report["path"] = out.to_string_lossy().into();
    }
    Ok(report)
}

/// Exports each item to its checked path (`paths[i]` is where `items[i]` is written) and returns
/// the manifest. Two items writing one file fail the batch before the first write; an item's own
/// failure or panic is its entry, never the batch's.
pub fn export_batch(
    tree: &Tree,
    fig: &Path,
    items: &[ExportItem],
    paths: &[PathBuf],
) -> Result<Value, Box<dyn Error>> {
    let mut seen = std::collections::HashSet::new();
    // Compare where `output::write` would publish, so a symlink or `./` alias is a repeat too.
    if let Some(path) = paths
        .iter()
        .find(|p| !seen.insert(figlens::output::destination(p).unwrap_or_else(|_| (*p).clone())))
    {
        return Err(format!("two items write {}", path.display()).into());
    }
    let (mut written, mut failed) = (0, 0);
    let mut entries = Vec::new();
    for (item, path) in items.iter().zip(paths) {
        let command = Command::Export {
            node: item.node.clone(),
            out: path.clone(),
            format: item.format,
            layout: item.layout,
            scale: item.scale,
            quality: item.quality,
            max_kb: item.max_kb,
            max_size: item.max_size,
            match_index: None,
            hide: item.hide.clone(),
        };
        let result = if written >= MAX_BATCH_BYTES {
            Err(
                "batch output budget (256 MiB) reached; export this item in another call"
                    .to_owned(),
            )
        } else {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| command.execute(tree, fig)))
                .unwrap_or_else(|_| Err("figlens crashed on this node".into()))
                .map_err(|e| e.to_string())
                .map(|output| output.json().cloned().unwrap_or_default())
        };
        let mut entry = match result {
            Ok(entry) => {
                written += entry["bytes"].as_u64().unwrap_or(0) as usize;
                entry
            }
            Err(error) => {
                failed += 1;
                json!({ "error": error })
            }
        };
        entry["node"] = item.node.as_str().into();
        entry["path"] = item.out.as_str().into();
        entries.push(entry);
    }
    Ok(json!({ "count": entries.len(), "failed": failed, "items": entries }))
}

/// `--bounds`, `--scale`, `--quality`, `--max-kb`, `--max-size`, `--match-dir` and `--hide`, as
/// `(layout, scale, quality, max_kb, max_size, match_dir, hide)`.
pub fn export_options(args: &[String]) -> Result<ExportOptions, Box<dyn Error>> {
    let (mut layout, mut scale, mut quality, mut max_kb, mut max_size, mut match_dir) =
        (false, 1.0_f64, 80, None, None, None);
    let mut hide = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = || rest.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--bounds" => {
                layout = match value()?.as_str() {
                    "layout" => true,
                    "render" => false,
                    _ => return Err("--bounds takes layout or render".into()),
                }
            }
            "--scale" => scale = value()?.trim_end_matches('x').parse()?,
            "--quality" => quality = value()?.parse()?,
            "--max-kb" => max_kb = Some(value()?.parse::<usize>()?),
            "--max-size" => max_size = Some(value()?.parse::<u32>()?),
            "--match-dir" => match_dir = Some(PathBuf::from(value()?)),
            "--hide" => hide.extend(id_list("hide", value()?)?),
            _ => return Err(USAGE.into()),
        }
    }
    check_export(scale, quality, max_kb)?;
    Ok((layout, scale, quality, max_kb, max_size, match_dir, hide))
}

type ExportOptions = (
    bool,
    f64,
    u8,
    Option<usize>,
    Option<u32>,
    Option<PathBuf>,
    Vec<String>,
);

/// Comma-separated node ids for `what`, normalized as any node argument is. None is an error:
/// an export that hides nothing, or a comparison that ignores nothing, would read as done.
pub fn id_list(what: &str, ids: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let ids: Vec<String> = ids
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(normalize_id)
        .collect();
    if ids.is_empty() {
        return Err(format!("{what} takes one or more comma-separated node ids").into());
    }
    Ok(ids)
}

/// Rejects out-of-range options; returns `--max-kb` in bytes.
fn check_export(
    scale: f64,
    quality: u8,
    max_kb: Option<usize>,
) -> Result<Option<usize>, Box<dyn Error>> {
    if !(1..=100).contains(&quality) || !scale.is_finite() || scale <= 0.0 {
        return Err("--quality takes 1-100 and --scale a positive number".into());
    }
    Ok(max_kb
        .map(|kb| kb.checked_mul(1024).ok_or("--max-kb is too large"))
        .transpose()?)
}

fn search_mode(mode: &str) -> Result<Mode, Box<dyn Error>> {
    Mode::parse(mode).ok_or_else(|| {
        figlens::FigError::new(
            figlens::ErrorKind::InvalidArguments,
            format!("search mode takes text, semantic or hybrid, or visual with --image or --image-ref, not {mode:?}"),
        )
        .into()
    })
}

pub fn search_types(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(',')
        .map(|t| t.trim().to_uppercase())
        .filter(|t| !t.is_empty())
}

/// What a search ranks with: text in a mode, or a query image (with optional text).
pub enum Query {
    Text(Mode),
    Image(figlens::workspace::visual_search::ImageQuery),
}

/// The query image of a search, if any: a local path or a source reference, never both; a
/// crop needs one of them. `mode` and `text` must fit an image search.
fn image_query(
    path: Option<PathBuf>,
    reference: Option<figlens::workspace::visual_search::Input>,
    crop: Option<figlens::search::visual::Crop>,
    mode: Option<&str>,
    text: &str,
) -> Result<Option<figlens::workspace::visual_search::ImageQuery>, Box<dyn Error>> {
    use figlens::workspace::visual_search::{ImageQuery, Input};
    let invalid = |m: &str| -> Box<dyn Error> {
        figlens::FigError::new(figlens::ErrorKind::InvalidArguments, m).into()
    };
    let input = match (path, reference) {
        (Some(_), Some(_)) => {
            return Err(invalid(
                "give a query image path or an image reference, not both",
            ));
        }
        (Some(path), None) => Input::Path(path),
        (None, Some(reference)) => reference,
        (None, None) if crop.is_some() => {
            return Err(invalid(
                "crop applies to a query image; give --image or --image-ref",
            ));
        }
        (None, None) => return Ok(None),
    };
    Ok(Some(ImageQuery::new(input, crop, mode, text)?))
}

/// The request with its query: an image search validates as one (text optional), any other
/// in its text mode.
fn query(
    request: &figlens::search::Request,
    image: Option<figlens::workspace::visual_search::ImageQuery>,
    mode: Option<&str>,
) -> Result<Query, Box<dyn Error>> {
    Ok(match image {
        Some(image) => {
            request.validate_image()?;
            Query::Image(image)
        }
        None => {
            let mode = search_mode(mode.unwrap_or("text"))?;
            request.validate()?;
            Query::Text(mode)
        }
    })
}

/// CLI search arguments, checked before the source is downloaded or read. With `--image` or
/// `--image-ref` the query text is optional: an image search whose first argument is an option
/// has none.
pub fn search_request(
    args: &[String],
) -> Result<(figlens::search::Request, Query), Box<dyn Error>> {
    let image_search = args.iter().any(|a| a == "--image" || a == "--image-ref");
    let text = match args.first() {
        Some(first) if image_search && first.starts_with("--") => None,
        first => Some(first.cloned().ok_or(USAGE)?),
    };
    let skip = usize::from(text.is_some());
    search_options(text, &args[skip..])
}

/// `search_request` with the query text already separated (a workspace search's `--` operand).
pub fn search_options(
    text: Option<String>,
    options: &[String],
) -> Result<(figlens::search::Request, Query), Box<dyn Error>> {
    let (mut mode, mut path, mut reference, mut crop) = (None, None, None, None);
    let mut request = figlens::search::Request {
        query: text.clone().unwrap_or_default(),
        scope: "0:0".into(),
        types: Vec::new(),
        include_hidden: false,
        include_internal: false,
        limit: figlens::search::DEFAULT_LIMIT,
        cursor: None,
    };
    let mut rest = options.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--include-hidden" => request.include_hidden = true,
            "--include-internal" => request.include_internal = true,
            _ => {
                let value = rest.next().ok_or(format!("{arg} needs a value"))?;
                match arg.as_str() {
                    "--node" => request.scope = normalize_id(value),
                    "--type" => request.types.extend(search_types(value)),
                    "--limit" => request.limit = value.parse().unwrap_or(0),
                    "--cursor" => request.cursor = Some(value.clone()),
                    "--mode" => mode = Some(value.clone()),
                    "--image" => {
                        path = Some(
                            std::fs::canonicalize(value)
                                .map_err(|e| format!("--image {value}: {e}"))?,
                        )
                    }
                    "--image-ref" => {
                        let value = serde_json::from_str(value).map_err(|_| "--image-ref takes a JSON object {\"sourceId\", \"snapshotId\", \"nodeId\"}")?;
                        reference = Some(figlens::workspace::visual_search::reference(&value)?)
                    }
                    "--crop" => crop = Some(figlens::workspace::visual_search::crop(value)?),
                    _ => return Err(USAGE.into()),
                }
            }
        }
    }
    let image = image_query(path, reference, crop, mode.as_deref(), &request.query)?;
    let query = query(&request, image, mode.as_deref())?;
    Ok((request, query))
}

/// The same request from schema-checked MCP arguments; `image` is the admitted canonical path
/// of the `image` argument.
pub fn search_tool(
    call: &crate::contract::PreparedCall,
    image: Option<&Path>,
) -> Result<(figlens::search::Request, Query), Box<dyn Error>> {
    let request = figlens::search::Request {
        query: call.string("query").unwrap_or_default().to_owned(),
        scope: normalize_id(call.string("node").unwrap()),
        types: call
            .string("type")
            .map(search_types)
            .into_iter()
            .flatten()
            .collect(),
        include_hidden: call.argument("includeHidden") == true,
        include_internal: call.argument("includeInternal") == true,
        limit: call.argument("limit").as_u64().unwrap() as usize,
        cursor: call.string("cursor").map(str::to_owned),
    };
    let reference = match call.argument("imageRef") {
        serde_json::Value::Null => None,
        value => Some(figlens::workspace::visual_search::reference(value)?),
    };
    let crop = match call.argument("crop") {
        serde_json::Value::Null => None,
        value => Some(figlens::workspace::visual_search::crop_value(value)?),
    };
    let mode = call.string("mode");
    let image = image_query(
        image.map(Path::to_path_buf),
        reference,
        crop,
        mode,
        &request.query,
    )?;
    let query = query(&request, image, mode)?;
    Ok((request, query))
}

/// An image search reads a workspace's stored visual layers: a single `.fig` has none, and a
/// search never creates a workspace, so it names the steps that build one.
pub fn image_needs_workspace(file: &str, scope: &str) -> Box<dyn Error> {
    figlens::FigError::new(
        figlens::ErrorKind::InvalidArguments,
        "an image search reads a workspace's visual layer: prepare the .fig, build its visual layer, then search the workspace",
    )
    .with_recovery(serde_json::json!([
        {"action": "prepare", "requires": "none", "tool": "index",
            "arguments": {"action": "prepare", "file": file, "scope": scope}},
        {"action": "index_visual", "requires": "none", "tool": "index",
            "arguments": {"action": "visual", "file": file, "scope": scope}}]))
    .into()
}

/// CLI usages arguments after the source: the node or resource, then options; checked before
/// the source is read.
pub fn usages_request(args: &[String]) -> Result<figlens::search::usages::Request, Box<dyn Error>> {
    use figlens::search::usages;
    if args.iter().any(|a| a == "--library") {
        return Err("--library reads a workspace: figlens usages --workspace DIR|--file <file.fig>|--id ws_... --library KIND --namespace NS --key KEY".into());
    }
    let target = args.first().ok_or(USAGE)?;
    let resource = ["VariableID:", "StyleKey:", "image:"]
        .iter()
        .any(|p| target.starts_with(p));
    let mut request = usages::Request {
        target: if resource {
            target.clone()
        } else {
            normalize_id(target)
        },
        resource,
        scope: "0:0".into(),
        relations: Vec::new(),
        outgoing: false,
        snapshot: None,
        include_hidden: false,
        limit: usages::DEFAULT_LIMIT,
        cursor: None,
    };
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--include-hidden" => request.include_hidden = true,
            "--out" => request.outgoing = true,
            _ => {
                let value = rest.next().ok_or(format!("{arg} needs a value"))?;
                match arg.as_str() {
                    "--scope" => request.scope = normalize_id(value),
                    "--relation" => request
                        .relations
                        .extend(value.split(',').map(|r| r.trim().to_owned())),
                    "--snapshot" => request.snapshot = Some(value.clone()),
                    "--limit" => request.limit = value.parse().unwrap_or(0),
                    "--cursor" => request.cursor = Some(value.clone()),
                    _ => return Err(USAGE.into()),
                }
            }
        }
    }
    request.validate()?;
    Ok(request)
}

/// CLI `--library KIND [--namespace NS] --key KEY` and its options, after the workspace
/// selector; `--definition-source/-snapshot/-node` select a definition, all three or none.
pub fn library_request(
    args: &[String],
) -> Result<figlens::workspace::library::Request, Box<dyn Error>> {
    let mut request = figlens::workspace::library::Request {
        kind: String::new(),
        namespace: None,
        key: String::new(),
        definition: None,
        scope: "0:0".into(),
        include_hidden: false,
        limit: figlens::search::usages::DEFAULT_LIMIT,
        cursor: None,
    };
    let mut definition: [Option<String>; 3] = Default::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--include-hidden" => request.include_hidden = true,
            "--out" | "--relation" | "--snapshot" => {
                return Err(format!("{arg} does not apply to --library").into());
            }
            _ => {
                let value = rest.next().ok_or(format!("{arg} needs a value"))?;
                match arg.as_str() {
                    "--library" => request.kind = value.clone(),
                    "--namespace" => request.namespace = Some(value.clone()),
                    "--definition-source" => definition[0] = Some(value.clone()),
                    "--definition-snapshot" => definition[1] = Some(value.clone()),
                    "--definition-node" => definition[2] = Some(normalize_id(value)),
                    "--key" => request.key = value.clone(),
                    "--scope" => request.scope = normalize_id(value),
                    "--limit" => request.limit = value.parse().unwrap_or(0),
                    "--cursor" => request.cursor = Some(value.clone()),
                    _ => return Err(USAGE.into()),
                }
            }
        }
    }
    request.definition = match definition {
        [None, None, None] => None,
        [Some(source_id), Some(snapshot_id), Some(node_id)] => {
            Some(figlens::workspace::library::DefinitionRef {
                source_id,
                snapshot_id,
                node_id,
            })
        }
        _ => {
            return Err(
                "--definition-source, --definition-snapshot and --definition-node go together"
                    .into(),
            );
        }
    };
    request.validate()?;
    Ok(request)
}

/// MCP `usages` with `library`, whose shape and exclusions the contract already checked.
pub fn library_tool(
    call: &crate::contract::PreparedCall,
) -> Result<figlens::workspace::library::Request, Box<dyn Error>> {
    let library = call.argument("library");
    let field = |v: &serde_json::Value, key: &str| v[key].as_str().map(str::to_owned);
    let definition = &library["definition"];
    let request = figlens::workspace::library::Request {
        kind: field(library, "kind").unwrap(),
        namespace: field(library, "namespace"),
        key: field(library, "key").unwrap(),
        // The schema requires all three of a definition's fields.
        definition: definition
            .is_object()
            .then(|| figlens::workspace::library::DefinitionRef {
                source_id: field(definition, "sourceId").unwrap(),
                snapshot_id: field(definition, "snapshotId").unwrap(),
                node_id: normalize_id(&field(definition, "nodeId").unwrap()),
            }),
        // watch subscribe takes no limit and defaults its own scope.
        scope: normalize_id(call.string("scope").unwrap_or("0:0")),
        include_hidden: call.argument("includeHidden") == true,
        limit: call
            .argument("limit")
            .as_u64()
            .map_or(figlens::search::usages::DEFAULT_LIMIT, |l| l as usize),
        cursor: call.string("cursor").map(str::to_owned),
    };
    request.validate()?;
    Ok(request)
}

/// The same request from schema-checked MCP arguments.
pub fn usages_tool(
    call: &crate::contract::PreparedCall,
) -> Result<figlens::search::usages::Request, Box<dyn Error>> {
    let (target, resource) = match (call.string("node"), call.string("resource")) {
        (Some(node), None) => (normalize_id(node), false),
        (None, Some(resource)) => (resource.to_owned(), true),
        _ => return Err("usages needs node or resource, not both".into()),
    };
    let request = figlens::search::usages::Request {
        target,
        resource,
        scope: normalize_id(call.string("scope").unwrap()),
        relations: call
            .string("relation")
            .map(|r| r.split(',').map(|r| r.trim().to_owned()).collect())
            .unwrap_or_default(),
        outgoing: call.string("direction") == Some("out"),
        snapshot: call.string("snapshotId").map(str::to_owned),
        include_hidden: call.argument("includeHidden") == true,
        limit: call.argument("limit").as_u64().unwrap() as usize,
        cursor: call.string("cursor").map(str::to_owned),
    };
    request.validate()?;
    Ok(request)
}

/// CLI `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]`: one summary
/// line per changed top-level frame, or the reply with `--json`.
pub fn diff_cli(args: &[String]) -> Result<String, Box<dyn Error>> {
    use figlens::search::diff;
    let (Some(old), Some(new)) = (args.first(), args.get(1)) else {
        return Err(USAGE.into());
    };
    let mut request = diff::Request {
        node: "0:0".into(),
        limit: diff::LIMIT,
        cursor: None,
    };
    let mut json = false;
    let mut rest = args[2..].iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--node" => request.node = normalize_id(rest.next().ok_or("--node needs an id")?),
            "--limit" => request.limit = rest.next().ok_or("--limit needs a number")?.parse()?,
            "--cursor" => {
                request.cursor = Some(rest.next().ok_or("--cursor needs a value")?.to_owned())
            }
            "--json" => json = true,
            _ => return Err(USAGE.into()),
        }
    }
    // Each side is opened first, so a missing or unreadable file is named, not the other one.
    let local = |file: &String| {
        figlens::remote::local(file)
            .and_then(|path| Ok(std::fs::File::open(&path).map(|_| path)?))
            .map_err(|e| crate::contract::source(file, e))
    };
    let value = diff::diff(&local(old)?, &local(new)?, &request).map_err(|e| {
        if e.is::<figlens::FigError>() {
            e
        } else {
            crate::contract::source(old, e)
        }
    })?;
    if json {
        return Ok(format!("{value}\n"));
    }
    Ok(diff_text(&value))
}

/// The text form of a diff reply: one line per changed frame, then coverage and paging.
fn diff_text(value: &Value) -> String {
    let mut out = String::new();
    for f in value["frames"].as_array().into_iter().flatten() {
        let change = f["change"].as_str().unwrap_or("changed");
        out += &format!(
            "{} {:?} {change}: {}",
            f["id"].as_str().unwrap_or(""),
            f["name"].as_str().unwrap_or(""),
            f["summary"].as_str().unwrap_or("")
        );
        if let Some(m) = f["possibleMatch"].as_object() {
            out += &format!(
                " (possible match {} by {})",
                m["id"].as_str().unwrap_or(""),
                m["basis"].as_str().unwrap_or("")
            );
        }
        out.push('\n');
    }
    // A side that stopped at a limit was not read to the end, so finding nothing proves nothing.
    let stopped = value["coverage"]["reasons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["reason"].as_str().is_some_and(|r| r.ends_with("_limit")))
        .map(|r| r["side"].as_str().unwrap_or("comparison"))
        .collect::<Vec<_>>();
    if !stopped.is_empty() {
        out += &format!(
            "stopped early ({}): not everything was compared; narrow it with --node <frame or page>\n",
            stopped.join(", ")
        );
    } else if value["frames"].as_array().is_some_and(Vec::is_empty) && value["nextCursor"].is_null()
    {
        out += "no changes\n";
    }
    if value["coverage"]["complete"] != true {
        out += &format!("coverage partial: {}\n", value["coverage"]["reasons"]);
    }
    if let Some(c) = value["nextCursor"].as_str() {
        out += &format!("more: --cursor {c}\n");
    }
    out
}

/// MCP `diff`: `old` against `file`, both admitted local paths.
pub fn diff_tool(
    call: &crate::contract::PreparedCall,
) -> Result<figlens::search::diff::Request, Box<dyn Error>> {
    Ok(figlens::search::diff::Request {
        node: normalize_id(call.string("node").unwrap()),
        limit: call.argument("limit").as_u64().unwrap() as usize,
        cursor: call.string("cursor").map(str::to_owned),
    })
}

/// MCP `usages` with `workspaceId`: one source's prepared index, or with `library` every visible
/// source's; never a build.
pub fn workspace_usages_tool(
    call: &crate::contract::PreparedCall,
    roots: &[PathBuf],
    storage_roots: &[PathBuf],
) -> Result<Output, Box<dyn Error>> {
    let storage = tool_storage(call, roots, storage_roots)?;
    let select = figlens::workspace::Select::Id(call.string("workspaceId").unwrap().to_owned());
    let value = if call.argument("library").is_object() {
        figlens::workspace::library::library(
            &storage,
            &select,
            call.string("sourceId"),
            &library_tool(call)?,
        )?
    } else {
        figlens::workspace::usages::usages(
            &storage,
            &select,
            call.string("sourceId"),
            &usages_tool(call)?,
        )?
    };
    Ok(Output::Json {
        value,
        pretty: false,
    })
}

/// Stateless usages: the same cached index search reads, so nothing is written.
pub fn usages(
    path: &Path,
    request: &figlens::search::usages::Request,
    label: &str,
) -> Result<Output, Box<dyn Error>> {
    let value = figlens::search::usages(path, request).map_err(|e| {
        if e.is::<figlens::FigError>() {
            e
        } else {
            crate::contract::source(label, e)
        }
    })?;
    Ok(Output::Json {
        value,
        pretty: false,
    })
}

/// The storage an MCP index, jobs or workspace search call uses: its custom `storage` only
/// under the server's storage roots, and only sources under the design roots visible.
fn tool_storage(
    call: &crate::contract::PreparedCall,
    roots: &[PathBuf],
    storage_roots: &[PathBuf],
) -> Result<figlens::workspace::Storage, Box<dyn Error>> {
    Ok(
        figlens::workspace::storage(call.string("storage").map(Path::new))?
            .confined(roots)
            .allow(storage_roots),
    )
}

/// MCP `search` with `workspaceId`: the workspace's prepared indexes, never a build.
pub fn workspace_search_tool(
    call: &crate::contract::PreparedCall,
    roots: &[PathBuf],
    storage_roots: &[PathBuf],
    image: Option<&Path>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Output, Box<dyn Error>> {
    let storage = tool_storage(call, roots, storage_roots)?;
    let select = figlens::workspace::Select::Id(call.string("workspaceId").unwrap().to_owned());
    let only: Vec<String> = call
        .string("sourceId")
        .map(str::to_owned)
        .into_iter()
        .collect();
    let (request, query) = search_tool(call, image)?;
    // Text search keeps its old behavior: it is not one of the calls a server stop cancels.
    let value = match query {
        Query::Text(Mode::Text) => {
            figlens::workspace::query::search(&storage, &select, &only, &request)?
        }
        Query::Text(mode) => figlens::workspace::query::search_mode(
            &storage, &select, &only, &request, mode, cancel,
        )?,
        Query::Image(image) => figlens::workspace::query::search_image(
            &storage, &select, &only, &request, &image, cancel,
        )?,
    };
    Ok(Output::Json {
        value,
        pretty: false,
    })
}

/// MCP `models`. configure runs only the figlens-embed shipped beside figlens, never a
/// client-chosen program, on a model directory inside the server's roots.
pub fn models_tool(
    call: &crate::contract::PreparedCall,
    roots: &[PathBuf],
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Output, Box<dyn Error>> {
    use figlens::semantic::profile;
    let storage = figlens::workspace::storage(None)?;
    let needs = |key: &str| {
        call.string(key).ok_or_else(|| {
            figlens::FigError::new(
                figlens::ErrorKind::InvalidArguments,
                format!(
                    "models {} needs {key}",
                    call.string("action").unwrap_or_default()
                ),
            )
        })
    };
    let value = match call.string("action").unwrap() {
        "list" => profile::list(&storage)?,
        "status" => profile::status(&storage)?,
        "configure" => {
            let known = profile::known(needs("profile")?)?;
            let dir = std::fs::canonicalize(needs("modelDir")?)?;
            // Every pinned file, followed through links, must lie inside the roots too, before
            // any of them is read.
            let inside = |p: &Path| roots.is_empty() || roots.iter().any(|r| p.starts_with(r));
            if !inside(&dir) {
                return Err(figlens::FigError::new(
                    figlens::ErrorKind::PermissionDenied,
                    "modelDir lies outside the server's roots",
                )
                .into());
            }
            for (name, _) in known.files {
                // A file not there yet is reported missing by configure, before anything is read.
                let Ok(target) = std::fs::canonicalize(dir.join(name)) else {
                    continue;
                };
                if !inside(&target) {
                    return Err(figlens::FigError::new(
                        figlens::ErrorKind::PermissionDenied,
                        format!("modelDir or its {name} lies outside the server's roots"),
                    )
                    .into());
                }
            }
            profile::configure(&storage, known, &dir, None, cancel)?
        }
        _ => profile::remove(&storage, needs("profile")?)?,
    };
    Ok(Output::Json {
        value,
        pretty: false,
    })
}

/// MCP `jobs`: status or cancel of one job.
pub fn jobs_tool(
    call: &crate::contract::PreparedCall,
    roots: &[PathBuf],
    storage_roots: &[PathBuf],
) -> Result<Output, Box<dyn Error>> {
    use figlens::workspace::jobs;
    let storage = tool_storage(call, roots, storage_roots)?;
    let id = call.string("jobId").unwrap();
    let value = match call.string("action").unwrap() {
        "status" => jobs::status(&storage, id)?,
        _ => jobs::cancel(&storage, id)?,
    };
    Ok(Output::Json {
        value,
        pretty: false,
    })
}

/// MCP `changes` and `watch`: recorded revisions, saved-query evaluations, checkpoints and
/// subscriptions.
pub fn changes_tool(
    call: &crate::contract::PreparedCall,
    roots: &[PathBuf],
    storage_roots: &[PathBuf],
) -> Result<Output, Box<dyn Error>> {
    use figlens::workspace::changes;
    let storage = tool_storage(call, roots, storage_roots)?;
    let select = figlens::workspace::Select::Id(call.string("workspaceId").unwrap().to_owned());
    let request = changes::Request {
        source: call.string("sourceId"),
        scope: call.string("scope"),
        consumer: call.string("consumer"),
        cursor: call.string("cursor"),
        revision: call.string("revision"),
        limit: call.argument("limit").as_u64().map(|l| l as usize),
        subscription: call.string("subscriptionId"),
    };
    let subscribe = changes::subscriptions::Subscribe {
        consumer: request.consumer,
        source: request.source,
        scope: request.scope,
        node: call.string("node"),
        owner: call.string("owner"),
        query: call.string("query"),
        types: call
            .string("type")
            .map(search_types)
            .into_iter()
            .flatten()
            .collect(),
        sources: call.string("sources"),
        library: match call.argument("library").is_object() {
            true => Some(library_tool(call)?),
            false => None,
        },
    };
    use changes::subscriptions as subs;
    let needs = |name: &str| -> Result<&str, Box<dyn Error>> {
        call.string(name).ok_or_else(|| {
            format!("watch {} needs {name}", call.string("action").unwrap_or("")).into()
        })
    };
    use figlens::workspace::watch;
    let value = match (call.name(), call.string("action")) {
        ("watch", Some("start")) => watch::start(
            &storage,
            &select,
            call.string("sourceId"),
            needs("scope")?,
            call.string("requestId"),
        )?,
        ("watch", Some("stop")) => watch::stop(&storage, &select, needs("leaseId")?)?,
        ("watch", Some("status")) => watch::status(&storage, &select)?,
        ("watch", Some("subscribe")) => subs::subscribe(&storage, &select, &subscribe)?,
        ("watch", Some("evaluate")) if request.source.is_some() => {
            return Err(
                "watch evaluate takes no sourceId: it evaluates the subscription's own source"
                    .into(),
            );
        }
        ("watch", Some("evaluate")) => changes::evaluate(&storage, &select, request.subscription)?,
        ("watch", Some("list")) => subs::list(
            &storage,
            &select,
            request.consumer,
            call.string("after"),
            request.subscription,
        )?,
        ("watch", Some("unsubscribe")) => subs::unsubscribe(
            &storage,
            &select,
            request.subscription,
            request.consumer,
            subscribe.owner,
        )?,
        ("watch", Some("forget")) => subs::forget(
            &storage,
            &select,
            request.consumer,
            request.source,
            request.scope,
        )?,
        ("watch", _) => changes::ack(&storage, &select, &request)?,
        _ => changes::changes(&storage, &select, &request)?,
    };
    Ok(Output::Json {
        value,
        pretty: false,
    })
}

/// Jobs an index call submitted that this process owns and must run.
pub struct Runs {
    pub storage: figlens::workspace::Storage,
    pub workspace_id: String,
    pub jobs: Vec<String>,
    /// Model (embed and visual) jobs, run apart from text builds so none waits behind one.
    pub model: bool,
}

/// MCP `index`: one workspace action. `file` and `project` are admitted paths. Returns the
/// reply and the jobs to run in the background; the reply already names them as queued.
pub fn index_tool(
    call: &crate::contract::PreparedCall,
    file: Option<&Path>,
    project: Option<&Path>,
    roots: &[PathBuf],
    storage_roots: &[PathBuf],
) -> Result<(Output, Option<Runs>), Box<dyn Error>> {
    let storage = tool_storage(call, roots, storage_roots)?;
    index_on(call, file, project, storage.clone())
        .map_err(|e| figlens::workspace::rooted(&storage, e))
}

fn index_on(
    call: &crate::contract::PreparedCall,
    file: Option<&Path>,
    project: Option<&Path>,
    storage: figlens::workspace::Storage,
) -> Result<(Output, Option<Runs>), Box<dyn Error>> {
    use figlens::workspace::Select;
    let action = call.string("action").unwrap();
    let id = call.string("workspaceId");
    let scope = call.string("scope").map(normalize_id);
    // For these actions the admitted file is a source, not a workspace selector.
    let source_action = matches!(action, "prepare" | "add" | "relocate");
    let select = Select::from_choice(project, file.filter(|_| !source_action), id)?;
    let request = index::Request {
        action,
        select,
        file,
        project,
        source: call.string("sourceId"),
        scope: scope.as_deref(),
        request: call.string("requestId"),
        replace: call.argument("replaceUnavailable") == true,
        cursor: call.string("cursor"),
        ..Default::default()
    };
    // Preserve option validation before the prepare-only selector error.
    if request.applicable() && action == "prepare" && id.is_some() {
        return Err(figlens::FigError::new(
            figlens::ErrorKind::InvalidArguments,
            "index prepare needs file (and workspace for a project), not workspaceId",
        )
        .into());
    }
    let (value, outcome) = match request.run(&storage)? {
        index::Reply::Report(value) => (value, None),
        index::Reply::Jobs(done) => (done.report, Some(done.run)),
    };
    let runs = outcome.filter(|jobs| !jobs.is_empty()).map(|jobs| Runs {
        workspace_id: value["workspaceId"].as_str().unwrap_or_default().to_owned(),
        storage,
        jobs,
        model: matches!(action, "embed" | "visual"),
    });
    Ok((
        Output::Json {
            value,
            pretty: false,
        },
        runs,
    ))
}

/// Search reads and hashes its own snapshot of `path`, so it never needs a loaded tree. Only a
/// semantic or hybrid search reads the model profiles, so text search needs no config root.
pub fn search(
    path: &Path,
    request: &figlens::search::Request,
    mode: Mode,
    label: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Output, Box<dyn Error>> {
    let found = match mode {
        Mode::Text => figlens::search::search(path, request),
        _ => figlens::workspace::storage(None).and_then(|storage| {
            figlens::search::semantic::search(path, request, mode, &storage, cancel)
        }),
    };
    let value = found.map_err(|e| {
        if e.is::<figlens::FigError>() {
            e
        } else {
            crate::contract::source(label, e)
        }
    })?;
    Ok(Output::Json {
        value,
        pretty: false,
    })
}

/// Strip the shared nodes flags for both single-node and batch CLI paths.
pub fn nodes_options(args: &[String]) -> Result<(Vec<String>, bool, bool), Box<dyn Error>> {
    let compact = args.iter().any(|arg| arg == "--compact");
    let subtrees = args.iter().any(|arg| arg == "--subtrees");
    if subtrees && !compact {
        return Err("--subtrees requires --compact".into());
    }
    Ok((
        args.iter()
            .filter(|arg| !matches!(arg.as_str(), "--compact" | "--subtrees"))
            .cloned()
            .collect(),
        compact,
        subtrees,
    ))
}

/// The `--match-dir` folder parsed once, for SVG output only.
pub fn match_index(
    dir: Option<PathBuf>,
    format: figlens::export::Format,
) -> Result<Option<Arc<SvgIndex>>, Box<dyn Error>> {
    let Some(dir) = dir else {
        return Ok(None);
    };
    if format != figlens::export::Format::Svg {
        return Err("--match-dir needs an .svg output".into());
    }
    Ok(Some(Arc::new(SvgIndex::load(&dir)?)))
}

fn export_format(out: &str) -> Result<figlens::export::Format, Box<dyn Error>> {
    figlens::export::Format::of(Path::new(out))
        .ok_or_else(|| "the output must end in .png, .jpg, .webp, .avif or .svg".into())
}

fn print_tree(
    out: &mut String,
    tree: &Tree,
    id: &str,
    level: usize,
    depth: usize,
) -> std::fmt::Result {
    let (kind, name) = label(tree, id);
    writeln!(out, "{}{} {} \"{}\"", "  ".repeat(level), id, kind, name)?;
    if level < depth {
        for child in tree.children.get(id).into_iter().flatten() {
            print_tree(out, tree, child, level + 1, depth)?;
        }
    }
    Ok(())
}

/// `children` is only included where the requested depth reaches. `size` is the local
/// pre-rotation `[width, height]`, absent without one (document, pages); `visible` appears
/// only when false and `layoutMode` (`HORIZONTAL`, `VERTICAL` or `GRID`) only when set, as in `nodes`.
/// A `top` node (a child of a page or section) adds `firstText`, so callers can tell a canvas
/// label (a box around one line of text) from a screen without opening it.
fn tree_json(tree: &Tree, id: &str, level: usize, depth: usize, top: bool) -> Value {
    let (kind, name) = label(tree, id);
    let raw = tree.nodes[id];
    let mut node = json!({ "id": id, "type": kind, "name": name });
    if let Some(size) = discover::local_size(raw) {
        node["size"] = json!(size);
    }
    if matches!(raw.get("visible"), Some(kiwi_schema::Value::Bool(false))) {
        node["visible"] = json!(false);
    }
    if let Some(kiwi_schema::Value::Enum(_, mode)) = raw.get("stackMode")
        && *mode != "NONE"
    {
        node["layoutMode"] = json!(mode);
    }
    if let Some(text) = top.then(|| first_text(tree, id)).flatten() {
        node["firstText"] = json!(text);
    }
    if let Some(children) = tree.children.get(id).filter(|_| level < depth) {
        let top = matches!(kind.as_str(), "CANVAS" | "SECTION");
        node["children"] = children
            .iter()
            .map(|child| tree_json(tree, child, level + 1, depth, top))
            .collect();
    }
    node
}

/// The first visible, non-blank TEXT's characters under `id` (itself included), breadth-first over stored
/// nodes, at most 200 characters. Instances are not expanded, so their text is not seen.
fn first_text(tree: &Tree, id: &str) -> Option<String> {
    let hidden = |id: &str| {
        matches!(
            tree.nodes.get(id).and_then(|n| n.get("visible")),
            Some(kiwi_schema::Value::Bool(false))
        )
    };
    let mut queue = std::collections::VecDeque::from([id]);
    // A crafted file can make a node its own descendant; visit each id once.
    let mut seen = std::collections::HashSet::from([id]);
    while let Some(id) = queue.pop_front() {
        let Some(node) = tree.nodes.get(id).filter(|_| !hidden(id)) else {
            continue;
        };
        let text = node.get("textData").and_then(|t| t.get("characters"));
        // Blank text is no label; keep looking.
        let text = text.map(|t| t.as_string()).filter(|t| !t.trim().is_empty());
        if let Some(text) = text.filter(|_| plugin_type(node) == "TEXT") {
            return Some(text.chars().take(200).collect());
        }
        for child in tree.children.get(id).into_iter().flatten() {
            if seen.insert(child) {
                queue.push_back(child);
            }
        }
    }
    None
}

/// `tree --top-frames`: ids of the screens below `id`, with their page and innermost section.
/// Screens are frames, components, component sets and instances that are direct children of a
/// page or of a section on one, at any section depth, in tree order; groups, shapes and text are
/// not screens. Internal pages are skipped unless they are the root.
fn top_frames<'t>(
    tree: &'t Tree,
    id: &str,
    page: Option<&'t str>,
    section: Option<&'t str>,
    out: &mut Vec<(&'t str, Option<&'t str>, Option<&'t str>)>,
) {
    let kind = plugin_type(tree.nodes[id]);
    let holds_screens = matches!(kind.as_str(), "CANVAS" | "SECTION");
    for child in tree.children.get(id).into_iter().flatten() {
        let node = tree.nodes[child.as_str()];
        match plugin_type(node).as_str() {
            "CANVAS"
                if kind == "DOCUMENT"
                    && node.get("internalOnly").map(|v| v.as_bool()) != Some(true) =>
            {
                top_frames(tree, child, Some(child), None, out)
            }
            "SECTION" if holds_screens => top_frames(tree, child, page, Some(child), out),
            "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE" if holds_screens => {
                out.push((child, page, section))
            }
            _ => {}
        }
    }
}

/// The stored parent of `id`. `children` can name a parent the file lacks; that is no parent.
fn parent<'t>(tree: &'t Tree, id: &str) -> Option<&'t str> {
    let mut parents = tree.children.iter();
    parents
        .find(|(_, c)| c.iter().any(|c| c == id))
        .map(|(p, _)| p.as_str())
        .filter(|p| tree.nodes.contains_key(*p))
}

/// The REST type `nodes` and `find` use, so `tree` output can be filtered by the same names.
fn label<'a>(tree: &Tree<'a>, id: &str) -> (String, &'a str) {
    let node = tree.nodes[id];
    let name = node.get("name").map(|n| n.as_string()).unwrap_or("");
    (plugin_type(node), name)
}
