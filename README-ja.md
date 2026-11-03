<p align="center">
  <img src="https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/logo.png" alt="figlens" width="220">
</p>

<h1 align="center">coding agent にデザインの詳細を。<br>実装し、比較し、変更を追跡するために。</h1>

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
  <a href="#start">はじめに</a> · <a href="#demo">デモ</a> · <a href="#why">理由</a> · <a href="#bundle">エージェントが受け取るもの</a> · <a href="#pi">Pi</a> · <a href="#limitations">制限</a> · <a href="https://figlens.runkids.cc/ja/docs/"><b>ドキュメント</b></a>
</p>

figlens は保存した Figma ファイルを、coding agent が活用できるデザイン情報に変えます。解釈が必要なスクリーンショットだけではありません。

- **元の詳細情報から実装。** レイアウト、テキスト、スタイル、variables、Dev Mode の注釈と Ready for dev の状態を読み取り、ベクターアイコンを SVG、画像を素材として書き出します。`bundle` は frame のデータ、素材、参照レンダリングをまとめ、`verify` は欠けたファイル、フォントの確認事項、ハンドオフの制限を報告します。
- **動作中の UI と比較。** `compare-image` はスクリーンショットを figlens のレンダリングと比較し、差分領域の下にあるレイヤー名を返します。差分とレンダリングの警告を確認してください。ピクセル単位の一致を保証するものではありません。
- **デザインの変更を追跡。** `diff` は保存したリビジョンを比較し、`changes` と `watch` は追跡中のローカルスコープの更新を報告します。デザインの変更を取り込むには Figma から新しい `.fig` を保存します。Figma とのライブ同期ではありません。

**基盤はオフライン。** Figma で **File > Save local copy** を選び、`design.fig` をコードの隣に置きます。1 画面だけなら、Figma で選択範囲をコピーし、保存したクリップボードの HTML を `figlens clipboard` で `.fig` に変換することもできます。ネイティブバイナリ 1 つでローカルに読み取り、起動中の Figma アプリ、ログイン、Figma へのネットワーク呼び出しは不要です。

<a id="start"></a>

## はじめに

**デザインファイルがまだない場合：** [小さな公開サンプルでセットアップを確認](examples/first-success/README.md)できます。使用するバイナリ、frame の検索、生成した bundle の内容を検証します。モデルは不要です。

`design.fig` をプロジェクトに置き、次のセットアップから**1 つ**を選びます。どれを選んでも、エージェントは figlens のツールと、計画・実装・画像準備のための同じ組み込みガイドを使えます。

