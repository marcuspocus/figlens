use super::*;
use crate::test_support;

fn allowed(path: &Path, roots: &[PathBuf]) -> bool {
    checked_path(path, roots).is_ok()
}
fn rooted_reply(line: &str, roots: &[PathBuf]) -> Value {
    match admit(line, roots) {
        Admission::Reply(reply) => reply,
        Admission::Call(call) => call.execute(None, roots, &[]),
        _ => panic!("expected reply"),
    }
}

#[test]
fn an_oversized_reply_is_bounded_before_it_waits_for_the_writer() {
    let (sender, replies) = mpsc::sync_channel(1);
    let text = "x".repeat(9 << 20);
    Writer(sender).send(serde_json::json!({"jsonrpc": "2.0", "id": 1,
            "result": {"content": [{"type": "text", "text": text}], "isError": false}}));
    assert!(replies.recv().unwrap().to_string().len() < 1 << 20);
}

#[test]
fn compact_nodes_reply_budget_counts_escaped_transport_text() {
    let compact = figlens::rest::compact::encode(
        serde_json::json!({
            "document": {"id":"1:1", "type":"TEXT", "characters":"\"".repeat(2 << 20)}
        }),
        false,
    );
    assert!(serde_json::to_vec(&compact).unwrap().len() < MAX_RESPONSE_BYTES);
    let result = tool_reply(
        "nodes",
        Ok(command::Output::Json {
            value: compact,
            pretty: false,
        }),
    );
    let (sender, replies) = mpsc::sync_channel(1);
    Writer(sender).send(response(&serde_json::json!(1), result));
    let reply = replies.recv().unwrap();
    assert_eq!(
        reply["result"]["_meta"]["figlens"]["code"],
        "response_budget_exceeded"
    );
}

#[test]
fn index_and_jobs_declare_that_they_change_state() {
    let tools = contract::tools();
    let hints = |name: &str| {
        tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()["annotations"]
            .clone()
    };
    assert_eq!(
        hints("index"),
        serde_json::json!({"readOnlyHint": false, "destructiveHint": true,
                "idempotentHint": false, "openWorldHint": false})
    );
    assert_eq!(
        hints("jobs"),
        serde_json::json!({"readOnlyHint": false, "destructiveHint": false, "openWorldHint": false})
    );
}

