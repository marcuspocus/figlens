use super::*;

#[test]
fn response_budget_counts_escaped_bytes_and_preserves_output_paths_on_failure() {
    let small = response(
        &json!(7),
        json!({"content":[{"type":"text","text":"small"}],"isError":false}),
    );
    assert_eq!(bounded_response(small.clone()), small);
    // Escaped control characters exceed the wire limit while the source string is much smaller.
    let huge = response(
        &json!(8),
        json!({"content":[{"type":"text","text":"\0".repeat(MAX_RESPONSE_BYTES/6+1)}],"isError":false,"structuredContent":{"path":"out.png"}}),
    );
    let error = bounded_response(huge);
    assert_eq!(error["id"], 8);
    assert_eq!(error["result"]["isError"], true);
    assert_eq!(
        error["result"]["_meta"]["figlens"]["code"],
        "response_budget_exceeded"
    );
    assert_eq!(error["result"]["_meta"]["figlens"]["outputPath"], "out.png");
    assert!(serde_json::to_vec(&error).unwrap().len() < 1024);
    let mut exact = ResponseSize(MAX_RESPONSE_BYTES - 1);
    assert_eq!(std::io::Write::write(&mut exact, b"x").unwrap(), 1);
    assert!(std::io::Write::write(&mut exact, b"x").is_err());
}

#[test]
fn request_line_budget_has_exact_boundaries_and_recovers_at_newlines_and_eof() {
    let ping = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}";
    for (extra, ending) in [(0, "\n"), (1, "\n"), (1, "")] {
        let mut line = ping.to_owned();
        line.push_str(&" ".repeat(MAX_REQUEST_BYTES as usize - ping.len() - ending.len() + extra));
        line.push_str(ending);
        let tail = if ending.is_empty() { "" } else { ping };
        line.push_str(tail);
        let mut input = std::io::BufReader::with_capacity(17, line.as_bytes());
        let Some(Admission::Reply(reply)) = next_request(&mut input, &[]).unwrap() else {
            panic!("expected reply")
        };
        if extra == 0 {
            assert_eq!(reply["result"], json!({}));
        } else {
            assert_eq!(reply["error"]["code"], -32600);
        }
        if !tail.is_empty() {
            let Some(Admission::Reply(reply)) = next_request(&mut input, &[]).unwrap() else {
                panic!("expected recovery")
            };
            assert_eq!(reply["result"], json!({}));
        }
        assert!(next_request(&mut input, &[]).unwrap().is_none());
    }
    let mut bytes = vec![0xff, b'\n'];
    bytes.extend_from_slice(ping.as_bytes());
    let replies = replies(bytes.as_slice(), &[]);
    assert_eq!(replies[0]["error"]["code"], -32700);
    assert_eq!(replies[1]["result"], json!({}));
}
use crate::test_support::TestDir;
use std::collections::VecDeque;
use std::io::{BufRead, Read};

fn call(id: Value, tool: &str, arguments: Value) -> String {
    json!({"id":id,"method":"tools/call","params":{"name":tool,"arguments":arguments}}).to_string()
}

