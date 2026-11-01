use super::super::tests::{guid, instance, node, obj, path, text};
use super::*;
use crate::search::build_as;
use kiwi_schema::Value as K;

fn fill(r: f64) -> (&'static str, K<'static>) {
    (
        "fillPaints",
        K::Array(vec![obj(vec![
            ("type", K::Enum("PaintType", "SOLID")),
            (
                "color",
                obj(vec![
                    ("r", K::Float(r as f32)),
                    ("g", K::Float(0.0)),
                    ("b", K::Float(0.0)),
                    ("a", K::Float(1.0)),
                ]),
            ),
        ])]),
    )
}

fn size(x: f32, y: f32) -> (&'static str, K<'static>) {
    ("size", obj(vec![("x", K::Float(x)), ("y", K::Float(y))]))
}

fn doc(nodes: Vec<K<'static>>) -> K<'static> {
    let mut all = vec![
        node(0, None, "DOCUMENT", "Document", vec![]),
        node(1, Some(0), "CANVAS", "Page", vec![]),
    ];
    all.extend(nodes);
    obj(vec![("nodeChanges", K::Array(all))])
}

fn screen(title: &str, red: f64, width: f32) -> Vec<K<'static>> {
    vec![
        node(10, Some(1), "FRAME", "Home", vec![size(width, 800.0)]),
        node(11, Some(10), "TEXT", "Title", vec![text(title)]),
        node(12, Some(10), "RECTANGLE", "Card", vec![fill(red)]),
        node(13, Some(10), "RECTANGLE", "Same", vec![fill(0.5)]),
    ]
}

fn req(limit: usize, cursor: Option<String>) -> Request {
    Request {
        node: "0:0".into(),
        limit,
        cursor,
    }
}

fn run(old: K<'static>, new: K<'static>) -> Json {
    let (a, b) = (Tree::new(&old), Tree::new(&new));
    page(
        &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0"),
        &req(LIMIT, None),
        false,
    )
    .unwrap()
}

