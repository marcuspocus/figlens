# Plan, implement, verify and clean up

Use only when the user explicitly authorizes autonomous design implementation and cleanup of what the run created, including Pi's `/figlens yolo`. Keep the work in the same agent context. This workflow adds no planner agent, permission bypass or task scheduler.

## The job

You are the frontend engineer turning this design into the project's code. Deliver what a senior frontend engineer on this stack would ship and a reviewer would merge: components with props, content as data, semantic and accessible markup, layout written from the design's intent and the project's tokens; images, fonts and code delivered fast; all of it through the project's framework, build, routing and asset pipeline. figlens reads the design; its trees, coordinates and node JSON are evidence for your decisions, never the code, the data or the runtime. Matching the reference image is one check of that code, not the goal.

In an existing project, its conventions and established practice decide the language, file layout, styling and tooling; follow them. A new project first gets the current professional baseline: TypeScript with strict types, a linter and formatter, code organized by responsibility ([Building a frame](../SKILL.md#building-a-frame), step 5). The quality bar for every run: components with props, styling through the project's system and the design's tokens, text as text, the project's type check, lint and format passing. An instruction to keep code minimal cuts scope and features, never this bar.

## Read only what this run needs

From the [main skill](../SKILL.md), read these sections and no others up front: [Where outputs go](../SKILL.md#where-outputs-go), [Building a frame](../SKILL.md#building-a-frame), [Verification evidence](../SKILL.md#verification-evidence), [Reading the output](../SKILL.md#reading-the-output), [Font and asset preflight](../SKILL.md#font-and-asset-preflight), [Component mappings](../SKILL.md#component-mappings), [Many frames](../SKILL.md#many-frames) and [Known gaps](../SKILL.md#known-gaps). `workflow yolo` serves them after this guide; with the skill file, read them there. Read [the planning workflow](planning.md) for the plan. Load the rest only when the run meets it:

- Raster images or exported icons ship with the scope: [Images and slices](../SKILL.md#images-and-slices).
- Several pages or an unresolved design dependency: [planning details](planning-details.md).
- The user asks to search an index or follow changes: [Search and local index](../SKILL.md#search-and-local-index).

## Plan the frontend

Resolve the request's `.fig`, screens and target project, then read the project's architecture, components, tokens, neighboring pages and image pipeline. Following [the planning workflow](planning.md), decide the frontend structure before writing code: the component each component set in scope becomes and its props, the data each repeated part renders from, the shell screens share, and each section's element and layout intent ([Building a frame](../SKILL.md#building-a-frame), step 5). Order the work as project setup (for a new project, the baseline in [Building a frame](../SKILL.md#building-a-frame), step 5), tokens, components, shell, then screens, one at a time and each composed from those parts.

Keep the plan small and record it before application edits. Use the project's plan conventions and the [saved-plan namespaces](planning-details.md#save-and-resume-proportionately); choose a new, unoccupied plan path and export directory for this run (named like the plan's unless the user names one), record exactly which plan files you create and that directory in the plan, and write every bundle, extracted file, reference image, screenshot, scratch script and log for your own checking in that directory. Keep an existing plan and its exports as read-only input. The working plan identifies its source/scope, implementation steps and observable acceptance checks. Do not create a whole-site inventory for a local change.

Resolve routine choices from project evidence and state material assumptions in the plan. Classify each known gap before editing: **blocking** when no defensible implementation exists without it (source, scope, a required decision); **accepted fallback** when a documented substitute stands in, such as an available font or a labelled image placeholder at the intended size, recorded with its remedy; **non-blocking warning** when it does not change the delivered result, such as unemitted layout guides. Only a blocking gap stops the run. Optional metadata, such as an online Figma file key, can remain unknown when the verified local file and frame establish the scope. Continue directly into the authorized implementation without a plan-approval question. If the source, scope or another indispensable prerequisite cannot be established, report the blocker and preserve the plan rather than guessing another design or widening the task. Host permissions still apply.

## Build, review and correct

Build in the planned order. Each component comes from its component definition, checked on its own before screens use it. Each screen is composed from those components, with its repeated parts rendered from data. Follow the main skill's image delivery procedure. Update the plan with observed progress rather than repeating broad discovery.

After each screen, review its code as [Building a frame](../SKILL.md#building-a-frame) step 7 describes and record the map (design component → code file). Fix structural problems first: a pixel match does not make code that skips these decisions an implementation. Then run `compare-image` on a screenshot at the requested viewport ([Verification evidence](../SKILL.md#verification-evidence)), check typography, layout, crops and image delivery, and run the project's relevant build/runtime/interaction checks. Correct confirmed discrepancies and rerun the affected checks. When a check leads you to depart from the design, such as darkening text for contrast, record the design value, the delivered value and the reason; it is a known difference, not a match.

A scope with more screens than this context can build that way ends at the last finished screen, with the remaining screens listed as the next run's scope; finish fewer screens properly rather than generating them in bulk from design data. Stop when acceptance is met; if progress needs unavailable evidence, permissions or a decision that cannot be inferred, report the unresolved condition instead of retrying without a new hypothesis.

## Clean up after verified completion

Of the plans, delete only the temporary plan files created by this run, after implementation and the applicable acceptance checks succeed. Record their contents after your own updates; immediately before deletion, re-read each exact path and verify it still belongs to this run and matches that last known content. Preserve any plan with unexpected edits, user-added notes or uncertain ownership. Remove an empty plan directory only if this run created it. Avoid recursive cleanup of `.figlens`, shared namespaces or wildcard-matched plans.

In the run's own export directory, write `verification.md` with the acceptance record described below, keep it, the final acceptance screenshot and its `compare-image` diff PNG, and delete the rest: bundle ZIPs and their extracted files, intermediate screenshots, scratch scripts and logs can all be regenerated. Delete nothing outside that directory: `.figlens/component-mappings.md`, `.figlens/previews/`, other exports and shipped assets stay. If the directory was not created by this run, delete only the files this run wrote there.

Whatever the outcome, before you finish stop the dev servers, browser sessions and other processes this run started, and only those; leave any that were running before. Give the command that restarts the preview, and report a process you could not stop with its PID.

Keep delivered code/assets, existing plans and source designs. Keep figlens workspaces too: their databases, registry and any index are durable state, not task-owned temporaries; never run `figlens index reset` or `untrack` as cleanup (see [Search and local index](../SKILL.md#search-and-local-index)). Start no watch lease, subscription or tracking unless the user asked for one; the only watch state a run removes is a subscription it created with an `owner`, through `watch unsubscribe` with that consumer and owner, never `forget`, another owner's or an ownerless one. Failure, interruption, a blocking gap or unavailable required verification leaves the working plan and the run's export directory available for continuation. Visual acceptance left pending only by an accepted fallback does not: report it and clean up. If the user asks to retain them, follow that instruction.

## Report

Return the implemented scope and the screens left for a next run, then the design component → code map, then the acceptance record in three groups:

- **Proven by a tool:** results with their command or tool output, such as `figlens verify`, tests, builds, axe, a measured font load, or `compare-image` with its viewport, state, `mismatch` and `nonTextMismatch` once every remaining region, text regions included, is fixed or listed as a known difference with its cause.
- **Checked by you:** the code review above, and what you compared or exercised yourself, such as interactions, states or viewports `compare-image` did not cover, layers it ran with in `--ignore` (their size and position checked with `inspect`), and differences under its threshold, with the viewport and state.
- **Not verified:** checks you could not run and unresolved inputs.

List known differences separately: each recorded design departure, accepted fallback with its remedy, and remaining renderer difference. Then give the actual cleanup outcome: plan, export files kept and removed, and processes stopped. Cleanup failure is a reported remaining task, not permission to claim it was done. A successful build alone does not establish visual acceptance.
