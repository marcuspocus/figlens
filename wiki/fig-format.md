# .fig format and pipeline

How figlens turns a `.fig` into REST-shaped JSON and a bundle. Load this before changing decoding, expansion, emitted fields, variables or the bundle; SVG and raster output is the `render` topic.

## Modules

- Done: decode (`src/lib.rs`), instance expansion + constraint resize (`src/expand.rs`), plugin-shaped `nodes.json` with fills/strokes/text/auto-layout, variable-mode paint resolution, effects, rotation, blendMode, layoutWrap, isMask, boundVariables, componentProperties, styles, absoluteRenderBounds and top-level components/componentSets/styles (`src/rest.rs`), `design/variables.json` (`src/variables.rs`), SVG/PNG rendering from the `.fig`'s own path and glyph outlines, no font files (`src/render.rs`, resvg), design bundle zip (`src/bundle.rs`: `design/nodes.json`, `design/variables.json`, `design/fonts.json`, `assets/` + `assets.json`, `references/screen.png`, `context.md`), direct export of one node (SVG or PNG/JPG/WebP/AVIF, raster quality, size budget; instance sublayer ids resolve through their instance) (`src/export.rs`), `compare` against REST exports incl. those fields and `variables.json` (`src/compare.rs`).
- CLI and MCP share command execution and result formatting in the binary-private `src/command.rs`. MCP parameter interpretation is in `src/contract.rs`; request admission is in `src/mcp.rs` (see `mcp`); JSON results reach `structuredContent` without reparsing stdout. Batch, comparison and update orchestration stay in `src/main.rs`. `nodes <file.fig> --frames <list>` (`-` reads stdin) or `nodes <file.fig> --ids <node>...` decodes once and streams one line per listed id (at most 10,000): the single `nodes` output with `nodeId` added, compact, or `{nodeId, error}`; keys stay sorted, so removing `nodeId` and pretty-printing gives the single call's bytes. `export <file.fig> --frames <list|-> --out-dir <dir> [--format F]` (default `png`, the single export options) runs each id through the single `export` command, one after another, writing `<dir>/<nodeId>.<format>` (`:` as `-`) and streaming the export output with `nodeId`; invalid options fail before any file is written. Timings are in `performance`. A failing or panicking id is only its line; the exit status is nonzero only when the `.fig` or the list cannot be read, the list is too long, or export options are invalid. MCP `export_batch` (and CLI `export --batch items.json`, per-item options) is the many-images form over MCP, since raster `export` returns its image inline; other tools need no batch form for local files because MCP reuses their parsed tree, and `gs://` sources parse again per call, so use the CLI batch for them.
- MCP workflow discovery and embedded skill guidance are documented in the `mcp` topic; they do not change design decoding or command output.
- Shared parsing budgets and source-safe output publication are in [parsing](parsing.md).
- A selection copied in Figma becomes a `.fig` with `figlens clipboard`; what a copy lacks is in [clipboard](clipboard.md).
- Discovery output (`find`, `inspect` and `inspect --flat` summaries) is in the `discovery` topic.
- `search` (`src/search.rs`) indexes effective instance content for ranked retrieval through `expand_instances_bounded`; see the `search` topic.
- `inspect --images` (MCP `images: true`) adds a separate `images` object from `src/images.rs`: up to 200 image paints in the expanded subtree, each with original source dimensions/bytes, mode/transform and local node size. Text-range fills include `textStyleId`, the key in that node's `styleOverrideTable`; used vector-region fills carry `vectorStyleId` from `vectorData.styleOverrideTable`, selected by fill-geometry style IDs. Both share the paint limit and retain each style's crop/rotation; unused or duplicate vector styles are omitted. `sourceRect` gives x/y/width/height in original image pixels (`coordinateSpace: source-image-pixels`), before masks/effects/ancestor clipping: FILL center-crops, FIT uses the whole source, axis-aligned STRETCH uses the normalized transform clamped to the source, and `outside` gives the source pixels it reaches past each side (`left`/`top`/`right`/`bottom`), which figlens draws empty. Nonzero image-paint rotation for FILL/FIT, rotated/skewed/singular STRETCH transforms and ones that sample none of the source remain unknown with a reason. Missing or unrecognized source dimensions are explicit. These are pixel-count facts, not quality or blur-cause measurements. `images.truncated` is independent of summary depth/budget; without this flag, inspect and REST `nodes` stay unchanged.
- MCP raster `export` keeps the image bytes produced by the shared renderer and returns a base64 image content block with the format's MIME type, alongside existing metadata and `structuredContent`. It never rereads the output path to attach an image. CLI stdout stays metadata only.
- Preview uses the same pipeline; load the `preview` topic for viewer routes, offline snapshots and browser verification. `nodes`, `inspect` (including images), scoped `variables`, `export`, `bundle`, batch nodes, batch exports and batch bundles select exact expanded IDs through `rest::resolve_nodes` / `resolve_render_nodes` (MCP `nodes`: `resolve_nodes_within`, which stops at the reply budget): expand the outer instance, then match the full path. `tree`, `find`, `inspect`, `nodes` and the preview layer tree print the same REST type (`expand::plugin_type`: `SYMBOL` is `COMPONENT`, a state group `COMPONENT_SET`, a resize-to-fit frame `GROUP`); only raw `node` shows the kiwi type. `tree --json` also gives each node its local pre-rotation `size` `[width, height]` (the same values as `find`; absent on the document and pages), `visible: false` on hidden layers and `layoutMode` on auto-layout frames, omitted when visible or `NONE` as in `nodes`. `tree`, `find` and raw `node` support stored roots only; an existing sublayer gets an unsupported-operation error, not not-found.
- CLI, MCP and preview SVG export share `export::export(Format::Svg)`: one selected subtree, render bounds (the layout box with `bounds layout`), embedded full-resolution images and outlined text. SVG ignores scale/quality; an exceeded size budget fails before writing. Raster export uses `Prepared::export_svg_in` over the same area for backdrops and image downsampling. MCP SVG export returns file metadata only; request PNG separately for an inline reference.
- Unverified against REST (2026-10-29): `individualStrokeWeights` comes from kiwi `borderStrokeWeightsIndependent` with `border{Top,Right,Bottom,Left}Weight`, emitted whenever that flag is set. An omitted side is 0, since the saved `strokeGeometry` covers only the set sides (over 3000 such nodes in two production files). `strokeWeight` stays the kiwi value, which can be 0; what REST reports there is not checked. Render, `inspect` and the preview read the same field.
- `render::Prepared` owns the resolved subtree, metadata and its image bytes for both `export` and bundle references. Selection preserves overrides, inherited modes and effective geometry; missing paths fail without a raw-child fallback. Nodes without export bounds report an unsupported target. Bundle vectors retain full images; composites use their render bounds at 2x without the reference backdrop.

