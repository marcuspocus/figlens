//! Create a local mapping scaffold without browser or API access.
use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use figlens::freshness::MAPPING;
use serde_json::{Value, json};

const SCAN_ENTRIES: usize = 4096;
const MAX_DESIGNS: usize = 200;
const MAX_BYTES: usize = 1 << 20;

pub fn init(root: &Path) -> Result<Value, Box<dyn Error>> {
    let root = fs::canonicalize(root)?;
    let out = root.join(MAPPING);
    if fs::symlink_metadata(&out).is_ok() {
        return Err(format!("{} already exists; edit it instead", out.display()).into());
    }
    let mut files = Vec::new();
    scan(&root, &root, 0, &mut 0, &mut files)?;
    files.sort();
    let mut names = HashSet::new();
    let mut designs = Vec::new();
    for file in files {
        let base = figlens::freshness::saved_name(&root.join(&file)).unwrap_or_else(|| {
            file.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let mut name = base.clone();
        let mut suffix = 2;
        while !names.insert(name.clone()) {
            name = format!("{base}-{suffix}");
            suffix += 1;
        }
        let label = file
            .to_str()
            .ok_or("a .fig path is not valid UTF-8")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        designs.push(json!({"name": name, "url": "", "file": label}));
    }
    if designs.is_empty() {
        return Err("no .fig files found; no mapping was created".into());
    }
    let mut bytes = serde_json::to_vec_pretty(&designs)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_BYTES {
        return Err("the generated mapping exceeds 1 MiB; use a smaller directory".into());
    }
    let folder = out.parent().unwrap();
    match fs::symlink_metadata(folder) {
        Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
            return Err(".figlens must be a directory, not a symlink".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(folder)?,
        Err(error) => return Err(error.into()),
    }
    // Publish a completed file without replacing any mapping created during the scan.
    let pending = folder.join(format!(".sources-{}.part", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        file.write_all(&bytes)?;
        drop(file);
        fs::hard_link(&pending, &out)?;
        Ok(())
    })();
    let _ = fs::remove_file(&pending);
    result?;
    Ok(
        json!({"mapping": out, "designs": designs, "hint": "fill in each url from Figma before pulling; optional account selects a browser/API alias, not a credential"}),
    )
}

fn scan(
    root: &Path,
    dir: &Path,
    depth: usize,
    count: &mut usize,
    files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    if depth > 64 {
        return Err(
            "sources init scan exceeds 64 directories deep; use a smaller directory".into(),
        );
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        *count += 1;
        if *count > SCAN_ENTRIES {
            return Err("sources init scan exceeds 4096 entries; use a smaller directory".into());
        }
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with('.') && !matches!(name.as_ref(), "node_modules" | "target") {
                scan(root, &path, depth + 1, count, files)?;
            }
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("fig"))
        {
            files.push(path.strip_prefix(root)?.to_owned());
            if files.len() > MAX_DESIGNS {
                return Err("a mapping lists at most 200 designs; use a smaller directory".into());
            }
        }
    }
    Ok(())
}
