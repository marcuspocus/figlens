//! Index jobs across real processes: the CLI and MCP server as users run them, the worker
//! child, cancellation, owner death, single-flight, authorization and publication fences.
//! `FIGLENS_TEST_HOLD=<stage>:<file>` (debug builds only) keeps a worker in a stage.

#[path = "../src/test_support.rs"]
#[allow(dead_code)]
mod test_support;

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use test_support::TestDir;

const BIN: &str = env!("CARGO_BIN_EXE_figlens");

struct Env {
    dir: TestDir,
    home: PathBuf,
}

impl Env {
    fn new() -> Self {
        let dir = TestDir::new();
        let home = std::fs::canonicalize(dir.join("")).unwrap().join("home");
        std::fs::create_dir(&home).unwrap();
        Self { dir, home }
    }

    fn fig(&self, name: &str) -> PathBuf {
        let path = self.dir.join(name);
        std::fs::copy("examples/first-success/design.fig", &path).unwrap();
        std::fs::canonicalize(path).unwrap()
    }

    fn command(&self, args: &[&str], hold: Option<(&str, &Path)>) -> Command {
        let mut command = Command::new(BIN);
        command
            .args(args)
            .env("HOME", &self.home)
            // Windows roots ignore HOME and use %LOCALAPPDATA%, which would be the runner's own.
            .env("LOCALAPPDATA", &self.home)
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_STATE_HOME")
            .env_remove("XDG_CACHE_HOME")
            .env_remove("XDG_DATA_HOME")
            .env_remove("FIGLENS_TEST_HOLD")
            // Run inside a Herdr terminal, `figlens mcp` would write its activity journal.
            .env_remove("HC_TERMINAL_ID");
        // A `gcloud` stand-in, when a test writes one, comes before the host's own.
        let path = std::env::var_os("PATH").unwrap_or_default();
        let dirs = std::iter::once(self.dir.join("bin")).chain(std::env::split_paths(&path));
        command.env("PATH", std::env::join_paths(dirs).unwrap());
        if let Some((stage, file)) = hold {
            command.env("FIGLENS_TEST_HOLD", format!("{stage}:{}", file.display()));
        }
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args, None).output().unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }

    fn spawn(&self, args: &[&str], hold: Option<(&str, &Path)>) -> Child {
        self.command(args, hold)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    /// A file whose existence holds a worker in its stage.
    fn gate(&self, name: &str) -> PathBuf {
        let path = self.dir.join(name);
        std::fs::write(&path, "").unwrap();
        path
    }
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The active job of the only workspace, read through the CLI.
fn active_job(env: &Env, file: &Path, stage: &str) -> String {
    let mut found = String::new();
    wait_for("the job to reach its stage", || {
        let status = env.json(&["index", "status", "--file", file.to_str().unwrap()]);
        let ws = status["workspaceId"].as_str().unwrap().to_owned();
        let db = PathBuf::from(status["paths"]["database"].as_str().unwrap());
        let conn =
            rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        let row: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT job_id, stage FROM jobs WHERE state IN ('queued', 'running')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        match row {
            Some((job, Some(at))) if at == stage && job.starts_with(&ws) => {
                found = job;
                true
            }
            _ => false,
        }
    });
    found
}

/// Whether a worker process of `job` is alive, from `ps`: independent of the worker-file lock
/// the product reads, so a lock dropped before the process exits is still caught.
#[cfg(unix)]
fn worker_running(_env: &Env, job: &str) -> bool {
    let out = Command::new("ps")
        .args(["-eo", "stat=,args="])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).lines().any(|l| {
        l.contains("__index-worker") && l.contains(job) && !l.trim_start().starts_with('Z')
    })
}

/// Whether a worker of `job` is alive: `ps` lists no arguments on Windows, so this reads the
/// lock a worker holds on its `claims/<job>.<attempt>.worker` file until it exits.
#[cfg(not(unix))]
fn worker_running(env: &Env, job: &str) -> bool {
    let prefix = format!("{job}.");
    let mut dirs = vec![env.dir.join("")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                dirs.push(path);
            } else if name.starts_with(&prefix)
                && name.ends_with(".worker")
                && std::fs::File::open(&path).is_ok_and(|f| f.try_lock_shared().is_err())
            {
                return true;
            }
        }
    }
    false
}

fn ids(reply: &Value) -> Vec<&str> {
    reply["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["ref"]["nodeId"].as_str().unwrap())
        .collect()
}

#[test]
fn every_root_and_workspace_path_stays_inside_the_test_home() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let prepared = env.json(&["index", "prepare", fig.to_str().unwrap()]);
    let mut paths: Vec<&str> = ["config", "state", "cache", "data"]
        .iter()
        .map(|k| prepared["roots"][k]["path"].as_str().unwrap())
        .collect();
    paths.extend(["workspace", "database", "lock"].map(|k| prepared["paths"][k].as_str().unwrap()));
    let outside: Vec<_> = paths
        .into_iter()
        .filter(|p| !Path::new(p).starts_with(&env.home))
        .collect();
    assert!(
        outside.is_empty(),
        "{outside:?} outside {}",
        env.home.display()
    );
}

#[test]
fn a_prepared_scope_searches_like_the_stateless_search_and_survives_a_rebuild() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let before = env.run(&["search", "--file", file, "first"]);
    assert!(String::from_utf8_lossy(&before.stderr).contains("is not prepared"));
    let prepared = env.json(&["index", "prepare", file, "--scope", "0:0"]);
    assert_eq!(prepared["job"]["state"], "succeeded");
    let stateless = env.json(&["search", file, "first"]);
    let indexed = env.json(&["search", "--file", file, "first"]);
    assert_eq!(indexed["state"], "ready");
    assert!(!ids(&stateless).is_empty());
    assert_eq!(ids(&indexed), ids(&stateless));
    // Whole results agree; only the source identity is the workspace's own.
    let plain = |reply: &Value| {
        let mut results = reply["results"].clone();
        for r in results.as_array_mut().unwrap() {
            r["ref"].as_object_mut().unwrap().remove("sourceId");
        }
        results
    };
    assert_eq!(plain(&indexed), plain(&stateless));
    assert_eq!(
        indexed["coverage"]["scannedRecords"],
        stateless["coverage"]["scannedRecords"]
    );
    assert_eq!(indexed["coverage"]["complete"], true);
    let rebuilt = env.json(&["index", "rebuild", "--file", file]);
    assert_eq!(rebuilt["jobs"][0]["state"], "succeeded");
    assert_ne!(
        rebuilt["jobs"][0]["generation"],
        prepared["job"]["generation"]
    );
    assert_eq!(
        ids(&env.json(&["search", "--file", file, "first"])),
        ids(&stateless)
    );
}

#[test]
fn two_sources_rank_as_one_corpus_and_a_cursor_over_a_replaced_generation_is_stale() {
    let env = Env::new();
    let project = env.dir.join("project");
    std::fs::create_dir(&project).unwrap();
    let dir = std::fs::canonicalize(project).unwrap();
    let dir = dir.to_str().unwrap();
    for name in ["a.fig", "b.fig"] {
        let fig = env.fig(name);
        env.json(&[
            "index",
            "prepare",
            fig.to_str().unwrap(),
            "--workspace",
            dir,
            "--scope",
            "0:0",
        ]);
    }
    let all = env.json(&["search", "--workspace", dir, "first"]);
    let sources: Vec<&Value> = all["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["ref"]["sourceId"])
        .collect();
    assert_eq!(sources.len(), 2);
    assert_ne!(sources[0], sources[1]);
    let page = env.json(&["search", "--workspace", dir, "first", "--limit", "1"]);
    assert_eq!(page["results"][0]["ref"], all["results"][0]["ref"]);
    let cursor = page["nextCursor"]
        .as_str()
        .expect("a second page")
        .to_owned();
    let next = env.json(&[
        "search",
        "--workspace",
        dir,
        "first",
        "--limit",
        "1",
        "--cursor",
        &cursor,
    ]);
    assert_eq!(next["results"][0]["ref"], all["results"][1]["ref"]);
    env.json(&["index", "rebuild", "--workspace", dir]);
    let out = env.run(&[
        "search",
        "--workspace",
        dir,
        "first",
        "--limit",
        "1",
        "--cursor",
        &cursor,
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("search again without cursor"));
}

#[test]
fn cancel_stops_the_worker_before_reporting_and_keeps_the_previous_index() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let expected = ids(&env.json(&["search", "--file", file, "first"]))
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for stage in ["hash", "parse", "write", "verify"] {
        let gate = env.gate(&format!("{stage}.hold"));
        let mut owner = env.spawn(&["index", "rebuild", "--file", file], Some((stage, &gate)));
        let job = active_job(&env, &fig, stage);
        assert!(worker_running(&env, &job));
        let start = Instant::now();
        let reply = env.json(&["jobs", "cancel", &job]);
        assert_eq!(reply["cancel"], "accepted", "{stage}: {reply}");
        assert_eq!(reply["state"], "cancelled");
        // Reported only after the worker exited.
        assert!(
            !worker_running(&env, &job),
            "{stage}: worker outlived the cancel"
        );
        assert!(start.elapsed() < Duration::from_millis(2500));
        assert!(!owner.wait().unwrap().success());
        std::fs::remove_file(&gate).unwrap();
        let search = env.json(&["search", "--file", file, "first"]);
        assert_eq!(search["state"], "ready", "{stage}");
        assert_eq!(ids(&search), expected, "{stage}");
    }
}

#[test]
fn a_dead_owner_leaves_an_interrupted_job_whose_worker_exits_and_a_refresh_retries_it() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file]);
    let gate = env.gate("parse.hold");
    let mut owner = env.spawn(
        &["index", "prepare", file, "--scope", "0:0"],
        Some(("parse", &gate)),
    );
    let job = active_job(&env, &fig, "parse");
    owner.kill().unwrap();
    owner.wait().unwrap();
    // The worker sees its stdin close and exits although its stage is still held.
    wait_for("the orphaned worker to exit", || {
        !worker_running(&env, &job)
    });
    assert_eq!(env.json(&["jobs", "status", &job])["state"], "interrupted");
    std::fs::remove_file(&gate).unwrap();
    let retried = env.json(&["index", "refresh", "--file", file, "--scope", "0:0"]);
    assert_eq!(retried["jobs"][0]["state"], "succeeded");
    assert_eq!(
        env.json(&["search", "--file", file, "first"])["state"],
        "ready"
    );
}

#[test]
fn concurrent_prepares_of_one_scope_share_one_job() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file]);
    let gate = env.gate("parse.hold");
    let first = env.spawn(
        &["index", "prepare", file, "--scope", "0:0"],
        Some(("parse", &gate)),
    );
    let job = active_job(&env, &fig, "parse");
    let second = env.spawn(&["index", "prepare", file, "--scope", "0:0"], None);
    std::thread::sleep(Duration::from_millis(300));
    std::fs::remove_file(&gate).unwrap();
    let replies: Vec<Value> = [first, second]
        .into_iter()
        .map(|c| {
            let out = c.wait_with_output().unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            serde_json::from_slice(&out.stdout).unwrap()
        })
        .collect();
    for reply in &replies {
        assert_eq!(reply["job"]["jobId"], job.as_str());
        assert_eq!(reply["job"]["state"], "succeeded");
    }
}

#[test]
fn a_source_removed_and_added_during_a_build_is_not_published_by_that_build() {
    let env = Env::new();
    let project = env.dir.join("project");
    std::fs::create_dir(&project).unwrap();
    let project = std::fs::canonicalize(project).unwrap();
    let fig = env.fig("a.fig");
    let (file, dir) = (fig.to_str().unwrap(), project.to_str().unwrap());
    let prepared = env.json(&["index", "prepare", file, "--workspace", dir]);
    let source = prepared["source"]["sourceId"].as_str().unwrap().to_owned();
    let gate = env.gate("write.hold");
    let owner = env.spawn(
        &[
            "index",
            "prepare",
            file,
            "--workspace",
            dir,
            "--scope",
            "0:0",
        ],
        Some(("write", &gate)),
    );
    let mut job = String::new();
    wait_for("the build to reach write", || {
        let status = env.json(&["index", "status", "--workspace", dir]);
        let db = status["paths"]["database"].as_str().unwrap().to_owned();
        let conn =
            rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        conn.query_row(
            "SELECT job_id FROM jobs WHERE stage = 'write' AND state = 'running'",
            [],
            |r| r.get::<_, String>(0),
        )
        .map(|j| job = j)
        .is_ok()
    });
    env.json(&["index", "remove", &source, "--workspace", dir]);
    let added = env.json(&["index", "add", file, "--workspace", dir]);
    assert_eq!(added["source"]["sourceId"], source.as_str());
    std::fs::remove_file(&gate).unwrap();
    let _ = owner.wait_with_output().unwrap();
    assert_ne!(env.json(&["jobs", "status", &job])["state"], "succeeded");
    let search = env.json(&["search", "--workspace", dir, "first"]);
    assert_eq!(search["state"], "not_prepared", "{search}");
}

/// One MCP server session; requests are answered by id.
struct Mcp {
    child: Child,
    lines: BufReader<std::process::ChildStdout>,
    next: u64,
}

impl Mcp {
    fn start(env: &Env, args: &[&str]) -> Self {
        Self::held(env, args, None)
    }

    fn held(env: &Env, args: &[&str], hold: Option<(&str, &Path)>) -> Self {
        Self::spawn(env.command(&[&["mcp"], args].concat(), hold))
    }

    /// A server started in Herdr terminal `t7`, so it writes the activity journal.
    fn in_herdr(env: &Env) -> Self {
        let mut command = env.command(&["mcp"], None);
        command.env("HC_TERMINAL_ID", "t7");
        Self::spawn(command)
    }

    fn spawn(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let lines = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            lines,
            next: 0,
        }
    }

    fn send(&mut self, message: Value) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.lines.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.next += 1;
        let request = json!({"jsonrpc": "2.0", "id": self.next, "method": "tools/call",
            "params": {"name": tool, "arguments": arguments}});
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{request}").unwrap();
        stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            self.lines.read_line(&mut line).unwrap();
            let reply: Value = serde_json::from_str(&line).unwrap();
            if reply["id"] == self.next {
                return reply["result"].clone();
            }
        }
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

#[test]
fn usages_agree_stateless_over_mcp_and_from_a_prepared_scope_and_reject_a_stale_ref() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let stateless = env.json(&["usages", file, "0:1", "--relation", "contains"]);
    assert_eq!(ids(&stateless), ["0:2"]);
    let mut mcp = Mcp::start(&env, &[]);
    let over_mcp = mcp.call(
        "usages",
        json!({"file": file, "node": "0:1", "relation": "contains"}),
    );
    drop(mcp);
    assert_eq!(
        over_mcp["structuredContent"]["results"],
        stateless["results"]
    );
    // Stateless usages, like search, never create a database.
    assert!(!env.home.join(".local/state/figlens").exists());
    let prepared = env.json(&["index", "prepare", file, "--scope", "0:0"]);
    assert_eq!(prepared["job"]["state"], "succeeded");
    // Each call is a new process: the published index is reused, never rebuilt.
    for _ in 0..2 {
        let indexed = env.json(&["usages", "--file", file, "0:1", "--relation", "contains"]);
        let plain = |reply: &Value| {
            let mut results = reply["results"].clone();
            for r in results.as_array_mut().unwrap() {
                r["ref"].as_object_mut().unwrap().remove("sourceId");
            }
            results
        };
        assert_eq!(plain(&indexed), plain(&stateless));
        assert_eq!(indexed["definition"]["state"], "indexed");
    }
    let snapshot = stateless["source"]["snapshotId"].as_str().unwrap();
    let current = env.run(&["usages", "--file", file, "0:1", "--snapshot", snapshot]);
    assert!(current.status.success());
    let stale = env.run(&["usages", "--file", file, "0:1", "--snapshot", "sha256:old"]);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("now serves"));
}

#[test]
fn a_schema_3_workspace_serves_usages_and_empty_changes_without_upgrading() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let prepared = env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let db = prepared["paths"]["database"].as_str().unwrap().to_owned();
    let version = || -> i64 {
        rusqlite::Connection::open(&db)
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    };
    // As the release before change tracking left it: relations, no tracking tables.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch(
            "DROP TABLE tracking; DROP TABLE facet_sets; DROP TABLE facets;
             DROP TABLE revisions; DROP TABLE revision_changes; DROP TABLE consumers;
             PRAGMA user_version = 3;",
        )
        .unwrap();
    let usages = env.json(&["usages", "--file", file, "0:1", "--relation", "contains"]);
    let changes = env.json(&["changes", "--file", file]);
    assert_eq!(
        (ids(&usages), changes["revisions"].clone(), version()),
        (vec!["0:2"], json!([]), 3)
    );
}

#[test]
fn library_usages_agree_over_the_cli_and_mcp_and_refuse_a_node_target() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let library = [
        "--library",
        "component",
        "--namespace",
        "lib",
        "--key",
        "k1",
    ];
    let cli = env.json(&[&["usages", "--file", file][..], &library].concat());
    assert_eq!(
        cli["definition"]["reason"], "library_source_unproven",
        "{cli}"
    );
    assert_eq!(cli["library"]["key"], "k1");
    let mut mcp = Mcp::start(&env, &[]);
    let reply = mcp.call(
        "usages",
        json!({"workspaceId": cli["workspaceId"],
            "library": {"kind": "component", "namespace": "lib", "key": "k1"}}),
    );
    drop(mcp);
    assert_eq!(reply["isError"], false, "{reply}");
    // Each call checks the stamp when it runs; everything else agrees.
    let checked = |mut c: Value| {
        c["sources"][0]["freshness"]["stampCheckedAtMs"] = Value::Null;
        c
    };
    assert_eq!(
        checked(reply["structuredContent"]["coverage"].clone()),
        checked(cli["coverage"].clone())
    );
    let both = env.run(&[&["usages", "--file", file, "0:1"][..], &library].concat());
    assert!(!both.status.success());
    assert!(String::from_utf8_lossy(&both.stderr).contains("not both"));
}

#[test]
fn a_library_question_by_a_relocated_path_is_asked_again_by_workspace() {
    let env = Env::new();
    let old = env.fig("a.fig");
    let prepared = env.json(&["index", "prepare", old.to_str().unwrap(), "--scope", "0:0"]);
    let new = env.dir.join("moved.fig");
    std::fs::rename(&old, &new).unwrap();
    let new = std::fs::canonicalize(new).unwrap();
    let source = prepared["source"]["sourceId"].as_str().unwrap();
    let old_s = old.to_str().unwrap();
    env.json(&[
        "index",
        "relocate",
        source,
        new.to_str().unwrap(),
        "--file",
        old_s,
    ]);
    std::fs::copy("examples/first-success/design.fig", &old).unwrap();
    let asked = env.run(&[
        "usages",
        "--file",
        old_s,
        "--library",
        "style",
        "--namespace",
        "lib",
        "--key",
        "k1",
        "--source",
        source,
    ]);
    assert!(!asked.status.success());
    let stderr = String::from_utf8_lossy(&asked.stderr);
    let recovery: Value = serde_json::from_str(stderr.lines().last().unwrap()).unwrap();
    let arguments = &recovery["recovery"][0]["arguments"];
    assert_eq!(
        arguments["workspaceId"], prepared["workspaceId"],
        "{stderr}"
    );
    assert_eq!(arguments["library"]["key"], "k1");
    assert_eq!(arguments["sourceId"], source);
    assert!(arguments.get("file").is_none(), "{stderr}");
}

#[test]
fn usages_of_a_scope_not_ready_report_its_state_next_step_and_null_fields() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file]);
    let idle = env.json(&["usages", "--file", file, "0:1"]);
    assert_eq!(idle["state"], "not_prepared");
    assert_eq!(idle["nextSteps"][0]["arguments"]["action"], "refresh");
    let gate = env.gate("parse.hold");
    let mut owner = env.spawn(
        &["index", "prepare", file, "--scope", "0:0"],
        Some(("parse", &gate)),
    );
    let job = active_job(&env, &fig, "parse");
    let mut mcp = Mcp::start(&env, &[]);
    let ws = idle["workspaceId"].as_str().unwrap();
    let reply = mcp.call("usages", json!({"workspaceId": ws, "node": "0:1"}));
    drop(mcp);
    assert_eq!(reply["isError"], false, "{reply}");
    let pending = &reply["structuredContent"];
    assert_eq!(pending["state"], "pending");
    assert_eq!(pending["nextSteps"][0]["arguments"]["jobId"], job.as_str());
    // The fields the output schema requires are present; definition is null until ready.
    for key in ["definition", "results", "coverage", "hasMore", "nextCursor"] {
        assert!(pending.get(key).is_some(), "{key}");
    }
    assert_eq!(pending["definition"], Value::Null);
    std::fs::remove_file(&gate).unwrap();
    assert!(owner.wait().unwrap().success());
    let ready = env.json(&["usages", "--file", file, "0:1"]);
    assert_eq!(ready["definition"]["state"], "indexed");
}

#[test]
fn mcp_builds_in_the_background_and_answers_status_and_search_meanwhile() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let mut mcp = Mcp::start(&env, &[]);
    let gate = env.gate("parse.hold");
    // The server's worker inherits its environment, so the hold applies to its build.
    drop(mcp);
    let mut child = env
        .command(&["mcp"], Some(("parse", &gate)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let lines = BufReader::new(child.stdout.take().unwrap());
    mcp = Mcp {
        child,
        lines,
        next: 0,
    };
    let started = mcp.call(
        "index",
        json!({"action": "prepare", "file": file, "scope": "0:0"}),
    );
    assert_eq!(started["isError"], false, "{started}");
    let job = started["structuredContent"]["job"]["jobId"]
        .as_str()
        .unwrap()
        .to_owned();
    let ws = started["structuredContent"]["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    wait_for("the build to start", || {
        mcp.call("jobs", json!({"action": "status", "jobId": job}))["structuredContent"]["stage"]
            == "parse"
    });
    let pending = mcp.call("search", json!({"workspaceId": ws, "query": "first"}));
    assert_eq!(pending["structuredContent"]["state"], "pending");
    let start = Instant::now();
    let cancelled = mcp.call("jobs", json!({"action": "cancel", "jobId": job}));
    assert!(start.elapsed() < Duration::from_millis(2500));
    assert_eq!(
        cancelled["structuredContent"]["state"], "cancelled",
        "{cancelled}"
    );
    assert!(!worker_running(&env, &job));
    std::fs::remove_file(&gate).unwrap();
    let again = mcp.call(
        "index",
        json!({"action": "refresh", "workspaceId": ws, "scope": "0:0"}),
    );
    let job = again["structuredContent"]["jobs"][0]["jobId"]
        .as_str()
        .unwrap()
        .to_owned();
    wait_for("the refresh to finish", || {
        mcp.call("jobs", json!({"action": "status", "jobId": job}))["structuredContent"]["state"]
            == "succeeded"
    });
    let found = mcp.call("search", json!({"workspaceId": ws, "query": "first"}));
    let stateless = mcp.call("search", json!({"file": file, "query": "first"}));
    assert_eq!(
        ids(&found["structuredContent"]),
        ids(&stateless["structuredContent"])
    );
}

