//! Where a workspace lives: the application roots, custom and remembered storage, location
//! records, the workspace directory, its marker and its locks.

use super::*;

/// One resolved application root and the rule that chose it.
#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    pub path: PathBuf,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Roots {
    pub config: Root,
    pub state: Root,
    pub cache: Root,
    pub data: Root,
}

impl Roots {
    pub(super) fn report(&self) -> Json {
        let root = |r: &Root| json!({"path": r.path, "rule": r.rule});
        json!({"config": root(&self.config), "state": root(&self.state),
            "cache": root(&self.cache), "data": root(&self.data)})
    }

    /// State, cache and data under the application root `dir`; config never moves, so the
    /// remembered locations stay findable.
    pub(super) fn under(&self, dir: &Path, rule: &str) -> Self {
        let root = |kind: &str| Root {
            path: dir.join(kind),
            rule: rule.into(),
        };
        Self {
            config: self.config.clone(),
            state: root("state"),
            cache: root("cache"),
            data: root("data"),
        }
    }
}

/// The default roots and the command's explicit `--storage` application root, if any.
#[derive(Debug, Clone)]
pub struct Storage {
    pub(super) defaults: Roots,
    pub(super) explicit: Option<PathBuf>,
    /// Canonical design roots a caller may see (MCP `--root`); sources outside them are left
    /// out of every report, search and job, with no path, name or count. `None` sees all.
    pub(super) visible: Option<Vec<PathBuf>>,
    /// Canonical storage directories an MCP server may use besides the default roots
    /// (`--storage-root`); `None` allows any, as the CLI does.
    pub(super) allowed: Option<Vec<PathBuf>>,
}

impl Storage {
    /// Allows custom storage only under `roots` (canonical paths); the default roots stay allowed.
    pub fn allow(mut self, roots: &[PathBuf]) -> Self {
        self.allowed = Some(roots.to_vec());
        self
    }

    /// Refuses a custom storage directory outside the allowed roots before anything there is
    /// read, so an excluded location's state is never revealed.
    pub(super) fn authorized(&self, dir: &Path) -> Result<()> {
        let Some(allowed) = &self.allowed else {
            return Ok(());
        };
        // Judged where the path really leads, links included, before anything is created
        // there; `..` is refused outright, since a missing directory before it hides where the
        // rest leads, and a path that cannot be resolved is refused.
        let defaults = [
            &self.defaults.config.path,
            &self.defaults.state.path,
            &self.defaults.cache.path,
            &self.defaults.data.path,
        ];
        if !dir
            .components()
            .any(|c| c == std::path::Component::ParentDir)
            && let Some(dir) = resolved(dir)
            && allowed
                .iter()
                .chain(defaults)
                .filter_map(|r| resolved(r))
                .any(|r| dir.starts_with(r))
        {
            return Ok(());
        }
        Err(FigError::new(
            ErrorKind::PermissionDenied,
            format!(
                "storage {} is outside this server's storage roots; start figlens mcp with --storage-root <dir> to allow it",
                dir.display()
            ),
        )
        .into())
    }

    /// figlens's state folder for this user (`$XDG_STATE_HOME/figlens`, `~/.local/state/figlens`
    /// or `%LOCALAPPDATA%\\figlens\\state`); nothing is created.
    pub fn state_dir(&self) -> &Path {
        &self.defaults.state.path
    }

    /// Limits every report, search and job to sources under `roots` (canonical paths).
    pub fn confined(mut self, roots: &[PathBuf]) -> Self {
        if !roots.is_empty() {
            self.visible = Some(roots.to_vec());
        }
        self
    }

    /// Whether a registered source is visible to this caller, judged where the path leads now,
    /// so a file later replaced by a link out of the roots is not read through it.
    pub(super) fn sees(&self, path: &Path) -> bool {
        self.visible.as_ref().is_none_or(|roots| {
            resolved(path).is_some_and(|path| roots.iter().any(|r| path.starts_with(r)))
        })
    }
}

/// (name, XDG variable, default under HOME) for each root.
pub(super) const KINDS: [(&str, &str, &str); 4] = [
    ("config", "XDG_CONFIG_HOME", ".config"),
    ("state", "XDG_STATE_HOME", ".local/state"),
    ("cache", "XDG_CACHE_HOME", ".cache"),
    ("data", "XDG_DATA_HOME", ".local/share"),
];

