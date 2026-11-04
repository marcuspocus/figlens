use super::*;

#[test]
fn catalog_lists_the_rest_types_nodes_uses() {
    let message = crate::test_support::component_set();
    let set = &catalog(&Tree::new(&message), "0:0")["children"][0];
    assert_eq!(
        (&set["type"], &set["children"][0]["type"]),
        (&json!("COMPONENT_SET"), &json!("COMPONENT"))
    );
}

#[test]
fn a_too_deep_file_fails_the_catalog_instead_of_aborting() {
    let id = |k: usize| {
        if k == 0 {
            "0:0".to_owned()
        } else {
            k.to_string()
        }
    };
    let tree = Tree {
        nodes: std::collections::HashMap::new(),
        children: (0..=crate::expand::MAX_DEPTH)
            .map(|k| (id(k), vec![id(k + 1)]))
            .collect(),
        blobs: Vec::new(),
    };
    let error = checked_catalog(&tree).unwrap_err();
    assert!(error.to_string().contains("layer depth"), "{error}");
}

#[test]
fn live_original_image_route_obeys_the_image_budget() {
    let dir = crate::test_support::TestDir::new();
    let file = dir.oversized_image("a1b2");
    let doc = Document::open(&file).unwrap();
    let message = doc.message().unwrap();
    let tree = Tree::new(&message);
    let version = fs::metadata(&file).unwrap().modified().unwrap();
    let revision = Revision {
        tree: &tree,
        index: Value::Null,
        file: &file,
        initial: None,
        generation: 1,
        version,
        workspace: &dir.join("bundles"),
        authority: "127.0.0.1:1234",
        token: "test",
        memory: Mutex::new(Memory::default()),
        disk: Mutex::new(DiskCache {
            dir: Some(dir.join("cache")),
            used: 0,
        }),
        bundles: AtomicUsize::new(0),
        source: Value::Null,
        indexes: Mutex::default(),
        queries: cancel::Requests::default(),
        indexing: &semantic::Indexing::default(),
        cancel: std::sync::atomic::AtomicBool::new(false),
    };
    let error = revision.respond("asset/1/0:1/a1b2").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("parse budget exceeded: image bytes")
    );
    assert!(revision.respond("catalog").is_ok());
}
fn served<T>(check: impl FnOnce(&Revision) -> T) -> T {
    let file = Path::new("examples/first-success/design.fig");
    let doc = staged(file).unwrap().0;
    let message = doc.message().unwrap();
    served_tree(&Tree::new(&message), check)
}

fn served_tree<T>(tree: &Tree, check: impl FnOnce(&Revision) -> T) -> T {
    let file = Path::new("examples/first-success/design.fig");
    let (_, version, source) = staged(file).unwrap();
    let dir = crate::test_support::TestDir::new();
    let revision = Revision {
        tree,
        index: Value::Null,
        file,
        initial: None,
        generation: 1,
        version,
        workspace: &dir.join("bundles"),
        authority: "127.0.0.1:1234",
        token: "test",
        memory: Mutex::new(Memory::default()),
        disk: Mutex::new(DiskCache { dir: None, used: 0 }),
        bundles: AtomicUsize::new(0),
        source,
        indexes: Mutex::default(),
        queries: cancel::Requests::default(),
        indexing: &semantic::Indexing::default(),
        cancel: std::sync::atomic::AtomicBool::new(false),
    };
    check(&revision)
}

fn reply(revision: &Revision, route: &str) -> Value {
    serde_json::from_slice(&revision.respond(route).unwrap().1).unwrap()
}

#[test]
fn search_and_usages_name_the_snapshot_of_the_bytes_served() {
    let bytes = fs::read("examples/first-success/design.fig").unwrap();
    let snapshot = search::snapshot_id(&bytes);
    served(|r| {
        let found = reply(r, "search/1?q=card");
        let usages = reply(r, "usages/1?target=0%3A2&relation=contains");
        assert_eq!(
            (
                &found["results"][0]["ref"]["snapshotId"],
                &found["results"][0]["ref"]["nodeId"],
                &usages["source"]["snapshotId"],
                usages["error"].is_null(),
                &reply(r, "revision")["source"]["snapshotId"],
            ),
            (
                &json!(snapshot),
                &json!("0:2"),
                &json!(snapshot),
                true,
                &json!(snapshot)
            )
        );
    });
}

