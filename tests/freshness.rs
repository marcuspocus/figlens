//! `freshness`, the stale hint and `pull` through the real CLI and MCP server, with a `curl`
//! stand-in on PATH serving Figma's file metadata and a pull helper stand-in; no network.
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

    fn accounts(&self, value: Value) {
        let dir = self.root.join("config/figlens");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("accounts.json"), value.to_string()).unwrap();
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
            .env_remove("FIGMA_TOKEN")
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env_remove("FIGMA_TOKEN_WORK")
            .env_remove("FIGMA_TOKEN_CLIENT")
            .env("FIGLENS_PULL_HELPER", self.root.join("bin/helper"));
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
    saved_copy_at(SAVED)
}

/// The public fixture saved at `at`.
fn saved_copy_at(at: &str) -> Vec<u8> {
    let mut source =
        zip::ZipArchive::new(std::fs::File::open("examples/first-success/design.fig").unwrap())
            .unwrap();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..source.len() {
        zip.raw_copy_file(source.by_index_raw(i).unwrap()).unwrap();
    }
    zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(json!({"exported_at": at}).to_string().as_bytes())
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
fn sources_init_scaffolds_names_paths_and_blank_urls_without_network() {
    let project = Project::new(LATER);
    std::fs::remove_file(project.root.join(".figlens/sources.json")).unwrap();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"{"file_name":"Design name"}"#).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    std::fs::write(project.root.join("figma/a.fig"), &bytes).unwrap();
    std::fs::write(project.root.join("figma/b.fig"), &bytes).unwrap();
    std::fs::create_dir(project.root.join("node_modules")).unwrap();
    std::fs::write(project.root.join("node_modules/ignored.fig"), &bytes).unwrap();
    std::os::unix::fs::symlink(project.fig(), project.root.join("link.fig")).unwrap();
    let report = json(&project.run(&["sources", "init"], false));
    assert_eq!(
        report["designs"],
        json!([
            {"name":"Design name", "url":"", "file":"figma/a.fig"},
            {"name":"Design name-2", "url":"", "file":"figma/b.fig"},
            {"name":"home", "url":"", "file":"figma/home.fig"}
        ])
    );
    let mapping = std::fs::read(project.root.join(".figlens/sources.json")).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&mapping).unwrap(),
        report["designs"]
    );
    assert_eq!(
        json(&project.run(&["freshness"], false))["designs"][2]["reason"],
        "url_unrecognized"
    );
    let pull = project.run(&["pull", "--only", "home"], false);
    assert!(!pull.status.success());
    assert!(String::from_utf8_lossy(&pull.stderr).contains("url"));
    assert!(!project.run(&["sources", "init"], false).status.success());
    assert_eq!(
        std::fs::read(project.root.join(".figlens/sources.json")).unwrap(),
        mapping
    );
    assert_eq!(project.calls(), "");
}

#[test]
fn sources_init_refuses_empty_oversized_and_symlinked_output() {
    let project = Project::new(LATER);
    let root = project.root.join("empty");
    std::fs::create_dir(&root).unwrap();
    let dir = root.to_str().unwrap();
    assert!(
        !project
            .run(&["sources", "init", dir], false)
            .status
            .success()
    );
    assert!(!root.join(".figlens").exists());
    for i in 0..201 {
        std::fs::write(root.join(format!("{i}.fig")), b"not a zip").unwrap();
    }
    let out = project.run(&["sources", "init", dir], false);
    assert!(String::from_utf8_lossy(&out.stderr).contains("200 designs"));
    assert!(!root.join(".figlens").exists());
    std::fs::remove_file(root.join("200.fig")).unwrap();
    std::os::unix::fs::symlink(project.root.join("figma"), root.join(".figlens")).unwrap();
    let out = project.run(&["sources", "init", dir], false);
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a symlink"));
    assert!(!project.root.join("figma/sources.json").exists());
}

#[test]
fn mapping_typos_fail_in_cli_mcp_and_pull_with_entry_and_key() {
    let project = Project::new(LATER);
    std::fs::write(
        project.root.join(".figlens/sources.json"),
        r#"[{"name":"home","url":"u","file":"figma/home.fig","accout":"work"}]"#,
    )
    .unwrap();
    for args in [vec!["freshness"], vec!["pull", "--only", "home"]] {
        let out = project.run(&args, false);
        assert!(!out.status.success());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(
            error.contains("entry 0")
                && error.contains("accout")
                && error.contains("allowed fields"),
            "{error}"
        );
    }
    let reply = project.mcp(
        "freshness",
        json!({"sources":project.root.join(".figlens/sources.json")}),
    );
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    assert!(reply.to_string().contains("accout"), "{reply}");
    assert_eq!(project.calls(), "");
}