/// Resolves the default roots from the environment and validates `explicit`. Nothing is
/// created here.
pub fn storage(explicit: Option<&Path>) -> Result<Storage> {
    resolve(
        explicit,
        &|name| std::env::var_os(name),
        cfg!(windows),
        &local_app_data,
    )
}

pub(super) fn resolve(
    explicit: Option<&Path>,
    env: &dyn Fn(&str) -> Option<OsString>,
    windows: bool,
    known: &dyn Fn() -> Option<PathBuf>,
) -> Result<Storage> {
    if let Some(dir) = explicit
        && (!dir.is_absolute() || (dir.exists() && !dir.is_dir()))
    {
        return Err(fail(
            ErrorKind::InvalidArguments,
            format!(
                "--storage {} must be an absolute directory path",
                dir.display()
            ),
        ));
    }
    if let Some(dir) = explicit {
        // Checked now: a prepare must not build a database it cannot remember.
        utf8(dir)?;
    }
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let default = |(kind, xdg, home): (&str, &str, &str)| -> Result<Root> {
        if let Some(dir) = absolute(xdg) {
            return Ok(Root {
                path: dir.join("figlens"),
                rule: xdg.into(),
            });
        }
        if windows {
            return windows_root(kind, absolute("LOCALAPPDATA"), known);
        }
        let base = absolute("HOME").ok_or_else(|| {
            fail(
                ErrorKind::InvalidArguments,
                format!("no {kind} directory: HOME is not an absolute path; set HOME or {xdg} (config is needed even with --storage)"),
            )
        })?;
        Ok(Root {
            path: base.join(home).join("figlens"),
            rule: "HOME".into(),
        })
    };
    let [config, state, cache, data] = KINDS;
    Ok(Storage {
        defaults: Roots {
            config: default(config)?,
            state: default(state)?,
            cache: default(cache)?,
            data: default(data)?,
        },
        explicit: explicit.map(Path::to_path_buf),
        visible: None,
        allowed: None,
    })
}

/// Windows has no HOME requirement: %LOCALAPPDATA%, then its Known Folder.
pub(super) fn windows_root(
    kind: &str,
    local: Option<PathBuf>,
    known: &dyn Fn() -> Option<PathBuf>,
) -> Result<Root> {
    let (dir, rule) = match local {
        Some(dir) => (dir, "LOCALAPPDATA"),
        None => (
            known().filter(|p| p.is_absolute()).ok_or_else(|| {
                fail(ErrorKind::InvalidArguments, format!(
                    "no {kind} directory: %LOCALAPPDATA% and the Local AppData known folder are unavailable; set %LOCALAPPDATA%"
                ))
            })?,
            "FOLDERID_LocalAppData",
        ),
    };
    Ok(Root {
        path: dir.join("figlens").join(kind),
        rule: rule.into(),
    })
}

#[cfg(windows)]
pub(super) fn local_app_data() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    };
    let mut raw = std::ptr::null_mut();
    // SAFETY: valid GUID and out pointer; the shell allocates `raw`, freed below in every case.
    let ok = unsafe {
        SHGetKnownFolderPath(
            &FOLDERID_LocalAppData,
            KF_FLAG_DEFAULT as u32,
            std::ptr::null_mut(),
            &mut raw,
        )
    } >= 0;
    let path = (ok && !raw.is_null()).then(|| {
        // SAFETY: on success `raw` is a NUL-terminated UTF-16 string.
        let len = (0..).take_while(|&i| unsafe { *raw.add(i) } != 0).count();
        PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(raw, len)
        }))
    });
    // SAFETY: `raw` came from SHGetKnownFolderPath (or is null, which CoTaskMemFree accepts).
    unsafe { CoTaskMemFree(raw as *const _) };
    path
}

#[cfg(not(windows))]
pub(super) fn local_app_data() -> Option<PathBuf> {
    None
}

// ---- Identity ----

/// What a workspace is for: a selected project directory, or one source when there is none.
#[derive(Debug, Clone, PartialEq)]
pub enum Locator {
    Project(PathBuf),
    Source(PathBuf),
}

impl Locator {
    /// The project directory `dir`, by its canonical path.
    pub fn project(dir: &Path) -> Result<Self> {
        let canonical = std::fs::canonicalize(dir)
            .ok()
            .filter(|p| p.is_dir())
            .ok_or_else(|| {
                fail(
                    ErrorKind::InvalidArguments,
                    format!("--workspace {} is not a directory", dir.display()),
                )
            })?;
        Ok(Self::Project(canonical))
    }

