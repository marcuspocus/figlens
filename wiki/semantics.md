# Partial semantic fields

`nodes` and bundle `design/nodes.json` add the fields below from the effective expanded node. Existing sizing fields and property assignments are unchanged. These mappings are **not compared with same-version Figma exports**; the response states this once as top-level `_figlens.semantics.verification: "not-compared-with-figma"` (also in `inspect`), not on every node. This is not a claim of REST API parity.

## Layout

| Output | Snapshot source | Supported recorded values |
|---|---|---|
| `constraints.horizontal` | `horizontalConstraint` | MIN → LEFT, MAX → RIGHT, CENTER, SCALE, STRETCH → LEFT_RIGHT |
| `constraints.vertical` | `verticalConstraint` | MIN → TOP, MAX → BOTTOM, CENTER, SCALE, STRETCH → TOP_BOTTOM |
| `layoutGrow` | `stackChildPrimaryGrow` | 0 or 1 on normal-flow children of HORIZONTAL/VERTICAL auto-layout |
| `layoutAlign` | `stackChildAlignSelf` | INHERIT, STRETCH, MIN, CENTER, MAX in the same context |

Missing axes stay absent, so `constraints` can contain only one axis. Swaps retain the original child constraints, grow, align and positioning (including absence) for these new fields; explicit overrides take precedence. Legacy sizing remains unchanged. Unknown enums and growth values are not coerced. Absolute-positioned, grid and non-auto-layout children do not receive grow/align; their recorded but unsupported values are identified in metadata. Existing `layoutSizingHorizontal`/`layoutSizingVertical` HUG/FILL/FIXED rules remain unchanged. A directly selected stored node still uses its actual parent for the new fields.

Each field's metadata contains `source` and `state`: `recorded` means a supported stored value and `unsupported` means a present value/context not emitted. A field whose source is not stored has no entry, and a node with no entries has no `_figlens.semantics`; through 0.11.3 such fields were listed with `state: "absent"`. On 61 screens from four community kits (2026-11-02), those entries plus per-node `verification` were 17–25% of `nodes` output; dropping them, with the wrappers they left empty, made it 18–26% smaller. Recorded does not mean calibrated against Figma. Expanded nodes additionally identify `instanceContext.instanceId` and `sourceNodeId` with `state: "effective"`; this describes the expansion used, not an override history.

## Grid auto layout

| Output | Snapshot source | Emitted |
|---|---|---|
| `gridRowCount`, `gridColumnCount` | entries of `gridRows`, `gridColumns` | on GRID frames |
| `gridRowGap`, `gridColumnGap` | same-named fields | when nonzero |
| `_figlens.gridTracks` `{rows, columns}` | `gridRows`/`gridColumns` ordered by `position`, sized by `gridRowsSizing`/`gridColumnsSizing` | each track as the Plugin API's `GridTrackSize` `{type: FLEX\|FIXED\|HUG, value}` (no value for HUG), `{min, max}` when the two stored sizing functions differ, `null` when a track has no stored size |
| `gridRowSpan`, `gridColumnSpan` | same-named fields | on grid children when not 1 |
| `gridChildHorizontalAlign`, `gridChildVerticalAlign` | same-named fields | on grid children when not `AUTO` |

A swapped grid child keeps its own span and alignment, as other placement does (`expand::GRID_PLACEMENT`, kept out of the search fingerprint). Counts, gaps, spans and alignment are copied from same-named stored fields, so they have no per-field `_figlens.semantics` entry; `gridTracks` is `recorded` when track sizes are stored. REST's `gridRowsSizing`/`gridColumnsSizing` are CSS `grid-template` strings whose exact format Figma does not document, so figlens gives the stored tracks instead of composing one. Children get no `gridRowAnchorIndex`/`gridColumnAnchorIndex`: in 22 grids of one production file (2026-11-02), 17 of 124 stored anchors disagreed with the saved child positions (two grids anchored nearly every child to cell 0,0), all within the 20 grids that also store `gridReflowEnabled`, while child order matched the saved positions row by row in all 22. Children with stored anchors report `gridCell`, and frames with `gridAutoTracks`/`gridReflowEnabled` report `gridAutoPlacement`, in the handoff gaps below. No grid was compared with a Figma export.

## Component definitions

