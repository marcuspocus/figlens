use super::*;
use crate::test_support::TestDir;
use std::io::Read;

/// The public fixture's canvas, wrapped as Figma wraps a copied selection in the clipboard HTML.
fn copied(dir: &TestDir) -> std::path::PathBuf {
    let mut zip = crate::open_zip(Path::new("examples/first-success/design.fig")).unwrap();
    let mut canvas = Vec::new();
    zip.by_name("canvas.fig")
        .unwrap()
        .read_to_end(&mut canvas)
        .unwrap();
    let html = format!(
        "<meta charset='utf-8'><span data-metadata=\"<!--(figmeta)e30=(/figmeta)-->\"></span><span data-buffer=\"<!--(figma){}(/figma)-->\"></span>",
        crate::render::base64(&canvas)
    );
    let path = dir.join("copied.html");
    std::fs::write(&path, html).unwrap();
    path
}

#[test]
fn a_saved_copy_becomes_a_fig_the_commands_read() {
    let dir = TestDir::new();
    let out = dir.join("copied.fig");
    let report = convert(Source::File(&copied(&dir)), &out).unwrap();
    let mut doc = Document::open(&out).unwrap();
    let message = doc.take_message().unwrap();
    assert!(Tree::new(&message).nodes.contains_key("0:6"));
    assert_eq!(report["layers"], json!(["0:1"]));
}

#[test]
fn the_report_and_export_name_images_the_copy_lacks() {
    let dir = TestDir::new();
    let out = dir.join("copied.fig");
    let report = convert(Source::File(&copied(&dir)), &out).unwrap();
    assert_eq!(report["missingImages"], json!(["first-success-tile"]));
    let mut doc = Document::open(&out).unwrap();
    let message = doc.take_message().unwrap();
    let tree = Tree::new(&message);
    let (_, export) = crate::export::export(
        &tree,
        &out,
        "0:1",
        crate::export::Format::Png,
        false,
        1.0,
        80,
        None,
        None,
        &[],
    )
    .unwrap();
    assert!(export["warnings"].as_array().unwrap().contains(&json!(
        "Image first-success-tile at node 0:3 is not in the .fig; it is drawn empty"
    )));
}

#[test]
fn html_without_figma_data_is_refused_before_writing() {
    let dir = TestDir::new();
    let input = dir.join("text.html");
    std::fs::write(&input, "<span>Cloth</span>").unwrap();
    let out = dir.join("copied.fig");
    let error = convert(Source::File(&input), &out).unwrap_err().to_string();
    assert!(error.starts_with("no Figma selection"), "{error}");
    assert!(!out.exists());
}

#[test]
fn an_existing_output_is_never_replaced() {
    let dir = TestDir::new();
    let out = dir.join("design.fig");
    std::fs::write(&out, "a full design").unwrap();
    let error = convert(Source::File(&copied(&dir)), &out)
        .unwrap_err()
        .to_string();
    assert!(
        error.ends_with("exists; choose a new path for the copied selection"),
        "{error}"
    );
    assert_eq!(std::fs::read(&out).unwrap(), b"a full design");
}

#[test]
fn applescript_hex_output_decodes_to_the_html() {
    let decode = |text: &str| applescript_data(text.as_bytes().to_vec());
    assert_eq!(decode("«data HTML3C623E»\n"), Some(b"<b>".to_vec()));
    assert_eq!(decode("«data HTML3C6»"), None);
    assert_eq!(decode("«data HTML+A»"), None);
    assert_eq!(decode("plain text"), None);
}

#[test]
fn a_missing_clipboard_tool_is_named() {
    let error = run(&[("figlens-no-such-reader", &[])], 16)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "cannot read the clipboard: install figlens-no-such-reader"
    );
}

#[cfg(unix)]
#[test]
fn a_reader_that_fails_means_no_layers_and_the_next_reader_is_tried() {
    assert_eq!(
        run(&[("false", &[]), ("echo", &["html"])], 16).unwrap(),
        b"html\n".to_vec()
    );
    assert_eq!(
        run(&[("false", &[])], 16).unwrap_err().to_string(),
        NO_LAYERS
    );
}

