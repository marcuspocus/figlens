//! `freshness` and the stale hint through the real CLI and MCP server, with a `curl` stand-in
//! on PATH serving Figma's file metadata; no network.
#![cfg(unix)]

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_figlens");
const SAVED: &str = "2026-10-21T18:27:49.965Z";
const LATER: &str = "2026-10-21T18:40:00Z";

/// A project with `.figlens/sources.json` mapping `home` to `figma/home.fig`, a copy of the public
/// fixture saved at `SAVED`, and a `curl` that answers the online file as touched at `online`.
struct Project {
    root: PathBuf,
}

/// Each project's own directory: a counter, never a name another test just freed.
static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Project {
    fn new(online: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(format!("samples/freshness-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("figma")).unwrap();
        std::fs::create_dir_all(root.join(".figlens")).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(
            root.join(".figlens/sources.json"),
            json!([{"name": "home", "url": "https://www.figma.com/design/AbC123/Home", "file": "figma/home.fig"}])
                .to_string(),
        )
        .unwrap();
        std::fs::write(root.join("figma/home.fig"), saved_copy()).unwrap();
        std::fs::write(
            root.join("bin/body"),
            json!({"file": {"last_touched_at": online, "version": "42"}}).to_string(),
        )
        .unwrap();
        let curl = root.join("bin/curl");
        std::fs::write(
            &curl,
            "#!/bin/sh\nd=$(dirname \"$0\")\necho \"$*\" >> \"$d/calls\"\ncat > /dev/null\n\
             cat \"$d/body\"\nprintf '\\n200'\n",
        )
        .unwrap();
        std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { root }
    }

    fn fig(&self) -> PathBuf {
        self.root.join("figma/home.fig")
    }

    fn command(&self, args: &[&str], token: bool) -> Command {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let dirs = std::iter::once(self.root.join("bin")).chain(std::env::split_paths(&path));
        let mut command = Command::new(BIN);
        command
            .args(args)
            .current_dir(&self.root)
            .env("PATH", std::env::join_paths(dirs).unwrap())
            .env_remove("FIGMA_TOKEN");
        if token {
            command.env("FIGMA_TOKEN", "test-token");
        }
        command
    }

    fn run(&self, args: &[&str], token: bool) -> Output {
        self.command(args, token).output().unwrap()
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.root.join("bin/calls")).unwrap_or_default()
    }

    /// The reply to one `tools/call` from a fresh server.
    fn mcp(&self, name: &str, arguments: Value) -> Value {
        self.mcp_all(&[(name, arguments)]).remove(0)
    }

    /// The replies, in call order, to several `tools/call`s sent to one server.
    fn mcp_all(&self, calls: &[(&str, Value)]) -> Vec<Value> {
        let root = self.root.to_str().unwrap();
        let mut child = self
            .command(&["mcp", "--root", root], true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        for (id, (name, arguments)) in calls.iter().enumerate() {
            let call = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": {"name": name, "arguments": arguments}});
            writeln!(stdin, "{call}").unwrap();
        }
        // Closing stdin stops the server and cancels running calls, so read every reply first.
        let mut lines = std::io::BufReader::new(child.stdout.take().unwrap()).lines();
        let mut replies: Vec<Value> = (0..calls.len())
            .map(|_| serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap())
            .collect();
        drop(stdin);
        child.wait().unwrap();
        replies.sort_by_key(|reply| reply["id"].as_u64());
        replies
    }
}

/// The public fixture plus a `meta.json` saved at `SAVED`, as Figma's Save local copy writes.
fn saved_copy() -> Vec<u8> {
    let mut source =
        zip::ZipArchive::new(std::fs::File::open("examples/first-success/design.fig").unwrap())
            .unwrap();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..source.len() {
        zip.raw_copy_file(source.by_index_raw(i).unwrap()).unwrap();
    }
    zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(json!({"exported_at": SAVED}).to_string().as_bytes())
        .unwrap();
    zip.finish().unwrap().into_inner()
}

fn json(out: &Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn freshness_lists_the_nearest_mapping_as_stale() {
    let project = Project::new(LATER);
    let report = json(&project.run(&["freshness"], true));
    assert_eq!(report["designs"][0]["state"], "stale", "{report}");
}

#[test]
fn freshness_without_a_token_is_unknown_and_calls_nothing() {
    let project = Project::new(LATER);
    let report = json(&project.run(&["freshness"], false));
    assert_eq!(
        (report["designs"][0]["reason"].clone(), project.calls()),
        (json!("no_token"), String::new()),
        "{report}"
    );
}

#[test]
fn a_missing_node_in_a_stale_copy_says_to_save_a_new_one() {
    let project = Project::new(LATER);
    let out = project.run(&["inspect", project.fig().to_str().unwrap(), "99:99"], true);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("save a new local copy"), "{stderr}");
}

#[test]
fn a_missing_node_in_a_current_copy_keeps_the_plain_error() {
    let project = Project::new("2026-10-21T18:00:00Z");
    let out = project.run(&["inspect", project.fig().to_str().unwrap(), "99:99"], true);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not found") && !stderr.contains("save a new"),
        "{stderr}"
    );
}

#[test]
fn a_missing_node_without_a_token_calls_nothing() {
    let project = Project::new(LATER);
    project.run(
        &["inspect", project.fig().to_str().unwrap(), "99:99"],
        false,
    );
    assert_eq!(project.calls(), "");
}

#[test]
fn mcp_find_with_no_results_in_a_stale_copy_carries_the_hint() {
    let project = Project::new(LATER);
    let file = project.fig();
    let reply = project.mcp("find", json!({"file": file, "query": "no such layer"}));
    let hint = &reply["result"]["structuredContent"]["freshness"]["hint"];
    assert!(
        hint.as_str().is_some_and(|h| h.contains("Save local copy")),
        "{reply}"
    );
}

#[test]
fn mcp_freshness_lists_a_mapping_inside_the_roots() {
    let project = Project::new(LATER);
    let sources = project.root.join(".figlens/sources.json");
    let reply = project.mcp("freshness", json!({"sources": sources}));
    let designs = &reply["result"]["structuredContent"]["designs"];
    assert_eq!(designs[0]["state"], "stale", "{reply}");
}

#[test]
fn mcp_freshness_refuses_a_mapping_outside_the_roots() {
    let project = Project::new(LATER);
    let outside = Path::new("/etc/hosts");
    let reply = project.mcp("freshness", json!({"sources": outside}));
    assert_eq!(reply["result"]["isError"], true, "{reply}");
}

#[test]
fn the_token_never_reaches_curl_arguments_or_output() {
    let project = Project::new(LATER);
    let out = project.run(&["freshness"], true);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(
        !project.calls().contains("test-token") && !printed.contains("test-token"),
        "{printed}"
    );
}

#[test]
fn cli_find_with_no_results_in_a_stale_copy_carries_the_hint() {
    let project = Project::new(LATER);
    let fig = project.fig();
    let found = json(&project.run(&["find", fig.to_str().unwrap(), "no such layer"], true));
    assert_eq!(found["freshness"]["state"], "stale", "{found}");
}

#[test]
fn a_bare_relative_mapping_resolves_files_from_the_folder_above_it() {
    let project = Project::new(LATER);
    let out = project
        .command(&["freshness", "--sources", "sources.json"], true)
        .current_dir(project.root.join(".figlens"))
        .output()
        .unwrap();
    assert_eq!(json(&out)["designs"][0]["state"], "stale");
}

#[test]
fn a_missing_file_is_file_missing_not_unmapped() {
    let project = Project::new(LATER);
    let report = json(&project.run(&["freshness", "figma/gone.fig"], true));
    assert_eq!(report["designs"][0]["reason"], "file_missing", "{report}");
}

#[test]
fn one_server_reuses_a_stale_answer_for_repeated_misses() {
    let project = Project::new(LATER);
    let file = project.fig();
    let find = || ("find", json!({"file": file, "query": "no such layer"}));
    let replies = project.mcp_all(&[find(), find()]);
    assert_eq!(
        project.calls().matches("api.figma.com").count(),
        1,
        "{replies:?}"
    );
}

#[test]
fn a_bad_freshness_flag_prints_only_its_usage() {
    let project = Project::new(LATER);
    let out = project.run(&["freshness", "--bogus"], false);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success()
            && stderr.contains("figlens freshness")
            && !stderr.contains("figlens tree"),
        "{stderr}"
    );
}

#[test]
fn mcp_inspect_of_a_missing_node_in_a_stale_copy_carries_the_hint() {
    let project = Project::new(LATER);
    let file = project.fig();
    let reply = project.mcp("inspect", json!({"file": file, "node": "99:99"}));
    let text = reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(text.contains("save a new local copy"), "{reply}");
}

#[test]
fn an_unmapped_file_reports_every_field() {
    let project = Project::new(LATER);
    let other = project.root.join("other.fig");
    std::fs::copy(project.fig(), &other).unwrap();
    let report = json(&project.run(&["freshness", "other.fig"], true));
    let design = &report["designs"][0];
    assert_eq!(
        (design["reason"].clone(), design.get("savedAt").cloned()),
        (json!("not_mapped"), Some(Value::Null)),
        "{report}"
    );
}
