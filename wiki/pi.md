# Pi extension

Load this for the Pi workflow, mapping persistence, review behavior or npm packaging. The [user walkthrough](../pi/README.md) owns commands, examples and limitations.

## Implementation

- `pi/requests.mjs` builds the shared open/preview/plan/YOLO/map requests from adjacent skill references.
- `pi/index.ts` is the Pi 1.3+ adapter: commands, tool schemas, MCP registration, reference images and dialogs. Pi supplies `typebox`, its SDK and TUI; do not bundle copies. Module scope registers callbacks only. The MCP server is registered at session start and managed by Pi, including shutdown. Index, `changes` and `watch` reach Pi as that server's own tools, so following changes needs no Pi tool; HELP and stale hooks use it, and `mcp_smoke` runs it through Pi's MCP command.
- `pi/workflow.mjs` reads CLI `find`/`nodes`, tracks signatures/hashes and writes `.figlens/component-mappings.md`. Optional `authorize(action, paths)` guards reads and writes; Pi omits it. Denial/cancellation stays an error, not stale evidence. `pi/index.ts` exports images; Rust decoding is unchanged.
- `figlens_bindings` is read-only. `file` and `frame` overrides are paired and do not replace the saved selection. `figlens_propose_bindings` only writes pending candidates. Approval is reachable through the user command, not a model tool; this is workflow separation, not filesystem isolation from Pi's other tools.
- Matching stays in one canonical source path. The signature covers main/set/key identity, expanded node types and typed properties; TEXT values are dynamic. Hidden instances are omitted. A changed whole-file hash reloads the frame. Names, local IDs and paths do not prove export lineage; this is not a public mapping format or Code Connect resolver.
- A TEXT property missing verbatim from visible descendant text is a review signal, not a diagnosis. Preserve both values. Listed conflicts require a resolution; its text hash and instance ID prevent silent reuse on another conflicted instance.
- Target and supporting evidence files resolve within the project. Reuse checks their hashes, not import validity or all transitive dependencies. Editing a target clears previously reported checks. Project compilation, interactions and visual comparison remain separate evidence.
- Writes use a per-project directory lock and atomic rename. Model tool writes also join Pi's shared file mutation queue. The complete read/modify/write is under the lock, including approval's evidence recheck. Re-read before replacement to detect manual edits. Malformed state and symbolic guide/directory links fail without overwrite. A process crash can leave a lock; never automatically delete another writer's lock.
- Bindings paginate at 20 instances by default (1–50), with a 24 KB JSON item budget; oversized single instances return `needs-inspection`. No frame payload is injected automatically; agents pass the current request’s file and frame explicitly. Human status lists at most eight with approved mappings and review signals first; full details remain in the tools. Model-facing strings are project/design data, not authority.

The legacy `frame` picker searches names with CLI `find --type FRAME` (up to 200 results), so nested frames are reachable. TUI uses Pi’s Input/SelectList with a terminal-height bound and wheel/keyboard navigation; RPC uses six-item pages. Review lists all pending proposals across source scopes without changing the saved selection. Human help and errors are Markdown. The preview message has its own image renderer and a persistent local PNG link under `.figlens/previews/`; no temporary link is presented as a lasting artifact.

## Conversational scope discovery

`/figlens open [description]` asks the current agent to resolve a design from the conversation, a local file, a screen name, its purpose or visible text. Missing information is requested in recognizable terms, not node IDs. It does not read or overwrite the manual selection, propose mappings or authorize application edits. Explicit `open <file> --frame ID` and the legacy `frame` picker retain manual-selection compatibility, listed only as advanced controls. The picker hands an unmatched description to the agent rather than requiring an exact layer name. `/figlens map` also resolves the conversational scope; a saved selection is used only when explicitly requested, avoiding an old frame overriding a newly discussed design. Dispatch is distinct from a completed model response, as with planning below.

## Interactive viewer shortcut

`/figlens preview <existing-file.fig>` starts the viewer immediately without a model. A description or no arguments dispatches a viewer-only request to resolve the current local file and optional scope, without relying on a saved selection. The `figlens_preview` Pi tool starts the CLI viewer through `pi/viewer.mjs`, passing argv directly through the existing package launcher or configured binary. It returns the actual loopback URL without opening a browser. This is separate from the CLI-only preview command and the inline PNG rendered for advanced manual selection.

