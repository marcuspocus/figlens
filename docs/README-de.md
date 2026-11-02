# figlens — Benutzerhandbuch

[← README](../README-de.md) · [figlens.runkids.cc](https://figlens.runkids.cc/de/docs/)

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

- [Eigenständige Installation](#install)
- [Warum figlens statt des offiziellen Figma MCP?](#why)
- [Was der Agent bekommt](#bundle)
- [Pi-Extension](#pi)
- [Ein Design ohne Figma durchsuchen](#viewer)
- [MCP-Server und Agent-Skill](#mcp-and-skill)
- [CLI-Referenz](#commands)

<a id="install"></a>

## Eigenständige Installation

**Kein Node.js?** Installiere das eigenständige Binary und verwende stattdessen `"command": "figlens", "args": ["mcp"]`:

| Plattform | Befehl |
|---|---|
| macOS / Linux | `curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh \| sh` |
| Homebrew | `brew install runkids/tap/figlens` |
| Windows (PowerShell) | `irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 \| iex` |

<a id="why"></a>

## Warum figlens statt des offiziellen Figma MCP?

Nutze **figlens**, wenn das Design als Datei wandern soll: Remote-Agents, CI, Offline-Arbeit, Übergabe zwischen Agents, unbegrenzt wiederholte Lesezugriffe. Nutze den **offiziellen Figma MCP**, wenn der Agent das Live-Design ohne Export-Schritt sehen muss.

| | figlens | Offizieller Figma MCP |
|---|---|---|
| Quelle | Exportierter `.fig`-Snapshot | Live-Figma-Dateien |
| Voraussetzung | Die Datei | Figma-Anmeldung; der Desktop-Server braucht die App |
| Rate Limits | Keine, lokales Parsen | Limits des Figma-Dienstes |
| Remote-Agent / CI | Datei kopieren, Binary ausführen | Remote-Server mit Anmeldung |
| Design-Änderungen | Neue `.fig` exportieren | Automatisch |
| Übergabe | `.fig` oder Frame-Bundle teilen | Figma-Zugriff teilen |
| Genauigkeit | Unabhängiger Parser mit [bekannten Lücken](../README-de.md#limitations) | Von Figma gepflegt |

Eine `.fig` zu lesen funktioniert offline. Installieren und Aktualisieren von figlens, `gs://`-Eingaben, dein Modellanbieter und die Abhängigkeiten deines Projekts brauchen weiterhin ein Netzwerk.

<a id="bundle"></a>

## Was der Agent bekommt

Ein Frame, eine Zip. Struktur zum Bauen, Assets zum Verwenden, ein Bild zum Abgleichen.

```text
frame.zip
├── context.md                Start here: instructions for your agent
├── design/
│   ├── nodes.json            Layout, text, styles and expanded instances
│   ├── fonts.json            Font faces the visible text uses, with node ids
│   └── variables.json        Bound variables, collections, modes and aliases
├── assets/                   Image fills, SVGs and composited node renders
├── assets.json               Maps design references to asset files
├── references/screen.png     A render of the frame to compare against
├── missing-assets.json       Anything that could not be produced
└── manifest.json             File roles, media types, sizes and SHA-256 hashes; the source .fig hash with --hash-source
```

Brauchst du nur ein Teil? Die CLI erledigt das direkt:

```sh
npx -y @runkids/figlens find design.fig "login" --type FRAME     # Frame-IDs nach Name oder Text
npx -y @runkids/figlens inspect design.fig 12:345                 # kompakte Layout- und Textzusammenfassung
npx -y @runkids/figlens nodes design.fig 12:345 > frame.json      # JSON im REST-Format
npx -y @runkids/figlens export design.fig 12:345 icon.svg         # SVG, PNG, JPG, WebP oder AVIF
npx -y @runkids/figlens bundle design.fig 12:345 frame.zip        # die Zip von oben
```

SVG-Exporte betten originale Bildfüllungen ein und wandeln Text in Glyphen-Umrisse um, daher werden keine Schriften benötigt. Über MCP kommen Raster-Exporte außerdem als Inline-Bilder zurück, sodass der Agent das Design sieht, ohne eine Datei zu öffnen.

<a id="accuracy"></a>

### Wie nah ist es an Figma?

Gemessen an Figmas eigenen JSON-, PNG- und SVG-Exporten für **826 Produktions-Frames** und fünf Community-Dateien (2026-10-22):

| Prüfung | Ergebnis |
|---|---|
| Knotenstruktur, Namen, Sichtbarkeit, Bounds und Text | Identisch in 771 / 826 Frames |
| Originale Bildfüllungen | 2.608 / 2.608 byte-identisch |
| SVG-Assets | 5.014 / 5.197 innerhalb des Pixelabweichungs-Schwellenwerts |
| Community-Frame-Bundles | 306 / 306 ohne fehlende Assets |

Ergebnisse für diese Dateien, keine Garantie für jedes Design. Methode und Schwellenwerte: [Genauigkeitsbericht](../wiki/accuracy.md).

<a id="pi"></a>

## Pi-Extension

```sh
pi install npm:@runkids/figlens
```

Installationsanleitungen: [OpenCode V2](../opencode/README.md) · [OMP](../omp/README.md).

Starte Pi neu oder führe `/reload` aus und bitte dann in normalen Worten um einen Bildschirm. Pi findet den Frame, liest ihn über den automatisch registrierten MCP-Server und baut mit deinem Stack. Keine Node-IDs, keine globale CLI.

Eine Frage nach Text oder einem Screen durchsucht die `.fig` direkt und speichert nichts. Für wiederholte Suchen bitte Pi, für ein Design und einen Bereich einen Index zu aktivieren: Es bereitet einmal einen lokalen Index vor (ohne Setup-Befehl), meldet den Job, bis er fertig ist, und nutzt ihn nach Neustarts und in anderen Hosts weiter. Den Index zurückzusetzen erfordert deine ausdrückliche Anweisung. Die Frage, wo eine Komponente, Variable, ein Style oder Bild verwendet wird, listet die exakten Usages auf dieselbe Weise: direkt aus der `.fig` oder, sobald aktiviert, aus diesem Index. Bittest du Pi, den Änderungen eines Designs zu folgen, verfolgt es diesen Bereich (und bereitet den Index bei Bedarf vor) und meldet, was sich seit der letzten Prüfung geändert hat; eine Frage zu zwei gespeicherten Kopien beantwortet `diff`, ohne etwas zu starten. Details: [lokaler Index](#local-index).

- `/figlens plan <request>` erstellt nur einen Plan und ändert die Anwendung nicht.
- `/figlens yolo <request>` plant, baut, prüft und räumt den eigenen temporären Plan, Arbeitsexporte und gestartete Prozesse auf.
- `/figlens preview [description]` öffnet den lokalen Viewer.
- `/figlens review` genehmigt wiederverwendbare Komponenten-Zuordnungen, die der Agent vorgeschlagen hat.

![Bitte Pi um einen Bildschirm, lass es das Design finden, mit deinem Stack bauen und das Ergebnis prüfen](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/pi-workflow.svg)

Vollständige Anleitung, Befehle und Grenzen: [pi/README.md](../pi/README.md).

<a id="viewer"></a>

## Ein Design ohne Figma durchsuchen

[![Das MCP-Apps-Design lokal inspizieren, mit Layer-Baum, Layout und Komponenten-Eigenschaften](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/preview-inspector.png)](https://github.com/runkids/figlens/blob/v0.10.1/assets/pi-demo.mp4)

```sh
npx -y @runkids/figlens preview design.fig                        # lokaler Viewer, Ctrl+C zum Beenden
npx -y @runkids/figlens preview design.fig 12:345 --out preview.html   # Offline-HTML-Snapshot zum Weitergeben an jeden
```

Durchsuche Seiten und Layer, suche, nutze die gerankte Suche und exakte Verwendungen der Auswahl, miss, inspiziere Layout, Typografie, Variablen und CSS, exportiere jeden Layer, kopiere einen Implementierungs-Prompt oder lade ein Bundle für die Auswahl herunter. Stelle die gerankte Suche mit einem optionalen lokalen Modell auf semantisch oder hybrid um: Der Viewer indexiert die Datei erst, wenn du auf Enable index klickst. Lege einen Screenshot deines Builds über das Design, um beide zu überlagern. Details: [Preview-Anleitung](../wiki/preview.md). Visual/Hybrid nimmt ein Referenz-PNG oder einen ausgewählten Layer mit Pixelzuschnitt an. Visuelle Indexierung wird ausdrücklich aktiviert; Suchbilder werden nicht gespeichert.

<a id="mcp-and-skill"></a>

## MCP-Server und Agent-Skill

**MCP.** Der Server läuft lokal über stdio und stellt `workflow`, `tree`, `find`, `search`, `usages`, `diff`, `inspect`, `nodes`, `variables`, `bundle`, `verify`, `index`, `jobs`, `changes`, `watch`, `export`, `export_batch`, `compare_image` und `models` bereit. `export_batch` schreibt bis zu 64 Dateien in einem Aufruf und liefert ein Manifest statt Inline-Bildern. Übergib absolute Pfade. Mit `--root /path/to/designs` (wiederholbar) beschränkst du jeden Datei- und Ausgabepfad auf ausgewählte Ordner. Explizit gewählter eigener Index-Speicher braucht `--storage-root DIR` (wiederholbar); gespeicherte Workspace-Orte werden ohne diese Option wiederverwendet. Siehe [Startargumente und Konfigurationsbeispiel mit mehreren Wurzeln](../README-de.md#mcp-args). Raster-Exporte werden auf `maxSize` (Standard 1600 px) verkleinert, damit Inline-Bilder klein bleiben; SVG liefert nur Metadaten.

Über `workflow` kann der Agent die integrierten Anleitungen `build`, `audit`, `images`, `search`, `plan`, `planning-details` oder das ausdrücklich angeforderte `yolo` lesen. Sie stammen aus denselben Quellen wie der portable Skill; dessen separate Installation ist für MCP optional. Die Umsetzung erfolgt mit den Projekt- und Browser-Werkzeugen des Agents und den Berechtigungen des Hosts. Das Lesen einer Anleitung führt den Workflow nicht aus; Client-Unterstützung und Modellverhalten bestimmen, wie sie genutzt wird.

Natives Windows mit Claude Code: folge dessen [stdio-Setup-Hinweisen](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server) und dem `cmd /c`-Wrapper für `npx`.

**Skill.** Der [figlens-Skill](../skills/figlens/SKILL.md) bringt einem Agent bei, Frames zu finden, das Bundle in kleinen Teilen zu lesen, begrenzte Arbeit zu planen und die laufende UI gegen das Design zu prüfen. Er installiert nur Anweisungen; die CLI oder der MCP-Server muss verfügbar sein. Installiere ihn mit der [Skills CLI](https://github.com/vercel-labs/skills) oder kopiere `skills/figlens/` in das Skills-Verzeichnis deines Agents (Claude Code: `~/.claude/skills/`):

```sh
npm install -g @runkids/figlens
npx skills add runkids/figlens --skill figlens
```

**Plugins.** Claude Code, Codex, Antigravity, Cursor und Kimi Code installieren Skill und MCP-Server zusammen als ein Plugin; siehe [Plugin-Einrichtung](../README-de.md#setup-plugin).

**skillshare.** Verwalte den Skill und die MCP-Verbindung für Claude Code, Codex und Cursor zentral mit [skillshare](https://github.com/runkids/skillshare):

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync && skillshare sync mcp --global
```

**Installationshinweise.** Der npm-Launcher lädt beim ersten Aufruf das passende native Binary herunter, prüft es gegen die SHA-256-Prüfsummen des Releases und legt es im Cache ab. Skript-Installationen aktualisierst du mit `figlens update`, Homebrew-Installationen mit `brew upgrade figlens`. Die Binaries decken macOS (Apple Silicon, Intel), Linux (arm64, x86_64, statisch mit musl) und Windows (x86_64) ab. Jeder Installationsweg legt außerdem `figlens-embed`, die Laufzeit für die [optionale semantische Suche](#semantic-search), neben `figlens`; sie lädt kein Modell, bis du eines konfigurierst.

<a id="local-index"></a>

## Normale Suche und der optionale lokale Index

|  | Normal (Standard) | Fortgeschritten: lokaler Index (optional) |
|---|---|---|
| Was es tut | Liest die `.fig` bei jedem Aufruf und beantwortet `search`, `find`, `inspect` und `export`. | Baut einmal einen Suchindex für einen gewählten Bereich und beantwortet `search` daraus, gerankt über Quellen. |
| Was geschrieben wird | Kein Workspace und keine Index-Datenbank (`export` schreibt nur die angeforderten Dateien). | Ein lokaler Workspace (SQLite-Datenbank) im Statusverzeichnis des Benutzers oder unter dem gewählten `--storage`. |
| Wann sinnvoll | Einzelfragen, Skripte, CI. | Wiederholte Suchen in einer Datei; mehrere Dateien eines Projekts als ein Korpus (`--workspace <dir>`); exakte Verwendungen von Komponenten, Variablen, Styles und Bildern. |

Eine normale Designfrage legt nie einen Index an. Sie aktivieren ihn einmal ausdrücklich („Lege einen Index für dieses Design an“); figlens wählt den Speicherort und braucht kein SQLite oder sonstiges Setup.

```sh
# Normal: zustandslos, legt nichts an
figlens search design.fig "checkout button"

# Fortgeschritten, einmalig: Bereich wählen (Node-ID oder 0:0 für die ganze Datei); der Build läuft im Vordergrund
figlens index prepare design.fig --scope 0:0

# Indexierte Suche; --node ergänzen, wenn der indexierte Bereich nicht die ganze Datei ist
figlens search --file design.fig "checkout button" --node 0:0

# Nach Änderung oder neuem Export der .fig: nur Bereiche mit geänderten Bytes neu bauen
figlens index refresh --file design.fig

# Nach dem Verschieben der .fig: Workspace behalten, dann refresh (workspaceId aus der Ausgabe von prepare)
figlens index relocate <sourceId> new/design.fig --id <workspaceId>
figlens index refresh --id <workspaceId>
```

MCP: `index` prepare liefert einen Job; `jobs` abfragen, bis `succeeded`, dann mit der `workspaceId` aus der Antwort suchen.

```json
{"tool": "index", "arguments": {"action": "prepare", "file": "/abs/path/design.fig", "scope": "0:0"}}
{"tool": "jobs", "arguments": {"action": "status", "jobId": "<jobId from the reply>"}}
{"tool": "search", "arguments": {"workspaceId": "<workspaceId from the reply>", "node": "0:0", "query": "checkout button"}}
```

Lebenszyklus: `refresh` baut nur Bereiche neu, deren `.fig`-Bytes sich geändert haben; `rebuild` baut auch Unveränderte neu; `clear_cache` löscht nur Daten, die ein Build neu erzeugen kann, und behält Registry und veröffentlichte Indizes; `reset` löscht die Datenbank dieses Workspaces (nie die `.fig` oder Exporte) und läuft nur auf ausdrücklichen Wunsch. Neue Quellen, ein größerer Bereich, reset und Neuanlage brauchen jeweils eine eigene ausdrückliche Anfrage.

`.fig` verschoben? `index relocate` behält `workspaceId`, `sourceId` und Speicherort; danach die betroffenen Bereiche mit `refresh` aktualisieren. Es gibt keine automatische Erkennung und kein Hash-basiertes Zusammenführen; ein reset ist nicht nötig.

Mehrere Designs in einem Ordner? `index add <dir> --workspace DIR` registriert bis zu 10 `.fig`-Dateien unter diesem Verzeichnis in einem Schritt und überspringt Links, die aus ihm herausführen; die Antwort nennt `added`, `existing`, `skipped` und `truncated`, eine Wiederholung fügt nichts hinzu, und jede Datei braucht weiterhin ihr eigenes `prepare --scope`. Eine innerhalb des Ordners verschobene Datei ist eine neue Quelle, es sei denn, sie wird mit `relocate` umgezogen. Mit `nextCursor` der Antwort als `--cursor C` (MCP `cursor`) werden die nächsten 10 hinzugefügt. Seitenweise Such- und Library-Cursor gelten eine Stunde; danach liefern sie `cursor_expired`, also erneut suchen.

Daten: Die SQLite-Datenbank enthält registrierte lokale Pfade, Layer-Namen und effektiven Text der gewählten Bereiche, die Komponenten-Property-Werte von Instanzen, Such-Postings, exakte Beziehungen (Verschachtelung, Instanzen, Varianten, Variablen-/Style-/Bild-Bindungen, Prototyp-Ziele) und die Definitionen, auf die sie zeigen, Hashes, Generationen und Jobs. Die ursprüngliche `.fig` bleibt unverändert, wo sie ist; ihre Rohbytes werden nicht in der Datenbank gespeichert. Semantische und visuelle Modell-Layer ergänzen ihre eingebetteten Vektoren, und eine aktivierte visuelle Indexierung ergänzt PNG-Renderings der indexierten Layer, auch ohne Modell; Abfragebilder werden nie gespeichert.

Pi, OpenCode und OMP nutzen denselben Index über ihren eigenen Opt-in-Einstieg; der Workspace wird über Neustarts und Hosts hinweg wiederverwendet. Nicht implementiert: Datei-Events (ein Watch fragt die Datei ab, und nur solange ein Holder läuft) und Push-Events.

<a id="semantic-search"></a>

## Optionale semantische Suche

Experimentell. `figlens search <file> <query> --mode semantic|hybrid` sortiert Layer mit einem lokalen Modell nach Bedeutung. Der Standard `--mode text` bleibt unverändert und braucht kein Modell.

- **Einmal einrichten.** Laden Sie BAAI/bge-m3 in der Revision `5617a9f61b028005a4858fdac845db406aefb181` (MIT; `config.json`, `pytorch_model.bin`, `tokenizer.json`) selbst herunter: figlens lädt nie herunter. `figlens models configure bge-m3 --model DIR [--runtime FILE]` prüft die Dateien per SHA-256, startet die Runtime `figlens-embed` einmal offline (gesucht neben der figlens-Programmdatei, außer `--runtime` ist angegeben) und speichert das Profil in `models.json` im Konfigurationsverzeichnis. `models list|status` zeigen es an, `models remove bge-m3` vergisst es und behält die Dateien.
- **Visuelles Profil.** `figlens models configure siglip-base --model DIR` prüft google/siglip-base-patch16-224 in der Revision `7fd15f0689c79d79e38b1c2e2e2370a7bf2761ed` (Apache-2.0; `config.json`, `preprocessor_config.json`, `model.safetensors`) auf dieselbe Weise und speichert das Bildmodell für die visuelle Suche.
- **Bildabfragen (experimentell).** `figlens search --workspace DIR --image shot.png` ordnet die Einheiten eines Workspace nach ihrem Aussehen; `--image-ref '{"sourceId":…,"snapshotId":…,"nodeId":…}'` nutzt stattdessen das gespeicherte Rendering eines Treffers, `--crop x,y,w,h` schneidet beides zu. Mit Suchtext wird daraus `hybrid`. Visuelle Ebenen baut `figlens index visual` (eine Suche baut keine, und eine Suche in einer einzelnen `.fig` lehnt ein Bild ab). Ohne Bildprofil oder wenn es fehlschlägt, ordnet allein der Pixel-Hash, und `query.usedModes` sagt das. Das Abfragebild wird nie gespeichert. Die Ranking-Regel wird noch geprüft; nimm die Treffer als Hinweise. MCP: `search` `image` (ein Pfad innerhalb der Roots), `imageRef`, `crop`.
- **Modi.** `semantic` sortiert jeden Layer nach seinem besten Chunk aus Name, Text, Komponenten-Properties oder Bereich. `hybrid` führt die Text- und die semantische Rangliste zusammen (Reciprocal Rank Fusion). Ohne Profil schlägt `semantic` mit `mode_unavailable` fehl; `hybrid` liefert die Text-Ergebnisse und nennt den Grund. `coverage.layers.semantic` meldet `ready`, `partial` oder `unavailable`; eine partielle Seite hat kein `nextCursor`. Die Modelle laufen auf der CPU; Strg-C stoppt die aktive Anfrage. Ein MCP-/Viewer-Prozess kann nach der Nutzung je ein Text- und Bildmodell im Speicher halten: allein das Textmodell etwa 2,5 GiB, die Summe ihrer Speicherspitzen etwa 3,5 GiB in einer Linux-ARM-Messung, ohne Kernprozess und Index-Worker. Dies sind Messwerte, keine Speichergrenzen.
- **Grenzen.** Eingebettet werden Layernamen, effektiver Text, die Property-Werte einer Instanz (ein nicht exakt auflösbarer Wert wird ausgelassen und gezählt) sowie Seiten- und Container-Namen jedes Layers. Die Suche mit `--workspace`/`--id` liest die gespeicherte Modellschicht jeder Quelle und bettet nie Designtext ein; sie entsteht mit `figlens index embed` (der MCP-Server reiht sie nach einem Text-Build ein). Eine Quelle ohne sie ist `pending`, die Seite partiell. Im eingefrorenen Testset dieser Funktion verfehlte das Modell das Qualitätskriterium für Synonym- und Zweck-Anfragen (falsche Top-1-Quote 0,25, Grenze 0,20); behandeln Sie Treffer als Hinweise.
- **MCP.** Das Tool `models` (`list`, `status`, `configure`, `remove`) und `search` `mode`; `configure` startet nur das `figlens-embed` neben figlens, Modellverzeichnis und -dateien müssen in den Roots des Servers liegen. Das Abbrechen des Aufrufs stoppt eine laufende Suche oder Einrichtung.

<a id="commands"></a>

## CLI-Referenz

| Befehl | Ausgabe |
|---|---|
| `tree <file> [node] [--depth N] [--json]` | Node-IDs, REST-Typen (wie in `nodes`) und Namen; eingerückter Text oder verschachteltes JSON mit Größe, Ausblendung und Auto-Layout-Modus jedes Nodes sowie `firstText` (erster sichtbarer Text) bei Nodes einer Seite oder Section |
| `tree <file> [node] --top-frames [--json]` | Nur die Screens, einer pro Zeile, direkt nutzbar für `--frames -`: Frames, Komponenten, Komponentensets und Instanzen auf einer Seite oder in ihren Sections (Gruppen und interne Seiten ausgenommen); `--json` ergänzt Seiten- und Section-IDs sowie den ersten sichtbaren Text jedes Screens (`firstText`), der Canvas-Beschriftungen von Screens unterscheidet |
| `find <file> <query> [--type TYPE] [--node ID] [--limit N]` | Suche über Layer-Namen und Text ohne Beachtung der Groß-/Kleinschreibung; Pfade, Größen und Vorschauen. Standardmäßig 20 Ergebnisse (1-200) |
| `search <file> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C] [--include-hidden] [--include-internal] [--mode text\|semantic\|hybrid\|visual] [--image PNG\|--image-ref JSON] [--crop x,y,w,h]` | Gerankte Suche über Layer-Namen und vollständigen Text mit angewendeten Instanz-Overrides, Text-/Boolean-Properties und Swaps; exakte Node-Referenzen (auch `I…;…`-Sublayer-IDs für `inspect`), Treffergründe, UTF-16-Spannen, Abdeckung und Cursor-Seiten. Standardmäßig 20 (1-200); ausgeblendete Layer und interne Seiten brauchen ihr Flag. Mit `--file`/`--workspace`/`--id` statt `<file>` liest es den vorbereiteten Index des Workspaces (siehe `index`) `--mode semantic\|hybrid` braucht ein eingerichtetes Modell: siehe [Optionale semantische Suche](#semantic-search). |
| `models list\|status\|configure <profile> --model DIR [--runtime FILE]\|remove <profile>` | Lokales Textmodell für die semantische Suche (`bge-m3`, experimentell) und Bildmodell für die Bildsuche (`siglip-base`, experimentell); `configure` prüft die selbst heruntergeladenen Dateien und lädt nie herunter; `remove` behält sie. Siehe [Optionale semantische Suche](#semantic-search). |
| `usages <file> <node\|resource> [--scope ID] [--relation R[,R]] [--out] [--snapshot S] [--include-hidden] [--limit N] [--cursor C]` | Exakte Beziehungen eines Layers oder einer Ressource (`VariableID:…`, `StyleKey:…`, `image:…`) in einem gespeicherten Bereich, verschachtelte Instanzen und Swaps eingeschlossen: wo eine Komponente, ein Set, eine Variable, ein Style oder ein Bild verwendet wird (Standard) oder worauf ein Layer zeigt (`--out`). Jedes Ergebnis behält seine exakte Referenz, Beziehung, seinen Zustand (`resolved`, `missing`, `unresolved`, …), Basis und Beleg; Namen gelten nie als Identität. Keine Gesamtzahl: Die Coverage sagt vollständig oder partiell. Standard 50 (1-200). Mit `--file`/`--workspace`/`--id` statt `<file>` liest es den vorbereiteten Index von `--scope` des Workspace. Mit `--out` nennt eine Verwendung einer gecachten Bibliothekskopie auch ihre `library` {kind, namespace, key}. |
| `usages --file\|--workspace\|--id … --library <kind> --namespace <ns> --key <key> [--definition-source ID --definition-snapshot SNAP --definition-node NODE] [--source ID] [--scope ID] [--include-hidden] [--limit N] [--cursor C]` | Layer, die eine Bibliothekskomponente, einen Bibliotheks-Style oder eine Bibliotheksvariable (`kind`) verwenden, so wie die zwischengespeicherte Kopie jeder Datei sie festhält, über den vorbereiteten `--scope` aller Quellen (`--source` beschränkt auf eine). Die Definition bleibt `unresolved`, außer die drei `--definition-*`-Flags wählen sie per exakter Referenz (`selected`, `keyEvidence: unverified`); die Coverage nennt den Zustand jeder Quelle und warum sie partiell ist. Die Angaben stehen im Feld `library` von `usages --out` auf einer Ebene, die es nutzt. |
| `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]` | Was sich zwischen zwei gespeicherten Revisionen geändert hat: eine Zeile pro geändertem Top-Level-Frame mit Zählern je Facette (Text, Layout, Aussehen, Eigenschaften, Struktur, Bindungen); `--json` ergänzt alte/neue Feldwerte und Edge-Änderungen. Layer werden nur über exakte ID und Typ zugeordnet; ein Frame mit gleichem Namen oder gleicher Position ist nur ein markierter Possible Match, und eine vorzeitig gestoppte Seite oder eine Ebene, deren einziger Unterschied ein nicht lesbares Feld ist, meldet unknown, nie entfernt oder geändert. Seiten mit 100 Änderungen (1-500) Stoppt eine Seite an einem Limit, steht `stopped early` statt `no changes`; mit `--node` eingrenzen. |
| `inspect <file> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat] [--full-text] [--colors]` | Kompakte Zusammenfassung des expandierten Layouts/Texts, höchstens 200 Nodes. `--images` ergänzt Bitmap-Abmessungen und Crop-Footprints. `--flat` gibt stattdessen eine Zeile pro Node aus: Position relativ zum Node, Größe, Text, Schrift, Füllung, Kontur, Radius, Auto-Layout, Ausrichtung, Fixed/Fill-Größen, Schatten und Blurs `--full-text` liefert jeden Text vollständig (bis 10.000 Zeichen); `--colors` ergänzt sichtbare Vollton-Füllungen (Hex, Deckkraft, gebundene Variable), auch mit `--flat`; keine Verläufe, Bilder oder Konturen. |
| `nodes <file> <node>` | Das Objekt je Node einer REST-nodes-Antwort (`document`, `components`, `componentSets`, `styles`): expandierte Instanzen und Overrides, Auto-Layout, Textstile, Füllungen, Effekte, gebundene Variablen, Komponenten-Eigenschaften, Render-Bounds |
| `nodes <file> --frames <list>`, `--frames -` (stdin), `--ids <node>...` | Ein Decode, viele Nodes: eine JSON-Zeile pro gelisteter ID, die `nodes`-Ausgabe mit ihrer `nodeId` oder `{nodeId, error}` |
| `variables <file> [node]`<br>`variables <file.fig> --format dtcg --collection ID --mode ID --out FILE` | Standard: unveränderte gebundene/lokale Variablen im REST-Format. DTCG-2025.10-Export: exakte Collection-/Mode-IDs, kein Node; nur Farben und einheitenlose Zahlen. Ausgabeordner vorher anlegen; bei Diagnosen schlägt der gesamte Export fehl, die Ausgabedatei bleibt unverändert. Eine Library-Collection wird aus den in der Datei zwischengespeicherten Kopien exportiert (`libraryCopy: true`). Ein Alias in eine andere Collection exportiert deren Wert, festgehalten in `aliasOf`; hat sie mehrere Modes, `--alias-mode COLLECTION=MODE` angeben. |
| `bundle <file> <node> <out.zip> [--states] [--hash-source]` | Ein Design-Bundle für einen Frame; gibt `{nodeId, path, warnings}` aus |
| `bundle <file> --frames <list> --out-dir <dir> [--jobs N] [--states] [--hash-source]`, `--frames -` (stdin) | Viele Bundles aus einem Decode, parallel |
| `verify <bundle> [--fonts <dir>]` | Prüft ein Bundle (`.zip` oder entpacktes Verzeichnis) gegen sein Manifest: verifizierte, fehlende und geänderte Dateien, fehlende Assets, die im Projekt zu bestätigenden Fonts und `findings`: jede fehlende, nicht unterstützte oder unverifizierte Handoff-Eingabe mit Nodes, Belegen und nächstem Schritt; ein Font mit Datei auf diesem Rechner oder in `--fonts <dir>` (Projektordner, liefert zusätzlich ein `@font-face`) ist `local_file`, kein Beleg, dass der Browser ihn lädt |
| `export <file> <node> <out> [--bounds layout] [--scale N] [--quality 1-100] [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID]]` | SVG, PNG, JPG, WebP oder AVIF; PNGs bis 4.194.304 Pixel werden automatisch verlustfrei optimiert, ohne die dekodierten RGBA-Werte zu ändern; nur kleinere Ergebnisse ersetzen die ursprünglichen kodierten Bytes, größere PNGs behalten die reguläre Kodierung; `--max-kb` senkt die verlustbehaftete Qualität, bis es passt (PNG schlägt bei weiterer Überschreitung fehl, ohne verlustbehafteten Fallback); `--bounds layout` exportiert die Layout-Box statt der Render-Grenzen; `--match-dir DIR` (nur SVG, mit Unterordnern) listet die SVGs dort, die dieselben Umrisse zeichnen (`matches`), und bis zu 10 ähnliche Formen mit Score (`similar`); `--hide ID[,ID]` zeichnet diese Ebenen im Node als ausgeblendet über derselben Fläche, sodass ein Vergleich mit dem normalen Export zeigt, wo diese Ebene, wie figlens sie zeichnet, das Ergebnis ändert (kein Unterschied beweist nicht, dass das übersprungene Rendering unschädlich ist) |
| `export <file> --frames <list> --out-dir <dir> [--format F] [--scale N] ...`, `--frames -` (stdin) | Ein Decode, viele Exporte (Standard `png`): schreibt `<dir>/<nodeId>.<format>`, eine JSON-Zeile pro ID, die `export`-Ausgabe mit ihrer `nodeId` oder `{nodeId, error}` |
| `export <file> --batch <items.json>`, `--batch -` (stdin) | Ein Decode, viele Exporte mit je eigenen Optionen: ein JSON-Array aus 1-64 `{node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}` (MCP `export_batch`). Gibt ein Manifest `{count, failed, items}` aus: je Eintrag die `export`-Ausgabe oder `{error}` (die übrigen werden trotzdem exportiert); ein fehlerhafter Eintrag oder ein doppeltes `out` scheitert vor jedem Schreiben |
| `preview <file> [node] [--out preview.html] [--port N] [--bind IP]` | Lokaler Viewer, der das Design nie schreibt; semantische/hybride Suche nach Enable index; `--out` schreibt einen eigenständigen Offline-Snapshot; Ergebnisse und kopierter Context nennen die Seite und, live, den Dateipfad, den Snapshot des geladenen Source und die lokale Revision (offline unbekannt; der Pfad wird nie eingebettet); der Context enthält zudem die Coverage des Referenz-Scopes, `complete: null` wenn ungeprüft. Visual/Hybrid nimmt ein Referenz-PNG oder einen ausgewählten Layer mit Pixelzuschnitt an. Visuelle Indexierung wird ausdrücklich aktiviert; Suchbilder werden nicht gespeichert. |
| `compare <file> <nodes.json> <node>` | Diff gegen einen REST-Export desselben Frames |
| `compare-image <file> <node> <screenshot.png>` | Vergleicht einen Screenshot der gebauten Oberfläche mit dem in seinem Maßstab gerenderten Node (`--scale`, `--limit`, `--out diff.png`, `--full-page` für eine ganzseitige Aufnahme, die kürzer als der Frame ist, `--ignore` für Ebenen mit Inhalten aus Daten, `--threshold` für helle Rahmen; `image: true` markiert Bildfüllungen; MCP `compare_image`). Gibt `mismatch`, `nonTextMismatch` und die größten abweichenden Bereiche in Node-Pixeln aus, je mit den tiefsten Ebenen darunter; `text: true` markiert Bereiche über Text (oft Schrift-Fallback; jeden prüfen) |
| `node <file> <node>` | Ein roher `.fig`-Node |
| `index prepare\|refresh\|rebuild\|clear\|status\|add\|remove\|relocate\|reset\|track\|untrack\|embed\|visual` | Lokaler Workspace im figlens-Statusverzeichnis des Benutzers oder unter einem `--storage DIR`, das sich der Workspace danach merkt. `prepare <file> --scope NODE` baut den Suchindex dieses Bereichs einmal; danach antwortet `search --file <file>` (oder `--workspace DIR`) ohne erneutes Parsen, über alle Quellen des Workspaces gerankt. `refresh` baut geänderte Dateien neu, `rebuild` alle oder eine `--source`/`--scope`, `clear` entfernt ungenutzte Indizes; der bisherige Index bleibt nutzbar, bis ein neuer fertig ist. `reset` entfernt nur die Workspace-Datenbank, nie Designs, Exporte oder den gemerkten Ort. `track --scope NODE` zeichnet die Änderungen dieses Scopes ab einem Baseline-Build auf; `untrack` beendet das und behält den Verlauf. `embed` bettet veröffentlichte Bereiche mit dem konfigurierten Modell ein (CPU und Speicher); nur ausführen, wenn Sie semantische Workspace-Suche wollen. `visual --scope NODE` rendert die Bildschirme und Komponenten dieses veröffentlichten Bereichs für die Bildsuche und bettet sie mit einem konfigurierten Bildprofil ein; nur ausführen, wenn Sie visuelle Workspace-Suche wollen. |
| `jobs status\|cancel <jobId>` | Status eines Index-Builds anzeigen oder ihn abbrechen; `cancel` kehrt erst zurück, wenn der Build gestoppt ist |
| `changes --file <file>\|--id ID\|--workspace DIR [--source ID] [--scope NODE] [--consumer NAME\|--subscription ID\|--cursor C] [--revision R] [--limit N]` | Aufgezeichnete Änderungen verfolgter Scopes, älteste zuerst: eine Revision pro veröffentlichtem Build (baseline oder changes) mit Abdeckung und Zählungen pro Änderungsart und Facette; jede changes-Revision hat eine `impact`-Zusammenfassung der Layer, die ihre Änderungen erreichen (null vor Schema 7); `--revision` blättert die geänderten Nodes dieser Revision, dann die von `impact` gelisteten Layer (`entry` sagt, welche); `tracking` zeigt, wann ein Scope `index refresh` braucht, damit eine Änderung aufgezeichnet wird. Nur lesend |
| `watch ack --file <file>\|--id ID\|--workspace DIR --consumer NAME\|--subscription ID --cursor C [--source ID] [--scope NODE]` | Den Checkpoint eines Consumers auf den `nextCursor` der letzten mit denselben Filtern verarbeiteten `changes`-Seite setzen; Wiederholen ist sicher, und er bewegt sich nur vorwärts, außer dass ein aktueller Recovery-Cursor einen Checkpoint hinter einer Löschung zurücksetzt, damit die behaltenen Revisionen erneut gelesen werden; ein Cursor von vor einer Löschung durch Retention, die er nicht passiert hat, ist `event_gap` und schreibt nichts |
| `watch run --file <file>\|--id ID\|--workspace DIR --scope NODE [--source ID] [--request-id R]` | Hält im Vordergrund einen Lease auf einen getrackten Scope: Der gemeinsame Runner des Workspace aktualisiert ihn bei jedem Speichern (Abfrage alle 250 ms, Inhaltsprüfung alle 60 s), sodass `changes` neue Revisionen ohne `index refresh` zeigt. Gibt den Lease aus, dann eine Endzeile, wenn stdin schließt, bei Ctrl-C oder SIGTERM oder wenn der Scope nicht mehr getrackt wird. MCP `watch` `start` und `stop` halten Leases, solange der Server läuft; ein nicht erneuerter Lease endet nach 30 s, und der Runner beendet sich nach dem letzten |
| `watch status --file <file>\|--id ID\|--workspace DIR` | Die Leases des Workspace (`active`, `unauthorized`, `untracked`, `expired`, `ended`) und sein Runner (`running`, `stopping`, `stopped`, `unknown`, mit Platzierung); schreibt nie |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]` | Einer Quelle, einem Scope oder genau einer Ebene (`--node`) ab ihrer neuesten Revision mit eigenem Checkpoint folgen, gelesen mit `changes --subscription` und bestätigt mit `watch ack --subscription`; eine fehlende oder mehrdeutige Ebene ist `target_missing` oder `identity_uncertain`, nie eine gleichnamige; `--owner` macht sie für diese Aufgabe temporär |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID --query TEXT [--type T[,T]] [--scope NODE]` | Eine Suche über den Scope einer Quelle speichern: ihre Top 50 werden sofort ausgewertet; `changes --subscription` liest die gespeicherten Auswertungen und berechnet keine |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --sources visible\|ID[,ID] --query TEXT\|--library KIND --key KEY [--namespace NS] [--definition-source ID --definition-snapshot SNAP --definition-node NODE]` | Speichert eine Suche oder die Verwendungen eines Bibliothekselements mit ihrem `definitionMatch` über eine feste Menge von höchstens 10 Quellen (`visible` wird jetzt aufgelöst; spätere Quellen kommen nie hinzu): `delta.changedSourceIds` nennt Quellen mit neuen Bytes, `corpus_changed` eine Änderung der Menge, `coverage.transitive` ist `not_followed` und `delta.notFollowedSourceIds` nennt geänderte Quellen ohne Verwendung, deren Abhängige nicht erneut geprüft wurden; das Entfernen eines Mitglieds deaktiviert sie (unsubscribe, dann erneut subscribe) |
| `watch evaluate --file <file>\|--id ID\|--workspace DIR --subscription ID` | Die Auswertung einer Query-Subscription für den aktuellen Index des Scopes speichern, sofern noch nicht ausgewertet; ein Watch auf dem Scope tut das nach jeder Veröffentlichung |
| `watch list --file <file>\|--id ID\|--workspace DIR [--consumer NAME] [--after ID] [--subscription ID]` | Die sichtbaren Subscriptions und ihren Zustand seitenweise auflisten; `--subscription` liest nur diese eine (leer, wenn nicht sichtbar) |
| `watch unsubscribe --file <file>\|--id ID\|--workspace DIR --subscription ID\|--consumer C --owner NAME` | Eine Subscription samt Checkpoint entfernen, oder die temporären eines Owners; Tracking und Verlauf bleiben |
| `watch forget --file <file>\|--id ID\|--workspace DIR --consumer NAME [--source ID] [--scope NODE]` | Die rohen Checkpoints eines Consumers entfernen; ein Consumer hält höchstens 64 Subscriptions und Checkpoints, ein Workspace 4096, und nichts wird automatisch verdrängt |
| `mcp` | Lokaler MCP-Server über stdio |
| `update` | Ersetzt das Binary durch das neueste Release |

Für zugehörige Komponentenvarianten `inspect --states` oder `bundle --states` verwenden (MCP: `states: true`, standardmäßig false; keine Konfigurationsänderung). Die Nachweise behalten Quellen-, Unbekannt- und Kürzungsmarkierungen. Bundles ergänzen `design/states.json` und eine Zusammenfassung in `context.md`. Umfang und Grenzen stehen unter [Zustandsnachweise](../wiki/states.md).

Zwei Varianten desselben Component-Sets mit `inspect <file> <variantA> --compare-variant <variantB>` vergleichen (MCP `compareVariant`). `variantComparison` meldet Unterschiede gespeicherter Felder und Wertemengen samt Quell-IDs und unbekannten Werten, Kindknoten werden nach eindeutigem Namen, dann nach Hauptkomponente, dann nach Stapelreihenfolge in gleich großen Namensgruppen gepaart (`pairedNodes` mit Änderungen je Paar; `pairedBy` prüfen, `order` ist der schwächste Beleg; der Rest bleibt ungepaart); gebundene Variablen werden benannt. Siehe [Vergleichsgrenzen](../wiki/states.md#compare-two-variants).

Um zwei beliebige Knoten derselben Datei zu vergleichen (leerer und gefüllter Screen, Mobile- und Desktop-Breakpoint, zwei Screens mit gemeinsamem Header), verwende `inspect <file> <nodeA> --compare <nodeB>` (MCP `compare`). `comparison` hat dieselbe Form plus `rootDelta` (Größe und Position relativ zum Elternknoten); Kinder werden nach eindeutigem Namen, dann nach Hauptkomponente, dann nach Stapelreihenfolge in gleich großen Namensgruppen gepaart (`pairedBy` nennt das Verfahren), der Rest bleibt ungepaart mit `reason: ambiguous-name`; Instanzen werden nicht expandiert, und ein Knoten lässt sich nicht mit seinem Vorfahren vergleichen. `--compare-variant` verlangt weiterhin dasselbe Component Set.

Ein Node-Argument akzeptiert `12:345`, `12-345` oder eine Figma-URL mit `?node-id=`. `<file>` kann auch `gs://bucket/object` sein, heruntergeladen mit deinem `gcloud`-Login und im Cache abgelegt. Der [GCS-Snapshot-Leitfaden](../wiki/gcs.md) beschreibt die Übergabe über ein festes Objekt, nötige Zugriffsrechte und Grenzen der Aktualisierung. Jeder CLI-Aufruf decodiert die Datei; MCP verwendet nur die zuletzt gelesene, unveränderte lokale Datei erneut. Remote-Aufrufe parsen erneut. Führe `figlens --help` oder `figlens inspect --help` (auch `-h`) für die vollständige CLI-Hilfe aus. `index add gs://bucket/object` registriert eine fixierte Kopie dieser Generation und meldet `remote {uri, generation}`; nur add startet gcloud.

`inspect` enthält gespeicherte Textgrößenanpassung, Kürzung, maximale Zeilenzahl und Textdekoration. Ein Elternknoten ohne Auto-Layout trägt `childSpacing: "derived"`: Abstände zwischen seinen Kindern sind Koordinatendifferenzen, keine Auto-Layout-Regel; `constraints` können sie dennoch fixieren. `handoffDiagnostics` meldet gespeicherte, aber nicht ausgegebene Semantik mit Anzahl, Beispielen und [Abdeckungsgrenzen](../wiki/semantics.md#known-handoff-gaps). `_figlens.semantics` listet nur Felder, deren Quelle gespeichert ist (ein fehlender Eintrag heißt: nicht gespeichert); jede Antwort sagt einmal, auf oberster Ebene in `_figlens.semantics.verification`, dass diese Zuordnungen nicht mit Figma verglichen wurden. Grid-Auto-Layout-Frames enthalten Zeilen- und Spaltenanzahl, Abstände und gespeicherte Spurgrößen (`_figlens.gridTracks`). Ausgaben schützen die Quelle und ersetzen das Ziel erst nach vollständigem Schreiben. MCP begrenzt Anfragezeilen auf 1 MiB und vollständige Antworten auf 8 MiB; Überschreitungen ergeben einen ausdrücklichen Fehler. Vollständiges JSON bleibt über die CLI verfügbar.
