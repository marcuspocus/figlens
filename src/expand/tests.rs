use super::*;

fn v2(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

fn obj(fields: Vec<(&'static str, Value<'static>)>) -> Value<'static> {
    Value::Object("Test", fields.into_iter().collect())
}

fn size(x: f32, y: f32) -> Value<'static> {
    obj(vec![("x", Value::Float(x)), ("y", Value::Float(y))])
}

#[test]
fn auto_layout_children_do_not_apply_constraints_again() {
    for (mode, positioning, expected) in [
        ("HORIZONTAL", "AUTO", 15.0),
        ("NONE", "AUTO", 17.5),
        ("HORIZONTAL", "ABSOLUTE", 17.5),
    ] {
        let main = obj(vec![("stackMode", Value::Enum("StackMode", mode))]);
        let group = obj(vec![
            ("type", Value::Enum("NodeType", "FRAME")),
            ("resizeToFit", Value::Bool(true)),
            (
                "stackPositioning",
                Value::Enum("StackPositioning", positioning),
            ),
            ("size", size(15.0, 15.0)),
        ]);
        let vector = obj(vec![
            ("type", Value::Enum("NodeType", "VECTOR")),
            ("size", size(10.0, 10.0)),
            ("horizontalConstraint", Value::Enum("Constraint", "SCALE")),
            ("verticalConstraint", Value::Enum("Constraint", "SCALE")),
        ]);
        let tree = Tree {
            nodes: HashMap::from([
                ("main".into(), &main),
                ("group".into(), &group),
                ("vector".into(), &vector),
            ]),
            children: HashMap::from([
                ("main".into(), vec!["group".into()]),
                ("group".into(), vec!["vector".into()]),
            ]),
            blobs: Vec::new(),
        };
        let mut ex = Expander {
            bounds: None,
            depth: 0,
            sizes: HashMap::new(),
            tree: &tree,
            out: Expansion {
                nodes: HashMap::new(),
                children: HashMap::new(),
            },
            symbols: Vec::new(),
            budget: None,
            exceeded: false,
        };
        let derived = Patches::from([(
            "group".into(),
            Patch {
                transform: Some(Mat {
                    tx: 6.5,
                    ty: 6.5,
                    ..Mat::IDENTITY
                }),
                ..Patch::default()
            },
        )]);
        ex.expand(
            "instance",
            "instance",
            "main",
            &[],
            &Patches::new(),
            &derived,
            Mat::IDENTITY,
            Some((v2(24.0, 24.0), v2(28.0, 28.0))),
            1.0,
        );
        assert_eq!(
            ex.out.nodes["Iinstance;group"].size,
            Some(v2(expected, expected))
        );
        let expected_vector = expected / 15.0 * 10.0;
        assert!((ex.out.nodes["Iinstance;vector"].size.unwrap().x - expected_vector).abs() < 0.001);
    }
}

#[test]
fn swapped_names_use_the_component_set_unless_explicitly_overridden() {
    let main = obj(vec![]);
    let child = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("name", Value::String("custom old name".into())),
    ]);
    let component = obj(vec![
        ("name", Value::String("State=on".into())),
        ("parentIndex", obj(vec![("guid", guid(1, 1))])),
    ]);
    let set = obj(vec![
        ("name", Value::String("Flag".into())),
        ("isStateGroup", Value::Bool(true)),
    ]);
    let tree = Tree {
        nodes: HashMap::from([
            ("main".into(), &main),
            ("child".into(), &child),
            ("component".into(), &component),
            ("1:1".into(), &set),
        ]),
        children: HashMap::from([("main".into(), vec!["child".into()])]),
        blobs: Vec::new(),
    };
    for name in [None, Some("explicit name".to_owned())] {
        let mut ex = Expander {
            bounds: None,
            depth: 0,
            sizes: HashMap::new(),
            tree: &tree,
            out: Expansion {
                nodes: HashMap::new(),
                children: HashMap::new(),
            },
            symbols: Vec::new(),
            budget: None,
            exceeded: false,
        };
        let ov = Patches::from([(
            "child".into(),
            Patch {
                name: name.clone(),
                swap: Some("component".into()),
                ..Patch::default()
            },
        )]);
        ex.expand(
            "outer",
            "outer",
            "main",
            &[],
            &ov,
            &Patches::new(),
            Mat::IDENTITY,
            None,
            1.0,
        );
        assert_eq!(
            ex.out.nodes["Iouter;child"].name,
            name.as_deref().unwrap_or("Flag")
        );
    }
}

fn guid(s: u32, l: u32) -> Value<'static> {
    obj(vec![
        ("sessionID", Value::UInt(s)),
        ("localID", Value::UInt(l)),
    ])
}

#[test]
fn a_component_containing_itself_stops_at_the_repeat() {
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
    ]);
    let main = obj(vec![]);
    let inner = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
    ]);
    let tree = Tree {
        nodes: HashMap::from([
            ("inst".into(), &inst),
            ("1:1".into(), &main),
            ("2:2".into(), &inner),
        ]),
        children: HashMap::from([("1:1".into(), vec!["2:2".into()]), ("inst".into(), vec![])]),
        blobs: Vec::new(),
    };
    let ex = expand_instances(&tree, "inst");
    assert_eq!((ex.nodes.len(), ex.children.get("Iinst;2:2")), (1, None));
}

/// `inst` shows main 1:0; main 1:k holds 2:k, an instance of main 1:k+1, for `mains` mains.
fn component_chain(mains: usize) -> Vec<(String, Value<'static>)> {
    let instance = |k: usize| {
        obj(vec![
            ("type", Value::Enum("NodeType", "INSTANCE")),
            ("symbolData", obj(vec![("symbolID", guid(1, k as u32))])),
        ])
    };
    let mut nodes = vec![("inst".to_owned(), instance(0))];
    for k in 0..mains {
        nodes.push((format!("1:{k}"), obj(vec![])));
        nodes.push((format!("2:{k}"), instance(k + 1)));
    }
    nodes
}

fn chain_tree<'a>(nodes: &'a [(String, Value<'a>)]) -> Tree<'a> {
    let mains = nodes.len() / 2;
    Tree {
        nodes: nodes.iter().map(|(id, n)| (id.clone(), n)).collect(),
        children: (0..mains - 1)
            .map(|k| (format!("1:{k}"), vec![format!("2:{k}")]))
            .collect(),
        blobs: Vec::new(),
    }
}

#[test]
fn a_component_chain_past_the_depth_limit_fails_the_scope() {
    let nodes = component_chain(MAX_DEPTH + 2);
    let error = crate::rest::resolve_nodes(&chain_tree(&nodes), "inst").unwrap_err();
    assert!(error.to_string().contains("layer depth"), "{error}");
}

#[test]
fn stored_children_keep_a_too_deep_main_from_failing_the_scope() {
    let nodes = component_chain(MAX_DEPTH + 2);
    let stored = obj(vec![]);
    let mut tree = chain_tree(&nodes);
    tree.nodes.insert("s".into(), &stored);
    tree.children.insert("inst".into(), vec!["s".into()]);
    assert!(crate::rest::resolve_nodes(&tree, "inst").is_ok());
}

#[test]
fn a_reply_budget_counts_only_the_selected_sublayer() {
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
    ]);
    let main = obj(vec![]);
    let rect = |name: &str| {
        obj(vec![
            ("type", Value::Enum("NodeType", "RECTANGLE")),
            ("name", Value::String(name.to_owned())),
        ])
    };
    let (small, large) = (rect("small"), rect(&"x".repeat(4096)));
    let tree = Tree {
        nodes: HashMap::from([
            ("inst".into(), &inst),
            ("1:1".into(), &main),
            ("1:4".into(), &small),
            ("1:5".into(), &large),
        ]),
        children: HashMap::from([("1:1".into(), vec!["1:4".into(), "1:5".into()])]),
        blobs: Vec::new(),
    };
    let within = |id, bytes| crate::rest::resolve_nodes_within(&tree, id, bytes);
    for id in ["Iinst;1:4", "inst"] {
        let full = crate::rest::resolve_nodes(&tree, id).unwrap();
        // The complete selection's own size always fits: the count is a lower bound.
        let size = serde_json::to_vec(&full["document"]).unwrap().len();
        assert_eq!(within(id, size).unwrap(), full);
    }
    let error = within("inst", 1024).unwrap_err();
    let error = error.downcast_ref::<crate::FigError>().unwrap();
    assert_eq!(error.kind(), crate::ErrorKind::ResponseBudget);
    assert!(within("Iinst;1:4", 1024).is_ok());
}