#[test]
fn mcp_roots_hide_sources_jobs_and_counts_outside_them() {
    let env = Env::new();
    let (inside, outside) = (env.dir.join("in"), env.dir.join("out"));
    std::fs::create_dir(&inside).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let project = std::fs::canonicalize(env.dir.join("")).unwrap();
    let copy = |dir: &Path| {
        let path = dir.join("design.fig");
        std::fs::copy("examples/first-success/design.fig", &path).unwrap();
        std::fs::canonicalize(path).unwrap()
    };
    let (seen, hidden) = (copy(&inside), copy(&outside));
    // The hidden directory as it appears inside a JSON reply, with the host's separators.
    let leak = serde_json::to_string(hidden.parent().unwrap()).unwrap();
    let leak = leak.trim_matches('"').to_owned();
    let dir = project.to_str().unwrap();
    env.json(&[
        "index",
        "prepare",
        seen.to_str().unwrap(),
        "--workspace",
        dir,
        "--scope",
        "0:0",
    ]);
    let other = env.json(&[
        "index",
        "prepare",
        hidden.to_str().unwrap(),
        "--workspace",
        dir,
        "--scope",
        "0:0",
    ]);
    let hidden_job = other["job"]["jobId"].as_str().unwrap().to_owned();
    let ws = other["workspaceId"].as_str().unwrap().to_owned();
    let root = std::fs::canonicalize(&inside).unwrap();
    let mut mcp = Mcp::start(&env, &["--root", root.to_str().unwrap()]);
    let status = mcp.call("index", json!({"action": "status", "workspaceId": ws}));
    let text = status.to_string();
    assert!(
        !text.contains(outside.file_name().unwrap().to_str().unwrap()),
        "{text}"
    );
    assert_eq!(
        status["structuredContent"]["sources"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let search = mcp.call("search", json!({"workspaceId": ws, "query": "first"}));
    assert_eq!(
        search["structuredContent"]["sources"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(!search.to_string().contains(&leak));
    let coverage = &search["structuredContent"]["coverage"];
    assert_eq!(coverage["complete"], false, "{search}");
    assert!(
        coverage["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!({"code": "authorization_restricted"}))
    );
    let job = mcp.call("jobs", json!({"action": "status", "jobId": hidden_job}));
    assert_eq!(job["isError"], true);
    assert!(!job.to_string().contains(&leak));
    // Usages read only the visible source; naming the hidden one finds nothing to read.
    let usages = mcp.call("usages", json!({"workspaceId": ws, "node": "0:1"}));
    assert_eq!(usages["isError"], false, "{usages}");
    assert!(!usages.to_string().contains(&leak));
    let all = env.json(&["index", "status", "--workspace", dir]);
    // The unrestricted status lists the hidden copy, so the token matches on this host.
    assert!(all.to_string().contains(&leak));
    let hidden_id = all["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| Path::new(s["path"].as_str().unwrap()) == hidden)
        .unwrap()["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let refused = mcp.call(
        "usages",
        json!({"workspaceId": ws, "sourceId": hidden_id, "node": "0:1"}),
    );
    assert_eq!(refused["isError"], true);
    assert!(!refused.to_string().contains(&leak));
}

#[test]
fn listing_tools_and_stateless_calls_create_no_database() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let mut mcp = Mcp::start(&env, &[]);
    mcp.call("find", json!({"file": fig, "query": "first"}));
    mcp.call("search", json!({"file": fig, "query": "first"}));
    mcp.call(
        "search",
        json!({"workspaceId": "ws_0000000000000000", "query": "first"}),
    );
    drop(mcp);
    assert!(!env.home.join(".local/state/figlens").exists());
}

#[test]
fn mcp_in_a_herdr_terminal_journals_successful_node_reads_but_not_searches() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let out = |name: &str| env.dir.join(name).to_str().unwrap().to_owned();
    let mut mcp = Mcp::in_herdr(&env);
    let link = "https://www.figma.com/design/KEY/Private-Name?node-id=0-4";
    mcp.call("inspect", json!({"file": fig, "node": link}));
    mcp.call(
        "export_batch",
        json!({"file": fig, "items": [{"node": "0:3", "out": out("a.svg")}, {"node": "0:5", "out": out("b.svg")}]}),
    );
    mcp.call("find", json!({"file": fig, "query": "first"}));
    mcp.call("search", json!({"file": fig, "query": "first"}));
    mcp.call("inspect", json!({"file": fig, "node": "99:99"}));
    drop(mcp);
    // The default state folder: Windows keeps it under %LOCALAPPDATA%, which Env points at home.
    let state = if cfg!(windows) {
        env.home.join("figlens").join("state")
    } else {
        env.home.join(".local/state/figlens")
    };
    let lines: Vec<Value> = std::fs::read_dir(state.join("activity"))
        .unwrap()
        .flat_map(|entry| {
            std::fs::read_to_string(entry.unwrap().path())
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .map(|line| serde_json::from_str(&line).unwrap())
        .collect();
    let seen: Vec<(Value, Value)> = lines
        .iter()
        .map(|l| (l["tool"].clone(), l["nodes"].clone()))
        .collect();
    assert_eq!(
        seen,
        [
            (json!("inspect"), json!(["0:4"])),
            (json!("export_batch"), json!(["0:3", "0:5"]))
        ]
    );
}

#[test]
fn a_relocated_source_scoped_file_keeps_its_workspace_and_source_under_the_new_path() {
    let env = Env::new();
    let old = env.fig("a.fig");
    let prepared = env.json(&["index", "prepare", old.to_str().unwrap(), "--scope", "0:0"]);
    let (ws, source) = (&prepared["workspaceId"], &prepared["source"]["sourceId"]);
    let new = env.dir.join("moved.fig");
    std::fs::rename(&old, &new).unwrap();
    let new = std::fs::canonicalize(new).unwrap();
    let (old, new) = (old.to_str().unwrap(), new.to_str().unwrap());
    let source_id = source.as_str().unwrap();
    env.json(&["index", "relocate", source_id, new, "--file", old]);
    let status = env.json(&["index", "status", "--file", new]);
    assert_eq!(&status["workspaceId"], ws);
    assert_eq!(&status["sources"][0]["sourceId"], source);
    // The new membership stops serving the old path's index until a refresh.
    let stale = env.run(&["search", "--file", new, "first"]);
    assert!(String::from_utf8_lossy(&stale.stderr).contains("refresh"));
    env.json(&["index", "refresh", "--file", new, "--scope", "0:0"]);
    let found = env.json(&["search", "--file", new, "first"]);
    assert_eq!(&found["workspaceId"], ws);
    assert_eq!(&found["results"][0]["ref"]["sourceId"], source);
    let again = env.json(&["index", "prepare", new]);
    assert_eq!(&again["workspaceId"], ws);
    assert_eq!(again["source"]["added"], false);
}

#[test]
fn mcp_without_flags_reuses_a_workspace_the_cli_placed_in_custom_storage() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let store = env.dir.join("store");
    std::fs::create_dir(&store).unwrap();
    let store = std::fs::canonicalize(store).unwrap();
    let prepared = env.json(&[
        "index",
        "prepare",
        file,
        "--storage",
        store.to_str().unwrap(),
        "--scope",
        "0:0",
    ]);
    let mut mcp = Mcp::start(&env, &[]);
    let status = mcp.call("index", json!({"action": "status", "file": file}));
    assert_eq!(
        status["structuredContent"]["incarnation"], prepared["incarnation"],
        "{status}"
    );
    assert_eq!(status["structuredContent"]["paths"], prepared["paths"]);
    let found = mcp.call(
        "search",
        json!({"workspaceId": prepared["workspaceId"], "query": "first"}),
    );
    assert_eq!(found["structuredContent"]["state"], "ready", "{found}");
    drop(mcp);
    assert!(!env.home.join(".local/state/figlens/workspaces").exists());
}

#[test]
fn mcp_searches_a_workspace_kept_under_a_storage_root() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let store = env.dir.join("store");
    std::fs::create_dir(&store).unwrap();
    let store = std::fs::canonicalize(store).unwrap();
    let store = store.to_str().unwrap();
    let prepared = env.json(&[
        "index",
        "prepare",
        file,
        "--storage",
        store,
        "--scope",
        "0:0",
    ]);
    let mut mcp = Mcp::start(&env, &["--storage-root", store]);
    let found = mcp.call(
        "search",
        json!({"workspaceId": prepared["workspaceId"], "storage": store, "query": "first"}),
    );
    assert_eq!(found["structuredContent"]["state"], "ready", "{found}");
}

#[test]
fn a_stored_parent_cycle_is_refused_as_an_unreadable_index_instead_of_spinning() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let prepared = env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let conn = rusqlite::Connection::open(prepared["paths"]["database"].as_str().unwrap()).unwrap();
    conn.execute("UPDATE records SET parent = rid WHERE node_id = '0:1'", [])
        .unwrap();
    drop(conn);
    let out = env.run(&["search", "--file", file, "first"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("rebuild"));
}

#[cfg(unix)]
#[test]
fn mcp_roots_do_not_serve_a_source_later_replaced_by_a_link_out_of_them() {
    let env = Env::new();
    let (inside, outside) = (env.dir.join("in"), env.dir.join("out"));
    std::fs::create_dir(&inside).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let (inside, outside) = (
        std::fs::canonicalize(inside).unwrap(),
        std::fs::canonicalize(outside).unwrap(),
    );
    let fig = inside.join("design.fig");
    std::fs::copy("examples/first-success/design.fig", &fig).unwrap();
    let prepared = env.json(&["index", "prepare", fig.to_str().unwrap(), "--scope", "0:0"]);
    let secret = outside.join("secret.fig");
    std::fs::rename(&fig, &secret).unwrap();
    std::os::unix::fs::symlink(&secret, &fig).unwrap();
    let mut mcp = Mcp::start(&env, &["--root", inside.to_str().unwrap()]);
    let found = mcp.call(
        "search",
        json!({"workspaceId": prepared["workspaceId"], "query": "first"}),
    );
    let text = found.to_string();
    assert!(!text.contains("secret"), "{text}");
    assert!(
        found["structuredContent"]["results"]
            .as_array()
            .is_none_or(Vec::is_empty),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn a_directory_add_over_mcp_stays_inside_its_roots_and_the_cli_dry_run_writes_nothing() {
    let env = Env::new();
    let dir = project(&env);
    let (inside, outside) = (dir.join("in"), dir.join("out"));
    std::fs::create_dir(&inside).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let (a, secret) = (inside.join("a.fig"), outside.join("secret.fig"));
    std::fs::copy("examples/first-success/design.fig", &a).unwrap();
    std::fs::copy("examples/first-success/design.fig", &secret).unwrap();
    std::os::unix::fs::symlink(&secret, inside.join("link.fig")).unwrap();
    let (d, i) = (dir.to_str().unwrap(), inside.to_str().unwrap());
    let skipped = json!([{"path": "link.fig", "reason": "escaping_link"}]);
    // The CLI lists what a directory add would register, creating nothing.
    let listed = env.json(&["index", "add", i, "--workspace", d, "--dry-run"]);
    assert_eq!(listed["directory"]["files"], json!([a]));
    assert_eq!(listed["directory"]["skipped"], skipped);
    assert!(!env.home.join(".local/state/figlens").exists());
    let ws = env.json(&["index", "prepare", a.to_str().unwrap(), "--workspace", d]);
    let ws = ws["workspaceId"].as_str().unwrap();
    // The add bound to the dry run's manifest registers that same set.
    let manifest = listed["directory"]["manifest"].as_str().unwrap();
    let bound = env.json(&["index", "add", i, "--id", ws, "--manifest", manifest]);
    assert_eq!(bound["directory"]["existing"][0]["path"], json!(a));
    let mut mcp = Mcp::start(&env, &["--root", i]);
    // A directory wider than the roots is refused before anything under it is read.
    let refused = mcp.call(
        "index",
        json!({"action": "add", "file": dir, "workspaceId": ws}),
    );
    assert_eq!(refused["isError"], true, "{refused}");
    let added = mcp.call(
        "index",
        json!({"action": "add", "file": inside, "workspaceId": ws}),
    );
    assert_eq!(added["isError"], false, "{added}");
    let found = &added["structuredContent"]["directory"];
    assert_eq!(found["added"], json!([]));
    assert_eq!(found["existing"][0]["path"], json!(a));
    assert_eq!(found["skipped"], skipped);
    assert!(!added.to_string().contains("secret"), "{added}");
    drop(mcp);
    let status = env.json(&["index", "status", "--id", ws]);
    assert_eq!(status["sources"].as_array().unwrap().len(), 1);
}

#[test]
fn add_only_flags_are_refused_by_every_other_index_action_before_it_writes() {
    let env = Env::new();
    let dir = project(&env);
    let fig = dir.join("a.fig");
    std::fs::copy("examples/first-success/design.fig", &fig).unwrap();
    let (f, d) = (fig.to_str().unwrap(), dir.to_str().unwrap());
    let refused = |args: &[&str]| !env.run(args).status.success();
    for flag in [
        &["--dry-run"][..],
        &["--manifest", "m"],
        &["--cursor", "d1.x.y"],
    ] {
        let prepare = [&["index", "prepare", f, "--workspace", d][..], flag].concat();
        assert!(refused(&prepare), "{flag:?}");
    }
    assert!(!env.home.join(".local/state/figlens").exists());
    let ws = env.json(&["index", "prepare", f, "--workspace", d]);
    let ws = ws["workspaceId"].as_str().unwrap();
    for action in ["refresh", "rebuild", "track", "untrack"] {
        let args = ["index", action, "--id", ws, "--scope", "0:0", "--dry-run"];
        assert!(refused(&args), "{action}");
    }
    let status = env.json(&["index", "status", "--id", ws]);
    assert_eq!(status["sources"][0]["indexed"], json!([]), "{status}");
}

#[test]
fn an_mcp_directory_add_continues_with_its_cursor_until_none_is_left() {
    let env = Env::new();
    let dir = project(&env);
    let files = dir.join("designs");
    std::fs::create_dir(&files).unwrap();
    for n in 0..12 {
        std::fs::copy(
            "examples/first-success/design.fig",
            files.join(format!("d{n:02}.fig")),
        )
        .unwrap();
    }
    let seed = dir.join("seed.fig");
    std::fs::copy("examples/first-success/design.fig", &seed).unwrap();
    let ws = env.json(&[
        "index",
        "prepare",
        seed.to_str().unwrap(),
        "--workspace",
        dir.to_str().unwrap(),
    ]);
    let mut mcp = Mcp::start(&env, &[]);
    let mut add = json!({"action": "add", "file": files, "workspaceId": ws["workspaceId"]});
    let first = mcp.call("index", add.clone());
    assert_eq!(first["isError"], false, "{first}");
    let page = &first["structuredContent"]["directory"];
    assert_eq!(page["added"].as_array().unwrap().len(), 10, "{first}");
    assert!(page["nextCursor"].is_string(), "{first}");
    add["cursor"] = page["nextCursor"].clone();
    let second = mcp.call("index", add);
    assert_eq!(second["isError"], false, "{second}");
    let page = &second["structuredContent"]["directory"];
    assert_eq!(page["added"].as_array().unwrap().len(), 2, "{second}");
    assert!(page["nextCursor"].is_null(), "{second}");
}

const OBJECT: &str = "gs://example-bucket/designs/home.fig";

/// A `gcloud` stand-in on the test PATH serving the public fixture as the generation in
/// `bin/generation`; it appends each call's arguments to `bin/calls`.
#[cfg(unix)]
fn gcloud(env: &Env, generation: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = env.dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::copy("examples/first-success/design.fig", bin.join("object.fig")).unwrap();
    std::fs::write(bin.join("generation"), generation).unwrap();
    let script = bin.join("gcloud");
    std::fs::write(
        &script,
        "#!/bin/sh\nd=$(dirname \"$0\")\necho \"$*\" >> \"$d/calls\"\n\
         case \"$1 $2\" in\n\
         'storage objects') cat \"$d/generation\" ;;\n\
         'storage cp') cp \"$d/object.fig\" \"$4\" ;;\n\
         *) exit 1 ;;\nesac\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin.join("calls")
}

fn calls(log: &Path) -> String {
    std::fs::read_to_string(log).unwrap_or_default()
}

fn source<'a>(status: &'a Value, id: &str) -> &'a Value {
    let sources = status["sources"].as_array().unwrap();
    sources.iter().find(|s| s["sourceId"] == id).unwrap()
}

#[cfg(unix)]
#[test]
fn a_gs_object_registers_its_generation_pinned_download_under_one_source_id() {
    let env = Env::new();
    let log = gcloud(&env, "101");
    let dir = project(&env);
    let d = dir.to_str().unwrap();
    let local = prepare_in(&env, &env.fig("a.fig"), d);
    let ws = local["workspaceId"].as_str().unwrap();
    let added = env.json(&["index", "add", OBJECT, "--workspace", d]);
    let first = &added["source"];
    assert_eq!(first["added"], true, "{added}");
    assert_eq!(first["remote"], json!({"uri": OBJECT, "generation": "101"}));
    // The download names the generation it records, so the bytes are that generation's.
    assert!(
        calls(&log).contains(&format!("storage cp {OBJECT}#101 ")),
        "{}",
        calls(&log)
    );
    let id = first["sourceId"].as_str().unwrap();
    env.json(&[
        "index", "refresh", "--id", ws, "--source", id, "--scope", "0:0",
    ]);
    let status = env.json(&["index", "status", "--id", ws]);
    let theirs = local["source"]["sourceId"].as_str().unwrap();
    // The snapshot is the hash of the downloaded bytes, here the local copy's own.
    assert_eq!(
        source(&status, id)["indexed"][0]["snapshotId"],
        source(&status, theirs)["indexed"][0]["snapshotId"]
    );
    assert_eq!(source(&status, id)["remote"]["generation"], "101");
    let again = env.json(&["index", "add", OBJECT, "--workspace", d]);
    assert_eq!(again["source"]["added"], false);
    assert_eq!(again["source"]["sourceId"], id);
    assert_eq!(calls(&log).matches("storage cp").count(), 1);
    // A newer upload is fetched by the next add and moves the same source to it.
    std::fs::write(env.dir.join("bin/generation"), "102").unwrap();
    let moved = env.json(&["index", "add", OBJECT, "--workspace", d]);
    assert_eq!(moved["source"]["sourceId"], id);
    assert_eq!(moved["source"]["remote"]["generation"], "102");
    assert_ne!(moved["source"]["path"], first["path"]);
    assert_eq!(
        env.json(&["index", "status", "--id", ws])["sources"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[cfg(unix)]
#[test]
fn a_plain_read_of_a_newer_generation_keeps_a_registered_download_readable() {
    let env = Env::new();
    let log = gcloud(&env, "101");
    let dir = project(&env);
    let d = dir.to_str().unwrap();
    prepare_in(&env, &env.fig("a.fig"), d);
    let added = env.json(&["index", "add", OBJECT, "--workspace", d]);
    let id = added["source"]["sourceId"].as_str().unwrap();
    let before = calls(&log);
    std::fs::write(env.dir.join("bin/generation"), "102").unwrap();
    let find = env.run(&["find", OBJECT, "first"]);
    assert!(
        find.status.success(),
        "{}",
        String::from_utf8_lossy(&find.stderr)
    );
    // A plain read copies the object as it is, never pinned to a generation.
    let read = calls(&log)[before.len()..].to_owned();
    assert!(read.contains(&format!("storage cp {OBJECT} ")), "{read}");
    assert!(!read.contains('#'), "{read}");
    env.json(&[
        "index",
        "refresh",
        "--workspace",
        d,
        "--source",
        id,
        "--scope",
        "0:0",
    ]);
    let status = env.json(&["index", "status", "--workspace", d]);
    assert_eq!(source(&status, id)["present"], true, "{status}");
    assert_eq!(source(&status, id)["remote"]["generation"], "101");
    let found = env.json(&["search", "--workspace", d, "first", "--source", id]);
    assert_eq!(found["results"][0]["ref"]["sourceId"], id, "{found}");
}

#[cfg(unix)]
#[test]
fn only_an_explicit_add_reaches_gcloud_never_status_search_refresh_or_watch() {
    let env = Env::new();
    let log = gcloud(&env, "7");
    let dir = project(&env);
    let d = dir.to_str().unwrap();
    prepare_in(&env, &env.fig("a.fig"), d);
    let added = env.json(&["index", "add", OBJECT, "--workspace", d]);
    let id = added["source"]["sourceId"].as_str().unwrap();
    let before = calls(&log);
    env.json(&[
        "index",
        "refresh",
        "--workspace",
        d,
        "--source",
        id,
        "--scope",
        "0:0",
    ]);
    env.json(&[
        "index",
        "track",
        "--workspace",
        d,
        "--source",
        id,
        "--scope",
        "0:0",
    ]);
    env.json(&["index", "refresh", "--workspace", d, "--source", id]);
    env.json(&["index", "status", "--workspace", d]);
    env.json(&["search", "--workspace", d, "first", "--source", id]);
    env.json(&[
        "changes",
        "--workspace",
        d,
        "--source",
        id,
        "--scope",
        "0:0",
    ]);
    let mut watch = env
        .command(
            &[
                "watch",
                "run",
                "--workspace",
                d,
                "--source",
                id,
                "--scope",
                "0:0",
            ],
            None,
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(watch.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(line.contains("\"active\""), "{line}");
    // Several watcher polls: each stats the local download, never the object.
    std::thread::sleep(Duration::from_millis(1500));
    drop(watch.stdin.take());
    watch.wait().unwrap();
    env.json(&["watch", "status", "--workspace", d]);
    assert_eq!(calls(&log), before);
}

#[cfg(unix)]
#[test]
fn mcp_adds_a_gs_object_without_roots_and_refuses_it_under_roots_before_gcloud() {
    let env = Env::new();
    let log = gcloud(&env, "9");
    let dir = project(&env);
    let local = prepare_in(&env, &env.fig("a.fig"), dir.to_str().unwrap());
    let ws = local["workspaceId"].as_str().unwrap();
    let add = json!({"action": "add", "file": OBJECT, "workspaceId": ws});
    let mut mcp = Mcp::start(&env, &["--root", dir.to_str().unwrap()]);
    let refused = mcp.call("index", add.clone());
    assert_eq!(refused["isError"], true, "{refused}");
    drop(mcp);
    assert_eq!(calls(&log), "");
    let mut mcp = Mcp::start(&env, &[]);
    let added = mcp.call("index", add);
    assert_eq!(added["isError"], false, "{added}");
    assert_eq!(
        added["structuredContent"]["source"]["remote"],
        json!({"uri": OBJECT, "generation": "9"})
    );
    // Only add takes an object; every other action names a registered source or local file.
    let other = mcp.call("index", json!({"action": "status", "file": OBJECT}));
    assert_eq!(other["isError"], true, "{other}");
}

#[cfg(unix)]
#[test]
fn a_queued_refresh_never_reads_a_registered_path_replaced_by_a_link_meanwhile() {
    let env = Env::new();
    let dir = env.dir.join("project");
    std::fs::create_dir(&dir).unwrap();
    let dir = std::fs::canonicalize(dir).unwrap();
    let fig = dir.join("a.fig");
    std::fs::copy("examples/first-success/design.fig", &fig).unwrap();
    let prepared = env.json(&["index", "prepare", fig.to_str().unwrap(), "--scope", "0:0"]);
    let ws = prepared["workspaceId"].as_str().unwrap();
    env.json(&["index", "track", "--id", ws, "--scope", "0:0"]);
    // The refresh is admitted, then the file is swapped for a link to other bytes before its
    // worker reads.
    let gate = env.dir.join("start.hold");
    std::fs::write(&gate, "").unwrap();
    let refresh = env
        .command(
            &["index", "refresh", "--id", ws, "--scope", "0:0"],
            Some(("start", &gate)),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for("the worker to start", || {
        env.dir.join("start.hold.reached").exists()
    });
    let other = env.dir.join("other.fig");
    corpus::write(other.as_os_str(), true).unwrap();
    let other = std::fs::canonicalize(other).unwrap();
    let link = dir.join("link.tmp");
    std::os::unix::fs::symlink(&other, &link).unwrap();
    std::fs::rename(&link, &fig).unwrap();
    std::fs::remove_file(&gate).unwrap();
    let out = refresh.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !out.status.success() && stdout.contains("now leads elsewhere through a link"),
        "{stdout}"
    );
    let changes = env.json(&["changes", "--id", ws, "--scope", "0:0"]);
    let kinds: Vec<_> = changes["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["kind"].clone())
        .collect();
    assert_eq!(kinds, [json!("baseline")]);
}

#[test]
fn an_unusable_workspace_lists_stateless_search_of_the_named_file() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let store = env.dir.join("store");
    std::fs::create_dir(&store).unwrap();
    let store = std::fs::canonicalize(store).unwrap();
    env.json(&[
        "index",
        "prepare",
        file,
        "--storage",
        store.to_str().unwrap(),
        "--scope",
        "0:0",
    ]);
    std::fs::rename(&store, env.dir.join("offline")).unwrap();
    let out = env.run(&["search", "--file", file, "first"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("\"stateless_search\""), "{stderr}");
}

#[cfg(unix)]
#[test]
fn a_source_replaced_by_a_fifo_is_reported_unreadable_without_blocking() {
    let env = Env::new();
    let project = env.dir.join("project");
    std::fs::create_dir(&project).unwrap();
    let dir = std::fs::canonicalize(project).unwrap();
    let dir = dir.to_str().unwrap();
    let fig = env.fig("a.fig");
    env.json(&[
        "index",
        "prepare",
        fig.to_str().unwrap(),
        "--workspace",
        dir,
        "--scope",
        "0:0",
    ]);
    std::fs::remove_file(&fig).unwrap();
    assert!(Command::new("mkfifo").arg(&fig).status().unwrap().success());
    let mut search = env.spawn(&["search", "--workspace", dir, "first"], None);
    let start = Instant::now();
    while search.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(10) {
            let _ = search.kill();
            panic!("search blocked on a FIFO source");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = search.wait_with_output().unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("no searched source can be read"));
}

#[test]
fn the_old_path_of_a_relocated_source_does_not_serve_it_under_that_name() {
    let env = Env::new();
    let old = env.fig("a.fig");
    let prepared = env.json(&["index", "prepare", old.to_str().unwrap(), "--scope", "0:0"]);
    let new = env.dir.join("moved.fig");
    std::fs::rename(&old, &new).unwrap();
    let new = std::fs::canonicalize(new).unwrap();
    let source = prepared["source"]["sourceId"].as_str().unwrap();
    let (old_s, new_s) = (old.to_str().unwrap(), new.to_str().unwrap());
    env.json(&["index", "relocate", source, new_s, "--file", old_s]);
    // A different design now sits at the old path.
    std::fs::copy("examples/first-success/design.fig", &old).unwrap();
    let searched = env.run(&["search", "--file", old_s, "first"]);
    assert!(!searched.status.success());
    let stderr = String::from_utf8_lossy(&searched.stderr);
    assert!(
        stderr.contains("relocated") && stderr.contains(new_s),
        "{stderr}"
    );
    let prepared_again = env.run(&["index", "prepare", old_s]);
    assert!(!prepared_again.status.success());
    assert!(String::from_utf8_lossy(&prepared_again.stderr).contains("relocated"));
    // The new path still selects the workspace.
    assert_eq!(
        env.json(&["index", "status", "--file", new_s])["workspaceId"],
        prepared["workspaceId"]
    );
}

/// A project workspace directory with `fig` prepared into it at scope `0:0`.
fn prepare_in(env: &Env, fig: &Path, dir: &str) -> Value {
    env.json(&[
        "index",
        "prepare",
        fig.to_str().unwrap(),
        "--workspace",
        dir,
        "--scope",
        "0:0",
    ])
}

fn project(env: &Env) -> PathBuf {
    let project = env.dir.join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::canonicalize(project).unwrap()
}

fn refs(reply: &Value) -> Vec<&Value> {
    reply["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["ref"])
        .collect()
}

/// Changes a file's size, so its stamp no longer equals the verified snapshot.
fn grow(fig: &Path) {
    let mut file = std::fs::OpenOptions::new().append(true).open(fig).unwrap();
    file.write_all(b"x").unwrap();
}

#[test]
fn one_node_id_in_two_sources_is_returned_once_per_source_with_its_own_ref() {
    let env = Env::new();
    let project = project(&env);
    let dir = project.to_str().unwrap();
    for name in ["a.fig", "b.fig"] {
        prepare_in(&env, &env.fig(name), dir);
    }
    let all = env.json(&["search", "--workspace", dir, "first"]);
    let found = refs(&all);
    assert_eq!(found.len(), 2, "{all}");
    assert_eq!(found[0]["nodeId"], found[1]["nodeId"]);
    assert_ne!(found[0]["sourceId"], found[1]["sourceId"]);
    for r in &found {
        let own = all["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["sourceId"] == r["sourceId"])
            .unwrap();
        assert_eq!(own["snapshotId"], r["snapshotId"]);
        let alone = env.json(&[
            "search",
            "--workspace",
            dir,
            "--source",
            r["sourceId"].as_str().unwrap(),
            "first",
        ]);
        assert_eq!(refs(&alone), [*r]);
    }
}

#[test]
fn identical_bytes_in_two_sources_keep_their_ids_and_a_hidden_one_never_answers() {
    let env = Env::new();
    let (inside, outside) = (env.dir.join("in"), env.dir.join("out"));
    std::fs::create_dir(&inside).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let copy = |dir: &Path| {
        let path = dir.join("design.fig");
        std::fs::copy("examples/first-success/design.fig", &path).unwrap();
        std::fs::canonicalize(path).unwrap()
    };
    let (seen, hidden) = (copy(&inside), copy(&outside));
    let project = std::fs::canonicalize(env.dir.join("")).unwrap();
    let dir = project.to_str().unwrap();
    let seen_id = prepare_in(&env, &seen, dir)["source"]["sourceId"].clone();
    let other = prepare_in(&env, &hidden, dir);
    let hidden_id = other["source"]["sourceId"].clone();
    assert_ne!(seen_id, hidden_id);
    let status = env.json(&["index", "status", "--workspace", dir]);
    let snapshots: Vec<&Value> = status["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["snapshotId"])
        .collect();
    assert_eq!(snapshots[0], snapshots[1], "{status}");
    let root = std::fs::canonicalize(&inside).unwrap();
    let mut mcp = Mcp::start(&env, &["--root", root.to_str().unwrap()]);
    let search = mcp.call(
        "search",
        json!({"workspaceId": other["workspaceId"], "query": "first"}),
    );
    let found = &search["structuredContent"];
    assert!(!refs(found).is_empty(), "{search}");
    assert!(refs(found).iter().all(|r| r["sourceId"] == seen_id));
    assert!(!search.to_string().contains(hidden_id.as_str().unwrap()));
    assert_eq!(found["coverage"]["totalRecords"], Value::Null);
    // Without the hidden copy the visible one answers the same.
    env.json(&[
        "index",
        "remove",
        hidden_id.as_str().unwrap(),
        "--workspace",
        dir,
    ]);
    let after = mcp.call(
        "search",
        json!({"workspaceId": other["workspaceId"], "query": "first"}),
    );
    assert_eq!(refs(&after["structuredContent"]), refs(found));
}

/// The mapping fixture's library copy (component `synthetic-button` of `synthetic-library`), as
/// `cargo test` builds the example next to the test binary.
fn library_fig(dir: &Path) -> PathBuf {
    library_fig_keyed(dir, "library.fig", "synthetic-button", "synthetic-library")
}

/// The same fixture with another component key and library namespace, written to `dir/name`.
fn library_fig_keyed(dir: &Path, name: &str, key: &str, namespace: &str) -> PathBuf {
    let example = Path::new(BIN)
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("mapping-fixture{}", std::env::consts::EXE_SUFFIX));
    let path = dir.join(name);
    let made = Command::new(example)
        .env("MAPPING_FIXTURE_COMPONENT_KEY", key)
        .env("MAPPING_FIXTURE_LIBRARY_KEY", namespace)
        .arg("--library")
        .arg(&path)
        .output()
        .unwrap();
    assert!(made.status.success(), "{made:?}");
    std::fs::canonicalize(path).unwrap()
}

/// Identical library bytes as two sources of one workspace, in `in/` and `out/`.
fn two_library_sources(env: &Env) -> (String, Value, Value, Value) {
    let project = std::fs::canonicalize(env.dir.join("")).unwrap();
    let dir = project.to_str().unwrap().to_owned();
    let [first, second] = ["in", "out"].map(|name| {
        let sub = env.dir.join(name);
        std::fs::create_dir(&sub).unwrap();
        prepare_in(env, &library_fig(&sub), &dir)
    });
    (
        dir,
        second["workspaceId"].clone(),
        first["source"]["sourceId"].clone(),
        second["source"]["sourceId"].clone(),
    )
}

const LIBRARY: [&str; 6] = [
    "--library",
    "component",
    "--namespace",
    "synthetic-library",
    "--key",
    "synthetic-button",
];

fn owners(reply: &Value) -> Vec<(Value, Value)> {
    refs(reply)
        .iter()
        .map(|r| (r["sourceId"].clone(), r["nodeId"].clone()))
        .collect()
}

#[test]
fn identical_library_bytes_in_two_sources_are_answered_each_under_its_own_source_id() {
    let env = Env::new();
    let (dir, _, first, second) = two_library_sources(&env);
    let ask = |extra: &[&str]| {
        env.json(&[&["usages", "--workspace", &dir][..], &LIBRARY, extra].concat())
    };
    let both = owners(&ask(&[]));
    let of = |id: &Value| -> Vec<_> { both.iter().filter(|(s, _)| s == id).cloned().collect() };
    assert_eq!((of(&first).len(), of(&second).len()), (2, 2), "{both:?}");
    assert_eq!(
        of(&first).iter().map(|(_, n)| n).collect::<Vec<_>>(),
        of(&second).iter().map(|(_, n)| n).collect::<Vec<_>>()
    );
    for id in [&first, &second] {
        let alone = ask(&["--source", id.as_str().unwrap()]);
        assert_eq!(owners(&alone), of(id), "{alone}");
    }
}

#[test]
fn a_hidden_source_of_identical_library_bytes_is_neither_answered_nor_counted() {
    let env = Env::new();
    let (_, workspace, seen, hidden) = two_library_sources(&env);
    let root = std::fs::canonicalize(env.dir.join("in")).unwrap();
    let mut mcp = Mcp::start(&env, &["--root", root.to_str().unwrap()]);
    let reply = mcp.call(
        "usages",
        json!({"workspaceId": workspace, "library": {"kind": "component",
            "namespace": "synthetic-library", "key": "synthetic-button"}}),
    );
    drop(mcp);
    let found = &reply["structuredContent"];
    assert!(owners(found).iter().all(|(s, _)| *s == seen), "{reply}");
    assert_eq!(found["coverage"]["sources"].as_array().unwrap().len(), 1);
    assert!(
        found["coverage"]["reasons"]
            .to_string()
            .contains("authorization_restricted")
    );
    assert!(!reply.to_string().contains(hidden.as_str().unwrap()));
}

#[test]
fn a_definition_chosen_before_its_source_was_rebuilt_is_stale_not_matched() {
    let env = Env::new();
    let project = project(&env);
    let dir = project.to_str().unwrap();
    let fig = library_fig(&env.dir.join(""));
    let prepared = prepare_in(&env, &fig, dir);
    let (source, snapshot) = (
        prepared["source"]["sourceId"].as_str().unwrap().to_owned(),
        env.json(&["index", "status", "--workspace", dir])["sources"][0]["indexed"][0]["snapshotId"]
            .as_str()
            .unwrap()
            .to_owned(),
    );
    let definition = [
        "--definition-source",
        &source,
        "--definition-snapshot",
        &snapshot,
    ];
    let ask = || {
        env.json(
            &[
                &["usages", "--workspace", dir][..],
                &LIBRARY,
                &definition,
                &["--definition-node", "0:10"],
            ]
            .concat(),
        )
    };
    assert_eq!(ask()["definition"]["state"], "selected");
    // The same path now holds other bytes, indexed again under a new snapshot.
    std::fs::remove_file(&fig).unwrap();
    std::fs::copy("examples/first-success/design.fig", &fig).unwrap();
    env.json(&["index", "refresh", "--workspace", dir, "--scope", "0:0"]);
    let rebuilt = ask();
    assert_eq!(
        rebuilt["definition"],
        json!({"state": "unresolved", "reason": "definition_snapshot_stale"}),
        "{rebuilt}"
    );
}

/// Two differently keyed library sources of one workspace; returns the workspace dir and ids.
fn keyed_library_sources(env: &Env, keyed: [(&str, &str); 2]) -> (String, [Value; 2]) {
    let project = std::fs::canonicalize(env.dir.join("")).unwrap();
    let dir = project.to_str().unwrap().to_owned();
    let ids = keyed.map(|(key, namespace)| {
        let sub = env.dir.join(key).join(namespace);
        std::fs::create_dir_all(&sub).unwrap();
        let fig = library_fig_keyed(&sub, "library.fig", key, namespace);
        prepare_in(env, &fig, &dir)["source"]["sourceId"].clone()
    });
    (dir, ids)
}

fn owned_by_only(env: &Env, dir: &str, namespace: &str, key: &str, id: &Value) -> bool {
    let reply = env.json(&[
        "usages",
        "--workspace",
        dir,
        "--library",
        "component",
        "--namespace",
        namespace,
        "--key",
        key,
    ]);
    let found = owners(&reply);
    !found.is_empty() && found.iter().all(|(s, _)| s == id)
}

#[test]
fn a_library_key_answers_only_the_source_that_holds_it() {
    let env = Env::new();
    let (dir, [first, second]) = keyed_library_sources(
        &env,
        [("k1", "synthetic-library"), ("k2", "synthetic-library")],
    );
    assert!(owned_by_only(&env, &dir, "synthetic-library", "k1", &first));
    assert!(owned_by_only(
        &env,
        &dir,
        "synthetic-library",
        "k2",
        &second
    ));
}

#[test]
fn a_library_namespace_never_answers_another_namespaces_uses() {
    let env = Env::new();
    let (dir, [first, second]) = keyed_library_sources(
        &env,
        [("synthetic-button", "ns1"), ("synthetic-button", "ns2")],
    );
    assert!(owned_by_only(&env, &dir, "ns1", "synthetic-button", &first));
    assert!(owned_by_only(
        &env,
        &dir,
        "ns2",
        "synthetic-button",
        &second
    ));
}

#[test]
fn a_missing_source_is_not_answered_by_a_same_name_file_registered_elsewhere() {
    let env = Env::new();
    let project = project(&env);
    let dir = project.to_str().unwrap();
    let old = env.fig("a.fig");
    let old_id = prepare_in(&env, &old, dir)["source"]["sourceId"].clone();
    let elsewhere = env.dir.join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::copy("examples/first-success/design.fig", elsewhere.join("a.fig")).unwrap();
    let new_id = prepare_in(&env, &elsewhere.join("a.fig"), dir)["source"]["sourceId"].clone();
    assert_ne!(old_id, new_id);
    std::fs::remove_file(&old).unwrap();
    let found = env.json(&["search", "--workspace", dir, "first"]);
    assert_eq!(
        refs(&found)
            .iter()
            .map(|r| &r["sourceId"])
            .collect::<Vec<_>>(),
        [&new_id],
        "{found}"
    );
    let gone = found["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["sourceId"] == old_id)
        .unwrap();
    assert_eq!(gone["state"], "unreadable");
    assert_eq!(found["coverage"]["complete"], false);
}

#[test]
fn a_source_recreated_at_its_path_with_other_bytes_is_stale_not_served() {
    let env = Env::new();
    let project = project(&env);
    let dir = project.to_str().unwrap();
    let fig = env.fig("a.fig");
    prepare_in(&env, &fig, dir);
    std::fs::remove_file(&fig).unwrap();
    std::fs::copy("examples/first-success/design.fig", &fig).unwrap();
    grow(&fig);
    let out = env.run(&["search", "--workspace", dir, "first"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("changed since it was indexed"));
}

#[test]
fn a_changed_source_is_listed_stale_and_left_out_of_a_workspace_search() {
    let env = Env::new();
    let project = project(&env);
    let dir = project.to_str().unwrap();
    let (a, b) = (env.fig("a.fig"), env.fig("b.fig"));
    let a_id = prepare_in(&env, &a, dir)["source"]["sourceId"].clone();
    let b_id = prepare_in(&env, &b, dir)["source"]["sourceId"].clone();
    grow(&b);
    let found = env.json(&["search", "--workspace", dir, "first"]);
    assert_eq!(
        refs(&found)
            .iter()
            .map(|r| &r["sourceId"])
            .collect::<Vec<_>>(),
        [&a_id],
        "{found}"
    );
    let stale = found["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["sourceId"] == b_id)
        .unwrap();
    assert_eq!(stale["state"], "stale");
    assert_eq!(found["coverage"]["complete"], false);
    assert!(
        found["coverage"]["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["code"] == "sources_not_ready" && r["sourceIds"] == json!([b_id]))
    );
}

#[test]
fn a_cursor_over_two_sources_is_stale_once_one_leaves_the_mcp_roots() {
    let env = Env::new();
    let (inside, outside) = (env.dir.join("in"), env.dir.join("out"));
    std::fs::create_dir(&inside).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let copy = |dir: &Path| {
        let path = dir.join("design.fig");
        std::fs::copy("examples/first-success/design.fig", &path).unwrap();
        std::fs::canonicalize(path).unwrap()
    };
    let (seen, hidden) = (copy(&inside), copy(&outside));
    let project = std::fs::canonicalize(env.dir.join("")).unwrap();
    let dir = project.to_str().unwrap();
    prepare_in(&env, &seen, dir);
    let ws = prepare_in(&env, &hidden, dir)["workspaceId"].clone();
    let leak = hidden.parent().unwrap().to_str().unwrap().to_owned();
    let mut wide = Mcp::start(&env, &["--root", dir]);
    let first = wide.call(
        "search",
        json!({"workspaceId": ws, "query": "first", "limit": 1}),
    );
    let cursor = first["structuredContent"]["nextCursor"].clone();
    assert!(cursor.is_string(), "{first}");
    let page = wide.call(
        "search",
        json!({"workspaceId": ws, "query": "first", "limit": 1, "cursor": cursor}),
    );
    assert_eq!(page["isError"], false, "{page}");
    // The same cursor in a session whose roots no longer reach one of its sources.
    let root = std::fs::canonicalize(&inside).unwrap();
    let mut narrow = Mcp::start(&env, &["--root", root.to_str().unwrap()]);
    let refused = narrow.call(
        "search",
        json!({"workspaceId": ws, "query": "first", "limit": 1, "cursor": cursor}),
    );
    assert_eq!(refused["isError"], true, "{refused}");
    assert_eq!(refused["_meta"]["figlens"]["code"], "cursor_stale");
    assert!(!refused.to_string().contains(&leak));
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_the_cli_owned_build_and_waits_for_its_worker() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let gate = env.gate("parse.hold");
    let owner = env.spawn(
        &["index", "rebuild", "--file", file],
        Some(("parse", &gate)),
    );
    let job = active_job(&env, &fig, "parse");
    assert!(
        Command::new("kill")
            .args(["-INT", &owner.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let start = Instant::now();
    let out = owner.wait_with_output().unwrap();
    assert!(start.elapsed() < Duration::from_millis(2500));
    assert!(!out.status.success());
    assert!(!worker_running(&env, &job));
    assert_eq!(env.json(&["jobs", "status", &job])["state"], "cancelled");
    std::fs::remove_file(&gate).unwrap();
    assert_eq!(
        env.json(&["search", "--file", file, "first"])["state"],
        "ready"
    );
}

#[cfg(unix)]
#[test]
fn mcp_answers_ping_while_control_waits_and_drops_a_queued_call_the_client_cancelled() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let gate = env.gate("parse.hold");
    let mut owner = env.spawn(
        &["index", "rebuild", "--file", file],
        Some(("parse", &gate)),
    );
    let job = active_job(&env, &fig, "parse");
    // A stopped owner cannot act on a cancel, so `jobs cancel` waits its whole CANCEL_WAIT.
    let pid = owner.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-STOP", &pid])
            .status()
            .unwrap()
            .success()
    );
    let mut mcp = Mcp::start(&env, &[]);
    let call = |id: u64, arguments: Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": "jobs", "arguments": arguments}})
    };
    mcp.send(call(1, json!({"action": "cancel", "jobId": job})));
    mcp.send(call(2, json!({"action": "status", "jobId": job})));
    mcp.send(
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 2}}),
    );
    mcp.send(json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}));
    mcp.send(call(4, json!({"action": "status", "jobId": job})));
    let ids: Vec<Value> = (0..3).map(|_| mcp.read()["id"].clone()).collect();
    assert_eq!(ids, [json!(3), json!(1), json!(4)]);
    assert!(
        Command::new("kill")
            .args(["-CONT", &pid])
            .status()
            .unwrap()
            .success()
    );
    std::fs::remove_file(&gate).unwrap();
    owner.wait().unwrap();
}

#[test]
fn a_search_past_the_query_envelope_is_a_work_limit_with_narrower_searches_not_a_bad_index() {
    let env = Env::new();
    let project = env.dir.join("project");
    std::fs::create_dir(&project).unwrap();
    let dir = std::fs::canonicalize(project).unwrap();
    let dir = dir.to_str().unwrap();
    for n in 0..11 {
        let fig = env.fig(&format!("{n}.fig"));
        env.json(&[
            "index",
            "prepare",
            fig.to_str().unwrap(),
            "--workspace",
            dir,
            "--scope",
            "0:1",
        ]);
    }
    let out = env.run(&[
        "search",
        "--workspace",
        dir,
        "first",
        "--node",
        "0:1",
        "--type",
        "text",
    ]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let recovery: Value = serde_json::from_str(stderr.lines().last().unwrap()).unwrap();
    let actions = recovery["recovery"].as_array().unwrap();
    assert_eq!(actions.len(), 11, "{stderr}");
    assert!(actions.iter().all(|a| a["action"] == "search_one_source"));
    let args = &actions[0]["arguments"];
    assert_eq!(
        (&args["node"], &args["type"]),
        (&json!("0:1"), &json!("TEXT"))
    );
    let one = args["sourceId"].as_str().unwrap();
    let narrowed = env.json(&[
        "search",
        "--workspace",
        dir,
        "first",
        "--source",
        one,
        "--node",
        "0:1",
        "--type",
        "TEXT",
    ]);
    assert_eq!(narrowed["state"], "ready");
    // Nothing was dropped from the registry.
    let status = env.json(&["index", "status", "--workspace", dir]);
    assert_eq!(status["sources"].as_array().unwrap().len(), 11);
}

#[test]
fn rebuild_narrowed_to_one_source_and_scope_builds_only_that_one_even_when_unchanged() {
    let env = Env::new();
    let project = env.dir.join("project");
    std::fs::create_dir(&project).unwrap();
    let dir = std::fs::canonicalize(project).unwrap();
    let dir = dir.to_str().unwrap();
    let mut sources = Vec::new();
    for name in ["a.fig", "b.fig"] {
        let fig = env.fig(name);
        let done = env.json(&[
            "index",
            "prepare",
            fig.to_str().unwrap(),
            "--workspace",
            dir,
            "--scope",
            "0:0",
        ]);
        sources.push(done["source"]["sourceId"].as_str().unwrap().to_owned());
    }
    let rebuilt = env.json(&[
        "index",
        "rebuild",
        "--workspace",
        dir,
        "--source",
        &sources[0],
        "--scope",
        "0:0",
    ]);
    let jobs = rebuilt["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 1, "{rebuilt}");
    assert_eq!(jobs[0]["sourceId"], sources[0].as_str());
    assert_eq!(jobs[0]["kind"], "rebuild");
    assert_eq!(jobs[0]["state"], "succeeded");
    assert_eq!(rebuilt["admission"], "complete");
}

/// Whether `value` has the JSON types `schema` declares, recursively through `properties`,
/// `required` and `items`: what an MCP client validating structuredContent checks.
fn conforms(value: &Value, schema: &Value, at: &str) -> Result<(), String> {
    let kind = |t: &str| match t {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => true,
    };
    let ok = match &schema["type"] {
        Value::String(t) => kind(t),
        Value::Array(ts) => ts.iter().any(|t| kind(t.as_str().unwrap_or(""))),
        _ => true,
    };
    if !ok {
        return Err(format!("{at}: {value} is not {}", schema["type"]));
    }
    for name in schema["required"].as_array().into_iter().flatten() {
        if value.get(name.as_str().unwrap()).is_none() {
            return Err(format!("{at}: missing {name}"));
        }
    }
    if let (Some(props), Some(object)) = (schema["properties"].as_object(), value.as_object()) {
        for (name, sub) in props {
            if let Some(v) = object.get(name) {
                conforms(v, sub, &format!("{at}/{name}"))?;
            }
        }
    }
    if let (Some(items), Some(list)) = (schema.get("items"), value.as_array()) {
        for (i, v) in list.iter().enumerate() {
            conforms(v, items, &format!("{at}/{i}"))?;
        }
    }
    Ok(())
}

#[test]
fn every_index_jobs_and_workspace_search_reply_matches_its_declared_output_schema() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let project = env.dir.join("project");
    std::fs::create_dir(&project).unwrap();
    let project = std::fs::canonicalize(project).unwrap();
    let dir = project.to_str().unwrap();
    let other = env.fig("b.fig");
    let mut mcp = Mcp::start(&env, &[]);
    mcp.send(json!({"jsonrpc": "2.0", "id": 0, "method": "tools/list"}));
    let tools = mcp.read()["result"]["tools"].clone();
    let schema = |name: &str| {
        tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()["outputSchema"]
            .clone()
    };
    let check = |mcp: &mut Mcp, tool: &str, arguments: Value| -> Value {
        let reply = mcp.call(tool, arguments.clone());
        assert_eq!(reply["isError"], false, "{tool} {arguments}: {reply}");
        let content = &reply["structuredContent"];
        conforms(content, &schema(tool), tool).unwrap_or_else(|e| panic!("{arguments}: {e}"));
        content.clone()
    };
    // Until the job's own reply is terminal. Only an active state or a workspace_busy status
    // call is waited out; any other reply fails with it as evidence.
    let wait = |mcp: &mut Mcp, job: &str| {
        let start = Instant::now();
        loop {
            let now = mcp.call("jobs", json!({"action": "status", "jobId": job}));
            let busy =
                now["isError"] == true && now["_meta"]["figlens"]["code"] == "workspace_busy";
            match now["structuredContent"]["state"].as_str() {
                Some("succeeded" | "failed" | "cancelled") if now["isError"] == false => break,
                Some("queued" | "running") if now["isError"] == false => {}
                _ if busy => {}
                _ => panic!("job {job}: unexpected status reply {now}"),
            }
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "job {job} never ended: {now}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    check(&mut mcp, "index", json!({"action": "status", "file": file}));
    check(
        &mut mcp,
        "index",
        json!({"action": "prepare", "file": file}),
    );
    let started = check(
        &mut mcp,
        "index",
        json!({"action": "prepare", "file": file, "scope": "0:0"}),
    );
    let job = started["job"]["jobId"].as_str().unwrap().to_owned();
    let ws = started["workspaceId"].as_str().unwrap().to_owned();
    let source = started["source"]["sourceId"].as_str().unwrap().to_owned();
    wait(&mut mcp, &job);
    let report = check(&mut mcp, "jobs", json!({"action": "status", "jobId": job}));
    assert_eq!(report["workspaceId"], ws.as_str());
    check(&mut mcp, "jobs", json!({"action": "cancel", "jobId": job}));
    check(
        &mut mcp,
        "index",
        json!({"action": "status", "workspaceId": ws}),
    );
    check(
        &mut mcp,
        "search",
        json!({"workspaceId": ws, "query": "first"}),
    );
    check(
        &mut mcp,
        "search",
        json!({"workspaceId": ws, "query": "first", "node": "0:2"}),
    );
    let refreshed = check(
        &mut mcp,
        "index",
        json!({"action": "refresh", "workspaceId": ws}),
    );
    // A rebuild does not join a running refresh, so it starts once the refresh ended.
    for job in refreshed["jobs"].as_array().unwrap() {
        wait(&mut mcp, job["jobId"].as_str().unwrap());
    }
    let rebuilt = check(
        &mut mcp,
        "index",
        json!({"action": "rebuild", "workspaceId": ws, "sourceId": source, "scope": "0:0"}),
    );
    for job in rebuilt["jobs"].as_array().unwrap() {
        wait(&mut mcp, job["jobId"].as_str().unwrap());
    }
    check(
        &mut mcp,
        "index",
        json!({"action": "clear_cache", "workspaceId": ws}),
    );
    check(
        &mut mcp,
        "index",
        json!({"action": "reset", "workspaceId": ws}),
    );
    check(
        &mut mcp,
        "index",
        json!({"action": "status", "workspaceId": ws}),
    );
    // A project workspace: add, relocate and remove.
    let prepared = check(
        &mut mcp,
        "index",
        json!({"action": "prepare", "file": file, "workspace": dir}),
    );
    let pws = prepared["workspaceId"].as_str().unwrap().to_owned();
    let added = check(
        &mut mcp,
        "index",
        json!({"action": "add", "workspaceId": pws, "file": other}),
    );
    let added_id = added["source"]["sourceId"].as_str().unwrap().to_owned();
    let moved = env.dir.join("moved.fig");
    std::fs::rename(&other, &moved).unwrap();
    check(
        &mut mcp,
        "index",
        json!({"action": "relocate", "workspaceId": pws, "sourceId": added_id, "file": moved}),
    );
    check(
        &mut mcp,
        "index",
        json!({"action": "remove", "workspaceId": pws, "sourceId": added_id}),
    );
}

#[test]
fn a_workspace_search_takes_a_query_starting_with_a_dash_after_double_dash() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let found = env.json(&["search", "--file", file, "--", "-20%"]);
    assert_eq!(found["state"], "ready", "{found}");
}

#[test]
fn an_mcp_opt_in_of_one_frame_is_searched_and_inspected_at_that_scope_only() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let mut mcp = Mcp::start(&env, &[]);
    let started = mcp.call(
        "index",
        json!({"action": "prepare", "file": file, "scope": "0:1"}),
    );
    let job = started["structuredContent"]["job"]["jobId"]
        .as_str()
        .unwrap()
        .to_owned();
    let ws = started["structuredContent"]["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    wait_for("the job", || {
        mcp.call("jobs", json!({"action": "status", "jobId": job}))["structuredContent"]["state"]
            == "succeeded"
    });
    let found = mcp.call(
        "search",
        json!({"workspaceId": ws, "node": "0:1", "query": "first"}),
    )["structuredContent"]
        .clone();
    assert_eq!(found["state"], "ready", "{found}");
    let node = found["results"][0]["ref"]["nodeId"].as_str().unwrap();
    let inspected = mcp.call("inspect", json!({"file": file, "node": node}));
    assert_eq!(inspected["isError"], false, "{inspected}");
    // The whole file was never opted in: no implicit 0:0 build.
    let whole = mcp.call("search", json!({"workspaceId": ws, "query": "first"}));
    assert_eq!(
        whole["structuredContent"]["sources"][0]["reason"], "scope_not_indexed",
        "{whole}"
    );
}

#[test]
fn a_queued_and_a_cancelled_before_start_job_match_the_jobs_output_schema() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let gate = env.gate("parse.hold");
    let mut child = env
        .command(&["mcp"], Some(("parse", &gate)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let lines = BufReader::new(child.stdout.take().unwrap());
    let mut mcp = Mcp {
        child,
        lines,
        next: 0,
    };
    mcp.send(json!({"jsonrpc": "2.0", "id": 0, "method": "tools/list"}));
    let tools = mcp.read()["result"]["tools"].clone();
    let schema = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "jobs")
        .unwrap()["outputSchema"]
        .clone();
    mcp.call(
        "index",
        json!({"action": "prepare", "file": file, "scope": "0:0"}),
    );
    let queued = mcp.call(
        "index",
        json!({"action": "prepare", "file": file, "scope": "0:1"}),
    );
    let job = queued["structuredContent"]["job"]["jobId"]
        .as_str()
        .unwrap()
        .to_owned();
    let status = mcp.call("jobs", json!({"action": "status", "jobId": job}));
    assert_eq!(status["structuredContent"]["state"], "queued", "{status}");
    let cancelled = mcp.call("jobs", json!({"action": "cancel", "jobId": job}));
    std::fs::remove_file(&gate).unwrap();
    for reply in [status, cancelled] {
        conforms(&reply["structuredContent"], &schema, "jobs").unwrap();
    }
}

#[test]
fn a_cancel_after_the_owner_died_is_pending_from_its_worker_s_start_to_its_exit() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file]);
    let gate = env.gate("start.hold");
    let mut owner = env
        .command(
            &["index", "prepare", file, "--scope", "0:0"],
            Some(("start", &gate)),
        )
        .env("FIGLENS_TEST_ORPHAN", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let reached = env.dir.join("start.hold.reached");
    wait_for("the worker to start", || reached.exists());
    let job = active_job(&env, &fig, "stage");
    owner.kill().unwrap();
    owner.wait().unwrap();
    // Not yet locked: a starting worker is not proof of exit.
    let starting = env.json(&["jobs", "cancel", &job]);
    assert_eq!(starting["cancel"], "pending", "{starting:#}");
    std::fs::remove_file(&gate).unwrap();
    wait_for("the orphaned worker to exit", || {
        env.json(&["jobs", "cancel", &job])["cancel"] == "accepted"
    });
}

#[test]
fn mcp_whose_reply_stream_closes_stops_its_build_and_exits_though_stdin_stays_open() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let gate = env.gate("parse.hold");
    let mut child = env
        .command(&["mcp"], Some(("parse", &gate)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap());
    let mut stdin = child.stdin.take().unwrap();
    let mut send = |message: Value| {
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    };
    send(
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "index",
        "arguments": {"action": "prepare", "file": file, "scope": "0:0"}}}),
    );
    let mut line = String::new();
    lines.read_line(&mut line).unwrap();
    let started: Value = serde_json::from_str(&line).unwrap();
    let job = started["result"]["structuredContent"]["job"]["jobId"]
        .as_str()
        .unwrap()
        .to_owned();
    active_job(&env, &fig, "parse");
    // The reply stream and stderr close; stdin stays open, and then a reply has to be written.
    drop(lines);
    drop(child.stderr.take());
    send(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    wait_for("the server to exit", || child.try_wait().unwrap().is_some());
    let state = env.json(&["jobs", "status", &job])["state"].clone();
    std::fs::remove_file(&gate).unwrap();
    assert_eq!(state, "cancelled");
}

#[test]
fn mcp_whose_reply_stream_closes_mid_submit_cancels_the_job_it_was_admitting() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let gate = env.gate("submit.hold");
    let mut child = env
        .command(&["mcp"], Some(("submit", &gate)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let lines = child.stdout.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut send = |message: Value| {
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    };
    send(
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "index",
        "arguments": {"action": "prepare", "file": fig.to_str().unwrap(), "scope": "0:0"}}}),
    );
    let reached = gate.with_extension("hold.reached");
    wait_for("the submit to admit its job", || reached.exists());
    // The job row exists but its build is not queued yet when the reply stream dies.
    drop(lines);
    send(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    std::thread::sleep(Duration::from_millis(200));
    let early = child.try_wait().unwrap();
    std::fs::remove_file(&gate).unwrap();
    wait_for("the server to exit", || child.try_wait().unwrap().is_some());
    let status = env.json(&["index", "status", "--file", fig.to_str().unwrap()]);
    let db = PathBuf::from(status["paths"]["database"].as_str().unwrap());
    let states: Vec<String> = rusqlite::Connection::open(db)
        .unwrap()
        .prepare("SELECT state FROM jobs")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!((early, states), (None, vec!["cancelled".to_owned()]));
}

#[test]
fn a_cancel_completes_when_the_owner_dies_while_its_worker_is_starting() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file]);
    let gate = env.gate("start.hold");
    // No FIGLENS_TEST_ORPHAN: the worker's stdin watchdog fires once the owner dies.
    let mut owner = env
        .command(
            &["index", "prepare", file, "--scope", "0:0"],
            Some(("start", &gate)),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for("the worker to start", || {
        env.dir.join("start.hold.reached").exists()
    });
    let job = active_job(&env, &fig, "stage");
    owner.kill().unwrap();
    owner.wait().unwrap();
    let starting = env.json(&["jobs", "cancel", &job]);
    assert_eq!(starting["cancel"], "pending", "{starting:#}");
    std::fs::remove_file(&gate).unwrap();
    wait_for("the cancel to complete", || {
        env.json(&["jobs", "cancel", &job])["cancel"] == "accepted"
    });
}

#[test]
fn a_text_only_search_cache_upgrades_once_for_usages_and_keeps_search_cursors() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let mut mcp = Mcp::start(&env, &[]);
    let search = |mcp: &mut Mcp, cursor: Option<&str>| {
        let mut args = json!({"file": file, "query": "first", "limit": 1});
        if let Some(c) = cursor {
            args["cursor"] = json!(c);
        }
        mcp.call("search", args)["structuredContent"].clone()
    };
    let cold = search(&mut mcp, None);
    let kinds = &cold["coverage"]["layers"]["relations"]["kinds"];
    // Ordinary search builds text and structure only; field relations are not built, not empty.
    assert_eq!(
        (
            &cold["coverage"]["complete"],
            &kinds["instance_of"],
            &kinds["binds_variable"]
        ),
        (&json!(true), &json!("ready"), &json!("not_built"))
    );
    assert_eq!(cold["source"]["freshness"]["reusedIndex"], false);
    let usages = |mcp: &mut Mcp| {
        mcp.call(
            "usages",
            json!({"file": file, "node": "0:1", "relation": "contains"}),
        )["structuredContent"]
            .clone()
    };
    // The first usages call rebuilds with every relation, once; the next one reuses it.
    let first = usages(&mut mcp);
    assert_eq!(
        (ids(&first), &first["source"]["freshness"]["reusedIndex"]),
        (vec!["0:2"], &json!(false))
    );
    assert_eq!(usages(&mut mcp)["source"]["freshness"]["reusedIndex"], true);
    // The full index serves search too, with the same results.
    let warm = search(&mut mcp, None);
    assert_eq!(
        (
            &warm["results"],
            &warm["source"]["freshness"]["reusedIndex"]
        ),
        (&cold["results"], &json!(true))
    );
    assert_eq!(
        warm["coverage"]["layers"]["relations"]["kinds"]["binds_variable"],
        "ready"
    );
    // A new revision is read again: same content, another snapshot, fresh text-only build.
    let bytes = std::fs::read(&fig).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::fs::File::create(&fig).unwrap());
    for i in 0..archive.len() {
        out.raw_copy_file(archive.by_index(i).unwrap()).unwrap();
    }
    out.start_file("revision.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    out.write_all(b"2").unwrap();
    out.finish().unwrap();
    let changed = search(&mut mcp, None);
    assert_ne!(
        changed["source"]["snapshotId"],
        cold["source"]["snapshotId"]
    );
    assert_eq!(
        (
            ids(&changed),
            &changed["coverage"]["layers"]["relations"]["kinds"]["binds_variable"]
        ),
        (ids(&cold), &json!("not_built"))
    );
    let after = usages(&mut mcp);
    assert_eq!(
        (ids(&after), &after["source"]["snapshotId"]),
        (vec!["0:2"], &changed["source"]["snapshotId"])
    );
    drop(mcp);
}

#[path = "../scripts/make-corpus.rs"]
#[allow(dead_code)]
mod corpus;

#[test]
fn diff_text_lists_a_page_points_at_the_next_and_names_the_missing_side() {
    let env = Env::new();
    let [old, new] = [("old.fig", false), ("new.fig", true)].map(|(name, revised)| {
        let path = env.dir.join(name);
        corpus::write(path.as_os_str(), revised).unwrap();
        path.to_str().unwrap().to_owned()
    });
    let page = env.run(&["diff", &old, &new, "--limit", "1"]);
    assert!(page.status.success());
    let text = String::from_utf8_lossy(&page.stdout);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(
        lines[0].contains(" changed: ")
            || lines[0].contains(" added: ")
            || lines[0].contains(" removed: "),
        "{text}"
    );
    assert!(lines[1].starts_with("more: --cursor "), "{text}");
    let same = env.run(&["diff", &old, &old]);
    assert_eq!(String::from_utf8_lossy(&same.stdout), "no changes\n");
    let missing = env.dir.join("missing.fig");
    let gone = env.run(&["diff", &old, missing.to_str().unwrap()]);
    assert!(!gone.status.success());
    let stderr = String::from_utf8_lossy(&gone.stderr);
    assert!(
        stderr.contains("missing.fig") && !stderr.contains("old.fig"),
        "{stderr}"
    );
}

#[test]
fn diff_of_two_revisions_pages_the_same_changes_over_the_cli_and_mcp() {
    let env = Env::new();
    let [old, new] = [("old.fig", false), ("new.fig", true)].map(|(name, revised)| {
        let path = env.dir.join(name);
        corpus::write(path.as_os_str(), revised).unwrap();
        std::fs::canonicalize(path).unwrap()
    });
    let (old, new) = (old.to_str().unwrap(), new.to_str().unwrap());
    let cli = env.json(&["diff", old, new, "--json"]);
    let changes = |reply: &Value| {
        let mut all: Vec<(String, String)> = reply["frames"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|f| f["nodes"].as_array().unwrap())
            .map(|n| {
                (
                    n["id"].as_str().unwrap().into(),
                    n["change"].as_str().unwrap().into(),
                )
            })
            .collect();
        all.sort();
        all
    };
    assert_eq!(
        changes(&cli),
        [
            ("0:1", "changed"),
            ("0:11", "changed"),
            ("0:13", "removed"),
            ("0:14", "added")
        ]
        .map(|(id, c)| (id.to_owned(), c.to_owned()))
    );
    assert_eq!(cli["coverage"]["complete"], true);
    // One change per MCP page: the cached pair pages through the same list.
    let mut mcp = Mcp::start(&env, &[]);
    let (mut paged, mut cursor) = (Vec::new(), Value::Null);
    loop {
        let mut args = json!({"old": old, "file": new, "limit": 1});
        if !cursor.is_null() {
            args["cursor"] = cursor.clone();
        }
        let reply = mcp.call("diff", args)["structuredContent"].clone();
        paged.extend(changes(&reply));
        cursor = reply["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    paged.sort();
    assert_eq!(paged, changes(&cli));
    // Stateless, like search: no database.
    assert!(!env.home.join(".local/state/figlens").exists());
}

#[test]
fn only_a_tracked_scope_s_build_stores_facets() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let prepared = env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let db = prepared["paths"]["database"].as_str().unwrap().to_owned();
    let count = |q: &str| {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.query_row(q, [], |r| r.get::<_, i64>(0)).unwrap()
    };
    assert_eq!(
        count("SELECT count(*) FROM facet_sets") + count("SELECT count(*) FROM facets"),
        0
    );
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "INSERT INTO tracking (source_id, scope, state, epoch, facet_version, enabled_ms)
             SELECT source_id, '0:0', 'active', 1, 'v', 0 FROM sources",
            [],
        )
        .unwrap();
    let rebuilt = env.json(&["index", "rebuild", "--file", file]);
    let gen_id = rebuilt["jobs"][0]["generation"].as_i64().unwrap();
    assert_eq!(
        [
            count(&format!(
                "SELECT count(*) FROM facet_sets WHERE gen = {gen_id}"
            )),
            count(&format!("SELECT count(*) FROM facets WHERE gen = {gen_id}")),
        ],
        [
            1,
            count(&format!(
                "SELECT count(*) FROM records WHERE gen = {gen_id}"
            ))
        ]
    );
    // Facets live and go with their generation; none outlive it.
    let again = env.json(&["index", "rebuild", "--file", file]);
    let gen_id = again["jobs"][0]["generation"].as_i64().unwrap();
    assert_eq!(
        count("SELECT count(*) FROM facets WHERE gen NOT IN (SELECT gen FROM generations)"),
        0
    );
    assert_eq!(count("SELECT max(gen) FROM facet_sets"), gen_id);
}

#[test]
fn track_builds_a_baseline_with_facets_and_each_state_change_bumps_the_epoch() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    let prepared = env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let db = prepared["paths"]["database"].as_str().unwrap().to_owned();
    // Whether the generation a reply's job built stored facets.
    let faceted = |reply: &Value| {
        let gen_id = reply["jobs"][0]["generation"].as_i64().unwrap();
        let conn = rusqlite::Connection::open(&db).unwrap();
        let q = "SELECT EXISTS (SELECT 1 FROM facet_sets WHERE gen = ?1)";
        conn.query_row(q, [gen_id], |r| r.get::<_, bool>(0))
            .unwrap()
    };
    let track = |action: &str| env.json(&["index", action, "--file", file, "--scope", "0:0"]);
    let state = |reply: &Value| {
        (
            reply["tracking"][0]["state"].clone(),
            reply["tracking"][0]["epoch"].clone(),
        )
    };
    let on = track("track");
    assert_eq!(state(&on), (json!("baseline_pending"), json!(1)));
    assert_eq!(on["jobs"][0]["state"], "succeeded");
    assert!(faceted(&on));
    // The baseline's publication made tracking active; tracking again changes nothing and
    // builds nothing.
    let again = track("track");
    assert_eq!(
        (state(&again), again["jobs"].clone()),
        ((json!("active"), json!(1)), json!([]))
    );
    assert_eq!(state(&track("untrack")), (json!("disabled"), json!(2)));
    assert!(!faceted(&env.json(&["index", "rebuild", "--file", file])));
    let retracked = track("track");
    assert_eq!(state(&retracked), (json!("baseline_pending"), json!(3)));
    assert!(faceted(&retracked));
    let out = env.run(&["index", "track", "--file", file, "--scope", "0:99"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no published scope 0:99"));
}

/// A corpus file prepared at scope 0:0 and tracked, with a save that replaces it and the
/// workspace database's path.
struct Tracked<'a> {
    env: &'a Env,
    path: PathBuf,
    file: String,
    db: String,
}

impl<'a> Tracked<'a> {
    fn new(env: &'a Env) -> Self {
        let path = env.dir.join("a.fig");
        corpus::write(path.as_os_str(), false).unwrap();
        let path = std::fs::canonicalize(path).unwrap();
        let file = path.to_str().unwrap().to_owned();
        let prepared = env.json(&["index", "prepare", &file, "--scope", "0:0"]);
        let db = prepared["paths"]["database"].as_str().unwrap().to_owned();
        let t = Self {
            env,
            path,
            file,
            db,
        };
        t.index("track");
        t
    }

    fn index(&self, action: &str) -> Value {
        self.env
            .json(&["index", action, "--file", &self.file, "--scope", "0:0"])
    }

    /// Replaces the file, as Figma's save does.
    fn save(&self, revised: bool) {
        let next = self.env.dir.join("next.fig");
        corpus::write(next.as_os_str(), revised).unwrap();
        std::fs::rename(&next, &self.path).unwrap();
    }

    fn sql(&self, q: &str) {
        rusqlite::Connection::open(&self.db)
            .unwrap()
            .execute_batch(q)
            .unwrap();
    }

    /// Each revision: kind, coverage, change count, detail, whether it has a from generation,
    /// its rows and how many facet sets its from generation still has.
    fn revisions(&self) -> Vec<Value> {
        let conn = rusqlite::Connection::open(&self.db).unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT r.kind, r.coverage, r.summary, r.detail, r.from_gen,
                 (SELECT group_concat(node || '=' || change, ',') FROM
                  (SELECT node, change FROM revision_changes WHERE rev = r.id ORDER BY node)),
                 (SELECT COUNT(*) FROM facet_sets WHERE gen = r.from_gen)
                 FROM revisions r ORDER BY seq",
            )
            .unwrap();
        let text = |s: String| serde_json::from_str::<Value>(&s).unwrap();
        stmt.query_map([], |r| {
            Ok(json!([
                r.get::<_, String>(0)?,
                text(r.get(1)?),
                text(r.get(2)?)["changes"],
                r.get::<_, String>(3)?,
                r.get::<_, Option<i64>>(4)?.is_some(),
                r.get::<_, Option<String>>(5)?,
                r.get::<_, i64>(6)?
            ]))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    }
}

#[test]
fn a_tracked_scope_records_a_baseline_then_each_published_change_and_untracking_keeps_history() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let tracked = t.revisions();
    t.index("untrack");
    t.save(false);
    t.index("refresh");
    // The changes read like a live diff of the two saves; the replaced generation keeps no
    // facets; an untracked save records nothing and the history stays.
    assert_eq!(
        (tracked.clone(), t.revisions()),
        (
            vec![
                json!(["baseline", {"basis": "baseline", "reason": "tracking_started"}, null, "complete", false, null, 0]),
                json!([
                    "changes",
                    tracked[1][1],
                    4,
                    "complete",
                    true,
                    "0:1=changed,0:11=changed,0:13=removed,0:14=added",
                    0
                ]),
            ],
            tracked.clone()
        )
    );
    assert_eq!(tracked[1][1]["complete"], true);
}

#[test]
fn a_failed_comparison_still_publishes_and_a_new_field_layout_is_a_baseline() {
    let env = Env::new();
    let t = Tracked::new(&env);
    // Unreadable facets of the generation being replaced: the save publishes, never unchanged.
    t.sql("UPDATE facets SET blob = CAST('bogus' AS BLOB) WHERE gen = (SELECT gen FROM published)");
    t.save(true);
    t.index("refresh");
    t.sql(
        "UPDATE facet_sets SET facet_version = 'diff-v0:0' WHERE gen = (SELECT gen FROM published)",
    );
    t.save(false);
    t.index("refresh");
    let got: Vec<_> = t.revisions()[1..]
        .iter()
        .map(|r| {
            (
                r[0].clone(),
                r[1]["reasons"][0]["reason"].clone(),
                r[1]["reason"].clone(),
                r[3].clone(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                json!("changes"),
                json!("comparison_failed"),
                json!(null),
                json!("truncated")
            ),
            (
                json!("baseline"),
                json!(null),
                json!("facet_version_changed"),
                json!("complete")
            ),
        ]
    );
}

#[test]
fn a_tracking_change_after_the_comparison_records_a_baseline_not_a_gap() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    let gate = env.gate("compare.hold");
    let mut owner = env.spawn(
        &["index", "refresh", "--file", &t.file],
        Some(("compare", &gate)),
    );
    let reached = PathBuf::from(format!("{}.reached", gate.display()));
    wait_for("the comparison", || reached.exists());
    t.sql("UPDATE tracking SET epoch = epoch + 1");
    std::fs::remove_file(&gate).unwrap();
    assert!(owner.wait().unwrap().success());
    // The generation still publishes, as a baseline: the next comparison starts from it.
    assert_eq!(
        (
            t.revisions().last().unwrap()[1]["reason"].clone(),
            env.json(&["search", "--file", &t.file, "first"])["state"].clone()
        ),
        (json!("tracking_changed"), json!("ready"))
    );
}

#[test]
fn a_tracking_change_after_the_worker_compared_is_compared_again_not_published_stale() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    // The worker compares before its verify stage, so this holds it with its comparison made.
    let gate = env.gate("verify.hold");
    let mut owner = env.spawn(
        &["index", "refresh", "--file", &t.file],
        Some(("verify", &gate)),
    );
    let reached = PathBuf::from(format!("{}.reached", gate.display()));
    wait_for("the worker's comparison", || reached.exists());
    t.sql("UPDATE tracking SET epoch = epoch + 1");
    std::fs::remove_file(&gate).unwrap();
    assert!(owner.wait().unwrap().success());
    // A stale comparison would record a `tracking_changed` baseline; the owner's own is current.
    assert_eq!(t.revisions().last().unwrap()[0], json!("changes"));
}

#[test]
fn a_cancel_while_the_worker_compares_stops_it_and_records_nothing() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let before = t.revisions().len();
    t.save(true);
    let gate = env.gate("plan.hold");
    let mut owner = env.spawn(
        &["index", "refresh", "--file", &t.file],
        Some(("plan", &gate)),
    );
    // The comparison runs at the end of `write`.
    let job = active_job(&env, &t.path, "write");
    wait_for("the worker's comparison", || {
        PathBuf::from(format!("{}.reached", gate.display())).exists()
    });
    let reply = env.json(&["jobs", "cancel", &job]);
    let gone = !worker_running(&env, &job);
    assert!(!owner.wait().unwrap().success());
    std::fs::remove_file(&gate).unwrap();
    assert_eq!(
        (reply["state"].clone(), gone, t.revisions().len()),
        (json!("cancelled"), true, before)
    );
}

#[test]
fn relocating_a_tracked_source_keeps_its_history_and_removing_it_deletes_it() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let moved = env.dir.join("moved.fig");
    std::fs::copy(&t.path, &moved).unwrap();
    let moved = std::fs::canonicalize(moved).unwrap();
    let ws = env.json(&["index", "status", "--file", &t.file]);
    let (id, source) = (
        ws["workspaceId"].as_str().unwrap().to_owned(),
        ws["sources"][0]["sourceId"].as_str().unwrap().to_owned(),
    );
    env.json(&[
        "index",
        "relocate",
        "--id",
        &id,
        &source,
        moved.to_str().unwrap(),
    ]);
    let relocated = t.revisions().len();
    env.json(&["index", "remove", "--id", &id, &source]);
    let count = |table: &str| {
        rusqlite::Connection::open(&t.db)
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
    };
    assert_eq!(
        (relocated, count("revisions"), count("tracking")),
        (1, 0, 0)
    );
}

#[test]
fn tracking_during_a_build_that_read_it_off_is_refused_and_tracking_after_builds_the_baseline() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let gate = env.gate("write.hold");
    let mut build = env.spawn(
        &["index", "rebuild", "--file", file],
        Some(("write", &gate)),
    );
    let reached = PathBuf::from(format!("{}.reached", gate.display()));
    wait_for("the build to pass its tracking read", || reached.exists());
    // A pending baseline enabled in the same millisecond the build was created (or by a clock
    // stepped back), though the build was admitted before it and read tracking off.
    let status = env.json(&["index", "status", "--file", file]);
    rusqlite::Connection::open(status["paths"]["database"].as_str().unwrap())
        .unwrap()
        .execute_batch(
            "INSERT INTO tracking (source_id, scope, state, epoch, facet_version, enabled_ms,
             enabled_seq) SELECT source_id, scope, 'baseline_pending', 1, '', created_ms, seq
             FROM jobs WHERE state = 'running'",
        )
        .unwrap();
    let refused = env.run(&["index", "track", "--file", file, "--scope", "0:0"]);
    std::fs::remove_file(&gate).unwrap();
    assert!(build.wait().unwrap().success());
    let tracked = env.json(&["index", "track", "--file", file, "--scope", "0:0"]);
    let state = env.json(&["index", "track", "--file", file, "--scope", "0:0"]);
    assert_eq!(
        (
            String::from_utf8_lossy(&refused.stderr).contains("track after it ends"),
            tracked["jobs"][0]["state"].clone(),
            state["tracking"][0]["state"].clone()
        ),
        (true, json!("succeeded"), json!("active"))
    );
}

