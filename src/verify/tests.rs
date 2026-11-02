use super::*;
use crate::test_support::TestDir;
use std::io::Write as _;

fn fixture_bundle(dir: &TestDir) -> PathBuf {
    let fig = Path::new("examples/first-success/design.fig");
    let mut doc = crate::Document::open(fig).unwrap();
    let message = doc.take_message().unwrap();
    let out = dir.join("bundle.zip");
    crate::bundle::write(
        &crate::Tree::new(&message),
        fig,
        None,
        None,
        None,
        "0:1",
        &out,
    )
    .unwrap();
    out
}

#[test]
fn a_fresh_bundle_verifies_with_its_fonts_unverified() {
    let dir = TestDir::new();
    let report = verify(&fixture_bundle(&dir)).unwrap();
    let fonts: Vec<_> = report["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["family"].clone(), f["status"].clone()))
        .collect();
    assert_eq!(
        (
            &report["files"]["missing"],
            &report["files"]["changed"],
            fonts
        ),
        (
            &json!([]),
            &json!([]),
            vec![(json!("Figlens Blocks"), json!("unverified"))]
        )
    );
}

#[test]
fn an_extracted_bundle_reports_changed_and_missing_files() {
    let dir = TestDir::new();
    let extracted = dir.join("extracted");
    crate::open_zip(&fixture_bundle(&dir))
        .unwrap()
        .extract(&extracted)
        .unwrap();
    std::fs::write(extracted.join("design/variables.json"), "{}").unwrap();
    std::fs::write(extracted.join("context.md"), vec![b'x'; 1 << 20]).unwrap();
    std::fs::remove_file(extracted.join("references/screen.png")).unwrap();
    let report = verify(&extracted).unwrap();
    let files = &report["files"];
    let changed: Vec<_> = files["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["path"].clone(), c["actual"].is_null()))
        .collect();
    assert_eq!(
        (&files["missing"], changed),
        (
            &json!(["references/screen.png"]),
            vec![
                (json!("context.md"), true),
                (json!("design/variables.json"), false)
            ]
        )
    );
}

#[test]
fn manifest_paths_and_links_never_reach_outside_the_bundle() {
    let dir = TestDir::new();
    let bundle = dir.join("bundle");
    std::fs::create_dir(&bundle).unwrap();
    std::fs::write(dir.join("outside"), "outside").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        std::fs::canonicalize(dir.join("outside")).unwrap(),
        bundle.join("link"),
    )
    .unwrap();
    let entry = |path: &str| json!({"path": path, "size": 7, "sha256": "unused"});
    let manifest = json!({"format": "figlens-bundle", "version": 1,
            "files": [entry("../outside"), entry("/outside"), entry("link")]});
    std::fs::write(bundle.join("manifest.json"), manifest.to_string()).unwrap();
    let report = verify(&bundle).unwrap();
    let files = &report["files"];
    assert_eq!(
        (&files["invalid"], &files["verified"]),
        (&json!(["../outside", "/outside"]), &json!(0))
    );
    #[cfg(unix)]
    assert_eq!(files["changed"][0]["actual"], Json::Null);
}

#[test]
fn missing_assets_are_reported_from_the_bundle() {
    let dir = TestDir::new();
    let entry = json!({"key": "img", "nodeId": "1:2", "error": "image not in the .fig"});
    let manifest = json!({"format": "figlens-bundle", "version": 1, "files": []});
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(dir.join("missing-assets.json"), json!([entry]).to_string()).unwrap();
    assert_eq!(
        verify(&dir.join("")).unwrap()["missingAssets"],
        json!([entry])
    );
}

#[test]
fn damaged_metadata_keeps_the_integrity_report() {
    let dir = TestDir::new();
    let extracted = dir.join("extracted");
    crate::open_zip(&fixture_bundle(&dir))
        .unwrap()
        .extract(&extracted)
        .unwrap();
    for damaged in ["design/nodes.json", "missing-assets.json"] {
        std::fs::write(extracted.join(damaged), "{").unwrap();
    }
    let report = verify(&extracted).unwrap();
    let changed: Vec<_> = report["files"]["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["path"].as_str().unwrap())
        .collect();
    let notes = report["unverified"].to_string();
    assert_eq!(
        (
            changed,
            notes.contains("font faces were not listed"),
            notes.contains("missingAssets is incomplete")
        ),
        (vec!["design/nodes.json", "missing-assets.json"], true, true)
    );
}

#[test]
fn declared_sizes_beyond_the_budget_are_reported_not_hashed() {
    let dir = TestDir::new();
    // Each half fits alone; the second no longer fits once the first is reserved.
    let half = |path| json!({"path": path, "size": HASH_BUDGET / 2, "sha256": "0"});
    let files = json!([half("a.bin"), half("b.bin")]);
    let manifest = json!({"format": "figlens-bundle", "version": 1, "files": files});
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(dir.join("a.bin"), "small").unwrap();
    std::fs::write(dir.join("b.bin"), "small").unwrap();
    let report = verify(&dir.join("")).unwrap();
    assert_eq!(
        (
            &report["files"]["changed"][0]["path"],
            &report["files"]["changed"].as_array().unwrap().len(),
            &report["unverified"][0]
        ),
        (
            &json!("a.bin"),
            &1,
            &json!(format!(
                "1 listed files were not hashed: their declared sizes exceed the {HASH_BUDGET}-byte verification budget."
            ))
        )
    );
}

#[test]
fn an_undecodable_zip_entry_is_reported_and_checking_continues() {
    let dir = TestDir::new();
    let (data, path) = (b"figlens stored entry".as_slice(), dir.join("damaged.zip"));
    let entry = |name| json!({"path": name, "size": data.len(), "sha256": digest(&mut &data[..]).unwrap().1});
    let manifest = json!({"format": "figlens-bundle", "version": 1, "files": [entry("a.txt"), entry("b.txt")]});
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("manifest.json", stored).unwrap();
    zip.write_all(manifest.to_string().as_bytes()).unwrap();
    for name in ["a.txt", "b.txt"] {
        zip.start_file(name, stored).unwrap();
        zip.write_all(data).unwrap();
    }
    let mut bytes = zip.finish().unwrap().into_inner();
    // Damage a.txt's stored bytes so its CRC check fails while the directory stays readable.
    let at = bytes.windows(data.len()).position(|w| w == data).unwrap();
    bytes[at] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    let report = verify(&path).unwrap();
    let changed = &report["files"]["changed"][0];
    assert_eq!(
        (
            &changed["path"],
            &changed["actual"],
            changed["error"].is_string(),
            &report["files"]["verified"]
        ),
        (&json!("a.txt"), &Json::Null, true, &json!(1))
    );
}

/// An extracted bundle holding only the given metadata files.
fn metadata_bundle(dir: &TestDir, files: &[(&str, Json)]) -> PathBuf {
    let manifest = json!({"format": "figlens-bundle", "version": 1, "files": [],
            "root": {"id": "1:1", "name": "Frame"}});
    std::fs::create_dir_all(dir.join("design")).unwrap();
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    for (path, value) in files {
        std::fs::write(dir.join(path), value.to_string()).unwrap();
    }
    dir.join("")
}

fn found<'a>(report: &'a Json, category: &str, status: &str) -> Vec<&'a Json> {
    items(&report["findings"])
        .filter(|f| f["category"] == category && f["status"] == status)
        .collect()
}