#[test]
fn a_bounded_expansion_stops_exponential_fan_out_but_keeps_fitting_scopes() {
    // Component d holds ten instances of component d + 1: 10^6 nested copies in all.
    let instance = |d: u32| {
        obj(vec![
            ("type", Value::Enum("NodeType", "INSTANCE")),
            ("symbolData", obj(vec![("symbolID", guid(d, 0))])),
        ])
    };
    let mut owned = vec![
        ("inst".to_owned(), instance(10)),
        ("16:0".to_owned(), obj(vec![])),
    ];
    let mut children = HashMap::new();
    for d in 10..16 {
        owned.push((format!("{d}:0"), obj(vec![])));
        let kids: Vec<String> = (1..=10).map(|i| format!("{d}:{i}")).collect();
        for kid in &kids {
            owned.push((kid.clone(), instance(d + 1)));
        }
        children.insert(format!("{d}:0"), kids);
    }
    let mut tree = Tree {
        nodes: owned.iter().map(|(id, v)| (id.clone(), v)).collect(),
        children,
        blobs: Vec::new(),
    };
    assert!(expand_instances_within(&tree, "inst", 1 << 20).is_none());
    let error = crate::rest::resolve_nodes_within(&tree, "inst", 1 << 20).unwrap_err();
    let error = error.downcast_ref::<crate::FigError>().unwrap();
    assert_eq!(error.kind(), crate::ErrorKind::ResponseBudget);
    // Two levels fit: the bounded run matches the full one.
    tree.children.insert("12:0".into(), Vec::new());
    let full = crate::rest::resolve_nodes(&tree, "inst").unwrap();
    let size = serde_json::to_vec(&full["document"]).unwrap().len();
    let bounded = expand_instances_within(&tree, "inst", size).unwrap();
    assert_eq!(bounded.nodes.len(), 110);
    assert_eq!(bounded.children, expand_instances(&tree, "inst").children);
    assert_eq!(
        crate::rest::resolve_nodes_within(&tree, "inst", size).unwrap(),
        full
    );
    // REST keeps stored children over an expansion, so a bounded run neither counts nor builds it.
    tree.children.insert("inst".into(), Vec::new());
    let full = crate::rest::resolve_nodes(&tree, "inst").unwrap();
    let size = serde_json::to_vec(&full["document"]).unwrap().len();
    assert!(
        expand_instances_within(&tree, "inst", 0)
            .unwrap()
            .nodes
            .is_empty()
    );
    assert_eq!(
        crate::rest::resolve_nodes_within(&tree, "inst", size).unwrap(),
        full
    );
}

fn bounded(tree: &Tree, root: &str, work: usize, bytes: usize) -> (Expansion, Bounds) {
    let bounds = Bounds {
        work,
        bytes,
        depth: 64,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
        stop: None,
    };
    expand_instances_bounded(tree, root, bounds)
}

#[test]
fn a_bounded_expansion_charges_text_property_copies_before_binding_them() {
    // One large assignment bound into 100 children copies it 100 times; 1000 more
    // assignments make every reference scan a wide list.
    let assignment = |id: u32, text: String| {
        obj(vec![
            ("defID", guid(2, id)),
            ("value", obj(vec![("textValue", Value::String(text))])),
        ])
    };
    let instance = |extra: u32| {
        let list = (0..=extra)
            .map(|i| {
                assignment(
                    i + 1,
                    if i == 0 {
                        "x".repeat(10_000)
                    } else {
                        "y".into()
                    },
                )
            })
            .collect();
        obj(vec![
            ("type", Value::Enum("NodeType", "INSTANCE")),
            ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
            ("componentPropAssignments", Value::Array(list)),
        ])
    };
    let (inst, wide) = (instance(0), instance(1000));
    let child = obj(vec![
        ("type", Value::Enum("NodeType", "TEXT")),
        (
            "componentPropRefs",
            Value::Array(vec![obj(vec![
                ("defID", guid(2, 1)),
                ("componentPropNodeField", Value::Enum("Field", "TEXT_DATA")),
            ])]),
        ),
    ]);
    let (page, main) = (obj(vec![]), obj(vec![]));
    let kids: Vec<String> = (0..100).map(|i| format!("c{i}")).collect();
    let mut nodes: HashMap<String, &Value> = kids.iter().map(|k| (k.clone(), &child)).collect();
    nodes.extend([
        ("page".into(), &page),
        ("wide page".into(), &page),
        ("inst".into(), &inst),
        ("wide".into(), &wide),
        ("1:1".into(), &main),
    ]);
    let tree = Tree {
        nodes,
        children: HashMap::from([
            ("page".into(), vec!["inst".into()]),
            ("wide page".into(), vec!["wide".into()]),
            ("1:1".into(), kids),
        ]),
        blobs: Vec::new(),
    };
    let (ex, bounds) = bounded(&tree, "page", 1 << 20, 200_000);
    assert_eq!((bounds.stop, ex.nodes.len()), (Some(Stop::Bytes), 0));
    assert_eq!(bounded(&tree, "page", 1 << 20, 4 << 20).0.nodes.len(), 100);
    // 100 references over 1001 assignments: the scans' work is charged before binding.
    let (ex, bounds) = bounded(&tree, "wide page", 50_000, 64 << 20);
    assert_eq!((bounds.stop, ex.nodes.len()), (Some(Stop::Work), 0));
    assert_eq!(
        bounded(&tree, "wide page", 1 << 20, 64 << 20).0.nodes.len(),
        100
    );
}

#[test]
fn a_bounded_expansion_charges_each_instance_ancestor_walk() {
    // 2,000 nested frames, each holding a childless instance: placing them walks 2M ancestors.
    let frames: Vec<Value> = (0..2000u32)
        .map(|i| {
            let mut fields = vec![("type", Value::Enum("NodeType", "FRAME"))];
            if i > 0 {
                fields.push(("parentIndex", obj(vec![("guid", guid(0, i - 1))])));
            }
            obj(fields)
        })
        .collect();
    let insts: Vec<Value> = (0..2000u32)
        .map(|i| {
            obj(vec![
                ("type", Value::Enum("NodeType", "INSTANCE")),
                ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
                ("parentIndex", obj(vec![("guid", guid(0, i))])),
            ])
        })
        .collect();
    let main = obj(vec![]);
    let mut nodes: HashMap<String, &Value> = HashMap::from([("1:1".into(), &main)]);
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for i in 0..2000u32 {
        nodes.insert(format!("0:{i}"), &frames[i as usize]);
        nodes.insert(format!("i{i}"), &insts[i as usize]);
        let kids = children.entry(format!("0:{i}")).or_default();
        kids.push(format!("i{i}"));
        if i + 1 < 2000 {
            kids.push(format!("0:{}", i + 1));
        }
    }
    let tree = Tree {
        nodes,
        children,
        blobs: Vec::new(),
    };
    // Visiting the 4,000 stored nodes fits easily; the walks do not.
    let (_, bounds) = bounded(&tree, "0:0", 100_000, 1 << 30);
    assert_eq!(bounds.stop, Some(Stop::Work));
    let (_, bounds) = bounded(&tree, "0:0", 5_000_000, 1 << 30);
    assert_eq!(bounds.stop, None);
}

/// Expands `nested` under a 2 kB id key inside one top-level instance; `extra` and
/// `inner_children` populate its component.
fn under_long_key<'v>(
    nested: &'v Value<'v>,
    extra: &[(&str, &'v Value<'v>)],
    inner_children: Vec<String>,
    bytes: usize,
) -> (Expansion, Bounds) {
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
    ]);
    let (page, outer, inner) = (obj(vec![]), obj(vec![]), obj(vec![]));
    let long = "n".repeat(2000);
    let mut nodes: HashMap<String, &Value> = HashMap::from([
        ("page".into(), &page),
        ("inst".into(), &inst),
        ("1:1".into(), &outer),
        ("2:2".into(), &inner),
        (long.clone(), nested),
    ]);
    nodes.extend(extra.iter().map(|(k, v)| (k.to_string(), *v)));
    let tree = Tree {
        nodes,
        children: HashMap::from([
            ("page".into(), vec!["inst".into()]),
            ("1:1".into(), vec![long]),
            ("2:2".into(), inner_children),
        ]),
        blobs: Vec::new(),
    };
    bounded(&tree, "page", 1 << 20, bytes)
}