#[test]
fn tracking_after_a_build_s_owner_died_takes_the_build_over_as_the_baseline() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let gate = env.gate("write.hold");
    let mut owner = env.spawn(
        &["index", "rebuild", "--file", file],
        Some(("write", &gate)),
    );
    let job = active_job(&env, &fig, "write");
    owner.kill().unwrap();
    owner.wait().unwrap();
    wait_for("the orphaned worker to exit", || {
        !worker_running(&env, &job)
    });
    std::fs::remove_file(&gate).unwrap();
    // Stored as running though nobody runs it: waiting would never end.
    let tracked = env.json(&["index", "track", "--file", file, "--scope", "0:0"]);
    let state = env.json(&["index", "track", "--file", file, "--scope", "0:0"]);
    assert_eq!(
        (
            tracked["jobs"][0]["state"].clone(),
            state["tracking"][0]["state"].clone()
        ),
        (json!("succeeded"), json!("active"))
    );
}

#[test]
fn a_track_request_id_used_by_a_rebuild_is_refused_before_tracking_changes() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let scope = ["--scope", "0:0"];
    env.json(
        &[
            &["index", "rebuild", "--file", file][..],
            &scope,
            &["--request-id", "x"],
        ]
        .concat(),
    );
    let refused = env.run(
        &[
            &["index", "track", "--file", file][..],
            &scope,
            &["--request-id", "x"],
        ]
        .concat(),
    );
    let untouched = env.json(&["index", "untrack", "--file", file, "--scope", "0:0"]);
    let tracked = env.json(
        &[
            &["index", "track", "--file", file][..],
            &scope,
            &["--request-id", "y"],
        ]
        .concat(),
    );
    let replayed = env.json(
        &[
            &["index", "track", "--file", file][..],
            &scope,
            &["--request-id", "y"],
        ]
        .concat(),
    );
    assert_eq!(
        (
            String::from_utf8_lossy(&refused.stderr).contains("used for a different request"),
            untouched["tracking"][0]["state"].clone(),
            tracked["tracking"][0]["state"].clone(),
            tracked["jobs"][0]["state"].clone(),
            replayed["tracking"][0]["state"].clone(),
        ),
        (
            true,
            json!("untracked"),
            json!("baseline_pending"),
            json!("succeeded"),
            json!("active")
        )
    );
}

