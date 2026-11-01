<p align="center">
  <img src="https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/logo.png" alt="figlens" width="220">
</p>

<h1 align="center">Tu coding agent lee archivos de Figma.<br>Sin app de Figma. Sin login. Sin cuota de API.</h1>

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
  <a href="#start">Empezar</a> · <a href="#demo">Demo</a> · <a href="#why">Por qué</a> · <a href="#bundle">Qué recibe el agente</a> · <a href="#pi">Pi</a> · <a href="#limitations">Limitaciones</a>
</p>

**El problema.** Tu agente necesita valores de layout, texto y recursos para implementar una pantalla. En un servidor remoto o en CI, ese contexto debe estar disponible junto al código sin depender de una aplicación de escritorio en ejecución.

**La solución.** En Figma, **File > Save local copy**. Coloca `design.fig` junto a tu código. figlens lo analiza en local y le da a tu agent layout, texto, estilos, variables, iconos, imágenes y un render de referencia de cualquier frame. Un binario nativo, cero llamadas de red a Figma.

<a id="start"></a>

## Primeros pasos

**¿Aún no tienes un archivo de diseño?** [Comprueba tu entorno con el pequeño ejemplo público](examples/first-success/README.md): verifica el binario elegido, encuentra un frame y valida su bundle. No se necesita un modelo.

Pon `design.fig` en tu proyecto y elige **una** configuración. Cada una le da a tu agent las herramientas de figlens y la misma guía integrada para planificar, implementar y preparar imágenes.

