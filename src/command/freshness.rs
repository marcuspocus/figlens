//! `freshness` (CLI and MCP) and the stale hint read commands add to a missing node.

use std::error::Error;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use figlens::freshness::{self, MAPPING, Source, TOKEN_VAR};
use figlens::{ErrorKind, FigError};
use serde_json::{Value, json};

use super::Output;

/// What to check: a mapping, one file (with its URL, or found in the nearest mapping), or the
/// nearest mapping at or above `start` (the CLI's working directory).
pub struct Request<'a> {
    pub sources: Option<&'a Path>,
    pub file: Option<(&'a Path, &'a str)>,
    pub url: Option<&'a str>,
    pub start: Option<&'a Path>,
}

/// How long one listing may spend on requests (it holds the MCP interactive lane); designs it
/// could not finish in time are `unknown`/`deadline`.
const DEADLINE: Duration = Duration::from_secs(60);

/// Each requested design's saved time, online time and state; `allowed` admits a path under
/// MCP roots (the CLI admits every path). Once `cancel` is set, remaining designs are not fetched.
pub fn report(
    request: &Request,
    allowed: &dyn Fn(&Path) -> bool,
    cancel: &AtomicBool,
) -> Result<Value, Box<dyn Error>> {
    let token = freshness::token();
    let started = Instant::now();
    let check = |source: &Source| {
        if !allowed(&source.file) {
            // A missing file inside the roots is missing, not outside them.
            let inside = source
                .file
                .ancestors()
                .find(|dir| dir.exists())
                .is_some_and(allowed);
            let reason = if inside {
                "file_missing"
            } else {
                "outside_roots"
            };
            return freshness::unknown(source.name.as_str().into(), &source.label, reason);
        }
        let token = match &token {
            None => Err("no_token"),
            Some(_) if cancel.load(Ordering::Relaxed) => Err("cancelled"),
            Some(_) if started.elapsed() + freshness::REQUEST_LIMIT > DEADLINE => Err("deadline"),
            Some(token) => Ok(token.as_str()),
        };
        freshness::check("curl", source, token)
    };
    let (mapping, designs) = match (request.sources, request.file, request.url) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            return Err("freshness takes a mapping or one file, not both".into());
        }
        (None, None, Some(_)) => {
            return Err("--url needs the local <file.fig> it was saved from".into());
        }
        (None, Some((file, label)), Some(url)) => {
            let name = file
                .file_stem()
                .map_or_else(|| label.to_owned(), |s| s.to_string_lossy().into_owned());
            let source = Source {
                name,
                url: url.to_owned(),
                label: label.to_owned(),
                file: file.to_owned(),
                account: None,
            };
            (None, vec![check(&source)])
        }
        (None, Some((file, label)), None) => {
            let unknown = |reason: &str| freshness::unknown(Value::Null, label, reason);
            if !file.is_file() {
                (None, vec![unknown("file_missing")])
            } else {
                match freshness::source_for(file, allowed)? {
                    None => (None, vec![unknown("not_mapped")]),
                    Some((mapping, None)) => (Some(mapping), vec![unknown("not_mapped")]),
                    Some((mapping, Some(source))) => (Some(mapping), vec![check(&source)]),
                }
            }
        }
        (sources, None, None) => {
            let mapping = match (sources, request.start) {
                (Some(path), _) => path.to_owned(),
                (None, Some(start)) => freshness::find_mapping(start, allowed).ok_or_else(|| {
                    format!(
                        "no {MAPPING} at or above this directory; pass --sources <mapping.json>, or <file.fig> --url <figma-url>"
                    )
                })?,
                (None, None) => return Err("freshness needs sources (a mapping) or file".into()),
            };
            let designs = freshness::read_mapping(&mapping)?
                .iter()
                .map(check)
                .collect();
            (Some(mapping), designs)
        }
    };
    Ok(json!({
        "mapping": mapping.map(|m| m.to_string_lossy().into_owned()),
        "tokenVar": TOKEN_VAR,
        "tokenSet": token.is_some(),
        "designs": designs,
    }))
}

/// Adds the stale hint to a missing node, or to a find that matched nothing, when the copy is
/// known to be older than its online file. Other results pass through without a network call.
pub fn with_hint(
    command: &str,
    result: Result<Output, Box<dyn Error>>,
    fig: &Path,
    allowed: &dyn Fn(&Path) -> bool,
) -> Result<Output, Box<dyn Error>> {
    match result {
        Err(error) => {
            let Some(missing) = error
                .downcast_ref::<FigError>()
                .filter(|e| e.kind() == ErrorKind::NodeNotFound)
            else {
                return Err(error);
            };
            let Some(stale) = freshness::stale(fig, allowed) else {
                return Err(error);
            };
            let mut hinted = FigError::new(
                ErrorKind::NodeNotFound,
                format!("{missing}; {}", freshness::stale_hint(&stale)),
            );
            if let Some(recovery) = missing.recovery() {
                hinted = hinted.with_recovery(recovery.clone());
            }
            Err(hinted.into())
        }
        Ok(Output::Json { mut value, pretty })
            if command == "find" && value["results"].as_array().is_some_and(Vec::is_empty) =>
        {
            if let Some(mut stale) = freshness::stale(fig, allowed) {
                stale["hint"] = freshness::stale_hint(&stale).into();
                value["freshness"] = stale;
            }
            Ok(Output::Json { value, pretty })
        }
        ok => ok,
    }
}