    /// The source-scoped workspace of the `.fig` at `file`. A file that was moved away still
    /// selects its workspace through its directory, so it can be relocated or reset.
    pub fn source(file: &Path) -> Result<Self> {
        match readable(file) {
            Ok(path) => Ok(Self::Source(path)),
            Err(e) => match (
                std::fs::symlink_metadata(file),
                file.parent(),
                file.file_name(),
            ) {
                (Err(gone), Some(dir), Some(name))
                    if gone.kind() == std::io::ErrorKind::NotFound =>
                {
                    let dir = if dir.as_os_str().is_empty() {
                        Path::new(".")
                    } else {
                        dir
                    };
                    std::fs::canonicalize(dir)
                        .map(|d| Self::Source(d.join(name)))
                        .map_err(|_| e)
                }
                _ => Err(e),
            },
        }
    }

    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::Project(_) => "project",
            Self::Source(_) => "source",
        }
    }

    pub(super) fn path(&self) -> Result<&str> {
        let (Self::Project(p) | Self::Source(p)) = self;
        utf8(p)
    }

    /// `ws_` and 64 bits of the SHA-256 of kind and canonical path. The marker, location record
    /// and database keep the full locator, so a colliding id is refused instead of reused.
    pub fn id(&self) -> Result<String> {
        let mut hash = Sha256::new();
        hash.update(self.kind());
        hash.update([0]);
        hash.update(self.path()?);
        Ok(format!(
            "ws_{}",
            &crate::search::hex(&hash.finalize())[..16]
        ))
    }

    pub(super) fn report(&self) -> Result<Json> {
        Ok(json!({"kind": self.kind(), "path": self.path()?}))
    }

    pub(super) fn parse(value: &Json) -> Option<Self> {
        let path = PathBuf::from(value["path"].as_str()?);
        match value["kind"].as_str()? {
            "project" => Some(Self::Project(path)),
            "source" => Some(Self::Source(path)),
            _ => None,
        }
    }
}

/// How a command names its workspace: by locator, or by the id an earlier call returned.
#[derive(Debug, Clone)]
pub enum Select {
    Locator(Locator),
    Id(String),
}

impl Select {
    /// At most one of a project directory, a `.fig` (its source-scoped workspace) and an id.
    pub fn from_choice(
        workspace: Option<&Path>,
        file: Option<&Path>,
        id: Option<&str>,
    ) -> Result<Option<Self>> {
        Ok(match (workspace, file, id) {
            (None, None, None) => None,
            (Some(dir), None, None) => Some(Self::Locator(Locator::project(dir)?)),
            (None, Some(file), None) => Some(Self::Locator(Locator::source(file)?)),
            (None, None, Some(id)) => Some(Self::Id(id.to_owned())),
            _ => {
                return Err(fail(
                    ErrorKind::InvalidArguments,
                    "choose one of --workspace, --file and --id",
                ));
            }
        })
    }

    pub(super) fn id(&self) -> Result<String> {
        match self {
            Self::Locator(l) => l.id(),
            Self::Id(id) => {
                let hex = id.strip_prefix("ws_").unwrap_or("");
                if hex.len() == 16 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                    Ok(id.clone())
                } else {
                    Err(fail(
                        ErrorKind::InvalidArguments,
                        format!(
                            "{id:?} is not a workspace id; ids look like ws_ and 16 hex digits"
                        ),
                    ))
                }
            }
        }
    }

    pub(super) fn locator(&self) -> Option<&Locator> {
        match self {
            Self::Locator(l) => Some(l),
            Self::Id(_) => None,
        }
    }
}

/// The canonical path of a readable regular file.
pub(super) fn readable(file: &Path) -> Result<PathBuf> {
    let source = |e: std::io::Error| {
        fail(
            ErrorKind::SourceUnreadable,
            format!("{}: {e}", file.display()),
        )
    };
    let path = std::fs::canonicalize(file).map_err(source)?;
    let regular = |meta: std::fs::Metadata| {
        if meta.is_file() {
            Ok(())
        } else {
            Err(fail(
                ErrorKind::SourceUnreadable,
                format!("{} is not a regular .fig file", file.display()),
            ))
        }
    };
    // Checked before opening too: opening a FIFO or device can block indefinitely.
    regular(std::fs::metadata(&path).map_err(source)?)?;
    regular(
        File::open(&path)
            .and_then(|f| f.metadata())
            .map_err(source)?,
    )?;
    Ok(path)
}

