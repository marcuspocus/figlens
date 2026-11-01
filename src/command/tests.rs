use crate::{call_tool, in_request_order, run, serve};
use std::io::Write as _;

use crate::test_support::TestDir;

#[test]
fn handoff_gaps_and_text_summary_reach_cli_mcp_and_bundle() {
    use kiwi_schema::{Def, Field, Schema, TYPE_STRING, TYPE_UINT, Value as K};
    let dir = TestDir::new();
    let original = std::path::Path::new("examples/first-success/design.fig");
    let doc = figlens::Document::open(original).unwrap();
    let mut message = doc.message().unwrap();
    let schema = Schema::decode(&doc.schema.encode()).unwrap();
    let schema = Schema::new(
        schema
            .defs
            .into_iter()
            .map(|d| {
                let mut fields = d.fields;
                if d.name == "NodeChange" {
                    for (name, type_id, value) in [
                        ("leadingTrim", TYPE_STRING, 100),
                        ("textAutoResize", TYPE_STRING, 101),
                        ("textTruncation", TYPE_STRING, 102),
                        ("maxLines", TYPE_UINT, 103),
                        ("textDecoration", TYPE_STRING, 104),
                    ] {
                        fields.push(Field {
                            name: name.into(),
                            type_id,
                            value,
                            is_array: false,
                        });
                    }
                }
                Def::new(d.name, d.kind, fields)
            })
            .collect(),
    );
    let K::Object(_, root) = &mut message else {
        unreachable!()
    };
    let K::Array(nodes) = root.get_mut("nodeChanges").unwrap() else {
        unreachable!()
    };
    let node = nodes
        .iter_mut()
        .find(|n| n.get("guid").and_then(figlens::guid_key).as_deref() == Some("0:4"))
        .unwrap();
    let K::Object(_, fields) = node else {
        unreachable!()
    };
    for (name, value) in [
        ("leadingTrim", K::String("CAP_HEIGHT".into())),
        ("textAutoResize", K::String("HEIGHT".into())),
        ("textTruncation", K::String("ENDING".into())),
        ("maxLines", K::UInt(2)),
        ("textDecoration", K::String("UNDERLINE".into())),
    ] {
        fields.insert(name, value);
    }
    let mut canvas = b"fig-kiwi".to_vec();
    canvas.extend(106u32.to_le_bytes());
    for raw in [schema.encode(), message.encode(&schema)] {
        let mut compressed =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        compressed.write_all(&raw).unwrap();
        let bytes = compressed.finish().unwrap();
        canvas.extend((bytes.len() as u32).to_le_bytes());
        canvas.extend(bytes);
    }
    let file = dir.join("handoff.fig");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&file).unwrap());
    zip.start_file("canvas.fig", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&canvas).unwrap();
    let mut original = zip::ZipArchive::new(std::fs::File::open(original).unwrap()).unwrap();
    let mut image = original.by_name("images/first-success-tile").unwrap();
    zip.start_file(
        "images/first-success-tile",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    std::io::copy(&mut image, &mut zip).unwrap();
    zip.finish().unwrap();
    let mut text = String::new();
    run(
        vec![
            "inspect".into(),
            file.to_string_lossy().into_owned(),
            "0:4".into(),
        ],
        &mut text,
    )
    .unwrap();
    let inspect: serde_json::Value = serde_json::from_str(&text).unwrap();
    for (field, value) in [
        ("textAutoResize", serde_json::json!("HEIGHT")),
        ("textTruncation", serde_json::json!("ENDING")),
        ("maxLines", serde_json::json!(2)),
        ("textDecoration", serde_json::json!("UNDERLINE")),
    ] {
        assert_eq!(inspect["node"]["style"][field], value);
    }
    assert_eq!(
        inspect["handoffDiagnostics"]["items"][0]["feature"],
        "leadingTrim"
    );
    assert_eq!(inspect["handoffDiagnostics"]["items"][0]["nodeCount"], 1);
    let reply = call_tool(
        &serde_json::json!({"name":"inspect","arguments":{"file":file,"node":"0:4"}}),
        None,
    );
    assert_eq!(reply["structuredContent"], inspect);
    let out = dir.join("bundle.zip");
    let reply = call_tool(
        &serde_json::json!({"name":"bundle","arguments":{"file":file,"node":"0:1","out":out}}),
        None,
    );
    assert_eq!(reply["isError"], false, "{reply}");
    assert!(
        reply["structuredContent"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("Handoff leadingTrim"))
    );
    let mut archive = zip::ZipArchive::new(std::fs::File::open(out).unwrap()).unwrap();
    let stored: serde_json::Value =
        serde_json::from_reader(archive.by_name("design/nodes.json").unwrap()).unwrap();
    assert_eq!(
        stored["_figlens"]["handoffDiagnostics"]["items"][0]["nodeCount"],
        1
    );
    let mut context = String::new();
    std::io::Read::read_to_string(&mut archive.by_name("context.md").unwrap(), &mut context)
        .unwrap();
    assert!(context.contains("Handoff leadingTrim"));
}

#[test]
fn outputs_do_not_overwrite_the_source_through_cli_or_mcp() {
    let dir = TestDir::new();
    for command in ["bundle", "export"] {
        // Source paths need no particular extension; export selects its encoder from out.
        let source = dir.join(if command == "export" {
            "source.png"
        } else {
            "source.fig"
        });
        let bytes = std::fs::read("examples/first-success/design.fig").unwrap();
        std::fs::write(&source, &bytes).unwrap();
        let label = source.to_string_lossy().into_owned();
        let error = run(
            vec![command.into(), label.clone(), "0:1".into(), label.clone()],
            &mut String::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("must not overwrite"));
        let reply = call_tool(
            &serde_json::json!({"name":command,"arguments":{"file":label,"node":"0:1","out":label}}),
            None,
        );
        assert_eq!(reply["isError"], true);
        assert_eq!(reply["_meta"]["figlens"]["code"], "invalid_arguments");
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
    }
}

