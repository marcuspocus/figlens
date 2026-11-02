use super::*;
use std::fs::File;

#[test]
fn optimized_png_preserves_rgba_and_never_grows() {
    fn decoded(bytes: &[u8]) -> (u32, u32, Vec<[u8; 4]>) {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::ALPHA);
        let mut reader = decoder.read_info().unwrap();
        let mut data = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut data).unwrap();
        let channels = info.color_type.samples();
        let pixels = data[..info.buffer_size()]
            .chunks_exact(channels)
            .map(|p| match info.color_type {
                png::ColorType::Rgba => [p[0], p[1], p[2], p[3]],
                png::ColorType::Rgb => [p[0], p[1], p[2], 255],
                png::ColorType::GrayscaleAlpha => [p[0], p[0], p[0], p[1]],
                png::ColorType::Grayscale => [p[0], p[0], p[0], 255],
                png::ColorType::Indexed => unreachable!("palette expanded by decoder"),
            })
            .collect();
        (info.width, info.height, pixels)
    }
    for kind in 0..4 {
        let mut p = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
        let mut seed = 1u32;
        for (i, px) in p.pixels_mut().iter_mut().enumerate() {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let [r, g, b, a] = match kind {
                0 => [30, 120, 180, 255],
                1 => [i as u8, i as u8, i as u8, 255],
                2 => [80, 140, 210, i as u8],
                _ => seed.to_le_bytes(),
            };
            *px = resvg::tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
        }
        let original = p.encode_png().unwrap();
        let optimized = encode_png(&p).unwrap();
        assert_eq!(decoded(&optimized), decoded(&original));
        assert!(optimized.len() <= original.len());
        if optimized.len() == original.len() {
            assert_eq!(optimized, original);
        }
        if kind == 0 {
            assert!(optimized.len() < original.len());
        }
    }
}

#[test]
fn png_optimization_stops_above_its_pixel_budget() {
    for width in [2048, 2049] {
        let mut p = resvg::tiny_skia::Pixmap::new(width, 2048).unwrap();
        p.fill(resvg::tiny_skia::Color::from_rgba8(30, 120, 180, 255));
        let original = p.encode_png().unwrap();
        let encoded = encode_png(&p).unwrap();
        if width == 2048 {
            assert!(encoded.len() < original.len());
        } else {
            assert_eq!(encoded, original);
        }
    }
}

#[test]
fn exported_definition_ids_are_disjoint_and_repeatable() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let images = |_: &str| None;
    let export = |id: &str| {
        let node = serde_json::json!({
            "id": id, "type": "FRAME", "clipsContent": true,
            "_r": { "size": { "x": 20, "y": 20 }, "m": [1,0,0,1,0,0],
                "fillPaints": [{ "type": "GRADIENT_LINEAR", "stops": [
                    { "position": 0, "color": { "r": 1, "g": 0, "b": 0, "a": 1 } },
                    { "position": 1, "color": { "r": 0, "g": 0, "b": 1, "a": 1 } }
                ] }]
            },
            "children": [{ "id": "1:2", "type": "RECTANGLE", "_r": {
                "size": { "x": 10, "y": 10 }, "m": [1,0,0,1,0,0],
                "fillPaints": [{ "type": "SOLID", "color": { "r": 1, "a": 1 } }]
            }}]
        });
        Svg::new(&tree, &images).document(&node, [0.0, 0.0, 20.0, 20.0], 1.0)
    };
    let mut seen = std::collections::HashSet::new();
    for node_id in [
        "217:23093",
        "217:23094",
        "I217:23093;1:2",
        "217-23093",
        "217_3a23093",
        "盾牌",
    ] {
        let svg = export(node_id);
        assert_eq!(svg, export(node_id));
        assert!(!svg.contains("data-node-id"));
        for group in svg.split("<g ").skip(1) {
            assert!(!group.split('>').next().unwrap().contains("id=\""));
        }
        let ids: Vec<_> = svg
            .split(" id=\"")
            .skip(1)
            .map(|part| part.split('"').next().unwrap())
            .collect();
        assert!(svg.contains("<linearGradient "));
        assert!(svg.contains("<clipPath "));
        for id in &ids {
            assert!(id.starts_with('n'));
            assert!(
                id.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            );
            assert!(seen.insert((*id).to_owned()), "duplicate definition: {id}");
            assert!(svg.contains(&format!("url(#{id})")));
        }
        for reference in svg.split("url(#").skip(1) {
            assert!(ids.contains(&reference.split(')').next().unwrap()));
        }
        let pixels = pixmap(&svg).unwrap();
        assert!(pixels.data().chunks_exact(4).any(|p| p[3] != 0));
    }
}

#[test]
fn diagnostics_preserve_drawn_stroke_warnings_and_other_sources() {
    use kiwi_schema::Value;
    let mut bytes = Vec::new();
    for (command, x, y) in [(1u8, 0f32, 0f32), (2, 16., 0.), (2, 16., 4.), (2, 0., 4.)] {
        bytes.push(command);
        bytes.extend(x.to_le_bytes());
        bytes.extend(y.to_le_bytes());
    }
    bytes.push(0);
    let blob = Value::Object(
        "Blob",
        std::collections::HashMap::from([("bytes", Value::Bytes(bytes))]),
    );
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: vec![&blob],
    };
    for geometry in [false, true] {
        // The third case: per-side weights with a zero strokeWeight, which count as stroked.
        for (weight, sides) in [(0, false), (2, false), (0, true)] {
            let mut node = serde_json::json!({"id":"1:1","type":"RECTANGLE","_r":{
                "fillPaints":[{"type":"GRADIENT_ANGULAR"}],
                "strokeWeight":weight,
                "strokePaints":[
                    {"type":"SOLID"},
                    {"type":"IMAGE","imageScaleMode":"FILL","paintFilter":{"exposure":0.2}},
                    {"type":"GRADIENT_LINEAR"},
                    {"type":"GRADIENT_ANGULAR","visible":false},
                    {"type":"GRADIENT_DIAMOND","opacity":0},
                    {"type":"GRADIENT_ANGULAR"},
                    {"type":"IMAGE","imageScaleMode":"TILE","paintFilter":{"contrast":0.2}}
                ],
                "effects":[{"type":"BACKGROUND_BLUR","radius":4}]
            }});
            if geometry {
                node["_r"]["strokeGeometry"] = serde_json::json!([{"commandsBlob":0}]);
            }
            if sides {
                node["individualStrokeWeights"] =
                    serde_json::json!({"top":0,"right":0,"bottom":2,"left":0});
            }
            let found = diagnostics(&tree, &node);
            let fields: Vec<_> = found.iter().map(|d| d["field"].as_str().unwrap()).collect();
            let mut expected = vec!["fillPaints[0].type"];
            if geometry && (weight > 0 || sides) {
                expected.extend([
                    "strokePaints[5].type",
                    "strokePaints[6].imageScaleMode",
                    "strokePaints[6].paintFilter.contrast",
                ]);
            }
            expected.push("effects[0]");
            assert_eq!(fields, expected);
        }
    }
}

