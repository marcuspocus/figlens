<p align="center">
  <img src="https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/logo.png" alt="figlens" width="220">
</p>

<h1 align="center">讓 coding agent 直接讀 Figma 檔案。<br>不用開 Figma、不用登入、沒有 API 額度限制。</h1>

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
  <a href="#start">開始使用</a> · <a href="#demo">示範</a> · <a href="#why">為什麼選 figlens</a> · <a href="#bundle">Agent 拿到什麼</a> · <a href="#pi">Pi</a> · <a href="#limitations">限制</a>
</p>

**問題。** Agent 需要排版數值、文字與素材才能實作畫面。在遠端主機或 CI，這些設計上下文需要和程式碼放在一起，讓 agent 不必依賴執行中的桌面 App。

**解法。** 在 Figma 選擇 **File > Save local copy**，把 `design.fig` 放在程式碼旁邊。figlens 在本機解析，給你的 agent 排版、文字、樣式、variables、圖示、圖片，以及任一 frame 的參考渲染圖。一個原生執行檔，完全不對 Figma 發出網路呼叫。

<a id="start"></a>

## 開始使用

**還沒有設計檔？** [用小型公開範例檢查環境](examples/first-success/README.md)：驗證所選執行檔、找到一個 frame，再核對 bundle 內容。不需要模型。

把 `design.fig` 放進你的專案，再從下面**擇一**設定。每種方式都會讓 agent 取得 figlens 工具，以及同一套內建的規劃、實作與圖片處理指引。