#[test]
fn oversized_requests_recover_without_discarding_the_loaded_document() {
    let fixture = Fixture::new();
    let call = |id| {
        serde_json::json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"inspect","arguments":{"file":fixture.file(),"node":"0:1"}}}).to_string()
    };
    let input = format!(
        "{}\n{}\n{}\n{}\n",
        call(1),
        "x".repeat(1024 * 1024 + 1),
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"ping"}),
        call(4)
    );
    let mut output = Vec::new();
    serve(input.as_bytes(), &mut output, &[]).unwrap();
    let replies: Vec<serde_json::Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(replies.len(), 4);
    let ids = [1.into(), serde_json::Value::Null, 3.into(), 4.into()];
    let replies = in_request_order(&replies, &ids);
    assert_eq!(replies[0]["result"]["isError"], false);
    assert_eq!(replies[1]["error"]["code"], -32600);
    assert_eq!(replies[1]["id"], serde_json::Value::Null);
    assert_eq!(replies[2]["result"], serde_json::json!({}));
    assert_eq!(replies[3]["result"], replies[0]["result"]);
}

#[test]
fn subcommand_help_does_not_open_a_source_or_run_an_action() {
    for command in [
        "tree",
        "find",
        "inspect",
        "node",
        "nodes",
        "variables",
        "bundle",
        "export",
        "compare",
        "preview",
        "mcp",
        "workflow",
        "cache",
        "update",
    ] {
        for flag in ["--help", "-h"] {
            let mut out = String::new();
            run(vec![command.into(), flag.into()], &mut out).unwrap();
            assert!(
                out.contains(&format!("figlens {command}")),
                "{command} {flag}"
            );
        }
    }
}

#[test]
fn cli_workflow_prints_the_mcp_guidance_with_cli_directions() {
    let mut out = String::new();
    run(vec!["workflow".into(), "images".into()], &mut out).unwrap();
    let mcp = crate::workflow::read("images").unwrap();
    let body = &mcp[mcp.find("\n\n").unwrap()..];
    assert!(out.ends_with(&format!("{body}\n")) && !out.contains("figlens MCP tools"));
    assert!(out.contains("run `figlens workflow <topic>`"));
    let error = run(vec!["workflow".into(), "nope".into()], &mut String::new()).unwrap_err();
    assert!(
        error.to_string().contains("choose build, audit, images"),
        "{error}"
    );
}

#[test]
fn image_budget_reaches_cli_mcp_and_offline_preview_without_writing_output() {
    let dir = TestDir::new();
    let file = dir
        .oversized_image("first-success-tile")
        .to_string_lossy()
        .into_owned();
    let output = dir.join("output").to_string_lossy().into_owned();
    for (command, extra) in [
        ("inspect", vec!["--images".to_owned()]),
        ("bundle", vec![format!("{output}.zip")]),
        ("export", vec![format!("{output}.png")]),
        (
            "preview",
            vec!["--out".to_owned(), format!("{output}.html")],
        ),
    ] {
        let args = [vec![command.into(), file.clone(), "0:1".into()], extra].concat();
        let error = run(args, &mut String::new()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("parse budget exceeded: image bytes"),
            "{command}: {error}"
        );
    }
    for extension in ["zip", "png", "html"] {
        assert!(!std::path::Path::new(&format!("{output}.{extension}")).exists());
    }
    let request = |id, file: &str| {
        serde_json::json!({
            "jsonrpc":"2.0", "id":id, "method":"tools/call",
            "params":{"name":"inspect","arguments":{"file":file,"node":"0:1","images":true}}
        })
    };
    let input = format!(
        "{}\n{}\n{}\n",
        request(1, &file),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
        request(3, "examples/first-success/design.fig")
    );
    let mut output = Vec::new();
    serve(input.as_bytes(), &mut output, &[]).unwrap();
    let replies: Vec<serde_json::Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 3);
    let replies = in_request_order(&replies, &[1.into(), 2.into(), 3.into()]);
    assert_eq!(replies[0]["result"]["isError"], true);
    assert_eq!(
        replies[0]["result"]["_meta"]["figlens"]["code"],
        "parse_budget_exceeded"
    );
    assert_eq!(replies[1]["result"], serde_json::json!({}));
    assert_eq!(replies[2]["result"]["isError"], false);
}

struct Fixture(TestDir);

impl Fixture {
    fn new() -> Self {
        Self::named("Frame")
    }

    fn named(name: &str) -> Self {
        Self::build(name, true)
    }

    fn build(name: &str, visible: bool) -> Self {
        use kiwi_schema::{
            Def, DefKind, Field, Schema, TYPE_BOOL, TYPE_FLOAT, TYPE_STRING, TYPE_UINT, Value,
        };
        use std::collections::HashMap;
        let dir = TestDir::new();
        let field = |name: &str, type_id, value, is_array| Field {
            name: name.into(),
            type_id,
            value,
            is_array,
        };
        let schema = Schema::new(vec![
            Def::new(
                "GUID".into(),
                DefKind::Struct,
                vec![
                    field("sessionID", TYPE_UINT, 0, false),
                    field("localID", TYPE_UINT, 0, false),
                ],
            ),
            Def::new(
                "NodeType".into(),
                DefKind::Enum,
                vec![field("DOCUMENT", 0, 0, false), field("FRAME", 0, 1, false)],
            ),
            Def::new(
                "Vector".into(),
                DefKind::Struct,
                vec![
                    field("x", TYPE_FLOAT, 0, false),
                    field("y", TYPE_FLOAT, 0, false),
                ],
            ),
            Def::new(
                "ParentIndex".into(),
                DefKind::Struct,
                vec![
                    field("guid", 0, 0, false),
                    field("position", TYPE_STRING, 0, false),
                ],
            ),
            Def::new(
                "NodeChange".into(),
                DefKind::Message,
                vec![
                    field("guid", 0, 1, false),
                    field("type", 1, 2, false),
                    field("name", TYPE_STRING, 3, false),
                    field("parentIndex", 3, 4, false),
                    field("size", 2, 5, false),
                    field("visible", TYPE_BOOL, 6, false),
                ],
            ),
            Def::new(
                "Message".into(),
                DefKind::Message,
                vec![field("nodeChanges", 4, 1, true)],
            ),
        ]);
        let guid = |id| {
            Value::Object(
                "GUID",
                HashMap::from([("sessionID", Value::UInt(0)), ("localID", Value::UInt(id))]),
            )
        };
        let message = Value::Object(
            "Message",
            HashMap::from([(
                "nodeChanges",
                Value::Array(vec![
                    Value::Object(
                        "NodeChange",
                        HashMap::from([
                            ("guid", guid(0)),
                            ("type", Value::Enum("NodeType", "DOCUMENT")),
                            ("name", Value::String("Document".into())),
                        ]),
                    ),
                    Value::Object(
                        "NodeChange",
                        HashMap::from([
                            ("guid", guid(1)),
                            ("type", Value::Enum("NodeType", "FRAME")),
                            ("name", Value::String(name.into())),
                            (
                                "parentIndex",
                                Value::Object(
                                    "ParentIndex",
                                    HashMap::from([
                                        ("guid", guid(0)),
                                        ("position", Value::String("a".into())),
                                    ]),
                                ),
                            ),
                            (
                                "size",
                                Value::Object(
                                    "Vector",
                                    HashMap::from([
                                        ("x", Value::Float(16.0)),
                                        ("y", Value::Float(8.0)),
                                    ]),
                                ),
                            ),
                            ("visible", Value::Bool(visible)),
                        ]),
                    ),
                ]),
            )]),
        );
        let mut canvas = b"fig-kiwi".to_vec();
        canvas.extend(1u32.to_le_bytes());
        for bytes in [schema.encode(), message.encode(&schema)] {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&bytes).unwrap();
            let chunk = encoder.finish().unwrap();
            canvas.extend((chunk.len() as u32).to_le_bytes());
            canvas.extend(chunk);
        }
        let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join("input.fig")).unwrap());
        zip.start_file("canvas.fig", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&canvas).unwrap();
        zip.finish().unwrap();
        Self(dir)
    }

    fn file(&self) -> String {
        self.0.join("input.fig").to_string_lossy().into_owned()
    }
}

