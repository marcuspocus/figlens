//! The agent activity journal for terminal hosts.
//!
//! When the MCP server runs inside a Herdr terminal (`HC_TERMINAL_ID` is set), every successful
//! tool call that reads the design's nodes appends one JSON line to
//! `<state>/activity/<design key>.jsonl`: when, which terminal, which tool, the design's path,
//! the node ids it named and, for one node, the screen that holds it and its box in that screen.
//! Of the design's content only node ids: never names, text or images. The Herdr panel reads
//! these files to show which screens agents are reading. Everywhere else nothing is written.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use figlens::Tree;
use figlens::expand::{abs_matrix, parent_of, plugin_type, vec2};
use figlens::normalize_id;
use serde_json::json;
use sha2::{Digest, Sha256};

/// A journal over this many bytes moves to `<key>.1.jsonl` (replacing the older one) before
/// the next line, so each design keeps at most about twice this.
const MAX_BYTES: u64 = 256 * 1024;
/// Journals untouched this long go when a new one starts: a design copy gets a new path, and so
/// a new journal, for each saved version, so the folder would otherwise only grow.
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Node ids kept per line; with ids only (no URLs or names) a line stays a few kilobytes.
const MAX_NODES: usize = 20;

pub(crate) struct Journal {
    dir: PathBuf,
    terminal: String,
}

/// This process's journal: `None` unless the server runs in a Herdr terminal.
pub(crate) fn journal() -> Option<&'static Journal> {
    static JOURNAL: OnceLock<Option<Journal>> = OnceLock::new();
    JOURNAL
        .get_or_init(|| {
            let terminal = terminal(std::env::var("HC_TERMINAL_ID").ok())?;
            let state = figlens::workspace::storage(None).ok()?;
            Some(Journal::new(state.state_dir().join("activity"), terminal))
        })
        .as_ref()
}

/// A Herdr terminal address (`t7`, `mac-mini/t1r`); anything else turns the journal off.
fn terminal(raw: Option<String>) -> Option<String> {
    raw.filter(|t| {
        !t.is_empty()
            && t.len() <= 64
            && t.chars()
                .all(|c| c.is_ascii_alphanumeric() || "/-_.:".contains(c))
    })
}

/// The journal file name for a design: the first 16 hex digits of the SHA-256 of its path as
/// figlens resolved it (canonical: symlinks resolved), so a host finds it from the same path.
fn key(file: &Path) -> String {
    Sha256::digest(file.to_string_lossy().as_bytes())[..8]
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// The node ids a call named, as figlens reads them (`12-3` and Figma links become `12:3`);
/// anything that is not an id is left out, so a link's file key and name never reach a line.
pub(crate) fn ids<'a>(raw: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in raw.into_iter().map(normalize_id) {
        if is_id(&id) && !out.contains(&id) && out.len() < MAX_NODES {
            out.push(id);
        }
    }
    out
}

/// `12:3`, or an instance layer's path `I12:3;45:6`.
fn is_id(id: &str) -> bool {
    let pair = |p: &str| {
        p.split_once(':').is_some_and(|(a, b)| {
            [a, b]
                .iter()
                .all(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
        })
    };
    let path = id.strip_prefix('I').unwrap_or(id);
    id.len() <= 256 && path.split(';').all(pair)
}

impl Journal {
    fn new(dir: PathBuf, terminal: String) -> Journal {
        Journal { dir, terminal }
    }

    /// Appends one line for a call on `file`. A failure is reported once on stderr (stdout is
    /// the MCP protocol) and never fails the call.
    pub(crate) fn record(&self, tool: &str, file: &Path, tree: &Tree, nodes: &[String]) {
        static WARNED: AtomicBool = AtomicBool::new(false);
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let line = line(&self.terminal, tool, file, tree, nodes, at);
        if let Err(e) = self.append(file, &line)
            && !WARNED.swap(true, Ordering::Relaxed)
        {
            // Not eprintln!: a closed stderr must not turn a finished call into a crash.
            let _ = writeln!(
                std::io::stderr(),
                "figlens: activity journal not written in {}: {e}",
                self.dir.display()
            );
        }
    }

    fn append(&self, file: &Path, line: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let key = key(file);
        let path = self.dir.join(format!("{key}.jsonl"));
        match std::fs::metadata(&path) {
            // A new journal: the moment to drop old ones.
            Err(_) => prune(&self.dir, SystemTime::now()),
            // Best effort: when two servers rotate at once, the later one may move the other's
            // fresh file over the older history. A failed move (another server moved it, or a
            // reader holds it open on Windows) still lets this line be written.
            Ok(m) if m.len() > MAX_BYTES => {
                let _ = std::fs::rename(&path, self.dir.join(format!("{key}.1.jsonl")));
            }
            Ok(_) => {}
        }
        append_line(&path, line)
    }
}

/// One write of one short line in append mode: concurrent servers never interleave it.
fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(line.as_bytes())
}