#[test]
fn diagnostics_report_only_active_paint_fallbacks() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let mut node = serde_json::json!({"id":"1:1", "type":"RECTANGLE", "_r": {
        "fillPaints": [
            {"type":"IMAGE", "imageScaleMode":"TILE", "paintFilter":{"contrast":0.2,"exposure":0.2,"vibrance":0.2,"tint":0}},
            {"type":"GRADIENT_ANGULAR"},
            {"type":"GRADIENT_DIAMOND", "visible":false},
            {"type":"IMAGE", "opacity":0, "paintFilter":{"temperature":0.3}}
        ], "effects":[{"type":"BACKGROUND_BLUR","radius":4}, {"type":"BACKGROUND_BLUR","radius":0}]
    }});
    let found = diagnostics(&tree, &node);
    assert_eq!(found.len(), 4);
    assert_eq!(found[0]["state"], "approximate");
    assert_eq!(found[1]["field"], "fillPaints[0].paintFilter.contrast");
    assert_eq!(found[3]["feature"], "backgroundBlur");
    node["_r"]["fillGeometry"] = serde_json::json!([{"styleID":1}]);
    node["_r"]["vectorStyles"] = serde_json::json!([
        {"styleID":1,"fillPaints":[{"type":"SOLID"}]},
        {"styleID":2,"fillPaints":[{"type":"GRADIENT_DIAMOND"}]}
    ]);
    node["_r"]["effects"] = serde_json::json!([]);
    assert!(
        diagnostics(&tree, &node).is_empty(),
        "overridden and unused fills are not rendered"
    );
    node["type"] = serde_json::json!("TEXT");
    node["_r"]["characterStyleIDs"] = serde_json::json!([1, 1]);
    node["_r"]["glyphs"] = serde_json::json!([{"firstCharacter":0}, {"firstCharacter":1}]);
    node["_r"]["styleOverrideTable"] = node["_r"]["vectorStyles"].clone();
    assert!(diagnostics(&tree, &node).is_empty());
    node["_r"]["styleOverrideTable"][0]["fillPaints"][0]["type"] =
        serde_json::json!("GRADIENT_DIAMOND");
    assert_eq!(
        diagnostics(&tree, &node).len(),
        1,
        "one report per effective paint, not per glyph"
    );
    node["_r"]["truncationStartIndex"] = serde_json::json!(0);
    assert!(
        diagnostics(&tree, &node).is_empty(),
        "truncated glyph styles are unused"
    );
}