#[test]
fn live_handoff_names_its_file_and_scope_coverage_while_snapshots_omit_the_path() {
    let file = Path::new("examples/first-success/design.fig");
    let path = fs::canonicalize(file).unwrap().display().to_string();
    let checked = |c: &Value, id: &str, unchecked: Value| {
        assert_eq!(c["scopeId"], id, "{c}");
        let clean = c["warnings"].as_array().unwrap().is_empty();
        assert_eq!(
            c["complete"],
            if clean { unchecked } else { json!(false) },
            "{c}"
        );
    };
    served(|r| {
        assert_eq!(reply(r, "revision")["source"]["path"], path);
        let coverage = &reply(r, "data/1/0:2")["coverage"];
        checked(coverage, "0:2", Value::Null);
        assert_eq!(coverage["reasons"], json!(["asset_renders_unchecked"]));
    });
    let dir = crate::test_support::TestDir::new();
    let out = dir.join("snapshot.html");
    snapshot(file, "0:2", &out).unwrap();
    let html = fs::read_to_string(&out).unwrap();
    // Raw and JSON-escaped (Windows backslashes) forms of the live path.
    assert!(!html.contains(&path) && !html.contains(json!(path).to_string().trim_matches('"')));
    let boot = html
        .split(r#"<script id="boot" type="application/json">"#)
        .nth(1)
        .unwrap();
    let boot: Value = serde_json::from_str(boot.split("</script>").next().unwrap()).unwrap();
    checked(&boot["snapshot"]["coverage"], "0:2", json!(true));
    assert_eq!(boot["snapshot"]["coverage"]["reasons"], json!([]));
}

#[test]
fn relation_routes_refuse_stale_revisions_and_unknown_or_bad_parameters() {
    served(|r| {
        let refused = |route: &str| r.respond(route).unwrap_err().to_string();
        assert_eq!(
            [
                refused("search/2?q=card"),
                refused("search/1?q=card&path=x"),
                refused("search/1?q=card&scope=..%2F"),
            ],
            [
                STALE.to_owned(),
                "unknown parameter path".to_owned(),
                "scope takes a node ID".to_owned(),
            ]
        );
        // Refused values answer as data, with the served source.
        assert_eq!(
            (
                &reply(r, "usages/1?target=0:2&hidden=yes")["error"]["message"],
                reply(r, "search/1?q=%20")["error"]["message"].is_string(),
            ),
            (&json!("hidden takes 0 or 1"), true)
        );
        // None of them paid for an index build.
        assert!(r.indexes.lock().unwrap().is_empty());
    });
}

#[test]
fn a_cursor_from_an_evicted_index_is_refused_by_a_different_rebuild() {
    const PAGE: &str = "usages/1?target=0%3A2&relation=contains&limit=1";
    served(|r| {
        let first = reply(r, PAGE);
        let cursor = first["nextCursor"].as_str().unwrap().to_owned();
        assert!(reply(r, &format!("{PAGE}&cursor={cursor}"))["error"].is_null());
        // Evicted, then rebuilt as another partial corpus of the same snapshot, kept like
        // one a byte limit stopped (a deadline stop would be rebuilt whole on next use).
        let mut partial = search::index_tree(r.tree, "0:0", std::time::Instant::now()).unwrap();
        partial.stop = Some("text_bytes_limit");
        let cell = Built::default();
        assert!(cell.set(Ok(Arc::new(partial))).is_ok());
        *r.indexes.lock().unwrap() = vec![("0:0".to_owned(), cell)];
        let paged = reply(r, &format!("{PAGE}&cursor={cursor}"));
        assert_eq!(
            (paged["results"].is_null(), &paged["source"]["snapshotId"]),
            (true, &first["source"]["snapshotId"]),
            "{paged}"
        );
        assert_eq!(paged["error"]["kind"], "CursorStale", "{paged}");
    });
}

#[test]
fn a_deadline_stopped_index_keeps_its_slot_until_idle_then_rebuilds_there() {
    served(|r| {
        let partial =
            Arc::new(search::index_tree(r.tree, "0:0", std::time::Instant::now()).unwrap());
        assert_eq!(partial.stop, Some("deadline"));
        let cell = Built::default();
        assert!(cell.set(Ok(partial.clone())).is_ok());
        *r.indexes.lock().unwrap() = vec![("0:0".to_owned(), cell)];
        let other = r.index("0:1").unwrap();
        let kind = |e: Box<dyn Error>| e.downcast_ref::<FigError>().map(|f| f.kind());
        // While a reply holds the partial index it stays counted: no rebuild, no third slot.
        assert_eq!(
            r.index("0:0").err().and_then(kind),
            Some(crate::ErrorKind::WorkspaceBusy)
        );
        assert_eq!(
            r.index("0:2").err().and_then(kind),
            Some(crate::ErrorKind::WorkspaceBusy)
        );
        drop((partial, other));
        assert_eq!(r.index("0:0").unwrap().stop, None);
        assert_eq!(r.indexes.lock().unwrap().len(), 2);
    });
}

#[test]
fn a_deep_effective_result_opens_and_queries_by_its_exact_id() {
    use kiwi_schema::Value as K;
    let obj = |f: Vec<(&'static str, K<'static>)>| K::Object("Test", f.into_iter().collect());
    let named = |kind: &'static str, name: &str| {
        obj(vec![
            ("type", K::Enum("NodeType", kind)),
            ("name", K::String(name.to_owned())),
        ])
    };
    let instance = |main: u32| {
        obj(vec![
            ("type", K::Enum("NodeType", "INSTANCE")),
            ("name", K::String("level".into())),
            (
                "symbolData",
                obj(vec![(
                    "symbolID",
                    obj(vec![("sessionID", K::UInt(1)), ("localID", K::UInt(main))]),
                )]),
            ),
        ])
    };
    // Main 1:k holds an instance of main 1:k+1; the last main holds the target layer.
    const LEVELS: u32 = 100;
    let mut nodes = vec![
        ("0:0".to_owned(), named("DOCUMENT", "doc")),
        ("0:1".to_owned(), named("CANVAS", "page")),
        ("3:0".to_owned(), named("FRAME", "screen")),
        ("4:0".to_owned(), instance(0)),
        ("5:0".to_owned(), named("RECTANGLE", "deepest")),
    ];
    let mut children = HashMap::from([
        ("0:0".to_owned(), vec!["0:1".to_owned()]),
        ("0:1".to_owned(), vec!["3:0".to_owned()]),
        ("3:0".to_owned(), vec!["4:0".to_owned()]),
        (format!("1:{LEVELS}"), vec!["5:0".to_owned()]),
    ]);
    for k in 0..=LEVELS {
        nodes.push((format!("1:{k}"), named("COMPONENT", "main")));
        if k < LEVELS {
            nodes.push((format!("2000000000:{k}"), instance(k + 1)));
            children.insert(format!("1:{k}"), vec![format!("2000000000:{k}")]);
        }
    }
    let tree = Tree {
        nodes: nodes.iter().map(|(id, n)| (id.clone(), n)).collect(),
        children,
        blobs: Vec::new(),
    };
    served_tree(&tree, |r| {
        let found = reply(r, "search/1?q=deepest");
        let id = found["results"][0]["ref"]["nodeId"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(id.len() > 1024 && id.ends_with(";5:0"), "{found}");
        // Opening it reads the layer's own data at that exact ID.
        let (_, data) = r.respond(&format!("data/1/{id}")).unwrap();
        let data: Value = serde_json::from_slice(&data).unwrap();
        assert_eq!(data["nodes"]["document"]["id"], id.as_str());
        // Its usages request, as the viewer encodes it, is admitted and reaches core intact.
        let query = format!(
            "target={}&relation=contains&outgoing=1",
            id.replace(':', "%3A").replace(';', "%3B")
        );
        let line = format!(
            "GET /secret/usages/1?{query}&scope=0%3A0 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n"
        );
        let route = parse(&line).unwrap();
        let usages = reply(r, &route);
        assert_eq!(
            usages["definition"]["ref"]["nodeId"],
            id.as_str(),
            "{usages}"
        );
    });
}

fn parse(input: &str) -> Result<String> {
    Ok(parse_with_body(input)?.0)
}

fn parse_with_body(input: &str) -> Result<(String, Option<Vec<u8>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let mut client = TcpStream::connect(listener.local_addr()?)?;
    let (server, _) = listener.accept()?;
    client.write_all(input.as_bytes())?;
    client.shutdown(std::net::Shutdown::Write)?;
    request(&server, "127.0.0.1:1234", "secret")
}

/// A POST of `body` to `route` with `extra` header lines, as the page sends it unless `extra` says otherwise.
fn post(route: &str, extra: &str, body: &str) -> Result<(String, Option<Vec<u8>>)> {
    parse_with_body(&format!(
        "POST /secret/{route} HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n{extra}Content-Length: {}\r\n\r\n{body}",
        body.len()
    ))
}

#[test]
fn only_a_same_origin_json_post_within_its_cap_reaches_a_state_change() {
    let page = "Origin: http://127.0.0.1:1234\r\nContent-Type: application/json\r\nSec-Fetch-Site: same-origin\r\n";
    let body = r#"{"scope":"0:1"}"#;
    let ok = post("enable/1", page, body).unwrap();
    assert_eq!(ok, ("enable/1".to_owned(), Some(body.as_bytes().to_vec())));
    for (extra, why) in [
        ("Content-Type: application/json\r\n", "no origin"),
        (
            "Origin: http://evil.test\r\nContent-Type: application/json\r\n",
            "another origin",
        ),
        ("Origin: http://127.0.0.1:1234\r\n", "not JSON"),
        (
            "Origin: http://127.0.0.1:1234\r\nContent-Type: application/json\r\nSec-Fetch-Site: same-site\r\n",
            "same-site",
        ),
    ] {
        assert!(post("enable/1", extra, body).is_err(), "{why}");
    }
    assert!(
        post("enable/1", page, &"x".repeat(POST_BYTES + 1)).is_err(),
        "over the cap"
    );
    // A body shorter than declared never completes.
    assert!(parse_with_body(&format!("POST /secret/enable/1 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n{page}Content-Length: 40\r\n\r\n{body}")).is_err());
    // A state change is never a GET, and data routes are never a POST.
    assert!(parse("GET /secret/enable/1 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n").is_err());
    assert!(post("search/1?q=x", page, body).is_err());
}

#[test]
fn an_enable_that_outlived_its_revision_starts_no_build() {
    served(|r| {
        // The revision ended (a save or a stop) while this request was still being read.
        r.cancel.store(true, Ordering::SeqCst);
        let (_, out) = r.post("enable/1", br#"{"scope":"0:1"}"#).unwrap();
        let reply: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(reply["error"]["message"], STALE, "{reply}");
        assert!(r.indexing.0.lock().unwrap().is_none());
    });
}

#[test]
fn enable_takes_one_stored_scope_of_the_current_revision() {
    served(|r| {
        let refused = |rev: &str, body: &str| {
            let (_, out) = r.post(&format!("enable/{rev}"), body.as_bytes()).unwrap();
            serde_json::from_slice::<Value>(&out).unwrap()["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        };
        assert!(refused("1", "[]").contains("JSON object"));
        assert!(refused("1", r#"{}"#).contains("exactly one scope"));
        assert!(refused("1", r#"{"scope":"0:1","x":1}"#).contains("exactly one scope"));
        assert!(refused("1", r#"{"scope":"9:9"}"#).contains("stored layer"));
        assert!(refused("1", r#"{"scope":"I0:2;0:3"}"#).contains("stored layer"));
        assert_eq!(refused("2", r#"{"scope":"0:1"}"#), STALE);
    });
}

#[test]
fn query_decoding_matches_url_search_params() {
    let pairs = form("q=a+b%2Bc%E4%B8%AD&type=TEXT&type=FRAME&cursor=").unwrap();
    let text = |i: usize| (pairs[i].0.as_str(), pairs[i].1.as_str());
    assert_eq!(
        (text(0), text(1), text(2), text(3)),
        (
            ("q", "a b+c中"),
            ("type", "TEXT"),
            ("type", "FRAME"),
            ("cursor", "")
        )
    );
    assert!(form("q=%E4%B8").is_err() && form("q=%zz").is_err());
    assert!(form(&"a=1&".repeat(MAX_PARAMS + 1)).is_err());
}

#[test]
fn one_index_per_scope_is_shared_and_held_ones_are_never_evicted() {
    served(|r| {
        let (a, again) = (r.index("0:0").unwrap(), r.index("0:0").unwrap());
        let b = r.index("0:1").unwrap();
        let busy = r.index("0:2").map(|_| ()).unwrap_err().to_string();
        drop((again, b));
        let c = r.index("0:2").map(|_| ());
        let kept = Arc::ptr_eq(&a, &r.index("0:0").unwrap());
        // A scope that fails to build is not cached, so it is tried again.
        assert!(r.index("9:9").is_err());
        assert!(!r.indexes.lock().unwrap().iter().any(|(s, _)| s == "9:9"));
        r.indexes
            .lock()
            .unwrap()
            .push(("8:8".to_owned(), Built::default()));
        let building = r.index("8:8").map(|_| ()).unwrap_err().to_string();
        assert!(building.contains("still building"), "{building}");
        assert_eq!(
            (busy.contains("building or in use"), c.is_ok(), kept),
            (true, true, true)
        );
    });
}

#[test]
fn disk_cache_round_trips_renders_and_their_export_area() {
    let dir = crate::test_support::TestDir::new();
    // Keep the owned root reserved while clearing the cache.
    let cache = dir.join("cache");
    fs::create_dir(&cache).unwrap();
    let mut disk = DiskCache {
        dir: Some(cache),
        used: 0,
    };
    disk.put(
        "image/1:2/",
        "image/png",
        Some("0,0,10,20"),
        b"\x89PNG\nbytes",
    );
    let (mime, area, bytes) = disk.get("image/1:2/").unwrap();
    assert_eq!(
        (mime, area.as_deref(), bytes.as_slice()),
        ("image/png", Some("0,0,10,20"), &b"\x89PNG\nbytes"[..])
    );
    assert!(disk.get("image/1:3/").is_none());
    disk.used = DISK_CACHE_BYTES;
    disk.put("thumb/1:2/", "image/png", None, b"x");
    assert!(
        disk.get("thumb/1:2/").is_none(),
        "the size cap stops new entries"
    );
    assert!(disk.clear().unwrap() > 0);
    assert!(
        disk.get("image/1:2/").is_none(),
        "clearing removes cached renders"
    );
    disk.put("image/1:2/", "image/png", None, b"again");
    assert!(
        disk.get("image/1:2/").is_some(),
        "the cache keeps working after a clear"
    );
    assert_eq!(clear_cache(&dir.join("missing")).unwrap(), 0);
}

#[test]
fn pick_geometry_keeps_only_shapes_their_box_misdescribes() {
    let node = |id: &str, kind: &str, m: [f64; 6]| json!({"id": id, "type": kind, "_r": {"m": m, "size": {"x": 10.0, "y": 20.0}}});
    let mut root = node("1:1", "FRAME", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    root["children"] = json!([
        node("1:2", "RECTANGLE", [0.0, 1.0, -1.0, 0.0, 5.0, 5.0]),
        node("1:3", "ELLIPSE", [1.0, 0.0, 0.0, 1.0, 2.0, 3.0]),
        node("1:4", "RECTANGLE", [1.0, 0.0, 0.0, 1.0, 2.0, 3.0]),
    ]);
    let mut out = serde_json::Map::new();
    pick_geometry(&root, &mut out);
    assert_eq!(out.keys().collect::<Vec<_>>(), ["1:2", "1:3"]);
    assert_eq!(
        out["1:2"],
        json!([0.0, 1.0, -1.0, 0.0, 5.0, 5.0, 10.0, 20.0, 0])
    );
    assert_eq!(out["1:3"][8], json!(1));
}

#[test]
fn embedded_design_cannot_close_script() {
    let page = html(&json!({"name":"</ScRiPt><script>globalThis.pwned=1</script>&"}));
    assert!(!page.contains("</ScRiPt>"));
    assert!(page.contains("\\u003c/ScRiPt\\u003e"));
    let page = html(&json!({"name":"/*SCRIPT*/ /*STYLE*/ /*DATA*/"}));
    assert!(page.contains(r#"{"name":"/*SCRIPT*/ /*STYLE*/ /*DATA*/"}"#));
}
#[test]
fn requests_reject_cross_origin_hosts_methods_and_oversized_headers() {
    assert_eq!(
        parse("GET /secret/data/I1:2;3:4 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n").unwrap(),
        "data/I1:2;3:4"
    );
    assert_eq!(
            parse("GET /secret/ HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nSec-Fetch-Site: cross-site\r\nSec-Fetch-Mode: navigate\r\nSec-Fetch-Dest: document\r\n\r\n").unwrap(),
            ""
        );
    for input in [
            "GET /secret/catalog HTTP/1.1\r\nHost: evil.test\r\n\r\n".to_owned(),
            String::new(),
            "GET /secret/catalog HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nOrigin: https://evil.test\r\n\r\n".to_owned(),
            "GET /secret/catalog HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nSec-Fetch-Site: cross-site\r\n\r\n".to_owned(),
            "GET /secret/catalog HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nSec-Fetch-Site: cross-site\r\nSec-Fetch-Mode: navigate\r\nSec-Fetch-Dest: document\r\n\r\n".to_owned(),
            "GET /secret/ HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nSec-Fetch-Site: cross-site\r\nSec-Fetch-Mode: navigate\r\nSec-Fetch-Dest: iframe\r\n\r\n".to_owned(),
            "POST /secret/bundle/1:2 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n".to_owned(),
            "GET /wrong/catalog HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n".to_owned(),
            format!("GET /secret/catalog HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nX: {}\r\n\r\n", "x".repeat(9000)),
            format!("GET /secret/data/{} HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n", "1".repeat(MAX_LINE)),
        ] { assert!(parse(&input).is_err(), "{input}"); }
}

#[test]
fn scoped_routes_pin_images_and_bundles_to_the_loaded_revision() {
    assert_eq!(
        scope_route("image/7/I1:2;3:4", 7).unwrap(),
        ("image", "I1:2;3:4")
    );
    assert_eq!(
        scope_route("asset/7/I1:2;3:4/a1b2c3", 7).unwrap(),
        ("asset", "I1:2;3:4/a1b2c3")
    );
    assert_eq!(
        scope_route("export/7/I1:2;3:4/webp-0.5-80", 7).unwrap(),
        ("export", "I1:2;3:4/webp-0.5-80")
    );
    assert_eq!(
        scope_route("export/7/1:2/svg-1-80", 7).unwrap(),
        ("export", "1:2/svg-1-80")
    );
    assert_eq!(
        scope_route("view/7/1:2/1024", 7).unwrap(),
        ("view", "1:2/1024")
    );
    for route in [
        "image/6/1:2",
        "image/7/../secret",
        "image/7/%31:2",
        "write/7/1:2",
        "image/7/1:2?path=/tmp/x",
        "asset/7/1:2",
        "asset/7/1:2/../x",
        "asset/7/1:2/abc?x",
        "asset/7/../x/abcd",
        "export/7/1:2/gif-1-80",
        "export/7/1:2/png-9-80",
        "export/7/1:2/png-1-0",
        "export/7/1:2/png-1-101",
        "export/7/1:2/png-1-80-x",
        "view/7/1:2/100",
        "view/7/1:2/99999",
    ] {
        assert!(scope_route(route, 7).is_err(), "{route}");
    }
}

#[test]
fn routes_only_accept_exact_instance_ids() {
    for id in ["12:34", "I1:2;I3:4;5:6"] {
        assert!(valid_id(id));
    }
    for id in ["", "../secret", "1:2?x=3", "%2e", "<script>"] {
        assert!(!valid_id(id));
    }
    assert!(!valid_id(&"1".repeat(search::store::ID_BYTES + 1)));
}

#[test]
fn image_posts_have_a_separate_mime_and_declared_byte_cap() {
    let origin = "Origin: http://127.0.0.1:1234\r\n";
    let png = format!("{origin}Content-Type: image/png\r\n");
    let json = format!("{origin}Content-Type: application/json\r\n");
    assert_eq!(
        post("image/1?mode=visual", &png, "PNG").unwrap().1,
        Some(b"PNG".to_vec())
    );
    // A node render keeps its GET route; only the revision-only route accepts a query body.
    assert_eq!(
        parse("GET /secret/image/1/0:1 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n").unwrap(),
        "image/1/0:1"
    );
    assert!(post("image/1/0:1", &png, "PNG").is_err());
    assert!(post("image/1", &json, "{}").is_err());
    assert!(post("enable/1", &png, "PNG").is_err());
    assert!(post("image-ref/1", &png, "PNG").is_err());
    assert!(post("image-ref/1", &json, r#"{"nodeId":"0:1"}"#).is_ok());
    assert!(post("image/1", "Content-Type: image/png\r\n", "PNG").is_err());
    assert!(
        post(
            "image/1",
            &format!("{png}Sec-Fetch-Site: cross-site\r\n"),
            "PNG"
        )
        .is_err()
    );
    assert!(parse("GET /secret/image/1 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n").is_err());
    let over = search::visual::MAX_BYTES + 1;
    assert!(parse_with_body(&format!("POST /secret/image/1 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n{png}Content-Length: {over}\r\n\r\n")).is_err());
    assert!(parse_with_body(&format!("POST /secret/image/1 HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n{png}Content-Length: 20\r\n\r\nPNG")).is_err());
}

#[test]
fn cancellation_names_one_running_call_and_revision_end_stops_the_rest() {
    let calls = cancel::Requests::default();
    let ended = std::sync::atomic::AtomicBool::new(false);
    let first = calls.begin(Some("first"), &ended).unwrap();
    assert!(
        calls.begin(Some("second"), &ended).is_err(),
        "a model wait must not occupy every HTTP worker"
    );
    assert!(calls.begin(Some("first"), &ended).is_err());
    assert!(calls.cancel("first").unwrap());
    assert!(first.cancel.load(Ordering::SeqCst));
    assert!(!calls.cancel("missing").unwrap());
    assert!(
        calls.begin(Some("missing"), &ended).is_err(),
        "an early cancel must stop a queued call"
    );
    assert!(calls.cancel("bad/id").is_err());
    drop(first);
    assert!(!calls.cancel("first").unwrap());
    let second = calls.begin(Some("second"), &ended).unwrap();
    assert!(
        !second.cancel.load(Ordering::SeqCst),
        "a later query does not inherit cancellation"
    );
    ended.store(true, Ordering::SeqCst);
    calls.stop();
    assert!(second.cancel.load(Ordering::SeqCst));
    assert!(calls.begin(Some("late"), &ended).is_err());
}

#[test]
fn a_viewer_no_page_asks_anything_of_stops_after_its_idle_time() {
    let design = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/first-success/design.fig"
    ));
    let started = std::time::Instant::now();
    serve_until(
        design,
        None,
        "127.0.0.1:0".parse().unwrap(),
        &std::sync::atomic::AtomicBool::new(false),
        Some(Duration::from_millis(300)),
    )
    .unwrap();
    assert!(started.elapsed() >= Duration::from_millis(300));
}
