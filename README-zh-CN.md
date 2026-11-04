<p align="center">
  <img src="https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/logo.png" alt="figlens" width="220">
</p>

<h1 align="center">给 coding agent 设计细节，<br>用于实现、比对与追踪变更。</h1>

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

<p align="center">
  <a href="https://figlens.runkids.cc/"><img src="https://img.shields.io/badge/website-figlens.runkids.cc-8b78c8" alt="figlens website"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-8b78c8" alt="MIT license"></a>
  <a href="https://github.com/runkids/figlens/releases"><img src="https://img.shields.io/github/downloads/runkids/figlens/total?label=GitHub%20downloads&amp;color=79b9a5" alt="Total GitHub release asset downloads"></a>
  <a href="https://pi.dev/packages/@runkids/figlens"><img src="https://img.shields.io/badge/Pi-package-8b78c8" alt="Pi package"></a>
  <a href="#setup-opencode"><img src="https://img.shields.io/badge/OpenCode-V2-8b78c8" alt="OpenCode V2 plugin"></a>
  <a href="#setup-omp"><img src="https://img.shields.io/badge/OMP-plugin-79b9a5" alt="OMP plugin"></a>
  <a href="#mcp-and-skill"><img src="https://img.shields.io/badge/agents-CLI_%2B_MCP-79b9a5" alt="CLI and MCP for AI agents"></a>
</p>

<p align="center">
  <a href="#start">开始使用</a> · <a href="#demo">演示</a> · <a href="#why">为什么选 figlens</a> · <a href="#bundle">agent 能拿到什么</a> · <a href="#pi">Pi</a> · <a href="#limitations">限制</a> · <a href="https://figlens.runkids.cc/zh-CN/docs/"><b>文档</b></a>
</p>

figlens 把保存的 Figma 文件转成 coding agent 可用的设计上下文，而不只是一张需要猜测的截图。

- **依据原始细节实现。** 读取布局、文字、样式、变量以及 Dev Mode 标注和 Ready for dev 状态，导出 SVG 矢量图标和图片素材。`bundle` 汇集 frame 的数据、素材和参考渲染图；`verify` 报告缺失文件、字体检查事项与交接限制。
- **比对运行中的 UI。** `compare-image` 将截图与 figlens 的渲染结果比对，并指出差异区域下方的图层。仍需检查差异与渲染警告，不保证像素级一致。
- **追踪设计变更。** `diff` 比较保存的版本；`changes` 与 `watch` 报告已追踪本地范围的更新；`freshness` 指出已保存副本是否比在线文件旧。设计改动后需从 Figma 保存新的 `.fig`，并非与 Figma 实时同步。