#[test]
fn a_bounded_expansion_charges_the_prefix_nested_overrides_copy() {
    // Each of 1,000 override or derived entries copies the 2 kB key prefix when merged.
    let entries: Vec<Value> = (0..1000)
        .map(|i| {
            obj(vec![
                (
                    "guidPath",
                    obj(vec![("guids", Value::Array(vec![guid(5, i)]))]),
                ),
                ("visible", Value::Bool(false)),
            ])
        })
        .collect();
    let symbol = |extra: Vec<(&'static str, Value<'static>)>| {
        let mut fields = vec![("symbolID", guid(2, 2))];
        fields.extend(extra);
        obj(fields)
    };
    for nested in [
        obj(vec![
            ("type", Value::Enum("NodeType", "INSTANCE")),
            (
                "symbolData",
                symbol(vec![("symbolOverrides", Value::Array(entries.clone()))]),
            ),
        ]),
        obj(vec![
            ("type", Value::Enum("NodeType", "INSTANCE")),
            ("symbolData", symbol(vec![])),
            ("derivedSymbolData", Value::Array(entries.clone())),
        ]),
    ] {
        let (ex, bounds) = under_long_key(&nested, &[], Vec::new(), 1_500_000);
        assert_eq!((bounds.stop, ex.nodes.len()), (Some(Stop::Bytes), 1));
        assert_eq!(
            under_long_key(&nested, &[], Vec::new(), 8 << 20).1.stop,
            None
        );
    }
}

#[test]
fn a_bounded_expansion_charges_the_keys_property_binding_builds() {
    // Binding one assignment writes the 2 kB prefix into a key for each of 1,000 layers.
    let nested = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(2, 2))])),
        (
            "componentPropAssignments",
            Value::Array(vec![obj(vec![
                ("defID", guid(3, 1)),
                ("value", obj(vec![("boolValue", Value::Bool(false))])),
            ])]),
        ),
    ]);
    let layer = obj(vec![
        ("type", Value::Enum("NodeType", "FRAME")),
        (
            "componentPropRefs",
            Value::Array(vec![obj(vec![
                ("defID", guid(3, 1)),
                ("componentPropNodeField", Value::Enum("Field", "VISIBLE")),
            ])]),
        ),
    ]);
    let kids: Vec<String> = (0..1000).map(|i| format!("k{i}")).collect();
    let extra: Vec<(&str, &Value)> = kids.iter().map(|k| (k.as_str(), &layer)).collect();
    let (ex, bounds) = under_long_key(&nested, &extra, kids.clone(), 1_500_000);
    assert_eq!((bounds.stop, ex.nodes.len()), (Some(Stop::Bytes), 1));
    assert_eq!(
        under_long_key(&nested, &extra, kids.clone(), 16 << 20)
            .1
            .stop,
        None
    );
    // Each of 50 references on one layer clones its 100 kB key.
    let refs = (0..50)
        .map(|_| {
            obj(vec![
                ("defID", guid(3, 1)),
                ("componentPropNodeField", Value::Enum("Field", "VISIBLE")),
            ])
        })
        .collect();
    let layer = obj(vec![
        ("type", Value::Enum("NodeType", "FRAME")),
        ("componentPropRefs", Value::Array(refs)),
    ]);
    let wide = "w".repeat(100_000);
    let extra = [(wide.as_str(), &layer)];
    let (ex, bounds) = under_long_key(&nested, &extra, vec![wide.clone()], 4_000_000);
    assert_eq!((bounds.stop, ex.nodes.len()), (Some(Stop::Bytes), 1));
    assert_eq!(
        under_long_key(&nested, &extra, vec![wide.clone()], 16 << 20)
            .1
            .stop,
        None
    );
}

#[test]
fn a_bounded_expansion_charges_every_string_an_expanded_node_keeps() {
    // 1,000 layers with 1 kB ids under a 2 kB key: each keeps its ~3 kB id twice plus its source.
    let nested = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(2, 2))])),
    ]);
    let layer = obj(vec![("type", Value::Enum("NodeType", "FRAME"))]);
    let kids: Vec<String> = (0..1000)
        .map(|i| format!("k{i}{}", "x".repeat(1000)))
        .collect();
    let extra: Vec<(&str, &Value)> = kids.iter().map(|k| (k.as_str(), &layer)).collect();
    let (_, bounds) = under_long_key(&nested, &extra, kids.clone(), 6_000_000);
    assert_eq!(bounds.stop, Some(Stop::Bytes));
    assert_eq!(
        under_long_key(&nested, &extra, kids.clone(), 16 << 20)
            .1
            .stop,
        None
    );
}

#[test]
fn a_bounded_expansion_charges_the_name_a_patch_keeps_beside_its_props() {
    // A 1 MB name override is kept as `Patch.name` and in `props`, then cloned at a nested instance.
    let name = "n".repeat(1_000_000);
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        (
            "symbolData",
            obj(vec![
                ("symbolID", guid(1, 1)),
                (
                    "symbolOverrides",
                    Value::Array(vec![obj(vec![
                        (
                            "guidPath",
                            obj(vec![("guids", Value::Array(vec![guid(5, 1)]))]),
                        ),
                        ("name", Value::String(name.clone())),
                    ])]),
                ),
            ]),
        ),
    ]);
    let nested = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(2, 2))])),
    ]);
    let (page, outer, inner) = (obj(vec![]), obj(vec![]), obj(vec![]));
    let tree = Tree {
        nodes: HashMap::from([
            ("page".into(), &page),
            ("inst".into(), &inst),
            ("1:1".into(), &outer),
            ("2:2".into(), &inner),
            ("c".into(), &nested),
        ]),
        children: HashMap::from([
            ("page".into(), vec!["inst".into()]),
            ("1:1".into(), vec!["c".into()]),
        ]),
        blobs: Vec::new(),
    };
    assert_eq!(
        bounded(&tree, "page", 1 << 20, 3_000_000).1.stop,
        Some(Stop::Bytes)
    );
    assert_eq!(bounded(&tree, "page", 1 << 20, 8 << 20).1.stop, None);
}

#[test]
fn a_bounded_expansion_charges_the_enum_names_overrides_copy() {
    // Each of 1,000 overrides converts a 10 kB enum member name into an owned string.
    let member: &'static str = Box::leak("E".repeat(10_000).into_boxed_str());
    let overrides = (0..1000)
        .map(|i| {
            obj(vec![
                (
                    "guidPath",
                    obj(vec![("guids", Value::Array(vec![guid(5, i)]))]),
                ),
                ("stackMode", Value::Enum("StackMode", member)),
            ])
        })
        .collect();
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        (
            "symbolData",
            obj(vec![
                ("symbolID", guid(1, 1)),
                ("symbolOverrides", Value::Array(overrides)),
            ]),
        ),
    ]);
    let (page, main) = (obj(vec![]), obj(vec![]));
    let tree = Tree {
        nodes: HashMap::from([
            ("page".into(), &page),
            ("inst".into(), &inst),
            ("1:1".into(), &main),
        ]),
        children: HashMap::from([("page".into(), vec!["inst".into()])]),
        blobs: Vec::new(),
    };
    assert_eq!(
        bounded(&tree, "page", 1 << 20, 4_000_000).1.stop,
        Some(Stop::Bytes)
    );
    assert_eq!(bounded(&tree, "page", 1 << 20, 32 << 20).1.stop, None);
}

#[test]
fn a_bounded_expansion_charges_the_field_names_a_nested_merge_clones() {
    // Schema field names are file data: 1,000 overrides of a 10 kB field, cloned at a nested instance.
    let field: &'static str = Box::leak("f".repeat(10_000).into_boxed_str());
    let overrides = (0..1000)
        .map(|i| {
            obj(vec![
                (
                    "guidPath",
                    obj(vec![("guids", Value::Array(vec![guid(5, i)]))]),
                ),
                (field, Value::Bool(true)),
            ])
        })
        .collect();
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        (
            "symbolData",
            obj(vec![
                ("symbolID", guid(1, 1)),
                ("symbolOverrides", Value::Array(overrides)),
            ]),
        ),
    ]);
    let nested = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(2, 2))])),
    ]);
    let (page, outer, inner) = (obj(vec![]), obj(vec![]), obj(vec![]));
    let tree = Tree {
        nodes: HashMap::from([
            ("page".into(), &page),
            ("inst".into(), &inst),
            ("1:1".into(), &outer),
            ("2:2".into(), &inner),
            ("c".into(), &nested),
        ]),
        children: HashMap::from([
            ("page".into(), vec!["inst".into()]),
            ("1:1".into(), vec!["c".into()]),
        ]),
        blobs: Vec::new(),
    };
    assert_eq!(
        bounded(&tree, "page", 1 << 20, 15_000_000).1.stop,
        Some(Stop::Bytes)
    );
    assert_eq!(bounded(&tree, "page", 1 << 20, 64 << 20).1.stop, None);
}

