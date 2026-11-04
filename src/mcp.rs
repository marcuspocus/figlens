//! Stdio request admission, lanes and borrowed local Tree reuse.
//!
//! A reader thread only frames and parses requests and answers protocol methods, so a jobs
//! status or cancel is read while a search or build runs. Tool calls go to bounded lanes:
//! control (jobs, index status), interactive (design tools and search, keeping the warm Tree),
//! index (workspace changes) and build (this server's index jobs, one at a time). One writer on
//! the calling thread applies the reply budget. Path authorization runs in the executing lane.

use std::error::Error;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use figlens::{Document, ErrorKind, Tree};
use serde_json::{Value, json};

use crate::{command, contract, workflow};

const MAX_REQUEST_BYTES: u64 = 1024 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

struct ResponseSize(usize);

impl std::io::Write for ResponseSize {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        if self.0 > MAX_RESPONSE_BYTES {
            return Err(std::io::Error::other("response budget exceeded"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn bounded_response(value: Value) -> Value {
    // Count the complete escaped wire message, including its newline, without another large buffer.
    if serde_json::to_writer(&mut ResponseSize(1), &value).is_ok() {
        return value;
    }
    let message = "response exceeds 8388608 bytes; use inspect, a scoped bundle, or CLI nodes instead of a larger MCP reply";
    if value["result"].get("isError").is_some() {
        let mut error = tool_reply(
            "",
            Err(contract::Failure::new(ErrorKind::ResponseBudget, message)),
        );
        // Only a written file's path; inspect's `path` is an ancestor list.
        if let Some(path) = value["result"]["structuredContent"]
            .get("path")
            .filter(|p| p.is_string())
        {
            error["_meta"]["figlens"]["outputPath"] = path.clone();
        }
        response(&value["id"], error)
    } else {
        protocol(value["id"].clone(), -32603, message)
    }
}

/// The writer's queue. Each reply is bounded before it waits there, so a stalled stdout holds
/// at most `WRITER_QUEUE` replies within the response budget, never complete oversized ones.
#[derive(Clone)]
struct Writer(mpsc::SyncSender<Value>);

impl Writer {
    fn send(&self, value: Value) {
        let _ = self.0.send(bounded_response(value));
    }
}

/// The next request line, parsed: no path, file or storage is touched here.
fn next_request(input: &mut impl BufRead, roots: &[PathBuf]) -> std::io::Result<Option<Admission>> {
    loop {
        let mut line = Vec::new();
        if input
            .by_ref()
            .take(MAX_REQUEST_BYTES + 1)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Ok(None);
        }
        if line.len() as u64 > MAX_REQUEST_BYTES {
            if line.last() != Some(&b'\n') {
                input.skip_until(b'\n')?;
            }
            return Ok(Some(Admission::Reply(protocol(
                Value::Null,
                -32600,
                "request line exceeds 1048576 bytes; send a smaller request",
            ))));
        }
        match std::str::from_utf8(&line) {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => return Ok(Some(frame(line, roots))),
            Err(_) => {
                return Ok(Some(Admission::Reply(protocol(
                    Value::Null,
                    -32700,
                    "parse error",
                ))));
            }
        }
    }
}

// Only admission can construct these values; execution consumes the checked paths.
struct AdmittedCall {
    id: Value,
    call: contract::PreparedCall,
    source: Option<Source>,
    output: Option<PathBuf>,
    /// An export_batch's checked `out` paths, in item order.
    outputs: Vec<PathBuf>,
    /// A bundle to verify: read, never loaded as a design.
    input: Option<PathBuf>,
    /// An export's folder of SVGs to compare against.
    match_dir: Option<PathBuf>,
    /// The project font folder verify looks in.
    fonts: Option<PathBuf>,
    /// The project directory of an index call.
    project: Option<PathBuf>,
    /// The earlier revision a diff reads beside `source`.
    old: Option<PathBuf>,
    /// A search's local query image: read for the call, never written.
    image: Option<PathBuf>,
    /// Its token in the server's pending calls, removed when the call is dropped; `None` for a
    /// call admitted outside a server.
    live: Option<Live>,
}

enum Source {
    Local(PathBuf),
    Remote(String),
}

enum Admission {
    Ignore,
    /// `notifications/cancelled` for this request id.
    Cancel(Value),
    Reply(Value),
    Call(Box<AdmittedCall>),
    /// A parsed tools/call, admitted later by the lane that runs it.
    Request(Value),
}

/// Parses one line: protocol methods are answered here, tool calls are left for their lane.
fn frame(line: &str, roots: &[PathBuf]) -> Admission {
    let req: Value = match serde_json::from_str(line) {
        Ok(req) => req,
        Err(_) => return Admission::Reply(protocol(Value::Null, -32700, "parse error")),
    };
    if envelope(&req) && req.get("id").is_some() && req["method"] == "tools/call" {
        return Admission::Request(req);
    }
    if envelope(&req) && req.get("id").is_none() && req["method"] == "notifications/cancelled" {
        return Admission::Cancel(req["params"]["requestId"].clone());
    }
    admit_value(req, roots)
}

fn envelope(req: &Value) -> bool {
    req.is_object()
        && req["method"].is_string()
        && req.get("jsonrpc").is_none_or(|v| v == "2.0")
        && req
            .get("id")
            .is_none_or(|v| v.is_null() || v.is_string() || v.is_number())
}

fn protocol(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "error":{"code":code,"message":message}})
}

fn response(id: &Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "result":result})
}