| セットアップ | 対象 | ガイドの入手元 |
|---|---|---|
| [プラグイン](#setup-plugin) | Claude Code, Codex, Antigravity, Cursor, Kimi Code | プラグイン同梱の skill とサーバーの `workflow` ツール |
| [MCP](#setup-mcp) | Claude Code、Cursor、Codex、Gemini CLI などの MCP クライアント | サーバーの `workflow` ツール |
| [Pi](#setup-pi) | Pi 1.3.0+ | パッケージに同梱の skill |
| [OpenCode V2](#setup-opencode) | OpenCode V2 2.2.0 | 同梱の skill |
| [OMP](#setup-omp) | OMP 18.13.2 | 同梱の skill |
| [CLI](#setup-cli) | シェルコマンドだけを実行するエージェント、スクリプト、CI、または自分で使う場合 | `figlens workflow <topic>`、または任意の [skill](#setup-skill) |

**通常と上級。** 通常の利用にセットアップは不要です。`figlens search design.fig "query"` は `.fig` を読むだけで何も作りません。任意のローカルインデックスは依頼したときだけ作成され（`figlens index prepare design.fig --scope 0:0`）、再起動やホストをまたいで再利用されます。[通常の検索と任意のローカルインデックス](docs/README-ja.md#local-index)を参照してください。 任意・実験的：`--mode semantic` または `hybrid` は、自分で用意したローカルモデルで意味による順位付けを行います。[任意のセマンティック検索](docs/README-ja.md#semantic-search)を参照。

<a id="setup-plugin"></a>

### プラグイン

1 回のインストールで figlens の skill と MCP サーバーをエージェントに追加します（Node.js 18+）。サーバーはプラグインのリリースに固定されます（`npx -y @runkids/figlens@<version> mcp`）。新しいバージョンにするにはプラグインを更新してください。

| ホスト | インストール |
|---|---|
| Claude Code | `claude plugin marketplace add runkids/figlens`, `claude plugin install figlens@figlens` |
| Codex | `codex plugin marketplace add runkids/figlens`, `codex plugin add figlens@figlens` |
| Antigravity | `agy plugin install https://github.com/runkids/figlens/tree/main` |
| Cursor | `git clone --depth 1 https://github.com/runkids/figlens ~/.cursor/plugins/local/figlens` の後、ウィンドウを再読み込み |
| Kimi Code | セッション内で `/plugins install https://github.com/runkids/figlens/tree/main`、その後 `/reload` |

[skillshare](https://github.com/runkids/skillshare) なら同じ plugin を複数のホストへ一度にインストールできます：`skillshare plugin add https://github.com/runkids/figlens.git --target claude --target codex --target cursor -g`。更新は `skillshare plugin update figlens -g`。

ホストを再起動するか新しいセッションを開始し、figlens のツールが表示されることを確認してください。プラグインはリポジトリをコピーします（約 7 MB。Codex は Git 履歴も保持し約 60 MB）。Antigravity は skill の先頭 24 KB しか読み込みません。完全なガイダンスはサーバーの `workflow` ツールが提供します。Muse はまだサポートしていません。[ホスト対応表と検証範囲](wiki/host-plugins.md)

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

**任意の MCP クライアント：**

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

Node.js がない場合は、スタンドアロン版のバイナリ（[CLI](#setup-cli)）をインストールし、`"command": "figlens", "args": ["mcp"]` を使います。新しいセッションを開き、figlens のツールが一覧に表示されることを確認します（Claude Code では `/mcp`）。skill を別途インストールする必要はありません。`mcp` の後に `--root <dir>` を付けると、figlens が読み書きできるフォルダーを制限できます。

<a id="mcp-args"></a>

#### MCP 起動引数

クライアントの `args` で `mcp` の後に指定します。MCP の起動オプションはこの 2 つだけで、どちらも省略・複数指定が可能です。

| オプション | 用途 |
|---|---|
| `--root DIR` | `DIR` とそのサブフォルダー内のデザイン読み取りとエクスポートを許可します。省略時は figlens 独自のパス制限はなく、OS の権限が適用されます。現在のプロジェクトに限定されるわけではなく、コンピューターを自動走査することもありません。 |
| `--storage-root DIR` | `DIR` とそのサブフォルダー内で明示的に要求されたカスタムのインデックス保存先を許可します。既定の保存先を選択・変更するものではありません。 |

既存ディレクトリの絶対パスを使ってください。オプションとパスはそれぞれ別の配列要素です。複数フォルダーにはオプションを繰り返します。`--root` はパスの境界であり管理者権限ではありません。figlens MCP のみを制限し、エージェントの他のツールは制限しません。

デザイン／出力用ルート 2 つと、保存先の許可ルート 2 つの例：

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

`figlens mcp --storage` オプションはありません。実際の保存先は `index` ツールの `storage` 引数（許可された保存先ルート内の絶対パス）、または CLI `figlens index prepare design.fig --storage /absolute/storage-a` で選択します。`prepare` 後は workspace の場所が記憶され、再起動やホストをまたいで `storage` や `--storage-root` を再指定せず再利用できます。未指定時はユーザーごとの既定ディレクトリを使うため、保存先フラグは不要です。

これらのパスをコミットする設定に書かずに済ませる方法や、Git worktree 間でデザインを共有する方法は、[コミットする設定での root](wiki/mcp.md#roots-in-a-checked-in-config) を参照してください。

保存先ルートを複数指定しても許可範囲が増えるだけで、各 `storage` が選ぶ場所は 1 つです。インデックスの分散・同期・バックアップは行いません。別のルートを指定するだけでは既存 workspace のデータベースは移動できず、場所が競合すると figlens は拒否します。

<a id="setup-pi"></a>

### Pi

1 つのパッケージに CLI、MCP、viewer、`/figlens` コマンド、skill が含まれます：

```sh
pi install npm:@runkids/figlens
```

[Pi 拡張機能ガイド](docs/README-ja.md#pi)

<a id="setup-opencode"></a>

### OpenCode V2

OpenCode V2 にパッケージをインストール：

```sh
opencode plugin add @runkids/figlens
```

ホストを再起動して `/figlens help` を実行します。 [OpenCode V2](opencode/README.md)

<a id="setup-omp"></a>

### OMP

OMP に同じパッケージをインストール：

```sh
omp plugin install @runkids/figlens
```

ホストを再起動して `/figlens help` を実行します。 [OMP](omp/README.md)

<a id="setup-cli"></a>

### CLI

npm 経由で実行するか、スタンドアロン版のバイナリをインストールします：

```sh
npx @runkids/figlens --help                                                                # Node.js 18+、インストール不要
curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh | sh  # macOS、Linux
brew install runkids/tap/figlens                                                           # Homebrew
irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 | iex      # Windows PowerShell
```

`npx` は解決したパッケージバージョンのバイナリを実行します。スクリプトでインストールした場合は `figlens update`、Homebrew の場合は `brew upgrade figlens` で更新します。バイナリは macOS（Apple silicon、Intel）、Linux（arm64、x86_64）、Windows（x86_64）に対応しています。

CLI を使うエージェントは `figlens workflow build`（ほかに `audit`、`images`、`search`、`plan`、`planning-details`、明示的に依頼された場合のみの `yolo`）でガイドを読みます。内容は常にバイナリのバージョンと一致します。[skill](#setup-skill) から読むこともできます。

[CLI リファレンス](docs/README-ja.md#commands)

<a id="setup-skill"></a>

### エージェント skill（任意）

skill は組み込みガイドと同じ流れをエージェントに教えます：frame を見つけ、bundle を少しずつ読み、範囲を絞って計画し、実行中の UI をデザインと比較します。内容は手順だけなので、CLI または MCP と組み合わせて使います。MCP と Pi のセットアップでは不要です。

**おすすめ：[skillshare](https://github.com/runkids/skillshare)。** skill を一度インストールすれば、使っている各エージェントに同期できます：

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare sync
```

同じツールで、それらのエージェントの figlens MCP 接続も設定できます：

```sh
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync mcp --global
```

skill のその他のインストール方法：

```sh
npx skills add runkids/figlens --skill figlens   # Skills CLI
```

または、このリポジトリか release アーカイブの `skills/figlens/` を、エージェントの skills ディレクトリ（Claude Code：`~/.claude/skills/`）にコピーします。インストールした skill はバイナリとは別に更新されます。`figlens workflow` は常にバイナリと一致します。

あとは画面を依頼します。

```text
Build the task board from ./design.fig in this project.
Find the matching frame, use this project's components and image workflow,
then compare the running UI with the design and fix confirmed differences.
Report missing assets and any checks you could not run.
```

エージェントは frame を見つけ、デザインデータ、素材、参照画像を bundle にまとめ、あなたのスタックで画面を実装します。共有ワークフローがフォントと素材の確認を案内し、不足は報告しつつ、記録した代替手段でレイアウト実装を続けます。変更したファイルとブラウザでの比較を見せるよう依頼してください。

<a id="demo"></a>

## デモ：Pi が .fig から React の画面を作る

[![Pi plans from a local .fig, builds a React screen and checks it beside a live browser](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/taskflow-demo.gif)](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)

GIF は編集済み動画をさらに 3 倍速でループ再生します。クリックすると 78 秒の動画全編を見られます。

**[78 秒の動画](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)。** Pi が `/figlens plan` を実行し、ローカルの `.fig` からダッシュボードの frame を読み、React と Tailwind CSS で作り、動作中の UI を確認して修正します。編集済みのタイムラプスであり、ベンチマークでもピクセル単位の一致の保証でもありません。デザイン：[Task Flow by Eymen Eid](https://www.figma.com/community/file/1224268866120711793) · [収録メモ](wiki/accuracy.md#taskflow-react-and-tailwind-demo)。

**[.fig から編集可能な HTML/CSS へ](examples/task-list/README.md)：** 条件を揃えたタスクリストの検証では、figlens を使った 6 回すべての試行で、表示される 19 箇所のテキストを実際の DOM テキストとして、4 つのアイコンも保持しました。Docker のネットワークを無効にした状態でも CLI と MCP の抽出に成功しました。設定手順、測定値、独立した画像比較は事例ページに掲載しています。

<a id="why"></a>

## 公式 Figma MCP ではなく figlens を使う理由

デザインをファイルとして持ち運びたいなら **figlens** を使います。リモートエージェント、CI、オフライン作業、エージェント間の引き継ぎ、無制限の繰り返し読み取りです。エクスポート手順なしでエージェントがライブのデザインを見る必要があるなら、**公式 Figma MCP** を使います。

`.fig` の読み取りはオフラインで動作します。figlens のインストールと更新、`gs://` 入力、モデルプロバイダー、プロジェクトの依存関係にはネットワークが必要です。

[詳しい比較](docs/README-ja.md#why)

<a id="bundle"></a>

## エージェントが受け取るもの

1 つの frame を 1 つの zip に。実装の土台となる構造、使う素材、照合用の画像です。

[Bundle の内容と使用例](docs/README-ja.md#bundle) · [Figma にどこまで近いか](docs/README-ja.md#accuracy)

## ドキュメント

- <a id="pi"></a>[Pi 拡張](docs/README-ja.md#pi)
- <a id="viewer"></a>[Figma なしでデザインを閲覧する](docs/README-ja.md#viewer)
- <a id="mcp-and-skill"></a>[MCP サーバーとエージェント skill](docs/README-ja.md#mcp-and-skill)
- <a id="commands"></a>[CLI リファレンス](docs/README-ja.md#commands)

<a id="limitations"></a>

## 制限

- **スナップショットであり、ライブ同期ではありません。** デザインが変わったら新しい `.fig` をエクスポートしてください。figlens は Figma の URL からデザインをダウンロードできません。
- **非公開のフォーマット。** figlens はファイルに埋め込まれたスキーマをデコードします。新しい Figma のエクスポートには figlens の更新が必要な場合があります。
- **フォントとテキスト：** フォントファイルは含まれません。オーバーライドされたテキストは Figma の保存済みレイアウトを保持することがあります。利用許諾のあるプロジェクトのフォントを使い、結果を比較してください。
- **対応範囲：** [レンダリングの改善](https://github.com/runkids/figlens/issues/36)と[ハンドオフのセマンティクス](https://github.com/runkids/figlens/issues/37)は issue で追跡しています。[現在のセマンティクス対応範囲](wiki/semantics.md)も参照してください。
- **出力を確認してください。** bundle の `warnings`、`missing-assets.json`、`context.md` 内の未解決の variables を読み、実装をデザインと比較してください。
- 既知の可視レンダリング制限はノードの `_figlens.renderDiagnostics`、件数制限のある `inspect` 要約、bundle と `export` の `warnings` に表示されます。`missing-assets.json` は素材の失敗のみを記録します。警告がなくてもピクセル一致は保証されません。

## figlens を育てる

figlens がデザインをコードにする助けになったなら、[リポジトリに star](https://github.com/runkids/figlens) を付けて、他の人が見つけやすくしてください。読み取れないファイルに出会いましたか？ コマンドと期待した結果を添えて [issue を作成](https://github.com/runkids/figlens/issues)してください。

開発では Rust を Docker 内でのみ実行します：`docker compose run --rm dev cargo test`。[AGENTS.md](AGENTS.md) と [wiki](wiki/README.md) を参照してください。

## ライセンス

[MIT](LICENSE)。figlens は独立したプロジェクトであり、Figma と提携しておらず、Figma の承認やスポンサーを受けていません。`.fig` フォーマットは非公開であり、変更される可能性があります。アクセスと共有の権限があるファイルと素材のみを使用してください。
