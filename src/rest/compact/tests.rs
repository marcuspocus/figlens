use super::*;

// Independent decoder of the published wire contract; table values are literals, not
// recursively interpreted references (including objects that happen to contain $style).
fn decode(compact: &Value) -> Value {
    fn node(n: &mut Value, compact: &Value) {
        if let Some(index) = n.get("$subtree").and_then(Value::as_u64) {
            let placements = n["placements"].as_array().unwrap().clone();
            *n = compact["subtrees"][index as usize].clone();
            let mut placements = placements.into_iter();
            walk_mut(n, &mut |n| {
                n.as_object_mut()
                    .unwrap()
                    .extend(placements.next().unwrap().as_object().unwrap().clone());
            });
            assert!(placements.next().is_none());
        }
        for key in STYLE_FIELDS {
            if let Some(index) = n
                .get(key)
                .and_then(Value::as_object)
                .filter(|o| o.len() == 1)
                .and_then(|o| o.get("$style"))
                .and_then(Value::as_u64)
            {
                n[key] = compact["sharedStyles"][index as usize].clone();
            }
        }
        if let Some(children) = n.get_mut("children").and_then(Value::as_array_mut) {
            for child in children {
                node(child, compact);
            }
        }
    }
    assert_eq!(compact["format"], FORMAT);
    let mut data = compact["data"].clone();
    node(&mut data["document"], compact);
    data
}

#[test]
fn references_are_deterministic_lossless_and_preserve_literal_collisions() {
    let style = json!({"fontFamily":"A font with a long name", "fontSize":14, "fontWeight":500});
    let children: Vec<_> = (0..20).map(|i| json!({
        "id": format!("1:{i}"), "name":"Repeated row", "type":"FRAME",
        "absoluteBoundingBox":{"x":i*10,"y":i*2,"width":120,"height":50},
        "children":[{"id":format!("I1:{i};2:1"),"type":"TEXT", "characters":"Literal 🐱\n\"text\"",
            "style":style,"fills":[{"type":"SOLID","color":{"r":0.1,"g":0.2,"b":0.3}}],
            "relativeTransform":[[1,0,0],[0,1,0]], "children":[]}]
    })).collect();
    let mut data = json!({"document":{"id":"0:1","type":"FRAME","children":children},
        "styles":{"style-id":{"name":"A style"}}, "components":{},"componentSets":{},
        "schemaVersion":0,"_figlens":{"semantics":{"verification":{"status":"unverified"}}}});
    data["document"]["children"][0]["absoluteRenderBounds"] = Value::Null;
    data["document"]["children"][1]["boundVariables"] = json!({"$style":0});
    data["document"]["children"][1]["layoutGrids"] = json!({"$style":0,"literal":true});
    // Provenance and transitions differ: these must not be normalized away.
    data["document"]["children"][2]["_figlens"] = json!({"prototype":{"source":"1:2"}});
    data["document"]["children"][3]["transitionNodeID"] = json!("1:5");
    for subtrees in [false, true] {
        let encoded = encode(data.clone(), subtrees);
        assert_eq!(decode(&encoded), data);
        assert_eq!(encode(data.clone(), subtrees), encoded);
        assert!(!encoded["sharedStyles"].as_array().unwrap().is_empty());
        assert_eq!(
            !encoded["subtrees"].as_array().unwrap().is_empty(),
            subtrees
        );
        assert!(
            serde_json::to_vec(&encoded).unwrap().len() < serde_json::to_vec(&data).unwrap().len()
        );
    }
}

#[test]
fn unique_small_values_and_missing_children_remain_literal() {
    let data = json!({"document":{"id":"0:1","fills":[],"children":[
        {"id":"0:2","fills":null,"effects":[]}, {"id":"0:3","fills":[]}
    ]},"styles":{}});
    let encoded = encode(data.clone(), true);
    assert_eq!(decode(&encoded), data);
    assert_eq!(encoded["sharedStyles"], json!([]));
    assert_eq!(encoded["subtrees"], json!([]));
}

