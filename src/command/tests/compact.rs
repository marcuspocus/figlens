use super::*;
use serde_json::{Value, json};

#[test]
fn nodes_compact_cli_and_mcp_agree_without_changing_defaults() {
    let file = "examples/first-success/design.fig";
    for subtrees in [false, true] {
        let mut args = vec![
            "nodes".into(),
            file.into(),
            "0:1".into(),
            "--compact".into(),
        ];
        if subtrees {
            args.push("--subtrees".into());
        }
        let mut text = String::new();
        run(args, &mut text).unwrap();
        let cli: Value = serde_json::from_str(&text).unwrap();
        let mcp = call_tool(
            &json!({"name":"nodes","arguments":{
                "file":file,"node":"0:1","compact":true,"subtrees":subtrees
            }}),
            None,
        );
        assert_ne!(mcp["isError"], true, "{mcp}");
        assert!(mcp.get("structuredContent").is_none());
        assert_eq!(cli["format"], "figlens.nodes.compact.v1");
        assert_eq!(
            serde_json::from_str::<Value>(mcp["content"][0]["text"].as_str().unwrap()).unwrap(),
            cli
        );
    }
    let default = call_tool(
        &json!({"name":"nodes","arguments":{"file":file,"node":"0:1"}}),
        None,
    );
    let explicit = call_tool(
        &json!({"name":"nodes","arguments":{"file":file,"node":"0:1","compact":false,"subtrees":false}}),
        None,
    );
    assert_eq!(default, explicit);
    let data: Value =
        serde_json::from_str(default["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(data.get("format").is_none());
    assert!(data["document"].is_object());
}

#[test]
fn nodes_compact_preflight_rejects_invalid_combinations_and_types() {
    for extra in [
        json!({"subtrees":true}),
        json!({"compact":false,"subtrees":true}),
        json!({"compact":"true"}),
        json!({"compact":null}),
        json!({"subtrees":1}),
    ] {
        let mut args = json!({"file":"absent.fig","node":"0:1"});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let reply = call_tool(&json!({"name":"nodes","arguments":args}), None);
        assert_eq!(
            reply["_meta"]["figlens"]["code"], "invalid_arguments",
            "{reply}"
        );
    }
    for flags in [
        vec!["0:1", "--subtrees"],
        vec!["0:1", "--unknown"],
        vec!["--compact"],
    ] {
        let mut args = vec!["nodes".into(), "examples/first-success/design.fig".into()];
        args.extend(flags.into_iter().map(str::to_owned));
        assert!(run(args, &mut String::new()).is_err());
    }
    let list = crate::contract::tools();
    let nodes = list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "nodes")
        .unwrap();
    for key in ["compact", "subtrees"] {
        assert_eq!(nodes["inputSchema"]["properties"][key]["type"], "boolean");
        assert_eq!(nodes["inputSchema"]["properties"][key]["default"], false);
    }
}
