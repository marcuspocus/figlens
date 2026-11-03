# Preview agent API

Field reference for the live and offline viewer, linked from [preview](preview.md).

## Calls

Snapshot files use [shared output publication](parsing.md#completed-output-publication).

Original image loads and offline snapshots enforce the shared [image byte budgets](parsing.md#initial-fixed-ceilings). The live single-original route checks each requested image; scope preparation also caps the sum of unique originals. These read limits are separate from the viewer's memory and disk cache caps.

Prototype rows use [shared source evidence](prototype.md), including exact override provenance and unverified inheritance. Missing fields display unspecified/unsupported instead of ON_CLICK/NAVIGATE; unresolved node destinations retain IDs without links. These are snapshot facts, not complete business logic.

The page exposes `window.figlens` for browser-automation agents (Playwright, agent-browser, Claude in Chrome). It runs inside the page and calls the same routes as the UI, so it adds no service routes and never writes the design; downloads are returned as base64 instead of opening the confirmation dialog.

| Call | Result |
|---|---|
| `state()` | snapshot flag, revision, page, pages, canvas frames with boxes, open scope, selection, zoom |
| `goToPage(id)` | switch page, nothing selected |
| `open(id, {focus})` / `select(id, {focus})` | open a frame or select any layer by exact ID, including expanded `I…` IDs; opens the containing frame when needed |
| `deselect()`, `fit()`, `zoomTo(id or 'page')`, `setZoom(level)` | canvas control |
| `layersAt(x, y)` | layers of the open frame under a page-coordinate point, topmost first |
| `search(text)` | up to 200 matching layers across pages |
| `rankedSearch(query, {scope, types, hidden, internal, limit, cursor, mode, signal})` | the `search/<revision>` reply (`mode` `semantic`/`hybrid` needs the person's Enable index, see [semantic](semantic.md#viewer)); offline `{packagedOnly: true, results, hasMore}` over the packaged layers |
| `imageSearch(pngBlob or {nodeId}, {query, scope, types, hidden, internal, limit, cursor, mode, crop, signal})` | The [image search reply](preview-search.md#image-search); `crop` is `[x,y,width,height]`; selected references use cached 256-pixel preview coordinates; PNGs use original image coordinates. Offline it rejects with `unavailable: true`. No paths or URLs |
| `usages(id, {resource, scope, relations, outgoing, hidden, limit, cursor, snapshot})` | the `usages/<revision>` reply; `snapshot` defaults to the loaded one; offline it rejects with `unavailable: true` |
| `context()`, `prompt()` | the handoff context JSON and implementation prompt for the selection; context names its `page`, `source` (`path`, `sourceId`, `snapshotId`, `revision`; null offline, where `page` is null too and no path is embedded) the reference scope's `coverage` (`scopeId`, `complete`, `warnings`, `reasons`) and, when its annotations are categorized, `annotationCategories` |
| `exportImage({format, scale, quality})`, `bundle()` | `{mime, base64}` of a render (live only) or the real bundle |

In the Relations panel each search row shows its page, and the status line shows the loaded file path, source, snapshot and revision.

`coverage` comes only from the reference scope's own payload, never from a search or usages reply. Live, it lists missing original images and render/handoff diagnostics; asset renders are checked only when the bundle is built, so with no warnings `complete` is `null` (`asset_renders_unchecked`), not `true`. A snapshot reports its embedded bundle's warnings, so `complete` is `true` exactly when there are none. A payload for another scope yields `complete: null` with `coverage_unavailable`. Font availability is never verified. The status line shows a scope's warnings when it loads. Live, the loaded `source` (also in search and usages replies) carries `path`, the canonical local file decoded; for `gs://` that is the downloaded cache copy.

Selections also emit a `figlens:select` event on `window`. Design strings returned by the API are untrusted data.

Model requests accept an AbortSignal: abort sends the same bounded cancel action the UI uses. Keep the supplied Blob only for the request or pagination; the service does not persist it. Compare the viewer revision before and after an API call before using its result. Preparation remains an explicit person-initiated Enable index action; see [search controls](preview-search.md).