fn assert_bundle(path: &str) {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["root"]["name"], "Renamed");
    assert_eq!(manifest["source"]["fileKey"], "Key");
    assert_eq!(manifest["source"]["nodeId"], "0:1");
    for entry in [
        "design/nodes.json",
        "design/variables.json",
        "references/screen.png",
        "assets.json",
        "missing-assets.json",
        "context.md",
    ] {
        assert!(zip.by_name(entry).is_ok(), "missing {entry}");
    }
    let nodes: serde_json::Value =
        serde_json::from_reader(zip.by_name("design/nodes.json").unwrap()).unwrap();
    assert_eq!(nodes["document"]["id"], "0:1");
    assert!(!nodes.to_string().contains("\"_r\""));
    let context = std::io::read_to_string(zip.by_name("context.md").unwrap()).unwrap();
    assert!(context.contains("Font availability: UNVERIFIED"));
    assert!(context.contains("Continue implementation without waiting for missing inputs"));
    assert!(context.contains("final visual acceptance"));
    assert!(!context.contains("Block affected work"));
}

#[test]
fn parse_budget_failure_is_a_tool_error_and_mcp_keeps_serving() {
    use kiwi_schema::{Def, DefKind, Field, Schema};

    let dir = TestDir::new();
    let file = dir.join("finite-depth.fig");
    // A tiny acyclic chain of 65 messages crosses the default depth of 64.
    // This is finite ordinary data, not a cyclic schema or a crash payload.
    let levels = 65;
    let schema = Schema::new(
        (0..levels)
            .map(|level| {
                Def::new(
                    if level == 0 {
                        "Message".into()
                    } else {
                        format!("Level{level}")
                    },
                    DefKind::Message,
                    if level + 1 == levels {
                        vec![]
                    } else {
                        vec![Field {
                            name: "child".into(),
                            type_id: level + 1,
                            is_array: false,
                            value: 1,
                        }]
                    },
                )
            })
            .collect(),
    );
    let mut bytes = vec![1; (levels - 1) as usize];
    bytes.extend(vec![0; levels as usize]);
    let mut canvas = b"fig-kiwi".to_vec();
    canvas.extend(106u32.to_le_bytes());
    for raw in [schema.encode(), bytes] {
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&raw).unwrap();
        let compressed = encoder.finish().unwrap();
        canvas.extend((compressed.len() as u32).to_le_bytes());
        canvas.extend(compressed);
    }
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&file).unwrap());
    zip.start_file("canvas.fig", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&canvas).unwrap();
    zip.finish().unwrap();
    let normal = Fixture::new();
    let request = |id, file: &str| {
        serde_json::json!({
            "jsonrpc":"2.0", "id":id, "method":"tools/call",
            "params":{"name":"tree","arguments":{"file":file,"depth":1}}
        })
        .to_string()
    };
    let input = format!(
        "{}\n{}\n{}\n",
        request(1, file.to_str().unwrap()),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
        request(3, &normal.file())
    );
    let mut output = Vec::new();
    serve(input.as_bytes(), &mut output, &[]).unwrap();
    let replies: Vec<serde_json::Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 3);
    let replies = in_request_order(&replies, &[1.into(), 2.into(), 3.into()]);
    assert_eq!(replies[0]["result"]["isError"], true);
    assert_eq!(
        replies[0]["result"]["_meta"]["figlens"]["code"],
        "parse_budget_exceeded"
    );
    assert!(
        replies[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("parse budget exceeded: value depth (limit 64)")
    );
    assert_eq!(replies[1]["result"], serde_json::json!({}));
    assert_eq!(replies[2]["result"]["isError"], false);
    assert!(
        replies[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Frame")
    );
    let error = run(
        vec!["tree".into(), file.to_string_lossy().into_owned()],
        &mut String::new(),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("parse budget exceeded: value depth")
    );
    run(vec!["tree".into(), normal.file()], &mut String::new()).unwrap();
}

#[test]
fn cli_and_mcp_success_results_match() {
    let fixture = Fixture::new();
    for (name, cli, arguments, expected) in [
        (
            "tree",
            vec!["--depth", "1"],
            serde_json::json!({"depth": 1}),
            Some("0:0 DOCUMENT \"Document\"\n  0:1 FRAME \"Frame\"\n"),
        ),
        (
            "tree",
            vec!["0-1", "--json", "--depth", "0"],
            serde_json::json!({"node": "0-1", "json": true, "depth": 0}),
            Some("{\"id\":\"0:1\",\"name\":\"Frame\",\"size\":[16.0,8.0],\"type\":\"FRAME\"}\n"),
        ),
        (
            "nodes",
            vec!["0-1"],
            serde_json::json!({"node": "0-1"}),
            None,
        ),
        (
            "find",
            vec!["frame", "--type", "frame", "--node", "0-0", "--limit", "1"],
            serde_json::json!({"query": "frame", "type": "frame", "node": "0-0", "limit": 1}),
            None,
        ),
        (
            "inspect",
            vec!["0-1", "--depth", "0"],
            serde_json::json!({"node": "0-1", "depth": 0}),
            None,
        ),
        (
            "inspect",
            vec!["0-1", "--images", "--depth", "0"],
            serde_json::json!({"node": "0-1", "depth": 0, "images": true}),
            None,
        ),
        ("variables", vec![], serde_json::json!({}), None),
        (
            "variables",
            vec!["0-1"],
            serde_json::json!({"node": "0-1"}),
            None,
        ),
        (
            "export",
            vec!["0-1", "image.png", "--scale", "2", "--quality", "90"],
            serde_json::json!({"node": "0-1", "out": "image.png", "scale": 2, "quality": 90}),
            None,
        ),
        (
            "bundle",
            vec![
                "https://www.figma.com/design/Key/Test?node-id=0-1",
                "bundle.zip",
                "--name",
                "Renamed",
            ],
            serde_json::json!({"node": "https://www.figma.com/design/Key/Test?node-id=0-1", "out": "bundle.zip", "name": "Renamed"}),
            None,
        ),
    ] {
        let mut cli: Vec<String> = cli.into_iter().map(str::to_owned).collect();
        let mut arguments = arguments;
        arguments["file"] = fixture.file().into();
        if let Some(out) = arguments["out"].as_str() {
            let out = fixture.0.join(out).to_string_lossy().into_owned();
            cli[1] = out.clone();
            arguments["out"] = out.into();
        }
        let mut argv = vec![name.into(), fixture.file()];
        argv.extend(cli);
        let mut text = String::new();
        run(argv, &mut text).unwrap();
        if name == "bundle" {
            let path = arguments["out"].as_str().unwrap();
            assert_bundle(path);
            std::fs::remove_file(path).unwrap();
        }
        let reply = call_tool(
            &serde_json::json!({"name": name, "arguments": arguments}),
            None,
        );
        assert_eq!(reply["isError"], false, "{name}: {reply}");
        assert_eq!(reply["content"][0]["text"], text, "{name}");
        if let Some(expected) = expected {
            assert_eq!(text, expected);
        } else {
            let value: serde_json::Value = serde_json::from_str(&text).unwrap();
            match name {
                "nodes" => assert_eq!(value["document"]["id"], "0:1"),
                "variables" => assert!(value.is_object()),
                "find" => {
                    assert_eq!(value["results"][0]["id"], "0:1");
                    assert_eq!(
                        value["results"][0]["size"],
                        serde_json::json!({"width": 16.0, "height": 8.0})
                    );
                    assert_eq!(value["truncated"], false);
                    assert_eq!(reply["structuredContent"], value);
                }
                "inspect" => {
                    assert_eq!(value["node"]["id"], "0:1");
                    assert_eq!(value["node"]["childCount"], 0);
                    assert_eq!(reply["structuredContent"], value);
                    if arguments["images"] == true {
                        assert_eq!(value["images"]["items"], serde_json::json!([]));
                        assert_eq!(value["images"]["truncated"], false);
                    } else {
                        assert!(value.get("images").is_none());
                    }
                }
                "export" => {
                    assert_eq!(
                        (value["width"].as_u64(), value["height"].as_u64()),
                        (Some(32), Some(16))
                    );
                    assert_eq!(value["quality"], 90);
                    assert_eq!(reply["structuredContent"], value);
                    assert_eq!(reply["content"][1]["type"], "image");
                    assert_eq!(reply["content"][1]["mimeType"], "image/png");
                    assert_eq!(
                        reply["content"][1]["data"],
                        figlens::render::base64(
                            &std::fs::read(arguments["out"].as_str().unwrap()).unwrap()
                        )
                    );
                    assert!(
                        std::fs::read(arguments["out"].as_str().unwrap())
                            .unwrap()
                            .starts_with(b"\x89PNG")
                    );
                }
                "bundle" => {
                    assert_eq!(value["nodeId"], "0:1");
                    assert!(value["warnings"].is_array());
                    assert_eq!(reply["structuredContent"], value);
                    assert_bundle(arguments["out"].as_str().unwrap());
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn mcp_images_keep_the_exported_format_and_bytes() {
    let fixture = Fixture::new();
    for (extension, mime) in [
        ("jpg", "image/jpeg"),
        ("webp", "image/webp"),
        ("avif", "image/avif"),
    ] {
        let out = fixture.0.join(&format!("image.{extension}"));
        let reply = call_tool(
            &serde_json::json!({"name": "export", "arguments": {
                "file": fixture.file(), "node": "0:1", "out": out,
            }}),
            None,
        );
        assert_eq!(reply["isError"], false, "{reply}");
        let bytes = std::fs::read(out).unwrap();
        assert_eq!(reply["structuredContent"]["bytes"], bytes.len());
        assert_eq!(reply["content"][1]["mimeType"], mime);
        assert_eq!(reply["content"][1]["data"], figlens::render::base64(&bytes));
    }
}

#[test]
fn mcp_exports_fit_max_size_unless_disabled() {
    let fixture = Fixture::new();
    // The fixture frame is 16x8; scale 200 would be 3200 wide.
    for (max_size, width) in [(None, 1600), (Some(0), 3200), (Some(400), 400)] {
        let out = fixture.0.join("sized.png");
        let mut arguments = serde_json::json!({
            "file": fixture.file(), "node": "0:1", "out": out, "scale": 200,
        });
        if let Some(max_size) = max_size {
            arguments["maxSize"] = max_size.into();
        }
        let reply = call_tool(
            &serde_json::json!({"name": "export", "arguments": arguments}),
            None,
        );
        assert_eq!(reply["isError"], false, "{reply}");
        assert_eq!(
            reply["structuredContent"]["width"], width,
            "maxSize {max_size:?}"
        );
    }
    // Legacy scale strings are rejected before writing; CLI scaling stays uncapped.
    let out = fixture.0.join("legacy.png");
    let reply = call_tool(
        &serde_json::json!({"name": "export", "arguments": {
            "file": fixture.file(), "node": "0:1", "out": &out, "scale": "200x",
        }}),
        None,
    );
    assert_eq!(reply["isError"], true, "{reply}");
    assert_eq!(reply["_meta"]["figlens"]["code"], "invalid_arguments");
    assert!(!out.exists());
    let mut text = String::new();
    let out = fixture.0.join("cli.png");
    run(
        vec![
            "export".into(),
            fixture.file(),
            "0:1".into(),
            out.to_string_lossy().into_owned(),
            "--scale".into(),
            "200".into(),
        ],
        &mut text,
    )
    .unwrap();
    let metadata: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(metadata["width"], 3200);
}

#[test]
fn serve_does_not_hand_a_remote_call_the_local_tree() {
    let fixture = Fixture::new();
    let call = |file: &str, id: u32| {
        format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"tree","arguments":{{"file":{}}}}}}}"#,
            serde_json::json!(file)
        )
    };
    let input = format!(
        "{}\n{}\n{}\n",
        call(&fixture.file(), 1),
        call("gs://bucket/other.fig", 2),
        call(&fixture.file(), 3)
    );
    let mut out = Vec::new();
    serve(input.as_bytes(), &mut out, &[]).unwrap();
    let replies: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies[0]["result"]["isError"], false, "{}", replies[0]);
    // The remote file resolves through gcloud, which is absent here: an error, not our tree.
    assert_eq!(replies[1]["result"]["isError"], true, "{}", replies[1]);
    assert_eq!(replies[2]["result"], replies[0]["result"]);
}