#[test]
fn a_missing_asset_is_a_missing_asset_finding_with_its_node() {
    let dir = TestDir::new();
    let entry = json!({"key": "img", "nodeId": "1:2", "error": "images/img is not in the .fig"});
    let report = verify(&metadata_bundle(
        &dir,
        &[("missing-assets.json", json!([entry]))],
    ))
    .unwrap();
    let f = found(&report, "asset", "missing");
    assert_eq!(
        (f.len(), &f[0]["nodeIds"], &f[0]["evidence"]),
        (1, &json!(["1:2"]), &entry)
    );
}

#[test]
fn an_unresolved_variable_keeps_its_id_and_binding_nodes_apart_from_known_tokens() {
    let dir = TestDir::new();
    let alias = |id: &str| json!({"type": "VARIABLE_ALIAS", "id": id});
    let nodes = json!({"document": {"id": "1:1", "children": [
        {"id": "1:2", "boundVariables": {"fills": [alias("VariableID:9:9"), alias("VariableID:9:9")]}},
        {"id": "1:3", "boundVariables": {"itemSpacing": alias("VariableID:1:5")}},
    ]}});
    let variables = json!({"variables": [{"id": "VariableID:1:5"}], "collections": [],
            "missingVariables": ["VariableID:9:9"]});
    let report = verify(&metadata_bundle(
        &dir,
        &[
            ("design/nodes.json", nodes),
            ("design/variables.json", variables),
        ],
    ))
    .unwrap();
    let missing = found(&report, "variable", "missing");
    let unmatched = found(&report, "variable", "unverified");
    assert_eq!(
        (
            missing.len(),
            &missing[0]["evidence"]["id"],
            &missing[0]["nodeIds"],
            &unmatched[0]["evidence"]
        ),
        (
            1,
            &json!("VariableID:9:9"),
            &json!(["1:2"]),
            &json!({"variableCount": 1})
        )
    );
}

