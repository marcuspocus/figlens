//! Known stored semantics that the REST projection does not represent yet.
use super::*;
use std::collections::BTreeMap;

/// Node fields Figma Motion stores (2026 schema). `easingType` is left out, as prototype
/// transitions use it too, and so are boolean switches, which a false value can carry.
const MOTION_FIELDS: &[&str] = &[
    "timelinePosition",
    "timelinePositionType",
    "timelineDuration",
    "timelineOffset",
    "timelineKeyframeSnappingFps",
    "timelineDefinitions",
    "timelineAssignments",
    "keyframeValue",
    "keyframeValueRef",
    "keyframeOperation",
    "easingData",
    "motionTransform",
    "motionPathBezierHandles",
    "animationPresets",
    "backingAnimationPresetId",
    "styleIdsForAnimation",
    "styleAnimations",
];

pub(super) fn fields(k: &Obj, kind: &str, parent: Option<&Obj>, out: &mut Obj) {
    let mut diagnostics = Vec::new();
    let mut add = |feature, sources: Vec<&str>, reason| {
        if !sources.is_empty() {
            diagnostics.push(json!({"feature":feature,"sourceFields":sources,"state":"not-emitted","reason":reason}));
        }
    };
    if kind == "TEXT" {
        if at(k, "leadingTrim").as_str().is_some_and(|v| v != "NONE") {
            add(
                "leadingTrim",
                vec!["leadingTrim"],
                "Saved vertical text trim is not emitted; glyph placement alone does not describe implementation metrics.",
            );
        }
        let link = |v: &Json| v.as_object().is_some_and(|v| !v.is_empty());
        let base = link(at(k, "hyperlink"));
        let range = at(k, "textData")["styleOverrideTable"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|r| link(&r["hyperlink"]) || (base && r.get("hyperlink").is_some()));
        let mut sources = Vec::new();
        if base {
            sources.push("hyperlink");
        }
        if range {
            sources.push("textData.styleOverrideTable[].hyperlink");
        }
        add(
            "hyperlinks",
            sources,
            "Saved base/range hyperlink destinations and cleared ranges are not emitted; no destination is inferred or opened.",
        );
        let lines = at(k, "textData")["lines"].as_array();
        let sources = [
            ("listStartOffset", "textData.lines[].listStartOffset"),
            ("isFirstLineOfList", "textData.lines[].isFirstLineOfList"),
        ]
        .into_iter()
        .filter(|(key, _)| {
            lines
                .into_iter()
                .flatten()
                .any(|line| line["lineType"] == "ORDERED_LIST" && line.get(*key).is_some())
        })
        .map(|(_, path)| path)
        .collect();
        add(
            "listNumbering",
            sources,
            "Ordered-list type is emitted, but saved start/continuation metadata is not; no starting number is inferred.",
        );
    }
    if at(k, "stackMode") == "GRID" {
        add(
            "gridAutoPlacement",
            ["gridAutoTracks", "gridReflowEnabled"]
                .into_iter()
                .filter(|key| k.contains_key(*key))
                .collect(),
            "Saved grid auto-placement settings are not emitted.",
        );
    }
    if parent.is_some_and(|p| at(p, "stackMode") == "GRID")
        && at(k, "stackPositioning") != "ABSOLUTE"
    {
        add(
            "gridCell",
            ["gridRowAnchor", "gridColumnAnchor"]
                .into_iter()
                .filter(|key| k.contains_key(*key))
                .collect(),
            "Stored grid anchors can disagree with the saved layout, so no cell index is emitted; read the cell from the child's order and saved position.",
        );
    }
    // Figma Motion (2026 schema): keyframe/track/preset nodes, or Motion fields on any node.
    let mut motion = Vec::new();
    if matches!(
        kind,
        "KEYFRAME" | "KEYFRAME_TRACK" | "ANIMATION_PRESET_INSTANCE"
    ) {
        motion.push("type");
    }
    motion.extend(MOTION_FIELDS.iter().filter(|f| k.contains_key(**f)));
    add(
        "motion",
        motion,
        "Saved Motion keyframes, timelines and easing are not emitted; the output is the static frame.",
    );
    let mut code = Vec::new();
    if matches!(
        kind,
        "CODE_LAYER"
            | "CODE_COMPONENT"
            | "CODE_INSTANCE"
            | "CODE_FILE"
            | "CODE_LIBRARY"
            | "CODE_EMBED"
            | "NATIVE_CODE_LAYER_INSTANCE"
    ) {
        code.push("type");
    }
    if at(k, "isEmbedCodeLayer") == true {
        code.push("isEmbedCodeLayer");
    }
    add(
        "codeLayers",
        code,
        "Saved code layers are not emitted; their source and rendered result are not read.",
    );
    if at(k, "measurements")
        .as_array()
        .is_some_and(|v| !v.is_empty())
    {
        add(
            "measurements",
            vec!["measurements"],
            "Saved Dev Mode measurements are not emitted; how their stored end node, side and offset map to Figma's is not established.",
        );
    }
    if at(k, "layoutGrids")
        .as_array()
        .is_some_and(|v| !v.is_empty())
    {
        add(
            "layoutGrids",
            vec!["layoutGrids"],
            "Saved layout guides are not emitted; these are distinct from grid auto layout.",
        );
    }
    let mut slots = Vec::new();
    if kind == "SLOT" {
        slots.push("type");
    }
    if at(k, "isSlotContent") == true {
        slots.push("isSlotContent");
    }
    if at(k, "componentPropDefs")
        .as_array()
        .into_iter()
        .flatten()
        .any(|d| d["type"] == "SLOT")
    {
        slots.push("componentPropDefs[].type");
    }
    if at(k, "componentPropAssignments")
        .as_array()
        .into_iter()
        .flatten()
        .any(|a| a["varValue"]["value"].get("slotContentIdValue").is_some())
    {
        slots.push("componentPropAssignments[].varValue.value.slotContentIdValue");
    }
    add(
        "slots",
        slots,
        "Slot relationships and per-instance content are not reconstructed; this marker does not prove which children are missing.",
    );
    if !diagnostics.is_empty() {
        out.entry("_figlens").or_insert_with(|| json!({}))["unrepresented"] = diagnostics.into();
    }
}