#[test]
fn pull_listing_explains_independent_api_setup_and_token_free_only() {
    let project = Project::new(LATER);
    std::fs::write(project.root.join(".figlens/sources.json"),
        json!([{"name":"home","url":"https://www.figma.com/design/AbC123/","file":"figma/home.fig","account":"work"}]).to_string()).unwrap();
    let report = json(&project.run(&["pull"], false));
    assert_eq!(report["designs"][0]["reason"], "account_not_configured");
    let hint = report["hint"].as_str().unwrap();
    assert!(
        hint.contains("--only NAME")
            && hint.contains("needs no API token")
            && hint.contains("accounts.json")
            && hint.contains("tokenEnv")
    );
    assert_eq!(project.calls(), "");
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

impl Project {
    /// A pull helper that answers each run with `download` (written into its download folder),
    /// or with a path outside that folder when `outside`, and logs its request to `bin/request`.
    fn helper(&self, download: &[u8], outside: bool) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(self.root.join("bin/download"), download).unwrap();
        let target = if outside {
            "$d/download".to_owned()
        } else {
            "$dl/0-design.fig".to_owned()
        };
        let script = format!(
            "#!/bin/sh\nd=$(dirname \"$0\")\nIFS= read -r req\nprintf '%s' \"$req\" > \"$d/request\"\n\
             dl=$(printf '%s' \"$req\" | sed 's/.*\"downloads\":\"\\([^\"]*\\)\".*/\\1/')\n\
             cp \"$d/download\" \"$dl/0-design.fig\"\n\
             printf '{{\"name\":\"home\",\"path\":\"%s\"}}\\n' \"{target}\"\n"
        );
        let helper = self.root.join("bin/helper");
        std::fs::write(&helper, script).unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn map(&self, sources: Value) {
        std::fs::write(self.root.join(".figlens/sources.json"), sources.to_string()).unwrap();
    }

    fn saved(&self) -> Vec<u8> {
        std::fs::read(self.fig()).unwrap()
    }
}

#[test]
fn pull_replaces_the_copy_with_a_newer_save() {
    let project = Project::new(LATER);
    let newer = saved_copy_at(LATER);
    project.helper(&newer, false);
    let report = json(&project.run(&["pull", "--only", "home"], false));
    assert_eq!(
        (
            report["designs"][0]["state"].clone(),
            project.saved() == newer
        ),
        (json!("pulled"), true),
        "{report}"
    );
}

#[test]
fn pull_keeps_the_copy_when_the_download_is_older() {
    let project = Project::new(LATER);
    let before = project.saved();
    project.helper(&saved_copy_at("2026-01-01T00:00:00Z"), false);
    let out = project.run(&["pull", "--only", "home"], false);
    assert_eq!(
        (out.status.success(), project.saved() == before),
        (false, true)
    );
}

#[test]
fn pull_keeps_the_copy_when_the_download_is_not_a_fig() {
    let project = Project::new(LATER);
    let before = project.saved();
    project.helper(b"<html>Sign in</html>", false);
    let out = project.run(&["pull", "--only", "home"], false);
    assert_eq!(
        (out.status.success(), project.saved() == before),
        (false, true)
    );
}

#[test]
fn pull_refuses_a_download_outside_its_folder() {
    let project = Project::new(LATER);
    let before = project.saved();
    project.helper(&saved_copy_at(LATER), true);
    let out = project.run(&["pull", "--only", "home"], false);
    assert_eq!(
        (out.status.success(), project.saved() == before),
        (false, true)
    );
}

#[test]
fn pull_stale_skips_a_current_copy_without_the_browser() {
    let project = Project::new(SAVED);
    project.helper(&saved_copy_at(LATER), false);
    let report = json(&project.run(&["pull", "--stale"], true));
    assert_eq!(
        (
            report["designs"][0]["reason"].clone(),
            project.root.join("bin/request").exists()
        ),
        (json!("current"), false),
        "{report}"
    );
}

#[test]
fn pull_sends_the_helper_a_key_only_url() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    json(&project.run(&["pull", "--only", "home"], false));
    let request = std::fs::read_to_string(project.root.join("bin/request")).unwrap();
    assert!(
        request.contains("https://www.figma.com/design/AbC123/\""),
        "{request}"
    );
}

#[test]
fn pull_of_an_unknown_name_fails_before_the_browser() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    let out = project.run(&["pull", "--only", "nope"], false);
    assert_eq!(
        (
            out.status.success(),
            project.root.join("bin/request").exists()
        ),
        (false, false)
    );
}

