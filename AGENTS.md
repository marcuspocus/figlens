# AGENTS.md — figlens

Reads Figma `.fig` files offline: no API rate limits, login or running Figma. Rust crate (`src/`) and native `figlens` CLI. Load task details by topic.

## Language

- Code, identifiers, comments, commits, instruction files (this file, `wiki/`, `skills/`) and `README.md`: English.

## Always Applies

- **Rust runs only in Docker** for development: `docker compose run --rm dev ...`; do not install or use a host Rust toolchain. Users run the native `figlens` binary with no Docker or Rust installed.
- **Format changes need evidence.** Before changing how nodes are decoded, expanded or emitted, load `fig-format`; claim accuracy only after measuring with the `accuracy` topic, on a file the change was not written against.
- **Keep the docs true.** When a doc disagrees with the code, trust the code: verify, then fix the stale doc in the same task. When your change alters behaviour that a topic describes, update that topic page in the same change. Put a new lesson in its topic as a one-line rule beside the procedure; the kernel takes only rules every task needs. Run `python3 scripts/ai-context.py check` after any doc change (see `ai-context`).
- **Search before you edit; change as little as possible.** Match the existing style.
- **Check CLI, MCP, Pi, OpenCode and OMP together.** For every feature or optimization, assess each interface, complete affected behavior/contracts/guidance/tests, and explain why any interface needs no change. A new, changed or removed feature also updates `--help`, every README locale, the site, a served skill section and its wiki topic; see `review`.
- **Helper files go in the repo.** Helper scripts go in `scripts/`; private designs, Figma exports and run outputs go in the gitignored `samples/`. The original public fixture below and the reviewed regression baselines in `scripts/corpus/` are the exceptions.
- **Review before every PR.** Run the `figlens-code-review` passes on the pinned diff and fix verified findings before opening or updating a PR, docs- and wiki-only changes included; see `review`.
- **Open PRs as drafts** with Conventional titles (Release Please's excepted). Drafts skip CI; request `@codex review`, fix findings, then mark ready for CI; see `review`.
- **Open issues by the rules.** English, `<scope>: <summary>` titles, one type label plus `area:` labels, no private design details; see `review`.
- **Own PR follow-up.** Check AI reviews and CI, fix verified findings, then request and check `@codex review` on the updated head; see `review`.

## Hard Limits

- Never commit private design files or Figma exports; `samples/` stays gitignored. Only the original synthetic `examples/first-success/design.fig` is distributable, with its [provenance and update procedure](examples/first-success/README.md).
- Shipped binaries build only from `Dockerfile.dist`'s digest-pinned base image; never point the release at a mutable tag.
- The release tag, `Cargo.toml`, `Cargo.lock`, `npm/package.json`, the host plugin manifests and the Herdr mod's engine `VERSION` carry one version.

## Load Context Per Task

```sh
python3 scripts/ai-context.py list      # topics and when to pick them
python3 scripts/ai-context.py <topic>   # print only that topic's sections
python3 scripts/ai-context.py check     # validate paths, headings, links, orphans and byte budgets
```

| Topic | Use when |
|---|---|
| `mcp` | Changing MCP instructions, tool discovery or workflow guidance |
| `gcs` | Changing the `gs://` snapshot handoff, its download cache or refresh |
| `freshness` | Changing metadata comparison, the sources mapping or stale hints |
| `pull` | Changing browser pull, profiles, helper or login/logout |
| `accounts` | Changing browser/API alias routing, accounts.json or environment references |
| `performance` | Measuring load time and memory, the perf baseline, cache or decode cost |
| `parsing` | Changing untrusted container/schema/value decoding or parsing budgets |
| `fig-format` | Changing decoding, instance expansion, emitted REST fields or variables |
| `tokens` | Changing DTCG variable export, validation or interoperability |
| `discovery` | Changing find/inspect summaries or `--flat`, `tree --top-frames` or `export --match-dir` |
| `search` | Changing search indexing, ranking, cursors, coverage or budgets |
| `visual` | Changing visual units, query-image admission, fingerprints or vision profiles |
| `semantic` | Semantic/hybrid search, `models`, model runtime |
| `diff` | Changing diff |
| `changes` | Tracking, `changes`, `watch ack` or stale notes |
| `subscriptions` | Changing subscriptions or checkpoints |
| `watch` | Changing leases or the runner |
| `workspace` | Changing workspace roots, identity, ownership, registry, locks or index actions |
| `index` | Changing index generations, jobs, publication or workspace search |
| `bundle` | Changing the bundle zip, manifest, context.md, batches, fonts, render warnings or `verify` |
| `render` | Changing SVG or raster output: text, glyphs, image fills/adjustments, effects, export areas |
| `preview` | Changing the local viewer, offline HTML, handoff or screenshot comparison |
| `pi` | Changing the Pi extension, component mappings or npm packaging |
| `integrations` | Changing OpenCode V2/OMP, packaging or permissions |
| `herdr` | Changing the figlens mod or layout for Herdr |
| `plugins` | Per-host plugin manifests |
| `host-acceptance` | Installed-host lanes |
| `accuracy` | Measuring output against Figma's exports, `compare`/`compare-image`, test files, CI corpus |
| `demos` | README demo recordings |
| `release` | Packaging, releases, Homebrew, npm, installers, `figlens update` |
| `review` | Delivering features/optimizations across all interfaces; reviews, PR follow-up, issues |
| `direction` | Deciding what to build next, or revisiting a past decision |
| `ai-context` | Adding, moving or splitting docs and topics; `check` failures |

Pick the closest topic; load a second only when a task crosses two seams.

When no topic fits, read `wiki/README.md`; to add a topic, load `ai-context`.

Over budget: split the topic, never raise the cap. Skills are workflow adapters over topics, not the source of truth.

## Verification

Area rules (parsing budgets, output writes, fixtures, rendering, hosts, release) live in their topics: load the topic before editing its area.

```sh
docker compose run --rm dev cargo test
docker compose run --rm dev sh -c 'cargo fmt --check && cargo clippy --all-targets -- -D warnings'
(cd npm && node --test)
npm test  # host, viewer and npm package tests
python3 scripts/ai-context.py check  # doc/topic changes; runs in CI
docker compose run --rm dev python3 -B scripts/corpus-check.py ...  # output changes; see `accuracy`
```