#[derive(Default)]
pub(super) struct Inventory {
    checked: usize,
    items: BTreeMap<String, Json>,
}

impl Inventory {
    pub(super) fn record(&mut self, node: &Json) {
        self.checked += 1;
        for diagnostic in node["_figlens"]["unrepresented"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let feature = diagnostic["feature"].as_str().unwrap();
            let item = self.items.entry(feature.into()).or_insert_with(|| {
                json!({
                    "feature":feature,"state":"not-emitted","reason":diagnostic["reason"],
                    "nodeCount":0,"exampleNodeIds":[],"examplesTruncated":false,
                })
            });
            let count = item["nodeCount"].as_u64().unwrap() + 1;
            item["nodeCount"] = count.into();
            let examples = item["exampleNodeIds"].as_array_mut().unwrap();
            if examples.len() < 5 && node["id"].as_str().is_some_and(|id| id.len() <= 1024) {
                examples.push(node["id"].clone());
            }
            item["examplesTruncated"] = json!(count as usize > examples.len());
        }
    }

    pub(super) fn finish(self) -> Option<Json> {
        (!self.items.is_empty()).then(|| json!({"checkedNodeCount":self.checked,
            "coverage":"known stored fields in the effective expanded subtree; hidden instance descendants and unreconstructed slot content are not checked",
            "verification":"not-compared-with-figma",
            "items":self.items.into_values().collect::<Vec<_>>()
        }))
    }
}