fn nodes(reply: &Json) -> Vec<(String, String)> {
    reply["frames"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f["nodes"].as_array().unwrap())
        .map(|n| {
            (
                n["id"].as_str().unwrap().to_owned(),
                n["change"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn changed_text_fill_and_size_are_reported_per_facet_and_unchanged_layers_are_not() {
    let out = run(
        doc(screen("Hello", 1.0, 400.0)),
        doc(screen("Hi", 0.0, 420.0)),
    );
    assert_eq!(
        nodes(&out),
        [
            ("0:10".into(), "changed".into()),
            ("0:11".into(), "changed".into()),
            ("0:12".into(), "changed".into())
        ]
    );
    let frame = &out["frames"][0];
    assert_eq!(frame["summary"], "3 listed, text 1, layout 1, appearance 1");
    let title = &frame["nodes"][1];
    assert_eq!(title["facets"], json!(["text"]));
    assert_eq!(
        title["fields"]["textData"]["old"]["value"]["characters"],
        "Hello"
    );
    assert_eq!(
        title["fields"]["textData"]["new"]["value"]["characters"],
        "Hi"
    );
    assert_eq!(
        frame["nodes"][0]["fields"]["size"]["new"]["value"]["x"],
        420.0
    );
    assert_eq!(out["coverage"]["complete"], true);
    assert_eq!(
        run(
            doc(screen("Same", 1.0, 400.0)),
            doc(screen("Same", 1.0, 400.0))
        )["frames"],
        json!([])
    );
}

#[test]
fn a_removed_subtree_is_one_change_and_a_same_name_frame_is_only_a_possible_match() {
    let mut new = screen("Hello", 1.0, 400.0);
    // Same name and children, new ids: never the same frame, only a labelled candidate.
    new[0] = node(20, Some(1), "FRAME", "Home", vec![size(400.0, 800.0)]);
    for (i, n) in new.iter_mut().enumerate().skip(1) {
        *n = node(20 + i as u32, Some(20), "RECTANGLE", "Child", vec![]);
    }
    let out = run(doc(screen("Hello", 1.0, 400.0)), doc(new));
    assert_eq!(
        nodes(&out),
        [
            ("0:10".into(), "removed".into()),
            ("0:20".into(), "added".into())
        ]
    );
    let (old, added) = (&out["frames"][0], &out["frames"][1]);
    assert_eq!(old["nodes"][0]["descendants"], 3);
    assert_eq!(
        old["possibleMatch"],
        json!({"id": "0:20", "basis": "name", "identity": "possible_match"})
    );
    assert_eq!(added["possibleMatch"]["id"], "0:10");
    assert_eq!(out["coverage"]["possibleMatches"], 1);
}

#[test]
fn a_same_id_with_another_kind_is_identity_uncertain() {
    let mut new = screen("Hello", 1.0, 400.0);
    new[2] = node(12, Some(10), "ELLIPSE", "Card", vec![fill(1.0)]);
    let out = run(doc(screen("Hello", 1.0, 400.0)), doc(new));
    let n = &out["frames"][0]["nodes"][0];
    assert_eq!(
        (&n["change"], &n["identity"], &n["oldKind"]),
        (
            &json!("kind_changed"),
            &json!("identity_uncertain"),
            &json!("RECTANGLE")
        )
    );
}

fn with_instance(main: u32, overrides: Vec<K<'static>>) -> K<'static> {
    doc(vec![
        node(30, Some(1), "SYMBOL", "A", vec![]),
        node(31, Some(30), "RECTANGLE", "Bg", vec![fill(1.0)]),
        node(40, Some(1), "SYMBOL", "B", vec![]),
        node(41, Some(40), "RECTANGLE", "Bg", vec![fill(1.0)]),
        node(10, Some(1), "FRAME", "Home", vec![]),
        instance(
            11,
            10,
            "Use",
            main,
            vec![("symbolOverrides", K::Array(overrides))],
        ),
    ])
}

#[test]
fn an_override_equal_to_the_main_value_is_no_change_and_a_swap_names_both_mains() {
    // The override is JSON, the main's field kiwi: both digest the same effective value.
    let same = obj(vec![path(&[31]), fill(1.0)]);
    assert_eq!(
        run(with_instance(30, vec![]), with_instance(30, vec![same]))["frames"],
        json!([])
    );
    let red = obj(vec![path(&[31]), fill(0.0)]);
    let out = run(with_instance(30, vec![]), with_instance(30, vec![red]));
    assert_eq!(nodes(&out), [("I0:11;0:31".into(), "changed".into())]);
    let swapped = run(with_instance(30, vec![]), with_instance(40, vec![]));
    let n = &swapped["frames"][0]["nodes"][0];
    assert_eq!(n["id"], "0:11");
    assert_eq!(n["edges"]["swap"], json!({"old": "0:30", "new": "0:40"}));
    assert!(
        n["edges"]["removed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["target"] == "0:30")
    );
}

#[test]
fn layers_past_a_side_that_stopped_are_unknown_not_removed() {
    let (a, b) = (
        doc(screen("Hello", 1.0, 400.0)),
        doc(screen("Hello", 1.0, 400.0)),
    );
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let work = (1..400)
        .find(|w| {
            build_as(&b, "0:0", super::super::tests::bounds(*w), true)
                .stop
                .is_some_and(|_| {
                    build_as(&b, "0:0", super::super::tests::bounds(*w + 1), true)
                        .stop
                        .is_none()
                })
        })
        .unwrap();
    let pair = compare_trees(&a, &b, [WORK, work, PAIR_WORK], "0:0");
    let out = page(&pair, &req(LIMIT, None), false).unwrap();
    assert!(nodes(&out).iter().all(|(_, c)| c == "unknown"), "{out}");
    assert_eq!(out["coverage"]["complete"], false);
}

#[test]
fn pages_walk_every_change_once_and_a_cursor_of_another_pair_is_stale() {
    let many = |r: f64| {
        doc((0..7)
            .map(|i| node(10 + i, Some(1), "RECTANGLE", "R", vec![fill(r)]))
            .collect())
    };
    let (a, b) = (many(1.0), many(0.0));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let (mut seen, mut cursor) = (Vec::new(), None);
    loop {
        let out = page(&pair, &req(3, cursor), false).unwrap();
        seen.extend(nodes(&out));
        match out["nextCursor"].as_str() {
            Some(c) => cursor = Some(c.to_owned()),
            None => break,
        }
    }
    assert_eq!(seen.len(), 7);
    let first = page(&pair, &req(3, None), false).unwrap();
    // Another selection of the same revisions binds another cursor.
    let other = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:1");
    let err = page(
        &other,
        &req(3, first["nextCursor"].as_str().map(str::to_owned)),
        false,
    )
    .unwrap_err();
    assert_eq!(
        err.downcast_ref::<FigError>().map(FigError::kind),
        Some(ErrorKind::CursorStale)
    );
}

#[test]
fn a_field_too_large_to_read_is_unread_never_unchanged() {
    let big = |r: f64| {
        let paint = obj(vec![
            ("type", K::Enum("PaintType", "SOLID")),
            ("opacity", K::Float(r as f32)),
        ]);
        ("fillPaints", K::Array(vec![paint; VISITS]))
    };
    let a = doc(vec![node(
        10,
        Some(1),
        "RECTANGLE",
        "R",
        vec![big(1.0), size(1.0, 1.0)],
    )]);
    let b = doc(vec![node(
        10,
        Some(1),
        "RECTANGLE",
        "R",
        vec![big(0.5), size(2.0, 1.0)],
    )]);
    let out = run(a, b);
    let n = &out["frames"][0]["nodes"][0];
    assert_eq!(
        (&n["unread"], &n["facets"]),
        (&json!(true), &json!(["layout"]))
    );
    assert_eq!(
        out["coverage"]["reasons"],
        json!([{"reason": "unread_fields", "count": 1}])
    );
    let _ = guid(0);
}

#[test]
fn a_giant_field_is_unread_and_an_escaped_one_is_digested_without_keeping_its_value() {
    let one = |s: &str| doc(vec![node(10, Some(1), "TEXT", "T", vec![text(s)])]);
    // Past the conversion cap: never converted, listed as unknown and unread, not taken for
    // unchanged.
    let giant = "x".repeat(FIELD_BYTES + 1);
    let out = run(one(&giant), one(&giant));
    let n = &out["frames"][0]["nodes"][0];
    assert_eq!(
        (&n["change"], &n["unread"]),
        (&json!("unknown"), &json!(true))
    );
    // Control characters escape to six bytes each; the stream is hashed, the value dropped.
    let escaped = "\u{1}".repeat(FIELD_BYTES / 8);
    let out = run(one(&escaped), one(&format!("{escaped}\u{2}")));
    let n = &out["frames"][0]["nodes"][0];
    assert_eq!((&n["facets"], n.get("unread")), (&json!(["text"]), None));
}

#[test]
fn content_a_now_hidden_instance_leaves_unexpanded_is_unknown_not_removed() {
    let hidden = doc(vec![
        node(30, Some(1), "SYMBOL", "A", vec![]),
        node(31, Some(30), "RECTANGLE", "Bg", vec![fill(1.0)]),
        node(40, Some(1), "SYMBOL", "B", vec![]),
        node(41, Some(40), "RECTANGLE", "Bg", vec![fill(1.0)]),
        node(10, Some(1), "FRAME", "Home", vec![]),
        instance(11, 10, "Use", 30, vec![("visible", K::Bool(false))]),
    ]);
    let changes = nodes(&run(with_instance(30, vec![]), hidden));
    assert!(
        changes.contains(&("I0:11;0:31".into(), "unknown".into())),
        "{changes:?}"
    );
    assert!(!changes.iter().any(|(_, c)| c == "removed"), "{changes:?}");
}

#[test]
fn a_selection_on_one_side_only_lists_its_children_for_drilling_in() {
    let (a, b) = (doc(screen("Hi", 1.0, 100.0)), doc(vec![]));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let out = page(
        &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:10"),
        &Request {
            node: "0:10".into(),
            limit: LIMIT,
            cursor: None,
        },
        false,
    )
    .unwrap();
    assert_eq!(
        nodes(&out),
        ["0:10", "0:11", "0:12", "0:13"].map(|id| (id.to_owned(), "removed".to_owned()))
    );
    assert_eq!(out["coverage"]["new"]["selected"], false);
}

#[test]
fn binary_values_of_equal_length_differ_and_an_override_holding_bytes_is_unread() {
    let blob = |b: u8| {
        (
            "fillPaints",
            K::Array(vec![obj(vec![
                ("type", K::Enum("PaintType", "SOLID")),
                ("blob", K::Bytes(vec![1, b])),
            ])]),
        )
    };
    let one = |b| doc(vec![node(10, Some(1), "RECTANGLE", "R", vec![blob(b)])]);
    let n = &run(one(2), one(3))["frames"][0]["nodes"][0];
    assert_eq!(n["facets"], json!(["appearance"]));
    // The override reaches the facets as JSON with `<2 bytes>`: unknown, not unchanged.
    let same = obj(vec![path(&[31]), blob(2)]);
    let changes = nodes(&run(
        with_instance(30, vec![same.clone()]),
        with_instance(30, vec![same]),
    ));
    assert_eq!(changes, [("I0:11;0:31".to_owned(), "unknown".to_owned())]);
}

#[test]
fn an_unread_field_is_charged_its_metadata_like_a_read_one() {
    let held = (
        "fillPaints",
        K::Array(vec![obj(vec![
            ("type", K::Enum("PaintType", "SOLID")),
            ("blob", K::Bytes(vec![1, 2])),
        ])]),
    );
    let giant = text(&"x".repeat(FIELD_BYTES + 1));
    // Too large to read, a malformed blob, and an override holding bytes.
    for (message, id) in [
        (
            doc(vec![node(10, Some(1), "TEXT", "T", vec![giant])]),
            "0:10",
        ),
        (vector_with(Some(K::UInt(1))), "0:10"),
        (
            with_instance(30, vec![obj(vec![path(&[31]), held])]),
            "I0:11;0:31",
        ),
    ] {
        let tree = Tree::new(&message);
        let bounds = super::super::tests::bounds;
        let (ex, _) = crate::expand::expand_instances_bounded(&tree, "0:0", bounds(WORK));
        let mut b = bounds(WORK);
        let before = b.bytes;
        let f = facets(&tree, id, ex.nodes.get(id), &mut b).unwrap();
        assert!(f.fields.iter().any(|f| f.digest.is_none()), "{id}");
        // Each field pays its metadata, and a kept value its size as JSON; the malformed
        // blob is found only after its stored geometry was converted and copied, which it pays.
        let converted = tree
            .nodes
            .get(id)
            .and_then(|n| n.get("fillGeometry"))
            .map_or(0, |g| 2 * crate::expand::kiwi_bytes(g, &mut VISITS.clone()));
        let kept: usize = f
            .fields
            .iter()
            .map(|f| {
                let value = f.value.as_ref();
                FIELD_META + f.key.len() + value.map_or(0, |v| held_value(v, v.to_string().len()))
            })
            .sum();
        // An expanded size and transform are each built after paying their fixed estimate.
        let e = ex.nodes.get(id);
        let built = usize::from(e.is_some_and(|e| e.size.is_some())) + usize::from(e.is_some());
        assert_eq!(
            before - b.bytes,
            kept + converted + built * OWNED_BYTES,
            "{id}"
        );
    }
}

#[test]
fn a_stored_field_pays_for_its_conversion_before_it_is_converted() {
    // Past what one value keeps, so only the conversion needs the bytes.
    let long = text(&"x".repeat(FIELD_BYTES / 4));
    let message = doc(vec![node(10, Some(1), "TEXT", "T", vec![long])]);
    let tree = Tree::new(&message);
    let mut b = super::super::tests::bounds(WORK);
    b.bytes = 1024;
    assert!(facets(&tree, "0:10", None, &mut b).is_none());
    assert_eq!(b.stop, Some(crate::expand::Stop::Bytes));
}

#[test]
fn a_field_estimate_walks_no_further_than_the_side_has_work_left() {
    let big = json!(vec![0; 5000]);
    // Zero or little work left: the walk stops there, not at `VISITS`.
    assert_eq!(estimate(&Value::Json(&big), 0).1, 0);
    assert_eq!(estimate(&Value::Json(&big), 100).1, 100);
    // A small field takes only its own visits.
    assert_eq!(estimate(&Value::Json(&json!([1, 2])), 100).1, 3);
    // Through `facets`, running out of work stops the side; it is not one unread field.
    let message = doc(vec![node(10, Some(1), "TEXT", "T", vec![text("Hi")])]);
    let tree = Tree::new(&message);
    let mut b = super::super::tests::bounds(WORK);
    b.work = 0;
    assert!(facets(&tree, "0:10", None, &mut b).is_none());
    assert_eq!(b.stop, Some(crate::expand::Stop::Work));
    // With a little work left, a large field spends it all, then stops the side.
    let wide = doc(vec![node(
        10,
        Some(1),
        "TEXT",
        "T",
        vec![("fillPaints", K::Array(vec![K::Float(0.0); 5000]))],
    )]);
    let tree = Tree::new(&wide);
    let mut b = super::super::tests::bounds(WORK);
    b.work = 100;
    assert!(facets(&tree, "0:10", None, &mut b).is_none());
    assert_eq!((b.work, b.stop), (0, Some(crate::expand::Stop::Work)));
}

#[test]
fn a_digest_stream_pays_before_hashing_and_split_writes_pay_as_one() {
    use std::io::Write;
    let digest = |work: usize, chunks: &[&[u8]]| {
        let mut b = super::super::tests::bounds(work);
        let mut w = Digesting {
            hash: Sha256::new(),
            len: 0,
            bounds: &mut b,
        };
        let ok = chunks.iter().all(|c| w.write(c).is_ok());
        let len = w.len;
        (ok, len, b.work)
    };
    // No work left: not one byte is hashed.
    assert_eq!(digest(0, &[b"x"]), (false, 0, 0));
    // 130 bytes are three started units, written whole or in pieces.
    let whole = digest(10, &[&[0; 130]]);
    assert_eq!(whole, (true, 130, 7));
    assert_eq!(digest(10, &[&[0; 1], &[0; 63], &[0; 1], &[0; 65]]), whole);
}

#[test]
fn an_escaped_field_pays_for_its_stream_before_hashing_past_the_side_s_work() {
    // 8,192 control characters estimate at about 130 visits but stream as about 49 KiB.
    let escaped = text(&"\u{1}".repeat(8192));
    let message = doc(vec![node(10, Some(1), "TEXT", "T", vec![escaped])]);
    let tree = Tree::new(&message);
    let mut b = super::super::tests::bounds(WORK);
    b.work = 300;
    assert!(facets(&tree, "0:10", None, &mut b).is_none());
    // The stream stopped where the work ran out, every hashed chunk paid for.
    assert_eq!((b.work, b.stop), (0, Some(crate::expand::Stop::Work)));
}

#[test]
fn an_expanded_size_or_transform_pays_before_it_is_built() {
    let message = doc(vec![
        node(30, Some(1), "SYMBOL", "A", vec![]),
        node(31, Some(30), "RECTANGLE", "Bg", vec![]),
        node(10, Some(1), "FRAME", "Home", vec![]),
        instance(11, 10, "Use", 30, vec![]),
    ]);
    let tree = Tree::new(&message);
    let bounds = super::super::tests::bounds;
    let (ex, _) = crate::expand::expand_instances_bounded(&tree, "0:0", bounds(WORK));
    let id = "I0:11;0:31";
    assert!(facets(&tree, id, ex.nodes.get(id), &mut bounds(WORK)).is_some());
    // Enough for what the record keeps, not for building its expanded transform first.
    let mut b = bounds(WORK);
    b.bytes = OWNED_BYTES - 1;
    assert!(facets(&tree, id, ex.nodes.get(id), &mut b).is_none());
    assert_eq!(b.stop, Some(crate::expand::Stop::Bytes));
}

#[test]
fn each_page_background_setting_is_an_appearance_change() {
    let red = |r: f32| {
        let channel = |k, v| (k, K::Float(v));
        obj(vec![
            channel("r", r),
            channel("g", 0.0),
            channel("b", 0.0),
            channel("a", 1.0),
        ])
    };
    let page = |fields: Vec<(&'static str, K<'static>)>| {
        obj(vec![(
            "nodeChanges",
            K::Array(vec![
                node(0, None, "DOCUMENT", "Document", vec![]),
                node(1, Some(0), "CANVAS", "Page", fields),
            ]),
        )])
    };
    let base = || {
        vec![
            ("backgroundColor", red(1.0)),
            ("backgroundEnabled", K::Bool(true)),
            ("backgroundOpacity", K::Float(1.0)),
        ]
    };
    let with = |key: &'static str, value: K<'static>| {
        let mut fields = base();
        fields.iter_mut().find(|(k, _)| *k == key).unwrap().1 = value;
        fields
    };
    for (setting, changed) in [
        ("backgroundColor", with("backgroundColor", red(0.0))),
        (
            "backgroundEnabled",
            with("backgroundEnabled", K::Bool(false)),
        ),
        (
            "backgroundOpacity",
            with("backgroundOpacity", K::Float(0.5)),
        ),
    ] {
        let out = run(page(base()), page(changed));
        let changes: Vec<_> = out["frames"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|f| f["nodes"].as_array().unwrap())
            .map(|n| (n["id"].clone(), n["change"].clone(), n["facets"].clone()))
            .collect();
        assert_eq!(
            changes,
            [(json!("0:1"), json!("changed"), json!(["appearance"]))],
            "{setting}: {out}"
        );
        assert_eq!(out["coverage"]["complete"], true, "{setting}");
    }
}

#[test]
fn a_page_full_of_long_names_ends_early_and_its_cursor_reaches_every_change() {
    let name = "n".repeat(2000);
    let side = |red: f64| {
        let mut nodes = vec![node(10, Some(1), "FRAME", "Home", vec![])];
        nodes
            .extend((0..200).map(|i| node(100 + i, Some(10), "RECTANGLE", &name, vec![fill(red)])));
        doc(nodes)
    };
    let (a, b) = (side(1.0), side(0.5));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let (mut seen, mut cursor, mut pages) = (0, None, 0);
    loop {
        let out = page(&pair, &req(MAX_LIMIT, cursor), true).unwrap();
        let listed = nodes(&out).len();
        assert!(listed > 0 && listed < 200, "{listed}");
        seen += listed;
        pages += 1;
        match out["nextCursor"].as_str() {
            Some(c) => cursor = Some(c.to_owned()),
            None => break,
        }
    }
    assert_eq!((seen, pages > 1), (200, true));
}

#[test]
fn possible_match_probes_are_charged_and_stop_the_pair_when_out_of_work() {
    let frames = |first: u32| {
        (0..300)
            .map(move |i| node(first + i, Some(1), "FRAME", "A", vec![]))
            .collect()
    };
    let (a, b) = (doc(frames(100)), doc(frames(1000)));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let run = |work| {
        page(
            &compare_trees(&a, &b, [WORK, WORK, work], "0:0"),
            &req(1, None),
            false,
        )
        .unwrap()
    };
    let full = run(PAIR_WORK);
    assert_eq!(full["coverage"]["possibleMatches"], POSSIBLE);
    // Enough for the maps and the 600 changes and frames, not for every probe.
    let short = run(2 * 600 + 600 * 6 + 100);
    // Every change was kept; only the probes ran out.
    assert_eq!(
        (
            &short["coverage"]["changes"],
            &short["coverage"]["complete"]
        ),
        (&json!(600), &json!(false))
    );
    assert!(
        short["coverage"]["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!({"reason": "diff_work_limit"}))
    );
}

#[test]
fn a_cursor_binds_the_retained_comparison_not_only_both_sides() {
    let frames = |first: u32| {
        (0..300)
            .map(move |i| node(first + i, Some(1), "FRAME", "A", vec![]))
            .collect()
    };
    let (a, b) = (doc(frames(100)), doc(frames(1000)));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let next = |work| {
        page(
            &compare_trees(&a, &b, [WORK, WORK, work], "0:0"),
            &req(1, None),
            false,
        )
        .unwrap()["nextCursor"]
            .as_str()
            .map(str::to_owned)
    };
    // Same sides, same changes; only the pair's probes stopped short.
    let short = next(2 * 600 + 600 * 6 + 100);
    let full = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let err = page(&full, &req(1, short), false).unwrap_err();
    assert_eq!(
        err.downcast_ref::<FigError>().map(FigError::kind),
        Some(ErrorKind::CursorStale)
    );
    // A rebuild of the full pair keeps its cursor.
    assert!(page(&full, &req(1, next(PAIR_WORK)), false).is_ok());
}

#[test]
fn a_first_change_too_large_for_a_page_is_an_error_not_an_oversized_page() {
    let long = "n".repeat(PAGE_VALUES + 1);
    let one = |red| doc(vec![node(10, Some(1), "RECTANGLE", &long, vec![fill(red)])]);
    let (a, b) = (one(1.0), one(0.5));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let e = page(&pair, &req(LIMIT, None), false).unwrap_err();
    assert_eq!(
        e.downcast_ref::<FigError>().unwrap().kind(),
        ErrorKind::ResponseBudget
    );
}

#[test]
fn listed_fields_keys_and_markers_count_against_the_page_bytes() {
    // Many fields differ by small values, so their keys and markers outweigh the values.
    const KEYS: [&str; 12] = [
        "stackSpacing",
        "stackHorizontalPadding",
        "stackVerticalPadding",
        "stackPaddingRight",
        "stackPaddingBottom",
        "strokeWeight",
        "borderTopWeight",
        "borderRightWeight",
        "borderBottomWeight",
        "borderLeftWeight",
        "opacity",
        "paragraphSpacing",
    ];
    let side = |v: f32| {
        let fields = || KEYS.iter().map(|k| (*k, K::Float(v))).collect::<Vec<_>>();
        doc(vec![
            node(10, Some(1), "FRAME", "Home", vec![]),
            node(11, Some(10), "RECTANGLE", "One", fields()),
            node(12, Some(10), "RECTANGLE", "Two", fields()),
        ])
    };
    let (a, b) = (side(1.0), side(0.5));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let at = |bytes| page_with(&pair, &req(LIMIT, None), false, [bytes, PAGE_WORK]);
    // The fewest bytes that list a change carry what the reply holds, markers included.
    let bytes = (1..PAGE_VALUES).find(|b| at(*b).is_ok()).unwrap();
    let page = at(bytes).unwrap();
    assert!(serde_json::to_string(&page["frames"]).unwrap().len() <= bytes);
}

#[test]
fn escaped_names_and_edges_count_against_the_page_bytes_at_their_reply_size() {
    // Control characters, quotes and backslashes grow when serialized.
    let odd = |n: usize| "\u{1}\"\\".repeat(n);
    let mixed = format!("{}\n\t\u{8}\u{1f}é\u{7f}", odd(3));
    let exact = serde_json::to_string(&mixed).unwrap().len();
    assert_eq!(escaped_within(&mixed, exact), Some(exact));
    // Over the room: refused, whether its raw length or only its escapes pass it.
    assert_eq!(escaped_within(&mixed, exact - 1), None);
    assert_eq!(escaped_within(&"x".repeat(1 << 20), 10), None);
    let side = |main: u32| {
        doc(vec![
            node(30, Some(1), "SYMBOL", &odd(40), vec![]),
            node(40, Some(1), "SYMBOL", &odd(40), vec![]),
            node(10, Some(1), "FRAME", &odd(300), vec![]),
            instance(11, 10, &odd(300), main, vec![]),
            instance(12, 10, &odd(300), main, vec![]),
        ])
    };
    let (a, b) = (side(30), side(40));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let at = |bytes| page_with(&pair, &req(LIMIT, None), false, [bytes, PAGE_WORK]);
    let bytes = (1..PAGE_VALUES).find(|b| at(*b).is_ok()).unwrap();
    let page = at(bytes).unwrap();
    assert!(serde_json::to_string(&page["frames"]).unwrap().len() <= bytes);
}

#[test]
fn a_page_of_edges_fields_and_swaps_serializes_within_its_bytes_at_every_budget() {
    // A frame painting 40 other images has 80 differing uses_image edges, and its opacity
    // differs: edges never crowd out the fields' markers, at any budget that lists the frame.
    let side = |first: u8, v: f32| {
        let image = |h: u8| {
            obj(vec![
                ("type", K::Enum("PaintType", "IMAGE")),
                ("image", obj(vec![("hash", K::Bytes(vec![h]))])),
            ])
        };
        let paints = K::Array((first..first + 40).map(image).collect());
        let fields = vec![("opacity", K::Float(v)), ("fillPaints", paints)];
        doc(vec![node(10, Some(1), "FRAME", "Home", fields)])
    };
    let (a, b) = (side(0, 1.0), side(100, 0.5));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let at = |bytes| page_with(&pair, &req(LIMIT, None), false, [bytes, PAGE_WORK]);
    let first = (1..PAGE_VALUES).find(|b| at(*b).is_ok()).unwrap();
    // The least budget lists the fields while most edges are only counted.
    let least = at(first).unwrap();
    assert!(least["frames"][0]["nodes"][0]["edges"]["truncated"].as_u64() > Some(40));
    for bytes in first..first + 600 {
        let page = at(bytes).unwrap();
        let size = serde_json::to_string(&page["frames"]).unwrap().len();
        assert!(size <= bytes, "{size} > {bytes}");
        let node = &page["frames"][0]["nodes"][0];
        assert!(node["fields"]["opacity"].is_object(), "{bytes}");
    }
}

#[test]
fn edges_past_the_page_work_end_the_page_or_fail_the_first_change() {
    let side = |main: u32| {
        doc(vec![
            node(30, Some(1), "SYMBOL", "A", vec![]),
            node(40, Some(1), "SYMBOL", "B", vec![]),
            node(10, Some(1), "FRAME", "Home", vec![]),
            instance(11, 10, "One", main, vec![]),
            instance(12, 10, "Two", main, vec![]),
        ])
    };
    let (a, b) = (side(30), side(40));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let at = |work, cursor| page_with(&pair, &req(LIMIT, cursor), false, [PAGE_VALUES, work]);
    let e = at(0, None).unwrap_err();
    assert_eq!(
        e.downcast_ref::<FigError>().unwrap().kind(),
        ErrorKind::QueryWorkLimit
    );
    // The least work that lists one changed instance lists only it; the cursor reaches the other.
    let work = (1..100).find(|w| at(*w, None).is_ok()).unwrap();
    let first = at(work, None).unwrap();
    let next = first["nextCursor"].as_str().map(str::to_owned);
    let instances = |page: &Json| {
        nodes(page)
            .into_iter()
            .filter(|(id, _)| id == "0:11" || id == "0:12")
            .count()
    };
    assert_eq!(instances(&first), 1);
    let second = at(work, next).unwrap();
    let all: Vec<_> = nodes(&first)
        .into_iter()
        .chain(nodes(&second))
        .map(|(id, _)| id)
        .collect();
    assert!(
        all.contains(&"0:11".to_owned()) && all.contains(&"0:12".to_owned()),
        "{all:?}"
    );
    assert!(
        first["frames"][0]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n["edges"].is_object())
    );
}

#[test]
fn a_vector_path_is_compared_by_its_blob_bytes_not_its_blob_index() {
    let file = |at: usize, bytes: Vec<u8>| {
        let mut blobs = vec![obj(vec![("bytes", K::Bytes(vec![9]))]); 2];
        blobs[at] = obj(vec![("bytes", K::Bytes(bytes))]);
        let geometry = obj(vec![("commandsBlob", K::UInt(at as u32))]);
        let K::Object(name, mut fields) = doc(vec![node(
            10,
            Some(1),
            "VECTOR",
            "Icon",
            vec![("fillGeometry", K::Array(vec![geometry]))],
        )]) else {
            unreachable!()
        };
        fields.insert("blobs", K::Array(blobs));
        K::Object(name, fields)
    };
    let n = &run(file(0, vec![1, 2]), file(0, vec![1, 3]))["frames"][0]["nodes"][0];
    assert_eq!(n["facets"], json!(["appearance"]));
    assert_eq!(
        run(file(0, vec![1, 2]), file(1, vec![1, 2]))["frames"],
        json!([])
    );
}

#[test]
fn derived_text_layout_is_text_and_its_glyphs_compare_by_blob_bytes() {
    let file = |line: f64, at: usize, bytes: Vec<u8>| {
        let mut blobs = vec![obj(vec![("bytes", K::Bytes(vec![9]))]); 2];
        blobs[at] = obj(vec![("bytes", K::Bytes(bytes))]);
        let derived = obj(vec![
            (
                "fontMetaData",
                K::Array(vec![obj(vec![("fontLineHeight", K::Float(line as f32))])]),
            ),
            (
                "glyphs",
                K::Array(vec![obj(vec![("commandsBlob", K::UInt(at as u32))])]),
            ),
        ]);
        let K::Object(name, mut fields) = doc(vec![node(
            10,
            Some(1),
            "TEXT",
            "Title",
            vec![text("Hi"), ("derivedTextData", derived)],
        )]) else {
            unreachable!()
        };
        fields.insert("blobs", K::Array(blobs));
        K::Object(name, fields)
    };
    let facets = |a, b| run(a, b)["frames"][0]["nodes"][0]["facets"].clone();
    assert_eq!(
        facets(file(1.2, 0, vec![1]), file(1.5, 0, vec![1])),
        json!(["text"])
    );
    assert_eq!(
        facets(file(1.2, 0, vec![1]), file(1.2, 0, vec![2])),
        json!(["text"])
    );
    assert_eq!(
        run(file(1.2, 0, vec![1]), file(1.2, 1, vec![1]))["frames"],
        json!([])
    );
}

#[test]
fn a_glyph_outline_moved_to_another_glyph_is_a_text_change() {
    // Glyphs at x 0 and 20; one names blob `at` (its outline), the other none (a space).
    let file = |outlined: usize, at: u32| {
        let glyph = |i: usize| {
            let mut fields = vec![(
                "position",
                obj(vec![("x", K::Float(20.0 * i as f32)), ("y", K::Float(0.0))]),
            )];
            if i == outlined {
                fields.push(("commandsBlob", K::UInt(at)));
            }
            obj(fields)
        };
        let derived = obj(vec![("glyphs", K::Array((0..2).map(glyph).collect()))]);
        let K::Object(name, mut fields) = doc(vec![node(
            10,
            Some(1),
            "TEXT",
            "Title",
            vec![text("I "), ("derivedTextData", derived)],
        )]) else {
            unreachable!()
        };
        let blob = obj(vec![("bytes", K::Bytes(vec![1]))]);
        fields.insert("blobs", K::Array(vec![blob.clone(), blob]));
        K::Object(name, fields)
    };
    let facets = |a, b| run(a, b)["frames"][0]["nodes"][0]["facets"].clone();
    assert_eq!(
        (
            facets(file(0, 0), file(1, 0)),
            run(file(0, 0), file(0, 1))["frames"].clone()
        ),
        (json!(["text"]), json!([]))
    );
}

#[test]
fn an_unchanged_long_text_layout_is_read_and_unchanged() {
    let file = || {
        let glyph = |i: u32| {
            obj(vec![
                ("commandsBlob", K::UInt(0)),
                (
                    "position",
                    obj(vec![("x", K::Float(i as f32)), ("y", K::Float(0.0))]),
                ),
                ("fontSize", K::Float(12.0)),
                ("firstCharacter", K::UInt(i)),
                ("advance", K::Float(7.0)),
            ])
        };
        let derived = obj(vec![("glyphs", K::Array((0..1500).map(glyph).collect()))]);
        let K::Object(name, mut fields) = doc(vec![node(
            10,
            Some(1),
            "TEXT",
            "Body",
            vec![text("long"), ("derivedTextData", derived)],
        )]) else {
            unreachable!()
        };
        fields.insert(
            "blobs",
            K::Array(vec![obj(vec![("bytes", K::Bytes(vec![1]))])]),
        );
        K::Object(name, fields)
    };
    let out = run(file(), file());
    assert_eq!(
        (&out["frames"], &out["coverage"]["complete"]),
        (&json!([]), &json!(true))
    );
}

#[test]
fn a_re_derived_instance_text_with_font_digest_bytes_is_read_and_unchanged() {
    let file = |line: f32| {
        let derived = obj(vec![(
            "fontMetaData",
            K::Array(vec![obj(vec![
                ("fontDigest", K::Bytes(vec![1, 2, 3])),
                ("fontLineHeight", K::Float(line)),
            ])]),
        )]);
        doc(vec![
            node(30, Some(1), "SYMBOL", "Card", vec![]),
            node(31, Some(30), "TEXT", "Label", vec![text("Hi")]),
            node(10, Some(1), "FRAME", "Home", vec![]),
            instance(
                11,
                10,
                "Use",
                30,
                vec![(
                    "derivedSymbolData",
                    K::Array(vec![obj(vec![path(&[31]), ("derivedTextData", derived)])]),
                )],
            ),
        ])
    };
    let same = run(file(1.2), file(1.2));
    assert_eq!(
        (&same["frames"], &same["coverage"]["complete"]),
        (&json!([]), &json!(true))
    );
    // The rest of the re-derived layout is still compared.
    let moved = run(file(1.2), file(1.5));
    assert_eq!(
        nodes(&moved),
        vec![("I0:11;0:31".to_owned(), "changed".to_owned())]
    );
}

#[test]
fn a_stored_layout_differing_only_in_font_digest_is_unchanged() {
    // Other bytes in the layout make it hashed raw; the digest is still left out.
    let one = |digest: u8| {
        let layout = obj(vec![
            ("glyphBytes", K::Bytes(vec![7, 7])),
            (
                "fontMetaData",
                K::Array(vec![obj(vec![("fontDigest", K::Bytes(vec![digest]))])]),
            ),
        ]);
        doc(vec![node(
            10,
            Some(1),
            "TEXT",
            "T",
            vec![text("Hi"), ("derivedTextData", layout)],
        )])
    };
    assert_eq!(run(one(1), one(2))["frames"], json!([]));
}

#[test]
fn an_unread_field_alone_is_unknown_and_proves_no_facet_change() {
    let one = |s: &str, fill_red: f64| {
        doc(vec![node(
            10,
            Some(1),
            "TEXT",
            "T",
            vec![text(s), fill(fill_red)],
        )])
    };
    let giant = "x".repeat(FIELD_BYTES + 1);
    let change = |a: K<'static>, b: K<'static>| {
        let out = run(a, b);
        let n = out["frames"][0]["nodes"][0].clone();
        (
            n["change"].clone(),
            n["facets"].clone(),
            n["unread"].clone(),
        )
    };
    // Unread on both sides, or read on one and unread on the other: no facet is proven to differ.
    let unknown = (json!("unknown"), Json::Null, json!(true));
    assert_eq!(change(one(&giant, 1.0), one(&giant, 1.0)), unknown);
    // It still names the field that was unread, and the frame lists it without calling it changed.
    let out = run(one("small", 1.0), one(&giant, 1.0));
    assert_eq!(out["frames"][0]["summary"], "1 listed, unread 1");
    assert!(out["frames"][0]["nodes"][0]["fields"]["textData"].is_object());
    assert_eq!(change(one("small", 1.0), one(&giant, 1.0)), unknown);
    // A proven change elsewhere is a change that also says a field was unread.
    assert_eq!(
        change(one(&giant, 1.0), one(&giant, 0.0)),
        (json!("changed"), json!(["appearance"]), json!(true))
    );
}

#[test]
fn a_field_unread_on_either_side_is_listed_with_its_markers() {
    let giant = "x".repeat(FIELD_BYTES + 1);
    let one = |s: Option<&str>, fill_red: f64| {
        let mut fields = vec![fill(fill_red)];
        fields.extend(s.map(text));
        doc(vec![node(10, Some(1), "TEXT", "T", fields)])
    };
    let listed = |a: K<'static>, b: K<'static>| {
        let out = run(a, b);
        let n = &out["frames"][0]["nodes"][0];
        (n["change"].clone(), n["fields"]["textData"].clone())
    };
    let marker = |old: Json, new: Json| json!({"facet": "text", "old": old, "new": new});
    let (unread, absent) = (json!({"unread": true}), json!({"present": false}));
    let small = json!({"value": {"characters": "small"}});
    let cases = [
        (
            Some(&*giant),
            Some(&*giant),
            "unknown",
            marker(unread.clone(), unread.clone()),
        ),
        (
            Some(&*giant),
            Some("small"),
            "unknown",
            marker(unread.clone(), small.clone()),
        ),
        (
            Some("small"),
            Some(&*giant),
            "unknown",
            marker(small, unread.clone()),
        ),
        (
            None,
            Some(&*giant),
            "changed",
            marker(absent, unread.clone()),
        ),
    ];
    for (i, (old, new, change, field)) in cases.into_iter().enumerate() {
        assert_eq!(
            listed(one(old, 1.0), one(new, 1.0)),
            (json!(change), field),
            "case {i}"
        );
    }
    // Beside a proven change, the field unread on both sides is still listed.
    let both = run(one(Some(&giant), 1.0), one(Some(&giant), 0.0));
    let fields = &both["frames"][0]["nodes"][0]["fields"];
    assert_eq!(
        (
            fields["textData"].clone(),
            fields["fillPaints"]["facet"].clone()
        ),
        (marker(unread.clone(), unread), json!("appearance"))
    );
}

#[test]
fn emitted_metadata_scopes_code_syntax_and_prototype_settings_are_properties() {
    let one = |key: &'static str, v: &str| {
        doc(vec![node(
            10,
            Some(1),
            "FRAME",
            "F",
            vec![(key, K::String(v.into()))],
        )])
    };
    for key in [
        "description",
        "symbolDescription",
        "styleType",
        "variableScopes",
        "codeSyntax",
        "prototypeInteractions",
    ] {
        let n = &run(one(key, "a"), one(key, "b"))["frames"][0]["nodes"][0];
        assert_eq!(n["facets"], json!(["properties"]), "{key}");
    }
}

#[test]
fn an_image_override_equal_to_the_main_fill_is_no_change() {
    let image = || {
        (
            "fillPaints",
            K::Array(vec![obj(vec![
                ("type", K::Enum("PaintType", "IMAGE")),
                ("image", obj(vec![("hash", K::Bytes(vec![0, 255]))])),
            ])]),
        )
    };
    let file = |overrides: Vec<K<'static>>| {
        doc(vec![
            node(30, Some(1), "SYMBOL", "A", vec![]),
            node(31, Some(30), "RECTANGLE", "Bg", vec![image()]),
            node(10, Some(1), "FRAME", "Home", vec![]),
            instance(
                11,
                10,
                "Use",
                30,
                vec![("symbolOverrides", K::Array(overrides))],
            ),
        ])
    };
    let same = obj(vec![path(&[31]), image()]);
    assert_eq!(run(file(vec![]), file(vec![same]))["frames"], json!([]));
}

#[test]
fn content_left_unexpanded_below_hidden_layers_makes_coverage_partial() {
    let hidden = |red| {
        doc(vec![
            node(30, Some(1), "SYMBOL", "A", vec![]),
            node(31, Some(30), "RECTANGLE", "Bg", vec![fill(red)]),
            node(10, Some(1), "FRAME", "Home", vec![]),
            instance(11, 10, "Use", 30, vec![("visible", K::Bool(false))]),
        ])
    };
    let coverage = &run(hidden(1.0), hidden(0.0))["coverage"];
    assert_eq!(
        (&coverage["complete"], &coverage["new"]["complete"]),
        (&json!(false), &json!(false))
    );
    assert!(
        coverage["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!({"reason": "hidden_content_not_expanded", "side": "new", "count": 1})),
        "{coverage}"
    );
}

fn vector_with(blob: Option<K<'static>>) -> K<'static> {
    let geometry = obj(vec![("commandsBlob", K::UInt(0))]);
    let K::Object(name, mut fields) = doc(vec![node(
        10,
        Some(1),
        "VECTOR",
        "Icon",
        vec![("fillGeometry", K::Array(vec![geometry]))],
    )]) else {
        unreachable!()
    };
    let blobs = blob.map(|b| obj(vec![("bytes", b)])).into_iter().collect();
    fields.insert("blobs", K::Array(blobs));
    K::Object(name, fields)
}

#[test]
fn a_missing_or_malformed_blob_is_unread_never_unchanged() {
    for blob in [None, Some(K::Array(vec![K::UInt(1)])), Some(K::UInt(1))] {
        let out = run(vector_with(blob.clone()), vector_with(blob));
        assert_eq!(nodes(&out), [("0:10".to_owned(), "unknown".to_owned())]);
        assert_eq!(out["coverage"]["complete"], false);
    }
}

#[test]
fn compact_blob_bytes_and_a_byte_array_digest_alike() {
    let array = K::Array(vec![K::Byte(1), K::Byte(2)]);
    let out = run(
        vector_with(Some(K::Bytes(vec![1, 2]))),
        vector_with(Some(array)),
    );
    assert_eq!(out["frames"], json!([]));
}

#[test]
fn a_byte_array_blob_past_the_visit_cap_is_unread() {
    let blob = || Some(K::Array(vec![K::Byte(1); VISITS + 1]));
    let out = run(vector_with(blob()), vector_with(blob()));
    assert_eq!(nodes(&out), [("0:10".to_owned(), "unknown".to_owned())]);
    assert_eq!(out["coverage"]["complete"], false);
}

#[test]
fn a_changed_truncation_line_count_is_a_text_change() {
    let one = |lines| {
        doc(vec![node(
            10,
            Some(1),
            "TEXT",
            "Clamp",
            vec![
                ("textTruncation", K::Enum("TextTruncation", "ENDING")),
                ("maxLines", K::UInt(lines)),
            ],
        )])
    };
    let n = &run(one(1), one(2))["frames"][0]["nodes"][0];
    assert_eq!(n["facets"], json!(["text"]));
}

#[test]
fn a_frame_renamed_in_place_is_a_structure_change_not_removed_and_added() {
    let file = |name| doc(vec![node(10, Some(1), "FRAME", name, vec![])]);
    let (a, b) = (file("Home"), file("Start"));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let out = page(
        &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0"),
        &Request {
            node: "0:0".into(),
            limit: LIMIT,
            cursor: None,
        },
        false,
    )
    .unwrap();
    let node = &out["frames"][0]["nodes"][0];
    assert_eq!(
        (&node["id"], &node["change"], &node["facets"]),
        (&json!("0:10"), &json!("changed"), &json!(["structure"]))
    );
}

#[test]
fn a_selected_node_moved_to_another_parent_is_a_structure_change() {
    let file = |parent| {
        doc(vec![
            node(10, Some(1), "FRAME", "Home", vec![]),
            node(20, Some(10), "FRAME", "Left", vec![]),
            node(30, Some(10), "FRAME", "Right", vec![]),
            node(21, Some(parent), "RECTANGLE", "Card", vec![fill(1.0)]),
        ])
    };
    let (a, b) = (file(20), file(30));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let out = page(
        &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:21"),
        &Request {
            node: "0:21".into(),
            limit: LIMIT,
            cursor: None,
        },
        false,
    )
    .unwrap();
    assert_eq!(out["frames"][0]["nodes"][0]["facets"], json!(["structure"]));
}

#[test]
fn a_selected_context_past_the_side_budget_is_unknown_not_position_zero() {
    let mut nodes = vec![node(10, Some(1), "FRAME", "Home", vec![])];
    nodes.extend((20..30).map(|i| node(i, Some(10), "RECTANGLE", "Card", vec![])));
    let file = doc(nodes);
    let tree = Tree::new(&file);
    let out = page(
        &compare_trees(&tree, &tree, [2, WORK, PAIR_WORK], "0:29"),
        &Request {
            node: "0:29".into(),
            limit: LIMIT,
            cursor: None,
        },
        false,
    )
    .unwrap();
    let reasons = out["coverage"]["reasons"].as_array().unwrap();
    assert!(
        reasons.contains(&json!({"reason": "selection_context_unknown", "side": "old"})),
        "{reasons:?}"
    );
}

#[test]
fn an_explicit_variable_mode_switch_is_an_appearance_change() {
    // Switching the theme leaves every cached paint below as it was; only the frame's mode map
    // changes, and it re-resolves their bound colors.
    let themed = |mode: u32| {
        let modes = obj(vec![(
            "entries",
            K::Array(vec![obj(vec![
                ("variableSetID", obj(vec![("guid", guid(70))])),
                ("variableModeID", guid(mode)),
            ])]),
        )]);
        doc(vec![
            node(
                10,
                Some(1),
                "FRAME",
                "Home",
                vec![("variableModeBySetMap", modes)],
            ),
            node(12, Some(10), "RECTANGLE", "Card", vec![fill(1.0)]),
        ])
    };
    let out = run(themed(71), themed(72));
    assert_eq!(nodes(&out), [("0:10".into(), "changed".into())]);
    assert_eq!(
        out["frames"][0]["nodes"][0]["facets"],
        json!(["appearance"])
    );
}

#[test]
fn a_resized_source_child_under_a_fixed_instance_size_is_an_appearance_change() {
    // The instance keeps its child at 20x20 and the outline bytes stay the same, but the
    // expanded child's outline is scaled from the source size it was drawn at.
    let file = |source: f32| {
        let K::Object(name, mut fields) = doc(vec![
            node(30, Some(1), "SYMBOL", "Icon", vec![]),
            node(
                31,
                Some(30),
                "VECTOR",
                "Glyph",
                vec![
                    size(source, source),
                    (
                        "fillGeometry",
                        K::Array(vec![obj(vec![("commandsBlob", K::UInt(0))])]),
                    ),
                ],
            ),
            // A layer without an outline draws the same at any source size.
            node(
                32,
                Some(30),
                "RECTANGLE",
                "Plate",
                vec![size(source, source)],
            ),
            node(10, Some(1), "FRAME", "Home", vec![]),
            instance(
                11,
                10,
                "Use",
                30,
                vec![(
                    "symbolOverrides",
                    K::Array(vec![
                        obj(vec![path(&[31]), size(20.0, 20.0)]),
                        obj(vec![path(&[32]), size(20.0, 20.0)]),
                    ]),
                )],
            ),
        ]) else {
            unreachable!()
        };
        fields.insert(
            "blobs",
            K::Array(vec![obj(vec![("bytes", K::Bytes(vec![1, 2]))])]),
        );
        K::Object(name, fields)
    };
    let (old, new) = (file(10.0), file(40.0));
    let (a, b) = (Tree::new(&old), Tree::new(&new));
    let out = page(
        &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:11"),
        &req(LIMIT, None),
        false,
    )
    .unwrap();
    assert_eq!(nodes(&out), [("I0:11;0:31".into(), "changed".into())]);
    assert_eq!(
        out["frames"][0]["nodes"][0]["facets"],
        json!(["appearance"])
    );
}

fn alias(id: u32) -> K<'static> {
    obj(vec![
        ("dataType", K::Enum("VariableDataType", "ALIAS")),
        ("value", obj(vec![("alias", obj(vec![("guid", guid(id))]))])),
    ])
}

/// A FLOAT variable 60 holding `value` in one mode, on the page.
fn variable(value: f32) -> K<'static> {
    let data = obj(vec![
        ("dataType", K::Enum("VariableDataType", "FLOAT")),
        ("value", obj(vec![("floatValue", K::Float(value))])),
    ]);
    let entry = obj(vec![("modeID", guid(70)), ("variableData", data)]);
    node(
        60,
        Some(1),
        "VARIABLE",
        "Gap",
        vec![(
            "variableDataValues",
            obj(vec![("entries", K::Array(vec![entry]))]),
        )],
    )
}

/// Frame 10 holds a layer filled from variable 60.
fn bound_screen(value: f32) -> K<'static> {
    let paint = obj(vec![
        ("type", K::Enum("PaintType", "SOLID")),
        ("colorVar", alias(60)),
    ]);
    doc(vec![
        variable(value),
        node(10, Some(1), "FRAME", "Home", vec![]),
        node(
            12,
            Some(10),
            "RECTANGLE",
            "Card",
            vec![("fillPaints", K::Array(vec![paint]))],
        ),
    ])
}

fn scoped(old: K<'static>, new: K<'static>, node: &str) -> Json {
    let (a, b) = (Tree::new(&old), Tree::new(&new));
    let mut r = req(LIMIT, None);
    r.node = node.into();
    page(
        &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], node),
        &r,
        false,
    )
    .unwrap()
}

/// Text style 20 at `font`, used by a layer of frame 10 but outside it.
fn styled(font: f32) -> K<'static> {
    doc(vec![
        node(
            20,
            Some(1),
            "TEXT",
            "Body",
            vec![("fontSize", K::Float(font))],
        ),
        node(10, Some(1), "FRAME", "Home", vec![]),
        node(
            11,
            Some(10),
            "TEXT",
            "Title",
            vec![
                text("Hi"),
                ("styleIdForText", obj(vec![("guid", guid(20))])),
            ],
        ),
    ])
}

#[test]
fn a_text_style_outside_the_scope_makes_coverage_partial_instead_of_unchanged() {
    // The style's own record changes, but only a layer referencing it is in scope.
    let out = scoped(styled(12.0), styled(16.0), "0:10");
    assert_eq!(
        (&out["coverage"]["complete"], &out["coverage"]["reasons"]),
        (
            &json!(false),
            &json!([
                {"reason": "definition_not_compared", "side": "old", "count": 1},
                {"reason": "definition_not_compared", "side": "new", "count": 1},
            ])
        )
    );
}

#[test]
fn a_variable_value_change_over_the_whole_file_is_a_properties_change() {
    let out = run(bound_screen(8.0), bound_screen(12.0));
    let facets = out["frames"][0]["nodes"][0]["facets"].clone();
    assert_eq!(
        (nodes(&out), facets, &out["coverage"]["complete"]),
        (
            vec![("0:60".into(), "changed".into())],
            json!(["properties"]),
            &json!(true)
        )
    );
}

#[test]
fn a_bound_variable_outside_the_scope_makes_coverage_partial() {
    let out = scoped(bound_screen(8.0), bound_screen(8.0), "0:10");
    assert_eq!(
        out["coverage"]["reasons"],
        json!([
            {"reason": "definition_not_compared", "side": "old", "count": 1},
            {"reason": "definition_not_compared", "side": "new", "count": 1},
        ])
    );
}

/// Variable `id` on `parent` with one mode holding `data`.
fn var(id: u32, parent: u32, data: K<'static>) -> K<'static> {
    let entry = obj(vec![("modeID", guid(70)), ("variableData", data)]);
    node(
        id,
        Some(parent),
        "VARIABLE",
        "V",
        vec![(
            "variableDataValues",
            obj(vec![("entries", K::Array(vec![entry]))]),
        )],
    )
}

fn float(value: f32) -> K<'static> {
    obj(vec![
        ("dataType", K::Enum("VariableDataType", "FLOAT")),
        ("value", obj(vec![("floatValue", K::Float(value))])),
    ])
}

/// Page 1 holds a card filled from variable 61 and `vars`; page 2 holds variable 62 = `value`.
fn aliased(vars: Vec<K<'static>>, value: f32) -> K<'static> {
    let paint = obj(vec![
        ("type", K::Enum("PaintType", "SOLID")),
        ("colorVar", alias(61)),
    ]);
    let mut all = vars;
    all.extend([
        node(10, Some(1), "FRAME", "Home", vec![]),
        node(
            12,
            Some(10),
            "RECTANGLE",
            "Card",
            vec![("fillPaints", K::Array(vec![paint]))],
        ),
        node(2, Some(0), "CANVAS", "Other", vec![]),
        var(62, 2, float(value)),
    ]);
    doc(all)
}

fn not_compared(count: usize) -> Json {
    json!([
        {"reason": "definition_not_compared", "side": "old", "count": count},
        {"reason": "definition_not_compared", "side": "new", "count": count},
    ])
}

#[test]
fn an_alias_to_a_variable_on_another_page_makes_a_page_diff_partial() {
    let tree = |v| aliased(vec![var(61, 1, alias(62))], v);
    let out = scoped(tree(8.0), tree(12.0), "0:1");
    assert_eq!(
        (nodes(&out), &out["coverage"]["reasons"]),
        (vec![], &not_compared(1))
    );
}

#[test]
fn an_alias_on_another_page_is_compared_over_the_whole_file() {
    let tree = |v| aliased(vec![var(61, 1, alias(62))], v);
    let out = run(tree(8.0), tree(12.0));
    assert_eq!(
        (nodes(&out), &out["coverage"]["complete"]),
        (vec![("0:62".into(), "changed".into())], &json!(true))
    );
}

#[test]
fn an_alias_chain_leaving_the_page_makes_it_partial_and_one_within_it_does_not() {
    // 61 -> 63 -> 62, where only 62 is on the other page.
    let chain = |v| aliased(vec![var(61, 1, alias(63)), var(63, 1, alias(62))], v);
    let out = scoped(chain(8.0), chain(12.0), "0:1");
    assert_eq!(out["coverage"]["reasons"], not_compared(1));
    // 61 -> 63 holding a value, both on the page: nothing outside is followed.
    let within = |v| aliased(vec![var(61, 1, alias(63)), var(63, 1, float(v))], 1.0);
    let out = scoped(within(8.0), within(12.0), "0:1");
    assert_eq!(
        (nodes(&out), &out["coverage"]["complete"]),
        (vec![("0:63".into(), "changed".into())], &json!(true))
    );
}

#[test]
fn a_variable_whose_values_were_unread_leaves_its_aliases_unverified() {
    let long = obj(vec![
        ("dataType", K::Enum("VariableDataType", "STRING")),
        (
            "value",
            obj(vec![("textValue", K::String("x".repeat(FIELD_BYTES + 1)))]),
        ),
    ]);
    let tree = || aliased(vec![var(61, 1, long.clone())], 8.0);
    let out = scoped(tree(), tree(), "0:1");
    // Besides the field's own `unread_fields`.
    let reasons = out["coverage"]["reasons"].as_array().unwrap();
    assert_eq!(Json::from(reasons[..2].to_vec()), not_compared(1));
}

/// An alias to library variable `key`, version 1.
fn library_alias(key: &str) -> K<'static> {
    let asset = obj(vec![
        ("key", K::String(key.to_owned())),
        ("version", K::String("1".into())),
    ]);
    obj(vec![
        ("dataType", K::Enum("VariableDataType", "ALIAS")),
        (
            "value",
            obj(vec![("alias", obj(vec![("assetRef", asset)]))]),
        ),
    ])
}

/// Work and bytes `facets` spends on variable 61 holding `data` under `key`, and its aliases.
fn spent(key: &'static str, data: K<'static>) -> (usize, usize, Option<Vec<String>>) {
    let entry = obj(vec![("modeID", guid(70)), ("variableData", data)]);
    let values = obj(vec![("entries", K::Array(vec![entry]))]);
    let message = doc(vec![node(
        61,
        Some(1),
        "VARIABLE",
        "V",
        vec![(key, values)],
    )]);
    let tree = Tree::new(&message);
    let mut b = super::super::tests::bounds(WORK);
    let start = (b.work, b.bytes);
    let f = facets(&tree, "0:61", None, &mut b).unwrap();
    (start.0 - b.work, start.1 - b.bytes, f.aliases)
}

#[test]
fn alias_ids_are_paid_for_before_they_are_copied() {
    // The same value under another properties key is digested alike but never walked for ids.
    let (work, bytes, ids) = spent("variableDataValues", alias(62));
    let (other_work, other_bytes, none) = spent("variableScopes", alias(62));
    assert_eq!(ids, Some(vec!["VariableID:0:62".to_owned()]));
    assert_eq!(none, Some(vec![]));
    // A visit per entry, and its bytes beyond the longer key's.
    let longer = "variableDataValues".len() - "variableScopes".len();
    assert_eq!(work, other_work + 1);
    assert!(bytes > other_bytes + longer + "VariableID:0:62".len());
}

#[test]
fn a_long_library_alias_key_is_paid_for_by_its_length_and_a_short_side_stops_first() {
    let key = "k".repeat(FIELD_BYTES / 2);
    let (_, bytes, ids) = spent("variableDataValues", library_alias(&key));
    let (_, other_bytes, _) = spent("variableScopes", library_alias(&key));
    assert_eq!(ids, Some(vec![format!("VariableID:{key}/1")]));
    assert!(bytes >= other_bytes + key.len());
    // One byte or one visit short of the capture stops the side before any id is made.
    let entry = obj(vec![
        ("modeID", guid(70)),
        ("variableData", library_alias(&key)),
    ]);
    let values = obj(vec![("entries", K::Array(vec![entry]))]);
    let message = doc(vec![node(
        61,
        Some(1),
        "VARIABLE",
        "V",
        vec![("variableDataValues", values)],
    )]);
    let tree = Tree::new(&message);
    let (work, _, _) = spent("variableDataValues", library_alias(&key));
    for (w, by, stop) in [
        (work - 1, usize::MAX, crate::expand::Stop::Work),
        (usize::MAX, bytes - 1, crate::expand::Stop::Bytes),
    ] {
        let mut b = super::super::tests::bounds(WORK);
        (b.work, b.bytes) = (w.min(b.work), by.min(b.bytes));
        assert!(facets(&tree, "0:61", None, &mut b).is_none());
        assert_eq!(b.stop, Some(stop));
    }
}

/// Page 1 holds a card filled from two-mode variable 61; page 2 holds its collection 64, whose
/// first mode by sort position is `first` (70 or 71).
fn collected(first: u32) -> K<'static> {
    let mode = |id: u32, at: &str| {
        obj(vec![
            ("id", guid(id)),
            ("sortPosition", K::String(at.to_owned())),
        ])
    };
    let modes = match first {
        70 => vec![mode(70, "a"), mode(71, "b")],
        _ => vec![mode(70, "b"), mode(71, "a")],
    };
    let entry = |m: u32, v: f32| obj(vec![("modeID", guid(m)), ("variableData", float(v))]);
    let mut all = aliased(
        vec![node(
            61,
            Some(1),
            "VARIABLE",
            "V",
            vec![
                (
                    "variableDataValues",
                    obj(vec![(
                        "entries",
                        K::Array(vec![entry(70, 1.0), entry(71, 2.0)]),
                    )]),
                ),
                ("variableSetID", obj(vec![("guid", guid(64))])),
            ],
        )],
        8.0,
    );
    if let K::Object(_, o) = &mut all
        && let Some(K::Array(nodes)) = o.get_mut("nodeChanges")
    {
        nodes.push(node(
            64,
            Some(2),
            "VARIABLE_SET",
            "Theme",
            vec![("variableSetModes", K::Array(modes))],
        ));
    }
    all
}