#[test]
fn preparation_selects_an_expanded_sublayer_and_only_loads_its_images() {
    use kiwi_schema::Value;
    use std::io::Write;

    let obj = |fields: Vec<(&'static str, Value<'static>)>| {
        Value::Object("Test", fields.into_iter().collect())
    };
    let paint = |hash: &'static str| {
        obj(vec![
            ("type", Value::Enum("PaintType", "IMAGE")),
            ("image", obj(vec![("hash", Value::String(hash.into()))])),
        ])
    };
    let main = obj(vec![("type", Value::Enum("NodeType", "FRAME"))]);
    let instance = obj(vec![
        ("type", Value::Enum("NodeType", "INSTANCE")),
        (
            "symbolData",
            obj(vec![(
                "symbolID",
                obj(vec![
                    ("sessionID", Value::UInt(2)),
                    ("localID", Value::UInt(1)),
                ]),
            )]),
        ),
    ]);
    let child = obj(vec![
        ("type", Value::Enum("NodeType", "RECTANGLE")),
        (
            "size",
            obj(vec![("x", Value::Float(10.0)), ("y", Value::Float(20.0))]),
        ),
        (
            "fillPaints",
            Value::Array(vec![paint("present"), paint("missing"), paint("present")]),
        ),
    ]);
    let sibling = obj(vec![
        ("type", Value::Enum("NodeType", "RECTANGLE")),
        ("fillPaints", Value::Array(vec![paint("unused")])),
    ]);
    let tree = Tree {
        nodes: HashMap::from([
            ("1:1".into(), &instance),
            ("2:1".into(), &main),
            ("2:2".into(), &child),
            ("2:3".into(), &sibling),
        ]),
        children: HashMap::from([("2:1".into(), vec!["2:2".into(), "2:3".into()])]),
        blobs: Vec::new(),
    };
    let dir = crate::test_support::TestDir::new();
    let path = dir.join("input.fig");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    for hash in ["present", "unused"] {
        zip.start_file(
            format!("images/{hash}"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(hash.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    let snapshot = std::fs::read(&path).unwrap();
    let staged = Prepared::from_snapshot(&tree, &snapshot, "I1:1;2:2").unwrap();
    let prepared = Prepared::new(&tree, &path, "I1:1;2:2").unwrap();
    assert_eq!(prepared.node()["id"], "I1:1;2:2");
    assert!(prepared.node()["_r"].is_object());
    assert_eq!(
        prepared.images,
        HashMap::from([("present".into(), b"present".to_vec())])
    );
    assert_eq!(staged.document, prepared.document);
    assert_eq!(staged.images, prepared.images);
    // A later source replacement must not change a generation's staged image paints.
    std::fs::write(&path, b"replaced source").unwrap();
    let staged_again = Prepared::from_snapshot(&tree, &snapshot, "I1:1;2:2").unwrap();
    assert_eq!(staged_again.images, prepared.images);
    let mut prefixed = b"saved local copy prefix".to_vec();
    prefixed.extend_from_slice(&snapshot);
    assert_eq!(
        Prepared::from_snapshot(&tree, &prefixed, "I1:1;2:2")
            .unwrap()
            .images,
        prepared.images
    );
    // Restore this owned fixture for the file-path error assertion below.
    std::fs::write(&path, &snapshot).unwrap();
    let pixels = pixmap(&prepared.export_svg(&tree, 2.0).unwrap()).unwrap();
    assert_eq!((pixels.width(), pixels.height()), (20, 40));
    assert_eq!(
        Prepared::new(&tree, &path, "I1:1;absent")
            .err()
            .unwrap()
            .to_string(),
        "node I1:1;absent not found in expanded instance 1:1; inspect 1:1 for available sublayer IDs"
    );
}

#[test]
fn prepared_page_export_draws_backdrop_across_the_child_union() {
    use kiwi_schema::Value;
    let page = Value::Object(
        "Test",
        vec![
            ("backgroundEnabled", Value::Bool(true)),
            (
                "backgroundColor",
                Value::Object(
                    "Color",
                    vec![
                        ("r", Value::Float(1.0)),
                        ("g", Value::Float(0.0)),
                        ("b", Value::Float(0.0)),
                        ("a", Value::Float(1.0)),
                    ]
                    .into_iter()
                    .collect(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );
    let tree = Tree {
        nodes: HashMap::from([("page".into(), &page)]),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let prepared = Prepared {
        document: serde_json::json!({ "document": {
            "id": "page", "type": "CANVAS", "children": [{
                "absoluteRenderBounds": { "x": 5, "y": 7, "width": 10, "height": 20 }
            }]
        }}),
        images: Default::default(),
    };
    let pixels = pixmap(&prepared.export_svg(&tree, 1.0).unwrap()).unwrap();
    assert_eq!((pixels.width(), pixels.height()), (10, 20));
    assert!(pixels.data().chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
}

#[test]
fn background_blur_draws_the_export_behind_the_node_blurred_and_cut_to_its_outline() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let box_ = |id: &str, x: f64, w: f64, effects: Json| {
        serde_json::json!({"id": id, "type": "RECTANGLE", "_r": {
            "m": [1, 0, 0, 1, x, 0], "size": {"x": w, "y": 10},
            "fillPaints": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 0.5}}],
            "effects": effects,
        }})
    };
    let blur = serde_json::json!([{"type": "BACKGROUND_BLUR", "radius": 4}]);
    let mut frame = serde_json::json!({"id": "1:1", "type": "FRAME", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 40, "y": 10}, "fillPaints": [],
        }, "children": [box_("1:2", 0.0, 40.0, Json::Null), box_("1:3", 10.0, 20.0, blur.clone()), box_("1:4", 20.0, 20.0, blur)]});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 40.0, 10.0], 1.0);
    // 1:3's backdrop, blurred with sigma = radius / 2, is cut to 1:3's outline.
    assert!(svg.contains("\"><path d=\"M10 0L30 0"));
    assert!(svg.contains("<feGaussianBlur stdDeviation=\"2\"/>"));
    assert_eq!(svg.matches("<feGaussianBlur").count(), 2);
    assert!(png(&svg).is_ok());
    frame["children"][1]["_r"]["effects"] = Json::Null;
    frame["children"][2]["_r"]["effects"] = Json::Null;
    let plain = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 40.0, 10.0], 1.0);
    assert!(
        !plain.contains("<use"),
        "exports without background blur are unchanged"
    );
}

#[test]
fn backdrops_leave_out_earlier_blurs_and_stop_at_the_blur_cap() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let chip = |i: usize| {
        serde_json::json!({"id": format!("1:{}", i + 2), "type": "RECTANGLE", "_r": {
            "m": [1, 0, 0, 1, i, 0], "size": {"x": 20, "y": 10},
            "fillPaints": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 0.5}}],
            "effects": [{"type": "BACKGROUND_BLUR", "radius": 4}],
        }})
    };
    let frame = serde_json::json!({"id": "1:1", "type": "FRAME", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 60, "y": 10}, "fillPaints": [],
        }, "children": (0..70).map(chip).collect::<Vec<_>>()});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 60.0, 10.0], 1.0);
    // Blurring each earlier blur again doubles the markup per sibling and exhausts the budget
    // well before the cap.
    assert_eq!(svg.matches("<feGaussianBlur").count(), MAX_BLURS);
}

#[test]
fn background_blurs_the_renderer_leaves_out_are_reported_as_skipped() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let mut node = serde_json::json!({"id": "1:1", "type": "RECTANGLE", "_r": {"effects": [
        {"type": "BACKGROUND_BLUR", "radius": 4}, {"type": "BACKGROUND_BLUR", "radius": 8},
    ]}});
    let states = |node: &Json| -> Vec<Json> {
        diagnostics(&tree, node)
            .iter()
            .map(|d| d["state"].clone())
            .collect()
    };
    assert_eq!(states(&node), ["approximate", "skipped"]);
    node["isMask"] = Json::Bool(true);
    assert_eq!(states(&node), ["skipped", "skipped"]);
}