#[test]
fn a_repeated_track_while_its_baseline_build_runs_joins_it() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let gate = env.gate("write.hold");
    let first = env.spawn(
        &["index", "track", "--file", file, "--scope", "0:0"],
        Some(("write", &gate)),
    );
    let reached = PathBuf::from(format!("{}.reached", gate.display()));
    wait_for("the baseline build to pass its tracking read", || {
        reached.exists()
    });
    let again = env.spawn(&["index", "track", "--file", file, "--scope", "0:0"], None);
    std::thread::sleep(Duration::from_millis(300));
    std::fs::remove_file(&gate).unwrap();
    let [first, again] = [first, again].map(|c| {
        let out = c.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    });
    let status = env.json(&["index", "track", "--file", file, "--scope", "0:0"]);
    assert_eq!(
        (
            again["jobs"][0]["jobId"] == first["jobs"][0]["jobId"],
            status["tracking"][0]["state"].clone()
        ),
        (true, json!("active"))
    );
}

impl Tracked<'_> {
    fn changes(&self, more: &[&str]) -> Value {
        let mut args = vec!["changes", "--file", &self.file];
        args.extend(more);
        self.env.json(&args)
    }

    fn fails(&self, args: &[&str]) -> String {
        let out = self.env.run(args);
        assert!(!out.status.success());
        String::from_utf8_lossy(&out.stderr).into_owned()
    }
}