#[test]
fn hidden_layers_are_refused_by_every_export_format_through_cli_and_mcp() {
    let fixture = Fixture::build("Frame", false);
    for name in ["frame.svg", "frame.png"] {
        let out = fixture.0.join(name);
        let error = run(
            vec![
                "export".into(),
                fixture.file(),
                "0:1".into(),
                out.display().to_string(),
            ],
            &mut String::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("the layer is hidden"), "{error}");
        let reply = call_tool(
            &serde_json::json!({"name": "export", "arguments": {
                "file": fixture.file(), "node": "0:1", "out": out,
            }}),
            None,
        );
        assert_eq!(reply["isError"], true, "{reply}");
        assert_eq!(reply["_meta"]["figlens"]["code"], "unsupported_node");
        assert_eq!(reply["content"][0]["text"], error);
        assert!(!out.exists());
    }
}

#[test]
fn hide_draws_layers_hidden_over_the_same_area_through_cli_and_mcp() {
    let dir = TestDir::new();
    let file = "examples/first-success/design.fig";
    let export = |out: &str, extra: &[&str]| {
        let mut args = vec!["export", file, "0:1", out];
        args.extend(extra);
        let mut text = String::new();
        run(args.into_iter().map(str::to_owned).collect(), &mut text)
            .map(|()| serde_json::from_str::<serde_json::Value>(&text).unwrap())
    };
    let (plain, hidden) = (dir.join("plain.png"), dir.join("hidden.png"));
    let base = export(plain.to_str().unwrap(), &[]).unwrap();
    let without = export(hidden.to_str().unwrap(), &["--hide", "0:2"]).unwrap();
    assert_eq!(without["hidden"], serde_json::json!(["0:2"]));
    assert_eq!(
        (&without["width"], &without["height"]),
        (&base["width"], &base["height"])
    );
    assert_ne!(
        std::fs::read(&plain).unwrap(),
        std::fs::read(&hidden).unwrap()
    );
    let mcp = dir.join("mcp.png");
    let reply = call_tool(
        &serde_json::json!({"name": "export", "arguments": {
            "file": file, "node": "0:1", "out": mcp, "hide": "0:2", "maxSize": 0,
        }}),
        None,
    );
    assert_eq!(
        reply["structuredContent"]["hidden"],
        serde_json::json!(["0:2"]),
        "{reply}"
    );
    assert_eq!(
        std::fs::read(&mcp).unwrap(),
        std::fs::read(&hidden).unwrap()
    );
    let refused = dir.join("refused.png");
    for id in ["0:1", "0:10", "9:9"] {
        let error = export(refused.to_str().unwrap(), &["--hide", id]).unwrap_err();
        assert!(
            error.to_string().contains("is not a layer inside node 0:1"),
            "{error}"
        );
    }
    assert!(!refused.exists());
    // Hiding nothing would read as a layer that does not show.
    assert!(export(refused.to_str().unwrap(), &["--hide", " , "]).is_err());
    for hide in ["", ","] {
        let reply = call_tool(
            &serde_json::json!({"name": "export", "arguments": {
                "file": file, "node": "0:1", "out": refused, "hide": hide,
            }}),
            None,
        );
        assert_eq!(reply["isError"], true, "{reply}");
    }
    assert!(!refused.exists());
}

