//! MCP parameter metadata/preflight and additive tool-error diagnostics.
use std::error::Error;

use figlens::{ErrorKind, FigError};
use serde_json::{Value, json};

/// An MCP parameter: key, JSON type, requiredness and description.
type Param = (&'static str, &'static str, bool, &'static str);
const FILE: Param = (
    "file",
    "string",
    true,
    "the saved .fig: a local path (absolute recommended), or gs://bucket/object using gcloud; remote sources are unavailable with --root",
);
const NODE: Param = (
    "node",
    "string",
    true,
    "a stored node id (12:345 or 12-345), full instance sublayer id (I12:345;67:89) from inspect/nodes, or a Figma URL with ?node-id=",
);
const STORAGE: Param = (
    "storage",
    "string",
    false,
    "absolute custom storage directory, only for a workspace kept outside the default storage; must be under the server's --storage-root",
);
const WORKSPACE_ID: Param = (
    "workspaceId",
    "string",
    false,
    "ws_... from a previous index reply",
);
/// CLI and tools share command execution and result formatting.
pub const TOOLS: &[(&str, &str, &[Param])] = &[
    (
        "workflow",
        "Read built-in figlens skill guidance for this agent: build for design-to-code and project reuse, audit for comparing existing UI with a design and correcting confirmed differences, images for website asset preparation, search for an index, usages and following changes, plan for scoped implementation planning, planning-details for page/multi-page work or unresolved design dependencies, yolo for explicitly requested autonomous implementation and cleanup of what the run created. Returns the requested guide only; no design file, project read/write, implementation, verification or cleanup is performed. Ordinary design questions and exports can use the design tools directly.",
        &[(
            "topic",
            "string",
            true,
            "Guide to read: build, audit, images, search, plan, planning-details or yolo.",
        )],
    ),
    (
        "tree",
        "List node ids, REST types (as in nodes and find) and names under a chosen root. A shallow outline locates pages and frames when their names are unknown; find searches known names or visible text directly.",
        &[
            FILE,
            (
                "node",
                "string",
                false,
                "a stored root id; default the whole document; use inspect for instance sublayers",
            ),
            ("depth", "integer", false, "levels to list below the root"),
            (
                "json",
                "boolean",
                false,
                "nested {id,type,name,size,visible,layoutMode,children} JSON instead of indented text (about 1.7x larger); size is the local pre-rotation [width,height] (absent on the document and pages), visible appears only when false and layoutMode only on auto-layout frames; nodes on a page or in a section add firstText, their first visible stored text (breadth-first, instances not expanded, at most 200 characters), which tells a canvas label (a box around one line of text) from a screen",
            ),
            (
                "topFrames",
                "boolean",
                false,
                "list only the screens under the root, at any depth: FRAME, COMPONENT, COMPONENT_SET and INSTANCE children of a page or of a section on one (groups, shapes and text are not screens; internal pages are skipped unless they are the root); depth does not apply; text lines start with the id, as nodes/export/bundle frame lists read them; with json, a flat array of entries without children (with firstText), adding page and, inside a section, section ids",
            ),
        ],
    ),
    (
        "find",
        "Search stored layer names and text (case-insensitive). Returns ids, paths, local sizes and text previews. Does not expand instance children; use inspect on the instance. Copy inside instance overrides, text or boolean properties or swapped components is found by search, not find.",
        &[
            FILE,
            (
                "query",
                "string",
                true,
                "literal substring in a layer name or text",
            ),
            (
                "node",
                "string",
                false,
                "search this stored subtree; default the document; use inspect for instance sublayers",
            ),
            (
                "type",
                "string",
                false,
                "Figma node type, e.g. FRAME or TEXT",
            ),
            (
                "limit",
                "integer",
                false,
                "1-200 results, default 20; truncated signals more matches",
            ),
        ],
    ),
    (
        "search",
        "Ranked search of layer names and full text in one stored scope, including instance text overrides, text/boolean properties and nested swaps as REST nodes resolve them. Each result has an exact ref {sourceId, snapshotId, nodeId} (instance sublayer IDs work with inspect/nodes/export), origin stored or instance-effective, textSource, page, scopeRoot (a navigation container, not a screen claim), path, and matches with reasons and UTF-16 spans. coverage says whether the scope was fully indexed (ready) or a work limit stopped it (partial, totalRecords null); hasMore/nextCursor page the same candidates. Hidden layers and internal pages are excluded unless requested. Give file to read and hash that .fig now (nothing is written), or workspaceId to rank the workspace's prepared indexes of exactly this scope across its sources as one corpus (sourceId narrows it, refs keep each source's sourceId); a workspace search never builds: sources without a ready index are listed in sources with state pending, not_prepared, stale, incompatible or unreadable and the nextSteps index call, and coverage then says partial.",
        &[
            (
                "file",
                "string",
                false,
                "the saved .fig to read now: a local path or gs://bucket/object; give file or workspaceId",
            ),
            (
                "workspaceId",
                "string",
                false,
                "ws_... from the index tool: search its prepared indexes instead of reading a file",
            ),
            (
                "sourceId",
                "string",
                false,
                "with workspaceId, search only this registered source",
            ),
            STORAGE,
            (
                "query",
                "string",
                false,
                "words or a phrase; case, width and accents fold, CJK matches by character pairs or a single character; no regex. Required unless image or imageRef is given",
            ),
            (
                "image",
                "string",
                false,
                "with workspaceId: a local PNG (at most 8 MiB, 4 MP) inside the server's roots to search by; read for this call only, never stored or sent anywhere. Not with imageRef",
            ),
            (
                "imageRef",
                "object",
                false,
                "with workspaceId: {sourceId, snapshotId, nodeId} of a node in a search result, to search by its stored render; the snapshot must still be the source's current one. Not with image",
            ),
            (
                "crop",
                "object",
                false,
                "{x, y, width, height} in whole pixels of the query image (of the stored 256-pixel render for imageRef)",
            ),
            (
                "node",
                "string",
                false,
                "stored scope root; default the document; narrow it when coverage is partial",
            ),
            (
                "type",
                "string",
                false,
                "Figma node types to keep, comma-separated, e.g. TEXT or TEXT,INSTANCE",
            ),
            (
                "limit",
                "integer",
                false,
                "1-200 results per page, default 20; independent of indexing work limits",
            ),
            (
                "cursor",
                "string",
                false,
                "nextCursor from the previous page of the same query, filters and snapshot; a workspace cursor lasts one hour (then cursor_expired)",
            ),
            (
                "includeHidden",
                "boolean",
                false,
                "also return hidden layers and instance copies hidden by overrides or properties",
            ),
            (
                "includeInternal",
                "boolean",
                false,
                "also return layers on Figma's internal-only page",
            ),
            (
                "mode",
                "string",
                false,
                "text (default); semantic or hybrid with a configured local model (see models); with image or imageRef, visual (default without query) or hybrid (default with query: image plus text channels)",
            ),
        ],
    ),
    (
        "usages",
        "Exact relations of one node or resource in one stored scope, from the same effective index as search (nested instances and swaps included): direction in (default) lists the locations that point at it, out lists what it points at; with out, an instance_of, uses_style or binds_variable result whose cached library copy names it carries library {kind, namespace, key}, the selector the library argument takes. Relations: instance_of (instance to main component, recorded symbolData or resolved definition/swap), variant_of (component to its set; a COMPONENT_SET node also lists its known members and returns their instances with via), binds_variable (paint, effect and field variable bindings, local or library), uses_style, uses_image (a text range's style comes from the last row of its styleID, as REST nodes give it; its paint images and variables from the first such row with fills, as rendered), prototype_target (recorded transitions; never a claim that a flow is reachable) and contains (asked for explicitly). Every result keeps an exact ref {sourceId, snapshotId, nodeId}, relation, state (resolved, missing, unresolved, unsupported, external, recorded), basis (recorded or resolved), evidence, page, scopeRoot and path; names never establish identity. The definition may lie outside the scope; scope limits only the returned locations. No total is reported; coverage says complete or partial with reasons (work limits, hidden content not expanded, truncated members, library_identity unavailable), and nextCursor pages in a stable order (a workspace library cursor lasts one hour, then cursor_expired). Give file to read that .fig now (nothing is written), or workspaceId to read a source's prepared index of exactly this scope (never builds). Pass snapshotId from the ref you hold: another current snapshot is source_stale. With workspaceId, library {kind, namespace, key} instead of node or resource lists uses_library locations of that library component, style or variable across every visible source's prepared scope (basis cached_copy: the namespace comes from each file's cached copy); the definition stays unresolved (library_source_unproven) unless library.definition selects it by exact ref (selected, keyEvidence unverified), coverage lists each source's state and reasons (source_not_ready, index_partial, namespace_unknown, hidden_excluded, authorization_restricted), and snapshotId, relation and direction do not apply; its results carry ref, name, type, relation, basis, state, evidence, hidden, internal, page and scopeRoot but no path or origin.",
        &[
            (
                "file",
                "string",
                false,
                "the saved .fig to read now: a local path or gs://bucket/object; give file or workspaceId",
            ),
            (
                "workspaceId",
                "string",
                false,
                "ws_... from the index tool: read its prepared index instead of the file",
            ),
            (
                "sourceId",
                "string",
                false,
                "with workspaceId, the source to read; needed when several have this scope; with library, narrows to that one consumer source",
            ),
            STORAGE,
            (
                "node",
                "string",
                false,
                "the exact layer id, stored or an instance sublayer I...;...; give node or resource",
            ),
            (
                "resource",
                "string",
                false,
                "a resource id instead of a layer: VariableID:..., StyleKey:... or image:<hash>",
            ),
            (
                "library",
                "object",
                false,
                "with workspaceId, a library reference instead of a node or resource: {kind: component, style or variable, namespace, key}, as an out result's library names it; lists the layers in every visible source (or sourceId) whose cached copy records it. Optional definition {sourceId, snapshotId, nodeId} selects the definition by exact ref; without namespace only copies that record none are listed",
            ),
            (
                "scope",
                "string",
                false,
                "stored scope root whose locations are returned; default the document; a workspace needs a prepared scope",
            ),
            (
                "relation",
                "string",
                false,
                "relations to keep, comma-separated; default every one but contains",
            ),
            (
                "direction",
                "string",
                false,
                "in (what points at it) or out (what it points at, with library {kind, namespace, key} on a use of a cached library copy)",
            ),
            (
                "snapshotId",
                "string",
                false,
                "the snapshot your ref came from; a different current snapshot is source_stale",
            ),
            (
                "limit",
                "integer",
                false,
                "1-200 results per page, default 50",
            ),
            (
                "cursor",
                "string",
                false,
                "nextCursor from the previous page of the same query and snapshot",
            ),
            (
                "includeHidden",
                "boolean",
                false,
                "also return hidden locations",
            ),
        ],
    ),
    (
        "diff",
        "What changed between two saved revisions of a design (old, file): exact layer ids and types only: a frame renamed in place (same id and type) is a structure change, while a recreated frame (a new id) is reported as removed plus added, with a labelled possibleMatch (same name, else same rounded size and position) that never claims identity; lineage between the two files is not verified. Each changed top-level frame (layers directly on a page group under the page) has one summary line with counts per facet: text (text, fonts and laid-out glyphs), layout (size, transform, constraints, auto layout), appearance (fills, strokes, effects, opacity, radius, visibility), properties (component property definitions and assignments, variable values, scopes and code syntax, descriptions, style type, Dev Mode annotations and categories, a page's Ready for dev map and prototype interactions), structure (name, type, parent, order, hidden) and bindings (variable, style, image, main component and prototype edges, old and new compared so removed ones are listed). Nodes carry changed fields with old/new values (each within 512 bytes and 4 KiB per node, 256 KiB per page; larger values say truncated, never unchanged) and edge additions/removals, with swap naming both mains. A side that stopped short reports its missing layers as unknown, never removed; a layer with a field it could not read and no proven difference is unknown with unread, never changed. Pages of changes in stable frame order; nextCursor binds both snapshots, the node, the built corpora and the retained result, so a rebuild that stopped elsewhere is cursor_stale. Nothing is written.",
        &[
            (
                "old",
                "string",
                true,
                "the earlier saved .fig: a local path",
            ),
            ("file", "string", true, "the later saved .fig: a local path"),
            (
                "node",
                "string",
                false,
                "stored scope root on both sides; default the document",
            ),
            (
                "limit",
                "integer",
                false,
                "1-500 changed layers per page, default 100",
            ),
            (
                "cursor",
                "string",
                false,
                "nextCursor from the previous page of the same pair and node",
            ),
        ],
    ),
    (
        "inspect",
        "Compact expanded node summary with path, the node's ancestors from the document down ([{id, name}] as in find; for a layer, path[1] is its page), so a pasted link's screen needs no find: bounds, layout, typography, text previews (fullText=true for complete text), Dev Mode annotation label previews and Ready for dev devStatus, and child counts. colors=true adds resolved solid fill colors with bound variables; other paints stay in nodes. A parent without auto-layout has childSpacing \"derived\": gaps between its children are coordinate differences, not an auto-layout rule; constraints may still pin them. For image decisions, images=true adds original dimensions/bytes and source-pixel rectangles before masks, not blur diagnoses. Depth and a 200-node budget bound the summary; image paints have their own 200-item limit. nodes returns full design data.",
        &[
            FILE,
            NODE,
            (
                "depth",
                "integer",
                false,
                "0-10 levels below the node, default 1; text previews up to 200 characters",
            ),
            (
                "images",
                "boolean",
                false,
                "include source bitmap dimensions/bytes and per-paint source-pixel crop rectangles; no files written",
            ),
            (
                "states",
                "boolean",
                false,
                "include bounded variants and prototype field evidence with exact sources/unknowns; retains instance overrides, infers no business conditions and renders no other variants",
            ),
            (
                "compareVariant",
                "string",
                false,
                "compare node with this other stored variant in the same component set; adds bounded raw-field differences, per-pair changes for children paired by unique name, main component or equal-count stacking order (check pairedBy; order is the weakest evidence), other descendants' value multisets, exact sources and bound variable names; no alternate rendering or business inference",
            ),
            (
                "compare",
                "string",
                false,
                "compare node with any other stored node of the file (another frame, breakpoint or state; no component set needed) with the same bounded raw-field differences, per-pair changes, unpaired nodes and exact sources as compareVariant, plus rootDelta (size and parent-relative position); children pair by unique name, then unique main component, then stacking order in equal-sized same-name groups (pairedBy), the rest are reported unpaired as ambiguous-name; instances are not expanded; an ancestor or descendant is rejected; adds comparison",
            ),
            (
                "fullText",
                "boolean",
                false,
                "return the complete characters of each TEXT node (up to 10000 characters, textTruncated beyond) instead of the 200-character preview; also applies to flat; the node and reply budgets still apply; default false",
            ),
            (
                "colors",
                "boolean",
                false,
                "add solidFills to nodes with visible SOLID fills (TEXT color, frame and shape fills): hex, opacity when the paint is below 1 (paint alpha x paint opacity, not ancestor or node opacity) and the bound color variableId/variableName; with flat, a fill shows a $variable suffix. Gradients, images, strokes, effects and vector geometry are not included (use nodes); default false",
            ),
            (
                "flat",
                "boolean",
                false,
                "one line per node instead of node JSON, for scanning a screen against an implementation: id, type, name, @x,y WxH relative to the inspected node, then hidden, rot, component, devStatus=READY_FOR_DEV|COMPLETED, annotations=N (a count; notes are in the JSON form), text, font=family size/weight/lineHeight, fill, stroke=paint/weight, r, visible effects (shadow|inner-shadow=x,y,blur[,spread],color, blur=, bg-blur=), H|V gap or GRID RxC gap=row,col rows=… cols=… (stored track sizes: flexN, px, hug or min..max; ? when unstored), pad=top,right,bottom,left, align=main/cross unless both min, size=w/h (fixed|fill|hug) unless both hug (or spacing=derived without auto-layout), span=rows,cols on grid children, opacity when set; children=N and '… N more' mark omitted nodes; same depth and node limits; not combinable with images, states or compareVariant; default false",
            ),
        ],
    ),
    (
        "nodes",
        "A node or exact instance sublayer as the per-node object of a Figma REST nodes response ({document, components, componentSets, styles, schemaVersion, _figlens}; _figlens.semantics.verification covers every node's _figlens.semantics): instances expanded, fills, strokes, effects, full text, auto-layout, masks, component data, Dev Mode annotations with labelMarkdown (named annotationCategories at the top level) and devStatus. Use it when inspect's compact summary lacks paints, masks or full text. A whole screen is often hundreds of KB, and agent hosts may show only part of a reply over about 20 KB, so read a screen with out, then query that file with jq or read it in parts; or scope to a smaller node, use inspect (flat for an outline), or write a bundle. Without out, replies over the MCP reply budget fail with response_budget_exceeded; with out, only a selection that outgrows that budget while being built does. Opt-in compact shares styles, and subtrees additionally shares templates; resolve the versioned references before passing to REST consumers. The reply budget applies to the compact result, with separate finite construction limits.",
        &[
            FILE,
            NODE,
            (
                "compact",
                "boolean",
                false,
                "lossless figlens.nodes.compact.v1 wrapper: data contains node style slots as {$style: index}, sharedStyles holds their literal values; default false; use for large full-paint reads, not REST consumers",
            ),
            (
                "subtrees",
                "boolean",
                false,
                "with compact: true, also share repeated subtrees; {$subtree: index, placements: [...]} references subtrees templates with preorder id/absoluteBoundingBox/absoluteRenderBounds overrides; default false",
            ),
            (
                "out",
                "string",
                false,
                "write the JSON (the same text the reply would carry) to this .json file (other extensions are refused) and reply {path, bytes, lines} instead. Unless the user names one, use .figlens/exports/ under the project root, created with a .gitignore containing * so it stays out of Git; create the parent directory first; never overwrites the source design",
            ),
        ],
    ),
    (
        "variables",
        "The variables a frame binds (collections, modes, aliases); without a node, every local variable. Opt-in format dtcg writes DTCG 2025.10 tokens: requires explicit collection and mode IDs plus out, takes no node, fails without publishing on any diagnostic. An alias into another collection exports its resolved value (aliasOf records the source), read in that collection's only mode or the aliasModes one. No workspace/index needed.",
        &[
            FILE,
            (
                "node",
                "string",
                false,
                "the node, including full instance sublayer IDs; omit for every local variable",
            ),
            (
                "format",
                "string",
                false,
                "rest (default) preserves variables JSON; dtcg exports one collection/mode",
            ),
            (
                "collection",
                "string",
                false,
                "exact collection ID from variables: a local one, or a library one the file has cached copies of (exported from them, marked libraryCopy); required only for dtcg",
            ),
            (
                "mode",
                "string",
                false,
                "exact modeId in that collection; required only for dtcg, no default",
            ),
            (
                "aliasModes",
                "object",
                false,
                "dtcg only: {collectionId: modeId} naming the mode each aliased collection is read in; an alias into a single-mode collection needs no entry. Such aliases export resolved values with aliasOf recording the first target",
            ),
            (
                "out",
                "string",
                false,
                "DTCG token file, preferably .tokens.json; required only for dtcg; create its parent directory first; never overwrites the source design",
            ),
        ],
    ),
    (
        "bundle",
        "Write a design bundle zip for one node or exact instance sublayer: nodes, variables, assets, reference render, context.md. When the user requests related component states, pass states=true to include their explicit variants and evidence gaps.",
        &[
            FILE,
            NODE,
            (
                "out",
                "string",
                true,
                "output path for the bundle ZIP; unless the user names one, use .figlens/exports/ under the project root, created with a .gitignore containing * so it stays out of Git; create the parent directory first",
            ),
            (
                "fileKey",
                "string",
                false,
                "Figma file key for the manifest",
            ),
            ("name", "string", false, "frame name for the manifest"),
            (
                "states",
                "boolean",
                false,
                "include design/states.json and a bounded context.md summary; retains inspect's source, unknown and truncation markers; default false",
            ),
            (
                "hashSource",
                "boolean",
                false,
                "record the .fig's sha256 as manifest.json source.sha256 (null when the file changed while bundling), to trace a built screen to the saved revision; reads the whole file once more; default false",
            ),
        ],
    ),
    (
        "verify",
        "Check a bundle, zipped or extracted, against its manifest before building from it: verified, missing and changed files, the bundle's missing assets, and the font faces the visible text uses (with node ids). Each font face is looked up by its name table in this machine's user and system font folders and in fonts (a project font folder): status local_file with localFiles {path, matchedBy, inProject} when a file is found (WOFF/WOFF2 by file name only), else unverified; a match inside fonts also carries a ready fontFace @font-face (src relative to fonts, font-display swap; licensing is the user's call, nothing is copied). local_file never shows the browser loads the face: confirm each in the running project with document.fonts.load, and check browser delivery of assets there. findings gathers these checks with the bundle's unresolved variables, truncated state inventory, render limitations and unemitted semantics into one handoff preflight list, each with node ids, evidence, impact and next action. Reports without blocking; no design file is read.",
        &[
            (
                "bundle",
                "string",
                true,
                "path to a bundle .zip or the directory it was extracted to",
            ),
            (
                "fonts",
                "string",
                false,
                "absolute path of a project directory of font files (subfolders included) searched before the system folders; must be inside the configured roots",
            ),
        ],
    ),
    (
        "freshness",
        "Tell whether saved .fig copies are older than their online Figma files, before concluding a design lacks a frame: the designs in a mapping (a JSON list of {name, url, file}, file relative to the project root for .figlens/sources.json, else to the mapping's folder), one file found in the nearest .figlens/sources.json at or above it, or one file with its url. Saved time is exported_at in the copy's meta.json; online time is last_touched_at from Figma's REST file metadata, read with an environment token (never an argument or output). The optional account overrides the mapping's account; named accounts resolve tokenEnv in the server user's figlens config/accounts.json, and unnamed entries keep FIGMA_TOKEN. Missing named config/token never falls back. Per-design account/tokenVar/tokenSet describe named routes; top-level tokenVar/tokenSet describe only default FIGMA_TOKEN. Each design is stale, current or unknown with a reason (no_token, account_not_configured, account_config_invalid, no_saved_time, not_mapped, file_missing, url_unrecognized, offline, token_rejected, file_not_found, rate_limited, curl_unavailable, outside_roots, unexpected_response, token_invalid, deadline when a listing ran out of its 60 s, cancelled). A stale copy needs the user to save a new local copy (File > Save local copy); nothing is downloaded. With a token, a node not found or an empty find in a mapped stale copy carries the same hint.",
        &[
            (
                "sources",
                "string",
                false,
                "path of a mapping JSON to list every design of; not with file",
            ),
            (
                "file",
                "string",
                false,
                "a local saved .fig to check alone; its mapping is the nearest .figlens/sources.json at or above it unless url is given",
            ),
            (
                "url",
                "string",
                false,
                "with file: the Figma file URL it was saved from, instead of a mapping",
            ),
            (
                "account",
                "string",
                false,
                "account alias overriding every checked entry, including file with url; 1-64 lowercase ASCII letters, digits, '-' or '_', starting with a letter or digit; tokenEnv is configured in the server user's accounts.json",
            ),
        ],
    ),
    (
        "export",
        "Export a node to SVG, PNG, JPG, WebP or AVIF. Create the output directory first. Returns metadata and, for raster formats, an inline image. PNG suits inline references: up to 4,194,304 pixels it is losslessly optimized with exact decoded RGBA, keeping original encoded bytes unless smaller; larger PNGs keep regular encoding. For website assets, follow the target project's existing image pipeline; otherwise start with WebP for bitmaps, sized for display dimensions and DPR using scale/maxSize and quality/maxKb. SVG embeds original images and can be large; keep it for vector artwork. When inlining or optimizing SVG, keep its root transform and unique ids (SVGO: prefixIds, not cleanupIds). The export covers what the node draws; bounds: layout exports the layout box, and warnings report when the two differ, any known skipped or approximated rendering in the subtree, as bundle warnings do, and images or components the file lacks. scale, quality and maxSize affect raster formats only; maxSize defaults to 1600. Check returned dimensions/bytes and isError; read content on failure.",
        &[
            FILE,
            NODE,
            (
                "out",
                "string",
                true,
                "output path ending in .png, .jpg, .jpeg, .webp, .avif or .svg; shipped assets follow the project's asset conventions, reference and scratch files go in .figlens/exports/ under the project root, created with a .gitignore containing * so it stays out of Git; create the parent directory first",
            ),
            (
                "bounds",
                "string",
                false,
                "render (default): what the node draws, shadows, outside strokes and overflowing children included, as Figma's Export panel; layout: the node's own box (absoluteBoundingBox), cutting off what is drawn past it",
            ),
            ("scale", "number", false, "default 1"),
            ("quality", "integer", false, "1-100, default 80"),
            (
                "maxKb",
                "integer",
                false,
                "KiB (1024 bytes) budget; lower lossy quality until it fits; PNG/SVG fail if over budget",
            ),
            (
                "maxSize",
                "integer",
                false,
                "longest side in pixels, default 1600; 0 keeps the scaled size",
            ),
            (
                "matchDir",
                "string",
                false,
                "SVG output only: a folder of SVG files, subfolders included, to compare with the export, to find an icon the project already has; matches lists the files (relative paths) that draw the same outlines once both are moved to the origin, ignoring paint, ids and path syntax; similar lists up to 10 others with a similar shape (drawn with strokes, or the same outlines at another size, which score 1) as {file, score} from 0.8, best first; confirm a similar file by looking at both",
            ),
            (
                "hide",
                "string",
                false,
                "comma-separated ids of layers inside node to draw as hidden, over the same area (reply lists hidden); compare with the plain export to see where that layer, as figlens draws it, changes the result; no difference does not prove its skipped rendering harmless (it may match what is beneath it)",
            ),
        ],
    ),
    (
        "export_batch",
        "Export many nodes of one file in a single call, one file per item; use it instead of repeated export calls for a screen's icons and photos. Each item takes the options export takes (node, out, bounds, scale, quality, maxKb, maxSize, hide; the format follows out's extension) except matchDir. Create the output directories first. At most 64 items, distinct out paths, all inside the configured roots; a malformed item or a path outside the roots fails the whole call before anything is written. Items then export in order: one that fails (a missing or hidden node, a file over maxKb) is reported with its error and the others still export, so check each entry's error. Returns a manifest, no inline images: per item node, path, width, height, bytes, quality, layoutBox, renderBox and warnings, or node, path and error; warnings say when the drawn area reaches past the layout box (a badge or shadow), as export's do. Once the files written reach 256 MiB, the remaining items fail with a budget error; export them in another call.",
        &[
            FILE,
            (
                "items",
                "array",
                true,
                "1-64 objects {node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}, each as the export tool's parameters of those names",
            ),
        ],
    ),
    (
        "compare_image",
        "Compare a PNG screenshot of the built screen with the node as figlens renders it, to check visual fidelity with a tool instead of by eye. The node is drawn over its layout box at the screenshot's scale (screenshot width / node width, or scale), composited over white; a pixel differs when no pixel within one CSS pixel in the other image is within the threshold (48/255 per channel by default), so anti-aliasing does not count, but differences under it (light borders, subtle tints) are not seen. Differing pixels are grouped into regions on an 8 px grid; regions under 12 px² are dropped. Returns sizes, scale, mismatch (differing pixels / compared pixels), nonTextMismatch and textMismatch, nonImageMismatch and imageMismatch, and the largest regions in node pixels from its top-left, each with the deepest layers under it (id, name, type) and text: true when nearly all of it lies over TEXT layers (often font fallback or line breaks, but also a wrong text color, size or copy: review each) and image: true when nearly all of it lies only over image-filled layers (content such as banners or avatars, often from data). ignore leaves listed layers' render boxes out of every count and region, for content that differs by data rather than layout (their own size and position are then unmeasured); the reply lists them. threshold lowers the 48 default to see light borders and dividers, at the cost of more anti-aliasing noise. A screenshot taller than the node is compared over the node's height and the rest reported as uncompared; a shorter one, or a width that does not match scale, is refused, unless fullPage states the shorter one is the whole page: then its rows are compared and the node rows it lacks (past 1 px rounding) are reported as uncompared.missing and counted as one differing region over the layers there. warnings name what figlens skipped or approximated in the node, where a difference may be figlens's. out writes a diff PNG: the screenshot washed out, differing pixels red, ignored pixels gray, regions outlined red (layout), blue (text) or purple (image). Capture the screenshot at the node's size with fonts loaded; a viewport capture of a taller page needs a full-page screenshot.",
        &[
            FILE,
            NODE,
            (
                "image",
                "string",
                true,
                "PNG screenshot of the implemented screen, cropped to the node (a full-page capture may be taller); at most 64 MiB and 25,000,000 pixels",
            ),
            (
                "scale",
                "number",
                false,
                "screenshot pixels per node pixel, such as the device pixel ratio; default: the screenshot width / the node width",
            ),
            (
                "limit",
                "integer",
                false,
                "most regions listed, largest first, 1-50, default 10; counts cover all regions",
            ),
            (
                "out",
                "string",
                false,
                "diff PNG path ending in .png, in .figlens/exports/ under the project root unless the user names one; create the parent directory first; never the screenshot or the design",
            ),
            (
                "ignore",
                "string",
                false,
                "comma-separated ids of layers inside node, such as data-driven images or user text, whose render boxes are left out of mismatch, its splits and regions; the reply lists them under ignored",
            ),
            (
                "threshold",
                "integer",
                false,
                "per-channel difference (of 255) above which pixels differ, 1-254, default 48; lower it (such as 16) to see a light border, divider or shadow the default misses",
            ),
            (
                "fullPage",
                "boolean",
                false,
                "the image is a full-page capture of the whole page: one shorter than the node is compared over its height and the missing rows reported as a differing region, a height finding; leave false for a viewport capture, which is then refused",
            ),
        ],
    ),
    (
        "index",
        "Local workspace indexes for search: prepare registers a .fig (file) in its own workspace, or in a project workspace (workspace: a project directory), and with scope starts a background build of exactly that stored scope; status reports sources, indexed scopes and storage; add, remove and relocate change the registered sources (remove withdraws their indexes; add of a directory registers up to 10 .fig files under it in one step, in path order, never through a link leading out of it, and replies directory {added, existing, skipped, notFig, truncated, nextCursor}; pass nextCursor as cursor to add the next 10; a repeat adds nothing, and a moved file is a new source unless relocated); refresh rebuilds changed scopes and rebuild every indexed scope even when unchanged (sourceId and scope narrow both), while the previous index keeps serving; track starts recording one scope's changes, read with the changes tool, with a baseline build, and untrack stops it, keeping its history; embed runs the configured local model (models tool) over published scopes for semantic search, as jobs beside the text builds (mode_unavailable without a profile), and a text build this server runs queues it after publishing; visual (scope required, never widened to the whole file) renders that published scope's screens and components, and the scope node below a page, for image search, and embeds them when a vision profile is configured (hash-only otherwise), as a job on the model lane beside text builds; called again it resumes a partial layer, retrying units that failed; a scope that had a visual layer is rendered again after a text build this server runs republishes it, and no other scope is rendered implicitly; a reply that could not admit every job has admission partial and deferred items, each with the nextStep that builds it later; clear_cache deletes only what a build can recreate; reset deletes this workspace's database after the user confirmed it, keeping every .fig and export. Builds run as jobs: the reply has job {jobId, state}; poll the jobs tool, cancel there. Nothing is built implicitly: connecting, listing, find, inspect and file search create no database. Errors carry _meta.figlens.recovery with only the actions that apply; recreate_elsewhere and reset need the user's confirmation.",
        &[
            (
                "action",
                "string",
                true,
                "prepare, status, add, remove, relocate, refresh, rebuild, track, untrack, embed, visual, clear_cache or reset",
            ),
            (
                "file",
                "string",
                false,
                "a local .fig: the source to prepare, add or relocate to (add also takes a directory under the roots, or without --root a gs://bucket/object, downloaded pinned to its generation); also selects its source-scoped workspace",
            ),
            (
                "workspace",
                "string",
                false,
                "a project directory: selects or prepares that project's workspace",
            ),
            WORKSPACE_ID,
            (
                "sourceId",
                "string",
                false,
                "a registered source, for remove, relocate, refresh, rebuild, track, untrack, embed and visual",
            ),
            (
                "scope",
                "string",
                false,
                "stored node id to index (prepare), refresh, rebuild, track, untrack, embed or visual (required); 0:0 is the whole file",
            ),
            (
                "requestId",
                "string",
                false,
                "an idempotency key: repeating a prepare, refresh, rebuild, track, embed or visual of one source and scope with it returns the same job; one first used for another request (track is its own) is refused",
            ),
            STORAGE,
            (
                "replaceUnavailable",
                "boolean",
                false,
                "prepare only, after the user confirmed: recreate a workspace whose storage is unavailable, denied or unreadable in storage, knowing only file; the old files stay untouched",
            ),
            (
                "cursor",
                "string",
                false,
                "add of a directory only: directory.nextCursor of the previous add of it; a cursor of another directory is cursor_stale",
            ),
        ],
    ),
    (
        "jobs",
        "Status or cancellation of an index job. status reports state (queued, running, succeeded, failed, cancelled, interrupted), stage, progress and the error or published generation; cancel stops the build and its worker process before reporting cancelled, and a job already publishing finishes (too_late). Responds while builds and searches run.",
        &[
            ("action", "string", true, "status or cancel"),
            ("jobId", "string", true, "the jobId from an index reply"),
            STORAGE,
        ],
    ),
    (
        "models",
        "Local model profiles: bge-m3 (text) for semantic and hybrid search, and siglip-base (image) for search with image or imageRef. list shows the documented profiles with modality, model, revision, license and the files each needs; status reports each configured profile ready or unavailable with the reason, and the setup steps for each modality no profile serves; configure checks a model directory the user already downloaded against pinned SHA-256 digests, starts the figlens-embed shipped beside figlens once offline and records the profile; remove forgets the profile and keeps its files. Nothing is downloaded and no design content leaves the machine.",
        &[
            (
                "action",
                "string",
                true,
                "list, status, configure or remove",
            ),
            (
                "profile",
                "string",
                false,
                "configure and remove: a profile name from list",
            ),
            (
                "modelDir",
                "string",
                false,
                "configure: the local directory holding the profile's files, inside the server's roots",
            ),
        ],
    ),
    (
        "changes",
        "Recorded design changes of the scopes index track follows; never writes or builds. A query subscription's subscriptionId instead pages its stored search evaluations (evaluations, evaluation, tracking; no revision pages); ack them the same way. Its evaluation is current, current_for_index (the file changed or is gone since that index: refresh, then evaluate), pending with an evaluate step, or unavailable (also when its last evaluate at this publication failed, with that reason, until a later publication); a stored evaluation too large or damaged to page fails with a recovery that acks past it. Each revision is one published build of a tracked scope: kind baseline (tracking started, or nothing comparable before it, with a reason: tracking_started, tracking_changed, previous_changed, previous_untracked, previous_unpublished, facet_version_changed, or index_version_changed when the previous build used other index versions, such as relations before library references) or changes, with coverage, exact summary counts per change kind and facet, and detail complete, truncated or expired. A changes revision's impact says which layers its changes reach: complete, reasons with counts (impact_truncated: layers left out of its detail) and listed, the count per class (direct, effective, effective_unchanged, possible; a class judges the layer's own fields, not its children, which have their own changes) of the layers its detail listed when recorded, never a total (an expired detail no longer keeps them); it is null for a baseline or a revision recorded before impact, never an empty complete one. A node subscription also reads revisions whose changes reach that layer or a layer of that top-level frame, or whose impact is incomplete. Pages run oldest first after cursor, else after the consumer's checkpoint (moved by watch ack), else from the oldest kept. Pass revision to page that revision's changed nodes, then the layers its impact lists, instead; each entry's entry field says which (change or impact). tracking lists each tracked scope's state, lastRevisionId and refreshRequired (the file changed since its last build: refresh the scope to record it; an empty page never proves the file unchanged); monitor is the watch monitor, separate from tracking. A cursor is bound to the filters, the visible sources and the workspace incarnation: any other use is cursor_stale. event_gap means retention deleted revisions before they were read: follow its recovery and treat the scope as possibly changed.",
        &[
            ("workspaceId", "string", true, "ws_... from the index tool"),
            ("sourceId", "string", false, "only this source's revisions"),
            ("scope", "string", false, "only this scope's revisions"),
            (
                "consumer",
                "string",
                false,
                "without cursor, start after this consumer's checkpoint",
            ),
            ("cursor", "string", false, "nextCursor of the previous page"),
            (
                "subscriptionId",
                "string",
                false,
                "read with this subscription's filters and its own checkpoint; pass no sourceId, scope or consumer with it",
            ),
            (
                "revision",
                "string",
                false,
                "a revisionId: page its changed nodes, then its impact's listed layers, instead",
            ),
            (
                "limit",
                "integer",
                false,
                "revisions per page, 1-50 (default 20); with revision, entries per page, 1-500 (default 100)",
            ),
            STORAGE,
        ],
    ),
    (
        "watch",
        "Watching tracked scopes and change-tracking consumers. start: this server holds a lease on tracked scope of one source (sourceId when the workspace has several) and the workspace's shared runner refreshes it on each save, so changes shows new revisions without an index call; the server renews its leases every 10 s and ends them when it exits, and an unrenewed lease ends after 30 s. A requestId replays this server's live lease and reports an ended one as ended (untracked when its scope was untracked or retracked since), never restarting it, for 1 h after the end among the newest 1024 ended leases; past that it is unknown and starts a new lease. A requestId of a lease another running process holds is refused. stop ends one lease this server holds (leaseId). status lists the workspace's leases and runner, read-only. ack moves consumer's checkpoint for these filters (sourceId, scope), or a subscription's own (subscriptionId), forward to cursor, the nextCursor of the last changes page it handled with the same filters; an older or equal cursor normally leaves it, so a retry is safe. A cursor issued before retention deleted revisions it had not passed is event_gap, writing nothing: follow its recovery. A current recovery cursor before a checkpoint that is behind such a deletion moves the checkpoint back to it, so the kept revisions replay. Replies with advanced, true only when the checkpoint moved forward, and the checkpoint now held, which is authoritative. subscribe follows one source, scope or exact node (node: a layer id that must name one layer kind in the published index, else target_missing or identity_uncertain, never a same-name layer) from its newest visible revision; a repeat returns the same subscription; owner marks it temporary for that task. subscribe with query (and type) saves a workspace search of one source's scope (default 0:0) and stores its first evaluation of the top 50; evaluate stores a newer one after a publication, and a watch lease on that scope makes its runner store it unasked; changes with its subscriptionId reads the stored evaluations (causes such as source_changed only for new file bytes or index_rebuilt; enteredTopK and leftTopK are exact sourceId/snapshotId/nodeId refs in the new and old snapshot, rankChanged the new snapshot's refs with from and to positions, null when rankComparable is false, all within the top-50 window, not the whole result set; nodes compare by sourceId, which relocate keeps; total only when the search counted every hit) and reports evaluation pending with an evaluate step, never computing one; an evaluation past 128 KiB is unavailable evaluation_too_large, nothing stored. sources (instead of sourceId) saves the search, or a library question (library), over a fixed set: visible resolves once to the sources this caller sees now, or a comma-separated list, at most 10, returned sorted in subscription.sources; it never gains a source added later. A member's publication evaluates it (its runner too); causes source_changed names members with new bytes in delta.changedSourceIds, corpus_changed a member's relocation or publication, or for a ranked search of several members their shared statistics; refs carry their sourceId and snapshot. A library question lists up to 50 uses with definitionMatch against the definition, followed by its exact nodeId in its source's current snapshot (delta.definitionMatchChanged); coverage.transitive is not_followed: a definition's own library dependencies are never traversed, and delta.notFollowedSourceIds names changed members that hold no use and are not the definition's source, whose dependent uses may be stale. A caller who cannot see every member reads the subscription as unknown; removing a member source disables it (state disabled, disabled.reason source_removed, evaluations deleted; a damaged stored set reads disabled with sources_unreadable): unsubscribe, then subscribe again. list pages the visible subscriptions with their state (subscriptionId: only that one, empty when unknown to the caller). unsubscribe removes one subscription and its checkpoint (or, with owner, that owner's temporary ones); forget removes a consumer's raw checkpoint views. Neither untracks or deletes history. A consumer holds at most 64 subscriptions and checkpoints, a workspace 4096: past that, workspace_busy names list, unsubscribe and forget; nothing is evicted.",
        &[
            (
                "action",
                "string",
                true,
                "start, stop, status, ack, subscribe, evaluate, list, unsubscribe or forget",
            ),
            ("workspaceId", "string", true, "ws_... from the index tool"),
            (
                "consumer",
                "string",
                false,
                "the consumer's name, 1-128 bytes: ack without subscriptionId, subscribe and forget need it; list narrows to it",
            ),
            (
                "cursor",
                "string",
                false,
                "ack: nextCursor from changes with the same filters",
            ),
            (
                "sourceId",
                "string",
                false,
                "start: the source to watch; ack: the sourceId filter of that changes call; subscribe: the source to follow; forget: only its views; evaluate takes none (it evaluates the subscription's own source)",
            ),
            (
                "scope",
                "string",
                false,
                "start: the tracked scope to watch; ack: the scope filter of that changes call; subscribe: only this scope; forget: only its views",
            ),
            (
                "subscriptionId",
                "string",
                false,
                "ack, evaluate or unsubscribe this subscription; list reads only it",
            ),
            (
                "query",
                "string",
                false,
                "subscribe: save this workspace search of the scope instead of following revisions",
            ),
            (
                "type",
                "string",
                false,
                "subscribe with query: only these node types, comma-separated, as search takes them",
            ),
            (
                "sources",
                "string",
                false,
                "subscribe with query or library, instead of sourceId: a fixed set, visible (the sources you see now) or comma-separated sourceIds, at most 10",
            ),
            (
                "library",
                "object",
                false,
                "subscribe: save this library question over sources (or sourceId) instead of a search: {kind, namespace, key} and optional definition {sourceId, snapshotId, nodeId}, an exact ref to the definition's current published snapshot in one of the sources",
            ),
            (
                "node",
                "string",
                false,
                "subscribe: follow this exact layer",
            ),
            (
                "owner",
                "string",
                false,
                "subscribe: a task-owned temporary subscription; unsubscribe (with consumer): remove that owner's temporary ones",
            ),
            (
                "after",
                "string",
                false,
                "list: the next of the previous page",
            ),
            STORAGE,
            (
                "requestId",
                "string",
                false,
                "start: replays this request's lease",
            ),
            ("leaseId", "string", false, "stop: the lease to end"),
        ],
    ),
];

/// The MCP outputSchema of the tools whose CLI command prints one JSON object.
pub fn output_schema(tool: &str) -> Option<serde_json::Value> {
    if tool == "variables" {
        return Some(json!({
            "type": "object",
            "properties": {
                "variables": {"type":"array"}, "collections": {"type":"array"},
                "missingVariables": {"type":"array"},
                "path": {"type":"string"}, "collectionId": {"type":"string"},
                "modeId": {"type":"string"}, "tokenCount": {"type":"integer"},
                "diagnostics": {"type":"array"}
            },
            "oneOf": [
                {"required":["variables","collections","missingVariables"]},
                {"required":["path","collectionId","modeId","tokenCount","diagnostics"]}
            ]
        }));
    }
    let (properties, required) = match tool {
        "find" => (
            serde_json::json!({
                "root": {"type": "string"},
                "results": {"type": "array", "items": {"type": "object"}},
                "truncated": {"type": "boolean"},
                "freshness": {"type": "object", "description": "only on no results in a mapped copy known to be stale: the freshness design report plus hint"},
            }),
            ["root", "results", "truncated"].as_slice(),
        ),
        "freshness" => (
            serde_json::json!({
                "mapping": {"type": ["string", "null"]},
                "tokenVar": {"type": "string", "description": "legacy default FIGMA_TOKEN only, not an aggregate of named accounts"},
                "tokenSet": {"type": "boolean", "description": "whether default FIGMA_TOKEN is nonempty"},
                "designs": {"type": "array", "items": {"type": "object"}, "description": "name, file (as mapped or given), savedAt, onlineModifiedAt, onlineVersion, state stale|current|unknown, reason when unknown; named routes add account, tokenVar (null when unresolved), tokenSet; never token values"},
            }),
            ["mapping", "tokenVar", "tokenSet", "designs"].as_slice(),
        ),
        "search" => (
            serde_json::json!({
                "query": {"type": "object", "description": "the query as given, its normalized phrase, mode and the whole tokens actually searched; an image search adds image (kind path|bytes|ref and crop, never the path), requestedMode and usedModes (text, semantic, visual_hash, visual_exact, visual_embedding)"},
                "source": {"type": "object", "description": "sourceId (opaque, from this path), snapshotId (sha256 of the exact .fig bytes decoded) and freshness: file-stamp reuse, not a per-query disk hash"},
                "scope": {"type": "object"},
                "ranking": {"type": "object", "description": "ranking and tokenizer versions, field weights and tie order; an image search names its fusion (visual-rrf-1), channels and candidatesPerChannel"},
                "results": {"type": "array", "items": {"type": "object"}, "description": "ranked rows; in an image search each match names its basis (lexical, semantic, normalized_pixels, perceptual_hash, visual_embedding) and each row has visual {state rendered|failed|absent, bounds, diagnostics, diagnosticsTruncated, error}"},
                "coverage": {"type": "object", "description": "complete, scannedRecords, totalRecords (null unless complete), layers.text.state ready|partial, layers.visual in image searches (hash and embedding state per source, ready|partial|pending|building|failed|stale|unavailable, and the approximate signal), layers.semantic in semantic and hybrid modes (state ready|partial|pending|unavailable, profileId, counts, reasons; workspace searches per source), layers.relations (per kind: ready or partial when built; stateless search builds contains, instance_of and variant_of and lists the others not_built, which says nothing about absence; library_identity relation_unavailable), reasons with counts, excluded hidden/internal matches, knownGaps and work limits"},
                "hasMore": {"type": "boolean"},
                "nextCursor": {"type": ["string", "null"]},
                "workspaceId": {"type": "string", "description": "workspace searches only"},
                "state": {"type": "string", "description": "workspace searches: ready, pending or not_prepared"},
                "sources": {"type": "array", "items": {"type": "object"}, "description": "workspace searches: each visible source's sourceId, state, snapshotId, freshness and active job"},
                "nextSteps": {"type": "array", "items": {"type": "object"}, "description": "workspace searches: the index or jobs calls that make more sources ready"},
            }),
            ["query", "results", "coverage", "hasMore", "nextCursor"].as_slice(),
        ),
        "usages" => (
            serde_json::json!({
                "library": {"type": "object", "description": "library queries: the kind, namespace and key asked"},
                "definition": {"type": ["object", "null"], "description": "library queries: state unresolved with a reason (library_source_unproven when none is selected), or selected with basis caller_ref, the selected ref and keyEvidence unverified; null while a workspace scope is pending or not_prepared; else ref or resource, state (indexed in scope, outside_scope, or the state of the edges that name it: missing, unresolved, external, recorded), name, type, setId and page when known"},
                "direction": {"type": "string"},
                "relations": {"type": "array", "items": {"type": "string"}},
                "scope": {"type": "object"},
                "members": {"type": "object", "description": "COMPONENT_SET queries: the known member ids and whether they are complete"},
                "results": {"type": "array", "items": {"type": "object"}, "description": "in: each location's ref, name, type, origin, hidden, page, scopeRoot, path, relation, state, basis, evidence and via (the member a set query matched); library queries: ref, name, type, relation, state, basis, evidence, hidden, internal, page and scopeRoot, no origin or path, plus definitionMatch when a definition is selected: equal (the use's cached copy has the selected definition's fingerprint on every facet; not proof the library published it), definition_mismatch (a facet differs) or unknown (a facet unread or unsupported, such as a field holding a file-local GUID, or no copy to compare); out: target ref or resource, its definition and the edge fields, plus library {kind, namespace (null with unknownReason when no copy records one), key, basis cached_copy} on an instance_of, uses_style or binds_variable use whose cached library copy names it: the selector a library query takes"},
                "coverage": {"type": "object", "description": "complete, layers.relations kinds and version, reasons; library queries: complete, sources (sourceId, path, state, snapshotId, freshness when ready, nextSteps) and reasons; never a total count"},
                "hasMore": {"type": "boolean"},
                "nextCursor": {"type": ["string", "null"]},
                "source": {"type": "object", "description": "sourceId and snapshotId; stateless calls add freshness"},
                "workspaceId": {"type": "string", "description": "workspace queries only"},
                "state": {"type": "string", "description": "workspace queries without a ready index: pending or not_prepared"},
                "nextSteps": {"type": "array", "items": {"type": "object"}, "description": "the jobs or index call that makes the scope ready"},
            }),
            ["definition", "results", "coverage", "hasMore", "nextCursor"].as_slice(),
        ),
        "diff" => (
            serde_json::json!({
                "source": {"type": "object", "description": "old and new sourceId and snapshotId, and freshness.reusedPair"},
                "node": {"type": "string"},
                "matching": {"type": "object", "description": "basis node_id+kind; lineage unverified"},
                "coverage": {"type": "object", "description": "complete; old and new stop and scannedRecords; changes (layers listed across pages, added/removed subtrees counted at their top), possibleMatches and reasons (a side's stop, diff_work_limit, unread_fields, relation_unverified, hidden_content_not_expanded, selection_context_unknown, definition_not_compared for styles, variables, main components and component sets outside the compared records, including variables a compared variable aliases, its collection, explicit variable modes inherited from above the selected node, and Dev Mode state stored above it: a page's Ready for dev status of a compared layer and the document's annotation categories a compared annotation names)"},
                "frames": {"type": "array", "items": {"type": "object"}, "description": "id, name, page, change when the frame itself changed, summary (N listed, then counts per facet and unread), possibleMatch {id, basis, identity possible_match}, and this page's nodes: id, kind, name, change, facets, fields {key: {facet, old, new}}, edges {added, removed, truncated count, swap {old, new} or {truncated}}, descendants, identity_uncertain with oldKind; unread with fields when a field could not be read (change unknown when nothing else proves a difference)"},
                "nextCursor": {"type": ["string", "null"]},
            }),
            ["source", "coverage", "frames", "nextCursor"].as_slice(),
        ),
        "index" => (
            serde_json::json!({
                "workspaceId": {"type": "string"},
                "state": {"type": "string", "description": "initialized, ready, absent or reset"},
                "schemaVersion": {"type": "integer"},
                "incarnation": {"type": "string", "description": "changes when the database is recreated; cached results of another incarnation are void"},
                "sources": {"type": "array", "items": {"type": "object"}, "description": "visible sources: sourceId, path, present and indexed scopes with snapshotId, records, complete and current; a gs:// source adds remote {uri, generation its download was pinned to}, not proof the object is unchanged"},
                "job": {"type": "object", "description": "the build prepare started or joined: jobId, state, stage, progress"},
                "jobs": {"type": "array", "items": {"type": "object"}, "description": "refresh, rebuild, track, embed and visual: one job per admitted source and scope, each with joined"},
                "admission": {"type": "string", "description": "refresh and rebuild: complete, or partial when some jobs were not admitted"},
                "deferred": {"type": "array", "items": {"type": "object"}, "description": "partial admission: sourceId, scope and the nextStep index call that builds it later"},
                "deferredBecause": {"type": ["object", "null"], "description": "partial admission: the error that stopped admission, {code, message}"},
                "coverage": {"type": "object", "description": "prepare without scope: text and relations not_requested"},
                "directory": {"type": "object", "description": "add of a directory: path, added and existing [{sourceId, path}], skipped [{path relative to it, reason escaping_link, linked_directory, unreadable, inside_storage or outside_roots}], notFig count, truncated null or {reason source_cap or entry_budget, limit}, nextCursor for the next 10 or null"},
                "partial": {"type": "boolean", "description": "clear_cache: true when a source left this caller's view meanwhile and kept its data; retry later"},
                "nextSteps": {"type": "array", "items": {"type": "object"}},
                "storage": {"type": "object"},
                "paths": {"type": ["object", "null"], "description": "workspace, database and lock paths (the lock sits beside the workspace directory); null when the workspace does not exist"},
            }),
            ["workspaceId", "state"].as_slice(),
        ),
        "models" => (
            serde_json::json!({
                "profiles": {"type": "array", "items": {"type": "object"}, "description": "list: name, modality, model, revision, license, files, configured, profileId, dataFlow; status: name, modality, profileId, model, runtime, dim, state ready or unavailable, code (runtime_missing, runtime_changed or model_missing), reason and recovery (models configure with the same files)"},
                "setup": {"type": "array", "items": {"type": "object"}, "description": "status: the steps to configure a profile for each modality (text, image) no configured profile serves"},
                "configRoot": {"type": "string", "description": "status: the config directory models.json lives in (ask write access to it before configure)"},
                "defaultRuntime": {"type": "object", "description": "status: {path, available} of the figlens-embed beside this figlens; available means the file exists, not that a model is ready"},
                "configured": {"type": "object", "description": "configure: the recorded profile and its profileId"},
                "removed": {"type": "boolean"},
            }),
            [].as_slice(),
        ),
        "jobs" => (
            serde_json::json!({
                "jobId": {"type": "string"},
                "workspaceId": {"type": "string", "description": "the workspace the job builds in, for index status"},
                "kind": {"type": "string", "description": "prepare, refresh, rebuild, track, embed (the model layer of a published scope) or visual (its rendered and, with a vision profile, embedded units)"},
                "state": {"type": "string", "description": "queued, running, succeeded, failed, cancelled or interrupted (its owner exited)"},
                "stage": {"type": ["string", "null"], "description": "null until the job starts"},
                "progress": {"type": "object"},
                "cancel": {"type": "string", "description": "cancel only: accepted, pending, too_late or not_active"},
                "error": {"type": ["object", "null"]},
                "evictedScopes": {"type": "array", "description": "a publish that retired the source's least recently published scope (past 8) names it"},
                "nextSteps": {"type": "array", "items": {"type": "object"}},
            }),
            ["jobId", "state"].as_slice(),
        ),
        "changes" => (
            serde_json::json!({
                "workspaceId": {"type": "string"},
                "revisions": {"type": "array", "items": {"type": "object"}, "description": "oldest first: revisionId, sourceId, scope, kind (baseline or changes), from (the previous revisionId, null for a baseline or when deleted), snapshotId, coverage (a baseline's reason, such as tracking_started or index_version_changed), summary (changes, kinds, facets, possibleMatches), detail (complete, truncated or expired), detailRows, impact (null, or complete, reasons [{reason, count}], listed per class)"},
                "nextCursor": {"type": ["string", "null"], "description": "pass as cursor for the next page, or to watch ack once handled"},
                "hasMore": {"type": "boolean"},
                "tracking": {"type": "array", "items": {"type": "object"}, "description": "each tracked scope of the visible sources: sourceId, scope, state (baseline_pending, active or disabled), lastRevisionId, lastCheckedAtMs (when its last build verified the file), refreshRequired (the file's size or mtime differs from that build's), model (its published generation's semantic layer under the active profile: state ready|partial|pending|unavailable, profileId, policy, generation; coverage only, never a revision)"},
                "trackingTruncated": {"type": "boolean", "description": "true when more tracked scopes exist than tracking lists"},
                "monitor": {"type": "object", "description": "the watch monitor's state, separate from tracking: inactive when no lease covers the view, partial when some tracked scope of the view has none, else watching or the first problem (unauthorized, source_busy, source_missing, source_unreadable, refresh_failed, stale); source_busy then source_unreadable also when the source path now leads elsewhere through a link, which is never read: relocate the source"},
                "revisionId": {"type": "string", "description": "with revision"},
                "detail": {"type": "string", "description": "with revision: complete, truncated (rows or impact entries past the cap were not kept) or expired (neither is kept)"},
                "nodes": {"type": "array", "items": {"type": "object"}, "description": "with revision: its change rows (entry change: frame, id, kind, change, facets, unread, descendants), then its impact's listed layers (entry impact: id, class, sides, facets {changed, unknown, equal}, via {relation, target} or null)"},
                "subscription": {"type": "object", "description": "with subscriptionId: as watch subscribe describes it"},
                "evaluations": {"type": "array", "items": {"type": "object"}, "description": "a query subscription's stored evaluations, oldest first: evaluationId, snapshotId, coverage, causes (initial, source_changed, index_rebuilt, coverage_changed, ranking_changed, corpus_changed), total (null unless every hit was counted), topK, delta (from evaluationId, fromSnapshotId, snapshotId; enteredTopK and leftTopK as exact sourceId/snapshotId/nodeId refs in the new and old snapshot; rankChanged within the window as new-snapshot refs with from and to, null when rankComparable is false; a source set's also sources, fromSources and changedSourceIds, a library question's definitionMatchChanged and notFollowedSourceIds), createdAtMs"},
                "evaluation": {"type": "object", "description": "a query subscription: state current, current_for_index (the file changed or is unreadable since that index: refresh, then evaluate), pending (with reason and an evaluate step), unavailable, or disabled (a source set: subscription.disabled.reason is source_removed, a member was removed, or sources_unreadable, its stored set is damaged; no evaluations: unsubscribe, then subscribe again)"},
            }),
            ["workspaceId", "nextCursor"].as_slice(),
        ),
        "watch" => (
            serde_json::json!({
                "workspaceId": {"type": "string"},
                "leaseId": {"type": "string", "description": "start, stop: the lease"},
                "sourceId": {"type": "string", "description": "start: the leased source"},
                "scope": {"type": "string", "description": "start: the leased scope"},
                "state": {"type": "string", "description": "start, stop: active, or ended with a reason"},
                "reason": {"type": "string", "description": "start, stop: why an ended lease ended (stopped, released, expired, start_failed, reset, source_removed)"},
                "expiresAtMs": {"type": "integer", "description": "start: when the lease ends unless renewed"},
                "renewEveryMs": {"type": "integer", "description": "start: how often the holder renews; MCP renews for you"},
                "runner": {"type": "object", "description": "start, status: the workspace's runner: state running, stopping, stopped or unknown; placement independent, or in_host_job when a Windows job object kept it inside the host and it ends with that job"},
                "leases": {"type": "array", "items": {"type": "object"}, "description": "status: leaseId, sourceId, scope, state (active, unauthorized, untracked, expired or ended, with reason), expiresAtMs, mine; live leases first, then the newest ended ones"},
                "truncated": {"type": "boolean", "description": "status: true when older ended leases are left out"},
                "consumer": {"type": "string"},
                "advanced": {"type": "boolean", "description": "ack: true only when the visible checkpoint moved forward"},
                "checkpoint": {"type": ["string", "null"], "description": "ack, subscribe: a cursor at the checkpoint now held; null when retention deleted that revision"},
                "subscriptionId": {"type": "string", "description": "ack of a subscription"},
                "subscription": {"type": "object", "description": "subscribe: subscriptionId, consumer, sourceId (null for a set), sources, scope, kind (source, scope, node or query), node, nodeKind, query, types, library, definition, topK, state (active, target_missing, identity_uncertain or disabled), disabled, lifetime (durable or temporary), filterVersion"},
                "evaluation": {"type": "object", "description": "subscribe with query or library, evaluate: state evaluated (evaluationId, causes, delta), current, pending (reason, nextSteps), unavailable (reason such as query_changed, search_failed or evaluation_too_large, nextSteps) or disabled (nextSteps)"},
                "created": {"type": "boolean", "description": "subscribe: false when the same subscription already existed"},
                "initialPosition": {"type": "string", "description": "subscribe: tail (the newest revision it can see), initial_evaluation (a query, before its first evaluation) or existing"},
                "tracking": {"type": "array", "items": {"type": "object"}, "description": "subscribe: the followed scopes' tracking state"},
                "subscriptions": {"type": "array", "items": {"type": "object"}, "description": "list: one page, as subscription"},
                "next": {"type": ["string", "null"], "description": "list: pass as after for the next page"},
                "removed": {"type": "integer", "description": "unsubscribe, forget: rows removed"},
                "subscriptionIds": {"type": "array", "items": {"type": "string"}, "description": "unsubscribe: the removed subscriptions' ids"},
                "keptUnknownOwnership": {"type": "integer", "description": "forget: views written before their sources were recorded, kept because this caller does not see every source"},
            }),
            ["workspaceId"].as_slice(),
        ),
        "inspect" => (
            serde_json::json!({
                "flat": {"type": "string", "description": "the one-line-per-node text when flat=true, instead of node"},
                "node": {"type": "object", "description": "Compact node with partial layout semantics and type-only property definitions; childSpacing \"derived\" marks a parent without auto-layout; _figlens.semantics records source and state (a missing entry means the field is not stored); the top-level _figlens.semantics.verification applies to all nodes. Definitions, definition evidence, annotations (plain label previews, labelTruncated when cut; at most 20 properties each) and _figlens.renderDiagnostics each have at most 20 entries per node; truncated includes omitted nodes or entries; with colors=true, nodes with visible SOLID fills carry solidFills [{hex, opacity?, variableId?, variableName?}]"},
                "truncated": {"type": "boolean"},
                "path": {"type": "array", "items": {"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}}}, "description": "the node's ancestors from the document down to the node itself, find's path shape; an instance sublayer's path continues through its expanded parents; absent with flat=true"},
                "images": {"type": "object", "description": "optional source image evidence when images=true"},
                "states": {"type": "object", "description": "optional variants and prototype field evidence when states=true; independent truncation, source file, node/override provenance and unknown markers; no inferred business logic"},
                "variantComparison": {"type":"object", "description":"optional same-set definition comparison when compareVariant is supplied; raw fields, paired children, descendant value multisets, unpaired nodes, unknowns and independent truncation; not a pixel comparison"},
                "comparison": {"type":"object", "description":"optional any-two-nodes definition comparison when compare is supplied; same shape as variantComparison without componentSetId, plus rootDelta and a reason on each unpaired node (ambiguous-name, name-truncated, no-counterpart); not a pixel comparison"},
                "_figlens": {"type":"object", "description":"semantics.verification: whether every node's _figlens.semantics was compared with Figma exports (not-compared-with-figma)"},
                "annotationCategories": {"type":"object", "description":"optional {categoryId: {label, isPreset, color?, labelTruncated?}} for the categories the kept annotations name"},
                "handoffDiagnostics": {"type":"object", "description":"optional known stored semantics not emitted by the projection; counts cover the selected expanded subtree independently of summary depth, with at most five example IDs per feature and explicit coverage limits"},
            }),
            ["truncated"].as_slice(),
        ),
        "export" => (
            serde_json::json!({
                "path": { "type": "string" },
                "width": { "type": "integer" },
                "height": { "type": "integer" },
                "bytes": { "type": "integer" },
                "quality": { "type": "integer", "description": "the quality used; --max-kb may lower it" },
                "layoutBox": { "type": ["object", "null"], "description": "absoluteBoundingBox in page coordinates" },
                "renderBox": { "type": ["object", "null"], "description": "absoluteRenderBounds in page coordinates" },
                "warnings": { "type": "array", "items": { "type": "string" }, "description": "first, when the drawn area differs from the layout box, how far it reaches past each side, what draws there when known, and which box was exported; then one `Render <state> at node <id> (<field>): <reason>` line per known skipped or approximated rendering in the subtree, as in bundle warnings; then one line per visible image paint whose bytes the file lacks (`Image <ref> at node <id> is not in the .fig; it is drawn empty`) and per instance whose component it lacks (`Component <id> of instance <id> is not in the .fig; ...`), as in a .fig from `figlens clipboard`; empty when none applies" },
                "matches": { "type": "array", "items": { "type": "string" }, "description": "with matchDir: files there (relative paths) that draw the same outlines" },
                "hidden": { "type": "array", "items": { "type": "string" }, "description": "with hide: the layers drawn as hidden" },
                "similar": { "type": "array", "items": { "type": "object" }, "description": "with matchDir: up to 10 other files with a similar shape as {file, score}, score 0.8-1, best first; not proof of the same icon" },
            }),
            ["path", "width", "height", "bytes", "quality"].as_slice(),
        ),
        "export_batch" => (
            serde_json::json!({
                "count": { "type": "integer" },
                "failed": { "type": "integer", "description": "items with an error; the others were written" },
                "items": { "type": "array", "items": { "type": "object" }, "description": "in request order: node, path and either the export fields (width, height, bytes, quality, layoutBox, renderBox, warnings) or error" },
            }),
            ["count", "failed", "items"].as_slice(),
        ),
        "compare_image" => (
            serde_json::json!({
                "node": { "type": "object", "description": "id, name and type of the compared node" },
                "frame": { "type": "object", "description": "the node's layout box width and height in node pixels" },
                "screenshot": { "type": "object", "description": "width and height in pixels" },
                "scale": { "type": "number" },
                "compared": { "type": "object", "description": "width and height of the compared area in screenshot pixels" },
                "uncompared": { "type": "object", "description": "below: screenshot rows past the node's height, not compared; missing: with fullPage, node rows past the screenshot's bottom, counted as differing" },
                "threshold": { "type": "integer", "description": "per-channel difference (of 255) above which pixels differ" },
                "mismatch": { "type": "number", "description": "differing pixels / compared pixels, 0-1; with uncompared.missing, both include the missing rows as differing; ignored pixels are in neither" },
                "nonTextMismatch": { "type": "number", "description": "differing pixels in regions' cells not over text layers / compared pixels" },
                "textMismatch": { "type": "number", "description": "differing pixels in regions' cells over text layers / compared pixels" },
                "nonImageMismatch": { "type": "number", "description": "differing pixels in regions' cells not only over image-filled layers / compared pixels" },
                "imageMismatch": { "type": "number", "description": "differing pixels in regions' cells only over image-filled layers / compared pixels" },
                "ignored": { "type": "object", "description": "with ignore: nodes {id, name, type} and pixels, the screenshot pixels left out of every count" },
                "regionCount": { "type": "integer" },
                "nonTextRegions": { "type": "integer", "description": "regions not marked text" },
                "regions": { "type": "array", "items": { "type": "object" }, "description": "largest first, at most limit: x, y, width, height in node pixels from its top-left, pixels (differing screenshot pixels), text, image, and nodes: up to 3 deepest layers under it as {id, name, type}, most differing pixels first" },
                "warnings": { "type": "array", "items": { "type": "string" }, "description": "known skipped or approximated rendering in the node, as export's" },
                "path": { "type": "string", "description": "with out: the diff PNG written" },
            }),
            [
                "node",
                "frame",
                "screenshot",
                "scale",
                "compared",
                "threshold",
                "mismatch",
                "nonTextMismatch",
                "textMismatch",
                "nonImageMismatch",
                "imageMismatch",
                "regionCount",
                "nonTextRegions",
                "regions",
                "warnings",
            ]
            .as_slice(),
        ),
        "bundle" => (
            serde_json::json!({
                "nodeId": { "type": "string" },
                "path": { "type": "string" },
                "warnings": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "missing assets, known rendering limits and truncated optional state inventory; missing-assets.json contains only asset failures; design/nodes.json has per-node _figlens.renderDiagnostics; design/states.json has inventory coverage when requested",
                },
            }),
            ["nodeId", "path", "warnings"].as_slice(),
        ),
        "verify" => (
            serde_json::json!({
                "path": { "type": "string" },
                "files": {
                    "type": "object",
                    "description": "verified count; missing paths; changed entries with expected and actual size/sha256 (actual null when larger than declared, unreadable, with error, or not a regular file inside the bundle); invalid manifest paths",
                },
                "missingAssets": { "type": "array", "description": "missing-assets.json entries" },
                "fonts": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "family, style, weight, postScriptName, nodeIds, status (local_file or unverified; neither shows the browser loads the face), localFiles (up to 3 {path, matchedBy postScriptName|family+style|filename, inProject}) and, for a match inside fonts, fontFace: an @font-face snippet",
                },
                "fontLookup": {
                    "type": "object",
                    "description": "searched folders, filesRead (font files whose names were read) and truncated (a file, entry or byte budget stopped the lookup, so an unverified face may still have a file)",
                },
                "unverified": { "type": "array", "items": { "type": "string" } },
                "findings": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "handoff preflight, missing inputs first: category (integrity, asset, variable, scope, render, semantics, font), status (missing, unsupported or unverified), source file, nodeIds (at most 20, none over 1 KiB) and nodeCount, evidence, impact and action; at most 500 entries and 1 MiB",
                },
                "findingsSummary": {
                    "type": "object",
                    "description": "scope (bundle root nodeId and name), total, returned, truncated, counts per category/status, unexamined metadata files, semanticsCoverage (the handoff inventory's coverage) and the coverage the findings do not establish",
                },
                "truncated": {
                    "type": "object",
                    "description": "original length of each other list cut to stay within the reply budget (files.missing, files.changed, files.invalid, missingAssets, fonts); empty when nothing was cut",
                },
            }),
            [
                "path",
                "files",
                "missingAssets",
                "fonts",
                "fontLookup",
                "unverified",
                "findings",
                "findingsSummary",
                "truncated",
            ]
            .as_slice(),
        ),
        _ => return None,
    };
    Some(serde_json::json!({ "type": "object", "properties": properties, "required": required }))
}

