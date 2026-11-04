<p align="center">
  <img src="https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/logo.png" alt="figlens" width="220">
</p>

<h1 align="center">Detalhes de design para seu coding agent:<br>implementar, comparar e acompanhar mudanças.</h1>

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
  <a href="#start">Começar</a> · <a href="#demo">Demo</a> · <a href="#why">Por quê</a> · <a href="#bundle">O que o agente recebe</a> · <a href="#pi">Pi</a> · <a href="#limitations">Limitações</a> · <a href="https://figlens.runkids.cc/pt-BR/docs/"><b>Documentação</b></a>
</p>

O figlens transforma arquivos salvos do Figma em contexto de design que seu coding agent pode usar, não apenas uma captura de tela para interpretar.

- **Implementar com os detalhes de origem.** Leia layout, texto, estilos, variáveis e as anotações e o status Ready for dev do Dev Mode; exporte ícones vetoriais como SVG e imagens como recursos. `bundle` reúne os dados, recursos e renderização de referência de um frame; `verify` informa arquivos ausentes, verificações de fontes e limitações do handoff.
- **Comparar a UI em execução.** `compare-image` compara sua captura com a renderização do figlens e identifica as camadas sob as regiões diferentes. Revise essas diferenças e os avisos de renderização; não é uma garantia de fidelidade pixel a pixel.
- **Acompanhar mudanças no design.** `diff` compara revisões salvas; `changes` e `watch` informam atualizações em escopos locais monitorados. Salve um novo `.fig` no Figma para incorporar alterações; não é sincronização em tempo real com o Figma.