## Terms

"The plugin" below means Figma's own REST export from the plugin API (`node.exportAsync({ format: "JSON_REST_V1" })`), the shape figlens matches; a "capture" is that export plus the plugin's image, SVG and PNG exports of the same frame. "The sweep" is the 826-frame comparison of figlens against captures of one large production file (see the `accuracy` topic).

## Test fixtures

Create a `TestDir` from `src/test_support.rs` before writing generated fixtures under `samples/`, and keep the guard alive through the assertions. It atomically claims a directory, retries occupied names and removes only its own directory on drop. Docker containers share this mount but can reuse PIDs, so a PID and process-local counter alone do not establish ownership. Library tests use the test-only module; binary tests include the same source with `#[path]`. `src/command/instance_tests.rs` generates two component instances and a nested instance, checking emitted IDs, overrides, assets and simple reference pixels through CLI/MCP. This is not Figma visual-accuracy evidence.

## Running the CLI

```bash
docker compose run --rm dev cargo run --release -- tree samples/design.fig --depth 2
docker compose run --rm dev cargo run --release -- node samples/design.fig 12:345
docker compose run --rm dev cargo run --release -- nodes samples/design.fig 12:345   # REST-shaped nodes.json
docker compose run --rm dev cargo run --release -- variables samples/design.fig 12:345
docker compose run --rm dev cargo run --release -- bundle samples/design.fig 12:345 samples/12-345.zip
docker compose run --rm dev cargo run --release -- export samples/design.fig 12:345 samples/12-345.svg
docker compose run --rm dev cargo run --release -- export samples/design.fig 12:345 samples/12-345.webp --scale 2 --quality 80 --max-kb 150
```

## DTCG token export

Opt-in `variables --format dtcg --collection ID --mode ID --out FILE [--alias-mode COLLECTION=MODE]` reuses this extraction without changing REST output or requiring a workspace. Conversion rules, diagnostics, identity extensions, bounds and consumer evidence are in the [tokens topic](tokens.md).

## Verified format rules (checked against the plugin)

Rules marked pending have local evidence; others are sweep-backed.