#[test]
fn freshness_is_a_read_only_open_world_tool_over_a_mapping_or_one_file() {
    let reply = mcp_reply(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
    let tool = reply["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "freshness")
        .unwrap();
    let mut params: Vec<_> = tool["inputSchema"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    params.sort();
    assert_eq!(
        (tool["annotations"].clone(), params),
        (
            serde_json::json!({"readOnlyHint": true, "openWorldHint": true}),
            vec![
                "account".to_owned(),
                "file".to_owned(),
                "sources".to_owned(),
                "url".to_owned()
            ]
        )
    );
}

#[test]
fn mcp_lists_every_tool() {
    let reply = mcp_reply(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
    let names: Vec<_> = reply["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "workflow",
            "tree",
            "find",
            "search",
            "usages",
            "diff",
            "inspect",
            "nodes",
            "variables",
            "bundle",
            "verify",
            "freshness",
            "export",
            "export_batch",
            "compare_image",
            "index",
            "jobs",
            "models",
            "changes",
            "watch"
        ]
    );
}

#[test]
fn mcp_workflows_work_without_design_files_under_restricted_roots() {
    let dir = test_support::TestDir::new();
    let roots = [dir.join("").canonicalize().unwrap()];
    let mut requests = vec![
        serde_json::json!({"id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"id": 2, "method": "tools/list"}),
    ];
    for topic in workflow::TOPICS {
        requests.push(
            serde_json::json!({"id": topic, "method": "tools/call", "params": {
                "name": "workflow", "arguments": {"topic": topic}
            }}),
        );
    }
    let input = requests
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = Vec::new();
    serve(std::io::Cursor::new(input), &mut out, &roots).unwrap();
    let replies: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let instructions = replies[0]["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("workflow(topic=build)"));
    assert!(instructions.contains("workflow(topic=audit)"));
    assert!(instructions.contains("Simple questions and exports"));
    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    let tool = tools.iter().find(|t| t["name"] == "workflow").unwrap();
    assert_eq!(
        tool["inputSchema"]["required"],
        serde_json::json!(["topic"])
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["topic"]["enum"],
        serde_json::json!(workflow::TOPICS)
    );
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert!(
        tool["description"]
            .as_str()
            .unwrap()
            .contains("audit for comparing existing UI")
    );
    assert!(workflow::TOPICS.contains(&"audit"));
    for reply in &replies[2..] {
        assert_eq!(reply["result"]["isError"], false, "{reply}");
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("host's own tools"));
        assert!(!text.contains("](references/") && !text.contains("](planning"));
        assert!(!text.contains("](../SKILL.md"));
        if reply["id"] == "audit" {
            assert!(text.contains("# Audit an existing UI"));
            assert!(text.contains("Static review is not visual acceptance"));
        }
    }
    assert_eq!(std::fs::read_dir(dir.join("")).unwrap().count(), 0);
}

#[test]
fn mcp_workflow_rejects_invalid_arguments_without_cli_fallback() {
    for arguments in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!({"topic": 1}),
        serde_json::json!({"topic": "../SKILL.md"}),
        serde_json::json!({"topic": "build", "file": "/private/design.fig"}),
    ] {
        let request = serde_json::json!({"id": 1, "method": "tools/call", "params": {
            "name": "workflow", "arguments": arguments
        }});
        assert!(matches!(
            admit(&request.to_string(), &[]),
            Admission::Reply(_)
        ));
        let result = call_tool(&request["params"], None);
        assert_eq!(result["isError"], true, "{result}");
        assert!(
            !result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("usage:")
        );
    }
}

#[test]
fn tree_json_nests_children_down_to_the_depth() {
    use kiwi_schema::Value;
    use std::collections::HashMap;
    let g = |l| {
        Value::Object(
            "GUID",
            HashMap::from([("sessionID", Value::UInt(0)), ("localID", Value::UInt(l))]),
        )
    };
    let node = |l: u32, parent: Option<u32>| {
        let mut fields = HashMap::from([
            ("guid", g(l)),
            ("type", Value::Enum("NodeType", "FRAME")),
            ("name", Value::String(format!("n{l}"))),
        ]);
        if let Some(p) = parent {
            fields.insert(
                "parentIndex",
                Value::Object(
                    "ParentIndex",
                    HashMap::from([("guid", g(p)), ("position", Value::String("a".into()))]),
                ),
            );
        }
        Value::Object("NodeChange", fields)
    };
    let message = Value::Object(
        "Message",
        HashMap::from([(
            "nodeChanges",
            Value::Array(vec![node(0, None), node(1, Some(0)), node(2, Some(1))]),
        )]),
    );
    let tree = Tree::new(&message);
    assert_eq!(
        command::Command::from_cli(
            "tree",
            &["--depth".into(), "1".into(), "--json".into()],
            &tree
        )
        .unwrap()
        .execute(&tree, Path::new("unused.fig"))
        .unwrap()
        .json()
        .unwrap(),
        &serde_json::json!({ "id": "0:0", "type": "FRAME", "name": "n0",
                "children": [{ "id": "0:1", "type": "FRAME", "name": "n1" }] })
    );
}

#[test]
fn tree_prints_the_rest_types_nodes_uses() {
    let message = test_support::component_set();
    let tree = Tree::new(&message);
    let run = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        command::Command::from_cli("tree", &args, &tree)
            .unwrap()
            .execute(&tree, Path::new("unused.fig"))
            .unwrap()
    };
    assert_eq!(
        run(&["--depth", "2", "--json"]).json().unwrap(),
        &serde_json::json!({ "id": "0:0", "type": "CANVAS", "name": "n0",
                "children": [{ "id": "0:1", "type": "COMPONENT_SET", "name": "n1",
                    "layoutMode": "HORIZONTAL",
                    "children": [{ "id": "0:2", "type": "COMPONENT", "name": "n2", "visible": false,
                        "size": [24.0, 16.0] }] }] })
    );
    let text = run(&["--depth", "2"]).text().unwrap();
    assert!(text.contains("0:1 COMPONENT_SET \"n1\"") && text.contains("0:2 COMPONENT \"n2\""));
}

#[test]
fn tree_json_gives_top_level_nodes_their_first_visible_text() {
    use kiwi_schema::Value;
    use std::collections::HashMap;
    let g = |l| {
        Value::Object(
            "GUID",
            HashMap::from([("sessionID", Value::UInt(0)), ("localID", Value::UInt(l))]),
        )
    };
    let node = |l: u32, parent: u32, kind, text: Option<&str>, visible: bool| {
        let mut fields = HashMap::from([
            ("guid", g(l)),
            ("type", Value::Enum("NodeType", kind)),
            ("name", Value::String(format!("n{l}"))),
            ("visible", Value::Bool(visible)),
            (
                "parentIndex",
                Value::Object(
                    "ParentIndex",
                    HashMap::from([
                        ("guid", g(parent)),
                        ("position", Value::String(format!("{l:02}"))),
                    ]),
                ),
            ),
        ]);
        if let Some(text) = text {
            let chars = HashMap::from([("characters", Value::String(text.into()))]);
            fields.insert("textData", Value::Object("TextData", chars));
        }
        Value::Object("NodeChange", fields)
    };
    let message = Value::Object(
        "Message",
        HashMap::from([(
            "nodeChanges",
            Value::Array(vec![
                node(1, 0, "CANVAS", None, true),
                // A canvas label: a box around one line of text.
                node(2, 1, "FRAME", None, true),
                node(3, 2, "TEXT", Some("Onboarding"), true),
                // A screen whose hidden text is skipped for the next visible one, deeper down.
                node(4, 1, "FRAME", None, true),
                node(5, 4, "TEXT", Some("hidden"), false),
                node(6, 4, "FRAME", None, true),
                node(7, 6, "TEXT", Some(&"x".repeat(250)), true),
                node(8, 1, "FRAME", None, true),
                // Inside a section, blank text is passed over for the next visible text.
                node(9, 1, "SECTION", None, true),
                node(10, 9, "FRAME", None, true),
                node(11, 10, "TEXT", Some(" \n"), true),
                node(12, 10, "TEXT", Some("Checkout"), true),
            ]),
        )]),
    );
    let tree = Tree::new(&message);
    let run = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        command::Command::from_cli("tree", &args, &tree)
            .unwrap()
            .execute(&tree, Path::new("unused.fig"))
            .unwrap()
            .json()
            .unwrap()
            .clone()
    };
    let page = run(&["0:1", "--depth", "2", "--json"]);
    let first = |v: &serde_json::Value| v.get("firstText").cloned();
    assert_eq!(first(&page["children"][0]), Some("Onboarding".into()));
    assert_eq!(first(&page["children"][1]), Some("x".repeat(200).into()));
    // Only children of a page or section carry it; a node without text has none.
    assert_eq!(first(&page["children"][0]["children"][0]), None);
    assert_eq!(first(&page["children"][2]), None);
    let section = run(&["0:9", "--depth", "1", "--json"]);
    assert_eq!(first(&section["children"][0]), Some("Checkout".into()));
    // Scoped to the frame itself, it keeps the text it has in the page listing.
    assert_eq!(first(&run(&["0:2", "--json"])), Some("Onboarding".into()));
    assert_eq!(first(&run(&["0:3", "--json"])), None);
    let frames = run(&["0:1", "--top-frames", "--json"]);
    assert_eq!(first(&frames[0]), Some("Onboarding".into()));
}

#[test]
fn tree_top_frames_lists_screens_in_pages_and_sections_for_frame_lists() {
    use kiwi_schema::Value;
    use std::collections::HashMap;
    let g = |l| {
        Value::Object(
            "GUID",
            HashMap::from([("sessionID", Value::UInt(0)), ("localID", Value::UInt(l))]),
        )
    };
    let node = |l: u32, parent: Option<u32>, kind, extra: Option<&'static str>| {
        let mut fields = HashMap::from([
            ("guid", g(l)),
            ("type", Value::Enum("NodeType", kind)),
            ("name", Value::String(format!("n{l}"))),
        ]);
        if let Some(p) = parent {
            let position = Value::String(format!("{l:02}"));
            fields.insert(
                "parentIndex",
                Value::Object(
                    "ParentIndex",
                    HashMap::from([("guid", g(p)), ("position", position)]),
                ),
            );
        }
        match extra {
            Some("newline") => {
                fields.insert("name", Value::String("a\n0:9 b".into()));
            }
            Some(flag) => {
                fields.insert(flag, Value::Bool(true));
            }
            None => {}
        }
        Value::Object("NodeChange", fields)
    };
    let message = Value::Object(
        "Message",
        HashMap::from([(
            "nodeChanges",
            Value::Array(vec![
                node(0, None, "DOCUMENT", None),
                node(1, Some(0), "CANVAS", None),
                node(2, Some(1), "FRAME", None),
                node(3, Some(1), "SECTION", None),
                node(4, Some(3), "SYMBOL", None),
                node(5, Some(3), "SECTION", None),
                node(6, Some(5), "INSTANCE", None),
                node(7, Some(1), "FRAME", Some("resizeToFit")),
                node(8, Some(7), "FRAME", None),
                node(9, Some(1), "TEXT", None),
                node(10, Some(2), "FRAME", None),
                node(11, Some(0), "CANVAS", Some("internalOnly")),
                node(12, Some(11), "FRAME", None),
                node(13, Some(3), "FRAME", Some("newline")),
                node(14, Some(99), "SECTION", None),
                node(15, Some(14), "FRAME", None),
            ]),
        )]),
    );
    let tree = Tree::new(&message);
    let run = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        command::Command::from_cli("tree", &args, &tree)
            .unwrap()
            .execute(&tree, Path::new("unused.fig"))
            .unwrap()
    };
    let text = run(&["--top-frames", "--depth", "0"]).text().unwrap();
    assert_eq!(
        text,
        "0:2 FRAME \"n2\"\n0:4 COMPONENT \"n4\"\n0:6 INSTANCE \"n6\"\n0:13 FRAME \"a\\n0:9 b\"\n"
    );
    // An escaped newline in a name does not add an id line.
    assert_eq!(crate::frame_ids(&text), ["0:2", "0:4", "0:6", "0:13"]);
    assert_eq!(
        run(&["--top-frames", "--json"]).json().unwrap(),
        &serde_json::json!([
            { "id": "0:2", "type": "FRAME", "name": "n2", "page": "0:1" },
            { "id": "0:4", "type": "COMPONENT", "name": "n4", "page": "0:1", "section": "0:3" },
            { "id": "0:6", "type": "INSTANCE", "name": "n6", "page": "0:1", "section": "0:5" },
            { "id": "0:13", "type": "FRAME", "name": "a\n0:9 b", "page": "0:1", "section": "0:3" },
        ])
    );
    // A section under a parent the file lacks lists its screens without a page.
    assert_eq!(
        run(&["0:14", "--top-frames", "--json"]).json().unwrap(),
        &serde_json::json!([{ "id": "0:15", "type": "FRAME", "name": "n15", "section": "0:14" }])
    );
    // A section root keeps its page; a frame root is not a page or section and lists nothing.
    let section = run(&["0:3", "--top-frames", "--json"])
        .json()
        .unwrap()
        .clone();
    assert_eq!(
        (&section[0]["page"], &section[0]["section"]),
        (&"0:1".into(), &"0:3".into())
    );
    assert_eq!(run(&["0:2", "--top-frames"]).text().unwrap(), "");
    // An internal page lists its screens when it is the root.
    assert_eq!(
        run(&["0:11", "--top-frames"]).text().unwrap(),
        "0:12 FRAME \"n12\"\n"
    );
    let file = std::fs::canonicalize("examples/first-success/design.fig").unwrap();
    let reply = call_tool(
        &serde_json::json!({"name":"tree","arguments":{"file":file,"topFrames":true,"json":true}}),
        None,
    );
    let frames: serde_json::Value =
        serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(frames[0]["id"], "0:1", "{reply}");
}

