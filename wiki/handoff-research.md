# Handoff field and MCP transport research

Read this when deciding the next semantic or transport change. These are local snapshot and executable observations from 2026-10-27, not Figma REST or pixel calibration. Private input, raw inventories, source hashes and runtime traces remain under ignored `samples/handoff-research/`.

## Raw field evidence

`scripts/probe-handoff.rs` inventories stored NodeChange fields without modifying a file or treating glyph positions as semantic rules:

```sh
docker compose run --rm dev cargo run --example handoff-probe -- samples/design.fig > samples/fields.jsonl
```

Four authorized local snapshots (43,841 stored nodes) supplied the following evidence. Counts below describe raw stored records, not unique expanded instances or missing rendered layers.

| Concept | Observed source | What is established |
|---|---|---|
| Vertical trim | `leadingTrim: CAP_HEIGHT` on 75 nodes in one community snapshot | The field exists and is currently omitted. A saved glyph render does not supply the implementation's cap-height metrics. |
| Hyperlinks | Base `hyperlink`, range `textData.styleOverrideTable[].hyperlink`; values include `{url: ...}`, `{guid: ...}` and `{}` | URL, node-link and explicit-clearing cases must remain distinct. Positive URLs occur in more than one snapshot. The target REST shape and inherited range clearing still need independent captures. |
| List numbering | `textData.lines[].lineType`, `isFirstLineOfList`, `listStartOffset` | Defaults occur widely. A separate public numbered-list fixture has a nonzero offset; type alone loses continuation intent. |
| Grid auto layout | 38 `stackMode: GRID` nodes across two snapshots; `gridRows`/`gridColumns` hold GUID/position entries, with sizing, gap and child-anchor fields | Counts, gaps, ordered track sizes, spans and alignment are now emitted ([semantics](semantics.md#grid-auto-layout)). Stored child anchors are not: some disagree with saved positions. Cached grid fields on non-grid nodes do not establish active grid layout. CSS track strings must not be guessed. |
| Motion and code layers | Defined in the embedded schema (`KEYFRAME`, `KEYFRAME_TRACK`, `CODE_LAYER` and related node types; timeline/keyframe/easing fields); 0 nodes in six snapshots (2026-11-03) | Reported as `motion`/`codeLayers` gaps from type and field presence only. No stored animation or code has been read; emitting it needs a real file. |
| Layout guides | Nonempty `layoutGrids` arrays in a community snapshot | Guides are separate from grid auto layout; the projection currently omits them. |
| SLOT | Four SLOT definitions, 89 direct assignments containing `slotContentIdValue`, additional nested override assignments, and 1,089 `isSlotContent` flags in one snapshot | No raw layer has `type: SLOT` in these files. Looking only for the REST layer type would miss the actual source representation. |

An owned local SLOT trace followed one instance's `componentPropAssignments[].varValue.value.slotContentIdValue.guid` to a real stored FRAME marked `isSlotContent`. Its stored parent was a page, not that instance. The instance's current expanded projection omitted that referenced content. This proves a source/projection gap for that case; it does not establish the complete grafting, inheritance, ordering or resize rules for all slots. Do not reparent every flagged node or treat the main component's empty slot as the instance's content.

For list numbering, the public fixture linked in [issue #686](https://github.com/storytold/figcraft/issues/686) supplied 24 nodes. Its second separately numbered item has `listStartOffset: 1`, while the reporter's Figma reference shows item 2. That is one positive lead for a zero-based offset, not a calibrated universal rule. The download stays in samples and is not distributable with figlens. Its glyph output can look correct while JSON still lacks numbering metadata.

Related leads: [vertical trim #466](https://github.com/storytold/figcraft/issues/466), [complete hyperlinks #459](https://github.com/storytold/figcraft/issues/459), [cleared hyperlink ranges #458](https://github.com/storytold/figcraft/issues/458), [grid tracks #465](https://github.com/storytold/figcraft/issues/465) and [SLOT import #792](https://github.com/storytold/figcraft/issues/792). Their import/export implementations are not specifications for figlens's local-file reader.

## Resulting behavior and implementation prerequisites

Known omissions are now reported by the [handoff inventory](semantics.md#known-handoff-gaps), with bounded examples and explicit scope limits. This adds evidence without claiming support for those semantics. Recorded resize, truncation, max lines and decoration already emitted by `nodes` also reach compact inspect typography.

Implementing the omitted fields still requires same-source captures covering base/range links (including long URLs, node links and clearing), trim placement and metrics, ordered-list resets/continuations, grid track/cell sizing, and default versus instance-specific slot content. Check nested overrides, swaps, hidden nodes and exact sublayer selection on a held-out snapshot. Pixel agreement alone cannot validate hyperlinks, lists or slot relationships. No independent same-version Figma JSON/SVG/PNG capture was available in this research; no layout, glyph or slot expansion rule was changed.

## MCP message sizes

`scripts/measure-mcp-payloads.py` records the actual UTF-8 JSON-RPC line returned by the real child process, including its newline. Component sizes are explanatory; they are not substituted for the measured wire size. Example:

```sh
docker compose run --rm dev python3 scripts/measure-mcp-payloads.py --binary target/debug/figlens samples/design.fig 12:345 > samples/payloads.json
```

Measurements before response-budget enforcement, using 0.7.2 source with the handoff inventory:

| Scope/tool | Complete reply bytes |
|---|---:|
| Public synthetic frame, inspect depth 0 | 1,199 |
| Public synthetic frame, nodes | 11,176 |
| Community application screen, inspect depth 0 | 2,467 |
| Same screen, nodes | 661,656 |
| Community component page, nodes | 3,951,223 |
| Whole community document, nodes | 171,415,926 |
| Four sampled inline PNG exports, maxSize 400 | 4,279–26,238 |

`nodes` currently returns text only: the largest line contains 163,028,648 UTF-8 text bytes and no structuredContent. Inspect returns both text and structured JSON. Export adds base64, which must also count toward the final message. Every measured child answered a subsequent ping; this does not prove that a particular MCP SDK accepts those large replies. [Issue #912](https://github.com/storytold/figcraft/issues/912) reports a 10 MiB client buffer limit; do not treat that historical client setting as a universal MCP limit.

The resulting policy caps complete replies at 8 MiB, leaving headroom below that reported limit. Oversized tool replies become a bounded `response_budget_exceeded` error; a completed output file's path is retained. JSON is not truncated into an apparently successful partial design. Full CLI JSON remains available. A 1 MiB request-line budget separately bounds admission reads, drains oversized lines and permits recovery. See [MCP behavior](mcp.md#shared-sources-and-runtime-behavior) for exact framing and errors.

These are transport limits. MCP `nodes` now stops building once its selection exceeds the reply budget ([measurement](performance.md#early-mcp-nodes-budget-2026-10-29)); other tools' projections and response objects can still be constructed before the reply-size check, and expansion is bounded only for MCP `nodes` on a stored root. A pre-expansion node/work budget or streaming local-file JSON export would require separate evidence and a contract that preserves full design access. Current pixel buffers, concurrent preparation and each client's own limits remain separate.

### Host output limits

Measured 2026-11-06 with the 0.13.1 binary on the 58 screens of the [compact-nodes measurement](discovery.md#limits-and-measurement), each sent as one MCP `tools/call` and measured as the reply's text bytes (per-screen sizes in gitignored `samples/issue-207/host-limits-2026-11-06.json`): default MCP `nodes` text had a median of 294 KB (max 1.6 MB) and 53 exceeded 100 KB; compact 84 KB median, 28 over 100 KB; `inspect` `flat` depth 10 at most 27 KB. One 689 KB `nodes` reply, each host non-interactive with default settings:

| Host | What the model received |
|---|---|
| Claude Code 2.1.318 | No data: a notice that the result exceeds the token limit, a spill file, and an instruction to read it in ~384-line chunks |
| Pi 1.5.0 | About 20 KB: head and tail with the middle cut, a truncation warning and a spill file |
| OMP 18.14.0 | The first and last 500 lines, the middle elided, and an artifact holding the rest |

OpenCode was not tested; [opencode#35661](https://github.com/anomalyco/opencode/issues/35661) reports a 50 KB / 2,000-line preview. No host drops the reply, but a head-and-tail preview looks like valid design data while most layers are missing. MCP `nodes` therefore takes `out` ([behavior](mcp.md#shared-sources-and-runtime-behavior)). The default text stays pretty-printed: minifying saves a median 61% but makes the JSON one line, and these hosts preview and spill by line.

## File publication

An isolated copy of the original public fixture reproduced `bundle <source> 0:1 <source>` succeeding while replacing its source. Outputs now use shared [source-safe publication](parsing.md#completed-output-publication), including exclusive sibling staging and failure cleanup. Existing output symlink targets/encoder labels remain covered; alternate hard-link publication preserves the source. This does not claim durable storage across a power loss or arbitrary directory-replacement race resistance.