#[test]
fn changes_pages_revisions_and_their_nodes_and_a_consumer_acks_its_place() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let all = t.changes(&[]);
    let first = t.changes(&["--limit", "1"]);
    let cursor = first["nextCursor"].as_str().unwrap();
    let rest = t.changes(&["--cursor", cursor]);
    let id = rest["revisions"][0]["revisionId"].as_str().unwrap();
    let nodes = t.changes(&["--revision", id, "--limit", "3"]);
    let more = t.changes(&[
        "--revision",
        id,
        "--cursor",
        nodes["nextCursor"].as_str().unwrap(),
    ]);
    let fresh = t.changes(&["--consumer", "bot"]);
    let ack = |c: &str| {
        env.json(&[
            "watch",
            "ack",
            "--file",
            &t.file,
            "--consumer",
            "bot",
            "--cursor",
            c,
        ])
    };
    let acked = ack(cursor);
    let after = t.changes(&["--consumer", "bot"]);
    let again = ack(cursor);
    assert_eq!(
        (
            all["revisions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["kind"].clone())
                .collect::<Vec<_>>(),
            (first["hasMore"].clone(), rest["hasMore"].clone()),
            rest["revisions"][0]["from"] == first["revisions"][0]["revisionId"],
            (
                nodes["nodes"].as_array().unwrap().len(),
                more["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| (n["entry"].clone(), n["sides"].clone()))
                    .collect::<Vec<_>>(),
                more["nextCursor"].clone(),
                rest["revisions"][0]["impact"].clone()
            ),
            (
                fresh["revisions"].as_array().unwrap().len(),
                after["revisions"][0]["revisionId"].clone()
            ),
            (
                acked["advanced"].clone(),
                again["advanced"].clone(),
                again["checkpoint"] == json!(cursor)
            ),
            (
                all["tracking"][0]["state"].clone(),
                all["tracking"][0]["refreshRequired"].clone(),
                all["monitor"].clone()
            ),
        ),
        (
            vec![json!("baseline"), json!("changes")],
            (json!(true), json!(false)),
            true,
            // The last change row, then the layers its changes reach: the added and removed
            // ones on the side they exist in.
            (
                3,
                vec![
                    (json!("change"), Value::Null),
                    (json!("impact"), json!(["old", "new"])),
                    (json!("impact"), json!(["old", "new"])),
                    (json!("impact"), json!(["old"])),
                    (json!("impact"), json!(["new"]))
                ],
                Value::Null,
                json!({"complete": true, "listed": {"direct": 4}, "reasons": []})
            ),
            (2, json!(id)),
            (json!(true), json!(false), true),
            (json!("active"), json!(false), json!({"state": "inactive"})),
        )
    );
}

#[test]
fn a_cursor_for_other_filters_is_stale_and_a_position_retention_deleted_is_a_gap() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    t.save(false);
    t.index("refresh");
    let page = t.changes(&["--scope", "0:0", "--limit", "1"]);
    let cursor = page["nextCursor"].as_str().unwrap();
    let source = page["revisions"][0]["sourceId"].as_str().unwrap();
    let unscoped = t.changes(&["--limit", "1"]);
    let unscoped = unscoped["nextCursor"].as_str().unwrap();
    let stale = t.fails(&["changes", "--file", &t.file, "--cursor", cursor]);
    env.json(&[
        "watch",
        "ack",
        "--file",
        &t.file,
        "--scope",
        "0:0",
        "--consumer",
        "bot",
        "--cursor",
        cursor,
    ]);
    // Retention deleted the revision the cursor and the checkpoint name, and the next, unread.
    t.sql(&format!(
        "DELETE FROM revisions WHERE seq < (SELECT MAX(seq) FROM revisions);
         UPDATE meta SET value = CAST(value AS INTEGER) + 1 WHERE key = 'revision_seq';
         INSERT INTO meta SELECT 'pruned' || char(31) || '{source}' || char(31) || '0:0',
         (MAX(seq) - 1) || ' ' || (SELECT value FROM meta WHERE key = 'revision_seq') FROM revisions"
    ));
    let gap = t.fails(&[
        "changes", "--file", &t.file, "--scope", "0:0", "--cursor", cursor,
    ]);
    let behind = t.fails(&[
        "changes",
        "--file",
        &t.file,
        "--scope",
        "0:0",
        "--consumer",
        "bot",
    ]);
    // Another scope pruned past everything this one keeps must not hide what it keeps.
    t.sql(&format!(
        "UPDATE meta SET value = CAST(value AS INTEGER) + 1 WHERE key = 'revision_seq';
         INSERT INTO meta SELECT 'pruned' || char(31) || '{source}' || char(31) || '9:9',
         '100' || ' ' || (SELECT value FROM meta WHERE key = 'revision_seq')"
    ));
    let mixed = t.fails(&["changes", "--file", &t.file, "--cursor", unscoped]);
    // Each recovery runs exactly as given.
    let resume = |stderr: &str| {
        let r: Value = serde_json::from_str(stderr.lines().last().unwrap()).unwrap();
        let a = &r["recovery"][0]["arguments"];
        let mut args = vec![
            "changes".to_owned(),
            "--id".into(),
            a["workspaceId"].as_str().unwrap().into(),
        ];
        for (key, flag) in [
            ("sourceId", "--source"),
            ("scope", "--scope"),
            ("cursor", "--cursor"),
        ] {
            if let Some(v) = a[key].as_str() {
                args.extend([flag.to_owned(), v.to_owned()]);
            }
        }
        env.json(&args.iter().map(String::as_str).collect::<Vec<_>>())["revisions"]
            .as_array()
            .unwrap()
            .len()
    };
    assert_eq!(
        (
            stale.contains("other filters"),
            behind.contains("deleted by retention"),
            resume(&gap),
            resume(&mixed),
        ),
        (true, true, 1, 1)
    );
}

#[test]
fn a_kept_cursor_before_another_scopes_deleted_revisions_is_a_gap_read_past_once() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let page = t.changes(&["--limit", "1"]);
    let cursor = page["nextCursor"].as_str().unwrap();
    let source = page["revisions"][0]["sourceId"].as_str().unwrap();
    // Retention deleted another scope's revisions recorded after the cursor's, which is kept.
    t.sql(&format!(
        "UPDATE meta SET value = CAST(value AS INTEGER) + 1 WHERE key = 'revision_seq';
         INSERT INTO meta SELECT 'pruned' || char(31) || '{source}' || char(31) || '9:9',
         '100' || ' ' || (SELECT value FROM meta WHERE key = 'revision_seq')"
    ));
    let gap = t.fails(&["changes", "--file", &t.file, "--cursor", cursor]);
    // The recovery accepts that deletion: its pages read on past it, not into the gap again.
    let r: Value = serde_json::from_str(gap.lines().last().unwrap()).unwrap();
    let oldest = r["recovery"][0]["arguments"]["cursor"].as_str().unwrap();
    let first = t.changes(&["--cursor", oldest, "--limit", "1"]);
    let next = t.changes(&["--cursor", first["nextCursor"].as_str().unwrap()]);
    assert_eq!(next["revisions"].as_array().unwrap().len(), 1);
}

#[test]
fn a_hidden_source_s_publication_changes_no_visible_cursor_or_ack() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let ack = |c: &str| {
        env.json(&[
            "watch",
            "ack",
            "--file",
            &t.file,
            "--consumer",
            "bot",
            "--cursor",
            c,
        ])
    };
    let before = t.changes(&[]);
    let cursor = before["nextCursor"].as_str().unwrap();
    ack(cursor);
    // Another source's publication moves the global seq and prunes there.
    t.sql(
        "UPDATE meta SET value = CAST(value AS INTEGER) + 5 WHERE key = 'revision_seq';
         INSERT INTO meta SELECT 'pruned' || char(31) || 'hidden' || char(31) || '0:0',
         '3' || ' ' || (SELECT value FROM meta WHERE key = 'revision_seq')",
    );
    let after = t.changes(&[]);
    let replay = ack(cursor);
    assert_eq!(
        (
            after["nextCursor"] == before["nextCursor"],
            cursor.len(),
            replay["advanced"].clone(),
            replay["checkpoint"] == json!(cursor),
        ),
        // `c1.`, the id and receipt, and the MAC, in hex: no counter.
        (true, 3 + (16 + 16 + 16) * 2, json!(false), true)
    );
}

#[test]
fn a_stale_ack_behind_a_new_deletion_is_a_gap_and_writes_nothing() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let page = t.changes(&["--limit", "1"]);
    let cursor = page["nextCursor"].as_str().unwrap();
    let source = page["revisions"][0]["sourceId"].as_str().unwrap();
    t.sql(&format!(
        "UPDATE meta SET value = CAST(value AS INTEGER) + 1 WHERE key = 'revision_seq';
         INSERT INTO meta SELECT 'pruned' || char(31) || '{source}' || char(31) || '9:9',
         '100' || ' ' || (SELECT value FROM meta WHERE key = 'revision_seq')"
    ));
    let refused = t.fails(&[
        "watch",
        "ack",
        "--file",
        &t.file,
        "--consumer",
        "bot",
        "--cursor",
        cursor,
    ]);
    let fresh = t.changes(&["--consumer", "bot"]);
    assert_eq!(
        (
            refused.contains("deleted by retention"),
            fresh["revisions"].as_array().unwrap().len()
        ),
        (true, 2)
    );
}