/// Preserve ordinary unrooted I/O failures instead of inventing new path restrictions.
/// Restricted new outputs require an existing canonical parent; dangling links are refused.
fn checked_path(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, contract::Failure> {
    #[cfg(test)]
    PATH_CHECKS.with(Counter::bump);
    let resolved = std::fs::canonicalize(path).or_else(|error| {
        if std::fs::symlink_metadata(path).is_ok() {
            return if roots.is_empty() {
                std::path::absolute(path)
            } else {
                Err(error)
            };
        }
        match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => {
                let parent = if parent.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    parent
                };
                std::fs::canonicalize(parent)
                    .map(|parent| parent.join(name))
                    .or_else(|e| {
                        if roots.is_empty() {
                            std::path::absolute(path)
                        } else {
                            Err(e)
                        }
                    })
            }
            _ if roots.is_empty() => std::path::absolute(path),
            _ => Err(error),
        }
    });
    if let Ok(path) = resolved
        && (roots.is_empty() || roots.iter().any(|root| path.starts_with(root)))
    {
        return Ok(path);
    }
    let folders: Vec<_> = roots.iter().map(|r| r.display().to_string()).collect();
    Err(contract::Failure::new(
        ErrorKind::PermissionDenied,
        format!(
            "source or output is outside the folders this server may use ({})",
            folders.join(", ")
        ),
    ))
}

/// An output path: as `checked_path`, but a missing folder inside a root is named as one to create.
fn checked_output(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, contract::Failure> {
    checked_path(path, roots).map_err(|denied| match missing_folder(path, roots) {
        Some(folder) => contract::Failure::new(
            ErrorKind::OutputWrite,
            format!(
                "folder {} does not exist; create it, then repeat the call",
                folder.display()
            ),
        ),
        None => denied,
    })
}

/// The missing folder `path` would go in, when its nearest existing ancestor is inside a root:
/// a folder to create, not a roots violation. `..` is refused, as it may lead out of the root.
fn missing_folder(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let folder = std::path::absolute(path).ok()?.parent()?.to_owned();
    if folder
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return None;
    }
    let existing = folder
        .ancestors()
        .find(|a| std::fs::symlink_metadata(a).is_ok())?;
    if existing == folder {
        return None;
    }
    let existing = std::fs::canonicalize(existing).ok()?;
    roots
        .iter()
        .any(|root| existing.starts_with(root))
        .then_some(folder)
}

#[cfg(test)]
fn admit(line: &str, roots: &[PathBuf]) -> Admission {
    match serde_json::from_str(line) {
        Ok(req) => admit_value(req, roots),
        Err(_) => Admission::Reply(protocol(Value::Null, -32700, "parse error")),
    }
}