/// The `export` parameters an `export_batch` item takes (`matchDir` is single-export only).
pub const EXPORT_ITEM_KEYS: [&str; 8] = [
    "node", "out", "bounds", "scale", "quality", "maxKb", "maxSize", "hide",
];

fn default(tool: &str, key: &str) -> Option<Value> {
    match (tool, key) {
        ("tree", "depth") => Some(json!(usize::MAX)),
        ("tree" | "find" | "search", "node") => Some(json!("0:0")),
        ("tree", "json" | "topFrames")
        | ("inspect", "images" | "states" | "flat" | "fullText" | "colors")
        | ("nodes", "compact" | "subtrees")
        | ("bundle", "states" | "hashSource")
        | ("search", "includeHidden" | "includeInternal") => Some(json!(false)),
        ("inspect", "depth") | ("export", "scale") => Some(json!(1)),
        ("find" | "search", "limit") => Some(json!(20)),
        ("compare_image", "limit") => Some(json!(10)),
        ("compare_image", "threshold") => Some(json!(figlens::compare_image::THRESHOLD)),
        ("compare_image", "fullPage") => Some(json!(false)),
        ("variables", "format") => Some(json!("rest")),
        ("usages", "scope") | ("diff", "node") => Some(json!("0:0")),
        ("diff", "limit") => Some(json!(100)),
        ("usages", "direction") => Some(json!("in")),
        ("usages", "limit") => Some(json!(50)),
        ("usages", "includeHidden") => Some(json!(false)),
        ("export", "bounds") => Some(json!("render")),
        ("export", "quality") => Some(json!(80)),
        ("export", "maxSize") => Some(json!(1600)),
        _ => None,
    }
}