#[test]
fn pull_without_a_selection_lists_freshness_and_downloads_nothing() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    let report = json(&project.run(&["pull"], true));
    assert_eq!(
        (
            report["designs"][0]["state"].clone(),
            project.root.join("bin/request").exists()
        ),
        (json!("stale"), false),
        "{report}"
    );
}

#[test]
fn pull_refuses_a_mapped_file_outside_the_project() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    project.map(json!([{"name": "home", "url": "https://www.figma.com/design/AbC123/Home", "file": "../home.fig"}]));
    let out = project.run(&["pull", "--only", "home"], false);
    assert_eq!(
        (
            out.status.success(),
            project.root.join("bin/request").exists()
        ),
        (false, false)
    );
}

#[test]
fn pull_refuses_a_name_mapped_twice() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    let entry = json!({"name": "home", "url": "https://www.figma.com/design/AbC123/Home", "file": "figma/home.fig"});
    project.map(json!([entry, {"name": "home", "url": "https://www.figma.com/design/XyZ789/Other", "file": "figma/other.fig"}]));
    let out = project.run(&["pull", "--only", "home"], false);
    assert_eq!(
        (
            out.status.success(),
            project.root.join("bin/request").exists()
        ),
        (false, false)
    );
}

#[test]
fn pull_ignores_a_bad_url_on_a_design_it_does_not_pull() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    let entry = json!({"name": "home", "url": "https://www.figma.com/design/AbC123/Home", "file": "figma/home.fig"});
    project.map(json!([entry, {"name": "notes", "url": "https://example.com/notes", "file": "figma/notes.fig"}]));
    let report = json(&project.run(&["pull", "--only", "home"], false));
    assert_eq!(report["designs"][0]["state"], "pulled", "{report}");
}

#[test]
fn pull_login_needs_no_mapping() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    std::fs::remove_file(project.root.join(".figlens/sources.json")).unwrap();
    project.run(&["pull", "--login"], false);
    let request = std::fs::read_to_string(project.root.join("bin/request")).unwrap();
    assert!(request.contains("\"login\":true"), "{request}");
}

impl Project {
    /// Respond to every selected design, keeping a log of account-group requests.
    fn grouped_helper(&self) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(self.root.join("bin/download"), saved_copy_at(LATER)).unwrap();
        let helper = self.root.join("bin/helper");
        std::fs::write(
            &helper,
            r#"#!/bin/sh
d=$(dirname "$0")
IFS= read -r req
printf '%s\n' "$req" >> "$d/requests"
dl=$(printf '%s' "$req" | sed 's/.*"downloads":"\([^"]*\)".*/\1/')
cp "$d/download" "$dl/group.fig"
printf '%s' "$req" | grep -o '"name":"[^"]*"' | cut -d '"' -f 4 | while IFS= read -r name; do
  printf '{"name":"%s","path":"%s/group.fig"}\n' "$name" "$dl"
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn requests(&self) -> Vec<Value> {
        std::fs::read_to_string(self.root.join("bin/requests"))
            .unwrap_or_default()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }

    /// The request the stand-in helper received on its last run.
    fn request(&self) -> Value {
        let request = std::fs::read_to_string(self.root.join("bin/request")).unwrap();
        serde_json::from_str(&request).unwrap()
    }
}

#[test]
fn pull_sends_the_chosen_browser_and_its_own_profile() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    project.run(&["pull", "--login", "--browser", "agent-browser"], false);
    let request = project.request();
    let profile = request["profiles"]["agent-browser"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(
        (
            request["browser"].clone(),
            profile.ends_with("pull/agent-browser-profile")
        ),
        (json!("agent-browser"), true),
        "{request}"
    );
}

#[test]
fn pull_refuses_an_unknown_browser() {
    let project = Project::new(LATER);
    let out = project.run(&["pull", "--login", "--browser", "firefox"], false);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("playwright or agent-browser"),
        "{stderr}"
    );
}

#[test]
fn pull_logout_deletes_every_browser_profile() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    project.run(&["pull", "--login"], false);
    let profiles = project.request()["profiles"].clone();
    let out = project.run(&["pull", "--logout"], false);
    let left: Vec<_> = ["playwright", "agent-browser"]
        .iter()
        .filter(|b| Path::new(profiles[**b].as_str().unwrap()).exists())
        .collect();
    assert_eq!(
        (out.status.success(), left.len()),
        (true, 0),
        "{profiles} {left:?}"
    );
}

#[test]
fn pull_logout_runs_alone() {
    let project = Project::new(LATER);
    let out = project.run(&["pull", "--logout", "--only", "home"], false);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("--logout only"),
        "{stderr}"
    );
}

