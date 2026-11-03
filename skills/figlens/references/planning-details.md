# Page planning and unresolved design dependencies

Read the sections needed for a page/multi-page request or a specific unresolved decision from [the main planning workflow](planning.md).

The request bounds discovery, including when it covers the whole design. FRAMEs can be layout containers, states or components; CANVAS does not establish a route. Group screens using actual parent relationships, purpose, copy, references and verified code entries. Preserve state/viewport differences. A fixed path offset, matching name or repeated route does not establish ownership or duplication. Treat annotations as leads to verify, including claims that a frame is a draft or excluded.

For multiple screens, record name/purpose, actual frame IDs and paths, state/size, code entry (verified/candidate/unknown), inclusion and important differences. Mark uninspected details. `find` searches stored nodes, requires a non-empty query, returns at most 200 matches and has no offset: refine or scope a truncated search. Instance contents need expansion. `inspect` can truncate by depth or its shared 200-node budget; inspect relevant child roots or extract local subtrees from saved full `nodes` output as needed. `truncated: false` covers that query, not the entire design. Bundle/render only the scopes needed next.

## Read design evidence beyond the reference image

Use the render to check appearance. When a decision depends on style names, masks or crop, read full `nodes`: the top-level `styles` map resolves node style IDs; node fills, `isMask`/`maskType`, clipping and image transforms describe structure absent from the image or compact `inspect` summary. When expanded instance data leaves a relevant decision unresolved, follow its component ID to the actual definition, including on canvases such as "Internal Only Canvas". Excluding a canvas from delivery does not exclude its components as evidence; retrieve only the dependencies relevant to this slice.

Read relevant Dev Mode annotations (`annotations` in `inspect`/`nodes`), hidden TEXT notes and component descriptions as design context, preserving their associated node and provenance; keep them out of visual output. Dev Mode measurements are not emitted by figlens. When one matters, request its value and the nodes it spans, then record it as supplied evidence. Continue independent work; leave a decision blocked only if the missing information would materially change that slice.

## Ground decisions in the project

Read the target implementation and a real usage or neighboring example. Trace only dependencies that affect this change: layout, component contract, styling/tokens, state/data, image delivery and allowed checks. Applicable instructions supplement the project's actual architecture and established practice. Identify absent or unavailable evidence instead of inferring a framework from design annotations.

Record shared facts once, with actual file paths and usage: components/props or native markup, token matches, relevant fonts, image component/loader/asset conventions and checks. Each section records its differences. Preserve existing behavior for a visual-only change. Existing mappings are candidates to recheck, not proof of imports, behavior or visual correctness. A design with zero component instances remains implementable.

Reuse existing foundations. Turning the design's component sets and repeated parts into components and data is the implementation itself ([Building a frame](../SKILL.md#building-a-frame), step 5), not a speculative abstraction. Beyond that, build only the wiring needed for a representative slice before proposing new shared abstractions. For multiple pages, examine differences that could invalidate the first slice's decisions; choose a slice exposing major layout or asset risks. Extract shared code when actual usages have compatible semantics/contracts. Beyond mapping the design's variables into the project's theme (or a new project's baseline), design variables, variants and annotations alone do not authorize new theming, i18n, routing or data architecture. State unverified assumptions and material decisions for the user.

## Detail for page or multi-page plans

Expand the four fields when page composition, states or dependencies would otherwise force the implementer to guess. Omit inapplicable details; point to design subtrees for exact values rather than transcribing JSON.

- **Identity and target:** actual source file and frame/node IDs, ancestor context when it affects layout or crop, reference location if available, current state/size, existing or candidate code targets, and the intended change. Distinguish observed, inferred and unknown facts. Keep full expanded instance IDs; use the skill's Figma link rules when a file key is known.
- **Layout and reuse:** meaningful sections such as header, filters/results or card grid; their root nodes, existing components/usages, relevant tokens, flow and positioning decisions. Repeated items share a specification plus content/state differences. Gaps under a parent with `childSpacing: "derived"` are coordinate differences, not an auto-layout rule: compare sibling screens before turning one into a token.
- **Images:** use the image decision row in [the four-field plan](planning.md), following the project's working pipeline and its props. Planned budgets remain separate from measured bytes. The same imageRef can have different crops; keep originals as sources.
- **Behavior, when involved:** trigger → expected result → existing handler or requirement → acceptance. A static active tab or error screen does not establish navigation, validation or backend contracts; verify behavior in code or requirements. Treat unavailable prototype behavior as unknown.
- **Responsive behavior, when involved:** requested viewports, observed design differences and relevant existing breakpoints. A desktop frame alone does not establish mobile behavior. Label any inferred adaptation and its verification.
- **Acceptance:** observable layout/text/state results, relevant interactions/keyboard operations and project-permitted checks. For image changes, check actual requested dimensions/bytes, crop, sharpness, loading and reserved space at relevant widths. Choose tests for the change, not a mandatory full-site matrix. Record unavailable checks and known renderer differences.

## Save and resume proportionately

If the user wants a saved plan or continuation needs one, follow the project's location conventions; otherwise default to `.figlens/plans/<scope>.md`. Put the bundles, reference images and other files you export for the plan in `.figlens/exports/<scope>/`, never loose in `.figlens/exports/`, and record that directory in the plan; a plan-only run deletes none of them, since implementation reuses them. Keep a small plan in one document. Linked page sub-specs are useful only when actual length or handoff warrants them. Tiny tasks can stay in chat. Match the user's requested language.

For multiple saved plans, inspect relevant existing names before choosing a path. Use `.figlens/plans/<design>/<scope>.md` when several design sources or same-named screens need separation; keep its exports under `.figlens/exports/<design>/<scope>/`, inside the self-ignoring `.figlens/exports/` described in the main skill's output rules (`Where outputs go`). Add a recognizable task/state segment only when the same source and scope need independent plans. Prefer the project's existing naming over a new directory scheme. These are agent-chosen paths, not directories allocated by the Pi command.

Reuse an existing plan only after verifying its source and authorized scope; a matching filename alone is insufficient. Record the actual `.fig` locator and frame/node identity inside the plan. Preserve unrelated plans and human notes, and choose a distinct path when an existing name belongs to different work. Load only the relevant plan and evidence, not every plan in the project.

Use this outline only for applicable content:

```text
Goal and authorized scope
Source snapshot and relevant project evidence (shared once)
Screen/state inventory (multi-screen requests only)
Next slice: design/code targets, decisions, dependencies and acceptance
Remaining slices: differences, risks and order; deepen when needed, or detail each like the next slice when the whole plan comes first
Material questions and assumptions
Execution record: changes, checks actually run, remaining differences
```

Read an existing plan before updating it and preserve human notes. Use stable slice labels. Record a measured source snapshot hash when available; bundle-entry hashes are not the `.fig` hash. On resume recheck relevant source and code: changed hashes require revalidation, not silent acceptance of old decisions. A local snapshot cannot prove the online design is current. Task completion and executed verification are separate facts.

## Stop at an actionable next step

Stop discovery when the next slice's target, relevant evidence, necessary dependencies, image decisions and acceptance are clear, with no unresolved decision that would materially change it. Unknowns elsewhere do not block it. When the user asked for the whole plan first, apply this to every slice, then stop for approval. Ask about genuine blockers such as indistinguishable target screens or required behavior conflicting with an existing contract. Avoid unsupported effort estimates and whole-project preparation for a small change.

Use the main planning workflow for plan-only versus implementation authorization and filesystem restrictions.
