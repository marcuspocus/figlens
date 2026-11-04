<p align="center">
  <img src="https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/logo.png" alt="figlens" width="220">
</p>

<h1 align="center">코딩 에이전트에 디자인 세부 정보를.<br>구현하고, 비교하고, 변경 사항을 추적하세요.</h1>

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
  <a href="#start">시작</a> · <a href="#demo">데모</a> · <a href="#why">이유</a> · <a href="#bundle">에이전트가 받는 것</a> · <a href="#pi">Pi</a> · <a href="#limitations">제한 사항</a> · <a href="https://figlens.runkids.cc/ko/docs/"><b>문서</b></a>
</p>

figlens는 저장한 Figma 파일을 코딩 에이전트가 활용할 수 있는 디자인 정보로 바꿉니다. 해석해야 할 스크린샷만 제공하는 것이 아닙니다.

- **원본 세부 정보로 구현.** 레이아웃, 텍스트, 스타일, 변수와 Dev Mode 주석, Ready for dev 상태를 읽고 벡터 아이콘은 SVG로, 이미지는 에셋으로 내보냅니다. `bundle`은 frame의 데이터, 에셋, 참조 렌더링을 모으고, `verify`는 누락 파일, 폰트 확인 사항, 핸드오프의 한계를 보고합니다.
- **실행 중인 UI 비교.** `compare-image`는 스크린샷을 figlens의 렌더링과 비교하고 차이가 있는 영역 아래의 레이어 이름을 알려줍니다. 차이와 렌더링 경고를 검토하세요. 픽셀 단위의 일치를 보장하지는 않습니다.
- **디자인 변경 추적.** `diff`는 저장된 리비전을 비교하고, `changes`와 `watch`는 추적 중인 로컬 범위의 업데이트를 보고하며, `freshness`는 저장된 사본이 온라인 파일보다 오래되었는지 알려 주고, `pull`은 본인의 브라우저 로그인으로 새 사본을 저장합니다. 디자인 변경을 반영하려면 Figma에서 새 `.fig`를 저장하세요. Figma와의 실시간 동기화가 아닙니다.