#[test]
fn background_blur_replaces_a_translucent_backdrop_instead_of_covering_it() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let rect = |id: &str, g: f64, effects: Json| {
        serde_json::json!({"id": id, "type": "RECTANGLE", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 40, "y": 40},
            "fillPaints": [{"type": "SOLID", "color": {"r": 1, "g": g, "b": g, "a": 0.5}}],
            "effects": effects,
        }})
    };
    let blur = serde_json::json!([{"type": "BACKGROUND_BLUR", "radius": 4}]);
    let frame = serde_json::json!({"id": "1:1", "type": "FRAME", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 40, "y": 40}, "fillPaints": [],
        }, "children": [rect("1:2", 0.0, Json::Null), rect("1:3", 1.0, blur)]});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 40.0, 40.0], 1.0);
    // Half-opaque white over the blurred half-opaque red: 0.75, not 0.875 from red twice.
    let alpha = pixmap(&svg).unwrap().pixel(20, 20).unwrap().alpha();
    assert!((190..=192).contains(&alpha), "{alpha}");
}

#[test]
fn a_nested_background_blur_replaces_its_parents_blur_too() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let glass = |id: &str, kind: &str, children: Json| {
        serde_json::json!({"id": id, "type": kind, "_r": {
                "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10},
                "effects": [{"type": "BACKGROUND_BLUR", "radius": 4}],
            }, "children": children})
    };
    let inner = glass("1:3", "RECTANGLE", Json::Null);
    let outer = glass("1:2", "FRAME", serde_json::json!([inner]));
    let frame = serde_json::json!({"id": "1:1", "type": "FRAME", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10},
            "fillPaints": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 0.5}}],
        }, "children": [outer]});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 10.0, 10.0], 1.0);
    // Painting the inner blur over the outer one would give 0.75 alpha.
    let alpha = pixmap(&svg).unwrap().pixel(5, 5).unwrap().alpha();
    assert!((126..=130).contains(&alpha), "{alpha}");
}

#[test]
fn per_side_strokes_draw_with_a_zero_stroke_weight_from_geometry_or_the_box() {
    use kiwi_schema::Value;
    // A 2px outside bottom stroke on a 16x10 box: Figma saves a 4px band centered on the edge.
    let mut bytes = Vec::new();
    for (command, x, y) in [(1u8, 0f32, 8f32), (2, 16., 8.), (2, 16., 12.), (2, 0., 12.)] {
        bytes.push(command);
        bytes.extend(x.to_le_bytes());
        bytes.extend(y.to_le_bytes());
    }
    bytes.push(0);
    let blob = Value::Object(
        "Blob",
        std::collections::HashMap::from([("bytes", Value::Bytes(bytes))]),
    );
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: vec![&blob],
    };
    let images = |_: &str| None;
    let node = |align: &str, height: f64, bottom: f64| {
        serde_json::json!({"id": "1:1", "type": "FRAME",
            "individualStrokeWeights": {"top": 0, "right": 0, "bottom": bottom, "left": 0},
            "_r": {
                "m": [1, 0, 0, 1, 0, 0], "size": {"x": 16, "y": height},
                "strokeWeight": 0, "strokeAlign": align,
                "strokePaints": [{"type": "SOLID", "color": {"r": 0, "g": 0, "b": 1, "a": 1}}],
        }})
    };
    // Alpha down the middle column, one value per row.
    let column = |n: &Json, rows: i32| {
        let svg = Svg::new(&tree, &images).document(n, [0.0, 0.0, 16.0, rows as f64], 1.0);
        let pixels = pixmap(&svg).unwrap();
        (0..rows)
            .map(|y| pixels.pixel(8, y as u32).unwrap().alpha())
            .collect::<Vec<_>>()
    };
    for geometry in [false, true] {
        let mut n = node("OUTSIDE", 10.0, 2.0);
        if geometry {
            n["_r"]["strokeGeometry"] = serde_json::json!([{"commandsBlob": 0}]);
        }
        assert_eq!(base_bounds(&tree, &n), Some([0.0, 0.0, 16.0, 12.0]));
        let alpha = column(&n, 14);
        assert_eq!([alpha[1], alpha[9], alpha[11], alpha[13]], [0, 0, 255, 0]);
    }
    // Without geometry each alignment places the band; a weight past the box fills it.
    assert_eq!(column(&node("INSIDE", 4.0, 2.0), 4), [0, 0, 255, 255]);
    assert_eq!(column(&node("CENTER", 4.0, 2.0), 6), [0, 0, 0, 255, 255, 0]);
    assert_eq!(column(&node("INSIDE", 2.0, 3.0), 4), [255, 255, 0, 0]);
}

#[test]
fn background_blur_continues_past_the_clipping_frame_edge() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let glass = serde_json::json!({"id": "1:2", "type": "RECTANGLE", "_r": {
        "m": [1, 0, 0, 1, 0, 0], "size": {"x": 40, "y": 10},
        "effects": [{"type": "BACKGROUND_BLUR", "radius": 8}],
    }});
    let frame = serde_json::json!({"id": "1:1", "type": "FRAME", "clipsContent": true, "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 40, "y": 10},
            "fillPaints": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1}}],
        }, "children": [glass]});
    let images = |_: &str| None;
    // A wider export area, as an effect margin gives: the frame's edge still bounds content.
    let svg = Svg::new(&tree, &images).document(&frame, [-10.0, -10.0, 50.0, 20.0], 1.0);
    let pixels = pixmap(&svg).unwrap();
    // Blurring past the edge into nothing would leave the frame's corner translucent.
    let alpha = pixels.pixel(11, 11).unwrap().alpha();
    assert!(alpha >= 250, "{alpha}");
}