#[test]
fn style_references_reuse_their_index_across_digit_boundaries() {
    let mut children: Vec<_> = (0..18)
        .map(|i| json!({"id":format!("0:{i}"),"fills":format!("{}{}", "large style ".repeat(8), i/2)}))
        .collect();
    children.extend((18..20).map(|i| json!({"id":format!("0:{i}"),"fills":"x".repeat(24)})));
    let data = json!({"document":{"id":"0:99","children":children}});
    let encoded = encode(data.clone(), false);
    assert_eq!(encoded["sharedStyles"].as_array().unwrap().len(), 10);
    for i in [18, 19] {
        assert_eq!(
            encoded["data"]["document"]["children"][i]["fills"],
            json!({"$style":9})
        );
    }
    assert_eq!(decode(&encoded), data);
}

#[test]
fn serialized_size_counts_escaped_bytes_and_stops_over_budget() {
    let value = json!({"text":"🧪\n\"text\"","children":[{},null,42]});
    let bytes = serde_json::to_vec(&value).unwrap().len();
    assert_eq!(serialized_size(&value, usize::MAX), bytes);
    assert_eq!(serialized_size(&value, bytes), bytes);
    assert!(serialized_size(&value, bytes - 1) > bytes - 1);
}

#[test]
fn compact_response_budget_does_not_reject_the_larger_rest_form() {
    use kiwi_schema::Value as K;
    use std::collections::HashMap;
    let object =
        |fields: Vec<(&'static str, K<'static>)>| K::Object("Test", fields.into_iter().collect());
    let parent = object(vec![("type", K::Enum("NodeType", "FRAME"))]);
    let child = object(vec![
        ("type", K::Enum("NodeType", "TEXT")),
        (
            "fontName",
            object(vec![(
                "family",
                K::String("Repeated font family ".repeat(100)),
            )]),
        ),
    ]);
    let ids: Vec<_> = (2..42).map(|i| format!("0:{i}")).collect();
    let mut nodes = HashMap::from([("0:1".into(), &parent)]);
    nodes.extend(ids.iter().map(|id| (id.clone(), &child)));
    let tree = crate::Tree {
        nodes,
        children: HashMap::from([("0:1".into(), ids)]),
        blobs: Vec::new(),
    };
    let default = super::super::resolve_nodes(&tree, "0:1").unwrap();
    for subtrees in [false, true] {
        let compact = super::super::resolve_compact_nodes(&tree, "0:1", subtrees, None).unwrap();
        let limit = serialized_size(&compact, usize::MAX);
        assert_eq!(decode(&compact), default);
        assert!(super::super::resolve_nodes_within(&tree, "0:1", limit).is_err());
        assert!(super::super::resolve_compact_nodes(&tree, "0:1", subtrees, Some(limit)).is_ok());
    }
}

#[test]
fn fixture_selections_roundtrip_and_compact_budget_is_exact() {
    let doc =
        crate::Document::open(std::path::Path::new("examples/first-success/design.fig")).unwrap();
    let message = doc.message().unwrap();
    let tree = crate::Tree::new(&message);
    for id in ["0:1", "0:2", "0:4"] {
        let default = super::super::resolve_nodes(&tree, id).unwrap();
        for subtrees in [false, true] {
            let encoded = super::super::resolve_compact_nodes(&tree, id, subtrees, None).unwrap();
            assert_eq!(decode(&encoded), default);
            let size = serde_json::to_vec(&encoded).unwrap().len();
            assert!(super::super::resolve_compact_nodes(&tree, id, subtrees, Some(size)).is_ok());
            let error = super::super::resolve_compact_nodes(&tree, id, subtrees, Some(size - 1))
                .unwrap_err();
            assert_eq!(
                error.downcast_ref::<crate::FigError>().unwrap().kind(),
                crate::ErrorKind::ResponseBudget
            );
        }
    }
}