pub(super) fn select(doc: &mut Json) {
    let mut inventory = Inventory::default();
    let mut stack = vec![&doc["document"]];
    while let Some(node) = stack.pop() {
        inventory.record(node);
        stack.extend(node["children"].as_array().into_iter().flatten());
    }
    if let Some(summary) = inventory.finish() {
        doc["_figlens"]["handoffDiagnostics"] = summary;
    } else if let Some(meta) = doc.get_mut("_figlens").and_then(Json::as_object_mut) {
        meta.remove("handoffDiagnostics");
        if meta.is_empty() {
            doc.as_object_mut().unwrap().remove("_figlens");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_losses_have_counts_bounded_examples_and_explicit_coverage() {
        let k = object(
            json!({"leadingTrim":"CAP_HEIGHT","hyperlink":{"url":"https://example.com"},"textData":{"styleOverrideTable":[{"hyperlink":{}}],"lines":[{"lineType":"ORDERED_LIST","listStartOffset":2}]},"stackMode":"GRID","gridAutoTracks":"ROWS","isSlotContent":true}),
        );
        let mut inventory = Inventory::default();
        for n in 0..7 {
            let mut out = Obj::from_iter([("id".into(), json!(format!("1:{n}")))]);
            fields(&k, "TEXT", None, &mut out);
            assert_eq!(out["_figlens"]["unrepresented"][0]["state"], "not-emitted");
            inventory.record(&Json::Object(out));
        }
        let result = inventory.finish().unwrap();
        assert_eq!(result["checkedNodeCount"], 7);
        assert_eq!(result["items"].as_array().unwrap().len(), 5);
        for item in result["items"].as_array().unwrap() {
            assert_eq!(item["nodeCount"], 7);
            assert_eq!(item["exampleNodeIds"].as_array().unwrap().len(), 5);
            assert_eq!(item["examplesTruncated"], true);
        }
        let mut out = Obj::new();
        fields(
            &object(
                json!({"leadingTrim":"NONE","hyperlink":{},"textData":{"lines":[{"lineType":"PLAIN","listStartOffset":0}]},"isSlotContent":false,"layoutGrids":[]}),
            ),
            "TEXT",
            None,
            &mut out,
        );
        assert!(out.is_empty());
        assert!(Inventory::default().finish().is_none());
        let mut plain = json!({"document":{"id":"1:1"}});
        select(&mut plain);
        assert!(plain.get("_figlens").is_none());
        let mut out = Obj::new();
        fields(
            &object(json!({"gridRowsSizing":"AUTO","gridRowAnchor":{"sessionID":0,"localID":1}})),
            "FRAME",
            None,
            &mut out,
        );
        assert!(
            out.is_empty(),
            "inactive cached grid fields are not reported"
        );
        let mut scoped = json!({"document":{"id":"1:1","children":[{"id":"1:2","_figlens":{"unrepresented":[{"feature":"slots","reason":"slot content","state":"not-emitted"}]}}]}});
        select(&mut scoped);
        assert_eq!(
            scoped["_figlens"]["handoffDiagnostics"]["checkedNodeCount"],
            2
        );
        scoped["document"] = json!({"id":"1:3"});
        select(&mut scoped);
        assert!(
            scoped.get("_figlens").is_none(),
            "a sibling's gaps cannot leak into the selected scope"
        );
    }

    #[test]
    fn motion_and_code_layers_are_reported_and_ordinary_nodes_are_not() {
        let gaps = |input: Json, kind: &str| -> Vec<(Json, Json)> {
            let mut out = Obj::new();
            fields(&object(input), kind, None, &mut out);
            out.get("_figlens")
                .and_then(|f| f["unrepresented"].as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .map(|d| (d["feature"].clone(), d["sourceFields"].clone()))
                .collect()
        };
        assert_eq!(
            gaps(json!({}), "KEYFRAME_TRACK"),
            [(json!("motion"), json!(["type"]))]
        );
        assert_eq!(
            gaps(json!({"easingData":{},"timelineDefinitions":{}}), "FRAME"),
            [(
                json!("motion"),
                json!(["timelineDefinitions", "easingData"])
            )]
        );
        assert_eq!(
            gaps(json!({}), "CODE_LAYER"),
            [(json!("codeLayers"), json!(["type"]))]
        );
        assert_eq!(
            gaps(json!({"isEmbedCodeLayer":true}), "FRAME"),
            [(json!("codeLayers"), json!(["isEmbedCodeLayer"]))]
        );
        assert_eq!(
            gaps(json!({"measurements":[{"freeText":""}]}), "FRAME"),
            [(json!("measurements"), json!(["measurements"]))]
        );
        assert!(gaps(json!({"measurements":[]}), "FRAME").is_empty());
        // Prototype transitions also store easingType; it is not Motion evidence, and neither
        // is a stored false switch.
        assert!(gaps(json!({"easingType":"EASE_IN"}), "FRAME").is_empty());
        assert!(
            gaps(
                json!({"isEmbedCodeLayer":false,"timelineDisabled":false}),
                "FRAME"
            )
            .is_empty()
        );
    }
}