#[test]
fn each_font_face_is_an_unverified_finding_with_its_text_nodes() {
    let dir = TestDir::new();
    let report = verify(&fixture_bundle(&dir)).unwrap();
    let f = found(&report, "font", "unverified");
    assert_eq!(
        (
            f.len(),
            &f[0]["evidence"]["family"],
            f[0]["nodeCount"].as_u64().unwrap() > 0
        ),
        (1, &json!("Figlens Blocks"), true)
    );
}

#[test]
fn render_and_semantic_limits_stay_apart_from_asset_failures() {
    let dir = TestDir::new();
    let diagnostic = json!({"nodeId": "1:2", "feature": "GLASS", "field": "effects", "state": "skipped", "reason": "not drawn"});
    let nodes = json!({"_figlens": {"handoffDiagnostics": {"items": [
                {"feature": "grid", "nodeCount": 7, "exampleNodeIds": ["1:3"], "examplesTruncated": true}]}},
            "document": {"id": "1:1", "children": [{"id": "1:2", "_figlens": {"renderDiagnostics": [diagnostic]}}]}});
    let report = verify(&metadata_bundle(&dir, &[("design/nodes.json", nodes)])).unwrap();
    let render = found(&report, "render", "unsupported");
    let semantics = found(&report, "semantics", "unsupported");
    assert_eq!(
        (
            &render[0]["evidence"],
            &render[0]["source"],
            &semantics[0]["nodeCount"],
            found(&report, "asset", "missing").len()
        ),
        (&diagnostic, &json!("design/nodes.json"), &json!(7), 0)
    );
}

#[test]
fn truncated_state_inventory_and_findings_stay_explicit() {
    let dir = TestDir::new();
    let diagnostics: Vec<_> = (0..MAX_FINDINGS + 1)
        .map(|n| json!({"nodeId": format!("1:{n}"), "state": "approximate"}))
        .collect();
    let nodes = json!({"document": {"id": "1:1", "_figlens": {"renderDiagnostics": diagnostics}}});
    let states = json!({"truncated": true, "reasons": ["variant_limit"], "coverage": {}});
    let report = verify(&metadata_bundle(
        &dir,
        &[("design/nodes.json", nodes), ("design/states.json", states)],
    ))
    .unwrap();
    let summary = &report["findingsSummary"];
    assert_eq!(
        (
            // Unverified scope sorts after the unsupported renders, beyond the cut.
            summary["counts"]["scope/unverified"].clone(),
            report["findings"].as_array().unwrap().len(),
            summary["truncated"].clone(),
            summary["counts"]["render/unsupported"].clone(),
        ),
        (json!(1), MAX_FINDINGS, json!(true), json!(MAX_FINDINGS + 1))
    );
}

#[test]
fn oversized_evidence_is_counted_but_not_returned() {
    let dir = TestDir::new();
    let huge = json!({"nodeId": "1:2", "reason": "x".repeat(FINDINGS_BYTES)});
    let nodes = json!({"document": {"id": "1:1", "_figlens": {"renderDiagnostics": [huge]}}});
    let report = verify(&metadata_bundle(&dir, &[("design/nodes.json", nodes)])).unwrap();
    let summary = &report["findingsSummary"];
    assert_eq!(
        (
            &report["findings"],
            &summary["total"],
            &summary["truncated"]
        ),
        (&json!([]), &json!(1), &json!(true))
    );
}

#[test]
fn oversized_bundle_lists_keep_their_totals() {
    let dir = TestDir::new();
    let huge = json!({"key": "x".repeat(LIST_BYTES), "nodeId": "1:2"});
    let path = metadata_bundle(&dir, &[("missing-assets.json", json!([huge]))]);
    let manifest = json!({"format": "figlens-bundle", "version": 1, "files": [],
            "root": {"id": "1:1", "name": "x".repeat(2048)}});
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    let report = verify(&path).unwrap();
    assert_eq!(
        (
            &report["missingAssets"],
            &report["truncated"],
            &report["findingsSummary"]["scope"]["name"]
        ),
        (&json!([]), &json!({"missingAssets": 1}), &Json::Null)
    );
}

