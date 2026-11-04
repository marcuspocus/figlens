# figlens — Guia de uso

[← README](../README-pt-BR.md) · [figlens.runkids.cc](https://figlens.runkids.cc/pt-BR/docs/)

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

- [Instalação independente](#install)
- [Por que figlens em vez do MCP oficial do Figma?](#why)
- [O que o agent recebe](#bundle)
- [Extensão do Pi](#pi)
- [Painel para o Herdr](#herdr)
- [Explore um design sem o Figma](#viewer)
- [Servidor MCP e skill para agents](#mcp-and-skill)
- [Referência da CLI](#commands)

<a id="install"></a>

## Instalação independente

**Sem Node.js?** Instale o binário independente e use `"command": "figlens", "args": ["mcp"]` no lugar:

| Plataforma | Comando |
|---|---|
| macOS / Linux | `curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh \| sh` |
| Homebrew | `brew install runkids/tap/figlens` |
| Windows (PowerShell) | `irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 \| iex` |

<a id="why"></a>

## Por que figlens em vez do MCP oficial do Figma?

Use o **figlens** quando o design precisa viajar como arquivo: agents remotos, CI, trabalho offline, handoff entre agents, leituras repetidas ilimitadas. Use o **MCP oficial do Figma** quando o agent precisa ver o design ao vivo, sem etapa de exportação.

| | figlens | MCP oficial do Figma |
|---|---|---|
| Fonte | Snapshot `.fig` exportado | Arquivos Figma ao vivo |
| Requer | O arquivo | Login no Figma; o servidor desktop exige o app |
| Rate limits | Nenhum, parsing local | Limites do serviço do Figma |
| Agent remoto / CI | Copie o arquivo, execute o binário | Servidor remoto com login |
| Mudanças no design | Exporte um novo `.fig` | Automático |
| Handoff | Compartilhe o `.fig` ou um bundle de frame | Compartilhe o acesso ao Figma |
| Fidelidade | Parser independente com [lacunas conhecidas](../README-pt-BR.md#limitations) | Mantido pelo Figma |

Ler um `.fig` funciona offline. Instalar e atualizar o figlens, entradas `gs://`, a verificação opcional `freshness`, seu provedor de modelo e as dependências do projeto ainda precisam de rede.

<a id="bundle"></a>

## O que o agent recebe

Um frame, um zip. Estrutura para construir, assets para usar, uma imagem para conferir.

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

Precisa de uma parte só? A CLI faz isso direto:

```sh
npx -y @runkids/figlens find design.fig "login" --type FRAME     # IDs de frames por nome ou texto
npx -y @runkids/figlens inspect design.fig 12:345                 # resumo compacto de layout e texto
npx -y @runkids/figlens nodes design.fig 12:345 > frame.json      # JSON no formato REST
npx -y @runkids/figlens export design.fig 12:345 icon.svg         # SVG, PNG, JPG, WebP ou AVIF
npx -y @runkids/figlens bundle design.fig 12:345 frame.zip        # o zip acima
```

Exportações SVG incorporam os preenchimentos de imagem originais e convertem o texto em contornos de glifos, então nenhuma fonte é necessária. Via MCP, exportações raster também voltam como imagens inline, e o agent vê o design sem abrir um arquivo.

<a id="copy-a-selection"></a>

### Sem um .fig salvo? Copie uma seleção

1. No Figma, selecione o frame ou as camadas e copie: clique direito > **Copy**, ou Cmd+C (Ctrl+C no Windows e no Linux).
2. Execute `figlens clipboard screen.fig`, ou peça ao seu agente para construir "a tela que acabei de copiar". Ele lê a área de transferência e grava um `.fig` que todos os comandos leem.
3. Passe `screen.fig` como qualquer outro arquivo: `figlens bundle screen.fig <node> frame.zip`.

Colar a cópia em um campo de texto traz só o texto dela; o figlens lê o HTML da cópia, que contém as camadas. O macOS foi verificado com cópias reais; Windows (PowerShell 5.1) e Linux (`wl-paste` ou `xclip`) ainda não.

| | **File > Save local copy** | Seleção copiada |
|---|---|---|
| Escopo | O arquivo inteiro | As camadas que você copiou, mais os componentes que elas usam |
| Preenchimentos de imagem e contornos vetoriais | Incluídos | Ausentes; os exports avisam onde desenham sem eles |
| Um link do Figma encontra o nó | Sim | Sim, para camadas copiadas: os ids coincidem com o arquivo de origem |
| Serve para | Construir a partir de um design que você mantém ao lado do código | Uma olhada rápida em uma tela quando não existe cópia salva |

<a id="accuracy"></a>

### Qual é a proximidade com o Figma?

Medido contra as próprias exportações JSON, PNG e SVG do Figma em **826 frames de produção** e cinco arquivos Community (2026-10-22):

| Verificação | Resultado |
|---|---|
| Estrutura, nomes, visibilidade, limites e texto dos nós | Idênticos em 771 / 826 frames |
| Preenchimentos de imagem originais | 2,608 / 2,608 idênticos byte a byte |
| Assets SVG | 5,014 / 5,197 dentro do limiar de diferença de pixels |
| Bundles de frames Community | 306 / 306 sem assets ausentes |

Resultados nesses arquivos, não uma garantia para todo design. Método e limiares: [relatório de precisão](../wiki/accuracy.md).

<a id="pi"></a>

## Extensão do Pi

```sh
pi install npm:@runkids/figlens
```

Guias de instalação: [OpenCode V2](../opencode/README.md) · [OMP](../omp/README.md).

Reinicie o Pi ou execute `/reload` e peça uma tela em linguagem natural. O Pi encontra o frame, lê pelo servidor MCP registrado automaticamente e constrói com a sua stack. Sem IDs de nós, sem CLI global.

Pedir um texto ou uma tela busca direto no `.fig` e não salva nada. Para buscas repetidas, peça ao Pi para ativar um índice de um design e escopo: ele prepara um índice local uma vez (sem comando de configuração), acompanha o job até ficar pronto e o reutiliza após reinícios e em outros hosts. Redefinir esse índice exige um pedido explícito seu. Perguntar onde um componente, variável, estilo ou imagem é usado lista seus usages exatos do mesmo jeito: direto do `.fig` ou, depois de ativado, desse índice. Pedir ao Pi para acompanhar as mudanças de um design faz o track desse escopo, preparando o índice se preciso, e informa o que mudou desde a última verificação; uma pergunta sobre duas cópias salvas é respondida com `diff`, sem iniciar nada. Detalhes: [índice local](#local-index).

- `/figlens plan <request>` cria apenas um plano; não modifica a aplicação.
- `/figlens yolo <request>` planeja, constrói, confere e limpa o próprio plano temporário, os exports de rascunho e os processos que iniciou.
- `/figlens preview [description]` abre o viewer local.
- `/figlens review` aprova mapeamentos de componentes reutilizáveis propostos pelo agent.

![Peça uma tela ao Pi, deixe que ele encontre o design, construa com a sua stack e confira o resultado](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/pi-workflow.svg)

Passo a passo completo, comandos e limites: [pi/README.md](../pi/README.md).

<a id="herdr"></a>

## Painel para o Herdr

Um painel do [Herdr](https://herdr.dev) para o design ao lado dos seus agentes: abra um `.fig`, encontre uma tela ou camada por nome, texto, ID de nó ou link do Figma, veja em tamanho real e entregue ao agente da esquerda como Build, Plan ou YOLO depois de um diálogo de confirmação que mostra a mensagem exata. Não precisa instalar o figlens e lê designs de máquinas remotas do Herdr no próprio lugar. Peça ao agente no Herdr: `Read ~/.config/herdr-app/AGENTS.md, then add this layout: https://github.com/runkids/figlens/tree/main/herdr/figlens-layout`. Detalhes: [herdr/README.md](../herdr/README.md).

<a id="viewer"></a>

## Explore um design sem o Figma

[![Inspecione o design MCP Apps localmente, com sua árvore de camadas, layout e propriedades de componentes](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/preview-inspector.png)](https://github.com/runkids/figlens/blob/v0.10.1/assets/pi-demo.mp4)

```sh
npx -y @runkids/figlens preview design.fig                        # viewer local, Ctrl+C para parar
npx -y @runkids/figlens preview design.fig 12:345 --out preview.html   # snapshot HTML offline para enviar a qualquer pessoa
```

Navegue por páginas e camadas, busque, use a busca ranqueada e os usos exatos da seleção, meça, inspecione layout, tipografia, variáveis e CSS, exporte qualquer camada, copie um prompt de implementação ou baixe um bundle da seleção. Mude a busca ranqueada para semântica ou híbrida com um modelo local opcional: o visualizador só indexa o arquivo quando você clica em Enable index. Importe uma captura de tela da sua implementação para sobrepô-la ao design. Detalhes: [guia de preview](../wiki/preview.md). Visual/Hybrid aceita um PNG de referência ou uma camada selecionada com recorte em pixels. Ative o índice visual explicitamente; imagens de consulta não são armazenadas.

<a id="mcp-and-skill"></a>

## Servidor MCP e skill para agents

**MCP.** O servidor roda localmente por stdio e expõe `workflow`, `tree`, `find`, `search`, `usages`, `diff`, `inspect`, `nodes`, `variables`, `bundle`, `verify`, `freshness`, `index`, `jobs`, `changes`, `watch`, `export`, `export_batch`, `compare_image` e `models`. `export_batch` grava até 64 arquivos em uma chamada e retorna um manifesto em vez de imagens inline. Use caminhos absolutos. Adicione `--root /path/to/designs` (repetível) para confinar todo arquivo e caminho de saída às pastas escolhidas. Armazenamento personalizado solicitado explicitamente exige `--storage-root DIR` (repetível); locais lembrados são reutilizados sem essa opção. Veja os [argumentos de inicialização e o exemplo com várias raízes](../README-pt-BR.md#mcp-args). Exportações raster são reduzidas a `maxSize` (padrão 1600 px) para manter as imagens inline pequenas; SVG retorna apenas metadados.

Com `workflow`, o agent pode ler os guias integrados `build`, `audit`, `images`, `search`, `plan`, `planning-details` ou `yolo` quando solicitado explicitamente. Eles compartilham as fontes do skill portátil; sua instalação separada é opcional para MCP. O agent executa o trabalho com suas ferramentas de projeto e navegador e as permissões do host. Ler um guia não executa o fluxo; seu uso depende do cliente e do comportamento do modelo.

Windows nativo com Claude Code: siga as [notas de configuração stdio](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server) e o wrapper `cmd /c` para `npx`.

**Skill.** O [skill do figlens](../skills/figlens/SKILL.md) ensina o agent a encontrar frames, ler o bundle em partes pequenas, planejar trabalho com escopo definido e verificar a UI em execução contra o design. Ele instala apenas instruções; a CLI ou o servidor MCP precisa estar disponível. Instale com a [Skills CLI](https://github.com/vercel-labs/skills) ou copie `skills/figlens/` para o diretório de skills do seu agent (Claude Code: `~/.claude/skills/`):

```sh
npm install -g @runkids/figlens
npx skills add runkids/figlens --skill figlens
```

**Plugins.** Claude Code, Codex, Antigravity, Cursor e Kimi Code instalam o skill e o servidor MCP juntos como um plugin; veja [Configuração do plugin](../README-pt-BR.md#setup-plugin).

**skillshare.** Gerencie o skill e a conexão MCP no Claude Code, Codex e Cursor em um só lugar com o [skillshare](https://github.com/runkids/skillshare):

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync && skillshare sync mcp --global
```

**Notas de instalação.** O launcher npm baixa o binário nativo correspondente no primeiro uso, verifica-o contra os checksums SHA-256 da release e o armazena em cache. Instalações por script são atualizadas com `figlens update`; as do Homebrew, com `brew upgrade figlens`. Os binários cobrem macOS (Apple silicon, Intel), Linux (arm64, x86_64, musl estático) e Windows (x86_64). Todo caminho de instalação também coloca `figlens-embed`, o runtime da [busca semântica opcional](#semantic-search), ao lado de `figlens`; ele não carrega nenhum modelo até você configurar um.

<a id="local-index"></a>

## Busca comum e o índice local opcional

|  | Comum (padrão) | Avançado: índice local (opcional) |
|---|---|---|
| O que faz | Lê o `.fig` a cada chamada e responde `search`, `find`, `inspect` e `export`. | Cria uma vez um índice de busca de um escopo escolhido e responde `search` a partir dele, com ranking entre fontes. |
| O que é gravado | Nenhum workspace nem banco de índice (`export` grava só os arquivos que você pedir). | Um workspace local (banco SQLite) no diretório de estado do usuário, ou no `--storage` escolhido. |
| Quando usar | Perguntas pontuais, scripts, CI. | Buscas repetidas em um arquivo; vários arquivos de um projeto como um único corpus (`--workspace <dir>`); usos exatos de componentes, variáveis, estilos e imagens. |

Uma pergunta de design comum nunca cria um índice. Você ativa uma vez, de forma explícita ("crie um índice para este design"); o figlens escolhe o armazenamento e não exige SQLite nem outra configuração.

```sh
# Comum: sem estado, não cria nada
figlens search design.fig "checkout button"

# Avançado, uma vez: escolha um escopo (um id de nó, ou 0:0 para o arquivo inteiro); o build roda em primeiro plano
figlens index prepare design.fig --scope 0:0

# Busca indexada; adicione --node quando o escopo indexado não for o arquivo inteiro
figlens search --file design.fig "checkout button" --node 0:0

# Depois de editar ou reexportar o .fig: recrie só os escopos cujos bytes mudaram
figlens index refresh --file design.fig

# Depois de mover o .fig: mantenha o workspace e faça refresh (use o workspaceId impresso por prepare)
figlens index relocate <sourceId> new/design.fig --id <workspaceId>
figlens index refresh --id <workspaceId>
```

MCP: `index` prepare devolve um job; consulte `jobs` até `succeeded` e busque com o `workspaceId` da resposta.

```json
{"tool": "index", "arguments": {"action": "prepare", "file": "/abs/path/design.fig", "scope": "0:0"}}
{"tool": "jobs", "arguments": {"action": "status", "jobId": "<jobId from the reply>"}}
{"tool": "search", "arguments": {"workspaceId": "<workspaceId from the reply>", "node": "0:0", "query": "checkout button"}}
```

Ciclo de vida: `refresh` recria só os escopos cujos bytes do `.fig` mudaram; `rebuild` recria mesmo sem mudança; `clear_cache` apaga só dados que um build pode recriar e mantém o registro e os índices publicados; `reset` apaga o banco de dados deste workspace (nunca o `.fig` nem as exportações) e só roda a pedido explícito. Novas fontes, um escopo maior, reset e recriação exigem, cada um, um pedido explícito próprio.

Moveu o `.fig`? `index relocate` mantém `workspaceId`, `sourceId` e armazenamento; depois faça `refresh` dos escopos afetados. Não há descoberta automática nem fusão por hash, e não é preciso reset.

Vários designs em uma pasta? `index add <dir> --workspace DIR` registra de uma vez até 10 arquivos `.fig` desse diretório e ignora links que saem dele; a resposta traz `added`, `existing`, `skipped` e `truncated`, repetir não adiciona nada, e cada arquivo ainda precisa do próprio `prepare --scope`. Um arquivo movido dentro da pasta é uma nova fonte, a menos que você use `relocate`. Passe o `nextCursor` da resposta como `--cursor C` (MCP `cursor`) para adicionar os 10 seguintes. Os cursores paginados de busca e de biblioteca duram uma hora; depois disso retornam `cursor_expired`, então busque de novo.

Dados: o banco SQLite guarda caminhos locais registrados, nomes de camadas e texto efetivo dos escopos escolhidos, valores de propriedades de componente das instâncias, postings de busca, relações exatas (contenção, instâncias, variantes, vínculos de variáveis/estilos/imagens, destinos de protótipo) e as definições a que apontam, hashes, gerações e jobs. O `.fig` original permanece inalterado onde está; seus bytes brutos não são guardados no banco. Camadas de modelos semânticos e de visão acrescentam os vetores que incorporam, e a indexação visual que você ativar acrescenta renderizações PNG das camadas indexadas, mesmo sem modelo; imagens de consulta nunca são guardadas.

Pi, OpenCode e OMP usam o mesmo índice por meio de sua própria entrada opt-in, e o workspace é reutilizado entre reinícios e hosts. Não implementado: eventos de arquivo (um watch consulta o arquivo, e só enquanto um holder roda) e eventos push.

<a id="semantic-search"></a>

## Busca semântica opcional

Experimental. `figlens search <file> <query> --mode semantic|hybrid` ordena as camadas por significado com um modelo local. O `--mode text` padrão não muda e não precisa de modelo.

- **Configuração única.** Baixe você mesmo o BAAI/bge-m3 na revisão `5617a9f61b028005a4858fdac845db406aefb181` (MIT; `config.json`, `pytorch_model.bin`, `tokenizer.json`): o figlens nunca baixa. `figlens models configure bge-m3 --model DIR [--runtime FILE]` confere os arquivos por SHA-256, inicia o runtime `figlens-embed` uma vez offline (procurado ao lado do executável do figlens, a menos que `--runtime` seja dado) e grava o perfil em `models.json` no diretório de configuração. `models list|status` o mostram; `models remove bge-m3` o esquece e mantém os arquivos.
- **Perfil visual.** `figlens models configure siglip-base --model DIR` verifica da mesma forma o google/siglip-base-patch16-224 na revisão `7fd15f0689c79d79e38b1c2e2e2370a7bf2761ed` (Apache-2.0; `config.json`, `preprocessor_config.json`, `model.safetensors`) e registra o modelo de imagem para a busca visual.
- **Consultas por imagem (experimental).** `figlens search --workspace DIR --image shot.png` ordena as unidades de um workspace pela aparência; `--image-ref '{"sourceId":…,"snapshotId":…,"nodeId":…}'` usa o render salvo de um resultado, e `--crop x,y,w,h` recorta qualquer um dos dois. Com texto de consulta vira `hybrid`. As camadas visuais são criadas por `figlens index visual` (uma busca não cria nenhuma, e a busca em um único `.fig` recusa a imagem). Sem o perfil de imagem, ou se ele falhar, só o hash de pixels ordena e `query.usedModes` informa isso. A imagem de consulta nunca é salva. A regra de ordenação ainda está em validação; trate os resultados como pistas. MCP: `search` `image` (um caminho dentro dos roots), `imageRef`, `crop`.
- **Modos.** `semantic` ordena cada camada pelo melhor trecho de nome, texto, propriedades de componente ou escopo. `hybrid` funde os rankings de texto e semântico (Reciprocal Rank Fusion). Sem perfil, `semantic` falha com `mode_unavailable` e `hybrid` devolve os resultados de texto e informa o motivo. `coverage.layers.semantic` informa `ready`, `partial` ou `unavailable`; uma página parcial não tem `nextCursor`. Os modelos usam CPU; Ctrl-C interrompe a requisição ativa. Um processo MCP/visualizador pode manter um modelo de texto e um de imagem na memória após o uso: o de texto sozinho mediu cerca de 2,5 GiB e a soma dos picos dos dois cerca de 3,5 GiB em uma medição Linux ARM, sem o núcleo e os workers de indexação. São medições, não limites de memória.
- **Limites.** São embutidos nomes de camada, texto efetivo, os valores de propriedades de componente de uma instância (um valor que não se resolve com exatidão é omitido e contado) e os nomes de página e contêineres de cada camada. A busca com `--workspace`/`--id` lê a camada de modelo armazenada de cada fonte e nunca incorpora texto do design; ela é criada com `figlens index embed` (o servidor MCP a enfileira após uma compilação de texto). Uma fonte sem ela fica `pending` e a página é parcial. No conjunto de testes congelado desta função, o modelo não atingiu o critério de qualidade para consultas de sinônimo e finalidade (taxa de top-1 errado 0,25, critério 0,20); trate os resultados como pistas.
- **MCP.** A ferramenta `models` (`list`, `status`, `configure`, `remove`) e `mode` em `search`; `configure` executa apenas o `figlens-embed` ao lado do figlens, e o diretório e os arquivos do modelo precisam estar dentro das raízes do servidor. Cancelar a chamada interrompe uma busca ou configuração em andamento.

<a id="commands"></a>

## Referência da CLI

| Comando | Saída |
|---|---|
| `tree <file> [node] [--depth N] [--json]` | IDs, tipos REST (como em `nodes`) e nomes dos nós; texto indentado, ou JSON aninhado com o tamanho de cada nó, se está oculto e o modo de auto layout, mais `firstText` (primeiro texto visível) nos nós de uma página ou seção |
| `tree <file> [node] --top-frames [--json]` | Só as telas, uma por linha, prontas para `--frames -`: frames, componentes, conjuntos de componentes e instâncias de uma página ou de suas seções (grupos e páginas internas ignorados); `--json` acrescenta os ids de página e seção e o primeiro texto visível de cada tela (`firstText`), que separa os rótulos do canvas das telas |
| `find <file> <query> [--type TYPE] [--node ID] [--limit N]` | Busca sem diferenciar maiúsculas em nomes de camadas e textos; caminhos, tamanhos e prévias. Padrão de 20 resultados (1-200) |
| `search <file> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C] [--include-hidden] [--include-internal] [--mode text\|semantic\|hybrid\|visual] [--image PNG\|--image-ref JSON] [--crop x,y,w,h]` | Busca ranqueada em nomes de camadas e texto completo com overrides de instância, propriedades de texto/booleanas e swaps aplicados; referências exatas de nós (incluindo ids de subcamadas `I…;…` para `inspect`), motivos de correspondência, intervalos UTF-16, cobertura e páginas por cursor. Padrão de 20 (1-200); camadas ocultas e páginas internas exigem sua flag. Com `--file`/`--workspace`/`--id` em vez de `<file>`, lê o índice preparado do workspace (veja `index`) `--mode semantic\|hybrid` precisa de um modelo configurado: veja [Busca semântica opcional](#semantic-search). |
| `models list\|status\|configure <profile> --model DIR [--runtime FILE]\|remove <profile>` | Modelo de texto local para a busca semântica (`bge-m3`, experimental) e modelo de imagem para a busca por imagem (`siglip-base`, experimental); `configure` confere os arquivos que você baixou e nunca baixa; `remove` os mantém. Veja [Busca semântica opcional](#semantic-search). |
| `usages <file> <node\|resource> [--scope ID] [--relation R[,R]] [--out] [--snapshot S] [--include-hidden] [--limit N] [--cursor C]` | Relações exatas de uma camada ou recurso (`VariableID:…`, `StyleKey:…`, `image:…`) em um escopo salvo, com instâncias aninhadas e swaps: onde um componente, set, variável, estilo ou imagem é usado (padrão) ou para o que uma camada aponta (`--out`). Cada resultado mantém a referência exata, relação, estado (`resolved`, `missing`, `unresolved`, …), base e evidência; um nome nunca conta como identidade. Sem totais: a cobertura diz completa ou parcial. Padrão 50 (1-200). Com `--file`/`--workspace`/`--id` no lugar de `<file>`, lê o índice preparado de `--scope` do workspace. Com `--out`, um uso de uma cópia de biblioteca em cache também informa sua `library` {kind, namespace, key}. |
| `usages --file\|--workspace\|--id … --library <kind> --namespace <ns> --key <key> [--definition-source ID --definition-snapshot SNAP --definition-node NODE] [--source ID] [--scope ID] [--include-hidden] [--limit N] [--cursor C]` | Camadas que usam um componente, estilo ou variável de biblioteca (`kind`), conforme a cópia em cache de cada arquivo registra, no `--scope` preparado de todas as fontes (`--source` restringe a uma). A definição fica `unresolved`, salvo se as três opções `--definition-*` a selecionarem por referência exata (`selected`, `keyEvidence: unverified`); a cobertura informa o estado de cada fonte e por que é parcial. Tome-a do campo `library` de `usages --out` numa camada que a use. |
| `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]` | O que mudou entre duas revisões salvas: uma linha por frame de nível superior alterado, com contagens por faceta (texto, layout, aparência, propriedades, estrutura, vínculos); `--json` adiciona valores de campo antigos/novos e mudanças de edges. Camadas só correspondem por id e tipo exatos; um frame com o mesmo nome ou posição é apenas um possible match rotulado, e um lado que parou antes, ou uma camada cuja única diferença é um campo que não pôde ser lido, informa unknown, nunca removido nem alterado. Páginas de 100 mudanças (1-500) Um lado que parou num limite imprime `stopped early` em vez de `no changes`; restrinja com `--node`. |
| `inspect <file> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat] [--full-text] [--colors]` | Resumo compacto de layout/texto expandido, no máximo 200 nós. `--images` adiciona dimensões de bitmap e áreas de recorte. `--flat` imprime uma linha por nó: posição relativa ao nó, tamanho, texto, fonte, preenchimento, traço, raio, auto-layout, alinhamento, tamanho fixed/fill, sombras e desfoques `--full-text` devolve cada texto completo (até 10.000 caracteres); `--colors` adiciona os preenchimentos sólidos visíveis (hex, opacidade, variável vinculada), também com `--flat`; sem gradientes, imagens ou traços. A saída JSON inclui `path`, os ancestrais do nó a partir do documento (`[{id, name}]`, como em `find`), também para subcamadas de instâncias. |
| `nodes <file> <node> [--compact [--subtrees]]` | O objeto por nó de uma resposta REST de nodes (`document`, `components`, `componentSets`, `styles`): instâncias e overrides expandidos, auto-layout, estilos de texto, preenchimentos, efeitos, variáveis vinculadas, propriedades de componentes, limites de renderização. REST permanece igual por padrão. `--compact` compartilha estilos repetidos; adicione `--subtrees` para subárvores repetidas, mantendo ID e limites por nó. Veja o [contrato sem perdas](../wiki/discovery.md#compact-nodes). Batch: `--frames` / `--ids` + `--compact [--subtrees]`. |
| `nodes <file> --frames <list>`, `--frames -` (stdin), `--ids <node>...` | Uma decodificação, muitos nós: uma linha JSON por id listado, a saída de `nodes` com seu `nodeId`, ou `{nodeId, error}` |
| `variables <file> [node]`<br>`variables <file.fig> --format dtcg --collection ID --mode ID --out FILE` | Padrão: variáveis vinculadas/locais no formato REST, sem alterações. Exportação DTCG 2025.10: IDs exatos de coleção e modo, sem nó; apenas cores e números sem unidade. Crie primeiro o diretório de saída; qualquer diagnóstico cancela toda a exportação sem alterar o arquivo de saída. Uma coleção de biblioteca é exportada das cópias em cache no arquivo (`libraryCopy: true`). Um alias para outra coleção exporta o valor dessa coleção, registrado em `aliasOf`; se ela tiver vários modos, informe `--alias-mode COLLECTION=MODE`. |
| `bundle <file> <node> <out.zip> [--states] [--hash-source]` | Um bundle de design de um frame; imprime `{nodeId, path, warnings}` |
| `bundle <file> --frames <list> --out-dir <dir> [--jobs N] [--states] [--hash-source]`, `--frames -` (stdin) | Vários bundles a partir de uma decodificação, em paralelo |
| `verify <bundle> [--fonts <dir>]` | Confere um bundle (`.zip` ou diretório extraído) com o manifesto: arquivos verificados, ausentes e alterados, assets ausentes, as fontes a confirmar no projeto e `findings`: cada entrada de handoff ausente, não suportada ou não verificada, com nós, evidência e próxima ação; uma fonte com arquivo nesta máquina ou em `--fonts <dir>` (pasta do projeto, que também gera um `@font-face`) é `local_file`, sem provar que o navegador a carrega |
| `export <file> <node> <out> [--bounds layout] [--scale N] [--quality 1-100] [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID]]` | SVG, PNG, JPG, WebP ou AVIF; PNGs de até 4.194.304 pixels são otimizados automaticamente sem perda, sem alterar os valores RGBA decodificados; os bytes codificados originais são mantidos se o resultado não for menor, e PNGs maiores usam a codificação normal; `--max-kb` reduz a qualidade com perda até caber (PNG falha se ainda exceder o limite, sem alternativa com perda); `--bounds layout` exporta a caixa de layout em vez dos limites de renderização; `--match-dir DIR` (só SVG, com subpastas) lista os SVGs que desenham os mesmos contornos (`matches`) e até 10 formas parecidas com pontuação (`similar`); `--hide ID[,ID]` desenha essas camadas do nó como ocultas na mesma área, então comparar com a exportação normal mostra onde essa camada, como o figlens a desenha, muda o resultado (nenhuma diferença não prova que o que foi pulado é inofensivo) |
| `export <file> --frames <list> --out-dir <dir> [--format F] [--scale N] ...`, `--frames -` (stdin) | Uma decodificação, muitas exportações (padrão `png`): grava `<dir>/<nodeId>.<format>`, uma linha JSON por id, a saída de `export` com seu `nodeId`, ou `{nodeId, error}` |
| `export <file> --batch <items.json>`, `--batch -` (stdin) | Uma decodificação, várias exportações, cada uma com suas opções: um array JSON de 1-64 `{node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}` (MCP `export_batch`). Imprime um manifesto `{count, failed, items}`: por item, a saída de `export` ou `{error}` (os demais são exportados mesmo assim); um item malformado ou um `out` repetido falha antes de qualquer escrita |
| `preview <file> [node] [--out preview.html] [--port N] [--bind IP]` | Viewer local que nunca grava o design; busca semântica/híbrida após Enable index; `--out` grava um snapshot offline independente; resultados e o context copiado indicam a página e, ao vivo, o caminho do arquivo, o snapshot do source carregado e a revision local (desconhecidos offline, onde o caminho nunca é incorporado); o context também traz a coverage do scope de referência, `complete: null` quando não verificada. Visual/Hybrid aceita um PNG de referência ou uma camada selecionada com recorte em pixels. Ative o índice visual explicitamente; imagens de consulta não são armazenadas. |
| `compare <file> <nodes.json> <node>` | Diferenças em relação a uma exportação REST do mesmo frame |
| `compare-image <file> <node> <screenshot.png>` | Compara uma captura da tela construída com o node renderizado na escala dela (`--scale`, `--limit`, `--out diff.png`, `--full-page` para uma captura de página inteira mais curta que o frame, `--ignore` para camadas cujo conteúdo vem de dados, `--threshold` para ver bordas claras; `image: true` marca preenchimentos de imagem; MCP `compare_image`). Imprime `mismatch`, `nonTextMismatch` e as maiores regiões diferentes em pixels do node, cada uma com as camadas mais profundas sob ela; `text: true` indica regiões sobre texto (muitas vezes fonte substituta; revise cada uma) |
| `node <file> <node>` | Um nó `.fig` bruto |
| `clipboard [copied.html] <out.fig>` | Para quando não há um `.fig` salvo: selecione camadas no Figma, copie-as (clique direito > Copy, ou Cmd/Ctrl+C) e execute isto para converter a área de transferência em um `.fig` que todos os comandos leem; colar em outro lugar traz só o texto das camadas. Lê a área de transferência do sistema (macOS; Windows PowerShell 5.1 e Linux `wl-paste`/`xclip` não verificados) ou um arquivo HTML da área de transferência salvo. Mesmos ids de nó, componentes usados incluídos. Informa `missingImages`, `missingComponents` e `layersWithoutOutlines`: uma cópia não traz bytes de imagem nem contornos vetoriais, então exports e bundles avisam onde desenham sem eles. Nunca substitui um arquivo existente. Com um `.fig` salvo, passe o link do Figma como nó: ele mantém imagens e vetores |
| `freshness [<file.fig> [--url URL] \| --sources <mapping.json>]` | Indica se cada cópia salva é mais antiga que o arquivo do Figma online, antes de concluir que falta um frame no design: os designs do `.figlens/sources.json` mais próximo (uma lista de `{name, url, file}`, com `file` relativo à raiz do repositório; um mapeamento de `--sources` fora de `.figlens/` usa a própria pasta), um arquivo, ou um arquivo com sua URL. Compara `exported_at` do `meta.json` da cópia com o `last_touched_at` online, lido com `curl` e um personal access token em `FIGMA_TOKEN` (scope `file_metadata:read`); imprime `stale`, `current` ou `unknown` com um motivo e nunca falha por falta de token ou de rede. Com um token, um nó não encontrado ou um `find` vazio em uma cópia mapeada e desatualizada também avisa para salvar uma nova cópia local (MCP `freshness`) |
| `index prepare\|refresh\|rebuild\|clear\|status\|add\|remove\|relocate\|reset\|track\|untrack\|embed\|visual` | Workspace local no diretório de estado do figlens do usuário, ou em um `--storage DIR` que o workspace passa a lembrar. `prepare <file> --scope NODE` cria uma vez o índice de busca desse escopo; depois `search --file <file>` (ou `--workspace DIR`) responde sem reanalisar, com ranking conjunto das fontes do workspace. `refresh` recria os arquivos alterados, `rebuild` todos ou uma `--source`/`--scope`, `clear` remove índices sem uso; o índice anterior continua servindo até o novo ficar pronto. `reset` remove apenas o banco de dados do workspace, nunca designs, exportações ou o local lembrado. `track --scope NODE` registra as mudanças desse scope a partir de um build baseline; `untrack` para e mantém o histórico. `embed` incorpora os escopos publicados com o modelo configurado (CPU e memória); execute só se quiser busca semântica no workspace. `visual --scope NODE` renderiza as telas e componentes desse escopo publicado para busca por imagem e os incorpora se houver um perfil de visão configurado; execute só se quiser busca visual no workspace. |
| `jobs status\|cancel <jobId>` | Ver ou cancelar a criação de um índice; `cancel` retorna depois que a criação parou |
| `changes --file <file>\|--id ID\|--workspace DIR [--source ID] [--scope NODE] [--consumer NAME\|--subscription ID\|--cursor C] [--revision R] [--limit N]` | Mudanças registradas dos scopes acompanhados, da mais antiga para a mais recente: uma revisão por build publicado (baseline ou changes) com cobertura e contagens por tipo de mudança e faceta; cada revisão changes tem um resumo `impact` das camadas que suas mudanças alcançam (null antes do schema 7); `--revision` pagina os nodes alterados dessa revisão e depois as camadas que seu impacto lista (`entry` diz qual); `tracking` informa quando um scope precisa de `index refresh` para registrar uma mudança. Somente leitura |
| `watch ack --file <file>\|--id ID\|--workspace DIR --consumer NAME\|--subscription ID --cursor C [--source ID] [--scope NODE]` | Move o checkpoint de um consumidor para o `nextCursor` da última página de `changes` processada com os mesmos filtros; repetir é seguro e ele só avança, exceto que um cursor de recuperação atual volta um checkpoint que ficou atrás de uma exclusão para reler as revisões mantidas; um cursor emitido antes de a retenção apagar revisões que ele não tinha passado é `event_gap` e não grava nada |
| `watch run --file <file>\|--id ID\|--workspace DIR --scope NODE [--source ID] [--request-id R]` | Mantém em primeiro plano um lease sobre um scope rastreado: o runner compartilhado do workspace o atualiza a cada salvamento (consulta a cada 250 ms, verificação de conteúdo a cada 60 s), então `changes` mostra novas revisões sem `index refresh`. Imprime o lease e depois uma linha final quando o stdin fecha, com Ctrl-C ou SIGTERM, ou quando o scope deixa de ser rastreado. MCP `watch` `start` e `stop` mantêm leases enquanto o servidor roda; um lease não renovado termina após 30 s e o runner sai depois do último |
| `watch status --file <file>\|--id ID\|--workspace DIR` | Os leases do workspace (`active`, `unauthorized`, `untracked`, `expired`, `ended`) e seu runner (`running`, `stopping`, `stopped`, `unknown`, com o posicionamento); nunca escreve |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]` | Acompanha uma fonte, um scope ou uma camada exata (`--node`) a partir da revisão mais recente com checkpoint próprio, lido com `changes --subscription` e confirmado com `watch ack --subscription`; uma camada ausente ou ambígua é `target_missing` ou `identity_uncertain`, nunca outra de mesmo nome; `--owner` a torna temporária para essa tarefa |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID --query TEXT [--type T[,T]] [--scope NODE]` | Salva uma busca no scope de uma fonte: seus 50 primeiros são avaliados na hora; `changes --subscription` lê as avaliações salvas e nunca calcula uma |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --sources visible\|ID[,ID] --query TEXT\|--library KIND --key KEY [--namespace NS] [--definition-source ID --definition-snapshot SNAP --definition-node NODE]` | Salva uma busca, ou os usos de um item de biblioteca com seu `definitionMatch`, sobre um conjunto fixo de até 10 fontes (`visible` é resolvido agora; fontes adicionadas depois nunca entram): `delta.changedSourceIds` nomeia as fontes com bytes novos, `corpus_changed` uma mudança do conjunto, `coverage.transitive` é `not_followed` e `delta.notFollowedSourceIds` nomeia as fontes alteradas sem usos, cujos dependentes não foram reverificados; remover um membro a desativa (unsubscribe e depois subscribe de novo) |
| `watch evaluate --file <file>\|--id ID\|--workspace DIR --subscription ID` | Salva a avaliação de uma assinatura de consulta sobre o índice atual do scope, a menos que já esteja avaliada; um watch no scope faz isso após cada publicação |
| `watch list --file <file>\|--id ID\|--workspace DIR [--consumer NAME] [--after ID] [--subscription ID]` | Lista as assinaturas visíveis e seu estado, uma página por vez; `--subscription` lê só aquela (vazio quando não visível) |
| `watch unsubscribe --file <file>\|--id ID\|--workspace DIR --subscription ID\|--consumer C --owner NAME` | Remove uma assinatura e seu checkpoint, ou as temporárias de um owner; o tracking e o histórico permanecem |
| `watch forget --file <file>\|--id ID\|--workspace DIR --consumer NAME [--source ID] [--scope NODE]` | Remove os checkpoints avulsos de um consumer; um consumer mantém no máximo 64 assinaturas e checkpoints e um workspace 4096, e nada é removido automaticamente |
| `mcp` | Servidor MCP local por stdio |
| `update` | Substitui o binário pela release mais recente |

Para variantes de componentes relacionadas, use `inspect --states` ou `bundle --states` (MCP: `states: true`, false por padrão; sem alterar a configuração). As evidências preservam origem, valores desconhecidos e marcações de truncamento. Os bundles adicionam `design/states.json` e um resumo em `context.md`. Consulte o escopo e os limites em [evidências de estados](../wiki/states.md).

Compare duas variantes do mesmo component set com `inspect <file> <variantA> --compare-variant <variantB>` (MCP `compareVariant`). `variantComparison` mostra diferenças dos campos e conjuntos de valores armazenados, com IDs de origem e valores desconhecidos, Nós filhos são pareados por nome único, depois por componente principal e depois por ordem de empilhamento em grupos de mesmo nome e tamanho (`pairedNodes`, com mudanças por par; confira `pairedBy`, `order` é a evidência mais fraca; o resto fica sem par), e variáveis vinculadas são nomeadas. Consulte os [limites](../wiki/states.md#compare-two-variants).

Para comparar dois nós quaisquer do mesmo arquivo (tela vazia e preenchida, breakpoints mobile e desktop, duas telas com o mesmo cabeçalho), use `inspect <file> <nodeA> --compare <nodeB>` (MCP `compare`). `comparison` tem o mesmo formato mais `rootDelta` (tamanho e posição relativa ao pai); os filhos são pareados por nome único, depois por componente principal e depois por ordem de empilhamento em grupos de mesmo nome e tamanho (`pairedBy` informa qual), o resto fica sem par com `reason: ambiguous-name`, as instâncias não são expandidas e um nó não pode ser comparado com seu próprio ancestral. `--compare-variant` continua exigindo o mesmo component set.

Um argumento de nó aceita `12:345`, `12-345` ou uma URL do Figma com `?node-id=`. `<file>` também pode ser `gs://bucket/object`, baixado com seu login do `gcloud` e armazenado em cache. Consulte o [guia de snapshots do GCS](../wiki/gcs.md) para a troca por um objeto fixo, as permissões necessárias e os limites de atualização. Cada chamada da CLI decodifica o arquivo; MCP reutiliza apenas o último arquivo local sem alterações. Chamadas remotas analisam o arquivo novamente. Execute `figlens --help` para ver o uso completo da CLI, ou `figlens inspect --help` (também `-h`) para a entrada de um comando. `index add gs://bucket/object` registra uma cópia fixada dessa geração e informa `remote {uri, generation}`; só add executa o gcloud.

`inspect` inclui redimensionamento do texto, truncamento, máximo de linhas e decoração registrados. As anotações do Dev Mode aparecem nos seus nós como `annotations` (prévia de `label` em texto simples, `properties` fixadas, `categoryId`; `nodes` acrescenta `labelMarkdown`) com as `annotationCategories` nomeadas; frames e seções Ready for dev trazem `devStatus`, e `--flat` marca ambos; as medições são informadas em `handoffDiagnostics`. Um pai sem auto-layout traz `childSpacing: "derived"`: os espaços entre os filhos são diferenças de coordenadas, não uma regra de auto-layout; `constraints` ainda pode fixá-los. `handoffDiagnostics` informa semântica salva, mas não emitida, com contagens, exemplos e [limites de cobertura](../wiki/semantics.md#known-handoff-gaps). `_figlens.semantics` lista apenas os campos cuja origem está salva (uma entrada ausente significa que não está), e cada resposta informa uma única vez, em `_figlens.semantics.verification` no nível superior, que esses mapeamentos não foram comparados com o Figma. Frames com grid auto layout trazem o número de linhas e colunas, os espaçamentos e os tamanhos de faixa salvos (`_figlens.gridTracks`). As saídas protegem a origem e substituem o destino após a escrita completa. MCP limita as linhas de requisição a 1 MiB e as respostas completas a 8 MiB; exceder o limite gera um erro explícito. O JSON completo continua disponível pela CLI.