| Configuración | Para | La guía viene de |
|---|---|---|
| [Plugin](#setup-plugin) | Claude Code, Codex, Antigravity, Cursor, Kimi Code | el skill del plugin y la herramienta `workflow` del servidor |
| [MCP](#setup-mcp) | Claude Code, Cursor, Codex, Gemini CLI y otros clientes MCP | la herramienta `workflow` del servidor |
| [Pi](#setup-pi) | Pi 1.3.0+ | el paquete, que incluye el skill |
| [OpenCode V2](#setup-opencode) | OpenCode V2 2.2.0 | el skill incluido en el paquete |
| [OMP](#setup-omp) | OMP 18.13.2 | el skill incluido en el paquete |
| [CLI](#setup-cli) | agents que solo ejecutan comandos de shell, scripts y CI, o tú | `figlens workflow <topic>`, o el [skill](#setup-skill) opcional |

**Ordinario vs. avanzado.** El uso ordinario no requiere configuración: `figlens search design.fig "query"` lee el `.fig` y no crea nada. Un índice local opcional solo se crea cuando lo pides (`figlens index prepare design.fig --scope 0:0`) y luego se reutiliza entre reinicios y hosts. Consulta [Búsqueda ordinaria e índice local opcional](docs/README-es.md#local-index). Opcional y experimental: `--mode semantic` o `hybrid` ordena por significado con un modelo local que configuras tú; consulta [Búsqueda semántica opcional](docs/README-es.md#semantic-search).

<a id="setup-plugin"></a>

### Plugin

Una sola instalación añade el skill de figlens y el servidor MCP a tu agente (Node.js 18+). El servidor queda fijado a la versión del plugin (`npx -y @runkids/figlens@<version> mcp`); actualiza el plugin para pasar a una versión nueva.

<!-- x-release-please-start-version -->
| Host | Instalación |
|---|---|
| Claude Code | `claude plugin marketplace add runkids/figlens`, `claude plugin install figlens@figlens` |
| Codex | `codex plugin marketplace add runkids/figlens`, `codex plugin add figlens@figlens` |
| Antigravity | `agy plugin install https://github.com/runkids/figlens/tree/v0.11.0` |
| Cursor | `git clone --depth 1 --branch v0.11.0 https://github.com/runkids/figlens ~/.cursor/plugins/local/figlens` y recarga la ventana |
| Kimi Code | `/plugins install https://github.com/runkids/figlens/tree/v0.11.0` en una sesión y luego `/reload` |
<!-- x-release-please-end -->

Reinicia el host o abre una sesión nueva y comprueba que aparecen las herramientas de figlens. La instalación de un plugin copia el repositorio: unos 7 MB, más el historial de Git (unos 60 MB) en Codex. Antigravity carga solo los primeros 24 KB del skill; la herramienta `workflow` del servidor ofrece la guía completa. Muse aún no está soportado. [Tabla de hosts y qué se verificó](wiki/host-plugins.md)

<a id="setup-mcp"></a>

### MCP

**Claude Code** (Node.js 18+, macOS/Linux):

<!-- x-release-please-start-version -->
```sh
claude mcp add --transport stdio --scope local figlens -- npx -y @runkids/figlens@0.11.0 mcp
```
<!-- x-release-please-end -->

**Codex CLI** (Node.js 18+, macOS/Linux):

<!-- x-release-please-start-version -->
```sh
codex mcp add figlens -- npx -y @runkids/figlens@0.11.0 mcp
```
<!-- x-release-please-end -->

**Cualquier cliente MCP:**

<!-- x-release-please-start-version -->
```json
{
  "mcpServers": {
    "figlens": {
      "command": "npx",
      "args": ["-y", "@runkids/figlens@0.11.0", "mcp"]
    }
  }
}
```
<!-- x-release-please-end -->

Sin Node.js, instala el binario independiente ([CLI](#setup-cli)) y usa `"command": "figlens", "args": ["mcp"]`. Abre una sesión nueva y comprueba que aparecen las herramientas de figlens (`/mcp` en Claude Code). No hace falta instalar el skill por separado. Añade `--root <dir>` después de `mcp` para limitar qué carpetas puede leer y escribir figlens.

<a id="mcp-args"></a>

#### Argumentos de inicio MCP

Añade estas opciones después de `mcp` en los `args` del cliente. Son las únicas opciones de inicio MCP; ambas son opcionales y repetibles.

| Opción | Significado |
|---|---|
| `--root DIR` | Permite leer diseños y exportar dentro de `DIR` y sus subcarpetas. Sin ella, figlens no añade límites de ruta; siguen aplicándose los permisos del sistema. No se limita al proyecto actual ni escanea el equipo. |
| `--storage-root DIR` | Permite solicitar explícitamente almacenamiento de índices personalizado dentro de `DIR` y sus subcarpetas. No selecciona ni cambia la ubicación predeterminada. |

Usa rutas absolutas a directorios existentes. Cada opción y ruta es un elemento del array; repite la opción para varias carpetas. `--root` es un límite de rutas, no acceso de administrador, y solo restringe figlens MCP, no otras herramientas del agent.

Ejemplo con dos raíces de diseño/salida y dos raíces de almacenamiento permitidas:

<!-- x-release-please-start-version -->
```json
{
  "mcpServers": {
    "figlens": {
      "command": "npx",
      "args": [
        "-y", "@runkids/figlens@0.11.0", "mcp",
        "--root", "/absolute/designs",
        "--root", "/absolute/project",
        "--storage-root", "/absolute/storage-a",
        "--storage-root", "/absolute/storage-b"
      ]
    }
  }
}
```
<!-- x-release-please-end -->

No existe `figlens mcp --storage`. Selecciona la ubicación real con el argumento `storage` del tool `index` (ruta absoluta dentro de una raíz permitida), o con CLI: `figlens index prepare design.fig --storage /absolute/storage-a`. Tras `prepare`, figlens recuerda la ubicación del workspace y la reutiliza entre reinicios y hosts sin repetir `storage` ni `--storage-root`. Sin ubicación personalizada se usan los directorios predeterminados del usuario, sin flags de almacenamiento.

Para no fijar estas rutas en una configuración versionada, o para compartir diseños entre worktrees de Git, consulta [roots en una configuración versionada](wiki/mcp.md#roots-in-a-checked-in-config).

Varias raíces de almacenamiento solo amplían las ubicaciones permitidas: cada `storage` elige una sola. No distribuyen, sincronizan ni respaldan índices. No se puede mover una base de datos existente indicando otra raíz; figlens rechaza la ubicación en conflicto.

<a id="setup-pi"></a>

### Pi

Un solo paquete incluye la CLI, MCP, el viewer, los comandos `/figlens` y el skill:

```sh
pi install npm:@runkids/figlens
```

[Guía de la extensión de Pi](docs/README-es.md#pi)

<a id="setup-opencode"></a>

### OpenCode V2

Instala el paquete en OpenCode V2:

```sh
opencode plugin add @runkids/figlens
```

Reinicia el cliente y ejecuta `/figlens help`. [OpenCode V2](opencode/README.md)

<a id="setup-omp"></a>

### OMP

Instala el mismo paquete en OMP:

```sh
omp plugin install @runkids/figlens
```

Reinicia el cliente y ejecuta `/figlens help`. [OMP](omp/README.md)

<a id="setup-cli"></a>

### CLI

Ejecútalo con npm o instala el binario independiente:

```sh
npx @runkids/figlens --help                                                                # Node.js 18+, sin instalar
curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh | sh  # macOS, Linux
brew install runkids/tap/figlens                                                           # Homebrew
irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 | iex      # Windows PowerShell
```

`npx` ejecuta el binario de la versión del paquete que resuelve. Las instalaciones por script se actualizan con `figlens update`; las de Homebrew, con `brew upgrade figlens`. Hay binarios para macOS (Apple silicon, Intel), Linux (arm64, x86_64) y Windows (x86_64).

Un agent que usa la CLI lee su guía con `figlens workflow build` (también `audit`, `images`, `search`, `plan`, `planning-details` y `yolo`, que solo se usa si se pide explícitamente), siempre acorde con la versión del binario, o desde el [skill](#setup-skill).

[Referencia de la CLI](docs/README-es.md#commands)

<a id="setup-skill"></a>

### Skill para agents (opcional)

El skill enseña a un agent el mismo flujo que la guía integrada: encontrar el frame, leer el bundle por partes, planificar el alcance y comparar la UI en ejecución con el diseño. Solo contiene instrucciones, así que combínalo con la CLI o MCP. Las configuraciones MCP y Pi no lo necesitan.

**Recomendado: [skillshare](https://github.com/runkids/skillshare).** Instala el skill una vez y sincronízalo con cada agent que uses:

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare sync
```

La misma herramienta también puede configurar la conexión MCP de figlens para esos agents:

<!-- x-release-please-start-version -->
```sh
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens@0.11.0 mcp
skillshare sync mcp --global
```
<!-- x-release-please-end -->

Otras formas de instalar el skill:

```sh
npx skills add runkids/figlens --skill figlens   # Skills CLI
```

O copia `skills/figlens/` de este repositorio o de un archivo de release al directorio de skills de tu agent (Claude Code: `~/.claude/skills/`). Un skill instalado se actualiza por separado del binario; `figlens workflow` siempre coincide con el binario.

Luego pide una pantalla:

```text
Build the task board from ./design.fig in this project.
Find the matching frame, use this project's components and image workflow,
then compare the running UI with the design and fix confirmed differences.
Report missing assets and any checks you could not run.
```

El agent encuentra el frame, reúne los datos del diseño, los recursos y una imagen de referencia, y construye la pantalla con tu stack. El flujo compartido guía la comprobación de fuentes y recursos; informa de lo que falta mientras continúa la implementación del layout con alternativas documentadas. Pídele los archivos modificados y una comparación en el navegador.

<a id="demo"></a>

## Demo: Pi construye una pantalla React desde un .fig

[![Pi plans from a local .fig, builds a React screen and checks it beside a live browser](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/taskflow-demo.gif)](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4)

El GIF en bucle reproduce el vídeo editado a triple velocidad. Haz clic para ver el vídeo completo de 78 segundos.

**[Video de 78 segundos](https://github.com/runkids/figlens/blob/v0.10.1/assets/taskflow-demo.mp4).** Pi ejecuta `/figlens plan`, lee un frame de dashboard de un `.fig` local, lo construye con React y Tailwind CSS, y luego revisa y corrige la UI en ejecución. Time-lapse editado, no un benchmark ni una promesa de fidelidad pixel-perfect. Diseño: [Task Flow by Eymen Eid](https://www.figma.com/community/file/1224268866120711793) · [notas de la grabación](wiki/accuracy.md#taskflow-react-and-tailwind-demo).

**[De .fig a HTML/CSS editable](examples/task-list/README.md):** en un estudio controlado de una lista de tareas, los seis intentos basados en figlens conservaron las 19 apariciones de texto visible como texto DOM real y los cuatro iconos. La extracción por CLI y MCP también pasó con la red de Docker desactivada. El caso incluye configuración, mediciones y comparaciones de imágenes independientes.

<a id="why"></a>

## ¿Por qué figlens en lugar del MCP oficial de Figma?

Usa **figlens** cuando el diseño deba viajar como archivo: agents remotos, CI, trabajo sin conexión, traspaso entre agents, lecturas repetidas sin límite. Usa el **MCP oficial de Figma** cuando el agent deba ver el diseño en vivo sin un paso de exportación.

Leer un `.fig` funciona sin conexión. Instalar y actualizar figlens, las entradas `gs://`, tu proveedor de modelos y las dependencias de tu proyecto sí necesitan red.

[Comparación completa](docs/README-es.md#why)

<a id="bundle"></a>

## Qué recibe el agent

Un frame, un zip. Estructura para construir, recursos para usar, una imagen para comprobar.

[Contenido del bundle y ejemplos](docs/README-es.md#bundle) · [¿Qué tan cerca está de Figma?](docs/README-es.md#accuracy)

## Documentación

- <a id="pi"></a>[Extensión de Pi](docs/README-es.md#pi)
- <a id="viewer"></a>[Explora un diseño sin Figma](docs/README-es.md#viewer)
- <a id="mcp-and-skill"></a>[Servidor MCP y skill para agents](docs/README-es.md#mcp-and-skill)
- <a id="commands"></a>[Referencia de la CLI](docs/README-es.md#commands)

<a id="limitations"></a>

## Limitaciones

- **Snapshots, no sincronización en vivo.** Exporta un nuevo `.fig` cuando cambie el diseño. figlens no puede descargar un diseño desde una URL de Figma.
- **Formato no documentado.** figlens decodifica el esquema incrustado en el archivo; las nuevas exportaciones de Figma pueden requerir una actualización de figlens.
- **Fuentes y texto:** no se incluyen archivos de fuentes; el texto con overrides puede conservar el layout guardado de Figma. Usa fuentes autorizadas del proyecto y compara el resultado.
- **Cobertura:** las [mejoras de renderizado](https://github.com/runkids/figlens/issues/36) y la [semántica de entrega](https://github.com/runkids/figlens/issues/37) se siguen en el gestor de issues. Consulta la [cobertura semántica actual](wiki/semantics.md).
- **Revisa la salida.** Lee los `warnings` del bundle, `missing-assets.json` y las variables sin resolver en `context.md`, y luego compara la implementación con el diseño.
- Las limitaciones visibles conocidas aparecen en `_figlens.renderDiagnostics` de los nodos, los resúmenes limitados de `inspect` y los `warnings` del bundle y de `export`; `missing-assets.json` solo contiene fallos de recursos. La ausencia de advertencias no garantiza fidelidad de píxeles.

## Ayuda a que figlens crezca

Si figlens convirtió un diseño en código para ti, [dale una estrella al repo](https://github.com/runkids/figlens) para que otros lo encuentren. ¿Encontraste un archivo que no puede leer? [Abre un issue](https://github.com/runkids/figlens/issues) con el comando y lo que esperabas.

El desarrollo ejecuta Rust solo en Docker: `docker compose run --rm dev cargo test`. Consulta [AGENTS.md](AGENTS.md) y el [wiki](wiki/README.md).

## Licencia

[MIT](LICENSE). figlens es un proyecto independiente, sin afiliación, respaldo ni patrocinio de Figma. El formato `.fig` no está documentado y puede cambiar; usa solo archivos y recursos a los que tengas permiso de acceder y compartir.