const SOURCE_HELP: &str = "Cannot read the requested .fig source; check the path and valid local copy, or gcloud availability, connectivity and authorized object read access.";

// Larger bounds break int64 and JavaScript schema parsers; some model backends reject every tool.
const JSON_SAFE: u64 = (1 << 53) - 1;

pub fn property(tool: &str, param: &Param) -> Value {
    let (key, kind, _, about) = *param;
    let mut p = json!({"type": kind, "description": about});
    let range = match (tool, key) {
        ("inspect", "depth") => Some((0, 10)),
        ("tree", "depth") => Some((0, JSON_SAFE.min(usize::MAX as u64))),
        ("find" | "search" | "usages", "limit") => Some((1, 200)),
        ("compare_image", "limit") => Some((1, 50)),
        ("compare_image", "threshold") => Some((1, 254)),
        ("diff" | "changes", "limit") => Some((1, 500)),
        ("export", "quality") => Some((1, 100)),
        ("export", "maxKb") => Some((0, JSON_SAFE.min((usize::MAX / 1024) as u64))),
        ("export", "maxSize") => Some((0, u64::from(u32::MAX))),
        _ => None,
    };
    if let Some((min, max)) = range {
        p["minimum"] = min.into();
        p["maximum"] = max.into();
    }
    if let Some(value) = default(tool, key).filter(|_| (tool, key) != ("tree", "depth")) {
        p["default"] = value;
    }
    if matches!(
        key,
        "file"
            | "old"
            | "out"
            | "query"
            | "compareVariant"
            | "compare"
            | "bundle"
            | "cursor"
            | "matchDir"
            | "hide"
            | "ignore"
            | "fonts"
            | "collection"
            | "mode"
    ) {
        p["minLength"] = json!(1);
    }
    if key == "query" {
        p["pattern"] = json!(r"\S");
    }
    if tool == "compare_image" && key == "image" {
        p["minLength"] = json!(1);
    }
    if tool == "export_batch" && key == "items" {
        p["minItems"] = json!(1);
        p["maxItems"] = json!(crate::command::MAX_EXPORT_ITEMS);
        p["items"] = json!({
            "type": "object",
            "properties": TOOLS.iter().find(|t| t.0 == "export").unwrap().2.iter()
                .filter(|param| EXPORT_ITEM_KEYS.contains(&param.0))
                .map(|param| (param.0.to_owned(), property("export", param)))
                .collect::<serde_json::Map<_, _>>(),
            "required": ["node", "out"],
            "additionalProperties": false,
        });
    }
    if tool == "export" && key == "scale" {
        p["exclusiveMinimum"] = json!(0);
    }
    if tool == "export" && key == "bounds" {
        p["enum"] = json!(["render", "layout"]);
    }
    if tool == "variables" && key == "format" {
        p["enum"] = json!(["rest", "dtcg"]);
    }
    if tool == "workflow" && key == "topic" {
        p["enum"] = json!(crate::workflow::TOPICS);
    }
    if tool == "index" && key == "action" {
        p["enum"] = json!([
            "prepare",
            "status",
            "add",
            "remove",
            "relocate",
            "refresh",
            "rebuild",
            "track",
            "untrack",
            "embed",
            "visual",
            "clear_cache",
            "reset"
        ]);
    }
    if tool == "jobs" && key == "action" {
        p["enum"] = json!(["status", "cancel"]);
    }
    if tool == "models" && key == "action" {
        p["enum"] = json!(["list", "status", "configure", "remove"]);
    }
    if tool == "watch" && key == "action" {
        p["enum"] = json!([
            "start",
            "stop",
            "status",
            "ack",
            "subscribe",
            "evaluate",
            "list",
            "unsubscribe",
            "forget"
        ]);
    }
    if tool == "search" && key == "mode" {
        p["enum"] = json!(["text", "semantic", "hybrid", "visual"]);
    }
    if tool == "variables" && key == "aliasModes" {
        p["propertyNames"] = json!({"minLength": 1});
        p["additionalProperties"] = json!({"type": "string", "minLength": 1});
    }
    if tool == "search" && key == "imageRef" {
        p["properties"] = json!({"sourceId": {"type": "string", "minLength": 1},
            "snapshotId": {"type": "string", "minLength": 1}, "nodeId": {"type": "string", "minLength": 1}});
        p["required"] = json!(["sourceId", "snapshotId", "nodeId"]);
        p["additionalProperties"] = json!(false);
    }
    if tool == "search" && key == "crop" {
        let pixel = json!({"type": "integer", "minimum": 0, "maximum": u32::MAX});
        p["properties"] = json!({"x": pixel, "y": pixel, "width": pixel, "height": pixel});
        p["required"] = json!(["x", "y", "width", "height"]);
        p["additionalProperties"] = json!(false);
    }
    if tool == "usages" && key == "direction" {
        p["enum"] = json!(["in", "out"]);
    }
    if (tool == "usages" || tool == "watch") && key == "library" {
        p["properties"] = json!({
            "kind": {"type": "string", "enum": figlens::workspace::library::KINDS},
            "namespace": {"type": "string", "minLength": 1, "description": "the library's sourceLibraryKey, as an outgoing usages result's library names it; omitted, only copies that record none are listed and the answer is never complete"},
            "key": {"type": "string", "minLength": 1, "description": "the component, style or variable key"},
            "definition": {"type": "object", "description": "the definition you select, as an exact ref an earlier reply gave: checked visible, ready at that snapshot and of the kind, then reported selected with keyEvidence unverified, never resolved from the key",
                "properties": {"sourceId": {"type": "string", "minLength": 1}, "snapshotId": {"type": "string", "minLength": 1}, "nodeId": {"type": "string", "minLength": 1}},
                "required": ["sourceId", "snapshotId", "nodeId"], "additionalProperties": false},
        });
        p["required"] = json!(["kind", "key"]);
        p["additionalProperties"] = json!(false);
    }
    p
}

