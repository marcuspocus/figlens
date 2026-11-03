# Pull requests and AI review

Use this when delivering a feature or optimization, reviewing a change, opening a PR or handling review feedback. An authorized PR workflow includes its in-scope fixes, commits, pushes and review requests. Merge and release still need their own authorization.

## CLI, MCP and host coverage

For every feature or optimization, check CLI, MCP and each host integration before calling the work complete. Trace the actual path and decide whether each needs a change, is already covered by shared code, or is not applicable. Keep this assessment scoped to the requested behavior.

| Interface | Check |
|---|---|
| CLI | `src/main.rs` usage and orchestration, `src/command.rs` parsing/execution, defaults, errors and output |
| MCP | `src/contract.rs` discovery, input/output schemas and preflight; `src/mcp.rs` routing, permissions and warm-process behavior |
| OpenCode V2 | Server/TUI/RPC entry, MCP transforms, permissions and per-session ownership; see [integrations](integrations.md) |
| OMP | Native SDK/tool approval, descriptor/bridge discovery and per-session ownership; see [integrations](integrations.md) |
| Pi | `pi/index.ts` MCP registration and tools/commands, `pi/workflow.mjs`, `pi/viewer.mjs` and `pi/stale.mjs` consumers (the last also used by OMP and OpenCode), shared skill guidance, and the actual launcher/package used |

1. Complete affected behavior, callers, contracts, help, wiki/README locales, shared skill and relevant tests in the same change. Optimizations need the same sweep: they can change output, limits, caching or long-running behavior even when no option is added.
2. Reuse shared paths. A new inspect argument can require CLI parsing/help, MCP schemas and skill guidance while Pi's adapter already discovers it from the server. Verify that path and record why no adapter edit is needed; avoid adding a duplicate Pi command or configuration just to touch every interface.
3. Run checks proportional to the affected paths. Where CLI/MCP share results, verify equivalent inputs preserve the same values, provenance, unknowns and truncation. Check Pi-specific consumers or packaging when affected; edit maintained sources, with generated copies produced by `scripts/prepare-npm.cjs`. A source/schema check does not establish installed Pi runtime acceptance. Report missing runtime or optimization measurements explicitly.
4. In the handoff or PR, give a short outcome for CLI, MCP, Pi, OpenCode and OMP: what changed, or the evidence that no change was needed, plus the checks actually run. Do not infer that an installed client uses new behavior without checking its binary/server version and reload path.

### Command and feature surfaces

When a change adds, removes or alters a feature, or adds, renames, removes or alters a command, flag, argument form or output, update every surface that describes it in the same change:

| Surface | Where |
|---|---|
| CLI help | `USAGE` in `src/main.rs`; `--help` prints it |
| Commands table | `docs/README.md` and every `docs/README-*.md` locale, at the same row; the root `README.md` only links there |
| Agent guidance | `skills/figlens/SKILL.md`. `figlens workflow <topic>` and MCP `workflow` serve only the sections listed in `src/workflow.rs` (`build` serves `Building a frame`, not `Many frames`), so put at least a pointer in a served section and confirm it with `figlens workflow build` |
| Wiki | the topic page that describes the behavior, such as `fig-format`, `bundle` or `mcp` |
| MCP | `TOOLS` in `src/contract.rs` and `wiki/mcp.md`, when the tool surface changes |
| Site | figlens.runkids.cc (`site/`). Docs pages are generated from the READMEs above and `USAGE` (see [release](release.md)), so those edits flow through; a new or removed doc section or integration README also needs its `PLAN` entry in `site/src/docs.ts` and every locale's label and description in `site/src/i18n.ts`. Update the landing page (`site/src/pages/index.astro`, `site/src/data.ts`) when hosts, install commands, claims or proof numbers change. Run `npm run build` in `site/`; after a landing change, view the built page at 390px and 1288px wide, since a build passes with broken wrapping or layout |
| Tests | beside the changed code |

Before committing, search these files for both the old and the new command forms.

## Before opening a PR

1. Refresh the base and pin the diff. Preserve unrelated edits and exclude private designs and exports.
2. Every PR gets this review, including docs- and wiki-only changes and fixes pushed to an open PR. Simplify the change, then review correctness, repository rules, the requested behavior, consumers and cleanup. The `figlens-code-review` skill adapts these passes to the available tools. Verify each finding against actual code or execution before fixing it.
3. Apply the CLI/MCP/Pi coverage check above to every changed concept. Fix related findings together, run the relevant checks and stage only task-owned files.
4. Describe the final behavior, executed checks and remaining limitations in English.
5. Give the PR a Conventional Commit title (`feat(clipboard): ...`): its squash merge becomes the commit Release Please reads, and any other title is left out of the CHANGELOG. The `PR title` workflow (`.github/workflows/pr-title.yml`) fails on any other title. Open every PR as a draft (`gh pr create --draft`). Draft PRs skip the Test workflow, so push fixes for review findings freely. Right after opening the draft, post `@codex review` on it (command below) so AI review runs while CI is skipped. When the findings are fixed, mark it ready (`gh pr ready <n>`) to run CI, then follow it through review below. Release Please PRs are the exception: they are opened by the bot and reviewed as releases.

