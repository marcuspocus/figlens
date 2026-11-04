<p align="center">
  <img src="https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/logo.png" alt="figlens" width="220">
</p>

# Give your coding agent the design details to build, compare and follow changes

**[Website](https://figlens.runkids.cc/)** · [Docs](https://figlens.runkids.cc/docs/) · [Pi guide](https://figlens.runkids.cc/docs/pi/) · [GitHub](https://github.com/runkids/figlens)

figlens turns saved Figma files into design context your coding agent can act on, not just a screenshot to interpret.

- **Build from the source details.** Read layout, text, styles and variables; export vector icons as SVG and images as assets. A frame bundle brings the structured design, assets and reference render together for a portable handoff.
- **Compare the running UI.** `compare-image` compares your screenshot with figlens's render and names the layers under differing regions. Review those differences and rendering warnings; it is not a pixel-perfect guarantee.
- **Follow design changes.** `diff` compares saved revisions; `changes` and `watch` report updates to tracked local scopes; `freshness` tells when a saved copy is older than the online file. Save a new `.fig` from Figma to pick up design edits; this is not live Figma sync.

Give your agent a saved `.fig`. figlens reads it on the machine where the agent runs, including a remote host or CI, with no Figma app, Figma sign-in or Figma API calls (only the opt-in `freshness` check, with your `FIGMA_TOKEN`, asks Figma when a file was last edited).

## Start with Pi

Requires Pi 1.3.0+. The package includes the CLI, MCP connection, extension and agent skill:

```sh
pi install npm:@runkids/figlens
```

Restart Pi or run `/reload`, then ask. Replace "task board" with a screen from your design:

```text
Build the task board from ./design.fig in this project.
Find the matching frame, use this project's components and image workflow,
then compare the running UI with the design and fix confirmed differences.
Report missing assets and any checks you could not run.
```

Export `design.fig` with Figma's **File > Save local copy** and place it in the project on the agent's machine. Pi resolves frame IDs itself; no manual `/figlens open` or separate global CLI install is required. With no saved copy, copy the layers in Figma (right-click > Copy) and run `npx @runkids/figlens clipboard copied.fig`. The copy keeps node IDs and used components but carries no images or vector outlines, which figlens reports as missing.

| Your task | Pi command |
|---|---|
| Plan only the requested scope | `/figlens plan <request>` |
| Plan, implement, check and correct | `/figlens yolo <request>` |
| Browse a saved design | `/figlens preview ./design.fig` |

The packaged skill guides the agent to follow existing project conventions, check fonts and image delivery, compare the running UI with the design and fix confirmed differences. It reports remaining gaps and checks it could not run. YOLO asks the agent to remove only its run-created temporary plan and scratch exports, and stop the processes it started, after successful verification; existing plans, other `.figlens` files and failed or blocked work are preserved. These are agent instructions, not a guarantee of model behavior or visual fidelity.

[Pi walkthrough](https://github.com/runkids/figlens/tree/main/pi) · [Pi package catalog](https://pi.dev/packages/@runkids/figlens)

## See Pi build the screen

[![Pi plans a local design and builds React beside the live browser](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/taskflow-demo.gif)](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)

Looping GIF at 3× the edited video’s playback speed. Click it for the full 78-second video.

[Watch the 78-second demo](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4). Pi uses `/figlens plan` to read one Cloth dashboard frame from a local `.fig`, builds it with React and Tailwind CSS, then checks and corrects the running UI. The video shows Pi and the live browser together, followed by the actual component structure. Edited time-lapse; video length is not a performance benchmark or a promise of pixel-perfect output. [Design credit, source and recording notes](https://github.com/runkids/figlens/blob/main/wiki/accuracy.md#taskflow-react-and-tailwind-demo).

## OpenCode V2 and OMP

The same package includes independent entries for [OpenCode V2 2.2.0](https://github.com/runkids/figlens/tree/main/opencode) and [OMP 18.13.2](https://github.com/runkids/figlens/tree/main/omp). OpenCode V2: `opencode plugin add @runkids/figlens`. OMP: `omp plugin install @runkids/figlens`.

In [Herdr](https://herdr.dev), the [figlens panel](https://github.com/runkids/figlens/tree/main/herdr) lists a design's screens beside your agents and hands one to the agent on the left after a confirm dialog; it needs no separate install of this package.

The integrations share the CLI launcher, skill and component mapping file. Restart the host, then run `/figlens help`.

## Use with another agent

Connect the local MCP server to your client:

```json
{ "mcpServers": { "figlens": { "command": "npx", "args": ["-y", "@runkids/figlens", "mcp"] } } }
```

For Claude Code on macOS/Linux:

```sh
claude mcp add --transport stdio --scope local figlens -- npx -y @runkids/figlens mcp
```

For Codex CLI on macOS/Linux:

```sh
codex mcp add figlens -- npx -y @runkids/figlens mcp
```

Start a new session and check `/mcp`. The MCP `workflow` tool includes on-demand guidance for design reading, project reuse, existing-UI audits, image preparation, scoped planning and explicitly requested autonomous implementation. It shares the [portable skill's](https://github.com/runkids/figlens#mcp-and-skill) sources; no separate skill install is required for MCP use. The agent carries out the task with its own project/browser tools and host permissions; reading a guide does not execute the workflow.

## Run the CLI

Node.js 18+; macOS, Linux and Windows:

```sh
npx @runkids/figlens preview design.fig
npx @runkids/figlens tree design.fig --depth 2
npx @runkids/figlens nodes design.fig 12:345 > frame.json
npx @runkids/figlens bundle design.fig 12:345 frame.zip
npx @runkids/figlens export design.fig 12:345 icon.svg
npx @runkids/figlens export design.fig 12:345 frame.png --scale 2
npx @runkids/figlens clipboard copied.fig               # the selection copied in Figma
npx @runkids/figlens workflow build   # agent guidance, same as the MCP workflow tool
npx @runkids/figlens workflow audit   # compare existing UI; fixes require a fix request
```

Replace `12:345` with a node ID from the tree. A bundle contains design JSON, variables, assets, reference render and `context.md` instructions. The viewer also exports a standalone offline HTML snapshot: `preview design.fig 12:345 --out preview.html`.

The first launch downloads the matching native release binary, verifies it against the release's `SHA256SUMS` and caches it. Initial setup needs npm and GitHub access; subsequent local parsing works offline. Your model provider and project dependencies may still require network access. You can also install once with `npm install -g @runkids/figlens` and run `figlens`.

## What to check

`.fig` is a snapshot: design updates need a new export. Some assets and fonts may be unavailable, and the independent parser/renderer has known gaps, including Dev Mode measurements. Check the source, output and missing-asset report before claiming a visual match. Exporting or sharing a design requires its owner's permission; the tool's MIT license does not license the design or its assets.

[Full documentation, limitations and standalone installs](https://github.com/runkids/figlens) · [Report a reproducible issue](https://github.com/runkids/figlens/issues)

MIT license.