/// `value` against an object schema of string, integer and object fields: every required field
/// present, no other one, each string non-empty and within its enum, each integer in range.
fn object_fits(value: &Value, schema: &Value) -> bool {
    // A map such as aliasModes: any non-empty key, each value a non-empty string.
    if schema["additionalProperties"]["type"] == "string" {
        return value.as_object().is_some_and(|fields| {
            fields.iter().all(|(name, v)| {
                !name.is_empty() && v.as_str().is_some_and(|text| !text.is_empty())
            })
        });
    }
    let (Some(fields), Some(props)) = (value.as_object(), schema["properties"].as_object()) else {
        return false;
    };
    let required = schema["required"].as_array().into_iter().flatten();
    required
        .filter_map(Value::as_str)
        .all(|r| fields.contains_key(r))
        && fields.iter().all(|(name, v)| match props.get(name) {
            None => false,
            Some(p) if p["type"] == "object" => object_fits(v, p),
            Some(p) if p["type"] == "integer" => integer(v).is_some_and(|n| {
                p["minimum"].as_u64().is_none_or(|m| n >= m)
                    && p["maximum"].as_u64().is_none_or(|m| n <= m)
            }),
            Some(p) => v.as_str().is_some_and(|text| {
                !text.is_empty() && p["enum"].as_array().is_none_or(|e| e.contains(v))
            }),
        })
}