fn admit_value(req: Value, roots: &[PathBuf]) -> Admission {
    if !envelope(&req) {
        return Admission::Reply(protocol(Value::Null, -32600, "invalid request"));
    }
    let Some(id) = req.get("id") else {
        return Admission::Ignore;
    };
    let result = match req["method"].as_str().unwrap() {
        "initialize" => {
            let mut instructions = workflow::INSTRUCTIONS.to_owned();
            if !roots.is_empty() {
                let folders: Vec<_> = roots.iter().map(|r| r.display().to_string()).collect();
                instructions +=
                    &format!(" Files and outputs must be under: {}.", folders.join(", "));
            }
            json!({
                "protocolVersion":req["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities":{"tools":{}},
                "serverInfo":{"name":"figlens","version":env!("CARGO_PKG_VERSION")},
                "instructions":instructions,
            })
        }
        "ping" => json!({}),
        "tools/list" => contract::tools(),
        "tools/call" => {
            let Some(name) = req["params"]["name"].as_str() else {
                return Admission::Reply(protocol(
                    id.clone(),
                    -32602,
                    "tools/call requires a tool name",
                ));
            };
            #[cfg(test)]
            PREPARATIONS.with(Counter::bump);
            let prepared = contract::prepare(name, &req["params"]["arguments"]);
            let admitted = prepared.and_then(|call| {
                let source = match call.string("file") {
                    Some(file) if file.starts_with("gs://") && call.name() == "diff" => {
                        return Err(contract::Failure::argument(
                            "diff reads two local .fig files; download the gs:// object first",
                        ));
                    }
                    Some(file) if file.starts_with("gs://") && call.name() == "index"
                        && call.string("action") != Some("add") => {
                        return Err(contract::Failure::argument(
                            "only index add takes a gs:// object; other index actions read registered sources or local .fig files",
                        ));
                    }
                    Some(file) if file.starts_with("gs://") => {
                        if !roots.is_empty() {
                            return Err(contract::Failure::new(ErrorKind::PermissionDenied,
                                "GCS sources are not supported with --root; use an authorized local copy inside the configured roots"));
                        }
                        Some(Source::Remote(file.to_owned()))
                    }
                    Some(file) => Some(Source::Local(checked_path(Path::new(file), roots)?)),
                    None => None,
                };
                let output = call.string("out").map(|out| checked_output(Path::new(out), roots)).transpose()?;
                let outputs = match call.name() {
                    "export_batch" => call.argument("items").as_array().unwrap().iter()
                        .map(|item| checked_output(Path::new(item["out"].as_str().unwrap()), roots))
                        .collect::<Result<_, _>>()?,
                    _ => Vec::new(),
                };
                let input = call.string("bundle").map(|path| checked_path(Path::new(path), roots)).transpose()?;
                let match_dir = call.string("matchDir").map(|path| checked_path(Path::new(path), roots)).transpose()?;
                let fonts = call.string("fonts").map(|path| checked_path(Path::new(path), roots)).transpose()?;
                let project = call.string("workspace").map(|path| checked_path(Path::new(path), roots)).transpose()?;
                let old = match call.string("old") {
                    Some(file) if file.starts_with("gs://") => {
                        return Err(contract::Failure::argument(
                            "diff reads two local .fig files; download the gs:// object first",
                        ));
                    }
                    file => file.map(|path| checked_path(Path::new(path), roots)).transpose()?,
                };
                let image = match call.string("image") {
                    Some(image) if image.contains("://") => {
                        return Err(contract::Failure::argument("image takes a local PNG path; no URL is read"));
                    }
                    image => image.map(|path| checked_path(Path::new(path), roots)).transpose()?,
                };
                Ok(AdmittedCall { id:id.clone(), call, source, output, outputs, input, match_dir, fonts, project, old, image, live: None })
            });
            return match admitted {
                Ok(call) => Admission::Call(Box::new(call)),
                Err(error) => Admission::Reply(response(id, tool_reply(name, Err(error)))),
            };
        }
        method => {
            return Admission::Reply(protocol(
                id.clone(),
                -32601,
                &format!("unknown method {method}"),
            ));
        }
    };
    Admission::Reply(response(id, result))
}

impl AdmittedCall {
    /// Set by `notifications/cancelled` for this call, or when the server stops while it runs or
    /// before it started; the calls that run owned model work poll it.
    fn cancel(&self) -> &AtomicBool {
        static NEVER: AtomicBool = AtomicBool::new(false);
        self.live.as_ref().map_or(&NEVER, |l| &l.token.cancel)
    }

    fn local(&self) -> Option<&Path> {
        match &self.source {
            Some(Source::Local(path)) => Some(path),
            _ => None,
        }
    }

    /// The local source a warm tree serves; search reads and hashes its own snapshot instead.
    fn tree_source(&self) -> Option<&Path> {
        self.local().filter(|_| {
            !matches!(
                self.call.name(),
                "search" | "usages" | "diff" | "index" | "jobs" | "models" | "changes" | "watch"
            )
        })
    }

    /// Whether this call reads or changes a workspace rather than a design file.
    fn workspace(&self) -> bool {
        matches!(
            self.call.name(),
            "index" | "jobs" | "models" | "changes" | "watch"
        ) || (matches!(self.call.name(), "search" | "usages")
            && self.call.string("workspaceId").is_some())
    }

    /// Runs a workspace call; the jobs an index call submitted for this process come back
    /// for the build lane.
    fn run_workspace(
        &self,
        roots: &[PathBuf],
        storage_roots: &[PathBuf],
    ) -> (Value, Option<command::Runs>) {
        let mut runs = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.recheck(roots)?;
            match self.call.name() {
                "jobs" => command::jobs_tool(&self.call, roots, storage_roots),
                "models" => command::models_tool(&self.call, roots, self.cancel()),
                "changes" | "watch" => command::changes_tool(&self.call, roots, storage_roots),
                "search" => command::workspace_search_tool(
                    &self.call,
                    roots,
                    storage_roots,
                    self.image.as_deref(),
                    self.cancel(),
                ),
                "usages" => command::workspace_usages_tool(&self.call, roots, storage_roots),
                _ => command::index_tool(
                    &self.call,
                    // An admitted gs:// object, only ever an add's, reaches index_add as its URI.
                    match &self.source {
                        Some(Source::Remote(uri)) => Some(Path::new(uri)),
                        _ => self.local(),
                    },
                    self.project.as_deref(),
                    roots,
                    storage_roots,
                )
                .map(|(output, submitted)| {
                    runs = submitted;
                    output
                }),
            }
            .map_err(contract::Failure::from_error)
        }))
        .unwrap_or_else(|_| {
            Err(contract::Failure::new(
                ErrorKind::OperationFailed,
                "figlens crashed on this workspace call",
            ))
        });
        (
            response(&self.id, tool_reply(self.call.name(), result)),
            runs,
        )
    }

    /// The node arguments this call names, as given: `node`, and each `export_batch` item's.
    fn nodes(&self) -> Vec<&str> {
        let items = self.call.argument("items").as_array().into_iter().flatten();
        self.call
            .string("node")
            .into_iter()
            .chain(items.filter_map(|item| item["node"].as_str()))
            .collect()
    }

    fn recheck(&self, roots: &[PathBuf]) -> Result<(), contract::Failure> {
        for path in self
            .local()
            .into_iter()
            .chain(self.output.as_deref())
            .chain(self.outputs.iter().map(PathBuf::as_path))
            .chain(self.input.as_deref())
            .chain(self.match_dir.as_deref())
            .chain(self.fonts.as_deref())
            .chain(self.project.as_deref())
            .chain(self.old.as_deref())
            .chain(self.image.as_deref())
        {
            // Recheck current filesystem state, not raw arguments or a permanent authorization lease.
            checked_path(path, roots)?;
        }
        Ok(())
    }

    fn execute(
        &self,
        loaded: Option<(&Path, &Tree, Option<figlens::search::Stamp>)>,
        roots: &[PathBuf],
        storage_roots: &[PathBuf],
    ) -> Value {
        if self.workspace() {
            return self.run_workspace(roots, storage_roots).0;
        }
        let name = self.call.name();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.recheck(roots)?;
            if name == "workflow" {
                return workflow::read(self.call.string("topic").unwrap())
                    .map(command::Output::Text)
                    .map_err(contract::Failure::argument);
            }
            if let Some(bundle) = &self.input {
                let mut value = figlens::verify::verify_with_fonts(bundle, self.fonts.as_deref())
                    .map_err(contract::Failure::from_error)?;
                // Reply with the caller's label, like output paths.
                value["path"] = self.call.string("bundle").unwrap().into();
                return Ok(command::Output::Json {
                    value,
                    pretty: false,
                });
            }
            if name == "diff" {
                let (old, new) = (self.old.as_deref().unwrap(), self.local().unwrap());
                return command::diff_tool(&self.call)
                    .and_then(|request| figlens::search::diff::diff(old, new, &request))
                    .map(|value| command::Output::Json {
                        value,
                        pretty: false,
                    })
                    .map_err(contract::Failure::from_error);
            }
            if matches!(name, "search" | "usages") {
                let path = match self.source.as_ref().unwrap() {
                    Source::Local(path) => path.clone(),
                    Source::Remote(file) => figlens::remote::local(file)
                        .map_err(|e| contract::Failure::from_error(contract::source(file, e)))?,
                };
                if name == "usages" {
                    return command::usages_tool(&self.call)
                        .and_then(|request| command::usages(&path, &request, ""))
                        .map_err(contract::Failure::from_error);
                }
                return command::search_tool(&self.call, self.image.as_deref())
                    .and_then(|(request, query)| match query {
                        command::Query::Text(mode) => {
                            command::search(&path, &request, mode, "", self.cancel())
                        }
                        command::Query::Image(_) => Err(command::image_needs_workspace(
                            self.call.string("file").unwrap_or_default(),
                            &request.scope,
                        )),
                    })
                    .map_err(contract::Failure::from_error);
            }
            let run_tool = |tree: &Tree, path: &Path, decoded| {
                if name == "compare_image" {
                    let opts = figlens::compare_image::Options {
                        scale: self.call.argument("scale").as_f64(),
                        limit: self.call.argument("limit").as_u64().unwrap() as usize,
                        full_page: self.call.argument("fullPage") == true,
                        ignore: self
                            .call
                            .string("ignore")
                            .map(|ids| command::id_list("ignore", ids))
                            .transpose()?
                            .unwrap_or_default(),
                        threshold: self.call.argument("threshold").as_u64().unwrap() as u8,
                        diff: false,
                    };
                    let value = command::compare_image(
                        tree,
                        path,
                        self.call.string("node").unwrap(),
                        self.image.as_deref().unwrap(),
                        self.output.as_deref(),
                        opts,
                    )?;
                    return Ok(command::Output::Json {
                        value,
                        pretty: false,
                    });
                }
                if name == "export_batch" {
                    let items = command::export_items(self.call.argument("items"), 1600)?;
                    let value = command::export_batch(tree, path, &items, &self.outputs)?;
                    return Ok(command::Output::Json {
                        value,
                        pretty: false,
                    });
                }
                command::Command::from_tool(
                    &self.call,
                    tree,
                    self.output.as_deref(),
                    self.match_dir.as_deref(),
                )?
                .decoded(decoded)
                .execute(tree, path)
            };
            // Inside a Herdr terminal, a call that read the design's nodes leaves one journal line.
            let run = |tree: &Tree, path: &Path, decoded| {
                run_tool(tree, path, decoded).inspect(|_| {
                    // find is a search: like search, usages and diff, it leaves no line.
                    if name != "find"
                        && let Some(journal) = crate::activity::journal()
                    {
                        journal.record(name, path, tree, &crate::activity::ids(self.nodes()));
                    }
                })
            };
            let result: Result<command::Output, Box<dyn Error>> = (|| {
                if let Some((path, tree, decoded)) = loaded {
                    // A metadata match alone does not prove that a warm source remains readable.
                    std::fs::File::open(path).map_err(|e| contract::source("", e.into()))?;
                    return run(tree, path, decoded);
                }
                let path = match self.source.as_ref().unwrap() {
                    Source::Local(path) => path.clone(),
                    Source::Remote(file) => {
                        figlens::remote::local(file).map_err(|e| contract::source(file, e))?
                    }
                };
                #[cfg(test)]
                LOADS.with(Counter::bump);
                let decoded = stamp(&path);
                let mut doc = Document::open(&path).map_err(|e| contract::source("", e))?;
                let message = doc.take_message()?;
                let tree = Tree::new(&message);
                run(&tree, &path, decoded)
            })();
            result.map_err(contract::Failure::from_error)
        }))
        .unwrap_or_else(|_| {
            Err(contract::Failure::new(
                ErrorKind::OperationFailed,
                "figlens crashed on this file",
            ))
        });
        // Keep the caller's output label in replies; it is never used for filesystem work.
        let result = result.map(|mut output| {
            if let Some(label) = self.call.string("out") {
                match &mut output {
                    command::Output::Json { value, .. } | command::Output::Image { value, .. } => {
                        value["path"] = label.into()
                    }
                    _ => {}
                }
            }
            output
        });
        response(&self.id, tool_reply(name, result))
    }
}