#[test]
fn a_collection_on_another_page_makes_a_page_diff_partial_and_the_whole_file_compares_it() {
    let out = scoped(collected(70), collected(71), "0:1");
    assert_eq!(
        (nodes(&out), &out["coverage"]["reasons"]),
        (vec![], &not_compared(1))
    );
    let out = run(collected(70), collected(71));
    assert_eq!(
        (nodes(&out), &out["coverage"]["complete"]),
        (vec![("0:64".into(), "changed".into())], &json!(true))
    );
}

#[test]
fn stored_generations_keep_a_variable_s_collection_like_the_live_trees() {
    outside_parity(collected(70), collected(71), "0:1");
}

/// `screen` on a page that sets an explicit mode of collection 64 for everything below it.
fn moded_page() -> K<'static> {
    let mut message = doc(screen("Hi", 0.5, 400.0));
    let entry = obj(vec![
        ("variableSetID", obj(vec![("guid", guid(64))])),
        ("variableModeID", guid(70)),
    ]);
    if let K::Object(_, o) = &mut message
        && let Some(K::Array(nodes)) = o.get_mut("nodeChanges")
        && let K::Object(_, page) = &mut nodes[1]
    {
        page.insert(
            "variableModeBySetMap",
            obj(vec![("entries", K::Array(vec![entry]))]),
        );
    }
    message
}