/// Fails when `source` lies inside any figlens root, default or placed, which an index must
/// never alias.
pub(super) fn outside_storage(source: &Path, storage: &Storage, place: &Place) -> Result<()> {
    let (d, p) = (&storage.defaults, &place.roots);
    for root in [d, p]
        .map(|r| [&r.config.path, &r.state.path, &r.cache.path, &r.data.path])
        .concat()
    {
        if let Ok(root) = std::fs::canonicalize(root)
            && source.starts_with(&root)
        {
            return Err(fail(
                ErrorKind::PermissionDenied,
                format!(
                    "{} is inside figlens storage {}; keep designs outside the storage roots",
                    source.display(),
                    root.display()
                ),
            ));
        }
    }
    Ok(())
}

// ---- Remembered locations ----

/// Where one workspace's storage lives, and why.
pub(super) struct Place {
    pub(super) roots: Roots,
    /// `--storage`, `remembered` or `default`.
    pub(super) rule: &'static str,
    /// The remembered application root, when there is one.
    pub(super) remembered: Option<PathBuf>,
    /// The nonce the location record and the database it names share; `None` for a database
    /// with no record, or one recorded before bindings existed.
    pub(super) binding: Option<String>,
}

/// A location record: where the workspace lives, which locator it serves and its binding.
pub(super) struct Remembered {
    pub(super) storage: PathBuf,
    pub(super) locator: Locator,
    pub(super) binding: Option<String>,
}

impl Place {
    pub(super) fn report(&self) -> Json {
        json!({"rule": self.rule, "remembered": self.remembered})
    }
}

impl Storage {
    /// The config root `models.json` lives in, whether or not it exists yet.
    pub fn config_root(&self) -> &Path {
        &self.defaults.config.path
    }

    /// `<config>/models.json`, the configured model profiles (#94); `None` before the first.
    pub fn models(&self) -> Result<Option<Json>> {
        let path = self.defaults.config.path.join("models.json");
        Ok(if plain(&path, false)? {
            Some(read_small(&path)?)
        } else {
            None
        })
    }

