//! Normal, generated local-copy fixture: two copies of a component with a nested instance.
use super::*;
use crate::{call_tool, in_request_order, run, serve, test_support::TestDir};
use kiwi_schema::{
    Def, DefKind, Field, Schema, TYPE_BOOL, TYPE_FLOAT, TYPE_STRING, TYPE_UINT, Value as K,
};
use std::io::{Read, Write};

fn obj(name: &'static str, fields: Vec<(&'static str, K<'static>)>) -> K<'static> {
    K::Object(name, fields.into_iter().collect())
}

fn guid(id: u32) -> K<'static> {
    obj(
        "GUID",
        vec![("sessionID", K::UInt(0)), ("localID", K::UInt(id))],
    )
}

fn size(w: f32, h: f32) -> K<'static> {
    obj("Vector", vec![("x", K::Float(w)), ("y", K::Float(h))])
}

fn transform(x: f32, y: f32) -> K<'static> {
    obj(
        "Transform",
        vec![
            ("m00", K::Float(1.0)),
            ("m01", K::Float(0.0)),
            ("m02", K::Float(x)),
            ("m10", K::Float(0.0)),
            ("m11", K::Float(1.0)),
            ("m12", K::Float(y)),
        ],
    )
}

fn paint(hash: &str) -> K<'static> {
    K::Array(vec![obj(
        "Paint",
        vec![
            ("type", K::Enum("PaintType", "IMAGE")),
            (
                "image",
                obj("Image", vec![("hash", K::String(hash.into()))]),
            ),
        ],
    )])
}

fn text(s: &str) -> K<'static> {
    obj("TextData", vec![("characters", K::String(s.into()))])
}

struct Fixture {
    dir: TestDir,
    file: String,
}

impl Fixture {
    fn new() -> Self {
        Self::with_variants(false)
    }

    fn with_variants(variants: bool) -> Self {
        Self::build(variants, false)
    }