#[test]
fn an_ancestor_walk_it_cannot_finish_counts_as_one_mode_not_compared() {
    // Frames 100.. chained under page 1; the last of `frames` has `frames + 1` ancestors.
    let chain = |frames: u32| {
        let mut nodes = vec![node(100, Some(1), "FRAME", "F", vec![])];
        nodes.extend((101..100 + frames).map(|id| node(id, Some(id - 1), "FRAME", "F", vec![])));
        doc(nodes)
    };
    let count = |message: &K<'static>, last: u32, work: usize| {
        let mut b = super::super::tests::bounds(work);
        inherited_modes(&Tree::new(message), &format!("0:{last}"), &mut b)
    };
    let depth = DEPTH as u32;
    assert_eq!(
        (
            count(&chain(depth - 1), 98 + depth, WORK),
            count(&chain(depth), 99 + depth, WORK),
            count(&chain(1), 100, 0),
        ),
        (0, 1, 1)
    );
}

#[test]
fn modes_a_frame_inherits_from_above_it_make_its_diff_partial() {
    let plain = scoped(
        doc(screen("Hi", 0.5, 400.0)),
        doc(screen("Hi", 0.5, 400.0)),
        "0:10",
    );
    assert_eq!(plain["coverage"]["complete"], json!(true));
    let out = scoped(moded_page(), moded_page(), "0:10");
    assert_eq!(out["coverage"]["reasons"], not_compared(1));
    // Over the whole page the mode setting is a compared record.
    let out = scoped(moded_page(), moded_page(), "0:1");
    assert_eq!(out["coverage"]["complete"], json!(true));
}