    /// Rewrites `<config>/models.json` from its current record under `models.lock`, so two
    /// updates never both start from the same old record.
    pub fn update_models(&self, change: impl FnOnce(Option<Json>) -> Result<Json>) -> Result<()> {
        let config = &self.defaults.config.path;
        create_durable(config)?;
        let path = config.join("models.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| storage_error(&path, e))?;
        lock(&file, &path, true, "model profiles")?;
        let record = change(self.models()?)?;
        write_small(&config.join("models.json"), &record)
    }

    pub(super) fn locations(&self) -> PathBuf {
        self.defaults.config.path.join("workspaces")
    }

    /// The remembered application root of workspace `id` and its locator, read from the fixed
    /// config directory. It only locates the workspace; ownership is checked at the workspace.
    pub(super) fn remembered(&self, id: &str) -> Result<Option<Remembered>> {
        let Some(locations) = locations(&self.defaults.config.path, false)? else {
            return Ok(None);
        };
        let path = locations.join(format!("{id}.json"));
        if !plain(&path, false)? {
            return Ok(None);
        }
        let record = read_small(&path)?;
        // A relocated source's alias names another workspace; this id has no location.
        if record.get("aliasOf").is_some() {
            return Ok(None);
        }
        if record["version"] != 1 && record["version"] != LOCATION {
            return Err(fail(
                ErrorKind::IndexIncompatible,
                format!(
                    "{} has location version {}; this figlens reads version {LOCATION} and left it untouched",
                    path.display(),
                    record["version"]
                ),
            ));
        }
        let storage = record["storage"]
            .as_str()
            .map(PathBuf::from)
            .filter(|p| p.is_absolute());
        let binding = match &record["binding"] {
            Json::Null => Some(None),
            Json::String(b) if b.len() == 32 => Some(Some(b.clone())),
            _ => None,
        };
        match (
            record["workspaceId"] == id,
            storage,
            Locator::parse(&record["locator"]),
            binding,
        ) {
            (true, Some(storage), Some(locator), Some(binding)) => Ok(Some(Remembered {
                storage,
                locator,
                binding,
            })),
            _ => Err(preserved(&path, "is not this workspace's location record")),
        }
    }

    /// The workspace a relocated source-scoped `.fig` belongs to: the id its alias record
    /// `<config>/workspaces/<id>.json` `{"aliasOf": ...}` names.
    pub(super) fn alias(&self, id: &str) -> Result<Option<String>> {
        let Some(locations) = locations(&self.defaults.config.path, false)? else {
            return Ok(None);
        };
        let path = locations.join(format!("{id}.json"));
        if !plain(&path, false)? {
            return Ok(None);
        }
        let record = read_small(&path)?;
        match &record["aliasOf"] {
            Json::Null => Ok(None),
            Json::String(old) if Select::Id(old.clone()).id().is_ok() => Ok(Some(old.clone())),
            _ => Err(preserved(&path, "is not a valid alias record")),
        }
    }

    /// `select`, with a relocated source's file selecting the workspace that now holds it.
    pub(super) fn follow(&self, select: &Select) -> Result<Select> {
        if let Select::Locator(locator @ Locator::Source(_)) = select
            && let Some(old) = self.alias(&locator.id()?)?
        {
            return Ok(Select::Id(old));
        }
        Ok(select.clone())
    }

    /// Precedence: this command's `--storage`, then the workspace's remembered root, then the
    /// default roots. A remembered root that is gone is an error, never a second database.
    pub(super) fn place(&self, id: &str, wanted: Option<&Locator>) -> Result<Place> {
        let remembered = self.remembered(id)?;
        if let (Some(found), Some(wanted)) = (&remembered, wanted)
            && found.locator != *wanted
        {
            return Err(preserved(
                &self.locations().join(format!("{id}.json")),
                "records a different workspace with the same id",
            ));
        }
        let binding = remembered.as_ref().and_then(|r| r.binding.clone());
        let remembered = remembered.map(|r| r.storage);
        // A remembered root is the user's own choice from config, valid for every host; only a
        // new request's storage must lie under this server's storage roots.
        if let Some(dir) = &self.explicit {
            self.authorized(dir)?;
        }
        // Its database may still be there, so even --storage must not start another one;
        // retrying the same --storage recreates its own place, which is not a second database.
        if let Some(dir) = &remembered
            && !self.explicit.as_ref().is_some_and(|e| same_dir(e, dir))
        {
            let record = self.locations().join(format!("{id}.json"));
            let gone = |why: &str| -> Box<dyn Error> {
                FigError::new(
                    ErrorKind::StorageUnavailable,
                    format!(
                        "workspace {id} is remembered at {}, which {why}; reconnect or restore it, then retry; only if its database is gone for good, delete {} to forget it, or recreate the workspace elsewhere; no other database was created",
                        dir.display(),
                        record.display()
                    ),
                )
                .with_recovery(unusable(ErrorKind::StorageUnavailable))
                .into()
            };
            match std::fs::metadata(dir) {
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => return Err(gone("is not a directory")),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(gone("is missing or offline"));
                }
                // Its database may be intact behind the error, so forgetting it is not offered.
                Err(e) => return Err(blocked(id, dir, e)),
            }
        }
        if let Some(explicit) = &self.explicit {
            // The workspace lives where it would without --storage; another root must not shadow
            // or duplicate a database there.
            let home = remembered.as_ref().map_or(self.defaults.clone(), |r| {
                self.defaults.under(r, "remembered")
            });
            let current = home.state.path.join("workspaces").join(id).join(DB);
            // Only a database known to be absent lets another root take over; an access error
            // must not read as absent.
            let exists = match std::fs::symlink_metadata(&current) {
                Ok(_) => true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => return Err(blocked(id, &current, e)),
            };
            if exists && !same_dir(&home.state.path, &explicit.join("state")) {
                return Err(fail(
                    ErrorKind::InvalidArguments,
                    format!(
                        "workspace {id} has its database at {}; omit --storage to use it, or reset it there before using --storage {}",
                        current.display(),
                        explicit.display()
                    ),
                ));
            }
        }
        Ok(match (&self.explicit, &remembered) {
            (Some(dir), _) => Place {
                roots: self.defaults.under(dir, "--storage"),
                rule: "--storage",
                binding: binding.filter(|_| remembered.as_ref().is_some_and(|r| same_dir(r, dir))),
                remembered,
            },
            (None, Some(dir)) => Place {
                roots: self.defaults.under(dir, "remembered"),
                rule: "remembered",
                remembered: remembered.clone(),
                binding,
            },
            (None, None) => Place {
                roots: self.defaults.clone(),
                rule: "default",
                remembered: None,
                binding: None,
            },
        })
    }
}

