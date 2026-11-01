//! MCP access to the same workflow sources shipped to Pi and skill users.

const SKILL: &str = include_str!("../skills/figlens/SKILL.md");
pub const TOPICS: &[&str] = &[
    "build",
    "audit",
    "images",
    "search",
    "plan",
    "planning-details",
    "yolo",
];

pub const INSTRUCTIONS: &str = "Read local Figma .fig snapshots with figlens. Resolve the requested screen from names or visible text using find/tree, or search for copy inside instance overrides, properties and swaps; ask about recognizable choices only when materially ambiguous, and resolve node IDs yourself. Use inspect for compact layout/text, nodes for full styles/masks/component data, variables for tokens, export for reference pixels or assets, compare_image to check a screenshot of the built screen against a frame, and verify to check a bundle against its manifest, list its font faces and read its handoff preflight findings. For implementation, call workflow(topic=build); for comparing existing UI with a design or correcting layout differences, call workflow(topic=audit); for implementation planning or careful reproduction, also load plan; for website image preparation, load images. Load planning-details only for page/multi-page scope or unresolved dependencies. Load yolo, which already includes the build sections it needs, only when the user explicitly requests autonomous implementation and cleanup of what the run created. A plan-only request ends with a plan, and so does a request to plan the whole scope before implementing: detail every slice, then wait for approval; an authorized build continues through implementation, code review, visual checks and corrections using the project's existing architecture, building components, data and layout as a frontend engineer would rather than reproducing the layer tree. Simple questions and exports use the design tools directly; when the host can run code, chain related calls in one script and return only the values you need. Ordinary design questions search the file directly and create nothing. Only when the user asks to create or enable an index for a design, index prepare its file with the scope to search (figlens picks the storage; no setup step), poll jobs status until succeeded, reporting queued, running or failed jobs as such, then search with workspaceId and node set to that scope, and reuse that workspace afterwards. To find screens that look like an image the user gives, search that workspace with image (a PNG path inside the roots) or a result's imageRef; never a URL or base64. Run index reset or replaceUnavailable only on the user's explicit request. For where a component, variable, style or image is used, call usages with its exact id on the file, or on an enabled index within its prepared scope. For what changed between two saved copies of a design, call diff with old and file (local paths); unknown is not removed. Only when the user asks to follow a design's changes, which opts in to preparing that scope when no index covers it, index track its scope, watch subscribe once its baseline succeeded, watch start that scope so saves are recorded while this server runs, page changes with workspaceId and that subscriptionId, and after handling a page and its revisions' detail pages watch ack that page's nextCursor, never a detail one or null; to follow a search's results instead, watch subscribe with query, watch evaluate after a refresh, and read changes with it; refreshRequired in tracking means index refresh records the change; watch unsubscribe stops following, and untrack only when the user asks. To reuse an earlier index (a project's is found by workspace or workspaceId, not file), narrow a multi-design workspace, recover one or read usages results, read workflow(topic=search). Workflow reads return guidance, not executed work; host tools and permissions govern project edits, browser checks and cleanup. Check isError and content on failed calls. Native Dev Mode annotations and font files are unavailable; report material gaps rather than guessing.";

const MCP_PREAMBLE: &str = "Use this guidance in the current agent context and within the user's authorized scope. It does not execute work or grant permissions. Use the configured figlens MCP tools with absolute file/output paths; CLI examples describe equivalent tool arguments. Project inspection/edits, directory creation, bundle extraction and browser checks require the host's own tools. Retrieve another topic only for missing guidance needed now: images covers image delivery; build covers design reading and project reuse; search covers an index, usages and following changes. Links beginning workflow: name another topic of this tool, with an optional section anchor; no local skill installation is needed.\n\n";
const CLI_PREAMBLE: &str = "Use this guidance in the current agent context and within the user's authorized scope. It does not execute work or grant permissions. Run the figlens commands shown; MCP argument names map to the same CLI arguments. Project inspection/edits, directory creation, bundle extraction and browser checks require the host's own tools. Retrieve another topic only for missing guidance needed now: images covers image delivery; build covers design reading and project reuse; search covers an index, usages and following changes. A link beginning workflow: names another topic: run `figlens workflow <topic>`; no local skill installation is needed.\n\n";

pub fn read(topic: &str) -> Result<String, String> {
    compose(topic, MCP_PREAMBLE)
}

/// The same guidance for an agent running the CLI without MCP or an installed skill.
pub fn read_cli(topic: &str) -> Result<String, String> {
    compose(topic, CLI_PREAMBLE)
}