#[test]
fn a_bounded_expansion_charges_placement_and_size_conversions() {
    // Schema enums and field shapes are file data: each layer converts a 10 kB placement
    // enum and copies a size carrying a 10 kB field into both geometry-size entries.
    let long: &'static str = Box::leak("e".repeat(10_000).into_boxed_str());
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
    ]);
    let layer = obj(vec![
        ("type", Value::Enum("NodeType", "FRAME")),
        ("stackChildAlignSelf", Value::Enum("Align", long)),
        (
            "size",
            obj(vec![("x", Value::Float(1.0)), (long, Value::Bool(true))]),
        ),
    ]);
    let (page, main) = (obj(vec![]), obj(vec![]));
    let kids: Vec<String> = (0..1000).map(|i| format!("k{i}")).collect();
    let mut nodes: HashMap<String, &Value> = HashMap::from([
        ("page".into(), &page),
        ("inst".into(), &inst),
        ("1:1".into(), &main),
    ]);
    nodes.extend(kids.iter().map(|k| (k.clone(), &layer)));
    let tree = Tree {
        nodes,
        children: HashMap::from([("page".into(), vec!["inst".into()]), ("1:1".into(), kids)]),
        blobs: Vec::new(),
    };
    assert_eq!(
        bounded(&tree, "page", 1 << 20, 15_000_000).1.stop,
        Some(Stop::Bytes)
    );
    assert_eq!(bounded(&tree, "page", 1 << 20, 64 << 20).1.stop, None);
}

#[test]
fn a_bounded_expansion_charges_no_size_copy_where_both_outlines_are_derived() {
    // 1,000 layers with a 10 kB-field size, all with derived fill and stroke outlines: no
    // geometry-size entry copies the size, so only the derived data is charged.
    let long: &'static str = Box::leak("e".repeat(10_000).into_boxed_str());
    let derived = (0..1000)
        .map(|i| {
            obj(vec![
                (
                    "guidPath",
                    obj(vec![("guids", Value::Array(vec![guid(5, i)]))]),
                ),
                ("fillGeometry", Value::Array(vec![])),
                ("strokeGeometry", Value::Array(vec![])),
            ])
        })
        .collect();
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
        ("derivedSymbolData", Value::Array(derived)),
    ]);
    let layer = obj(vec![
        ("type", Value::Enum("NodeType", "FRAME")),
        (
            "size",
            obj(vec![("x", Value::Float(1.0)), (long, Value::Bool(true))]),
        ),
    ]);
    let (page, main) = (obj(vec![]), obj(vec![]));
    let kids: Vec<String> = (0..1000).map(|i| format!("5:{i}")).collect();
    let mut nodes: HashMap<String, &Value> = HashMap::from([
        ("page".into(), &page),
        ("inst".into(), &inst),
        ("1:1".into(), &main),
    ]);
    nodes.extend(kids.iter().map(|k| (k.clone(), &layer)));
    let tree = Tree {
        nodes,
        children: HashMap::from([("page".into(), vec!["inst".into()]), ("1:1".into(), kids)]),
        blobs: Vec::new(),
    };
    // Two charged size copies per layer alone would pass 20 MB.
    assert_eq!(bounded(&tree, "page", 1 << 20, 4_000_000).1.stop, None);
}

#[test]
fn a_bounded_expansion_charges_the_field_names_an_expanded_node_copies() {
    // 1,000 overrides of a 10 kB field, each copied with its name into its expanded layer.
    let field: &'static str = Box::leak("f".repeat(10_000).into_boxed_str());
    let overrides = (0..1000)
        .map(|i| {
            obj(vec![
                (
                    "guidPath",
                    obj(vec![("guids", Value::Array(vec![guid(5, i)]))]),
                ),
                (field, Value::Bool(true)),
            ])
        })
        .collect();
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        (
            "symbolData",
            obj(vec![
                ("symbolID", guid(1, 1)),
                ("symbolOverrides", Value::Array(overrides)),
            ]),
        ),
    ]);
    let layer = obj(vec![("type", Value::Enum("NodeType", "FRAME"))]);
    let (page, main) = (obj(vec![]), obj(vec![]));
    let kids: Vec<String> = (0..1000).map(|i| format!("5:{i}")).collect();
    let mut nodes: HashMap<String, &Value> = HashMap::from([
        ("page".into(), &page),
        ("inst".into(), &inst),
        ("1:1".into(), &main),
    ]);
    nodes.extend(kids.iter().map(|k| (k.clone(), &layer)));
    let tree = Tree {
        nodes,
        children: HashMap::from([("page".into(), vec!["inst".into()]), ("1:1".into(), kids)]),
        blobs: Vec::new(),
    };
    assert_eq!(
        bounded(&tree, "page", 1 << 20, 15_000_000).1.stop,
        Some(Stop::Bytes)
    );
    assert_eq!(bounded(&tree, "page", 1 << 20, 64 << 20).1.stop, None);
}

#[test]
fn a_bounded_expansion_charges_comparing_large_reference_ids() {
    // Schema-defined defIDs: 100 references of a 10 kB enum against 100 small assignments.
    let long: &'static str = Box::leak("d".repeat(10_000).into_boxed_str());
    let assignments = (0..100)
        .map(|i| {
            obj(vec![
                ("defID", guid(3, i)),
                ("value", obj(vec![("boolValue", Value::Bool(false))])),
            ])
        })
        .collect();
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
        ("componentPropAssignments", Value::Array(assignments)),
    ]);
    let refs = (0..100)
        .map(|_| {
            obj(vec![
                ("defID", Value::Enum("Def", long)),
                ("componentPropNodeField", Value::Enum("Field", "VISIBLE")),
            ])
        })
        .collect();
    let layer = obj(vec![
        ("type", Value::Enum("NodeType", "FRAME")),
        ("componentPropRefs", Value::Array(refs)),
    ]);
    let (page, main) = (obj(vec![]), obj(vec![]));
    let tree = Tree {
        nodes: HashMap::from([
            ("page".into(), &page),
            ("inst".into(), &inst),
            ("1:1".into(), &main),
            ("5:1".into(), &layer),
        ]),
        children: HashMap::from([
            ("page".into(), vec!["inst".into()]),
            ("1:1".into(), vec!["5:1".into()]),
        ]),
        blobs: Vec::new(),
    };
    assert_eq!(
        bounded(&tree, "page", 200_000, 64 << 20).1.stop,
        Some(Stop::Work)
    );
    assert_eq!(bounded(&tree, "page", 4_000_000, 64 << 20).1.stop, None);
}

#[test]
fn properties_bind_by_equal_definition_ids_of_any_shape() {
    let bound = |def: Value<'static>| {
        let inst = obj(vec![(
            "componentPropAssignments",
            Value::Array(vec![obj(vec![
                ("defID", def.clone()),
                ("value", obj(vec![("boolValue", Value::Bool(false))])),
            ])]),
        )]);
        let layer = obj(vec![(
            "componentPropRefs",
            Value::Array(vec![obj(vec![
                ("defID", def),
                ("componentPropNodeField", Value::Enum("Field", "VISIBLE")),
            ])]),
        )]);
        let main = obj(vec![]);
        let tree = Tree {
            nodes: HashMap::from([("1:1".into(), &main), ("5:1".into(), &layer)]),
            children: HashMap::from([("1:1".into(), vec!["5:1".into()])]),
            blobs: Vec::new(),
        };
        bind_properties(&tree, "1:1", "", Patches::new(), &inst, &Patch::default())
            .get("5:1")
            .and_then(|p| p.visible)
    };
    assert_eq!(bound(guid(3, 1)), Some(false));
    assert_eq!(bound(Value::Enum("Def", "same")), Some(false));
}

#[test]
fn maximal_charges_stop_on_work_instead_of_overflowing() {
    let cost = Cost {
        defs: usize::MAX,
        ..Cost::default()
    };
    assert_eq!(cost.comparisons(usize::MAX), usize::MAX);
    let tree = Tree {
        nodes: HashMap::new(),
        children: HashMap::new(),
        blobs: Vec::new(),
    };
    let mut ex = Expander {
        bounds: Some(Bounds {
            work: 10,
            bytes: 10,
            depth: 64,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
            stop: None,
        }),
        depth: 0,
        sizes: HashMap::new(),
        tree: &tree,
        out: Expansion {
            nodes: HashMap::new(),
            children: HashMap::new(),
        },
        symbols: Vec::new(),
        budget: None,
        exceeded: false,
    };
    let estimate = |left: &mut usize| {
        *left -= 1;
        usize::MAX
    };
    assert_eq!(ex.spend_estimated(usize::MAX, usize::MAX, estimate), None);
    assert_eq!(ex.bounds.unwrap().stop, Some(Stop::Work));
}