#[test]
fn background_blur_tiles_the_backdrop_as_far_as_it_reads() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let glass = serde_json::json!({"id": "1:2", "type": "RECTANGLE", "_r": {
        "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10},
        "effects": [{"type": "BACKGROUND_BLUR", "radius": 16}],
    }});
    let frame = serde_json::json!({"id": "1:1", "type": "FRAME", "clipsContent": true, "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10},
            "fillPaints": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1}}],
        }, "children": [glass]});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 30.0, 30.0], 1.0);
    // Sigma 8 reads past one mirror image of a 10-unit frame.
    let alpha = pixmap(&svg).unwrap().pixel(5, 5).unwrap().alpha();
    assert!(alpha >= 250, "{alpha}");
}

#[test]
fn cuts_for_background_blurs_stop_at_their_cap() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let rect = |i: usize, effects: Json| {
        serde_json::json!({"id": format!("1:{}", i + 2), "type": "RECTANGLE", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10},
            "fillPaints": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1}}],
            "effects": effects,
        }})
    };
    let mut kids: Vec<Json> = (0..MAX_CUTS + 100).map(|i| rect(i, Json::Null)).collect();
    kids.push(rect(
        MAX_CUTS + 100,
        serde_json::json!([{"type": "BACKGROUND_BLUR", "radius": 4}]),
    ));
    let frame = serde_json::json!({"id": "1:1", "type": "FRAME", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10}, "fillPaints": [],
        }, "children": kids});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 10.0, 10.0], 1.0);
    // Each cut redraws the outlines it cuts, so a crafted file must not multiply them.
    assert_eq!(svg.matches("<mask id=").count(), MAX_CUTS);
}

#[test]
fn invisible_background_blurs_neither_draw_nor_use_up_the_budget() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let rect = |i: usize, opacity: f64| {
        serde_json::json!({"id": format!("1:{}", i + 2), "type": "RECTANGLE", "opacity": opacity, "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10},
            "fillPaints": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1}}],
            "effects": [{"type": "BACKGROUND_BLUR", "radius": 4}],
        }})
    };
    let mut kids: Vec<Json> = (0..70).map(|i| rect(i, 0.0)).collect();
    kids.push(rect(70, 1.0));
    let frame = serde_json::json!({"id": "1:1", "type": "FRAME", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10}, "fillPaints": [],
        }, "children": kids});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 10.0, 10.0], 1.0);
    assert_eq!(svg.matches("<feGaussianBlur").count(), 1);
    // A blurred shape without fills still paints its outline.
    let glass = serde_json::json!({"id": "1:2", "type": "RECTANGLE", "_r": {
        "m": [1, 0, 0, 1, 8, 0], "size": {"x": 10, "y": 10},
        "effects": [{"type": "BACKGROUND_BLUR", "radius": 4}],
    }});
    assert_eq!(base_bounds(&tree, &glass), Some([8.0, 0.0, 18.0, 10.0]));
}

#[test]
fn mask_content_draws_no_background_blur() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let fill = serde_json::json!([{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1}}]);
    let rect = |id: &str, effects: Json| {
        serde_json::json!({"id": id, "type": "RECTANGLE", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10}, "fillPaints": fill,
            "effects": effects,
        }})
    };
    let blur = serde_json::json!([{"type": "BACKGROUND_BLUR", "radius": 4}]);
    let mask = serde_json::json!({"id": "1:2", "type": "FRAME", "isMask": true, "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10}, "fillPaints": [],
        }, "children": [rect("1:3", blur.clone())]});
    let frame = serde_json::json!({"id": "1:1", "type": "FRAME", "_r": {
            "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10}, "fillPaints": fill,
        }, "children": [mask, rect("1:4", blur)]});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&frame, [0.0, 0.0, 10.0, 10.0], 1.0);
    // Only 1:4's: masks are referenced by url(), so a blur inside one escapes blur-free
    // backdrops.
    assert_eq!(svg.matches("<feGaussianBlur").count(), 1);
}

#[test]
fn unknown_effect_types_are_reported_instead_of_drawn_as_blurs() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let node = serde_json::json!({"id": "1:1", "type": "RECTANGLE", "_r": {
        "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10},
        "fillPaints": [{"type": "SOLID", "color": {"r": 0, "g": 0, "b": 0, "a": 1}}],
        "effects": [{"type": "GLASS", "radius": 24}],
    }});
    let images = |_: &str| None;
    let svg = Svg::new(&tree, &images).document(&node, [0.0, 0.0, 10.0, 10.0], 1.0);
    // Drawn as a layer blur, a glass input bar vanished.
    assert!(!svg.contains("<filter"));
    let found = diagnostics(&tree, &node);
    assert_eq!(
        (&found[0]["feature"], &found[0]["field"], &found[0]["state"]),
        (
            &serde_json::json!("effect"),
            &serde_json::json!("effects[0].type"),
            &serde_json::json!("skipped")
        )
    );
}

#[test]
fn unknown_paint_types_are_reported_instead_of_silently_dropped() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    // A shader fill under a drawn solid, and a noise stroke drawn around the box (no stroke
    // outlines): only the shader and the noise are gaps.
    let node = serde_json::json!({"id": "1:1", "type": "RECTANGLE", "_r": {
        "m": [1, 0, 0, 1, 0, 0], "size": {"x": 10, "y": 10},
        "fillPaints": [
            {"type": "SOLID", "color": {"r": 0, "g": 0, "b": 0, "a": 1}},
            {"type": "CUSTOM"},
        ],
        "strokePaints": [{"type": "NOISE"}],
        "strokeWeight": 2,
    }});
    let fields: Vec<_> = diagnostics(&tree, &node)
        .into_iter()
        .map(|d| (d["feature"].clone(), d["field"].clone(), d["state"].clone()))
        .collect();
    assert_eq!(
        fields,
        [
            ("fillPaints[1].type", "skipped"),
            ("strokePaints[0].type", "skipped")
        ]
        .map(|(f, s)| (
            serde_json::json!("paint"),
            serde_json::json!(f),
            serde_json::json!(s)
        ))
    );
}