// ---- Owned directories and locks ----

/// `<root>/workspaces` of the state or config root, through the canonical root (which may be
/// an alias) and a real directory, so a symlinked leaf cannot redirect reads, writes or
/// cleanup. Created only when `create`.
/// `<config>/workspaces`, the location records. Config never moves with `--storage`, so its
/// errors say so instead of reading like a storage problem another root would avoid.
pub(super) fn locations(config: &Path, create: bool) -> Result<Option<PathBuf>> {
    workspaces(config, create).map_err(|e| match e.downcast::<FigError>() {
        Ok(f) => fail(
            f.kind(),
            format!("{f} (the figlens config root, which holds workspace locations; --storage does not change it)"),
        ),
        Err(e) => e,
    })
}

pub(super) fn workspaces(state: &Path, create: bool) -> Result<Option<PathBuf>> {
    if create {
        create_durable(state)?;
    }
    let root = match std::fs::canonicalize(state) {
        Ok(root) => root,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(storage_error(state, e)),
    };
    let dir = root.join("workspaces");
    if create {
        mkdir(&dir)?;
        return Ok(Some(dir));
    }
    Ok(plain(&dir, true)?.then_some(dir))
}

/// Creates `dir` and its missing ancestors, syncing each new entry into its parent (Unix; see
/// `sync_dir`), so a record or database written below them is not lost with them in a crash.
/// Directories that already existed are left as they are.
pub(super) fn create_durable(dir: &Path) -> Result<()> {
    let mut missing = Vec::new();
    for path in dir.ancestors() {
        match std::fs::symlink_metadata(path) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => missing.push(path),
            Err(e) => return Err(storage_error(path, e)),
        }
    }
    for path in missing.into_iter().rev() {
        match std::fs::create_dir(path) {
            Ok(()) => {}
            // Created concurrently: sync its entry all the same before building on it.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {}
            Err(e) => return Err(storage_error(path, e)),
        }
        if let Some(parent) = path.parent() {
            sync_dir(parent)?;
        }
    }
    Ok(())
}

/// Opens the lock file at `path`, creating it when `create`. Every database connection holds
/// it shared; prepare and reset hold it exclusive. It is never removed, so all processes
/// lock the same file.
pub(super) fn open_lock(path: &Path, create: bool) -> Result<Option<File>> {
    if !plain(path, false)? && !create {
        return Ok(None);
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .open(path)
        .map(Some)
        .map_err(|e| storage_error(path, e))
}

/// Takes the lock within `LOCK_WAIT`, or reports the workspace busy. std has no timed lock.
pub(super) fn lock(file: &File, path: &Path, exclusive: bool, id: &str) -> Result<()> {
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        let attempt = if exclusive {
            file.try_lock()
        } else {
            file.try_lock_shared()
        };
        match attempt {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(TryLockError::WouldBlock) => {
                return Err(fail(
                    ErrorKind::WorkspaceBusy,
                    format!(
                        "workspace {id} is in use by another figlens process; retry when it finishes"
                    ),
                ));
            }
            Err(TryLockError::Error(e)) => return Err(storage_error(path, e)),
        }
    }
}

pub(super) fn marker(id: &str, locator: &Locator) -> Result<Json> {
    Ok(
        json!({"owner": "figlens", "kind": "workspace", "workspaceId": id,
        "locator": locator.report()?}),
    )
}

/// The marker's locator when `dir` holds a figlens marker for `id`; any other marker is refused.
pub(super) fn read_marker(dir: &Path, id: &str) -> Result<Option<Locator>> {
    let path = dir.join(MARKER);
    if !plain(&path, false)? {
        return Ok(None);
    }
    let value = read_small(&path)?;
    (value["owner"] == "figlens" && value["kind"] == "workspace" && value["workspaceId"] == id)
        .then(|| Locator::parse(&value["locator"]))
        .flatten()
        .map(Some)
        .ok_or_else(|| preserved(&path, "is not this workspace's figlens marker"))
}

