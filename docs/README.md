# figlens — User guide

[← README](../README.md) · [figlens.runkids.cc](https://figlens.runkids.cc/docs/)

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

- [Standalone installation](#install)
- [Why figlens instead of the official Figma MCP?](#why)
- [What the agent gets](#bundle)
- [Pi extension](#pi)
- [Browse a design without Figma](#viewer)
- [MCP server and agent skill](#mcp-and-skill)
- [CLI reference](#commands)

<a id="install"></a>

## Standalone installation

**No Node.js?** Install the standalone binary and use `"command": "figlens", "args": ["mcp"]` instead:

| Platform | Command |
|---|---|
| macOS / Linux | `curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh \| sh` |
| Homebrew | `brew install runkids/tap/figlens` |
| Windows (PowerShell) | `irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 \| iex` |

<a id="why"></a>

## Why figlens instead of the official Figma MCP?

Use **figlens** when the design should travel as a file: remote agents, CI, offline work, handoff between agents, unlimited repeated reads. Use the **official Figma MCP** when the agent must see the live design without an export step.

| | figlens | Official Figma MCP |
|---|---|---|
| Source | Exported `.fig` snapshot | Live Figma files |
| Needs | The file | Figma sign-in; desktop server needs the app |
| Rate limits | None, local parsing | Figma service limits |
| Remote agent / CI | Copy the file, run the binary | Remote server with sign-in |
| Design changes | Export a new `.fig` | Automatic |
| Handoff | Share the `.fig` or a frame bundle | Share Figma access |
| Fidelity | Independent parser with [known gaps](../README.md#limitations) | Maintained by Figma |

Reading a `.fig` is offline. Installing and updating figlens, `gs://` inputs, your model provider and your project's dependencies still need a network.

<a id="bundle"></a>

## What the agent gets

One frame, one zip. Structure to build from, assets to use, a picture to check against.

```text
frame.zip
├── context.md                Start here: instructions for your agent
├── design/
│   ├── nodes.json            Layout, text, styles, Dev Mode notes and expanded instances
│   ├── fonts.json            Font faces the visible text uses, with node ids
│   └── variables.json        Bound variables, collections, modes and aliases
├── assets/                   Image fills, SVGs and composited node renders
├── assets.json               Maps design references to asset files
├── references/screen.png     A render of the frame to compare against
├── missing-assets.json       Anything that could not be produced
└── manifest.json             File roles, media types, sizes and SHA-256 hashes; the source .fig hash with --hash-source
```

Need one piece? The CLI does it directly:

```sh
npx -y @runkids/figlens find design.fig "login" --type FRAME     # frame ids by name or text
npx -y @runkids/figlens inspect design.fig 12:345                 # compact layout and text summary
npx -y @runkids/figlens nodes design.fig 12:345 > frame.json      # REST-shaped JSON
npx -y @runkids/figlens export design.fig 12:345 icon.svg         # SVG, PNG, JPG, WebP or AVIF
npx -y @runkids/figlens bundle design.fig 12:345 frame.zip        # the zip above
```

SVG exports embed original image fills and outline text as glyphs, so no fonts are needed. Through MCP, raster exports also come back as inline images, so the agent sees the design without opening a file.

<a id="copy-a-selection"></a>

### No saved .fig? Copy a selection

1. In Figma, select the frame or layers and copy them: right-click > **Copy**, or Cmd+C (Ctrl+C on Windows and Linux).
2. Run `figlens clipboard screen.fig`, or ask your agent to build "the screen I just copied". It reads the clipboard and writes a `.fig` every command reads.
3. Pass `screen.fig` like any other file: `figlens bundle screen.fig <node> frame.zip`.

Pasting the copy into a text field gives only its text; figlens reads the copy's HTML, which holds the layers. macOS is checked against real copies; Windows (PowerShell 5.1) and Linux (`wl-paste` or `xclip`) are not yet.

| | **File > Save local copy** | Copied selection |
|---|---|---|
| Scope | The whole file | The layers you copied, plus the components they use |
| Image fills and vector outlines | Included | Missing; exports warn where they draw without them |
| A Figma link finds the node | Yes | Yes, for copied layers: ids match the source file |
| Fits | Building from a design you keep beside the code | A quick look at one screen when no saved copy exists |

<a id="accuracy"></a>

### How close is it to Figma?

Measured against Figma's own JSON, PNG and SVG exports on **826 production frames** and five Community files (2026-10-22):

| Check | Result |
|---|---|
| Node structure, names, visibility, bounds and text | Identical in 771 / 826 frames |
| Original image fills | 2,608 / 2,608 byte-identical |
| SVG assets | 5,014 / 5,197 within the pixel-difference threshold |
| Community frame bundles | 306 / 306 with no missing assets |

Results on those files, not a guarantee for every design. Method and thresholds: [accuracy report](../wiki/accuracy.md).

<a id="pi"></a>

## Pi extension

```sh
pi install npm:@runkids/figlens
```

Installation guides: [OpenCode V2](../opencode/README.md) · [OMP](../omp/README.md).

Restart Pi or `/reload`, then ask for a screen in plain words. Pi finds the frame, reads it over the auto-registered MCP server and builds with your stack. No node IDs, no global CLI.

Asking to find copy or a screen in a design searches the `.fig` directly and stores nothing. For repeated searches, ask Pi to enable an index for a design and scope: it prepares a local index once (no setup command), reports the job until it is ready and reuses it across restarts and hosts. Resetting that index needs your explicit request. Asking where a component, variable, style or image is used lists its exact usages the same way: from the `.fig` directly, or from that index once enabled. Asking Pi to follow a design's changes tracks that scope, preparing its index if needed, and reports what changed since it last checked; a question about two saved copies is answered with `diff` and starts nothing. Details: [local index](#local-index).

- `/figlens plan <request>` writes a plan only; it does not edit the application.
- `/figlens yolo <request>` plans, builds, checks and cleans up its own temporary plan, scratch exports and processes.
- `/figlens preview [description]` opens the local viewer.
- `/figlens review` approves reusable component mappings the agent proposed.

![Ask Pi for a screen, let it find the design, build with your stack, and check the result](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/pi-workflow.svg)

Full walkthrough, commands and limits: [pi/README.md](../pi/README.md).

<a id="viewer"></a>

## Browse a design without Figma

[![Inspect the MCP Apps design locally, with its layer tree, layout and component properties](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/preview-inspector.png)](https://github.com/runkids/figlens/blob/v0.10.1/assets/pi-demo.mp4)

```sh
npx -y @runkids/figlens preview design.fig                        # local viewer, Ctrl+C to stop
npx -y @runkids/figlens preview design.fig 12:345 --out preview.html   # offline HTML snapshot to send to anyone
```

Browse pages and layers, search, run ranked search and exact usages of the selection, measure, inspect layout, typography, variables and CSS, export any layer, copy an implementation prompt or download a bundle for the selection. Switch ranked search to semantic or hybrid with an optional local model: the viewer indexes the file only when you press Enable index. Drop in a screenshot of your build to overlay it on the design. Details: [preview guide](../wiki/preview.md). Visual/Hybrid accepts a reference PNG or selected layer with a pixel crop. Enable visual indexing explicitly; query images are not stored.

<a id="mcp-and-skill"></a>

## MCP server and agent skill

**MCP.** The server runs locally over stdio and exposes `workflow`, `tree`, `find`, `search`, `usages`, `diff`, `inspect`, `nodes`, `variables`, `bundle`, `verify`, `index`, `jobs`, `changes`, `watch`, `export`, `export_batch`, `compare_image` and `models`. `export_batch` writes up to 64 files in one call and returns a manifest instead of inline images. The agent can read built-in `build`, `audit`, `images`, `search`, `plan`, `planning-details` or opt-in `yolo` guidance through `workflow`, then use its own project and browser tools to carry out the authorized task. These guides share the portable skill's source; installing that skill separately is optional for MCP users. Client support and model behavior determine how the guidance is used. Pass absolute paths. Add `--root /path/to/designs` (repeatable) to confine every file and output path to chosen folders. Explicit custom index storage needs `--storage-root DIR` (repeatable); remembered workspace locations are reused without it. See [startup arguments and a multi-root config example](../README.md#mcp-args). Raster exports are scaled down to `maxSize` (default 1600 px) so inline images stay small; SVG returns metadata only.

Native Windows with Claude Code: follow its [stdio setup notes](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server) and the `cmd /c` wrapper for `npx`.

**Skill.** The [figlens skill](../skills/figlens/SKILL.md) teaches an agent to find frames, read the bundle in small pieces, plan scoped work and verify the running UI against the design. It installs instructions only; the CLI or MCP server must be available. Without it, `figlens workflow <topic>` prints the same guidance from the binary, matching its version. Install with the [Skills CLI](https://github.com/vercel-labs/skills) or copy `skills/figlens/` into your agent's skills directory (Claude Code: `~/.claude/skills/`):

```sh
npm install -g @runkids/figlens
npx skills add runkids/figlens --skill figlens
```

**Plugins.** Claude Code, Codex, Antigravity, Cursor and Kimi Code install the skill and the MCP server together as one plugin; see [Plugin setup](../README.md#setup-plugin).

**skillshare.** Manage the skill and the MCP connection across Claude Code, Codex and Cursor from one place with [skillshare](https://github.com/runkids/skillshare):

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync && skillshare sync mcp --global
```

**Install notes.** The npm launcher downloads the matching native binary on first use, verifies it against the release's SHA-256 checksums and caches it. Script installs update with `figlens update`; Homebrew installs with `brew upgrade figlens`. Binaries cover macOS (Apple silicon, Intel), Linux (arm64, x86_64, static musl) and Windows (x86_64). Every install path also puts `figlens-embed`, the runtime for the [optional semantic search](#semantic-search), beside `figlens`; it loads no model until you configure one.

<a id="local-index"></a>

## Ordinary search and the optional local index

|  | Ordinary (default) | Advanced: local index (optional) |
|---|---|---|
| What it does | Reads the `.fig` on each call and answers `search`, `find`, `inspect` and `export`. | Builds a search index of a chosen scope once, then answers `search` from it, ranked across sources. |
| What is written | No workspace or index database (`export` writes only the files you ask for). | A local workspace (SQLite database) in the per-user state directory, or the `--storage` you chose. |
| When to use | One-off questions, scripts, CI. | Repeated searches of one file; several files of a project as one corpus (`--workspace <dir>`); exact usages of components, variables, styles and images. |

A normal design question never creates an index. You opt in once, explicitly ("create an index for this design"); figlens picks the storage and needs no SQLite or other setup.

```sh
# Ordinary: stateless, creates nothing
figlens search design.fig "checkout button"

# Advanced, once: pick a scope (a node id, or 0:0 for the whole file); the build runs in the foreground
figlens index prepare design.fig --scope 0:0

# Indexed search: pass the scope you prepared as --node
figlens search --file design.fig "checkout button" --node 0:0

# After you edit or re-export the .fig: rebuild only the scopes whose bytes changed
figlens index refresh --file design.fig

# After you move the .fig: keep the workspace, then refresh (use the workspaceId printed by prepare)
figlens index relocate <sourceId> new/design.fig --id <workspaceId>
figlens index refresh --id <workspaceId>
```

MCP: `index` prepare returns a job; poll `jobs` until `succeeded`, then search with the `workspaceId` from the reply and `node` set to the prepared scope (without `node`, search reads `0:0`).

```json
{"tool": "index", "arguments": {"action": "prepare", "file": "/abs/path/design.fig", "scope": "0:0"}}
{"tool": "jobs", "arguments": {"action": "status", "jobId": "<jobId from the reply>"}}
{"tool": "search", "arguments": {"workspaceId": "<workspaceId from the reply>", "node": "0:0", "query": "checkout button"}}
```

Lifecycle: `refresh` rebuilds only scopes whose `.fig` bytes changed; `rebuild` rebuilds even when unchanged; `clear_cache` deletes only data a build can recreate and keeps the registry and published indexes; `reset` deletes this workspace's database (never the `.fig` or exports) and runs only on your explicit request. New sources, a wider scope, reset and recreate each need their own explicit request.

Moved the `.fig`? `index relocate` keeps `workspaceId`, `sourceId` and storage; then `refresh` the affected scopes. There is no auto-discovery and no hash-based merge, and no reset is needed.

Several designs in one folder? `index add <dir> --workspace DIR` registers up to 10 `.fig` files under that directory in one step, skipping links that lead out of it; the reply lists `added`, `existing`, `skipped` and `truncated`, a repeat adds nothing, and each file still needs its own `prepare --scope`. A file moved inside the folder is a new source unless you `relocate` it. Pass the reply's `nextCursor` as `--cursor C` (MCP `cursor`) to add the next 10. Paged search and library cursors last one hour; after that they return `cursor_expired`, so search again.

Data: the SQLite database holds registered local paths, the selected scopes' layer names and effective text, instances' component property values, search postings, exact relations (containment, instances, variants, variable/style/image bindings, prototype targets) and the definitions they point at, hashes, generations and jobs. The original `.fig` stays unchanged where it is; its raw bytes are not stored in the database. Semantic and vision model layers add the vectors they embed, and visual indexing you enable adds PNG renders of the indexed layers, even without a model; query images are never stored.

Pi, OpenCode and OMP use the same index through their own opt-in entry, and the workspace is reused across restarts and hosts. Not implemented: file events (a watch polls the file, and only while a holder runs) and push events.

<a id="semantic-search"></a>

## Optional semantic search

Experimental. `figlens search <file> <query> --mode semantic|hybrid` ranks layers by meaning with a local model. The default `--mode text` is unchanged and needs no model.

- **Set up once.** Download BAAI/bge-m3 at revision `5617a9f61b028005a4858fdac845db406aefb181` (MIT; `config.json`, `pytorch_model.bin`, `tokenizer.json`) yourself: figlens never downloads. `figlens models configure bge-m3 --model DIR [--runtime FILE]` checks the files by SHA-256, starts the `figlens-embed` runtime once offline (looked up next to the figlens executable unless `--runtime` is given) and records the profile in `models.json` in the config directory. `models list|status` show it, `models remove bge-m3` forgets it and keeps the files.
- **Visual profile.** `figlens models configure siglip-base --model DIR` checks google/siglip-base-patch16-224 at revision `7fd15f0689c79d79e38b1c2e2e2370a7bf2761ed` (Apache-2.0; `config.json`, `preprocessor_config.json`, `model.safetensors`) the same way and records the image model for visual search.
- **Image queries (experimental).** `figlens search --workspace DIR --image shot.png` ranks a workspace's units by how they look; `--image-ref '{"sourceId":…,"snapshotId":…,"nodeId":…}'` uses a result's stored render instead, `--crop x,y,w,h` cuts either. Adding query text makes it `hybrid`. Visual layers are built by `figlens index visual` (a search builds none, and a single `.fig` search refuses an image). Without the vision profile, or when it fails, results rank by pixel hash alone and `query.usedModes` says so. The query image is never stored. The ranking policy is still being validated, so treat results as hints. MCP: `search` `image` (a path inside the roots), `imageRef`, `crop`.
- **Modes.** `semantic` ranks each layer by its best chunk of name, text, component properties or scope. `hybrid` fuses the text and semantic rankings (Reciprocal Rank Fusion). Without a profile `semantic` fails with `mode_unavailable` and `hybrid` returns the text results and says why. `coverage.layers.semantic` reports `ready`, `partial` or `unavailable`; a partial page has no `nextCursor`. Models run on the CPU; Ctrl-C stops the active request. An MCP/viewer process can keep one text and one image model resident after use: text alone measured about 2.5 GiB, the two models' peak RSS summed to about 3.5 GiB in one Linux ARM diagnostic, excluding the core and index workers. These are measurements, not memory caps.
- **Limits.** Embedded: layer names, effective text, an instance's component property values (a value that cannot be resolved exactly is skipped and counted) and each layer's page and container names. `--workspace`/`--id` search reads each source's stored model layer and never embeds design text; build it with `figlens index embed` (the MCP server queues it after a text build). A source without it is `pending`, and the page is partial. On the frozen test suite for this feature the model missed the quality gate for synonym and purpose queries (wrong top-1 rate 0.25, gate 0.20), so treat results as hints.
- **MCP.** The `models` tool (`list`, `status`, `configure`, `remove`) and `search` `mode`; `configure` runs only the `figlens-embed` beside figlens, and the model directory and files must lie inside the server's roots. Cancelling the call stops a running search or setup.

<a id="commands"></a>

## CLI reference

| Command | Output |
|---|---|
| `tree <file> [node] [--depth N] [--json]` | Node ids, REST types (as in `nodes`) and names; indented text, or nested JSON with each node's size, hidden flag and auto-layout mode, plus `firstText` (first visible text) on nodes of a page or section |
| `tree <file> [node] --top-frames [--json]` | Only the screens, one per line, ready for `--frames -`: frames, components, component sets and instances on a page or in its sections (groups and internal pages skipped); `--json` adds page and section ids and each screen's first visible text (`firstText`), which tells canvas labels from screens |
| `find <file> <query> [--type TYPE] [--node ID] [--limit N]` | Case-insensitive search over layer names and text; paths, sizes and previews. Default 20 results (1-200) |
| `search <file> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C] [--include-hidden] [--include-internal] [--mode text\|semantic\|hybrid\|visual] [--image PNG\|--image-ref JSON] [--crop x,y,w,h]` | Ranked search of layer names and full text with instance overrides, text/boolean properties and swaps applied; exact node refs (including `I…;…` sublayer ids for `inspect`), match reasons, UTF-16 spans, coverage and cursor pages. Default 20 (1-200); hidden layers and internal pages need their flag. With `--file`/`--workspace`/`--id` instead of `<file>` it reads the workspace's prepared index (see `index`) `--mode semantic\|hybrid` needs a configured model: see [Optional semantic search](#semantic-search). |
| `models list\|status\|configure <profile> --model DIR [--runtime FILE]\|remove <profile>` | Local text model for semantic search (`bge-m3`, experimental) and image model for image search (`siglip-base`, experimental); `configure` checks the files you downloaded and never downloads; `remove` keeps them. See [Optional semantic search](#semantic-search). |
| `usages <file> <node\|resource> [--scope ID] [--relation R[,R]] [--out] [--snapshot S] [--include-hidden] [--limit N] [--cursor C]` | Exact relations of one layer or resource (`VariableID:…`, `StyleKey:…`, `image:…`) in one stored scope, nested instances and swaps included: where a component, set, variable, style or image is used (default), or what a layer points at (`--out`). Each result keeps its exact ref, relation, state (`resolved`, `missing`, `unresolved`, …), basis and evidence; names never count as identity. No totals: coverage says complete or partial. Default 50 (1-200). With `--file`/`--workspace`/`--id` instead of `<file>` it reads the workspace's prepared index of `--scope`. With `--out`, a use of a cached library copy also names its `library` {kind, namespace, key}. |
| `usages --file\|--workspace\|--id … --library <kind> --namespace <ns> --key <key> [--definition-source ID --definition-snapshot SNAP --definition-node NODE] [--source ID] [--scope ID] [--include-hidden] [--limit N] [--cursor C]` | Layers that use one library component, style or variable (`kind`), as each file's cached copy records it, across every source's prepared `--scope` (`--source` narrows to one). The definition stays `unresolved` unless the three `--definition-*` flags select it by exact ref (`selected`, `keyEvidence: unverified`); coverage lists each source's state and why it is partial. Name it from the `library` field of `usages --out` on a layer that uses it. |
| `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]` | What changed between two saved revisions: one line per changed top-level frame with counts per facet (text, layout, appearance, properties, structure, bindings); `--json` adds old/new field values and edge changes. Layers match by exact id and type only; a same-name or same-place frame is only a labelled possible match, and a side that stopped short, or a layer whose only difference is a field it could not read, reports unknown, never removed or changed. Pages of 100 changes (1-500) A side that stopped at a limit prints `stopped early` instead of `no changes`; narrow it with `--node`. |
| `inspect <file> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat] [--full-text] [--colors]` | Compact expanded layout/text summary, at most 200 nodes. `--images` adds bitmap dimensions and crop footprints. `--flat` prints one line per node instead: position relative to the node, size, text, font, fill, stroke, radius, auto-layout, alignment, fixed/fill sizing, shadows and blurs `--full-text` returns each text in full (up to 10,000 characters); `--colors` adds visible solid fills (hex, opacity, bound variable), also on `--flat`; no gradients, images or strokes. |
| `nodes <file> <node> [--compact [--subtrees]]` | The per-node object of a REST nodes response (`document`, `components`, `componentSets`, `styles`): expanded instances and overrides, auto-layout, text styles, fills, effects, bound variables, component properties, render bounds. Default REST is unchanged. `--compact` shares repeated styles; add `--subtrees` for repeated subtrees with per-node IDs and bounds. See the [lossless wire contract](../wiki/discovery.md#compact-nodes). Batch: `--frames` / `--ids` + `--compact [--subtrees]`. |
| `nodes <file> --frames <list>`, `--frames -` (stdin), `--ids <node>...` | One decode, many nodes: one JSON line per listed id, the `nodes` output with its `nodeId`, or `{nodeId, error}` |
| `variables <file> [node]`<br>`variables <file.fig> --format dtcg --collection ID --mode ID --out FILE` | Default: unchanged REST-shaped bound/local variables. DTCG 2025.10 export: exact collection/mode IDs, no node; colors and unitless numbers only. Create the output directory first; diagnostics fail the whole export without changing the output. A library collection exports from the copies cached in the file (`libraryCopy: true`). An alias into another collection exports that collection's value, recorded in `aliasOf`; several target modes need `--alias-mode COLLECTION=MODE`. |
| `bundle <file> <node> <out.zip> [--states] [--hash-source]` | A design bundle for one frame; prints `{nodeId, path, warnings}` |
| `bundle <file> --frames <list> --out-dir <dir> [--jobs N] [--states] [--hash-source]`, `--frames -` (stdin) | Many bundles from one decode, in parallel |
| `verify <bundle> [--fonts <dir>]` | Check a bundle `.zip` or its extracted directory against its manifest: verified, missing and changed files, missing assets, the font faces to confirm in the project, and `findings`: each missing, unsupported or unverified handoff input with its nodes, evidence and next action; a face with a font file on this machine or in `--fonts <dir>` (a project folder, which also yields an `@font-face`) is `local_file`, not proof the browser loads it |
| `export <file> <node> <out> [--bounds layout] [--scale N] [--quality 1-100] [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID]]` | SVG, PNG, JPG, WebP or AVIF; PNGs up to 4,194,304 pixels are automatically losslessly optimized, with decoded RGBA unchanged; original encoded bytes are kept unless smaller, and larger PNGs use regular encoding; `--max-kb` lowers lossy quality until it fits (PNG fails if still over budget, with no lossy fallback); `--bounds layout` exports the layout box instead of the render bounds; `--match-dir DIR` (SVG only, subfolders included) lists the SVGs there that draw the same outlines (`matches`) and up to 10 similar shapes with a score (`similar`); `--hide ID[,ID]` draws those layers inside the node as hidden over the same area, so diffing it with the plain export shows where that layer, as figlens draws it, changes the result (no difference does not prove the skipped rendering is harmless) |
| `export <file> --frames <list> --out-dir <dir> [--format F] [--scale N] ...`, `--frames -` (stdin) | One decode, many exports (default `png`): writes `<dir>/<nodeId>.<format>`, one JSON line per id, the `export` output with its `nodeId`, or `{nodeId, error}` |
| `export <file> --batch <items.json>`, `--batch -` (stdin) | One decode, many exports, each with its own options: a JSON array of 1-64 `{node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}` (MCP `export_batch`). Prints one manifest `{count, failed, items}`: per item the `export` output, or `{error}` (the others still export); a malformed item or a repeated `out` fails before any write |
| `preview <file> [node] [--out preview.html] [--port N] [--bind IP]` | Local viewer that never writes the design; Semantic/Hybrid ranked search after its Enable index; `--out` writes a standalone offline snapshot; results and copied context name the page and, live, the loaded file path, source snapshot and local revision (unknown offline, where the path is never embedded); context also carries the reference scope's coverage, `complete: null` when unchecked. Visual/Hybrid accepts a reference PNG or selected layer with a pixel crop. Enable visual indexing explicitly; query images are not stored. |
| `compare <file> <nodes.json> <node>` | Diff against a REST export of the same frame |
| `compare-image <file> <node> <screenshot.png>` | Compare a screenshot of the built screen with the node rendered at its scale (`--scale`, `--limit`, `--out diff.png`, `--full-page` for a whole-page capture shorter than the frame, `--ignore` for layers whose content comes from data, `--threshold` to see light borders; `image: true` marks image fills; MCP `compare_image`). Prints `mismatch`, `nonTextMismatch` and the largest differing regions in node pixels, each with the deepest layers under it; `text: true` marks regions over text (often font fallback; review each) |
| `node <file> <node>` | One raw `.fig` node |
| `clipboard [copied.html] <out.fig>` | For when there is no saved `.fig`: select layers in Figma, copy them (right-click > Copy, or Cmd/Ctrl+C), then run this to turn the clipboard into a `.fig` every command reads; pasting elsewhere gives only the layers' text. Reads the system clipboard (macOS; Windows PowerShell 5.1 and Linux `wl-paste`/`xclip` unverified) or a saved clipboard HTML file. Same node ids, used components included. Reports `missingImages`, `missingComponents` and `layersWithoutOutlines`: a copy carries no image bytes or vector outlines, so exports and bundles warn where they draw without them. Never replaces an existing file. With a saved `.fig`, pass the Figma link as the node instead: it keeps images and vectors |
| `index prepare\|refresh\|rebuild\|clear\|status\|add\|remove\|relocate\|reset\|track\|untrack\|embed\|visual` | Local workspace in the per-user figlens state directory, or under a `--storage DIR` that the workspace then remembers. `prepare <file> --scope NODE` builds that scope's search index once; then `search --file <file>` (or `--workspace DIR`) answers without parsing, ranked across the workspace's sources. `refresh` rebuilds changed files, `rebuild` all or one `--source`/`--scope`, `clear` drops unused indexes; the previous index serves until a new one is ready. `reset` removes only the workspace database, never designs, exports or the remembered location. `track --scope NODE` records that scope's changes from a baseline build; `untrack` stops, keeping its history. `embed` embeds published scopes with the configured model, using CPU and memory; run it only when you want semantic workspace search. `visual --scope NODE` renders that published scope's screens and components for image search, embedding them when a vision profile is configured; run it only when you want visual workspace search. |
| `jobs status\|cancel <jobId>` | Show or cancel an index build; cancel returns after the build stopped |
| `changes --file <file>\|--id ID\|--workspace DIR [--source ID] [--scope NODE] [--consumer NAME\|--subscription ID\|--cursor C] [--revision R] [--limit N]` | Recorded changes of tracked scopes, oldest first: one revision per published build (baseline or changes) with coverage and counts per change kind and facet; each changes revision has an `impact` summary of the layers its changes reach (null before schema 7); `--revision` pages that revision's changed nodes, then the layers its impact lists (`entry` says which); `tracking` says when a scope needs `index refresh` to record a change. Reads only |
| `watch ack --file <file>\|--id ID\|--workspace DIR --consumer NAME\|--subscription ID --cursor C [--source ID] [--scope NODE]` | Move a consumer's checkpoint to the `nextCursor` of the last `changes` page it handled with the same filters; repeating it is safe, and it only moves forward except that a current recovery cursor moves a checkpoint behind a deletion back to replay the kept revisions; a cursor issued before retention deleted revisions it had not passed is `event_gap` and writes nothing |
| `watch run --file <file>\|--id ID\|--workspace DIR --scope NODE [--source ID] [--request-id R]` | Hold a lease on a tracked scope in the foreground: the workspace's shared runner refreshes it on each save (polled every 250 ms, content checked every 60 s), so `changes` shows new revisions with no `index refresh`. Prints the lease, then one end line when stdin closes, on Ctrl-C or SIGTERM, or when the scope is untracked. MCP `watch` `start` and `stop` hold leases while the server runs; an unrenewed lease ends after 30 s and the runner exits after the last one |
| `watch status --file <file>\|--id ID\|--workspace DIR` | The workspace's leases (`active`, `unauthorized`, `untracked`, `expired`, `ended`) and runner (`running`, `stopping`, `stopped`, `unknown`, with its placement); never writes |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]` | Follow a source, scope or exact layer (`--node`) from its newest revision with its own checkpoint, read with `changes --subscription` and acked with `watch ack --subscription`; a missing or ambiguous layer is `target_missing` or `identity_uncertain`, never a same-name match; `--owner` makes it temporary for that task |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID --query TEXT [--type T[,T]] [--scope NODE]` | Save a search of one source's scope: its top 50 are evaluated now; `changes --subscription` reads the stored evaluations and never computes one |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --sources visible\|ID[,ID] --query TEXT\|--library KIND --key KEY [--namespace NS] [--definition-source ID --definition-snapshot SNAP --definition-node NODE]` | Save a search, or a library item's uses with their `definitionMatch`, over a fixed set of at most 10 sources (`visible` is resolved now; later sources are never added): `delta.changedSourceIds` names sources with new bytes, `corpus_changed` a set change, `coverage.transitive` is `not_followed` and `delta.notFollowedSourceIds` names changed sources holding no use, whose dependents were not re-checked; removing a member disables it (unsubscribe, then subscribe again) |
| `watch evaluate --file <file>\|--id ID\|--workspace DIR --subscription ID` | Store a query subscription's evaluation of the scope's current index, unless it is already evaluated; a watch on the scope does this after each publication |
| `watch list --file <file>\|--id ID\|--workspace DIR [--consumer NAME] [--after ID] [--subscription ID]` | List the visible subscriptions and their state, a page at a time; `--subscription` reads only that one (empty when not visible) |
| `watch unsubscribe --file <file>\|--id ID\|--workspace DIR --subscription ID\|--consumer C --owner NAME` | Remove one subscription and its checkpoint, or an owner's temporary ones; tracking and history stay |
| `watch forget --file <file>\|--id ID\|--workspace DIR --consumer NAME [--source ID] [--scope NODE]` | Remove a consumer's raw checkpoints; a consumer keeps at most 64 subscriptions and checkpoints and a workspace 4096, and nothing is evicted automatically |
| `mcp` | Local MCP server over stdio |
| `update` | Replace the binary with the latest release |

For related component variants, use `inspect --states` or `bundle --states` (MCP `states: true`, default false; no configuration change). Evidence retains source, unknown and truncation markers. Bundles add `design/states.json` and a `context.md` summary. See [state evidence](../wiki/states.md) for scope and limits.

To compare two variants in the same component set, use `inspect <file> <variantA> --compare-variant <variantB>` (MCP `compareVariant`). `variantComparison` reports stored field/value-multiset differences with source IDs and unknowns; descendants pair by unique name, then by main component, then by stacking order among equal-sized same-name groups (`pairedNodes`, with per-pair changes; check `pairedBy`, `order` is the weakest evidence; the rest are unpaired), and bound variables are named. See [comparison limits](../wiki/states.md#compare-two-variants).

To compare any two nodes of one file (an empty and a filled screen, mobile and desktop breakpoints, two screens sharing a header), use `inspect <file> <nodeA> --compare <nodeB>` (MCP `compare`). `comparison` has the same shape plus `rootDelta` (size and parent-relative position); children pair by unique name, then by main component, then by stacking order among equal-sized same-name groups (`pairedBy` says which), the rest stay unpaired with `reason: ambiguous-name`, and instances are not expanded. A node and its own ancestor cannot be compared. `--compare-variant` still requires the same component set.

A node argument accepts `12:345`, `12-345` or a Figma URL with `?node-id=`. `<file>` can also be `gs://bucket/object`, downloaded with your `gcloud` login and cached. See the [GCS snapshot cookbook](../wiki/gcs.md) for a fixed-object handoff, access requirements and refresh limits. Each CLI call decodes the file; MCP reuses the last unchanged local file, while remote calls parse again. Run `figlens --help` or `figlens inspect --help` (also `-h`) for full CLI usage. `index add gs://bucket/object` registers a pinned copy of that generation and reports `remote {uri, generation}`; only add runs gcloud.

`inspect` includes recorded text resizing, truncation, max lines and decoration. Dev Mode annotations appear on their nodes as `annotations` (plain `label` preview, pinned `properties`, `categoryId`; `nodes` adds `labelMarkdown`) with the named `annotationCategories`; Ready for dev frames and sections carry `devStatus`, and `--flat` marks both; measurements are reported in `handoffDiagnostics`. A parent without auto-layout carries `childSpacing: "derived"`: gaps between its children are coordinate differences, not an auto-layout rule; `constraints` may still pin them. `handoffDiagnostics` reports known stored semantics not emitted, with counts, examples and [coverage limits](../wiki/semantics.md#known-handoff-gaps). `_figlens.semantics` lists only fields whose source is stored (a missing entry means it is not), and each response states once, in top-level `_figlens.semantics.verification`, that these mappings are not compared with Figma. Grid auto layout frames carry row/column counts, gaps and stored track sizes (`_figlens.gridTracks`). Outputs protect the source and replace the destination only after a completed write. MCP limits request lines to 1 MiB and complete replies to 8 MiB; over-budget replies return an explicit error. Full CLI JSON remains available.