#[test]
fn a_current_cursor_before_a_checkpoint_behind_a_gap_moves_it_back_to_repeat_not_skip() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let ack = |consumer: &str, c: &str| {
        env.json(&[
            "watch",
            "ack",
            "--file",
            &t.file,
            "--consumer",
            consumer,
            "--cursor",
            c,
        ])
    };
    let all = t.changes(&[]);
    ack("bot", all["nextCursor"].as_str().unwrap());
    ack("tail", all["nextCursor"].as_str().unwrap());
    let source = all["revisions"][0]["sourceId"].as_str().unwrap();
    // Another scope's unread revisions after the checkpoint were deleted.
    t.sql(&format!(
        "UPDATE meta SET value = CAST(value AS INTEGER) + 1 WHERE key = 'revision_seq';
         INSERT INTO meta SELECT 'pruned' || char(31) || '{source}' || char(31) || '9:9',
         '100' || ' ' || (SELECT value FROM meta WHERE key = 'revision_seq')"
    ));
    let gap = t.fails(&["changes", "--file", &t.file, "--consumer", "bot"]);
    let r: Value = serde_json::from_str(gap.lines().last().unwrap()).unwrap();
    let oldest = r["recovery"][0]["arguments"]["cursor"].as_str().unwrap();
    // The first retained page lies before the checkpoint; any current cursor there, from the
    // recovery or an unrelated read, moves the checkpoint back.
    let first = t.changes(&["--cursor", oldest, "--limit", "1"]);
    let unrelated = t.changes(&["--limit", "1"]);
    let acked = ack("bot", unrelated["nextCursor"].as_str().unwrap());
    let resumed = t.changes(&["--consumer", "bot"]);
    // Reading the recovery up to the checkpoint and acking there ends its gap.
    let through = t.changes(&["--cursor", oldest]);
    let ended = ack("tail", through["nextCursor"].as_str().unwrap());
    let caught_up = t.changes(&["--consumer", "tail"]);
    let kept = all["revisions"].as_array().unwrap().len();
    assert_eq!(
        (
            first["nextCursor"] == unrelated["nextCursor"],
            acked["advanced"].clone(),
            acked["checkpoint"] == first["nextCursor"],
            resumed["revisions"].as_array().unwrap().len(),
            (
                ended["advanced"].clone(),
                ended["checkpoint"] == through["nextCursor"]
            ),
            caught_up["revisions"].as_array().unwrap().len(),
        ),
        (true, json!(false), true, kept - 1, (json!(false), true), 0)
    );
}

#[test]
fn a_cursor_past_a_new_deletion_or_in_another_scope_reads_on() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let tail = t.changes(&[]);
    let scoped = t.changes(&["--scope", "0:0"]);
    let source = tail["revisions"][0]["sourceId"].as_str().unwrap();
    let prune = |scope: &str, seq: u32| {
        t.sql(&format!(
            "UPDATE meta SET value = CAST(value AS INTEGER) + 1 WHERE key = 'revision_seq';
             INSERT INTO meta SELECT 'pruned' || char(31) || '{source}' || char(31) || '{scope}',
             '{seq}' || ' ' || (SELECT value FROM meta WHERE key = 'revision_seq')"
        ))
    };
    // A deletion behind every position read, then one in a scope `0:0` is a prefix of.
    prune("9:9", 1);
    let at_tail = t.changes(&["--cursor", tail["nextCursor"].as_str().unwrap()]);
    prune("0:01", 100);
    let in_scope = t.changes(&[
        "--scope",
        "0:0",
        "--cursor",
        scoped["nextCursor"].as_str().unwrap(),
    ]);
    assert_eq!(
        (
            at_tail["revisions"].as_array().unwrap().len(),
            in_scope["revisions"].as_array().unwrap().len(),
            // The other scope's mark is not this view's: its receipt is unchanged.
            t.changes(&["--scope", "0:0"])["nextCursor"] == scoped["nextCursor"]
        ),
        (0, 0, true)
    );
}

#[test]
fn retention_marks_past_their_allowance_are_refused_and_hidden_ones_cost_nothing() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let source = t.changes(&[])["revisions"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let insert = |source: &str| {
        t.sql(&format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1200)
             INSERT INTO meta SELECT 'pruned' || char(31) || '{source}' || char(31) || i || ':'
             || printf('%.900c', 'x'), '1 1' FROM n"
        ))
    };
    insert("hidden");
    let hidden = t.changes(&[]);
    insert(&source);
    let refused = t.fails(&["changes", "--file", &t.file]);
    let narrower = t.changes(&["--scope", "0:0"]);
    assert_eq!(
        (
            hidden["revisions"].as_array().unwrap().len(),
            refused.contains("pass a narrower sourceId or scope"),
            narrower["revisions"].as_array().unwrap().len(),
        ),
        (1, true, 1)
    );
}

#[test]
fn changes_and_ack_refuse_limits_and_consumers_out_of_range() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let page = t.changes(&[]);
    let id = page["revisions"][0]["revisionId"].as_str().unwrap();
    let cursor = page["nextCursor"].as_str().unwrap();
    let long = "c".repeat(129);
    let refused = [
        t.fails(&["changes", "--file", &t.file, "--limit", "51"]),
        t.fails(&[
            "changes",
            "--file",
            &t.file,
            "--revision",
            id,
            "--limit",
            "501",
        ]),
        t.fails(&[
            "watch",
            "ack",
            "--file",
            &t.file,
            "--consumer",
            &long,
            "--cursor",
            cursor,
        ]),
    ];
    assert_eq!(
        refused
            .each_ref()
            .map(|e| e.contains("limit must be 1 to") || e.contains("pass consumer")),
        [true; 3],
        "{refused:?}"
    );
}

#[test]
fn changes_names_a_scope_as_track_does() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let page = t.changes(&["--scope", "0-0"]);
    assert_eq!(page["revisions"].as_array().unwrap().len(), 1);
}

#[test]
fn a_deleted_revision_reads_as_a_gap_and_a_checkpoint_on_it_as_null() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let page = t.changes(&[]);
    let id = page["revisions"][0]["revisionId"].as_str().unwrap();
    let ack = |c: &str| {
        env.json(&[
            "watch",
            "ack",
            "--file",
            &t.file,
            "--consumer",
            "bot",
            "--cursor",
            c,
        ])
    };
    ack(page["nextCursor"].as_str().unwrap());
    let source = page["revisions"][0]["sourceId"].as_str().unwrap();
    t.sql(&format!(
        "DELETE FROM revisions;
         UPDATE meta SET value = CAST(value AS INTEGER) + 1 WHERE key = 'revision_seq';
         INSERT INTO meta SELECT 'pruned' || char(31) || '{source}' || char(31) || '0:0',
         '1' || ' ' || (SELECT value FROM meta WHERE key = 'revision_seq')"
    ));
    let detail = t.fails(&["changes", "--file", &t.file, "--revision", id]);
    let r: Value = serde_json::from_str(detail.lines().last().unwrap()).unwrap();
    let oldest = r["recovery"][0]["arguments"]["cursor"].as_str().unwrap();
    assert_eq!(
        (
            detail.contains("deleted by retention"),
            ack(oldest)["checkpoint"].clone()
        ),
        (true, Value::Null)
    );
}

#[test]
fn mcp_changes_shows_only_visible_sources_and_shares_cli_cursors() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let cli = t.changes(&["--limit", "1"]);
    let ws = cli["workspaceId"].clone();
    let (cursor, id) = (
        cli["nextCursor"].clone(),
        cli["revisions"][0]["revisionId"].clone(),
    );
    let other = env.dir.join("other");
    std::fs::create_dir(&other).unwrap();
    let other = std::fs::canonicalize(other).unwrap();
    let root = std::fs::canonicalize(env.dir.join("")).unwrap();
    let mut seen = Mcp::start(&env, &["--root", root.to_str().unwrap()]);
    let next = seen.call("changes", json!({"workspaceId": ws, "cursor": cursor}));
    let acked = seen.call(
        "watch",
        json!({"action": "ack", "workspaceId": ws, "consumer": "bot", "cursor": next["structuredContent"]["nextCursor"]}),
    );
    let mut hidden = Mcp::start(&env, &["--root", other.to_str().unwrap()]);
    let empty = hidden.call("changes", json!({"workspaceId": ws}));
    let stale = hidden.call("changes", json!({"workspaceId": ws, "cursor": cursor}));
    let detail = hidden.call("changes", json!({"workspaceId": ws, "revision": id}));
    let code = |r: &Value| r["_meta"]["figlens"]["code"].clone();
    assert_eq!(
        (
            next["structuredContent"]["revisions"][0]["kind"].clone(),
            acked["structuredContent"]["advanced"].clone(),
            (
                empty["structuredContent"]["revisions"].clone(),
                empty["structuredContent"]["tracking"].clone()
            ),
            (code(&stale), code(&detail)),
        ),
        (
            json!("changes"),
            json!(true),
            (json!([]), json!([])),
            (json!("cursor_stale"), json!("invalid_arguments")),
        )
    );
}

#[test]
fn a_consumer_checkpoint_is_kept_per_view_so_a_narrower_view_skips_nothing() {
    let env = Env::new();
    for d in ["project", "in", "out"] {
        std::fs::create_dir(env.dir.join(d)).unwrap();
    }
    let dir = std::fs::canonicalize(env.dir.join("project")).unwrap();
    let dir = dir.to_str().unwrap();
    for name in ["in/a.fig", "out/b.fig"] {
        let fig = env.fig(name);
        env.json(&[
            "index",
            "prepare",
            fig.to_str().unwrap(),
            "--workspace",
            dir,
            "--scope",
            "0:0",
        ]);
    }
    env.json(&["index", "track", "--workspace", dir, "--scope", "0:0"]);
    let all = env.json(&["changes", "--workspace", dir]);
    env.json(&[
        "watch",
        "ack",
        "--workspace",
        dir,
        "--consumer",
        "bot",
        "--cursor",
        all["nextCursor"].as_str().unwrap(),
    ]);
    let inside = std::fs::canonicalize(env.dir.join("in")).unwrap();
    let mut mcp = Mcp::start(&env, &["--root", inside.to_str().unwrap()]);
    let narrow = mcp.call(
        "changes",
        json!({"workspaceId": all["workspaceId"], "consumer": "bot"}),
    );
    assert_eq!(
        (
            all["revisions"].as_array().unwrap().len(),
            narrow["structuredContent"]["revisions"]
                .as_array()
                .unwrap()
                .len()
        ),
        (2, 1)
    );
}

#[test]
fn tracking_every_source_builds_only_those_not_yet_tracked() {
    let env = Env::new();
    for d in ["project", "in", "out"] {
        std::fs::create_dir(env.dir.join(d)).unwrap();
    }
    let dir = std::fs::canonicalize(env.dir.join("project")).unwrap();
    let dir = dir.to_str().unwrap();
    let mut ids = Vec::new();
    for name in ["in/a.fig", "out/b.fig"] {
        let fig = env.fig(name);
        let prepared = env.json(&[
            "index",
            "prepare",
            fig.to_str().unwrap(),
            "--workspace",
            dir,
            "--scope",
            "0:0",
        ]);
        ids.push(prepared["job"]["sourceId"].as_str().unwrap().to_owned());
    }
    let track = |more: &[&str]| {
        let mut args = vec!["index", "track", "--workspace", dir, "--scope", "0:0"];
        args.extend(more);
        env.json(&args)
    };
    track(&["--source", &ids[0]]);
    let both = track(&[]);
    let refused = env.run(&[
        "index",
        "track",
        "--workspace",
        dir,
        "--scope",
        "9:9",
        "--request-id",
        "r1",
    ]);
    let built = |r: &Value| {
        r["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|j| j["sourceId"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        (
            built(&both),
            String::from_utf8_lossy(&refused.stderr).contains("requestId names one job")
        ),
        (vec![json!(ids[1])], true)
    );
}

#[test]
fn a_poll_lists_at_most_a_hundred_tracked_scopes_and_says_so() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.sql(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 100)
         INSERT INTO tracking (source_id, scope, state, epoch, facet_version, enabled_ms)
         SELECT source_id, '9:' || i, 'disabled', 1, '', 0 FROM sources, n",
    );
    let page = t.changes(&[]);
    assert_eq!(
        (
            page["tracking"].as_array().unwrap().len(),
            page["trackingTruncated"].clone()
        ),
        (100, json!(true))
    );
}