**离线是基础。** 在 Figma 中选择 **File > Save local copy**，把 `design.fig` 放在代码旁边。之后只需一个 Figma 链接：其中的节点 ID 会选定画面。没有保存的副本时，在 Figma 中复制图层（右键 > Copy），再运行 `figlens clipboard screen.fig`；复制内容不含图片和矢量轮廓（[方法](docs/README-zh-CN.md#copy-a-selection)）。一个原生可执行文件在本地读取，无需运行 Figma 应用、登录或向 Figma 发出网络请求；只有可选的 `freshness` 检查会使用你的 `FIGMA_TOKEN` 向 Figma 查询文件的最后编辑时间。

<a id="start"></a>

## 开始使用

**还没有设计文件？** [用小型公开范例检查环境](examples/first-success/README.md)：验证所选的可执行文件、找到一个 frame，再核对 bundle 内容。不需要模型。

把 `design.fig` 放进项目，然后从下面**任选一种**设置。每种方式都会让 agent 获得 figlens 工具，以及同一套内置的规划、实现和图片处理指引。

| 方式 | 适用对象 | 指引来源 |
|---|---|---|
| [Plugin](#setup-plugin) | Claude Code, Codex, Antigravity, Cursor, Kimi Code | plugin 自带的 skill 和 server 的 `workflow` tool |
| [MCP](#setup-mcp) | Claude Code、Cursor、Codex、Gemini CLI 等 MCP 客户端 | 服务器的 `workflow` tool |
| [Pi](#setup-pi) | Pi 1.3.0+ | 包内附带的 skill |
| [OpenCode V2](#setup-opencode) | OpenCode V2 2.2.0 | 包内附带的 skill |
| [OMP](#setup-omp) | OMP 18.13.2 | 包内附带的 skill |
| [Herdr](herdr/README.md) | Herdr 0.1.12+ 中放在智能体旁的面板：找到画面并交给智能体 | 接收方智能体自己的设置 |
| [CLI](#setup-cli) | 只执行 shell 命令的 agent、脚本与 CI，或你自己 | `figlens workflow <topic>`，或可选的 [skill](#setup-skill) |

**普通与高级。** 普通使用无需任何设置：`figlens search design.fig "query"` 只读取 `.fig`，不会创建任何东西。可选的本地索引仅在你要求时创建（`figlens index prepare design.fig --scope 0:0`），之后跨重启和宿主复用。参见[普通搜索与可选的本地索引](docs/README-zh-CN.md#local-index)。 可选且为实验性功能：`--mode semantic` 或 `hybrid` 使用你自行配置的本地模型按语义排序，参见[可选的语义搜索](docs/README-zh-CN.md#semantic-search)。

<a id="setup-plugin"></a>

### Plugin

一次安装就把 figlens skill 和 MCP server 加入 agent（需要 Node.js 18+）。Server 固定在 plugin 对应的版本（`npx -y @runkids/figlens@<version> mcp`）；要换新版本就更新 plugin。

| 宿主 | 安装 |
|---|---|
| Claude Code | `claude plugin marketplace add runkids/figlens`, `claude plugin install figlens@figlens` |
| Codex | `codex plugin marketplace add runkids/figlens`, `codex plugin add figlens@figlens` |
| Antigravity | `agy plugin install https://github.com/runkids/figlens/tree/main` |
| Cursor | `git clone --depth 1 https://github.com/runkids/figlens ~/.cursor/plugins/local/figlens`，然后重新加载窗口 |
| Kimi Code | 在会话中运行 `/plugins install https://github.com/runkids/figlens/tree/main`，然后运行 `/reload` |

[skillshare](https://github.com/runkids/skillshare) 可一次把同一个 plugin 安装到多个宿主：`skillshare plugin add https://github.com/runkids/figlens.git --target claude --target codex --target cursor -g`；之后用 `skillshare plugin update figlens -g` 更新。

重启宿主或开启新会话，确认已列出 figlens tools。Plugin 安装会复制仓库：约 7 MB，Codex 另外保留 Git 历史（约 60 MB）。Antigravity 只加载 skill 的前 24 KB；完整指引由 server 的 `workflow` tool 提供。暂不支持 Muse。[宿主对照表与验证范围](wiki/host-plugins.md)

<a id="setup-mcp"></a>

### MCP

**Claude Code**（Node.js 18+，macOS/Linux）：

```sh
claude mcp add --transport stdio --scope local figlens -- npx -y @runkids/figlens mcp
```

**Codex CLI**（Node.js 18+，macOS/Linux）：

```sh
codex mcp add figlens -- npx -y @runkids/figlens mcp
```

**任意 MCP 客户端：**

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

没有 Node.js 时，先安装独立可执行文件（见 [CLI](#setup-cli)），再改用 `"command": "figlens", "args": ["mcp"]`。打开新的 session，确认能看到 figlens 工具（Claude Code 用 `/mcp`）。无需另外安装 skill。在 `mcp` 后面加上 `--root <dir>`，可以限制 figlens 能读写的文件夹。

<a id="mcp-args"></a>

#### MCP 启动参数

将以下选项放在客户端的 `args` 中、`mcp` 之后。MCP 启动选项只有这两个，均可选且可重复。

| 选项 | 用途 |
|---|---|
| `--root DIR` | 允许读取 `DIR` 及子文件夹中的设计文件，以及写出导出文件。未指定时，figlens 不增加路径限制，仍受操作系统权限约束；不是默认限制在当前项目，也不会扫描整台电脑。 |
| `--storage-root DIR` | 允许在 `DIR` 及子文件夹中明确指定自定义索引存储位置。它不会选择或更改默认存储位置。 |

请使用已存在目录的绝对路径。每个选项和路径都是独立的数组项；多个文件夹要重复写选项。`--root` 是路径边界，不是管理员权限；只限制 figlens MCP，不限制 agent 的其他工具。

两个设计／输出范围和两个 storage 允许范围的示例：

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

没有 `figlens mcp --storage` 选项。实际位置由 `index` tool 的 `storage` 参数指定（允许的 storage 范围内的绝对路径），或使用 CLI `figlens index prepare design.fig --storage /absolute/storage-a`。`prepare` 后会记住该 workspace 的位置，跨重启、跨 host 复用，不必再传 `storage` 或 `--storage-root`。未自定义位置时，使用每位用户的默认目录，无需 storage 标志。

要避免在提交进 Git 的配置里写死这些路径，或在多个 Git worktree 之间共享设计稿，请看 [提交进 Git 的配置中的 root](wiki/mcp.md#roots-in-a-checked-in-config)。

多个 storage root 只会扩大允许范围：每次 `storage` 仍只选一个位置，不会自动分流、同步或备份索引。已有 workspace 数据库不能仅靠改指定另一个 root 搬家；位置冲突时 figlens 会拒绝。

<a id="setup-pi"></a>

### Pi

一个包就包含 CLI、MCP、viewer、`/figlens` 命令和 skill：

```sh
pi install npm:@runkids/figlens
```

[Pi 扩展说明](docs/README-zh-CN.md#pi)

<a id="setup-opencode"></a>

### OpenCode V2

在 OpenCode V2 安装该包：

```sh
opencode plugin add @runkids/figlens
```

重新启动 OpenCode，再执行 `/figlens help`。 [OpenCode V2](opencode/README.md)

<a id="setup-omp"></a>

### OMP

在 OMP 安装同一个包：

```sh
omp plugin install @runkids/figlens
```

重新启动 OMP，再执行 `/figlens help`。 [OMP](omp/README.md)

<a id="setup-cli"></a>

### CLI

通过 npm 运行，或安装独立可执行文件：

```sh
npx @runkids/figlens --help                                                                # Node.js 18+，免安装
curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh | sh  # macOS、Linux
brew install runkids/tap/figlens                                                           # Homebrew
irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 | iex      # Windows PowerShell
```

`npx` 会运行它解析到的包版本对应的可执行文件。用安装脚本装的通过 `figlens update` 更新；Homebrew 装的通过 `brew upgrade figlens` 更新。可执行文件支持 macOS（Apple silicon、Intel）、Linux（arm64、x86_64）和 Windows（x86_64）。

使用 CLI 的 agent 可以运行 `figlens workflow build` 读取指引（另有 `audit`、`images`、`search`、`plan`、`planning-details` 和需明确启用的 `yolo`），内容始终与可执行文件版本一致；也可以改用 [agent skill](#setup-skill)。

[CLI 参考](docs/README-zh-CN.md#commands)

<a id="setup-skill"></a>

### Agent skill（可选）

skill 教给 agent 的流程与内置指引相同：找到 frame、分段读取 bundle、规划范围内的工作，再把运行中的 UI 与设计对比。skill 只有说明，不含可执行文件，需要搭配 CLI 或 MCP 使用。使用 MCP 或 Pi 则无需另外安装。

**推荐：[skillshare](https://github.com/runkids/skillshare)。** 安装一次 skill，就能同步到你使用的每个 agent：

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare sync
```

同一个工具也能为这些 agent 设置 figlens 的 MCP 连接：

```sh
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync mcp --global
```

其他安装 skill 的方式：

```sh
npx skills add runkids/figlens --skill figlens   # Skills CLI
```

也可以从这个仓库或 release 归档中把 `skills/figlens/` 复制到 agent 的 skills 目录（Claude Code：`~/.claude/skills/`）。单独安装的 skill 与可执行文件分开更新；`figlens workflow` 则始终与可执行文件同版本。

然后让它做一个画面：

```text
Build the task board from ./design.fig in this project.
Find the matching frame, use this project's components and image workflow,
then compare the running UI with the design and fix confirmed differences.
Report missing assets and any checks you could not run.
```

agent 会找到 frame，将设计数据、素材和参考图打包成 bundle，并用你的技术栈构建画面。共享工作流会指导字体与素材检查；缺失项会被报告，布局实现仍使用已记录的替代方案继续。请让它展示修改的文件与浏览器对比结果。

<a id="demo"></a>

## 演示：Pi 从 .fig 构建 React 画面

[![Pi 从本地 .fig 规划并构建 React 画面，并在实时浏览器旁检查](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/taskflow-demo.gif)](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)

GIF 以剪辑版视频的 3 倍速度循环播放；点击可观看完整 78 秒视频。

**[78 秒视频](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)。** Pi 运行 `/figlens plan`，从本地 `.fig` 读取一个 dashboard frame，用 React 和 Tailwind CSS 实现，然后检查并修正运行中的 UI。视频经过缩时剪辑，不是 benchmark，也不保证 pixel-perfect。设计：[Task Flow by Eymen Eid](https://www.figma.com/community/file/1224268866120711793) · [录制说明](wiki/accuracy.md#taskflow-react-and-tailwind-demo)。

**[从 .fig 到可编辑 HTML/CSS](examples/task-list/README.md)：** 在受控的任务列表实验中，6 次基于 figlens 的尝试均保留了全部 19 处可见文字为真实 DOM 文本，以及全部 4 个图标。CLI 与 MCP 提取也通过了禁用 Docker 网络的验证。案例页提供设置步骤、测量与独立图片对比。

<a id="why"></a>

## 为什么用 figlens，而不是官方 Figma MCP？

当设计需要以文件形式流转时，用 **figlens**：远程 agent、CI、离线工作、agent 之间交接、不限次数的重复读取。当 agent 必须无需导出步骤就能看到实时设计时，用**官方 Figma MCP**。

读取 `.fig` 可离线进行。安装和更新 figlens、`gs://` 输入、选用的 `freshness` 检查、你的模型提供商以及项目依赖仍然需要网络。

[完整对比](docs/README-zh-CN.md#why)

<a id="bundle"></a>

## agent 能拿到什么

一个 frame，一个 zip。有可据以构建的结构、可直接使用的素材，以及用来比对的图片。

[Bundle 内容与使用示例](docs/README-zh-CN.md#bundle) · [与 Figma 有多接近？](docs/README-zh-CN.md#accuracy)

## 详细文档

- <a id="pi"></a>[Pi 扩展](docs/README-zh-CN.md#pi)
- <a id="viewer"></a>[无需 Figma 浏览设计](docs/README-zh-CN.md#viewer)
- <a id="mcp-and-skill"></a>[MCP server 与 agent skill](docs/README-zh-CN.md#mcp-and-skill)
- <a id="commands"></a>[CLI 参考](docs/README-zh-CN.md#commands)

<a id="limitations"></a>

## 限制

- **快照，不是实时同步。** 设计变更后请导出新的 `.fig`。figlens 无法通过 Figma URL 下载设计。
- **格式未公开。** figlens 解码文件内嵌的 schema；新的 Figma 导出可能需要更新 figlens。
- **字体与文字：** 不包含字体文件；被覆写的文字可能保留 Figma 保存的布局。请使用有授权的项目字体并对比结果。
- **支持范围：** [渲染改进](https://github.com/runkids/figlens/issues/36)与[交接语义](https://github.com/runkids/figlens/issues/37)已在 issue 中跟踪。另见[当前语义支持范围](wiki/semantics.md)。
- **检查输出。** 阅读 bundle 的 `warnings`、`missing-assets.json` 和 `context.md` 中未解析的变量，然后将实现与设计对比。
- 已知可见渲染限制会列在节点 `_figlens.renderDiagnostics`、有数量上限的 `inspect` 摘要及 bundle 与 `export` 的 `warnings`；`missing-assets.json` 只列素材失败。没有警告不代表像素一致。

## 帮助 figlens 成长

如果 figlens 帮你把设计变成了代码，欢迎[给仓库点个 star](https://github.com/runkids/figlens)，让更多人发现它。遇到它读不了的文件？请[提交 issue](https://github.com/runkids/figlens/issues)，附上命令和你预期的结果。

开发时 Rust 只在 Docker 中运行：`docker compose run --rm dev cargo test`。参见 [AGENTS.md](AGENTS.md) 和 [wiki](wiki/README.md)。

## 许可

[MIT](LICENSE)。figlens 是独立项目，与 Figma 没有隶属、背书或赞助关系。`.fig` 格式未公开，可能变更；请仅使用你有权访问和分享的文件与素材。
