# MCP workflow discovery

Use this topic for MCP instructions, tool discovery and built-in workflow guidance. Design decoding and result contracts are in `fig-format`; Pi-specific commands and tools are in `pi`; OpenCode/OMP MCP registration, override policy and host permissions are in [integrations](integrations.md).

## Startup arguments

Client `args` pass `--root DIR` and `--storage-root DIR` after `mcp`; these are the only startup options, both optional and repeatable. Use absolute paths to existing directories. Without `--root`, figlens adds no design/output path boundary beyond OS permissions; it neither defaults to the project nor scans the computer. These boundaries apply only to figlens MCP, not other host tools. `--storage-root` authorizes explicit custom storage; it does not choose a location. The `index` tool's `storage` argument chooses it, and `prepare` remembers it for later hosts without another flag. See the [README parameter table and multi-root JSON example](../README.md#mcp-args) and [workspace authorization](workspace.md#mcp-authorization).

## Roots in a checked-in config

A config committed to Git cannot hold one machine's absolute paths, and a relative `--root` resolves against the client's start directory. When git-ignored designs live only in the main checkout, a relative `figma` root does not exist inside a linked worktree. Point the client at a script in the repository that resolves the roots with Git, then starts figlens:

```sh
#!/bin/sh
# figlens MCP with absolute roots that also work in linked worktrees (Git >= 2.31)
set -eu
cd "$(dirname "$0")"
top=$(git rev-parse --show-toplevel)
main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
mkdir -p "$top/.figlens" # figlens refuses a missing root
set -- --root "$top/.figlens"
if [ -d "$main/figma" ]; then set -- "$@" --root "$main/figma"; fi
exec npx -y @runkids/figlens mcp "$@"
```

```json
{ "mcpServers": { "figlens": { "command": "scripts/figlens-mcp.sh" } } }
```

These roots allow scratch output in `.figlens/` and reading the shared designs, so figlens MCP never writes source files: the agent copies a shipped asset from `.figlens/exports/` into the project with its own tools, under the host's permissions. Add the project's asset folders as roots when figlens should export into them directly. The command path is relative, so the client must start the server from the repository root, and the script needs a POSIX shell.

## Intent and tool selection

`src/mcp.rs` serves stdio JSON-RPC; `src/main.rs` configures roots and starts it. Initialization returns a short intent router: resolve recognizable screens through `find`/`tree` (or `search` for effective instance copy), read compact values with `inspect`, full style/mask/component data with `nodes`, tokens with `variables`, and pixels/assets with `export`. `bundle` writes a scoped handoff, `verify` checks it. Only if asked for an index: `index` prepare with a scope, `jobs` status, `search` with `workspaceId`, also by `image` or `imageRef` for screens that look like a given image. `usages` takes an exact id on the file or within a prepared scope. Outside the router, `models` lists, configures or removes the local model profiles: the text one `search` `mode` `semantic`/`hybrid` and `index` `embed` need, and the image one workspace `search` with `image`/`imageRef` uses ([semantic](semantic.md), [visual search](visual-search.md)). `diff` pages changes between two local revisions ([diff](diff.md)). Only if asked to follow changes: `index` track, then `changes` with a consumer and `watch` ack of each handled page ([changes](changes.md)).

`inspect` and `bundle` accept optional `states: true` (CLI `--states`) for a bounded, source-scoped list of explicitly related component variants. The agent supplies this per-call argument when related states are requested. Inspect adds `states`; bundle adds `design/states.json`, a manifest pointer and a bounded context summary. Independent inventory truncation/coverage must be checked; supported prototype fields retain exact sources and unknown markers, without business inference. Both reuse their selected effective subtree and do not render alternate variants. See [related variants](states.md) for evidence, batch usage and limits.

