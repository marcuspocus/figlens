# figlens — 사용자 가이드

[← README](../README-ko.md) · [figlens.runkids.cc](https://figlens.runkids.cc/ko/docs/)

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

- [독립 바이너리 설치](#install)
- [공식 Figma MCP 대신 figlens를 쓰는 이유](#why)
- [에이전트가 받는 것](#bundle)
- [Pi 확장](#pi)
- [Herdr 패널](#herdr)
- [Figma 없이 디자인 둘러보기](#viewer)
- [MCP 서버와 에이전트 skill](#mcp-and-skill)
- [CLI 레퍼런스](#commands)

<a id="install"></a>

## 독립 바이너리 설치

**Node.js가 없나요?** 독립 바이너리를 설치하고 `"command": "figlens", "args": ["mcp"]`를 대신 사용하세요.

| 플랫폼 | 명령어 |
|---|---|
| macOS / Linux | `curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh \| sh` |
| Homebrew | `brew install runkids/tap/figlens` |
| Windows (PowerShell) | `irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 \| iex` |

<a id="why"></a>

## 공식 Figma MCP 대신 figlens를 쓰는 이유

디자인을 파일로 전달해야 할 때는 **figlens**를 사용하세요. 원격 에이전트, CI, 오프라인 작업, 에이전트 간 전달, 제한 없는 반복 읽기가 해당합니다. 내보내기 없이 에이전트가 실시간 디자인을 봐야 할 때는 **공식 Figma MCP**를 사용하세요.

| | figlens | 공식 Figma MCP |
|---|---|---|
| 소스 | 내보낸 `.fig` 스냅샷 | 실시간 Figma 파일 |
| 필요한 것 | 파일 | Figma 로그인. 데스크톱 서버는 앱 필요 |
| Rate limit | 없음, 로컬 파싱 | Figma 서비스 한도 |
| 원격 에이전트 / CI | 파일을 복사하고 바이너리 실행 | 로그인이 필요한 원격 서버 |
| 디자인 변경 | 새 `.fig` 내보내기 | 자동 |
| 전달 | `.fig` 또는 frame bundle 공유 | Figma 접근 권한 공유 |
| 충실도 | [알려진 차이](../README-ko.md#limitations)가 있는 독립 파서 | Figma가 유지 관리 |

`.fig`를 읽는 일은 오프라인입니다. figlens 설치와 업데이트, `gs://` 입력, 선택 사항인 `freshness` 확인과 `pull`, 모델 제공업체, 프로젝트 의존성에는 여전히 네트워크가 필요합니다.

<a id="bundle"></a>

## 에이전트가 받는 것

frame 하나, zip 하나. 구현할 구조, 사용할 에셋, 비교할 이미지가 담겨 있습니다.

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

일부만 필요한가요? CLI로 바로 가져올 수 있습니다.

```sh
npx -y @runkids/figlens find design.fig "login" --type FRAME     # 이름 또는 텍스트로 frame id 찾기
npx -y @runkids/figlens inspect design.fig 12:345                 # 간결한 레이아웃·텍스트 요약
npx -y @runkids/figlens nodes design.fig 12:345 > frame.json      # REST 형태 JSON
npx -y @runkids/figlens export design.fig 12:345 icon.svg         # SVG, PNG, JPG, WebP 또는 AVIF
npx -y @runkids/figlens bundle design.fig 12:345 frame.zip        # 위의 zip
```

SVG 내보내기는 원본 이미지 채우기를 포함하고 텍스트를 글리프 윤곽선으로 변환하므로 글꼴이 필요 없습니다. MCP에서는 래스터 내보내기도 인라인 이미지로 반환되어 에이전트가 파일을 열지 않고 디자인을 볼 수 있습니다.

<a id="copy-a-selection"></a>

### 저장한 .fig가 없다면? 선택 영역 복사

1. Figma에서 프레임이나 레이어를 선택해 복사합니다: 마우스 오른쪽 클릭 > **Copy** 또는 Cmd+C(Windows와 Linux는 Ctrl+C).
2. `figlens clipboard screen.fig`를 실행하거나, 에이전트에게 "방금 복사한 화면"을 만들어 달라고 요청하세요. 클립보드를 읽어 모든 명령이 읽는 `.fig`를 씁니다.
3. `screen.fig`를 다른 파일처럼 전달합니다: `figlens bundle screen.fig <node> frame.zip`.

복사본을 텍스트 필드에 붙여넣으면 텍스트만 나옵니다. figlens는 레이어가 들어 있는 복사본의 HTML을 읽습니다. macOS는 실제 복사본으로 확인했고, Windows(PowerShell 5.1)와 Linux(`wl-paste` 또는 `xclip`)는 아직 확인하지 않았습니다.

| | **File > Save local copy** | 복사한 선택 영역 |
|---|---|---|
| 범위 | 파일 전체 | 복사한 레이어와 그 레이어가 쓰는 컴포넌트 |
| 이미지 채우기와 벡터 윤곽선 | 포함됨 | 없음. export는 그것 없이 그린 곳을 경고함 |
| Figma 링크로 노드를 찾을 수 있음 | 예 | 예, 복사한 레이어는 가능. ID가 원본 파일과 일치함 |
| 적합한 경우 | 코드 옆에 두는 디자인으로 구현할 때 | 저장한 복사본이 없을 때 화면 하나를 빠르게 볼 때 |

<a id="accuracy"></a>

### Figma와 얼마나 가깝나요?

Figma 자체의 JSON, PNG, SVG 내보내기와 비교해 **실제 서비스 frame 826개**와 Community 파일 5개에서 측정했습니다(2026-10-22).

| 검증 항목 | 결과 |
|---|---|
| 노드 구조, 이름, 표시 여부, 경계, 텍스트 | 826개 중 771개 frame에서 동일 |
| 원본 이미지 채우기 | 2,608 / 2,608개가 바이트 단위로 동일 |
| SVG 에셋 | 5,014 / 5,197개가 픽셀 차이 임계값 이내 |
| Community frame bundle | 306 / 306개에서 누락 에셋 없음 |

해당 파일에 대한 결과이며 모든 디자인에 대한 보장은 아닙니다. 방법과 임계값: [정확도 보고서](../wiki/accuracy.md).

<a id="pi"></a>

## Pi 확장

```sh
pi install npm:@runkids/figlens
```

설치 안내: [OpenCode V2](../opencode/README.md) · [OMP](../omp/README.md).

Pi를 다시 시작하거나 `/reload`를 실행한 뒤 평소 말로 화면을 요청하세요. Pi는 frame을 찾고, 자동 등록된 MCP 서버로 읽고, 프로젝트의 기술 스택으로 구현합니다. 노드 ID도, 전역 CLI도 필요 없습니다.

문구나 화면을 찾아 달라고 하면 `.fig`를 직접 검색하며 아무것도 저장하지 않습니다. 반복 검색이 필요하면 디자인과 범위를 지정해 인덱스를 활성화해 달라고 Pi에 요청하세요. 로컬 인덱스를 한 번 준비하고(설정 명령 없음), 준비될 때까지 작업 상태를 알려 주며, 재시작 후와 다른 호스트에서도 재사용합니다. 인덱스를 초기화하려면 명시적으로 요청해야 합니다. 컴포넌트, 변수, 스타일, 이미지가 어디에 쓰이는지 물으면 같은 방식으로 정확한 usages를 알려 줍니다. `.fig`를 직접 읽거나, 인덱스를 활성화한 뒤에는 그 인덱스를 읽습니다. Pi에게 디자인의 변경을 따라가 달라고 하면, 필요할 때 인덱스를 준비해 그 범위를 track하고 마지막 확인 이후 바뀐 점을 알려 줍니다. 저장한 두 사본의 차이를 물으면 `diff`로 답하며 아무것도 시작하지 않습니다. 자세히: [로컬 인덱스](#local-index).

- `/figlens plan <request>`는 계획만 작성하며 애플리케이션을 수정하지 않습니다.
- `/figlens yolo <request>`는 계획, 구현, 확인을 하고 자신이 만든 임시 계획, 임시 내보내기 파일과 시작한 프로세스를 정리합니다.
- `/figlens preview [description]`는 로컬 viewer를 엽니다.
- `/figlens review`는 에이전트가 제안한 재사용 컴포넌트 매핑을 승인합니다.

![Pi에게 화면을 요청하면 디자인을 찾고, 기술 스택으로 구현하고, 결과를 확인합니다](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/pi-workflow.svg)

전체 안내, 명령어, 제한: [pi/README.md](../pi/README.md).

<a id="herdr"></a>

## Herdr 패널

에이전트 옆에서 디자인을 다루는 [Herdr](https://herdr.dev) 패널입니다. `.fig`를 열고 이름, 텍스트, 노드 ID, Figma 링크로 화면이나 레이어를 찾아 전체 크기로 본 뒤, 보낼 메시지를 그대로 보여 주는 확인 대화상자를 거쳐 왼쪽 에이전트에게 Build, Plan, YOLO로 넘깁니다. figlens 설치가 필요 없고, 원격 Herdr 머신의 디자인도 그 자리에서 읽습니다. Herdr 안의 에이전트에게 이렇게 요청하세요: `Read ~/.config/herdr-app/AGENTS.md, then add this layout: https://github.com/runkids/figlens/tree/main/herdr/figlens-layout`. 자세한 내용: [herdr/README.md](../herdr/README.md). 에이전트가 Herdr 터미널에서 figlens(MCP 또는 CLI)를 쓰면, 패널에 각 에이전트가 읽고 있는 화면이 표시되고 보드에서 화면마다 추적합니다: 작업 중, compare_image로 검증됨, 구현 후 디자인 변경.

<a id="viewer"></a>

## Figma 없이 디자인 둘러보기

[![레이어 트리, 레이아웃, 컴포넌트 속성과 함께 MCP Apps 디자인을 로컬에서 확인하는 모습](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/preview-inspector.png)](https://github.com/runkids/figlens/blob/v0.10.1/assets/pi-demo.mp4)

```sh
npx -y @runkids/figlens preview design.fig                        # 로컬 viewer, Ctrl+C로 종료
npx -y @runkids/figlens preview design.fig 12:345 --out preview.html   # 누구에게나 보낼 수 있는 오프라인 HTML 스냅샷
```

페이지와 레이어를 탐색하고, 검색하고, 순위 검색과 선택 항목의 정확한 사용처(usages)를 확인하고, 거리를 측정하고, 레이아웃, 타이포그래피, 변수, CSS를 확인하고, 모든 레이어를 내보내고, 구현 프롬프트를 복사하거나 선택 영역의 bundle을 다운로드할 수 있습니다. 선택적인 로컬 모델로 순위 검색을 시맨틱 또는 하이브리드로 전환할 수 있으며, 파일 인덱싱은 Enable index를 눌렀을 때만 이루어집니다. 구현 결과의 스크린샷을 가져오면 디자인 위에 겹쳐 볼 수 있습니다. 자세한 내용: [미리보기 가이드](../wiki/preview.md). Visual/Hybrid는 참조 PNG 또는 선택한 레이어와 픽셀 자르기를 지원합니다. 시각 인덱스는 명시적으로 활성화하며 검색 이미지는 저장하지 않습니다. **Copy reference prompt**는 선택한 레이어를 가리키는 짧은 프롬프트를 복사해 어떤 에이전트에든 붙여 넣을 수 있게 합니다.

<a id="mcp-and-skill"></a>

## MCP 서버와 에이전트 skill

**MCP.** 서버는 로컬에서 stdio로 실행되며 `workflow`, `tree`, `find`, `search`, `usages`, `diff`, `inspect`, `nodes`, `variables`, `bundle`, `verify`, `freshness`, `index`, `jobs`, `changes`, `watch`, `export`, `export_batch`, `compare_image`, `models`를 제공합니다. `export_batch`는 한 번의 호출로 최대 64개 파일을 쓰고 인라인 이미지 대신 매니페스트를 반환합니다. 절대 경로를 전달하세요. `--root /path/to/designs`(반복 가능)를 추가하면 모든 파일과 출력 경로를 선택한 폴더로 제한합니다. 사용자 지정 저장소를 명시적으로 요청하려면 `--storage-root DIR`(반복 가능)이 필요합니다. 기억된 workspace 위치는 이 옵션 없이 재사용합니다. [시작 인수와 여러 루트 설정 예](../README-ko.md#mcp-args)를 참고하세요. 래스터 내보내기는 `maxSize`(기본 1600px)에 맞게 축소되어 인라인 이미지가 작게 유지되며, SVG는 메타데이터만 반환합니다.

agent는 `workflow`로 `build`, `audit`, `images`, `search`, `plan`, `planning-details` 또는 명시적으로 요청된 경우 `yolo` 가이드를 읽을 수 있습니다. portable skill과 같은 소스를 사용하므로 MCP 사용 시 별도 skill 설치는 선택 사항입니다. 실제 작업은 agent의 프로젝트·브라우저 도구와 호스트 권한으로 수행합니다. 가이드를 읽는 것만으로 작업이 실행되지는 않으며, 활용 방식은 클라이언트 지원과 모델 동작에 따라 달라집니다.

Claude Code를 네이티브 Windows에서 사용한다면 [stdio 설정 안내](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server)와 `npx`용 `cmd /c` 래퍼를 따르세요.

**Skill.** [figlens skill](../skills/figlens/SKILL.md)은 에이전트가 frame을 찾고, bundle을 작게 나누어 읽고, 범위가 정해진 작업을 계획하고, 실행 중인 UI를 디자인과 대조해 검증하도록 안내합니다. 지침만 설치하므로 CLI 또는 MCP 서버를 사용할 수 있어야 합니다. [Skills CLI](https://github.com/vercel-labs/skills)로 설치하거나 `skills/figlens/`를 에이전트의 skills 디렉터리(Claude Code는 `~/.claude/skills/`)에 복사하세요.

```sh
npm install -g @runkids/figlens
npx skills add runkids/figlens --skill figlens
```

**플러그인.** Claude Code, Codex, Antigravity, Cursor, Kimi Code에서는 skill과 MCP 서버를 하나의 플러그인으로 함께 설치할 수 있습니다. [플러그인 설정](../README-ko.md#setup-plugin)을 참고하세요.

**skillshare.** [skillshare](https://github.com/runkids/skillshare)로 Claude Code, Codex, Cursor 전반의 skill과 MCP 연결을 한곳에서 관리하세요.

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync && skillshare sync mcp --global
```

**설치 참고.** npm 런처는 처음 사용할 때 해당 네이티브 바이너리를 내려받고, 릴리스의 SHA-256 체크섬으로 검증한 뒤 캐시합니다. 스크립트로 설치했다면 `figlens update`, Homebrew로 설치했다면 `brew upgrade figlens`로 업데이트하세요. 바이너리는 macOS(Apple silicon, Intel), Linux(arm64, x86_64, 정적 musl), Windows(x86_64)를 지원합니다.모든 설치 방법은 [선택형 시맨틱 검색](#semantic-search)의 런타임인 `figlens-embed`도 `figlens` 옆에 둡니다. 모델을 설정하기 전에는 모델을 불러오지 않습니다.

<a id="local-index"></a>

## 일반 검색과 선택형 로컬 인덱스

|  | 일반(기본) | 고급: 로컬 인덱스(선택) |
|---|---|---|
| 하는 일 | 호출할 때마다 `.fig`를 읽어 `search`, `find`, `inspect`, `export`에 답합니다. | 선택한 범위의 검색 인덱스를 한 번 만들고, 이후 `search`는 그 인덱스로 소스 전체를 함께 순위 매겨 답합니다. |
| 기록되는 것 | workspace도 인덱스 데이터베이스도 만들지 않습니다(`export`는 요청한 파일만 씁니다). | 사용자별 state 디렉터리 또는 선택한 `--storage` 아래의 로컬 workspace(SQLite 데이터베이스). |
| 쓰는 때 | 일회성 질문, 스크립트, CI. | 한 파일을 반복 검색할 때, 프로젝트의 여러 파일을 하나의 코퍼스로 검색할 때(`--workspace <dir>`), 컴포넌트, 변수, 스타일, 이미지의 정확한 사용처를 찾을 때. |

일반적인 디자인 질문은 인덱스를 만들지 않습니다. 한 번 명시적으로 켭니다("이 디자인의 인덱스를 만들어 줘"). 저장 위치는 figlens가 고르며 SQLite 등 별도 설정은 필요 없습니다.

```sh
# 일반: 상태 없음, 아무것도 만들지 않음
figlens search design.fig "checkout button"

# 고급, 한 번만: 범위(노드 id, 파일 전체는 0:0) 선택; 빌드는 포그라운드로 실행
figlens index prepare design.fig --scope 0:0

# 인덱스 검색; 인덱스 범위가 파일 전체가 아니면 --node 추가
figlens search --file design.fig "checkout button" --node 0:0

# .fig를 수정하거나 다시 내보낸 뒤: 바이트가 바뀐 범위만 다시 빌드
figlens index refresh --file design.fig

# .fig를 옮긴 뒤: workspace를 유지하고 refresh (prepare가 출력한 workspaceId 사용)
figlens index relocate <sourceId> new/design.fig --id <workspaceId>
figlens index refresh --id <workspaceId>
```

MCP: `index` prepare는 job을 반환합니다. `succeeded`가 될 때까지 `jobs`를 조회한 뒤 응답의 `workspaceId`로 검색합니다.

```json
{"tool": "index", "arguments": {"action": "prepare", "file": "/abs/path/design.fig", "scope": "0:0"}}
{"tool": "jobs", "arguments": {"action": "status", "jobId": "<jobId from the reply>"}}
{"tool": "search", "arguments": {"workspaceId": "<workspaceId from the reply>", "node": "0:0", "query": "checkout button"}}
```

수명 주기: `refresh`는 `.fig` 바이트가 바뀐 범위만 다시 빌드하고, `rebuild`는 바뀌지 않았어도 다시 빌드합니다. `clear_cache`는 빌드로 다시 만들 수 있는 데이터만 지우며 레지스트리와 게시된 인덱스는 유지합니다. `reset`은 이 workspace의 데이터베이스를 지우며(`.fig`와 내보내기는 건드리지 않음) 명시적으로 요청했을 때만 실행됩니다. 새 소스, 더 넓은 범위, reset, 재생성은 각각 별도의 명시적 요청이 필요합니다.

`.fig`를 옮겼나요? `index relocate`는 `workspaceId`, `sourceId`, 저장 위치를 유지합니다. 이후 영향받은 범위를 `refresh`하세요. 자동 탐색이나 해시 기반 자동 병합은 없으며 reset은 필요 없습니다.

한 폴더에 디자인이 여러 개인가요? `index add <dir> --workspace DIR`는 그 디렉터리 아래의 `.fig` 파일을 한 번에 최대 10개 등록하고, 디렉터리 밖으로 나가는 링크는 건너뜁니다. 응답에는 `added`, `existing`, `skipped`, `truncated`가 있으며, 반복해도 아무것도 추가되지 않고 각 파일에는 여전히 자체 `prepare --scope`가 필요합니다. 폴더 안에서 옮긴 파일은 `relocate`를 쓰지 않으면 새 소스가 됩니다. 응답의 `nextCursor`를 `--cursor C`(MCP `cursor`)로 넘기면 다음 10개를 추가합니다. 페이지 검색과 라이브러리 커서는 한 시간 동안 유효하며, 이후에는 `cursor_expired`를 반환하므로 다시 검색하세요.

데이터: SQLite 데이터베이스에는 등록된 로컬 경로, 선택한 범위의 레이어 이름과 유효 텍스트, 인스턴스의 컴포넌트 속성 값, 검색 postings, 정확한 관계(포함, 인스턴스, 변형, 변수/스타일/이미지 바인딩, 프로토타입 대상)와 그 대상 정의, 해시, 세대, job이 들어 있습니다. 원본 `.fig`는 제자리에서 변경되지 않으며, 원시 바이트는 데이터베이스에 저장되지 않습니다. 시맨틱·비전 모델 레이어는 임베딩한 벡터를, 활성화한 시각 인덱싱은 모델이 없어도 인덱싱된 레이어의 PNG 렌더링을 추가합니다. 쿼리 이미지는 저장하지 않습니다.

Pi, OpenCode, OMP는 각자의 opt-in 진입점으로 같은 인덱스를 쓰며, workspace는 재시작과 호스트를 넘어 재사용됩니다. 미구현: 파일 이벤트(watch는 파일을 폴링하며, holder가 실행 중일 때만 동작)와 푸시 이벤트.

<a id="semantic-search"></a>

## 선택형 시맨틱 검색

실험적 기능입니다. `figlens search <file> <query> --mode semantic|hybrid`는 로컬 모델로 레이어를 의미 기준으로 순위 매깁니다. 기본값 `--mode text`는 그대로이며 모델이 필요 없습니다.

- **최초 1회 설정.** BAAI/bge-m3 리비전 `5617a9f61b028005a4858fdac845db406aefb181`(MIT, `config.json`, `pytorch_model.bin`, `tokenizer.json`)를 직접 내려받으세요. figlens는 내려받지 않습니다. `figlens models configure bge-m3 --model DIR [--runtime FILE]`는 파일을 SHA-256으로 검증하고, `figlens-embed` 런타임을 오프라인으로 한 번 실행(`--runtime`이 없으면 figlens 실행 파일 옆에서 찾음)한 뒤 설정 디렉터리의 `models.json`에 프로파일을 기록합니다. `models list|status`로 확인하고, `models remove bge-m3`는 프로파일을 지우되 파일은 남깁니다.
- **비주얼 프로필.** `figlens models configure siglip-base --model DIR`는 google/siglip-base-patch16-224 리비전 `7fd15f0689c79d79e38b1c2e2e2370a7bf2761ed`(Apache-2.0; `config.json`, `preprocessor_config.json`, `model.safetensors`)를 같은 방식으로 확인하고 비주얼 검색용 이미지 모델로 등록합니다.
- **이미지 검색(실험적).** `figlens search --workspace DIR --image shot.png`는 workspace의 유닛을 생김새로 정렬합니다. `--image-ref '{"sourceId":…,"snapshotId":…,"nodeId":…}'`는 결과에 저장된 렌더를 대신 쓰고, `--crop x,y,w,h`는 둘 다 잘라냅니다. 검색어를 더하면 `hybrid`가 됩니다. 비주얼 레이어는 `figlens index visual`이 만듭니다(검색은 만들지 않으며 단일 `.fig` 검색은 이미지를 거부합니다). 이미지 프로필이 없거나 실패하면 픽셀 해시로만 정렬하고 `query.usedModes`에 그렇게 표시합니다. 검색 이미지는 저장되지 않습니다. 정렬 규칙은 아직 검증 중이므로 결과는 단서로 보세요. MCP: `search`의 `image`(root 안의 경로), `imageRef`, `crop`.
- **모드.** `semantic`은 각 레이어를 이름, 텍스트, 컴포넌트 속성, 범위 중 가장 잘 맞는 청크로 순위 매깁니다. `hybrid`는 텍스트와 시맨틱 순위를 합칩니다(Reciprocal Rank Fusion). 프로파일이 없으면 `semantic`은 `mode_unavailable`로 실패하고, `hybrid`는 텍스트 결과와 이유를 반환합니다. `coverage.layers.semantic`은 `ready`, `partial`, `unavailable`을 보고하며, 부분 페이지에는 `nextCursor`가 없습니다. 모델은 CPU에서 실행되며 Ctrl-C는 현재 요청을 멈춥니다. 각 MCP/뷰어 프로세스는 사용 후에도 텍스트 모델과 이미지 모델을 하나씩 메모리에 유지할 수 있습니다. 한 Linux ARM 측정에서 텍스트만 약 2.5 GiB, 두 모델의 최고 메모리 사용량 합계는 약 3.5 GiB였으며 코어와 인덱스 작업자는 제외했습니다. 측정값이며 메모리 상한이 아닙니다.
- **제한.** 레이어 이름, 유효 텍스트, 인스턴스의 컴포넌트 속성 값(정확히 해석할 수 없는 값은 제외하고 개수를 보고), 각 레이어의 페이지와 컨테이너 이름을 임베딩합니다. `--workspace`/`--id` 검색은 각 소스의 저장된 모델 레이어를 읽으며 디자인 텍스트를 임베딩하지 않습니다. 레이어는 `figlens index embed`로 만듭니다(MCP 서버는 텍스트 빌드 뒤에 큐에 넣습니다). 없는 소스는 `pending`이고 페이지는 부분 결과입니다. 이 기능의 고정 테스트 세트에서 모델은 동의어·목적 질의의 품질 기준을 넘지 못했습니다(top-1 오답률 0.25, 기준 0.20). 결과는 참고용으로 보세요.
- **MCP.** `models` 도구(`list`, `status`, `configure`, `remove`)와 `search`의 `mode`가 있습니다. `configure`는 figlens 옆의 `figlens-embed`만 실행하며, 모델 디렉터리와 파일은 서버 root 안에 있어야 합니다. 호출을 취소하면 실행 중인 검색이나 설정이 멈춥니다.

<a id="commands"></a>

## CLI 레퍼런스

| 명령어 | 출력 |
|---|---|
| `tree <file> [node] [--depth N] [--json]` | 노드 ID, REST 유형(`nodes`와 동일), 이름. 들여쓰기 텍스트, 또는 각 노드의 크기, 숨김 여부, 오토 레이아웃 모드을 담은 중첩 JSON. 페이지나 섹션 바로 아래 노드에는 `firstText`(첫 번째 보이는 텍스트)도 포함 |
| `tree <file> [node] --top-frames [--json]` | 화면만 한 줄에 하나씩, `--frames -`에 바로 넘길 수 있게 출력. 페이지나 그 섹션 바로 아래의 프레임, 컴포넌트, 컴포넌트 세트, 인스턴스(그룹과 내부 페이지 제외). `--json`은 페이지와 섹션 ID, 그리고 캔버스 라벨과 화면을 구분할 수 있는 각 화면의 첫 번째 보이는 텍스트(`firstText`)를 추가 |
| `find <file> <query> [--type TYPE] [--node ID] [--limit N]` | 대소문자 구분 없이 레이어 이름과 텍스트 검색. 경로, 크기, 미리보기. 기본 20개(1-200) |
| `search <file> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C] [--include-hidden] [--include-internal] [--mode text\|semantic\|hybrid\|visual] [--image PNG\|--image-ref JSON] [--crop x,y,w,h]` | 인스턴스 오버라이드, 텍스트/불리언 속성, 스왑을 적용한 레이어 이름과 전체 텍스트의 순위 검색. 정확한 노드 참조(`inspect`에 쓰는 `I…;…` 하위 레이어 ID 포함), 일치 이유, UTF-16 범위, 커버리지, 커서 페이지. 기본 20개(1-200), 숨긴 레이어와 내부 페이지는 해당 플래그 필요. `<file>` 대신 `--file`/`--workspace`/`--id`를 쓰면 workspace의 준비된 인덱스를 읽습니다(`index` 참고) `--mode semantic\|hybrid`에는 설정된 모델이 필요합니다. [선택형 시맨틱 검색](#semantic-search)을 참고하세요. |
| `models list\|status\|configure <profile> --model DIR [--runtime FILE]\|remove <profile>` | 시맨틱 검색용 로컬 텍스트 모델(`bge-m3`, 실험적)과 이미지 검색용 이미지 모델(`siglip-base`, 실험적). `configure`는 직접 내려받은 파일을 검증하며 내려받지 않습니다. `remove`는 파일을 남깁니다. [선택형 시맨틱 검색](#semantic-search)을 참고하세요. |
| `usages <file> <node\|resource> [--scope ID] [--relation R[,R]] [--out] [--snapshot S] [--include-hidden] [--limit N] [--cursor C]` | 저장된 한 범위에서 레이어 또는 리소스(`VariableID:…`, `StyleKey:…`, `image:…`)의 정확한 관계를 반환합니다. 중첩 인스턴스와 스왑도 포함합니다. 기본은 컴포넌트, 세트, 변수, 스타일, 이미지가 쓰이는 위치이고 `--out`은 레이어가 가리키는 대상입니다. 각 결과는 정확한 참조, 관계, 상태(`resolved`, `missing`, `unresolved` 등), 근거, 증거를 유지하며 이름은 동일성으로 보지 않습니다. 총합은 없고 coverage가 완전 또는 부분을 알려 줍니다. 기본 50개(1-200). `<file>` 대신 `--file`/`--workspace`/`--id`를 주면 workspace에 준비된 `--scope` 인덱스를 읽습니다. `--out`에서는 캐시된 라이브러리 사본의 사용이 그 `library` {kind, namespace, key}도 알려 줍니다. |
| `usages --file\|--workspace\|--id … --library <kind> --namespace <ns> --key <key> [--definition-source ID --definition-snapshot SNAP --definition-node NODE] [--source ID] [--scope ID] [--include-hidden] [--limit N] [--cursor C]` | 라이브러리 컴포넌트, 스타일 또는 변수(`kind`)를 쓰는 레이어를, 각 파일의 캐시된 사본이 기록한 대로 모든 소스의 준비된 `--scope`에서 반환합니다(`--source`로 하나로 좁힘). 정의는 `unresolved`로 남고(세 `--definition-*` 플래그로 정확한 참조를 지정하면 `selected`, `keyEvidence: unverified`) coverage는 소스별 상태와 부분인 이유를 알려 줍니다. 지정 값은 그것을 쓰는 레이어에 대한 `usages --out`의 `library` 필드에서 가져옵니다 |
| `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]` | 저장된 두 리비전 사이의 변경: 변경된 최상위 frame마다 한 줄, facet(텍스트, 레이아웃, 외형, 속성, 구조, 바인딩)별 개수. `--json`은 이전/새 필드 값과 edge 변경을 더합니다. 레이어는 정확한 id와 타입으로만 대응하며, 같은 이름·위치의 frame은 라벨이 붙은 possible match일 뿐이고, 중간에 멈춘 쪽이나 읽지 못한 필드만 다른 레이어는 삭제나 변경이 아니라 unknown으로 보고합니다. 페이지당 100개(1-500) 한도에서 멈춘 쪽이 있으면 `no changes` 대신 `stopped early`를 출력합니다. `--node`로 범위를 좁히세요. |
| `inspect <file> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat] [--full-text] [--colors]` | 펼쳐진 간결한 레이아웃/텍스트 요약, 최대 200노드. `--images`는 비트맵 크기와 크롭 범위를 추가. `--flat`은 대신 노드당 한 줄로 출력(노드 기준 위치, 크기, 텍스트, 폰트, 채우기, 선, 반경, auto-layout, 정렬, fixed/fill 크기, 그림자와 블러) `--full-text`는 각 텍스트를 전체로 반환하고(최대 10,000자), `--colors`는 보이는 단색 채우기(hex, 불투명도, 바인딩된 변수)를 추가합니다(`--flat`에서도 적용). 그라디언트, 이미지, 선은 포함하지 않습니다. JSON 출력에는 `path`(문서부터 노드까지의 상위 노드, `find`와 같은 `[{id, name}]`)가 포함되며 인스턴스 하위 레이어도 지원합니다. |
| `nodes <file> <node> [--compact [--subtrees]]` | REST nodes 응답의 노드별 객체(`document`, `components`, `componentSets`, `styles`): 펼쳐진 인스턴스와 재정의, 자동 레이아웃, 텍스트 스타일, 채우기, 효과, 연결 변수, 컴포넌트 속성, 렌더링 경계. 기본 REST는 그대로입니다. `--compact`는 반복 스타일을 공유하고, `--subtrees`를 더하면 각 노드의 ID와 경계를 유지하며 반복 하위 트리를 공유합니다. [무손실 형식 계약](../wiki/discovery.md#compact-nodes)을 참고하세요. Batch: `--frames` / `--ids` + `--compact [--subtrees]`. |
| `nodes <file> --frames <list>`, `--frames -` (stdin), `--ids <node>...` | 한 번 디코딩해 여러 노드: 목록의 id마다 JSON 한 줄(`nodeId`가 붙은 `nodes` 출력 또는 `{nodeId, error}`) |
| `variables <file> [node]`<br>`variables <file.fig> --format dtcg --collection ID --mode ID --out FILE` | 기본 REST 형식의 연결된/로컬 변수 출력은 그대로입니다. DTCG 2025.10 내보내기: 정확한 collection/mode ID를 지정하고 node는 생략합니다. 색상과 단위 없는 숫자만 지원합니다. 출력 디렉터리를 먼저 만드세요. 진단이 있으면 전체 내보내기가 실패하며 출력 파일은 변경되지 않습니다. 라이브러리 collection은 파일에 캐시된 사본에서 내보냅니다(`libraryCopy: true`). 다른 collection을 가리키는 alias는 그 collection의 값으로 내보내고 `aliasOf`에 기록합니다. mode가 여러 개면 `--alias-mode COLLECTION=MODE`를 지정하세요. |
| `bundle <file> <node> <out.zip> [--states] [--hash-source]` | 한 frame의 디자인 bundle. `{nodeId, path, warnings}` 출력 |
| `bundle <file> --frames <list> --out-dir <dir> [--jobs N] [--states] [--hash-source]`, `--frames -` (stdin) | 한 번 디코딩해 여러 bundle을 병렬 생성 |
| `verify <bundle> [--fonts <dir>]` | bundle(`.zip` 또는 압축을 푼 디렉터리)을 manifest와 대조: 검증·누락·변경된 파일, 누락된 asset, 프로젝트에서 확인할 폰트, 그리고 `findings`: 누락·미지원·미검증 handoff 입력마다 노드, 근거, 다음 조치; 이 컴퓨터 또는 `--fonts <dir>`(프로젝트 폴더, `@font-face`도 생성)에 폰트 파일이 있으면 `local_file`이며, 브라우저가 로드한다는 증거는 아닙니다 |
| `export <file> <node> <out> [--bounds layout] [--scale N] [--quality 1-100] [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID]]` | SVG, PNG, JPG, WebP, AVIF. 4,194,304픽셀 이하의 PNG는 디코딩된 RGBA를 바꾸지 않고 자동 무손실 최적화하며, 더 작아지지 않으면 원래 인코딩된 바이트를 유지. 더 큰 PNG는 일반 인코딩 사용. `--max-kb`는 크기에 맞을 때까지 손실 품질을 낮춤(PNG는 여전히 한도를 넘으면 실패하며 손실 방식으로 대체하지 않음). `--bounds layout`은 render bounds 대신 layout box로 내보냄. `--match-dir DIR`(SVG만, 하위 폴더 포함)은 같은 윤곽을 그리는 SVG(`matches`)와 점수가 붙은 비슷한 형태 최대 10개(`similar`)를 나열. `--hide ID[,ID]`는 node 안의 해당 레이어를 숨긴 상태로 같은 영역에 그리므로, 일반 내보내기와 비교하면 figlens가 그리는 그 레이어가 결과의 어디를 바꾸는지 알 수 있음(차이가 없어도 렌더링되지 않은 조정이 무해하다는 뜻은 아님) |
| `export <file> --frames <list> --out-dir <dir> [--format F] [--scale N] ...`, `--frames -` (stdin) | 한 번 디코딩해 여러 개 내보내기(기본 `png`): `<dir>/<nodeId>.<format>`에 쓰고 id마다 JSON 한 줄(`nodeId`가 붙은 `export` 출력 또는 `{nodeId, error}`) |
| `export <file> --batch <items.json>`, `--batch -` (stdin) | 한 번의 디코드로 여러 개를 내보내며 항목마다 자신의 옵션을 가짐: 1-64개의 `{node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}` JSON 배열 (MCP `export_batch`). 매니페스트 `{count, failed, items}` 하나를 출력하며 각 항목은 `export` 출력 또는 `{error}`(나머지는 계속 내보냄). 잘못된 항목이나 중복된 `out`은 파일을 쓰기 전에 실패합니다 |
| `preview <file> [node] [--out preview.html] [--port N] [--bind IP] [--idle-exit SECONDS]` | 디자인을 쓰지 않는 로컬 viewer. Enable index 후 시맨틱/하이브리드 검색 가능. `--out`은 독립 오프라인 스냅샷 생성. 결과와 복사한 context는 페이지와, live에서는 불러온 파일 경로, source의 snapshot과 로컬 revision을 표시(오프라인에서는 알 수 없으며 경로는 포함하지 않음). context에는 참조 scope의 coverage도 담기며, 확인하지 않았으면 `complete: null`. Visual/Hybrid는 참조 PNG 또는 선택한 레이어와 픽셀 자르기를 지원합니다. 시각 인덱스는 명시적으로 활성화하며 검색 이미지는 저장하지 않습니다. `--idle-exit SECONDS`는 열린 페이지 없이 그 시간이 지나면 멈춥니다. |
| `compare <file> <nodes.json> <node>` | 같은 frame의 REST 내보내기와 차이 비교 |
| `compare-image <file> <node> <screenshot.png>` | 구현한 화면의 스크린샷을 그 배율로 렌더링한 node와 비교 (`--scale`, `--limit`, `--out diff.png`, frame보다 짧은 전체 페이지 캡처에는 `--full-page`, 데이터에서 오는 콘텐츠 레이어에는 `--ignore`, 옅은 테두리를 보려면 `--threshold`; `image: true`는 이미지 채우기 영역; MCP `compare_image`). `mismatch`, `nonTextMismatch`와 가장 큰 차이 영역을 node 픽셀 좌표로 출력하고 각 영역 아래의 가장 깊은 레이어를 표시합니다. `text: true`는 텍스트 위의 영역입니다(대개 대체 글꼴이지만 각각 확인하세요) |
| `node <file> <node>` | 원시 `.fig` 노드 하나 |
| `clipboard [copied.html] <out.fig>` | 저장한 `.fig`가 없을 때 사용: Figma에서 레이어를 선택해 복사(마우스 오른쪽 클릭 > Copy 또는 Cmd/Ctrl+C)한 뒤 이 명령을 실행하면 클립보드가 모든 명령이 읽는 `.fig`로 변환됨. 다른 곳에 붙여넣으면 레이어의 텍스트만 나옴. 시스템 클립보드(macOS, Windows PowerShell 5.1과 Linux `wl-paste`/`xclip`은 미검증) 또는 저장한 클립보드 HTML 파일을 읽음. 노드 ID는 원본과 같고 사용된 컴포넌트도 포함. `missingImages`, `missingComponents`, `layersWithoutOutlines`를 보고: 복사본에는 이미지 바이트와 벡터 윤곽선이 없으므로 export와 bundle은 그것 없이 그린 곳을 경고. 기존 파일은 덮어쓰지 않음. 저장한 `.fig`가 있으면 대신 Figma 링크를 노드로 전달: 이미지와 벡터가 유지됨 |
| `freshness [<file.fig> [--url URL] \| --sources <mapping.json>]` | 디자인에 프레임이 없다고 판단하기 전에 저장된 사본이 온라인 Figma 파일보다 오래되었는지 확인합니다. 대상은 가장 가까운 `.figlens/sources.json`의 디자인(`{name, url, file}` 목록, `file`은 저장소 루트 기준 상대 경로이며, `.figlens/` 밖의 `--sources` 매핑은 자기 폴더 기준), 파일 하나, 또는 URL을 지정한 파일 하나입니다. 사본 `meta.json`의 `exported_at`을 `curl`과 `FIGMA_TOKEN`의 personal access token(스코프 `file_metadata:read`)으로 읽은 온라인 `last_touched_at`과 비교하여 `stale`, `current` 또는 이유가 붙은 `unknown`을 출력하며, 토큰이나 네트워크가 없어도 실패하지 않습니다. 토큰이 있으면 매핑된 오래된 사본에서 노드를 찾지 못하거나 `find` 결과가 없을 때 로컬 사본을 다시 저장하라고 알려 줍니다(MCP `freshness`) |
| `pull [--only NAME]... [--stale] [--sources <mapping.json>] \| --login` | 가장 가까운 `.figlens/sources.json`(또는 `--sources`)에서 매핑된 저장 사본을 새로 고칩니다. `--only`나 `--stale`이 없으면 아무것도 내려받지 않고 각 디자인의 최신 여부를 나열합니다. `--only`는 디자인을 고르고, `--stale`은 오래되었거나 없는 사본만 가져옵니다(`FIGMA_TOKEN` 필요). 전용 Chrome 프로필이 **File > Save local copy**를 실행하며, 다운로드는 디코딩되어야 하고 현재 사본보다 오래되지 않은 `exported_at`을 가져야 사본을 원자적으로 교체합니다. 검사에 실패하면 기존 사본을 유지하고, 실패 시 해당 단계 이름(`open`, `editor`, `menu`, `export`, `download`, `install`)을 알려 줍니다. `--login`은 로그인용 창을 띄웁니다(세션이 만료되면 pull도 창을 띄웁니다). 그 창에서 로그인하면 다른 기기의 Figma에서 로그아웃될 수 있습니다. Node, Chrome, `playwright-core`(`npm i -g @runkids/figlens playwright-core`)가 필요하며 Figma UI는 영어만 지원합니다. CLI 전용 |
| `index prepare\|refresh\|rebuild\|clear\|status\|add\|remove\|relocate\|reset\|track\|untrack\|embed\|visual` | 사용자별 figlens state 디렉터리 또는 workspace가 이후 기억하는 `--storage DIR` 아래의 로컬 workspace. `prepare <file> --scope NODE`로 해당 범위의 검색 인덱스를 한 번 만들면, 이후 `search --file <file>`(또는 `--workspace DIR`)은 다시 파싱하지 않고 workspace의 모든 소스를 함께 순위 매겨 답합니다. `refresh`는 변경된 파일을, `rebuild`는 전부(또는 `--source`/`--scope` 하나)를 다시 만들고, `clear`는 쓰지 않는 인덱스를 지웁니다. 새 인덱스가 준비될 때까지 이전 인덱스가 계속 쓰입니다. `reset`은 workspace 데이터베이스만 지우며 디자인, 내보내기, 기억된 위치는 건드리지 않습니다. `track --scope NODE`는 baseline 빌드부터 그 스코프의 변경을 기록하고, `untrack`은 기록을 남긴 채 중단합니다. `embed`는 게시된 스코프를 설정된 모델로 임베딩합니다(CPU와 메모리 사용). workspace 시맨틱 검색이 필요할 때만 실행하세요. `visual --scope NODE`는 게시된 스코프의 화면과 컴포넌트를 이미지 검색용으로 렌더링하고, 비전 프로필이 설정되면 임베딩도 합니다. workspace 이미지 검색이 필요할 때만 실행하세요. |
| `jobs status\|cancel <jobId>` | 인덱스 빌드 상태 확인 또는 취소. `cancel`은 빌드가 멈춘 뒤에 반환합니다 |
| `changes --file <file>\|--id ID\|--workspace DIR [--source ID] [--scope NODE] [--consumer NAME\|--subscription ID\|--cursor C] [--revision R] [--limit N]` | 추적 중인 스코프에 기록된 변경을 오래된 순으로 반환합니다. 게시된 빌드마다 리비전 하나(baseline 또는 changes)와 커버리지, 변경 종류·facet별 개수를 포함합니다. changes 리비전마다 변경이 닿는 레이어의 `impact` 요약이 있습니다(스키마 7 이전은 null). `--revision`은 그 리비전의 변경된 노드와 이어서 impact가 나열한 레이어를 페이지로 나눕니다(`entry`가 어느 쪽인지 알려 줍니다). `tracking`은 변경을 기록하려면 `index refresh`가 필요한 스코프를 알려 줍니다. 읽기 전용 |
| `watch ack --file <file>\|--id ID\|--workspace DIR --consumer NAME\|--subscription ID --cursor C [--source ID] [--scope NODE]` | 같은 필터로 처리한 마지막 `changes` 페이지의 `nextCursor`로 consumer의 체크포인트를 옮깁니다. 반복해도 안전하며 앞으로만 움직입니다. 다만 삭제 뒤에 남은 체크포인트는 현재 복구 커서가 되돌려 보존된 리비전을 다시 읽게 합니다. 아직 지나지 않은 리비전을 보존 정책이 삭제하기 전에 발급된 커서는 `event_gap`이며 아무것도 쓰지 않습니다 |
| `watch run --file <file>\|--id ID\|--workspace DIR --scope NODE [--source ID] [--request-id R]` | 추적 중인 scope의 lease를 포그라운드에서 유지합니다. workspace의 공유 runner가 저장할 때마다 갱신하므로(250 ms마다 폴링, 60 s마다 내용 확인) `index refresh` 없이 `changes`에 새 revision이 나옵니다. lease를 출력한 뒤 stdin이 닫히거나 Ctrl-C, SIGTERM을 받거나 scope 추적이 해제되면 종료 줄을 한 줄 출력합니다. MCP `watch` `start`와 `stop`은 서버가 실행되는 동안 lease를 유지하고, 갱신되지 않은 lease는 30 s 후 끝나며 runner는 마지막 lease 뒤에 종료됩니다 |
| `watch status --file <file>\|--id ID\|--workspace DIR` | workspace의 lease(`active`, `unauthorized`, `untracked`, `expired`, `ended`)와 runner(`running`, `stopping`, `stopped`, `unknown`, 배치 포함). 쓰지 않습니다 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]` | 소스, 스코프 또는 정확한 레이어(`--node`)를 최신 리비전부터 전용 체크포인트로 따라갑니다. `changes --subscription`으로 읽고 `watch ack --subscription`으로 확인합니다. 없거나 모호한 레이어는 `target_missing` 또는 `identity_uncertain`이며 같은 이름의 레이어로 대신하지 않습니다. `--owner`를 주면 그 작업 동안만 쓰는 임시 구독이 됩니다 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID --query TEXT [--type T[,T]] [--scope NODE]` | 한 소스의 scope에 대한 검색을 저장합니다. 상위 50개를 바로 평가하며, `changes --subscription`은 저장된 평가를 읽기만 하고 계산하지 않습니다 |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --sources visible\|ID[,ID] --query TEXT\|--library KIND --key KEY [--namespace NS] [--definition-source ID --definition-snapshot SNAP --definition-node NODE]` | 최대 10개 소스의 고정 집합에 대해 검색 또는 라이브러리 항목의 사용처와 `definitionMatch`를 저장합니다(`visible`은 지금 확정되며 나중에 추가된 소스는 포함되지 않음). `delta.changedSourceIds`는 바이트가 바뀐 소스, `corpus_changed`는 집합 변경을 나타내고 `coverage.transitive`는 `not_followed`이며 `delta.notFollowedSourceIds`는 사용처가 없는 변경된 소스(그 하위 의존은 다시 확인되지 않음)를 나타냅니다. 멤버를 제거하면 비활성화됩니다(unsubscribe 후 다시 subscribe) |
| `watch evaluate --file <file>\|--id ID\|--workspace DIR --subscription ID` | 쿼리 구독을 scope의 현재 인덱스로 평가해 저장합니다. 이미 평가되었으면 아무것도 하지 않습니다. scope를 watch 중이면 게시할 때마다 자동으로 합니다 |
| `watch list --file <file>\|--id ID\|--workspace DIR [--consumer NAME] [--after ID] [--subscription ID]` | 보이는 구독과 상태를 페이지 단위로 나열합니다. `--subscription`은 그 구독만 읽습니다(보이지 않으면 비어 있음) |
| `watch unsubscribe --file <file>\|--id ID\|--workspace DIR --subscription ID\|--consumer C --owner NAME` | 구독 하나와 그 체크포인트, 또는 owner의 임시 구독을 삭제합니다. 트래킹과 기록은 남습니다 |
| `watch forget --file <file>\|--id ID\|--workspace DIR --consumer NAME [--source ID] [--scope NODE]` | consumer의 원시 체크포인트를 삭제합니다. consumer당 구독과 체크포인트는 최대 64개, 워크스페이스 전체 4096개이며 자동으로 밀려나지 않습니다 |
| `mcp` | stdio를 사용하는 로컬 MCP 서버 |
| `update` | 최신 릴리스로 바이너리 교체 |

관련 컴포넌트 variants는 `inspect --states` 또는 `bundle --states`로 확인합니다(MCP는 `states: true`, 기본값 false, 설정 변경 불필요). 근거에는 출처, 알 수 없는 값, 생략 표시가 유지됩니다. Bundle에는 `design/states.json`과 `context.md` 요약이 추가됩니다. 범위와 제한은 [상태 근거](../wiki/states.md)를 참조하세요.

같은 component set의 두 variants는 `inspect <file> <variantA> --compare-variant <variantB>`(MCP `compareVariant`)로 비교합니다. `variantComparison`은 저장된 필드와 값 집합의 차이, 출처 ID, 알 수 없는 항목을 반환하며 자식 노드는 고유한 이름, 메인 컴포넌트, 같은 개수의 동명 그룹 안의 쌓임 순서 순으로 짝짓고(`pairedNodes`, 쌍별 변경 포함. `pairedBy`를 확인하며 `order`가 가장 약한 근거, 나머지는 짝 없음), 바인딩된 variables는 이름을 표시합니다. [비교 제한](../wiki/states.md#compare-two-variants)을 참조하세요.

한 파일의 임의의 두 노드(빈 화면과 채워진 화면, 모바일과 데스크톱 브레이크포인트, 헤더를 공유하는 두 화면 등)는 `inspect <file> <nodeA> --compare <nodeB>`(MCP `compare`)로 비교할 수 있습니다. `comparison`은 같은 형태에 `rootDelta`(크기와 부모 기준 위치)를 더해 반환합니다. 자식은 고유한 이름, 메인 컴포넌트, 같은 개수의 동명 그룹 안의 쌓임 순서 순으로 짝지어지며(`pairedBy`가 방법을 알려 줍니다) 나머지는 `reason: ambiguous-name`과 함께 짝지어지지 않습니다. 인스턴스는 펼쳐지지 않고 노드와 그 조상은 비교할 수 없습니다. `--compare-variant`는 여전히 같은 component set이 필요합니다.

노드 인수는 `12:345`, `12-345`, `?node-id=`가 있는 Figma URL을 받을 수 있습니다. `<file>`에 `gs://bucket/object`도 지정할 수 있으며 `gcloud` 로그인으로 다운로드하고 캐시합니다. 고정 객체를 통한 전달, 필요한 접근 권한과 갱신 제한은 [GCS 스냅샷 가이드](../wiki/gcs.md)를 참고하세요. CLI 호출은 매번 파일을 디코딩합니다. MCP는 마지막으로 읽은 변경 없는 로컬 파일만 재사용합니다. 원격 입력은 다시 파싱합니다. 전체 CLI 사용법은 `figlens --help`, 명령 하나의 항목은 `figlens inspect --help`(`-h`도 가능)를 실행하세요. `index add gs://bucket/object`는 해당 generation의 고정된 사본을 등록하고 `remote {uri, generation}`을 보고하며, gcloud는 add만 실행합니다.

`inspect`에는 기록된 텍스트 크기 조정, 잘림, 최대 줄 수와 장식이 포함됩니다. Dev Mode 주석은 각 노드의 `annotations`(일반 텍스트 `label` 미리 보기, 고정된 `properties`, `categoryId`; `nodes`는 `labelMarkdown`도 포함)에 담기고 이름이 있는 `annotationCategories`가 함께 제공됩니다. Ready for dev 프레임과 섹션에는 `devStatus`가 붙고 `--flat`은 둘 다 표시합니다. 측정은 `handoffDiagnostics`에 보고됩니다. auto-layout이 없는 부모에는 `childSpacing: "derived"`가 붙습니다. 자식 사이의 간격은 좌표 차이이며 auto-layout 규칙이 아닙니다. `constraints`가 고정할 수는 있습니다. `handoffDiagnostics`는 저장되었지만 출력되지 않는 의미 정보를 개수, 예시, [범위 제한](../wiki/semantics.md#known-handoff-gaps)과 함께 보고합니다. `_figlens.semantics`에는 소스가 저장된 필드만 나옵니다(항목이 없으면 저장되지 않은 것입니다). 이 매핑이 Figma와 비교되지 않았다는 점은 응답마다 한 번, 최상위 `_figlens.semantics.verification`에 표시됩니다. grid auto layout 프레임에는 행·열 수, 간격, 저장된 트랙 크기(`_figlens.gridTracks`)가 포함됩니다. 출력은 원본을 보호하고 쓰기가 완료된 뒤 대상을 교체합니다. MCP 요청 행은 1 MiB, 전체 응답은 8 MiB로 제한되며 초과하면 명시적인 오류를 반환합니다. CLI에서는 전체 JSON을 계속 출력할 수 있습니다.
