//! Lossless, opt-in sharing for REST nodes. References occur only in documented node slots.

use std::collections::HashMap;

use serde_json::{Value, json};

const STYLE_FIELDS: [&str; 6] = [
    "fills",
    "strokes",
    "effects",
    "style",
    "boundVariables",
    "layoutGrids",
];
const PLACEMENT_FIELDS: [&str; 3] = ["id", "absoluteBoundingBox", "absoluteRenderBounds"];
pub const FORMAT: &str = "figlens.nodes.compact.v1";

/// Encode an owned REST response without changing its metadata or any literal style values.
pub fn encode(mut data: Value, subtrees: bool) -> Value {
    let mut counts = HashMap::<Vec<u8>, usize>::new();
    walk(&data["document"], &mut |node| {
        for key in STYLE_FIELDS {
            if let Some(value) = node.get(key) {
                *counts
                    .entry(serde_json::to_vec(value).unwrap())
                    .or_default() += 1;
            }
        }
    });
    let mut shared = Vec::new();
    let mut indices = HashMap::<Vec<u8>, usize>::new();
    walk_mut(&mut data["document"], &mut |node| {
        for key in STYLE_FIELDS {
            let Some(value) = node.get_mut(key) else {
                continue;
            };
            let bytes = serde_json::to_vec(value).unwrap();
            if let Some(&index) = indices.get(&bytes) {
                *value = json!({"$style": index});
                continue;
            }
            let count = counts[&bytes];
            let reference = json!({"$style": shared.len()});
            // Pay for the table entry as well as all references, not just duplicate bytes.
            if (value
                .as_object()
                .is_some_and(|o| o.len() == 1 && o.contains_key("$style")))
                || (count > 1
                    && bytes.len() * (count - 1)
                        > serde_json::to_vec(&reference).unwrap().len() * count + 1)
            {
                let index = *indices.entry(bytes).or_insert_with(|| {
                    shared.push(value.clone());
                    shared.len() - 1
                });
                *value = json!({"$style": index});
            }
        }
    });
    let mut result =
        json!({"format": FORMAT, "data": data, "sharedStyles": shared, "subtrees": []});
    if subtrees {
        let mut intern = Intern::default();
        let root = intern.record(&result["data"]["document"]);
        let mut table = Vec::new();
        let mut indices = HashMap::new();
        let document = intern.emit(&result["data"]["document"], root, &mut table, &mut indices);
        // Ancestor sharing can hide occurrences counted for descendants; keep it only if
        // the complete document plus its actually used templates is smaller.
        let before = serialized_size(&result["data"]["document"], usize::MAX) + 2;
        let table = Value::Array(table);
        let after = serialized_size(&document, usize::MAX) + serialized_size(&table, usize::MAX);
        if after < before {
            result["data"]["document"] = document;
            result["subtrees"] = table;
        }
    }
    result
}

pub(super) fn serialized_size(value: &Value, limit: usize) -> usize {
    struct Count {
        bytes: usize,
        limit: usize,
    }
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self.bytes.saturating_add(bytes.len());
            if self.bytes > self.limit {
                return Err(std::io::Error::other("compact nodes size limit"));
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count { bytes: 0, limit };
    let _ = serde_json::to_writer(&mut count, value);
    count.bytes
}

fn walk(node: &Value, f: &mut impl FnMut(&Value)) {
    f(node);
    for child in node["children"].as_array().into_iter().flatten() {
        walk(child, f);
    }
}

fn walk_mut(node: &mut Value, f: &mut impl FnMut(&mut Value)) {
    f(node);
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children {
            walk_mut(child, f);
        }
    }
}

struct Entry {
    // Children hold intern indices, so no subtree is repeatedly serialized or retained.
    node: Value,
    count: usize,
    bytes: usize,
    nodes: usize,
}

#[derive(Default)]
struct Intern {
    keys: HashMap<Vec<u8>, usize>,
    entries: Vec<Entry>,
}

impl Intern {
    fn record(&mut self, node: &Value) -> usize {
        let mut local = Value::Object(
            node.as_object()
                .unwrap()
                .iter()
                .filter(|(key, _)| {
                    key.as_str() != "children" && !PLACEMENT_FIELDS.contains(&key.as_str())
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        );
        let children = node.get("children");
        let mut nodes = 1;
        let mut child_bytes = 0;
        if let Some(children) = children {
            let ids: Vec<_> = children
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    let id = self.record(c);
                    nodes += self.entries[id].nodes;
                    child_bytes += self.entries[id].bytes;
                    json!(id)
                })
                .collect();
            local["children"] = ids.into();
        }
        let key = serde_json::to_vec(&local).unwrap();
        if let Some(&id) = self.keys.get(&key) {
            self.entries[id].count += 1;
            return id;
        }
        let bytes = key.len() + child_bytes
            - local["children"].as_array().map_or(0, |a| {
                a.iter().map(|id| id.to_string().len()).sum::<usize>()
            });
        let id = self.entries.len();
        self.entries.push(Entry {
            node: local,
            count: 1,
            bytes,
            nodes,
        });
        self.keys.insert(key, id);
        id
    }

    fn template(&self, id: usize) -> Value {
        let mut node = self.entries[id].node.clone();
        if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
            for child in children {
                *child = self.template(child.as_u64().unwrap() as usize);
            }
        }
        node
    }

    fn emit(
        &self,
        node: &Value,
        id: usize,
        table: &mut Vec<Value>,
        indices: &mut HashMap<usize, usize>,
    ) -> Value {
        let entry = &self.entries[id];
        if entry.count > 1
            && entry.bytes.saturating_sub(40 + entry.nodes * 3) * entry.count > entry.bytes + 2
        {
            let index = *indices.entry(id).or_insert_with(|| {
                table.push(self.template(id));
                table.len() - 1
            });
            let mut placements = Vec::new();
            walk(node, &mut |n| {
                let mut placement = serde_json::Map::new();
                for key in PLACEMENT_FIELDS {
                    if let Some(value) = n.get(key) {
                        placement.insert(key.to_owned(), value.clone());
                    }
                }
                placements.push(Value::Object(placement));
            });
            return json!({"$subtree": index, "placements": placements});
        }
        let mut result = Value::Object(
            node.as_object()
                .unwrap()
                .iter()
                .filter(|(key, _)| key.as_str() != "children")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        );
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            result["children"] = children
                .iter()
                .zip(entry.node["children"].as_array().unwrap())
                .map(|(child, id)| self.emit(child, id.as_u64().unwrap() as usize, table, indices))
                .collect::<Vec<_>>()
                .into();
        }
        result
    }
}

#[cfg(test)]
mod tests;
