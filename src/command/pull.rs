//! `pull`: refresh mapped `.fig` copies through a browser helper (#225).
//!
//! figlens owns the mapping, the selection, the profile lock, validation and the atomic replace;
//! the helper (`FIGLENS_PULL_HELPER`, a Node + playwright-core script the npm package ships) only
//! drives Figma's File > Save local copy in a dedicated browser profile. The helper reads one
//! JSON request on stdin and answers one JSON line per design on stdout; its stderr is the
//! person's progress log.

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use figlens::freshness::{self, MAPPING, Source};
use serde_json::{Value, json};

/// The helper the npm launcher points at; a native-only install sets it by hand.
pub const HELPER_VAR: &str = "FIGLENS_PULL_HELPER";

/// What to pull: the designs named in `only`, or only stale ones; with neither, nothing is
/// downloaded and the mapped designs are listed with their freshness.
pub struct Request<'a> {
    pub sources: Option<&'a Path>,
    pub only: Vec<&'a str>,
    pub stale: bool,
    pub login: bool,
    pub start: &'a Path,
}

/// Pulls the requested designs and reports each; an error after the report means some failed.
pub fn run(request: &Request, state: &Path) -> Result<Value, Box<dyn Error>> {
    // Signing in needs no project.
    let mapping = if request.login {
        None
    } else {
        Some(match request.sources {
            Some(path) => path.to_owned(),
            None => freshness::find_mapping(request.start, &|_| true).ok_or_else(|| {
                format!("no {MAPPING} at or above this directory; pass --sources <mapping.json>")
            })?,
        })
    };
    if let Some(mapping) = &mapping
        && request.only.is_empty()
        && !request.stale
    {
        let listing = super::freshness::Request {
            sources: Some(mapping),
            file: None,
            url: None,
            start: None,
        };
        let mut report = super::freshness::report(&listing, &|_| true, &AtomicBool::new(false))?;
        report["hint"] =
            "nothing was downloaded; pass --only NAME (repeatable) or --stale to pull".into();
        return Ok(report);
    }
    let sources = match &mapping {
        Some(mapping) => freshness::read_mapping(mapping)?,
        None => Vec::new(),
    };
    let selected = select(&sources, request)?;
    let helper = std::env::var_os(HELPER_VAR).map(PathBuf::from).ok_or_else(|| {
        format!(
            "pull needs its browser helper: install figlens with npm (npm i -g @runkids/figlens playwright-core), or set {HELPER_VAR} to its pull/helper.mjs"
        )
    })?;
    let dir = state.join("pull");
    // Downloads are whole designs and the profile holds Figma's login cookies.
    private_dir(&dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("lock"))?;
    // The OS drops the lock when this process ends, however it ends.
    lock.try_lock().map_err(|_| {
        "another figlens pull is running; it shares one browser profile, so wait for it to finish"
    })?;
    let downloads = dir.join("downloads");
    // Held under the lock, so anything left here is from a run that stopped early.
    let _ = std::fs::remove_dir_all(&downloads);
    std::fs::create_dir_all(&downloads)?;
    let profile = dir.join("profile");
    private_dir(&profile)?;
    let wanted: Vec<&Source> = selected
        .iter()
        .filter_map(|(s, skip)| skip.is_none().then_some(*s))
        .collect();
    let outcome = helper_run(&helper, &profile, &downloads, request.login, &wanted);
    let _ = std::fs::remove_dir_all(&downloads);
    let (mut results, stopped) = outcome?;
    let Some(mapping) = mapping else {
        return Ok(json!({"login": "signed in"}));
    };
    let designs: Vec<Value> = selected
        .iter()
        .map(|(source, skip)| {
            let base = json!({"name": source.name, "file": source.label});
            match skip {
                Some(reason) => with(base, "skipped", reason),
                None => results
                    .remove(&source.name)
                    .unwrap_or_else(|| failed(base, "helper", &stopped)),
            }
        })
        .collect();
    let failed = designs.iter().filter(|d| d["state"] == "failed").count();
    let report = json!({"mapping": mapping.to_string_lossy(), "designs": designs});
    if failed > 0 {
        return Err(Failed { report, failed }.into());
    }
    Ok(report)
}