#[test]
fn mcp_diff_checks_both_paths_and_pages_the_cli_reply() {
    let inside = test_support::TestDir::new();
    let outside = test_support::TestDir::new();
    let roots = [std::fs::canonicalize(inside.join("")).unwrap()];
    for dir in [&inside, &outside] {
        std::fs::copy("examples/first-success/design.fig", dir.join("a.fig")).unwrap();
    }
    let call = |old: &Path, new: &Path| {
        let line = json!({"id":1,"method":"tools/call","params":{"name":"diff","arguments":{"old":old,"file":new}}});
        rooted_reply(&line.to_string(), &roots)["result"].clone()
    };
    let (a, out) = (inside.join("a.fig"), outside.join("a.fig"));
    let same = call(&a, &a);
    let value = &same["structuredContent"];
    assert_eq!(
        (&value["frames"], &value["coverage"]["complete"]),
        (&json!([]), &json!(true)),
        "{same}"
    );
    assert_eq!(
        value["matching"],
        json!({"basis": "node_id+kind", "lineage": "unverified"})
    );
    // The old file is admitted like `file`: outside the roots it is refused before any read.
    for (old, new) in [(&out, &a), (&a, &out)] {
        assert_eq!(
            call(old, new)["_meta"]["figlens"]["code"],
            "permission_denied"
        );
    }
    let gs = json!({"id":1,"method":"tools/call","params":{"name":"diff","arguments":{"old":"gs://b/o.fig","file":a}}});
    assert_eq!(
        rooted_reply(&gs.to_string(), &[])["result"]["_meta"]["figlens"]["code"],
        "invalid_arguments"
    );
}

