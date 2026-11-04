# figlens — 使用指南

[← README](../README-zh-CN.md) · [figlens.runkids.cc](https://figlens.runkids.cc/zh-CN/docs/)

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

- [独立可执行文件安装](#install)
- [为什么用 figlens，而不是官方 Figma MCP？](#why)
- [agent 能拿到什么](#bundle)
- [Pi 扩展](#pi)
- [Herdr 面板](#herdr)
- [无需 Figma 浏览设计](#viewer)
- [MCP server 与 agent skill](#mcp-and-skill)
- [CLI 参考](#commands)

<a id="install"></a>

## 独立可执行文件安装

**没有 Node.js？** 安装独立可执行文件，并改用 `"command": "figlens", "args": ["mcp"]`：

| 平台 | 命令 |
|---|---|
| macOS / Linux | `curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh \| sh` |
| Homebrew | `brew install runkids/tap/figlens` |
| Windows (PowerShell) | `irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 \| iex` |

<a id="why"></a>

## 为什么用 figlens，而不是官方 Figma MCP？

当设计需要以文件形式流转时，用 **figlens**：远程 agent、CI、离线工作、agent 之间交接、不限次数的重复读取。当 agent 必须无需导出步骤就能看到实时设计时，用**官方 Figma MCP**。

| | figlens | 官方 Figma MCP |
|---|---|---|
| 来源 | 导出的 `.fig` 快照 | 实时 Figma 文件 |
| 需要 | 文件本身 | Figma 登录；桌面端 server 需要应用 |
| 速率限制 | 无，本地解析 | Figma 服务限制 |
| 远程 agent / CI | 复制文件，运行可执行文件 | 需登录的远程 server |
| 设计变更 | 导出新的 `.fig` | 自动同步 |
| 交接 | 分享 `.fig` 或 frame bundle | 分享 Figma 访问权限 |
| 还原度 | 独立解析器，有[已知差距](../README-zh-CN.md#limitations) | 由 Figma 维护 |

读取 `.fig` 可离线进行。安装和更新 figlens、`gs://` 输入、选用的 `freshness` 检查和 `pull`、你的模型提供商以及项目依赖仍然需要网络。

<a id="bundle"></a>

## agent 能拿到什么

一个 frame，一个 zip。有可据以构建的结构、可直接使用的素材，以及用来比对的图片。

```text
frame.zip
├── context.md                Start here: instructions for your agent
├── design/
│   ├── nodes.json            Layout, text, styles, Dev Mode notes and expanded instances
│   ├── fonts.json            Font faces the visible text uses, with node ids
│   └── variables.json        Bound variables, collections, modes and aliases
├── assets/                   Image fills, SVGs and composited node renders
├── assets.json               Maps design references to asset files
├── references/screen.png     A render of the frame to compare against
├── missing-assets.json       Anything that could not be produced
└── manifest.json             File roles, media types, sizes and SHA-256 hashes; the source .fig hash with --hash-source
```

只需要其中一部分？CLI 可以直接完成：

```sh
npx -y @runkids/figlens find design.fig "login" --type FRAME     # 按名称或文字查找 frame id
npx -y @runkids/figlens inspect design.fig 12:345                 # 精简的布局与文字摘要
npx -y @runkids/figlens nodes design.fig 12:345 > frame.json      # REST 格式 JSON
npx -y @runkids/figlens export design.fig 12:345 icon.svg         # SVG、PNG、JPG、WebP 或 AVIF
npx -y @runkids/figlens bundle design.fig 12:345 frame.zip        # 上面的 zip
```

SVG 导出会嵌入原始图片填充，并把文字转为字形轮廓，因此不需要字体。通过 MCP，光栅导出也会以内联图片返回，agent 无需打开文件就能看到设计。

<a id="copy-a-selection"></a>

### 没有保存的 .fig？复制一个选区

1. 在 Figma 中选中画面或图层并复制：右键 > **Copy**，或 Cmd+C（Windows 和 Linux 用 Ctrl+C）。
2. 运行 `figlens clipboard screen.fig`，或让你的 agent 构建“我刚复制的那个画面”。它会读取剪贴板，并写出每个命令都能读取的 `.fig`。
3. 像其他文件一样传入 `screen.fig`：`figlens bundle screen.fig <node> frame.zip`。

把复制内容粘贴到文本框只会得到其中的文字；figlens 读取的是复制内容的 HTML，其中包含图层。macOS 已用真实复制内容验证；Windows（PowerShell 5.1）和 Linux（`wl-paste` 或 `xclip`）尚未验证。

| | **File > Save local copy** | 复制的选区 |
|---|---|---|
| 范围 | 整个文件 | 你复制的图层，加上它们用到的组件 |
| 图片填充和矢量轮廓 | 包含 | 缺失；export 会在缺少它们的地方发出警告 |
| Figma 链接能找到节点 | 能 | 能，限复制的图层：ID 与源文件一致 |
| 适用于 | 基于放在代码旁边的设计来构建 | 没有保存的副本时，快速查看一个画面 |

<a id="accuracy"></a>

### 与 Figma 有多接近？

以 Figma 自身的 JSON、PNG 和 SVG 导出为基准，在 **826 个正式设计 frame** 和五个 Community 文件上测量（2026-10-22）：

| 检查项 | 结果 |
|---|---|
| 节点结构、名称、可见性、边界和文字 | 771 / 826 个 frame 完全一致 |
| 原始图片填充 | 2,608 / 2,608 字节完全一致 |
| SVG 素材 | 5,014 / 5,197 在像素差异阈值内 |
| Community frame bundle | 306 / 306 无缺失素材 |

这是对上述文件的结果，不保证适用于每个设计。方法和阈值：[准确度报告](../wiki/accuracy.md)。

<a id="pi"></a>

## Pi 扩展

```sh
pi install npm:@runkids/figlens
```

安装说明: [OpenCode V2](../opencode/README.md) · [OMP](../omp/README.md).

重启 Pi 或执行 `/reload`，然后用日常语言要一个画面。Pi 会找到 frame，通过自动注册的 MCP server 读取，并用你的技术栈构建。不需要 node ID，也不需要全局 CLI。

请它找文案或画面时，会直接搜索 `.fig`，不保存任何东西。需要反复搜索时，请 Pi 为某个设计和范围启用索引：它会准备一次本地索引（不需要设置命令），在完成前回报任务状态，之后在重启和其他 host 中继续使用。重置该索引需要你明确要求。询问某个 component、variable、style 或图片用在哪里时，会以同样方式列出精确的 usages：直接读 `.fig`，或在启用索引后读该索引。 请 Pi 跟踪某个设计的变更时，它会在需要时准备索引并 track 该范围，报告上次检查以来的变化；询问两个已保存副本的差异时则用 `diff` 回答，不会启动任何东西。 详情：[本地索引](#local-index)。

- `/figlens plan <request>` 只生成计划，不修改应用程序。
- `/figlens yolo <request>` 规划、构建、检查，并清理自己创建的临时计划、暂存导出与启动的进程。
- `/figlens preview [description]` 打开本地 viewer。
- `/figlens review` 批准 agent 提出的可复用组件映射。

![向 Pi 要一个画面，让它找到设计、用你的技术栈构建并检查结果](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/pi-workflow.svg)

完整流程、命令和限制：[pi/README.md](../pi/README.md)。

<a id="herdr"></a>

## Herdr 面板

在智能体旁边查看设计的 [Herdr](https://herdr.dev) 面板：打开 `.fig`，按名称、文字、节点 ID 或 Figma 链接找到画面或图层，查看完整尺寸，并在显示完整消息的确认对话框之后，以 Build、Plan 或 YOLO 交给左侧的智能体。无需安装 figlens，远程 Herdr 机器上的设计也在原处读取。在 Herdr 中让智能体执行：`Read ~/.config/herdr-app/AGENTS.md, then add this layout: https://github.com/runkids/figlens/tree/main/herdr/figlens-layout`。详情：[herdr/README.md](../herdr/README.md)。智能体的 figlens MCP 服务器在 Herdr 终端中运行时，面板会显示每个智能体正在读取的画面。

<a id="viewer"></a>

## 无需 Figma 浏览设计

[![在本地检查 MCP Apps 设计的图层树、布局和组件属性](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/preview-inspector.png)](https://github.com/runkids/figlens/blob/v0.10.1/assets/pi-demo.mp4)

```sh
npx -y @runkids/figlens preview design.fig                        # 本地 viewer，Ctrl+C 停止
npx -y @runkids/figlens preview design.fig 12:345 --out preview.html   # 可发给任何人的离线 HTML 快照
```

浏览页面和图层、搜索、排序搜索与所选内容的精确用处（usages）、测量，检查布局、字体样式、变量和 CSS，导出任意图层，复制实现提示词，或下载所选内容的 bundle。排序搜索可借助可选的本地模型切换为语义或混合模式，只有按下 Enable index 时，查看器才会为文件建立索引。导入你实现的截图，即可叠加在设计上比对。详情：[预览指南](../wiki/preview.md)。 Visual／Hybrid 可使用参考 PNG 或所选图层，并按像素裁剪。请明确启用视觉索引；查询图片不会保存。

<a id="mcp-and-skill"></a>

## MCP server 与 agent skill

**MCP。** server 在本地通过 stdio 运行，提供 `workflow`、`tree`、`find`、`search`、`usages`、`diff`、`inspect`、`nodes`、`variables`、`bundle`、`verify`、`freshness`、`index`、`jobs`、`changes`、`watch`、`export`、`export_batch`、`compare_image` 和 `models`。`export_batch` 一次调用最多写出 64 个文件，返回 manifest 而非内嵌图片。请传入绝对路径。加上 `--root /path/to/designs`（可重复）可把所有文件和输出路径限制在指定文件夹内。明确指定自定义索引位置时，需要 `--storage-root DIR`（可重复）；已记住的 workspace 位置无需再次指定。参见[启动参数及多个 root 的配置示例](../README-zh-CN.md#mcp-args)。光栅导出会缩小到 `maxSize`（默认 1600 px），让内联图片保持小巧；SVG 只返回 metadata。

agent 可通过 `workflow` 读取内建的 `build`、`audit`、`images`、`search`、`plan`、`planning-details`，或明确请求时的 `yolo` 指引。它们与 portable skill 共用来源；MCP 用户可自行选择是否另装 skill。实际工作仍由 agent 使用项目及浏览器工具，并遵守 host 权限。读取指引不会执行工作流程；使用方式取决于客户端支持与模型行为。

在原生 Windows 上使用 Claude Code：请参照其 [stdio 设置说明](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server)，并为 `npx` 加上 `cmd /c` 包装。

**Skill。** [figlens skill](../skills/figlens/SKILL.md) 教 agent 查找 frame、分段阅读 bundle、规划限定范围的工作，并对照设计验证运行中的 UI。它只安装说明；CLI 或 MCP server 必须另行可用。可用 [Skills CLI](https://github.com/vercel-labs/skills) 安装，或把 `skills/figlens/` 复制到 agent 的 skills 目录（Claude Code：`~/.claude/skills/`）：

```sh
npm install -g @runkids/figlens
npx skills add runkids/figlens --skill figlens
```

**Plugin。** Claude Code、Codex、Antigravity、Cursor 和 Kimi Code 可以把 skill 和 MCP server 作为一个 plugin 一起安装；见 [Plugin 设置](../README-zh-CN.md#setup-plugin)。

**skillshare。** 用 [skillshare](https://github.com/runkids/skillshare) 在一处统一管理 Claude Code、Codex 和 Cursor 的 skill 与 MCP 连接：

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync && skillshare sync mcp --global
```

**安装说明。** npm launcher 会在首次使用时下载匹配的原生可执行文件，对照 release 的 SHA-256 校验和进行验证并缓存。用安装脚本装的通过 `figlens update` 更新；Homebrew 装的通过 `brew upgrade figlens` 更新。可执行文件覆盖 macOS（Apple silicon、Intel）、Linux（arm64、x86_64、静态 musl）和 Windows（x86_64）。每种安装方式都会把[可选的语义搜索](#semantic-search)所用的运行时 `figlens-embed` 放在 `figlens` 旁边；在你配置模型之前，它不会加载任何模型。

<a id="local-index"></a>

## 普通搜索与可选的本地索引

|  | 普通（默认） | 高级：本地索引（可选） |
|---|---|---|
| 作用 | 每次调用都读取 `.fig`，回答 `search`、`find`、`inspect` 和 `export`。 | 为选定范围一次性建立搜索索引，之后 `search` 从索引作答，并跨来源统一排序。 |
| 写入内容 | 不创建 workspace 或索引数据库（`export` 只写入你要求的文件）。 | 用户 state 目录或所选 `--storage` 下的本地 workspace（SQLite 数据库）。 |
| 适用场景 | 一次性提问、脚本、CI。 | 反复搜索同一文件；把项目的多个文件当作一个语料搜索（`--workspace <dir>`）；查找组件、变量、样式和图片的精确使用位置。 |

普通的设计问题不会创建索引。你只需明确选择启用一次（“为这个设计创建索引”）；存储位置由 figlens 决定，不需要配置 SQLite 或其他东西。

```sh
# 普通：无状态，不创建任何东西
figlens search design.fig "checkout button"

# 高级，仅一次：选择范围（节点 id，整个文件用 0:0）；构建在前台运行
figlens index prepare design.fig --scope 0:0

# 索引搜索；若索引范围不是整个文件，加上 --node
figlens search --file design.fig "checkout button" --node 0:0

# 编辑或重新导出 .fig 后：只重建字节有变化的范围
figlens index refresh --file design.fig

# 移动 .fig 后：保留 workspace，再 refresh（使用 prepare 输出的 workspaceId）
figlens index relocate <sourceId> new/design.fig --id <workspaceId>
figlens index refresh --id <workspaceId>
```

MCP：`index` 的 prepare 返回一个 job；轮询 `jobs` 直到 `succeeded`，再用回复中的 `workspaceId` 搜索。

```json
{"tool": "index", "arguments": {"action": "prepare", "file": "/abs/path/design.fig", "scope": "0:0"}}
{"tool": "jobs", "arguments": {"action": "status", "jobId": "<jobId from the reply>"}}
{"tool": "search", "arguments": {"workspaceId": "<workspaceId from the reply>", "node": "0:0", "query": "checkout button"}}
```

生命周期：`refresh` 只重建 `.fig` 字节有变化的范围；`rebuild` 即使没有变化也重建；`clear_cache` 只删除构建可重新生成的数据，保留注册表和已发布的索引；`reset` 删除此 workspace 的数据库（绝不删除 `.fig` 或导出），仅在你明确要求时执行。新增来源、扩大范围、reset 和重新创建，各自都需要单独的明确请求。

移动了 `.fig`？`index relocate` 保留 `workspaceId`、`sourceId` 和存储位置；然后对受影响的范围执行 `refresh`。没有自动发现，也没有基于哈希的自动合并，不需要 reset。

一个文件夹里有多个设计？`index add <dir> --workspace DIR` 一次登记该目录下最多 10 个 `.fig` 文件，并跳过指向目录之外的链接；回复列出 `added`、`existing`、`skipped` 和 `truncated`，重复执行不会新增任何内容，每个文件仍需各自执行 `prepare --scope`。在文件夹内移动的文件会成为新来源，除非使用 `relocate`。 将回复中的 `nextCursor` 作为 `--cursor C`（MCP 为 `cursor`）传入，即可继续登记接下来的 10 个。分页的搜索与库游标有效期为一小时，过期后返回 `cursor_expired`，请重新搜索。

数据：SQLite 数据库保存已注册的本地路径、所选范围的图层名称与有效文本、实例的组件属性值、搜索 postings、精确关系（包含、实例、变体、变量/样式/图片绑定、原型目标）及其指向的定义、哈希、generation 和 job。原始 `.fig` 留在原处且保持不变；其原始字节不会存入数据库。 语义与视觉模型层会加入其嵌入的向量；你启用的视觉索引即使没有模型也会加入已索引图层的 PNG 渲染图；查询图片从不保存。

Pi、OpenCode 和 OMP 通过各自的 opt-in 入口使用同一个索引，workspace 可跨重启和宿主复用。尚未实现：文件事件（watch 以轮询读取文件，且只在有 holder 运行时进行）与推送事件。

<a id="semantic-search"></a>

## 可选的语义搜索

实验性功能。`figlens search <file> <query> --mode semantic|hybrid` 用本地模型按语义对图层排序。默认的 `--mode text` 不变，也不需要模型。

- **一次性设置。** 请自行下载 BAAI/bge-m3 的 `5617a9f61b028005a4858fdac845db406aefb181` 修订版（MIT；`config.json`、`pytorch_model.bin`、`tokenizer.json`），figlens 从不下载。`figlens models configure bge-m3 --model DIR [--runtime FILE]` 用 SHA-256 校验文件，离线启动一次 `figlens-embed` 运行时（未给 `--runtime` 时在 figlens 可执行文件旁查找），并把配置记录在配置目录的 `models.json`。`models list|status` 查看，`models remove bge-m3` 忘记该配置但保留文件。
- **视觉 profile。** `figlens models configure siglip-base --model DIR` 以同样方式校验 google/siglip-base-patch16-224 的修订版 `7fd15f0689c79d79e38b1c2e2e2370a7bf2761ed`（Apache-2.0；`config.json`, `preprocessor_config.json`, `model.safetensors`），并把它登记为视觉搜索使用的图像模型。
- **图片查询（实验性）。** `figlens search --workspace DIR --image shot.png` 按外观对 workspace 的单元排序；`--image-ref '{"sourceId":…,"snapshotId":…,"nodeId":…}'` 改用某个结果已保存的渲染，`--crop x,y,w,h` 可裁切两者。加上查询文字即为 `hybrid`。视觉层由 `figlens index visual` 建立（搜索不会建立，单一 `.fig` 搜索会拒绝图片）。没有图像 profile 或它失败时，只按像素哈希排序，`query.usedModes` 会注明。查询图片从不保存。排序规则仍在验证中，请把结果当作线索。MCP：`search` 的 `image`（root 内的路径）、`imageRef`、`crop`。
- **模式。** `semantic` 按每个图层在名称、文本、组件属性或所在范围中最匹配的片段排序。`hybrid` 融合文本与语义两种排名（Reciprocal Rank Fusion）。没有配置时，`semantic` 以 `mode_unavailable` 失败，`hybrid` 返回文本结果并说明原因。`coverage.layers.semantic` 报告 `ready`、`partial` 或 `unavailable`；部分结果的页面没有 `nextCursor`。模型使用 CPU，Ctrl-C 停止当前请求。每个 MCP/viewer 进程在使用后可同时保留一个文本模型和一个图像模型；一次 Linux ARM 诊断中，文本模型单独约占 2.5 GiB，两者峰值内存之和约 3.5 GiB，不含核心与索引 worker。这些是测量值，不是内存上限。
- **限制。** 嵌入图层名称、有效文本、实例的组件属性值（无法精确解析的值会略过并计数）以及每个图层所在页面与容器的名称。使用 `--workspace`/`--id` 搜索时，读取每个来源已存储的模型层，不会嵌入设计文本；该层由 `figlens index embed` 建立（MCP 服务器会在文本构建后排入队列）。没有该层的来源为 `pending`，页面为部分结果。在此功能冻结的测试集上，模型未达到同义词与用途查询的质量门槛（top-1 错误率 0.25，门槛 0.20），请把结果当作线索。
- **MCP。** `models` 工具（`list`、`status`、`configure`、`remove`）以及 `search` 的 `mode`；`configure` 只运行 figlens 旁的 `figlens-embed`，模型目录和文件必须位于服务器的 root 之内。取消调用会停止正在运行的搜索或设置。

<a id="commands"></a>

## CLI 参考

| 命令 | 输出 |
|---|---|
| `tree <file> [node] [--depth N] [--json]` | 节点 id、REST 类型（与 `nodes` 相同）和名称；缩进文本，或附带每个节点尺寸、隐藏状态与 auto layout 模式的嵌套 JSON；页面或 section 下的节点另附 `firstText`（第一段可见文字） |
| `tree <file> [node] --top-frames [--json]` | 只列出画面，每行一个，可直接传给 `--frames -`：页面或其 section 下的 frame、component、component set 与 instance（跳过 group 与内部页面）；`--json` 附上 page 与 section id，以及每个画面第一段可见文字（`firstText`），可用来区分画布标签与画面 |
| `find <file> <query> [--type TYPE] [--node ID] [--limit N]` | 不区分大小写地搜索图层名称和文字；返回路径、尺寸和预览。默认 20 条结果（1-200） |
| `search <file> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C] [--include-hidden] [--include-internal] [--mode text\|semantic\|hybrid\|visual] [--image PNG\|--image-ref JSON] [--crop x,y,w,h]` | 对图层名称和完整文字做排序搜索，已套用 instance override、文字/布尔属性和 swap；返回精确节点引用（包括可用于 `inspect` 的 `I…;…` 子图层 id）、匹配原因、UTF-16 范围、覆盖情况和 cursor 分页。默认 20 条（1-200）；隐藏图层和内部页面需加对应 flag。以 `--file`/`--workspace`/`--id` 取代 `<file>` 时，读取 workspace 已准备的索引（见 `index`） `--mode semantic\|hybrid` 需要已配置的模型，参见[可选的语义搜索](#semantic-search)。 |
| `models list\|status\|configure <profile> --model DIR [--runtime FILE]\|remove <profile>` | 语义搜索用的本地文本模型（`bge-m3`，实验性）与图片搜索用的图像模型（`siglip-base`，实验性）；`configure` 校验你自行下载的文件，从不下载；`remove` 保留文件。参见[可选的语义搜索](#semantic-search)。 |
| `usages <file> <node\|resource> [--scope ID] [--relation R[,R]] [--out] [--snapshot S] [--include-hidden] [--limit N] [--cursor C]` | 在一个已保存范围内返回图层或资源（`VariableID:…`、`StyleKey:…`、`image:…`）的精确关系，包括嵌套 instance 和 swap：默认返回组件、组件集、变量、样式或图片被使用的位置，`--out` 返回图层指向的目标。每个结果保留精确引用、关系、状态（`resolved`、`missing`、`unresolved` 等）、依据和证据；名称从不算作同一身份。不返回总数，coverage 说明完整或部分。默认 50 条（1-200）。用 `--file`/`--workspace`/`--id` 代替 `<file>` 时读取 workspace 已准备好的 `--scope` 索引。使用 `--out` 时，缓存库副本的使用也会给出其 `library` {kind, namespace, key}。 |
| `usages --file\|--workspace\|--id … --library <kind> --namespace <ns> --key <key> [--definition-source ID --definition-snapshot SNAP --definition-node NODE] [--source ID] [--scope ID] [--include-hidden] [--limit N] [--cursor C]` | 返回使用某个库组件、样式或变量（`kind`）的图层，依据各文件缓存副本的记录，范围是所有来源已准备好的 `--scope`（`--source` 限定为一个）。定义保持 `unresolved`（以三个 `--definition-*` 按精确引用选定时为 `selected`，`keyEvidence: unverified`）；coverage 列出每个来源的状态和部分的原因。参数取自对使用它的图层执行 `usages --out` 时的 `library` 字段 |
| `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]` | 两个已保存版本之间的变更：每个有变更的顶层 frame 一行，按 facet（文本、布局、外观、属性、结构、绑定）计数；`--json` 另含新旧字段值与 edge 变更。图层只按完全相同的 id 与类型匹配；同名或同位置的 frame 只标为 possible match，中途停止的一侧，或唯一差异是无法读取的字段的图层，报告 unknown，绝不当作删除或变更。每页 100 条（1-500）有一侧在上限处停止时，打印 `stopped early` 而不是 `no changes`；用 `--node` 缩小范围。 |
| `inspect <file> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat] [--full-text] [--colors]` | 展开后的精简布局/文字摘要，最多 200 个节点。`--images` 会加上位图尺寸和裁切范围。`--flat` 改为每个节点一行：相对该节点的位置、尺寸、文字、字体、填充、描边、圆角、auto-layout、对齐、fixed/fill 尺寸、阴影与模糊 `--full-text` 返回每段文字的完整内容（最多 10,000 字符）；`--colors` 加入可见的纯色填充（hex、不透明度、绑定变量），`--flat` 也适用；不含渐变、图片或描边。 JSON 输出包含 `path`：从文档到该节点的祖先（`[{id, name}]`，与 `find` 相同），实例子图层同样适用。 |
| `nodes <file> <node> [--compact [--subtrees]]` | REST nodes 响应中单个节点的对象（`document`、`components`、`componentSets`、`styles`）：展开的 instance 与覆写、auto-layout、文字样式、填充、效果、绑定变量、组件属性、渲染边界. 默认 REST 不变。`--compact` 共享重复样式；加上 `--subtrees` 共享重复子树并保留各节点 ID 与边界。见[无损格式契约](../wiki/discovery.md#compact-nodes)。 Batch: `--frames` / `--ids` + `--compact [--subtrees]`. |
| `nodes <file> --frames <list>`, `--frames -` (stdin), `--ids <node>...` | 一次解码读取多个节点：列表中每个 id 一行 JSON（带 `nodeId` 的 `nodes` 输出，或 `{nodeId, error}`） |
| `variables <file> [node]`<br>`variables <file.fig> --format dtcg --collection ID --mode ID --out FILE` | 默认的 REST 格式绑定变量／本地变量输出保持不变。DTCG 2025.10 导出须指定准确的 collection 和 mode ID，不可指定 node；仅支持颜色和无单位数值。请先创建输出目录；任何诊断都会使整个导出失败，输出文件保持不变。library collection 会从文件中缓存的副本导出（`libraryCopy: true`）。指向其他 collection 的 alias 会导出该 collection 的值并记录在 `aliasOf`；该 collection 有多个 mode 时，用 `--alias-mode COLLECTION=MODE` 指定。 |
| `bundle <file> <node> <out.zip> [--states] [--hash-source]` | 单个 frame 的设计 bundle；输出 `{nodeId, path, warnings}` |
| `bundle <file> --frames <list> --out-dir <dir> [--jobs N] [--states] [--hash-source]`, `--frames -` (stdin) | 一次解码，并行生成多个 bundle |
| `verify <bundle> [--fonts <dir>]` | 按 manifest 检查 bundle（`.zip` 或解压后的目录）：已验证、缺失和被改动的文件，缺失的素材，需要在项目中确认的字体，以及 `findings`：每项缺失、不支持或未验证的 handoff 输入，附节点、证据和下一步；在本机或 `--fonts <dir>`（项目文件夹，同时生成 `@font-face`）中找到字体文件的字体为 `local_file`，不代表浏览器能加载 |
| `export <file> <node> <out> [--bounds layout] [--scale N] [--quality 1-100] [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID]]` | SVG、PNG、JPG、WebP 或 AVIF；不超过 4,194,304 像素的 PNG 自动进行无损优化，解码后的 RGBA 不变，若未变小则保留原始编码字节，更大的 PNG 使用常规编码；`--max-kb` 会降低有损质量直到符合大小（PNG 若仍超限则失败，不会回退为有损编码）；`--bounds layout` 按 layout box 而不是 render bounds 导出；`--match-dir DIR`（仅 SVG，含子文件夹）列出绘制相同轮廓的 SVG（`matches`），以及最多 10 个带分数的相似形状（`similar`）；`--hide ID[,ID]` 把 node 内的这些图层当作隐藏、在相同范围内绘制，与正常导出对比即可看出 figlens 画出的这个图层改变了结果的哪些地方（没有差异不代表未渲染的调整没有影响） |
| `export <file> --frames <list> --out-dir <dir> [--format F] [--scale N] ...`, `--frames -` (stdin) | 一次解码导出多个节点（默认 `png`）：写入 `<dir>/<nodeId>.<format>`，每个 id 一行 JSON（带 `nodeId` 的 `export` 输出，或 `{nodeId, error}`） |
| `export <file> --batch <items.json>`, `--batch -` (stdin) | 一次解码导出多个节点，每项各有自己的选项：1-64 个 `{node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}` 的 JSON 数组（MCP `export_batch`）。输出一份 manifest `{count, failed, items}`：每项为 `export` 输出或 `{error}`（其余照常导出）；条目格式错误或 `out` 重复会在写入任何文件前失败 |
| `preview <file> [node] [--out preview.html] [--port N] [--bind IP]` | 从不写入设计的本地 viewer；按 Enable index 后可用语义/混合搜索；`--out` 写出独立的离线快照；结果与复制的 context 会标明页面，live 时也标明已加载文件的路径、source 的 snapshot 与本地 revision（离线时未知，且绝不嵌入路径）；context 也带有参考 scope 的 coverage，未检查时为 `complete: null`. Visual／Hybrid 可使用参考 PNG 或所选图层，并按像素裁剪。请明确启用视觉索引；查询图片不会保存。 |
| `compare <file> <nodes.json> <node>` | 与同一 frame 的 REST 导出做差异比较 |
| `compare-image <file> <node> <screenshot.png>` | 把实现画面的截图与按其缩放渲染的 node 比较（`--scale`、`--limit`、`--out diff.png`、`--full-page` 用于比 frame 短的整页截图，`--ignore` 用于内容来自数据的图层，`--threshold` 用于看出浅色边框；`image: true` 标记图片填充；MCP `compare_image`）。输出 `mismatch`、`nonTextMismatch` 和差异最大的区域（node 像素坐标），每个区域附其下最深的图层；`text: true` 表示区域位于文字上（多为字体替代，需逐一检查） |
| `node <file> <node>` | 单个原始 `.fig` 节点 |
| `clipboard [copied.html] <out.fig>` | 用于没有已保存 `.fig` 的情况：在 Figma 中选中图层并复制（右键 > Copy，或 Cmd/Ctrl+C），再运行此命令，把剪贴板转成每个命令都能读取的 `.fig`；粘贴到其他地方只会得到图层的文字。读取系统剪贴板（macOS；Windows PowerShell 5.1 和 Linux `wl-paste`/`xclip` 未验证）或已保存的剪贴板 HTML 文件。节点 ID 与源文件相同，并包含用到的组件。报告 `missingImages`、`missingComponents` 和 `layersWithoutOutlines`：复制内容不含图片字节和矢量轮廓，因此 export 和 bundle 会在缺少它们的地方发出警告。不会覆盖已有文件。有已保存的 `.fig` 时，改为把 Figma 链接作为节点传入：可保留图片和矢量 |
| `freshness [<file.fig> [--url URL] \| --sources <mapping.json>]` | 在断定设计缺少某个 frame 之前，先确认每份已保存副本是否比在线 Figma 文件旧：检查最近的 `.figlens/sources.json` 中的设计（`{name, url, file}` 列表，`file` 相对于 repository 根目录；`.figlens/` 之外的 `--sources` 映射则相对于它自己的文件夹）、单个文件，或附带 URL 的单个文件。比较副本 `meta.json` 中的 `exported_at` 与在线的 `last_touched_at`（通过 `curl` 和 `FIGMA_TOKEN` 中的 personal access token 读取，scope 为 `file_metadata:read`）；输出 `stale`、`current` 或附原因的 `unknown`，缺少 token 或网络时也不会失败。设置 token 后，在已映射且过期的副本中找不到节点或 `find` 没有结果时，也会提示重新保存本地副本（MCP `freshness`） |
| `pull [--only NAME]... [--stale] [--sources <mapping.json>] \| --login` | 根据最近的 `.figlens/sources.json`（或 `--sources`）刷新已映射的保存副本。未指定 `--only` 或 `--stale` 时不会下载任何内容，只列出每个设计的新旧状态。`--only` 选择设计，`--stale` 只拉取过期或缺失的副本（需要 `FIGMA_TOKEN`）。由专用的 Chrome profile 执行 **File > Save local copy**；下载内容必须能解码，且 `exported_at` 不早于当前副本，才会以原子方式替换；检查失败则保留旧副本；失败时会指出所在阶段（`open`、`editor`、`menu`、`export`、`download`、`install`）。`--login` 打开可见窗口以登录（会话过期时 pull 也会打开）；在其中登录可能会使你在另一台设备上退出 Figma。需要 Node、Chrome 和 `playwright-core` (`npm i -g @runkids/figlens playwright-core`)；仅支持英文 Figma 界面。仅 CLI |
| `index prepare\|refresh\|rebuild\|clear\|status\|add\|remove\|relocate\|reset\|track\|untrack\|embed\|visual` | 用户 figlens state 目录中的本地 workspace，或位于 workspace 之后会记住的 `--storage DIR` 下。`prepare <file> --scope NODE` 为该范围建立一次搜索索引；之后 `search --file <file>`（或 `--workspace DIR`）不再解析文件，跨 workspace 的所有来源统一排序作答。`refresh` 重建有变更的文件，`rebuild` 重建全部或单一 `--source`/`--scope`，`clear` 删除未使用的索引；新索引就绪前继续使用旧索引。`reset` 只删除 workspace 数据库，不会删除设计、导出或记住的位置。`track --scope NODE` 从 baseline 构建开始记录该范围的变更；`untrack` 停止记录并保留历史。 `embed` 用已配置的模型嵌入已发布的范围（占用 CPU 和内存）；仅在需要 workspace 语义搜索时运行。 `visual --scope NODE` 为图片搜索渲染该已发布范围的画面与组件，配置视觉模型时一并嵌入；仅在需要 workspace 视觉搜索时运行。 |
| `jobs status\|cancel <jobId>` | 查看或取消索引构建；`cancel` 在构建停止后才返回 |
| `changes --file <file>\|--id ID\|--workspace DIR [--source ID] [--scope NODE] [--consumer NAME\|--subscription ID\|--cursor C] [--revision R] [--limit N]` | 已跟踪范围记录的变更，从最旧开始：每次发布的构建一个修订（baseline 或 changes），含覆盖情况及按变更类型与 facet 的计数；每个 changes 修订都有 `impact` 摘要，说明变更波及的图层（schema 7 之前为 null）；`--revision` 分页列出该修订变更的节点，再列出 impact 列出的图层（`entry` 标明是哪一种）；`tracking` 指出哪些范围需要 `index refresh` 才能记录变更。只读 |
| `watch ack --file <file>\|--id ID\|--workspace DIR --consumer NAME\|--subscription ID --cursor C [--source ID] [--scope NODE]` | 把 consumer 的检查点移到它以相同筛选条件处理完的最后一页 `changes` 的 `nextCursor`；重复执行是安全的，而且只会前进；唯一例外是当前的 recovery cursor 会把落在删除之后的检查点退回，以重读保留的 revision；在保留策略删除它尚未经过的 revision 之前发出的 cursor 会得到 `event_gap`，不写入任何内容 |
| `watch run --file <file>\|--id ID\|--workspace DIR --scope NODE [--source ID] [--request-id R]` | 在前台持有一个已跟踪 scope 的 lease：workspace 共享的 runner 在每次保存时刷新它（每 250 ms 轮询，每 60 s 校验内容），因此 `changes` 无需 `index refresh` 就会出现新 revision。先输出 lease，在 stdin 关闭、收到 Ctrl-C 或 SIGTERM、或 scope 被取消跟踪时输出一行结束信息。MCP `watch` 的 `start` 与 `stop` 在服务器运行期间持有 lease；未续期的 lease 30 s 后结束，最后一个 lease 结束后 runner 退出 |
| `watch status --file <file>\|--id ID\|--workspace DIR` | workspace 的 lease（`active`、`unauthorized`、`untracked`、`expired`、`ended`）与 runner（`running`、`stopping`、`stopped`、`unknown`，附放置方式）；从不写入 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]` | 从最新修订开始跟随一个源、scope 或确切图层（`--node`），使用它自己的检查点，用 `changes --subscription` 读取、`watch ack --subscription` 确认；不存在或有歧义的图层为 `target_missing` 或 `identity_uncertain`，绝不换成同名图层；`--owner` 让它只在该任务内临时存在 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID --query TEXT [--type T[,T]] [--scope NODE]` | 保存对一个来源 scope 的搜索：立即评估其前 50 名；`changes --subscription` 只读取已保存的评估，从不计算 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --sources visible\|ID[,ID] --query TEXT\|--library KIND --key KEY [--namespace NS] [--definition-source ID --definition-snapshot SNAP --definition-node NODE]` | 对最多 10 个来源的固定集合保存搜索，或保存库项目的使用处及其 `definitionMatch`（`visible` 在此时解析，之后新增的来源不会加入）：`delta.changedSourceIds` 列出字节有变的来源，`corpus_changed` 表示集合有变，`coverage.transitive` 为 `not_followed`，`delta.notFollowedSourceIds` 列出没有使用处的已变更来源（其下游未重新检查）；移除成员会停用它（先 unsubscribe，再重新 subscribe） |
| `watch evaluate --file <file>\|--id ID\|--workspace DIR --subscription ID` | 按 scope 当前的索引评估并保存查询订阅，已评估则不做任何事；该 scope 处于 watch 时，每次发布后会自动进行 |
| `watch list --file <file>\|--id ID\|--workspace DIR [--consumer NAME] [--after ID] [--subscription ID]` | 分页列出可见的订阅及其状态；`--subscription` 只读取该订阅（不可见时为空） |
| `watch unsubscribe --file <file>\|--id ID\|--workspace DIR --subscription ID\|--consumer C --owner NAME` | 删除一个订阅及其检查点，或某个 owner 的临时订阅；tracking 和历史保留 |
| `watch forget --file <file>\|--id ID\|--workspace DIR --consumer NAME [--source ID] [--scope NODE]` | 删除 consumer 的原始检查点；每个 consumer 最多保留 64 个订阅与检查点，整个 workspace 4096 个，不会自动淘汰 |
| `mcp` | 通过 stdio 运行的本地 MCP server |
| `update` | 用最新 release 替换可执行文件 |

需要相关组件 variants 时，使用 `inspect --states` 或 `bundle --states`（MCP 传入 `states: true`，默认 false；无需修改配置）。证据保留来源、未知及截断标记。Bundle 会添加 `design/states.json` 和 `context.md` 摘要。范围与限制参见[状态证据](../wiki/states.md)。

比较同一 component set 的两个 variants：`inspect <file> <variantA> --compare-variant <variantB>`（MCP `compareVariant`）。`variantComparison` 列出原始字段及值集合差异，保留来源和未知标记；子节点依唯一名称、主组件、同数量同名分组内的堆叠顺序配对（`pairedNodes`，附逐对差异；请检查 `pairedBy`，`order` 证据最弱；其余不配对），并标出绑定变量的名称。详见[比较限制](../wiki/states.md#compare-two-variants)。

要比较同一文件中的任意两个节点（空白与已填内容的画面、手机与桌面断点、共用页头的两个画面），请使用 `inspect <file> <nodeA> --compare <nodeB>`（MCP `compare`）。`comparison` 格式相同，另有 `rootDelta`（尺寸与相对于父级的位置）；子节点依唯一名称、主组件、同数量同名分组内的堆叠顺序配对（`pairedBy` 标明方式），其余不配对并标注 `reason: ambiguous-name`；instance 不会展开，节点也不能与自己的祖先比较。`--compare-variant` 仍要求同一个 component set。

node 参数接受 `12:345`、`12-345`，或带 `?node-id=` 的 Figma URL。`<file>` 也可以是 `gs://bucket/object`，通过你的 `gcloud` 登录下载并缓存。固定 object 交接、所需访问权限与更新限制，请参阅 [GCS 快照操作说明](../wiki/gcs.md)。每次 CLI 调用都会解码文件；MCP 只复用最后读取且未变更的本地文件，远程输入仍会重新解析。运行 `figlens --help` 查看完整 CLI 用法，或运行 `figlens inspect --help`（也可用 `-h`）只看该命令的条目。`index add gs://bucket/object` 会登记该 generation 的固定副本并报告 `remote {uri, generation}`；只有 add 会运行 gcloud。

`inspect` 包含已记录的文字尺寸调整、截断、最大行数与装饰。Dev Mode 标注出现在对应节点的 `annotations`（纯文本 `label` 预览、固定的 `properties`、`categoryId`；`nodes` 另含 `labelMarkdown`），并附上有名称的 `annotationCategories`；Ready for dev 的 frame 和 section 带 `devStatus`，`--flat` 会标出两者；测量列在 `handoffDiagnostics`。没有 auto-layout 的父层会带 `childSpacing: "derived"`：子层之间的间距是坐标差，不是 auto-layout 规则；`constraints` 仍可能固定它们。`handoffDiagnostics` 会列出已保存但尚未输出的语义、数量、示例及[覆盖限制](../wiki/semantics.md#known-handoff-gaps)。`_figlens.semantics` 只列出来源已保存的字段（缺少条目即表示未保存）；这些映射未与 Figma 比对，每个响应会在顶层的 `_figlens.semantics.verification` 中说明一次。grid auto layout 的 frame 会带行列数、间距和已保存的轨道尺寸（`_figlens.gridTracks`）。输出会保护源文件，完整写入后才替换目标。MCP 请求行上限为 1 MiB，完整响应上限为 8 MiB，超限会明确报错；CLI 仍可输出完整 JSON。