/// Page 1 holds color variable 61 (red in mode 70, blue in 71), its collection 64, a component
/// 20 whose card is filled from it and an instance 30 of it, whose copy REST resolves again in
/// the modes in effect; the document above the page selects `mode` of collection 64.
fn document_moded(mode: u32) -> K<'static> {
    let color = |r: f32, b: f32| {
        obj(vec![
            ("dataType", K::Enum("VariableDataType", "COLOR")),
            (
                "value",
                obj(vec![(
                    "colorValue",
                    obj(vec![
                        ("r", K::Float(r)),
                        ("g", K::Float(0.0)),
                        ("b", K::Float(b)),
                        ("a", K::Float(1.0)),
                    ]),
                )]),
            ),
        ])
    };
    let entry = |m: u32, data| obj(vec![("modeID", guid(m)), ("variableData", data)]);
    let mode_of = |id: u32, at: &str| {
        obj(vec![
            ("id", guid(id)),
            ("sortPosition", K::String(at.to_owned())),
        ])
    };
    let paint = obj(vec![
        ("type", K::Enum("PaintType", "SOLID")),
        ("colorVar", alias(61)),
    ]);
    let mut message = doc(vec![
        node(
            64,
            Some(1),
            "VARIABLE_SET",
            "Theme",
            vec![(
                "variableSetModes",
                K::Array(vec![mode_of(70, "a"), mode_of(71, "b")]),
            )],
        ),
        node(
            61,
            Some(1),
            "VARIABLE",
            "Brand",
            vec![
                (
                    "variableDataValues",
                    obj(vec![(
                        "entries",
                        K::Array(vec![entry(70, color(1.0, 0.0)), entry(71, color(0.0, 1.0))]),
                    )]),
                ),
                ("variableSetID", obj(vec![("guid", guid(64))])),
            ],
        ),
        node(20, Some(1), "SYMBOL", "Chip", vec![]),
        node(
            21,
            Some(20),
            "RECTANGLE",
            "Card",
            vec![("fillPaints", K::Array(vec![paint]))],
        ),
        instance(30, 1, "Use", 20, vec![]),
    ]);
    let selected = obj(vec![
        ("variableSetID", obj(vec![("guid", guid(64))])),
        ("variableModeID", guid(mode)),
    ]);
    if let K::Object(_, o) = &mut message
        && let Some(K::Array(nodes)) = o.get_mut("nodeChanges")
        && let K::Object(_, document) = &mut nodes[0]
    {
        document.insert(
            "variableModeBySetMap",
            obj(vec![("entries", K::Array(vec![selected]))]),
        );
    }
    message
}

#[test]
fn a_document_mode_switch_changes_a_page_s_rest_fills_and_leaves_its_diff_partial() {
    let (red, blue) = (document_moded(70), document_moded(71));
    let rest = |m: &K<'static>| crate::rest::resolve_nodes(&Tree::new(m), "0:1").unwrap();
    assert_ne!(rest(&red), rest(&blue));
    let out = scoped(red, blue, "0:1");
    assert_eq!(
        (nodes(&out), &out["coverage"]["reasons"]),
        (vec![], &not_compared(1))
    );
}

#[test]
fn stored_generations_keep_inherited_modes_like_the_live_trees() {
    outside_parity(moded_page(), moded_page(), "0:10");
}

#[test]
fn numeric_variants_trim_links_mask_type_and_grids_are_compared() {
    let link = |url: &str| obj(vec![("url", K::String(url.to_owned()))]);
    for (key, old, new, facet) in [
        (
            "fontVariantNumericFigure",
            K::Enum("FontVariantNumericFigure", "LINING"),
            K::Enum("FontVariantNumericFigure", "OLDSTYLE"),
            "text",
        ),
        (
            "fontVariantNumericSpacing",
            K::Enum("FontVariantNumericSpacing", "TABULAR"),
            K::Enum("FontVariantNumericSpacing", "PROPORTIONAL"),
            "text",
        ),
        (
            "leadingTrim",
            K::Enum("LeadingTrim", "NONE"),
            K::Enum("LeadingTrim", "CAP_HEIGHT"),
            "text",
        ),
        (
            "hyperlink",
            link("https://a.test"),
            link("https://b.test"),
            "text",
        ),
        (
            "maskType",
            K::Enum("MaskType", "ALPHA"),
            K::Enum("MaskType", "OUTLINE"),
            "appearance",
        ),
        (
            "layoutGrids",
            K::Array(vec![obj(vec![("sectionSize", K::Float(8.0))])]),
            K::Array(vec![obj(vec![("sectionSize", K::Float(12.0))])]),
            "layout",
        ),
    ] {
        let one = |v: K<'static>| doc(vec![node(10, Some(1), "TEXT", "Layer", vec![(key, v)])]);
        let n = &run(one(old), one(new))["frames"][0]["nodes"][0];
        assert_eq!(n["facets"], json!([facet]), "{key}");
    }
}

#[test]
fn renaming_a_component_property_definition_is_a_properties_change() {
    let def = |name: &str| {
        (
            "componentPropDefs",
            K::Array(vec![obj(vec![
                ("id", guid(70)),
                ("name", K::String(name.to_owned())),
                ("type", K::Enum("ComponentPropType", "BOOL")),
                ("initialValue", obj(vec![("boolValue", K::Bool(true))])),
            ])]),
        )
    };
    let one = |name: &str| {
        doc(vec![node(
            20,
            Some(1),
            "COMPONENT",
            "Button",
            vec![def(name)],
        )])
    };
    let out = run(one("Show icon"), one("Has icon"));
    assert_eq!(
        (nodes(&out), &out["frames"][0]["nodes"][0]["facets"]),
        (
            vec![("0:20".into(), "changed".into())],
            &json!(["properties"])
        )
    );
}

/// A main component outside frame `0:10` with one Boolean definition, and an instance of it in
/// the frame.
fn used(name: &str, default: bool) -> K<'static> {
    doc(vec![
        node(
            20,
            Some(1),
            "COMPONENT",
            "Button",
            vec![(
                "componentPropDefs",
                K::Array(vec![obj(vec![
                    ("id", guid(70)),
                    ("name", K::String(name.to_owned())),
                    ("type", K::Enum("ComponentPropType", "BOOL")),
                    ("initialValue", obj(vec![("boolValue", K::Bool(default))])),
                ])]),
            )],
        ),
        node(10, Some(1), "FRAME", "Home", vec![]),
        instance(11, 10, "Button", 20, vec![]),
    ])
}

