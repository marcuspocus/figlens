# Plan, implement, verify and clean up

Use only when the user explicitly authorizes autonomous design implementation and cleanup of what the run created, including Pi's `/figlens yolo`. Keep the work in the same agent context. This workflow adds no planner agent, permission bypass or task scheduler.

## Read only what this run needs

From the [main skill](../SKILL.md), read these sections and no others up front: [Where outputs go](../SKILL.md#where-outputs-go), [Building a frame](../SKILL.md#building-a-frame), [Verification evidence](../SKILL.md#verification-evidence), [Reading the output](../SKILL.md#reading-the-output), [Font and asset preflight](../SKILL.md#font-and-asset-preflight), [Component mappings](../SKILL.md#component-mappings), [Many frames](../SKILL.md#many-frames) and [Known gaps](../SKILL.md#known-gaps). `workflow yolo` serves them after this guide; with the skill file, read them there. Read [the planning workflow](planning.md) for the plan. Load the rest only when the run meets it:

- Raster images or exported icons ship with the scope: [Images and slices](../SKILL.md#images-and-slices).
- Several pages or an unresolved design dependency: [planning details](planning-details.md).
- The user asks to search an index or follow changes: [Search and local index](../SKILL.md#search-and-local-index).

## Plan before editing the application

Resolve the current request's `.fig`, recognizable screen/region and target project. Read its actual architecture, neighboring implementations, applicable instructions and image pipeline. Use [the planning workflow](planning.md) to inspect only the authorized design scope and dependencies and make the next slice actionable.

Create a small working plan before application edits. Use the project's plan conventions and the [saved-plan namespaces](planning-details.md#save-and-resume-proportionately). Choose a new, unoccupied path for this run and record exactly which plan files you create; keep an existing plan as read-only context. Likewise choose a new, unoccupied export directory for this run, named like the plan's (see the namespaces above) unless the user names one, and write every bundle, extracted file, reference image, screenshot, scratch script and log for your own checking there. Record it in the plan. An existing plan's exports can be read as input, not cleaned up. The working plan identifies its source/scope, implementation steps and observable acceptance checks. Do not create a whole-site inventory for a local change.

Resolve routine implementation choices from project evidence and state material assumptions in the plan. Classify each known gap before editing: **blocking** when no defensible implementation exists without it (source, scope, a required decision); **accepted fallback** when a documented substitute stands in, such as an available font or a labelled image placeholder at the intended size, recorded with its remedy; **non-blocking warning** when it does not change the delivered result, such as unemitted layout guides. Only a blocking gap stops the run. Optional metadata, such as an online Figma file key, can remain unknown when the verified local file and frame establish the scope. Continue directly into the authorized implementation without a plan-approval question. If the source, scope or another indispensable prerequisite cannot be established, report the blocker and preserve the plan rather than guessing another design or widening the task. Host permissions still apply.

## Build, check and correct

Implement coherent slices using the project's components, tokens, layout and asset conventions. Follow the main skill's image delivery and reference-comparison procedure. Update the working plan with observed progress rather than repeating broad discovery.

After the first pass, compare the running screen at the requested viewport with the design evidence by running `compare-image` on its screenshot ([Verification evidence](../SKILL.md#verification-evidence)), verify typography, layout, crops and image delivery, and run the project's relevant build/runtime/interaction checks. Correct confirmed discrepancies and rerun the affected checks. When a check leads you to depart from the design, such as darkening text for contrast, record each departure in the plan with the design value, the delivered value and the reason; it is a known difference, not a match. Stop when acceptance is met; if progress needs unavailable evidence, permissions or a decision that cannot be inferred, report the unresolved condition instead of retrying without a new hypothesis.

## Clean up after verified completion

Of the plans, delete only the temporary plan files created by this run, after implementation and the applicable acceptance checks succeed. Record their contents after your own updates; immediately before deletion, re-read each exact path and verify it still belongs to this run and matches that last known content. Preserve any plan with unexpected edits, user-added notes or uncertain ownership. Remove an empty plan directory only if this run created it. Avoid recursive cleanup of `.figlens`, shared namespaces or wildcard-matched plans.

In the run's own export directory, write `verification.md` with the acceptance record described below, keep it, the final acceptance screenshot and its `compare-image` diff PNG, and delete the rest: bundle ZIPs and their extracted files, intermediate screenshots, scratch scripts and logs can all be regenerated. Delete nothing outside that directory: `.figlens/component-mappings.md`, `.figlens/previews/`, other exports and shipped assets stay. If the directory was not created by this run, delete only the files this run wrote there.

Whatever the outcome, before you finish stop the dev servers, browser sessions and other processes this run started, and only those; leave any that were running before. Give the command that restarts the preview, and report a process you could not stop with its PID.

Keep delivered code/assets, existing plans and source designs. Keep figlens workspaces too: their databases, registry and any index are durable state, not task-owned temporaries; never run `figlens index reset` or `untrack` as cleanup (see [Search and local index](../SKILL.md#search-and-local-index)). Start no watch lease, subscription or tracking unless the user asked for one; the only watch state a run removes is a subscription it created with an `owner`, through `watch unsubscribe` with that consumer and owner, never `forget`, another owner's or an ownerless one. Failure, interruption, a blocking gap or unavailable required verification leaves the working plan and the run's export directory available for continuation. Visual acceptance left pending only by an accepted fallback does not: report it and clean up. If the user asks to retain them, follow that instruction.

Return the implemented scope, then the acceptance record in three groups:

- **Proven by a tool:** results with their command or tool output, such as `figlens verify`, tests, builds, axe, a measured font load, or `compare-image` with its viewport, state, `mismatch` and `nonTextMismatch` once every remaining region, text regions included, is fixed or listed as a known difference with its cause.
- **Checked by you:** what you compared or exercised yourself, such as interactions, states or viewports `compare-image` did not cover, layers it ran with in `--ignore` (their size and position checked with `inspect`), and differences under its threshold, with the viewport and state.
- **Not verified:** checks you could not run and unresolved inputs.

List known differences separately: each recorded design departure, accepted fallback with its remedy, and remaining renderer difference. Then give the actual cleanup outcome: plan, export files kept and removed, and processes stopped. Cleanup failure is a reported remaining task, not permission to claim it was done. A successful build alone does not establish visual acceptance.