pub(crate) fn integer(value: &Value) -> Option<u64> {
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    let n = value.as_f64()?;
    // u64::MAX rounds up to 2^64 as f64; exclude that endpoint before casting.
    ((0.0..18_446_744_073_709_551_616.0).contains(&n) && n.fract() == 0.0).then_some(n as u64)
}

/// An immutable call whose identity, defaults and arguments have passed the contract.
pub struct PreparedCall {
    name: &'static str,
    arguments: Value,
}

impl PreparedCall {
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn argument(&self, key: &str) -> &Value {
        &self.arguments[key]
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        self.argument(key).as_str()
    }
}

/// Validate before source I/O, normalize mathematical integers and materialize MCP defaults.
pub fn prepare(tool: &str, args: &Value) -> Result<PreparedCall, Failure> {
    let (name, _, params) = TOOLS
        .iter()
        .find(|t| t.0 == tool)
        .ok_or_else(|| Failure::argument("unknown tool"))?;
    let object = args
        .as_object()
        .ok_or_else(|| Failure::argument("arguments must be an object"))?;
    // A misspelled optional parameter would otherwise fall back to its default and still succeed.
    if let Some(key) = object
        .keys()
        .find(|key| !params.iter().any(|p| p.0 == *key))
    {
        let accepted = params.iter().map(|p| p.0).collect::<Vec<_>>().join(", ");
        return Err(Failure::argument(format!(
            "unknown parameter {key}; accepted: {accepted}"
        )));
    }
    let mut checked = json!({});
    for param in *params {
        let (key, kind, required, _) = *param;
        let Some(value) = object.get(key) else {
            if required {
                return Err(Failure::argument(format!(
                    "missing required parameter {key}"
                )));
            }
            if let Some(value) = default(tool, key) {
                checked[key] = value;
            }
            continue;
        };
        let schema = property(tool, param);
        checked[key] = value.clone();
        let bad = || Failure::argument(format!("invalid parameter {key}; expected {schema}"));
        match kind {
            "integer" => {
                let n = integer(value).ok_or_else(bad)?;
                if n < schema["minimum"].as_u64().unwrap()
                    || n > schema["maximum"].as_u64().unwrap()
                {
                    return Err(bad());
                }
                checked[key] = n.into();
            }
            "number" => {
                let n = value.as_f64().ok_or_else(bad)?;
                if !n.is_finite() || n <= 0.0 {
                    return Err(bad());
                }
            }
            "boolean" if !value.is_boolean() => return Err(bad()),
            // Library (usages, watch), imageRef and crop (search) list every field; aliasModes is a map.
            "object" if !object_fits(value, &schema) => return Err(bad()),
            "string" => {
                let text = value.as_str().ok_or_else(bad)?;
                if (schema["minLength"] == 1 && text.is_empty())
                    || (key == "query" && text.trim().is_empty())
                {
                    return Err(bad());
                }
                if let Some(choices) = schema["enum"].as_array()
                    && !choices.contains(value)
                {
                    return Err(bad());
                }
            }
            _ => {}
        }
    }
    if tool == "nodes" && checked["subtrees"] == true && checked["compact"] != true {
        return Err(Failure::argument("nodes subtrees requires compact: true"));
    }
    // A JSON file only, so nodes out never replaces another design.
    if tool == "nodes"
        && checked["out"].as_str().is_some_and(|out| {
            std::path::Path::new(out)
                .extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("json"))
        })
    {
        return Err(Failure::argument("nodes out must end in .json"));
    }
    if tool == "export"
        && figlens::export::Format::of(std::path::Path::new(checked["out"].as_str().unwrap()))
            .is_none()
    {
        return Err(Failure::argument(
            "out must end in .png, .jpg, .jpeg, .webp, .avif or .svg",
        ));
    }
    if tool == "compare_image"
        && checked["out"]
            .as_str()
            .is_some_and(|out| !out.to_ascii_lowercase().ends_with(".png"))
    {
        return Err(Failure::argument("out must end in .png"));
    }
    if tool == "compare_image"
        && let Some(ids) = checked["ignore"].as_str()
    {
        crate::command::id_list("ignore", ids).map_err(|e| Failure::argument(e.to_string()))?;
    }
    if tool == "export_batch" {
        crate::command::export_items(&checked["items"], 1600).map_err(Failure::argument)?;
    }
    let call = PreparedCall {
        name,
        arguments: checked,
    };
    if tool == "variables" {
        crate::command::Command::variables(
            call.string("node").map(str::to_owned),
            call.string("format"),
            call.string("collection").map(str::to_owned),
            call.string("mode").map(str::to_owned),
            call.string("out").map(Into::into),
            crate::command::alias_modes(&call),
        )
        .map_err(|error| Failure::argument(error.to_string()))?;
    }
    if tool == "search" {
        let file = call.string("file").is_some();
        if file == call.string("workspaceId").is_some()
            || (file && (call.string("sourceId").is_some() || call.string("storage").is_some()))
        {
            return Err(Failure::argument(
                "search needs file or workspaceId, not both; sourceId and storage apply to workspaceId",
            ));
        }
        // Only validates: the image path is admitted under the roots before the call runs.
        crate::command::search_tool(&call, call.string("image").map(std::path::Path::new))
            .map_err(Failure::from_error)?;
    }
    if tool == "usages" {
        let file = call.string("file").is_some();
        if file == call.string("workspaceId").is_some()
            || (file && (call.string("sourceId").is_some() || call.string("storage").is_some()))
        {
            return Err(Failure::argument(
                "usages needs file or workspaceId, not both; sourceId and storage apply to workspaceId",
            ));
        }
        if object.contains_key("library") {
            if file {
                return Err(Failure::argument(
                    "usages with library reads a workspace: pass workspaceId, not file",
                ));
            }
            if let Some(key) = ["node", "resource", "snapshotId", "relation", "direction"]
                .into_iter()
                .find(|key| object.contains_key(*key))
            {
                return Err(Failure::argument(format!(
                    "usages with library takes no {key}: library replaces node and resource, and snapshotId, relation and direction apply only to them"
                )));
            }
            crate::command::library_tool(&call).map_err(Failure::from_error)?;
        } else {
            crate::command::usages_tool(&call).map_err(Failure::from_error)?;
        }
    }
    Ok(call)
}

