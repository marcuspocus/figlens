//! Dev Mode annotations and ready-for-dev status, read from the snapshot's own fields.
//! Not compared with same-version Figma exports.
use super::*;

/// Kiwi `AnnotationPropertyType` as REST/Plugin `AnnotationProperty.type`. Both enums list the
/// same 33 properties; unknown values pass through unchanged.
fn property(kiwi: &str) -> &str {
    match kiwi {
        "FILL" => "fills",
        "STROKE" => "strokes",
        "WIDTH" => "width",
        "HEIGHT" => "height",
        "MIN_WIDTH" => "minWidth",
        "MIN_HEIGHT" => "minHeight",
        "MAX_WIDTH" => "maxWidth",
        "MAX_HEIGHT" => "maxHeight",
        "STROKE_WIDTH" => "strokeWeight",
        "CORNER_RADIUS" => "cornerRadius",
        "EFFECT" => "effects",
        "TEXT_STYLE" => "textStyleId",
        "TEXT_ALIGN_HORIZONTAL" => "textAlignHorizontal",
        "FONT_FAMILY" => "fontFamily",
        "FONT_STYLE" => "fontStyle",
        "FONT_SIZE" => "fontSize",
        "FONT_WEIGHT" => "fontWeight",
        "LINE_HEIGHT" => "lineHeight",
        "LETTER_SPACING" => "letterSpacing",
        "STACK_SPACING" => "itemSpacing",
        "STACK_PADDING" => "padding",
        "STACK_MODE" => "layoutMode",
        "STACK_ALIGNMENT" => "alignItems",
        "OPACITY" => "opacity",
        "COMPONENT" => "mainComponent",
        "GRID_ROW_GAP" => "gridRowGap",
        "GRID_COLUMN_GAP" => "gridColumnGap",
        "GRID_ROW_COUNT" => "gridRowCount",
        "GRID_COLUMN_COUNT" => "gridColumnCount",
        "GRID_ROW_ANCHOR_INDEX" => "gridRowAnchorIndex",
        "GRID_COLUMN_ANCHOR_INDEX" => "gridColumnAnchorIndex",
        "GRID_ROW_SPAN" => "gridRowSpan",
        "GRID_COLUMN_SPAN" => "gridColumnSpan",
        other => other,
    }
}

/// REST `annotations` (`label`, `properties`) plus the Plugin API's `labelMarkdown` and
/// `categoryId`. An instance sublayer takes its main's annotations and any override (possibly an
/// empty list) through the expansion's field merge.
pub(super) fn node(k: &Obj, out: &mut Obj) {
    let list: Vec<Json> = at(k, "annotations")
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| {
            let mut entry = json!({});
            // Quill stores an emptied label as `<p><br></p>`, which leaves no text.
            if let Some((plain, markdown)) = a["label"]
                .as_str()
                .map(label)
                .filter(|(plain, _)| !plain.trim().is_empty())
            {
                entry["label"] = plain.into();
                entry["labelMarkdown"] = markdown.into();
            }
            if let Some(props) = a["properties"].as_array().filter(|p| !p.is_empty()) {
                entry["properties"] = props
                    .iter()
                    .filter_map(|p| p["type"].as_str())
                    .map(|t| json!({ "type": property(t) }))
                    .collect();
            }
            if let Some(id) = super::semantics::definition_id(&a["categoryId"]) {
                entry["categoryId"] = id.into();
            }
            entry
        })
        .collect();
    if !list.is_empty() {
        out.insert("annotations".into(), list.into());
    }
}

/// REST `devStatus` from one page's `handoffStatusMap` entry; Figma stores Ready for dev as BUILD.
pub(super) fn dev_status(info: &Json) -> Option<Json> {
    let kind = match info["status"].as_str()? {
        "BUILD" => "READY_FOR_DEV",
        "COMPLETED" => "COMPLETED",
        _ => return None,
    };
    let mut status = json!({ "type": kind });
    if let Some(d) = info["description"].as_str().filter(|d| !d.is_empty()) {
        status["description"] = d.into();
    }
    Some(status)
}

/// Plugin API `AnnotationCategory` fields (`label`, `isPreset`, `color` when stored) of each
/// category in the document's `annotationCategories`, by id. A preset's color is not stored.
pub(super) fn categories(stored: &Json) -> HashMap<String, Json> {
    let mut out = HashMap::new();
    for item in stored["items"].as_array().into_iter().flatten() {
        let Some(id) = super::semantics::definition_id(&item["id"]) else {
            continue;
        };
        let custom = &item["custom"];
        let preset = item["preset"].as_str().filter(|p| *p != "NONE");
        let label = match preset {
            // Figma's preset names are the enum in title case (Development, Interaction, ...).
            Some(p) => {
                let mut chars = p.chars();
                chars.next().into_iter().collect::<String>() + &chars.as_str().to_lowercase()
            }
            None => match custom["label"].as_str() {
                Some(l) => l.to_owned(),
                None => continue,
            },
        };
        let mut category = json!({ "label": label, "isPreset": preset.is_some() });
        if let Some(color) = custom["color"].as_str() {
            category["color"] = color.to_ascii_lowercase().into();
        }
        out.insert(id, category);
    }
    out
}