#[test]
fn pull_logout_refuses_a_mapping() {
    let project = Project::new(LATER);
    let out = project.run(&["pull", "--logout", "--sources", "x.json"], false);
    assert!(!out.status.success(), "{out:?}");
}

#[test]
fn pull_reports_why_the_helper_stopped() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new(LATER);
    let helper = project.root.join("bin/helper");
    std::fs::write(
        &helper,
        "#!/bin/sh\necho 'pull needs playwright-core' >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = project.run(&["pull", "--only", "home"], false);
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        report["designs"][0]["reason"],
        "the helper stopped before this design: pull needs playwright-core",
        "{report}"
    );
}

#[test]
fn pull_named_login_is_isolated_from_legacy_profiles_and_needs_no_mapping() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    let legacy_report = json(&project.run(&["pull", "--login"], false));
    assert!(legacy_report.get("account").is_none());
    let legacy = project.request();
    for browser in ["playwright", "agent-browser"] {
        let profile = Path::new(legacy["profiles"][browser].as_str().unwrap());
        std::fs::write(profile.join("cookies"), "keep me").unwrap();
    }
    std::fs::remove_file(project.root.join(".figlens/sources.json")).unwrap();
    for browser in ["playwright", "agent-browser"] {
        let report = json(&project.run(
            &["pull", "--login", "--account", "work", "--browser", browser],
            false,
        ));
        assert_eq!(report["account"], "work");
        let named = project.request();
        assert!(
            named["profiles"][browser]
                .as_str()
                .unwrap()
                .contains("/accounts/account-work/")
        );
        assert_ne!(named["profiles"], legacy["profiles"]);
    }
    json(&project.run(&["pull", "--login"], false));
    assert_eq!(project.request()["profiles"], legacy["profiles"]);
    for browser in ["playwright", "agent-browser"] {
        let profile = Path::new(legacy["profiles"][browser].as_str().unwrap());
        assert_eq!(
            std::fs::read_to_string(profile.join("cookies")).unwrap(),
            "keep me"
        );
    }
}

#[test]
fn pull_groups_mapped_accounts_and_cli_account_overrides_them() {
    let project = Project::new(LATER);
    project.grouped_helper();
    project.map(json!([
        {"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"},
        {"name":"other", "url":"https://www.figma.com/design/XyZ789/", "file":"figma/other.fig", "account":"client"},
        {"name":"third", "url":"https://www.figma.com/design/AbC456/", "file":"figma/third.fig", "account":"work"},
        {"name":"legacy", "url":"https://www.figma.com/design/AbC789/", "file":"figma/legacy.fig"}
    ]));
    let args = [
        "pull", "--only", "home", "--only", "other", "--only", "third", "--only", "legacy",
    ];
    let report = json(&project.run(&args, false));
    let requests = project.requests();
    assert_eq!(requests.len(), 3, "{requests:?}");
    assert_eq!(requests[0]["designs"].as_array().unwrap().len(), 2);
    assert_eq!(requests[0]["designs"][1]["name"], "third");
    assert!(
        requests[0]["profiles"]["playwright"]
            .as_str()
            .unwrap()
            .contains("account-work/")
    );
    assert!(
        requests[1]["profiles"]["playwright"]
            .as_str()
            .unwrap()
            .contains("account-client/")
    );
    assert!(
        requests[2]["profiles"]["playwright"]
            .as_str()
            .unwrap()
            .ends_with("pull/profile")
    );
    for (index, name) in ["home", "other", "third", "legacy"].iter().enumerate() {
        assert_eq!(report["designs"][index]["name"], *name);
        assert_eq!(report["designs"][index]["state"], "pulled");
    }
    assert_eq!(report["designs"][0]["account"], "work");
    assert!(report["designs"][3].get("account").is_none());
    let mut override_args = args.to_vec();
    override_args.extend(["--account", "override"]);
    let report = json(&project.run(&override_args, false));
    let requests = project.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[3]["designs"].as_array().unwrap().len(), 4);
    assert!(
        requests[3]["profiles"]["playwright"]
            .as_str()
            .unwrap()
            .contains("account-override/")
    );
    assert!(
        report["designs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["account"] == "override")
    );
}

