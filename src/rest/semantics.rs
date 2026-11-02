//! Additive snapshot semantics. Mappings are not yet compared with same-version Figma exports.
use super::*;

/// Provenance of one mapped field; `None` when the snapshot does not store its source,
/// so an omitted entry means "absent" without repeating it on every node.
fn status(source: &str, present: bool, supported: bool) -> Option<Json> {
    present.then(
        || json!({ "source": source, "state": if supported { "recorded" } else { "unsupported" } }),
    )
}

fn put(evidence: &mut Json, key: &str, entry: Option<Json>) {
    if let Some(entry) = entry {
        evidence[key] = entry;
    }
}

pub(super) fn layout(k: &Obj, parent: Option<&Obj>, out: &mut Obj) -> Json {
    let mut evidence = json!({});
    let mut constraints = Obj::new();
    for (key, axis, field, min, max, stretch) in [
        (
            "constraints.horizontal",
            "horizontal",
            "horizontalConstraint",
            "LEFT",
            "RIGHT",
            "LEFT_RIGHT",
        ),
        (
            "constraints.vertical",
            "vertical",
            "verticalConstraint",
            "TOP",
            "BOTTOM",
            "TOP_BOTTOM",
        ),
    ] {
        let mapped = match at(k, field).as_str() {
            Some("MIN") => Some(min),
            Some("MAX") => Some(max),
            Some("CENTER") => Some("CENTER"),
            Some("SCALE") => Some("SCALE"),
            Some("STRETCH") => Some(stretch),
            _ => None,
        };
        put(
            &mut evidence,
            key,
            status(field, k.contains_key(field), mapped.is_some()),
        );
        if let Some(value) = mapped {
            constraints.insert(axis.into(), json!(value));
        }
    }
    if !constraints.is_empty() {
        out.insert("constraints".into(), constraints.into());
    }
    let flow = matches!(
        parent.and_then(|p| at(p, "stackMode").as_str()),
        Some("HORIZONTAL" | "VERTICAL")
    ) && matches!(at(k, "stackPositioning").as_str(), None | Some("AUTO"));
    let grow = at(k, "stackChildPrimaryGrow")
        .as_f64()
        .filter(|v| flow && (*v == 0.0 || *v == 1.0));
    let align = at(k, "stackChildAlignSelf")
        .as_str()
        .filter(|v| flow && matches!(*v, "INHERIT" | "STRETCH" | "MIN" | "CENTER" | "MAX"));
    put(
        &mut evidence,
        "layoutGrow",
        status(
            "stackChildPrimaryGrow",
            k.contains_key("stackChildPrimaryGrow"),
            grow.is_some(),
        ),
    );
    put(
        &mut evidence,
        "layoutAlign",
        status(
            "stackChildAlignSelf",
            k.contains_key("stackChildAlignSelf"),
            align.is_some(),
        ),
    );
    if let Some(value) = grow {
        out.insert("layoutGrow".into(), json!(value));
    }
    if let Some(value) = align {
        out.insert("layoutAlign".into(), json!(value));
    }
    evidence
}

fn definition_id(value: &Json) -> Option<String> {
    let id = guid(value)?;
    let session = value["sessionID"].as_u64()?;
    let local = value["localID"].as_u64()?;
    (session <= u32::MAX as u64 && local <= u32::MAX as u64 && id != "4294967295:4294967295")
        .then_some(id)
}

/// Keep the assignment path's existing parent-definition fallback unchanged.
pub(super) fn resolve_definition<'a>(
    own: &'a Json,
    parents: &HashMap<String, &'a Json>,
) -> &'a Json {
    guid(&own["parentPropDefId"])
        .and_then(|id| parents.get(&id).copied())
        .unwrap_or(own)
}