#[test]
fn malformed_handoff_items_and_invalid_paths_do_not_displace_missing_inputs() {
    let dir = TestDir::new();
    let nodes =
        json!({"_figlens": {"handoffDiagnostics": {"items": [false]}}, "document": {"id": "1:1"}});
    let entry = json!({"key": "img", "nodeId": "1:2", "error": "missing"});
    let path = metadata_bundle(
        &dir,
        &[
            ("design/nodes.json", nodes),
            ("missing-assets.json", json!([entry])),
        ],
    );
    let manifest = json!({"format": "figlens-bundle", "version": 1, "files": [{"path": "../x"}],
            "root": {"id": "1:1", "name": "Frame"}});
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    let report = verify(&path).unwrap();
    let order: Vec<_> = items(&report["findings"])
        .map(|f| {
            (
                f["category"].as_str().unwrap(),
                f["status"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            ("asset", "missing"),
            ("semantics", "unsupported"),
            ("integrity", "unverified")
        ]
    );
}

#[test]
fn semantic_coverage_is_reported_once_and_only_when_short() {
    let dir = TestDir::new();
    let report = |coverage: Json| {
        let nodes = json!({"_figlens": {"handoffDiagnostics": {"coverage": coverage,
                "items": [{"feature": "grid", "nodeCount": 1}, {"feature": "slots", "nodeCount": 1}]}},
                "document": {"id": "1:1"}});
        verify(&metadata_bundle(&dir, &[("design/nodes.json", nodes)])).unwrap()
    };
    let short = report(json!("known stored fields"));
    let long = report(json!("x".repeat(2048)));
    assert_eq!(
        (
            &short["findingsSummary"]["semanticsCoverage"],
            found(&short, "semantics", "unsupported")[0]["evidence"].get("coverage"),
            &long["findingsSummary"]["semanticsCoverage"]
        ),
        (&json!("known stored fields"), None, &Json::Null)
    );
}

#[test]
fn variable_bindings_count_every_finding_but_keep_bounded_node_ids() {
    let dir = TestDir::new();
    let ids: Vec<_> = (0..MAX_FINDINGS + 100)
        .map(|n| format!("VariableID:9:{n}"))
        .collect();
    let aliases: Vec<_> = ids
        .iter()
        .map(|id| json!({"type": "VARIABLE_ALIAS", "id": id}))
        .collect();
    let nodes = json!({"document": {"id": "1:1", "children": [
            {"id": "x".repeat(2048), "boundVariables": {"fills": aliases}}]}});
    let variables = json!({"variables": [], "missingVariables": ids});
    let report = verify(&metadata_bundle(
        &dir,
        &[
            ("design/nodes.json", nodes),
            ("design/variables.json", variables),
        ],
    ))
    .unwrap();
    let first = found(&report, "variable", "missing")[0];
    assert_eq!(
        (
            &report["findingsSummary"]["counts"]["variable/missing"],
            report["findings"].as_array().unwrap().len(),
            &first["nodeIds"],
            &first["nodeCount"]
        ),
        (
            &json!(MAX_FINDINGS + 100),
            MAX_FINDINGS,
            &json!([]),
            &json!(1)
        )
    );
}

#[test]
fn a_directory_without_a_bundle_manifest_is_an_error() {
    let dir = TestDir::new();
    assert!(verify(&dir.join("")).is_err());
}

#[test]
fn a_font_in_the_project_folder_is_a_local_file_with_a_font_face() {
    let dir = TestDir::new();
    let bundle = fixture_bundle(&dir);
    let face = verify(&bundle).unwrap()["fonts"][0].clone();
    let (family, style) = (
        face["family"].as_str().unwrap(),
        face["style"].as_str().unwrap(),
    );
    let fonts = dir.join("fonts");
    std::fs::create_dir_all(&fonts).unwrap();
    std::fs::write(
        fonts.join("Brand.otf"),
        crate::fonts::tests::sfnt("Other-PS", family, style),
    )
    .unwrap();
    let report = verify_with_fonts(&bundle, Some(&fonts)).unwrap();
    let face = &report["fonts"][0];
    let still_a_finding = found(&report, "font", "unverified").len();
    assert_eq!(
        (
            &face["status"],
            &face["localFiles"][0]["matchedBy"],
            &face["localFiles"][0]["inProject"],
            face["fontFace"]
                .as_str()
                .unwrap()
                .contains("src: url(\"Brand.otf\") format(\"opentype\")"),
            still_a_finding,
        ),
        (
            &json!("local_file"),
            &json!("family+style"),
            &json!(true),
            true,
            1
        )
    );
}

#[test]
fn a_missing_project_font_folder_is_an_error() {
    let dir = TestDir::new();
    let bundle = fixture_bundle(&dir);
    assert!(verify_with_fonts(&bundle, Some(&dir.join("nope"))).is_err());
}