#[test]
fn svg_exports_directly_through_cli_and_mcp() {
    let fixture = Fixture::new();
    let out = fixture.0.join("frame.svg");
    let mut text = String::new();
    run(
        vec![
            "export".into(),
            fixture.file(),
            "0:1".into(),
            out.display().to_string(),
            "--scale".into(),
            "2".into(),
        ],
        &mut text,
    )
    .unwrap();
    let metadata: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(metadata["width"], 16);
    assert_eq!(metadata["height"], 8);
    let svg = std::fs::read_to_string(&out).unwrap();
    assert!(svg.starts_with("<svg "));
    assert!(svg.contains("viewBox=\"0 0 16 8\""));
    let parsed = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
    assert_eq!(parsed.size().width(), 16.0);
    assert_eq!(parsed.size().height(), 8.0);
    let reply = call_tool(
        &serde_json::json!({"name": "export", "arguments": {
            "file": fixture.file(), "node": "0:1", "out": out,
        }}),
        None,
    );
    assert_eq!(reply["isError"], false, "{reply}");
    assert_eq!(reply["structuredContent"]["bytes"], svg.len());
    assert_eq!(reply["structuredContent"], metadata);
    assert_eq!(reply["content"].as_array().unwrap().len(), 1);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), svg);
    let error = run(
        vec![
            "export".into(),
            fixture.file(),
            "0:1".into(),
            out.display().to_string(),
            "--max-kb".into(),
            "0".into(),
        ],
        &mut String::new(),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("SVG comes to"), "{error}");
    assert_eq!(std::fs::read_to_string(out).unwrap(), svg);
}

