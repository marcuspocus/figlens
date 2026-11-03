# figlens — ユーザーガイド

[← README](../README-ja.md) · [figlens.runkids.cc](https://figlens.runkids.cc/ja/docs/)

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

- [スタンドアロン版のインストール](#install)
- [公式 Figma MCP ではなく figlens を使う理由](#why)
- [エージェントが受け取るもの](#bundle)
- [Pi 拡張](#pi)
- [Figma なしでデザインを閲覧する](#viewer)
- [MCP サーバーとエージェント skill](#mcp-and-skill)
- [CLI リファレンス](#commands)

<a id="install"></a>

## スタンドアロン版のインストール

**Node.js がない場合は？** 単体バイナリをインストールし、代わりに `"command": "figlens", "args": ["mcp"]` を使います。

| プラットフォーム | コマンド |
|---|---|
| macOS / Linux | `curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh \| sh` |
| Homebrew | `brew install runkids/tap/figlens` |
| Windows (PowerShell) | `irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 \| iex` |

<a id="why"></a>

## 公式 Figma MCP ではなく figlens を使う理由

デザインをファイルとして持ち運びたいなら **figlens** を使います。リモートエージェント、CI、オフライン作業、エージェント間の引き継ぎ、無制限の繰り返し読み取りです。エクスポート手順なしでエージェントがライブのデザインを見る必要があるなら、**公式 Figma MCP** を使います。

| | figlens | 公式 Figma MCP |
|---|---|---|
| ソース | エクスポートした `.fig` スナップショット | ライブの Figma ファイル |
| 必要なもの | ファイル | Figma へのサインイン。デスクトップサーバーはアプリが必要 |
| レート制限 | なし（ローカル解析） | Figma サービスの制限 |
| リモートエージェント / CI | ファイルをコピーしてバイナリを実行 | サインイン付きのリモートサーバー |
| デザインの変更 | 新しい `.fig` をエクスポート | 自動 |
| 引き継ぎ | `.fig` または frame の bundle を共有 | Figma のアクセス権を共有 |
| 忠実度 | 独自のパーサー、[既知のギャップ](../README-ja.md#limitations)あり | Figma が保守 |

`.fig` の読み取りはオフラインで動作します。figlens のインストールと更新、`gs://` 入力、モデルプロバイダー、プロジェクトの依存関係にはネットワークが必要です。

<a id="bundle"></a>

## エージェントが受け取るもの

1 つの frame を 1 つの zip に。実装の土台となる構造、使う素材、照合用の画像です。

```text
frame.zip
├── context.md                ここから開始：エージェント向けの指示
├── design/
│   ├── nodes.json            レイアウト、テキスト、スタイル、展開済みインスタンス
│   ├── fonts.json            表示テキストが使うフォントフェイスとノード ID
│   └── variables.json        バインドされた variables、コレクション、モード、エイリアス
├── assets/                   画像フィル、SVG、合成済みノードのレンダリング
├── assets.json               デザイン上の参照を素材ファイルに対応付ける
├── references/screen.png     比較用の frame のレンダリング
├── missing-assets.json       生成できなかったもの
└── manifest.json             ファイルの役割、メディアタイプ、サイズ、SHA-256 ハッシュ（--hash-source で元の .fig のハッシュも）
```

1 つだけ必要なら、CLI で直接取得できます。

```sh
npx -y @runkids/figlens find design.fig "login" --type FRAME     # 名前またはテキストで frame ID を検索
npx -y @runkids/figlens inspect design.fig 12:345                 # レイアウトとテキストの要約
npx -y @runkids/figlens nodes design.fig 12:345 > frame.json      # REST 形式の JSON
npx -y @runkids/figlens export design.fig 12:345 icon.svg         # SVG、PNG、JPG、WebP、AVIF
npx -y @runkids/figlens bundle design.fig 12:345 frame.zip        # 上記の zip
```

SVG エクスポートは元の画像フィルを埋め込み、テキストをグリフにアウトライン化するため、フォントは不要です。MCP 経由では、ラスターエクスポートもインライン画像として返るので、エージェントはファイルを開かずにデザインを確認できます。

<a id="accuracy"></a>

### Figma にどこまで近いか

Figma 自身の JSON、PNG、SVG エクスポートと比較して測定しました。**本番 frame 826 個**と Community ファイル 5 つが対象です（2026-10-22）。

| チェック | 結果 |
|---|---|
| ノード構造、名前、可視性、bounds、テキスト | 826 frame 中 771 で一致 |
| 元の画像フィル | 2,608 / 2,608 がバイト単位で一致 |
| SVG 素材 | 5,197 中 5,014 がピクセル差の閾値内 |
| Community frame の bundle | 306 / 306 で欠落素材なし |

これらのファイルでの結果であり、すべてのデザインを保証するものではありません。手法と閾値：[accuracy レポート](../wiki/accuracy.md)。

<a id="pi"></a>

## Pi 拡張

```sh
pi install npm:@runkids/figlens
```

インストールガイド: [OpenCode V2](../opencode/README.md) · [OMP](../omp/README.md).

Pi を再起動するか `/reload` を実行し、普通の言葉で画面を依頼します。Pi が frame を見つけ、自動登録された MCP サーバー経由で読み取り、あなたのスタックで実装します。ノード ID もグローバル CLI も不要です。

文言や画面を探すよう頼むと `.fig` を直接検索し、何も保存しません。繰り返し検索する場合は、デザインと範囲を指定してインデックスの有効化を Pi に頼みます。ローカルインデックスを一度だけ準備し（セットアップ用コマンドは不要）、完了までジョブの状態を伝え、再起動後や他のホストでも再利用します。インデックスのリセットにはあなたの明示的な指示が必要です。コンポーネント、変数、スタイル、画像の使用箇所を尋ねると、同じように正確な usages を返します。`.fig` を直接読むか、有効化後はそのインデックスを読みます。 デザインの変更を追うよう Pi に頼むと、必要ならインデックスを用意してそのスコープを track し、前回の確認以降の変更を報告します。保存した 2 つのコピーの違いを尋ねた場合は `diff` で答え、何も開始しません。 詳細：[ローカルインデックス](#local-index)。

- `/figlens plan <request>`は計画のみを作成し、アプリケーションを変更しません。
- `/figlens yolo <request>` は計画、実装、確認を行い、自身の一時的な計画、作業用エクスポートと起動したプロセスを片付けます。
- `/figlens preview [description]` はローカル viewer を開きます。
- `/figlens review` はエージェントが提案した再利用可能なコンポーネントのマッピングを承認します。

![Ask Pi for a screen, let it find the design, build with your stack, and check the result](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/pi-workflow.svg)

詳しい手順、コマンド、制限：[pi/README.md](../pi/README.md)。

<a id="viewer"></a>

## Figma なしでデザインを閲覧する

[![Inspect the MCP Apps design locally, with its layer tree, layout and component properties](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/preview-inspector.png)](https://github.com/runkids/figlens/blob/v0.10.1/assets/pi-demo.mp4)

```sh
npx -y @runkids/figlens preview design.fig                        # ローカル viewer、Ctrl+C で停止
npx -y @runkids/figlens preview design.fig 12:345 --out preview.html   # 誰にでも送れるオフライン HTML スナップショット
```

ページとレイヤーの閲覧、検索、ランク付き検索と選択範囲の正確な使用箇所（usages）の確認、計測、レイアウト・タイポグラフィ・variables・CSS の確認、任意のレイヤーのエクスポート、実装プロンプトのコピー、選択範囲の bundle のダウンロードができます。ランク付き検索は、任意のローカルモデルでセマンティックまたはハイブリッドに切り替えられます。ファイルのインデックス化は Enable index を押したときだけ行われます。作成した画面のスクリーンショットをドロップすると、デザインに重ねて比較できます。詳細：[preview ガイド](../wiki/preview.md)。 Visual／Hybrid は参照 PNG または選択レイヤーとピクセル単位の切り抜きに対応。視覚インデックスは明示的に有効化し、検索画像は保存しません。

<a id="mcp-and-skill"></a>

## MCP サーバーとエージェント skill

**MCP。** サーバーは stdio 経由でローカル実行され、`workflow`、`tree`、`find`、`search`、`usages`、`diff`、`inspect`、`nodes`、`variables`、`bundle`、`verify`、`index`、`jobs`、`changes`、`watch`、`export`、`export_batch`、`compare_image`、`models` を公開します。`export_batch` は 1 回の呼び出しで最大 64 ファイルを書き出し、インライン画像の代わりにマニフェストを返します。パスは絶対パスで渡します。`--root /path/to/designs`（複数指定可）を追加すると、すべてのファイルパスと出力パスを指定フォルダーに制限できます。カスタム保存先を明示するには `--storage-root DIR`（複数指定可）が必要です。記憶済みの workspace の場所はこのオプションなしで再利用できます。[起動引数と複数ルートの設定例](../README-ja.md#mcp-args)を参照してください。ラスターエクスポートは `maxSize`（デフォルト 1600 px）に縮小され、インライン画像は小さく保たれます。SVG はメタデータのみを返します。

エージェントは `workflow` で `build`、`audit`、`images`、`search`、`plan`、`planning-details`、または明示的に依頼された場合の `yolo` ガイドを取得できます。portable skill と同じソースを使うため、MCP 利用時の skill の別途インストールは任意です。実作業にはエージェント自身のプロジェクト・ブラウザツールとホストの権限を使います。ガイドを読むだけでは処理は実行されません。利用方法はクライアントの対応とモデルの動作に依存します。

Claude Code でネイティブ Windows を使う場合：その [stdio セットアップ手順](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server)と、`npx` 用の `cmd /c` ラッパーに従ってください。

**Skill。** [figlens skill](../skills/figlens/SKILL.md) は、エージェントに frame の検索、bundle の小分けでの読み取り、スコープを絞った作業の計画、動作中の UI とデザインの照合を教えます。インストールされるのは指示のみで、CLI または MCP サーバーが利用可能である必要があります。[Skills CLI](https://github.com/vercel-labs/skills) でインストールするか、`skills/figlens/` をエージェントの skills ディレクトリにコピーします（Claude Code：`~/.claude/skills/`）。

```sh
npm install -g @runkids/figlens
npx skills add runkids/figlens --skill figlens
```

**プラグイン。** Claude Code、Codex、Antigravity、Cursor、Kimi Code では skill と MCP サーバーを 1 つのプラグインとしてまとめてインストールできます。[プラグインの設定](../README-ja.md#setup-plugin)を参照してください。

**skillshare。** [skillshare](https://github.com/runkids/skillshare) を使うと、Claude Code、Codex、Cursor 全体の skill と MCP 接続を 1 か所で管理できます。

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync && skillshare sync mcp --global
```

**インストールの注意。** npm ランチャーは初回使用時に対応するネイティブバイナリをダウンロードし、リリースの SHA-256 チェックサムで検証してキャッシュします。スクリプトでインストールした場合は `figlens update`、Homebrew の場合は `brew upgrade figlens` で更新します。バイナリは macOS（Apple silicon、Intel）、Linux（arm64、x86_64、静的 musl）、Windows（x86_64）に対応しています。どのインストール方法でも、[任意のセマンティック検索](#semantic-search)のランタイム `figlens-embed` が `figlens` の隣に置かれます。モデルを設定するまでモデルは読み込みません。

<a id="local-index"></a>

## 通常の検索と任意のローカルインデックス

|  | 通常（デフォルト） | 上級：ローカルインデックス（任意） |
|---|---|---|
| できること | 呼び出しごとに `.fig` を読み、`search`、`find`、`inspect`、`export` に答えます。 | 選んだ範囲の検索インデックスを一度作り、以後の `search` はそこから、ソース横断でランク付けして答えます。 |
| 書き込まれるもの | workspace もインデックス用データベースも作りません（`export` は指定したファイルだけを書き込みます）。 | ユーザーごとの state ディレクトリ、または選んだ `--storage` の下にあるローカル workspace（SQLite データベース）。 |
| 使う場面 | 単発の質問、スクリプト、CI。 | 同じファイルを繰り返し検索する場合、プロジェクトの複数ファイルを一つのコーパスとして検索する場合（`--workspace <dir>`）、コンポーネント、変数、スタイル、画像の正確な使用箇所を調べたい場合。 |

通常のデザインに関する質問でインデックスが作られることはありません。明示的に一度だけ有効にします（「このデザインのインデックスを作成して」）。保存先は figlens が選び、SQLite などの設定は不要です。

```sh
# 通常：ステートレスで何も作らない
figlens search design.fig "checkout button"

# 上級、一度だけ：範囲（ノード ID、またはファイル全体なら 0:0）を指定。ビルドはフォアグラウンドで実行
figlens index prepare design.fig --scope 0:0

# インデックス検索。インデックス範囲がファイル全体でない場合は --node を付ける
figlens search --file design.fig "checkout button" --node 0:0

# .fig を編集・再エクスポートしたら、バイトが変わった範囲だけ再構築
figlens index refresh --file design.fig

# .fig を移動したら：workspace を維持して refresh（prepare が出力した workspaceId を使う）
figlens index relocate <sourceId> new/design.fig --id <workspaceId>
figlens index refresh --id <workspaceId>
```

MCP：`index` の prepare は job を返します。`succeeded` になるまで `jobs` を確認し、返信の `workspaceId` で検索します。

```json
{"tool": "index", "arguments": {"action": "prepare", "file": "/abs/path/design.fig", "scope": "0:0"}}
{"tool": "jobs", "arguments": {"action": "status", "jobId": "<jobId from the reply>"}}
{"tool": "search", "arguments": {"workspaceId": "<workspaceId from the reply>", "node": "0:0", "query": "checkout button"}}
```

ライフサイクル：`refresh` は `.fig` のバイトが変わった範囲だけを再構築し、`rebuild` は変更がなくても再構築します。`clear_cache` はビルドで再作成できるデータだけを削除し、レジストリと公開済みインデックスは残します。`reset` はこの workspace のデータベースを削除し（`.fig` やエクスポートは対象外）、明示的に求めたときだけ実行されます。新しいソース、範囲の拡大、reset、再作成は、それぞれ個別の明示的な依頼が必要です。

`.fig` を移動した場合：`index relocate` は `workspaceId`、`sourceId`、保存先を維持します。その後、影響を受けた範囲を `refresh` します。自動検出やハッシュによる自動統合はなく、reset は不要です。

1 つのフォルダーに複数のデザインがある場合：`index add <dir> --workspace DIR` はそのディレクトリ以下の `.fig` を一度に最大 10 個登録し、ディレクトリの外へ出るリンクはスキップします。応答には `added`、`existing`、`skipped`、`truncated` が含まれ、繰り返しても何も追加されません。各ファイルには引き続き個別の `prepare --scope` が必要です。フォルダー内で移動したファイルは、`relocate` を使わない限り新しいソースになります。 応答の `nextCursor` を `--cursor C`（MCP では `cursor`）に渡すと、次の 10 個を追加します。ページ送りの検索とライブラリのカーソルは 1 時間有効で、過ぎると `cursor_expired` を返すので、検索し直してください。

データ：SQLite データベースには、登録されたローカルパス、選択した範囲のレイヤー名と実効テキスト、インスタンスのコンポーネントプロパティ値、検索 postings、正確な関係（包含、インスタンス、バリアント、変数/スタイル/画像の参照、プロトタイプの遷移先）とその参照先の定義、ハッシュ、世代、job が入ります。元の `.fig` はその場所で変更されず、その生バイトはデータベースに保存されません。 セマンティック／ビジョンのモデルレイヤーは埋め込んだベクトルを、有効にしたビジュアルインデックスはモデルがなくてもインデックス済みレイヤーの PNG レンダリングを追加します。クエリ画像は保存しません。

Pi、OpenCode、OMP は、それぞれのオプトイン用エントリから同じインデックスを使い、workspace は再起動やホストをまたいで再利用されます。未実装：ファイルイベント（watch はファイルをポーリングし、holder の実行中だけ動きます）とプッシュイベント。

<a id="semantic-search"></a>

## 任意のセマンティック検索

実験的機能です。`figlens search <file> <query> --mode semantic|hybrid` はローカルモデルでレイヤーを意味で順位付けします。既定の `--mode text` は変わらず、モデルは不要です。

- **初回設定。** BAAI/bge-m3 のリビジョン `5617a9f61b028005a4858fdac845db406aefb181`（MIT。`config.json`、`pytorch_model.bin`、`tokenizer.json`）は自分でダウンロードしてください。figlens はダウンロードしません。`figlens models configure bge-m3 --model DIR [--runtime FILE]` は SHA-256 でファイルを検証し、`figlens-embed` ランタイムをオフラインで一度起動（`--runtime` がなければ figlens の実行ファイルの隣を探します）して、設定ディレクトリの `models.json` にプロファイルを記録します。`models list|status` で確認でき、`models remove bge-m3` はプロファイルを削除してファイルは残します。
- **ビジュアルプロファイル。** `figlens models configure siglip-base --model DIR` は google/siglip-base-patch16-224 のリビジョン `7fd15f0689c79d79e38b1c2e2e2370a7bf2761ed`（Apache-2.0、`config.json`, `preprocessor_config.json`, `model.safetensors`）を同じ方法で確認し、ビジュアル検索用の画像モデルとして登録します。
- **画像検索（実験的）。** `figlens search --workspace DIR --image shot.png` は workspace のユニットを見た目の近さで並べます。`--image-ref '{"sourceId":…,"snapshotId":…,"nodeId":…}'` は結果に保存されたレンダリングを使い、`--crop x,y,w,h` はどちらも切り抜きます。検索テキストを加えると `hybrid` になります。ビジュアルレイヤーは `figlens index visual` が作ります（検索では作られず、単一 `.fig` の検索は画像を拒否します）。画像プロファイルがない、または失敗した場合はピクセルハッシュだけで並べ、`query.usedModes` にそう表示されます。検索画像は保存されません。並べ方のルールは検証中なので、結果は手がかりとして扱ってください。MCP: `search` の `image`（root 内のパス）、`imageRef`、`crop`。
- **モード。** `semantic` は各レイヤーを名前、テキスト、コンポーネントプロパティ、スコープのうち最良のチャンクで順位付けします。`hybrid` はテキストとセマンティックの順位を統合します（Reciprocal Rank Fusion）。プロファイルがないと `semantic` は `mode_unavailable` で失敗し、`hybrid` はテキスト結果を返して理由を示します。`coverage.layers.semantic` は `ready`、`partial`、`unavailable` を報告し、部分的なページに `nextCursor` はありません。モデルは CPU で動作し、Ctrl-C は実行中のリクエストを停止します。MCP/ビューアーの各プロセスは使用後もテキスト用と画像用のモデルを一つずつメモリに保持できます。ある Linux ARM の測定ではテキスト単体が約 2.5 GiB、両方のピークメモリの合計が約 3.5 GiB で、コアと索引ワーカーは含みません。測定値でありメモリ上限ではありません。
- **制限。** 埋め込み対象はレイヤー名、有効なテキスト、インスタンスのコンポーネントプロパティ値（正確に解決できない値は除外して件数を報告）、各レイヤーのページ名とコンテナ名です。`--workspace`/`--id` の検索は各ソースの保存済みモデルレイヤーを読み、デザインのテキストを埋め込むことはありません。レイヤーは `figlens index embed` で作成します（MCP サーバーはテキストビルドの後にキューに入れます）。未作成のソースは `pending` で、ページは部分的になります。この機能の固定テストセットでは、同義語・用途クエリの品質基準を満たしませんでした（top-1 誤り率 0.25、基準 0.20）。結果は参考として扱ってください。
- **MCP。** `models` ツール（`list`、`status`、`configure`、`remove`）と `search` の `mode`。`configure` は figlens の隣の `figlens-embed` だけを実行し、モデルのディレクトリとファイルはサーバーの root 内になければなりません。呼び出しをキャンセルすると、実行中の検索やセットアップが止まります。

<a id="commands"></a>

## CLI リファレンス

| コマンド | 出力 |
|---|---|
| `tree <file> [node] [--depth N] [--json]` | ノード ID、REST タイプ（`nodes` と同じ）、名前。インデント付きテキスト、または各ノードのサイズ、非表示状態、オートレイアウトのモードを含むネストした JSON。ページやセクション直下のノードには `firstText`（最初の表示テキスト）も付く |
| `tree <file> [node] --top-frames [--json]` | 画面だけを 1 行に 1 つ、`--frames -` にそのまま渡せる形で出力。ページまたはそのセクション直下のフレーム、コンポーネント、コンポーネントセット、インスタンス（グループと内部ページは除外）。`--json` はページとセクションの ID と、キャンバスのラベルと画面を見分けるための各画面の最初の表示テキスト（`firstText`）を追加 |
| `find <file> <query> [--type TYPE] [--node ID] [--limit N]` | レイヤー名とテキストの大文字小文字を区別しない検索。パス、サイズ、プレビュー。デフォルト 20 件（1-200） |
| `search <file> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C] [--include-hidden] [--include-internal] [--mode text\|semantic\|hybrid\|visual] [--image PNG\|--image-ref JSON] [--crop x,y,w,h]` | インスタンスのオーバーライド、テキスト/ブーリアンプロパティ、スワップを適用したレイヤー名と全文のランク付き検索。正確なノード参照（`inspect` で使える `I…;…` サブレイヤー ID を含む）、一致理由、UTF-16 範囲、カバレッジ、カーソルによるページ送り。デフォルト 20 件（1-200）。非表示レイヤーと内部ページはそれぞれのフラグが必要。`<file>` の代わりに `--file`/`--workspace`/`--id` を使うと、workspace の準備済みインデックスを読みます（`index` を参照） `--mode semantic\|hybrid` には設定済みのモデルが必要です。[任意のセマンティック検索](#semantic-search)を参照。 |
| `models list\|status\|configure <profile> --model DIR [--runtime FILE]\|remove <profile>` | セマンティック検索用のローカルテキストモデル（`bge-m3`、実験的）と画像検索用の画像モデル（`siglip-base`、実験的）。`configure` は自分でダウンロードしたファイルを検証し、ダウンロードはしません。`remove` はファイルを残します。[任意のセマンティック検索](#semantic-search)を参照。 |
| `usages <file> <node\|resource> [--scope ID] [--relation R[,R]] [--out] [--snapshot S] [--include-hidden] [--limit N] [--cursor C]` | 保存済みの一つの範囲で、レイヤーまたはリソース（`VariableID:…`、`StyleKey:…`、`image:…`）の正確な関係を返します。ネストしたインスタンスとスワップも含みます。既定ではコンポーネント、セット、変数、スタイル、画像が使われている場所を、`--out` ではレイヤーが指す先を返します。各結果は正確な参照、関係、状態（`resolved`、`missing`、`unresolved` など）、根拠、証拠を持ち、名前は同一性として扱いません。総数は返さず、coverage が完全か部分的かを示します。既定 50 件（1-200）。`<file>` の代わりに `--file`/`--workspace`/`--id` を使うと、workspace で準備済みの `--scope` のインデックスを読みます。`--out` では、キャッシュされたライブラリコピーの使用箇所がその `library` {kind, namespace, key} も示します。 |
| `usages --file\|--workspace\|--id … --library <kind> --namespace <ns> --key <key> [--definition-source ID --definition-snapshot SNAP --definition-node NODE] [--source ID] [--scope ID] [--include-hidden] [--limit N] [--cursor C]` | ライブラリのコンポーネント、スタイル、変数（`kind`）を使うレイヤーを、各ファイルのキャッシュ済みコピーの記録に基づき、すべてのソースの準備済み `--scope` から返します（`--source` で一つに絞り込み）。定義は `unresolved` のままで（3 つの `--definition-*` で正確な参照を指定すると `selected`、`keyEvidence: unverified`）、coverage は各ソースの状態と部分的な理由を示します。指定値は、それを使うレイヤーに対する `usages --out` の `library` フィールドから取ります |
| `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]` | 保存された二つのリビジョン間の変更：変更のあるトップレベル frame ごとに一行、facet（テキスト、レイアウト、外観、プロパティ、構造、バインディング）ごとの件数。`--json` で新旧のフィールド値と edge の変更も返します。レイヤーは完全一致する id と種類でのみ照合し、同名・同位置の frame はラベル付きの possible match に留まり、途中で止まった側や、読めなかったフィールドだけが違うレイヤーは、削除や変更ではなく unknown と報告します。1 ページ 100 件（1-500）上限で止まった側があると `no changes` ではなく `stopped early` と表示します。`--node` で範囲を絞ってください。 |
| `inspect <file> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat] [--full-text] [--colors]` | 展開済みレイアウト/テキストのコンパクトな要約（最大 200 ノード）。`--images` でビットマップの寸法とクロップ範囲を追加。`--flat` は代わりに 1 ノード 1 行で出力（ノード基準の位置、サイズ、テキスト、フォント、塗り、線、角丸、auto-layout、整列、fixed/fill サイズ、シャドウとブラー） `--full-text` は各テキストを全文で返し（最大 10,000 文字）、`--colors` は表示中のソリッド塗り（hex、不透明度、バインドされた変数）を追加します（`--flat` でも有効）。グラデーション、画像、線は含みません。 |
| `nodes <file> <node> [--compact [--subtrees]]` | REST nodes レスポンスのノードごとのオブジェクト（`document`、`components`、`componentSets`、`styles`）：展開済みインスタンスとオーバーライド、auto-layout、テキストスタイル、フィル、エフェクト、バインドされた variables、コンポーネントプロパティ、レンダリング bounds. 既定の REST は変更なし。`--compact` は重複スタイルを共有し、`--subtrees` を追加すると各ノードの ID と bounds を保持して重複サブツリーを共有します。[可逆形式の契約](../wiki/discovery.md#compact-nodes)を参照。 Batch: `--frames` / `--ids` + `--compact [--subtrees]`. |
| `nodes <file> --frames <list>`, `--frames -` (stdin), `--ids <node>...` | 1 回のデコードで複数ノード：リストの id ごとに 1 行の JSON（`nodeId` 付きの `nodes` 出力、または `{nodeId, error}`） |
| `variables <file> [node]`<br>`variables <file.fig> --format dtcg --collection ID --mode ID --out FILE` | 既定の REST 形式のバインド済み／ローカル variables は変更なし。DTCG 2025.10 エクスポートは正確な collection／mode ID を指定し、node は指定不可。色と単位なしの数値のみ対応。出力先ディレクトリを先に作成してください。診断があれば全体が失敗し、出力ファイルは変更されません。ライブラリの collection はファイル内のキャッシュされたコピーから書き出します（`libraryCopy: true`）。別の collection への alias はその collection の値を書き出し、`aliasOf` に記録します。mode が複数ある場合は `--alias-mode COLLECTION=MODE` を指定します。 |
| `bundle <file> <node> <out.zip> [--states] [--hash-source]` | 1 つの frame の design bundle。`{nodeId, path, warnings}` を出力 |
| `bundle <file> --frames <list> --out-dir <dir> [--jobs N] [--states] [--hash-source]`, `--frames -` (stdin) | 1 回のデコードから複数の bundle を並列生成 |
| `verify <bundle> [--fonts <dir>]` | bundle（`.zip` または展開したディレクトリ）を manifest と照合：検証済み・欠落・変更されたファイル、欠落アセット、プロジェクトで確認すべきフォント、および `findings`：欠落・未対応・未検証の handoff 入力ごとのノード、根拠、次のアクション；このマシンまたは `--fonts <dir>`（プロジェクトのフォルダ。`@font-face` も出力）にフォントファイルがあれば `local_file`。ブラウザが読み込む証拠にはなりません |
| `export <file> <node> <out> [--bounds layout] [--scale N] [--quality 1-100] [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID]]` | SVG、PNG、JPG、WebP、AVIF。4,194,304 ピクセル以下の PNG はデコード後の RGBA を変えずに自動で可逆最適化し、小さくならなければ元のエンコード済みバイトを保持。それより大きい PNG は通常のエンコードを使用。`--max-kb` は収まるまで非可逆の品質を下げる（PNG はなお上限を超える場合エラーとなり、非可逆へのフォールバックはしない）。`--bounds layout` は render bounds ではなく layout box で書き出す。`--match-dir DIR`（SVG のみ、サブフォルダ含む）は同じアウトラインを描く SVG（`matches`）と、スコア付きの類似形状を最大 10 件（`similar`）列挙。`--hide ID[,ID]` は node 内のそれらのレイヤーを非表示として同じ範囲で描くので、通常の書き出しと比べれば、figlens が描くそのレイヤーが結果のどこを変えているかがわかる（差がなくても、描画されなかった調整が無害だとは限らない） |
| `export <file> --frames <list> --out-dir <dir> [--format F] [--scale N] ...`, `--frames -` (stdin) | 1 回のデコードで複数を書き出し（既定は `png`）：`<dir>/<nodeId>.<format>` に書き、id ごとに 1 行の JSON（`nodeId` 付きの `export` 出力、または `{nodeId, error}`） |
| `export <file> --batch <items.json>`, `--batch -` (stdin) | 1 回のデコードで複数を書き出し、項目ごとにオプションを指定：1-64 個の `{node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}` の JSON 配列（MCP `export_batch`）。マニフェスト `{count, failed, items}` を 1 つ出力し、各項目は `export` の出力または `{error}`（他は書き出される）。不正な項目や重複した `out` は何も書く前に失敗します |
| `preview <file> [node] [--out preview.html] [--port N] [--bind IP]` | デザインを書き換えないローカル viewer。Enable index 後はセマンティック/ハイブリッド検索も可能。`--out` でスタンドアロンのオフラインスナップショットを書き出し。結果とコピーした context はページと、live では読み込んだファイルのパス、source の snapshot とローカル revision を示す（オフラインでは不明で、パスは埋め込まない）。context には参照 scope の coverage も含まれ、未確認なら `complete: null`. Visual／Hybrid は参照 PNG または選択レイヤーとピクセル単位の切り抜きに対応。視覚インデックスは明示的に有効化し、検索画像は保存しません。 |
| `compare <file> <nodes.json> <node>` | 同じ frame の REST エクスポートとの差分 |
| `compare-image <file> <node> <screenshot.png>` | 実装した画面のスクリーンショットを、そのスケールで描画した node と比較（`--scale`、`--limit`、`--out diff.png`、frame より短いフルページキャプチャには `--full-page`、データ由来のコンテンツのレイヤーには `--ignore`、薄い枠線を見るには `--threshold`、`image: true` は画像塗りの領域、MCP `compare_image`）。`mismatch`、`nonTextMismatch` と、差分の大きい領域を node のピクセル座標で出力し、各領域の下にある最も深いレイヤーを示します。`text: true` はテキスト上の領域です（多くはフォント代替。各領域を確認してください） |
| `node <file> <node>` | `.fig` の生ノード 1 つ |
| `clipboard <copied.html> <out.fig>` | Figma でコピーした選択範囲（Cmd/Ctrl+C）をクリップボードの HTML として保存し、Save local copy なしで全コマンドが読める `.fig` に変換。ノード ID は元ファイルと同じで、使用中のコンポーネントも含む。`missingImages`、`missingComponents`、`layersWithoutOutlines` を報告: コピーには画像バイトもベクターのアウトラインも含まれないため、export と bundle はそれらなしで描いた箇所を警告する。既存ファイルは置き換えない |
| `index prepare\|refresh\|rebuild\|clear\|status\|add\|remove\|relocate\|reset\|track\|untrack\|embed\|visual` | ユーザーごとの figlens state ディレクトリ、または workspace が以後記憶する `--storage DIR` の下にあるローカル workspace。`prepare <file> --scope NODE` でその範囲の検索インデックスを一度だけ作成し、以後 `search --file <file>`（または `--workspace DIR`）は再解析せず、workspace の全ソースをまとめてランク付けして答えます。`refresh` は変更されたファイル、`rebuild` はすべて（または `--source`/`--scope` の一つ）を再作成し、`clear` は使われていないインデックスを削除します。新しいインデックスができるまで以前のものが使われます。`reset` は workspace データベースだけを削除し、デザイン、エクスポート、記憶された場所には触れません。`track --scope NODE` は baseline ビルドからそのスコープの変更を記録し、`untrack` は履歴を残したまま停止します。 `embed` は公開済みスコープを設定済みモデルで埋め込みます（CPU とメモリを使用）。workspace のセマンティック検索が必要なときだけ実行してください。 `visual --scope NODE` は公開済みスコープの画面とコンポーネントを画像検索用にレンダリングし、ビジョンプロファイル設定時は埋め込みも行います。workspace の画像検索が必要なときだけ実行してください。 |
| `jobs status\|cancel <jobId>` | インデックス作成の状態表示または取り消し。`cancel` は作成が止まってから戻ります |
| `changes --file <file>\|--id ID\|--workspace DIR [--source ID] [--scope NODE] [--consumer NAME\|--subscription ID\|--cursor C] [--revision R] [--limit N]` | 追跡中のスコープで記録された変更を古い順に返します。公開されたビルドごとに 1 リビジョン（baseline または changes）で、カバレッジと変更種別・ファセットごとの件数を含みます。changes リビジョンには変更が及ぶレイヤーの `impact` 要約があります（スキーマ 7 より前は null）。`--revision` でそのリビジョンの変更ノード、続いて impact が列挙するレイヤーをページングします（`entry` がどちらかを示します）。`tracking` は変更を記録するために `index refresh` が必要なスコープを示します。読み取りのみ |
| `watch ack --file <file>\|--id ID\|--workspace DIR --consumer NAME\|--subscription ID --cursor C [--source ID] [--scope NODE]` | 同じフィルターで処理した最後の `changes` ページの `nextCursor` まで、コンシューマーのチェックポイントを進めます。繰り返しても安全で、通常は前にしか進みません。ただし削除の後ろに残ったチェックポイントは、現在のリカバリーカーソルで戻され、保持されたリビジョンを読み直します。まだ通過していないリビジョンを保持期間の削除より前に発行したカーソルは `event_gap` になり、何も書き込みません |
| `watch run --file <file>\|--id ID\|--workspace DIR --scope NODE [--source ID] [--request-id R]` | 追跡中の scope の lease をフォアグラウンドで保持します。workspace 共有の runner が保存のたびに更新し（250 ms ごとにポーリング、60 s ごとに内容を確認）、`index refresh` なしで `changes` に新しい revision が出ます。lease を出力し、stdin が閉じたとき、Ctrl-C や SIGTERM、または scope の追跡解除で終了行を 1 行出力します。MCP の `watch` `start` と `stop` はサーバーの実行中 lease を保持します。更新されない lease は 30 s で終わり、最後の lease の後に runner は終了します |
| `watch status --file <file>\|--id ID\|--workspace DIR` | workspace の lease（`active`、`unauthorized`、`untracked`、`expired`、`ended`）と runner（`running`、`stopping`、`stopped`、`unknown`、配置付き）。書き込みはしません |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]` | ソース、スコープ、または特定のレイヤー（`--node`）を最新リビジョンから専用のチェックポイントで追跡します。`changes --subscription` で読み、`watch ack --subscription` で確定します。存在しないか曖昧なレイヤーは `target_missing` または `identity_uncertain` になり、同名のレイヤーで代用しません。`--owner` を付けるとそのタスク限りの一時的なものになります |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID --query TEXT [--type T[,T]] [--scope NODE]` | 1 つのソースの scope に対する検索を保存します。上位 50 件をすぐ評価し、`changes --subscription` は保存済みの評価を読むだけで計算しません |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --sources visible\|ID[,ID] --query TEXT\|--library KIND --key KEY [--namespace NS] [--definition-source ID --definition-snapshot SNAP --definition-node NODE]` | 最大 10 ソースの固定セットに対して検索、またはライブラリ項目の使用箇所とその `definitionMatch` を保存します（`visible` はこの時点で解決され、後から追加されたソースは含まれません）。`delta.changedSourceIds` はバイトが変わったソース、`corpus_changed` はセットの変化を示し、`coverage.transitive` は `not_followed` で、`delta.notFollowedSourceIds` は使用箇所を持たない変更ソース（依存先は再確認されていません）を示します。メンバーを削除すると無効になります（unsubscribe してから再度 subscribe） |
| `watch evaluate --file <file>\|--id ID\|--workspace DIR --subscription ID` | クエリサブスクリプションを scope の現在のインデックスで評価して保存します。評価済みなら何もしません。scope を watch 中なら公開のたびに自動で行います |
| `watch list --file <file>\|--id ID\|--workspace DIR [--consumer NAME] [--after ID] [--subscription ID]` | 見えるサブスクリプションと状態をページ単位で一覧します。`--subscription` はそのサブスクリプションだけを読みます（見えなければ空） |
| `watch unsubscribe --file <file>\|--id ID\|--workspace DIR --subscription ID\|--consumer C --owner NAME` | サブスクリプション 1 件とそのチェックポイント、または owner の一時的なものを削除します。トラッキングと履歴は残ります |
| `watch forget --file <file>\|--id ID\|--workspace DIR --consumer NAME [--source ID] [--scope NODE]` | consumer の素のチェックポイントを削除します。consumer ごとにサブスクリプションとチェックポイントは最大 64、ワークスペース全体で 4096 で、自動で追い出されることはありません |
| `mcp` | stdio 経由のローカル MCP サーバー |
| `update` | バイナリを最新リリースに置き換え |

関連するコンポーネント variants には `inspect --states` または `bundle --states` を使用します（MCP は `states: true`、既定は false、設定変更は不要）。証拠にはソース、不明点、省略のマーカーが残ります。Bundle には `design/states.json` と `context.md` の要約が追加されます。範囲と制限は[状態の証拠](../wiki/states.md)を参照してください。

同じ component set の 2 つの variants は `inspect <file> <variantA> --compare-variant <variantB>`（MCP `compareVariant`）で比較できます。`variantComparison` は保存フィールドと値集合の差、ソース ID、不明点を返します。子ノードは一意な名前、メインコンポーネント、同数の同名グループ内の重なり順の順に対応付け（`pairedNodes`、ペアごとの差分付き。`pairedBy` を確認し、`order` が最も弱い根拠。残りは未対応）、バインドされた variables は名前を示します。[比較の制限](../wiki/states.md#compare-two-variants)を参照してください。

1 つのファイル内の任意の 2 ノード（空の画面と入力済みの画面、モバイルとデスクトップのブレークポイント、ヘッダーを共有する 2 画面など）は `inspect <file> <nodeA> --compare <nodeB>`（MCP `compare`）で比較できます。`comparison` は同じ形式で、さらに `rootDelta`（サイズと親基準の位置）を返します。子は一意な名前、メインコンポーネント、同数の同名グループ内の重なり順の順に対応付けされ（`pairedBy` が方法を示します）、残りは `reason: ambiguous-name` 付きで未対応のままです。インスタンスは展開されず、ノードとその祖先は比較できません。`--compare-variant` は引き続き同じ component set を必要とします。

node 引数には `12:345`、`12-345`、または `?node-id=` 付きの Figma URL を指定できます。`<file>` には `gs://bucket/object` も指定でき、`gcloud` ログインでダウンロードしてキャッシュします。固定オブジェクトでの受け渡し、必要なアクセス権、更新の制限は [GCS スナップショットガイド](../wiki/gcs.md)を参照してください。CLI は呼び出しごとにファイルをデコードします。MCP が再利用するのは最後に読み取った変更のないローカルファイルだけです。リモート入力は毎回解析し直します。CLI の全使用法は `figlens --help` または `figlens inspect --help`（`-h` も可）で確認できます。`index add gs://bucket/object` はその世代のピン留めしたコピーを登録し、`remote {uri, generation}` を返します。gcloud を実行するのは add だけです。

`inspect` は記録済みのテキストのリサイズ、切り詰め、最大行数、装飾を含みます。auto-layout のない親には `childSpacing: "derived"` が付きます。子要素の間隔は座標の差で、auto-layout のルールではありません。`constraints` で固定されている場合もあります。`handoffDiagnostics` は保存済みでも未出力の意味情報を件数、例、[対象範囲の制限](../wiki/semantics.md#known-handoff-gaps)とともに示します。`_figlens.semantics` には保存済みのソースを持つフィールドだけが載ります（項目がなければ保存されていません）。これらの対応付けが Figma と比較されていないことは、レスポンスごとに一度、最上位の `_figlens.semantics.verification` に示されます。grid auto layout のフレームには行数・列数、間隔、保存済みのトラックサイズ（`_figlens.gridTracks`）が含まれます。出力は元ファイルを保護し、書き込み完了後に保存先を置き換えます。MCP のリクエスト行は 1 MiB、完全な応答は 8 MiB が上限で、超過時は明示的なエラーを返します。CLI では引き続き完全な JSON を出力できます。