For two selected variants, `inspect` accepts `compareVariant: "other-node-id"` (CLI `--compare-variant ID`) and adds `variantComparison`. Both IDs must be stored component definitions in the same set. It compares bounded raw root fields and descendant value multisets, retains exact sources/unknowns, pairs children by name, component or order (`pairedBy`), and names bound variables. `compare: "other-node-id"` (CLI `--compare ID`) takes any two nodes and adds `comparison`; see [states](states.md#compare-any-two-nodes).

`inspect` accepts `fullText: true` (CLI `--full-text`, complete `characters` up to 10,000 per text) and `colors: true` (CLI `--colors`, `solidFills` on nodes with visible SOLID fills); both default false and also apply to `flat`, so the default reply is unchanged. `inspect` accepts `flat: true` (CLI `--flat`): the content text is one line per node and `structuredContent` is `{flat, truncated}` without `node`, which is why the inspect output schema requires only `truncated`. `export` accepts `matchDir` (CLI `--match-dir`) for SVG output; it is admitted and rechecked under `--root` like `out`, and the reply adds `matches` and `similar` (paths relative to that folder; [render](render.md)).

`export_batch`: [render](render.md). `compare_image`: [compare-image](compare-image.md).

`variables` adds `format: "rest" | "dtcg"` (default `"rest"`) optional string fields `collection`, `mode`, `out`, and an optional `aliasModes` map of collection ID to mode ID. REST preserves its existing node-scoped/all-local response and rejects these export fields. DTCG requires all three, uses exact collection/mode IDs, and rejects `node`; inspect REST variables first, create the output parent directory, then request the export. It reads the live tree, not a workspace/search index. Conversion diagnostics are separate from the token tree: any diagnostic returns `isError: true` without changing the output. Existing root confinement, source-safe publication and complete-reply budgets apply. Pi, OpenCode and OMP use this shared MCP surface, with no dedicated token UI or converter. See [DTCG mapping and identity](fig-format.md#dtcg-token-export); review the result before replacing project tokens.

The `workflow` tool lets a model retrieve detailed guidance without a separate skill installation:

| `topic` | Use when |
|---|---|
| `build` | Implement a design as frontend code with the project's components, tokens and architecture, review that code, then compare and correct the running UI |
| `audit` | Compare existing UI with a design, report evidence and coverage, and correct confirmed differences only when requested |
| `images` | Prepare website assets using the project's image pipeline and verify dimensions, crops, bytes and delivery |
| `search` | Check for or enable an index, maintain it, semantic and image search, `usages`, following changes; build and audit carry only its decision table |
| `plan` | Plan an implementation or carefully reproduce a design; stop at the plan for plan-only or plan-first requests |
| `planning-details` | Detail page/multi-page scope or a specific unresolved design/project dependency |
| `yolo` | The user explicitly requests autonomous implementation and cleanup of this run's temporary plan, scratch exports and processes after successful checks; builds tokens, components, shell, then screens one at a time as hand-written frontend code, reviewed before pixel checks; includes the build sections every run needs, not the index guide |

The tool requires only an enumerated `topic`, accepts no file/path argument and rejects extra properties. Reading guidance neither reads a project nor writes, implements, verifies or cleans it. The current agent uses its host tools for project inspection/edits, directory creation, bundle extraction and browser checks. Host permissions still apply. A guide is not an implementation result or a plan-cleanup guarantee.

## Shared sources and runtime behavior

CLI and MCP index calls share action execution and option applicability in `src/command/index.rs`. MCP retains schema preflight, admitted paths, storage confinement and background job scheduling; CLI retains its positional syntax, directory dry-run/manifest flags and foreground job completion. Pi, OpenCode and OMP use the same MCP contract without host-specific changes. Workspace search, library and source usages share published rows and readiness checks in `src/workspace/readiness.rs`, after visibility filtering within the same read transaction. Reader-specific schema gates, cursors and recovery/result policies remain separate; only search reports interrupted jobs beside current publications. New focused tests live in adjacent `index/tests.rs` and `readiness/tests.rs` files.

Stdio request lines are capped at 1 MiB including line endings before JSON parsing or admission. Oversized lines are drained through their newline and return JSON-RPC `-32600` with a null ID; invalid UTF-8 returns `-32700`. The next request still runs, warm document included. This is a byte limit, not an elapsed-time limit on a peer that never finishes a line.

Complete serialized replies, including escaping, structured content, inline base64 and newline, are capped at 8 MiB. The server counts serialization without allocating a second full reply buffer. Over-budget tool replies become `isError: true` with `_meta.figlens.code: response_budget_exceeded`; completed bundle/export files remain and their path is retained as `_meta.figlens.outputPath`. Full design JSON is never silently truncated. MCP `nodes` also applies the budget while building: once the selection's compact node fields, counted without children, exceed it, building stops with the same error before the full JSON exists. That count is a lower bound of the reply, so no reply that fits is rejected. For a stored root, instance expansion stops the same way, counting each expanded node's emitted `id`, `name` and `type`. For an instance sublayer, the outer instance still expands and builds first; `inspect`, `variables`, `bundle` and the CLI do not use this guard. See [measurement](performance.md#early-mcp-nodes-budget-2026-10-29). Use scoped inspect/bundles, CLI nodes, or a smaller export `maxSize`. Non-tool protocol replies return `-32603` on overflow. This transport policy leaves headroom below a reported 10 MiB client limit; it is not a guarantee for every MCP client or a process-memory limit. CLI output remains uncapped. See [measurements](handoff-research.md#mcp-message-sizes).

`src/workflow.rs` embeds `skills/figlens/SKILL.md` and its workflow references with `include_str!`. Build includes the font/asset preflight: check required font faces and asset files, report missing/unverified dependencies, and verify actual browser loading before acceptance. `document.fonts.check` alone can pass on fallback. Bundle `context.md` also carries a pre-implementation warning: project font availability starts `UNVERIFIED`, even with no bundle warnings. Missing/unverified inputs do not block implementation: use documented project-font fallbacks or labelled image placeholders at the intended dimensions, retaining exact gaps and remedies in the handoff. Only final visual acceptance remains pending for unresolved inputs. Decoding, assets and reference pixels are unchanged; `verify` alone looks fonts up by name ([contract](mcp-contract.md#verify-font-lookup)). Build, audit and images return relevant level-two sections, listed in `compose` in `src/workflow.rs`; `out` descriptions repeat the location rule. Plan, planning-details, YOLO and search add their reference. Pi and the portable skill use these sources too; CLI `figlens workflow <topic>` prints them with a CLI preamble instead of the MCP one. Update section selectors if a selected heading changes.

Audit adds `references/audit.md` to the shared evidence rules also returned by build: compare elements, visual relationships and supported variations, identify scope/design/code evidence and distinguish static review from actual screenshot comparison. Scope coverage records structure-only and unexamined items separately. Reuse current snapshot evidence; stop an unresolved item when another read cannot add evidence, with its missing input and next action. Browser restrictions leave visual acceptance pending rather than blocking independent static work. Review-only requests grant no application-edit permission. CLI exposes `figlens workflow audit`; Pi discovers it through the same MCP schema and packaged skill, without a separate command or mapping change.

Local reference links in returned text become `workflow:<topic>#<optional-section>`; the agent follows them with another call to this tool. No runtime Markdown files, network fetch or skill discovery mechanism is required. The binary can serve guidance from an empty working directory, including under `--root`; design/output path restrictions remain in force for design tools.

The client decides whether initialization instructions enter model context. The workflow tool's description also exposes the choices so retrieval is discoverable through `tools/list`. No model is proven to pick or follow the right guide.

MCP source failures hide raw paths/cloud stderr. Local CLI errors retain their diagnostics; URL-source errors are redacted in both channels. Error categories follow typed provenance in the existing `FigError`, not human-text matching. They are a recoverability aid, not a promise that every possible process failure is caught.

Workspace calls (`index`, `jobs`, `changes`/`watch`, `search`/`usages` by id) skip the document thread. The reader only frames and parses; `jobs`, index `status` and `models` list/status/remove go to a control lane, document tools and workspace search to the interactive lane (keeping the warm document), and build-submitting `index` actions and `models` configure to an index lane feeding a one-at-a-time build lane, and model jobs (the `embed` and `visual` actions, and those queued after a text build publishes) to a separate model lane, so a text build never waits for the model ([semantic](semantic.md#workspace-layer)). The reader answers `ping` and `tools/list` at once, possibly before earlier tool replies; pair replies by id. Lanes queue 64, 64 and 8 calls, the build lane 8 submissions; past that, `workspace_busy` at once. `notifications/cancelled` drops a queued call unanswered; a running semantic search or `models` configure stops with its model child, other running calls finish (stop a build with `jobs cancel`). Blocking I/O runs in the executing lane, so status and cancel answer during a build or slow search; stdin EOF or a write error cancels builds before exit. Storage roots (`--storage-root`) and `--root` visibility are in [workspace](workspace.md#mcp-authorization); jobs are in [index](index-jobs.md).

Admission stores checked paths even without roots; execution/cache use them. Reply labels and encoders survive symlinks; missing parents stay missing. Roots reject dangling links/escapes. Cache reuse rechecks policy, readability, mtime and length.

Frontend framing lives in Building a frame, the workflow references, bundle `context.md`, the viewer prompt and `INSTRUCTIONS`; change them together (tests in `src/workflow.rs`).

## Verification

Run Docker Rust tests/lint and `python3 scripts/ai-context.py check`. Tests cover schema/defaults, admission, notification side effects, cached-source changes and rooted/unrooted paths. After building, run `docker compose run --rm dev python3 -B scripts/check-mcp-admission.py --binary /work/target/debug/figlens` for actual POSIX warm-cache permission denial/recovery. Pi/npm tests and source inspection alone do not prove candidate runtime compatibility or model/browser behavior.

## Parameter contract and recoverable failures

Parameter ranges and defaults, unknown-field rejection and the `_meta.figlens` failure codes are in the [MCP contract reference](mcp-contract.md); read it when changing a tool's parameters, a failure code or a JSON-RPC error.
