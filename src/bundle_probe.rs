//! Opt-in development probe; never linked into the CLI or MCP server.
use super::*;
use std::io::Read;
use std::time::Instant;

fn stage<T>(name: &str, rows: &mut Vec<Json>, action: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = action();
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let read = |name: &str| {
        status.lines().find_map(|line| {
            line.strip_prefix(name)?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
                .map(|n| n * 1024)
        })
    };
    rows.push(json!({"stage": name, "ms": elapsed, "rssBytes": read("VmRSS:"), "lifetimeHighWaterBytes": read("VmHWM:")}));
    value
}

#[test]
#[ignore = "manual bounded local benchmark; requires FIGLENS_PROBE_OUTPUT"]
fn stages() {
    let output = std::env::var("FIGLENS_PROBE_OUTPUT").expect("FIGLENS_PROBE_OUTPUT");
    let file = std::env::var("FIGLENS_PROBE_FILE")
        .unwrap_or_else(|_| "examples/first-success/design.fig".into());
    let node = std::env::var("FIGLENS_PROBE_NODE").unwrap_or_else(|_| "0:1".into());
    let file = Path::new(&file);
    let mut rows = Vec::new();
    let mut document = stage("openInflateSchema", &mut rows, || {
        crate::Document::open(file).unwrap()
    });
    let message = stage("decodePreflight", &mut rows, || {
        document.take_message().unwrap()
    });
    let tree = stage("tree", &mut rows, || Tree::new(&message));
    let rest = stage("restExpansion", &mut rows, || {
        crate::rest::resolve_render_nodes(&tree, &node).unwrap()
    });
    let images = stage("imageRead", &mut rows, || {
        render::images(file, &rest["document"]).unwrap()
    });
    let prepared = Prepared {
        document: rest,
        images,
    };
    let svg = stage("svg", &mut rows, || {
        prepared.export_svg(&tree, 1.0).unwrap()
    });
    let png = stage("rasterPng", &mut rows, || render::png(&svg).unwrap());
    assert!(!png.is_empty());
    drop((prepared, svg, png));
    let dir = crate::test_support::TestDir::new();
    let bundle = dir.join("bundle.zip");
    stage("completeBundleInclusive", &mut rows, || {
        write(&tree, file, None, None, None, &node, &bundle).unwrap()
    });
    // Repack exactly the entries emitted by the complete bundle, excluding its manifest.
    let mut zip = crate::open_zip(&bundle).unwrap();
    let manifest: Json = serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap();
    let mut files = Vec::new();
    for entry in manifest["files"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        let mut bytes = Vec::new();
        zip.by_name(path).unwrap().read_to_end(&mut bytes).unwrap();
        assert_eq!(sha256(&bytes), entry["sha256"]);
        // Metadata does not affect compressed asset cost; retain roles from known bundle outputs.
        let role = match entry["role"].as_str().unwrap() {
            "asset" => "asset",
            "reference" => "reference",
            "structure" => "structure",
            "variables" => "variables",
            "prompt" => "prompt",
            _ => "metadata",
        };
        files.push(File_ {
            path: path.into(),
            bytes,
            role,
            media: "application/octet-stream",
            node: None,
        });
    }
    stage("packExistingEntries", &mut rows, || {
        pack(
            files,
            None,
            None,
            &node,
            "probe",
            &[],
            file,
            &dir.join("repacked.zip"),
        )
        .unwrap()
    });
    std::fs::write(output, serde_json::to_vec_pretty(&json!({
        "file": file, "node": node, "stages": rows,
        "boundaries": "Sequential stages retain predecessors; high-water is cumulative. completeBundleInclusive repeats preparation/rendering. packExistingEntries isolates zip/hash/manifest work using the real entry bytes, with probe metadata. No subtraction estimates."
    })).unwrap()).unwrap();
}