    fn build(variants: bool, prototype: bool) -> Self {
        let field = |name: &str, type_id, value, is_array| Field {
            name: name.into(),
            type_id,
            value,
            is_array,
        };
        let def = |name: &str, kind, fields| Def::new(name.into(), kind, fields);
        let schema = Schema::new(vec![
            def(
                "GUID",
                DefKind::Struct,
                vec![
                    field("sessionID", TYPE_UINT, 0, false),
                    field("localID", TYPE_UINT, 0, false),
                ],
            ),
            def(
                "NodeType",
                DefKind::Enum,
                [
                    "DOCUMENT",
                    "FRAME",
                    "SYMBOL",
                    "INSTANCE",
                    "RECTANGLE",
                    "TEXT",
                    "CANVAS",
                ]
                .into_iter()
                .enumerate()
                .map(|(i, n)| field(n, 0, i as u32, false))
                .collect(),
            ),
            def(
                "Vector",
                DefKind::Struct,
                vec![
                    field("x", TYPE_FLOAT, 0, false),
                    field("y", TYPE_FLOAT, 0, false),
                ],
            ),
            def(
                "ParentIndex",
                DefKind::Struct,
                vec![
                    field("guid", 0, 0, false),
                    field("position", TYPE_STRING, 0, false),
                ],
            ),
            def(
                "Transform",
                DefKind::Struct,
                ["m00", "m01", "m02", "m10", "m11", "m12"]
                    .into_iter()
                    .map(|n| field(n, TYPE_FLOAT, 0, false))
                    .collect(),
            ),
            def("Path", DefKind::Message, vec![field("guids", 0, 1, true)]),
            def(
                "Image",
                DefKind::Struct,
                vec![field("hash", TYPE_STRING, 0, false)],
            ),
            def(
                "PaintType",
                DefKind::Enum,
                vec![field("IMAGE", 0, 0, false)],
            ),
            def(
                "Paint",
                DefKind::Message,
                vec![field("type", 7, 1, false), field("image", 6, 2, false)],
            ),
            def(
                "TextData",
                DefKind::Message,
                vec![field("characters", TYPE_STRING, 1, false)],
            ),
            def(
                "Override",
                DefKind::Message,
                vec![
                    field("guidPath", 5, 1, false),
                    field("name", TYPE_STRING, 2, false),
                    field("size", 2, 3, false),
                    field("fillPaints", 8, 4, true),
                    field("textData", 9, 5, false),
                    field("prototypeInteractions", 21, 6, true),
                ],
            ),
            def(
                "SymbolData",
                DefKind::Message,
                vec![
                    field("symbolID", 0, 1, false),
                    field("symbolOverrides", 10, 2, true),
                ],
            ),
            def(
                "NodeChange",
                DefKind::Message,
                vec![
                    field("guid", 0, 1, false),
                    field("type", 1, 2, false),
                    field("name", TYPE_STRING, 3, false),
                    field("parentIndex", 3, 4, false),
                    field("size", 2, 5, false),
                    field("transform", 4, 6, false),
                    field("symbolData", 11, 7, false),
                    field("fillPaints", 8, 8, true),
                    field("textData", 9, 9, false),
                    field("overrideKey", 0, 10, false),
                    field("isStateGroup", TYPE_BOOL, 11, false),
                    field("componentPropDefs", 15, 12, true),
                    field("variantPropSpecs", 16, 13, true),
                    field("prototypeInteractions", 21, 14, true),
                ],
            ),
            def(
                "Message",
                DefKind::Message,
                vec![field("nodeChanges", 12, 1, true)],
            ),
            def(
                "PropertyType",
                DefKind::Enum,
                vec![field("VARIANT", 0, 0, false)],
            ),
            def(
                "PropertyDefinition",
                DefKind::Message,
                vec![
                    field("id", 0, 1, false),
                    field("name", TYPE_STRING, 2, false),
                    field("type", 14, 3, false),
                ],
            ),
            def(
                "VariantSpec",
                DefKind::Message,
                vec![
                    field("propDefId", 0, 1, false),
                    field("value", TYPE_STRING, 2, false),
                ],
            ),
            def(
                "EventType",
                DefKind::Enum,
                vec![
                    field("ON_CLICK", 0, 0, false),
                    field("ON_HOVER", 0, 1, false),
                    field("ON_PRESS", 0, 2, false),
                ],
            ),
            def(
                "ActionType",
                DefKind::Enum,
                vec![field("NAVIGATE", 0, 0, false), field("BACK", 0, 1, false)],
            ),
            def(
                "PrototypeEvent",
                DefKind::Message,
                vec![field("interactionType", 17, 1, false)],
            ),
            def(
                "PrototypeAction",
                DefKind::Message,
                vec![
                    field("navigationType", 18, 1, false),
                    field("connectionType", 18, 2, false),
                    field("transitionNodeID", 0, 3, false),
                    field("url", TYPE_STRING, 4, false),
                ],
            ),
            def(
                "PrototypeInteraction",
                DefKind::Message,
                vec![
                    field("event", 19, 1, false),
                    field("actions", 20, 2, true),
                    field("isDeleted", TYPE_BOOL, 3, false),
                    field("condition", TYPE_STRING, 4, false),
                ],
            ),
        ]);
        let node = |id, parent, kind, name: &str, extra: Vec<_>| {
            let mut fields = vec![
                ("guid", guid(id)),
                ("type", K::Enum("NodeType", kind)),
                ("name", K::String(name.into())),
            ];
            if let Some(p) = parent {
                fields.push((
                    "parentIndex",
                    obj(
                        "ParentIndex",
                        vec![
                            ("guid", guid(p)),
                            ("position", K::String(format!("{id:03}"))),
                        ],
                    ),
                ));
            }
            fields.extend(extra);
            obj("NodeChange", fields)
        };
        let patch = |ids: &[u32], values: Vec<_>| {
            let mut fields = vec![(
                "guidPath",
                obj(
                    "Path",
                    vec![("guids", K::Array(ids.iter().copied().map(guid).collect()))],
                ),
            )];
            fields.extend(values);
            obj("Override", fields)
        };
        let instance = |id, x, y, label: &str, hash: &str, w, h| {
            node(
                id,
                Some(1),
                "INSTANCE",
                label,
                vec![
                    ("size", size(40.0, 30.0)),
                    ("transform", transform(x, y)),
                    (
                        "symbolData",
                        obj(
                            "SymbolData",
                            vec![
                                ("symbolID", guid(10)),
                                (
                                    "symbolOverrides",
                                    K::Array(vec![
                                        patch(
                                            &[11],
                                            vec![
                                                ("name", K::String(label.into())),
                                                ("size", size(w, h)),
                                            ],
                                        ),
                                        patch(
                                            &[112],
                                            vec![("size", size(w, h)), ("fillPaints", paint(hash))],
                                        ),
                                        patch(&[13], vec![("textData", text(label))]),
                                        patch(&[14, 21], vec![("fillPaints", paint(hash))]),
                                    ]),
                                ),
                            ],
                        ),
                    ),
                ],
            )
        };
        let message = obj(
            "Message",
            vec![(
                "nodeChanges",
                K::Array(vec![
                    node(0, None, "DOCUMENT", "Document", vec![]),
                    node(30, Some(0), "CANVAS", "Empty page", vec![]),
                    node(
                        1,
                        Some(0),
                        "FRAME",
                        "Screen",
                        vec![("size", size(100.0, 100.0))],
                    ),
                    instance(2, 10.0, 20.0, "First", "blue", 8.0, 6.0),
                    instance(3, 50.0, 60.0, "Second", "green", 6.0, 4.0),
                    node(
                        10,
                        Some(if variants { 40 } else { 0 }),
                        "SYMBOL",
                        "Card",
                        if variants {
                            vec![
                                ("size", size(40.0, 30.0)),
                                (
                                    "variantPropSpecs",
                                    K::Array(vec![obj(
                                        "VariantSpec",
                                        vec![
                                            ("propDefId", guid(50)),
                                            ("value", K::String("Default".into())),
                                        ],
                                    )]),
                                ),
                            ]
                        } else {
                            vec![("size", size(40.0, 30.0))]
                        },
                    ),
                    node(
                        11,
                        Some(10),
                        "FRAME",
                        "Default card",
                        vec![("size", size(8.0, 6.0)), ("transform", transform(2.0, 3.0))],
                    ),
                    // A copied library layer uses overrideKey in expanded IDs, not its local guid.
                    node(
                        12,
                        Some(11),
                        "RECTANGLE",
                        "Artwork",
                        vec![
                            ("overrideKey", guid(112)),
                            ("size", size(8.0, 6.0)),
                            ("fillPaints", paint("red")),
                        ],
                    ),
                    node(
                        13,
                        Some(11),
                        "TEXT",
                        "Label",
                        vec![("textData", text("Default"))],
                    ),
                    node(
                        14,
                        Some(10),
                        "INSTANCE",
                        "Nested",
                        vec![
                            ("size", size(4.0, 3.0)),
                            ("transform", transform(20.0, 2.0)),
                            (
                                "symbolData",
                                obj("SymbolData", vec![("symbolID", guid(20))]),
                            ),
                        ],
                    ),
                    node(
                        20,
                        Some(0),
                        "SYMBOL",
                        "Badge",
                        vec![("size", size(4.0, 3.0))],
                    ),
                    node(
                        21,
                        Some(20),
                        "RECTANGLE",
                        "Badge artwork",
                        vec![("size", size(4.0, 3.0)), ("fillPaints", paint("red"))],
                    ),
                    // Draws past its unclipped frame: render bounds 15x15 over a 10x10 box.
                    node(
                        60,
                        Some(0),
                        "FRAME",
                        "Overflow",
                        vec![
                            ("size", size(10.0, 10.0)),
                            ("transform", transform(200.0, 0.0)),
                        ],
                    ),
                    node(
                        61,
                        Some(60),
                        "RECTANGLE",
                        "Overflowing artwork",
                        vec![
                            ("size", size(10.0, 10.0)),
                            ("transform", transform(5.0, 5.0)),
                            ("fillPaints", paint("red")),
                        ],
                    ),
                ]),
            )],
        );
        let mut message = message;
        if variants {
            let K::Object(_, fields) = &mut message else {
                unreachable!()
            };
            let Some(K::Array(nodes)) = fields.get_mut("nodeChanges") else {
                unreachable!()
            };
            nodes.extend([
                node(
                    40,
                    Some(0),
                    "FRAME",
                    "Card variants",
                    vec![
                        ("isStateGroup", K::Bool(true)),
                        (
                            "componentPropDefs",
                            K::Array(vec![obj(
                                "PropertyDefinition",
                                vec![
                                    ("id", guid(50)),
                                    ("name", K::String("State".into())),
                                    ("type", K::Enum("PropertyType", "VARIANT")),
                                ],
                            )]),
                        ),
                    ],
                ),
                node(
                    41,
                    Some(40),
                    "SYMBOL",
                    "Loading",
                    vec![(
                        "variantPropSpecs",
                        K::Array(vec![obj(
                            "VariantSpec",
                            vec![
                                ("propDefId", guid(50)),
                                ("value", K::String("Loading".into())),
                            ],
                        )]),
                    )],
                ),
                node(42, Some(0), "FRAME", "Loading", vec![]),
            ]);
        }
        if prototype {
            let event = |kind, actions| {
                obj(
                    "PrototypeInteraction",
                    vec![
                        (
                            "event",
                            obj(
                                "PrototypeEvent",
                                vec![("interactionType", K::Enum("EventType", kind))],
                            ),
                        ),
                        ("actions", K::Array(actions)),
                    ],
                )
            };
            let navigate = |target| {
                obj(
                    "PrototypeAction",
                    vec![
                        ("navigationType", K::Enum("ActionType", "NAVIGATE")),
                        ("transitionNodeID", guid(target)),
                    ],
                )
            };
            let K::Object(_, fields) = &mut message else {
                unreachable!()
            };
            let Some(K::Array(nodes)) = fields.get_mut("nodeChanges") else {
                unreachable!()
            };
            for node in nodes.iter_mut() {
                let K::Object(_, fields) = node else {
                    unreachable!()
                };
                let id = fields["guid"].get("localID").unwrap().as_uint();
                match id {
                    1 => {
                        fields.insert(
                            "prototypeInteractions",
                            K::Array(vec![
                                obj("PrototypeInteraction", vec![("isDeleted", K::Bool(true))]),
                                event(
                                    "ON_HOVER",
                                    vec![
                                        navigate(42),
                                        obj(
                                            "PrototypeAction",
                                            vec![
                                                ("connectionType", K::Enum("ActionType", "BACK")),
                                                (
                                                    "url",
                                                    K::String(
                                                        "https://example.invalid/do-not-open"
                                                            .into(),
                                                    ),
                                                ),
                                            ],
                                        ),
                                    ],
                                ),
                                obj(
                                    "PrototypeInteraction",
                                    vec![
                                        ("actions", K::Array(vec![navigate(999)])),
                                        (
                                            "condition",
                                            K::String("API failure must not be inferred".into()),
                                        ),
                                    ],
                                ),
                            ]),
                        );
                    }
                    2 => {
                        let Some(K::Object(_, symbol)) = fields.get_mut("symbolData") else {
                            unreachable!()
                        };
                        let Some(K::Array(overrides)) = symbol.get_mut("symbolOverrides") else {
                            unreachable!()
                        };
                        overrides.push(patch(
                            &[112],
                            vec![(
                                "prototypeInteractions",
                                K::Array(vec![event("ON_PRESS", vec![navigate(41)])]),
                            )],
                        ));
                        overrides.push(patch(&[14], vec![("overriddenSymbolID", guid(20))]));
                    }
                    10 | 12 | 20 => {
                        fields.insert(
                            "prototypeInteractions",
                            K::Array(vec![event("ON_CLICK", vec![navigate(41)])]),
                        );
                    }
                    41 => {
                        fields.insert(
                            "prototypeInteractions",
                            K::Array(vec![event("ON_HOVER", vec![])]),
                        );
                    }
                    _ => {}
                }
            }
            nodes.push(node(
                43,
                Some(0),
                "FRAME",
                "Bounded prototype",
                vec![
                    ("size", size(20.0, 20.0)),
                    (
                        "prototypeInteractions",
                        K::Array(
                            (0..32)
                                .map(|_| {
                                    event("ON_HOVER", (0..16).map(|_| navigate(999)).collect())
                                })
                                .collect(),
                        ),
                    ),
                ],
            ));
        }
        let dir = TestDir::new();
        let file = dir.join("instances.fig");
        let mut canvas = b"fig-kiwi".to_vec();
        canvas.extend(1u32.to_le_bytes());
        for bytes in [schema.encode(), message.encode(&schema)] {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&bytes).unwrap();
            let bytes = encoder.finish().unwrap();
            canvas.extend((bytes.len() as u32).to_le_bytes());
            canvas.extend(bytes);
        }
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&file).unwrap());
        zip.start_file("canvas.fig", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&canvas).unwrap();
        for (hash, color) in [
            ("red", [255, 0, 0, 255]),
            ("blue", [0, 0, 255, 255]),
            ("green", [0, 255, 0, 255]),
        ] {
            let mut pixel = resvg::tiny_skia::Pixmap::new(1, 1).unwrap();
            pixel.data_mut().copy_from_slice(&color);
            zip.start_file(
                format!("images/{hash}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(&pixel.encode_png().unwrap()).unwrap();
        }
        zip.finish().unwrap();
        Self {
            file: file.to_string_lossy().into_owned(),
            dir,
        }
    }

    fn cli(&self, name: &str, args: &[&str]) -> Result<String, Box<dyn Error>> {
        let mut argv = vec![name.into(), self.file.clone()];
        argv.extend(args.iter().map(|s| (*s).into()));
        let mut out = String::new();
        run(argv, &mut out)?;
        Ok(out)
    }

    fn tool(&self, name: &str, mut arguments: Value) -> Value {
        arguments["file"] = self.file.clone().into();
        call_tool(&json!({"name": name, "arguments": arguments}), None)
    }
}