#[cfg(unix)]
#[test]
fn a_failed_reader_beside_a_missing_one_names_the_missing_tool() {
    let error = run(&[("false", &[]), ("figlens-no-such-reader", &[])], 16)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with(NO_LAYERS), "{error}");
    assert!(
        error.ends_with("if layers are copied, install figlens-no-such-reader"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn reader_output_past_the_limit_is_refused() {
    assert!(run(&[("echo", &["0123456789"])], 4).is_err());
}

#[test]
fn windows_cf_html_keeps_its_header_before_the_markers() {
    let html = format!(
        "Version:0.9\r\nStartHTML:00000097\r\nEndHTML:00000200\r\n<html><!--(figma){}(/figma)--></html>",
        crate::render::base64(b"kiwi")
    );
    assert_eq!(
        canvas(html.as_bytes(), NO_LAYERS).unwrap(),
        b"kiwi".to_vec()
    );
}

#[test]
fn clipboard_html_without_figma_layers_says_to_copy_them() {
    let error = canvas(b"<span>text</span>", NO_LAYERS)
        .unwrap_err()
        .to_string();
    assert_eq!(error, NO_LAYERS);
}

#[test]
fn a_clipboard_read_has_no_source_file_to_protect() {
    let dir = TestDir::new();
    let out = dir.join("copied.fig");
    crate::output::write(Path::new(""), &out, |file| Ok(file.write_all(b"x")?)).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"x");
}

#[test]
fn base64_reverses_the_renderer_encoding() {
    for len in 0..8 {
        let bytes: Vec<u8> = (0..len)
            .map(|i: u8| i.wrapping_mul(37).wrapping_add(200))
            .collect();
        assert_eq!(
            base64(crate::render::base64(&bytes).as_bytes()),
            Some(bytes)
        );
    }
    assert_eq!(base64(b"QQ==QQ"), None);
    assert_eq!(base64(b"Q"), None);
}

#[test]
fn the_report_names_missing_components_and_outlines_in_page_order() {
    use kiwi_schema::Value as K;
    use std::collections::HashMap;
    let guid = |l: u32| {
        K::Object(
            "GUID",
            HashMap::from([("sessionID", K::UInt(1)), ("localID", K::UInt(l))]),
        )
    };
    let node = |l: u32,
                kind: &'static str,
                parent: Option<(u32, &str)>,
                extra: Vec<(&'static str, K<'static>)>| {
        let mut fields = HashMap::from([("guid", guid(l)), ("type", K::Enum("NodeType", kind))]);
        if let Some((p, position)) = parent {
            fields.insert(
                "parentIndex",
                K::Object(
                    "ParentIndex",
                    HashMap::from([("guid", guid(p)), ("position", K::String(position.into()))]),
                ),
            );
        }
        fields.extend(extra);
        K::Object("NodeChange", fields)
    };
    let symbol = K::Object("SymbolData", HashMap::from([("symbolID", guid(99))]));
    let message = K::Object(
        "Message",
        HashMap::from([
            ("pasteFileKey", K::String("KEY".into())),
            (
                "nodeChanges",
                K::Array(vec![
                    node(1, "DOCUMENT", None, vec![]),
                    node(2, "CANVAS", Some((1, "a")), vec![]),
                    node(10, "FRAME", Some((2, "b")), vec![]),
                    node(3, "INSTANCE", Some((2, "a")), vec![("symbolData", symbol)]),
                    node(
                        4,
                        "VECTOR",
                        Some((10, "a")),
                        vec![("fillGeometry", K::Array(vec![]))],
                    ),
                    node(
                        5,
                        "CANVAS",
                        Some((1, "b")),
                        vec![("internalOnly", K::Bool(true))],
                    ),
                    node(6, "COMPONENT", Some((5, "a")), vec![]),
                ]),
            ),
        ]),
    );
    let report = report(&message, &Tree::new(&message));
    assert_eq!(report["sourceFileKey"], "KEY");
    assert_eq!(
        report["layers"],
        json!(["1:3", "1:10"]),
        "page order, internal page skipped"
    );
    assert_eq!(
        report["missingComponents"],
        json!([{"instanceId": "1:3", "componentId": "1:99"}])
    );
    assert_eq!(
        report["layersWithoutOutlines"], 1,
        "a missing stroke outline counts"
    );
    assert_eq!(report["warnings"].as_array().unwrap().len(), 2);
}