#[test]
fn mcp_ignores_notifications() {
    assert!(mcp_reply(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
}

#[test]
fn roots_confine_files_and_outputs() {
    let inside = test_support::TestDir::new();
    let outside = test_support::TestDir::new();
    let roots = [std::fs::canonicalize(inside.join("")).unwrap()];
    assert!(allowed(&inside.join("a.fig"), &roots));
    assert!(!allowed(&outside.join("a.fig"), &roots));
    assert!(!allowed(&inside.join("../escape.fig"), &roots));
    assert!(!allowed(Path::new("gs://bucket/a.fig"), &roots));
    assert!(allowed(&outside.join("a.fig"), &[]));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.join("new.png"), inside.join("link.png")).unwrap();
        assert!(!allowed(&inside.join("link.png"), &roots));
    }
    // Argument failures precede path admission even with roots.
    let malformed = r#"{"id":3,"method":"tools/call","params":{"name":"tree","arguments":{"node":"outside.fig"}}}"#;
    assert!(matches!(admit(malformed, &roots), Admission::Reply(_)));
    assert_eq!(
        mcp_reply(malformed).unwrap()["result"]["_meta"]["figlens"]["code"],
        "invalid_arguments"
    );
    let legacy = serde_json::json!({"id":3,"method":"tools/call","params":{"name":"tree",
            "arguments":{"file":inside.join("a.fig"),"depth":"0"}}})
    .to_string();
    assert_eq!(
        rooted_reply(&legacy, &roots)["result"]["_meta"]["figlens"]["code"],
        "invalid_arguments"
    );

    let call = |file: &Path| {
        format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"tree","arguments":{{"file":{}}}}}}}"#,
            serde_json::json!(file)
        )
    };
    let input = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{{}}}}\n{}\n{}\n",
        call(&outside.join("a.fig")),
        call(&inside.join("a.fig"))
    );
    let mut out = Vec::new();
    serve(input.as_bytes(), &mut out, &roots).unwrap();
    let replies: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let instructions = replies[0]["result"]["instructions"].as_str().unwrap();
    assert!(
        instructions.contains(&roots[0].display().to_string()),
        "{instructions}"
    );
    let text = replies[1]["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(replies[1]["result"]["isError"], true);
    assert!(text.contains("outside the folders"), "{text}");
    // Inside the root the call runs and fails on the missing file instead.
    let text = replies[2]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(!text.contains("outside the folders"), "{text}");
}