#[test]
fn filter_covers_the_unclipped_content_plus_three_sigma() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let images = |_: &str| None;
    let mut svg = Svg::new(&tree, &images);
    // An ancestor clipped the render bounds to the right half; the blur still needs the left.
    let n = serde_json::json!({
        "absoluteRenderBounds": { "x": 50, "y": 0, "width": 50, "height": 50 },
        "_r": {
            "fb": { "x": 0, "y": 0, "width": 100, "height": 50 },
            "effects": [{ "type": "LAYER_BLUR", "radius": 20, "visible": true }],
        },
    });
    svg.effects(&n);
    assert!(
        svg.defs
            .contains("x=\"-30\" y=\"-30\" width=\"160\" height=\"110\""),
        "{}",
        svg.defs
    );
}

#[test]
fn pure_blur_skips_shadow_intermediates_without_changing_pixels() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let images = |_: &str| None;
    for kind in ["LAYER_BLUR", "FOREGROUND_BLUR"] {
        for radius in [0.0, 1.0, 19.5] {
            let mut svg = Svg::new(&tree, &images).for_raster();
            let mut n = serde_json::json!({ "_r": {
                "fb": { "x": 10.25, "y": 10.75, "width": 30, "height": 20 },
                "effects": [{ "type": kind, "radius": radius, "visible": true }]
            }});
            let id = svg.effects(&n).unwrap();
            assert!(!svg.defs.contains("feColorMatrix"));
            assert!(!svg.defs.contains("feMerge"));
            assert!(svg.defs.contains("in=\"SourceGraphic\""));
            let mut vector = Svg::new(&tree, &images);
            vector.effects(&n);
            assert!(vector.defs.contains("result=\"hard\""));
            assert!(vector.defs.contains("<feMerge result=\"all\">"));
            assert!(vector.defs.contains("<feGaussianBlur in=\"all\""));
            for scale in [1, 2] {
                let document = |defs: &str| {
                    format!(
                        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 60 50\"><defs>{defs}</defs><g filter=\"url(#{id})\"><rect x=\"10.25\" y=\"10.75\" width=\"30\" height=\"20\" fill=\"#a54cfe\" opacity=\"0.53\"/></g></svg>",
                        60 * scale,
                        50 * scale
                    )
                };
                let pixels = pixmap(&document(&svg.defs)).unwrap();
                assert!(pixels.data().iter().any(|b| *b != 0));
                assert_eq!(
                    pixels.data(),
                    pixmap(&document(&vector.defs)).unwrap().data()
                );
            }
            for shadow in ["DROP_SHADOW", "INNER_SHADOW"] {
                n["_r"]["effects"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({ "type": shadow, "radius": 4, "visible": true }));
                let mut mixed = Svg::new(&tree, &images).for_raster();
                mixed.effects(&n);
                assert!(mixed.defs.contains("result=\"hard\""));
                assert!(mixed.defs.contains("<feMerge result=\"all\">"));
                assert!(mixed.defs.contains("<feGaussianBlur in=\"all\""));
                n["_r"]["effects"].as_array_mut().unwrap().pop();
            }
        }
    }
}

