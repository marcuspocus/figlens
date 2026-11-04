//! The live viewer's optional semantic search (#94): read-only readiness, the explicit Enable
//! index action and the jobs it owns. Text search never comes here, so opening a design and
//! searching its text create no workspace and start no model.
use super::*;
use crate::workspace::{self, PrepareOptions, Select, jobs};
use std::sync::atomic::AtomicBool;

/// The one index build this viewer owns at a time: the scope the user enabled for a revision.
#[derive(Default)]
pub(super) struct Indexing(pub(super) Mutex<Option<Owned>>);

pub(super) struct Owned {
    scope: String,
    visual: bool,
    cancel: Arc<AtomicBool>,
    /// The latest job report, or the error that ended the run.
    state: Arc<Mutex<Value>>,
    thread: std::thread::JoinHandle<()>,
}

impl Indexing {
    /// Whether the build this viewer owns is still running: it keeps an idle viewer alive.
    pub(super) fn building(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|o| o.as_ref().is_some_and(|o| !o.thread.is_finished()))
    }
}

impl Drop for Indexing {
    /// However the viewer ends, the build it owns ends with it.
    fn drop(&mut self) {
        self.stop();
    }
}

impl Indexing {
    /// Cancels the owned run and waits until its jobs, and the workers they own, have ended.
    pub(super) fn stop(&self) {
        if let Some(owned) = self.0.lock().ok().and_then(|mut o| o.take()) {
            owned.cancel.store(true, Ordering::SeqCst);
            let _ = owned.thread.join();
        }
    }
}

/// Runs owned jobs and awaits joined ones without taking their ownership.
fn phase(
    storage: &workspace::Storage,
    ws: &str,
    outcome: workspace::Outcome,
    cancel: &AtomicBool,
    state: &Mutex<Value>,
) -> Result<bool> {
    let note = |report: &Value| {
        if let Ok(mut s) = state.lock() {
            *s = report.clone();
        }
    };
    note(&outcome.report);
    for id in &outcome.run {
        let report = jobs::run(storage, ws, id, cancel, &mut |r| note(r))?;
        note(&report);
        if report["state"] != "succeeded" {
            return Ok(false);
        }
    }
    let joined = outcome
        .report
        .get("job")
        .into_iter()
        .chain(outcome.report["jobs"].as_array().into_iter().flatten());
    for job in joined {
        let Some(id) = job["jobId"].as_str() else {
            continue;
        };
        if outcome.run.iter().any(|owned| owned == id) {
            continue;
        }
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Ok(false);
            }
            let report = jobs::status(storage, id)?;
            note(&report);
            match report["state"].as_str() {
                Some("queued" | "running") => std::thread::sleep(Duration::from_millis(100)),
                Some("succeeded") => break,
                _ => return Ok(false),
            }
        }
    }
    Ok(outcome.report["admission"] != "partial")
}