/// A report in which some designs failed; the CLI prints the report, then fails.
#[derive(Debug)]
pub struct Failed {
    pub report: Value,
    pub failed: usize,
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "pull failed for {} design(s); the report lists why, and each failed copy was left as it was",
            self.failed
        )
    }
}

impl Error for Failed {}

fn with(mut base: Value, state: &str, reason: &str) -> Value {
    base["state"] = state.into();
    base["reason"] = reason.into();
    base
}

/// A failed design; `stage` names the step that failed, so a changed Figma UI (`menu`) reads
/// differently from a slow export (`export`) or a rejected download (`install`).
fn failed(base: Value, stage: &str, reason: &str) -> Value {
    let mut value = with(base, "failed", reason);
    value["stage"] = stage.into();
    value
}

/// A requested source and the reason it is skipped, if it is.
type Selected<'a> = (&'a Source, Option<String>);

/// Each requested source, with the reason it is skipped (`current`, or why staleness is unknown).
fn select<'a>(
    sources: &'a [Source],
    request: &Request,
) -> Result<Vec<Selected<'a>>, Box<dyn Error>> {
    let missing: Vec<_> = request
        .only
        .iter()
        .filter(|name| !sources.iter().any(|s| s.name == **name))
        .collect();
    if !missing.is_empty() {
        let names: Vec<_> = missing.iter().map(|n| n.to_string()).collect();
        return Err(format!("no design named {} in the mapping", names.join(", ")).into());
    }
    if request.login {
        return Ok(Vec::new());
    }
    let chosen: Vec<&Source> = sources
        .iter()
        .filter(|s| request.only.is_empty() || request.only.contains(&s.name.as_str()))
        .collect();
    for (index, source) in chosen.iter().enumerate() {
        // Downloads come back by name, so one name must mean one file.
        if chosen[..index].iter().any(|s| s.name == source.name) {
            return Err(format!("the mapping names {} more than once", source.name).into());
        }
        if freshness::file_key(&source.url).is_none() {
            return Err(format!("{}: url is not a Figma design or file URL", source.name).into());
        }
        // A pull overwrites the file, so a cloned mapping cannot aim it outside its project.
        let inside = Path::new(&source.label)
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
        if !inside {
            return Err(format!(
                "{}: file {} leaves the mapping's folder; pull writes only below it",
                source.name, source.label
            )
            .into());
        }
    }
    let chosen = chosen.into_iter();
    if !request.stale {
        return Ok(chosen.map(|s| (s, None)).collect());
    }
    let token = freshness::token().ok_or_else(|| {
        format!(
            "--stale compares online times, which needs {}",
            freshness::TOKEN_VAR
        )
    })?;
    Ok(chosen
        .map(|s| {
            let report = freshness::check("curl", s, Ok(&token));
            let pull = report["state"] == "stale" || report["reason"] == "file_missing";
            let skip = (!pull).then(|| match report["state"].as_str() {
                Some("current") => "current".to_owned(),
                _ => format!(
                    "staleness unknown: {}",
                    report["reason"].as_str().unwrap_or("?")
                ),
            });
            (s, skip)
        })
        .collect())
}

/// A directory only this user can read: the profile holds Figma's login cookies.
fn private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Downloads installed or failed by design name, and why the helper stopped early.
type Outcome = (std::collections::HashMap<String, Value>, String);