#[test]
fn a_bounded_expansion_skips_a_nested_instance_of_a_missing_component() {
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
    ]);
    let nested = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(9, 9))])),
    ]);
    let (page, main) = (obj(vec![]), obj(vec![]));
    let tree = Tree {
        nodes: HashMap::from([
            ("page".into(), &page),
            ("inst".into(), &inst),
            ("1:1".into(), &main),
            ("nested".into(), &nested),
        ]),
        children: HashMap::from([
            ("page".into(), vec!["inst".into()]),
            ("1:1".into(), vec!["nested".into()]),
        ]),
        blobs: Vec::new(),
    };
    let (ex, bounds) = bounded(&tree, "page", 1 << 20, 1 << 20);
    assert_eq!(bounds.stop, None);
    assert_eq!(ex.children, expand_instances(&tree, "page").children);
}

#[test]
fn a_bounded_expansion_charges_the_names_it_copies() {
    // Twenty instances each copy the main child's 10 kB default name.
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
    ]);
    let child = obj(vec![
        ("type", Value::Enum("NodeType", "FRAME")),
        ("name", Value::String("n".repeat(10_000))),
    ]);
    let (page, main) = (obj(vec![]), obj(vec![]));
    let insts: Vec<String> = (0..20).map(|i| format!("i{i}")).collect();
    let mut nodes: HashMap<String, &Value> = insts.iter().map(|k| (k.clone(), &inst)).collect();
    nodes.extend([
        ("page".into(), &page),
        ("1:1".into(), &main),
        ("child".into(), &child),
    ]);
    let tree = Tree {
        nodes,
        children: HashMap::from([("page".into(), insts), ("1:1".into(), vec!["child".into()])]),
        blobs: Vec::new(),
    };
    let (ex, bounds) = bounded(&tree, "page", 1 << 20, 100_000);
    assert_eq!(bounds.stop, Some(Stop::Bytes));
    assert!(ex.nodes.len() < 10, "{}", ex.nodes.len());
    assert_eq!(bounded(&tree, "page", 1 << 20, 1 << 20).0.nodes.len(), 20);
}

#[test]
fn subtree_visits_a_cycle_once() {
    let node = obj(vec![]);
    let tree = Tree {
        nodes: HashMap::from([("a".into(), &node), ("b".into(), &node)]),
        children: HashMap::from([
            ("a".into(), vec!["b".into()]),
            ("b".into(), vec!["a".into()]),
        ]),
        blobs: Vec::new(),
    };
    assert_eq!(subtree(&tree, "a"), vec!["a", "b"]);
}

#[test]
fn a_repeated_sublayer_id_is_expanded_once() {
    // Two sublayers sharing one overrideKey would give one id twice and a node its own child.
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
    ]);
    let main = obj(vec![]);
    let frame = || {
        obj(vec![
            ("type", Value::Enum("NodeType", "FRAME")),
            ("overrideKey", guid(5, 5)),
        ])
    };
    let (a, b) = (frame(), frame());
    let tree = Tree {
        nodes: HashMap::from([
            ("inst".into(), &inst),
            ("1:1".into(), &main),
            ("a".into(), &a),
            ("b".into(), &b),
        ]),
        children: HashMap::from([
            ("1:1".into(), vec!["a".into()]),
            ("a".into(), vec!["b".into()]),
        ]),
        blobs: Vec::new(),
    };
    let ex = expand_instances(&tree, "inst");
    assert_eq!(
        (ex.children["inst"].clone(), ex.children.get("Iinst;5:5")),
        (vec!["Iinst;5:5".to_owned()], None)
    );
}

#[test]
fn boolean_component_properties_hide_nested_subtrees() {
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 1))])),
        (
            "componentPropAssignments",
            Value::Array(vec![obj(vec![
                ("defID", guid(2, 1)),
                (
                    "varValue",
                    obj(vec![(
                        "value",
                        obj(vec![("boolValue", Value::Bool(false))]),
                    )]),
                ),
            ])]),
        ),
    ]);
    let main = obj(vec![]);
    let child = obj(vec![
        ("type", Value::Enum("NodeType", "FRAME")),
        (
            "componentPropRefs",
            Value::Array(vec![obj(vec![
                ("defID", guid(2, 1)),
                ("componentPropNodeField", Value::Enum("Field", "VISIBLE")),
            ])]),
        ),
    ]);
    let leaf = obj(vec![]);
    let tree = Tree {
        nodes: HashMap::from([
            ("inst".into(), &inst),
            ("1:1".into(), &main),
            ("child".into(), &child),
            ("leaf".into(), &leaf),
        ]),
        children: HashMap::from([
            ("1:1".into(), vec!["child".into()]),
            ("child".into(), vec!["leaf".into()]),
        ]),
        blobs: Vec::new(),
    };
    let ex = expand_instances(&tree, "inst");
    assert_eq!(
        ex.nodes["Iinst;child"].props.get("visible"),
        Some(&Json::Bool(false))
    );
    assert!(!ex.nodes.contains_key("Iinst;leaf"));
}

#[test]
fn text_component_properties_reach_expanded_children_without_replacing_overrides() {
    for (form, text, explicit) in [
        (0, "查看全部", false),
        (1, "Plain label", false),
        (2, "String label", false),
        (2, "按钮文字", false),
        (3, "Fallback label", false),
        (0, "", false),
        (0, "Property label", true),
    ] {
        let text_data =
            |characters: &str| obj(vec![("characters", Value::String(characters.into()))]);
        let assignment = |characters: &str| {
            let value = obj(vec![(
                if form == 0 {
                    "textDataValue"
                } else {
                    "textValue"
                },
                if form == 2 {
                    Value::String(characters.into())
                } else {
                    text_data(characters)
                },
            )]);
            let mut fields = vec![
                ("defID", guid(2, 1)),
                if form == 0 {
                    ("varValue", obj(vec![("value", value)]))
                } else {
                    ("value", value)
                },
            ];
            if form == 3 {
                fields.push(("varValue", obj(vec![("value", obj(vec![]))])));
            }
            Value::Array(vec![obj(fields)])
        };
        let inst = obj(vec![
            ("type", Value::Enum("NodeType", "INSTANCE")),
            (
                "symbolData",
                obj(vec![
                    ("symbolID", guid(1, 1)),
                    (
                        "symbolOverrides",
                        Value::Array(vec![obj(vec![
                            (
                                "guidPath",
                                obj(vec![("guids", Value::Array(vec![guid(1, 3)]))]),
                            ),
                            ("componentPropAssignments", assignment(text)),
                        ])]),
                    ),
                ]),
            ),
            ("componentPropAssignments", assignment(text)),
        ]);
        let main = obj(vec![]);
        let nested = obj(vec![
            ("type", Value::Enum("NodeType", "INSTANCE")),
            (
                "symbolData",
                obj(vec![
                    ("symbolID", guid(1, 2)),
                    (
                        "symbolOverrides",
                        Value::Array(if explicit {
                            vec![obj(vec![
                                (
                                    "guidPath",
                                    obj(vec![("guids", Value::Array(vec![guid(1, 4)]))]),
                                ),
                                ("textData", text_data("Explicit label")),
                            ])]
                        } else {
                            vec![]
                        }),
                    ),
                ]),
            ),
            ("componentPropAssignments", assignment("Nested default")),
        ]);
        let child = obj(vec![
            ("type", Value::Enum("NodeType", "TEXT")),
            (
                "textData",
                obj(vec![
                    ("characters", Value::String("按钮文字".into())),
                    ("characterStyleIDs", Value::Array(vec![Value::UInt(1); 4])),
                    (
                        "styleOverrideTable",
                        Value::Array(vec![obj(vec![
                            ("styleID", Value::UInt(1)),
                            ("fontSize", Value::Float(22.0)),
                        ])]),
                    ),
                    (
                        "lines",
                        Value::Array(vec![obj(vec![
                            ("lineType", Value::Enum("LineType", "UNORDERED_LIST")),
                            ("indentationLevel", Value::UInt(2)),
                        ])]),
                    ),
                ]),
            ),
            (
                "componentPropRefs",
                Value::Array(vec![obj(vec![
                    ("defID", guid(2, 1)),
                    ("componentPropNodeField", Value::Enum("Field", "TEXT_DATA")),
                ])]),
            ),
        ]);
        let tree = Tree {
            nodes: HashMap::from([
                ("inst".into(), &inst),
                ("1:1".into(), &main),
                ("1:2".into(), &main),
                ("1:3".into(), &nested),
                ("1:4".into(), &child),
            ]),
            children: HashMap::from([
                ("1:1".into(), vec!["1:4".into(), "1:3".into()]),
                ("1:2".into(), vec!["1:4".into()]),
            ]),
            blobs: Vec::new(),
        };
        let ex = expand_instances(&tree, "inst");
        assert_eq!(ex.nodes["Iinst;1:4"].props["textData"]["characters"], text);
        let doc = crate::rest::nodes(&tree, "inst");
        let direct = &doc["document"]["children"][0];
        assert_eq!(direct["characters"], text);
        assert_eq!(
            direct["characterStyleOverrides"],
            serde_json::json!([1, 1, 1, 1])
        );
        assert_eq!(direct["styleOverrideTable"]["1"]["fontSize"], 22.0);
        assert_eq!(direct["lineTypes"], serde_json::json!(["UNORDERED"]));
        assert_eq!(direct["lineIndentations"], serde_json::json!([2]));
        assert_eq!(
            ex.nodes["Iinst;1:3;1:4"].props["textData"]["characters"],
            if explicit { "Explicit label" } else { text }
        );
    }
}