#[test]
fn pull_targeted_logout_keeps_other_accounts_and_unqualified_logout_removes_all() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    json(&project.run(&["pull", "--login"], false));
    let legacy = project.request();
    let mut named = Vec::new();
    for account in ["work", "client"] {
        json(&project.run(&["pull", "--login", "--account", account], false));
        named.push(project.request());
    }
    std::fs::remove_file(project.root.join(".figlens/sources.json")).unwrap();
    let report = json(&project.run(&["pull", "--logout", "--account", "work"], false));
    assert_eq!(report["account"], "work");
    for browser in ["playwright", "agent-browser"] {
        assert!(!Path::new(named[0]["profiles"][browser].as_str().unwrap()).exists());
        assert!(Path::new(named[1]["profiles"][browser].as_str().unwrap()).exists());
        assert!(Path::new(legacy["profiles"][browser].as_str().unwrap()).exists());
    }
    json(&project.run(&["pull", "--logout", "--account", "missing"], false));
    json(&project.run(&["pull", "--logout"], false));
    for request in [&legacy, &named[1]] {
        for browser in ["playwright", "agent-browser"] {
            assert!(!Path::new(request["profiles"][browser].as_str().unwrap()).exists());
        }
    }
}

#[test]
fn pull_refuses_invalid_cli_and_mapped_accounts_before_the_browser() {
    let project = Project::new(LATER);
    let before = project.saved();
    project.grouped_helper();
    for account in [
        "",
        "../work",
        "/work",
        "work/team",
        "work\\team",
        ".",
        "..",
        "Work",
        "-work",
        "a b",
        &"a".repeat(65),
    ] {
        for mode in ["--login", "--logout"] {
            let out = project.run(&["pull", mode, "--account", account], false);
            assert!(!out.status.success(), "{account:?}");
        }
    }
    for account in [json!(null), json!(1), json!("../work"), json!("")] {
        project.map(json!([{"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":account}]));
        let out = project.run(&["pull", "--only", "home"], false);
        assert!(!out.status.success(), "{account}");
    }
    assert!(project.requests().is_empty());
    assert_eq!(project.saved(), before);
    assert!(
        !project
            .run(&["pull", "--login", "--account"], false)
            .status
            .success()
    );
}

#[test]
fn pull_stale_routes_configured_tokens_and_skips_account_permission_failures() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new(LATER);
    project.grouped_helper();
    project.map(json!([
        {"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"},
        {"name":"other", "url":"https://www.figma.com/design/XyZ789/", "file":"figma/other.fig", "account":"client"}
    ]));
    std::fs::write(project.root.join("figma/other.fig"), saved_copy()).unwrap();
    project
        .accounts(json!({"work":{"tokenEnv":"FIGMA_TOKEN"}, "client":{"tokenEnv":"FIGMA_TOKEN"}}));
    let curl = project.root.join("bin/curl");
    std::fs::write(&curl, "#!/bin/sh\nd=$(dirname \"$0\")\necho \"$*\" >> \"$d/calls\"\ncat >> \"$d/tokens\"\ncase \"$*\" in *XyZ789*) printf '{}\\n403';; *) cat \"$d/body\"; printf '\\n200';; esac\n").unwrap();
    std::fs::set_permissions(curl, std::fs::Permissions::from_mode(0o755)).unwrap();
    let no_token = json(&project.run(&["pull", "--stale", "--account", "work"], false));
    assert_eq!(
        no_token["designs"][0]["reason"],
        "staleness unknown: no_token"
    );
    assert!(project.requests().is_empty());
    let report = json(&project.run(&["pull", "--stale"], true));
    assert_eq!(report["designs"][0]["state"], "pulled");
    assert_eq!(
        report["designs"][1]["reason"],
        "staleness unknown: token_rejected"
    );
    assert_eq!(project.requests().len(), 1);
    assert_eq!(
        std::fs::read(project.root.join("figma/other.fig")).unwrap(),
        saved_copy()
    );
    let tokens = std::fs::read_to_string(project.root.join("bin/tokens")).unwrap();
    assert_eq!(tokens.matches("X-Figma-Token: test-token").count(), 2);
    assert!(!project.calls().contains("test-token"));
    assert!(!report.to_string().contains("test-token"));
    // MCP freshness shares the token routing, never browser cookies.
    let reply = project.mcp(
        "freshness",
        json!({"sources":project.root.join(".figlens/sources.json")}),
    );
    assert_ne!(reply["result"]["isError"], true, "{reply}");
}