/// A workspace directory under its held lock.
pub(super) struct Held {
    pub(super) id: String,
    pub(super) dir: PathBuf,
    pub(super) locator: Locator,
    pub(super) _lock: File,
}

/// Locks workspace `id` exclusively and returns it, creating its directory and marker first
/// when missing. The lock sits beside the directory, so a concurrent prepare waits here and
/// never sees a directory without its marker. An existing unmarked directory is preserved.
pub(super) fn acquire_new(state: &Path, id: &str, locator: &Locator) -> Result<Held> {
    let parent = workspaces(state, true)?.expect("created");
    let lock_path = parent.join(format!("{id}.lock"));
    let lock_file = open_lock(&lock_path, true)?.expect("created");
    lock(&lock_file, &lock_path, true, id)?;
    let dir = parent.join(id);
    mkdir(&dir)?;
    match read_marker(&dir, id)? {
        Some(found) if found == *locator => {}
        Some(_) => return Err(preserved(&dir, SAME_ID)),
        // Empty, or left by a prepare interrupted before its marker: claiming it loses nothing.
        None if unclaimed(&dir)? => write_small(&dir.join(MARKER), &marker(id, locator)?)?,
        None => return Err(preserved(&dir, "has no figlens workspace marker")),
    }
    Ok(Held {
        id: id.to_owned(),
        dir,
        locator: locator.clone(),
        _lock: lock_file,
    })
}

pub(super) const SAME_ID: &str = "belongs to a different workspace with the same id";