#[test]
fn discovery_rejects_invalid_bounds_in_cli_and_mcp() {
    let fixture = Fixture::new();
    for (name, cli, arguments, error) in [
        (
            "find",
            vec!["  "],
            serde_json::json!({"query": "  "}),
            "non-empty query",
        ),
        (
            "find",
            vec!["frame", "--limit", "0"],
            serde_json::json!({"query": "frame", "limit": 0}),
            "--limit takes 1-200",
        ),
        (
            "inspect",
            vec!["0:1", "--depth", "11"],
            serde_json::json!({"node": "0:1", "depth": 11}),
            "--depth takes 0-10",
        ),
        (
            "find",
            vec!["frame", "--node", "9:9"],
            serde_json::json!({"query": "frame", "node": "9:9"}),
            "node 9:9 not found",
        ),
    ] {
        let mut argv = vec![name.to_owned(), fixture.file()];
        argv.extend(cli.into_iter().map(str::to_owned));
        assert!(
            run(argv, &mut String::new())
                .unwrap_err()
                .to_string()
                .contains(error)
        );
        let mut arguments = arguments;
        arguments["file"] = fixture.file().into();
        let reply = call_tool(
            &serde_json::json!({"name": name, "arguments": arguments}),
            None,
        );
        assert_eq!(reply["isError"], true);
        assert_eq!(
            reply["_meta"]["figlens"]["code"],
            if arguments.get("node").is_some_and(|n| n == "9:9") {
                "node_not_found"
            } else {
                "invalid_arguments"
            }
        );
        assert_eq!(reply["content"].as_array().unwrap().len(), 1);
        assert!(reply.get("structuredContent").is_none());
    }
}

#[test]
fn variables_commands_distinguish_local_variables_from_frame_bindings() {
    use super::Command;
    use figlens::Tree;
    use kiwi_schema::Value;
    use std::collections::HashMap;
    use std::path::Path;

    let obj = |fields: Vec<(&'static str, Value<'static>)>| {
        Value::Object("Test", fields.into_iter().collect())
    };
    let guid = |id| {
        obj(vec![
            ("sessionID", Value::UInt(0)),
            ("localID", Value::UInt(id)),
        ])
    };
    let reference = |id| obj(vec![("guid", guid(id))]);
    let frame = obj(vec![
        ("guid", guid(1)),
        ("type", Value::Enum("NodeType", "FRAME")),
        ("parentIndex", reference(2)),
        (
            "parameterConsumptionMap",
            obj(vec![(
                "entries",
                Value::Array(vec![obj(vec![
                    (
                        "variableField",
                        Value::Enum("VariableField", "STACK_SPACING"),
                    ),
                    (
                        "variableData",
                        obj(vec![
                            ("dataType", Value::Enum("VariableDataType", "ALIAS")),
                            ("value", obj(vec![("alias", reference(4))])),
                        ]),
                    ),
                ])]),
            )]),
        ),
    ]);
    let canvas = obj(vec![("type", Value::Enum("NodeType", "CANVAS"))]);
    let set = obj(vec![
        ("guid", guid(3)),
        ("type", Value::Enum("NodeType", "VARIABLE_SET")),
        ("name", Value::String("Spacing".into())),
        (
            "variableSetModes",
            Value::Array(vec![obj(vec![
                ("id", guid(6)),
                ("name", Value::String("Default".into())),
            ])]),
        ),
    ]);
    let variable = |id, name: &str, position: &str, number| {
        obj(vec![
            ("guid", guid(id)),
            ("type", Value::Enum("NodeType", "VARIABLE")),
            ("name", Value::String(name.into())),
            ("sortPosition", Value::String(position.into())),
            (
                "variableResolvedType",
                Value::Enum("VariableResolvedType", "FLOAT"),
            ),
            ("variableSetID", reference(3)),
            (
                "variableDataValues",
                obj(vec![(
                    "entries",
                    Value::Array(vec![obj(vec![
                        ("modeID", guid(6)),
                        (
                            "variableData",
                            obj(vec![
                                ("dataType", Value::Enum("VariableDataType", "FLOAT")),
                                ("value", obj(vec![("floatValue", Value::Float(number))])),
                            ]),
                        ),
                    ])]),
                )]),
            ),
        ])
    };
    let used = variable(4, "Used", "a", 8.0);
    let unused = variable(5, "Unused", "b", 16.0);
    let tree = Tree {
        nodes: HashMap::from([
            ("0:1".into(), &frame),
            ("0:2".into(), &canvas),
            ("0:3".into(), &set),
            ("0:4".into(), &used),
            ("0:5".into(), &unused),
        ]),
        children: HashMap::from([
            ("0:0".into(), vec!["0:2".into()]),
            (
                "0:2".into(),
                vec!["0:1".into(), "0:3".into(), "0:4".into(), "0:5".into()],
            ),
        ]),
        blobs: Vec::new(),
    };
    for (cli, arguments, expected) in [
        (
            vec![],
            serde_json::json!({}),
            vec!["VariableID:0:4", "VariableID:0:5"],
        ),
        (
            vec!["0-1".into()],
            serde_json::json!({"node": "0-1"}),
            vec!["VariableID:0:4"],
        ),
    ] {
        for command in [
            Command::from_cli("variables", &cli, &tree).unwrap(),
            Command::from_tool(
                &crate::contract::prepare("variables", &{
                    let mut args = arguments.clone();
                    args["file"] = "unused.fig".into();
                    args
                })
                .unwrap(),
                &tree,
                None,
                None,
            )
            .unwrap(),
        ] {
            let output = command.execute(&tree, Path::new("unused.fig")).unwrap();
            let value = output.json().unwrap();
            let ids: Vec<_> = value["variables"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["id"].as_str().unwrap())
                .collect();
            assert_eq!(ids, expected);
            assert_eq!(value["variables"][0]["valuesByMode"]["0:6"], 8.0);
            assert_eq!(value["collections"][0]["id"], "VariableCollectionId:0:3");
            assert_eq!(value["missingVariables"], serde_json::json!([]));
        }
    }
}