#[test]
fn pull_keeps_failed_account_copies_and_continues_other_accounts() {
    let project = Project::new(LATER);
    project.grouped_helper();
    let helper = project.root.join("bin/helper");
    let script = std::fs::read_to_string(&helper).unwrap().replace(
        "cp \"$d/download\" \"$dl/group.fig\"",
        "case \"$req\" in *account-work/*) echo 'account cannot export' >&2; exit 1;; esac\ncp \"$d/download\" \"$dl/group.fig\"",
    );
    std::fs::write(helper, script).unwrap();
    project.map(json!([
        {"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"},
        {"name":"other", "url":"https://www.figma.com/design/XyZ789/", "file":"figma/other.fig", "account":"client"}
    ]));
    let out = project.run(&["pull", "--only", "home", "--only", "other"], false);
    assert!(!out.status.success());
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["designs"][0]["state"], "failed", "{report}");
    assert_eq!(report["designs"][1]["state"], "pulled", "{report}");
    assert_eq!(project.saved(), saved_copy());
    assert_eq!(project.requests().len(), 2);
}

#[test]
fn pull_keeps_completed_account_results_when_the_helper_stream_breaks() {
    let project = Project::new(LATER);
    let newer = saved_copy_at(LATER);
    project.helper(&newer, false);
    let helper = project.root.join("bin/helper");
    let mut script = std::fs::read_to_string(&helper).unwrap();
    script.push_str("printf '\\377\\n'\n");
    std::fs::write(helper, script).unwrap();
    project.map(json!([
        {"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"},
        {"name":"other", "url":"https://www.figma.com/design/XyZ789/", "file":"figma/other.fig", "account":"work"}
    ]));
    std::fs::write(project.root.join("figma/other.fig"), saved_copy()).unwrap();
    let out = project.run(&["pull", "--only", "home", "--only", "other"], false);
    assert!(!out.status.success());
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["designs"][0]["state"], "pulled", "{report}");
    assert_eq!(report["designs"][1]["state"], "failed", "{report}");
    assert_eq!(project.saved(), newer);
    assert_eq!(
        std::fs::read(project.root.join("figma/other.fig")).unwrap(),
        saved_copy()
    );
}

#[test]
fn pull_keeps_completed_results_and_continues_after_profile_preparation_fails() {
    use std::os::unix::fs::symlink;
    let project = Project::new(LATER);
    let newer = saved_copy_at(LATER);
    project.helper(&newer, false);
    json(&project.run(&["pull", "--login"], false));
    let request = project.request();
    let pull = Path::new(request["profiles"]["playwright"].as_str().unwrap())
        .parent()
        .unwrap();
    std::fs::create_dir_all(pull.join("accounts")).unwrap();
    let outside = project.root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    symlink(&outside, pull.join("accounts/account-client")).unwrap();
    project.grouped_helper();
    project.map(json!([
        {"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"},
        {"name":"other", "url":"https://www.figma.com/design/XyZ789/", "file":"figma/other.fig", "account":"client"},
        {"name":"third", "url":"https://www.figma.com/design/Third123/", "file":"figma/third.fig"}
    ]));
    std::fs::write(project.root.join("figma/other.fig"), saved_copy()).unwrap();
    let out = project.run(
        &[
            "pull", "--only", "home", "--only", "other", "--only", "third",
        ],
        false,
    );
    assert!(!out.status.success());
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["designs"][0]["state"], "pulled", "{report}");
    assert_eq!(report["designs"][1]["state"], "failed", "{report}");
    assert_eq!(report["designs"][1]["account"], "client", "{report}");
    assert_eq!(report["designs"][2]["state"], "pulled", "{report}");
    assert_eq!(project.saved(), newer);
    assert_eq!(
        std::fs::read(project.root.join("figma/third.fig")).unwrap(),
        newer
    );
    assert_eq!(
        std::fs::read(project.root.join("figma/other.fig")).unwrap(),
        saved_copy()
    );
    assert_eq!(project.requests().len(), 2);
    assert!(std::fs::read_dir(outside).unwrap().next().is_none());
}

#[test]
fn pull_named_login_failure_preserves_account_recovery_guidance() {
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    std::fs::write(
        project.root.join("bin/helper"),
        "#!/bin/sh\nread -r req\necho 'login cancelled' >&2\nexit 1\n",
    )
    .unwrap();
    let out = project.run(&["pull", "--login", "--account", "work"], false);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("retry the same login command"), "{stderr}");
    assert!(
        !stderr.contains("run figlens pull --login again"),
        "{stderr}"
    );
}

#[test]
fn pull_refuses_symbolic_account_directories() {
    use std::os::unix::fs::symlink;
    let project = Project::new(LATER);
    project.helper(&saved_copy_at(LATER), false);
    json(&project.run(&["pull", "--login"], false));
    let request = project.request();
    let pull = Path::new(request["profiles"]["playwright"].as_str().unwrap())
        .parent()
        .unwrap();
    let outside = project.root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep"), "keep").unwrap();
    symlink(&outside, pull.join("accounts")).unwrap();
    for args in [
        vec!["pull", "--login", "--account", "work"],
        vec!["pull", "--logout", "--account", "work"],
        vec!["pull", "--logout"],
    ] {
        let out = project.run(&args, false);
        assert!(!out.status.success(), "{out:?}");
    }
    assert_eq!(
        std::fs::read_to_string(outside.join("keep")).unwrap(),
        "keep"
    );
    assert!(!outside.join("account-work").exists());
}

