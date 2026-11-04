# figlens Wiki Router

The wiki holds procedures, reference and decisions, and no task needs all of it at once. Rules for every task are in the root `AGENTS.md`. The topic-to-section map is in `wiki/ai-context.json`. Load a topic with `python3 scripts/ai-context.py <topic>`.

## Task routing

| Topic | Use when | Main sources |
|---|---|---|
| `mcp` | Changing MCP instructions, tool discovery or built-in workflow guidance | `mcp.md` |
| `gcs` | Changing the `gs://` snapshot handoff, its download cache or refresh | `gcs.md` |
| `freshness` | Changing `freshness`, the `.figlens/sources.json` mapping or the stale hint on a missing node | `freshness.md` |
| `performance` | Measuring load time and memory, reading the perf baseline, changing cache or decode cost | `performance.md` |
| `parsing` | Changing untrusted container/schema/value decoding or parsing budgets | `parsing.md`, `clipboard.md` |
| `fig-format` | Changing decoding, instance expansion, emitted REST fields or variables | `fig-format.md` |
| `tokens` | Changing DTCG variable export, validation or interoperability | `tokens.md` |
| `discovery` | Changing find or inspect summaries (fields, limits, truncation, `--flat` lines), `tree --top-frames` screen lists or `export --match-dir` icon matching | `discovery.md` |
| `search` | Changing search indexing, normalization, ranking, cursors, coverage or its budgets | `search.md`, `usages.md`, `library-usages.md` |
| `visual` | Changing visual units, query-image admission, fingerprints or vision profiles | `visual-search.md` |
| `semantic` | Changing semantic or hybrid search, `models` or the model profile and runtime lookup | `semantic.md` |
| `diff` | Changing the diff engine (facets, matching, coverage, pair budgets, paging or its cursor) | `diff.md` |
| `changes` | Changing change tracking, its revisions, `changes` reads, `watch ack` checkpoints or host stale-context notes | `changes.md`, `stale-context.md` |
| `subscriptions` | Changing watch subscriptions, their filters, consumer checkpoint bounds or forget | `subscriptions.md` |
| `watch` | Changing watch leases, the shared watch runner, its detector or the changes monitor | `watch.md` |
| `workspace` | Changing local workspace roots, identity, ownership, the SQLite registry, locks or index actions | `workspace.md`, `directory-add.md` |
| `index` | Changing durable index generations, index jobs (refresh, rebuild, clear_cache, cancel), publication fences or workspace search | `index-jobs.md` |
| `bundle` | Changing the bundle zip, manifest, context.md, batch bundles, font inventory, render warnings or `verify` | `bundle.md`, `render-diagnostics.md` |
| `render` | Changing SVG or raster output: text and glyphs, image fills, effects, export areas, image adjustments | `render.md` |
| `preview` | Changing the local viewer, offline HTML, handoff or screenshot comparison | `preview.md` |
| `pi` | Changing the Pi extension, reviewed component mappings or its npm packaging | `pi.md` |
| `integrations` | Changing OpenCode V2/OMP, shared package or permissions | `integrations.md` |
| `herdr` | Changing the figlens mod or layout for Herdr | `herdr.md` |
| `plugins` | Changing per-host plugin manifests or adding a host | `host-plugins.md` |
| `host-acceptance` | Running or changing installed-host lanes (OpenCode, OMP, Pi) | `host-acceptance.md` |
| `accuracy` | Measuring output against Figma's own exports, running `compare` or `compare-image`, finding test files, the CI regression corpus | `accuracy.md` (all but the demos), `compare-image.md` |
| `demos` | Recording or updating the README demos and their notes | `accuracy.md` § Local design-to-HTML demo, § Taskflow React and Tailwind demo |
| `release` | Packaging, installation acceptance, GitHub releases, Homebrew and npm | `release.md`, `installation.md` |
| `review` | Delivering features/optimizations across CLI, MCP, Pi, OpenCode and OMP; reviews, PR follow-up and opening issues | `review.md` |
| `direction` | Deciding what to build next, or revisiting a past decision | `direction.md`, `handoff-research.md` |
| `ai-context` | Maintaining this router: add, move or split docs, fix `check` | `ai-context.md` |

`wiki/ai-context.json` is the only source of truth. This table is the human entry point. After changing topics, run `python3 scripts/ai-context.py check`.

## Field reference

- [Partial semantic fields](semantics.md): emitted mappings, unknowns, provenance and validation limits; read when working on these fields.
- [Related component variants](states.md): opt-in inspect inventory, source IDs, evidence gaps and bounds.
- [Prototype field evidence](prototype.md): exact event/action/destination sources, instance limits and unknowns.

## History

- [Reviewed component mappings in Pi](history/pi-workflow.md): implementation and verification scope for the preview/Pi release.