fn compose(topic: &str, preamble: &str) -> Result<String, String> {
    let headings: &[&str] = match topic {
        "build" => &[
            "Search and local index",
            "Where outputs go",
            "Building a frame",
            "Verification evidence",
            "Font and asset preflight",
            "Link back to Figma",
            "Reading the output",
            "Known gaps",
        ],
        "audit" => &[
            "Search and local index",
            "Where outputs go",
            "Verification evidence",
            "Known gaps",
        ],
        "images" => &["Where outputs go", "Images and slices", "Known gaps"],
        "search" => &["Search and local index"],
        // yolo serves the build sections every run needs; its guide names the conditional ones.
        "yolo" => &[
            "Where outputs go",
            "Building a frame",
            "Verification evidence",
            "Reading the output",
            "Font and asset preflight",
            "Component mappings",
            "Many frames",
            "Known gaps",
        ],
        "plan" | "planning-details" => &[],
        _ => {
            return Err(format!(
                "unknown workflow topic {topic:?}; choose {}",
                TOPICS.join(", ")
            ));
        }
    };
    let mut text = String::from(preamble);
    match topic {
        "audit" => text.push_str(include_str!("../skills/figlens/references/audit.md")),
        "plan" => text.push_str(include_str!("../skills/figlens/references/planning.md")),
        "planning-details" => text.push_str(include_str!(
            "../skills/figlens/references/planning-details.md"
        )),
        "yolo" => text.push_str(include_str!("../skills/figlens/references/yolo.md")),
        _ => {}
    }
    let skill = SKILL.replace("\r\n", "\n");
    for heading in headings {
        let marker = format!("## {heading}\n");
        let start = skill
            .find(&marker)
            .ok_or_else(|| format!("embedded workflow section missing: {heading}"))?;
        let section = &skill[start..];
        let section = section
            .split_once("\n## ")
            .map_or(section, |(first, _)| first);
        text.push_str(section);
        text.push_str("\n\n");
    }
    if topic == "search" {
        text.push_str(include_str!("../skills/figlens/references/search.md"));
    }
    for (file, topic) in [
        ("../SKILL.md", "build"),
        ("audit.md", "audit"),
        ("planning.md", "plan"),
        ("planning-details.md", "planning-details"),
        ("yolo.md", "yolo"),
        ("search.md", "search"),
    ] {
        text = text.replace(
            &format!("(references/{file}"),
            &format!("(workflow:{topic}"),
        );
        text = text.replace(&format!("({file}"), &format!("(workflow:{topic}"));
    }
    // A link to a section this topic serves stays on the page.
    for heading in headings {
        let anchor = heading.to_lowercase().replace(' ', "-");
        text = text.replace(&format!("(workflow:build#{anchor}"), &format!("(#{anchor}"));
    }
    // Only these topics serve these sections in full, so their links name them.
    for (anchor, owner) in [
        ("search-and-local-index", "search"),
        ("where-it-is-used", "search"),
        ("images-and-slices", "images"),
    ] {
        for from in [format!("(#{anchor}"), format!("(workflow:build#{anchor}")] {
            text = text.replace(&from, &format!("(workflow:{owner}#{anchor}"));
        }
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_and_build_share_evidence_rules_across_cli_and_mcp() {
        let marker = "## Verification evidence\n";
        let skill = SKILL.replace("\r\n", "\n");
        let evidence = skill
            .split_once(marker)
            .unwrap()
            .1
            .split("\n## ")
            .next()
            .unwrap();
        for topic in ["audit", "build"] {
            let mcp = read(topic).unwrap();
            let cli = read_cli(topic).unwrap();
            assert_eq!(
                mcp.strip_prefix(MCP_PREAMBLE),
                cli.strip_prefix(CLI_PREAMBLE)
            );
            assert!(mcp.contains(evidence));
            assert!(mcp.contains("Static review is not visual acceptance"));
            assert!(mcp.contains("Relationships") && mcp.contains("Variation"));
            assert!(!mcp.contains("the reference image only confirms them"));
        }
        let audit = read("audit").unwrap();
        assert!(audit.contains("# Audit an existing UI"));
        assert!(audit.contains("review-only request authorizes no application edits"));
        assert!(audit.contains("reuse its existing bundle"));
        assert!(audit.contains("Stop that item"));
        assert!(audit.contains("1, 3 and 7"));
        assert!(audit.contains("workflow:build#building-a-frame"));
        assert!(!audit.contains("](../SKILL.md"));
        assert!(read("build").unwrap().contains("workflow:audit"));
        assert!(INSTRUCTIONS.contains("workflow(topic=audit)"));
        assert!(INSTRUCTIONS.contains("call usages with its exact id on the file"));
        assert!(INSTRUCTIONS.contains("watch ack that page's nextCursor, never a detail one"));

        assert!(INSTRUCTIONS.contains("call diff with old and file"));
        assert!(INSTRUCTIONS.contains("search that workspace with image"));
    }

    #[test]
    fn build_and_audit_serve_the_skill_search_and_index_decision() {
        let section = SKILL
            .replace("\r\n", "\n")
            .split_once("## Search and local index\n")
            .unwrap()
            .1
            .split("\n## ")
            .next()
            .unwrap()
            .replace("(#where-it-is-used", "(workflow:search#where-it-is-used")
            .replace("(references/search.md", "(workflow:search");
        assert!(section.contains("needs no index"));
        for topic in ["images", "yolo", "build"] {
            assert!(
                read(topic)
                    .unwrap()
                    .contains("](workflow:search#search-and-local-index)")
            );
        }
        // The index steps live in their own guide, served only by the search topic.
        let full = read("search").unwrap().replace("\r\n", "\n");
        assert!(full.contains(&section));
        assert!(full.contains("never authorizes them") && full.contains("### Following changes"));
        assert!(full.contains("## What changed between two copies"));
        for topic in ["audit", "build"] {
            let text = read(topic).unwrap().replace("\r\n", "\n");
            assert!(text.contains(&section));
            assert!(!text.contains("Opt-in steps:") && !text.contains("### Following changes"));
        }
    }

    #[test]
    fn implementation_guides_ask_for_frontend_code_not_a_copy_of_the_layer_tree() {
        // An agent given only design data replayed the node tree as a runtime; every
        // implementation entry point must frame the job as components, data and layout.
        for topic in ["build", "yolo"] {
            let text = read(topic).unwrap();
            assert!(text.contains("**Decide the frontend structure") && text.contains("**Data:**"));
            // A new project gets a professional setup, not the smallest one that renders.
            assert!(text.contains("**New project:**"));
            // Code review, project checks included, comes before pixels.
            assert!(text.contains("type check, lint and format pass"));
        }
        let yolo = read("yolo").unwrap();
        assert!(yolo.contains("## The job") && yolo.contains("design component → code file"));
        assert!(read("plan").unwrap().contains("2. **Structure:**"));
        assert!(INSTRUCTIONS.contains("rather than reproducing the layer tree"));
        // A "keep it minimal" instruction must not lower the bar on any implementation path.
        for topic in ["build", "yolo", "plan"] {
            assert!(read(topic).unwrap().contains("never this bar"));
        }
    }

    #[test]
    fn build_reports_missing_dependencies_without_blocking_implementation() {
        let build = read("build").unwrap();
        assert!(build.contains("## Font and asset preflight"));
        assert!(build.contains("document.fonts.load"));
        assert!(build.contains("empty array"));
        assert!(build.contains("missing-assets.json"));
        assert!(build.contains("Continue implementation without waiting for missing inputs"));
        assert!(build.contains("final visual acceptance"));
        assert!(!build.contains("Block affected work"));
    }

    #[test]
    fn guides_keep_scope_image_evidence_and_cleanup_boundaries() {
        let build = read("build").unwrap();
        assert!(build.contains("## Building a frame") && build.contains("## Known gaps"));
        assert!(!build.contains("## Setup") && !build.contains("## Component mappings"));
        assert!(build.contains("figlens_bindings") && build.contains("integrations.md"));
        let images = read("images").unwrap();
        assert!(
            images.contains("crop, mask or effects") && images.contains("browser transfer bytes")
        );
        assert!(!images.contains("## Building a frame"));
        // Both guides lead to writes, and an agent may load only one of them.
        assert!(build.contains("## Where outputs go") && images.contains("## Where outputs go"));
        let plan = read("plan").unwrap();
        assert!(plan.contains("A plan-only request ends with the plan"));
        // A whole-scope plan must not shrink to slice 1 and then start implementing.
        assert!(plan.contains("give every slice all four fields and stop with the complete plan"));
        assert!(INSTRUCTIONS.contains("detail every slice, then wait for approval"));
        assert!(INSTRUCTIONS.contains("read workflow(topic=search)"));
        assert!(plan.contains("(workflow:planning-details#save-and-resume-proportionately)"));
        let details = read("planning-details").unwrap();
        assert!(
            details.contains("Internal Only Canvas")
                && details.contains("Native Dev Mode annotations")
        );
        let yolo = read("yolo").unwrap();
        assert!(yolo.contains("after implementation and the applicable acceptance checks succeed"));
        assert!(
            yolo.contains("unexpected edits")
                && yolo
                    .contains("leaves the working plan and the run's export directory available")
        );
        assert!(
            yolo.contains("Start no watch lease, subscription or tracking unless the user asked")
                && yolo.contains("never `forget`, another owner's or an ownerless one")
        );
        // Cleanup is scoped to what the run created: its export directory and its processes.
        assert!(
            yolo.contains("`.figlens/exports/<scope>/`, never loose in it")
                && yolo.contains("Delete nothing outside that directory")
                && yolo.contains("this run started, and only those")
        );
        assert!(details.contains("a plan-only run deletes none of them"));
        // yolo carries the build sections a run needs but not the index guide it rarely does.
        assert!(
            yolo.contains("## Building a frame\n")
                && yolo.contains("## Verification evidence\n")
                && !yolo.contains("## Search and local index\n")
                && yolo.contains("](workflow:images#images-and-slices)")
                && yolo.contains("](#component-mappings)")
                && !yolo.contains("](workflow:build#")
        );
        assert!(yolo.contains("**Proven by a tool:**") && yolo.contains("**Not verified:**"));
        // A documented fallback must not hold the plan the way a blocking gap does.
        assert!(
            yolo.contains("**accepted fallback**")
                && yolo.contains("Only a blocking gap stops the run")
        );
        let build = read("build").unwrap();
        // CLI tree and diff print text by default; a jq pipeline needs --json.
        assert!(
            build.contains("**Chaining calls**")
                && build.contains("`tree` and `diff` print text unless you add `--json`")
        );
    }
}