#[test]
fn named_api_tokens_route_lists_pulls_and_overrides_without_default_token() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new(LATER);
    project.grouped_helper();
    project.accounts(
        json!({"work":{"tokenEnv":"FIGMA_TOKEN_WORK"}, "client":{"tokenEnv":"FIGMA_TOKEN_CLIENT"}}),
    );
    project.map(json!([
        {"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"},
        {"name":"other", "url":"https://www.figma.com/design/XyZ789/", "file":"figma/other.fig", "account":"client"}
    ]));
    std::fs::write(project.root.join("figma/other.fig"), saved_copy()).unwrap();
    let curl = project.root.join("bin/curl");
    std::fs::write(
        &curl,
        r#"#!/bin/sh
d=$(dirname "$0")
echo "$*" >> "$d/calls"
header=$(cat)
case "$*:$header" in
  *AbC123*work-secret*|*XyZ789*client-secret*) cat "$d/body"; printf '\n200';;
  *) printf '{}\n403';;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(curl, std::fs::Permissions::from_mode(0o755)).unwrap();
    let run = |args: &[&str]| {
        project
            .command(args, false)
            .env("FIGMA_TOKEN_WORK", "work-secret")
            .env("FIGMA_TOKEN_CLIENT", "client-secret")
            .output()
            .unwrap()
    };
    for args in [vec!["freshness"], vec!["pull"]] {
        let out = run(&args);
        let report = json(&out);
        assert_eq!(report["tokenSet"], false);
        assert_eq!(report["designs"][0]["state"], "stale");
        assert_eq!(report["designs"][1]["state"], "stale");
        assert_eq!(report["designs"][0]["tokenVar"], "FIGMA_TOKEN_WORK");
        assert_eq!(report["designs"][1]["tokenVar"], "FIGMA_TOKEN_CLIENT");
        assert!(!report.to_string().contains("-secret"));
        assert!(!String::from_utf8_lossy(&out.stderr).contains("-secret"));
    }
    assert!(project.requests().is_empty());
    let overridden = json(&run(&["freshness", "--account", "work"]));
    assert_eq!(overridden["designs"][1]["account"], "work");
    assert_eq!(overridden["designs"][1]["reason"], "token_rejected");
    let direct_default = json(&run(&[
        "freshness",
        project.fig().to_str().unwrap(),
        "--url",
        "https://www.figma.com/design/AbC123/",
    ]));
    assert_eq!(direct_default["designs"][0]["reason"], "no_token");
    assert!(direct_default["designs"][0].get("account").is_none());
    let direct = json(&run(&[
        "freshness",
        project.fig().to_str().unwrap(),
        "--url",
        "https://www.figma.com/design/AbC123/",
        "--account",
        "work",
    ]));
    assert_eq!(direct["designs"][0]["state"], "stale");
    let pulled = json(&run(&["pull", "--stale"]));
    assert_eq!(pulled["designs"][0]["state"], "pulled");
    assert_eq!(pulled["designs"][1]["state"], "pulled");
    assert_eq!(project.requests().len(), 2);
    assert!(!project.calls().contains("-secret"));
}