fn replies(input: impl BufRead + Send, roots: &[PathBuf]) -> Vec<Value> {
    let mut output = Vec::new();
    serve(input, &mut output, roots).unwrap();
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn rooted_reply(line: &str, roots: &[PathBuf]) -> Value {
    match admit(line, roots) {
        Admission::Reply(reply) => reply,
        Admission::Call(call) => call.execute(None, roots, &[]),
        _ => panic!("expected reply"),
    }
}

fn fixture(dir: &TestDir, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::copy("examples/first-success/design.fig", &path).unwrap();
    path
}

#[test]
fn notification_and_explicit_null_matrix_has_no_hidden_work() {
    let dir = TestDir::new();
    let file = fixture(&dir, "design.fig");
    let out = dir.join("never.png");
    for roots in [vec![], vec![dir.join("").canonicalize().unwrap()]] {
        PREPARATIONS.with(|n| n.set(0));
        PATH_CHECKS.with(|n| n.set(0));
        LOADS.with(|n| n.set(0));
        let mut notifications = vec![
            json!({"method":"unknown"}),
            json!({"method":"tools/call","params":{}}),
            json!({"method":"tools/call","params":{"name":42}}),
            json!({"method":"tools/call","params":{"name":"unknown"}}),
            json!({"method":"tools/call","params":{"name":"tree","arguments":{"depth":"2"}}}),
            json!({"method":"tools/call","params":{"name":"export","arguments":{"file":file,"node":"0:1","out":out}}}),
            json!({"method":"tools/call","params":{"name":"nodes","arguments":{"file":"gs://must-not-contact/design.fig","node":"0:1"}}}),
        ];
        let omitted = notifications.clone();
        for request in &mut notifications {
            request["jsonrpc"] = "2.0".into();
        }
        notifications.extend(omitted);
        let input = notifications
            .iter()
            .map(ToString::to_string)
            .chain([json!({"id":9,"method":"ping"}).to_string()])
            .collect::<Vec<_>>()
            .join("\n");
        let results = replies(input.as_bytes(), &roots);
        assert_eq!(results, vec![response(&json!(9), json!({}))]);
        PREPARATIONS.with(|n| assert_eq!(n.get(), 0));
        PATH_CHECKS.with(|n| assert_eq!(n.get(), 0));
        LOADS.with(|n| assert_eq!(n.get(), 0));
        assert!(!out.exists());

        let messages = [
            "{".to_owned(),
            json!({}).to_string(),
            json!({"method":false}).to_string(),
            json!({"jsonrpc":"1.0","method":"ping"}).to_string(),
            json!({"id":true,"method":"ping"}).to_string(),
            json!({"id":null,"method":"unknown"}).to_string(),
            json!({"id":null,"method":"tools/call","params":{}}).to_string(),
            json!({"id":null,"method":"tools/call","params":{"name":42}}).to_string(),
            call(Value::Null, "unknown", json!({})),
            call(Value::Null, "tree", json!({})),
            call(Value::Null, "nodes", json!({"file":file,"node":"0:1"})),
            json!({"id":null,"method":"ping"}).to_string(),
        ]
        .join("\n");
        let mut results = replies(messages.as_bytes(), &roots);
        // The ping answers as soon as it is read, maybe before the tool calls above it.
        let ping = results
            .iter()
            .position(|r| r.get("result") == Some(&json!({})))
            .unwrap();
        results.remove(ping);
        for (result, code) in results.iter().zip([
            -32700, -32600, -32600, -32600, -32600, -32601, -32602, -32602,
        ]) {
            assert_eq!(result["error"]["code"], code, "{results:?}");
            assert!(result["id"].is_null());
        }
        for result in &results[8..10] {
            assert_eq!(
                result["result"]["_meta"]["figlens"]["code"],
                "invalid_arguments"
            );
        }
        assert_eq!(results[10]["result"]["isError"], false);
        assert!(results.iter().all(|result| result["id"].is_null()));
    }
}

// Run mutations between lines without globals, sleeps or permission-bit assumptions.
type Step = (String, Option<Box<dyn FnOnce() + Send>>);
/// Gives each request line, and runs its action, only after every earlier reply was written,
/// as a client waiting for each reply would.
struct Steps {
    pending: VecDeque<Step>,
    current: Vec<u8>,
    offset: usize,
    given: usize,
    seen: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

/// Collected output that counts the reply lines written so far.
struct Counted(Vec<u8>, std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl std::io::Write for Counted {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.extend_from_slice(bytes);
        let lines = bytes.iter().filter(|b| **b == b'\n').count();
        self.1.fetch_add(lines, std::sync::atomic::Ordering::SeqCst);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Read for Steps {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let bytes = self.fill_buf()?;
        let count = out.len().min(bytes.len());
        out[..count].copy_from_slice(&bytes[..count]);
        self.consume(count);
        Ok(count)
    }
}
impl BufRead for Steps {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.offset == self.current.len() {
            self.current.clear();
            self.offset = 0;
            if let Some((line, action)) = self.pending.pop_front() {
                let start = std::time::Instant::now();
                while self.seen.load(std::sync::atomic::Ordering::SeqCst) < self.given {
                    assert!(start.elapsed().as_secs() < 30, "a reply never arrived");
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                self.given += 1;
                if let Some(action) = action {
                    action();
                }
                self.current.extend_from_slice(line.as_bytes());
                self.current.push(b'\n');
            }
        }
        Ok(&self.current[self.offset..])
    }
    fn consume(&mut self, count: usize) {
        self.offset += count;
        assert!(self.offset <= self.current.len());
    }
}

#[test]
fn cache_carries_prepared_calls_once_and_recovers_from_source_changes() {
    let dir = TestDir::new();
    let file = fixture(&dir, "first.fig");
    let second = fixture(&dir, "second.fig");
    PREPARATIONS.with(|n| n.set(0));
    LOADS.with(|n| n.set(0));
    let request = |id, path: &Path| call(json!(id), "nodes", json!({"file":path,"node":"0:1"}));
    let changed = file.clone();
    let removed = file.clone();
    let restored = file.clone();
    let seen: std::sync::Arc<std::sync::atomic::AtomicUsize> = Default::default();
    let steps = Steps {
        pending: VecDeque::from([
            (request(1, &file), None),
            (json!({"id":2,"method":"ping"}).to_string(), None),
            (call(json!(3), "workflow", json!({"topic":"build"})), None),
            (
                call(json!(4), "tree", json!({"file":file,"depth":"0"})),
                None,
            ),
            (
                request(5, &file),
                Some(
                    Box::new(|| LOADS.with(|n| assert_eq!(n.get(), 1))) as Box<dyn FnOnce() + Send>
                ),
            ),
            (
                request(6, &file),
                Some(Box::new(move || {
                    // A readable, changed but invalid design must not reuse the old Tree.
                    std::fs::write(changed, b"not a design").unwrap();
                })),
            ),
            (
                request(7, &file),
                Some(Box::new(move || {
                    std::fs::remove_file(removed).unwrap();
                })),
            ),
            (
                request(8, &file),
                Some(Box::new(move || {
                    std::fs::copy("examples/first-success/design.fig", restored).unwrap();
                })),
            ),
            (request(9, &second), None),
            (request(10, &second), None),
            (json!({"id":11,"method":"ping"}).to_string(), None),
        ]),
        current: Vec::new(),
        offset: 0,
        given: 0,
        seen: seen.clone(),
    };
    let mut output = Counted(Vec::new(), seen);
    serve(steps, &mut output, &[]).unwrap();
    let results: Vec<Value> = String::from_utf8(output.0)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(results.len(), 11);
    for id in [1, 3, 5, 8, 9, 10] {
        assert_eq!(
            results[id - 1]["result"]["isError"],
            false,
            "id {id}: {}",
            results[id - 1]
        );
    }
    for id in [6, 7] {
        assert_eq!(
            results[id - 1]["result"]["_meta"]["figlens"]["code"],
            "source_unreadable"
        );
    }
    PREPARATIONS.with(|n| assert_eq!(n.get(), 9));
    LOADS.with(|n| assert_eq!(n.get(), 6));
}

#[test]
fn unrooted_paths_preserve_relative_labels_missing_parents_and_symlinks() {
    let dir = TestDir::new();
    let file = fixture(&dir, "design.fig");
    let output = dir.join("relative.png");
    let request = call(
        json!(1),
        "export",
        json!({"file":file,"node":"0:1","out":output}),
    );
    let result = rooted_reply(&request, &[]);
    assert_eq!(result["result"]["isError"], false, "{result}");
    assert_eq!(
        result["result"]["structuredContent"]["path"],
        output.to_string_lossy().as_ref()
    );
    assert!(output.exists());
    for tool in ["export", "bundle"] {
        let out = dir.join(if tool == "export" {
            "missing/new.png"
        } else {
            "missing/new.zip"
        });
        let result = rooted_reply(
            &call(json!(2), tool, json!({"file":file,"node":"0:1","out":out})),
            &[],
        );
        assert_eq!(
            result["result"]["_meta"]["figlens"]["code"],
            "output_write_failed"
        );
        assert!(!dir.join("missing").exists());
    }
    // Unknown fields fail preparation before root checks, even when named like another tool's paths.
    let result = rooted_reply(
        &call(
            json!(3),
            "find",
            json!({"file":file,"query":"First","out":"/not-authorized/ignored.png"}),
        ),
        &[dir.join("").canonicalize().unwrap()],
    );
    assert_eq!(
        result["result"]["_meta"]["figlens"]["code"],
        "invalid_arguments"
    );
    #[cfg(unix)]
    {
        let target = dir.join("target.png");
        let link = dir.join("link.png");
        std::os::unix::fs::symlink("target.png", &link).unwrap();
        let result = rooted_reply(
            &call(
                json!(4),
                "export",
                json!({"file":file,"node":"0:1","out":link}),
            ),
            &[],
        );
        assert_eq!(result["result"]["isError"], false, "{result}");
        assert!(target.exists());
        // Existing output aliases must not change the requested encoder or reply label.
        let target = dir.join("target.jpg");
        std::fs::write(&target, b"old output").unwrap();
        let link = dir.join("encoding.png");
        std::os::unix::fs::symlink("target.jpg", &link).unwrap();
        for roots in [vec![], vec![dir.join("").canonicalize().unwrap()]] {
            let result = rooted_reply(
                &call(
                    json!(6),
                    "export",
                    json!({"file":file,"node":"0:1","out":link}),
                ),
                &roots,
            );
            assert_eq!(result["result"]["isError"], false, "{result}");
            assert_eq!(
                result["result"]["structuredContent"]["path"],
                link.to_string_lossy().as_ref()
            );
            assert!(
                std::fs::read(&target)
                    .unwrap()
                    .starts_with(b"\x89PNG\r\n\x1a\n")
            );
        }
        let input = dir.join("input-link.fig");
        std::os::unix::fs::symlink("design.fig", &input).unwrap();
        let result = rooted_reply(
            &call(json!(5), "nodes", json!({"file":input,"node":"0:1"})),
            &[],
        );
        assert_eq!(result["result"]["isError"], false);
    }
}

#[test]
#[cfg(unix)]
fn admitted_paths_survive_alias_changes_and_recheck_output_parents() {
    let dir = TestDir::new();
    let outside = TestDir::new();
    let source = fixture(&dir, "design.fig");
    let alias = dir.join("alias.fig");
    std::os::unix::fs::symlink("design.fig", &alias).unwrap();
    let roots = [dir.join("").canonicalize().unwrap()];
    let request = call(json!(1), "nodes", json!({"file":alias,"node":"0:1"}));
    let Admission::Call(admitted) = admit(&request, &roots) else {
        panic!()
    };
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(outside.join("outside.fig"), &alias).unwrap();
    assert_eq!(
        admitted.execute(None, &roots, &[])["result"]["isError"],
        false
    );
    // Execution used the stored source, not the now-retargeted argument.
    assert_eq!(
        admitted.local(),
        Some(source.canonicalize().unwrap().as_path())
    );
    assert_eq!(
        rooted_reply(&request, &roots)["result"]["_meta"]["figlens"]["code"],
        "permission_denied"
    );

    std::fs::create_dir(dir.join("outputs")).unwrap();
    let out = dir.join("outputs/new.png");
    let Admission::Call(admitted) = admit(
        &call(
            json!(2),
            "export",
            json!({"file":source,"node":"0:1","out":out}),
        ),
        &roots,
    ) else {
        panic!()
    };
    std::fs::remove_dir(dir.join("outputs")).unwrap();
    std::os::unix::fs::symlink(
        outside.join("").canonicalize().unwrap(),
        dir.join("outputs"),
    )
    .unwrap();
    assert_eq!(
        admitted.execute(None, &roots, &[])["result"]["_meta"]["figlens"]["code"],
        "permission_denied"
    );
    assert!(!outside.join("new.png").exists());
}

#[test]
#[cfg(unix)]
fn non_utf8_output_targets_are_not_reinterpreted_as_lossy_paths() {
    use std::os::unix::ffi::OsStringExt;

    let dir = TestDir::new();
    let file = fixture(&dir, "design.fig");
    let doc = Document::open(&file).unwrap();
    let message = doc.message().unwrap();
    let tree = Tree::new(&message);
    for (tool, extension) in [("export", "png"), ("bundle", "zip")] {
        let mut name = b"target-\xff.".to_vec();
        name.extend_from_slice(extension.as_bytes());
        let target = dir.join("").join(std::ffi::OsString::from_vec(name));
        let prepared = contract::prepare(
            tool,
            &json!({"file":file,"node":"0:1","out":format!("alias.{extension}")}),
        )
        .unwrap();
        // Assert the handoff without requiring the host filesystem to store non-UTF-8 names.
        let command = command::Command::from_tool(&prepared, &tree, Some(&target), None).unwrap();
        let out = match command {
            command::Command::Bundle { out, .. } | command::Command::Export { out, .. } => out,
            _ => panic!("expected output command"),
        };
        assert_eq!(out, target);
        assert_ne!(out, PathBuf::from(target.to_string_lossy().as_ref()));
    }
}

#[test]
fn export_match_dir_stays_inside_roots_and_flat_inspect_is_structured() {
    let inside = TestDir::new();
    let outside = TestDir::new();
    let file = fixture(&inside, "design.fig");
    let roots = [inside.join("").canonicalize().unwrap()];
    let export = |id, dir: PathBuf| {
        let args = json!({"file":file,"node":"0:2","out":inside.join("card.svg"),"matchDir":dir});
        rooted_reply(&call(json!(id), "export", args), &roots)
    };
    assert_eq!(
        export(1, outside.join(""))["result"]["_meta"]["figlens"]["code"],
        "permission_denied"
    );
    let matches =
        |id| export(id, inside.join(""))["result"]["structuredContent"]["matches"].clone();
    // The export itself is in the folder but is not reported.
    assert_eq!(matches(2), json!([]));
    std::fs::copy(inside.join("card.svg"), inside.join("copy.svg")).unwrap();
    assert_eq!(matches(3), json!(["copy.svg"]));
    let flat = rooted_reply(
        &call(
            json!(4),
            "inspect",
            json!({"file":file,"node":"0:1","flat":true}),
        ),
        &roots,
    );
    let result = &flat["result"];
    assert_eq!(
        result["structuredContent"]["flat"],
        result["content"][0]["text"]
    );
}

#[test]
fn verify_reads_only_bundles_inside_roots_and_keeps_the_caller_label() {
    let inside = TestDir::new();
    let outside = TestDir::new();
    for dir in [&inside, &outside] {
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"format":"figlens-bundle","version":1,"files":[]}"#,
        )
        .unwrap();
    }
    let roots = [inside.join("").canonicalize().unwrap()];
    let denied = rooted_reply(
        &call(json!(1), "verify", json!({"bundle": outside.join("")})),
        &roots,
    );
    assert_eq!(
        denied["result"]["_meta"]["figlens"]["code"],
        "permission_denied"
    );
    let label = inside.join("");
    let report = rooted_reply(&call(json!(2), "verify", json!({"bundle": label})), &roots);
    let content = &report["result"]["structuredContent"];
    assert_eq!(
        (
            content["path"].as_str(),
            content["findings"].is_array(),
            &content["findingsSummary"]["unexamined"]
        ),
        (
            Some(label.to_string_lossy().as_ref()),
            true,
            &json!([
                "missing-assets.json",
                "design/nodes.json",
                "design/variables.json"
            ])
        )
    );
}

#[test]
fn verify_of_quote_heavy_evidence_fits_the_whole_reply_budget() {
    let dir = TestDir::new();
    let quotes = "\"".repeat(64 << 10);
    let assets: Vec<_> = (0..20)
        .map(|n| json!({"key": quotes, "nodeId": format!("1:{n}")}))
        .collect();
    let diagnostics: Vec<_> = (0..20)
        .map(|n| json!({"nodeId": format!("2:{n}"), "reason": quotes}))
        .collect();
    let nodes = json!({"document": {"id": "1:1", "_figlens": {"renderDiagnostics": diagnostics}}});
    std::fs::create_dir(dir.join("design")).unwrap();
    for (path, value) in [
        (
            "manifest.json",
            json!({"format": "figlens-bundle", "version": 1, "files": []}),
        ),
        ("missing-assets.json", json!(assets)),
        ("design/nodes.json", nodes),
    ] {
        std::fs::write(dir.join(path), value.to_string()).unwrap();
    }
    let roots = [dir.join("").canonicalize().unwrap()];
    let reply = bounded_response(rooted_reply(
        &call(json!(1), "verify", json!({"bundle": dir.join("")})),
        &roots,
    ));
    let content = &reply["result"]["structuredContent"];
    assert_eq!(
        (
            &reply["result"]["isError"],
            &content["findingsSummary"]["truncated"],
            &content["truncated"]["missingAssets"]
        ),
        (&json!(false), &json!(true), &json!(20))
    );
}

#[test]
fn export_batch_checks_every_path_and_the_cap_before_writing_and_reports_items() {
    let inside = TestDir::new();
    let outside = TestDir::new();
    let file = fixture(&inside, "design.fig");
    let roots = [inside.join("").canonicalize().unwrap()];
    let batch = |id, items: Value| {
        rooted_reply(
            &call(
                json!(id),
                "export_batch",
                json!({"file":file,"items":items}),
            ),
            &roots,
        )["result"]
            .clone()
    };
    let (a, b) = (inside.join("a.png"), inside.join("b.svg"));
    let stray = outside.join("c.png");
    let denied = batch(
        1,
        json!([{"node":"0:1","out":a},{"node":"0:2","out":stray}]),
    );
    assert_eq!(denied["_meta"]["figlens"]["code"], "permission_denied");
    assert!(!a.exists() && !stray.exists());
    let items: Vec<_> = (0..65)
        .map(|i| json!({"node":"0:1","out":inside.join(&format!("{i}.png"))}))
        .collect();
    let capped = batch(2, json!(items));
    assert_eq!(capped["_meta"]["figlens"]["code"], "invalid_arguments");
    assert!(!inside.join("0.png").exists());
    let twice = batch(
        3,
        json!([{"node":"0:1","out":a},{"node":"0:2","out":inside.join("./a.png")}]),
    );
    assert_eq!(twice["isError"], true, "{twice}");
    assert!(!a.exists());
    let reply = batch(
        4,
        json!([{"node":"0:1","out":a},{"node":"9:9","out":inside.join("x.png")},{"node":"0:2","out":b,"hide":"0:3"},{"node":"0:4","out":inside.join("t.png")}]),
    );
    assert_eq!(reply["isError"], false, "{reply}");
    let manifest = &reply["structuredContent"];
    assert_eq!(manifest["failed"], 1);
    assert_eq!(manifest["items"][0]["path"], a.to_string_lossy().as_ref());
    assert_eq!(
        manifest["items"][0]["bytes"],
        std::fs::metadata(&a).unwrap().len()
    );
    assert!(manifest["items"][1]["error"].is_string());
    assert!(!inside.join("x.png").exists() && b.exists());
    assert!(manifest["items"][2]["hidden"].is_array(), "{manifest}");
    // READY's text is narrower than its layout box: export's warning stays in the manifest.
    assert!(
        manifest["items"][3]["warnings"][0]
            .as_str()
            .unwrap()
            .contains("differs from the layout box")
    );
}

#[test]
fn compare_image_full_page_admits_a_short_screenshot() {
    let dir = TestDir::new();
    let file = fixture(&dir, "design.fig");
    let roots = [dir.join("").canonicalize().unwrap()];
    let shot = dir.join("short.png");
    let blank = resvg::tiny_skia::Pixmap::new(320, 100).unwrap();
    std::fs::write(&shot, figlens::render::encode_png(&blank).unwrap()).unwrap();
    let reply = |id, full: bool| {
        let args = json!({"file":file,"node":"0:1","image":shot,"fullPage":full});
        rooted_reply(&call(json!(id), "compare_image", args), &roots)["result"].clone()
    };
    assert_eq!(
        reply(1, false)["_meta"]["figlens"]["code"],
        "invalid_arguments"
    );
    assert_eq!(
        reply(2, true)["structuredContent"]["uncompared"]["missing"],
        80
    );
}

#[test]
fn compare_image_reads_and_writes_only_inside_the_roots() {
    let inside = TestDir::new();
    let outside = TestDir::new();
    let file = fixture(&inside, "design.fig");
    let roots = [inside.join("").canonicalize().unwrap()];
    let reply = |id, tool, args: Value| {
        rooted_reply(&call(json!(id), tool, args), &roots)["result"].clone()
    };
    let shot = inside.join("shot.png");
    let args = json!({"file":file,"node":"0:1","out":shot,"bounds":"layout"});
    assert_eq!(reply(1, "export", args)["isError"], false);
    std::fs::copy(&shot, outside.join("shot.png")).unwrap();
    let args = json!({"file":file,"node":"0:1","image":outside.join("shot.png")});
    let stray = reply(2, "compare_image", args);
    assert_eq!(stray["_meta"]["figlens"]["code"], "permission_denied");
    let args = json!({"file":file,"node":"0:1","image":shot,"out":shot});
    let over = reply(3, "compare_image", args);
    assert_eq!(
        over["_meta"]["figlens"]["code"], "invalid_arguments",
        "{over}"
    );
    let diff = inside.join("diff.png");
    let args = json!({"file":file,"node":"0:1","image":shot,"out":diff});
    let same = reply(4, "compare_image", args);
    assert_eq!(same["isError"], false, "{same}");
    assert_eq!(same["structuredContent"]["mismatch"], 0.0);
    assert_eq!(
        same["structuredContent"]["path"],
        diff.to_string_lossy().as_ref()
    );
    assert!(diff.exists());
}

#[test]
fn verify_fonts_outside_the_roots_is_refused() {
    let dir = TestDir::new();
    let outside = TestDir::new();
    let roots = [dir.join("").canonicalize().unwrap()];
    let request = call(
        json!(1),
        "verify",
        json!({"bundle":dir.join("b.zip"),"fonts":outside.join("").canonicalize().unwrap()}),
    );
    assert_eq!(
        rooted_reply(&request, &roots)["result"]["_meta"]["figlens"]["code"],
        "permission_denied"
    );
}
