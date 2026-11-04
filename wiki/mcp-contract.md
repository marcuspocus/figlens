# MCP contract reference

Field reference linked from [mcp](mcp.md).

`src/contract.rs` owns metadata, discovery, normalization and defaults. `prepare` returns a private-field, immutable `PreparedCall` required by `Command::from_tool`, once before admission/loading. Mathematical JSON integers (`2.0`) normalize; fractions, negative integers, overflow and nonfinite scales fail.

| Parameter | Range / default |
|---|---|
| `tree.depth` | 0 through `2^53-1` (JSON-safe); omitted means every level; past 256 levels the call fails with `parse_budget_exceeded` ([layer depth](parsing.md#layer-depth)) |
| `inspect.depth` | 0–10 / 1 |
| `nodes.compact`, `nodes.subtrees` | boolean / false; subtrees requires compact. Versioned lossless references instead of default REST, with the same complete-reply cap; [wire contract](discovery.md#compact-nodes) |
| `find.limit`, `search.limit` | 1–200 / 20 |
| `usages.limit`, `usages.scope` | 1–200 / 50; `0:0` |
| `changes.limit` | revisions 1–50 / 20; with `revision`, entries (changes, then impact) 1–500 / 100; with a query `subscriptionId`, evaluations 1–50 / 20 |
| `export.scale` | finite and greater than 0 / 1 |
| `export.quality` | 1–100 / 80 |
| `export.maxKb` | 0 through `2^53-1`; omitted means no byte budget |
| `export.maxSize` | 0 through `u32::MAX` / 1600; 0 disables the raster side cap |
| `export.hide` | comma-separated ids of layers inside `node`, at least one; each must be inside it |
| `export.bounds` | `render` or `layout` / `render` |
| `compare_image.image` | required local PNG inside the roots, no URL; at most 64 MiB and 25,000,000 pixels, checked before decoding |
| `compare_image.scale` | finite and greater than 0 / screenshot width ÷ node width; a width more than 1 px off is refused |
| `compare_image.limit` | 1–50 / 10 regions listed; counts cover all |
| `compare_image.out` | optional diff path ending in `.png`; never the screenshot or the design |
| `compare_image.fullPage` | boolean / false; true lets a screenshot shorter than the node compare, its missing rows a differing region |
| `compare_image.ignore` | comma-separated ids of layers strictly inside `node`, at least one; their render boxes leave every count |
| `compare_image.threshold` | 1–254 / 48 per-channel difference |
| `freshness.sources`, `freshness.file`, `freshness.url` | all optional strings; `sources` (a mapping) or `file` is required, never both; `url` only with `file`; `gs://` refused. Missing token, network or mapping entry is `state: unknown` with a `reason`, not a failure ([freshness](freshness.md)) |
| `variables.format` | `rest` (default) or `dtcg`; DTCG requires nonempty `collection`, `mode`, `out`, takes optional `aliasModes` (`{collectionId: modeId}`, non-empty strings) and forbids `node`; REST rejects export selectors/output |

`tree`/`find`/`search` root defaults to `0:0`; booleans default false. `tree` `topFrames: true` ignores `depth` and returns the screens under the root (rule in the `discovery` topic): `id TYPE "name"` lines whose JSON-escaped names keep one id per line, or with `json` a flat array of entries without children plus `page` and innermost `section`. With `json`, children of a page or section (and every `topFrames` entry) add `firstText`. Search queries take at most 256 characters and 64 distinct terms (a CJK run of n characters is n-1 bigrams), types 16 names, scopes 1 KiB and cursors offset 100,000 (past it `hasMore` stays true without a `nextCursor`, so narrow the query); a page whose estimated copied names, paths and snippets exceed 8 MiB is refused with `response_budget_exceeded` before results are built (the serialized-reply cap above still applies), so lower `limit` or narrow `node`; `scripts/measure-search.py` measures cold CLI, MCP first and warm search cost and peak RSS on one approved input. File/output strings are nonempty; find queries contain non-whitespace. Export extensions follow the existing case-insensitive encoder selection, including `.jpeg`. Search type remains a free-form, case-insensitive plugin type, not an invented enum. Successful outputs and CLI parameter order remain unchanged.

Tools reject unknown fields (`additionalProperties: false`), so a misspelled `nodeId` cannot fall back to `0:0`; workflow enumerates topics. **Breaking change:** MCP no longer translates arguments into CLI flags. Replace `depth: "0"` with `depth: 0`, `scale: "2x"` with `scale: 2`, and uncapped `maxSize: null`/`false` with `maxSize: 0` (omission means **1600**, not uncapped). Other optional null/non-boolean false values fail; boolean false is valid. Flag-like nodes are not CLI options. No legacy mode marker remains. Under `--root`, former legacy calls return `invalid_arguments`, not `permission_denied`. CLI is unchanged.

`variables` retains the REST object by default and also supplies it as `structuredContent`. DTCG writes the standard token tree to `out` and returns `{path, collectionId, modeId, tokenCount, diagnostics: []}` instead. Invalid export combinations fail before source I/O. Conversion failures return `operation_failed` with a JSON diagnostic report in text content and publish nothing; source/output failures retain their existing codes. The tool declares `readOnlyHint: false` and `destructiveHint: true` because its opt-in export can replace an existing output, never the source. Host approval must cover writing; root admission/rechecks and the complete 8 MiB reply budget are unchanged.

Tool failures retain `isError: true` and text content; additive `_meta.figlens` carries stable `code` and corrective `nextStep`. Success payload schemas do not change:

| Code | Meaning / next action |
|---|---|
| `invalid_arguments` | Missing/wrong/invalid tool parameters or unknown tool; consult `tools/list` |
| `source_unreadable` | Local or remote source cannot be read/decoded; check valid local copy and authorized source access |
| `node_not_found` | Resolve again with find/tree or inspect the containing instance; retain full IDs |
| `unsupported_node` | Target exists but that operation cannot use it; use supported inspection/export or a drawable node as directed |
| `parse_budget_exceeded` | A named finite parsing budget was crossed; use a smaller copy or report the limit |
| `output_write_failed` | Bundle packing or export writing failed, or an output's folder inside a root does not exist yet (the message names it, #160); create an authorized writable directory and check space/permissions |
| `permission_denied` | Configured roots forbid the source/output; retain the restriction |
| `workspace_busy` | Another process holds the workspace, or a lane or checkpoint quota is full; retry once clear (a quota: follow `recovery`), never kill it |
| `storage_unavailable` | The workspace's storage is missing or offline; follow `_meta.figlens.recovery`: restore it, or with the user's confirmation recreate elsewhere |
| `index_incompatible` | Another schema or index version; follow the recovery (`rebuild`, or that figlens version) |
| `query_work_limit` | Over 10 sources or 100k records per search; run a listed narrower one |
| `mode_unavailable` | `semantic` search or `index` `embed` with no usable model profile; follow recovery to `models` status |
| `source_stale` | Every searched source changed since indexing; run the listed `refresh`, then search again |
| `cursor_stale`/`cursor_expired` | Paged data was replaced or deleted, a workspace search cursor's one-hour lease ended, or a `changes` cursor was issued for another view; restart paging, for `changes` and `watch` without the cursor |
| `event_gap` | Retention deleted `changes` revisions before they were read (or before a `watch` `ack` cursor accepted them); follow its recovery (from the oldest kept) and treat the scope as possibly changed |
| `cancelled` | The job was cancelled; the previous index still serves |
| `operation_failed` | Other existing operation error; inspect its diagnostic without assuming a more specific category |

JSON parsing (`-32700`), invalid JSON-RPC envelopes (`-32600`), unknown methods (`-32601`) and a missing/non-string `tools/call` name (`-32602`) remain protocol errors. Business/argument failures inside a named tool call are tool results. Envelope-valid messages without `id` are ignored before preparation/I/O, even with unknown methods or invalid arguments. `id: null` gets a reply. Omitted `jsonrpc` remains accepted; prefer `"jsonrpc":"2.0"`.

See the [GCS cookbook](gcs.md) for download versus parsed-tree caching and [bounded baseline](performance.md) for measurement limits. `scripts/mcp_smoke.py` shares its real subprocess session with GCS mock checks and measurement; no model, sockets or paid API are needed. The candidate installation matrix runs the same contract assertions through native and packaged wrapper stdio paths.

## Stale hint on design tools

A `node_not_found` failure keeps its code and recovery and gains a "save a new local copy" sentence, and a `find` with no results gains `freshness` (the design report plus `hint`), only when `FIGMA_TOKEN` is set and the mapped copy is older than its online file; details in [freshness](freshness.md#stale-hint).

## verify font lookup

`verify` takes `bundle` and an optional absolute `fonts` folder of project font files. Only `fonts` is admitted and rechecked under `--root`, like `matchDir`; the platform's user and system font folders are read (name tables only) regardless of roots, and a link in `fonts` must resolve inside it.

Each `fonts` entry gains `status` (`local_file` or `unverified`; neither shows the browser loads the face), `localFiles` (up to 3 `{path, matchedBy: postScriptName|family+style|filename, inProject}`) and, for a match inside `fonts`, `fontFace`, a ready `@font-face` (nothing is copied; licensing is the user's call). `fontLookup {searched, filesRead, truncated}` is always present: `truncated: true` means a scan budget stopped the search or a valid font was skipped for a per-font cap, so an `unverified` face may still have a file. The `font/unverified` finding stays and carries `evidence.localFiles`. Budgets and matching rules: [bundle](bundle.md#readiness-check-2026-10-28).
