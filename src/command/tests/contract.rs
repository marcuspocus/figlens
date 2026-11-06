use super::*;
use crate::{TOOLS, contract, mcp_reply};
use serde_json::{Value, json};

fn check(tool: &str, args: Value) -> Result<contract::PreparedCall, contract::Failure> {
    contract::prepare(tool, &args)
}

/// Keep only the parameters this tool accepts, so one shared argument set fits every tool.
fn own(tool: &str, mut args: Value) -> Value {
    let params = TOOLS.iter().find(|t| t.0 == tool).unwrap().2;
    args.as_object_mut()
        .unwrap()
        .retain(|key, _| params.iter().any(|p| p.0 == key));
    if tool == "export_batch" {
        args["items"] = json!([{"node": "0:1", "out": "out.png"}]);
    }
    if tool == "variables" {
        args.as_object_mut().unwrap().remove("out");
    }
    if tool == "compare_image" {
        args["image"] = json!("absent.png");
    }
    args
}

#[test]
fn schema_and_numeric_preflight_share_boundaries() {
    let list = mcp_reply(r#"{"id":1,"method":"tools/list"}"#).unwrap();
    let tools = list["result"]["tools"].as_array().unwrap();
    for (tool, key, min, max) in [
        ("inspect", "depth", 0, 10),
        ("find", "limit", 1, 200),
        ("search", "limit", 1, 200),
        ("export", "quality", 1, 100),
        ("export", "maxSize", 0, u64::from(u32::MAX)),
        ("tree", "depth", 0, (1 << 53) - 1),
        ("export", "maxKb", 0, (1 << 53) - 1),
        ("compare_image", "limit", 1, 50),
        ("compare_image", "threshold", 1, 254),
    ] {
        let schema = &tools.iter().find(|t| t["name"] == tool).unwrap()["inputSchema"];
        assert_eq!(schema["properties"][key]["minimum"], min);
        assert_eq!(schema["properties"][key]["maximum"], max);
        let mut args = own(
            tool,
            json!({"file":"absent.fig", "node":"0:1", "query":"Frame", "out":"out.png", "bundle":"absent.zip"}),
        );
        for value in [min, max] {
            args[key] = json!(value);
            assert!(check(tool, args.clone()).is_ok()); // No expensive rendering at large bounds.
        }
        for value in [
            json!(-1),
            json!(1.5),
            json!(max + 1),
            json!("invalid"),
            json!([]),
        ] {
            args[key] = value;
            let reply = call_tool(&json!({"name":tool,"arguments":args}), None);
            assert_eq!(
                reply["_meta"]["figlens"]["code"], "invalid_arguments",
                "{reply}"
            );
        }
    }
    assert_eq!(
        check("tree", json!({"file":"a.fig","depth":2.0}))
            .unwrap_or_else(|_| panic!())
            .argument("depth"),
        2
    );
    for args in [
        json!({"scale":0}),
        json!({"scale":-1}),
        json!({"scale":"NaN"}),
        json!({"scale":"inf"}),
        json!({"maxKb":u64::MAX}),
    ] {
        let mut all = json!({"file":"absent.fig","node":"0:1","out":"out.png"});
        all.as_object_mut()
            .unwrap()
            .extend(args.as_object().unwrap().clone());
        assert!(check("export", all).is_err());
    }
}

#[test]
fn strict_types_unknowns_and_flag_like_nodes_are_explicit() {
    for args in [
        json!({}),
        json!({"file":null}),
        json!({"file":42}),
        json!({"file":""}),
        json!({"file":"a.fig","json":"false"}),
        json!({"file":"a.fig","depth":"0"}),
        json!({"file":"a.fig","node":false}),
        json!({"file":"a.fig","depth":null}),
        json!({"file":"a.fig","futureOption":42}),
        json!({"file":"a.fig","nodeId":"0:1"}),
    ] {
        assert!(check("tree", args).is_err());
    }
    for args in [
        json!({"file":"a.fig","json":false}),
        json!({"file":"a.fig","node":"--json"}),
    ] {
        assert!(check("tree", args).is_ok());
    }
    for (key, value) in [
        ("scale", json!("2x")),
        ("maxSize", Value::Null),
        ("maxSize", json!(false)),
    ] {
        let mut args = json!({"file":"a.fig","node":"0:1","out":"a.png"});
        args[key] = value;
        assert!(check("export", args).is_err());
    }
    assert!(check("workflow", json!({"topic":"build","extra":true})).is_err());
    assert!(check("workflow", json!({"topic":null})).is_err());
    assert!(check("workflow", json!({"topic":"missing"})).is_err());
    assert!(check("nodes", json!({"file":"a.fig","node":null})).is_err());
}

#[test]
fn advertised_numbers_fit_json_safe_integers() {
    // Model backends parse tool schemas as int64 or JavaScript numbers; one larger value rejects every tool.
    fn walk(value: &Value) {
        match value {
            Value::Number(n) => assert!(n.as_u64().is_none_or(|n| n < 1 << 53), "{n}"),
            Value::Array(items) => items.iter().for_each(walk),
            Value::Object(map) => map.values().for_each(walk),
            _ => {}
        }
    }
    walk(&mcp_reply(r#"{"id":1,"method":"tools/list"}"#).unwrap()["result"]["tools"]);
}

#[test]
fn discovery_and_preparation_share_defaults() {
    for (name, _, params) in TOOLS {
        let args = if *name == "workflow" {
            json!({"topic":"build"})
        } else if matches!(*name, "changes" | "watch") {
            own(
                name,
                json!({"action":"ack","workspaceId":"ws_0000000000000000","consumer":"bot","cursor":"c1.00"}),
            )
        } else if *name == "nodes" {
            own(name, json!({"file":"a.fig","node":"0:1","out":"a.json"}))
        } else {
            own(
                name,
                json!({"file":"a.fig","old":"a.fig","node":"0:1","query":"Frame","out":"a.png","bundle":"a.zip","action":"status","jobId":"ws_0000000000000000.0"}),
            )
        };
        let call = check(name, args.clone()).unwrap();
        for param in *params {
            let property = contract::property(name, param);
            if let Some(default) = property.get("default") {
                let mut omitted = args.clone();
                omitted.as_object_mut().unwrap().remove(param.0);
                assert_eq!(
                    check(name, omitted).unwrap().argument(param.0),
                    default,
                    "{name}.{}",
                    param.0
                );
            }
        }
        assert_eq!(call.name(), *name);
    }
    let call = check("tree", json!({"file":"a.fig"})).unwrap();
    assert_eq!(call.argument("depth"), usize::MAX);
    assert!(call.string("out").is_none());
    let call = check(
        "export",
        json!({"file":"a.fig","node":"0:1","out":"a.png","maxSize":0}),
    )
    .unwrap();
    assert_eq!(call.argument("maxSize"), 0);
}

#[test]
fn inspect_states_is_an_optional_strict_boolean() {
    let args = json!({"file":"absent.fig", "node":"0:1"});
    assert_eq!(
        check("inspect", args.clone()).unwrap().argument("states"),
        false
    );
    for value in [
        json!(true),
        json!(false),
        json!("true"),
        json!(1),
        Value::Null,
    ] {
        let mut args = args.clone();
        args["states"] = value.clone();
        assert_eq!(check("inspect", args).is_ok(), value.is_boolean());
    }
    let list = mcp_reply(r#"{"id":1,"method":"tools/list"}"#).unwrap();
    let tool = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "inspect")
        .unwrap();
    assert_eq!(
        tool["inputSchema"]["properties"]["states"]["default"],
        false
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["states"]["type"],
        "boolean"
    );
    assert_eq!(
        tool["outputSchema"]["properties"]["states"]["type"],
        "object"
    );
    assert!(
        !tool["outputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("states"))
    );
}

#[test]
fn compare_variant_is_an_optional_nonempty_string() {
    let base = json!({"file":"absent.fig","node":"0:10"});
    assert!(
        check("inspect", base.clone())
            .unwrap()
            .string("compareVariant")
            .is_none()
    );
    for value in [
        json!("0:11"),
        json!(""),
        json!(true),
        json!(42),
        json!([]),
        Value::Null,
    ] {
        let mut args = base.clone();
        args["compareVariant"] = value.clone();
        assert_eq!(
            check("inspect", args).is_ok(),
            value.as_str().is_some_and(|s| !s.is_empty())
        );
    }
    let schema = contract::output_schema("inspect").unwrap();
    assert_eq!(schema["properties"]["variantComparison"]["type"], "object");
    assert_eq!(schema["properties"]["comparison"]["type"], "object");
    assert!(
        check(
            "inspect",
            json!({"file":"absent.fig","node":"0:10","compare":""})
        )
        .is_err()
    );
    assert!(
        check(
            "inspect",
            json!({"file":"absent.fig","node":"0:10","compare":"0:11"})
        )
        .is_ok()
    );
}

#[test]
fn flat_is_a_strict_boolean_and_match_dir_a_nonempty_string() {
    let inspect = json!({"file":"absent.fig","node":"0:10"});
    let export = json!({"file":"absent.fig","node":"0:10","out":"a.svg"});
    for value in [
        json!(true),
        json!(false),
        json!("true"),
        json!(1),
        Value::Null,
    ] {
        for key in ["flat", "fullText", "colors"] {
            let mut args = inspect.clone();
            args[key] = value.clone();
            assert_eq!(check("inspect", args).is_ok(), value.is_boolean());
        }
    }
    for value in [json!("icons"), json!(""), json!(true), Value::Null] {
        let mut args = export.clone();
        args["matchDir"] = value.clone();
        assert_eq!(
            check("export", args).is_ok(),
            value.as_str().is_some_and(|s| !s.is_empty())
        );
    }
    // A flat reply has no `node`, so the inspect schema may only require `truncated`.
    let schema = contract::output_schema("inspect").unwrap();
    assert_eq!(schema["required"], json!(["truncated"]));
    assert_eq!(schema["properties"]["flat"]["type"], "string");
}

#[test]
fn bundle_states_is_an_optional_strict_boolean() {
    let base = json!({"file":"absent.fig", "node":"0:1", "out":"out.zip"});
    assert_eq!(
        check("bundle", base.clone()).unwrap().argument("states"),
        false
    );
    for value in [
        json!(true),
        json!(false),
        json!("true"),
        json!(1),
        Value::Null,
    ] {
        let mut args = base.clone();
        args["states"] = value.clone();
        assert_eq!(check("bundle", args).is_ok(), value.is_boolean());
    }
}

#[test]
fn stable_error_codes_preserve_tool_recovery_and_redact_sources() {
    let sensitive = contract::source(
        "gs://example/object?signature=hidden",
        "cloud error signature=hidden".into(),
    );
    assert!(!sensitive.to_string().contains("hidden"));
    let fixture = Fixture::new();
    for (name, arguments, code) in [
        (
            "inspect",
            json!({"file":fixture.file(),"node":"9:9"}),
            "node_not_found",
        ),
        (
            "export",
            json!({"file":fixture.file(),"node":"0:1","out":fixture.0.join("missing/out.png")}),
            "output_write_failed",
        ),
        (
            "bundle",
            json!({"file":fixture.file(),"node":"0:1","out":fixture.0.join("missing/out.zip")}),
            "output_write_failed",
        ),
        (
            "tree",
            json!({"file":"https://invalid.test/design.fig?signature=do-not-echo"}),
            "source_unreadable",
        ),
    ] {
        let reply = call_tool(&json!({"name":name,"arguments":arguments}), None);
        assert_eq!(reply["isError"], true);
        assert_eq!(reply["_meta"]["figlens"]["code"], code, "{reply}");
        assert!(!reply.to_string().contains("do-not-echo"));
        assert!(reply["_meta"]["figlens"]["nextStep"].is_string());
    }
    let reply = call_tool(
        &json!({"name":"inspect","arguments":{"file":fixture.file(),"node":"0:1"}}),
        None,
    );
    assert_eq!(reply["isError"], false);
    assert!(reply.get("_meta").is_none());
}

#[test]
fn protocol_failures_are_not_business_failures() {
    for (request, code) in [
        ("{", -32700),
        ("[]", -32600),
        (r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#, -32600),
        (r#"{"id":7,"method":"missing"}"#, -32601),
        (r#"{"id":7,"method":"tools/call","params":{}}"#, -32602),
    ] {
        assert_eq!(mcp_reply(request).unwrap()["error"]["code"], code);
    }
    let reply =
        mcp_reply(r#"{"id":7,"method":"tools/call","params":{"name":"tree","arguments":{}}}"#)
            .unwrap();
    assert_eq!(reply["id"], 7);
    assert!(reply.get("error").is_none());
    assert_eq!(
        reply["result"]["_meta"]["figlens"]["code"],
        "invalid_arguments"
    );
}

#[test]
fn watch_start_and_stop_name_their_missing_argument() {
    for (args, missing) in [
        (
            r#"{"action":"start","workspaceId":"ws_0000000000000000"}"#,
            "watch start needs scope",
        ),
        (
            r#"{"action":"stop","workspaceId":"ws_0000000000000000"}"#,
            "watch stop needs leaseId",
        ),
    ] {
        let reply = mcp_reply(&format!(
            r#"{{"id":1,"method":"tools/call","params":{{"name":"watch","arguments":{args}}}}}"#
        ))
        .unwrap();
        let text = reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(text.contains(missing), "{missing}: {reply}");
    }
}

#[test]
fn every_tool_rejects_missing_required_fields_and_wrong_types_before_io() {
    for (name, _, params) in TOOLS {
        let valid = if *name == "workflow" {
            json!({"topic":"build"})
        } else if matches!(*name, "changes" | "watch") {
            own(
                name,
                json!({"action":"ack","workspaceId":"ws_0000000000000000","consumer":"bot","cursor":"c1.00"}),
            )
        } else if *name == "nodes" {
            own(
                name,
                json!({"file":"absent.fig","node":"0:1","out":"out.json"}),
            )
        } else {
            own(
                name,
                json!({"file":"absent.fig","old":"absent.fig","node":"0:1","query":"Frame","out":"out.png","bundle":"absent.zip","action":"status","jobId":"ws_0000000000000000.0"}),
            )
        };
        assert!(check(name, valid.clone()).is_ok());
        for (key, _, required, _) in *params {
            let mut args = valid.clone();
            args[*key] = json!([]);
            let reply = call_tool(&json!({"name":name,"arguments":args}), None);
            assert_eq!(
                reply["_meta"]["figlens"]["code"], "invalid_arguments",
                "{name}.{key}: {reply}"
            );
            if *required {
                args.as_object_mut().unwrap().remove(*key);
                assert!(check(name, args).is_err(), "{name}.{key}");
            }
        }
    }
}

#[test]
fn index_failures_carry_stable_codes_and_their_recovery() {
    use figlens::{ErrorKind, FigError};
    for (kind, code) in [
        (ErrorKind::SourceStale, "source_stale"),
        (ErrorKind::CursorStale, "cursor_stale"),
        (ErrorKind::CursorExpired, "cursor_expired"),
        (ErrorKind::Cancelled, "cancelled"),
        (ErrorKind::QueryWorkLimit, "query_work_limit"),
        (ErrorKind::ModeUnavailable, "mode_unavailable"),
    ] {
        let recovery = json!([{"action": "refresh", "requires": "none"}]);
        let error = FigError::new(kind, "x").with_recovery(recovery.clone());
        let meta = contract::Failure::from_error(Box::new(error)).metadata();
        assert_eq!(meta["code"], code);
        assert_eq!(meta["recovery"], recovery);
    }
}

#[test]
fn a_storage_failure_points_at_restoring_the_storage_not_the_design_roots() {
    use figlens::{ErrorKind, FigError};
    let recovery = json!([{"action": "restore_access", "requires": "none"}]);
    let error = FigError::new(ErrorKind::PermissionDenied, "x").with_recovery(recovery);
    let meta = contract::Failure::from_error(Box::new(error)).metadata();
    assert!(
        meta["nextStep"]
            .as_str()
            .unwrap()
            .contains("figlens storage"),
        "{meta}"
    );
}

#[test]
fn usages_library_is_a_workspace_question_apart_from_node_questions() {
    let ask = |extra: Value| {
        let mut args = json!({"workspaceId": "ws_0000000000000000",
            "library": {"kind": "component", "namespace": "lib", "key": "k1"}});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        args
    };
    let call = check("usages", ask(json!({"sourceId": "s1", "limit": 7})))
        .unwrap_or_else(|f| panic!("{}", f.metadata()));
    let request = crate::command::library_tool(&call).unwrap();
    assert_eq!(
        (request.kind.as_str(), request.scope.as_str(), request.limit),
        ("component", "0:0", 7)
    );
    // A key alone, and a definition selected by its exact ref.
    let call = check(
        "usages",
        ask(
            json!({"library": {"kind": "component", "key": "k1", "definition":
            {"sourceId": "s2", "snapshotId": "sha256:a", "nodeId": "1-2"}}}),
        ),
    )
    .unwrap_or_else(|f| panic!("{}", f.metadata()));
    let request = crate::command::library_tool(&call).unwrap();
    let d = request.definition.unwrap();
    assert_eq!(
        (request.namespace, d.source_id, d.snapshot_id, d.node_id),
        (None, "s2".into(), "sha256:a".into(), "1:2".into())
    );
    let definition =
        |d: Value| json!({"library": {"kind": "component", "key": "k1", "definition": d}});
    for bad in [
        definition(json!({"sourceId": "s2", "snapshotId": "sha256:a"})),
        definition(
            json!({"sourceId": "s2", "snapshotId": "sha256:a", "nodeId": "1:2", "name": "x"}),
        ),
        definition(json!({"sourceId": "", "snapshotId": "sha256:a", "nodeId": "1:2"})),
        json!({"workspaceId": null, "file": "a.fig"}),
        json!({"node": "0:1"}),
        json!({"resource": "image:00"}),
        json!({"snapshotId": "sha256:x"}),
        json!({"relation": "instance_of"}),
        json!({"direction": "in"}),
        json!({"library": {"kind": "frame", "namespace": "lib", "key": "k1"}}),
        json!({"library": {"kind": "style", "namespace": "", "key": "k1"}}),
        json!({"library": {"kind": "style", "namespace": "lib"}}),
        json!({"library": {"kind": "style", "namespace": "lib", "key": "k1", "id": "x"}}),
        json!({"library": "component/lib/k1"}),
    ] {
        let mut args = ask(bad.clone());
        args.as_object_mut().unwrap().retain(|_, v| !v.is_null());
        let failure = check("usages", args)
            .err()
            .unwrap_or_else(|| panic!("{bad}"));
        assert_eq!(failure.metadata()["code"], "invalid_arguments", "{bad}");
    }
}

#[test]
fn cli_library_takes_its_reference_and_definition_flags_and_no_node_options() {
    let args = |list: &[&str]| list.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let request = crate::command::library_request(&args(&[
        "--library",
        "style",
        "--namespace",
        "lib",
        "--key",
        "k1",
        "--scope",
        "1:2",
        "--limit",
        "3",
        "--include-hidden",
    ]))
    .unwrap();
    assert_eq!(
        (request.kind, request.namespace, request.key, request.scope),
        (
            "style".into(),
            Some("lib".into()),
            "k1".into(),
            "1:2".into()
        )
    );
    assert!(request.include_hidden && request.limit == 3);
    for extra in [
        &["--out"][..],
        &["--snapshot", "s"],
        &["--relation", "uses_style"],
    ] {
        let mut list = args(&["--library", "style", "--namespace", "lib", "--key", "k1"]);
        list.extend(args(extra));
        let error = crate::command::library_request(&list)
            .unwrap_err()
            .to_string();
        assert!(error.contains("does not apply to --library"), "{error}");
    }
    assert!(
        crate::command::library_request(&args(&["--library", "style", "--namespace", "lib"]))
            .is_err()
    );
    let selected = crate::command::library_request(&args(&[
        "--library",
        "component",
        "--key",
        "k1",
        "--definition-source",
        "s2",
        "--definition-snapshot",
        "sha256:a",
        "--definition-node",
        "1-2",
    ]))
    .unwrap();
    assert_eq!(
        (selected.namespace, selected.definition.map(|d| d.node_id)),
        (None, Some("1:2".into()))
    );
    let error = crate::command::library_request(&args(&[
        "--library",
        "component",
        "--key",
        "k1",
        "--definition-node",
        "1:2",
    ]))
    .unwrap_err();
    assert!(error.to_string().contains("go together"), "{error}");
    let error = crate::command::usages_request(&args(&["--library", "style"])).unwrap_err();
    assert!(error.to_string().contains("reads a workspace"), "{error}");
}

#[test]
fn dtcg_selection_is_explicit_and_validated_before_source_io() {
    let valid = json!({"file":"absent.fig", "format":"dtcg",
        "collection":"VariableCollectionId:0:10", "mode":"0:11", "out":"out.tokens.json"});
    assert!(check("variables", valid.clone()).is_ok());
    for key in ["collection", "mode", "out"] {
        let mut args = valid.clone();
        args.as_object_mut().unwrap().remove(key);
        let reply = call_tool(&json!({"name":"variables","arguments":args}), None);
        assert_eq!(
            reply["_meta"]["figlens"]["code"], "invalid_arguments",
            "{reply}"
        );
    }
    for extra in [
        json!({"format":"rest"}),
        json!({"format":"css"}),
        json!({"node":"0:1"}),
        json!({"collection":""}),
        json!({"mode":""}),
        json!({"out":""}),
    ] {
        let mut args = valid.clone();
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let reply = call_tool(&json!({"name":"variables","arguments":args}), None);
        assert_eq!(
            reply["_meta"]["figlens"]["code"], "invalid_arguments",
            "{reply}"
        );
    }
    for key in ["collection", "mode", "out"] {
        let mut args = json!({"file":"absent.fig"});
        args[key] = valid[key].clone();
        assert!(check("variables", args).is_err());
    }
}
