<p align="center">
  <img src="https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/logo.png" alt="figlens" width="220">
</p>

<h1 align="center">Your coding agent reads Figma files.<br>No Figma app. No login. No API quota.</h1>

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-8b78c8" alt="MIT license"></a>
  <a href="https://github.com/runkids/figlens/releases"><img src="https://img.shields.io/github/downloads/runkids/figlens/total?label=GitHub%20downloads&amp;color=79b9a5" alt="Total GitHub release asset downloads"></a>
  <a href="https://pi.dev/packages/@runkids/figlens"><img src="https://img.shields.io/badge/Pi-package-8b78c8" alt="Pi package"></a>
  <a href="#setup-opencode"><img src="https://img.shields.io/badge/OpenCode-V2-8b78c8" alt="OpenCode V2 plugin"></a>
  <a href="#setup-omp"><img src="https://img.shields.io/badge/OMP-plugin-79b9a5" alt="OMP plugin"></a>
  <a href="#mcp-and-skill"><img src="https://img.shields.io/badge/agents-CLI_%2B_MCP-79b9a5" alt="CLI and MCP for AI agents"></a>
</p>

<p align="center">
  <a href="#start">Start</a> · <a href="#demo">Demo</a> · <a href="#why">Why</a> · <a href="#bundle">What the agent gets</a> · <a href="#pi">Pi</a> · <a href="#limitations">Limitations</a>
</p>

**The problem.** Your agent needs layout values, text and assets to build a screen. On a remote host or in CI, that design context needs to be available alongside the code, without relying on a running desktop app.

**The fix.** In Figma, **File > Save local copy**. Drop `design.fig` next to your code. figlens parses it locally and gives your agent layout, text, styles, variables, icons, images and a reference render of any frame. One native binary, zero network calls to Figma.

<a id="start"></a>

## Get started

**No design file yet?** [Check your setup with the tiny public example](examples/first-success/README.md): verify your chosen binary, find one frame and validate its bundle. No model required.

Put `design.fig` in your project, then choose **one** setup. Each gives your agent the figlens tools and the same built-in guidance for planning, implementation and image preparation.