pub fn tools() -> Value {
    json!({ "tools": TOOLS.iter().map(|(name, about, params)| {
        let mut tool = json!({
            "name": name,
            "description": about,
            "inputSchema": {
                "type": "object",
                "properties": params.iter().map(|param|
                    (param.0.to_owned(), property(name, param))
                ).collect::<serde_json::Map<_, _>>(),
                "required": params.iter().filter(|p| p.2).map(|p| p.0).collect::<Vec<_>>(),
                "additionalProperties": false,
            }
        });
        if let Some(schema) = output_schema(name) {
            tool["outputSchema"] = schema;
        }
        match *name {
            "workflow" => tool["annotations"] = json!({ "readOnlyHint": true, "openWorldHint": false }),
            // This tool can publish a token file; hosts must not grant it a read-only tier.
            "variables" => tool["annotations"] = json!({"readOnlyHint": false,
                "destructiveHint": true, "openWorldHint": false}),
            // Mixed actions, reset among them.
            "index" => tool["annotations"] = json!({"readOnlyHint": false, "destructiveHint": true,
                "idempotentHint": false, "openWorldHint": false}),
            // cancel changes a job.
            "jobs" => tool["annotations"] = json!({"readOnlyHint": false, "destructiveHint": false,
                "openWorldHint": false}),
            // remove forgets a configured profile.
            "models" => tool["annotations"] = json!({"readOnlyHint": false, "destructiveHint": true,
                "openWorldHint": false}),
            "changes" => tool["annotations"] = json!({"readOnlyHint": true, "openWorldHint": false}),
            // Reads Figma's REST API with the server's token.
            "freshness" => tool["annotations"] = json!({"readOnlyHint": true, "openWorldHint": true}),
            // unsubscribe and forget delete checkpoints; start makes a new lease each time.
            "watch" => tool["annotations"] = json!({"readOnlyHint": false, "destructiveHint": true,
                "idempotentHint": false, "openWorldHint": false}),
            _ => {}
        }
        tool
    }).collect::<Vec<_>>() })
}