#[test]
fn cli_validation_order_and_mcp_preflight_errors() {
    let fixture = Fixture::new();
    for (name, cli, mut arguments, expected) in [
        (
            "nodes",
            vec!["9-9"],
            serde_json::json!({"node": "9-9"}),
            "node 9:9 not found; list ids with `figlens tree <file.fig> --depth 2`",
        ),
        (
            "variables",
            vec!["9-9"],
            serde_json::json!({"node": "9-9"}),
            "node 9:9 not found; list ids with `figlens tree <file.fig> --depth 2`",
        ),
        (
            "bundle",
            vec!["9-9", "output.zip", "--name", "ignored"],
            serde_json::json!({"node": "9-9", "out": "output.zip", "name": "ignored"}),
            "node 9:9 not found; list ids with `figlens tree <file.fig> --depth 2`",
        ),
        (
            "export",
            vec!["9-9;1:1", "output.txt", "--quality", "0"],
            serde_json::json!({"node": "9-9;1:1", "out": "output.txt", "quality": 0}),
            "node 9:9;1:1 not found; list ids with `figlens tree <file.fig> --depth 2`",
        ),
        (
            "export",
            vec!["0-1", "output.txt", "--quality", "0"],
            serde_json::json!({"node": "0-1", "out": "output.txt", "quality": 0}),
            "the output must end in .png, .jpg, .webp, .avif or .svg",
        ),
        (
            "export",
            vec!["0-1", "output.png", "--quality", "0"],
            serde_json::json!({"node": "0-1", "out": "output.png", "quality": 0}),
            "--quality takes 1-100 and --scale a positive number",
        ),
        (
            "tree",
            vec!["--depth", "invalid"],
            serde_json::json!({"depth": "invalid"}),
            "invalid digit found in string",
        ),
    ] {
        arguments["file"] = fixture.file().into();
        let mut argv = vec![name.into(), fixture.file()];
        argv.extend(cli.into_iter().map(str::to_owned));
        let mut text = String::new();
        let error = run(argv, &mut text).unwrap_err().to_string();
        assert_eq!(error, expected);
        assert!(text.is_empty());
        let reply = call_tool(
            &serde_json::json!({"name": name, "arguments": arguments}),
            None,
        );
        assert_eq!(reply["isError"], true);
        if arguments.get("quality").is_some() || arguments.get("depth").is_some() {
            assert_eq!(reply["_meta"]["figlens"]["code"], "invalid_arguments");
        } else {
            assert_eq!(reply["content"][0]["text"], expected);
        }
        assert!(reply.get("structuredContent").is_none());
    }
}

#[test]
fn preview_snapshot_embeds_design_safely_and_never_overwrites_source() {
    let fixture = Fixture::named("</ScRiPt><script>globalThis.pwned=1</script>&");
    let output = fixture.0.join("preview.html");
    let mut result = String::new();
    run(
        vec![
            "preview".into(),
            fixture.file(),
            "0:1".into(),
            "--out".into(),
            output.to_string_lossy().into_owned(),
        ],
        &mut result,
    )
    .unwrap();
    let page = std::fs::read_to_string(&output).unwrap();
    assert!(!page.contains("</ScRiPt>"));
    let boot = page
        .split("<script id=\"boot\" type=\"application/json\">")
        .nth(1)
        .unwrap()
        .split("</script>")
        .next()
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(boot).unwrap();
    assert_eq!(
        data["snapshot"]["nodes"]["document"]["name"],
        "</ScRiPt><script>globalThis.pwned=1</script>&"
    );
    assert!(
        data["snapshot"]["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert!(
        data["snapshot"]["bundle"]
            .as_str()
            .unwrap()
            .starts_with("UEs")
    );
    let source = fixture.0.join("source.html");
    std::fs::copy(fixture.file(), &source).unwrap();
    let before = std::fs::read(&source).unwrap();
    assert!(figlens::preview::snapshot(&source, "0:1", &source).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), before);
    assert!(
        figlens::preview::snapshot(
            std::path::Path::new(&fixture.file()),
            "0:1",
            &fixture.0.join("input.fig")
        )
        .is_err()
    );
}

#[test]
fn mcp_rejects_legacy_coercions_without_changing_cli() {
    let fixture = Fixture::new();
    for (cli, mut arguments, code) in [
        (
            vec!["--json"],
            serde_json::json!({"node":"--json"}),
            "node_not_found",
        ),
        (
            vec!["--depth", "0"],
            serde_json::json!({"depth":"0"}),
            "invalid_arguments",
        ),
        (
            vec![],
            serde_json::json!({"node":false,"depth":null,"json":false}),
            "invalid_arguments",
        ),
    ] {
        arguments["file"] = fixture.file().into();
        let mut argv = vec!["tree".into(), fixture.file()];
        argv.extend(cli.into_iter().map(str::to_owned));
        run(argv, &mut String::new()).unwrap();
        let reply = call_tool(
            &serde_json::json!({"name":"tree","arguments":arguments}),
            None,
        );
        assert_eq!(reply["_meta"]["figlens"]["code"], code, "{reply}");
        assert!(reply["_meta"]["figlens"].get("argumentMode").is_none());
    }
    let out = fixture.0.join("coerced.png");
    let reply = call_tool(
        &serde_json::json!({"name":"export","arguments":{
            "file":fixture.file(),"node":"0:1","out":&out,"scale":"2x"
        }}),
        None,
    );
    assert_eq!(reply["_meta"]["figlens"]["code"], "invalid_arguments");
    assert!(!out.exists());
}

