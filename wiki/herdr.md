# Herdr panel

Load this when changing the figlens mod or layout for [Herdr](https://herdr.dev) under `herdr/`. The [user walkthrough](../herdr/README.md) owns install and use; the [mod recipe](../herdr/figlens/RECIPE.md) and [layout recipe](../herdr/figlens-layout/RECIPE.md) describe what a build must do, and [`herdr/AGENTS.md`](../herdr/AGENTS.md) holds the folder's rules.

## Shape

- `herdr/figlens/` is the mod as shared at herdr.dev/mods; `herdr/figlens-layout/` is Herdr's `main` layout plus a tab-bar toggle and the panel in the side `panel-body` stack, with an identical copy of the mod. Tests in `herdr/tests/` run in `npm test` and CI (`test.yaml`) and check the copies match.
- The mod is a client of the CLI: it calls `tree`, `export` (`--frames`, `--scale`, `--max-size`), `inspect`, `find`, `preview`, `cache` and the release download, never the MCP server. A change to those commands' output shape can break it; run `node --test herdr/tests/*.test.mjs` and `herdr-app check herdr/figlens/mods/figlens`.
- It runs figlens on the machine that holds the design (Herdr's `exec` with `on`), so a remote design never moves for reading. Only images (thumbnails as one tar.gz per batch, previews, snapshots) come back.
- It hands work to the agent focused on the left through `hc send`, after a confirm dialog that shows the exact message. Pi and OpenCode get their own `/figlens plan` and `/figlens yolo`; Claude Code the plugin's `workflow(topic=…)`; others a plain prompt naming the skill's `references/planning.md` or `references/yolo.md`. OMP gets the plain prompt because its extension does not dispatch plan or yolo. Keep these strings in step with those hosts' commands and the skill's reference paths (`frames.js` `intentText`, tested).

## Rules

- `VERSION` in `engine.js` (both copies) is bumped by Release Please through a `generic` extra-files entry that rewrites its marked line; a test checks it equals `Cargo.toml`'s. Between merging the Release PR and running Publish Release, `main` names a version whose assets are not published yet, so the panel's one-click download fails for anyone who installs from `main` in that window; a figlens already on PATH or in the npm cache still works. The engine downloads that release's archive and checks it against its `SHA256SUMS`, like the npm launcher, and shares its cache (`~/.cache/figlens/npm/<version>/`).
- Large transfers go in parts: a remote command's output stops at 8 MB, `hc file write` at 8 MB, `hc put` at 16 MB, and a mod's own file writes are capped. Snapshots and design copies move through `hc exec dd` in 4 or 6 MB parts written by the shell.
- A design copied to another machine for an agent goes to `~/.cache/figlens/herdr-designs/<key>/` there, once per saved version; the cache row's clear removes it with the panel's images and `figlens cache clear`.
- Preview pictures are real screenshots in English, of a public or synthetic design.
- Not on Windows: the engine, discovery and transfers use `sh`.