**기반은 오프라인.** Figma에서 **File > Save local copy**를 선택하고 `design.fig`를 코드 옆에 두세요. 그러면 Figma 링크만 있으면 되고, 노드 ID가 프레임을 정합니다. 저장한 복사본이 없으면 Figma에서 레이어를 복사(마우스 오른쪽 클릭 > Copy)한 뒤 `figlens clipboard screen.fig`를 실행하세요. 복사본에는 이미지나 벡터 윤곽선이 없습니다 ([방법](docs/README-ko.md#copy-a-selection)). 네이티브 바이너리 하나로 로컬에서 읽으며, 실행 중인 Figma 앱이나 로그인, Figma에 대한 네트워크 호출이 필요 없습니다. 선택 사항인 `freshness` 확인(`FIGMA_TOKEN` 사용)과 `pull`(브라우저 로그인 사용)만 Figma에 접속합니다.

<a id="start"></a>

## 시작하기

**디자인 파일이 아직 없나요?** [작은 공개 예제로 설정을 확인하세요](examples/first-success/README.md): 사용할 바이너리, frame 검색, bundle 내용을 검증합니다. 모델은 필요하지 않습니다.

프로젝트에 `design.fig`를 놓고 아래 설정 중 **하나**를 고르세요. 어느 것을 골라도 에이전트가 figlens 도구와, 계획·구현·이미지 준비를 위한 같은 내장 가이드를 사용할 수 있습니다.

| 설정 | 대상 | 가이드 출처 |
|---|---|---|
| [플러그인](#setup-plugin) | Claude Code, Codex, Antigravity, Cursor, Kimi Code | 플러그인에 포함된 skill과 서버의 `workflow` 도구 |
| [MCP](#setup-mcp) | Claude Code, Cursor, Codex, Gemini CLI 등 MCP 클라이언트 | 서버의 `workflow` 도구 |
| [Pi](#setup-pi) | Pi 1.3.0+ | skill이 포함된 패키지 |
| [OpenCode V2](#setup-opencode) | OpenCode V2 2.2.0 | 패키지에 포함된 skill |
| [OMP](#setup-omp) | OMP 18.13.2 | 패키지에 포함된 skill |
| [Herdr](herdr/README.md) | Herdr 0.1.12+에서 에이전트 옆에 두는 패널: 화면을 찾아 넘기고, 화면마다 진행 상황을 보드에서 추적 | 받는 에이전트의 설정 |
| [CLI](#setup-cli) | 셸 명령만 실행하는 에이전트, 스크립트와 CI, 또는 직접 사용 | `figlens workflow <topic>` 또는 선택 사항인 [skill](#setup-skill) |

**일반 vs 고급.** 일반 사용에는 설정이 필요 없습니다. `figlens search design.fig "query"`는 `.fig`를 읽을 뿐 아무것도 만들지 않습니다. 선택형 로컬 인덱스는 요청할 때만 만들어지며(`figlens index prepare design.fig --scope 0:0`) 이후 재시작과 호스트를 넘어 재사용됩니다. [일반 검색과 선택형 로컬 인덱스](docs/README-ko.md#local-index)를 참고하세요. 선택 사항·실험적: `--mode semantic` 또는 `hybrid`는 직접 준비한 로컬 모델로 의미 기준 순위를 매깁니다. [선택형 시맨틱 검색](docs/README-ko.md#semantic-search)을 참고하세요.

<a id="setup-plugin"></a>

### 플러그인

한 번 설치하면 figlens skill과 MCP 서버가 에이전트에 추가됩니다(Node.js 18+). 서버는 플러그인 릴리스에 고정됩니다(`npx -y @runkids/figlens@<version> mcp`). 새 버전으로 바꾸려면 플러그인을 업데이트하세요.

| 호스트 | 설치 |
|---|---|
| Claude Code | `claude plugin marketplace add runkids/figlens`, `claude plugin install figlens@figlens` |
| Codex | `codex plugin marketplace add runkids/figlens`, `codex plugin add figlens@figlens` |
| Antigravity | `agy plugin install https://github.com/runkids/figlens/tree/main` |
| Cursor | `git clone --depth 1 https://github.com/runkids/figlens ~/.cursor/plugins/local/figlens` 후 창을 다시 로드 |
| Kimi Code | 세션에서 `/plugins install https://github.com/runkids/figlens/tree/main` 실행 후 `/reload` |

[skillshare](https://github.com/runkids/skillshare)로 같은 plugin을 여러 호스트에 한 번에 설치할 수 있습니다: `skillshare plugin add https://github.com/runkids/figlens.git --target claude --target codex --target cursor -g`; 이후 업데이트는 `skillshare plugin update figlens -g`.

호스트를 다시 시작하거나 새 세션을 열고 figlens 도구가 표시되는지 확인하세요. 플러그인 설치는 저장소를 복사합니다(약 7 MB, Codex는 Git 기록도 보관해 약 60 MB). Antigravity는 skill의 처음 24 KB만 로드하며, 전체 지침은 서버의 `workflow` 도구가 제공합니다. Muse는 아직 지원하지 않습니다. [호스트 표와 검증 범위](wiki/host-plugins.md)

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

**모든 MCP 클라이언트:**

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

Node.js가 없다면 독립 바이너리([CLI](#setup-cli))를 설치하고 `"command": "figlens", "args": ["mcp"]`를 사용하세요. 새 세션을 열고 figlens 도구가 목록에 나타나는지 확인하세요(Claude Code에서는 `/mcp`). skill을 따로 설치할 필요는 없습니다. `mcp` 뒤에 `--root <dir>`를 붙이면 figlens가 읽고 쓸 수 있는 폴더를 제한할 수 있습니다.

<a id="mcp-args"></a>

#### MCP 시작 인수

클라이언트의 `args`에서 `mcp` 뒤에 지정하세요. MCP 시작 옵션은 이 두 가지뿐이며, 모두 선택 사항이고 반복할 수 있습니다.

| 옵션 | 의미 |
|---|---|
| `--root DIR` | `DIR` 및 하위 폴더에서 디자인 읽기와 내보내기를 허용합니다. 생략하면 figlens의 추가 경로 제한 없이 OS 권한이 적용됩니다. 현재 프로젝트로 제한되지 않으며 컴퓨터를 자동 검색하지도 않습니다. |
| `--storage-root DIR` | `DIR` 및 하위 폴더에서 명시적으로 요청한 사용자 지정 인덱스 저장소를 허용합니다. 기본 저장 위치를 선택하거나 변경하지 않습니다. |

기존 디렉터리의 절대 경로를 사용하세요. 각 옵션과 경로는 별도 배열 항목입니다. 여러 폴더에는 옵션을 반복하세요. `--root`는 경로 경계이지 관리자 권한이 아니며, figlens MCP만 제한하고 agent의 다른 도구는 제한하지 않습니다.

디자인/출력 루트 두 개와 허용된 저장소 루트 두 개의 예:

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

`figlens mcp --storage` 옵션은 없습니다. 실제 위치는 `index` tool의 `storage` 인수(허용된 저장소 루트 내 절대 경로) 또는 CLI `figlens index prepare design.fig --storage /absolute/storage-a`로 선택합니다. `prepare` 후에는 workspace 위치를 기억하므로 재시작이나 호스트 변경 후에도 `storage`나 `--storage-root` 없이 재사용합니다. 사용자 지정 위치가 없으면 사용자별 기본 디렉터리를 사용하며 저장소 플래그는 필요 없습니다.

커밋되는 설정에 이 경로를 고정하지 않거나 Git worktree 사이에서 디자인을 공유하려면 [커밋되는 설정의 root](wiki/mcp.md#roots-in-a-checked-in-config)를 참고하세요.

여러 저장소 루트는 허용 위치만 늘립니다. 각 `storage`는 여전히 한 위치를 선택하며 인덱스를 분산, 동기화 또는 백업하지 않습니다. 다른 루트를 지정하는 것만으로 기존 workspace 데이터베이스를 옮길 수 없으며, figlens는 충돌하는 위치를 거부합니다.

<a id="setup-pi"></a>

### Pi

패키지 하나에 CLI, MCP, viewer, `/figlens` 명령과 skill이 모두 들어 있습니다:

```sh
pi install npm:@runkids/figlens
```

[Pi 확장 가이드](docs/README-ko.md#pi)

<a id="setup-opencode"></a>

### OpenCode V2

OpenCode V2에서 패키지를 설치하세요:

```sh
opencode plugin add @runkids/figlens
```

호스트를 다시 시작한 뒤 `/figlens help`를 실행하세요. [OpenCode V2](opencode/README.md)

<a id="setup-omp"></a>

### OMP

OMP에서 같은 패키지를 설치하세요:

```sh
omp plugin install @runkids/figlens
```

호스트를 다시 시작한 뒤 `/figlens help`를 실행하세요. [OMP](omp/README.md)

<a id="setup-cli"></a>

### CLI

npm으로 실행하거나 독립 바이너리를 설치하세요:

```sh
npx @runkids/figlens --help                                                                # Node.js 18+, 설치 불필요
curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh | sh  # macOS, Linux
brew install runkids/tap/figlens                                                           # Homebrew
irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 | iex      # Windows PowerShell
```

`npx`는 해석한 패키지 버전의 바이너리를 실행합니다. 스크립트로 설치했다면 `figlens update`, Homebrew로 설치했다면 `brew upgrade figlens`로 업데이트하세요. 바이너리는 macOS(Apple silicon, Intel), Linux(arm64, x86_64), Windows(x86_64)를 지원합니다.

CLI를 사용하는 에이전트는 `figlens workflow build`(그 밖에 `audit`, `images`, `search`, `plan`, `planning-details`, 명시적으로 요청할 때만 쓰는 `yolo`)로 가이드를 읽습니다. 내용은 항상 바이너리 버전과 일치합니다. [skill](#setup-skill)에서 읽을 수도 있습니다.

[CLI 레퍼런스](docs/README-ko.md#commands)

<a id="setup-skill"></a>

### 에이전트 skill (선택 사항)

skill은 내장 가이드와 같은 흐름을 에이전트에게 알려 줍니다: frame을 찾고, bundle을 조금씩 읽고, 범위를 정해 계획하고, 실행 중인 UI를 디자인과 비교합니다. 지침만 담고 있으므로 CLI나 MCP와 함께 사용하세요. MCP와 Pi 설정에서는 필요하지 않습니다.

**추천: [skillshare](https://github.com/runkids/skillshare).** skill을 한 번 설치하면 사용하는 모든 에이전트에 동기화할 수 있습니다:

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare sync
```

같은 도구로 해당 에이전트들의 figlens MCP 연결도 설정할 수 있습니다:

```sh
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync mcp --global
```

skill을 설치하는 다른 방법:

```sh
npx skills add runkids/figlens --skill figlens   # Skills CLI
```

또는 이 저장소나 release 아카이브의 `skills/figlens/`를 에이전트의 skills 디렉터리(Claude Code: `~/.claude/skills/`)에 복사하세요. 설치한 skill은 바이너리와 별도로 업데이트됩니다. `figlens workflow`는 항상 바이너리와 일치합니다.

그런 다음 화면을 요청하세요.

```text
Build the task board from ./design.fig in this project.
Find the matching frame, use this project's components and image workflow,
then compare the running UI with the design and fix confirmed differences.
Report missing assets and any checks you could not run.
```

에이전트는 frame을 찾고 디자인 데이터, 에셋, 참조 이미지를 bundle로 묶은 뒤 프로젝트의 기술 스택으로 화면을 구현합니다. 공통 워크플로가 폰트와 에셋 확인을 안내하며, 부족한 항목은 보고하면서 기록된 대안으로 레이아웃 구현을 계속합니다. 변경된 파일과 브라우저 비교 결과를 보여 달라고 요청하세요.

<a id="demo"></a>

## 데모: Pi가 .fig로 React 화면 구현하기

[![Pi가 로컬 .fig로 계획하고, React 화면을 구현하고, 실시간 브라우저 옆에서 확인합니다](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/taskflow-demo.gif)](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)

GIF는 편집 영상보다 3배 빠르게 반복 재생됩니다. 클릭하면 전체 78초 영상을 볼 수 있습니다.

**[78초 영상](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4).** Pi가 `/figlens plan`을 실행해 로컬 `.fig`의 dashboard frame을 읽고, React와 Tailwind CSS로 구현한 뒤, 실행 중인 UI를 확인하고 수정합니다. 편집된 타임랩스이며 벤치마크나 pixel-perfect 보장이 아닙니다. 디자인: [Task Flow by Eymen Eid](https://www.figma.com/community/file/1224268866120711793) · [녹화 설명](wiki/accuracy.md#taskflow-react-and-tailwind-demo).

**[.fig에서 편집 가능한 HTML/CSS로](examples/task-list/README.md):** 조건을 통제한 작업 목록 실험에서 figlens 기반 시도 6회 모두 화면의 텍스트 19곳을 실제 DOM 텍스트로 유지하고 아이콘 4개도 보존했습니다. Docker 네트워크를 비활성화한 상태에서도 CLI와 MCP 추출에 성공했습니다. 설정 절차, 측정값, 독립 이미지 비교는 사례 페이지에서 확인할 수 있습니다.

<a id="why"></a>

## 공식 Figma MCP 대신 figlens를 쓰는 이유

디자인을 파일로 전달해야 할 때는 **figlens**를 사용하세요. 원격 에이전트, CI, 오프라인 작업, 에이전트 간 전달, 제한 없는 반복 읽기가 해당합니다. 내보내기 없이 에이전트가 실시간 디자인을 봐야 할 때는 **공식 Figma MCP**를 사용하세요.

`.fig`를 읽는 일은 오프라인입니다. figlens 설치와 업데이트, `gs://` 입력, 선택 사항인 `freshness` 확인과 `pull`, 모델 제공업체, 프로젝트 의존성에는 여전히 네트워크가 필요합니다.

[전체 비교](docs/README-ko.md#why)

<a id="bundle"></a>

## 에이전트가 받는 것

frame 하나, zip 하나. 구현할 구조, 사용할 에셋, 비교할 이미지가 담겨 있습니다.

[Bundle 내용과 사용 예시](docs/README-ko.md#bundle) · [Figma와 얼마나 가깝나요?](docs/README-ko.md#accuracy)

## 문서

- <a id="pi"></a>[Pi 확장](docs/README-ko.md#pi)
- <a id="viewer"></a>[Figma 없이 디자인 둘러보기](docs/README-ko.md#viewer)
- <a id="mcp-and-skill"></a>[MCP 서버와 에이전트 skill](docs/README-ko.md#mcp-and-skill)
- <a id="commands"></a>[CLI 레퍼런스](docs/README-ko.md#commands)

<a id="limitations"></a>

## 제한 사항

- **스냅샷이며 실시간 동기화가 아닙니다.** 디자인이 바뀌면 새 `.fig`를 내보내세요. figlens는 Figma URL에서 디자인을 다운로드할 수 없습니다.
- **문서화되지 않은 형식.** figlens는 파일에 내장된 스키마를 디코딩하므로 새 Figma 내보내기에는 figlens 업데이트가 필요할 수 있습니다.
- **폰트와 텍스트:** 폰트 파일은 포함되지 않으며, 재정의된 텍스트에 Figma의 저장된 레이아웃이 남을 수 있습니다. 사용 허가를 받은 프로젝트 폰트를 사용하고 결과를 비교하세요.
- **지원 범위:** [렌더링 개선](https://github.com/runkids/figlens/issues/36)과 [핸드오프 의미 정보](https://github.com/runkids/figlens/issues/37)는 이슈로 추적합니다. [현재 의미 정보 지원 범위](wiki/semantics.md)도 참고하세요.
- **출력을 확인하세요.** bundle의 `warnings`, `missing-assets.json`, `context.md`의 해결되지 않은 변수를 읽고, 구현 결과를 디자인과 비교하세요.
- 알려진 가시적 렌더링 제한은 노드의 `_figlens.renderDiagnostics`, 개수가 제한된 `inspect` 요약, bundle 및 `export`의 `warnings`에 표시됩니다. `missing-assets.json`은 에셋 실패만 기록합니다. 경고가 없어도 픽셀 일치를 보장하지 않습니다.

## figlens 응원하기

figlens가 디자인을 코드로 구현하는 데 도움이 됐다면 [별을 눌러](https://github.com/runkids/figlens) 다른 사람들도 찾을 수 있게 해주세요. 읽지 못하는 파일을 만났나요? 실행한 명령어와 기대한 결과를 담아 [이슈를 열어주세요](https://github.com/runkids/figlens/issues).

개발 시 Rust는 Docker에서만 실행합니다: `docker compose run --rm dev cargo test`. [AGENTS.md](AGENTS.md)와 [wiki](wiki/README.md)를 참고하세요.

## 라이선스

[MIT](LICENSE). figlens는 Figma와 제휴, 보증, 후원 관계가 없는 독립 프로젝트입니다. `.fig` 형식은 문서화되지 않았으며 바뀔 수 있습니다. 접근하고 공유할 권한이 있는 파일과 에셋만 사용하세요.