`componentPropertyDefinitions` appears on selected COMPONENT/COMPONENT_SET nodes and on referenced `components`/`componentSets` metadata. Definitions resolve `parentPropDefId` through the same parent lookup used by existing instance property assignments. Provenance entries retain own/resolved definition IDs and source node IDs.

- BOOL/BOOLEAN, TEXT and INSTANCE_SWAP emit their known type. Non-variant names use `name#definitionId`.
- Defaults use the existing selection: `varValue.value`, then the `value` object (or `initialValue` when `value` is not an object). Boolean false and empty text are preserved. A default with the wrong type is omitted and marked unsupported. Swap defaults require a real stored GUID; arbitrary strings and the unset sentinel are omitted.
- Swap `preferredValues.instanceSwapValues` emits only lists entirely composed of COMPONENT/COMPONENT_SET entries with nonempty keys. Unsupported lists are omitted and marked; an explicitly empty supported list remains empty.
- VARIANT emits its name/type only. Default and complete options are not established and are marked unsupported. SLOT and unknown types remain provenance-only. Unresolved parent references do not become guessed definitions.
- Missing definitions are absent, explicitly empty arrays are recorded, and malformed definition lists are unsupported. Metadata distinguishes these from unsupported entries within a recorded list.

`inspect` shows layout fields, type-only definitions and provenance for the nodes it already includes. Definition lists, provenance entries and render diagnostics are each limited to 20 per shown node; counts/truncation remain explicit. Full values stay in `nodes`/bundles. `compare` counts constraints, grow, align and definitions separately, including definitions on referenced component metadata; it does not compare `_figlens` provenance. Asset planning also ignores `_figlens` metadata. Pi mapping signatures and human approval rules are unchanged.

## Known handoff gaps

`nodes` optionally adds `_figlens.handoffDiagnostics` beside `document`. It counts known stored fields the projection does not emit: non-default `leadingTrim`, base/range hyperlinks (including explicit clearing against a base link), ordered-list start/continuation metadata, grid auto-placement settings and stored grid child anchors, layout guides and SLOT relationships. Effective nodes retain `_figlens.unrepresented` source-field markers. Cached grid fields outside active grid contexts do not establish grid layout. Hyperlink data is never opened, reconstructed or truncated into a different destination.

Each feature has a node count, reason and up to five example node IDs (at most 1,024 bytes each); `examplesTruncated` discloses omissions. Counts concern the selected expanded subtree, including recorded hidden nodes, independently of inspect depth. Hidden instance descendants and unreconstructed slot content are not checked. Exact instance-sublayer selection recomputes the inventory so siblings cannot leak into its counts. Missing diagnostics do not establish complete semantics or Figma parity. Verification remains `not-compared-with-figma`.

`inspect` exposes that inventory as `handoffDiagnostics`; it also preserves existing recorded `textAutoResize`, `textTruncation`, `maxLines` and `textDecoration` in its compact typography. Full text, style ranges and source markers remain in `nodes`. Bundles retain the inventory in `design/nodes.json`, emit one warning per feature and include those warnings in `context.md`. Asset failures remain separate. Export pixels are unchanged; there is no new rendering or inference. Pi's existing discovered tools return the same summary/errors; mapping signatures ignore this metadata. The [raw-field and transport research](handoff-research.md) records evidence and remaining implementation prerequisites.

## Evidence and limits

On 2026-10-26, authorized Aurora and held-out DashArt local snapshots supplied recorded constraint/align values; Aurora additionally supplied growth, inherited text definitions and incomplete variant records. DashArt supplied no component definitions. Positive Boolean and swap/default/preferred-value cases are original synthetic tests, not independent real-file confirmation. Tests cover absence/unknown values, parent resolution, effective instance overrides/swaps, bounded inspection and compare categories.

No paired Figma JSON, PNG or SVG was available. Same-version independent captures are still needed to calibrate the mappings and establish any pixel-fidelity improvement. Snapshot omissions must not be interpreted as Figma defaults. No Dev Mode annotations, full grid semantics, SLOT inference, font reflow or new rendering algorithm is included.

The target shapes are described by the official [REST node types](https://developers.figma.com/docs/rest-api/file-node-types/), [layout constraints](https://developers.figma.com/docs/rest-api/file-property-types/#layoutconstraint) and [component property definitions](https://developers.figma.com/docs/plugins/api/ComponentPropertyDefinitions/). Those documents specify the public target shape; they do not prove the private snapshot mappings.