#[test]
fn named_api_registry_is_snapshotted_per_operation_and_needs_only_config_root() {
    let project = Project::new(LATER);
    project.accounts(json!({"work":{"tokenEnv":"FIGMA_TOKEN"}}));
    project.map(json!([
        {"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"},
        {"name":"other", "url":"https://www.figma.com/design/XyZ789/", "file":"figma/other.fig", "account":"work"}
    ]));
    std::fs::write(project.root.join("figma/other.fig"), saved_copy()).unwrap();
    let out = project
        .command(&["freshness"], true)
        .env_remove("HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("XDG_CACHE_HOME")
        .env_remove("XDG_DATA_HOME")
        .output()
        .unwrap();
    assert_eq!(json(&out)["designs"][0]["state"], "stale");

    std::fs::write(
        project.root.join("bin/curl"),
        r#"#!/bin/sh
d=$(dirname "$0")
cat >/dev/null
cat "$d/body"
printf '\n200'
printf '{}' > "$XDG_CONFIG_HOME/figlens/accounts.json"
"#,
    )
    .unwrap();
    let first = json(&project.run(&["freshness"], true));
    assert_eq!(first["designs"][0]["state"], "stale");
    assert_eq!(first["designs"][1]["state"], "stale");
    let next = json(&project.run(&["freshness"], true));
    assert_eq!(next["designs"][0]["reason"], "account_not_configured");
}

#[test]
fn named_api_failures_never_fall_back_and_default_ignores_invalid_config() {
    let project = Project::new(LATER);
    project.map(json!([{"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"}]));
    let missing = json(&project.run(&["freshness"], true));
    assert_eq!(missing["designs"][0]["reason"], "account_not_configured");
    assert_eq!(missing["designs"][0]["tokenVar"], Value::Null);
    project.accounts(json!({"work":{"tokenEnv":"FIGMA_TOKEN_WORK"}}));
    let missing = json(&project.run(&["freshness"], true));
    assert_eq!(missing["designs"][0]["reason"], "no_token");
    assert_eq!(missing["designs"][0]["tokenSet"], false);
    assert!(project.calls().is_empty());
    for invalid in [
        json!({"work":{"tokenEnv":"not a variable"}}),
        json!({"work":{"tokenEnv":"1BAD"}}),
        json!({"work":{"tokenEnv":"FIGMA_TOKEN", "token":"private-value"}}),
        json!({"../work":{"tokenEnv":"FIGMA_TOKEN"}}),
        json!([]),
    ] {
        project.accounts(invalid);
        let out = project.run(&["freshness"], true);
        let report = json(&out);
        assert_eq!(report["designs"][0]["reason"], "account_config_invalid");
        assert!(!report.to_string().contains("private-value"));
        assert!(project.calls().is_empty());
    }
    project.map(json!([{"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig"}]));
    let default = json(&project.run(&["freshness"], true));
    assert_eq!(default["designs"][0]["state"], "stale");
    assert!(default["designs"][0].get("account").is_none());
    for args in [
        vec!["freshness", "--account", "../bad"],
        vec!["freshness", "--account"],
    ] {
        assert!(!project.run(&args, true).status.success());
    }
}

#[test]
fn mcp_freshness_and_stale_hints_use_mapped_account_without_fallback() {
    let project = Project::new(LATER);
    project.accounts(json!({"work":{"tokenEnv":"FIGMA_TOKEN"}}));
    project.map(json!([{"name":"home", "url":"https://www.figma.com/design/AbC123/", "file":"figma/home.fig", "account":"work"}]));
    let replies = project.mcp_all(&[
        ("freshness", json!({"file":project.fig()})),
        (
            "freshness",
            json!({"file":project.fig(), "account":"unknown"}),
        ),
        (
            "find",
            json!({"file":project.fig(),"query":"impossible-name"}),
        ),
        (
            "find",
            json!({"file":project.fig(),"query":"impossible-name"}),
        ),
    ]);
    let report = &replies[0]["result"]["structuredContent"];
    assert_eq!(report["designs"][0]["account"], "work", "{replies:?}");
    assert_eq!(report["designs"][0]["state"], "stale");
    assert_eq!(
        replies[1]["result"]["structuredContent"]["designs"][0]["reason"],
        "account_not_configured"
    );
    for reply in &replies[2..] {
        assert_eq!(
            reply["result"]["structuredContent"]["freshness"]["account"], "work",
            "{reply}"
        );
    }
    assert_eq!(project.calls().matches("api.figma.com").count(), 2);
    // A registry change in one live server must invalidate the unchanged copy's hint.
    let curl = project.root.join("bin/curl");
    let script = std::fs::read_to_string(&curl).unwrap();
    let config = project.root.join("config/figlens/accounts.json");
    let next = json!({"work":{"tokenEnv":"FIGMA_TOKEN_WORK"}});
    std::fs::write(
        &curl,
        format!("{script}\nprintf '%s' '{next}' > '{}'\n", config.display()),
    )
    .unwrap();
    let replies = project.mcp_all(&[
        (
            "find",
            json!({"file":project.fig(),"query":"impossible-name"}),
        ),
        (
            "find",
            json!({"file":project.fig(),"query":"impossible-name"}),
        ),
    ]);
    assert!(
        replies[0]["result"]["structuredContent"]
            .get("freshness")
            .is_some()
    );
    assert!(
        replies[1]["result"]["structuredContent"]
            .get("freshness")
            .is_none()
    );
    let reply = project.mcp(
        "find",
        json!({"file":project.fig(),"query":"impossible-name"}),
    );
    assert!(
        reply["result"]["structuredContent"]
            .get("freshness")
            .is_none()
    );
}
