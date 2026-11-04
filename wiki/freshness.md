# Saved copy freshness

Load this to change `freshness`, the `.figlens/sources.json` mapping, or the stale hint on a missing node. A saved `.fig` is a snapshot: when a designer adds a frame after the save, an agent gets `node not found` (or an empty `find`) and would report a design gap. This tells it the copy is older than the online file instead (#224). Nothing is downloaded; refreshing a copy is still the person's **File > Save local copy**.

Code: `src/freshness.rs` (mapping, saved time, REST call, comparison), `src/command/freshness.rs` (the shared request and `with_hint`), `src/main.rs` (CLI `freshness`, the hint on read commands), `src/mcp.rs` and `src/contract.rs` (the `freshness` tool, `sources` admission, the hint on design tools).

## Sources and comparison

| Side | Source | Why |
|---|---|---|
| Saved | `exported_at` in the `.fig`'s `meta.json`, read through `open_zip` with a 1 MiB entry cap | File times change on every checkout or copy. A file without it (the public fixture, `clipboard` output) is `unknown`/`no_saved_time`, never guessed. |
| Online | `file.last_touched_at` (and `version`) from `GET /v1/files/:key/meta` | Tier 3 and scope `file_metadata:read`, against Tier 1 and `file_content:read` for `GET /v1/files/:key?depth=1`, which also returns the components and styles maps. Checked 2026-11-04 in Figma's file-endpoint and rate-limit docs. |
| Compare | Both parsed to epoch milliseconds (`millis`, RFC 3339 with any fraction or offset) | String order fails across precisions (`49Z` against `49.965Z`). Online later than saved is `stale`; otherwise `current`. |

The request runs `curl` (no HTTP dependency in the binary, like `gcloud` for `gs://`), with `-q` (no `~/.curlrc`), a 5 s connect and 10 s total timeout and a 1 MiB response cap. The token comes only from `FIGMA_TOKEN` and reaches curl as a `header = "X-Figma-Token: ..."` config line on stdin, so it is never on a command line or in output. A token holding `"`, `\` or control characters is refused (`token_invalid`) rather than escaped. The file key comes from a `design`, `file` or `proto` URL (a `/branch/<key>` URL's branch key; a file whose name slug is `branch` keeps its own key), ASCII alphanumeric only.

`unknown` reasons: `no_token`, `deadline` (a listing starts no request it could not finish within 60 s, since it holds the MCP interactive lane), `cancelled` (MCP `notifications/cancelled` or the server stopping), `no_saved_time`, `not_mapped`, `file_missing`, `url_unrecognized`, `outside_roots` (MCP), `curl_unavailable`, `offline` (curl failed or timed out), `token_rejected` (401/403), `file_not_found` (404), `rate_limited` (429), `unexpected_response`, `token_invalid`. None of them fails the command, and every `unknown` design carries all the report's fields, null when unknown.

## Mapping

`.figlens/sources.json` at a project root: a JSON list of `{name, url, file}` with `file` relative to the project root (the folder holding `.figlens/`); a mapping anywhere else, given by `--sources`/`sources`, names files relative to its own folder. `.fig` copies can live anywhere a mapping names, but the stale hint finds a copy's mapping by walking up from the copy's own folder, so a copy outside the project folder gets listings and no hint. It sits in `.figlens/` beside the committed `component-mappings.md`, figlens's project folder (user decision 2026-11-04, over `figma/sources.json`), and is committed too. Limits: 1 MiB, 200 entries; a malformed entry fails the listing with its index. `--sources`/`sources` reads another path by the same rule, made absolute first so a bare `.figlens`-relative `sources.json` still resolves from the project root. Discovery walks up from the CLI's working directory, or from a checked file's folder, to the first `.figlens/sources.json`; under MCP roots a mapping outside them is skipped, and an entry outside them is `outside_roots` (a missing file whose existing ancestor is inside them is `file_missing`). #225 (`pull`) will read the same mapping.

## Command and tool

- CLI `figlens freshness` lists the nearest mapping; `<file.fig>` checks one file from its nearest mapping (`file_missing` when it does not exist, `not_mapped` when no mapping lists it); `<file.fig> --url URL` needs no mapping; `--sources PATH` lists that mapping. Prints `{mapping, tokenVar, tokenSet, designs: [{name, file, savedAt, onlineModifiedAt, onlineVersion, state, reason}]}`.
- MCP `freshness` takes the same as `sources`, `file` and `url` (no working directory, so one of `sources`/`file` is required). It refuses `gs://`. Annotations: read-only, open world. It runs on the interactive lane and never loads the warm document.
- Pi, OpenCode and OMP discover the tool from the server's `tools/list`; no adapter change ([integrations](integrations.md#package-and-shared-behavior) records the `FIGMA_TOKEN` environment caveat). Their READMEs describe it under "Checking a saved copy"; the MCP parameters are in [mcp-contract](mcp-contract.md). OpenCode's CLI-path fallback tools (used when its MCP server is disabled) have no `freshness`; it covers the search flow only.
- The Herdr panel (`herdr/`) needs no change: it runs CLI `find` and `inspect`, ignores a `find` reply's `freshness` and shows the hinted error text as is. With `FIGMA_TOKEN` in Herdr's environment, each empty panel search on a mapped copy makes one request, since separate CLI processes share no hint cache.

## Stale hint

`with_hint` wraps tree, find, inspect, node, nodes, variables, bundle and export in the CLI, and every design tool's result in MCP. Only a `NodeNotFound` error, or a find with no results, looks further, and only with `FIGMA_TOKEN` set, a mapping found from the file's folder and an entry naming that file does it make one request. The answer (stale or not) is kept per path, modification time and size for 60 s, and concurrent misses on one copy wait for the first, so an agent trying several ids makes one request; a re-saved copy is checked again. `stale` and the `freshness` report share `source_for` to find a file's mapping entry. A stale copy appends the "save a new local copy" sentence to the error (kind and recovery kept), or adds `freshness` (the design report plus `hint`) to the find reply. `nodes --ids`, `export --frames`/`--batch`, `compare` and `gs://` sources do not carry it.

## Tests

`src/freshness.rs` unit tests cover time parsing, URL keys, mapping resolution, `exported_at`, stale/current/rejected with a `curl` stand-in script, and the token staying off curl's arguments. `tests/freshness.rs` runs the CLI and MCP server with a `curl` stand-in on PATH: listing, a bare relative `--sources`, a missing file, no-token calls nothing, the hint on a stale copy and not on a current one, CLI and MCP find, MCP inspect of a missing node, every field on an unmapped file, one request for repeated misses, root refusal and a bad flag's usage. `src/mcp/tests.rs` checks the tool's annotations and parameters. No test reaches Figma; the real endpoint was not exercised in CI.