| Setup | Use it for | Guidance comes from |
|---|---|---|
| [Plugin](#setup-plugin) | Claude Code, Codex, Antigravity, Cursor, Kimi Code | the plugin's skill and the server's `workflow` tool |
| [MCP](#setup-mcp) | Claude Code, Cursor, Codex, Gemini CLI and other MCP clients | the server's `workflow` tool |
| [Pi](#setup-pi) | Pi 1.3.0+ | the package, which ships the skill |
| [OpenCode V2](#setup-opencode) | OpenCode V2 2.2.0 | the package skill |
| [OMP](#setup-omp) | OMP 18.13.2 | the package skill |
| [CLI](#setup-cli) | agents that only run shell commands, scripts and CI, or you | `figlens workflow <topic>`, or the optional [skill](#setup-skill) |

**Ordinary vs advanced.** Ordinary use needs no setup: `figlens search design.fig "query"` reads the `.fig` and creates nothing. An optional local index is created only when you ask for one (`figlens index prepare design.fig --scope 0:0`), then reused across restarts and hosts. See [Ordinary search and the optional local index](docs/README.md#local-index). Optional and experimental: `--mode semantic` or `hybrid` ranks by meaning with a local model you set up yourself; see [Optional semantic search](docs/README.md#semantic-search).

<a id="setup-plugin"></a>

### Plugin

One install adds the figlens skill and MCP server to your agent (Node.js 18+). The server is pinned to the plugin's release (`npx -y @runkids/figlens@<version> mcp`); update the plugin to move to a new version.

| Host | Install |
|---|---|
| Claude Code | `claude plugin marketplace add runkids/figlens`, `claude plugin install figlens@figlens` |
| Codex | `codex plugin marketplace add runkids/figlens`, `codex plugin add figlens@figlens` |
| Antigravity | `agy plugin install https://github.com/runkids/figlens/tree/main` |
| Cursor | `git clone --depth 1 https://github.com/runkids/figlens ~/.cursor/plugins/local/figlens`, then reload the window |
| Kimi Code | `/plugins install https://github.com/runkids/figlens/tree/main` in a session, then `/reload` |

Restart the host or start a new session, then check that the figlens tools are listed. A plugin install copies the repository: about 7 MB, plus the Git history (about 60 MB) in Codex. Antigravity loads only the first 24 KB of the skill; the server's `workflow` tool serves the full guidance. Muse is not supported yet. [Host matrix and what was verified](wiki/host-plugins.md)

<a id="setup-mcp"></a>

### MCP

**Claude Code** (Node.js 18+, macOS/Linux):

```sh
claude mcp add --transport stdio --scope local figlens -- npx -y @runkids/figlens mcp
```

**Codex CLI** (Node.js 18+, macOS/Linux):

```sh
codex mcp add figlens -- npx -y @runkids/figlens mcp
```

**Any MCP client:**

```json
{
  "mcpServers": {
    "figlens": {
      "command": "npx",
      "args": ["-y", "@runkids/figlens", "mcp"]
    }
  }
}
```

Without Node.js, install the standalone binary ([CLI](#setup-cli)) and use `"command": "figlens", "args": ["mcp"]`. Start a new session and check that the figlens tools are listed (`/mcp` in Claude Code). No separate skill install is required. Add `--root <dir>` after `mcp` to limit which folders figlens may read and write.

<a id="mcp-args"></a>

#### MCP startup arguments

Add these options after `mcp` in your client's `args`. These are the only MCP startup options; both are optional and repeatable.

| Option | Meaning |
|---|---|
| `--root DIR` | Allows design reads and exports inside `DIR` and its subfolders. Without it, figlens adds no path restriction: OS permissions still apply. It does not default to the current project or scan the computer. |
| `--storage-root DIR` | Allows explicitly requested custom index storage inside `DIR` and its subfolders. It does not select or change the default storage location. |

Use absolute paths to existing directories. Each option and path is a separate array item; repeat the option for multiple folders. `--root` means a path boundary, not administrator access, and only restricts figlens MCP, not the agent's other tools.

Example with two design/output roots and two allowed storage roots:

```json
{
  "mcpServers": {
    "figlens": {
      "command": "npx",
      "args": [
        "-y", "@runkids/figlens", "mcp",
        "--root", "/absolute/designs",
        "--root", "/absolute/project",
        "--storage-root", "/absolute/storage-a",
        "--storage-root", "/absolute/storage-b"
      ]
    }
  }
}
```

There is no `figlens mcp --storage` option. Choose the actual location with the `index` tool's `storage` argument (an absolute path under an allowed storage root), or CLI `figlens index prepare design.fig --storage /absolute/storage-a`. After `prepare`, figlens remembers that workspace's location and reuses it across restarts and hosts without repeating `storage` or `--storage-root`. Without a custom location, the per-user defaults apply; no storage flag is needed.

To keep these paths out of a checked-in config, or to share designs across Git worktrees, see [roots in a checked-in config](wiki/mcp.md#roots-in-a-checked-in-config).

Multiple storage roots only expand the allowed locations: each `storage` argument still selects one location. They do not distribute, sync or back up indexes. An existing workspace database cannot be moved by simply naming another root; figlens refuses the conflicting location.

<a id="setup-pi"></a>

### Pi

One package ships the CLI, MCP, viewer, `/figlens` commands and skill:

```sh
pi install npm:@runkids/figlens
```

[Pi extension guide](docs/README.md#pi)

<a id="setup-opencode"></a>

### OpenCode V2

Install the package in OpenCode V2:

```sh
opencode plugin add @runkids/figlens
```

Restart the host, then run `/figlens help`. [OpenCode V2](opencode/README.md)

<a id="setup-omp"></a>

### OMP

Install the same package in OMP:

```sh
omp plugin install @runkids/figlens
```

Restart the host, then run `/figlens help`. [OMP](omp/README.md)

<a id="setup-cli"></a>

### CLI

Run it through npm, or install the standalone binary:

```sh
npx @runkids/figlens --help                                                                # Node.js 18+, no install
curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh | sh  # macOS, Linux
brew install runkids/tap/figlens                                                           # Homebrew
irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 | iex      # Windows PowerShell
```

`npx` runs the binary of the package version it resolves. Script installs update with `figlens update`; Homebrew installs with `brew upgrade figlens`. Binaries cover macOS (Apple silicon, Intel), Linux (arm64, x86_64) and Windows (x86_64).

An agent that uses the CLI reads its guidance with `figlens workflow build` (also `audit`, `images`, `search`, `plan`, `planning-details` and the opt-in `yolo`), which always matches the binary's version, or from the [agent skill](#setup-skill).

[CLI reference](docs/README.md#commands)

<a id="setup-skill"></a>

### Agent skill (optional)

The skill teaches an agent the same workflow as the built-in guidance: find the frame, read the bundle in small pieces, plan scoped work and check the running UI against the design. It holds instructions only, so pair it with the CLI or MCP. MCP and Pi setups do not need it.

**Recommended: [skillshare](https://github.com/runkids/skillshare).** Install the skill once and sync it to each agent you use:

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare sync
```

The same tool can also set up the figlens MCP connection for those agents:

```sh
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync mcp --global
```

Other ways to install the skill:

```sh
npx skills add runkids/figlens --skill figlens   # Skills CLI
```

Or copy `skills/figlens/` from this repository or a release archive into your agent's skills directory (Claude Code: `~/.claude/skills/`). An installed skill updates separately from the binary; `figlens workflow` always matches it.

Then ask for a screen:

```text
Build the task board from ./design.fig in this project.
Find the matching frame, use this project's components and image workflow,
then compare the running UI with the design and fix confirmed differences.
Report missing assets and any checks you could not run.
```

The agent finds the frame, bundles its design data, assets and reference image, then builds the screen with your stack. The shared workflow guides font and asset checks; missing inputs are reported while layout implementation continues with documented fallbacks. Ask for the changed files and a browser comparison.

<a id="demo"></a>

## Demo: Pi builds a React screen from a .fig

[![Pi plans from a local .fig, builds a React screen and checks it beside a live browser](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/taskflow-demo.gif)](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)

Looping GIF at 3× the edited video’s playback speed. Click it for the full 78-second video.

**[78-second video](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4).** Pi runs `/figlens plan`, reads a dashboard frame from a local `.fig`, builds it with React and Tailwind CSS, then checks and corrects the running UI. Edited time-lapse, not a benchmark or a pixel-perfect promise. Design: [Task Flow by Eymen Eid](https://www.figma.com/community/file/1224268866120711793) · [recording notes](wiki/accuracy.md#taskflow-react-and-tailwind-demo).

**[From .fig to editable HTML/CSS](examples/task-list/README.md):** in a controlled task-list study, all six figlens-based attempts preserved all 19 visible text occurrences as real DOM text and all four icons. CLI and MCP extraction also passed with Docker networking disabled. Follow the case study for setup, measurements and independent image comparisons.

<a id="why"></a>

## Why figlens instead of the official Figma MCP?

Use **figlens** when the design should travel as a file: remote agents, CI, offline work, handoff between agents, unlimited repeated reads. Use the **official Figma MCP** when the agent must see the live design without an export step.

Reading a `.fig` is offline. Installing and updating figlens, `gs://` inputs, your model provider and your project's dependencies still need a network.

[Full comparison](docs/README.md#why)

<a id="bundle"></a>

## What the agent gets

One frame, one zip. Structure to build from, assets to use, a picture to check against.

[Bundle contents and examples](docs/README.md#bundle) · [How close is it to Figma?](docs/README.md#accuracy)

## Documentation

- <a id="pi"></a>[Pi extension](docs/README.md#pi)
- <a id="viewer"></a>[Browse a design without Figma](docs/README.md#viewer)
- <a id="mcp-and-skill"></a>[MCP server and agent skill](docs/README.md#mcp-and-skill)
- <a id="commands"></a>[CLI reference](docs/README.md#commands)

<a id="limitations"></a>

## Limitations

- **Snapshots, not live sync.** Export a new `.fig` when the design changes. figlens cannot download a design from a Figma URL.
- **Undocumented format.** figlens decodes the file's embedded schema; new Figma exports may need a figlens update.
- **Fonts and text:** font files are not bundled; overridden text can retain Figma's saved layout. Use licensed project fonts and compare the result.
- **Support coverage:** follow [rendering improvements](https://github.com/runkids/figlens/issues/36) and [handoff semantics](https://github.com/runkids/figlens/issues/37) in the issue tracker. See [current semantic coverage](wiki/semantics.md).
- **Check the output.** Read bundle `warnings`, `missing-assets.json` and unresolved variables in `context.md`, then compare the implementation with the design.
- Known visible rendering limitations appear in node `_figlens.renderDiagnostics`, bounded `inspect` summaries and bundle and `export` `warnings`; `missing-assets.json` lists only asset failures. No warnings does not establish pixel fidelity.

## Help figlens grow

If figlens turned a design into code for you, [star the repo](https://github.com/runkids/figlens) so others find it. Hit a file it cannot read? [Open an issue](https://github.com/runkids/figlens/issues) with the command and what you expected.

Development runs Rust in Docker only: `docker compose run --rm dev cargo test`. See [AGENTS.md](AGENTS.md) and the [wiki](wiki/README.md).

## License

[MIT](LICENSE). figlens is an independent project, not affiliated with, endorsed by or sponsored by Figma. The `.fig` format is undocumented and may change; use only files and assets you have permission to access and share.
