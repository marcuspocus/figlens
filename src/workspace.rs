//! Local workspaces: the figlens application roots, where each workspace's storage lives,
//! workspace identity and ownership, and the SQLite registry of a workspace's sources. Load
//! the `workspace` wiki topic before changing it.

use std::error::Error;
use std::ffi::OsString;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};

use crate::{ErrorKind, FigError};

/// The workspace schema this binary writes. Schemas 1 to 10 are upgraded in place, keeping every
/// registered source and index; any newer version is left untouched.
pub const SCHEMA: i64 = 11;
/// The version of a remembered workspace location record this binary writes; version 1 (no
/// binding) is still read.
const LOCATION: i64 = 2;
/// The longest wait for another process's workspace lock.
#[cfg(not(test))]
const LOCK_WAIT: Duration = Duration::from_secs(5);
#[cfg(test)]
const LOCK_WAIT: Duration = Duration::from_secs(2);
const MARKER: &str = "workspace.json";
const SMALL_FILE: u64 = 64 * 1024;
const DB: &str = "index.sqlite";
/// The database and every file SQLite keeps beside it; reset removes exactly these.
const DB_FILES: [&str; 4] = [
    DB,
    "index.sqlite-wal",
    "index.sqlite-shm",
    "index.sqlite-journal",
];

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn fail(kind: ErrorKind, message: impl Into<String>) -> Box<dyn Error> {
    FigError::new(kind, message).into()
}

/// The error kind and the recovery an agent or user can act on for a storage I/O failure.
/// None of them is another --storage: that would meet the same blocked storage again.
fn recovery(error: &std::io::Error) -> (ErrorKind, &'static str) {
    use std::io::ErrorKind as Io;
    match error.kind() {
        Io::PermissionDenied | Io::ReadOnlyFilesystem => (
            ErrorKind::PermissionDenied,
            "restore access to it (figlens never changes permissions), then retry",
        ),
        Io::NotFound | Io::NotConnected | Io::TimedOut | Io::StaleNetworkFileHandle => (
            ErrorKind::StorageUnavailable,
            "it is missing or offline: reconnect or restore it, then retry",
        ),
        _ => (
            ErrorKind::OperationFailed,
            "fix the storage there, then retry",
        ),
    }
}

/// The current storage of workspace `id` cannot be checked at `path`. Its database may still
/// exist, so no other storage may take over: the only way on is access to this one.
fn blocked(id: &str, path: &Path, error: std::io::Error) -> Box<dyn Error> {
    let (kind, next) = recovery(&error);
    let message = format!(
        "workspace {id} may still have its database under {}, which cannot be checked: {error}; {next}; figlens does not start another database while it may exist",
        path.display()
    );
    match kind {
        ErrorKind::StorageUnavailable | ErrorKind::PermissionDenied => FigError::new(kind, message)
            .with_recovery(unusable(kind))
            .into(),
        _ => fail(kind, message),
    }
}

/// The recovery of a workspace whose storage is unusable: restore it, or recreate the workspace
/// at a new location from an explicitly authorized source, leaving every old file in place.
fn unusable(kind: ErrorKind) -> Json {
    let restore = if kind == ErrorKind::PermissionDenied {
        "restore_access"
    } else {
        "restore_storage"
    };
    json!([{"action": restore, "requires": "none",
            "detail": "make the named storage reachable again, then retry the same call"},
        recreate_elsewhere()])
}

/// Recreating at a new location: a new database there knows only the sources named in the call.
fn recreate_elsewhere() -> Json {
    json!({"action": "recreate_elsewhere", "requires": "user_confirmation",
        "tool": "index", "arguments": {"action": "prepare", "file": "<a .fig of this workspace>",
            "storage": "<new absolute directory>", "replaceUnavailable": true},
        "cli": "figlens index prepare <file.fig> --storage <dir> --replace-unavailable",
        "effects": {"keeps": "the old database and every old file, untouched",
            "new": "an empty workspace with a new incarnation that knows only the given source"}})
}

/// A storage failure names its path and never moves on to another directory.
fn storage_error(path: &Path, error: std::io::Error) -> Box<dyn Error> {
    let (kind, next) = recovery(&error);
    let failure = FigError::new(
        kind,
        format!(
            "cannot use figlens storage at {}: {error}; {next}; nothing was written elsewhere",
            path.display()
        ),
    );
    match unusable(kind).get(0) {
        Some(restore) if kind != ErrorKind::OperationFailed => {
            let mut restore = restore.clone();
            restore["path"] = json!(path);
            failure.with_recovery(json!([restore])).into()
        }
        _ => failure.into(),
    }
}

/// A recovery action for the workspace at `dir`, so a host can authorize that placement.
fn placed(mut action: Json, dir: &Path) -> Json {
    action["path"] = json!(dir);
    action["lock"] = json!(lock_beside(dir));
    action
}

/// `<workspaces>/<workspaceId>.lock`, the lock beside workspace directory `dir`, which a
/// prepare or reset also writes; hosts authorize it from here rather than derive it.
pub(crate) fn lock_beside(dir: &Path) -> PathBuf {
    let mut path = dir.as_os_str().to_owned();
    path.push(".lock");
    PathBuf::from(path)
}