#[test]
fn mcp_nodes_rejects_an_oversized_selection_early_then_serves_the_next_call() {
    use super::Command;
    use figlens::Tree;
    use kiwi_schema::Value;
    use std::collections::HashMap;

    let frame = |name: String| {
        Value::Object(
            "Test",
            [
                ("type", Value::Enum("NodeType", "FRAME")),
                ("name", Value::String(name)),
            ]
            .into_iter()
            .collect(),
        )
    };
    let (large, small) = (
        frame("x".repeat(crate::mcp::MAX_RESPONSE_BYTES)),
        frame("small".into()),
    );
    let tree = Tree {
        nodes: HashMap::from([("0:1".into(), &large), ("0:2".into(), &small)]),
        children: HashMap::new(),
        blobs: Vec::new(),
    };
    // call_tool skips the wire-size check, so only the early guard can reject here.
    let fixture = std::path::Path::new("examples/first-success/design.fig");
    let call = |node: &str| {
        call_tool(
            &serde_json::json!({"name":"nodes","arguments":{"file":fixture,"node":node}}),
            Some((fixture, &tree)),
        )
    };
    let rejected = call("0:1");
    assert_eq!(
        rejected["_meta"]["figlens"]["code"],
        "response_budget_exceeded"
    );
    assert!(serde_json::to_vec(&rejected).unwrap().len() < 1024);
    let healthy = call("0:2");
    assert_eq!(healthy["isError"], false, "{healthy}");
    let doc: serde_json::Value =
        serde_json::from_str(healthy["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(doc, figlens::rest::resolve_nodes(&tree, "0:2").unwrap());
    // The CLI keeps full extraction.
    let cli = Command::from_cli("nodes", &["0:1".into()], &tree).unwrap();
    assert!(cli.execute(&tree, fixture).is_ok());
}

mod contract;

#[test]
fn cli_index_rejects_bad_arguments_before_touching_storage() {
    let index = |args: &[&str]| {
        let args = ["index"]
            .iter()
            .chain(args)
            .map(|a| a.to_string())
            .collect();
        run(args, &mut String::new()).unwrap_err().to_string()
    };
    assert!(index(&["status"]).contains("choose the workspace"));
    assert!(index(&["status", "--id", "ws_1", "--workspace", "/"]).contains("choose one of"));
    assert!(index(&["status", "--id"]).contains("--id needs a value"));
    assert!(index(&["status", "--id", "nope"]).contains("is not a workspace id"));
    assert!(
        index(&["status", "--id", "ws_0000000000000000", "--storage", "rel"]).contains("absolute")
    );
    assert!(index(&["prune", "--id", "ws_0000000000000000"]).contains("usage: figlens index"));
    assert!(index(&["status", "--bogus"]).contains("usage: figlens index"));
    assert!(
        index(&[
            "reset",
            "--id",
            "ws_0000000000000000",
            "--id",
            "ws_1111111111111111"
        ])
        .contains("given twice")
    );
    assert!(
        index(&["status", "--workspace", "--id", "ws_0000000000000000"])
            .contains("--workspace needs a value")
    );
}

#[test]
fn diff_text_says_a_stopped_side_was_not_compared_instead_of_no_changes() {
    let reply = |reasons: serde_json::Value| {
        serde_json::json!({"frames": [], "nextCursor": null,
            "coverage": {"complete": reasons.as_array().unwrap().is_empty(), "reasons": reasons}})
    };
    let stopped = super::diff_text(&reply(serde_json::json!([
        {"reason": "work_limit", "side": "old"},
        {"reason": "hidden_content_not_expanded", "side": "new", "count": 2},
        {"reason": "work_limit", "side": "new"}
    ])));
    assert!(!stopped.contains("no changes"), "{stopped}");
    assert!(
        stopped.starts_with("stopped early (old, new):") && stopped.contains("--node"),
        "{stopped}"
    );
    let partial = super::diff_text(&reply(
        serde_json::json!([{"reason": "definition_not_compared", "side": "new", "count": 1}]),
    ));
    assert!(
        partial.starts_with("no changes\ncoverage partial:"),
        "{partial}"
    );
    assert_eq!(
        super::diff_text(&reply(serde_json::json!([]))),
        "no changes\n"
    );
}

#[test]
fn cli_export_batch_prints_a_manifest_and_rejects_a_repeated_out_before_writing() {
    let dir = TestDir::new();
    let file = "examples/first-success/design.fig";
    let (a, b) = (dir.join("a.png"), dir.join("b.webp"));
    let batch = |items: serde_json::Value| {
        let list = dir.join("items.json");
        std::fs::write(&list, items.to_string()).unwrap();
        let mut text = String::new();
        run(
            vec![
                "export".into(),
                file.into(),
                "--batch".into(),
                list.to_string_lossy().into_owned(),
            ],
            &mut text,
        )
        .map(|()| serde_json::from_str::<serde_json::Value>(&text).unwrap())
    };
    assert!(batch(serde_json::json!([{"node":"0:1","out":a},{"node":"0:2","out":a}])).is_err());
    assert!(!a.exists());
    // A symlink to another item's file is the same output under a second name.
    #[cfg(unix)]
    {
        let alias = dir.join("alias.png");
        std::os::unix::fs::symlink(std::path::absolute(&a).unwrap(), &alias).unwrap();
        assert!(
            batch(serde_json::json!([{"node":"0:1","out":a},{"node":"0:2","out":alias}])).is_err()
        );
        assert!(!a.exists());
        std::fs::remove_file(&alias).unwrap();
    }
    let manifest = batch(serde_json::json!([
        {"node":"0:1","out":a,"scale":2},
        {"node":"0:4","out":b,"maxSize":40},
    ]))
    .unwrap();
    assert_eq!(manifest["failed"], 0);
    assert_eq!(manifest["items"][0]["width"], 640);
    assert_eq!(manifest["items"][1]["width"], 40);
    assert!(manifest["items"][1]["warnings"][0].is_string());
    assert_eq!(
        manifest["items"][1]["bytes"],
        std::fs::metadata(&b).unwrap().len()
    );
}

#[test]
fn compare_image_args_read_full_page() {
    let args: Vec<String> = ["0:1", "shot.png", "--full-page"].map(String::from).into();
    assert!(crate::command::compare_image_args(&args).unwrap().5);
}