#[test]
fn swaps_use_the_new_mains_visibility_unless_overridden() {
    let main = obj(vec![]);
    let child = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("visible", Value::Bool(false)),
    ]);
    let target = obj(vec![("visible", Value::Bool(true))]);
    let leaf = obj(vec![]);
    let tree = Tree {
        nodes: HashMap::from([
            ("main".into(), &main),
            ("child".into(), &child),
            ("target".into(), &target),
            ("leaf".into(), &leaf),
        ]),
        children: HashMap::from([
            ("main".into(), vec!["child".into()]),
            ("target".into(), vec!["leaf".into()]),
        ]),
        blobs: Vec::new(),
    };
    for visible in [None, Some(false)] {
        let mut ex = Expander {
            bounds: None,
            depth: 0,
            sizes: HashMap::new(),
            tree: &tree,
            out: Expansion {
                nodes: HashMap::new(),
                children: HashMap::new(),
            },
            symbols: Vec::new(),
            budget: None,
            exceeded: false,
        };
        let ov = Patches::from([(
            "child".into(),
            Patch {
                swap: Some("target".into()),
                visible,
                ..Patch::default()
            },
        )]);
        ex.expand(
            "outer",
            "outer",
            "main",
            &[],
            &ov,
            &Patches::new(),
            Mat::IDENTITY,
            None,
            1.0,
        );
        assert_eq!(
            ex.out.nodes.contains_key("Iouter;child;leaf"),
            visible != Some(false)
        );
    }
}

#[test]
fn symbol_properties_on_nested_overrides_swap_the_referenced_instance() {
    let assignment = Value::Array(vec![obj(vec![
        ("defID", guid(2, 1)),
        (
            "varValue",
            obj(vec![(
                "value",
                obj(vec![("symbolIdValue", obj(vec![("guid", guid(1, 4))]))]),
            )]),
        ),
    ])]);
    let inst = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        (
            "symbolData",
            obj(vec![
                ("symbolID", guid(1, 1)),
                (
                    "symbolOverrides",
                    Value::Array(vec![obj(vec![
                        (
                            "guidPath",
                            obj(vec![("guids", Value::Array(vec![guid(1, 3)]))]),
                        ),
                        ("componentPropAssignments", assignment),
                    ])]),
                ),
            ]),
        ),
    ]);
    let main = obj(vec![]);
    let nested = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("symbolData", obj(vec![("symbolID", guid(1, 2))])),
    ]);
    let icon = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("name", Value::String("Refresh".into())),
        (
            "componentPropRefs",
            Value::Array(vec![obj(vec![
                ("defID", guid(2, 1)),
                (
                    "componentPropNodeField",
                    Value::Enum("Field", "OVERRIDDEN_SYMBOL_ID"),
                ),
            ])]),
        ),
    ]);
    let target = obj(vec![("name", Value::String("Close".into()))]);
    let leaf = obj(vec![]);
    let tree = Tree {
        nodes: HashMap::from([
            ("inst".into(), &inst),
            ("1:1".into(), &main),
            ("1:2".into(), &main),
            ("1:3".into(), &nested),
            ("icon".into(), &icon),
            ("1:4".into(), &target),
            ("leaf".into(), &leaf),
        ]),
        children: HashMap::from([
            ("1:1".into(), vec!["1:3".into()]),
            ("1:2".into(), vec!["icon".into()]),
            ("1:4".into(), vec!["leaf".into()]),
        ]),
        blobs: Vec::new(),
    };
    let ex = expand_instances(&tree, "inst");
    assert_eq!(ex.nodes["Iinst;1:3;icon"].source, "1:4");
    assert_eq!(ex.nodes["Iinst;1:3;icon"].name, "Close");
    assert!(ex.nodes.contains_key("Iinst;1:3;icon;leaf"));
}

#[test]
fn state_group_frames_are_component_sets() {
    let node = Value::Object(
        "NodeChange",
        HashMap::from([
            ("type", Value::Enum("NodeType", "FRAME")),
            ("isStateGroup", Value::Bool(true)),
        ]),
    );
    assert_eq!(plugin_type(&node), "COMPONENT_SET");
}

#[test]
fn stretch_adds_the_container_delta_and_scale_multiplies() {
    let resize = Some((v2(100.0, 100.0), v2(150.0, 200.0)));
    let size = Some(v2(40.0, 40.0));
    assert_eq!(
        constrain(size, Some("STRETCH"), Some("SCALE"), resize),
        Some(v2(90.0, 80.0))
    );
}

#[test]
fn other_constraints_keep_the_size() {
    let resize = Some((v2(100.0, 100.0), v2(150.0, 200.0)));
    assert_eq!(
        constrain(Some(v2(40.0, 40.0)), Some("MIN"), None, resize),
        Some(v2(40.0, 40.0))
    );
}

#[test]
fn quarter_turn_swaps_bbox_axes() {
    let turn = Mat {
        a: 0.0,
        b: 1.0,
        c: -1.0,
        d: 0.0,
        ..Mat::IDENTITY
    };
    assert_eq!(turn.bbox(v2(10.0, 20.0)), v2(20.0, 10.0));
}

#[test]
fn child_origin_adds_parent_translation_and_rotation() {
    let parent = Mat {
        tx: 100.0,
        ty: 50.0,
        ..Mat::IDENTITY
    };
    let turned_child = Mat {
        a: 0.0,
        b: 1.0,
        c: -1.0,
        d: 0.0,
        tx: 30.0,
        ty: 10.0,
    };
    // A quarter turn about the child's own origin puts its box left of it.
    assert_eq!(
        (parent * turned_child).origin(v2(10.0, 20.0)),
        v2(110.0, 60.0)
    );
}

#[test]
fn inverse_undoes_the_transform() {
    let m = Mat {
        a: 0.0,
        b: 2.0,
        c: -1.0,
        d: 0.0,
        tx: 4.0,
        ty: -2.0,
    };
    assert_eq!(m.inverse().apply(m.apply(v2(1.0, 2.0))), v2(1.0, 2.0));
}

#[test]
fn outer_override_wins_per_field() {
    let outer = Patch {
        name: Some("outer".into()),
        ..Patch::default()
    };
    let inner = Patch {
        name: Some("inner".into()),
        visible: Some(false),
        ..Patch::default()
    };
    let merged = outer.over(inner);
    assert_eq!(
        (merged.name.as_deref(), merged.visible),
        (Some("outer"), Some(false))
    );
}

#[test]
fn outer_entry_without_size_keeps_the_nested_size() {
    // Seen in a real file: the outer derived entry only holds fillGeometry.
    let outer = Patches::from([("a;b".to_owned(), Patch::default())]);
    let inner = Patch {
        size: Some(v2(36.0, 36.0)),
        ..Patch::default()
    };
    let merged = nest(&outer, "a", Patches::from([("b".to_owned(), inner)]));
    assert_eq!(merged["a;b"].size, Some(v2(36.0, 36.0)));
}