/// `e` with the config root named in each storage or placed workspace recovery action, so a
/// host can ask for access to it (recreation writes the location record there) before running
/// it. A caller confined to roots is not offered a reset, which it cannot run.
pub fn rooted(storage: &Storage, e: Box<dyn Error>) -> Box<dyn Error> {
    let Some(fig) = e.downcast_ref::<FigError>() else {
        return e;
    };
    let Some(Json::Array(actions)) = fig.recovery() else {
        return e;
    };
    // Only the config root is known for certain; the failing storage is the action's `path`.
    let roots = json!({"config": {"path": storage.defaults.config.path}});
    let storage_action = |a: &Json| {
        matches!(
            a["action"].as_str(),
            Some("restore_access" | "restore_storage" | "recreate_elsewhere")
        ) || (matches!(a["action"].as_str(), Some("prepare" | "reset")) && a.get("path").is_some())
    };
    if !actions.iter().any(storage_action) {
        return e;
    }
    let actions: Vec<Json> = actions
        .iter()
        .filter(|a| storage.visible.is_none() || a["action"] != "reset")
        .map(|a| {
            let mut a = a.clone();
            if storage_action(&a) {
                a["roots"] = roots.clone();
            }
            a
        })
        .collect();
    FigError::new(fig.kind(), fig.to_string())
        .with_recovery(json!(actions))
        .into()
}

fn preserved(path: &Path, why: &str) -> Box<dyn Error> {
    fail(
        ErrorKind::OperationFailed,
        format!(
            "{} {why}; figlens does not own it and left it as found",
            path.display()
        ),
    )
}

fn utf8(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| {
        fail(
            ErrorKind::InvalidArguments,
            format!("{} is not valid UTF-8", path.display()),
        )
    })
}

/// Whether `path` exists as a plain directory (`dir`) or plain file, never following a
/// symlink. Another kind of entry is refused and preserved.
fn plain(path: &Path, dir: bool) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if (dir && meta.is_dir()) || (!dir && meta.is_file()) => Ok(true),
        Ok(_) => Err(preserved(
            path,
            if dir {
                "is not a plain directory"
            } else {
                "is not a plain file"
            },
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(storage_error(path, e)),
    }
}

/// Creates a plain directory unless it exists; true when this call created it.
fn mkdir(path: &Path) -> Result<bool> {
    match std::fs::create_dir(path) {
        // The new entry must outlive a crash before anything is built inside it.
        Ok(()) => {
            if let Some(parent) = path.parent() {
                sync_dir(parent)?;
            }
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            plain(path, true)?;
            Ok(false)
        }
        Err(e) => Err(storage_error(path, e)),
    }
}

fn read_small(path: &Path) -> Result<Json> {
    let mut text = String::new();
    File::open(path)
        .and_then(|f| f.take(SMALL_FILE).read_to_string(&mut text))
        .map_err(|e| storage_error(path, e))?;
    serde_json::from_str(&text).map_err(|_| preserved(path, "is not a figlens record"))
}

/// Writes `value` to `path` through a sibling temporary file and a rename, asking the OS to
/// flush the bytes and the new entry before the caller goes on, so a database created after a
/// location record does not outlive that record in a crash the filesystem honours flushes for.
/// Callers hold the lock that serializes writers of `path`, so a leftover `.part` is stale.
fn write_small(path: &Path, value: &Json) -> Result<()> {
    let text = format!("{value}\n");
    // read_small reads at most SMALL_FILE bytes, so a longer record could never be read back.
    if text.len() as u64 > SMALL_FILE {
        return Err(fail(
            ErrorKind::InvalidArguments,
            format!(
                "{}: the record would be {} bytes, over the {SMALL_FILE}-byte limit; use shorter paths",
                path.display(),
                text.len()
            ),
        ));
    }
    let part = path.with_extension("json.part");
    if plain(&part, false)? {
        std::fs::remove_file(&part).map_err(|e| storage_error(&part, e))?;
    }
    // create_new never follows a link planted at `part`.
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)
        .and_then(|mut f| {
            std::io::Write::write_all(&mut f, text.as_bytes())?;
            f.sync_all()
        })
        .and_then(|()| replace(&part, path));
    written.map_err(|e| {
        let _ = std::fs::remove_file(&part);
        storage_error(path, e)
    })?;
    match path.parent() {
        Some(dir) => sync_dir(dir),
        None => Ok(()),
    }
}

#[cfg(unix)]
fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

/// Returns only once the move is flushed to disk, which syncing the directory gives on Unix.
#[cfg(windows)]
fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let wide = |p: &Path| -> std::io::Result<Vec<u16>> {
        let wide: Vec<u16> = p.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path contains a NUL character",
            ));
        }
        Ok(wide.into_iter().chain([0]).collect())
    };
    let (from, to) = (wide(from)?, wide(to)?);
    // SAFETY: both are NUL-terminated UTF-16 paths that outlive the call.
    let moved = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Makes the entries of `dir` durable; an error stops the caller before it builds on them.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> Result<()> {
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| storage_error(dir, e))
}

/// A no-op: std cannot open a directory to sync it on Windows. Records get a file sync and a
/// same-volume write-through move (`replace`); new directories are not flushed explicitly.
#[cfg(windows)]
fn sync_dir(_: &Path) -> Result<()> {
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

// ---- Application roots ----

pub mod changes;
mod database;
pub mod jobs;
pub mod library;
pub mod model;
pub mod query;
mod readiness;
mod registry;
mod storage;
pub mod usages;
pub mod visual;
pub mod visual_job;
pub mod visual_search;

use database::*;
use storage::*;

pub(crate) use database::Opened;
pub use jobs::{clear_cache, rebuild, refresh};
pub use registry::{
    Outcome, PrepareOptions, add, add_dir, add_remote, list_add, prepare, relocate, remove, reset,
    status,
};
pub use storage::{Locator, Root, Roots, Select, Storage, config_root, storage};

pub mod watch;

#[cfg(test)]
pub(crate) use database::{V2, V3, V4, V8, V11};
#[cfg(test)]
mod tests;