- `canvas.fig`: `fig-kiwi` magic, u32 LE version (106), then u32-LE-length chunks: 0 = binary schema, 1 = `Message`. zstd or raw deflate.
- Node id = `guid` `sessionID:localID` = Figma URL `node-id` with `-` → `:`. `4294967295:4294967295` (u32::MAX twice) means no guid, e.g. an unset `overrideKey`; `guid_key` returns none for it, since using it as a path element gives unrelated instance sublayers one id and the tree then contains itself (Real Estate App UI Kit, 7 frames overflowed the stack).
- GROUP is a FRAME with `resizeToFit`. `size` is pre-rotation; apply the `transform` chain before comparing with REST `absoluteBoundingBox`.
- Instances store only themselves; expand from `symbolData.symbolID`. Child ids are `I<instance>;<nested instance ids…>;<node>`: only instance boundaries enter the path, plain frames do not. Path elements use `overrideKey ?? guid` (library component copies keep library ids in `overrideKey`).
- COMPONENT_SET is a FRAME with `isStateGroup`.
- Untrusted input: `Tree::new` drops every node whose parent chain loops (and what hangs under it; about 5% of opening a large file), expansion skips an id it already produced and a component that contains itself (MCP `nodes` on a stored root also stops exponential fan-out at the reply budget), `subtree` visits each id once, `context.md` quotes design-file strings as one-line code spans, renders allow only known blend modes and resolve no `<image>` href outside `data:` URLs. A crafted `.fig` (its schema, and so its enum names, ship inside it) must not recurse forever or reach the disk.
- `symbolData.symbolOverrides` and `derivedSymbolData` use the same guidPath keys; nested instances carry their own, relative to themselves. Merge **field by field** (outer wins per field): one list can hold several entries for the same path (e.g. a swap, then a size), and an outer derived entry may hold only geometry while the nested one holds the size. Whole-entry merging was the cause of what looked like footer design edits and an auto-layout gap (button 216→336); Figma does store those sizes. `overrideLevel` is not read and needs no ordering: across nine files (2026-10-28, #53), no list set one field twice for one path, and every outer entry with level ≥ 2 carried the same value as the nested instance's own entry.
- `componentPropRefs` binds assignments: SYMBOL_ID swaps instances, BOOLEAN sets visibility, TEXT_DATA merges source `textData` unless directly overridden (calibration pending). `overriddenSymbolID` also swaps. Swaps take the new main's visibility and name (a variant's set name), unless explicitly renamed.
- `derivedSymbolData` stores sizes only sometimes: SCALE/STRETCH constraint resizes must be computed (groups and boolean ops pass the outer resize through to their children), except for children in auto-layout flow (only ABSOLUTE-positioned ones follow constraints).
- Variable-bound paints of instance sublayers must be resolved in the nearest explicit `variableModeBySetMap` mode (else the set's first mode); direct nodes' cached colors are already right. Library variable values are present in the `.fig`.

## Verified rules for the bundle fields (2026-10-22, 826-frame sweep)

- REST variable ids: local `VariableID:<s:l>`, library `VariableID:<key>/<assetRef.version>`; collections likewise (`VariableCollectionId:`). `boundVariables` lists library variables by key string first, then local ones by numeric guid.
- Field bindings come from `parameterConsumptionMap`, which supersedes `variableConsumptionMap`; a `FONT_STYLE` binding wraps its alias in `value.fontStyleValue.asFloat` and is REST `fontWeight`. A text style's own FONT_SIZE/FONT_STYLE bindings also appear on the nodes using the style.
- Kiwi variable scopes `STROKE` → `STROKE_COLOR`, `FONT_STYLE` → `FONT_WEIGHT`; no scopes means `["ALL_SCOPES"]`. Collection modes sort by `sortPosition`; the first is `defaultModeId`.
- componentProperties: VARIANT values from the main's `variantPropSpecs`; others keyed `name#<set def id>` via `parentPropDefId`; BOOL is REST BOOLEAN.
- styles: `styleIdForText/Fill/StrokeFill/Effect/Grid` → text/fill/stroke/effect/grid; an `assetRef` resolves to the local copy with the same key. Frame-like nodes repeat the fill style under `fills`. Local style, component and variable keys are server-assigned and not in the `.fig`. The response-level `styles` resolves each id to `{name, styleType, remote, description}`, plus `key` and `remote: true` for library copies; across snapshots, match library styles by `key` (the local copy's id changes on re-import) and local styles by name.
- `layoutPositioning: "ABSOLUTE"` for an auto-layout parent's child with kiwi `stackPositioning: ABSOLUTE`; the plugin omits the default `AUTO` (sets match the plugin in all 13 frames checked).
- Missing `stackPaddingRight`/`Bottom` is 0, not the other side: 35 one-sided nodes in 3 files sat flush there (2026-10-23).
- Effects: `FOREGROUND_BLUR` is REST `LAYER_BLUR`; `spread` only when nonzero; `showShadowBehindNode` only on DROP_SHADOW. SECTION and CANVAS have no blendMode/effects.
- Path blobs (`commandsBlob`): u8 command (0 Z, 1 M, 2 L, 3 Q, 4 C) then f32 LE points. Vectors are in local px; glyphs in em units, y up (`translate(pos) scale(fs,-fs)`), and may start with Z.
- Resized instance children: use derived `fillGeometry`/`strokeGeometry`/`derivedTextData` when present, else scale the source geometry by size / source size (fill and stroke separately).
- absoluteRenderBounds: leaves are tight transformed geometry (INSIDE strokes intersected with the fill shape, effects expand it); containers union their own box with their children; clipped by `clipsContent` ancestors; null when fully clipped, hidden, opacity 0, a boolean operand, or a mask (siblings after a mask clip to it). Text truncation keeps `glyphs[..truncationStartIndex]`.
- Assets as the plugin selects them: image fills → `assets/<sha256>` keyed by imageRef (last node in DFS order); rendered vector types not inside a boolean op → `node:<id>` SVG; blend/effect/mask nodes with an image in the subtree → `render:<id>` 2x PNG. Export size is `ceil(len*scale - 1e-3)`.

## Bundle

Bundle files, batch mode, the font inventory and `verify` are in the `bundle` topic ([bundle.md](bundle.md)).

## Field references

[Semantics and handoff gaps](semantics.md), [variants](states.md), [prototype evidence](prototype.md); Figma comparison pending.
