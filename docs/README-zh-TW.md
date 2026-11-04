# figlens — 使用指南

[← README](../README-zh-TW.md) · [figlens.runkids.cc](https://figlens.runkids.cc/zh-TW/docs/)

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

- [獨立執行檔安裝](#install)
- [為什麼用 figlens，而不是官方 Figma MCP？](#why)
- [Agent 拿到什麼](#bundle)
- [Pi extension](#pi)
- [Herdr 面板](#herdr)
- [不用 Figma 也能瀏覽設計](#viewer)
- [MCP 伺服器與 agent skill](#mcp-and-skill)
- [CLI 參考](#commands)

<a id="install"></a>

## 獨立執行檔安裝

**沒有 Node.js？** 安裝獨立執行檔，並改用 `"command": "figlens", "args": ["mcp"]`：

| 平台 | 指令 |
|---|---|
| macOS / Linux | `curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh \| sh` |
| Homebrew | `brew install runkids/tap/figlens` |
| Windows (PowerShell) | `irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 \| iex` |

<a id="why"></a>

## 為什麼用 figlens，而不是官方 Figma MCP？

當設計需要以檔案形式傳遞時，用 **figlens**：遠端 agent、CI、離線工作、agent 之間的交接、不限次數的重複讀取。當 agent 必須不經匯出步驟就看到即時設計時，用**官方 Figma MCP**。

| | figlens | 官方 Figma MCP |
|---|---|---|
| 來源 | 匯出的 `.fig` 快照 | 即時的 Figma 檔案 |
| 需求 | 該檔案 | Figma 登入；桌面版伺服器需要 App |
| 速率限制 | 無，本機解析 | Figma 服務限制 |
| 遠端 agent / CI | 複製檔案，執行執行檔 | 需登入的遠端伺服器 |
| 設計變更 | 匯出新的 `.fig` | 自動 |
| 交接 | 分享 `.fig` 或 frame bundle | 分享 Figma 存取權 |
| 精確度 | 獨立的 parser，有[已知缺漏](../README-zh-TW.md#limitations) | 由 Figma 維護 |

讀取 `.fig` 可離線進行。安裝與更新 figlens、`gs://` 輸入、選用的 `freshness` 檢查與 `pull`、你的模型供應商，以及專案的相依套件仍需要網路。

<a id="bundle"></a>

## Agent 拿到什麼

一個 frame，一個 zip。有可依循的結構、可使用的素材，以及可供比對的圖片。

```text
frame.zip
├── context.md                從這裡開始：給你的 agent 的說明
├── design/
│   ├── nodes.json            排版、文字、樣式、Dev Mode 標註與展開後的 instance
│   ├── fonts.json            可見文字用到的字型與節點 ID
│   └── variables.json        綁定的 variables、collection、mode 與 alias
├── assets/                   圖片填色、SVG 與合成後的節點渲染圖
├── assets.json               將設計中的參照對應到素材檔案
├── references/screen.png     供比對的 frame 渲染圖
├── missing-assets.json       所有無法產生的項目
└── manifest.json             檔案角色、媒體類型、大小與 SHA-256 雜湊；加上 --hash-source 時含來源 .fig 的雜湊
```

只需要其中一項？CLI 可以直接做到：

```sh
npx -y @runkids/figlens find design.fig "login" --type FRAME     # 依名稱或文字找 frame id
npx -y @runkids/figlens inspect design.fig 12:345                 # 精簡的排版與文字摘要
npx -y @runkids/figlens nodes design.fig 12:345 > frame.json      # REST 格式的 JSON
npx -y @runkids/figlens export design.fig 12:345 icon.svg         # SVG、PNG、JPG、WebP 或 AVIF
npx -y @runkids/figlens bundle design.fig 12:345 frame.zip        # 上面的 zip
```

SVG 匯出會內嵌原始圖片填色，並將文字轉為 glyph 外框，因此不需要字型。透過 MCP 時，點陣圖匯出也會以內嵌圖片回傳，agent 不必開啟檔案就能看到設計。

<a id="copy-a-selection"></a>

### 沒有已存的 .fig？複製一個選取範圍

1. 在 Figma 選取畫面或圖層並複製：右鍵 > **Copy**，或 Cmd+C（Windows 與 Linux 用 Ctrl+C）。
2. 執行 `figlens clipboard screen.fig`，或請你的 agent 建置「我剛複製的那個畫面」。它會讀取剪貼簿，並寫出每個指令都能讀取的 `.fig`。
3. 像其他檔案一樣傳入 `screen.fig`：`figlens bundle screen.fig <node> frame.zip`。

把複製內容貼到文字欄位只會得到其中的文字；figlens 讀取的是複製內容的 HTML，其中包含圖層。macOS 已用真實複製內容驗證；Windows（PowerShell 5.1）與 Linux（`wl-paste` 或 `xclip`）尚未驗證。

| | **File > Save local copy** | 複製的選取範圍 |
|---|---|---|
| 範圍 | 整個檔案 | 你複製的圖層，加上它們用到的元件 |
| 圖片填色與向量輪廓 | 包含 | 缺少；export 會在缺少它們的地方發出警告 |
| Figma 連結能找到節點 | 能 | 能，限複製的圖層：ID 與來源檔案一致 |
| 適用於 | 根據放在程式碼旁邊的設計來建置 | 沒有已存的副本時，快速查看一個畫面 |

<a id="accuracy"></a>

### 與 Figma 有多接近？

以 Figma 自己的 JSON、PNG 與 SVG 匯出為基準，在 **826 個正式環境 frame** 與五個 Community 檔案上測量（2026-10-22）：

| 檢查項目 | 結果 |
|---|---|
| 節點結構、名稱、可見性、邊界與文字 | 826 個 frame 中有 771 個完全一致 |
| 原始圖片填色 | 2,608 / 2,608 位元組完全一致 |
| SVG 素材 | 5,014 / 5,197 在像素差異門檻內 |
| Community frame bundle | 306 / 306 沒有缺少素材 |

這是在這些檔案上的結果，不保證每個設計都如此。方法與門檻：[精確度報告](../wiki/accuracy.md)。

<a id="pi"></a>

## Pi extension

```sh
pi install npm:@runkids/figlens
```

安裝說明: [OpenCode V2](../opencode/README.md) · [OMP](../omp/README.md).

重新啟動 Pi 或執行 `/reload`，然後用一般的話請它做一個畫面。Pi 會找到 frame，透過自動註冊的 MCP 伺服器讀取，並用你的技術堆疊實作。不需要 node ID，不需要全域 CLI。

請它找文案或畫面時，會直接搜尋 `.fig`，不儲存任何東西。需要反覆搜尋時，請 Pi 為某個設計和範圍啟用索引：它會準備一次本機索引（不需要設定指令），完成前回報工作狀態，之後在重新啟動和其他 host 中繼續使用。重設該索引需要你明確要求。詢問某個 component、variable、style 或圖片用在哪裡時，會以同樣方式列出精確的 usages：直接讀 `.fig`，或在啟用索引後讀該索引。 請 Pi 追蹤某個設計的變更時，它會在需要時準備索引並 track 該範圍，回報上次檢查以來的變化；詢問兩個已儲存副本的差異時則用 `diff` 回答，不會啟動任何東西。 詳情：[本機索引](#local-index)。

- `/figlens plan <request>` 只產生計劃書，不修改應用程式。
- `/figlens yolo <request>` 規劃、實作、檢查，並清理自己的暫存規劃、暫存匯出與啟動的程序。
- `/figlens preview [description]` 開啟本機 viewer。
- `/figlens review` 核准 agent 提出的可重用元件對應。

![請 Pi 做一個畫面，讓它找到設計、用你的技術堆疊實作，並檢查結果](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/pi-workflow.svg)

完整流程、指令與限制：[pi/README.md](../pi/README.md)。

<a id="herdr"></a>

## Herdr 面板

在代理旁邊檢視設計的 [Herdr](https://herdr.dev) 面板：打開 `.fig`，用名稱、文字、節點 ID 或 Figma 連結找到畫面或圖層，看完整尺寸，並在顯示完整訊息的確認框之後，以 Build、Plan 或 YOLO 交給左側的代理。不需要安裝 figlens，遠端 Herdr 機器上的設計也在原處讀取。在 Herdr 裡請代理執行：`Read ~/.config/herdr-app/AGENTS.md, then add this layout: https://github.com/runkids/figlens/tree/main/herdr/figlens-layout`。詳細說明：[herdr/README.md](../herdr/README.md)。代理的 figlens MCP 伺服器在 Herdr 終端機裡執行時，面板會顯示每個代理正在讀取的畫面。

<a id="viewer"></a>

## 不用 Figma 也能瀏覽設計

[![在本機檢視 MCP Apps 設計，包含圖層樹、排版與元件屬性](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/preview-inspector.png)](https://github.com/runkids/figlens/blob/v0.10.1/assets/pi-demo.mp4)

```sh
npx -y @runkids/figlens preview design.fig                        # 本機 viewer，按 Ctrl+C 停止
npx -y @runkids/figlens preview design.fig 12:345 --out preview.html   # 可寄給任何人的離線 HTML 快照
```

可瀏覽頁面與圖層、搜尋、排序搜尋與所選項目的精確用處（usages）、量測，檢視排版、字體、variables 與 CSS，匯出任何圖層，複製實作 prompt，或下載所選項目的 bundle。排序搜尋可搭配選用的本機模型切換為語意或混合模式，只有按下 Enable index 時，檢視器才會為檔案建立索引。拖入你實作成果的截圖，即可疊在設計上比對。詳情：[預覽指南](../wiki/preview.md)。 Visual／Hybrid 可使用參考 PNG 或所選圖層，並以像素裁切。請明確啟用視覺索引；查詢圖片不會保存。

<a id="mcp-and-skill"></a>

## MCP 伺服器與 agent skill

**MCP。** 伺服器透過 stdio 在本機執行，提供 `workflow`、`tree`、`find`、`search`、`usages`、`diff`、`inspect`、`nodes`、`variables`、`bundle`、`verify`、`freshness`、`index`、`jobs`、`changes`、`watch`、`export`、`export_batch`、`compare_image` 與 `models`。`export_batch` 一次呼叫最多寫出 64 個檔案，回傳 manifest 而非內嵌圖片。請傳入絕對路徑。加上 `--root /path/to/designs`（可重複）可將所有檔案與輸出路徑限制在指定的資料夾。明確指定自訂索引位置時，需要 `--storage-root DIR`（可重複）；已記住的 workspace 位置不必再指定。參見[啟動參數與多個 root 的設定範例](../README-zh-TW.md#mcp-args)。點陣圖匯出會縮小到 `maxSize`（預設 1600 px），讓內嵌圖片維持小巧；SVG 只回傳 metadata。

agent 可透過 `workflow` 讀取內建的 `build`、`audit`、`images`、`search`、`plan`、`planning-details`，或明確要求時的 `yolo` 指引。它們與 portable skill 共用來源；MCP 使用者可自行選擇是否另裝 skill。實際工作仍由 agent 使用專案與瀏覽器工具，並遵守 host 權限。讀取指引不會執行工作流程；使用方式取決於客戶端支援與模型行為。

在 Windows 原生環境使用 Claude Code：請依照其 [stdio 設定說明](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server)，並為 `npx` 加上 `cmd /c` 包裝。

**Skill。** [figlens skill](../skills/figlens/SKILL.md) 教 agent 找出 frame、分小段讀取 bundle、規劃範圍明確的工作，並對照設計驗證執行中的 UI。它只安裝說明；仍需有可用的 CLI 或 MCP 伺服器。可用 [Skills CLI](https://github.com/vercel-labs/skills) 安裝，或將 `skills/figlens/` 複製到 agent 的 skills 目錄（Claude Code：`~/.claude/skills/`）：

```sh
npm install -g @runkids/figlens
npx skills add runkids/figlens --skill figlens
```

**Plugin。** Claude Code、Codex、Antigravity、Cursor 和 Kimi Code 可以把 skill 和 MCP server 當成一個 plugin 一起安裝；請見 [Plugin 設定](../README-zh-TW.md#setup-plugin)。

**skillshare。** 用 [skillshare](https://github.com/runkids/skillshare) 在同一處管理 Claude Code、Codex 與 Cursor 的 skill 與 MCP 連線：

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync && skillshare sync mcp --global
```

**安裝說明。** npm launcher 在首次使用時下載對應的原生執行檔，以發行版的 SHA-256 checksum 驗證後快取。用安裝腳本裝的以 `figlens update` 更新；Homebrew 裝的以 `brew upgrade figlens` 更新。執行檔涵蓋 macOS（Apple silicon、Intel）、Linux（arm64、x86_64、靜態 musl）與 Windows（x86_64）。每種安裝方式都會把[選用的語意搜尋](#semantic-search)所用的執行環境 `figlens-embed` 放在 `figlens` 旁邊；在你設定模型之前，它不會載入任何模型。

<a id="local-index"></a>

## 一般搜尋與選用的本機索引

|  | 一般（預設） | 進階：本機索引（選用） |
|---|---|---|
| 作用 | 每次呼叫都讀取 `.fig`，回答 `search`、`find`、`inspect` 和 `export`。 | 為選定範圍一次建立搜尋索引，之後 `search` 從索引作答，並跨來源統一排序。 |
| 寫入內容 | 不建立 workspace 或索引資料庫（`export` 只寫入你要求的檔案）。 | 使用者 state 目錄或所選 `--storage` 下的本機 workspace（SQLite 資料庫）。 |
| 適用情境 | 一次性提問、腳本、CI。 | 反覆搜尋同一檔案；把專案的多個檔案當作一個語料搜尋（`--workspace <dir>`）；查找元件、變數、樣式和圖片的精確使用位置。 |

一般的設計問題不會建立索引。你只需明確選擇啟用一次（「為這個設計建立索引」）；儲存位置由 figlens 決定，不需要設定 SQLite 或其他東西。

```sh
# 一般：無狀態，不建立任何東西
figlens search design.fig "checkout button"

# 進階，僅一次：選擇範圍（節點 id，整個檔案用 0:0）；建置在前景執行
figlens index prepare design.fig --scope 0:0

# 索引搜尋；若索引範圍不是整個檔案，加上 --node
figlens search --file design.fig "checkout button" --node 0:0

# 編輯或重新匯出 .fig 後：只重建位元組有變化的範圍
figlens index refresh --file design.fig

# 移動 .fig 後：保留 workspace，再 refresh（使用 prepare 輸出的 workspaceId）
figlens index relocate <sourceId> new/design.fig --id <workspaceId>
figlens index refresh --id <workspaceId>
```

MCP：`index` 的 prepare 會回傳一個 job；輪詢 `jobs` 直到 `succeeded`，再用回應中的 `workspaceId` 搜尋。

```json
{"tool": "index", "arguments": {"action": "prepare", "file": "/abs/path/design.fig", "scope": "0:0"}}
{"tool": "jobs", "arguments": {"action": "status", "jobId": "<jobId from the reply>"}}
{"tool": "search", "arguments": {"workspaceId": "<workspaceId from the reply>", "node": "0:0", "query": "checkout button"}}
```

生命週期：`refresh` 只重建 `.fig` 位元組有變化的範圍；`rebuild` 即使沒有變化也重建；`clear_cache` 只刪除建置可重新產生的資料，保留註冊表和已發布的索引；`reset` 刪除此 workspace 的資料庫（絕不刪除 `.fig` 或匯出），僅在你明確要求時執行。新增來源、擴大範圍、reset 和重新建立，各自都需要單獨的明確請求。

移動了 `.fig`？`index relocate` 保留 `workspaceId`、`sourceId` 和儲存位置；接著對受影響的範圍執行 `refresh`。沒有自動探索，也沒有以雜湊為基礎的自動合併，不需要 reset。

一個資料夾裡有多個設計？`index add <dir> --workspace DIR` 一次登記該目錄下最多 10 個 `.fig` 檔案，並略過指向目錄之外的連結；回覆列出 `added`、`existing`、`skipped` 和 `truncated`，重複執行不會新增任何內容，每個檔案仍需各自執行 `prepare --scope`。在資料夾內移動的檔案會成為新來源，除非使用 `relocate`。 將回覆中的 `nextCursor` 作為 `--cursor C`（MCP 為 `cursor`）傳入，即可繼續登記接下來的 10 個。分頁的搜尋與 library 游標有效一小時，過期後會回傳 `cursor_expired`，請重新搜尋。

資料：SQLite 資料庫保存已註冊的本機路徑、所選範圍的圖層名稱與有效文字、實例的元件屬性值、搜尋 postings、精確關係（包含、實例、變體、變數/樣式/圖片綁定、原型目標）及其指向的定義、雜湊、generation 和 job。原始 `.fig` 留在原處且保持不變；其原始位元組不會存入資料庫。 語意與視覺模型層會加入其嵌入的向量；你啟用的視覺索引即使沒有模型也會加入已索引圖層的 PNG 渲染圖；查詢圖片從不保存。

Pi、OpenCode 和 OMP 透過各自的 opt-in 入口使用同一個索引，workspace 可跨重啟和宿主重複使用。尚未實作：檔案事件（watch 以輪詢讀取檔案，且只在有 holder 執行時進行）與推送事件。

<a id="semantic-search"></a>

## 選用的語意搜尋

實驗性功能。`figlens search <file> <query> --mode semantic|hybrid` 以本機模型依語意為圖層排序。預設的 `--mode text` 不變，也不需要模型。

- **一次性設定。** 請自行下載 BAAI/bge-m3 的 `5617a9f61b028005a4858fdac845db406aefb181` 修訂版（MIT；`config.json`、`pytorch_model.bin`、`tokenizer.json`），figlens 從不下載。`figlens models configure bge-m3 --model DIR [--runtime FILE]` 以 SHA-256 驗證檔案，離線啟動一次 `figlens-embed` 執行環境（未給 `--runtime` 時在 figlens 執行檔旁尋找），並把設定記錄在設定目錄的 `models.json`。`models list|status` 可查看，`models remove bge-m3` 會忘記該設定但保留檔案。
- **視覺 profile。** `figlens models configure siglip-base --model DIR` 以同樣方式檢查 google/siglip-base-patch16-224 的修訂版 `7fd15f0689c79d79e38b1c2e2e2370a7bf2761ed`（Apache-2.0；`config.json`, `preprocessor_config.json`, `model.safetensors`），並登記為視覺搜尋使用的圖片模型。
- **圖片查詢（實驗性）。** `figlens search --workspace DIR --image shot.png` 依外觀排序 workspace 的單元；`--image-ref '{"sourceId":…,"snapshotId":…,"nodeId":…}'` 改用某個結果已儲存的渲染，`--crop x,y,w,h` 可裁切兩者。加上查詢文字即為 `hybrid`。視覺層由 `figlens index visual` 建立（搜尋不會建立，單一 `.fig` 搜尋會拒絕圖片）。沒有圖片 profile 或它失敗時，只依像素雜湊排序，`query.usedModes` 會註明。查詢圖片不會被儲存。排序規則仍在驗證中，請把結果當作線索。MCP：`search` 的 `image`（root 內的路徑）、`imageRef`、`crop`。
- **模式。** `semantic` 依每個圖層在名稱、文字、元件屬性或所在範圍中最相符的片段排序。`hybrid` 融合文字與語意兩種排名（Reciprocal Rank Fusion）。沒有設定時，`semantic` 以 `mode_unavailable` 失敗，`hybrid` 回傳文字結果並說明原因。`coverage.layers.semantic` 回報 `ready`、`partial` 或 `unavailable`；部分結果的頁面沒有 `nextCursor`。模型使用 CPU，Ctrl-C 停止目前請求。每個 MCP/viewer 程序在使用後可同時保留一個文字模型和一個影像模型；一次 Linux ARM 診斷中，文字模型單獨約佔 2.5 GiB，兩者峰值記憶體總和約 3.5 GiB，不含核心與索引 worker。這些是量測值，不是記憶體上限。
- **限制。** 嵌入圖層名稱、有效文字、實例的元件屬性值（無法精確解析的值會略過並計數）以及每個圖層所在頁面與容器的名稱。使用 `--workspace`/`--id` 搜尋時，讀取每個來源已儲存的模型層，不會嵌入設計文字；該層由 `figlens index embed` 建立（MCP 伺服器會在文字建置後排入佇列）。沒有該層的來源為 `pending`，頁面為部分結果。在此功能凍結的測試集上，模型未達到同義詞與用途查詢的品質門檻（top-1 錯誤率 0.25，門檻 0.20），請把結果當作線索。
- **MCP。** `models` 工具（`list`、`status`、`configure`、`remove`）以及 `search` 的 `mode`；`configure` 只執行 figlens 旁的 `figlens-embed`，模型目錄與檔案必須位於伺服器的 root 之內。取消呼叫會停止執行中的搜尋或設定。

<a id="commands"></a>

## CLI 參考

| 指令 | 輸出 |
|---|---|
| `tree <file> [node] [--depth N] [--json]` | 節點 id、REST 類型（與 `nodes` 相同）與名稱；縮排文字，或附上每個節點尺寸、隱藏狀態與 auto layout 模式的巢狀 JSON；頁面或 section 下的節點另附 `firstText`（第一段可見文字） |
| `tree <file> [node] --top-frames [--json]` | 只列出畫面，每行一個，可直接傳給 `--frames -`：頁面或其 section 下的 frame、component、component set 與 instance（略過 group 與內部頁面）；`--json` 附上 page 與 section id，以及每個畫面第一段可見文字（`firstText`），可用來區分畫布標籤與畫面 |
| `find <file> <query> [--type TYPE] [--node ID] [--limit N]` | 不分大小寫搜尋圖層名稱與文字；路徑、尺寸與預覽。預設 20 筆結果（1-200） |
| `search <file> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C] [--include-hidden] [--include-internal] [--mode text\|semantic\|hybrid\|visual] [--image PNG\|--image-ref JSON] [--crop x,y,w,h]` | 對圖層名稱與完整文字做排序搜尋，已套用 instance override、文字/布林屬性與 swap；回傳精確節點參照（包括可用於 `inspect` 的 `I…;…` 子圖層 id）、比對原因、UTF-16 範圍、涵蓋狀態與 cursor 分頁。預設 20 筆（1-200）；隱藏圖層與內部頁面需加對應 flag。以 `--file`/`--workspace`/`--id` 取代 `<file>` 時，讀取 workspace 已準備的索引（見 `index`） `--mode semantic\|hybrid` 需要已設定的模型，請見[選用的語意搜尋](#semantic-search)。 |
| `models list\|status\|configure <profile> --model DIR [--runtime FILE]\|remove <profile>` | 語意搜尋用的本機文字模型（`bge-m3`，實驗性）與圖片搜尋用的圖片模型（`siglip-base`，實驗性）；`configure` 驗證你自行下載的檔案，從不下載；`remove` 保留檔案。請見[選用的語意搜尋](#semantic-search)。 |
| `usages <file> <node\|resource> [--scope ID] [--relation R[,R]] [--out] [--snapshot S] [--include-hidden] [--limit N] [--cursor C]` | 在一個已儲存範圍內回傳圖層或資源（`VariableID:…`、`StyleKey:…`、`image:…`）的精確關係，包括巢狀 instance 和 swap：預設回傳元件、元件集、變數、樣式或圖片被使用的位置，`--out` 回傳圖層指向的目標。每筆結果保留精確參照、關係、狀態（`resolved`、`missing`、`unresolved` 等）、依據與證據；名稱從不視為同一身分。不回傳總數，coverage 說明完整或部分。預設 50 筆（1-200）。用 `--file`/`--workspace`/`--id` 代替 `<file>` 時讀取 workspace 已準備好的 `--scope` 索引。使用 `--out` 時，快取程式庫副本的使用也會給出其 `library` {kind, namespace, key}。 |
| `usages --file\|--workspace\|--id … --library <kind> --namespace <ns> --key <key> [--definition-source ID --definition-snapshot SNAP --definition-node NODE] [--source ID] [--scope ID] [--include-hidden] [--limit N] [--cursor C]` | 回傳使用某個程式庫元件、樣式或變數（`kind`）的圖層，依據各檔案快取副本的記錄，範圍是所有來源已準備好的 `--scope`（`--source` 限定為一個）。定義維持 `unresolved`（以三個 `--definition-*` 依精確參照選定時為 `selected`，`keyEvidence: unverified`）；coverage 列出每個來源的狀態與部分的原因。參數取自對使用它的圖層執行 `usages --out` 時的 `library` 欄位 |
| `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]` | 兩個已儲存版本之間的變更：每個有變更的頂層 frame 一行，依 facet（文字、版面、外觀、屬性、結構、綁定）計數；`--json` 另含新舊欄位值與 edge 變更。圖層只依完全相同的 id 與類型比對；同名或同位置的 frame 只標為 possible match，中途停止的一側，或唯一差異是無法讀取的欄位的圖層，回報 unknown，絕不當成刪除或變更。每頁 100 筆（1-500）有一側在上限處停止時，印出 `stopped early` 而不是 `no changes`；請用 `--node` 縮小範圍。 |
| `inspect <file> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat] [--full-text] [--colors]` | 精簡的展開後排版/文字摘要，最多 200 個節點。`--images` 會加上點陣圖尺寸與裁切範圍。`--flat` 改為每個節點一行：相對該節點的位置、尺寸、文字、字型、填色、描邊、圓角、auto-layout、對齊、fixed/fill 尺寸、陰影與模糊 `--full-text` 回傳每段文字的完整內容（最多 10,000 字元）；`--colors` 加入可見的純色填色（hex、不透明度、綁定變數），`--flat` 也適用；不含漸層、圖片或描邊。 JSON 輸出包含 `path`：從文件到該節點的祖先（`[{id, name}]`，與 `find` 相同），instance 子圖層也適用。 |
| `nodes <file> <node> [--compact [--subtrees]]` | REST nodes 回應中單一節點的物件（`document`、`components`、`componentSets`、`styles`）：展開後的 instance 與 override、auto-layout、文字樣式、填色、效果、綁定的 variables、元件屬性、渲染邊界. 預設 REST 不變。`--compact` 共用重複樣式；加上 `--subtrees` 共用重複子樹並保留各節點 ID 與邊界。見[無損格式契約](../wiki/discovery.md#compact-nodes)。 Batch: `--frames` / `--ids` + `--compact [--subtrees]`. |
| `nodes <file> --frames <list>`, `--frames -` (stdin), `--ids <node>...` | 一次解碼讀取多個節點：清單中每個 id 一行 JSON（帶 `nodeId` 的 `nodes` 輸出，或 `{nodeId, error}`） |
| `variables <file> [node]`<br>`variables <file.fig> --format dtcg --collection ID --mode ID --out FILE` | 預設的 REST 格式綁定變數／本機變數輸出維持不變。DTCG 2025.10 匯出須指定精確的 collection 與 mode ID，不可指定 node；僅支援顏色與無單位數值。請先建立輸出目錄；任何診斷都會使整次匯出失敗，輸出檔案維持不變。library collection 會從檔案中快取的副本匯出（`libraryCopy: true`）。指向其他 collection 的 alias 會匯出該 collection 的值並記錄在 `aliasOf`；該 collection 有多個 mode 時，用 `--alias-mode COLLECTION=MODE` 指定。 |
| `bundle <file> <node> <out.zip> [--states] [--hash-source]` | 單一 frame 的設計 bundle；輸出 `{nodeId, path, warnings}` |
| `bundle <file> --frames <list> --out-dir <dir> [--jobs N] [--states] [--hash-source]`, `--frames -` (stdin) | 一次解碼，平行產生多個 bundle |
| `verify <bundle> [--fonts <dir>]` | 依 manifest 檢查 bundle（`.zip` 或解開後的目錄）：已驗證、缺少與被改動的檔案、缺少的素材，需在專案中確認的字型，以及 `findings`：每項缺少、不支援或未驗證的 handoff 輸入，附節點、證據與下一步；在本機或 `--fonts <dir>`（專案資料夾，同時產生 `@font-face`）找到字型檔的字型為 `local_file`，不代表瀏覽器能載入 |
| `export <file> <node> <out> [--bounds layout] [--scale N] [--quality 1-100] [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID]]` | SVG、PNG、JPG、WebP 或 AVIF；不超過 4,194,304 像素的 PNG 自動進行無損最佳化，解碼後的 RGBA 不變，若未變小則保留原始編碼位元組，更大的 PNG 使用一般編碼；`--max-kb` 會降低有損品質直到符合大小（PNG 若仍超限則失敗，不會退回有損編碼）；`--bounds layout` 以 layout box 而非 render bounds 匯出；`--match-dir DIR`（僅 SVG，含子資料夾）列出繪製相同輪廓的 SVG（`matches`），以及最多 10 個附分數的相似形狀（`similar`）；`--hide ID[,ID]` 把 node 內的這些圖層當作隱藏、在相同範圍內繪製，與正常匯出比對即可看出 figlens 畫出的這個圖層改變了結果的哪些地方（沒有差異不代表未渲染的調整沒有影響） |
| `export <file> --frames <list> --out-dir <dir> [--format F] [--scale N] ...`, `--frames -` (stdin) | 一次解碼匯出多個節點（預設 `png`）：寫入 `<dir>/<nodeId>.<format>`，每個 id 一行 JSON（帶 `nodeId` 的 `export` 輸出，或 `{nodeId, error}`） |
| `export <file> --batch <items.json>`, `--batch -` (stdin) | 一次解碼匯出多個節點，每項各有自己的選項：1-64 個 `{node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}` 的 JSON 陣列（MCP `export_batch`）。輸出一份 manifest `{count, failed, items}`：每項為 `export` 輸出或 `{error}`（其餘照常匯出）；項目格式錯誤或 `out` 重複會在寫入任何檔案前失敗 |
| `preview <file> [node] [--out preview.html] [--port N] [--bind IP] [--idle-exit SECONDS]` | 絕不寫入設計的本機 viewer；按 Enable index 後可用語意/混合搜尋；`--out` 會寫出獨立的離線快照；結果與複製的 context 會標明頁面，live 時也標明已載入檔案的路徑、source 的 snapshot 與本機 revision（離線時未知，且絕不嵌入路徑）；context 也帶有參考 scope 的 coverage，未檢查時為 `complete: null`. Visual／Hybrid 可使用參考 PNG 或所選圖層，並以像素裁切。請明確啟用視覺索引；查詢圖片不會保存。 `--idle-exit SECONDS` 在沒有開著的頁面達到該秒數後停止。 |
| `compare <file> <nodes.json> <node>` | 與同一 frame 的 REST 匯出做差異比對 |
| `compare-image <file> <node> <screenshot.png>` | 將實作畫面的截圖與依其縮放渲染的 node 比對（`--scale`、`--limit`、`--out diff.png`、`--full-page` 用於比 frame 短的整頁截圖，`--ignore` 用於內容來自資料的圖層，`--threshold` 用於看出淺色邊框；`image: true` 標記圖片填充；MCP `compare_image`）。輸出 `mismatch`、`nonTextMismatch` 與差異最大的區域（node 像素座標），每個區域附上其下最深的圖層；`text: true` 表示區域位於文字上（多為字型替代，需逐一檢查） |
| `node <file> <node>` | 單一原始 `.fig` 節點 |
| `clipboard [copied.html] <out.fig>` | 用於沒有已存 `.fig` 的情況：在 Figma 選取圖層並複製（右鍵 > Copy，或 Cmd/Ctrl+C），再執行此指令，把剪貼簿轉成每個指令都能讀取的 `.fig`；貼到其他地方只會得到圖層的文字。讀取系統剪貼簿（macOS；Windows PowerShell 5.1 與 Linux `wl-paste`/`xclip` 未驗證）或已存的剪貼簿 HTML 檔。節點 ID 與來源檔案相同，並包含用到的元件。回報 `missingImages`、`missingComponents` 和 `layersWithoutOutlines`：複製內容沒有圖片位元組和向量輪廓，因此 export 和 bundle 會在缺少它們的地方發出警告。不會覆寫既有檔案。有已存的 `.fig` 時，改為把 Figma 連結當作節點傳入：可保留圖片和向量 |
| `freshness [<file.fig> [--url URL] \| --sources <mapping.json>]` | 在斷定設計缺少某個 frame 之前，先確認每份已存副本是否比線上 Figma 檔案舊：檢查最近的 `.figlens/sources.json` 中的設計（`{name, url, file}` 清單，`file` 相對於 repository 根目錄；`.figlens/` 以外的 `--sources` 對照檔則相對於它自己的資料夾）、單一檔案，或附上 URL 的單一檔案。比較副本 `meta.json` 中的 `exported_at` 與線上的 `last_touched_at`（以 `curl` 和 `FIGMA_TOKEN` 中的 personal access token 讀取，scope 為 `file_metadata:read`）；輸出 `stale`、`current` 或附原因的 `unknown`，缺少 token 或網路時也不會失敗。設定 token 後，在已對應且過期的副本中找不到節點或 `find` 沒有結果時，也會提示重新儲存本機副本（MCP `freshness`） |
| `pull [--only NAME]... [--stale] [--sources <mapping.json>] \| --login` | 根據最近的 `.figlens/sources.json`（或 `--sources`）重新整理已對應的儲存副本。未指定 `--only` 或 `--stale` 時不會下載任何內容，只列出每個設計的新舊狀態。`--only` 選擇設計，`--stale` 只抓取過期或缺少的副本（需要 `FIGMA_TOKEN`）。由專用的 Chrome profile 執行 **File > Save local copy**；下載內容必須能解碼，且 `exported_at` 不早於目前副本，才會以原子方式取代；檢查失敗則保留舊副本；失敗時會指出所在階段（`open`、`editor`、`menu`、`export`、`download`、`install`）。`--login` 開啟可見視窗以登入（工作階段過期時 pull 也會開啟）；在其中登入可能會讓你在另一台裝置上登出 Figma。需要 Node、Chrome 與 `playwright-core` (`npm i -g @runkids/figlens playwright-core`)；僅支援英文 Figma 介面。僅 CLI |
| `index prepare\|refresh\|rebuild\|clear\|status\|add\|remove\|relocate\|reset\|track\|untrack\|embed\|visual` | 使用者 figlens state 目錄中的本機 workspace，或位於 workspace 之後會記住的 `--storage DIR` 下。`prepare <file> --scope NODE` 為該範圍建立一次搜尋索引；之後 `search --file <file>`（或 `--workspace DIR`）不再解析檔案，跨 workspace 的所有來源統一排序作答。`refresh` 重建有變更的檔案，`rebuild` 重建全部或單一 `--source`/`--scope`，`clear` 刪除未使用的索引；新索引就緒前繼續使用舊索引。`reset` 只刪除 workspace 資料庫，不會刪除設計、匯出或記住的位置。`track --scope NODE` 從 baseline 建置開始記錄該範圍的變更；`untrack` 停止記錄並保留歷史。 `embed` 用已設定的模型嵌入已發布的範圍（使用 CPU 與記憶體）；僅在需要 workspace 語意搜尋時執行。 `visual --scope NODE` 為圖片搜尋渲染該已發布範圍的畫面與元件，設定視覺模型時一併嵌入；僅在需要 workspace 視覺搜尋時執行。 |
| `jobs status\|cancel <jobId>` | 查看或取消索引建置；`cancel` 在建置停止後才返回 |
| `changes --file <file>\|--id ID\|--workspace DIR [--source ID] [--scope NODE] [--consumer NAME\|--subscription ID\|--cursor C] [--revision R] [--limit N]` | 已追蹤範圍記錄的變更，從最舊開始：每次發布的建置一個修訂（baseline 或 changes），含涵蓋狀況及依變更類型與 facet 的計數；每個 changes 修訂都有 `impact` 摘要，說明變更波及的圖層（schema 7 之前為 null）；`--revision` 分頁列出該修訂變更的節點，再列出 impact 列出的圖層（`entry` 標明是哪一種）；`tracking` 指出哪些範圍需要 `index refresh` 才能記錄變更。唯讀 |
| `watch ack --file <file>\|--id ID\|--workspace DIR --consumer NAME\|--subscription ID --cursor C [--source ID] [--scope NODE]` | 把 consumer 的檢查點移到它以相同篩選條件處理完的最後一頁 `changes` 的 `nextCursor`；重複執行是安全的，而且只會前進；唯一例外是目前的 recovery cursor 會把落在刪除之後的檢查點退回，以重讀保留的 revision；在保留策略刪除它尚未經過的 revision 之前發出的 cursor 會得到 `event_gap`，不寫入任何內容 |
| `watch run --file <file>\|--id ID\|--workspace DIR --scope NODE [--source ID] [--request-id R]` | 在前景持有一個已追蹤 scope 的 lease：workspace 共用的 runner 在每次存檔時更新它（每 250 ms 輪詢，每 60 s 驗證內容），所以 `changes` 不需 `index refresh` 就會出現新的 revision。先輸出 lease，在 stdin 關閉、收到 Ctrl-C 或 SIGTERM，或 scope 被取消追蹤時輸出一行結束訊息。MCP `watch` 的 `start` 與 `stop` 在伺服器執行期間持有 lease；未續約的 lease 30 s 後結束，最後一個 lease 結束後 runner 會退出 |
| `watch status --file <file>\|--id ID\|--workspace DIR` | workspace 的 lease（`active`、`unauthorized`、`untracked`、`expired`、`ended`）與 runner（`running`、`stopping`、`stopped`、`unknown`，附放置方式）；從不寫入 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]` | 從最新修訂開始追蹤一個來源、scope 或確切圖層（`--node`），使用它自己的檢查點，用 `changes --subscription` 讀取、`watch ack --subscription` 確認；不存在或有歧義的圖層為 `target_missing` 或 `identity_uncertain`，絕不換成同名圖層；`--owner` 讓它只在該任務內暫時存在 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID --query TEXT [--type T[,T]] [--scope NODE]` | 儲存對一個來源 scope 的搜尋：立即評估其前 50 名；`changes --subscription` 只讀取已儲存的評估，從不計算 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --sources visible\|ID[,ID] --query TEXT\|--library KIND --key KEY [--namespace NS] [--definition-source ID --definition-snapshot SNAP --definition-node NODE]` | 對最多 10 個來源的固定集合儲存搜尋，或儲存程式庫項目的使用處及其 `definitionMatch`（`visible` 於此時解析，之後新增的來源不會加入）：`delta.changedSourceIds` 列出位元組有變的來源，`corpus_changed` 表示集合有變，`coverage.transitive` 為 `not_followed`，`delta.notFollowedSourceIds` 列出沒有使用處的已變更來源（其下游未重新檢查）；移除成員會停用它（先 unsubscribe，再重新 subscribe） |
| `watch evaluate --file <file>\|--id ID\|--workspace DIR --subscription ID` | 依 scope 目前的索引評估並儲存查詢訂閱，已評估則不做任何事；該 scope 正在 watch 時，每次發布後會自動進行 |
| `watch list --file <file>\|--id ID\|--workspace DIR [--consumer NAME] [--after ID] [--subscription ID]` | 分頁列出可見的訂閱及其狀態；`--subscription` 只讀取該訂閱（不可見時為空） |
| `watch unsubscribe --file <file>\|--id ID\|--workspace DIR --subscription ID\|--consumer C --owner NAME` | 刪除一個訂閱及其檢查點，或某個 owner 的暫時訂閱；tracking 與歷史保留 |
| `watch forget --file <file>\|--id ID\|--workspace DIR --consumer NAME [--source ID] [--scope NODE]` | 刪除 consumer 的原始檢查點；每個 consumer 最多保留 64 個訂閱與檢查點，整個 workspace 4096 個，不會自動淘汰 |
| `mcp` | 透過 stdio 的本機 MCP 伺服器 |
| `update` | 將執行檔換成最新發行版 |

需要相關元件 variants 時，使用 `inspect --states` 或 `bundle --states`（MCP 傳入 `states: true`，預設 false；不需修改設定）。證據保留來源、未知與截斷標記。Bundle 會新增 `design/states.json` 與 `context.md` 摘要。範圍及限制請見[狀態證據](../wiki/states.md)。

比較同一 component set 的兩個 variants：`inspect <file> <variantA> --compare-variant <variantB>`（MCP `compareVariant`）。`variantComparison` 列出原始欄位及值集合差異，保留來源與未知標記；子節點依唯一名稱、主元件、同數量同名群組內的堆疊順序配對（`pairedNodes`，附逐對差異；請檢查 `pairedBy`，`order` 證據最弱；其餘不配對），並標出綁定變數的名稱。詳見[比較限制](../wiki/states.md#compare-two-variants)。

要比較同一檔案中的任意兩個節點（空白與已填內容的畫面、手機與桌面斷點、共用標頭的兩個畫面），請使用 `inspect <file> <nodeA> --compare <nodeB>`（MCP `compare`）。`comparison` 格式相同，另有 `rootDelta`（尺寸與相對於父層的位置）；子節點依唯一名稱、主元件、同數量同名群組內的堆疊順序配對（`pairedBy` 標示方式），其餘不配對並標示 `reason: ambiguous-name`；instance 不會展開，節點也不能與自己的祖先比較。`--compare-variant` 仍要求同一個 component set。

node 引數可接受 `12:345`、`12-345`，或帶有 `?node-id=` 的 Figma URL。`<file>` 也可以是 `gs://bucket/object`，會用你的 `gcloud` 登入下載並快取。固定 object 交接、所需存取權限與更新限制，請參閱 [GCS 快照操作說明](../wiki/gcs.md)。每次 CLI 呼叫都會解碼檔案；MCP 只重用最後讀取且未變更的本機檔案，遠端輸入仍會重新解析。執行 `figlens --help` 查看完整 CLI 用法，或執行 `figlens inspect --help`（也可用 `-h`）只看該指令的項目。`index add gs://bucket/object` 會登錄該 generation 的固定副本並回報 `remote {uri, generation}`；只有 add 會執行 gcloud。

`inspect` 包含已記錄的文字尺寸調整、截斷、最大行數與裝飾。Dev Mode 標註出現在對應節點的 `annotations`（純文字 `label` 預覽、釘選的 `properties`、`categoryId`；`nodes` 另含 `labelMarkdown`），並附上有名稱的 `annotationCategories`；Ready for dev 的 frame 和 section 帶 `devStatus`，`--flat` 會標出兩者；測量列在 `handoffDiagnostics`。沒有 auto-layout 的父層會帶 `childSpacing: "derived"`：子層之間的間距是座標差，不是 auto-layout 規則；`constraints` 仍可能固定它們。`handoffDiagnostics` 會列出已儲存但尚未輸出的語意、數量、範例及[涵蓋限制](../wiki/semantics.md#known-handoff-gaps)。`_figlens.semantics` 只列出來源已儲存的欄位（缺少條目即表示未儲存）；這些對應未與 Figma 比對，每個回應會在最上層的 `_figlens.semantics.verification` 說明一次。grid auto layout 的 frame 會帶列欄數、間距和已儲存的軌道尺寸（`_figlens.gridTracks`）。輸出會保護來源，完整寫入後才替換目標。MCP 請求行上限為 1 MiB，完整回應上限為 8 MiB，超限會明確回報錯誤；CLI 仍可輸出完整 JSON。