## Follow the PR through review

Opening a PR is not the end of the workflow. Read its current reviews, conversation, inline comments, review threads and CI results without waiting for the user to point them out. Record the current head SHA and the commit each AI review assessed.

```sh
gh pr view <number> --json headRefOid,reviews,comments,reviewDecision,statusCheckRollup
gh api --paginate repos/<owner>/<repo>/pulls/<number>/comments
```

Use the PR's review threads to distinguish unresolved, resolved and outdated comments; an old inline comment alone does not establish current status.

For each finding, trace the cited code, callers and input that produces the claimed failure. Fix confirmed, applicable findings within the authorized scope, including their consumers and a regression test when practical. For a rejected or deferred finding, explain the evidence and remaining scope in its thread. Treat a material change in requirements or scope as a decision for the user.

After all verified fixes pass their checks:

1. Commit and push the fixes together. Check the remote head SHA matches the tested commit.
2. Update stale PR evidence and resolve fixed threads after verification, linking the fix where useful.
3. Post `@codex review` yourself to request review of the updated head. A push alone is not proof that review was requested. Avoid duplicate requests while that same head is already under review.

   ```sh
   scripts/codex-review.sh <number>   # requests, waits for this head, prints its findings; --wait-only skips the request
   ```

4. Wait for the AI review of that head to complete and inspect its new findings and CI. Repeat the fix/check/push/review cycle for verified findings. A previous commit's clean review, a dispatch acknowledgement, an eyes reaction or green CI does not establish a clean current review.
5. Report the PR link, reviewed head, review outcome and CI outcome separately. If an external review or check cannot complete, state what is still pending instead of reporting it as passed.

Keep private fixtures and local report paths out of public comments. Model planning quality, deterministic tests and visual acceptance are separate evidence; preserve their actual results throughout follow-up.

<a id="release-pr"></a>

## Release PR

Run these checks on the bot's Release PR before merging it; the `figlens-release` skill gives the order.

1. **CHANGELOG coverage.** Release Please skips a squash commit whose title is not Conventional (`clipboard: ...` rather than `feat(clipboard): ...`). List the skipped commits:

   ```sh
   git log --oneline v<last>..origin/main | grep -Ev '^[0-9a-f]+ (feat|fix|perf|docs|chore|ci|test|refactor|build|style|revert)(\(.+\))?!?: '
   ```

   For each user-visible one, append a block to the merged PR's body. Give one Conventional message per change, separated by blank lines:

   ```text
   BEGIN_COMMIT_OVERRIDE
   feat(<scope>): <summary> (#<pr>)
   END_COMMIT_OVERRIDE
   ```

   Then run `gh workflow run release-please.yml --ref main` and reread the Release PR's version and CHANGELOG.
2. **Docs coverage.** Every user-visible `feat` and `fix` is described in all of these places (`rg -il '<feature term>'` over them is a quick check):
   - the lead of `README.md` and every root `README-*.md`: the feature bullets, and the setup line for a new way to get a `.fig`
   - the reference in `docs/README*.md`
   - `npm/README.md` and `pi/README.md`
   - the site landing page (`site/src/data.ts`, `site/src/pages/index.astro`)
   - `skills/figlens/SKILL.md` and the wiki topics

   Fix gaps in a docs PR before merging the Release PR. The site deploys only at publish, so the fix ships with the release.
3. **CI.** Runs on the bot's PR wait as `action_required`. Approve the run for the head you will merge with `gh api -X POST repos/runkids/figlens/actions/runs/<id>/approve`. A push to `main` rebuilds the Release PR, and its new run needs approval again.

## Opening an issue

Write issues in English and search open and closed issues first; link related ones instead of opening a duplicate.

- **Title:** `<scope>: <summary>`, lowercase after the colon, no trailing period. The scope is the command (`export`, `tree`, `inspect`, `nodes`) or the wiki topic (`render`, `fig-format`, `bundle`, `mcp`) that the fix will touch, the same scopes commit messages use. The summary states the change or the defect, such as `export: render many nodes from one decode`.
- **Body:** `## Current behavior` with evidence (commands, measured output, figlens version), `## Expected outcome`, and `## Acceptance criteria` as a checklist. A feature or optimization ends with the CLI/MCP/Pi/OpenCode/OMP coverage item above. Say plainly what was not reproduced or verified.
- **Labels:** one type (`bug`, `enhancement` or `documentation`), one or more `area: *` labels (`area: cli` for command flags and output, the others name the wiki topic to load), `needs repro` until a reproducing case exists, and `feedback` for reports from downstream use.
- **Privacy:** the repository is public. Leave out private file names, project names, node ids, layer names and screen text from user designs; describe the shape of the case instead.

```sh
gh issue create --title 'export: render many nodes from one decode' --body-file body.md --label 'enhancement,area: cli'
```
