//! Whether a saved `.fig` is older than its online Figma file.
//!
//! The saved side is `exported_at` in the copy's `meta.json`, which Figma's Save local copy
//! writes; file times change on every checkout or copy, so they are never used. The online side
//! is `last_touched_at` from `GET /v1/files/:key/meta` (Tier 3, scope `file_metadata:read`),
//! fetched with `curl` and a personal access token from `FIGMA_TOKEN`. The token reaches curl on
//! stdin, never its command line, and never appears in output. Without a token, a saved time or
//! a recognizable URL, nothing is fetched and the state is `unknown`.

use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};

pub const TOKEN_VAR: &str = "FIGMA_TOKEN";
/// A project's mapping, at its repository root; each entry's `file` is relative to that root.
pub const MAPPING: &str = ".figlens/sources.json";
const MAPPING_BYTES: u64 = 1 << 20;
const MAX_ENTRIES: usize = 200;
const META_BYTES: u64 = 1 << 20;
/// The longest one request can take, so a listing starts none it cannot finish by its deadline.
pub const REQUEST_LIMIT: Duration = Duration::from_secs(15);
/// curl's limits for one metadata request: connect and total seconds, then response bytes.
const CONNECT_TIMEOUT: &str = "5";
const TIMEOUT: &str = "10";
const RESPONSE_BYTES: &str = "1048576";

/// One mapped design: its name, Figma URL and local copy, resolved against the mapping's root.
pub struct Source {
    pub name: String,
    pub url: String,
    /// The `file` exactly as the mapping wrote it, for replies.
    pub label: String,
    pub file: PathBuf,
}

/// The nearest `.figlens/sources.json` at or above `start` that `allowed` admits.
pub fn find_mapping(start: &Path, allowed: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    let start = std::fs::canonicalize(start).ok()?;
    start
        .ancestors()
        .map(|dir| dir.join(MAPPING))
        .find(|path| path.is_file() && allowed(path))
}

/// Reads a mapping: a JSON list of `{name, url, file}`, `file` relative to the project root for a
/// mapping in `.figlens/`, else to the mapping's own folder.
pub fn read_mapping(path: &Path) -> Result<Vec<Source>, Box<dyn Error>> {
    // Absolute first: the parents of a bare `sources.json` are not the folders above it.
    let path = &std::path::absolute(path)?;
    let shown = path.display();
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(MAPPING_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAPPING_BYTES {
        return Err(format!("{shown}: a mapping is at most 1 MiB").into());
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("{shown}: {e}"))?;
    let items = value
        .as_array()
        .ok_or_else(|| format!("{shown}: expected a JSON list of {{name, url, file}}"))?;
    if items.len() > MAX_ENTRIES {
        return Err(format!("{shown}: a mapping lists at most {MAX_ENTRIES} designs").into());
    }
    // `.figlens/` is figlens's folder at a project root; any other mapping names files from its own.
    let dir = path.parent().unwrap_or(Path::new("/"));
    let root = match dir.file_name() {
        Some(name) if name == ".figlens" => dir.parent().unwrap_or(dir),
        _ => dir,
    };
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let field = |key: &str| {
                item[key]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{shown}: entry {i} needs a non-empty string {key}"))
            };
            let label = field("file")?;
            Ok(Source {
                name: field("name")?,
                url: field("url")?,
                file: root.join(&label),
                label,
            })
        })
        .collect()
}

/// The REST file key of a Figma file URL; a branch URL names the branch's own key.
pub fn file_key(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("figma.com/")?;
    let mut parts = rest.split(['/', '?', '#']);
    if !matches!(parts.next()?, "design" | "file" | "proto") {
        return None;
    }
    let is_key = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric());
    let key = parts.next().filter(|k| is_key(k))?;
    // `/branch/<key>` only; a file whose name slug is `branch` keeps its own key.
    match (parts.next(), parts.next()) {
        (Some("branch"), Some(branch)) if is_key(branch) => Some(branch),
        _ => Some(key),
    }
}

