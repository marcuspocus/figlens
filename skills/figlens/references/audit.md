# Audit an existing UI

Use this workflow when comparing existing code with a design, checking visual acceptance or correcting layout differences. A review-only request authorizes no application edits; a request to fix authorizes scoped corrections. Follow host restrictions: a browser prohibition stays in force, and strict no-writes also excludes reference exports and saved reports. Continue permitted static work and keep unavailable visual checks pending.

## 1. Establish scope and coverage

Identify the design snapshot and frame, platform, route, viewport and relevant data/UI states, then locate the actual implementation. Resolve recognizable design names with `find`/`tree`; use [Building a frame](../SKILL.md#building-a-frame) for tool contracts when needed. Record the snapshot identity and code revision when available. A local copy does not establish that the online design is current.

For a page or several pages, list only the requested screens/states and mark each as not inspected, structure inspected, statically compared or visually compared. Inspecting a parent or delegating a page does not complete its children. Preserve unexamined items in the final coverage list. For one region, keep this in chat; use the project's report convention only when persistence is needed.

## 2. Gather targeted evidence

For the same snapshot and scope, reuse its existing bundle or saved output when it answers the question; revalidate it if the design changes. Read the relevant subtree rather than repeatedly loading the whole file. Use `inspect` for bounds/layout/text and its truncation markers (`childSpacing: "derived"` means the gaps are coordinate differences, not an auto-layout rule), `nodes` for full paints/masks/styles, and `variables` for tokens. Export the containing frame when overlap, clipping or a backdrop matters. Check render/handoff diagnostics and font/asset gaps before treating figlens pixels as design evidence; absence of diagnostics is not a fidelity guarantee.

A flattened image supplies visual evidence, not editable child typography or layout values. Mark those values unavailable rather than inventing them from the image's outer bounds.

Trace the project's real component, props, data mapping and effective styles, including shared wrappers, inherited rules and breakpoint overrides. Literal values found by search are starting points, not proof of the layout the browser produces. Preserve existing behavior and reuse the project's parts when fixing a visual difference.

Apply the shared [Verification evidence](../SKILL.md#verification-evidence) rules below to each comparison. For example:

- A row of seven 104px cells does not establish whether the product requires fixed slots or distribution over the current item count. If supported by the existing data contract, check 1, 3 and 7 items against the intended alignment. One seven-item snapshot alone does not prove a fixed seven-column requirement; keep that decision unresolved without design/requirement evidence.
- For progress markers, check the connector endpoints and stacking, not only circle diameter and line width. A line behind a translucent circle may remain visible inside it. Compare the composited result and ancestor opacity/clipping before prescribing a fix.

When `compare-image` regions sit over content that comes from data (back-office images, user names, balances, an empty list on a test account), rerun with those layers in `--ignore` (MCP `ignore`) so the score tracks layout, and say what was ignored and why. An ignored layer's own size and position are no longer measured: check them with `inspect`. `image: true` regions are image content to confirm (asset, crop, fit) before ignoring, not proof of a layout match. After the default run, rerun with `--threshold 16` (lower for fainter shadows) to find light borders, dividers and shadows the default 48 cannot see.

## 3. Correct and stop on evidence

When fixes are authorized, correct confirmed differences in the shared implementation, within the project's components, tokens and conventions and keeping its checks passing, and rerun the state that exposed each one. Run relevant project checks as well; compilation/lint does not replace the visual regression check. Leave unsupported design intent as a question rather than inventing product behavior.

Each further read or rerun must answer a named unresolved question with new evidence: a relevant state, descendant, effective code path or implementation screenshot. If rereading the same evidence leaves the decision unresolved, name the missing evidence and its next action. Stop that item until new evidence is available; finish independent items instead of restarting the audit.

Deliver findings separately from verification and coverage: use the shared evidence fields for each item, list checks actually executed and unexamined states, and hand off exact route/state/viewport screenshot instructions when browser checks are unavailable. A round is complete when every scoped item has a supported result or an explicit evidence gap; visual acceptance remains pending wherever required evidence is missing.