/// Whether `dir` holds nothing but an unfinished marker write.
pub(super) fn unclaimed(dir: &Path) -> Result<bool> {
    for entry in std::fs::read_dir(dir).map_err(|e| storage_error(dir, e))? {
        let entry = entry.map_err(|e| storage_error(dir, e))?;
        if entry.file_name() != "workspace.json.part"
            || !entry.file_type().is_ok_and(|t| t.is_file())
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Locks an existing workspace, or `None` when it is absent. Reading (`!exclusive`) creates no
/// file; reset may recreate a lost lock file because the marker proves ownership.
pub(super) fn acquire(state: &Path, select: &Select, exclusive: bool) -> Result<Option<Held>> {
    let id = select.id()?;
    let Some(parent) = workspaces(state, false)? else {
        return Ok(None);
    };
    let dir = parent.join(&id);
    if !plain(&dir, true)? {
        return Ok(None);
    }
    let lock_path = parent.join(format!("{id}.lock"));
    let Some(lock_file) = open_lock(&lock_path, exclusive)? else {
        let missing = FigError::new(
            ErrorKind::OperationFailed,
            format!(
                "{} is missing; `figlens index prepare` or `figlens index reset` recreates it",
                lock_path.display()
            ),
        );
        // Placed actions only for a directory whose marker proves it is the selected workspace.
        let marker = read_marker(&dir, &id).ok().flatten();
        if !marker.is_some_and(|found| select.locator().is_none_or(|wanted| *wanted == found)) {
            return Err(missing.into());
        }
        let mut actions = vec![placed(reset_action(&id), &dir)];
        if let Some(locator) = select.locator() {
            let prepare = json!({"action": "prepare", "requires": "user_confirmation",
                "tool": "index", "arguments": registry::prepare_step(Some(locator), "")[0]["arguments"]});
            actions.insert(0, placed(prepare, &dir));
        }
        return Err(missing.with_recovery(json!(actions)).into());
    };
    lock(&lock_file, &lock_path, exclusive, &id)?;
    let locator = read_marker(&dir, &id)?
        .ok_or_else(|| preserved(&dir, "has no figlens workspace marker"))?;
    if select.locator().is_some_and(|wanted| *wanted != locator) {
        return Err(preserved(&dir, SAME_ID));
    }
    Ok(Some(Held {
        id,
        dir,
        locator,
        _lock: lock_file,
    }))
}

// ---- Database ----

pub(super) fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || resolved(a).is_some_and(|a| resolved(b).is_some_and(|b| a == b))
}

/// Whether a registered (canonical) source path still leads to itself: no link swapped in at it
/// or above it.
pub(super) fn leads_to_itself(path: &Path) -> bool {
    std::fs::canonicalize(path).is_ok_and(|p| p == path)
}

/// A registered source's bytes, read only from the file at its canonical path, the one its
/// holders authorized: a link swapped in at it or above it is refused.
pub(super) fn staged(
    path: &Path,
) -> std::result::Result<(Vec<u8>, crate::search::Stamp), Box<dyn std::error::Error>> {
    crate::search::stage_with(|| registered(path))
}

fn linked(path: &Path) -> Box<dyn std::error::Error> {
    fail(
        ErrorKind::SourceUnreadable,
        format!(
            "{} now leads elsewhere through a link; relocate the source to the file it should read",
            path.display()
        ),
    )
}

/// Opens `path`, canonical, one component at a time from `/` without following any link, so no
/// swap at it or above it, before or during the open, can lead the handle elsewhere. The caller
/// refuses a leaf that is not a regular file.
#[cfg(unix)]
pub(super) fn registered(path: &Path) -> std::result::Result<File, Box<dyn std::error::Error>> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    // Search-only directory handles, so a known file under an execute-only parent stays readable.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const DIR: libc::c_int = libc::O_PATH | libc::O_DIRECTORY;
    // O_SEARCH is in XNU from macOS 13, the floor zig 0.15.2 gives the shipped Darwin builds;
    // lowering that floor would need a fallback here.
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const DIR: libc::c_int = libc::O_SEARCH;
    let open = |at: libc::c_int, name: &std::ffi::OsStr, flags: libc::c_int| {
        let name = CString::new(name.as_bytes())?;
        // SAFETY: `name` is NUL-terminated and `at` is AT_FDCWD or an open directory handle.
        let fd = unsafe {
            libc::openat(
                at,
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            let e = std::io::Error::last_os_error();
            return Err(match e.raw_os_error() {
                Some(libc::ELOOP | libc::ENOTDIR) => linked(path),
                _ => e.into(),
            });
        }
        // SAFETY: `fd` was just opened and nothing else owns it.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    };
    let mut names = Vec::new();
    for part in path.components() {
        match part {
            std::path::Component::RootDir if names.is_empty() => {}
            std::path::Component::Normal(name) => names.push(name),
            _ => return Err(linked(path)),
        }
    }
    if !path.is_absolute() {
        return Err(linked(path));
    }
    let (leaf, dirs) = names.split_last().ok_or_else(|| linked(path))?;
    let mut dir = open(libc::AT_FDCWD, "/".as_ref(), DIR)?;
    for name in dirs {
        dir = open(dir.as_raw_fd(), name, DIR)?;
    }
    // Nonblocking, so a FIFO swapped in is refused below instead of blocking the open.
    let file = File::from(open(
        dir.as_raw_fd(),
        leaf,
        libc::O_RDONLY | libc::O_NONBLOCK,
    )?);
    Ok(file)
}

/// Opens `path` and checks the handle's own final path is it, which a link anywhere on the way
/// changes; the system answers for the handle, so no swap after the open can fool it.
#[cfg(windows)]
pub(super) fn registered(path: &Path) -> std::result::Result<File, Box<dyn std::error::Error>> {
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
    // Checked before opening too, so a device or pipe a link leads to is never opened.
    if !leads_to_itself(path) {
        return Err(linked(path));
    }
    let file = File::open(path)?;
    let mut buf = vec![0u16; 32_768];
    // SAFETY: the handle is open for this call and `buf` holds `buf.len()` wide characters;
    // flags 0 ask for the normalized `\\?\` DOS path, as `canonicalize` gives.
    let n = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle() as _,
            buf.as_mut_ptr(),
            buf.len() as u32,
            0,
        )
    } as usize;
    if n == 0 || n >= buf.len() {
        return Err(std::io::Error::last_os_error().into());
    }
    if PathBuf::from(std::ffi::OsString::from_wide(&buf[..n])) != path {
        return Err(linked(path));
    }
    Ok(file)
}

/// `path` with its longest existing prefix canonicalized and the missing rest applied
/// lexically, so two spellings of one directory compare equal even after it was removed
/// (`C:\store` and a remembered `\\?\C:\store`, or a path through `..`).
pub(super) fn resolved(path: &Path) -> Option<PathBuf> {
    for base in path.ancestors() {
        match std::fs::canonicalize(base) {
            Ok(mut dir) => {
                for part in path.strip_prefix(base).ok()?.components() {
                    match part {
                        std::path::Component::Normal(name) => dir.push(name),
                        std::path::Component::ParentDir => {
                            dir.pop();
                        }
                        std::path::Component::CurDir => {}
                        _ => return None,
                    }
                }
                return Some(dir);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
    }
    None
}