/// Removes journals (and their rotated halves) not written for `MAX_AGE`, best effort.
fn prune(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|at| now.duration_since(at).ok())
            .is_some_and(|age| age > MAX_AGE);
        if stale && path.extension().is_some_and(|e| e == "jsonl") {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// One journal line, newline included.
fn line(terminal: &str, tool: &str, file: &Path, tree: &Tree, ids: &[String], at: u64) -> String {
    let mut value = json!({
        "v": 1, "at": at, "terminal": terminal, "tool": tool,
        "file": file.to_string_lossy(), "nodes": ids,
    });
    if let [id] = ids {
        // A layer inside an instance (`I12:3;45:6`) is not stored: place it by its outermost
        // instance, without a box.
        let stored = figlens::export::root_of(id);
        if let Some(screen) = screen(tree, stored) {
            if stored == id
                && let Some(b) = box_in(tree, id, &screen)
            {
                value["box"] = json!(b);
            }
            value["screen"] = json!(screen);
        }
    }
    format!("{value}\n")
}

/// The screen that holds `id`, as `tree --top-frames` names screens: the first frame,
/// component, component set or instance on the way up whose parent is a page that is not
/// internal, or a section on one (at any section depth).
fn screen(tree: &Tree, id: &str) -> Option<String> {
    let kind = |id: &str| tree.nodes.get(id).map(|n| plugin_type(n));
    let up = |id: &str| tree.nodes.get(id).and_then(|n| parent_of(n));
    let mut cur = id.to_owned();
    // Tree::new drops parent cycles; the bound only guards a pathological depth.
    for _ in 0..4096 {
        let parent = up(&cur)?;
        let screen_kind = matches!(
            kind(&cur)?.as_str(),
            "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE"
        );
        if screen_kind && on_page(tree, &parent) {
            return Some(cur);
        }
        cur = parent;
    }
    None
}

/// Whether `id` is a page that is not internal, or a section on one through sections only.
fn on_page(tree: &Tree, id: &str) -> bool {
    let mut cur = id.to_owned();
    for _ in 0..4096 {
        let Some(node) = tree.nodes.get(cur.as_str()) else {
            return false;
        };
        match plugin_type(node).as_str() {
            "CANVAS" => return node.get("internalOnly").map(|v| v.as_bool()) != Some(true),
            "SECTION" => match parent_of(node) {
                Some(parent) => cur = parent,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// `id`'s layout box in `screen`'s space, `[x, y, width, height]`, rounded to 0.01.
fn box_in(tree: &Tree, id: &str, screen: &str) -> Option<[f64; 4]> {
    let s = vec2(tree.nodes.get(id)?.get("size")?);
    let m = abs_matrix(tree, id, screen);
    let (o, b) = (m.origin(s), m.bbox(s));
    // `+ 0.0` turns -0.0 into 0.0.
    let r = |v: f64| (v * 100.0).round() / 100.0 + 0.0;
    Some([r(o.x), r(o.y), r(b.x), r(b.y)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use figlens::Document;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/first-success/design.fig"
    );

    fn with_tree(f: impl FnOnce(&Tree)) {
        let mut doc = Document::open(Path::new(FIXTURE)).unwrap();
        let message = doc.take_message().unwrap();
        f(&Tree::new(&message));
    }

    fn parse(line: &str) -> serde_json::Value {
        assert!(line.ends_with('\n'));
        serde_json::from_str(line.trim_end()).unwrap()
    }

    #[test]
    fn a_nested_node_names_its_screen_and_its_box_there() {
        with_tree(|tree| {
            let v = parse(&line(
                "t7",
                "inspect",
                Path::new("/d/design.fig"),
                tree,
                &ids(["0:4"]),
                5,
            ));
            assert_eq!(v["screen"], "0:1");
            assert_eq!(v["box"], json!([80.0, 32.0, 120.0, 28.0]));
        });
    }

    #[test]
    fn a_screen_is_its_own_screen_at_the_origin() {
        with_tree(|tree| {
            let v = parse(&line(
                "t7",
                "export",
                Path::new("/d/design.fig"),
                tree,
                &ids(["0:1"]),
                5,
            ));
            assert_eq!(
                (v["screen"].clone(), v["box"].clone()),
                (json!("0:1"), json!([0.0, 0.0, 320.0, 180.0]))
            );
        });
    }

    #[test]
    fn a_line_carries_ids_only_and_no_box_for_several_nodes() {
        with_tree(|tree| {
            let v = parse(&line(
                "mac-mini/t1r",
                "export_batch",
                Path::new("/d/design.fig"),
                tree,
                &ids(["0:3", "0:4", "0:3"]),
                5,
            ));
            assert_eq!(
                v,
                json!({"v":1,"at":5,"terminal":"mac-mini/t1r","tool":"export_batch","file":"/d/design.fig","nodes":["0:3","0:4"]})
            );
        });
    }

    #[test]
    fn a_line_keeps_at_most_twenty_ids() {
        with_tree(|tree| {
            let raw: Vec<String> = (0..30).map(|i| format!("9:{i}")).collect();
            let v = parse(&line(
                "t7",
                "nodes",
                Path::new("/d/a.fig"),
                tree,
                &ids(raw.iter().map(String::as_str)),
                5,
            ));
            assert_eq!(v["nodes"].as_array().unwrap().len(), MAX_NODES);
        });
    }

    #[test]
    fn figma_links_and_dashed_ids_are_kept_as_ids_only() {
        let link = "https://www.figma.com/design/KEY123/Private-App?node-id=12-3&t=x";
        assert_eq!(
            ids([link, "12-3", "I1:2;3:4", "Checkout screen", "0:4"]),
            ["12:3", "I1:2;3:4", "0:4"]
        );
    }

    #[test]
    fn a_layer_inside_an_instance_names_its_instances_screen_without_a_box() {
        with_tree(|tree| {
            let v = parse(&line(
                "t7",
                "inspect",
                Path::new("/d/design.fig"),
                tree,
                &ids(["I0:2;9:9"]),
                5,
            ));
            assert_eq!((v["screen"].clone(), v.get("box")), (json!("0:1"), None));
        });
    }

    #[test]
    fn only_a_herdr_terminal_address_turns_the_journal_on() {
        assert_eq!(
            terminal(Some("mac-mini/t1r".into())),
            Some("mac-mini/t1r".into())
        );
        assert_eq!(terminal(None), None);
        assert_eq!(terminal(Some(String::new())), None);
        assert_eq!(terminal(Some("t1\nfake".into())), None);
    }

    #[test]
    fn a_full_journal_moves_aside_before_the_next_line() {
        let dir = crate::test_support::TestDir::new();
        let journal = Journal::new(dir.join("activity"), "t7".into());
        let file = Path::new("/d/design.fig");
        std::fs::create_dir_all(dir.join("activity")).unwrap();
        let path = dir.join("activity").join(format!("{}.jsonl", key(file)));
        std::fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        with_tree(|tree| journal.record("inspect", file, tree, &ids(["0:4"])));
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
        assert!(
            dir.join("activity")
                .join(format!("{}.1.jsonl", key(file)))
                .exists()
        );
    }

    #[test]
    fn servers_writing_at_once_leave_whole_lines() {
        let scratch = crate::test_support::TestDir::new();
        let dir = scratch.join("activity");
        let file = Path::new("/d/design.fig");
        with_tree(|tree| {
            std::thread::scope(|s| {
                for t in 0..8 {
                    let dir = dir.clone();
                    s.spawn(move || {
                        let journal = Journal::new(dir, format!("t{t}"));
                        for _ in 0..50 {
                            journal.record("inspect", file, tree, &ids(["0:4"]));
                        }
                    });
                }
            });
        });
        let text = std::fs::read_to_string(dir.join(format!("{}.jsonl", key(file)))).unwrap();
        assert_eq!(text.lines().count(), 400);
        assert!(
            text.lines()
                .all(|l| serde_json::from_str::<serde_json::Value>(l).is_ok())
        );
    }

    #[test]
    fn a_new_journal_removes_ones_untouched_for_a_week() {
        let dir = crate::test_support::TestDir::new();
        let activity = dir.join("activity");
        std::fs::create_dir_all(&activity).unwrap();
        let (old, recent) = (
            activity.join("0000000000000000.jsonl"),
            activity.join("1111111111111111.jsonl"),
        );
        for path in [&old, &recent] {
            std::fs::write(path, "{}\n").unwrap();
        }
        let week_ago = SystemTime::now() - MAX_AGE - Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(week_ago)
            .unwrap();
        with_tree(|tree| {
            Journal::new(activity.clone(), "t7".into()).record(
                "inspect",
                Path::new("/d/new.fig"),
                tree,
                &ids(["0:4"]),
            )
        });
        assert_eq!((old.exists(), recent.exists()), (false, true));
    }

    #[test]
    fn a_design_key_hides_its_path() {
        let k = key(Path::new("/Users/someone/Private Project/app.fig"));
        assert_eq!(k.len(), 16);
        assert!(k.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