impl Tracked<'_> {
    fn watch(&self, action: &str, more: &[&str]) -> Value {
        let mut args = vec!["watch", action, "--file", &self.file];
        args.extend(more);
        self.env.json(&args)
    }

    fn subscribe(&self, more: &[&str]) -> String {
        let source = self.changes(&[])["tracking"][0]["sourceId"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut args = vec!["--consumer", "bot", "--source", &source];
        args.extend(more);
        self.watch("subscribe", &args)["subscription"]["subscriptionId"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn kinds(&self, page: &Value) -> Vec<Value> {
        page["revisions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["kind"].clone())
            .collect()
    }
}

#[test]
fn a_subscription_starts_at_the_tail_and_keeps_its_own_checkpoint_apart_from_the_consumer_s() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let id = t.subscribe(&[]);
    let again = t.subscribe(&[]);
    let at_start = t.changes(&["--subscription", &id]);
    t.save(true);
    t.index("refresh");
    let page = t.changes(&["--subscription", &id]);
    let cursor = page["nextCursor"].as_str().unwrap();
    let acked = t.watch("ack", &["--subscription", &id, "--cursor", cursor]);
    let after = t.changes(&["--subscription", &id]);
    // The consumer's raw checkpoint is its own: never moved by the subscription's ack.
    let raw = t.changes(&["--consumer", "bot"]);
    let mixed = t.fails(&[
        "changes",
        "--file",
        &t.file,
        "--subscription",
        &id,
        "--consumer",
        "bot",
    ]);
    assert_eq!(
        (
            again == id,
            t.kinds(&at_start),
            t.kinds(&page),
            acked["advanced"].clone(),
            t.kinds(&after),
            t.kinds(&raw),
            mixed.contains("pass none of them"),
        ),
        (
            true,
            vec![],
            vec![json!("changes")],
            json!(true),
            vec![],
            vec![json!("baseline"), json!("changes")],
            true
        )
    );
}

#[test]
fn a_node_subscription_reads_only_revisions_that_change_that_node_and_never_a_missing_one() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let id = t.changes(&[])["revisions"][1]["revisionId"]
        .as_str()
        .unwrap()
        .to_owned();
    let nodes = t.changes(&["--revision", &id, "--limit", "500"]);
    let touched: Vec<String> = nodes["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|n| [n["id"].clone(), n["frame"].clone()])
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let changed = touched[0].clone();
    let untouched: String = rusqlite::Connection::open(&t.db)
        .unwrap()
        .query_row(
            "SELECT r.node_id FROM records r JOIN published p ON p.gen = r.gen
             WHERE r.node_id NOT IN (SELECT value FROM json_each(?1)) LIMIT 1",
            [json!(touched).to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let on_changed = t.subscribe(&["--node", &changed]);
    let on_untouched = t.subscribe(&["--node", &untouched]);
    // Saving the first revision again changes the same layers back.
    t.save(false);
    t.index("refresh");
    let source = t.changes(&[])["tracking"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let missing = t.fails(&[
        "watch",
        "subscribe",
        "--file",
        &t.file,
        "--consumer",
        "bot",
        "--source",
        &source,
        "--node",
        "999:999",
    ]);
    let page = t.changes(&["--subscription", &on_changed]);
    assert_eq!(
        (
            t.kinds(&page),
            page["subscription"]["state"].clone(),
            t.kinds(&t.changes(&["--subscription", &on_untouched])),
            missing.contains("target_missing"),
        ),
        (vec![json!("changes")], json!("active"), vec![], true)
    );
}

#[test]
fn full_checkpoints_are_refused_not_evicted_and_unsubscribe_and_forget_remove_only_their_rows() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let durable = t.subscribe(&[]);
    let temporary = t.subscribe(&["--scope", "0:0", "--owner", "task"]);
    // Raw checkpoint views written before their sources were recorded, filling the consumer.
    t.sql(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 62)
         INSERT INTO consumers (consumer, filter, after_seq, floor, updated_ms)
         SELECT 'bot', 'x' || char(31) || i || char(31), 0, 0, 0 FROM n",
    );
    let source = t.changes(&[])["tracking"][0]["sourceId"].clone();
    let refused = t.fails(&[
        "watch",
        "subscribe",
        "--file",
        &t.file,
        "--consumer",
        "bot",
        "--source",
        source.as_str().unwrap(),
        "--node",
        "0:1",
    ]);
    let dropped = t.watch("unsubscribe", &["--consumer", "bot", "--owner", "task"]);
    let forgotten = t.watch("forget", &["--consumer", "bot"]);
    let listed = t.watch("list", &["--consumer", "bot"]);
    let ids: Vec<Value> = listed["subscriptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["subscriptionId"].clone())
        .collect();
    let gone = t.fails(&[
        "watch",
        "unsubscribe",
        "--file",
        &t.file,
        "--subscription",
        &temporary,
    ]);
    let tracked = t.changes(&[])["tracking"][0]["state"].clone();
    assert_eq!(
        (
            refused.contains("unsubscribe or forget"),
            json!([dropped["removed"], dropped["subscriptionIds"]]),
            forgotten["removed"].clone(),
            ids,
            gone.contains("no subscription"),
            tracked,
        ),
        (
            true,
            json!([1, [temporary]]),
            json!(62),
            vec![json!(durable)],
            true,
            json!("active")
        )
    );
}

#[test]
fn one_consumer_never_shares_a_target_across_owners_and_an_owner_s_cleanup_stays_in_its_consumer() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let source = t.changes(&[])["tracking"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let subscribe = |consumer: &str, more: &[&str]| {
        let mut args = vec![
            "watch",
            "subscribe",
            "--file",
            &t.file,
            "--consumer",
            consumer,
        ];
        args.extend(["--source", &source]);
        args.extend(more);
        args.iter().map(|a| a.to_string()).collect::<Vec<_>>()
    };
    let run = |args: Vec<String>| env.run(&args.iter().map(String::as_str).collect::<Vec<_>>());
    env.json(
        &subscribe("bot", &[])
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    let again = env.json(
        &subscribe("bot", &[])
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    let collision = run(subscribe("bot", &["--owner", "task"]));
    env.json(
        &subscribe("other", &["--owner", "task"])
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    let dropped = t.watch("unsubscribe", &["--consumer", "bot", "--owner", "task"]);
    let left = t.watch("list", &[])["subscriptions"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(
        (
            again["initialPosition"].clone(),
            String::from_utf8_lossy(&collision.stderr).contains("under another owner"),
            json!([dropped["removed"], dropped["subscriptionIds"]]),
            left,
        ),
        (json!("existing"), true, json!([0, []]), 2)
    );
}

#[test]
fn forget_removes_a_checkpoint_view_whose_source_was_removed_since() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let page = t.changes(&["--consumer", "bot"]);
    t.watch(
        "ack",
        &[
            "--consumer",
            "bot",
            "--cursor",
            page["nextCursor"].as_str().unwrap(),
        ],
    );
    let source = page["tracking"][0]["sourceId"].as_str().unwrap().to_owned();
    let id = page["workspaceId"].as_str().unwrap().to_owned();
    env.json(&["index", "remove", "--id", &id, &source]);
    let forgotten = env.json(&["watch", "forget", "--id", &id, "--consumer", "bot"]);
    assert_eq!(forgotten["removed"], json!(1), "{forgotten}");
}

#[test]
fn a_node_subscription_reads_a_revision_whose_comparison_was_incomplete() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let source = t.changes(&[])["tracking"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let untouched = t.subscribe(&["--node", "0:12"]);
    // A partial comparison whose retained rows name only other layers: detail is complete, the
    // comparison is not, so the layer may have changed unseen.
    t.sql(&format!(
        "INSERT INTO revisions (id, seq, source_id, scope, epoch, gen, from_gen, snapshot_id,
         kind, coverage, summary, detail, detail_rows, detail_bytes, created_ms)
         SELECT randomblob(16), seq + 100, source_id, scope, epoch, gen, from_gen, snapshot_id,
         kind, '{{\"complete\":false}}', summary, 'complete', 0, 0, created_ms
         FROM revisions WHERE source_id = '{source}' ORDER BY seq DESC LIMIT 1"
    ));
    let page = t.changes(&["--subscription", &untouched]);
    assert_eq!(page["revisions"].as_array().unwrap().len(), 1, "{page}");
}

#[test]
fn a_frame_subscription_reads_a_complete_revision_that_reaches_it_only_through_impact() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let frame = t.subscribe(&["--node", "0:4"]);
    // Only the Chip component, outside the Components frame, changes its fill; both chips in
    // the frame keep fills of their own.
    let next = env.dir.join("next.fig");
    corpus::write_restyled(next.as_os_str()).unwrap();
    std::fs::rename(&next, &t.path).unwrap();
    t.index("refresh");
    let page = t.changes(&["--subscription", &frame]);
    let revision = &page["revisions"][0];
    let id = revision["revisionId"].as_str().unwrap();
    let detail = t.changes(&["--revision", id, "--limit", "500"]);
    let entries = detail["nodes"].as_array().unwrap();
    let in_frame = entries
        .iter()
        .filter(|n| n["entry"] == "change" && n["frame"] == "0:4")
        .count();
    let mut chips: Vec<(Value, Value)> = entries
        .iter()
        .filter(|n| n["entry"] == "impact" && ["0:41", "0:42"].contains(&n["id"].as_str().unwrap()))
        .map(|n| (n["id"].clone(), n["class"].clone()))
        .collect();
    chips.sort_by_key(|c| c.0.to_string());
    assert_eq!(
        (
            page["revisions"].as_array().unwrap().len(),
            revision["impact"]["complete"].clone(),
            in_frame,
            chips
        ),
        (
            1,
            json!(true),
            0,
            vec![
                (json!("0:41"), json!("effective_unchanged")),
                (json!("0:42"), json!("effective_unchanged"))
            ]
        ),
        "{detail}"
    );
}

/// Follows each node, replaces the file with `write`'s output and refreshes; then how many
/// revisions each subscription reads, the newest revision's stored entries and the old and new
/// top-level frames stored for its impact hit on `0:42`.
fn follow_through(
    env: &Env,
    nodes: &[&str],
    write: fn(&std::ffi::OsStr) -> Result<(), Box<dyn std::error::Error>>,
) -> (Vec<usize>, Value, Value) {
    let t = Tracked::new(env);
    let subs: Vec<String> = nodes.iter().map(|n| t.subscribe(&["--node", n])).collect();
    let next = env.dir.join("next.fig");
    write(next.as_os_str()).unwrap();
    std::fs::rename(&next, &t.path).unwrap();
    t.index("refresh");
    let counts = subs
        .iter()
        .map(|s| {
            t.changes(&["--subscription", s])["revisions"]
                .as_array()
                .unwrap()
                .len()
        })
        .collect();
    let all = t.changes(&[]);
    let id = all["revisions"].as_array().unwrap().last().unwrap()["revisionId"]
        .as_str()
        .unwrap()
        .to_owned();
    let frames = rusqlite::Connection::open(&t.db)
        .unwrap()
        .query_row(
            "SELECT json_array(old_frame, new_frame) FROM revision_impacts WHERE node = '0:42'",
            [],
            |r| r.get::<_, String>(0),
        )
        .map_or(Value::Null, |f| serde_json::from_str(&f).unwrap());
    (
        counts,
        t.changes(&["--revision", &id, "--limit", "500"]),
        frames,
    )
}

fn impact_entry(detail: &Value, id: &str) -> Value {
    detail["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["entry"] == "impact" && n["id"] == id)
        .map_or(Value::Null, |n| json!([n["class"], n["sides"]]))
}

#[test]
fn a_node_subscription_inside_a_removed_then_re_added_frame_reads_both_revisions() {
    let env = Env::new();
    let t = Tracked::new(&env);
    // 0:12 lies inside frame 0:1, which each save removes or adds whole; 0:31 is elsewhere.
    let subs: Vec<String> = ["0:12", "0:31"]
        .iter()
        .map(|n| t.subscribe(&["--node", n]))
        .collect();
    let save = |removed: bool| {
        let next = env.dir.join("next.fig");
        match removed {
            true => corpus::write_auto_layout_removed(next.as_os_str()),
            false => corpus::write(next.as_os_str(), false),
        }
        .unwrap();
        std::fs::rename(&next, &t.path).unwrap();
        t.index("refresh");
        let counts: Vec<usize> = subs
            .iter()
            .map(|s| {
                t.changes(&["--subscription", s])["revisions"]
                    .as_array()
                    .unwrap()
                    .len()
            })
            .collect();
        let all = t.changes(&[]);
        let id = all["revisions"].as_array().unwrap().last().unwrap()["revisionId"]
            .as_str()
            .unwrap()
            .to_owned();
        let detail = t.changes(&["--revision", &id, "--limit", "500"]);
        (counts, impact_entry(&detail, "0:12"))
    };
    assert_eq!(
        (save(true), save(false)),
        (
            (vec![1, 0], json!(["direct", ["old"]])),
            (vec![2, 0], json!(["direct", ["new"]]))
        )
    );
}

#[test]
fn node_subscriptions_follow_a_component_child_change_to_the_instance_and_its_frame() {
    let env = Env::new();
    // The Dot child changes; neither chip overrides it. The Chip main 0:50 holds the change
    // itself; frame 0:1 and layer 0:12 are unrelated.
    let (counts, detail, _) = follow_through(
        &env,
        &["0:41", "0:4", "0:50", "0:1", "0:12"],
        corpus::write_dot_restyled,
    );
    // The instance's own facets are unchanged, while its expanded Dot child's appearance is not:
    // a recorded change in the frame, whose REST fill differs between the two files.
    let dot = detail["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["entry"] == "change" && n["id"] == "I0:41;0:51")
        .map_or(Value::Null, |n| {
            json!([n["change"], n["facets"], n["frame"]])
        });
    let fills = |write: fn(&std::ffi::OsStr) -> Result<(), Box<dyn std::error::Error>>,
                 name: &str| {
        let path = env.dir.join(name);
        write(path.as_os_str()).unwrap();
        fn find<'a>(v: &'a Value, id: &str) -> Option<&'a Value> {
            if v["id"] == id {
                return Some(v);
            }
            match v {
                Value::Object(m) => m.values().find_map(|c| find(c, id)),
                Value::Array(a) => a.iter().find_map(|c| find(c, id)),
                _ => None,
            }
        }
        let rest = env.json(&["nodes", path.to_str().unwrap(), "0:41"]);
        find(&rest, "I0:41;0:51").unwrap()["fills"].clone()
    };
    let before = fills(|p| corpus::write(p, false), "old-dot.fig");
    let after = fills(corpus::write_dot_restyled, "new-dot.fig");
    assert_eq!(
        (counts, impact_entry(&detail, "0:41"), dot, before == after),
        (
            vec![1, 1, 1, 0, 0],
            json!(["effective_unchanged", ["old", "new"]]),
            json!(["changed", ["appearance"], "0:4"]),
            false
        ),
        "{detail}"
    );
}

#[test]
fn a_removed_instance_keeps_its_old_frame_and_its_own_subscription_reading_the_revision() {
    let env = Env::new();
    // Chip 0:42 is gone from the new file: its link to the component exists on the old side
    // only. Its removal leaves the appearance verdict unknown, so the impact is incomplete and
    // every node subscription reads the revision (0:12), as it cannot be ruled out.
    let (counts, detail, frames) = follow_through(
        &env,
        &["0:4", "0:42", "0:12"],
        corpus::write_dot_restyled_chip_removed,
    );
    assert_eq!(
        (counts, impact_entry(&detail, "0:42"), frames),
        (
            vec![1, 1, 1],
            json!(["direct", ["old"]]),
            json!(["0:4", null])
        ),
        "{detail}"
    );
}

#[test]
fn only_the_frames_holding_an_affected_instance_on_either_side_read_a_component_change() {
    let env = Env::new();
    // Chip 0:42 moves from Components (0:4) into Effects (0:3).
    let (counts, detail, frames) = follow_through(
        &env,
        &["0:1", "0:2", "0:3", "0:4"],
        corpus::write_dot_restyled_chip_moved,
    );
    assert_eq!(
        (counts, frames),
        (vec![0, 0, 1, 1], json!(["0:4", "0:3"])),
        "{detail}"
    );
}

#[test]
fn a_node_subscription_reads_revisions_whose_impact_reaches_that_node_or_is_incomplete() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let source = t.changes(&[])["tracking"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let reached = t.subscribe(&["--node", "0:12"]);
    // A complete revision that changes other layers and reaches 0:12 through an instance.
    t.sql(&format!(
        "INSERT INTO revisions (id, seq, source_id, scope, epoch, gen, from_gen, snapshot_id,
         kind, coverage, summary, detail, detail_rows, detail_bytes, created_ms, impact)
         SELECT x'000102030405060708090a0b0c0d0e0f', seq + 100, source_id, scope, epoch, gen,
         from_gen, snapshot_id, kind, '{{\"complete\":true}}', summary, 'complete', 0, 0,
         created_ms, impact FROM revisions WHERE source_id = '{source}' ORDER BY seq DESC LIMIT 1;
         INSERT INTO revision_impacts VALUES (x'000102030405060708090a0b0c0d0e0f', 0, '0:12',
         'effective', 3, 1, 0, 0, 'instance_of', '0:11', NULL, NULL);"
    ));
    // And a complete one whose only hit is a child of each followed frame: moved, it is held
    // by one frame before and another after.
    let framed = t.subscribe(&["--node", "0:11"]);
    let moved = t.subscribe(&["--node", "0:14"]);
    t.sql(&format!(
        "INSERT INTO revisions (id, seq, source_id, scope, epoch, gen, from_gen, snapshot_id,
         kind, coverage, summary, detail, detail_rows, detail_bytes, created_ms, impact)
         SELECT randomblob(16), seq + 1, source_id, scope, epoch, gen, from_gen,
         snapshot_id, kind, '{{\"complete\":true}}', summary, 'complete', 0, 0, created_ms,
         '{{\"complete\":true,\"reasons\":[],\"listed\":{{\"effective\":1}}}}'
         FROM revisions WHERE source_id = '{source}' ORDER BY seq DESC LIMIT 1;
         INSERT INTO revision_impacts SELECT id, 0, '0:99', 'effective', 3, 1, 0, 0,
         'instance_of', '0:30', '0:11', '0:14' FROM revisions ORDER BY seq DESC LIMIT 1;"
    ));
    let count = |sub: &str| {
        t.changes(&["--subscription", sub])["revisions"]
            .as_array()
            .unwrap()
            .len()
    };
    assert_eq!((count(&framed), count(&moved)), (1, 1));
    // And a complete one reaching nothing it names, whose impact could not rule it out.
    t.sql(&format!(
        "INSERT INTO revisions (id, seq, source_id, scope, epoch, gen, from_gen, snapshot_id,
         kind, coverage, summary, detail, detail_rows, detail_bytes, created_ms, impact)
         SELECT randomblob(16), seq + 1, source_id, scope, epoch, gen, from_gen, snapshot_id,
         kind, coverage, summary, 'complete', 0, 0, created_ms,
         '{{\"complete\":false,\"reasons\":[{{\"reason\":\"depth_limit\",\"count\":1}}],\"listed\":{{}}}}'
         FROM revisions WHERE source_id = '{source}' ORDER BY seq DESC LIMIT 1"
    ));
    let page = t.changes(&["--subscription", &reached]);
    assert_eq!(page["revisions"].as_array().unwrap().len(), 2, "{page}");
}

#[test]
fn a_node_subscription_reads_a_revision_recorded_before_impact_was() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let source = t.changes(&[])["tracking"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let followed = t.subscribe(&["--node", "0:12"]);
    // A complete revision naming none of 0:12, as schema 7 keeps one recorded before it: its
    // impact is unknown, never an empty complete one, so it cannot rule the node out.
    t.sql(&format!(
        "INSERT INTO revisions (id, seq, source_id, scope, epoch, gen, from_gen, snapshot_id,
         kind, coverage, summary, detail, detail_rows, detail_bytes, created_ms, impact)
         SELECT randomblob(16), seq + 1, source_id, scope, epoch, gen, from_gen, snapshot_id,
         'changes', '{{\"complete\":true}}', summary, 'complete', 0, 0, created_ms, NULL
         FROM revisions WHERE source_id = '{source}' ORDER BY seq DESC LIMIT 1"
    ));
    let page = t.changes(&["--subscription", &followed]);
    assert_eq!(page["revisions"].as_array().unwrap().len(), 1, "{page}");
}

#[test]
fn a_revision_s_detail_pages_one_entry_at_a_time_across_its_changes_and_impact() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let id = t.changes(&[])["revisions"][1]["revisionId"]
        .as_str()
        .unwrap()
        .to_owned();
    let whole = t.changes(&["--revision", &id, "--limit", "500"])["nodes"].clone();
    let (mut paged, mut cursor) = (Vec::new(), None::<String>);
    loop {
        let mut args = vec!["--revision", &id, "--limit", "1"];
        if let Some(c) = &cursor {
            args.extend(["--cursor", c]);
        }
        let page = t.changes(&args);
        paged.extend(page["nodes"].as_array().unwrap().iter().cloned());
        match page["nextCursor"].as_str() {
            Some(c) => cursor = Some(c.to_owned()),
            None => break,
        }
    }
    let entries: Vec<Value> = paged.iter().map(|n| n["entry"].clone()).collect();
    assert_eq!(
        (json!(paged) == whole, entries),
        (
            true,
            ["change"; 4]
                .into_iter()
                .chain(["impact"; 4])
                .map(|e| json!(e))
                .collect()
        )
    );
}

#[test]
fn a_schema_6_workspace_reads_impact_as_null_and_records_it_once_a_build_upgrades_it() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    // As schema 6 left it: revisions recorded before impact was, and no fingerprints or
    // subscription source sets (schema 8).
    t.sql(
        "DROP TABLE revision_impacts; ALTER TABLE revisions DROP COLUMN impact;
         DROP TABLE fingerprints; ALTER TABLE subscriptions DROP COLUMN sources;
         ALTER TABLE subscriptions DROP COLUMN disabled; DROP INDEX sources_remote;
         ALTER TABLE sources DROP COLUMN remote; ALTER TABLE sources DROP COLUMN generation; DROP TABLE model_vectors; DROP TABLE model_refs; DROP TABLE model_layers; DROP TABLE visual_units; DROP TABLE visual_rasters; DROP INDEX jobs_visual; DROP INDEX jobs_embed; ALTER TABLE jobs DROP COLUMN profile_id;
         ALTER TABLE records DROP COLUMN props;
         PRAGMA user_version = 6;",
    );
    let version = || -> i64 {
        rusqlite::Connection::open(&t.db)
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    };
    let impacts = |page: &Value| -> Vec<Value> {
        page["revisions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["impact"]["complete"].clone())
            .collect()
    };
    let before = t.changes(&[]);
    let id = before["revisions"][1]["revisionId"].as_str().unwrap();
    let detail = t.changes(&["--revision", id, "--limit", "500"]);
    let entries: Vec<Value> = detail["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["entry"].clone())
        .collect();
    let read = version();
    t.save(false);
    t.index("refresh");
    let after = t.changes(&[]);
    assert_eq!(
        (impacts(&before), entries, read, impacts(&after), version()),
        (
            vec![Value::Null; 2],
            vec![json!("change"); 4],
            6,
            vec![Value::Null, Value::Null, json!(true)],
            figlens::workspace::SCHEMA
        )
    );
}

#[test]
fn a_92_generation_refreshed_under_new_relations_is_a_baseline_keeping_history_and_checkpoints() {
    let env = Env::new();
    let t = Tracked::new(&env);
    t.save(true);
    t.index("refresh");
    let id = t.subscribe(&[]);
    let tail = t.changes(&[])["nextCursor"].as_str().unwrap().to_owned();
    t.watch("ack", &["--consumer", "bot", "--cursor", &tail]);
    // As #92 left it: schema 7, generations built by relations-5, no library references,
    // fingerprints or subscription source sets.
    t.sql(
        "DELETE FROM edges WHERE relation = 'uses_library'; DROP TABLE fingerprints;
         ALTER TABLE subscriptions DROP COLUMN sources;
         ALTER TABLE subscriptions DROP COLUMN disabled; DROP INDEX sources_remote;
         ALTER TABLE sources DROP COLUMN remote; ALTER TABLE sources DROP COLUMN generation;
         UPDATE generations SET versions = replace(versions, '\"relations-6\"', '\"relations-5\"');
         DROP TABLE model_vectors; DROP TABLE model_refs; DROP TABLE model_layers; DROP TABLE visual_units; DROP TABLE visual_rasters; DROP INDEX jobs_visual; DROP INDEX jobs_embed; ALTER TABLE jobs DROP COLUMN profile_id;
         ALTER TABLE records DROP COLUMN props;
         PRAGMA user_version = 7;",
    );
    let before = t.revisions();
    let checkpoints = "SELECT count(*) FROM consumers";
    let held = t.count(checkpoints);
    t.changes(&["--consumer", "bot"]);
    t.watch("list", &[]);
    let read = t.count("PRAGMA user_version");
    // The same bytes, so only the extraction changed: a baseline, never a comparison.
    t.index("refresh");
    let after = t.revisions();
    let kinds = |more: &[&str]| t.kinds(&t.changes(more));
    assert_eq!(
        (
            read,
            &after[..2],
            after[2].clone(),
            t.count("PRAGMA user_version"),
            t.count(checkpoints),
            kinds(&["--consumer", "bot"]),
            kinds(&["--subscription", &id]),
        ),
        (
            7,
            &before[..],
            json!(["baseline", {"basis": "baseline", "reason": "index_version_changed"}, null, "complete", false, null, 0]),
            figlens::workspace::SCHEMA,
            held,
            vec![json!("baseline")],
            vec![json!("baseline")],
        )
    );
}

#[test]
fn watch_list_reads_a_schema_7_workspace_without_its_source_set_columns() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let id = t.subscribe(&[]);
    // As schema 7 left it: no subscription source sets or disabled reasons (schema 8).
    t.sql(
        "ALTER TABLE subscriptions DROP COLUMN sources;
         ALTER TABLE subscriptions DROP COLUMN disabled; DROP INDEX sources_remote;
         ALTER TABLE sources DROP COLUMN remote; ALTER TABLE sources DROP COLUMN generation; DROP TABLE model_vectors; DROP TABLE model_refs; DROP TABLE model_layers; DROP TABLE visual_units; DROP TABLE visual_rasters; DROP INDEX jobs_visual; DROP INDEX jobs_embed; ALTER TABLE jobs DROP COLUMN profile_id;
         PRAGMA user_version = 7;",
    );
    let listed = t.watch("list", &[]);
    assert_eq!(listed["subscriptions"][0]["subscriptionId"], json!(id));
}

#[test]
fn a_subscribe_reply_describes_its_source_as_authorized_though_it_is_relocated_meanwhile() {
    let env = Env::new();
    let (inside, outside) = (env.dir.join("in"), env.dir.join("out"));
    std::fs::create_dir(&inside).unwrap();
    std::fs::create_dir(&outside).unwrap();
    corpus::write(inside.join("a.fig").as_os_str(), false).unwrap();
    let file = std::fs::canonicalize(inside.join("a.fig")).unwrap();
    let file = file.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    env.json(&["index", "track", "--file", file, "--scope", "0:0"]);
    let status = env.json(&["index", "status", "--file", file]);
    let (ws, source) = (
        status["workspaceId"].clone(),
        status["sources"][0]["sourceId"]
            .as_str()
            .unwrap()
            .to_owned(),
    );
    let moved = std::fs::canonicalize(&outside).unwrap().join("a.fig");
    let root = std::fs::canonicalize(&inside).unwrap();
    let gate = env.gate("subscribe.hold");
    let mut mcp = Mcp::held(
        &env,
        &["--root", root.to_str().unwrap()],
        Some(("subscribe", &gate)),
    );
    mcp.send(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "watch",
        "arguments": {"action": "subscribe", "workspaceId": ws, "consumer": "bot", "sourceId": source}}}));
    let reached = PathBuf::from(format!("{}.reached", gate.display()));
    wait_for("subscribe to write its row", || reached.exists());
    // The move out of the root waits for the subscribe's write and gives up while it is held.
    std::fs::rename(file, &moved).unwrap();
    let relocate = env.run(&[
        "index",
        "relocate",
        &source,
        moved.to_str().unwrap(),
        "--file",
        file,
    ]);
    std::fs::remove_file(&gate).unwrap();
    let reply = loop {
        let reply = mcp.read();
        if reply["id"] == 1 {
            break reply["result"]["structuredContent"].clone();
        }
    };
    assert_eq!(
        (
            relocate.status.success(),
            reply["subscription"]["state"].clone(),
            reply["tracking"][0]["sourceId"].clone(),
        ),
        (false, json!("active"), json!(source))
    );
}

impl Tracked<'_> {
    fn count(&self, q: &str) -> i64 {
        rusqlite::Connection::open(&self.db)
            .unwrap()
            .query_row(q, [], |r| r.get(0))
            .unwrap()
    }

    /// A saved query subscription to `query` and its subscribe reply.
    fn query(&self, query: &str) -> (String, Value) {
        let source = self.changes(&[])["tracking"][0]["sourceId"]
            .as_str()
            .unwrap()
            .to_owned();
        let reply = self.watch(
            "subscribe",
            &["--consumer", "bot", "--source", &source, "--query", query],
        );
        let id = reply["subscription"]["subscriptionId"]
            .as_str()
            .unwrap()
            .to_owned();
        (id, reply)
    }
}

#[test]
fn evaluate_refuses_a_source_it_would_ignore_over_the_cli_and_mcp() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("badge");
    let source = t.changes(&[])["tracking"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let ws = env.json(&["index", "status", "--file", &t.file])["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let rows = "SELECT COUNT(*) FROM query_evaluations";
    let before = t.count(rows);
    let mut cli = Vec::new();
    for given in ["nonexistent", source.as_str()] {
        cli.push(t.fails(&[
            "watch",
            "evaluate",
            "--file",
            &t.file,
            "--subscription",
            &id,
            "--source",
            given,
        ]));
    }
    let mut mcp = Mcp::start(&env, &[]);
    let refused = mcp.call(
        "watch",
        json!({"action": "evaluate", "workspaceId": ws, "subscriptionId": id, "sourceId": "nonexistent"}),
    );
    drop(mcp);
    assert!(
        cli.iter().all(|e| e.contains("unknown flag --source")),
        "{cli:?}"
    );
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(
        refused.to_string().contains("takes no sourceId"),
        "{refused}"
    );
    assert_eq!(t.count(rows), before, "a refused evaluate stored one");
}

#[test]
fn a_query_subscription_reads_stored_evaluations_and_only_evaluate_adds_one() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, subscribed) = t.query("badge");
    t.save(true);
    t.index("refresh");
    let rows = "SELECT COUNT(*) FROM query_evaluations";
    let before = t.count(rows);
    let waiting = t.changes(&["--subscription", &id]);
    let unchanged = t.count(rows);
    let evaluated = t.watch("evaluate", &["--subscription", &id]);
    let again = t.watch("evaluate", &["--subscription", &id]);
    let page = t.changes(&["--subscription", &id]);
    let cursor = page["nextCursor"].as_str().unwrap().to_owned();
    let acked = t.watch("ack", &["--subscription", &id, "--cursor", &cursor]);
    let after = t.changes(&["--subscription", &id]);
    t.watch("unsubscribe", &["--subscription", &id]);
    assert_eq!(
        json!([
            subscribed["evaluation"]["causes"],
            subscribed["initialPosition"],
            waiting["evaluation"]["state"],
            [before, unchanged],
            evaluated["evaluation"]["causes"],
            evaluated["evaluation"]["delta"]["enteredTopK"]
                .as_array()
                .unwrap()
                .len(),
            again["evaluation"]["state"],
            page["evaluations"].as_array().unwrap().len(),
            acked["advanced"],
            after["evaluations"].as_array().unwrap().len(),
            t.count(rows)
        ]),
        json!([
            ["initial"],
            "initial_evaluation",
            "pending",
            [1, 1],
            ["source_changed"],
            1,
            "current",
            2,
            true,
            0,
            0
        ])
    );
}

#[test]
fn a_rebuild_that_changes_no_result_adds_no_evaluation_and_reads_current() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("auto layout");
    t.index("rebuild");
    let evaluated = t.watch("evaluate", &["--subscription", &id]);
    let page = t.changes(&["--subscription", &id]);
    assert_eq!(
        json!([
            evaluated["evaluation"]["state"],
            page["evaluation"]["state"],
            t.count("SELECT COUNT(*) FROM query_evaluations")
        ]),
        json!(["current", "current", 1])
    );
}

#[test]
fn spellings_search_normalizes_alike_are_one_query_subscription_and_type_needs_a_query() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (first, _) = t.query("auto layout");
    let (again, repeat) = t.query("  AUTO   Layout ");
    let source = t.changes(&[])["tracking"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let refused = t.fails(&[
        "watch",
        "subscribe",
        "--file",
        &t.file,
        "--consumer",
        "bot",
        "--source",
        &source,
        "--type",
        "TEXT",
    ]);
    assert_eq!(
        (
            first == again,
            repeat["created"].clone(),
            repeat.get("evaluation").is_none(),
            refused.contains("pass query with it")
        ),
        (true, json!(false), true, true)
    );
}