#[test]
fn entries_on_the_same_path_keep_both_swap_and_size() {
    // Seen in a real file: one entry swaps the node, a later one sets its size.
    let guid = |s, l| {
        Value::Object(
            "GUID",
            HashMap::from([("sessionID", Value::UInt(s)), ("localID", Value::UInt(l))]),
        )
    };
    let path = || {
        Value::Object(
            "GUIDPath",
            HashMap::from([("guids", Value::Array(vec![guid(7, 42)]))]),
        )
    };
    let size = Value::Object(
        "Vector",
        HashMap::from([("x", Value::Float(10.0)), ("y", Value::Float(20.0))]),
    );
    let list = Value::Array(vec![
        Value::Object(
            "SymbolOverride",
            HashMap::from([("guidPath", path()), ("overriddenSymbolID", guid(18, 1))]),
        ),
        Value::Object(
            "SymbolOverride",
            HashMap::from([("guidPath", path()), ("size", size)]),
        ),
    ]);
    let patch = &entries(Some(&list), None)["7:42"];
    assert_eq!(
        (patch.swap.as_deref(), patch.size),
        (Some("18:1"), Some(v2(10.0, 20.0)))
    );
}
#[test]
fn an_estimate_stops_at_the_remaining_work() {
    let tree = Tree {
        nodes: HashMap::new(),
        children: HashMap::new(),
        blobs: Vec::new(),
    };
    let mut ex = Expander {
        bounds: Some(Bounds {
            work: 10,
            bytes: usize::MAX,
            depth: 8,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
            stop: None,
        }),
        depth: 0,
        sizes: HashMap::new(),
        tree: &tree,
        out: Expansion {
            nodes: HashMap::new(),
            children: HashMap::new(),
        },
        symbols: Vec::new(),
        budget: None,
        exceeded: false,
    };
    let payload = Value::Array(vec![Value::Int(0); 100_000]);
    let mut visited = 0;
    let estimate = |left: &mut usize| {
        let before = *left;
        let bytes = kiwi_bytes(&payload, left);
        visited = before - *left;
        bytes
    };
    assert_eq!(ex.spend_estimated(0, 0, estimate), None);
    assert_eq!((visited, ex.bounds.unwrap().stop), (11, Some(Stop::Work)));
}

#[test]
fn an_exhausted_estimate_stops_enumerating_its_collection() {
    let mut yielded = 0;
    let items = (0..100_000).inspect(|_| yielded += 1);
    let mut left = 10;
    sum_while(items, &mut left, |_, left| {
        *left -= 1;
        1
    });
    assert_eq!(yielded, 11);
}

fn transform(tx: f32, ty: f32) -> Value<'static> {
    obj(vec![
        ("m00", Value::Float(1.0)),
        ("m11", Value::Float(1.0)),
        ("m02", Value::Float(tx)),
        ("m12", Value::Float(ty)),
    ])
}

fn row(guids: Vec<Value<'static>>, fields: Vec<(&'static str, Value<'static>)>) -> Value<'static> {
    let mut all = vec![("guidPath", obj(vec![("guids", Value::Array(guids))]))];
    all.extend(fields);
    obj(all)
}

/// An instance of main `1:1` stored at `size`, scaled by `factor` with the Scale tool.
fn scaled_instance(
    factor: Option<f32>,
    size_: f32,
    overrides: Vec<Value<'static>>,
    derived: Vec<Value<'static>>,
) -> Value<'static> {
    let mut sd = vec![
        ("symbolID", guid(1, 1)),
        ("symbolOverrides", Value::Array(overrides)),
    ];
    if let Some(f) = factor {
        sd.push(("uniformScaleFactor", Value::Float(f)));
    }
    obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("size", size(size_, size_)),
        ("symbolData", obj(sd)),
        ("derivedSymbolData", Value::Array(derived)),
    ])
}

/// A 200 px rectangle at (20, 40) in a 256 px main, with every kind of length it can hold.
fn rectangle() -> Value<'static> {
    let px = |v| {
        obj(vec![
            ("value", Value::Float(v)),
            ("units", Value::Enum("NumberUnits", "PIXELS")),
        ])
    };
    obj(vec![
        ("type", Value::Enum("NodeType", "RECTANGLE")),
        ("size", size(200.0, 200.0)),
        ("transform", transform(20.0, 40.0)),
        ("cornerRadius", Value::Float(64.0)),
        ("strokeWeight", Value::Float(8.0)),
        (
            "dashPattern",
            Value::Array(vec![Value::Float(16.0), Value::Float(8.0)]),
        ),
        (
            "effects",
            Value::Array(vec![obj(vec![
                ("radius", Value::Float(20.0)),
                ("spread", Value::Float(4.0)),
                ("offset", size(10.0, 6.0)),
                ("startOffset", size(0.5, 0.0)),
            ])]),
        ),
        ("fontSize", Value::Float(16.0)),
        ("letterSpacing", px(2.0)),
        ("minSize", obj(vec![("value", size(12.0, 0.0))])),
        (
            "textData",
            obj(vec![(
                "styleOverrideTable",
                Value::Array(vec![obj(vec![
                    ("styleID", Value::UInt(1)),
                    ("fontSize", Value::Float(20.0)),
                    ("lineHeight", px(24.0)),
                    (
                        "letterSpacing",
                        obj(vec![
                            ("value", Value::Float(5.0)),
                            ("units", Value::Enum("NumberUnits", "PERCENT")),
                        ]),
                    ),
                ])]),
            )]),
        ),
        (
            "lineHeight",
            obj(vec![
                ("value", Value::Float(150.0)),
                ("units", Value::Enum("NumberUnits", "PERCENT")),
            ]),
        ),
    ])
}

fn rectangle_tree<'a>(
    inst: &'a Value<'static>,
    main: &'a Value<'static>,
    rect: &'a Value<'static>,
) -> Tree<'a> {
    Tree {
        nodes: HashMap::from([
            ("inst".into(), inst),
            ("1:1".into(), main),
            ("1:2".into(), rect),
        ]),
        children: HashMap::from([("1:1".into(), vec!["1:2".into()])]),
        blobs: Vec::new(),
    }
}

#[test]
fn a_scaled_instance_reads_its_mains_lengths_at_its_factor() {
    let inst = scaled_instance(Some(0.5), 128.0, vec![], vec![]);
    let main = obj(vec![("size", size(256.0, 256.0))]);
    let rect = rectangle();
    let ex = expand_instances(&rectangle_tree(&inst, &main, &rect), "inst");
    let e = &ex.nodes["Iinst;1:2"];
    assert_eq!(
        (e.size, e.matrix.tx, e.matrix.ty, e.scale),
        (Some(v2(100.0, 100.0)), 10.0, 20.0, 0.5)
    );
    assert_eq!(
        serde_json::json!([
            e.props["cornerRadius"],
            e.props["strokeWeight"],
            e.props["dashPattern"],
            e.props["effects"][0]["radius"],
            e.props["effects"][0]["spread"],
            e.props["effects"][0]["offset"],
            e.props["effects"][0]["startOffset"],
            e.props["fontSize"],
            e.props["letterSpacing"]["value"],
            e.props["minSize"]["value"],
        ]),
        serde_json::json!([32.0, 4.0, [8.0, 4.0], 10.0, 2.0, {"x": 5.0, "y": 3.0}, {"x": 0.5, "y": 0.0}, 8.0, 1.0, {"x": 6.0, "y": 0.0}])
    );
    // A percent line height is no length.
    assert!(!e.props.contains_key("lineHeight"));
    let range = &e.props["textData"]["styleOverrideTable"][0];
    assert_eq!(
        (
            &range["styleID"],
            &range["fontSize"],
            &range["lineHeight"]["value"],
            &range["letterSpacing"]["value"]
        ),
        (
            &serde_json::json!(1),
            &serde_json::json!(10.0),
            &serde_json::json!(12.0),
            &serde_json::json!(5.0)
        )
    );
}

#[test]
fn a_row_is_at_the_instances_scale_and_a_derived_one_wins() {
    let path = || vec![guid(1, 2)];
    let inst = scaled_instance(
        Some(0.5),
        128.0,
        vec![row(
            path(),
            vec![
                ("cornerRadius", Value::Float(7.0)),
                ("strokeWeight", Value::Float(1.0)),
            ],
        )],
        vec![row(
            path(),
            vec![
                ("size", size(90.0, 90.0)),
                ("strokeWeight", Value::Float(3.0)),
            ],
        )],
    );
    let main = obj(vec![("size", size(256.0, 256.0))]);
    let rect = rectangle();
    let ex = expand_instances(&rectangle_tree(&inst, &main, &rect), "inst");
    let e = &ex.nodes["Iinst;1:2"];
    assert_eq!(
        (
            e.size,
            &e.props["cornerRadius"],
            &e.props["strokeWeight"],
            &e.props["fontSize"]
        ),
        (
            Some(v2(90.0, 90.0)),
            &serde_json::json!(7.0),
            &serde_json::json!(3.0),
            &serde_json::json!(8.0)
        )
    );
}