#[derive(Debug)]
pub struct Failure {
    pub kind: ErrorKind,
    pub text: String,
    /// Implemented recovery actions that apply, from the core's error.
    pub recovery: Option<Value>,
}

impl Failure {
    pub fn new(kind: ErrorKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            recovery: None,
        }
    }

    pub fn argument(text: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidArguments, text)
    }

    pub fn from_error(error: Box<dyn Error>) -> Self {
        let kind = error
            .downcast_ref::<FigError>()
            .map_or(ErrorKind::OperationFailed, FigError::kind);
        let text = match kind {
            ErrorKind::SourceUnreadable | ErrorKind::InvalidData => SOURCE_HELP.to_owned(),
            _ => error.to_string(),
        };
        Self {
            recovery: error
                .downcast_ref::<FigError>()
                .and_then(FigError::recovery)
                .cloned(),
            ..Self::new(kind, text)
        }
    }

    pub fn metadata(&self) -> Value {
        let (code, next) = match self.kind {
            ErrorKind::InvalidArguments => (
                "invalid_arguments",
                "Use tools/list types and ranges; omit optional values for defaults, or use maxSize: 0 for no side cap.",
            ),
            ErrorKind::SourceUnreadable | ErrorKind::InvalidData => (
                "source_unreadable",
                "Check the source path, export a valid .fig, and verify authorized access; do not bypass permissions.",
            ),
            ErrorKind::NodeNotFound => (
                "node_not_found",
                "Use find/tree on a stored root or inspect the containing instance; copy the complete returned ID.",
            ),
            ErrorKind::UnsupportedNode => (
                "unsupported_node",
                "Use inspect/nodes for instance sublayers, or choose a sized drawable node for export/bundle.",
            ),
            ErrorKind::ParseBudget => (
                "parse_budget_exceeded",
                "Use a smaller local copy or report the named limit for review.",
            ),
            ErrorKind::ResponseBudget => (
                "response_budget_exceeded",
                "Use inspect, bundle one frame, or CLI nodes for full JSON; for inline exports lower maxSize. A completed file is identified by outputPath when present.",
            ),
            ErrorKind::OutputWrite => (
                "output_write_failed",
                "Create an authorized writable output directory; check free space and permissions.",
            ),
            ErrorKind::PermissionDenied => (
                "permission_denied",
                "Use an authorized local source and output inside the configured roots; retain the restrictions.",
            ),
            ErrorKind::OperationFailed => (
                "operation_failed",
                "Inspect the error and supported operation; report a reproducible case without private design data.",
            ),
            ErrorKind::WorkspaceBusy => (
                "workspace_busy",
                "Another figlens process is using this workspace; retry after it finishes, without killing it.",
            ),
            ErrorKind::IndexIncompatible => (
                "index_incompatible",
                "The workspace database was written by a different figlens schema; use that figlens version or reset the workspace.",
            ),
            ErrorKind::StorageUnavailable => (
                "storage_unavailable",
                "The workspace storage is missing or offline; reconnect or restore it and retry, or follow recovery to recreate the workspace elsewhere.",
            ),
            ErrorKind::SourceStale => (
                "source_stale",
                "The source changed since it was indexed; refresh it with the index tool, then search again.",
            ),
            ErrorKind::CursorStale => (
                "cursor_stale",
                "This cursor was issued for another index, filter set or view; call the same tool again without cursor.",
            ),
            ErrorKind::CursorExpired => (
                "cursor_expired",
                "The index this cursor paged was deleted, or its one-hour lease ended; search again without cursor.",
            ),
            ErrorKind::EventGap => (
                "event_gap",
                "Revisions after this position were deleted before they were read; follow recovery to read changes from the oldest kept, and treat the scope as possibly changed.",
            ),
            ErrorKind::Cancelled => (
                "cancelled",
                "The operation was cancelled before it finished; nothing was published.",
            ),
            ErrorKind::QueryWorkLimit => (
                "query_work_limit",
                "The search would read more indexed data than one query supports; follow recovery to search fewer sources or index a narrower scope. Nothing is wrong with the index.",
            ),
            ErrorKind::ModeUnavailable => (
                "mode_unavailable",
                "This search mode needs a configured local model; check models status and configure one there, or search with mode text.",
            ),
        };
        // A storage failure is fixed by restoring that storage, not by the design roots.
        let storage = self
            .recovery
            .as_ref()
            .and_then(|r| r[0]["action"].as_str())
            .is_some_and(|a| a.starts_with("restore_"));
        // A stored evaluation that cannot be paged is passed or dropped, not fixed at the file.
        let stored = self.recovery.as_ref().is_some_and(|r| {
            r[0]["tool"] == "watch"
                && matches!(r[0]["action"].as_str(), Some("ack" | "unsubscribe"))
        });
        // A budget failure that names its own narrower request is fixed by that request.
        let narrowed = self.kind == ErrorKind::ResponseBudget && self.recovery.is_some() && !stored;
        let next = if narrowed {
            "Follow recovery: it names a narrower request or the exact item to read instead."
        } else if storage {
            "The figlens storage named in the error is missing or denied; ask the user to restore it (never change permissions yourself), then retry; recreate_elsewhere only after the user confirms."
        } else if stored {
            "A stored saved-query evaluation is damaged or too large to page; follow recovery (ack past it, or unsubscribe); the design file is not at fault."
        } else {
            next
        };
        let mut meta = json!({"code": code, "nextStep": next});
        if let Some(recovery) = &self.recovery {
            meta["recovery"] = recovery.clone();
        }
        meta
    }
}

/// Keep local CLI diagnostics and budget provenance; URL-source diagnostics can contain secrets.
pub fn source(file: &str, error: Box<dyn Error>) -> Box<dyn Error> {
    if error
        .downcast_ref::<FigError>()
        .is_some_and(|e| e.kind() == ErrorKind::ParseBudget)
    {
        error
    } else {
        Box::new(FigError::new(
            ErrorKind::SourceUnreadable,
            if file.contains("://") {
                SOURCE_HELP.to_owned()
            } else {
                format!("{file}: {error}")
            },
        ))
    }
}