/// Registered identity of this file; a project binding must not widen a viewer query/build.
fn source(storage: &workspace::Storage, select: &Select, path: &Value) -> Result<String> {
    let status = workspace::status(storage, select)?;
    status["sources"]
        .as_array()
        .and_then(|sources| sources.iter().find(|s| s["path"] == *path))
        .and_then(|s| s["sourceId"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| "enable an index of this file first".into())
}

/// Explicitly prepare this file/scope, then its visual layer or optional text model layer.
fn build(
    file: &Path,
    scope: &str,
    visual: bool,
    cancel: &AtomicBool,
    state: &Mutex<Value>,
) -> Result<()> {
    let storage = workspace::storage(None)?;
    let prepared = workspace::prepare(
        &storage,
        file,
        None,
        PrepareOptions {
            scope: Some(scope),
            ..Default::default()
        },
    )?;
    let ws = prepared.report["workspaceId"]
        .as_str()
        .ok_or("prepare named no workspace")?
        .to_owned();
    if !phase(&storage, &ws, prepared, cancel, state)? || cancel.load(Ordering::SeqCst) {
        return Ok(());
    }
    if !visual && crate::semantic::profile::active(&storage)?.is_none() {
        return Ok(());
    }
    let select = Select::Id(ws.clone());
    let path = json!(fs::canonicalize(file)?.display().to_string());
    let source = source(&storage, &select, &path)?;
    let layer = if visual {
        jobs::visual(&storage, &select, Some(&source), Some(scope), None)?
    } else {
        jobs::embed(&storage, &select, Some(&source), Some(scope), None)?
    };
    phase(&storage, &ws, layer, cancel, state)?;
    Ok(())
}

impl Revision<'_> {
    /// The scope a semantic route names for `revision`: a stored layer of this revision, never
    /// widened.
    fn indexed_scope(&self, revision: &str, scope: Option<&str>) -> Result<String> {
        if revision.parse::<u64>().map_err(|_| "unknown route")? != self.generation {
            return Err(STALE.into());
        }
        let scope = scope
            .filter(|s| valid_id(s))
            .ok_or("takes exactly one scope")?;
        if !self.tree.nodes.contains_key(scope) {
            return Err("scope takes a stored layer of this file".into());
        }
        Ok(scope.to_owned())
    }

    pub(super) fn select(&self) -> Result<Select> {
        Ok(
            Select::from_choice(None, Some(self.file), None)?
                .ok_or("no workspace for this file")?,
        )
    }

    pub(super) fn indexed_source(&self, storage: &workspace::Storage) -> Result<String> {
        source(storage, &self.select()?, &self.source["path"])
    }

    /// A semantic or hybrid search of this file's workspace index, served only while that index
    /// holds exactly this revision's bytes; the reply then names them as `source`, as text's does.
    pub(super) fn indexed_search(
        &self,
        req: &search::Request,
        mode: crate::semantic::Mode,
        cancel: &AtomicBool,
    ) -> Result<Value> {
        let storage = workspace::storage(None)?;
        let source = self.indexed_source(&storage)?;
        let out =
            workspace::query::search_mode(&storage, &self.select()?, &[source], req, mode, cancel)?;
        self.indexed_reply(out)
    }

    pub(super) fn indexed_reply(&self, mut out: Value) -> Result<Value> {
        // The file at this path with exactly these bytes. A relocated workspace keeps the source
        // id it registered, which its refs keep for workspace consumers; `source` names this
        // revision only to accept the reply.
        let current = match out["sources"].as_array().map(Vec::as_slice) {
            Some([one]) => {
                one["state"] == "ready"
                    && one["path"] == self.source["path"]
                    && one["snapshotId"] == self.source["snapshotId"]
            }
            _ => false,
        };
        if !current {
            return Err(FigError::new(
                crate::ErrorKind::ModeUnavailable,
                "this revision has no ready index of the scope; enable indexing for this revision",
            )
            .into());
        }
        out["source"] = self.source.clone();
        Ok(out)
    }

    /// `semantic/<revision>?scope=ID`, read-only: the model profiles' readiness, this file's
    /// workspace (a missing one stays missing) and the viewer's own build of `scope`, if any.
    pub(super) fn semantic(&self, rest: &str) -> Result<Value> {
        let (revision, query) = rest.split_once('?').unwrap_or((rest, ""));
        let scope = match form(query)?.as_slice() {
            [(key, scope)] if key == "scope" => self.indexed_scope(revision, Some(scope))?,
            _ => self.indexed_scope(revision, None)?,
        };
        let storage = workspace::storage(None)?;
        let models = crate::semantic::profile::status(&storage)?;
        let index = workspace::status(&storage, &self.select()?)?;
        let owned = self.indexing.0.lock().map_err(|_| "indexing poisoned")?;
        let job = owned.as_ref().filter(|o| o.scope == scope).map(|o| {
            let report = o.state.lock().map_or(Value::Null, |s| s.clone());
            json!({"running": !o.thread.is_finished(), "visual": o.visual, "report": report})
        });
        Ok(
            json!({"scope": scope, "models": {"profiles": models["profiles"], "setup": models["setup"]},
            "index": index, "job": job}),
        )
    }

    /// `POST enable/<revision>` with `{"scope": ID}`, the user's explicit Enable index: starts
    /// this viewer's build of `scope` (rejoining it while it runs). The only route that creates
    /// a workspace.
    pub(super) fn post(&self, route: &str, body: &[u8]) -> Result<(&'static str, Vec<u8>)> {
        if let Some(revision) = route.strip_prefix("cancel/") {
            let result = (|| -> Result<Value> {
                if revision.parse::<u64>()? != self.generation {
                    return Err(STALE.into());
                }
                let object: serde_json::Map<String, Value> = serde_json::from_slice(body)?;
                let (1, Some(id)) = (
                    object.len(),
                    object.get("requestId").and_then(Value::as_str),
                ) else {
                    return Err("takes exactly one requestId".into());
                };
                Ok(json!({"accepted": self.queries.cancel(id)?}))
            })()
            .unwrap_or_else(|e| refusal(&*e, &self.source));
            return Ok(("application/json", result.to_string().into_bytes()));
        }
        let revision = route.strip_prefix("enable/").ok_or("unknown route")?;
        let reply = serde_json::from_slice::<serde_json::Map<String, Value>>(body)
            .map_err(|_| "takes a JSON object".into())
            .and_then(|o| {
                let visual = match o.get("visual") {
                    None => false,
                    Some(Value::Bool(value)) => *value,
                    _ => return Err("visual takes true or false".into()),
                };
                match (o.len(), o.get("scope").and_then(Value::as_str)) {
                    (n, Some(scope)) if n == 1 + usize::from(o.contains_key("visual")) => {
                        self.enable(revision, scope, visual)
                    }
                    _ => Err("takes exactly one scope and optional visual".into()),
                }
            })
            .unwrap_or_else(|e| refusal(&*e, &self.source));
        Ok(("application/json", reply.to_string().into_bytes()))
    }

    fn enable(&self, revision: &str, scope: &str, visual: bool) -> Result<Value> {
        let scope = self.indexed_scope(revision, Some(scope))?;
        let mut owned = self.indexing.0.lock().map_err(|_| "indexing poisoned")?;
        // Checked under the lock the revision's end takes to stop the build: a request that
        // outlived its revision never admits a build the next revision would inherit.
        if self.cancel.load(Ordering::SeqCst) {
            return Err(STALE.into());
        }
        if let Some(o) = owned.as_ref()
            && !o.thread.is_finished()
        {
            if o.scope == scope && o.visual == visual {
                return Ok(json!({"started": false, "scope": scope}));
            }
            return Err(FigError::new(
                crate::ErrorKind::WorkspaceBusy,
                format!(
                    "an index build of {} is running; retry when it ends",
                    o.scope
                ),
            )
            .into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(json!({"state": "starting"})));
        let (file, run_scope) = (self.file.to_owned(), scope.clone());
        let (stop, report) = (cancel.clone(), state.clone());
        let thread = std::thread::Builder::new()
            .name("preview-index".into())
            .spawn(move || {
                if let Err(e) = build(&file, &run_scope, visual, &stop, &report)
                    && let Ok(mut s) = report.lock()
                {
                    *s = refusal(&*e, &Value::Null);
                }
            })?;
        *owned = Some(Owned {
            scope: scope.clone(),
            visual,
            cancel,
            state,
            thread,
        });
        Ok(json!({"started": true, "scope": scope}))
    }
}
