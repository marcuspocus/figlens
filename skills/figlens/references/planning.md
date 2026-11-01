# Plan implementation slices

Use the figlens skill's tool contracts and image workflow. Keep planning and implementation in the same agent context.

A plan here is a frontend plan: it decides what the design becomes in the project's code (components and their props, the data repeated parts render from, markup and layout) and how it is delivered fast through the project's build and asset pipeline, so the code reads as if the team wrote it. The design's layer tree is evidence for those decisions, not the structure of the code. An existing project's conventions decide language, layout, styling and tooling; for a new project the first slice is setup with the baseline in [Building a frame](../SKILL.md#building-a-frame) step 5 (TypeScript, linter and formatter, code organized by responsibility, styling system with the design's tokens). An instruction to keep code minimal cuts scope and features, never this bar.

## Start with the requested scope

Identify the requested file/node or recognizable region and the existing code it changes. Read that design scope and relevant project code first. A supplied, unambiguous node needs no complete ancestor inventory. Ask only about missing information that could materially change the requested work.

For a region, component or one state, deliver these four fields directly:

1. **Target:** actual source file/node and existing code block.
2. **Structure:** the frontend structure of the change, following [Building a frame](../SKILL.md#building-a-frame) step 5: each component set in scope → an existing component with the props to set, or a new component with its props from variant and component properties; each repeated part → the component plus the data array it renders from; each section → its element and layout intent; shared shell. Preserve unrelated behavior.
3. **Images, if involved:** `node/source + crop/mask → CSS size × DPR → pipeline/format + output size/quality/budget → loading/priority → browser URL/decoded size/transfer bytes + visual check`. Use `inspect` with `images: true`: `sourceRect` is in original source-image pixels, before masks/effects/ancestor clipping; its `outside` gives how far a crop reaches past the source, which figlens renders empty. Unequal normalized transform coefficients alone do not establish distortion. Pixel counts are not a visual assessment or blur diagnosis. Keep unavailable evidence unknown and planned budgets separate from measured bytes. Use the existing image pipeline; pure vector artwork can stay SVG.
4. **Acceptance:** the code review of [Building a frame](../SKILL.md#building-a-frame) step 7 (each planned component and data array exists and is used; the project's type check, lint and format pass), observable results and project-permitted checks; separate checks already run, future verification and material unknowns.

Deliver when these fields make the next change actionable. Choose another read only for a specific missing decision that could change this slice. Keep a small plan in chat unless saving was requested or continuation needs it.

When saving a plan, use [Save and resume proportionately](planning-details.md#save-and-resume-proportionately) for source/scope namespaces and related evidence paths.

## Load detail when needed

For a page or several pages, read [the detailed workflow](planning-details.md) to inventory only the authorized screens/states and detail the next slice, or every slice when the whole plan comes first. For a small scope, reach that reference only for a concrete unresolved dependency: design styles/masks/notes, component reuse, responsive behavior or saved-plan continuity. Read the relevant section, not the whole design or unrelated plans.

A plan-only request ends with the plan. When the user asks to plan the whole scope before implementing, including "plan, then implement", give every slice all four fields and stop with the complete plan for approval; a multi-slice plan is not authorization to start on slice 1. An already authorized implementation continues through the requested scope and verification, updating the plan from observed results. Respect host permissions and filesystem restrictions: exporting references and saving plans write files; under a strict no-writes request, keep the plan in chat and use permitted reads. This workflow is guidance, not a tool sandbox.