fn stamp(path: &Path) -> Option<(std::time::SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Requests a lane may queue before the reader answers busy instead.
const CONTROL_QUEUE: usize = 64;
const INTERACTIVE_QUEUE: usize = 64;
const INDEX_QUEUE: usize = 8;
/// Replies waiting for stdout before every sender, the reader included, waits.
const WRITER_QUEUE: usize = 64;
/// Build submissions this server may hold before a build-submitting call is answered busy.
const BUILD_QUEUE: usize = 8;

/// One queued or running tool call's cancel state, made when the reader queues the call and
/// carried with it, so a call is found by identity, never by its (reusable) id.
#[derive(Default)]
struct Token {
    cancel: AtomicBool,
    started: AtomicBool,
}

/// Tool calls queued in a lane or running, by request id; a reused id keeps one token per call
/// and a cancel names every call of that id. Bounded by the lane queues plus one running call
/// per lane.
type Pending = std::sync::Mutex<std::collections::HashMap<String, Vec<Arc<Token>>>>;

/// Removes exactly `token` of `id`.
fn forget(pending: &Pending, id: &str, token: &Arc<Token>) {
    let mut pending = pending.lock().unwrap();
    if let Some(calls) = pending.get_mut(id) {
        calls.retain(|t| !Arc::ptr_eq(t, token));
        if calls.is_empty() {
            pending.remove(id);
        }
    }
}

/// An admitted call's place in `Pending`, removed when the call is dropped after its reply.
struct Live {
    pending: Arc<Pending>,
    id: String,
    token: Arc<Token>,
}

impl Drop for Live {
    fn drop(&mut self) {
        forget(&self.pending, &self.id, &self.token);
    }
}

/// Admits a call as its lane starts it. A call cancelled while it queued is dropped unanswered,
/// as before; one that starts after the server stopped starts cancelled, so it does not begin
/// owned model work, while ordinary calls still run and answer.
fn admit_live(
    pending: &Arc<Pending>,
    (req, token): (Value, Arc<Token>),
    roots: &[PathBuf],
    stop: &AtomicBool,
) -> Admission {
    let id = req["id"].to_string();
    token.started.store(true, Ordering::SeqCst);
    if token.cancel.load(Ordering::SeqCst) {
        forget(pending, &id, &token);
        return Admission::Ignore;
    }
    if stop.load(Ordering::SeqCst) {
        token.cancel.store(true, Ordering::SeqCst);
    }
    match admit_value(req, roots) {
        Admission::Call(mut call) => {
            call.live = Some(Live {
                pending: pending.clone(),
                id,
                token,
            });
            Admission::Call(call)
        }
        other => {
            forget(pending, &id, &token);
            other
        }
    }
}

/// Cancels every running call when input ends or replies cannot be written; a queued call
/// starts cancelled instead (see `admit_live`).
fn cancel_running(pending: &Pending) {
    for token in pending.lock().unwrap().values().flatten() {
        if token.started.load(Ordering::SeqCst) {
            token.cancel.store(true, Ordering::SeqCst);
        }
    }
}

/// Whether an index call submits jobs to the build lane.
fn submits(req: &Value) -> bool {
    matches!(
        req["params"]["arguments"]["action"].as_str(),
        Some("prepare" | "refresh" | "rebuild" | "track" | "embed" | "visual")
    )
}

enum Lane {
    Control,
    Interactive,
    Index,
}

/// The lane that runs a tool call, chosen from its name and action alone.
fn lane(req: &Value) -> Lane {
    match (
        req["params"]["name"].as_str(),
        req["params"]["arguments"]["action"].as_str(),
    ) {
        (Some("jobs"), _)
        | (Some("index"), Some("status"))
        | (Some("models"), Some("list" | "status" | "remove")) => Lane::Control,
        // Configure hashes gigabytes and loads the model once: never ahead of interactive calls.
        (Some("index"), _) | (Some("models"), _) => Lane::Index,
        _ => Lane::Interactive,
    }
}

fn busy(id: &Value) -> Value {
    response(
        id,
        tool_reply(
            "",
            Err(contract::Failure::new(
                ErrorKind::WorkspaceBusy,
                "figlens is busy with earlier calls of this kind; retry after a reply arrives",
            )),
        ),
    )
}

/// Replies listed in the order of `ids`: protocol replies answer at once, so they can overtake
/// earlier tool calls; JSON-RPC pairs them by id.
#[cfg(test)]
pub(crate) fn in_request_order(replies: &[Value], ids: &[Value]) -> Vec<Value> {
    ids.iter()
        .map(|id| replies.iter().find(|r| &r["id"] == id).cloned().unwrap())
        .collect()
}

/// Serves MCP on `input`/`output` with design roots `roots` and no custom storage roots.
#[cfg(test)]
pub fn serve(
    input: impl BufRead + Send,
    output: impl std::io::Write,
    roots: &[PathBuf],
) -> Result<(), Box<dyn Error>> {
    serve_with(input, output, roots, &[])
}

/// Serves MCP until `input` ends. `storage_roots` allow custom workspace storage besides the
/// default roots. At the end of input, this server's running build is cancelled; when a reply
/// cannot be written, its builds are cancelled and the process exits 1.
pub fn serve_with(
    mut input: impl BufRead + Send,
    mut output: impl std::io::Write,
    roots: &[PathBuf],
    storage_roots: &[PathBuf],
) -> Result<(), Box<dyn Error>> {
    // Bounded: when stdout is slow, replies wait here and then the reader stops reading input,
    // so memory stays at this many capped replies.
    let (sender, replies) = mpsc::sync_channel::<Value>(WRITER_QUEUE);
    let to_writer = Writer(sender);
    let stop = AtomicBool::new(false);
    // Set when a reply cannot be written; no lane admits a build after that.
    let dead = AtomicBool::new(false);
    let pending = Arc::new(Pending::default());
    let builds = std::sync::atomic::AtomicUsize::new(0);
    #[cfg(test)]
    let counters = Counter::share();
    std::thread::scope(|scope| -> Result<(), Box<dyn Error>> {
        let (control, control_rx) = mpsc::sync_channel::<(Value, Arc<Token>)>(CONTROL_QUEUE);
        let (interactive, interactive_rx) =
            mpsc::sync_channel::<(Value, Arc<Token>)>(INTERACTIVE_QUEUE);
        let (index, index_rx) = mpsc::sync_channel::<(Value, Arc<Token>)>(INDEX_QUEUE);
        let (to_build, build_rx) = mpsc::channel::<command::Runs>();
        let (to_model, model_rx) = mpsc::channel::<command::Runs>();
        let (stop, dead, pending, builds) = (&stop, &dead, &pending, &builds);
        // Control and index lanes: each call admitted and run in arrival order.
        for (rx, build) in [(control_rx, None), (index_rx, Some(to_build))] {
            let out = to_writer.clone();
            #[cfg(test)]
            let counters = counters.clone();
            scope.spawn(move || {
                #[cfg(test)]
                Counter::adopt(counters);
                for queued in rx {
                    let submitting = submits(&queued.0);
                    let req = match admit_live(pending, queued, roots, stop) {
                        Admission::Ignore => continue,
                        admitted => admitted,
                    };
                    // Counted before the call creates its job, so a writer that fails meanwhile
                    // waits for it instead of exiting with the job unowned.
                    // With `dead` checked after the count (both SeqCst), either the writer sees
                    // this build or this lane sees the writer died.
                    let counted =
                        build.is_some() && submitting && matches!(req, Admission::Call(_));
                    if counted {
                        let full = builds.fetch_add(1, Ordering::SeqCst) >= BUILD_QUEUE;
                        if full || dead.load(Ordering::SeqCst) {
                            builds.fetch_sub(1, Ordering::SeqCst);
                            if let Admission::Call(call) = &req {
                                out.send(busy(&call.id));
                            }
                            continue;
                        }
                    }
                    let mut queued = false;
                    match req {
                        Admission::Call(call) => {
                            let (reply, runs) = call.run_workspace(roots, storage_roots);
                            if let (Some(runs), Some(build)) = (runs, &build) {
                                if !counted {
                                    builds.fetch_add(1, Ordering::SeqCst);
                                }
                                queued = build.send(runs).is_ok();
                            }
                            out.send(reply);
                        }
                        Admission::Reply(reply) => {
                            out.send(reply);
                        }
                        _ => {}
                    }
                    if counted && !queued {
                        builds.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            });
        }
        // Watch leases this server holds: parked until a start, then renewed every `RENEW`
        // until input ends.
        scope.spawn(move || figlens::workspace::watch::renew_until(stop));
        // Build lane: this server's jobs, one at a time; they stop when input ends or a reply
        // cannot be written. A text build that publishes queues its model layer's embed, and the
        // visual job of a scope that had a visual layer, on the model lane, so no text build
        // waits behind the model; each send counts as a build.
        scope.spawn(move || {
            for runs in build_rx {
                if runs.model {
                    // Already counted: the model lane ends it.
                    if to_model.send(runs).is_err() {
                        builds.fetch_sub(1, Ordering::SeqCst);
                    }
                    continue;
                }
                let mut model_jobs = Vec::new();
                for job in &runs.jobs {
                    let report = figlens::workspace::jobs::run(
                        &runs.storage,
                        &runs.workspace_id,
                        job,
                        stop,
                        &mut |_| {},
                    );
                    if let Some(r) = report.ok().filter(|_| !stop.load(Ordering::SeqCst)) {
                        use figlens::workspace::jobs::{embed_after, visual_after};
                        model_jobs.extend(embed_after(&runs.storage, &r));
                        model_jobs.extend(visual_after(&runs.storage, &r));
                    }
                }
                if !model_jobs.is_empty() {
                    builds.fetch_add(1, Ordering::SeqCst);
                    let model = command::Runs {
                        storage: runs.storage.clone(),
                        workspace_id: runs.workspace_id.clone(),
                        jobs: model_jobs,
                        model: true,
                    };
                    if to_model.send(model).is_err() {
                        builds.fetch_sub(1, Ordering::SeqCst);
                    }
                }
                builds.fetch_sub(1, Ordering::SeqCst);
            }
        });
        // Model lane: this server's embed and visual jobs, one at a time, beside its text builds.
        scope.spawn(move || {
            for runs in model_rx {
                for job in &runs.jobs {
                    let _ = figlens::workspace::jobs::run(
                        &runs.storage,
                        &runs.workspace_id,
                        job,
                        stop,
                        &mut |_| {},
                    );
                }
                builds.fetch_sub(1, Ordering::SeqCst);
            }
        });
        // Interactive lane: the warm Tree stays on this thread.
        let out = to_writer.clone();
        #[cfg(test)]
        let lane_counters = counters.clone();
        scope.spawn(move || {
            #[cfg(test)]
            Counter::adopt(lane_counters);
            let next = || {
                interactive_rx
                    .recv()
                    .ok()
                    .map(|queued| admit_live(pending, queued, roots, stop))
            };
            interactive_loop(
                next,
                |value| {
                    out.send(value);
                },
                roots,
                storage_roots,
            );
        });
        // Reader: framing and protocol only.
        let reader = to_writer.clone();
        #[cfg(test)]
        let reader_counters = counters.clone();
        scope.spawn(move || {
            #[cfg(test)]
            Counter::adopt(reader_counters);
            let lanes = [control, index];
            loop {
                let request = match next_request(&mut input, roots) {
                    Ok(Some(request)) => request,
                    Ok(None) => break,
                    Err(e) => {
                        reader.send(protocol(Value::Null, -32700, &format!("input failed: {e}")));
                        break;
                    }
                };
                match request {
                    // Protocol replies never wait behind a lane, so ping stays responsive.
                    Admission::Reply(reply) => {
                        reader.send(reply);
                    }
                    Admission::Cancel(id) => {
                        for token in pending
                            .lock()
                            .unwrap()
                            .get(&id.to_string())
                            .into_iter()
                            .flatten()
                        {
                            token.cancel.store(true, Ordering::SeqCst);
                        }
                    }
                    Admission::Request(req) => {
                        let id = req["id"].clone();
                        let token = Arc::<Token>::default();
                        pending
                            .lock()
                            .unwrap()
                            .entry(id.to_string())
                            .or_default()
                            .push(token.clone());
                        let queued = (req, token.clone());
                        let sent = match lane(&queued.0) {
                            Lane::Interactive => interactive.try_send(queued),
                            Lane::Control => lanes[0].try_send(queued),
                            Lane::Index => lanes[1].try_send(queued),
                        };
                        if sent.is_err_and(|e| matches!(e, mpsc::TrySendError::Full(_))) {
                            forget(pending, &id.to_string(), &token);
                            reader.send(busy(&id));
                        }
                    }
                    _ => {}
                }
            }
            stop.store(true, Ordering::SeqCst);
            cancel_running(pending);
            figlens::workspace::watch::wake();
        });
        drop(to_writer);
        // Writer: every reply, already bounded, in completion order.
        for value in replies.iter() {
            if let Err(e) = writeln!(output, "{value}").and_then(|_| output.flush()) {
                // The client stopped reading, and the reader may wait on an open stdin forever:
                // stop this server's builds, wait until they cancelled their workers, then end.
                dead.store(true, Ordering::SeqCst);
                stop.store(true, Ordering::Relaxed);
                cancel_running(pending);
                // Replies still sent meanwhile are dropped, so no lane waits on this queue.
                let tick = std::time::Duration::from_millis(20);
                while builds.load(Ordering::SeqCst) > 0 {
                    if let Err(mpsc::RecvTimeoutError::Disconnected) = replies.recv_timeout(tick) {
                        std::thread::sleep(tick);
                    }
                }
                figlens::workspace::watch::release();
                // Not eprintln!: a closed stderr would panic and leave the reader's join hanging.
                use std::io::Write as _;
                let _ = writeln!(
                    std::io::stderr(),
                    "figlens mcp: cannot write a reply ({e}); stopped this server's builds"
                );
                std::process::exit(1);
            }
        }
        Ok(())
    })?;
    // Input ended: this server's leases end now, not when they would expire.
    figlens::workspace::watch::release();
    Ok(())
}

/// Keep Document/message/Tree lexical ownership; carry admitted calls, never raw lines.
fn interactive_loop(
    mut next: impl FnMut() -> Option<Admission>,
    mut reply: impl FnMut(Value),
    roots: &[PathBuf],
    storage_roots: &[PathBuf],
) {
    let mut carried = None;
    loop {
        let call = match if let Some(call) = carried.take() {
            Some(Admission::Call(call))
        } else {
            next()
        } {
            None => return,
            Some(Admission::Call(call)) => call,
            Some(Admission::Reply(value)) => {
                reply(value);
                continue;
            }
            Some(_) => continue,
        };
        let Some(path) = call.tree_source() else {
            reply(call.execute(None, roots, storage_roots));
            continue;
        };
        let Some(stamp_at_open) = stamp(path) else {
            reply(call.execute(None, roots, storage_roots));
            continue;
        };
        if let Err(error) = call.recheck(roots) {
            reply(response(&call.id, tool_reply(call.call.name(), Err(error))));
            continue;
        }
        #[cfg(test)]
        LOADS.with(Counter::bump);
        let Ok(Ok(mut doc)) = std::panic::catch_unwind(|| Document::open(path)) else {
            reply(call.execute(None, roots, storage_roots));
            continue;
        };
        let document = &mut doc;
        let Ok(Ok(message)) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let document = document;
            document.take_message()
        })) else {
            reply(call.execute(None, roots, storage_roots));
            continue;
        };
        let Ok(tree) = std::panic::catch_unwind(|| Tree::new(&message)) else {
            reply(call.execute(None, roots, storage_roots));
            continue;
        };
        reply(call.execute(
            Some((path, &tree, Some(stamp_at_open))),
            roots,
            storage_roots,
        ));
        loop {
            match next() {
                None => return,
                Some(Admission::Reply(value)) => reply(value),
                Some(Admission::Call(next_call)) => match next_call.tree_source() {
                    None => reply(next_call.execute(None, roots, storage_roots)),
                    Some(next_path) if next_path == path && stamp(path) == Some(stamp_at_open) => {
                        reply(next_call.execute(
                            Some((path, &tree, Some(stamp_at_open))),
                            roots,
                            storage_roots,
                        ))
                    }
                    Some(_) => {
                        carried = Some(next_call);
                        break;
                    }
                },
                Some(_) => {}
            }
        }
    }
}

