# Host plugins

Load this for the per-host plugin manifests at the repository root, which install the shared `skills/figlens/` skill and the figlens MCP server in one step. The npm package's Pi, OpenCode and OMP entries are in [integrations](integrations.md); these manifests do not touch them.

## Layout

The repository root is the plugin root for every host: one `skills/` tree, plus a thin manifest per host. Each MCP entry runs `npx -y @runkids/figlens@<version> mcp`.

| Host | Files | Skill | MCP |
|---|---|---|---|
| Claude Code | `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json` | default `skills/` | inline `mcpServers` |
| Codex | `.codex-plugin/plugin.json`, `.codex-plugin/mcp.json`, `.agents/plugins/marketplace.json` | `skills` | `mcpServers` path; each server needs `"type": "stdio"` |
| Antigravity (`agy`) | `.antigravity-plugin/plugin.json`, `mcp_config.json` | default `skills/` | root `mcp_config.json` |
| Cursor | `.cursor-plugin/plugin.json` | `skills` | inline `mcpServers` |
| Kimi Code | `.kimi-plugin/plugin.json` | `skills` | inline `mcpServers` |

No root `.mcp.json`: Claude Code would offer it to everyone who opens this repository. No hooks or session-start injection: the skill is invoked per task and the server serves its own instructions.

## Install source

Claude Code's and Codex's marketplace entries use `"source": "./"`, the form single-plugin repositories use and tools such as Skillshare read. Every host installs from the default branch (Antigravity and Kimi Code from `/tree/main`, Cursor from a plain `git clone`), so between a feature merge and the next release the skill can name a tool or flag the pinned server lacks, and the server answers with an error; frequent releases keep that window short. Pinning the tag with a `github` or `url` source and `ref` would keep the skill at the release, but Skillshare rejects such an entry as an external source. Skillshare installs the plugin from the repository URL on its `claude`, `codex` and `cursor` targets (`skillshare plugin add https://github.com/runkids/figlens.git --target claude -g`); its Antigravity desktop target finds no manifest it accepts and is blocked.

Between merging the Release Please PR and Publish Release, the manifests' bumped pins name an npm version that is not published yet, so publish promptly.

## Verification

| Host | Evidence | Not verified |
|---|---|---|
| Claude Code 2.1.316 | `claude plugin validate .`; with an isolated `CLAUDE_CONFIG_DIR`, a local marketplace with `"./"` installs it; `plugin details` lists skill and MCP `figlens`; `claude mcp list` reports it connected | a model session |
| Codex 0.166.0 | isolated `CODEX_HOME`: a local marketplace with `"./"` installs it, and `codex mcp list` shows server `figlens` | a model session |
| Antigravity 1.4.2 | `agy plugin validate`: 1 skill, 1 MCP server | install (it writes the real home); a session |
| Cursor, Kimi Code | fields from the hosts' plugin docs and published plugins that ship MCP | no host run |

Codex installs a Git copy of the committed tree: test it after committing, or it reads the previous manifest. Antigravity loads only the first 24,000 bytes of a skill body. The server's `workflow` tool still serves the full guidance.

Not shipped:
- **Hermes Agent** was evaluated and is not supported: its plugin installer refused the repository on size (then over its 10 MB / 400-file limit) and on its security scan, and its skill scanner blocks the skill's `curl … | sh` install line.
- **Muse.** It has no official manifest schema, and its plugins are experimental.

## Install size

Demo media is not in the tree; docs link it at the `v0.10.1` tag (keep that tag), and Git history still holds it. Tags up to `v0.10.1` still carry it. A `--depth 1` clone is about 8.5 MB (6.6 MB of files), down from 95 MB with the media. Claude Code keeps only the files. Codex clones without `--depth` and keeps the full history (about 60 MB). Cursor's documented command is shallow; Antigravity and Kimi Code clone depth is unknown.

## Versions

Release Please bumps every manifest `version` and pinned `@runkids/figlens@<version>` through `extra-files` in `.github/release-please-config.json`. Documentation install commands carry no version (`npx -y @runkids/figlens mcp`, `/tree/main`, a plain `git clone`), so a release edits no doc. The only `generic` entries rewrite the marked `VERSION` line in the Herdr mod's two `engine.js` copies, which are code; `herdr/tests` checks they equal `Cargo.toml`. `scripts/host_plugins_test.py` checks that every manifest pin equals `npm/package.json` and is bumped by `extra-files`, that no tracked Markdown file pins a version, that marketplace sources are the repository root, and that manifest paths exist.

## Adding a host

1. Read the host's plugin docs or installed CLI for its skill and MCP fields. Do not copy another host's fields.
2. Add its manifest directory, list it in `HOST_FILES`, and add each version field to `extra-files`.
3. Document an install command without a version, then install it with an isolated config directory. Confirm the skill and the server's tools, and record the result in the table above.
4. Update the Plugin setup in `README.md` and every locale.