#[test]
fn a_scaled_then_resized_instance_fits_from_its_scaled_main() {
    // 0.5 makes the 100 px main 50 px; the instance was then resized to 100 px.
    let inst = scaled_instance(Some(0.5), 100.0, vec![], vec![]);
    let main = obj(vec![("size", size(100.0, 100.0))]);
    let child = |constraint| {
        obj(vec![
            ("type", Value::Enum("NodeType", "RECTANGLE")),
            ("size", size(40.0, 40.0)),
            (
                "horizontalConstraint",
                Value::Enum("Constraint", constraint),
            ),
            ("verticalConstraint", Value::Enum("Constraint", constraint)),
        ])
    };
    let (scale, min) = (child("SCALE"), child("MIN"));
    let tree = Tree {
        nodes: HashMap::from([
            ("inst".into(), &inst),
            ("1:1".into(), &main),
            ("1:2".into(), &scale),
            ("1:3".into(), &min),
        ]),
        children: HashMap::from([("1:1".into(), vec!["1:2".into(), "1:3".into()])]),
        blobs: Vec::new(),
    };
    let ex = expand_instances(&tree, "inst");
    assert_eq!(
        (ex.nodes["Iinst;1:2"].size, ex.nodes["Iinst;1:3"].size),
        (Some(v2(40.0, 40.0)), Some(v2(20.0, 20.0)))
    );
}

#[test]
fn nested_scaled_instances_compose_and_their_own_rows_take_the_outer_scale() {
    // Outer 0.5 around Inner 0.5: Inner's main lengths are read at 0.25, while Inner's own
    // override row is stored at the outer main's scale and only halves.
    let outer = scaled_instance(Some(0.5), 100.0, vec![], vec![]);
    let main = obj(vec![("size", size(200.0, 200.0))]);
    let inner = inner_instance(0.5);
    let (inner_main, plain, overridden) = inner_main_and_dots();
    let ex = expand_instances(
        &nested_tree([&outer, &main, &inner, &inner_main, &plain, &overridden]),
        "inst",
    );
    let inner = &ex.nodes["Iinst;1:2"];
    assert_eq!((inner.size, inner.matrix.tx), (Some(v2(50.0, 50.0)), 5.0));
    let plain = &ex.nodes["Iinst;1:2;2:2"];
    assert_eq!(
        (plain.size, &plain.props["cornerRadius"], plain.scale),
        (Some(v2(50.0, 50.0)), &serde_json::json!(10.0), 0.25)
    );
    let own = &ex.nodes["Iinst;1:2;2:3"].props;
    assert_eq!(
        (&own["cornerRadius"], &own["strokeWeight"]),
        (&serde_json::json!(3.0), &serde_json::json!(3.0))
    );
}

#[test]
fn a_composed_factor_is_clamped() {
    // 500 inside 500 composes to 250000, which reads the inner main at the 1000 limit.
    let outer = scaled_instance(Some(500.0), 100_000.0, vec![], vec![]);
    let main = obj(vec![("size", size(200.0, 200.0))]);
    let inner = inner_instance(500.0);
    let (inner_main, plain, overridden) = inner_main_and_dots();
    let ex = expand_instances(
        &nested_tree([&outer, &main, &inner, &inner_main, &plain, &overridden]),
        "inst",
    );
    let plain = &ex.nodes["Iinst;1:2;2:2"];
    assert_eq!(
        (&plain.props["cornerRadius"], plain.scale),
        (&serde_json::json!(40_000.0), 1000.0)
    );
}

/// Instance `1:2` of main `2:1`, scaled by `factor`, with its own override and derived rows for
/// its child `2:3`.
fn inner_instance(factor: f32) -> Value<'static> {
    let own = |field| vec![row(vec![guid(2, 3)], vec![(field, Value::Float(6.0))])];
    obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        ("size", size(200.0 * factor, 200.0 * factor)),
        ("transform", transform(10.0, 10.0)),
        (
            "symbolData",
            obj(vec![
                ("symbolID", guid(2, 1)),
                ("uniformScaleFactor", Value::Float(factor)),
                ("symbolOverrides", Value::Array(own("cornerRadius"))),
            ]),
        ),
        ("derivedSymbolData", Value::Array(own("strokeWeight"))),
    ])
}

/// Main `2:1` holding two 200 px dots of radius 40, `2:2` and `2:3`.
fn inner_main_and_dots() -> (Value<'static>, Value<'static>, Value<'static>) {
    let dot = || {
        obj(vec![
            ("type", Value::Enum("NodeType", "ELLIPSE")),
            ("size", size(200.0, 200.0)),
            ("cornerRadius", Value::Float(40.0)),
        ])
    };
    (obj(vec![("size", size(200.0, 200.0))]), dot(), dot())
}

/// `inst` (main `1:1`) holding `1:2`, an instance of main `2:1` with children `2:2` and `2:3`.
fn nested_tree<'a>(nodes: [&'a Value<'static>; 6]) -> Tree<'a> {
    let ids = ["inst", "1:1", "1:2", "2:1", "2:2", "2:3"];
    Tree {
        nodes: ids.iter().map(|id| id.to_string()).zip(nodes).collect(),
        children: HashMap::from([
            ("1:1".into(), vec!["1:2".into()]),
            ("2:1".into(), vec!["2:2".into(), "2:3".into()]),
        ]),
        blobs: Vec::new(),
    }
}

#[test]
fn an_unresized_scaled_instance_fits_nothing() {
    // 0.3 is not exact in f32: the main's 100 px times it is a few ulps off the stored 30.
    let inst = scaled_instance(Some(0.3), 30.0, vec![], vec![]);
    let main = obj(vec![("size", size(100.0, 100.0))]);
    let child = obj(vec![
        ("type", Value::Enum("NodeType", "RECTANGLE")),
        ("size", size(40.0, 40.0)),
        ("horizontalConstraint", Value::Enum("Constraint", "STRETCH")),
    ]);
    let ex = expand_instances(&rectangle_tree(&inst, &main, &child), "inst");
    let side = 40.0 * f64::from(0.3f32);
    assert_eq!(ex.nodes["Iinst;1:2"].size, Some(v2(side, side)));
}

#[test]
fn a_factor_of_one_missing_or_out_of_range_changes_nothing() {
    for factor in [
        Some(1.0),
        None,
        Some(-0.5),
        Some(0.0),
        Some(1e30),
        Some(f32::NAN),
    ] {
        let inst = scaled_instance(factor, 256.0, vec![], vec![]);
        let main = obj(vec![("size", size(256.0, 256.0))]);
        let rect = rectangle();
        let ex = expand_instances(&rectangle_tree(&inst, &main, &rect), "inst");
        let e = &ex.nodes["Iinst;1:2"];
        assert_eq!(
            (
                e.size,
                e.matrix.tx,
                e.scale,
                e.props.contains_key("cornerRadius")
            ),
            (Some(v2(200.0, 200.0)), 20.0, 1.0, false),
            "{factor:?}"
        );
    }
}

#[test]
fn a_nested_rows_outline_keeps_the_size_it_was_drawn_at() {
    // Inner's own derived row outlines its 200 px dot; at the outer 0.5 the dot is 100 px, so
    // the paths are drawn at 200 px. An outer outline for the same layer is drawn at 100 px.
    let outline = || ("fillGeometry", Value::Array(vec![]));
    for outer_outline in [false, true] {
        let derived = if outer_outline {
            vec![row(vec![guid(1, 2), guid(2, 3)], vec![outline()])]
        } else {
            vec![]
        };
        let outer = scaled_instance(Some(0.5), 100.0, vec![], derived);
        let main = obj(vec![("size", size(200.0, 200.0))]);
        let inner = obj(vec![
            ("type", Value::Enum("NodeType", "INSTANCE")),
            ("size", size(200.0, 200.0)),
            ("symbolData", obj(vec![("symbolID", guid(2, 1))])),
            (
                "derivedSymbolData",
                Value::Array(vec![row(vec![guid(2, 3)], vec![outline()])]),
            ),
        ]);
        let (inner_main, plain, overridden) = inner_main_and_dots();
        let ex = expand_instances(
            &nested_tree([&outer, &main, &inner, &inner_main, &plain, &overridden]),
            "inst",
        );
        let dot = &ex.nodes["Iinst;1:2;2:3"];
        assert_eq!(dot.size, Some(v2(100.0, 100.0)));
        let drawn = (!outer_outline).then(|| serde_json::json!({"x": 200.0, "y": 200.0}));
        assert_eq!(
            dot.props.get("fillGeometrySize").cloned(),
            drawn,
            "{outer_outline}"
        );
        assert!(!dot.props.contains_key("fillGeometryScale"));
    }
}