fn tool_reply(name: &str, result: Result<command::Output, contract::Failure>) -> serde_json::Value {
    let mut structured = None;
    let mut image = None;
    let result = result.and_then(|output| {
        let text = output
            .text()
            .map_err(|e| contract::Failure::from_error(Box::new(e)))?;
        if let command::Output::Image { bytes, mime, .. } = &output {
            image = Some(serde_json::json!({
                "type": "image", "data": figlens::render::base64(bytes), "mimeType": mime,
            }));
        }
        if contract::output_schema(name).is_some() {
            structured = output.json().cloned();
        }
        Ok(text)
    });
    let (text, diagnostic) = match result {
        Ok(text) => (text, None),
        Err(error) => (error.text.clone(), Some(error.metadata())),
    };
    let is_error = diagnostic.is_some();
    let mut reply = serde_json::json!({
        "content": [{ "type": "text", "text": text }], "isError": is_error
    });
    if let Some(diagnostic) = diagnostic {
        reply["_meta"] = serde_json::json!({"figlens": diagnostic});
    }
    if let Some(value) = structured {
        reply["structuredContent"] = value;
    }
    if let Some(image) = image {
        reply["content"].as_array_mut().unwrap().push(image);
    }
    reply
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod admission_tests;

/// A test counter shared by a serve call's lanes and the thread that started it.
#[cfg(test)]
#[derive(Default)]
struct Counter(std::cell::RefCell<std::sync::Arc<std::sync::atomic::AtomicUsize>>);

#[cfg(test)]
type Shared = [std::sync::Arc<std::sync::atomic::AtomicUsize>; 3];

#[cfg(test)]
impl Counter {
    fn get(&self) -> usize {
        self.0.borrow().load(Ordering::SeqCst)
    }
    fn set(&self, n: usize) {
        self.0.borrow().store(n, Ordering::SeqCst)
    }
    fn bump(&self) {
        self.0.borrow().fetch_add(1, Ordering::SeqCst);
    }
    fn share() -> Shared {
        [&PREPARATIONS, &PATH_CHECKS, &LOADS].map(|c| c.with(|c| c.0.borrow().clone()))
    }
    fn adopt(shared: Shared) {
        for (c, s) in [&PREPARATIONS, &PATH_CHECKS, &LOADS]
            .into_iter()
            .zip(shared)
        {
            c.with(|c| *c.0.borrow_mut() = s);
        }
    }
}

#[cfg(test)]
thread_local! {
    static PREPARATIONS: Counter = Counter::default();
    static PATH_CHECKS: Counter = Counter::default();
    static LOADS: Counter = Counter::default();
}

#[cfg(test)]
pub fn mcp_reply(line: &str) -> Option<Value> {
    match admit(line, &[]) {
        Admission::Reply(reply) => Some(reply),
        Admission::Call(call) => Some(call.execute(None, &[], &[])),
        _ => None,
    }
}

#[cfg(test)]
pub fn call_tool(params: &Value, loaded: Option<(&Path, &Tree)>) -> Value {
    let request = json!({"id":1,"method":"tools/call","params":params}).to_string();
    match admit(&request, &[]) {
        Admission::Reply(reply) => reply["result"].clone(),
        Admission::Call(call) => {
            call.execute(loaded.map(|(path, tree)| (path, tree, None)), &[], &[])["result"].clone()
        }
        _ => unreachable!(),
    }
}
