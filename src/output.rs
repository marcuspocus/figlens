//! Source-safe publication of completed output files.
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{ErrorKind, FigError};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Pending(PathBuf);

impl Drop for Pending {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn destination(out: &Path) -> io::Result<PathBuf> {
    let mut out = out.to_owned();
    for _ in 0..40 {
        match fs::canonicalize(&out) {
            Ok(path) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if fs::symlink_metadata(&out).is_ok_and(|m| m.file_type().is_symlink()) {
                    out = out
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join(fs::read_link(&out)?);
                    continue;
                }
                let parent = out
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let name = out
                    .file_name()
                    .ok_or_else(|| io::Error::other("output needs a filename"))?;
                let folder = fs::canonicalize(parent).map_err(|e| {
                    let reason = if e.kind() == io::ErrorKind::NotFound {
                        "does not exist; create it first".to_owned()
                    } else {
                        e.to_string()
                    };
                    io::Error::new(
                        e.kind(),
                        format!("output folder {}: {reason}", parent.display()),
                    )
                })?;
                return Ok(folder.join(name));
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("too many output symlinks"))
}

fn distinct(source: &Path, out: &Path) -> Result<(), Box<dyn Error>> {
    // An empty source means none, such as a selection read from the clipboard.
    if source.as_os_str().is_empty() {
        return Ok(());
    }
    match fs::canonicalize(source) {
        Ok(source) if source == out => Err(FigError::new(
            ErrorKind::InvalidArguments,
            "output must not overwrite the source design",
        )
        .into()),
        Ok(_) => Ok(()),
        // Library callers can supply a synthetic tree with no source archive.
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Write through an exclusively owned sibling, preserving the destination on failure.
/// Existing output symlinks retain their target and encoding is selected by the caller.
pub fn write(
    source: &Path,
    out: &Path,
    write: impl FnOnce(&mut File) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let out = destination(out).map_err(|e| FigError::new(ErrorKind::OutputWrite, e.to_string()))?;
    distinct(source, &out)?;
    let publish = || -> Result<(), Box<dyn Error>> {
        let permissions = match fs::metadata(&out) {
            Ok(metadata) => Some(metadata.permissions()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if permissions.as_ref().is_some_and(fs::Permissions::readonly) {
            return Err(
                io::Error::new(io::ErrorKind::PermissionDenied, "output is read-only").into(),
            );
        }
        let pending;
        let mut file;
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = out
                .parent()
                .unwrap()
                .join(format!(".figlens-{}-{id}.part", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(opened) => {
                    pending = Pending(path);
                    file = opened;
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        let result = write(&mut file);
        drop(file);
        result?;
        distinct(source, &out)?;
        fs::rename(&pending.0, &out)?;
        Ok(())
    };
    publish().map_err(|error| {
        if error.downcast_ref::<FigError>().is_some() {
            error
        } else {
            Box::new(FigError::new(ErrorKind::OutputWrite, error.to_string()))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn a_missing_output_folder_is_named() {
        let dir = crate::test_support::TestDir::new();
        let source = dir.join("a.fig");
        let error = write(&source, &dir.join("missing/x.png"), |_| Ok(()))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("missing") && error.contains("does not exist; create it first"),
            "{error}"
        );
    }

    #[test]
    fn failed_and_panicking_writers_preserve_existing_outputs_and_clean_owned_files() {
        let dir = crate::test_support::TestDir::new();
        let source = dir.join("source.fig");
        let out = dir.join("out.zip");
        fs::write(&source, b"source").unwrap();
        fs::write(&out, b"old").unwrap();
        assert!(
            write(&source, &out, |file| {
                file.write_all(b"partial")?;
                Err(io::Error::other("injected").into())
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| write(&source, &out, |file| {
                file.write_all(b"partial")?;
                panic!("injected")
            }))
            .is_err()
        );
        assert_eq!(fs::read(&out).unwrap(), b"old");
        assert_eq!(fs::read_dir(dir.join("")).unwrap().count(), 2);
        write(&source, &out, |file| {
            file.write_all(b"complete")?;
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"complete");
        assert_eq!(fs::read(&source).unwrap(), b"source");
    }

    #[test]
    fn source_aliases_are_rejected_and_hard_link_publication_preserves_the_source() {
        let dir = crate::test_support::TestDir::new();
        let source = dir.join("source.fig");
        fs::write(&source, b"source").unwrap();
        assert!(write(&source, &source, |_| panic!("must not write")).is_err());
        let hard = dir.join("hard.zip");
        fs::hard_link(&source, &hard).unwrap();
        write(&source, &hard, |file| {
            file.write_all(b"bundle")?;
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(fs::read(&hard).unwrap(), b"bundle");
        #[cfg(unix)]
        {
            let alias = dir.join("alias.zip");
            std::os::unix::fs::symlink("source.fig", &alias).unwrap();
            assert!(write(&source, &alias, |_| panic!("must not write")).is_err());
            assert!(
                fs::symlink_metadata(&alias)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            let dangling = dir.join("dangling.zip");
            std::os::unix::fs::symlink("new.zip", &dangling).unwrap();
            write(&source, &dangling, |file| {
                file.write_all(b"new")?;
                Ok(())
            })
            .unwrap();
            assert_eq!(fs::read(dir.join("new.zip")).unwrap(), b"new");
            assert!(
                fs::symlink_metadata(&dangling)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        }
    }

    #[test]
    fn concurrent_publication_never_mixes_writes_and_failed_rename_cleans_the_staging_file() {
        let dir = crate::test_support::TestDir::new();
        let source = dir.join("source.fig");
        let out = dir.join("out.zip");
        fs::write(&source, b"source").unwrap();
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            for byte in *b"ab" {
                let (source, out, barrier) = (&source, &out, &barrier);
                scope.spawn(move || {
                    write(source, out, |file| {
                        file.write_all(&vec![byte; 65536])?;
                        barrier.wait();
                        Ok(())
                    })
                    .unwrap()
                });
            }
        });
        let bytes = fs::read(&out).unwrap();
        assert!(bytes == vec![b'a'; 65536] || bytes == vec![b'b'; 65536]);
        let directory = dir.join("directory.zip");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("owner"), b"keep").unwrap();
        assert!(
            write(&source, &directory, |file| {
                file.write_all(b"partial")?;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(fs::read(directory.join("owner")).unwrap(), b"keep");
        assert_eq!(fs::read_dir(dir.join("")).unwrap().count(), 3);
    }
}