#[test]
fn a_variant_whose_set_is_outside_the_scope_makes_coverage_partial() {
    // The set's definition is renamed and its default flipped; the variant inherits it through
    // parentPropDefId, so its emitted definitions change while its own record does not.
    let set = |name: &str, default: bool| {
        doc(vec![
            node(
                30,
                Some(1),
                "FRAME",
                "Button",
                vec![
                    ("isStateGroup", K::Bool(true)),
                    (
                        "componentPropDefs",
                        K::Array(vec![obj(vec![
                            ("id", guid(70)),
                            ("name", K::String(name.to_owned())),
                            ("type", K::Enum("ComponentPropType", "BOOL")),
                            ("initialValue", obj(vec![("boolValue", K::Bool(default))])),
                        ])]),
                    ),
                ],
            ),
            node(
                20,
                Some(30),
                "COMPONENT",
                "State=Default",
                vec![(
                    "componentPropDefs",
                    K::Array(vec![obj(vec![
                        ("id", guid(71)),
                        ("parentPropDefId", guid(70)),
                    ])]),
                )],
            ),
        ])
    };
    let (old, new) = (|| set("Show icon", true), || set("Has icon", false));
    let out = scoped(old(), new(), "0:20");
    assert_eq!(
        (
            &out["coverage"]["complete"],
            nodes(&out),
            &out["coverage"]["reasons"]
        ),
        (
            &json!(false),
            vec![],
            &json!([
                {"reason": "definition_not_compared", "side": "old", "count": 1},
                {"reason": "definition_not_compared", "side": "new", "count": 1},
            ])
        )
    );
    // With the set in scope its own definitions are compared.
    let whole = run(old(), new());
    assert_eq!(
        (nodes(&whole), &whole["coverage"]["complete"]),
        (vec![("0:30".into(), "changed".into())], &json!(true))
    );
}

#[test]
fn a_main_outside_the_scope_makes_coverage_partial_when_its_property_definitions_change() {
    // Only the instance is in scope; its main's Boolean definition is renamed and its default
    // flipped, which changes the instance's emitted componentProperties.
    let out = scoped(used("Show icon", true), used("Has icon", false), "0:10");
    assert_eq!(
        (
            &out["coverage"]["complete"],
            nodes(&out),
            &out["coverage"]["reasons"]
        ),
        (
            &json!(false),
            vec![],
            &json!([
                {"reason": "definition_not_compared", "side": "old", "count": 1},
                {"reason": "definition_not_compared", "side": "new", "count": 1},
            ])
        )
    );
}

/// A workspace schema in memory, with `trees` saved as generations 1, 2, … of scope `0:0`.
fn saved(trees: &[&Tree], scope: &str) -> rusqlite::Connection {
    saved_with(trees, scope, crate::search::Library::Skip)
}

/// `saved`, building with `library` as a stored generation's build does.
fn saved_with(
    trees: &[&Tree],
    scope: &str,
    library: crate::search::Library,
) -> rusqlite::Connection {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
         CREATE TABLE sources (source_id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE,
         added_at_ms INTEGER NOT NULL) STRICT; {} {} {} {}
         -- Schema 8's table a stored build writes its fingerprints to.
         CREATE TABLE fingerprints (
             gen INTEGER NOT NULL, rid INTEGER NOT NULL, facet TEXT NOT NULL, state TEXT NOT NULL,
             digest BLOB, PRIMARY KEY (gen, rid, facet)) STRICT, WITHOUT ROWID;",
        crate::workspace::V2,
        crate::workspace::V3,
        crate::workspace::V4,
        crate::workspace::V11
    ))
    .unwrap();
    for (i, tree) in trees.iter().enumerate() {
        let gen_id = i as i64 + 1;
        conn.execute(
            "INSERT INTO generations (gen, source_id, membership, scope, versions, job_id, state,
             created_ms) VALUES (?1, 's', 'm', ?2, 'v', 'j', 'building', 0)",
            rusqlite::params![gen_id, scope],
        )
        .unwrap();
        let (index, tracked) = tracked(tree, scope, Instant::now() + DEADLINE, library);
        crate::search::store::save(
            &mut conn,
            gen_id,
            &index,
            &|_| Ok(()),
            &mut |_, _, _| Ok(()),
        )
        .unwrap();
        save_facets(&mut conn, gen_id, &tracked, &|_| Ok(()), &mut |_, _, _| {
            Ok(())
        })
        .unwrap();
    }
    conn
}

#[test]
fn stored_generations_diff_exactly_like_the_live_trees() {
    let mut moved = screen("Hi", 0.0, 420.0);
    moved.push(node(14, Some(1), "FRAME", "Other", vec![]));
    moved[3] = node(13, Some(14), "RECTANGLE", "Same", vec![fill(0.5)]);
    let (old, new) = (doc(screen("Hello", 1.0, 400.0)), doc(moved));
    let (a, b) = (Tree::new(&old), Tree::new(&new));
    let live = page(
        &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0"),
        &req(LIMIT, None),
        false,
    )
    .unwrap();
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    let conn = saved(&[&a, &b], "0:0");
    let stored = stored_diff(&conn, (1, side("old")), (2, side("new")), &req(LIMIT, None));
    let unstamped = |mut v: Json| {
        v["source"]["freshness"]["hashedAtMs"].take();
        v
    };
    assert_eq!(unstamped(stored.unwrap()), unstamped(live));
}

#[test]
fn stored_generations_report_an_outside_definition_like_the_live_trees() {
    outside_parity(styled(12.0), styled(16.0), "0:10");
    outside_parity(used("Show icon", true), used("Has icon", false), "0:10");
}

/// Diffs `old` and `new` from stored generations of `node` and asserts the same partial
/// coverage reasons as the live trees.
fn outside_parity(old: K<'static>, new: K<'static>, node: &str) {
    let (a, b) = (Tree::new(&old), Tree::new(&new));
    let mut r = req(LIMIT, None);
    r.node = node.into();
    let live = page(
        &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], node),
        &r,
        false,
    )
    .unwrap();
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    let conn = saved(&[&a, &b], node);
    let stored = stored_diff(&conn, (1, side("old")), (2, side("new")), &r).unwrap();
    let coverage = |v: &Json| {
        (
            v["coverage"]["complete"].clone(),
            v["coverage"]["reasons"].clone(),
        )
    };
    assert_eq!(
        coverage(&stored),
        (json!(false), coverage(&live).1),
        "{stored}"
    );
}

#[test]
fn tampered_or_other_layout_facets_are_refused_not_compared() {
    let old = doc(screen("Hello", 1.0, 400.0));
    let tree = Tree::new(&old);
    let source = json!({"sourceId": "local:a", "snapshotId": "sha256:a"});
    let run = |conn: &rusqlite::Connection| {
        let err = stored_diff(
            conn,
            (1, source.clone()),
            (1, source.clone()),
            &req(LIMIT, None),
        )
        .unwrap_err();
        err.downcast_ref::<FigError>().map(FigError::kind)
    };
    let conn = saved(&[&tree], "0:0");
    conn.execute(
        "UPDATE facets SET blob = CAST('{\"fields\":[[\"bogus\",null]],\"aliases\":[]}' AS BLOB) WHERE rid = 1",
        [],
    )
    .unwrap();
    assert_eq!(run(&conn), Some(ErrorKind::IndexIncompatible));
    let conn = saved(&[&tree], "0:0");
    conn.execute(
        "INSERT INTO facets SELECT gen, MAX(rid) + 1, CAST('{\"fields\":[],\"aliases\":[]}' AS BLOB) FROM facets",
        [],
    )
    .unwrap();
    assert_eq!(run(&conn), Some(ErrorKind::IndexIncompatible));
    let conn = saved(&[&tree], "0:0");
    conn.execute("UPDATE facet_sets SET facet_version = 'diff-v0:0'", [])
        .unwrap();
    assert_eq!(run(&conn), Some(ErrorKind::IndexIncompatible));
    // Facets stored by diff-v9, before collections and inherited modes were kept, are never
    // read as knowing none: same fields, older version.
    let conn = saved(&[&tree], "0:0");
    let v9 = facet_version().replacen(VERSION, "diff-v9", 1);
    conn.execute("UPDATE facet_sets SET facet_version = ?1", [v9])
        .unwrap();
    assert_eq!(run(&conn), Some(ErrorKind::IndexIncompatible));
    // An inherited-mode count past the walk's bound is refused, not added.
    let conn = saved(&[&tree], "0:0");
    conn.execute(
        "UPDATE facet_sets SET origin = '[null, 18446744073709551615]'",
        [],
    )
    .unwrap();
    assert_eq!(run(&conn), Some(ErrorKind::IndexIncompatible));
}

#[test]
fn stored_facets_keep_the_bytes_of_their_json_tree() {
    let field = |key, digest, value| Field {
        key,
        facet: 0,
        digest,
        local: false,
        value,
    };
    let fields = vec![
        field(
            "characters",
            Some([7; 32]),
            Some(json!({"b": "é\"", "a": [1, null]})),
        ),
        field("fontSize", Some([0xab; 32]), None),
        field("fontName", None, None),
    ];
    for aliases in [None, Some(vec![]), Some(vec!["VariableID:0:62".to_owned()])] {
        let f = Facets {
            fields: fields.clone(),
            aliases,
        };
        let rows: Vec<Json> = f
            .fields
            .iter()
            .map(|f| match &f.value {
                Some(v) => json!([f.key, f.digest.map(|d| hex(&d)), v]),
                None => json!([f.key, f.digest.map(|d| hex(&d))]),
            })
            .collect();
        let tree = json!({"fields": rows, "aliases": f.aliases}).to_string();
        assert_eq!(String::from_utf8(encode(&f)).unwrap(), tree);
    }
}

#[test]
fn stored_facets_pay_for_their_text_before_parsing_it() {
    // Unparseable text past the budget is a limit, not a refused layout: no parse ran.
    let mut spent = BYTES - 3;
    let err = decode("{{{{", &mut spent).err().unwrap();
    assert_eq!(
        err.downcast_ref::<FigError>().map(FigError::kind),
        Some(ErrorKind::QueryWorkLimit)
    );
}

#[test]
fn stored_facets_keep_a_readable_value_and_charge_text_metadata_and_value() {
    let text = format!(
        r#"{{"fields":[["fontSize","{}",12]],"aliases":[]}}"#,
        "a".repeat(64)
    );
    let mut spent = 0;
    let facets = decode(&text, &mut spent).unwrap();
    assert_eq!(
        (facets.fields[0].value.clone(), spent),
        (
            Some(serde_json::json!(12)),
            text.len() + FIELD_META + "fontSize".len() + held_value(&serde_json::json!(12), 2)
        )
    );
}

#[test]
fn stored_facets_charge_a_kept_value_after_its_metadata() {
    let text = format!(
        r#"{{"fields":[["fontSize","{}",12]],"aliases":[]}}"#,
        "a".repeat(64)
    );
    // Text and metadata fit; the value does not.
    let held = held_value(&serde_json::json!(12), 2);
    let mut spent = BYTES - text.len() - FIELD_META - "fontSize".len() - held + 1;
    let err = decode(&text, &mut spent).err().unwrap();
    assert_eq!(
        err.downcast_ref::<FigError>().map(FigError::kind),
        Some(ErrorKind::QueryWorkLimit)
    );
}

