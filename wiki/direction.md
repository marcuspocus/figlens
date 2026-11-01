# Direction

Load this to decide what to work on next or to revisit an architecture decision.

## Next steps

1. `nodes` emits partial constraints, layoutAlign/layoutGrow and componentPropertyDefinitions with provenance; [same-version Figma calibration remains pending](semantics.md). layoutGrids are still omitted.
2. Text reflow for overridden text (stale derived sizes), only if a same-day reference shows it is real.
3. Renderer gaps (`src/render.rs`; the ones in code carry `shortcut:` comments): image contrast/temperature/tint/highlights/shadows adjustments (the largest remaining reference difference), background blur of content outside the export, through ancestor masks/clips/opacity/blend modes, or of an earlier blur, GLASS effects, angular/diamond gradients drawn as radial, TILE rotation ignored and tiling unverified against Figma, JPEG fills not pre-downscaled, INSIDE strokes doubling edge antialiasing (small ellipses).
4. Batch mode: a stack overflow aborts the whole process, which `catch_unwind` cannot stop. The one reproduced cause, layer depth, is now bounded per item and in tree, preview and compare ([parsing](parsing.md#layer-depth)); any other abort (out of memory, a recursion not yet found) still ends the whole batch. Consider child processes only with a reproduced cause that a limit cannot bound, since they lose the shared decode.
5. Regression CI on community files, so a Figma format change is caught and fixed quickly.

## Background decisions

- Goal: AI design-to-code unconstrained by Figma (no API rate limits, no login, no running Figma), not replacing Figma for designers (2026-10-21).
- Positioning (2026-11-01): figlens parses the design; the agent using it is the frontend engineer, responsible for frontend practice, web performance and the project's infrastructure. Implementation guidance (skill, plan, yolo, bundle `context.md`, the viewer prompt) frames building as hand-written components, data and intent-based layout, reviewed before pixels; a pixel match is one check, never the goal. Cause: a yolo run of a whole design shipped the node tree as JSON replayed by a generic renderer.
- Raster export encoders (2026-10-22): `jpeg-encoder` and `ravif` (pure Rust, `asm` off so no nasm), and libwebp via `webp` because the pure-Rust WebP encoder is lossless only and export needs a quality setting.
- Rust chosen for rendering; Go lacks 2D/CJK text shaping. The renderer draws the outlines the `.fig` already stores (shape geometry, text glyphs) to SVG and rasterizes with resvg, so no font files or text shaping are needed.
- `nodes` prints the per-node object of a REST nodes response, with no option for the `{nodes: {id: ...}}` envelope (2026-10-29, #82): `jq '{nodes: {("<id>"): .}}'` wraps it, and `--frames` lines already carry `nodeId`. A flag across CLI, MCP and docs is not worth a one-line wrap.
- Rejected: Figma remote MCP and REST as the read path (rate limits; REST file reads are Tier 1), a self-built Figma replacement, unreviewed `.fig` parsers.
