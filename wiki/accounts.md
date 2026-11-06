# Figma accounts

Load this for browser/API alias routing or `accounts.json`. An account is a local alias, not a verified Figma identity. Its browser login and API token are independent: the person must choose matching identities and grant each access. Browser login never obtains an API token.

## Setup

1. Keep the existing default unchanged: no `account` and no override reuse the old browser profiles and `FIGMA_TOKEN`. No migration, registry or new login is required. The default never reads the registry.
2. For named browser access, run `figlens pull --login --account work`; sign in personally in Chrome. Profiles are separate per alias and browser backend.
3. Create the global registry at `$XDG_CONFIG_HOME/figlens/accounts.json`, or `~/.config/figlens/accounts.json` when unset. Windows uses `%LOCALAPPDATA%\figlens\config\accounts.json` unless `XDG_CONFIG_HOME` overrides it. This is the existing workspace storage config root, not a project file.
4. Record environment **names only**, never token values:

   ```json
   {
     "work": {"tokenEnv": "FIGMA_TOKEN_WORK"},
     "client": {"tokenEnv": "FIGMA_TOKEN_CLIENT"}
   }
   ```

5. Set those environment variables privately to Figma personal access tokens with `file_metadata:read`; make them available to the process running figlens. Each CLI shell and MCP host/server must inherit them. Do not paste secrets into commands, logs, committed files, tool arguments or replies.
6. Route designs in the project's committed `.figlens/sources.json`:

   ```json
   [
     {"name":"app","url":"https://www.figma.com/design/WorkKey/","file":"designs/app.fig","account":"work"},
     {"name":"web","url":"https://www.figma.com/design/ClientKey/","file":"designs/web.fig","account":"client"},
     {"name":"old","url":"https://www.figma.com/design/DefaultKey/","file":"designs/old.fig"}
   ]
   ```

Aliases are 1–64 lowercase ASCII letters, digits, `_` or `-`, starting with a letter or digit. Environment references are 1–128 ASCII letters, digits or `_`, starting with a letter or `_`. The registry is a JSON object of at most 200 entries/1 MiB; each entry has exactly one `tokenEnv` string. Invalid fields, aliases or references invalidate the whole registry. The reader never echoes its contents. It is global, so the same alias across projects uses the same profile and environment reference.

## Routing and failures

| Invocation | Selection and account |
|---|---|
| `figlens pull` | List only, no download or browser. Each mapped account selects its API token. |
| `pull --only app --only web` | Download the named designs through their mapped browser profiles, sequentially by account; no API token needed. |
| `pull --stale` | Each design's API account checks metadata; stale and missing copies are eligible for its browser profile. |
| `pull --account work` | Still list only; override all mapped aliases for metadata checks. |
| `pull --only web --account work` or `pull --stale --account work` | Override browser profile and API environment routing for the selection. |
| `freshness [file.fig] --account work` | Override mapping API routing; also works with `--url URL` or `--sources PATH`. MCP uses `account: "work"`. |
| Missing-node/empty-find hint | Use the mapped account, with no command override. No resolved token means no hint request. |

Without an override, each design uses its mapped alias; an unmapped alias field means the legacy default, not whichever account signed in most recently. Never guess accounts or probe another on access denial. Named accounts do not fall back to `FIGMA_TOKEN`, even when it is set. A named registry entry may explicitly reference `FIGMA_TOKEN` if the user wants that token.

Missing registry or alias yields `unknown/account_not_configured`; an unreadable, oversized or invalid registry yields `unknown/account_config_invalid`; absent/empty environment token yields `unknown/no_token`. Invalid token characters yield `token_invalid`, and API 401/403 yields `token_rejected`. Local file/metadata problems are checked first and may take precedence. Existing unknown copies are skipped by `--stale`; missing copies remain eligible in named/mixed selections. For compatibility, an entirely default selection with no `FIGMA_TOKEN` still fails `--stale` before browser work. Use `--only NAME` when API access is unavailable.

Named freshness rows add `account`, `tokenVar` (null if unresolved) and `tokenSet`; the latter means a nonempty value exists, not that it is valid or has permission. Top-level `tokenVar: "FIGMA_TOKEN"` and `tokenSet` describe only the legacy default, **not** all accounts. Neither credential values nor token-derived cache digests are returned.

`pull --logout --account work` removes only that alias's browser profiles; plain `--logout` removes all legacy and named profiles. Neither deletes the registry nor changes environment tokens. Browser isolation never proves the user selected the intended Figma identity.

## Consumers and verification

CLI freshness, bare pull, `pull --stale` and stale hints share `src/freshness.rs` credential resolution. MCP exposes the same optional override; Pi, OpenCode and OMP discover it dynamically. Herdr's CLI reads inherit its environment and use mapping routing; no browser tool is added. The browser helper receives profile paths, never API tokens, so its protocol is unchanged.

`tests/freshness.rs` exercises separate token values through curl stdin, CLI/MCP overrides, missing/invalid registries, no fallback, default compatibility, token absence/rejection, browser grouping and a changed registry within one live MCP hint cache. Tokens never appear in curl arguments or reports. No real multi-account Figma login/export is exercised in CI.