#[test]
fn stored_facets_keep_unread_aliases_apart_from_none_and_pay_for_each_id() {
    let round = |text: &str| decode(text, &mut 0).unwrap().aliases;
    let id = "VariableID:0:62";
    // One id per aliasing mode, so a repeat is kept.
    let text = format!(r#"{{"fields":[],"aliases":["{id}","{id}"]}}"#);
    let mut spent = 0;
    decode(&text, &mut spent).unwrap();
    assert_eq!(
        (
            round(r#"{"fields":[],"aliases":null}"#),
            round(r#"{"fields":[],"aliases":[]}"#),
            round(&text),
            spent
        ),
        (
            None,
            Some(vec![]),
            Some(vec![id.to_owned(), id.to_owned()]),
            text.len() + 2 * (std::mem::size_of::<String>() + 2 * id.len())
        )
    );
}

#[test]
fn stored_facets_pay_for_each_alias_before_keeping_it() {
    let id = "VariableID:0:62";
    let text = format!(r#"{{"fields":[],"aliases":["{id}"]}}"#);
    let mut spent = BYTES - text.len() - std::mem::size_of::<String>() - 2 * id.len() + 1;
    let err = decode(&text, &mut spent).err().unwrap();
    assert_eq!(
        err.downcast_ref::<FigError>().map(FigError::kind),
        Some(ErrorKind::QueryWorkLimit)
    );
}

#[test]
fn stored_facets_without_their_aliases_are_refused_not_read_as_none() {
    for text in [
        r#"[["fontSize",null]]"#,
        r#"{"fields":[["fontSize",null]]}"#,
        r#"{"fields":[],"aliases":[1]}"#,
    ] {
        let err = decode(text, &mut 0).err().unwrap();
        assert_eq!(
            err.downcast_ref::<FigError>().map(FigError::kind),
            Some(ErrorKind::IndexIncompatible),
            "{text}"
        );
    }
}

#[test]
fn stored_facets_whose_rows_are_not_lists_are_refused() {
    for text in [
        r#"{"fields":{},"aliases":[]}"#,
        r#"{"fields":["fontSize"],"aliases":[]}"#,
    ] {
        let err = decode(text, &mut 0).err().unwrap();
        assert_eq!(
            err.downcast_ref::<FigError>().map(FigError::kind),
            Some(ErrorKind::IndexIncompatible),
            "{text}"
        );
    }
}

#[test]
fn stored_generations_report_an_alias_outside_the_page_like_the_live_trees() {
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    let coverage = |old: K<'static>, new: K<'static>| {
        let (a, b) = (Tree::new(&old), Tree::new(&new));
        let mut r = req(LIMIT, None);
        r.node = "0:1".into();
        let live = page(
            &compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:1"),
            &r,
            false,
        )
        .unwrap();
        let conn = saved(&[&a, &b], "0:1");
        let stored = stored_diff(&conn, (1, side("old")), (2, side("new")), &r).unwrap();
        let reasons = |v: &Json| v["coverage"].clone();
        (reasons(&stored), reasons(&live))
    };
    // 61 -> 62, with 62 on the other page: partial once reopened.
    let outside = |v| aliased(vec![var(61, 1, alias(62))], v);
    let (stored, live) = coverage(outside(8.0), outside(12.0));
    assert_eq!((&stored["reasons"], &stored), (&not_compared(1), &live));
    // 61 -> 63, both on the page: the stored empty closure stays complete.
    let within = |v| aliased(vec![var(61, 1, alias(63)), var(63, 1, float(v))], 1.0);
    let (stored, live) = coverage(within(8.0), within(12.0));
    assert_eq!((&stored["complete"], &stored), (&json!(true), &live));
    // A library key read within the field cap that escaping makes longer than it.
    let key = "\u{1}".repeat(FIELD_BYTES / 3);
    let escaped = || aliased(vec![var(61, 1, library_alias(&key))], 8.0);
    outside_parity(escaped(), escaped(), "0:1");
    // Values too long to read leave the aliases unverified once reopened too.
    let long = obj(vec![
        ("dataType", K::Enum("VariableDataType", "STRING")),
        (
            "value",
            obj(vec![("textValue", K::String("x".repeat(FIELD_BYTES + 1)))]),
        ),
    ]);
    let unread = || aliased(vec![var(61, 1, long.clone())], 8.0);
    outside_parity(unread(), unread(), "0:1");
}

#[test]
fn stored_facets_charge_their_text_then_each_field_as_the_live_build_does() {
    // Once the text is paid for, nothing is left for the field's metadata plus key.
    let text = r#"{"fields":[["fontSize",null]],"aliases":[]}"#;
    let mut spent = BYTES - text.len();
    let err = decode(text, &mut spent).err().unwrap();
    assert_eq!(
        err.downcast_ref::<FigError>().map(FigError::kind),
        Some(ErrorKind::QueryWorkLimit)
    );
}

/// A main whose fill changes, a frame whose fill changes, an instance that overrides the fill
/// with the same color on both sides and, when `plain`, one that records no fill of its own.
fn impacted(red: f64, plain: bool) -> K<'static> {
    let mut nodes = vec![
        node(30, Some(1), "SYMBOL", "Main", vec![fill(red)]),
        node(10, Some(1), "FRAME", "Home", vec![fill(red)]),
        instance(12, 10, "Fixed", 30, vec![fill(0.2)]),
        node(13, Some(10), "TEXT", "Label", vec![]),
    ];
    if plain {
        nodes.push(instance(11, 10, "Plain", 30, vec![]));
    }
    doc(nodes)
}

fn probe(pair: &Pair, recipient: &str, facet: u8, work: usize) -> (Verdict, bool) {
    let mut evidence = PairEvidence {
        pair,
        ids: None,
        orders: None,
        aliased: [None, None],
    };
    let mut bounds = Bounds {
        work,
        bytes: BYTES,
        depth: DEPTH,
        deadline: Instant::now() + DEADLINE,
        stop: None,
    };
    let p = Probe {
        recipient,
        through: "0:30",
        seed: "0:30",
        facet,
    };
    let verdict = super::super::impact::Evidence::compare(&mut evidence, &p, &mut bounds);
    (verdict.unwrap(), bounds.stop.is_some())
}

#[test]
fn a_probe_proves_one_facet_of_one_record_both_generations_hold() {
    let (a, b) = (impacted(1.0, false), impacted(0.5, false));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let appearance = 1 << 2;
    let at = |id, facet| probe(&pair, id, facet, WORK).0;
    assert_eq!(at("0:10", appearance), Verdict::Changed);
    assert_eq!(at("0:12", appearance), Verdict::Equal);
    // Structure is proven equal on two complete sides.
    assert_eq!(at("0:12", 1 << 4), Verdict::Equal);
    // A facet with no field on either side is never a vacuous equal.
    assert_eq!(at("0:13", 1 << 0), Verdict::Unknown);
    // Edges, unread, several facets at once, or a record one side lacks prove nothing.
    for facet in [EDGES, UNREAD, appearance | 1 << 4, 0] {
        assert_eq!(at("0:12", facet), Verdict::Unknown, "{facet}");
    }
    assert_eq!(at("0:99", appearance), Verdict::Unknown);
}

#[test]
fn a_probe_of_structure_on_a_stopped_side_is_unknown() {
    let (a, b) = (impacted(1.0, false), impacted(0.5, false));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    // The old side stops partway, so sibling positions there are not known.
    let shared = (1..100).find_map(|work| {
        let pair = compare_trees(&a, &b, [work, WORK, PAIR_WORK], "0:0");
        let id = pair
            .old
            .index
            .records
            .iter()
            .map(|r| r.id.clone())
            .find(|id| pair.new.index.records.iter().any(|r| r.id == *id));
        id.filter(|_| pair.old.index.stop.is_some())
            .map(|id| (pair, id))
    });
    let (pair, id) = shared.unwrap();
    assert_eq!(probe(&pair, &id, 1 << 4, WORK).0, Verdict::Unknown);
}

#[test]
fn a_probe_of_a_retyped_or_unread_record_is_unknown() {
    let side = |kind| doc(vec![node(10, Some(1), kind, "Home", vec![fill(1.0)])]);
    let (a, b) = (side("FRAME"), side("RECTANGLE"));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    assert_eq!(probe(&pair, "0:10", 1 << 2, WORK).0, Verdict::Unknown);
    // A field read on one side only is no proof of equality.
    let field = |digest| Field {
        local: false,
        key: "fillPaints",
        facet: 2,
        digest,
        value: None,
    };
    let (read, unread) = ([field(Some([1; 32]))], [field(None)]);
    assert_eq!(proof(&read, &unread, 2), None);
    assert_eq!(proof(&read, &read, 2), Some(false));
    assert_eq!(proof(&read, &[field(Some([2; 32]))], 2), Some(true));
    assert_eq!(proof(&read, &[], 2), Some(true));
}

#[test]
fn a_probe_pays_for_its_lookups_before_building_them() {
    let (a, b) = (impacted(1.0, false), impacted(0.5, false));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let records = pair.old.index.records.len() + pair.new.index.records.len();
    assert_eq!(probe(&pair, "0:10", 1 << 2, 0), (Verdict::Unknown, true));
    assert_eq!(
        probe(&pair, "0:10", 1 << 2, records),
        (Verdict::Unknown, true)
    );
    assert_eq!(
        probe(&pair, "0:10", 1 << 2, records + 2),
        (Verdict::Changed, false)
    );
}

#[test]
fn a_reached_layer_names_its_top_level_frame_on_each_side_even_when_moved() {
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    // As `impacted(0.5, false)`, with the instance moved to another top-level frame.
    let moved = doc(vec![
        node(30, Some(1), "SYMBOL", "Main", vec![fill(0.5)]),
        node(10, Some(1), "FRAME", "Home", vec![fill(0.5)]),
        node(20, Some(1), "FRAME", "Away", vec![]),
        instance(12, 20, "Fixed", 30, vec![fill(0.2)]),
        node(13, Some(10), "TEXT", "Label", vec![]),
    ]);
    let at = |new: &K| {
        let conn = saved(&[&Tree::new(&impacted(1.0, false)), &Tree::new(new)], "0:0");
        let r = revision(
            &conn,
            (1, side("old")),
            (2, side("new")),
            "0:0",
            REVISION_BYTES,
        )
        .unwrap();
        let frames = r
            .impact
            .hits
            .iter()
            .find(|h| h.node == "0:12")
            .map(|h| h.frames.clone());
        (frames, r.impact.reasons.contains_key("frames_unknown"))
    };
    let frame = |id: &str| Some(id.to_owned());
    assert_eq!(
        [at(&impacted(0.5, false)), at(&moved)],
        [
            (Some([frame("0:10"), frame("0:10")]), false),
            (Some([frame("0:10"), frame("0:20")]), false)
        ]
    );
}

#[test]
fn a_reached_layer_no_side_holds_is_never_placed_complete() {
    let (a, b) = (impacted(1.0, false), impacted(0.5, false));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let mut evidence = PairEvidence {
        pair: &pair,
        ids: None,
        orders: None,
        aliased: [None, None],
    };
    let mut impact = super::super::impact::Impact {
        hits: vec![super::super::impact::Hit {
            node: "0:99".into(),
            class: super::super::impact::Class::Possible,
            side: 3,
            changed: 0,
            unknown: 0,
            equal: 0,
            via: None,
            frames: [None, None],
        }],
        complete: true,
        reasons: Default::default(),
    };
    let mut bounds = Bounds {
        work: WORK,
        bytes: BYTES,
        depth: DEPTH,
        deadline: Instant::now() + DEADLINE,
        stop: None,
    };
    place(&mut evidence, &mut impact, &mut bounds);
    assert_eq!(
        (
            impact.hits[0].frames.clone(),
            impact.reasons.get("frames_unknown")
        ),
        ([None, None], Some(&1))
    );
}

#[test]
fn a_revision_classifies_what_its_kept_changes_reach() {
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    let saved_pair = |plain| {
        let (a, b) = (impacted(1.0, plain), impacted(0.5, plain));
        saved(&[&Tree::new(&a), &Tree::new(&b)], "0:0")
    };
    let (conn, with_plain) = (saved_pair(false), saved_pair(true));
    let at = |bytes| revision(&conn, (1, side("old")), (2, side("new")), "0:0", bytes).unwrap();
    let whole = at(REVISION_BYTES);
    let class =
        |r: &Revision, id: &str| r.impact.hits.iter().find(|h| h.node == id).map(|h| h.class);
    use super::super::impact::Class;
    assert_eq!(whole.coverage["complete"], true);
    assert_eq!(class(&whole, "0:30"), Some(Class::Direct));
    assert_eq!(class(&whole, "0:12"), Some(Class::EffectiveUnchanged));
    assert!(whole.impact.complete, "{:?}", whole.impact.reasons);
    // An instance recording no fill of its own is reached, but its fill is not proven either way.
    let plain = revision(
        &with_plain,
        (1, side("old")),
        (2, side("new")),
        "0:0",
        REVISION_BYTES,
    )
    .unwrap();
    assert_eq!(class(&plain, "0:11"), Some(Class::Possible));
    assert_eq!(
        (&plain.impact.reasons, plain.impact.complete),
        (
            &std::collections::BTreeMap::from([("comparison_unknown", 1)]),
            false
        )
    );
    // Rows past the bytes are no seeds: never complete, even with nothing reached.
    let none = at(0);
    assert_eq!((none.impact.hits.len(), none.impact.complete), (0, false));
    assert_eq!(
        none.impact.reasons.get("seeds_truncated"),
        Some(&(whole.rows.len() as u64))
    );
}

#[test]
fn a_kept_row_whose_seed_does_not_fit_stays_a_row_but_leaves_the_impact_incomplete() {
    let (a, b) = (impacted(1.0, false), impacted(0.5, false));
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    let conn = saved(&[&Tree::new(&a), &Tree::new(&b)], "0:0");
    // Room for exactly one seed: the second kept row is not copied into one.
    let bounds = Bounds {
        work: WORK,
        bytes: std::mem::size_of::<Seed>() + 4,
        depth: DEPTH,
        deadline: Instant::now() + DEADLINE,
        stop: None,
    };
    let r = revision_with(
        &conn,
        (1, side("old")),
        (2, side("new")),
        "0:0",
        REVISION_BYTES,
        bounds,
    )
    .unwrap();
    assert_eq!(
        (
            r.rows.len(),
            r.impact.complete,
            r.impact.reasons.get("seeds_truncated")
        ),
        (2, false, Some(&1))
    );
}

#[test]
fn a_removed_subtree_seeds_each_layer_below_its_top_or_leaves_the_impact_incomplete() {
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    let other = || node(20, Some(1), "FRAME", "Other", vec![]);
    let old = doc(vec![
        node(10, Some(1), "FRAME", "Home", vec![]),
        node(13, Some(10), "TEXT", "Label", vec![]),
        node(14, Some(10), "RECTANGLE", "Dot", vec![]),
        other(),
    ]);
    let new = doc(vec![other()]);
    let conn = saved(&[&Tree::new(&old), &Tree::new(&new)], "0:0");
    let at = |work, bytes| {
        let bounds = Bounds {
            work,
            bytes,
            depth: DEPTH,
            deadline: Instant::now() + DEADLINE,
            stop: None,
        };
        revision_with(
            &conn,
            (1, side("old")),
            (2, side("new")),
            "0:0",
            REVISION_BYTES,
            bounds,
        )
        .unwrap()
    };
    use super::super::impact::Class;
    let whole = at(WORK, PAIR_BYTES);
    let class = |id: &str| {
        whole
            .impact
            .hits
            .iter()
            .find(|h| h.node == id)
            .map(|h| h.class)
    };
    assert_eq!(
        (class("0:13"), class("0:14"), whole.impact.complete),
        (Some(Class::Direct), Some(Class::Direct), true)
    );
    // Room for the top's seed only: the layers below it are cut, never a complete empty impact.
    let cut = at(WORK, std::mem::size_of::<Seed>() + 4);
    assert_eq!(
        (
            cut.impact.complete,
            cut.impact.reasons.contains_key("seeds_truncated")
        ),
        (false, true)
    );
    // Work for the row seeds, the id lookups and both layers' seeds, but not the pass that finds
    // those layers: it is paid first, so they stay unseeded.
    let pair = compare(
        stored(&conn, 1, side("old")).unwrap(),
        stored(&conn, 2, side("new")).unwrap(),
        "0:0",
        PAIR_WORK,
    );
    let records = pair.old.index.records.len() + pair.new.index.records.len();
    let unpaid = at(pair.changes.len() + records + 2, PAIR_BYTES);
    assert_eq!(
        (
            unpaid.impact.hits.iter().any(|h| h.node == "0:13"),
            unpaid.impact.reasons.contains_key("seeds_truncated")
        ),
        (false, true)
    );
}

#[test]
fn a_partial_comparison_leaves_its_impact_incomplete_without_changes() {
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    let (a, b) = (styled(12.0), styled(12.0));
    let (a, b) = (Tree::new(&a), Tree::new(&b));
    let conn = saved(&[&a, &b], "0:10");
    let r = revision(
        &conn,
        (1, side("old")),
        (2, side("new")),
        "0:10",
        REVISION_BYTES,
    )
    .unwrap();
    assert_eq!(r.coverage["complete"], false);
    assert_eq!((r.rows.len(), r.impact.complete), (0, false));
    assert_eq!(r.impact.reasons.get("source_comparison_partial"), Some(&1));
}
#[test]
fn a_variable_moved_to_another_corner_is_a_change_and_the_same_binding_is_not() {
    let corner = |field: &'static str| {
        let alias = obj(vec![
            ("dataType", K::Enum("VariableDataType", "ALIAS")),
            ("value", obj(vec![("alias", obj(vec![("guid", guid(20))]))])),
        ]);
        doc(vec![
            node(20, Some(1), "VARIABLE", "Radius", vec![]),
            node(10, Some(1), "FRAME", "Home", vec![size(400.0, 800.0)]),
            node(
                12,
                Some(10),
                "RECTANGLE",
                "Card",
                vec![(
                    "parameterConsumptionMap",
                    obj(vec![(
                        "entries",
                        K::Array(vec![obj(vec![
                            ("variableField", K::Enum("VariableField", field)),
                            ("variableData", alias),
                        ])]),
                    )]),
                )],
            ),
        ])
    };
    let (top, bottom) = (
        "RECTANGLE_TOP_LEFT_CORNER_RADIUS",
        "RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS",
    );
    assert_eq!(
        (
            nodes(&run(corner(top), corner(bottom))),
            nodes(&run(corner(top), corner(top)))
        ),
        (vec![("0:12".into(), "changed".into())], vec![])
    );
}

/// The impact of revision 1 → 2 saved from `old` and `new`.
fn impact_of(old: K<'static>, new: K<'static>, work: usize) -> super::super::impact::Impact {
    let side = |label: &str| json!({"sourceId": format!("local:{label}"), "snapshotId": format!("sha256:{label}")});
    let conn = saved(&[&Tree::new(&old), &Tree::new(&new)], "0:0");
    let bounds = Bounds {
        work,
        bytes: PAIR_BYTES,
        depth: DEPTH,
        deadline: Instant::now() + DEADLINE,
        stop: None,
    };
    revision_with(
        &conn,
        (1, side("old")),
        (2, side("new")),
        "0:0",
        REVISION_BYTES,
        bounds,
    )
    .unwrap()
    .impact
}

#[test]
fn a_variable_only_change_reaches_the_layer_bound_to_it_and_no_other() {
    let screen = |value| {
        let mut d = bound_screen(value);
        let K::Object(_, fields) = &mut d else {
            unreachable!()
        };
        let Some(K::Array(nodes)) = fields.get_mut("nodeChanges") else {
            unreachable!()
        };
        nodes.push(node(13, Some(10), "RECTANGLE", "Unbound", vec![]));
        d
    };
    let out = impact_of(screen(1.0), screen(2.0), WORK);
    use super::super::impact::Class;
    let hits: Vec<_> = out
        .hits
        .iter()
        .map(|h| {
            (
                h.node.as_str(),
                h.class,
                h.via.clone(),
                h.frames[1].as_deref(),
            )
        })
        .collect();
    // The binding alone proves nothing equal, so the bound layer stays possible.
    let via = Some(("binds_variable", "VariableID:0:60".to_owned()));
    let unknown = std::collections::BTreeMap::from([("comparison_unknown", 1)]);
    assert_eq!(
        (hits, out.reasons),
        (
            vec![
                ("0:12", Class::Possible, via, Some("0:10")),
                ("0:60", Class::Direct, None, Some("0:60")),
            ],
            unknown
        )
    );
}

#[test]
fn a_variable_change_reaches_the_layer_bound_through_an_alias_of_it() {
    // Variable 61 aliases 60; rectangle 12 binds 61. The alias hop counts once per side.
    let screen = |value| {
        let entry = obj(vec![("modeID", guid(70)), ("variableData", alias(60))]);
        let values = obj(vec![("entries", K::Array(vec![entry]))]);
        let paint = obj(vec![
            ("type", K::Enum("PaintType", "SOLID")),
            ("colorVar", alias(61)),
        ]);
        doc(vec![
            variable(value),
            node(
                61,
                Some(1),
                "VARIABLE",
                "Accent",
                vec![("variableDataValues", values)],
            ),
            node(10, Some(1), "FRAME", "Home", vec![]),
            node(
                12,
                Some(10),
                "RECTANGLE",
                "Card",
                vec![("fillPaints", K::Array(vec![paint]))],
            ),
        ])
    };
    let out = impact_of(screen(1.0), screen(2.0), WORK);
    use super::super::impact::Class;
    let hits: Vec<_> = out
        .hits
        .iter()
        .map(|h| (h.node.as_str(), h.class, h.via.clone()))
        .collect();
    let via = |t: &str| Some(("binds_variable", t.to_owned()));
    let unknown = std::collections::BTreeMap::from([("comparison_unknown", 3)]);
    assert_eq!(
        (hits, out.complete, out.reasons),
        (
            vec![
                ("0:12", Class::Possible, via("VariableID:0:61")),
                ("0:60", Class::Direct, None),
                ("0:61", Class::Possible, via("VariableID:0:60")),
            ],
            false,
            unknown
        )
    );
}

#[test]
fn a_collection_only_change_reaches_its_variables_and_the_layer_bound_to_them() {
    let out = impact_of(collected(70), collected(71), WORK);
    let reached: Vec<_> = out
        .hits
        .iter()
        .map(|h| (h.node.as_str(), h.class))
        .collect();
    use super::super::impact::Class;
    assert!(
        reached.contains(&("0:64", Class::Direct)) && reached.contains(&("0:61", Class::Possible)),
        "{reached:?}"
    );
    assert!(
        out.hits
            .iter()
            .any(|h| h.via == Some(("binds_variable", "VariableID:0:61".into())))
    );
    // No layer's change was proven, so the impact never reads complete.
    assert!(!out.complete && out.reasons.contains_key("comparison_unknown"));
}

/// Variables 60 and 61 alias each other; rectangle 12 binds 61, and 60 holds `value` too.
fn cycle(value: f32) -> K<'static> {
    let values = |to: u32| {
        let entry = obj(vec![("modeID", guid(70)), ("variableData", alias(to))]);
        obj(vec![("entries", K::Array(vec![entry]))])
    };
    let paint = obj(vec![
        ("type", K::Enum("PaintType", "SOLID")),
        ("colorVar", alias(61)),
    ]);
    doc(vec![
        node(
            60,
            Some(1),
            "VARIABLE",
            "A",
            vec![
                ("variableDataValues", values(61)),
                ("variableScopes", K::Float(value)),
            ],
        ),
        node(
            61,
            Some(1),
            "VARIABLE",
            "B",
            vec![("variableDataValues", values(60))],
        ),
        node(10, Some(1), "FRAME", "Home", vec![]),
        node(
            12,
            Some(10),
            "RECTANGLE",
            "Card",
            vec![("fillPaints", K::Array(vec![paint]))],
        ),
    ])
}

#[test]
fn an_alias_cycle_ends_and_reaches_the_bound_layer() {
    let out = impact_of(cycle(1.0), cycle(2.0), WORK);
    let ids: Vec<_> = out.hits.iter().map(|h| h.node.as_str()).collect();
    assert_eq!(ids, ["0:12", "0:60", "0:61"]);
}

/// `n` variables aliasing variable 60, which holds `value`; layer 12 binds the last.
fn fan(n: u32, value: f32) -> K<'static> {
    let mut nodes = vec![variable(value)];
    for v in 0..n {
        let entry = obj(vec![("modeID", guid(70)), ("variableData", alias(60))]);
        let values = obj(vec![("entries", K::Array(vec![entry]))]);
        nodes.push(node(
            100 + v,
            Some(1),
            "VARIABLE",
            "V",
            vec![("variableDataValues", values)],
        ));
    }
    doc(nodes)
}