/// Runs the helper and installs each download as it arrives, keyed by design name. The helper's
/// stdin stays open until it is done, so it can exit when figlens goes away.
fn helper_run(
    helper: &Path,
    profile: &Path,
    downloads: &Path,
    login: bool,
    sources: &[&Source],
) -> Result<Outcome, Box<dyn Error>> {
    let mut results = std::collections::HashMap::new();
    if sources.is_empty() && !login {
        return Ok((results, String::new()));
    }
    let script = helper.extension().is_some_and(|e| e == "mjs" || e == "js");
    let mut command = if script {
        let mut node = Command::new("node");
        node.arg(helper);
        node
    } else {
        Command::new(helper)
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start the pull helper {}: {e}", helper.display()))?;
    // Progress passes through to the person; the last line explains an early stop.
    let last = Arc::new(Mutex::new(String::new()));
    let relay = child.stderr.take().map(|stderr| {
        let last = Arc::clone(&last);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("{line}");
                if !line.trim().is_empty()
                    && let Ok(mut last) = last.lock()
                {
                    *last = line;
                }
            }
        })
    });
    let read = read_events(&mut child, profile, downloads, login, sources, &mut results);
    if read.is_err() {
        let _ = child.kill();
    }
    let status = child.wait();
    if let Some(relay) = relay {
        let _ = relay.join();
    }
    read?;
    let status = status?;
    let last = last.lock().map(|l| l.clone()).unwrap_or_default();
    let stopped = match (status.success(), last.is_empty()) {
        (true, _) => "the helper stopped before this design".to_owned(),
        (false, true) => format!("the helper stopped before this design ({status})"),
        (false, false) => format!("the helper stopped before this design: {last}"),
    };
    if login && !status.success() {
        return Err(format!(
            "the Figma login did not finish ({last}); run figlens pull --login again"
        )
        .into());
    }
    Ok((results, stopped))
}

/// Sends the request and installs each download the helper reports.
fn read_events(
    child: &mut Child,
    profile: &Path,
    downloads: &Path,
    login: bool,
    sources: &[&Source],
    results: &mut std::collections::HashMap<String, Value>,
) -> Result<(), Box<dyn Error>> {
    let designs: Vec<Value> = sources
        .iter()
        .map(|s| {
            let key = freshness::file_key(&s.url).unwrap_or_default();
            json!({"name": s.name, "url": format!("https://www.figma.com/design/{key}/")})
        })
        .collect();
    let request = json!({
        "profile": profile,
        "downloads": downloads,
        "login": login,
        "designs": designs,
    });
    // Kept until stdout ends; a helper that already exited is reported by its status instead.
    let mut stdin = child.stdin.take();
    if let Some(pipe) = stdin.as_mut()
        && writeln!(pipe, "{request}").is_err()
    {
        stdin = None;
    }
    let stdout = child.stdout.take().ok_or("the pull helper has no stdout")?;
    for line in BufReader::new(stdout).lines() {
        let Ok(event) = serde_json::from_str::<Value>(&line?) else {
            continue;
        };
        let Some(source) = event["name"]
            .as_str()
            .and_then(|name| sources.iter().find(|s| s.name == name))
        else {
            continue;
        };
        let base = json!({"name": source.name, "file": source.label});
        let result = match (event["path"].as_str(), event["error"].as_str()) {
            (Some(path), _) => match install(Path::new(path), downloads, &source.file) {
                Ok(saved) => {
                    let mut done = base;
                    done["state"] = "pulled".into();
                    done["savedAt"] = saved.into();
                    done
                }
                Err(e) => failed(base, "install", &e.to_string()),
            },
            (None, Some(error)) => failed(base, event["stage"].as_str().unwrap_or("helper"), error),
            (None, None) => continue,
        };
        results.insert(source.name.clone(), result);
    }
    drop(stdin);
    Ok(())
}

/// Validates a download as a decodable copy no older than the current one, then replaces the
/// target atomically, keeping its permissions and any symlink. A failed check leaves the target
/// as it was. Returns the new saved time.
fn install(download: &Path, downloads: &Path, target: &Path) -> Result<String, Box<dyn Error>> {
    // Only files the helper wrote into this run's folder are taken.
    let download = std::fs::canonicalize(download)?;
    if !download.starts_with(std::fs::canonicalize(downloads)?) {
        return Err("the helper named a file outside its download folder".into());
    }
    figlens::Document::open(&download)
        .and_then(|mut doc| doc.take_message().map(|_| ()))
        .map_err(|e| format!("the download is not a readable .fig: {e}"))?;
    let saved = freshness::saved_at(&download).ok_or(
        "the download has no saved time (meta.json exported_at); it is not a Save local copy",
    )?;
    if let Some(current) = freshness::saved_at(target)
        && freshness::newer(&current, &saved)
    {
        return Err(format!(
            "the download (saved {saved}) is older than the current copy (saved {current})"
        )
        .into());
    }
    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    figlens::output::write(&download, target, |file| {
        std::io::copy(&mut File::open(&download)?, file)?;
        Ok(())
    })?;
    Ok(saved)
}