/// Deepest list level indented; deeper items share it, so output stays linear in the input.
const MAX_INDENT: usize = 8;

/// The stored label is rich-text HTML (`<p>`, lists, links, emphasis). Returns it as plain text
/// (one line per paragraph or list item) and as Markdown. A label without tags is taken as text.
fn label(html: &str) -> (String, String) {
    if !html.contains('<') {
        return (html.to_owned(), html.to_owned());
    }
    let (mut plain, mut md) = (Text::default(), Text::default());
    // Open lists: ordered flag and the next item number.
    let mut lists: Vec<(bool, u32)> = Vec::new();
    let mut hrefs: Vec<Option<String>> = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(start) = rest.find('<') else {
            let text = entities(rest);
            plain.push(&text);
            md.push(&text);
            break;
        };
        let text = entities(&rest[..start]);
        plain.push(&text);
        md.push(&text);
        // A `<` that opens no tag is text.
        let Some(end) = rest[start..].find('>') else {
            plain.push(&rest[start..]);
            md.push(&rest[start..]);
            break;
        };
        let tag = &rest[start + 1..start + end];
        rest = &rest[start + end + 1..];
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let attr = |key: &str| {
            let at = tag.find(&format!("{key}=\""))? + key.len() + 2;
            Some(entities(&tag[at..at + tag[at..].find('"')?]))
        };
        match (name.as_str(), closing) {
            ("p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => {
                plain.block(LINE);
                md.block(if lists.is_empty() { PARAGRAPH } else { LINE });
                if let Some(level) = name.strip_prefix('h').and_then(|n| n.parse().ok()) {
                    md.push(&format!("{} ", "#".repeat(level)));
                }
            }
            ("br", _) => {
                plain.block(LINE);
                md.block(HARD_BREAK);
            }
            ("ol" | "ul", false) => lists.push((name == "ol", 1)),
            ("ol" | "ul", true) => {
                lists.pop();
            }
            ("li", false) => {
                plain.block(LINE);
                md.block(LINE);
                let depth = lists.len().saturating_sub(1).min(MAX_INDENT);
                // Quill marks each item's kind; an `<ol>` can hold bullet items.
                let ordered = match attr("data-list").as_deref() {
                    Some("bullet") => false,
                    Some("ordered") => true,
                    _ => lists.last().is_some_and(|l| l.0),
                };
                let marker = match lists.last_mut() {
                    Some(list) if ordered => {
                        list.1 += 1;
                        format!("{}. ", list.1 - 1)
                    }
                    _ => "- ".to_owned(),
                };
                let item = format!("{}{marker}", "  ".repeat(depth));
                plain.push(&item);
                md.push(&item);
            }
            ("strong" | "b", _) => md.push("**"),
            ("em" | "i", _) => md.push("_"),
            ("s" | "strike" | "del", _) => md.push("~~"),
            ("code", _) => md.push("`"),
            ("a", false) => {
                hrefs.push(attr("href"));
                md.push("[");
            }
            ("a", true) => match hrefs.pop().flatten() {
                Some(href) => md.push(&format!("]({href})")),
                None => md.push("]"),
            },
            _ => {}
        }
    }
    (plain.finish(), md.finish())
}

/// Separators, weakest first: a line break, a Markdown hard break, a blank line.
const LINE: (u8, &str) = (0, "\n");
const HARD_BREAK: (u8, &str) = (1, "  \n");
const PARAGRAPH: (u8, &str) = (2, "\n\n");

/// Output that separates blocks lazily, so empty paragraphs and `<br>`s leave no blank lines.
#[derive(Default)]
struct Text {
    out: String,
    /// The strongest separator owed before the next text.
    pending: Option<(u8, &'static str)>,
}

impl Text {
    fn block(&mut self, sep: (u8, &'static str)) {
        if !self.out.is_empty() && self.pending.is_none_or(|p| sep.0 > p.0) {
            self.pending = Some(sep);
        }
    }

    fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some((_, sep)) = self.pending.take() {
            self.out.truncate(self.out.trim_end_matches(' ').len());
            self.out.push_str(sep);
        }
        self.out.push_str(text);
    }

    fn finish(self) -> String {
        self.out.trim().to_owned()
    }
}

fn entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        // A reference is at most `&#x10FFFF;`; looking further makes many `&`s quadratic.
        let decoded = rest
            .bytes()
            .take(11)
            .position(|b| b == b';')
            .and_then(|end| {
                let name = &rest[1..end];
                let c = match name {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => name
                        .strip_prefix("#x")
                        .or_else(|| name.strip_prefix("#X"))
                        .map_or_else(
                            || name.strip_prefix('#').and_then(|n| n.parse().ok()),
                            |hex| u32::from_str_radix(hex, 16).ok(),
                        )
                        .and_then(char::from_u32),
                };
                c.map(|c| (c, end))
            });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quill_lists_become_lines_and_markdown_items() {
        let html = r#"<ol><li value="1" data-list="bullet">Red &amp; blue</li><li value="2" data-list="bullet">Green</li></ol>"#;
        assert_eq!(
            label(html),
            (
                "- Red & blue\n- Green".into(),
                "- Red & blue\n- Green".into()
            )
        );
    }