#[test]
fn each_side_s_aliases_are_scanned_once_however_many_variables_ask() {
    let (old, new) = (fan(40, 1.0), fan(40, 2.0));
    let (a, b) = (Tree::new(&old), Tree::new(&new));
    let pair = compare_trees(&a, &b, [WORK, WORK, PAIR_WORK], "0:0");
    let mut evidence = PairEvidence {
        pair: &pair,
        ids: None,
        orders: None,
        aliased: [None, None],
    };
    let mut bounds = super::super::tests::bounds(WORK);
    let mut ask = |key: &str| {
        let before = bounds.work;
        let ids = super::super::impact::Evidence::aliasing(&mut evidence, 1, key, &mut bounds);
        (ids.map(|v| v.len()), before - bounds.work)
    };
    let (first, scan) = ask("VariableID:0:60");
    assert_eq!(first, Some(40));
    // Each later ask pays only for the ids it returns, never another scan of the side.
    let later: Vec<_> = (100..140)
        .map(|v| ask(&format!("VariableID:0:{v}")))
        .collect();
    assert!(
        scan > 40 && later.iter().all(|&l| l == (Some(0), 0)),
        "{scan} {later:?}"
    );
}

#[test]
fn a_variable_aliased_by_an_unbound_one_leaves_the_impact_incomplete() {
    let out = impact_of(fan(1, 1.0), fan(1, 2.0), WORK);
    let ids: Vec<_> = out.hits.iter().map(|h| h.node.as_str()).collect();
    assert_eq!((ids, out.complete), (vec!["0:100", "0:60"], false));
}

/// Revisions of each pair from both stored generations and from the new one still in memory,
/// as a worker that just saved it compares them.
fn stored_and_live() -> Vec<(Revision, Revision)> {
    let side =
        |l: &str| json!({"sourceId": format!("local:{l}"), "snapshotId": format!("sha256:{l}")});
    let other = || node(20, Some(1), "FRAME", "Other", vec![]);
    let mut moved = screen("Hi", 0.0, 420.0);
    moved.push(node(14, Some(1), "FRAME", "Other", vec![]));
    moved[3] = node(13, Some(14), "RECTANGLE", "Same", vec![fill(0.5)]);
    // A library component's copy and a library style a layer uses, which a stored build keeps
    // as references.
    let s = |v: &str| K::String(v.into());
    let library = |style: &str| {
        let r = obj(vec![("key", s(style)), ("version", s("v1"))]);
        doc(vec![
            node(
                10,
                Some(1),
                "SYMBOL",
                "Button",
                vec![("componentKey", s("k1")), ("sourceLibraryKey", s("lib"))],
            ),
            instance(20, 1, "One", 10, vec![]),
            node(
                40,
                Some(1),
                "RECTANGLE",
                "Box",
                vec![("styleIdForFill", obj(vec![("assetRef", r)]))],
            ),
        ])
    };
    let pairs = [
        (library("s1"), library("s2"), "0:0"),
        (impacted(1.0, false), impacted(0.5, false), "0:0"),
        (
            doc(vec![
                node(10, Some(1), "FRAME", "Home", vec![]),
                node(13, Some(10), "TEXT", "Label", vec![]),
                other(),
            ]),
            doc(vec![other()]),
            "0:0",
        ),
        (doc(screen("Hello", 1.0, 400.0)), doc(moved), "0:0"),
        (styled(12.0), styled(16.0), "0:10"),
        // The scope itself is deleted: the new side is an empty build.
        (
            doc(vec![
                node(30, Some(1), "FRAME", "Gone", vec![]),
                node(31, Some(30), "TEXT", "Label", vec![]),
                other(),
            ]),
            doc(vec![other()]),
            "0:30",
        ),
    ];
    pairs
        .into_iter()
        .map(|(old, new, scope)| {
            let (a, b) = (Tree::new(&old), Tree::new(&new));
            let conn = saved_with(&[&a, &b], scope, crate::search::Library::Stored);
            let old = || (1, side("old"));
            let stored = revision(&conn, old(), (2, side("new")), scope, REVISION_BYTES).unwrap();
            let deadline = Instant::now() + DEADLINE;
            let (index, kept) = tracked(&b, scope, deadline, crate::search::Library::Stored);
            let new = (index, kept, side("new"));
            let live = revision_live(&conn, old(), new, scope, REVISION_BYTES).unwrap();
            (stored, live)
        })
        .collect()
}

#[test]
fn a_new_generation_still_in_memory_revises_exactly_like_its_stored_copy() {
    let pairs = stored_and_live();
    let json = |(s, l): &(Revision, Revision)| (s.to_json(), l.to_json());
    let (stored, live): (Vec<_>, Vec<_>) = pairs.iter().map(json).unzip();
    assert_eq!(live, stored);
}

#[test]
fn a_revision_reads_back_from_its_json_unchanged() {
    let written: Vec<Json> = stored_and_live().iter().map(|(s, _)| s.to_json()).collect();
    let read: Vec<Option<Json>> = written
        .iter()
        .map(|v| Revision::from_json(v).map(|r| r.to_json()))
        .collect();
    assert_eq!(read, written.into_iter().map(Some).collect::<Vec<_>>());
}

#[test]
fn a_revision_with_an_unknown_impact_reason_does_not_read_back() {
    let (stored, _) = stored_and_live().swap_remove(0);
    let mut v = stored.to_json();
    v["impact"]["reasons"]["not_a_reason"] = json!(1);
    assert!(Revision::from_json(&v).is_none());
}

/// A layer of frame 10 using a library text style, stored as local copy `local` of key `key`.
fn library_styled(local: u32, key: &str) -> K<'static> {
    let s = |v: &str| K::String(v.into());
    let reference = obj(vec![
        ("guid", guid(local)),
        ("assetRef", obj(vec![("key", s(key)), ("version", s("v1"))])),
    ]);
    doc(vec![
        node(
            local,
            Some(1),
            "TEXT",
            "Body",
            vec![("fontSize", K::Float(12.0))],
        ),
        node(10, Some(1), "FRAME", "Home", vec![]),
        node(
            11,
            Some(10),
            "TEXT",
            "Title",
            vec![text("Hi"), ("styleIdForText", reference)],
        ),
    ])
}

#[test]
fn a_reimported_library_style_with_a_new_local_id_is_no_change() {
    let out = scoped(library_styled(20, "k"), library_styled(21, "k"), "0:10");
    assert_eq!(nodes(&out), Vec::<(String, String)>::new(), "{out}");
}

#[test]
fn a_layer_moved_to_another_library_style_is_a_bindings_change() {
    let out = scoped(library_styled(20, "k"), library_styled(20, "other"), "0:10");
    let facets = &out["frames"][0]["nodes"][0]["facets"];
    assert_eq!(
        (nodes(&out), facets),
        (
            vec![("0:11".into(), "changed".into())],
            &json!(["bindings"])
        ),
        "{out}"
    );
}