#[test]
fn evaluations_past_retention_are_an_event_gap_and_past_the_byte_quota_are_refused() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("badge");
    // Fifteen more kept evaluations: the next one deletes the oldest the checkpoint is before.
    t.sql(
        "WITH RECURSIVE n(i) AS (SELECT 2 UNION ALL SELECT i + 1 FROM n WHERE i < 16)
         INSERT INTO query_evaluations (id, sub, seq, gen, snapshot_id, membership, ranking,
         coverage, causes, refs, total, delta, bytes, created_ms)
         SELECT randomblob(16), sub, i, gen, snapshot_id, membership, ranking, coverage, causes,
         refs, total, delta, bytes, created_ms FROM query_evaluations, n",
    );
    t.save(true);
    t.index("refresh");
    t.watch("evaluate", &["--subscription", &id]);
    let gap = t
        .env
        .run(&["changes", "--file", &t.file, "--subscription", &id]);
    t.sql("UPDATE query_evaluations SET bytes = 8 * 1024 * 1024");
    t.save(false);
    t.index("refresh");
    let refused = t.env.run(&[
        "watch",
        "evaluate",
        "--file",
        &t.file,
        "--subscription",
        &id,
    ]);
    assert_eq!(
        (
            t.count("SELECT COUNT(*) FROM query_evaluations"),
            String::from_utf8_lossy(&gap.stderr).contains("deleted by retention"),
            String::from_utf8_lossy(&refused.stderr).contains("evaluations already hold"),
        ),
        (16, true, true)
    );
}

#[test]
fn mcp_saves_a_typed_query_evaluates_it_and_a_confined_caller_cannot_read_it() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let polled = t.changes(&[]);
    let (ws, source) = (
        polled["workspaceId"].clone(),
        polled["tracking"][0]["sourceId"].clone(),
    );
    let mut mcp = Mcp::start(&env, &[]);
    let subscribed = mcp.call(
        "watch",
        json!({"action": "subscribe", "workspaceId": ws, "consumer": "bot", "sourceId": source,
            "query": "badge", "type": "rectangle"}),
    )["structuredContent"]
        .clone();
    let id = subscribed["subscription"]["subscriptionId"].clone();
    t.save(true);
    t.index("refresh");
    let evaluated = mcp.call(
        "watch",
        json!({"action": "evaluate", "workspaceId": ws, "subscriptionId": id}),
    )["structuredContent"]
        .clone();
    let elsewhere = env.dir.join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let elsewhere = std::fs::canonicalize(elsewhere).unwrap();
    let mut confined = Mcp::start(&env, &["--root", elsewhere.to_str().unwrap()]);
    let hidden = confined.call("changes", json!({"workspaceId": ws, "subscriptionId": id}));
    // list by subscriptionId reads only that row, through MCP and the CLI alike.
    let list = json!({"action": "list", "workspaceId": ws, "subscriptionId": id});
    let only = |reply: &Value| -> Vec<Value> {
        reply["subscriptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["subscriptionId"].clone())
            .collect()
    };
    let listed = (
        only(&mcp.call("watch", list.clone())["structuredContent"]),
        only(&t.watch("list", &["--subscription", id.as_str().unwrap()])),
        only(&confined.call("watch", list)["structuredContent"]),
    );
    assert_eq!(listed, (vec![id.clone()], vec![id.clone()], vec![]));
    assert_eq!(
        json!([
            subscribed["subscription"]["types"],
            evaluated["evaluation"]["delta"]["enteredTopK"][0]["snapshotId"]
                == evaluated["evaluation"]["delta"]["snapshotId"],
            evaluated["evaluation"]["delta"]["fromSnapshotId"]
                != evaluated["evaluation"]["delta"]["snapshotId"],
            hidden["content"][0]["text"]
                .as_str()
                .is_some_and(|t| t.contains("no subscription")),
        ]),
        json!([["RECTANGLE"], true, true, true])
    );
}

#[test]
fn a_query_evaluation_is_current_only_for_its_index_once_the_file_changes_or_goes() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("badge");
    let fresh = t.changes(&["--subscription", &id])["evaluation"].clone();
    t.save(true);
    let saved = t.changes(&["--subscription", &id])["evaluation"].clone();
    std::fs::remove_file(&t.path).unwrap();
    let gone = t.changes(&["--subscription", &id])["evaluation"].clone();
    assert_eq!(
        json!([
            fresh["state"],
            saved["state"],
            saved["nextSteps"][0]["arguments"]["action"],
            gone["state"]
        ]),
        json!([
            "current",
            "current_for_index",
            "refresh",
            "current_for_index"
        ])
    );
}

#[test]
fn a_damaged_stored_evaluation_is_invalid_data_and_a_new_ranking_compares_no_positions() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("auto layout");
    t.sql("UPDATE query_evaluations SET ranking = 'older'");
    t.index("rebuild");
    let evaluated = t.watch("evaluate", &["--subscription", &id]);
    // A damaged later row ends the page before it; read first, it fails with an ack past it.
    t.sql("UPDATE query_evaluations SET delta = '{' WHERE seq = 2");
    let before = t.changes(&["--subscription", &id]);
    let cursor = before["nextCursor"].as_str().unwrap();
    let damaged = t.fails(&[
        "changes",
        "--file",
        &t.file,
        "--subscription",
        &id,
        "--cursor",
        cursor,
    ]);
    assert_eq!(
        json!([
            evaluated["evaluation"]["causes"],
            evaluated["evaluation"]["delta"]["rankChanged"],
            evaluated["evaluation"]["delta"]["rankComparable"],
            before["evaluations"].as_array().unwrap().len(),
            before["hasMore"],
            damaged.contains("is damaged") && damaged.contains("ack")
        ]),
        json!([
            ["index_rebuilt", "ranking_changed"],
            null,
            false,
            1,
            true,
            true
        ])
    );
}

#[test]
fn a_query_subscription_follows_a_relocated_source_and_compares_its_same_nodes() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("badge");
    let moved = env.dir.join("moved.fig");
    std::fs::copy(&t.path, &moved).unwrap();
    let moved = std::fs::canonicalize(moved).unwrap();
    let moved = moved.to_str().unwrap();
    let ws = env.json(&["index", "status", "--file", &t.file]);
    let (wid, source) = (
        ws["workspaceId"].as_str().unwrap(),
        ws["sources"][0]["sourceId"].as_str().unwrap(),
    );
    env.json(&["index", "relocate", "--id", wid, source, moved]);
    env.json(&["index", "refresh", "--file", moved, "--scope", "0:0"]);
    let e =
        env.json(&["watch", "evaluate", "--id", wid, "--subscription", &id])["evaluation"].clone();
    assert_eq!(
        json!([
            e["state"],
            e["causes"]
                .as_array()
                .unwrap()
                .contains(&json!("corpus_changed")),
            e["delta"]["enteredTopK"],
            e["delta"]["leftTopK"],
            e["delta"]["rankComparable"]
        ]),
        json!(["evaluated", true, [], [], true]),
        "{e}"
    );
}

#[test]
fn an_understated_stored_size_never_admits_an_oversized_evaluation() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("badge");
    t.sql("UPDATE query_evaluations SET bytes = 1, delta = '\"' || zeroblob(300000) || '\"'");
    let refused = t.fails(&["changes", "--file", &t.file, "--subscription", &id]);
    assert!(refused.contains("takes more than"), "{refused}");
}

#[test]
fn a_last_evaluation_whose_columns_together_pass_the_row_cap_is_damaged_not_read() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("badge");
    // Neither column alone passes 128 KiB; together they do.
    t.sql("UPDATE query_evaluations SET coverage = '\"' || zeroblob(70000) || '\"', refs = '[\"' || zeroblob(70000) || '\"]'");
    t.index("rebuild");
    let refused = t.fails(&[
        "watch",
        "evaluate",
        "--file",
        &t.file,
        "--subscription",
        &id,
    ]);
    assert!(refused.contains("is damaged"), "{refused}");
}

/// A design inside an MCP root, tracked, and a path outside it to relocate the design to.
fn inside_and_out(env: &Env) -> (String, String, String, String) {
    let (inside, outside) = (env.dir.join("in"), env.dir.join("out"));
    std::fs::create_dir(&inside).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let path = inside.join("a.fig");
    corpus::write(path.as_os_str(), false).unwrap();
    let file = std::fs::canonicalize(&path).unwrap();
    let file = file.to_str().unwrap().to_owned();
    env.json(&["index", "prepare", &file, "--scope", "0:0"]);
    env.json(&["index", "track", "--file", &file, "--scope", "0:0"]);
    let root = std::fs::canonicalize(inside).unwrap();
    let moved = std::fs::canonicalize(outside).unwrap().join("a.fig");
    let source = env.json(&["index", "status", "--file", &file])["sources"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    (
        file,
        root.to_str().unwrap().to_owned(),
        moved.to_str().unwrap().to_owned(),
        source,
    )
}

#[test]
fn a_changes_read_relocated_out_of_its_root_meanwhile_reads_the_registry_it_authorized() {
    let env = Env::new();
    let (file, root, moved, source) = inside_and_out(&env);
    let ws = env.json(&["changes", "--file", &file])["workspaceId"].clone();
    let before = env.json(&["changes", "--file", &file])["revisions"].clone();
    let gate = env.gate("changes.hold");
    let mut mcp = Mcp::held(&env, &["--root", &root], Some(("changes", &gate)));
    mcp.send(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "changes", "arguments": {"workspaceId": ws}}}));
    let reached = PathBuf::from(format!("{}.reached", gate.display()));
    wait_for("the read to choose its visible sources", || {
        reached.exists()
    });
    std::fs::rename(&file, &moved).unwrap();
    env.json(&["index", "relocate", &source, &moved, "--file", &file]);
    // A revision published after the move. A real build would wait for this held reader to
    // let the write-ahead log reset, so the row is written as its publication would.
    let db = env.json(&["index", "status", "--file", &moved])["paths"]["database"]
        .as_str()
        .unwrap()
        .to_owned();
    rusqlite::Connection::open(db)
        .unwrap()
        .execute_batch(
            "INSERT INTO revisions SELECT randomblob(16), seq + 100, source_id, scope, epoch,
             gen, from_gen, snapshot_id, kind, coverage, summary, detail, detail_rows,
             detail_bytes, created_ms, impact FROM revisions ORDER BY seq DESC LIMIT 1",
        )
        .unwrap();
    let after = env.json(&["changes", "--file", &moved])["revisions"].clone();
    std::fs::remove_file(&gate).unwrap();
    let reply = loop {
        let reply = mcp.read();
        if reply["id"] == 1 {
            break reply["result"]["structuredContent"].clone();
        }
    };
    // The build recorded after the move is never served to a caller that cannot see it.
    assert_eq!(
        (
            after.as_array().unwrap().len() > before.as_array().unwrap().len(),
            reply["revisions"].clone(),
        ),
        (true, before)
    );
}

#[test]
fn a_clear_cache_keeps_what_a_source_moved_out_of_its_root_meanwhile_owns() {
    let env = Env::new();
    let (file, root, moved, source) = inside_and_out(&env);
    // The baseline rebuild retired the first generation.
    let ws = env.json(&["index", "status", "--file", &file])["workspaceId"].clone();
    let db = env.json(&["index", "status", "--file", &file])["paths"]["database"]
        .as_str()
        .unwrap()
        .to_owned();
    let retired = || -> i64 {
        rusqlite::Connection::open(&db)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM generations WHERE state = 'retired'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    let kept = retired();
    let gate = env.gate("clear.hold");
    let mut mcp = Mcp::held(&env, &["--root", &root], Some(("clear_cache", &gate)));
    mcp.send(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "index", "arguments": {"action": "clear_cache", "workspaceId": ws}}}));
    let reached = PathBuf::from(format!("{}.reached", gate.display()));
    wait_for("clear_cache to choose what it deletes", || reached.exists());
    std::fs::rename(&file, &moved).unwrap();
    env.json(&["index", "relocate", &source, &moved, "--file", &file]);
    std::fs::remove_file(&gate).unwrap();
    let reply = loop {
        let reply = mcp.read();
        if reply["id"] == 1 {
            break reply["result"]["structuredContent"].clone();
        }
    };
    assert_eq!(
        (
            kept > 0,
            reply["partial"].clone(),
            reply["removed"]["generations"].clone(),
            retired()
        ),
        (true, json!(true), json!(0), kept)
    );
}

#[test]
fn a_retried_track_request_after_untrack_replays_its_job_and_never_tracks_again() {
    let env = Env::new();
    let fig = env.fig("a.fig");
    let file = fig.to_str().unwrap();
    env.json(&["index", "prepare", file, "--scope", "0:0"]);
    let track = |id: &str| {
        env.json(&[
            "index",
            "track",
            "--file",
            file,
            "--scope",
            "0:0",
            "--request-id",
            id,
        ])
    };
    let first = track("x");
    env.json(&["index", "untrack", "--file", file, "--scope", "0:0"]);
    let retried = track("x");
    let fresh = track("y");
    let outcome = |r: &Value| {
        (
            r["tracking"][0]["state"].clone(),
            r["jobs"][0]["jobId"] == first["jobs"][0]["jobId"],
        )
    };
    assert_eq!(
        (outcome(&retried), outcome(&fresh)),
        (
            (json!("disabled"), true),
            (json!("baseline_pending"), false)
        )
    );
}

#[test]
fn a_track_request_of_an_already_tracked_scope_keeps_its_id_and_replays_after_untrack() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let with = |action: &str, id: &str| {
        env.run(&[
            "index",
            action,
            "--file",
            &t.file,
            "--scope",
            "0:0",
            "--request-id",
            id,
        ])
    };
    // Fifty earlier ones that started no job: recording another retires the oldest.
    let db = rusqlite::Connection::open(&t.db).unwrap();
    for i in 0..50 {
        db.execute(
            "INSERT INTO requests VALUES (?1, 'f', '')",
            [format!("r{i}")],
        )
        .unwrap();
    }
    let noop: Value = serde_json::from_slice(&with("track", "z").stdout).unwrap();
    let kept: (i64, bool, bool) = db
        .query_row(
            "SELECT COUNT(*), SUM(request_id = 'r0'), SUM(request_id = 'z') FROM requests
             WHERE job_id = ''",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get::<_, i64>(1)? == 1,
                    r.get::<_, i64>(2)? == 1,
                ))
            },
        )
        .unwrap();
    drop(db);
    t.index("untrack");
    let retried: Value = serde_json::from_slice(&with("track", "z").stdout).unwrap();
    let reused = with("rebuild", "z");
    let state = |r: &Value| (r["tracking"][0]["state"].clone(), r["jobs"].clone());
    assert_eq!(
        (kept, state(&noop), state(&retried), reused.status.success()),
        (
            (50, false, true),
            (json!("active"), json!([])),
            (json!("disabled"), json!([])),
            false
        )
    );
}

#[test]
fn a_tracked_scope_deleted_from_the_file_records_its_removal() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let scope =
        |action: &str, node: &str| env.json(&["index", action, "--file", &t.file, "--scope", node]);
    scope("refresh", "0:13");
    scope("track", "0:13");
    t.save(true);
    scope("refresh", "0:13");
    let revisions = env.json(&["changes", "--file", &t.file, "--scope", "0:13"])["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| json!([r["kind"], r["detail"], r["summary"]["kinds"]]))
        .collect::<Vec<_>>();
    // A scope never tracked, and missing, is still refused.
    let refused = t.fails(&["index", "refresh", "--file", &t.file, "--scope", "0:99"]);
    assert_eq!(
        (revisions, refused.contains("ended \"failed\"")),
        (
            vec![
                json!(["baseline", "complete", null]),
                json!(["changes", "complete", {"removed": 1}])
            ],
            true
        )
    );
}

#[test]
fn a_failed_evaluation_stays_reported_by_changes_until_a_later_publication() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let (id, _) = t.query("badge");
    // Not evaluated at this publication, and its search fails: the file is gone.
    t.sql("UPDATE query_evaluations SET ranking = 'older'");
    std::fs::remove_file(&t.path).unwrap();
    let evaluated = t.watch("evaluate", &["--subscription", &id])["evaluation"].clone();
    let read = t.changes(&["--subscription", &id])["evaluation"].clone();
    assert_eq!(
        (
            evaluated["reason"].clone(),
            read["state"].clone(),
            read["reason"].clone()
        ),
        (
            json!("search_failed"),
            json!("unavailable"),
            json!("search_failed")
        ),
        "{evaluated} {read}"
    );
}

#[test]
fn a_tracked_scope_publishes_definition_fingerprints_as_an_untracked_build_does() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let conn = rusqlite::Connection::open(&t.db).unwrap();
    // The published generation is the tracked build's: it has a facet set.
    let prints: i64 = conn
        .query_row(
            "SELECT count(*) FROM fingerprints f JOIN published p ON p.gen = f.gen
             JOIN facet_sets s ON s.gen = p.gen",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(prints > 0, "{prints}");
}

#[test]
fn a_ranked_set_evaluation_with_a_transiently_stale_member_is_unavailable_not_stored() {
    let env = Env::new();
    let project = env.dir.join("project");
    std::fs::create_dir(&project).unwrap();
    let dir = std::fs::canonicalize(project).unwrap();
    let dir = dir.to_str().unwrap();
    let figs = ["a.fig", "b.fig"].map(|name| env.fig(name));
    for fig in &figs {
        env.json(&[
            "index",
            "prepare",
            fig.to_str().unwrap(),
            "--workspace",
            dir,
            "--scope",
            "0:0",
        ]);
    }
    let status = env.json(&["index", "status", "--workspace", dir]);
    let db = status["paths"]["database"].as_str().unwrap().to_owned();
    let b = std::fs::canonicalize(&figs[1]).unwrap();
    let b_id = status["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["path"] == b.to_str().unwrap())
        .unwrap()["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    // A build still running would stamp the touched time, so each step waits for jobs to finish.
    let idle = || {
        wait_for("jobs to finish", || {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.query_row(
                "SELECT count(*) FROM jobs WHERE state IN ('queued', 'running')",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
                == 0
        })
    };
    idle();
    // b keeps its bytes but not its modification time: the search stays ready over a alone.
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&figs[1])
        .unwrap()
        .set_modified(later)
        .unwrap();
    let saved = env.json(&[
        "watch",
        "subscribe",
        "--workspace",
        dir,
        "--consumer",
        "bot",
        "--sources",
        "visible",
        "--query",
        "first",
    ]);
    env.json(&[
        "index",
        "refresh",
        "--workspace",
        dir,
        "--source",
        &b_id,
        "--scope",
        "0:0",
    ]);
    idle();
    let id = saved["subscription"]["subscriptionId"].as_str().unwrap();
    let evaluated = env.json(&[
        "watch",
        "evaluate",
        "--workspace",
        dir,
        "--subscription",
        id,
    ]);
    assert_eq!(
        [
            &saved["evaluation"]["state"],
            &saved["evaluation"]["reason"],
            &evaluated["evaluation"]["state"]
        ],
        [&json!("unavailable"), &json!("stale"), &json!("evaluated")],
        "{saved} {evaluated}"
    );
}

#[test]
fn a_library_evaluation_over_a_transiently_stale_source_is_unavailable_not_stored() {
    let env = Env::new();
    let t = Tracked::new(&env);
    // A build still running would stamp the touched time, so each step waits for jobs to finish.
    let idle = || {
        wait_for("jobs to finish", || {
            let conn = rusqlite::Connection::open(&t.db).unwrap();
            conn.query_row(
                "SELECT count(*) FROM jobs WHERE state IN ('queued', 'running')",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
                == 0
        })
    };
    idle();
    // Same bytes, newer modification time: stale until a refresh reuses its generation.
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&t.path)
        .unwrap()
        .set_modified(later)
        .unwrap();
    let library = t.watch(
        "subscribe",
        &[
            "--consumer",
            "bot",
            "--sources",
            "visible",
            "--library",
            "component",
            "--namespace",
            "ns",
            "--key",
            "k",
        ],
    );
    t.index("refresh");
    idle();
    let id = library["subscription"]["subscriptionId"].as_str().unwrap();
    let evaluated = t.watch("evaluate", &["--subscription", id]);
    assert_eq!(
        [
            &library["evaluation"]["state"],
            &library["evaluation"]["reason"],
            &evaluated["evaluation"]["state"]
        ],
        [&json!("unavailable"), &json!("stale"), &json!("evaluated")],
        "{library} {evaluated}"
    );
}

#[test]
fn cli_subscribe_refuses_library_flags_without_library() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let refused = ["--namespace", "--key", "--definition-node"].map(|flag| {
        t.fails(&[
            "watch",
            "subscribe",
            "--file",
            &t.file,
            "--consumer",
            "bot",
            "--sources",
            "visible",
            "--query",
            "badge",
            flag,
            "k",
        ])
    });
    assert!(
        refused
            .iter()
            .all(|e| e.contains("applies only with --library")),
        "{refused:?}"
    );
}

#[test]
fn an_owner_unsubscribe_removes_its_source_set_subscription_and_names_it() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let set = t.watch(
        "subscribe",
        &[
            "--consumer",
            "bot",
            "--sources",
            "visible",
            "--query",
            "badge",
            "--owner",
            "task",
        ],
    )["subscription"]["subscriptionId"]
        .clone();
    let dropped = t.watch("unsubscribe", &["--consumer", "bot", "--owner", "task"]);
    assert_eq!(
        json!([dropped["removed"], dropped["subscriptionIds"]]),
        json!([1, [set]])
    );
}

#[test]
fn cli_and_mcp_save_questions_over_a_source_set_and_name_the_source_that_changed() {
    let env = Env::new();
    let t = Tracked::new(&env);
    let polled = t.changes(&[]);
    let (ws, source) = (
        polled["workspaceId"].clone(),
        polled["tracking"][0]["sourceId"].clone(),
    );
    let library = t.watch(
        "subscribe",
        &[
            "--consumer",
            "bot",
            "--sources",
            "visible",
            "--library",
            "component",
            "--namespace",
            "ns",
            "--key",
            "k",
        ],
    );
    let mut mcp = Mcp::start(&env, &[]);
    let saved = mcp.call(
        "watch",
        json!({"action": "subscribe", "workspaceId": ws, "consumer": "bot", "sources": "visible",
            "query": "badge"}),
    )["structuredContent"]
        .clone();
    t.save(true);
    t.index("refresh");
    let evaluated = mcp.call(
        "watch",
        json!({"action": "evaluate", "workspaceId": ws,
            "subscriptionId": saved["subscription"]["subscriptionId"]}),
    )["structuredContent"]
        .clone();
    assert_eq!(
        json!([
            library["subscription"]["sources"],
            library["subscription"]["library"],
            library["evaluation"]["state"],
            saved["subscription"]["sources"],
            saved["subscription"]["sourceId"],
            evaluated["evaluation"]["causes"][0],
            evaluated["evaluation"]["delta"]["changedSourceIds"],
        ]),
        json!([
            [source],
            {"kind": "component", "namespace": "ns", "key": "k"},
            "evaluated",
            [source],
            null,
            "source_changed",
            [source],
        ])
    );
}