#[test]
fn clipped_sprite_exports_keep_the_last_image_slice() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    let mut sprite = resvg::tiny_skia::Pixmap::new(300, 100).unwrap();
    let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    for (i, pixel) in sprite.data_mut().chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(&colors[(i % 300) / 100]);
    }
    for format in [
        crate::export::Format::Png,
        crate::export::Format::Jpg,
        crate::export::Format::Webp,
    ] {
        let bytes = crate::export::encode(&sprite, format, 95, None)
            .unwrap()
            .bytes;
        let images = |_: &str| Some(bytes.clone());
        for raster in [false, true] {
            for scale in [1.0, 3.0] {
                for (index, offset) in [4.48, 70.56, 136.64].into_iter().enumerate() {
                    let frame = serde_json::json!({
                        "id": "frame", "type": "FRAME", "clipsContent": true,
                        "_r": { "size": { "x": 56, "y": 56 }, "m": [1,0,0,1,0,0] },
                        "children": [{ "id": "image", "type": "RECTANGLE", "_r": {
                            "size": { "x": 196, "y": 64.96 }, "m": [1,0,0,1,-offset,-4.48],
                            "fillPaints": [{ "type": "IMAGE", "imageScaleMode": "FILL",
                                "image": { "hash": "sprite" } }]
                        }}]
                    });
                    let mut svg = Svg::new(&tree, &images);
                    if raster {
                        svg = svg.for_raster();
                    }
                    let pixels =
                        pixmap(&svg.document(&frame, [0.0, 0.0, 56.0, 56.0], scale)).unwrap();
                    let pixel = pixels
                        .pixel((50.0 * scale) as u32, (28.0 * scale) as u32)
                        .unwrap();
                    let expected = colors[index];
                    assert!(
                        pixel.alpha() > 250
                            && pixel.red().abs_diff(expected[0]) < 5
                            && pixel.green().abs_diff(expected[1]) < 5
                            && pixel.blue().abs_diff(expected[2]) < 5,
                        "sprite slice {index}, scale {scale}, raster {raster}, format {format:?}: {pixel:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_reused_image_is_embedded_once_per_mip_level() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    // 64x64: left half red, right half blue.
    let mut image = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
    for (i, pixel) in image.data_mut().chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if i % 64 < 32 {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        });
    }
    let bytes = crate::export::encode(&image, crate::export::Format::Png, 95, None)
        .unwrap()
        .bytes;
    let images = |_: &str| Some(bytes.clone());
    let child = |id: &str, x: f64, w: f64, h: f64, mode: &str| {
        serde_json::json!({ "id": id, "type": "RECTANGLE", "_r": {
            "size": { "x": w, "y": h }, "m": [1,0,0,1,x,0],
            "fillPaints": [{ "type": "IMAGE", "imageScaleMode": mode, "image": { "hash": "a" } }]
        }})
    };
    // Two full-size fills and a FIT into a wide box; then two small fills, a deeper mip level.
    let frame = serde_json::json!({
        "id": "frame", "type": "FRAME", "_r": { "size": { "x": 300, "y": 64 }, "m": [1,0,0,1,0,0] },
        "children": [child("a", 0.0, 64.0, 64.0, "FILL"), child("b", 64.0, 64.0, 64.0, "FILL"),
            child("c", 128.0, 128.0, 64.0, "FIT"), child("d", 256.0, 8.0, 8.0, "FILL"),
            child("e", 264.0, 8.0, 8.0, "FILL")]
    });
    let area = [0.0, 0.0, 300.0, 64.0];
    let svg = Svg::new(&tree, &images).document(&frame, area, 1.0);
    assert_eq!(svg.matches("base64,").count(), 1);
    let raster = Svg::new(&tree, &images)
        .for_raster()
        .document(&frame, area, 1.0);
    assert_eq!(raster.matches("base64,").count(), 2);
    // FIT keeps the image square and centred: empty, red, blue, empty across the wide box.
    let pixels = pixmap(&svg).unwrap();
    let alpha = |x: u32| pixels.pixel(x, 32).unwrap().alpha();
    let red = |x: u32| pixels.pixel(x, 32).unwrap().red() > 250;
    assert_eq!((alpha(140), alpha(244)), (0, 0));
    assert!(red(176) && !red(208) && alpha(208) == 255);
}

#[test]
fn stretch_fills_leave_the_area_past_the_image_empty() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    // 10x20: top half red, bottom half blue.
    let mut image = resvg::tiny_skia::Pixmap::new(10, 20).unwrap();
    for (i, pixel) in image.data_mut().chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if i / 10 < 10 {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        });
    }
    let bytes = crate::export::encode(&image, crate::export::Format::Png, 95, None)
        .unwrap()
        .bytes;
    let images = |_: &str| Some(bytes.clone());
    let stretch = |m00: f64, m02: f64, m11: f64, m12: f64| {
        serde_json::json!({ "type": "IMAGE", "imageScaleMode": "STRETCH",
                "image": { "hash": "stretch" },
                "transform": { "m00": m00, "m01": 0, "m02": m02, "m10": 0, "m11": m11, "m12": m12 } })
    };
    let (empty, red, blue) = (None, Some([255, 0, 0]), Some([0, 0, 255]));
    // (paint, child y offset in the 20 px wide clipping frame, frame height, [(x, y, expected)])
    let cases = [
        // v -0.25..1.25 over the 30 px node: empty 0..5, red 5..15, blue 15..25, empty 25..30.
        (
            stretch(1.0, 0.0, 1.5, -0.25),
            0.0,
            30.0,
            [
                (10.0, 2.0, empty),
                (10.0, 10.0, red),
                (10.0, 20.0, blue),
                (10.0, 28.0, empty),
            ],
        ),
        (
            stretch(1.0, 0.0, 1.5, -0.25),
            -10.0,
            20.0,
            [
                (10.0, 2.0, red),
                (10.0, 8.0, blue),
                (10.0, 12.0, blue),
                (10.0, 18.0, empty),
            ],
        ),
        // u -0.25..1.25 over the 20 px node: empty columns 0..3.3 and 16.7..20.
        (
            stretch(1.5, -0.25, 1.0, 0.0),
            0.0,
            30.0,
            [
                (1.0, 10.0, empty),
                (10.0, 10.0, red),
                (10.0, 20.0, blue),
                (19.0, 20.0, empty),
            ],
        ),
        // Shifted far right of the image: the node samples none of it.
        (
            stretch(1.0, 4.0, 1.0, 0.0),
            0.0,
            30.0,
            [
                (1.0, 2.0, empty),
                (10.0, 10.0, empty),
                (10.0, 20.0, empty),
                (19.0, 28.0, empty),
            ],
        ),
    ];
    // A skew whose sampled range is ~1000x1000 image units would need a ~400M px tile.
    let mut skewed = stretch(1.0, 0.0, 1000.0, 0.0);
    skewed["transform"]["m01"] = 1000.into();
    let node = serde_json::json!({ "id": "image", "type": "RECTANGLE", "_r": {
            "size": { "x": 20, "y": 30 }, "m": [1,0,0,1,0,0], "fillPaints": [skewed] }});
    let svg = Svg::new(&tree, &images).document(&node, [0.0, 0.0, 20.0, 30.0], 1.0);
    assert!(svg.contains("width=\"1\" height=\"1\""), "{svg}");
    for (paint, offset, height, probes) in cases {
        let frame = serde_json::json!({
            "id": "frame", "type": "FRAME", "clipsContent": true,
            "_r": { "size": { "x": 20, "y": height }, "m": [1,0,0,1,0,0] },
            "children": [{ "id": "image", "type": "RECTANGLE", "_r": {
                "size": { "x": 20, "y": 30 }, "m": [1,0,0,1,0,offset],
                "fillPaints": [paint.clone()] }}]
        });
        for raster in [false, true] {
            for scale in [1.0, 3.0] {
                let mut svg = Svg::new(&tree, &images);
                if raster {
                    svg = svg.for_raster();
                }
                let pixels =
                    pixmap(&svg.document(&frame, [0.0, 0.0, 20.0, height], scale)).unwrap();
                for (x, y, expected) in probes {
                    let pixel = pixels
                        .pixel((x * scale) as u32, (y * scale) as u32)
                        .unwrap();
                    let ok = match expected {
                        None => pixel.alpha() < 5,
                        Some([r, g, b]) => {
                            pixel.alpha() > 250
                                && pixel.red().abs_diff(r) < 5
                                && pixel.green().abs_diff(g) < 5
                                && pixel.blue().abs_diff(b) < 5
                        }
                    };
                    assert!(
                        ok,
                        "paint {paint}, offset {offset}, ({x}, {y}), scale {scale}, raster {raster}: {pixel:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn tile_fills_repeat_the_image_at_its_natural_size_from_the_node_origin() {
    let tree = Tree {
        nodes: Default::default(),
        children: Default::default(),
        blobs: Vec::new(),
    };
    // 20x10: left half red, right half blue.
    let mut tile = resvg::tiny_skia::Pixmap::new(20, 10).unwrap();
    for (i, pixel) in tile.data_mut().chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if i % 20 < 10 {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        });
    }
    let bytes = crate::export::encode(&tile, crate::export::Format::Png, 95, None)
        .unwrap()
        .bytes;
    let images = |_: &str| Some(bytes.clone());
    let (red, blue) = (true, false);
    // (paint fields, child x offset in the clipping frame, [(frame x, red?)])
    let cases = [
        (
            serde_json::json!({}),
            0.0,
            [(5.0, red), (15.0, blue), (25.0, red), (35.0, blue)],
        ),
        (
            serde_json::json!({"scale": 0.5}),
            0.0,
            [(2.0, red), (7.0, blue), (12.0, red), (17.0, blue)],
        ),
        (
            serde_json::json!({"originalImageWidth": 40, "originalImageHeight": 20}),
            0.0,
            [(15.0, red), (25.0, blue), (45.0, red), (5.0, red)],
        ),
        (
            serde_json::json!({}),
            -15.0,
            [(2.0, blue), (7.0, red), (12.0, red), (17.0, blue)],
        ),
        // A tile larger than the node keeps its period.
        (
            serde_json::json!({"scale": 10}),
            0.0,
            [(5.0, red), (25.0, red), (45.0, red), (49.0, red)],
        ),
        // No usable tile size falls back to FILL: 80 wide, so red then blue at the middle.
        (
            serde_json::json!({"scale": 0}),
            0.0,
            [(5.0, red), (35.0, red), (45.0, blue), (49.0, blue)],
        ),
    ];
    for (fields, offset, probes) in cases {
        let mut paint = serde_json::json!({ "type": "IMAGE", "imageScaleMode": "TILE",
                "image": { "hash": "tile" } });
        paint
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        let frame = serde_json::json!({
            "id": "frame", "type": "FRAME", "clipsContent": true,
            "_r": { "size": { "x": 50, "y": 10 }, "m": [1,0,0,1,0,0] },
            "children": [{ "id": "image", "type": "RECTANGLE", "_r": {
                "size": { "x": 80, "y": 10 }, "m": [1,0,0,1,offset,0],
                "fillPaints": [paint.clone()] }}]
        });
        for raster in [false, true] {
            for scale in [1.0, 3.0] {
                let mut svg = Svg::new(&tree, &images);
                if raster {
                    svg = svg.for_raster();
                }
                let pixels = pixmap(&svg.document(&frame, [0.0, 0.0, 50.0, 10.0], scale)).unwrap();
                for (x, is_red) in probes {
                    let pixel = pixels
                        .pixel((x * scale) as u32, (5.0 * scale) as u32)
                        .unwrap();
                    assert!(
                        pixel.alpha() > 250
                            && (pixel.red() > 250) == is_red
                            && (pixel.blue() > 250) != is_red,
                        "{paint} offset {offset} x {x}, scale {scale}, raster {raster}: {pixel:?}"
                    );
                }
            }
        }
    }
    // Past MAX_TILE_PX² the pattern is cut to the node instead of holding the whole tile.
    let node = serde_json::json!({ "id": "image", "type": "RECTANGLE", "_r": {
            "size": { "x": 80, "y": 10 }, "m": [1,0,0,1,0,0],
            "fillPaints": [{ "type": "IMAGE", "imageScaleMode": "TILE", "scale": 1000,
                "image": { "hash": "tile" } }] }});
    let svg = Svg::new(&tree, &images).document(&node, [0.0, 0.0, 80.0, 10.0], 1.0);
    assert!(
        svg.contains("width=\"80\" height=\"10\" patternTransform"),
        "{svg}"
    );
}

#[test]
fn halving_averages_each_2x2_block() {
    let mut p = resvg::tiny_skia::Pixmap::new(2, 2).unwrap();
    for (i, px) in p.data_mut().chunks_mut(4).enumerate() {
        px.copy_from_slice(&[if i == 0 { 200 } else { 0 }, 0, 0, 255]);
    }
    assert_eq!(halve(&p).data(), &[50, 0, 0, 255]);
}

#[test]
fn sections_export_with_a_margin_and_pages_as_their_children() {
    let bounds = |x: f64| serde_json::json!({ "x": x, "y": 0, "width": 10, "height": 10 });
    let section = serde_json::json!({ "type": "SECTION", "absoluteRenderBounds": bounds(0.0) });
    let page = serde_json::json!({ "type": "CANVAS", "children": [
            { "absoluteRenderBounds": bounds(0.0) }, { "absoluteRenderBounds": bounds(30.0) } ] });
    assert_eq!(
        (export_area(&section), export_area(&page)),
        (
            Some([-40.0, -40.0, 50.0, 50.0]),
            Some([0.0, 0.0, 40.0, 10.0])
        )
    );
}

#[test]
fn decodes_move_cubic_and_close_commands() {
    let mut blob = vec![1u8];
    for v in [1.0f32, 2.0] {
        blob.extend(v.to_le_bytes());
    }
    blob.push(4);
    for v in [3.0f32, 4.0, 5.0, 6.0, 7.0, 8.0] {
        blob.extend(v.to_le_bytes());
    }
    blob.push(0);
    let v = |x, y| Vec2 { x, y };
    assert_eq!(
        commands(&blob),
        [
            Seg::M(v(1.0, 2.0)),
            Seg::C(v(3.0, 4.0), v(5.0, 6.0), v(7.0, 8.0)),
            Seg::Z
        ]
    );
}

#[test]
fn base64_pads_like_the_standard_alphabet() {
    assert_eq!(
        (base64(b"Man"), base64(b"Ma"), base64(b"M")),
        ("TWFu".into(), "TWE=".into(), "TQ==".into())
    );
}