fn zip_bytes(zip: &mut zip::ZipArchive<std::fs::File>, path: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    zip.by_name(path).unwrap().read_to_end(&mut bytes).unwrap();
    bytes
}

fn bundle_files(path: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let names: Vec<String> = zip
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    names
        .into_iter()
        .map(|name| {
            let bytes = zip_bytes(&mut zip, &name);
            (name, bytes)
        })
        .collect()
}

fn descendant<'a>(node: &'a Value, id: &str) -> &'a Value {
    if node["id"] == id {
        return node;
    }
    node["children"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|n| {
            let found = descendant(n, id);
            (!found.is_null()).then_some(found)
        })
        .unwrap_or(&Value::Null)
}

#[test]
fn inspect_states_cli_and_mcp_preserve_effective_overrides_and_optional_outputs() {
    let f = Fixture::with_variants(true);
    let read = |args: &[&str]| -> Value {
        serde_json::from_str(&f.cli("inspect", args).unwrap()).unwrap()
    };
    let ordinary = read(&["0:1", "--depth", "4"]);
    let mut with_states = read(&["0:1", "--depth", "4", "--states"]);
    let reply = f.tool("inspect", json!({"node":"0:1","depth":4,"states":true}));
    assert_eq!(reply["isError"], false, "{reply}");
    assert_eq!(reply["structuredContent"]["states"], with_states["states"]);
    assert_eq!(reply["structuredContent"]["node"], with_states["node"]);
    assert_eq!(
        reply["structuredContent"]["truncated"],
        with_states["truncated"]
    );
    let states = with_states
        .as_object_mut()
        .unwrap()
        .remove("states")
        .unwrap();
    assert_eq!(with_states, ordinary);
    assert_eq!(
        states["source"]["file"],
        std::fs::canonicalize(&f.file)
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(states["sets"].as_array().unwrap().len(), 1);
    assert_eq!(states["sets"][0]["variants"].as_array().unwrap().len(), 2);
    assert_eq!(
        states["sets"][0]["variants"][1]["properties"]["entries"][0]["value"],
        "Loading"
    );
    assert_eq!(
        states["instances"][0]["currentProperties"][0]["value"],
        "Default"
    );
    assert_eq!(states["instances"][1]["nodeId"], "I0:2;0:14");
    for variant in states["sets"][0]["variants"].as_array().unwrap() {
        let id = variant["nodeId"].as_str().unwrap();
        let resolved: Value = serde_json::from_str(&f.cli("nodes", &[id]).unwrap()).unwrap();
        assert_eq!(resolved["document"]["id"], id);
    }
    let mut combined = read(&["0:1", "--images", "--states"]);
    let reply = f.tool("inspect", json!({"node":"0:1","images":true,"states":true}));
    assert_eq!(reply["isError"], false, "{reply}");
    assert_eq!(reply["structuredContent"], combined);
    combined.as_object_mut().unwrap().remove("states");
    assert_eq!(combined, read(&["0:1", "--images"]));
    let other = Fixture::new();
    let absent = other.tool("inspect", json!({"node":"0:2","states":true}));
    assert_eq!(absent["structuredContent"]["states"]["sets"], json!([]));
    assert_eq!(
        absent["structuredContent"]["states"]["instances"][0]["state"],
        "no_component_set"
    );
    assert_ne!(
        absent["structuredContent"]["states"]["source"],
        states["source"]
    );
}

#[test]
fn bundle_states_preserve_existing_files_and_match_inspection_through_cli_and_mcp() {
    use sha2::{Digest, Sha256};
    let f = Fixture::with_variants(true);
    for id in ["0:1", "0:2", "I0:2;0:14"] {
        let plain = f.dir.join("plain.zip");
        let included = f.dir.join("states.zip");
        f.cli(
            "bundle",
            &[id, plain.to_str().unwrap(), "--file-key", "Key"],
        )
        .unwrap();
        f.cli(
            "bundle",
            &[
                id,
                included.to_str().unwrap(),
                "--states",
                "--file-key",
                "Key",
            ],
        )
        .unwrap();
        let plain = bundle_files(&plain);
        let files = bundle_files(&included);
        assert!(!plain.contains_key("design/states.json"));
        let legacy: Value = serde_json::from_slice(&plain["manifest.json"]).unwrap();
        assert!(legacy["root"].get("states").is_none());
        assert_eq!(files.len(), plain.len() + 1);
        for (path, bytes) in &plain {
            if path != "manifest.json" && path != "context.md" {
                assert_eq!(&files[path], bytes, "changed existing bundle entry {path}");
            }
        }
        let inspected = f.tool("inspect", json!({"node":id,"states":true}));
        let states: Value = serde_json::from_slice(&files["design/states.json"]).unwrap();
        assert_eq!(states, inspected["structuredContent"]["states"]);
        let manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        assert_eq!(manifest["version"], 1);
        assert_eq!(manifest["source"], legacy["source"]);
        assert_eq!(manifest["root"]["id"], id);
        assert_eq!(manifest["root"]["states"], "design/states.json");
        for entry in manifest["files"].as_array().unwrap() {
            let bytes = &files[entry["path"].as_str().unwrap()];
            assert_eq!(entry["size"], bytes.len());
            let hash: String = Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(entry["sha256"], hash);
        }
        let context = String::from_utf8(files["context.md"].clone()).unwrap();
        assert!(context.starts_with(std::str::from_utf8(&plain["context.md"]).unwrap()));
        assert!(context.len() - plain["context.md"].len() <= 8 * 1024);
        assert!(context.contains("supported snapshot fields"));
        assert!(context.contains("effective instance overrides remain"));
        if id == "I0:2;0:14" {
            assert_eq!(states["sets"], json!([]));
            assert!(context.contains("no_component_set"));
            assert!(context.contains("No related component sets were listed"));
        } else {
            assert!(context.contains("Variant `Loading` (`0:41`)"));
        }
        let mcp = f.dir.join("mcp-states.zip");
        let reply = f.tool(
            "bundle",
            json!({"node":id,"out":mcp,"states":true,"fileKey":"Key"}),
        );
        assert_eq!(reply["isError"], false, "{reply}");
        let mcp = bundle_files(&mcp);
        for (path, bytes) in files.iter().filter(|(path, _)| *path != "manifest.json") {
            assert_eq!(&mcp[path], bytes, "MCP entry {path}");
        }
    }
    let other = Fixture::new();
    let path = other.dir.join("no-variants.zip");
    other
        .cli("bundle", &["0:1", path.to_str().unwrap(), "--states"])
        .unwrap();
    let other = bundle_files(&path);
    let states: Value = serde_json::from_slice(&other["design/states.json"]).unwrap();
    assert_eq!(states["sets"], json!([]));
    assert_eq!(states["instances"][0]["state"], "no_component_set");
    assert_eq!(states["triggerEvidence"]["state"], "absent");
}

#[test]
fn batch_bundle_states_match_single_bundles_in_both_job_modes() {
    let f = Fixture::with_variants(true);
    let ids = ["0:1", "0:2", "I0:2;0:14"];
    let list = f.dir.join("frames-states.txt");
    std::fs::write(&list, ids.join("\n")).unwrap();
    for jobs in ["1", "2"] {
        let out = f.dir.join(&format!("states-jobs-{jobs}"));
        f.cli(
            "bundle",
            &[
                "--frames",
                list.to_str().unwrap(),
                "--states",
                "--out-dir",
                out.to_str().unwrap(),
                "--jobs",
                jobs,
                "--hash-source",
            ],
        )
        .unwrap();
        for id in ids {
            let single = f.dir.join("single-states.zip");
            let args = [id, single.to_str().unwrap(), "--states", "--hash-source"];
            f.cli("bundle", &args).unwrap();
            let batch = bundle_files(&out.join(format!("{}.zip", id.replace(':', "-"))));
            let single = bundle_files(&single);
            for (path, bytes) in &single {
                if path == "manifest.json" {
                    let mut a: Value = serde_json::from_slice(bytes).unwrap();
                    let mut b: Value = serde_json::from_slice(&batch[path]).unwrap();
                    assert!(a["source"]["sha256"].is_string());
                    a.as_object_mut().unwrap().remove("createdAt");
                    b.as_object_mut().unwrap().remove("createdAt");
                    assert_eq!(a, b);
                } else {
                    assert_eq!(bytes, &batch[path], "batch entry {path}");
                }
            }
            assert_eq!(single.len(), batch.len());
        }
    }
}

#[test]
fn prototype_evidence_reaches_cli_mcp_bundle_and_snapshot_with_exact_sources() {
    let f = Fixture::build(true, true);
    let inspected = f.tool("inspect", json!({"node":"0:1","states":true}));
    assert_eq!(inspected["isError"], false, "{inspected}");
    let states = &inspected["structuredContent"]["states"];
    assert_eq!(states["triggerEvidence"]["state"], "recorded");
    assert_eq!(
        states["triggerEvidence"]["businessConditions"],
        "not-inferred"
    );
    let evidence = states["triggerEvidence"]["nodes"].as_array().unwrap();
    let source = |id| evidence.iter().find(|n| n["nodeId"] == id).unwrap();
    assert_eq!(source("I0:2;0:112")["sourceNodeId"], "0:2");
    assert_eq!(
        source("I0:2;0:112")["sourceField"],
        "symbolData.symbolOverrides[4].prototypeInteractions"
    );
    assert_eq!(
        source("I0:2;0:112")["interactions"][0]["trigger"]["value"],
        "ON_PRESS"
    );
    assert_eq!(source("I0:3;0:112")["sourceNodeId"], "0:12");
    assert_eq!(
        source("I0:3;0:112")["interactions"][0]["trigger"]["value"],
        "ON_CLICK"
    );
    assert_eq!(source("0:2")["basis"], "component-definition");
    assert_eq!(source("0:2")["inheritanceState"], "unsupported");
    for id in ["I0:2;0:14", "I0:3;0:14"] {
        assert_eq!(source(id)["basis"], "component-definition");
        assert_eq!(source(id)["sourceNodeId"], "0:20");
        assert_eq!(source(id)["inheritanceState"], "unsupported");
    }
    assert_eq!(source("0:1")["interactions"][0]["index"], 1);
    assert_eq!(
        source("0:1")["interactions"][0]["actions"][0]["destination"]["nodeId"],
        "0:42"
    );
    assert_eq!(
        source("0:1")["interactions"][1]["trigger"]["state"],
        "absent"
    );
    assert!(
        source("0:1")["interactions"][1]["trigger"]
            .get("value")
            .is_none()
    );
    assert_eq!(
        source("0:1")["interactions"][1]["actions"][0]["destination"]["nodeId"],
        "0:999"
    );
    assert_eq!(
        source("0:1")["interactions"][1]["actions"][0]["destination"]["state"],
        "unresolved"
    );
    assert!(
        !states
            .to_string()
            .contains("API failure must not be inferred")
    );
    assert!(
        !states
            .to_string()
            .contains("https://example.invalid/do-not-open")
    );
    assert!(
        !states["sets"][0]["variants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["nodeId"] == "0:42")
    );
    let raw: Value = serde_json::from_str(&f.cli("node", &["0:2"]).unwrap()).unwrap();
    assert_eq!(
        raw["symbolData"]["symbolOverrides"][4]["prototypeInteractions"][0]["event"]["interactionType"],
        source("I0:2;0:112")["interactions"][0]["trigger"]["value"]
    );
    let cli: Value =
        serde_json::from_str(&f.cli("inspect", &["0:1", "--states"]).unwrap()).unwrap();
    assert_eq!(&cli["states"], states);
    let bundle = f.dir.join("prototype.zip");
    let reply = f.tool("bundle", json!({"node":"0:1","states":true,"out":bundle}));
    assert_eq!(reply["isError"], false, "{reply}");
    let files = bundle_files(&bundle);
    let bundled: Value = serde_json::from_slice(&files["design/states.json"]).unwrap();
    assert_eq!(&bundled, states);
    let nodes: Value = serde_json::from_slice(&files["design/nodes.json"]).unwrap();
    assert_eq!(
        descendant(&nodes["document"], "I0:2;0:112")["_figlens"]["prototype"]["sourceNodeId"],
        "0:2"
    );
    let context = std::str::from_utf8(&files["context.md"]).unwrap();
    assert!(context.contains("`ON_PRESS`"));
    assert!(context.contains("`0:999` (`unresolved`)"));
    assert!(context.contains("`symbolData.symbolOverrides[4].prototypeInteractions`"));
    assert!(!context.contains("API failure must not be inferred"));
    let html = f.dir.join("prototype.html");
    f.cli("preview", &["0:1", "--out", html.to_str().unwrap()])
        .unwrap();
    let html = std::fs::read_to_string(html).unwrap();
    let boot = html
        .split("<script id=\"boot\" type=\"application/json\">")
        .nth(1)
        .unwrap()
        .split("</script>")
        .next()
        .unwrap();
    let boot: Value = serde_json::from_str(boot).unwrap();
    assert_eq!(
        boot["snapshot"]["interactions"]["I0:2;0:112"],
        descendant(&nodes["document"], "I0:2;0:112")["_figlens"]["prototype"]
    );
    for id in ["I0:2;0:14", "I0:3;0:14"] {
        assert_eq!(
            boot["snapshot"]["interactions"][id]["inheritanceState"],
            "unsupported"
        );
        assert_eq!(
            boot["snapshot"]["interactions"][id],
            descendant(&nodes["document"], id)["_figlens"]["prototype"]
        );
    }
    assert_eq!(
        boot["snapshot"]["interactions"]["0:1"]["interactions"][1]["trigger"]["state"],
        "absent"
    );
    let plain = Fixture::with_variants(true);
    let path = plain.dir.join("without-prototype.zip");
    plain
        .cli("bundle", &["0:1", path.to_str().unwrap()])
        .unwrap();
    let plain = bundle_files(&path);
    for (path, bytes) in plain.iter().filter(|(path, _)| {
        path.starts_with("assets/")
            || path.starts_with("references/")
            || *path == "assets.json"
            || *path == "missing-assets.json"
    }) {
        assert_eq!(
            &files[path], bytes,
            "prototype changed asset/reference {path}"
        );
    }
}

#[test]
fn truncated_prototype_actions_match_cli_mcp_bundle_and_snapshot() {
    let f = Fixture::build(true, true);
    let cli: Value =
        serde_json::from_str(&f.cli("inspect", &["0:43", "--states"]).unwrap()).unwrap();
    let mcp = f.tool("inspect", json!({"node":"0:43", "states":true}));
    assert_eq!(mcp["isError"], false, "{mcp}");
    assert_eq!(mcp["structuredContent"], cli);
    let evidence = &cli["states"]["triggerEvidence"]["nodes"][0];
    assert_eq!(evidence["truncated"], true);
    assert!(
        evidence["interactions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["actionsTruncated"] == true && i["actions"].as_array().unwrap().is_empty())
    );
    let raw: Value = serde_json::from_str(&f.cli("node", &["0:43"]).unwrap()).unwrap();
    for interaction in evidence["interactions"].as_array().unwrap() {
        let index = interaction["index"].as_u64().unwrap() as usize;
        assert_eq!(
            raw["prototypeInteractions"][index]["actions"]
                .as_array()
                .unwrap()
                .len(),
            16
        );
    }
    let cli_nodes: Value = serde_json::from_str(&f.cli("nodes", &["0:43"]).unwrap()).unwrap();
    let mcp_nodes = f.tool("nodes", json!({"node":"0:43"}));
    assert_eq!(mcp_nodes["isError"], false, "{mcp_nodes}");
    assert_eq!(
        serde_json::from_str::<Value>(mcp_nodes["content"][0]["text"].as_str().unwrap()).unwrap(),
        cli_nodes
    );
    let mut projected = evidence.clone();
    projected.as_object_mut().unwrap().remove("nodeId");
    assert_eq!(cli_nodes["document"]["_figlens"]["prototype"], projected);
    for mcp in [false, true] {
        let bundle = f.dir.join("truncated-prototype.zip");
        if mcp {
            let reply = f.tool("bundle", json!({"node":"0:43","states":true,"out":bundle}));
            assert_eq!(reply["isError"], false, "{reply}");
        } else {
            f.cli("bundle", &["0:43", bundle.to_str().unwrap(), "--states"])
                .unwrap();
        }
        let files = bundle_files(&bundle);
        let states: Value = serde_json::from_slice(&files["design/states.json"]).unwrap();
        let nodes: Value = serde_json::from_slice(&files["design/nodes.json"]).unwrap();
        assert_eq!(states, cli["states"]);
        assert_eq!(nodes, cli_nodes);
        assert!(
            std::str::from_utf8(&files["context.md"])
                .unwrap()
                .contains("truncated: true")
        );
    }
    let html = f.dir.join("truncated-prototype.html");
    f.cli("preview", &["0:43", "--out", html.to_str().unwrap()])
        .unwrap();
    let html = std::fs::read_to_string(html).unwrap();
    let boot: Value = serde_json::from_str(
        html.split("<script id=\"boot\" type=\"application/json\">")
            .nth(1)
            .unwrap()
            .split("</script>")
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(boot["snapshot"]["interactions"]["0:43"], projected);
}

#[test]
fn variant_comparison_cli_and_mcp_match_without_changing_existing_outputs() {
    let f = Fixture::build(true, true);
    let normal: Value =
        serde_json::from_str(&f.cli("inspect", &["0:10", "--states", "--images"]).unwrap())
            .unwrap();
    assert!(normal.get("variantComparison").is_none());
    let cli: Value = serde_json::from_str(
        &f.cli(
            "inspect",
            &["0:10", "--states", "--images", "--compare-variant", "0-41"],
        )
        .unwrap(),
    )
    .unwrap();
    let mcp=f.tool("inspect",json!({"node":"0:10","states":true,"images":true,"compareVariant":"https://www.figma.com/design/Key/Test?node-id=0-41"}));
    assert_eq!(mcp["isError"], false, "{mcp}");
    assert_eq!(mcp["structuredContent"], cli);
    assert_eq!(cli["variantComparison"]["leftNodeId"], "0:10");
    assert_eq!(cli["variantComparison"]["rightNodeId"], "0:41");
    assert_eq!(cli["variantComparison"]["componentSetId"], "0:40");
    let mut original = cli.clone();
    original
        .as_object_mut()
        .unwrap()
        .remove("variantComparison");
    assert_eq!(original, normal);
    let shallow: Value = serde_json::from_str(
        &f.cli(
            "inspect",
            &["0:10", "--depth", "0", "--compare-variant", "0:41"],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(shallow["variantComparison"], cli["variantComparison"]);
    for id in ["0:2", "0:42", "0:20", "0:10", "9:9", "I0:2;0:14"] {
        assert!(
            f.cli("inspect", &["0:10", "--compare-variant", id])
                .is_err(),
            "{id}"
        );
        let reply = f.tool("inspect", json!({"node":"0:10","compareVariant":id}));
        assert_eq!(reply["isError"], true, "{id}: {reply}");
    }
    assert!(f.cli("inspect", &["0:10", "--compare-variant"]).is_err());
}

#[test]
fn compare_takes_any_two_nodes_and_matches_across_cli_and_mcp() {
    let f = Fixture::new();
    let cli: Value =
        serde_json::from_str(&f.cli("inspect", &["0:10", "--compare", "0-2"]).unwrap()).unwrap();
    let mcp = f.tool("inspect", json!({"node":"0:10","compare":"0:2"}));
    assert_eq!(mcp["isError"], false, "{mcp}");
    assert_eq!(mcp["structuredContent"], cli);
    assert_eq!(cli["comparison"]["leftNodeId"], "0:10");
    assert_eq!(cli["comparison"]["rightNodeId"], "0:2");
    assert!(cli.get("variantComparison").is_none());
    for args in [
        vec!["0:10", "--compare", "0:10"],
        vec!["0:10", "--compare", "9:9"],
        vec!["0:10", "--compare", "0:2", "--compare-variant", "0:41"],
        vec!["0:10", "--compare", "0:2", "--flat"],
        vec!["0:10", "--compare"],
    ] {
        assert!(f.cli("inspect", &args).is_err(), "{args:?}");
    }
}

#[test]
fn emitted_instance_ids_select_the_same_structure_assets_and_pixels() {
    let f = Fixture::new();
    let found = f.tool("find", json!({"query": "Screen"}));
    assert_eq!(found["isError"], false, "{found}");
    let screen = found["structuredContent"]["results"][0]["id"]
        .as_str()
        .unwrap();
    let discovery = f.tool("inspect", json!({"node": screen, "depth": 4}));
    assert_eq!(discovery["isError"], false, "{discovery}");
    let expanded = &discovery["structuredContent"]["node"];
    for (id, name, hash, w, h, x, y, rgba) in [
        ("I0:2;0:11", "First", "blue", 8, 6, 12, 23, [0, 0, 255, 255]),
        (
            "I0:3;0:11",
            "Second",
            "green",
            6,
            4,
            52,
            63,
            [0, 255, 0, 255],
        ),
        (
            "I0:2;0:112",
            "Artwork",
            "blue",
            8,
            6,
            12,
            23,
            [0, 0, 255, 255],
        ),
        (
            "I0:3;0:112",
            "Artwork",
            "green",
            6,
            4,
            52,
            63,
            [0, 255, 0, 255],
        ),
        (
            "I0:2;0:14;0:21",
            "Badge artwork",
            "blue",
            4,
            3,
            30,
            22,
            [0, 0, 255, 255],
        ),
        (
            "I0:3;0:14;0:21",
            "Badge artwork",
            "green",
            4,
            3,
            70,
            62,
            [0, 255, 0, 255],
        ),
    ] {
        // Feed back the actual tool-emitted ID, never a raw component child as a fallback.
        let emitted = descendant(expanded, id)["id"].as_str().unwrap();
        let png = f.dir.join("export.png");
        let bundle = f.dir.join("bundle.zip");
        f.cli("export", &[emitted, png.to_str().unwrap()]).unwrap();
        let expected_pixels =
            resvg::tiny_skia::Pixmap::decode_png(&std::fs::read(&png).unwrap()).unwrap();
        assert_eq!((expected_pixels.width(), expected_pixels.height()), (w, h));
        assert!(
            expected_pixels.data().chunks_exact(4).all(|p| p == rgba),
            "{id}"
        );
        for mcp in [false, true] {
            if mcp {
                let reply = f.tool("bundle", json!({"node": emitted, "out": bundle}));
                assert_eq!(reply["isError"], false, "{reply}");
                assert_eq!(reply["structuredContent"]["nodeId"], emitted);
            } else {
                f.cli("bundle", &[emitted, bundle.to_str().unwrap()])
                    .unwrap();
            }
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&bundle).unwrap()).unwrap();
            let nodes: Value =
                serde_json::from_slice(&zip_bytes(&mut zip, "design/nodes.json")).unwrap();
            let n = &nodes["document"];
            assert_eq!(n["id"], emitted);
            assert_eq!(n["name"], name);
            assert_eq!(
                n["absoluteBoundingBox"],
                json!({"x": f64::from(x), "y": f64::from(y), "width": f64::from(w), "height": f64::from(h)})
            );
            assert!(!nodes.to_string().contains("\"_r\""));
            if id.ends_with(";0:11") {
                assert_eq!(n["children"][1]["characters"], name);
            }
            let reference =
                resvg::tiny_skia::Pixmap::decode_png(&zip_bytes(&mut zip, "references/screen.png"))
                    .unwrap();
            assert_eq!(reference, expected_pixels);
            let assets: Value =
                serde_json::from_slice(&zip_bytes(&mut zip, "assets.json")).unwrap();
            assert_eq!(assets.as_array().unwrap().len(), 1);
            assert_eq!(assets[0]["key"], hash);
            let asset = resvg::tiny_skia::Pixmap::decode_png(&zip_bytes(
                &mut zip,
                assets[0]["path"].as_str().unwrap(),
            ))
            .unwrap();
            assert_eq!(asset.data(), rgba);
            let manifest: Value =
                serde_json::from_slice(&zip_bytes(&mut zip, "manifest.json")).unwrap();
            assert_eq!(manifest["source"]["nodeId"], emitted);
            let args = json!({"node": emitted, "depth": 2, "images": true});
            let inspected = f.tool("inspect", args);
            assert_eq!(inspected["isError"], false, "{inspected}");
            let summary = &inspected["structuredContent"];
            assert_eq!(summary["node"]["id"], emitted);
            assert_eq!(
                summary["node"]["absoluteBoundingBox"],
                n["absoluteBoundingBox"]
            );
            assert_eq!(summary["images"]["items"][0]["imageRef"], hash);
            let cli: Value = serde_json::from_str(
                &f.cli("inspect", &[emitted, "--depth", "2", "--images"])
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(cli, *summary);
            let cli: Value = serde_json::from_str(&f.cli("nodes", &[emitted]).unwrap()).unwrap();
            assert_eq!(cli, nodes);
            let reply = f.tool("nodes", json!({"node": emitted}));
            assert_eq!(reply["isError"], false, "{reply}");
            assert_eq!(
                serde_json::from_str::<Value>(reply["content"][0]["text"].as_str().unwrap())
                    .unwrap(),
                nodes
            );
        }
        let reply = f.tool("export", json!({"node": emitted, "out": png}));
        assert_eq!(reply["isError"], false, "{reply}");
        assert_eq!(
            resvg::tiny_skia::Pixmap::decode_png(&std::fs::read(png).unwrap()).unwrap(),
            expected_pixels
        );
        let svg = f.dir.join("export.svg");
        f.cli("export", &[emitted, svg.to_str().unwrap()]).unwrap();
        assert_eq!(
            figlens::render::pixmap(&std::fs::read_to_string(&svg).unwrap()).unwrap(),
            expected_pixels
        );
        let reply = f.tool("export", json!({"node": emitted, "out": svg}));
        assert_eq!(reply["isError"], false, "{reply}");
        assert_eq!(
            figlens::render::pixmap(&std::fs::read_to_string(svg).unwrap()).unwrap(),
            expected_pixels
        );
    }
}

#[test]
fn stored_nodes_and_instances_keep_their_identity() {
    let f = Fixture::new();
    for (id, kind, name, w, h) in [
        ("0:1", "FRAME", "Screen", 100, 100),
        ("0:2", "INSTANCE", "First", 40, 30),
        ("0:3", "INSTANCE", "Second", 40, 30),
        ("0:12", "RECTANGLE", "Artwork", 8, 6),
        ("I0:2;0:14", "INSTANCE", "Nested", 4, 3),
    ] {
        let doc: Value = serde_json::from_str(&f.cli("nodes", &[id]).unwrap()).unwrap();
        assert_eq!(doc["document"]["id"], id);
        assert_eq!(doc["document"]["type"], kind);
        assert_eq!(doc["document"]["name"], name);
        let inspected = f.tool("inspect", json!({"node": id, "depth": 4}));
        assert_eq!(inspected["isError"], false, "{inspected}");
        assert_eq!(inspected["structuredContent"]["node"]["id"], id);
        let png = f.dir.join("stored.png");
        let bundle = f.dir.join("stored.zip");
        f.cli("export", &[id, png.to_str().unwrap()]).unwrap();
        f.cli("bundle", &[id, bundle.to_str().unwrap()]).unwrap();
        let expected = resvg::tiny_skia::Pixmap::decode_png(&std::fs::read(png).unwrap()).unwrap();
        assert_eq!((expected.width(), expected.height()), (w, h));
        let mut zip = zip::ZipArchive::new(std::fs::File::open(bundle).unwrap()).unwrap();
        let bundled: Value =
            serde_json::from_slice(&zip_bytes(&mut zip, "design/nodes.json")).unwrap();
        assert_eq!(bundled, doc);
        assert_eq!(
            resvg::tiny_skia::Pixmap::decode_png(&zip_bytes(&mut zip, "references/screen.png"))
                .unwrap(),
            expected
        );
        if id == "0:12" {
            assert_eq!(doc["document"]["fills"][0]["imageRef"], "red");
            assert!(
                expected
                    .data()
                    .chunks_exact(4)
                    .all(|p| p == [255, 0, 0, 255])
            );
        }
    }
    // Existing dash and URL inputs keep the entire expanded path and source manifest ID.
    for input in [
        "I0-2;0-14;0-21",
        "https://www.figma.com/design/Key/Test?node-id=I0-2;0-14;0-21",
    ] {
        let n: Value = serde_json::from_str(&f.cli("nodes", &[input]).unwrap()).unwrap();
        assert_eq!(n["document"]["id"], "I0:2;0:14;0:21");
        let reply = f.tool("variables", json!({"node": input}));
        assert_eq!(reply["isError"], false, "{reply}");
    }
}

#[test]
fn batch_uses_the_same_bundle_target_resolver() {
    let f = Fixture::new();
    let list = f.dir.join("frames.txt");
    let out = f.dir.join("batch");
    let ids = ["0:12", "I0:2;0:11", "I0:3;0:14;0:21"];
    std::fs::write(&list, ids.join("\n")).unwrap();
    f.cli(
        "bundle",
        &[
            "--frames",
            list.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--jobs",
            "1",
        ],
    )
    .unwrap();
    for id in ids {
        let path = out.join(format!("{}.zip", id.replace(':', "-")));
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let nodes: Value =
            serde_json::from_slice(&zip_bytes(&mut zip, "design/nodes.json")).unwrap();
        assert_eq!(nodes["document"]["id"], id);
        assert!(
            resvg::tiny_skia::Pixmap::decode_png(&zip_bytes(&mut zip, "references/screen.png"))
                .is_ok()
        );
    }
}

#[test]
fn nodes_frames_match_single_nodes_calls_and_isolate_missing_ids() {
    let f = Fixture::new();
    let list = f.dir.join("nodes-frames.txt");
    let ids = ["0:12", "I0:2;0:11", "9:9", "I0:3;0:14;0:21"];
    std::fs::write(&list, format!("{}\n\n", ids.join("\n"))).unwrap();
    let mut doc = figlens::Document::open(Path::new(&f.file)).unwrap();
    let message = doc.take_message().unwrap();
    let tree = Tree::new(&message);
    let frames = crate::node_list(&["--frames".into(), list.to_string_lossy().into()]).unwrap();
    let mut out = Vec::new();
    crate::nodes_batch(&tree, &frames, &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), ids.len());
    for (id, mut line) in ids.into_iter().zip(lines) {
        assert_eq!(line.as_object_mut().unwrap().remove("nodeId").unwrap(), id);
        match f.cli("nodes", &[id]) {
            Ok(single) => assert_eq!(
                format!("{}\n", serde_json::to_string_pretty(&line).unwrap()),
                single
            ),
            Err(e) => assert_eq!(line, json!({ "error": e.to_string() })),
        }
    }
    let too_many = vec!["0:12".to_owned(); crate::MAX_NODE_FRAMES + 1];
    let err = crate::nodes_batch(&tree, &too_many, &mut Vec::new()).unwrap_err();
    assert!(err.to_string().contains("at most"), "{err}");
}

fn layer(id: u32, parent: u32, kind: &'static str) -> K<'static> {
    obj(
        "NodeChange",
        vec![
            ("guid", guid(id)),
            ("type", K::Enum("NodeType", kind)),
            ("size", size(10.0, 10.0)),
            (
                "parentIndex",
                obj(
                    "ParentIndex",
                    vec![("guid", guid(parent)), ("position", K::String("a".into()))],
                ),
            ),
        ],
    )
}

/// Under 0:0, `kind` 0:2 heads a chain of `kind` one layer more than the limit below it.
fn too_deep_chain(kind: &'static str) -> Vec<K<'static>> {
    let last = 2 + figlens::expand::MAX_DEPTH as u32 + 1;
    let mut changes = vec![layer(2, 0, kind)];
    changes.extend((3..=last).map(|id| layer(id, id - 1, kind)));
    changes
}

/// Frame 0:1 is shallow; 0:2 holds a chain of `kind` one layer more than the limit below it.
fn too_deep_message(kind: &'static str) -> K<'static> {
    let mut changes = vec![
        obj(
            "NodeChange",
            vec![("guid", guid(0)), ("type", K::Enum("NodeType", "CANVAS"))],
        ),
        layer(1, 0, "FRAME"),
    ];
    changes.extend(too_deep_chain(kind));
    obj("Message", vec![("nodeChanges", K::Array(changes))])
}

#[test]
fn nodes_frames_report_a_too_deep_frame_and_continue() {
    let message = too_deep_message("FRAME");
    let tree = Tree::new(&message);
    let mut out = Vec::new();
    crate::nodes_batch(&tree, &["0:2".into(), "0:1".into()], &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        (
            lines[0]["error"]
                .as_str()
                .map(|e| e.contains("layer depth")),
            &lines[1]["document"]["id"]
        ),
        (Some(true), &json!("0:1"))
    );
}

#[test]
fn bundle_frames_skip_a_too_deep_frame_and_continue() {
    let message = too_deep_message("FRAME");
    let tree = Tree::new(&message);
    let dir = TestDir::new();
    let list = dir.join("frames.txt");
    std::fs::write(&list, "0:2\n0:1\n").unwrap();
    let out = dir.join("out");
    let args = [
        "--frames",
        list.to_str().unwrap(),
        "--out-dir",
        out.to_str().unwrap(),
        "--jobs",
        "2",
    ];
    let fig = Path::new("examples/first-success/design.fig");
    crate::batch(&tree, fig, None, &args.map(String::from)).unwrap();
    assert_eq!(
        (out.join("0-2.zip").exists(), out.join("0-1.zip").exists()),
        (false, true)
    );
}

#[test]
fn tree_listings_reject_a_too_deep_file_unless_depth_bounds_them() {
    let message = too_deep_message("FRAME");
    let tree = Tree::new(&message);
    let rejected = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        Command::from_cli("tree", &args, &tree)
            .unwrap()
            .execute(&tree, Path::new("unused.fig"))
            .err()
            .map(|e| e.to_string().contains("layer depth"))
    };
    assert_eq!(
        [
            rejected(&[]),
            rejected(&["--json"]),
            rejected(&["--top-frames"]),
            rejected(&["--depth", "2"]),
        ],
        [Some(true), Some(true), None, None]
    );
}

#[test]
fn top_frames_ignores_deep_sections_on_a_skipped_internal_page() {
    // Document 0:90000 holds a public page with one screen, and internal page 0:0 whose
    // sections nest past the limit.
    let mut changes = vec![
        obj(
            "NodeChange",
            vec![
                ("guid", guid(90000)),
                ("type", K::Enum("NodeType", "DOCUMENT")),
            ],
        ),
        layer(90001, 90000, "CANVAS"),
        layer(90002, 90001, "FRAME"),
        obj(
            "NodeChange",
            vec![
                ("guid", guid(0)),
                ("type", K::Enum("NodeType", "CANVAS")),
                ("internalOnly", K::Bool(true)),
                (
                    "parentIndex",
                    obj(
                        "ParentIndex",
                        vec![("guid", guid(90000)), ("position", K::String("b".into()))],
                    ),
                ),
            ],
        ),
    ];
    changes.extend(too_deep_chain("SECTION"));
    let message = obj("Message", vec![("nodeChanges", K::Array(changes))]);
    let tree = Tree::new(&message);
    let args = ["0:90000".to_owned(), "--top-frames".to_owned()];
    let output = Command::from_cli("tree", &args, &tree)
        .unwrap()
        .execute(&tree, Path::new("unused.fig"))
        .unwrap();
    assert_eq!(output.text().unwrap(), "0:90002 FRAME \"\"\n");
}

#[test]
fn top_frames_rejects_sections_nested_past_the_limit() {
    let message = too_deep_message("SECTION");
    let tree = Tree::new(&message);
    let error = Command::from_cli("tree", &["--top-frames".into()], &tree)
        .unwrap()
        .execute(&tree, Path::new("unused.fig"))
        .err();
    assert!(error.is_some_and(|e| e.to_string().contains("layer depth")));
}

#[test]
fn compare_reports_a_too_deep_scope_as_an_error() {
    let message = too_deep_message("FRAME");
    let tree = Tree::new(&message);
    let oracle = json!({"document": {"id": "0:2"}});
    let error = figlens::compare::compare(&tree, &oracle, "0:2").err();
    assert!(error.is_some_and(|e| e.to_string().contains("layer depth")));
}

#[test]
fn export_frames_match_single_exports_and_isolate_missing_ids() {
    let f = Fixture::new();
    let list = f.dir.join("export-frames.txt");
    let (dir, single) = (f.dir.join("export-batch"), f.dir.join("single.svg"));
    let ids = ["0:12", "9:9", "I0:2;0:11"];
    std::fs::write(&list, ids.join("\n")).unwrap();
    let mut doc = figlens::Document::open(Path::new(&f.file)).unwrap();
    let message = doc.take_message().unwrap();
    let tree = Tree::new(&message);
    let args = |extra: &[&str]| {
        let mut a = vec!["--frames", list.to_str().unwrap(), "--out-dir"];
        a.extend_from_slice(&[dir.to_str().unwrap(), "--format", "svg"]);
        a.extend_from_slice(extra);
        a.into_iter().map(str::to_owned).collect::<Vec<_>>()
    };
    let mut out = Vec::new();
    crate::export_batch(&tree, Path::new(&f.file), &args(&[]), &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), ids.len());
    for (id, mut line) in ids.into_iter().zip(lines) {
        assert_eq!(line.as_object_mut().unwrap().remove("nodeId").unwrap(), id);
        match f.cli("export", &[id, single.to_str().unwrap()]) {
            Ok(_) => {
                let path = dir.join(format!("{}.svg", id.replace(':', "-")));
                assert_eq!(line["path"], path.to_str().unwrap());
                assert_eq!(
                    std::fs::read(path).unwrap(),
                    std::fs::read(&single).unwrap()
                );
            }
            Err(e) => assert_eq!(line, json!({ "error": e.to_string() })),
        }
    }
    // --match-dir reaches every line; the folder is read once, before the batch writes.
    let icons = f.dir.join("icons");
    std::fs::create_dir_all(&icons).unwrap();
    f.cli(
        "export",
        &["0:12", icons.join("card.svg").to_str().unwrap()],
    )
    .unwrap();
    let mut out = Vec::new();
    let matching = args(&["--match-dir", icons.to_str().unwrap()]);
    crate::export_batch(&tree, Path::new(&f.file), &matching, &mut out).unwrap();
    let first: Value =
        serde_json::from_str(String::from_utf8(out).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!(first["matches"], json!(["card.svg"]));
    // Invalid options fail the whole call before anything is written.
    std::fs::remove_dir_all(&dir).unwrap();
    let huge = (usize::MAX / 1024 + 1).to_string();
    for extra in [
        &["--quality", "0"][..],
        &["--format", "a/b.png"],
        &["--max-kb", &huge],
        &["--format", "png", "--match-dir", icons.to_str().unwrap()],
    ] {
        let mut out = Vec::new();
        let result = crate::export_batch(&tree, Path::new(&f.file), &args(extra), &mut out);
        assert!(result.is_err() && out.is_empty(), "{extra:?}");
        assert!(!dir.exists(), "{extra:?}");
    }
}

#[test]
fn export_bounds_pick_the_render_bounds_or_the_layout_box_and_report_both() {
    let f = Fixture::new();
    for ext in ["svg", "png"] {
        let out = f.dir.join(&format!("overflow.{ext}"));
        let export = |extra: &[&str]| {
            let mut args = vec!["0:60", out.to_str().unwrap()];
            args.extend_from_slice(extra);
            serde_json::from_str::<Value>(&f.cli("export", &args).unwrap()).unwrap()
        };
        let render = export(&[]);
        let layout = export(&["--bounds", "layout"]);
        assert_eq!(
            (&render["width"], &layout["width"]),
            (&json!(15), &json!(10)),
            "{ext}"
        );
        assert_eq!(
            (&render["height"], &layout["height"]),
            (&json!(15), &json!(10)),
            "{ext}"
        );
        assert_eq!(render["layoutBox"]["width"], 10.0);
        assert_eq!(render["renderBox"]["width"], 15.0);
        let warning = render["warnings"][0].as_str().unwrap();
        assert!(
            warning.contains("right 5, bottom 5, left 0 px")
                && warning.contains("from child geometry"),
            "{warning}"
        );
        assert!(
            layout["warnings"][0]
                .as_str()
                .unwrap()
                .contains("exported at the layout box")
        );
        if ext == "svg" {
            // The viewport rounds fractional sizes up, so only a clip keeps overflow out.
            export(&["--bounds", "layout"]);
            assert!(std::fs::read_to_string(&out).unwrap().contains("clip-path"));
            export(&[]);
            assert!(!std::fs::read_to_string(&out).unwrap().contains("clip-path"));
        }
    }
    // At scale 0.15 the 10px box rounds up to 2px; the last pixel must not show the overflow.
    let small = f.dir.join("overflow-small.png");
    let path = small.to_str().unwrap();
    f.cli(
        "export",
        &["0:60", path, "--bounds", "layout", "--scale", "0.15"],
    )
    .unwrap();
    let pixels = resvg::tiny_skia::Pixmap::load_png(&small).unwrap();
    assert!(pixels.pixel(1, 1).unwrap().alpha() < 255);
    let png = f.dir.join("overflow-mcp.png");
    let reply = f.tool(
        "export",
        json!({"node": "0:60", "out": png, "bounds": "layout"}),
    );
    assert_eq!(reply["structuredContent"]["width"], 10, "{reply}");
    let reply = f.tool(
        "export",
        json!({"node": "0:60", "out": png, "bounds": "box"}),
    );
    assert_eq!(reply["isError"], true, "{reply}");
    let err = f
        .cli(
            "export",
            &["0:60", png.to_str().unwrap(), "--bounds", "box"],
        )
        .unwrap_err();
    assert!(err.to_string().contains("layout or render"), "{err}");
}

#[test]
fn nodes_ids_take_every_following_argument_and_need_one() {
    let args =
        |a: &[&str]| crate::node_list(&a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
    assert_eq!(
        args(&["--ids", "12-345", "I0:2;0:11"]).unwrap(),
        ["12:345", "I0:2;0:11"]
    );
    for invalid in [&["--ids"][..], &["--frames"], &["--frames", "a", "b"]] {
        assert!(args(invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn missing_and_unsupported_ids_fail_without_fallback_or_output() {
    let f = Fixture::new();
    let png = f.dir.join("invalid.png");
    let bundle = f.dir.join("invalid.zip");
    for id in [
        "",
        "invalid",
        "9:9",
        "I9:9;0:112",
        "I0:2",
        "I0:2;",
        "I0:2;;0:112",
        "I0:2;0:12",       // Stored library guid is not its expanded overrideKey.
        "I0:2;0:21",       // Missing nested instance boundary.
        "I0:2;0:11;0:112", // Plain frames do not form instance boundaries.
        "I0:2;0:14;0:999",
        "I0:1;0:2", // No selecting a stored child instead.
    ] {
        let mut errors = Vec::new();
        for (name, args, tool_args) in [
            ("nodes", vec![id], json!({"node": id})),
            ("inspect", vec![id], json!({"node": id})),
            (
                "inspect",
                vec![id, "--images"],
                json!({"node": id, "images": true}),
            ),
            (
                "export",
                vec![id, png.to_str().unwrap()],
                json!({"node": id, "out": png}),
            ),
            (
                "bundle",
                vec![id, bundle.to_str().unwrap()],
                json!({"node": id, "out": bundle}),
            ),
        ] {
            let error = f.cli(name, &args).unwrap_err().to_string();
            assert!(error.contains("not found"), "{name} {id}: {error}");
            let reply = f.tool(name, tool_args);
            assert_eq!(reply["isError"], true, "{reply}");
            assert_eq!(reply["content"][0]["text"], error);
            assert_eq!(reply["_meta"]["figlens"]["code"], "node_not_found");
            errors.push(error);
        }
        assert!(
            errors.iter().all(|error| error == &errors[0]),
            "{id}: {errors:?}"
        );
        assert!(!png.exists());
        assert!(!bundle.exists());
    }
    for (name, args, arguments) in [
        ("tree", vec!["I0:2;0:11"], json!({"node": "I0:2;0:11"})),
        (
            "find",
            vec!["Artwork", "--node", "I0:2;0:11"],
            json!({"query": "Artwork", "node": "I0:2;0:11"}),
        ),
    ] {
        let error = f.cli(name, &args).unwrap_err().to_string();
        assert!(error.contains("supports stored nodes only"), "{error}");
        assert!(!error.contains("not found"));
        let reply = f.tool(name, arguments);
        assert_eq!(reply["isError"], true);
        assert_eq!(reply["_meta"]["figlens"]["code"], "unsupported_node");
        assert_eq!(reply["content"][0]["text"], error);
    }
    assert!(
        f.cli("node", &["I0:2;0:11"])
            .unwrap_err()
            .to_string()
            .contains("supports stored nodes only")
    );
    // An empty page exists and is inspectable, but has no child union to render.
    assert_eq!(f.tool("inspect", json!({"node": "0:30"}))["isError"], false);
    for (name, out) in [("export", &png), ("bundle", &bundle)] {
        let error = f
            .cli(name, &["0:30", out.to_str().unwrap()])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("unsupported for {name}: no drawable bounds")),
            "{error}"
        );
        let reply = f.tool(name, json!({"node": "0:30", "out": out}));
        assert_eq!(reply["isError"], true);
        assert_eq!(reply["content"][0]["text"], error);
        assert!(!out.exists());
    }
    let request = |id, method, params| {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
    };
    let input = [
        request(1, "tools/call", json!({"name": "bundle", "arguments": {"file": f.file, "node": "I0:2;0:21", "out": bundle}})),
        request(2, "ping", json!({})),
        request(3, "tools/call", json!({"name": "inspect", "arguments": {"file": f.file, "node": "I0:2;0:14;0:21"}})),
    ].join("\n") + "\n";
    let mut output = Vec::new();
    serve(input.as_bytes(), &mut output, &[]).unwrap();
    let replies: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 3);
    let replies = in_request_order(&replies, &[1.into(), 2.into(), 3.into()]);
    assert_eq!(replies[0]["result"]["isError"], true);
    assert_eq!(replies[1]["result"], json!({}));
    assert_eq!(replies[2]["result"]["isError"], false);
    assert_eq!(
        replies[2]["result"]["structuredContent"]["node"]["id"],
        "I0:2;0:14;0:21"
    );
}

#[test]
fn search_shares_effective_results_across_cli_and_mcp_and_validates_before_reading() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let without_freshness = |mut v: Value| {
        v["source"]["freshness"] = Value::Null;
        v
    };
    let cli: Value = serde_json::from_str(&f.cli("search", &["first"]).unwrap()).unwrap();
    let mcp = f.tool("search", json!({"query": "first"}));
    assert_eq!(mcp["isError"], false);
    assert_eq!(
        without_freshness(cli.clone()),
        without_freshness(mcp["structuredContent"].clone())
    );
    let digest: String = Sha256::digest(std::fs::read(&f.file).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(cli["source"]["snapshotId"], format!("sha256:{digest}"));
    let hit = cli["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["ref"]["nodeId"] == "I0:2;0:13")
        .unwrap();
    assert_eq!(
        (&hit["origin"], &hit["textSource"]),
        (&json!("instance-effective"), &json!("override"))
    );
    // The exact reference opens with inspect; find keeps searching stored layers only.
    let inspected = f.tool("inspect", json!({"node": "I0:2;0:13"}));
    assert_eq!(
        inspected["structuredContent"]["node"]["characters"],
        "First"
    );
    let found = f.tool("find", json!({"query": "first"}));
    assert!(
        found["structuredContent"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| !r["id"].as_str().unwrap().starts_with('I'))
    );
    // A warm server reuses the index and still answers other tools from its tree.
    let request = |id, name, arguments: Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}}).to_string()
    };
    let input = [
        request(1, "search", json!({"file": f.file, "query": "first"})),
        request(2, "find", json!({"file": f.file, "query": "first"})),
        request(3, "search", json!({"file": f.file, "query": "first"})),
    ]
    .join("\n")
        + "\n";
    let mut output = Vec::new();
    serve(input.as_bytes(), &mut output, &[]).unwrap();
    let replies: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        replies[1]["result"]["structuredContent"],
        found["structuredContent"]
    );
    let warm = &replies[2]["result"]["structuredContent"];
    assert_eq!(warm["source"]["freshness"]["reusedIndex"], true);
    assert_eq!(without_freshness(warm.clone()), without_freshness(cli));
    // Invalid arguments fail before the (missing) source is read.
    for arguments in [
        json!({"query": "x", "mode": "visual"}),
        json!({"query": "x", "limit": 0}),
        json!({"query": "x", "type": "te-xt"}),
        json!({"query": " / "}),
        json!({"query": "x", "cursor": "s1.bad"}),
    ] {
        let mut arguments = arguments;
        arguments["file"] = "/nonexistent/missing.fig".into();
        let reply = call_tool(&json!({"name": "search", "arguments": arguments}), None);
        assert_eq!(
            reply["_meta"]["figlens"]["code"], "invalid_arguments",
            "{reply}"
        );
    }
    let args = ["search", "/nonexistent/missing.fig", "x", "--limit", "0"];
    let error = run(
        args.iter().map(|s| (*s).into()).collect(),
        &mut String::new(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("1-200"), "{error}");
}