    #[test]
    fn ordered_items_are_numbered_per_list_and_nested_lists_indent() {
        let html = r#"<ol><li data-list="ordered">a</li><li>b</li><ul><li>c</li></ul></ol>"#;
        assert_eq!(label(html).0, "1. a\n2. b\n  - c");
    }

    #[test]
    fn paragraphs_links_and_emphasis_keep_markdown_only_markers() {
        let html = r#"<p>See <a href="https://example.com/x?a=1&amp;b=2" rel="noopener" target="_blank">spec</a></p><p><em>Note</em><br>two</p><p></p>"#;
        let (plain, md) = label(html);
        assert_eq!(plain, "See spec\nNote\ntwo");
        assert_eq!(
            md,
            "See [spec](https://example.com/x?a=1&b=2)\n\n_Note_  \ntwo"
        );
    }

    #[test]
    fn headings_strong_code_and_strike_are_markdown() {
        let html = "<h2>Title</h2><p><strong>b</strong> <code>c</code> <s>d</s> <a>e</a></p>";
        assert_eq!(label(html).1, "## Title\n\n**b** `c` ~~d~~ [e]");
    }

    #[test]
    fn an_empty_quill_paragraph_adds_no_blank_line() {
        assert_eq!(
            label("<p>a</p><p><br></p><p>b</p>"),
            ("a\nb".into(), "a\n\nb".into())
        );
    }

    #[test]
    fn references_decode_and_a_bare_ampersand_stays() {
        assert_eq!(
            label("<p>&#65;&#x42;&nbsp;&amp &copy;</p>").0,
            "AB &amp &copy;"
        );
    }

    #[test]
    fn untagged_labels_are_text_and_a_stray_bracket_keeps_the_rest() {
        assert_eq!(label("Tom &amp; Jerry").0, "Tom &amp; Jerry");
        assert_eq!(label("<p>a</p> < b").0, "a < b");
    }

    #[test]
    fn deeply_nested_lists_stay_linear_in_the_input() {
        let n = 20_000;
        let html = "<ol>".repeat(n) + &"<li>x".repeat(n);
        let (plain, md) = label(&html);
        // Each item is at most a newline, the capped indent, a marker and its text.
        assert!(
            plain.len() <= n * (1 + 2 * MAX_INDENT + 8),
            "{}",
            plain.len()
        );
        assert_eq!(plain.len(), md.len());
    }

    #[test]
    fn annotations_map_properties_and_skip_unset_categories() {
        let k = object(json!({"annotations": [
            {"label": "<p>x</p>", "properties": [{"type": "FILL"}, {"type": "STACK_SPACING"}, {"type": "NEW"}],
             "categoryId": {"sessionID": 4294967295u64, "localID": 4294967295u64}},
            {"label": "<p><br></p>", "properties": [], "categoryId": {"sessionID": 1, "localID": 2}},
            {"label": "  ", "properties": []},
        ]}));
        let mut out = Obj::new();
        node(&k, &mut out);
        assert_eq!(
            out["annotations"],
            json!([
                {"label": "x", "labelMarkdown": "x", "properties": [{"type": "fills"}, {"type": "itemSpacing"}, {"type": "NEW"}]},
                {"categoryId": "1:2"},
                {},
            ])
        );
    }

    #[test]
    fn empty_override_list_emits_nothing() {
        let mut out = Obj::new();
        node(&object(json!({"annotations": []})), &mut out);
        assert!(!out.contains_key("annotations"));
    }

    #[test]
    fn presets_and_custom_categories_read_like_the_plugin_api() {
        let id = |l: u64| json!({"sessionID": 1, "localID": l});
        let table = categories(&json!({"items": [
            {"id": id(0), "preset": "DEVELOPMENT"},
            {"id": id(2), "preset": "NONE", "custom": {"color": "PINK", "label": "Requirement"}},
            {"id": {"sessionID": 4294967295u64, "localID": 4294967295u64}, "preset": "CONTENT"},
        ]}));
        assert_eq!(table.len(), 2);
        assert_eq!(
            table["1:0"],
            json!({"label": "Development", "isPreset": true})
        );
        assert_eq!(
            table["1:2"],
            json!({"label": "Requirement", "isPreset": false, "color": "pink"})
        );
    }

    #[test]
    fn build_is_ready_for_dev_completed_keeps_a_description_and_none_is_absent() {
        assert_eq!(
            dev_status(&json!({"status": "BUILD", "description": "v2"})),
            Some(json!({"type": "READY_FOR_DEV", "description": "v2"}))
        );
        assert_eq!(
            dev_status(&json!({"status": "COMPLETED", "description": ""})),
            Some(json!({"type": "COMPLETED"}))
        );
        assert_eq!(dev_status(&json!({"status": "NONE"})), None);
    }
}