| 方式 | 適用對象 | 指引來源 |
|---|---|---|
| [Plugin](#setup-plugin) | Claude Code, Codex, Antigravity, Cursor, Kimi Code | plugin 內附的 skill 與 server 的 `workflow` tool |
| [MCP](#setup-mcp) | Claude Code、Cursor、Codex、Gemini CLI 等 MCP client | server 的 `workflow` tool |
| [Pi](#setup-pi) | Pi 1.3.0+ | 套件內附的 skill |
| [OpenCode V2](#setup-opencode) | OpenCode V2 2.2.0 | 套件內附的 skill |
| [OMP](#setup-omp) | OMP 18.13.2 | 套件內附的 skill |
| [CLI](#setup-cli) | 只會執行 shell 指令的 agent、腳本與 CI，或你自己 | `figlens workflow <topic>`，或選用的 [skill](#setup-skill) |

**一般與進階。** 一般使用不需要任何設定：`figlens search design.fig "query"` 只讀取 `.fig`，不會建立任何東西。選用的本機索引只在你要求時建立（`figlens index prepare design.fig --scope 0:0`），之後跨重啟和宿主重複使用。請見[一般搜尋與選用的本機索引](docs/README-zh-TW.md#local-index)。 選用且屬實驗性功能：`--mode semantic` 或 `hybrid` 使用你自行設定的本機模型依語意排序，請見[選用的語意搜尋](docs/README-zh-TW.md#semantic-search)。

<a id="setup-plugin"></a>

### Plugin

一次安裝就把 figlens skill 和 MCP server 加進 agent（需要 Node.js 18+）。Server 固定在 plugin 對應的版本（`npx -y @runkids/figlens@<version> mcp`）；要換新版本就更新 plugin。

| 宿主 | 安裝 |
|---|---|
| Claude Code | `claude plugin marketplace add runkids/figlens`, `claude plugin install figlens@figlens` |
| Codex | `codex plugin marketplace add runkids/figlens`, `codex plugin add figlens@figlens` |
| Antigravity | `agy plugin install https://github.com/runkids/figlens/tree/main` |
| Cursor | `git clone --depth 1 https://github.com/runkids/figlens ~/.cursor/plugins/local/figlens`，再重新載入視窗 |
| Kimi Code | 在 session 中執行 `/plugins install https://github.com/runkids/figlens/tree/main`，再執行 `/reload` |

重新啟動宿主或開新的 session，確認 figlens tools 已列出。Plugin 安裝會複製 repository：約 7 MB，Codex 另外保留 Git 歷史（約 60 MB）。Antigravity 只載入 skill 的前 24 KB；完整指引由 server 的 `workflow` tool 提供。目前還不支援 Muse。[宿主對照表與驗證範圍](wiki/host-plugins.md)

<a id="setup-mcp"></a>

### MCP

**Claude Code**（Node.js 18+、macOS/Linux）：

```sh
claude mcp add --transport stdio --scope local figlens -- npx -y @runkids/figlens mcp
```

**Codex CLI**（Node.js 18+、macOS/Linux）：

```sh
codex mcp add figlens -- npx -y @runkids/figlens mcp
```

**任何 MCP client：**

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

沒有 Node.js 時，先安裝獨立執行檔（見 [CLI](#setup-cli)），再改用 `"command": "figlens", "args": ["mcp"]`。開啟新的 session，確認看得到 figlens 工具（Claude Code 用 `/mcp`）。不必另外安裝 skill。在 `mcp` 後面加上 `--root <dir>`，可以限制 figlens 能讀寫的資料夾。

<a id="mcp-args"></a>

#### MCP 啟動參數

將以下選項放在 client 的 `args` 中、`mcp` 之後。MCP 啟動選項只有這兩個，皆為選填且可重複。

| 選項 | 用途 |
|---|---|
| `--root DIR` | 允許讀取 `DIR` 與子資料夾內的設計檔，以及寫出匯出檔案。未指定時，figlens 不加上路徑限制，仍受作業系統權限約束；不是預設限制在目前專案，也不會掃描整台電腦。 |
| `--storage-root DIR` | 允許在 `DIR` 與子資料夾內明確指定自訂索引儲存位置。它不會選擇或改變預設儲存位置。 |

請使用已存在目錄的絕對路徑。每個選項和路徑都是獨立的陣列項目；多個資料夾要重複寫選項。`--root` 是路徑邊界，不是管理員權限；只限制 figlens MCP，不限制 agent 的其他工具。

兩個設計／輸出範圍與兩個 storage 允許範圍的範例：

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

沒有 `figlens mcp --storage` 這個選項。實際位置由 `index` tool 的 `storage` 參數指定（允許的 storage 範圍內的絕對路徑），或使用 CLI `figlens index prepare design.fig --storage /absolute/storage-a`。`prepare` 後會記住該 workspace 的位置，跨重啟、跨 host 重用，不必再傳 `storage` 或 `--storage-root`。未自訂位置時，使用每位使用者的預設目錄，不必設定 storage 旗標。

要避免在進 Git 的設定裡寫死這些路徑，或在多個 Git worktree 之間共用設計稿，請看 [進 Git 的設定中的 root](wiki/mcp.md#roots-in-a-checked-in-config)。

多個 storage root 只會擴大允許範圍：每次 `storage` 仍只選一個位置，不會自動分流、同步或備份索引。既有 workspace 資料庫不能只靠改指定另一個 root 搬家；位置衝突時 figlens 會拒絕。

<a id="setup-pi"></a>

### Pi

一個套件包含 CLI、MCP、viewer、`/figlens` 指令與 skill：

```sh
pi install npm:@runkids/figlens
```

[Pi 擴充套件說明](docs/README-zh-TW.md#pi)

<a id="setup-opencode"></a>

### OpenCode V2

在 OpenCode V2 安裝套件：

```sh
opencode plugin add @runkids/figlens
```

重新啟動 OpenCode，再執行 `/figlens help`。 [OpenCode V2](opencode/README.md)

<a id="setup-omp"></a>

### OMP

在 OMP 安裝同一套件：

```sh
omp plugin install @runkids/figlens
```

重新啟動 OMP，再執行 `/figlens help`。 [OMP](omp/README.md)

<a id="setup-cli"></a>

### CLI

透過 npm 執行，或安裝獨立執行檔：

```sh
npx @runkids/figlens --help                                                                # Node.js 18+，免安裝
curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh | sh  # macOS、Linux
brew install runkids/tap/figlens                                                           # Homebrew
irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 | iex      # Windows PowerShell
```

`npx` 會執行它解析到的套件版本所對應的執行檔。用安裝腳本裝的以 `figlens update` 更新；Homebrew 裝的以 `brew upgrade figlens` 更新。執行檔支援 macOS（Apple silicon、Intel）、Linux（arm64、x86_64）與 Windows（x86_64）。

使用 CLI 的 agent 可以執行 `figlens workflow build` 讀取指引（另有 `audit`、`images`、`search`、`plan`、`planning-details` 與需明確啟用的 `yolo`），內容永遠與執行檔版本一致；也可以改用 [agent skill](#setup-skill)。

[CLI 參考](docs/README-zh-TW.md#commands)

<a id="setup-skill"></a>

### Agent skill（選用）

skill 教 agent 的流程和內建指引相同：找到 frame、分段讀取 bundle、規劃範圍內的工作，再拿執行中的 UI 和設計比對。skill 只有說明，不含執行檔，要搭配 CLI 或 MCP 使用。用 MCP 或 Pi 則不需要另外安裝。

**推薦：[skillshare](https://github.com/runkids/skillshare)。** 安裝一次 skill，就能同步到你用的每個 agent：

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare sync
```

同一個工具也能幫這些 agent 設定 figlens 的 MCP 連線：

```sh
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync mcp --global
```

其他安裝 skill 的方式：

```sh
npx skills add runkids/figlens --skill figlens   # Skills CLI
```

也可以從這個 repo 或 release archive 把 `skills/figlens/` 複製到 agent 的 skills 目錄（Claude Code：`~/.claude/skills/`）。另外安裝的 skill 和執行檔分開更新；`figlens workflow` 則一定和執行檔同版。

接著請它做一個畫面：

```text
Build the task board from ./design.fig in this project.
Find the matching frame, use this project's components and image workflow,
then compare the running UI with the design and fix confirmed differences.
Report missing assets and any checks you could not run.
```

Agent 會找到 frame，將設計資料、素材與參考圖打包成 bundle，再用你的技術堆疊實作畫面。共用工作流程會引導字型與素材檢查；缺料會回報，切版仍使用已記錄的替代方案繼續。請它展示變更檔案與瀏覽器比對結果。

<a id="demo"></a>

## 示範：Pi 從 .fig 做出 React 畫面

[![Pi 從本機 .fig 規劃、做出 React 畫面，並與即時瀏覽器並排檢查](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/taskflow-demo.gif)](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)

GIF 以剪輯版影片的 3 倍速度循環播放；點擊可觀看完整 78 秒影片。

**[78 秒影片](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)。** Pi 執行 `/figlens plan`，從本機 `.fig` 讀取儀表板 frame，用 React 與 Tailwind CSS 實作，再檢查並修正執行中的 UI。這是剪輯過的縮時影片，不是基準測試，也不保證像素級一致。設計稿：[Eymen Eid 的 Task Flow](https://www.figma.com/community/file/1224268866120711793) · [錄製筆記](wiki/accuracy.md#taskflow-react-and-tailwind-demo)。

**[從 .fig 到可編輯 HTML/CSS](examples/task-list/README.md)：** 在受控的任務清單實驗中，6 次使用 figlens 的嘗試都保留了全部 19 處可見文字為真正的 DOM 文字，以及全部 4 個圖示。CLI 與 MCP 擷取也通過了停用 Docker 網路的驗證。案例頁提供設定步驟、量測與獨立圖片比對。

<a id="why"></a>

## 為什麼用 figlens，而不是官方 Figma MCP？

當設計需要以檔案形式傳遞時，用 **figlens**：遠端 agent、CI、離線工作、agent 之間的交接、不限次數的重複讀取。當 agent 必須不經匯出步驟就看到即時設計時，用**官方 Figma MCP**。

讀取 `.fig` 可離線進行。安裝與更新 figlens、`gs://` 輸入、你的模型供應商，以及專案的相依套件仍需要網路。

[完整比較](docs/README-zh-TW.md#why)

<a id="bundle"></a>

## Agent 拿到什麼

一個 frame，一個 zip。有可依循的結構、可使用的素材，以及可供比對的圖片。

[Bundle 內容與使用範例](docs/README-zh-TW.md#bundle) · [與 Figma 有多接近？](docs/README-zh-TW.md#accuracy)

## 詳細文件

- <a id="pi"></a>[Pi extension](docs/README-zh-TW.md#pi)
- <a id="viewer"></a>[不用 Figma 也能瀏覽設計](docs/README-zh-TW.md#viewer)
- <a id="mcp-and-skill"></a>[MCP 伺服器與 agent skill](docs/README-zh-TW.md#mcp-and-skill)
- <a id="commands"></a>[CLI 參考](docs/README-zh-TW.md#commands)

<a id="limitations"></a>

## 限制

- **快照，不是即時同步。** 設計變更時請匯出新的 `.fig`。figlens 無法從 Figma URL 下載設計。
- **格式未公開。** figlens 解碼檔案內嵌的 schema；新版 Figma 匯出可能需要更新 figlens。
- **字型與文字：** 不包含字型檔；被覆寫的文字可能保留 Figma 儲存的排版。請使用有授權的專案字型並比對結果。
- **支援範圍：** [渲染改進](https://github.com/runkids/figlens/issues/36)與[交接語意](https://github.com/runkids/figlens/issues/37)已開 issue 追蹤。另見[目前語意支援範圍](wiki/semantics.md)。
- **請檢查輸出。** 閱讀 bundle 的 `warnings`、`missing-assets.json`，以及 `context.md` 中未解析的 variables，再將實作與設計比對。
- 已知可見渲染限制會列在節點 `_figlens.renderDiagnostics`、有數量上限的 `inspect` 摘要及 bundle 與 `export` 的 `warnings`；`missing-assets.json` 只列素材失敗。沒有警告不代表像素一致。

## 幫助 figlens 成長

如果 figlens 幫你把設計變成程式碼，請[為這個 repo 加星](https://github.com/runkids/figlens)，讓更多人找到它。遇到讀不了的檔案？請[開 issue](https://github.com/runkids/figlens/issues)，附上指令與你預期的結果。

開發只在 Docker 中執行 Rust：`docker compose run --rm dev cargo test`。請見 [AGENTS.md](AGENTS.md) 與 [wiki](wiki/README.md)。

## 授權

[MIT](LICENSE)。figlens 是獨立專案，與 Figma 無隸屬、背書或贊助關係。`.fig` 格式未公開，可能會變動；請只使用你有權存取與分享的檔案和素材。