The CLI prints its URL before decoding; startup succeeds only after the catalog returns valid JSON and any requested node resolves through its revision-scoped data route. A catalog fetch that fails at the network level (fetch's `TypeError`; the Windows cause behind #113 is unknown) is retried every 250 ms until the 120 s startup timeout; an exit, HTTP error or invalid JSON ends startup at once, and errors, including the timeout, keep the last fetch cause and stderr. Startup failure, an unavailable node or cancellation stops the candidate process tree and preserves a previous viewer; successful startup replaces it. `/figlens close` and `session_shutdown` stop owned viewers, including a native child of the npm launcher. POSIX uses an owned process group; Windows uses `taskkill /T /F`. Render cache may be written; application files, manual selection and mappings are unchanged by the tool. A one-shot process cannot retain a viewer after session shutdown. See the preview topic for service behavior and renderer limits.

## Design implementation planning

`/figlens plan [request]` waits for idle, reads the adjacent packaged [four-field planning workflow](../skills/figlens/references/planning.md) and sends a plan-only request to the current agent. It includes the workflow source path so relative references resolve after packaging; page/dependency detail stays in the adjacent planning-details.md and loads only when needed. It points to the adjacent skill for tool/image guidance. It works without open, leaves the manual selection/mappings unchanged and uses the current request/conversation rather than injecting a saved frame. No arguments means resolve the conversation's task or ask for missing source/scope. It adds no global `/plan`, model, tool permission toggle or session state.

`sendUserMessage` dispatches without awaiting model completion. RPC consumers must wait for the model result; one-shot print consumers should use an ordinary planning prompt through the skill. Loader/dispatch tests do not establish a completed headless model run.

The portable skill routes explicit plans and careful reconstruction to that reference. Scope controls discovery and detail; a local change does not need a whole-site inventory. Use full `nodes` for styles, masks and crops, and follow component IDs onto non-delivery canvases. Dev Mode annotations are in `inspect`/`nodes`; hidden text notes are separate. Resolve actual ancestry and code usage. Record relevant reuse, image delivery, behavior assumptions and acceptance. Saved plans follow project conventions or `.figlens/plans/<scope>.md`, exports `.figlens/exports/<scope>/` (kept); multiple designs use `<design>/<scope>`. The agent chooses these paths and rechecks sources on resume; the command allocates none. Plan writing and exports are filesystem writes, not a sandbox.

`/figlens yolo [request]` separately waits for idle and loads adjacent [yolo.md](../skills/figlens/references/yolo.md) plus the skill path into the current agent. It authorizes scope-limited implementation after planning, verification/correction, then cleanup of only the run-created plan, scratch exports and processes after verified success. Other plans/notes and `.figlens` files remain; failed, interrupted or blocked verification keeps the plan and exports. It uses the exact request or conversation, never injects a saved selection, and reports missing indispensable prerequisites. This is model guidance, not deterministic cleanup or a permission bypass; `/figlens plan` remains plan-only.

## Packaging and verification

Cross-host package boundaries, shared staging and installed-adapter rules live in [integrations](integrations.md), and their installed lanes in [host acceptance](host-acceptance.md). Pi retains its own SDK, TUI and runtime MCP registration; the private root supports source discovery. Edit maintained sources, never generated `npm/` copies.

In the published package the extension invokes its adjacent `bin/figlens.js` through the current Node executable. This keeps the native binary version aligned and needs no PATH/global install. Source checkouts use `figlens` on PATH, or `FIGLENS_BIN`. MCP and slash commands use the same launcher. The existing publish workflow and npm Trusted Publisher release the CLI and extension together; no new npm package or credentials are required.

Concurrent first-use downloads publish the verified temporary directory atomically. If another process wins, use its installed binary and discard the losing temporary directory; never remove a cache that a running MCP process may be using. The npm regression simulates Windows' in-use binary restriction with the real cache filesystem.

```sh
npm test
node --check pi/index.ts
node --test scripts/preview.test.cjs
(cd npm && node --test)
node scripts/prepare-npm.cjs
(cd npm && npm pack --dry-run --ignore-scripts)
python3 scripts/ai-context.py check
```

The mapping tests exercise pending/approved/stale states, Vue slot examples, cross-frame and cross-session reuse, variant/structure changes, changed/missing supporting code, cross-file ID collisions, text conflict review, approval-time rechecks, target edits, guide-note preservation, corruption, locks, symlinks and bounded pagination. They are workflow tests, not agent quality or render accuracy measurements.

Run `node scripts/check-pi-preview.mjs <pi-package-root> <binary> <sample.fig> <new-output-directory> [extension-entry]` for actual native viewer readiness, failed-start preservation, close and session-shutdown cleanup. `pi/viewer.test.mjs` also checks a nested npm-style launcher and cancellation, and runs in both Linux and Windows CI. Local macOS results do not establish a Windows pass; check that job's result.

Run `python3 scripts/check-pi-plan-model.py <previous-case> <new-output-directory>` to repeat a local planning case with a clean project copy and its isolated provider profile. It records source/fixture hashes and the selected executable path/hash (`FIGLENS_BIN` overrides the default fixture binary), and waits for an actual `agent_start` followed by `agent_settled` and final text, without a model time limit; it never treats RPC dispatch success as completion. Use only a trusted case/profile and inspect the resulting plan against current design evidence.

Run `node scripts/check-pi-planning.mjs <pi-package-root> <new-output-directory> [extension-entry]` against the minimum and current supported SDKs, including an unpacked npm entry. It checks plan/YOLO dispatch in TUI/RPC/print, exact request text, no saved-scope injection, idle ordering, plan-only separation, packaged reference loading and skill discovery. It does not call a model. The archive test uses a stub compiler, not Rust. Evaluate model behavior separately with fixed source/project/task inputs; dispatch tests do not prove implementation, verification or safe plan cleanup.

For provider-backed YOLO acceptance, use an approved source/model and isolated fixture. Observe actual agent start, settlement and final text. Check successful implementation and browser verification before own-plan deletion, then a missing-prerequisite case that retains its plan. Seed an unrelated human plan and compare its bytes/hashes afterward. Record exact prompts, loaded reference hashes, model, corrections and unavailable checks; tightly prompted cases do not prove general reliability.

The npm metadata includes `pi-package` plus `pi.image` and `pi.video` preview URLs. This makes a published version eligible for discovery in the [Pi package catalog](https://pi.dev/packages), as described by [Pi's package documentation](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/packages.md#create-a-package). Check the catalog after publishing; eligibility does not guarantee immediate indexing or placement.

For installed-package acceptance, run `python3 -B scripts/check-installation.py --artifacts <candidate-directory> --commit <checkout-sha> --pi-version 1.3.0` (or 1.4.0). [Installation acceptance](installation.md) covers tarball/cache/MCP checks and evidence layers. It uses a fresh consumer, real SDK loader, request dispatch and native viewer sockets; no private designs, user profile or model key.

`scripts/check-pi-integration.mjs <installed-pi-root> <installed-figlens-root> <public.fig> <isolated-consumer>` is that harness's loader sub-check: runtime MCP registration, commands, public-frame reads through the adjacent wrapper and skill discovery. Scripted message sinks capture help/plan text; this is not a full interactive session. The older Aurora/Skillshare case below records historical acceptance.

For the MCP Apps usability regression, run `node scripts/check-pi-usability.mjs <pi-package-root> <figlens-binary> samples/mcp-apps-for-claude.fig samples/pi-usability-new [extension-entry]`. It exercises readable help/errors, no-open proposals/review, explicit scope despite an older Button selection, the real host image renderer and persistent PNG, terminal resizing/wheel navigation, nested name search and host skill discovery. Pass an installed tarball's `pi/index.ts` to check its adjacent launcher and skill. These are scripted SDK checks, not a measured natural-language success rate or GUI acceptance. Provider-backed model tests need authorization to transmit the sample and use the configured account.

## Demo and verification

The v0.3.0 preview/Pi recording uses **`samples/mcp-apps-for-claude.fig`**, with the two **List views** frames `467:24651` and `467:25127`. Credit: [MCP Apps for Claude (Community)](https://www.figma.com/community/file/1597641111449594397/mcp-apps-for-claude). Designs and raw recording artifacts stay under ignored `samples/`; documentation screenshots/video are the published demonstration.

The candidate maps **App header**, instance `467:24657`, to the existing `.app-header` markup, styles and `#app-code` handler in the HTML implementation used by the original README demo. It records `index.html` plus the two original SVG assets as code evidence. The second frame's instance `467:25133` uses the same observed component and is reused after review and a new Pi session. This demonstrates plain HTML reuse, not React generation or Figma Code Connect.

The preview portion is captured from the actual local viewer. The terminal portion is actual Pi 1.3.0 output captured by `scripts/record-terminal.py` as asciicast v2, replayed through a local terminal viewer and recorded with agent-browser. Pauses are shortened and scene changes are edited; the video is not a real-time speed benchmark. The candidate is prepared before recording, so agent discovery/generation is neither shown nor timed. It demonstrates selection, confirmation, persistence and reuse. The adjacent design image is a reference render, not an automatically synchronized browser selection or newly generated application.

The earlier Aurora regression uses Thursday `10:200` and Hero `22:103`, with copied Skillshare Button/Card/SegmentedControl sources. It also checks the unreviewed Open variant and the `Not today` / `Hold to confirm` text discrepancy. Neither example establishes agent time savings or whole-screen visual equivalence.