**Offline como base.** No Figma, **File > Save local copy**. Coloque `design.fig` ao lado do seu código. Depois disso, um link do Figma basta: o id do nó escolhe o frame. Sem uma cópia salva, copie as camadas no Figma (clique direito > Copy) e execute `figlens clipboard screen.fig`; uma cópia não traz imagens nem contornos vetoriais ([como](docs/README-pt-BR.md#copy-a-selection)). Um binário nativo o lê localmente, sem app do Figma em execução, login ou chamadas de rede ao Figma.

<a id="start"></a>

## Primeiros passos

**Ainda não tem um arquivo de design?** [Confira seu ambiente com o pequeno exemplo público](examples/first-success/README.md): verifique o binário escolhido, encontre um frame e valide seu bundle. Não é necessário um modelo.

Coloque `design.fig` no seu projeto e escolha **uma** configuração. Cada uma dá ao seu agent as ferramentas do figlens e o mesmo guia integrado para planejamento, implementação e preparo de imagens.

| Configuração | Para | O guia vem de |
|---|---|---|
| [Plugin](#setup-plugin) | Claude Code, Codex, Antigravity, Cursor, Kimi Code | o skill do plugin e a ferramenta `workflow` do servidor |
| [MCP](#setup-mcp) | Claude Code, Cursor, Codex, Gemini CLI e outros clientes MCP | a ferramenta `workflow` do servidor |
| [Pi](#setup-pi) | Pi 1.3.0+ | o pacote, que inclui o skill |
| [OpenCode V2](#setup-opencode) | OpenCode V2 2.2.0 | o skill incluído no pacote |
| [OMP](#setup-omp) | OMP 18.13.2 | o skill incluído no pacote |
| [Herdr](herdr/README.md) | um painel ao lado dos seus agentes no Herdr 0.1.12+: encontre uma tela e entregue | a configuração do agente que recebe |
| [CLI](#setup-cli) | agents que só executam comandos de shell, scripts e CI, ou você | `figlens workflow <topic>`, ou o [skill](#setup-skill) opcional |

**Comum vs avançado.** O uso comum não exige configuração: `figlens search design.fig "query"` lê o `.fig` e não cria nada. Um índice local opcional só é criado quando você pede (`figlens index prepare design.fig --scope 0:0`) e depois é reutilizado entre reinícios e hosts. Veja [Busca comum e o índice local opcional](docs/README-pt-BR.md#local-index). Opcional e experimental: `--mode semantic` ou `hybrid` ordena por significado com um modelo local que você mesmo configura; veja [Busca semântica opcional](docs/README-pt-BR.md#semantic-search).

<a id="setup-plugin"></a>

### Plugin

Uma única instalação adiciona o skill do figlens e o servidor MCP ao seu agente (Node.js 18+). O servidor fica fixado na versão do plugin (`npx -y @runkids/figlens@<version> mcp`); atualize o plugin para mudar de versão.

| Host | Instalação |
|---|---|
| Claude Code | `claude plugin marketplace add runkids/figlens`, `claude plugin install figlens@figlens` |
| Codex | `codex plugin marketplace add runkids/figlens`, `codex plugin add figlens@figlens` |
| Antigravity | `agy plugin install https://github.com/runkids/figlens/tree/main` |
| Cursor | `git clone --depth 1 https://github.com/runkids/figlens ~/.cursor/plugins/local/figlens` e recarregue a janela |
| Kimi Code | `/plugins install https://github.com/runkids/figlens/tree/main` em uma sessão e depois `/reload` |

[skillshare](https://github.com/runkids/skillshare) instala o mesmo plugin em vários hosts de uma vez: `skillshare plugin add https://github.com/runkids/figlens.git --target claude --target codex --target cursor -g`; atualize depois com `skillshare plugin update figlens -g`.

Reinicie o host ou abra uma nova sessão e confira se as ferramentas do figlens aparecem. A instalação de um plugin copia o repositório: cerca de 7 MB, mais o histórico do Git (cerca de 60 MB) no Codex. O Antigravity carrega só os primeiros 24 KB do skill; a ferramenta `workflow` do servidor fornece o guia completo. O Muse ainda não é suportado. [Tabela de hosts e o que foi verificado](wiki/host-plugins.md)

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

**Qualquer cliente MCP:**

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

Sem Node.js, instale o binário independente ([CLI](#setup-cli)) e use `"command": "figlens", "args": ["mcp"]`. Abra uma nova sessão e confira se as ferramentas do figlens aparecem (`/mcp` no Claude Code). Não é preciso instalar o skill separadamente. Adicione `--root <dir>` depois de `mcp` para limitar quais pastas o figlens pode ler e gravar.

<a id="mcp-args"></a>

#### Argumentos de inicialização MCP

Adicione estas opções após `mcp` nos `args` do cliente. São as únicas opções de inicialização MCP; ambas são opcionais e repetíveis.

| Opção | Significado |
|---|---|
| `--root DIR` | Permite ler designs e exportar dentro de `DIR` e suas subpastas. Sem ela, figlens não adiciona restrições de caminho; as permissões do sistema continuam valendo. Não se limita ao projeto atual nem varre o computador. |
| `--storage-root DIR` | Permite armazenamento de índices personalizado solicitado explicitamente dentro de `DIR` e suas subpastas. Não seleciona nem altera o local padrão. |

Use caminhos absolutos para diretórios existentes. Cada opção e caminho é um item separado do array; repita a opção para várias pastas. `--root` é um limite de caminho, não acesso de administrador, e restringe apenas figlens MCP, não outras ferramentas do agent.

Exemplo com duas raízes de design/saída e duas raízes de armazenamento permitidas:

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

Não existe `figlens mcp --storage`. Escolha o local real com o argumento `storage` do tool `index` (caminho absoluto dentro de uma raiz permitida), ou pela CLI: `figlens index prepare design.fig --storage /absolute/storage-a`. Após `prepare`, figlens lembra o local do workspace e o reutiliza entre reinicializações e hosts sem repetir `storage` ou `--storage-root`. Sem local personalizado, usa os diretórios padrão do usuário, sem flags de armazenamento.

Para não fixar esses caminhos em uma configuração versionada, ou para compartilhar designs entre worktrees do Git, veja [roots em uma configuração versionada](wiki/mcp.md#roots-in-a-checked-in-config).

Várias raízes de armazenamento apenas ampliam os locais permitidos: cada `storage` ainda escolhe um só. Não distribuem, sincronizam nem fazem backup dos índices. Indicar outra raiz não move um banco de dados existente; figlens recusa o local em conflito.

<a id="setup-pi"></a>

### Pi

Um único pacote inclui CLI, MCP, viewer, comandos `/figlens` e skill:

```sh
pi install npm:@runkids/figlens
```

[Guia da extensão Pi](docs/README-pt-BR.md#pi)

<a id="setup-opencode"></a>

### OpenCode V2

Instale o pacote no OpenCode V2:

```sh
opencode plugin add @runkids/figlens
```

Reinicie o cliente e execute `/figlens help`. [OpenCode V2](opencode/README.md)

<a id="setup-omp"></a>

### OMP

Instale o mesmo pacote no OMP:

```sh
omp plugin install @runkids/figlens
```

Reinicie o cliente e execute `/figlens help`. [OMP](omp/README.md)

<a id="setup-cli"></a>

### CLI

Execute pelo npm ou instale o binário independente:

```sh
npx @runkids/figlens --help                                                                # Node.js 18+, sem instalar
curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh | sh  # macOS, Linux
brew install runkids/tap/figlens                                                           # Homebrew
irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 | iex      # Windows PowerShell
```

O `npx` executa o binário da versão do pacote que ele resolve. Instalações por script são atualizadas com `figlens update`; as do Homebrew, com `brew upgrade figlens`. Há binários para macOS (Apple silicon, Intel), Linux (arm64, x86_64) e Windows (x86_64).

Um agent que usa a CLI lê o guia com `figlens workflow build` (também `audit`, `images`, `search`, `plan`, `planning-details` e `yolo`, usado só quando pedido explicitamente), sempre na mesma versão do binário, ou pelo [skill](#setup-skill).

[Referência da CLI](docs/README-pt-BR.md#commands)

<a id="setup-skill"></a>

### Skill para agents (opcional)

O skill ensina ao agent o mesmo fluxo do guia integrado: encontrar o frame, ler o bundle em partes pequenas, planejar o escopo e comparar a UI em execução com o design. Ele contém só instruções, então use-o com a CLI ou com MCP. As configurações MCP e Pi não precisam dele.

**Recomendado: [skillshare](https://github.com/runkids/skillshare).** Instale o skill uma vez e sincronize-o com cada agent que você usa:

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare sync
```

A mesma ferramenta também pode configurar a conexão MCP do figlens para esses agents:

```sh
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync mcp --global
```

Outras formas de instalar o skill:

```sh
npx skills add runkids/figlens --skill figlens   # Skills CLI
```

Ou copie `skills/figlens/` deste repositório ou de um arquivo de release para o diretório de skills do seu agent (Claude Code: `~/.claude/skills/`). Um skill instalado é atualizado separadamente do binário; o `figlens workflow` sempre corresponde ao binário.

Depois peça uma tela:

```text
Build the task board from ./design.fig in this project.
Find the matching frame, use this project's components and image workflow,
then compare the running UI with the design and fix confirmed differences.
Report missing assets and any checks you could not run.
```

O agent encontra o frame, reúne os dados do design, os assets e uma imagem de referência, e constrói a tela com a sua stack. O fluxo compartilhado orienta a conferência de fontes e recursos; o que falta é informado enquanto a implementação do layout continua com alternativas documentadas. Peça os arquivos alterados e uma comparação no navegador.

<a id="demo"></a>

## Demo: o Pi cria uma tela React a partir de um .fig

[![O Pi planeja a partir de um .fig local, cria uma tela React e a confere ao lado de um navegador ao vivo](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/taskflow-demo.gif)](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)

O GIF em loop reproduz o vídeo editado a três vezes a velocidade. Clique para ver o vídeo completo de 78 segundos.

**[Vídeo de 78 segundos](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4).** O Pi executa `/figlens plan`, lê um frame de dashboard de um `.fig` local, o constrói com React e Tailwind CSS e depois confere e corrige a UI em execução. Time-lapse editado, não é um benchmark nem uma promessa de pixel-perfect. Design: [Task Flow por Eymen Eid](https://www.figma.com/community/file/1224268866120711793) · [notas da gravação](wiki/accuracy.md#taskflow-react-and-tailwind-demo).

**[De .fig a HTML/CSS editável](examples/task-list/README.md):** em um estudo controlado de uma lista de tarefas, as seis tentativas baseadas em figlens preservaram as 19 ocorrências de texto visível como texto DOM real e os quatro ícones. A extração por CLI e MCP também passou com a rede do Docker desativada. O estudo inclui configuração, medições e comparações de imagens independentes.

<a id="why"></a>

## Por que figlens em vez do MCP oficial do Figma?

Use o **figlens** quando o design precisa viajar como arquivo: agents remotos, CI, trabalho offline, handoff entre agents, leituras repetidas ilimitadas. Use o **MCP oficial do Figma** quando o agent precisa ver o design ao vivo, sem etapa de exportação.

Ler um `.fig` funciona offline. Instalar e atualizar o figlens, entradas `gs://`, seu provedor de modelo e as dependências do projeto ainda precisam de rede.

[Comparação completa](docs/README-pt-BR.md#why)

<a id="bundle"></a>

## O que o agent recebe

Um frame, um zip. Estrutura para construir, assets para usar, uma imagem para conferir.

[Conteúdo do bundle e exemplos](docs/README-pt-BR.md#bundle) · [Qual é a proximidade com o Figma?](docs/README-pt-BR.md#accuracy)

## Documentação

- <a id="pi"></a>[Extensão do Pi](docs/README-pt-BR.md#pi)
- <a id="viewer"></a>[Explore um design sem o Figma](docs/README-pt-BR.md#viewer)
- <a id="mcp-and-skill"></a>[Servidor MCP e skill para agents](docs/README-pt-BR.md#mcp-and-skill)
- <a id="commands"></a>[Referência da CLI](docs/README-pt-BR.md#commands)

<a id="limitations"></a>

## Limitações

- **Snapshots, não sincronização ao vivo.** Exporte um novo `.fig` quando o design mudar. O figlens não consegue baixar um design a partir de uma URL do Figma.
- **Formato não documentado.** O figlens decodifica o schema embutido no arquivo; novas exportações do Figma podem exigir uma atualização do figlens.
- **Fontes e texto:** os arquivos de fontes não estão incluídos; texto com override pode manter o layout salvo pelo Figma. Use fontes autorizadas do projeto e compare o resultado.
- **Cobertura:** as [melhorias de renderização](https://github.com/runkids/figlens/issues/36) e a [semântica de entrega](https://github.com/runkids/figlens/issues/37) são acompanhadas nas issues. Veja a [cobertura semântica atual](wiki/semantics.md).
- **Confira a saída.** Leia os `warnings` do bundle, `missing-assets.json` e as variáveis não resolvidas em `context.md`, depois compare a implementação com o design.
- As limitações visíveis conhecidas aparecem em `_figlens.renderDiagnostics` dos nós, nos resumos limitados de `inspect` e nos `warnings` do bundle e do `export`; `missing-assets.json` contém apenas falhas de assets. A ausência de avisos não garante fidelidade de pixels.

## Ajude o figlens a crescer

Se o figlens transformou um design em código para você, [dê uma estrela no repo](https://github.com/runkids/figlens) para que outras pessoas o encontrem. Encontrou um arquivo que ele não consegue ler? [Abra uma issue](https://github.com/runkids/figlens/issues) com o comando e o que você esperava.

O desenvolvimento roda Rust apenas em Docker: `docker compose run --rm dev cargo test`. Veja [AGENTS.md](AGENTS.md) e o [wiki](wiki/README.md).

## Licença

[MIT](LICENSE). O figlens é um projeto independente, sem afiliação, endosso ou patrocínio do Figma. O formato `.fig` não é documentado e pode mudar; use apenas arquivos e assets que você tem permissão para acessar e compartilhar.