#[test]
fn serve_answers_every_call_when_the_file_cannot_be_opened() {
    let call = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tree","arguments":{"file":"/missing.fig"}}}"#;
    let input = format!("{call}\n{call}\n{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}}\n");
    let mut out = Vec::new();
    serve(input.as_bytes(), &mut out, &[]).unwrap();
    let replies: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies.len(), 3);
    let calls: Vec<_> = replies.iter().filter(|r| r["id"] == 1).collect();
    assert!(calls.len() == 2 && calls.iter().all(|r| r["result"]["isError"] == true));
    assert!(
        replies
            .iter()
            .any(|r| r["id"] == 2 && r["result"] == serde_json::json!({}))
    );
}

#[test]
fn mcp_reports_a_failed_call_as_a_tool_error() {
    let reply = mcp_reply(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"tree","arguments":{"file":"/missing.fig"}}}"#,
    )
    .unwrap();
    assert_eq!(reply["result"]["isError"], true);
}

#[test]
fn mcp_models_configure_refuses_a_model_dir_outside_roots() {
    let dir = test_support::TestDir::new();
    let inside = dir.join("inside");
    std::fs::create_dir(&inside).unwrap();
    let roots = [inside.canonicalize().unwrap()];
    let input = serde_json::json!({"id": 1, "method": "tools/call", "params": {"name": "models",
        "arguments": {"action": "configure", "profile": "bge-m3", "modelDir": dir.join("").canonicalize().unwrap()}}});
    let mut out = Vec::new();
    serve(std::io::Cursor::new(input.to_string()), &mut out, &roots).unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(reply["result"]["isError"], true);
    assert!(reply.to_string().contains("permission_denied"), "{reply}");
}