/// When Figma saved this local copy: `exported_at` from its `meta.json`.
pub fn saved_at(fig: &Path) -> Option<String> {
    let mut archive = crate::open_zip(fig).ok()?;
    let mut text = String::new();
    archive
        .by_name("meta.json")
        .ok()?
        .take(META_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let at = value["exported_at"].as_str()?;
    millis(at).map(|_| at.to_owned())
}

/// The online file's `last_touched_at` and `version`, or the reason it is unknown.
fn online(curl: &str, key: &str, token: &str) -> Result<(String, Option<String>), &'static str> {
    // The token is quoted in curl's config; a value it would have to escape is not a token.
    if token
        .chars()
        .any(|c| matches!(c, '"' | '\\') || c.is_control())
    {
        return Err("token_invalid");
    }
    // `-q` first: a user's ~/.curlrc must not change the output this parses.
    let mut child = Command::new(curl)
        .args(["-q", "--silent", "--connect-timeout", CONNECT_TIMEOUT])
        .args(["--max-time", TIMEOUT, "--max-filesize", RESPONSE_BYTES])
        .args(["--config", "-", "--write-out", "\n%{http_code}"])
        .arg(format!("https://api.figma.com/v1/files/{key}/meta"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "curl_unavailable")?;
    let config = format!("header = \"X-Figma-Token: {token}\"\n");
    let written = child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(config.as_bytes()));
    let out = child.wait_with_output().map_err(|_| "offline")?;
    if !matches!(written, Some(Ok(()))) || !out.status.success() {
        return Err("offline");
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let (body, code) = text.rsplit_once('\n').ok_or("unexpected_response")?;
    match code.trim() {
        "200" => {}
        "401" | "403" => return Err("token_rejected"),
        "404" => return Err("file_not_found"),
        "429" => return Err("rate_limited"),
        _ => return Err("unexpected_response"),
    }
    let value: Value = serde_json::from_str(body).map_err(|_| "unexpected_response")?;
    let at = value["file"]["last_touched_at"]
        .as_str()
        .filter(|at| millis(at).is_some())
        .ok_or("unexpected_response")?;
    let version = value["file"]["version"].as_str().map(str::to_owned);
    Ok((at.to_owned(), version))
}

/// One design's freshness: `stale` when the online file changed after the copy was saved,
/// `current` when it did not, else `unknown` with a `reason`. `token` is the token, or the
/// reason not to fetch (`no_token`, `deadline`); then nothing is fetched.
pub fn check(curl: &str, source: &Source, token: Result<&str, &'static str>) -> Value {
    let mut report = unknown(source.name.as_str().into(), &source.label, "");
    // A known state carries no reason; every `unknown` below sets one.
    if let Some(fields) = report.as_object_mut() {
        fields.remove("reason");
    }
    let unknown = |mut report: Value, reason: &str| {
        report["reason"] = reason.into();
        report
    };
    if !source.file.is_file() {
        return unknown(report, "file_missing");
    }
    let Some(saved) = saved_at(&source.file) else {
        return unknown(report, "no_saved_time");
    };
    report["savedAt"] = saved.as_str().into();
    let Some(key) = file_key(&source.url) else {
        return unknown(report, "url_unrecognized");
    };
    match token.and_then(|token| online(curl, key, token)) {
        Err(reason) => unknown(report, reason),
        Ok((at, version)) => {
            let stale = newer(&at, &saved);
            report["onlineModifiedAt"] = at.into();
            report["onlineVersion"] = version.into();
            report["state"] = if stale { "stale" } else { "current" }.into();
            report
        }
    }
}

/// A design report with nothing known: `state` `unknown` for `reason`, every time null.
pub fn unknown(name: Value, label: &str, reason: &str) -> Value {
    json!({
        "name": name,
        "file": label,
        "savedAt": null,
        "onlineModifiedAt": null,
        "onlineVersion": null,
        "state": "unknown",
        "reason": reason,
    })
}

/// Whether RFC 3339 time `a` is later than `b`; an unreadable time is never later.
pub fn newer(a: &str, b: &str) -> bool {
    millis(a).zip(millis(b)).is_some_and(|(a, b)| a > b)
}

/// `FIGMA_TOKEN` when set and non-empty.
pub fn token() -> Option<String> {
    std::env::var(TOKEN_VAR).ok().filter(|t| !t.is_empty())
}

/// A mapping's path and the entry naming a file, when one does.
pub type Mapped = (PathBuf, Option<Source>);

/// The nearest mapping at or above `fig`'s folder and the entry naming `fig`, if any; `None`
/// when no admitted mapping is found or `fig` does not exist.
pub fn source_for(
    fig: &Path,
    allowed: &dyn Fn(&Path) -> bool,
) -> Result<Option<Mapped>, Box<dyn Error>> {
    let Ok(fig) = std::fs::canonicalize(fig) else {
        return Ok(None);
    };
    let Some(mapping) = fig.parent().and_then(|dir| find_mapping(dir, allowed)) else {
        return Ok(None);
    };
    let source = read_mapping(&mapping)?
        .into_iter()
        .find(|s| std::fs::canonicalize(&s.file).is_ok_and(|f| f == fig));
    Ok(Some((mapping, source)))
}

/// How long a hint's answer for one saved copy is reused: an agent trying several ids in a
/// stale copy makes one request, not one per id.
const HINT_TTL: Duration = Duration::from_secs(60);

/// A copy's stale report by path and file stamp, so a re-saved copy is checked again.
type HintCache = HashMap<(PathBuf, Option<SystemTime>, u64), (Instant, Option<Value>)>;
static HINTS: Mutex<Option<HintCache>> = Mutex::new(None);

/// The mapped design `fig` belongs to, checked online, only when it is known to be stale.
/// No token, no mapping or an unmapped file makes no network call; an answer is reused for
/// `HINT_TTL`.
pub fn stale(fig: &Path, allowed: &dyn Fn(&Path) -> bool) -> Option<Value> {
    let token = token()?;
    let fig = std::fs::canonicalize(fig).ok()?;
    let meta = std::fs::metadata(&fig).ok()?;
    let key = (fig.clone(), meta.modified().ok(), meta.len());
    // Held across the request, so concurrent misses (on any copy) wait and reuse its answer.
    let mut hints = HINTS.lock().ok()?;
    let hints = hints.get_or_insert_with(HashMap::new);
    hints.retain(|_, (at, _)| at.elapsed() < HINT_TTL);
    if let Some((_, report)) = hints.get(&key) {
        return report.clone();
    }
    let report = match source_for(&fig, allowed) {
        Ok(Some((_, Some(source)))) => {
            let report = check("curl", &source, Ok(&token));
            (report["state"] == "stale").then_some(report)
        }
        _ => None,
    };
    hints.insert(key, (Instant::now(), report.clone()));
    report
}

/// The sentence a stale copy adds to a not-found error or an empty find.
pub fn stale_hint(report: &Value) -> String {
    format!(
        "the online Figma file changed after this local copy was saved (saved {}, online {}); \
         save a new local copy (File > Save local copy) and retry before treating it as missing",
        report["savedAt"].as_str().unwrap_or("?"),
        report["onlineModifiedAt"].as_str().unwrap_or("?"),
    )
}

/// Milliseconds since the Unix epoch of an RFC 3339 time such as `2026-10-21T18:27:49.965Z`.
fn millis(at: &str) -> Option<i64> {
    let digits = |from: usize, to: usize| {
        let s = at.get(from..to)?;
        s.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| s.parse::<i64>().ok())?
    };
    let b = at.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (y, mo, d) = (digits(0, 4)?, digits(5, 7)?, digits(8, 10)?);
    let (h, mi, s) = (digits(11, 13)?, digits(14, 16)?, digits(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let mut rest = &at[19..];
    let mut ms = 0;
    if let Some(fraction) = rest.strip_prefix('.') {
        let n = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            return None;
        }
        let first: String = fraction[..n].chars().chain("00".chars()).take(3).collect();
        ms = first.parse::<i64>().ok()?;
        rest = &fraction[n..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (oh, om) = (rest.get(1..3)?, rest.get(4..6)?);
            if rest.len() != 6 || rest.as_bytes()[3] != b':' {
                return None;
            }
            sign * (oh.parse::<i64>().ok()? * 60 + om.parse::<i64>().ok()?)
        }
    };
    // Days from 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's days_from_civil).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((mo + 9) % 12) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(((days * 24 + h) * 60 + mi - offset) * 60_000 + s * 1000 + ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestDir;

    #[test]
    fn times_compare_across_precision_and_offsets() {
        assert_eq!(millis("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(millis("2026-10-21T18:27:49.965Z"), Some(1_792_607_269_965));
        assert_eq!(millis("2026-10-21T18:27:49Z"), Some(1_792_607_269_000));
        assert_eq!(
            millis("2026-10-22T02:27:49.965+08:00"),
            millis("2026-10-21T18:27:49.965Z")
        );
        assert_eq!(millis("2026-10-21 18:27:49Z"), None);
        assert_eq!(millis("2026-13-21T18:27:49Z"), None);
    }

    #[test]
    fn file_keys_come_from_design_file_proto_and_branch_urls() {
        let key = |url| file_key(url);
        assert_eq!(
            key("https://www.figma.com/design/AbC123/Name?node-id=1-2"),
            Some("AbC123")
        );
        assert_eq!(key("https://figma.com/file/AbC123"), Some("AbC123"));
        assert_eq!(
            key("https://www.figma.com/design/AbC123/branch/Br4nch/Name"),
            Some("Br4nch")
        );
        assert_eq!(
            key("https://www.figma.com/design/AbC123/branch?node-id=1-2"),
            Some("AbC123")
        );
        assert_eq!(key("https://www.figma.com/community/file/123"), None);
        assert_eq!(key("https://www.figma.com/design/../x"), None);
    }

    #[test]
    fn a_figlens_mapping_names_files_from_the_project_root() {
        let dir = TestDir::new();
        std::fs::create_dir(dir.join(".figlens")).unwrap();
        let path = dir.join(".figlens/sources.json");
        std::fs::write(
            &path,
            r#"[{"name":"home","url":"u","file":"figma/home.fig"}]"#,
        )
        .unwrap();
        let sources = read_mapping(&path).unwrap();
        let expected = std::path::absolute(dir.join("figma/home.fig")).unwrap();
        assert_eq!(sources[0].file, expected);
    }

    #[test]
    fn another_mapping_names_files_from_its_own_folder() {
        let dir = TestDir::new();
        let path = dir.join("designs.json");
        std::fs::write(&path, r#"[{"name":"home","url":"u","file":"home.fig"}]"#).unwrap();
        let sources = read_mapping(&path).unwrap();
        assert_eq!(
            sources[0].file,
            std::path::absolute(dir.join("home.fig")).unwrap()
        );
    }

    #[test]
    fn a_mapping_entry_without_a_file_is_rejected() {
        let dir = TestDir::new();
        let path = dir.join("sources.json");
        std::fs::write(&path, r#"[{"name":"home","url":"u"}]"#).unwrap();
        assert!(read_mapping(&path).is_err());
    }

    /// A copy of the public fixture whose `meta.json` says it was saved at `at`.
    fn saved(dir: &TestDir, at: &str) -> PathBuf {
        let mut source = crate::open_zip(Path::new("examples/first-success/design.fig")).unwrap();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for i in 0..source.len() {
            zip.raw_copy_file(source.by_index_raw(i).unwrap()).unwrap();
        }
        zip.start_file("meta.json", options).unwrap();
        zip.write_all(json!({"exported_at": at}).to_string().as_bytes())
            .unwrap();
        let path = dir.join("design.fig");
        std::fs::write(&path, zip.finish().unwrap().into_inner()).unwrap();
        path
    }

    /// A `curl` stand-in answering `status` with `body`; it logs its arguments and stdin.
    #[cfg(unix)]
    fn curl(dir: &TestDir, status: &str, body: &str) -> String {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(dir.join("body"), body).unwrap();
        let script = dir.join("curl");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nd=$(dirname \"$0\")\necho \"$*\" > \"$d/args\"\ncat > \"$d/stdin\"\n\
                 cat \"$d/body\"\nprintf '\\n{status}'\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script.to_str().unwrap().to_owned()
    }

    const URL: &str = "https://www.figma.com/design/AbC123/Home";

    fn home(file: PathBuf) -> Source {
        Source {
            name: "home".into(),
            url: URL.into(),
            label: "home.fig".into(),
            file,
        }
    }
    const SAVED: &str = "2026-10-21T18:27:49.965Z";

    fn meta(at: &str) -> String {
        json!({"file": {"last_touched_at": at, "version": "42"}}).to_string()
    }

    #[test]
    fn the_public_fixture_has_no_saved_time() {
        assert_eq!(
            saved_at(Path::new("examples/first-success/design.fig")),
            None
        );
    }

    #[test]
    fn a_saved_copy_reports_its_exported_at() {
        let dir = TestDir::new();
        assert_eq!(saved_at(&saved(&dir, SAVED)).as_deref(), Some(SAVED));
    }

    #[cfg(unix)]
    #[test]
    fn an_online_change_after_the_save_is_stale() {
        let dir = TestDir::new();
        let (fig, curl) = (
            saved(&dir, SAVED),
            curl(&dir, "200", &meta("2026-10-21T18:40:00Z")),
        );
        let report = check(&curl, &home(fig), Ok("tok"));
        assert_eq!(report["state"], "stale", "{report}");
    }

    #[cfg(unix)]
    #[test]
    fn no_online_change_since_the_save_is_current() {
        let dir = TestDir::new();
        let (fig, curl) = (
            saved(&dir, SAVED),
            curl(&dir, "200", &meta("2026-10-21T18:00:00Z")),
        );
        let report = check(&curl, &home(fig), Ok("tok"));
        assert_eq!(report["state"], "current", "{report}");
    }

    #[cfg(unix)]
    #[test]
    fn the_token_reaches_curl_on_stdin_only() {
        let dir = TestDir::new();
        let (fig, curl) = (saved(&dir, SAVED), curl(&dir, "200", &meta(SAVED)));
        let report = check(&curl, &home(fig), Ok("secret-tok"));
        let args = std::fs::read_to_string(dir.join("args")).unwrap();
        let stdin = std::fs::read_to_string(dir.join("stdin")).unwrap();
        assert!(
            !args.contains("secret-tok")
                && !report.to_string().contains("secret-tok")
                && stdin.contains("X-Figma-Token: secret-tok"),
            "{args} {report}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_rejected_token_is_unknown_with_its_reason() {
        let dir = TestDir::new();
        let (fig, curl) = (saved(&dir, SAVED), curl(&dir, "403", "{}"));
        let report = check(&curl, &home(fig), Ok("tok"));
        assert_eq!(report["reason"], "token_rejected", "{report}");
    }

    #[test]
    fn no_token_is_unknown_without_a_request() {
        let dir = TestDir::new();
        let fig = saved(&dir, SAVED);
        let report = check("/nonexistent/curl", &home(fig), Err("no_token"));
        assert_eq!(report["reason"], "no_token", "{report}");
    }
}