pub(super) fn definitions(tree: &Tree, id: &str, k: &Obj, out: &mut Obj, evidence: &mut Json) {
    let list = at(k, "componentPropDefs").as_array();
    let parent_id = list
        .filter(|definitions| !definitions.is_empty())
        .and_then(|_| parent_id(tree, id));
    let parent = parent_id.as_ref().map(|id| to_json(tree.nodes[id]));
    let parents = parent
        .iter()
        .flat_map(|p| p["componentPropDefs"].as_array().into_iter().flatten())
        .filter_map(|d| Some((guid(&d["id"])?, d)))
        .collect::<HashMap<_, _>>();
    let mut declarations = Obj::new();
    let mut entries = Vec::new();
    for (index, own) in list.into_iter().flatten().enumerate() {
        let def = resolve_definition(own, &parents);
        let inherited = own.get("parentPropDefId").is_some();
        let resolved = !inherited
            || definition_id(&own["parentPropDefId"]).is_some_and(|id| parents.contains_key(&id));
        let def_id = definition_id(&def["id"]);
        let own_id = definition_id(&own["id"]);
        let name = def["name"].as_str();
        let kind = match def["type"].as_str() {
            Some("BOOL" | "BOOLEAN") => Some("BOOLEAN"),
            Some("TEXT") => Some("TEXT"),
            Some("INSTANCE_SWAP") => Some("INSTANCE_SWAP"),
            Some("VARIANT") => Some("VARIANT"),
            _ => None,
        };
        let supported =
            resolved && def_id.is_some() && own_id.is_some() && name.is_some() && kind.is_some();
        let mut entry = status(
            "componentPropDefs",
            true,
            supported && kind != Some("VARIANT"),
        )
        .unwrap();
        entry["index"] = json!(index);
        entry["id"] = json!(own_id);
        entry["definitionId"] = json!(def_id);
        entry["name"] = json!(name);
        entry["type"] = json!(def["type"].as_str());
        entry["sourceNodeId"] = json!(if inherited && resolved {
            parent_id.as_deref().unwrap_or(id)
        } else {
            id
        });
        if supported {
            let kind = kind.unwrap();
            let mut declaration = json!({"type":kind});
            let value = prop_value_source(def);
            let usable = value.as_ref().is_some_and(|(v, source)| {
                let raw = match *source {
                    "varValue.value" => &def["varValue"]["value"],
                    source => &def[source],
                };
                match kind {
                    "BOOLEAN" => v.is_boolean(),
                    "TEXT" => {
                        v.is_string()
                            && (raw["textDataValue"]["characters"].is_string()
                                || raw["textValue"].is_string()
                                || raw["textValue"]["characters"].is_string())
                    }
                    "INSTANCE_SWAP" => definition_id(&raw["symbolIdValue"]["guid"])
                        .or_else(|| definition_id(&raw["guidValue"]))
                        .is_some_and(|id| v.as_str() == Some(id.as_str())),
                    _ => false,
                }
            });
            let source = value
                .as_ref()
                .map_or("varValue.value|value|initialValue", |(_, source)| *source);
            let default = status(
                source,
                ["varValue", "value", "initialValue"]
                    .iter()
                    .any(|f| def.get(*f).is_some()),
                usable,
            );
            put(&mut entry, "defaultValue", default);
            if usable {
                declaration["defaultValue"] = value.unwrap().0;
            }
            if kind == "INSTANCE_SWAP" {
                let preferred = def["preferredValues"]["instanceSwapValues"].as_array();
                let usable = preferred.is_some_and(|a| {
                    a.iter().all(|v| {
                        matches!(v["type"].as_str(), Some("COMPONENT" | "COMPONENT_SET"))
                            && v["key"].as_str().is_some_and(|k| !k.is_empty())
                    })
                });
                let preferred_state = status(
                    "preferredValues.instanceSwapValues",
                    def["preferredValues"].get("instanceSwapValues").is_some(),
                    usable,
                );
                put(&mut entry, "preferredValues", preferred_state);
                if usable {
                    declaration["preferredValues"] = preferred
                        .unwrap()
                        .iter()
                        .map(|v| json!({"type":v["type"],"key":v["key"]}))
                        .collect();
                }
            }
            if kind == "VARIANT" {
                // shortcut: stored variant defaults/options are not established; add them after independent capture validation.
                entry["defaultValue"] = json!({"source": source, "state": "unsupported"});
                entry["variantOptions"] = json!({"state":"unsupported", "source":"componentPropDefs", "reason":"complete variant options are not established"});
            }
            let key = if kind == "VARIANT" {
                name.unwrap().to_owned()
            } else {
                format!("{}#{}", name.unwrap(), def_id.unwrap())
            };
            declarations.insert(key, declaration);
        }
        entries.push(entry);
    }
    if let Some(mut state) = status(
        "componentPropDefs",
        k.contains_key("componentPropDefs"),
        list.is_some(),
    ) {
        if !entries.is_empty() {
            state["entries"] = entries.into();
        }
        evidence["componentPropertyDefinitions"] = state;
    }
    if !declarations.is_empty() {
        out.insert("componentPropertyDefinitions".into(), declarations.into());
    }
}

#[cfg(test)]
mod tests;