#[cfg(unix)]
#[test]
fn mcp_models_configure_refuses_a_model_file_linked_outside_roots() {
    let dir = test_support::TestDir::new();
    let inside = dir.join("inside");
    std::fs::create_dir(&inside).unwrap();
    std::fs::write(dir.join("outside.bin"), "weights").unwrap();
    for (name, _) in figlens::semantic::profile::known("bge-m3").unwrap().files {
        std::fs::write(inside.join(name), "x").unwrap();
    }
    std::fs::remove_file(inside.join("pytorch_model.bin")).unwrap();
    std::os::unix::fs::symlink(
        dir.join("outside.bin").canonicalize().unwrap(),
        inside.join("pytorch_model.bin"),
    )
    .unwrap();
    let roots = [inside.canonicalize().unwrap()];
    let input = serde_json::json!({"id": 1, "method": "tools/call", "params": {"name": "models",
        "arguments": {"action": "configure", "profile": "bge-m3", "modelDir": roots[0]}}});
    let mut out = Vec::new();
    serve(std::io::Cursor::new(input.to_string()), &mut out, &roots).unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(reply.to_string().contains("permission_denied"), "{reply}");
}

#[test]
fn a_missing_output_folder_inside_a_root_is_named_not_called_outside() {
    let inside = test_support::TestDir::new();
    let outside = test_support::TestDir::new();
    let roots = [std::fs::canonicalize(inside.join("")).unwrap()];
    let export = |out: PathBuf| {
        let line = serde_json::json!({"id":1,"method":"tools/call","params":{"name":"export",
            "arguments":{"file":inside.join("a.fig"),"node":"1:2","out":out}}});
        rooted_reply(&line.to_string(), &roots)["result"].clone()
    };
    let missing = export(inside.join("new/deeper/x.png"));
    assert_eq!(
        missing["_meta"]["figlens"]["code"], "output_write_failed",
        "{missing}"
    );
    let text = missing["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("deeper does not exist") && !text.contains("outside the folders"),
        "{text}"
    );
    // A source in a missing folder is not something to create.
    let find = serde_json::json!({"id":1,"method":"tools/call","params":{"name":"find",
        "arguments":{"file":inside.join("new/a.fig"),"query":"x"}}});
    assert_eq!(
        rooted_reply(&find.to_string(), &roots)["result"]["_meta"]["figlens"]["code"],
        "permission_denied"
    );
    for out in [outside.join("new/x.png"), inside.join("new/../../x/y.png")] {
        assert_eq!(export(out)["_meta"]["figlens"]["code"], "permission_denied");
    }
}

#[test]
fn nodes_out_stays_inside_the_roots_and_off_the_source() {
    let inside = test_support::TestDir::new();
    let outside = test_support::TestDir::new();
    let source = inside.join("design.fig");
    std::fs::copy("examples/first-success/design.fig", &source).unwrap();
    let before = std::fs::read(&source).unwrap();
    let roots = [std::fs::canonicalize(inside.join("")).unwrap()];
    let nodes = |out: PathBuf| {
        let line = serde_json::json!({"id":1,"method":"tools/call","params":{"name":"nodes",
            "arguments":{"file":source,"node":"0:1","out":out}}});
        rooted_reply(&line.to_string(), &roots)["result"].clone()
    };
    let escaped = nodes(outside.join("nodes.json"));
    assert_eq!(
        escaped["_meta"]["figlens"]["code"], "permission_denied",
        "{escaped}"
    );
    assert_eq!(nodes(source.clone())["isError"], true);
    let other = inside.join("other.fig");
    std::fs::write(&other, b"design").unwrap();
    assert_eq!(
        nodes(other.clone())["_meta"]["figlens"]["code"],
        "invalid_arguments"
    );
    #[cfg(unix)]
    {
        let link = inside.join("link.json");
        std::os::unix::fs::symlink("other.fig", &link).unwrap();
        assert_eq!(nodes(link)["_meta"]["figlens"]["code"], "invalid_arguments");
    }
    assert_eq!(std::fs::read(&other).unwrap(), b"design");
    assert_eq!(std::fs::read(&source).unwrap(), before);
}
