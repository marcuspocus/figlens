# figlens — Guide utilisateur

[← README](../README-fr.md) · [figlens.runkids.cc](https://figlens.runkids.cc/fr/docs/)

<p align="center">
  <a href="README.md">English</a> · <a href="README-de.md">Deutsch</a> · <a href="README-es.md">Español</a> · <a href="README-fr.md">Français</a> · <a href="README-ja.md">日本語</a> · <a href="README-ko.md">한국어</a> · <a href="README-pt-BR.md">Português</a> · <a href="README-zh-CN.md">简体中文</a> · <a href="README-zh-TW.md">繁體中文</a>
</p>

- [Installation autonome](#install)
- [Pourquoi figlens plutôt que le MCP Figma officiel ?](#why)
- [Ce que reçoit l’agent](#bundle)
- [Extension Pi](#pi)
- [Panneau Herdr](#herdr)
- [Parcourir un design sans Figma](#viewer)
- [Serveur MCP et skill pour agent](#mcp-and-skill)
- [Référence de la CLI](#commands)

<a id="install"></a>

## Installation autonome

**Pas de Node.js ?** Installez le binaire autonome et utilisez `"command": "figlens", "args": ["mcp"]` à la place :

| Plateforme | Commande |
|---|---|
| macOS / Linux | `curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh \| sh` |
| Homebrew | `brew install runkids/tap/figlens` |
| Windows (PowerShell) | `irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 \| iex` |

<a id="why"></a>

## Pourquoi figlens plutôt que le MCP Figma officiel ?

Utilisez **figlens** quand le design doit voyager sous forme de fichier : agents distants, CI, travail hors ligne, passation entre agents, lectures répétées illimitées. Utilisez le **MCP Figma officiel** quand l’agent doit voir le design en direct, sans étape d’export.

| | figlens | MCP Figma officiel |
|---|---|---|
| Source | Snapshot `.fig` exporté | Fichiers Figma en direct |
| Prérequis | Le fichier | Connexion à Figma ; le serveur desktop nécessite l’application |
| Limites de débit | Aucune, analyse locale | Limites du service Figma |
| Agent distant / CI | Copier le fichier, lancer le binaire | Serveur distant avec connexion |
| Modifications du design | Exporter un nouveau `.fig` | Automatique |
| Passation | Partager le `.fig` ou un bundle de frame | Partager l’accès Figma |
| Fidélité | Parser indépendant avec des [lacunes connues](../README-fr.md#limitations) | Maintenu par Figma |

La lecture d’un `.fig` se fait hors ligne. L’installation et la mise à jour de figlens, les entrées `gs://`, la vérification facultative `freshness` et `pull`, votre fournisseur de modèle et les dépendances de votre projet nécessitent toujours un réseau.

<a id="bundle"></a>

## Ce que reçoit l’agent

Une frame, un zip. Une structure pour construire, des assets à utiliser, une image pour vérifier.

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

Besoin d’un seul élément ? La CLI le fait directement :

```sh
npx -y @runkids/figlens find design.fig "login" --type FRAME     # ids de frames par nom ou texte
npx -y @runkids/figlens inspect design.fig 12:345                 # résumé compact de la mise en page et du texte
npx -y @runkids/figlens nodes design.fig 12:345 > frame.json      # JSON au format REST
npx -y @runkids/figlens export design.fig 12:345 icon.svg         # SVG, PNG, JPG, WebP ou AVIF
npx -y @runkids/figlens bundle design.fig 12:345 frame.zip        # le zip ci-dessus
```

Les exports SVG intègrent les image fills d’origine et convertissent le texte en glyphes vectoriels, donc aucune police n’est nécessaire. Via MCP, les exports raster reviennent aussi en images inline : l’agent voit le design sans ouvrir de fichier.

<a id="copy-a-selection"></a>

### Pas de .fig enregistré ? Copiez une sélection

1. Dans Figma, sélectionnez le frame ou les calques et copiez-les : clic droit > **Copy**, ou Cmd+C (Ctrl+C sous Windows et Linux).
2. Lancez `figlens clipboard screen.fig`, ou demandez à votre agent de construire « l'écran que je viens de copier ». Il lit le presse-papiers et écrit un `.fig` que toutes les commandes lisent.
3. Passez `screen.fig` comme n'importe quel autre fichier : `figlens bundle screen.fig <node> frame.zip`.

Coller la copie dans un champ de texte ne donne que son texte ; figlens lit le HTML de la copie, qui contient les calques. macOS est vérifié sur de vraies copies ; Windows (PowerShell 5.1) et Linux (`wl-paste` ou `xclip`) ne le sont pas encore.

| | **File > Save local copy** | Sélection copiée |
|---|---|---|
| Portée | Le fichier entier | Les calques copiés, plus les composants qu'ils utilisent |
| Remplissages d'image et contours vectoriels | Inclus | Absents ; les exports avertissent là où ils dessinent sans eux |
| Un lien Figma trouve le nœud | Oui | Oui, pour les calques copiés : les ids correspondent au fichier source |
| Convient pour | Construire à partir d'un design conservé à côté du code | Un coup d'œil rapide à un écran quand il n'existe pas de copie enregistrée |

<a id="accuracy"></a>

### Quelle est la proximité avec Figma ?

Mesurée par rapport aux exports JSON, PNG et SVG de Figma lui-même sur **826 frames de production** et cinq fichiers Community (2026-10-22) :

| Contrôle | Résultat |
|---|---|
| Structure des nodes, noms, visibilité, bounds et texte | Identiques dans 771 / 826 frames |
| Image fills d’origine | 2 608 / 2 608 identiques à l’octet près |
| Assets SVG | 5 014 / 5 197 dans le seuil de différence de pixels |
| Bundles de frames Community | 306 / 306 sans asset manquant |

Résultats sur ces fichiers, pas une garantie pour tous les designs. Méthode et seuils : [rapport de précision](../wiki/accuracy.md).

<a id="pi"></a>

## Extension Pi

```sh
pi install npm:@runkids/figlens
```

Guides d’installation: [OpenCode V2](../opencode/README.md) · [OMP](../omp/README.md).

Redémarrez Pi ou lancez `/reload`, puis demandez un écran en langage courant. Pi trouve la frame, la lit via le serveur MCP enregistré automatiquement et construit avec votre stack. Pas d’ids de nodes, pas de CLI globale.

Demander un texte ou un écran cherche directement dans le `.fig` et n'enregistre rien. Pour des recherches répétées, demandez à Pi d'activer un index pour un design et une portée : il prépare une fois un index local (sans commande d'installation), suit la tâche jusqu'à ce qu'il soit prêt et le réutilise après un redémarrage et dans d'autres hôtes. Réinitialiser cet index exige votre demande explicite. Demander où un composant, une variable, un style ou une image est utilisé liste ses usages exacts de la même façon : directement depuis le `.fig` ou, une fois activé, depuis cet index. Demander à Pi de suivre les changements d'un design active le suivi de ce périmètre, en préparant son index si besoin, et indique ce qui a changé depuis la dernière vérification ; une question sur deux copies enregistrées se règle avec `diff`, sans rien démarrer. Détails : [index local](#local-index).

- `/figlens plan <request>` rédige uniquement un plan, sans modifier l’application.
- `/figlens yolo <request>` planifie, construit, vérifie et nettoie son propre plan temporaire, ses exports de travail et les processus qu’il a lancés.
- `/figlens preview [description]` ouvre le viewer local.
- `/figlens review` approuve les mappings de composants réutilisables proposés par l’agent.

![Demandez un écran à Pi, laissez-le trouver le design, construire avec votre stack et vérifier le résultat](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/pi-workflow.svg)

Guide complet, commandes et limites : [pi/README.md](../pi/README.md).

<a id="herdr"></a>

## Panneau Herdr

Un panneau [Herdr](https://herdr.dev) pour le design à côté de vos agents : ouvrez un `.fig`, trouvez un écran ou un calque par nom, texte, ID de nœud ou lien Figma, regardez-le en taille réelle et confiez-le à l’agent de gauche en Build, Plan ou YOLO après une boîte de confirmation qui montre le message exact. Aucune installation de figlens n’est nécessaire, et les designs des machines Herdr distantes sont lus sur place. Demandez à l’agent dans Herdr : `Read ~/.config/herdr-app/AGENTS.md, then add this layout: https://github.com/runkids/figlens/tree/main/herdr/figlens-layout`. Détails : [herdr/README.md](../herdr/README.md). Lorsque des agents utilisent figlens dans des terminaux Herdr (MCP ou CLI), le panneau montre quel écran chacun lit, et son tableau suit chaque écran : en cours, vérifié avec compare_image ou modifié depuis qu’un agent l’a construit.

<a id="viewer"></a>

## Parcourir un design sans Figma

[![Inspecter localement le design MCP Apps, avec son arbre de calques, sa mise en page et les propriétés des composants](https://raw.githubusercontent.com/runkids/figlens/v0.10.1/assets/preview-inspector.png)](https://github.com/runkids/figlens/blob/v0.10.1/assets/pi-demo.mp4)

```sh
npx -y @runkids/figlens preview design.fig                        # viewer local, Ctrl+C pour arrêter
npx -y @runkids/figlens preview design.fig 12:345 --out preview.html   # snapshot HTML hors ligne à envoyer à n’importe qui
```

Parcourez les pages et les calques, recherchez, lancez la recherche classée et les usages exacts de la sélection, mesurez, inspectez la mise en page, la typographie, les variables et le CSS, exportez n’importe quel calque, copiez un prompt d’implémentation ou téléchargez un bundle pour la sélection. Passez la recherche classée en mode sémantique ou hybride avec un modèle local facultatif : le visualiseur n’indexe le fichier que lorsque vous cliquez sur Enable index. Déposez une capture d’écran de votre build pour la superposer au design. Détails : [guide du preview](../wiki/preview.md). Visual/Hybrid accepte un PNG de référence ou un calque sélectionné avec recadrage en pixels. Activez explicitement l’index visuel ; les images de recherche ne sont pas conservées. **Copy reference prompt** copie un court prompt qui nomme le calque sélectionné, à coller dans n’importe quel agent.

<a id="mcp-and-skill"></a>

## Serveur MCP et skill pour agent

**MCP.** Le serveur s’exécute en local via stdio et expose `workflow`, `tree`, `find`, `search`, `usages`, `diff`, `inspect`, `nodes`, `variables`, `bundle`, `verify`, `freshness`, `index`, `jobs`, `changes`, `watch`, `export`, `export_batch`, `compare_image` et `models`. `export_batch` écrit jusqu’à 64 fichiers en un appel et renvoie un manifeste au lieu d’images inline. Passez des chemins absolus. Ajoutez `--root /path/to/designs` (répétable) pour confiner chaque chemin de fichier et de sortie aux dossiers choisis. Un stockage personnalisé explicitement demandé exige `--storage-root DIR` (répétable) ; les emplacements mémorisés sont réutilisés sans cette option. Voir les [arguments de démarrage et l’exemple de configuration avec plusieurs racines](../README-fr.md#mcp-args). Les exports raster sont réduits à `maxSize` (1600 px par défaut) pour que les images inline restent légères ; SVG ne renvoie que des métadonnées.

Avec `workflow`, l’agent peut lire les guides intégrés `build`, `audit`, `images`, `search`, `plan`, `planning-details` ou `yolo` sur demande explicite. Ils partagent les sources du skill portable ; son installation séparée est facultative avec MCP. L’agent réalise le travail avec ses outils de projet et de navigateur et les permissions de l’hôte. Lire un guide n’exécute pas le workflow ; son utilisation dépend du client et du comportement du modèle.

Windows natif avec Claude Code : suivez ses [notes de configuration stdio](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server) et le wrapper `cmd /c` pour `npx`.

**Skill.** Le [skill figlens](../skills/figlens/SKILL.md) apprend à un agent à trouver des frames, lire le bundle par petits morceaux, planifier un travail ciblé et vérifier l’UI en cours d’exécution par rapport au design. Il n’installe que des instructions ; la CLI ou le serveur MCP doit être disponible. Installez-le avec la [Skills CLI](https://github.com/vercel-labs/skills) ou copiez `skills/figlens/` dans le répertoire de skills de votre agent (Claude Code : `~/.claude/skills/`) :

```sh
npm install -g @runkids/figlens
npx skills add runkids/figlens --skill figlens
```

**Plugins.** Claude Code, Codex, Antigravity, Cursor et Kimi Code installent le skill et le serveur MCP ensemble en un seul plugin ; voir [Configuration du plugin](../README-fr.md#setup-plugin).

**skillshare.** Gérez le skill et la connexion MCP pour Claude Code, Codex et Cursor depuis un seul endroit avec [skillshare](https://github.com/runkids/skillshare) :

```sh
skillshare install github.com/runkids/figlens/skills/figlens
skillshare mcp add figlens --global --target claude --target codex --target cursor --no-tui -- npx -y @runkids/figlens mcp
skillshare sync && skillshare sync mcp --global
```

**Notes d’installation.** Le lanceur npm télécharge le binaire natif correspondant à la première utilisation, le vérifie avec les checksums SHA-256 de la release et le met en cache. Les installations par script se mettent à jour avec `figlens update`, celles de Homebrew avec `brew upgrade figlens`. Les binaires couvrent macOS (Apple silicon, Intel), Linux (arm64, x86_64, musl statique) et Windows (x86_64). Chaque méthode d’installation place aussi `figlens-embed`, le runtime de la [recherche sémantique optionnelle](#semantic-search), à côté de `figlens` ; il ne charge aucun modèle tant que vous n’en configurez pas.

<a id="local-index"></a>

## Recherche ordinaire et index local optionnel

|  | Ordinaire (par défaut) | Avancé : index local (optionnel) |
|---|---|---|
| Ce que ça fait | Lit le `.fig` à chaque appel et répond à `search`, `find`, `inspect` et `export`. | Construit une fois un index de recherche d'une portée choisie, puis répond à `search` depuis cet index, classé entre les sources. |
| Ce qui est écrit | Ni workspace ni base d'index (`export` n'écrit que les fichiers demandés). | Un workspace local (base SQLite) dans le répertoire d'état de l'utilisateur, ou sous le `--storage` choisi. |
| Quand l'utiliser | Questions ponctuelles, scripts, CI. | Recherches répétées dans un fichier ; plusieurs fichiers d'un projet comme un seul corpus (`--workspace <dir>`) ; usages exacts des composants, variables, styles et images. |

Une question de design ordinaire ne crée jamais d'index. Vous l'activez une fois, explicitement (« crée un index pour ce design ») ; figlens choisit le stockage et n'exige ni SQLite ni autre configuration.

```sh
# Ordinaire : sans état, ne crée rien
figlens search design.fig "checkout button"

# Avancé, une fois : choisir une portée (un id de nœud, ou 0:0 pour tout le fichier) ; la construction s'exécute au premier plan
figlens index prepare design.fig --scope 0:0

# Recherche indexée ; ajouter --node si la portée indexée n'est pas tout le fichier
figlens search --file design.fig "checkout button" --node 0:0

# Après modification ou réexport du .fig : reconstruire seulement les portées dont les octets ont changé
figlens index refresh --file design.fig

# Après déplacement du .fig : garder le workspace, puis refresh (utiliser le workspaceId affiché par prepare)
figlens index relocate <sourceId> new/design.fig --id <workspaceId>
figlens index refresh --id <workspaceId>
```

MCP : `index` prepare renvoie un job ; interroger `jobs` jusqu'à `succeeded`, puis chercher avec le `workspaceId` de la réponse.

```json
{"tool": "index", "arguments": {"action": "prepare", "file": "/abs/path/design.fig", "scope": "0:0"}}
{"tool": "jobs", "arguments": {"action": "status", "jobId": "<jobId from the reply>"}}
{"tool": "search", "arguments": {"workspaceId": "<workspaceId from the reply>", "node": "0:0", "query": "checkout button"}}
```

Cycle de vie : `refresh` reconstruit seulement les portées dont les octets du `.fig` ont changé ; `rebuild` reconstruit même sans changement ; `clear_cache` supprime seulement ce qu'une construction peut recréer et garde le registre et les index publiés ; `reset` supprime la base de ce workspace (jamais le `.fig` ni les exports) et ne s'exécute que sur demande explicite. De nouvelles sources, une portée plus large, reset et recréation exigent chacun leur propre demande explicite.

`.fig` déplacé ? `index relocate` garde `workspaceId`, `sourceId` et le stockage ; faites ensuite `refresh` des portées concernées. Pas de découverte automatique ni de fusion par hash, et aucun reset n'est nécessaire.

Plusieurs designs dans un dossier ? `index add <dir> --workspace DIR` enregistre en une fois jusqu'à 10 fichiers `.fig` de ce répertoire et ignore les liens qui en sortent ; la réponse liste `added`, `existing`, `skipped` et `truncated`, une répétition n'ajoute rien, et chaque fichier a toujours besoin de son propre `prepare --scope`. Un fichier déplacé dans le dossier est une nouvelle source, sauf avec `relocate`. Passez le `nextCursor` de la réponse en `--cursor C` (MCP `cursor`) pour ajouter les 10 suivants. Les curseurs paginés de recherche et de bibliothèque durent une heure ; ensuite ils renvoient `cursor_expired`, il faut donc relancer la recherche.

Données : la base SQLite contient les chemins locaux enregistrés, les noms de calques et le texte effectif des portées choisies, les valeurs des propriétés de composant des instances, les postings de recherche, les relations exactes (imbrication, instances, variantes, liaisons de variables/styles/images, cibles de prototype) et les définitions qu'elles désignent, les hashes, les générations et les jobs. Le `.fig` d'origine reste inchangé où il est ; ses octets bruts ne sont pas stockés dans la base. Les couches des modèles sémantiques et de vision y ajoutent les vecteurs qu'elles intègrent, et l'indexation visuelle que vous activez ajoute des rendus PNG des calques indexés, même sans modèle ; les images de requête ne sont jamais stockées.

Pi, OpenCode et OMP utilisent le même index via leur propre entrée opt-in, et le workspace est réutilisé entre redémarrages et hôtes. Non implémenté : événements de fichier (un watch interroge le fichier, et seulement tant qu'un holder tourne) et événements push.

<a id="semantic-search"></a>

## Recherche sémantique optionnelle

Expérimental. `figlens search <file> <query> --mode semantic|hybrid` classe les calques par le sens avec un modèle local. Le `--mode text` par défaut est inchangé et n'exige aucun modèle.

- **Configuration unique.** Téléchargez vous-même BAAI/bge-m3 à la révision `5617a9f61b028005a4858fdac845db406aefb181` (MIT ; `config.json`, `pytorch_model.bin`, `tokenizer.json`) : figlens ne télécharge jamais. `figlens models configure bge-m3 --model DIR [--runtime FILE]` vérifie les fichiers par SHA-256, démarre une fois le runtime `figlens-embed` hors ligne (cherché à côté de l'exécutable figlens sauf si `--runtime` est donné) et enregistre le profil dans `models.json` du répertoire de configuration. `models list|status` l'affichent ; `models remove bge-m3` l'oublie et garde les fichiers.
- **Profil visuel.** `figlens models configure siglip-base --model DIR` vérifie de la même façon google/siglip-base-patch16-224 à la révision `7fd15f0689c79d79e38b1c2e2e2370a7bf2761ed` (Apache-2.0 ; `config.json`, `preprocessor_config.json`, `model.safetensors`) et enregistre le modèle d’image pour la recherche visuelle.
- **Requêtes par image (expérimental).** `figlens search --workspace DIR --image shot.png` classe les unités d’un workspace selon leur apparence ; `--image-ref '{"sourceId":…,"snapshotId":…,"nodeId":…}'` utilise plutôt le rendu enregistré d’un résultat, et `--crop x,y,w,h` recadre l’un ou l’autre. Avec un texte de requête, le mode devient `hybrid`. Les couches visuelles sont construites par `figlens index visual` (une recherche n’en construit aucune, et une recherche dans un seul `.fig` refuse l’image). Sans profil d’image, ou s’il échoue, seul le hash des pixels classe et `query.usedModes` l’indique. L’image de requête n’est jamais enregistrée. La règle de classement est encore en validation ; traitez les résultats comme des indices. MCP : `search` `image` (un chemin dans les roots), `imageRef`, `crop`.
- **Modes.** `semantic` classe chaque calque par son meilleur fragment de nom, de texte, de propriétés de composant ou de portée. `hybrid` fusionne les classements texte et sémantique (Reciprocal Rank Fusion). Sans profil, `semantic` échoue avec `mode_unavailable` et `hybrid` renvoie les résultats texte en indiquant pourquoi. `coverage.layers.semantic` indique `ready`, `partial` ou `unavailable` ; une page partielle n'a pas de `nextCursor`. Les modèles tournent sur CPU ; Ctrl-C arrête la requête active. Un processus MCP/visualiseur peut garder un modèle texte et un modèle image en mémoire après usage : environ 2,5 Gio pour le texte seul, 3,5 Gio pour la somme de leurs pics dans une mesure Linux ARM, hors noyau et workers d'indexation. Ce sont des mesures, pas des plafonds mémoire.
- **Limites.** Sont encodés les noms de calques, le texte effectif, les valeurs de propriétés de composant d'une instance (une valeur non résolue exactement est omise et comptée) et les noms de page et de conteneurs de chaque calque. La recherche `--workspace`/`--id` lit la couche de modèle stockée de chaque source et n'encode jamais le texte du design ; on la construit avec `figlens index embed` (le serveur MCP la met en file après une construction texte). Une source sans elle est `pending` et la page est partielle. Sur la suite de tests figée de cette fonction, le modèle a manqué le seuil de qualité pour les requêtes de synonymes et d'usage (taux de top-1 erroné 0,25, seuil 0,20) ; considérez les résultats comme des indices.
- **MCP.** L'outil `models` (`list`, `status`, `configure`, `remove`) et `mode` dans `search` ; `configure` ne lance que le `figlens-embed` placé à côté de figlens, et le répertoire et les fichiers du modèle doivent se trouver dans les racines du serveur. Annuler l'appel arrête une recherche ou une configuration en cours.

<a id="commands"></a>

## Référence de la CLI

| Commande | Sortie |
|---|---|
| `tree <file> [node] [--depth N] [--json]` | Ids, types REST (comme `nodes`) et noms des nodes ; texte indenté, ou JSON imbriqué avec la taille de chaque node, son masquage et son mode d'auto layout, plus `firstText` (premier texte visible) sur les nodes d'une page ou d'une section |
| `tree <file> [node] --top-frames [--json]` | Uniquement les écrans, un par ligne, prêts pour `--frames -` : frames, composants, ensembles de composants et instances d'une page ou de ses sections (groupes et pages internes ignorés) ; `--json` ajoute les ids de page et de section et le premier texte visible de chaque écran (`firstText`), qui distingue les libellés du canevas des écrans |
| `find <file> <query> [--type TYPE] [--node ID] [--limit N]` | Recherche insensible à la casse dans les noms de calques et le texte ; chemins, tailles et aperçus. 20 résultats par défaut (1-200) |
| `search <file> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C] [--include-hidden] [--include-internal] [--mode text\|semantic\|hybrid\|visual] [--image PNG\|--image-ref JSON] [--crop x,y,w,h]` | Recherche classée dans les noms de calques et le texte complet, avec overrides d’instance, propriétés texte/booléennes et swaps appliqués ; références exactes de nœuds (y compris les ids de sous-calques `I…;…` pour `inspect`), raisons de correspondance, plages UTF-16, couverture et pages par curseur. 20 par défaut (1-200) ; calques masqués et pages internes exigent leur flag. Avec `--file`/`--workspace`/`--id` au lieu de `<file>`, il lit l'index préparé du workspace (voir `index`) `--mode semantic\|hybrid` exige un modèle configuré : voir [Recherche sémantique optionnelle](#semantic-search). |
| `models list\|status\|configure <profile> --model DIR [--runtime FILE]\|remove <profile>` | Modèle de texte local pour la recherche sémantique (`bge-m3`, expérimental) et modèle d’image pour la recherche par image (`siglip-base`, expérimental) ; `configure` vérifie les fichiers que vous avez téléchargés et ne télécharge jamais ; `remove` les garde. Voir [Recherche sémantique optionnelle](#semantic-search). |
| `usages <file> <node\|resource> [--scope ID] [--relation R[,R]] [--out] [--snapshot S] [--include-hidden] [--limit N] [--cursor C]` | Relations exactes d'un calque ou d'une ressource (`VariableID:…`, `StyleKey:…`, `image:…`) dans une portée enregistrée, instances imbriquées et swaps compris : où un composant, un set, une variable, un style ou une image est utilisé (par défaut), ou ce que désigne un calque (`--out`). Chaque résultat garde sa référence exacte, sa relation, son état (`resolved`, `missing`, `unresolved`, …), sa base et sa preuve ; un nom ne vaut jamais identité. Pas de total : la couverture dit complète ou partielle. 50 par défaut (1-200). Avec `--file`/`--workspace`/`--id` au lieu de `<file>`, lit l'index préparé de `--scope` du workspace. Avec `--out`, un usage d'une copie de bibliothèque en cache donne aussi sa `library` {kind, namespace, key}. |
| `usages --file\|--workspace\|--id … --library <kind> --namespace <ns> --key <key> [--definition-source ID --definition-snapshot SNAP --definition-node NODE] [--source ID] [--scope ID] [--include-hidden] [--limit N] [--cursor C]` | Calques qui utilisent un composant, un style ou une variable de bibliothèque (`kind`), tel que la copie en cache de chaque fichier l'enregistre, dans le `--scope` préparé de toutes les sources (`--source` limite à une). La définition reste `unresolved`, sauf si les trois options `--definition-*` la sélectionnent par référence exacte (`selected`, `keyEvidence: unverified`) ; la couverture donne l'état de chaque source et pourquoi elle est partielle. Prenez-la dans le champ `library` de `usages --out` sur un calque qui l'utilise. |
| `diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]` | Ce qui a changé entre deux révisions enregistrées : une ligne par frame de premier niveau modifié, avec des comptes par facette (texte, layout, apparence, propriétés, structure, liaisons) ; `--json` ajoute les valeurs de champ anciennes/nouvelles et les changements d'edges. Les calques ne se correspondent que par id et type exacts ; un frame de même nom ou de même position n'est qu'un possible match étiqueté, et un côté arrêté avant la fin, ou un calque dont la seule différence est un champ illisible, signale unknown, jamais supprimé ni modifié. Pages de 100 changements (1-500) Un côté arrêté à une limite affiche `stopped early` au lieu de `no changes` ; restreignez avec `--node`. |
| `inspect <file> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat] [--full-text] [--colors]` | Résumé compact de la mise en page/du texte étendus, 200 nodes au plus. `--images` ajoute les dimensions des bitmaps et les emprises de recadrage. `--flat` affiche plutôt une ligne par node : position relative au node, taille, texte, police, remplissage, contour, rayon, auto-layout, alignement, taille fixed/fill, ombres et flous `--full-text` renvoie chaque texte en entier (jusqu'à 10 000 caractères) ; `--colors` ajoute les remplissages unis visibles (hex, opacité, variable liée), aussi avec `--flat` ; ni dégradés, ni images, ni contours. La sortie JSON contient `path`, les ancêtres du nœud depuis le document (`[{id, name}]`, comme dans `find`), y compris pour les sous-calques d'instance. |
| `nodes <file> <node> [--compact [--subtrees]]` | L'objet par nœud d'une réponse REST nodes (`document`, `components`, `componentSets`, `styles`) : instances étendues et overrides, auto-layout, styles de texte, fills, effets, variables liées, propriétés de composants, render bounds. REST inchangé par défaut. `--compact` partage les styles répétés ; ajoutez `--subtrees` pour les sous-arbres répétés en conservant ID et limites par nœud. Voir le [contrat sans perte](../wiki/discovery.md#compact-nodes). Batch: `--frames` / `--ids` + `--compact [--subtrees]`. |
| `nodes <file> --frames <list>`, `--frames -` (stdin), `--ids <node>...` | Un seul décodage, plusieurs nœuds : une ligne JSON par id listé, la sortie de `nodes` avec son `nodeId`, ou `{nodeId, error}` |
| `variables <file> [node]`<br>`variables <file.fig> --format dtcg --collection ID --mode ID --out FILE` | Par défaut : variables liées/locales au format REST inchangé. Export DTCG 2025.10 : IDs exacts de collection et de mode, sans nœud ; couleurs et nombres sans unité uniquement. Créez d’abord le dossier de sortie ; tout diagnostic fait échouer l’export entier sans modifier le fichier de sortie. Une collection de bibliothèque est exportée depuis les copies mises en cache dans le fichier (`libraryCopy: true`). Un alias vers une autre collection exporte la valeur de celle-ci, notée dans `aliasOf` ; si elle a plusieurs modes, indiquez `--alias-mode COLLECTION=MODE`. |
| `bundle <file> <node> <out.zip> [--states] [--hash-source]` | Un bundle de design pour une frame ; affiche `{nodeId, path, warnings}` |
| `bundle <file> --frames <list> --out-dir <dir> [--jobs N] [--states] [--hash-source]`, `--frames -` (stdin) | Plusieurs bundles depuis un seul décodage, en parallèle |
| `verify <bundle> [--fonts <dir>]` | Vérifie un bundle (`.zip` ou dossier extrait) par rapport à son manifeste : fichiers vérifiés, manquants et modifiés, assets manquants, polices à confirmer dans le projet, et `findings` : chaque entrée de handoff manquante, non prise en charge ou non vérifiée, avec ses nœuds, preuves et prochaine action; une police dont le fichier est sur cette machine ou dans `--fonts <dir>` (dossier du projet, qui fournit aussi un `@font-face`) est `local_file`, sans preuve que le navigateur la charge |
| `export <file> <node> <out> [--bounds layout] [--scale N] [--quality 1-100] [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID]]` | SVG, PNG, JPG, WebP ou AVIF ; les PNG jusqu’à 4 194 304 pixels sont automatiquement optimisés sans perte, sans modifier les valeurs RGBA décodées ; les octets encodés d’origine sont conservés si le résultat n’est pas plus petit, et les PNG plus grands gardent l’encodage habituel ; `--max-kb` abaisse la qualité avec perte jusqu’à tenir dans la limite (PNG échoue s’il la dépasse encore, sans solution de repli avec perte) ; `--bounds layout` exporte la boîte de layout au lieu des limites de rendu ; `--match-dir DIR` (SVG uniquement, sous-dossiers compris) liste les SVG qui dessinent les mêmes contours (`matches`) et jusqu’à 10 formes proches avec un score (`similar`) ; `--hide ID[,ID]` dessine ces calques du nœud comme masqués sur la même zone, si bien que la comparaison avec l’export normal montre où ce calque, tel que figlens le dessine, modifie le résultat (l’absence de différence ne prouve pas que le rendu ignoré est sans effet) |
| `export <file> --frames <list> --out-dir <dir> [--format F] [--scale N] ...`, `--frames -` (stdin) | Un décodage, plusieurs exports (`png` par défaut) : écrit `<dir>/<nodeId>.<format>`, une ligne JSON par id, la sortie d’`export` avec son `nodeId`, ou `{nodeId, error}` |
| `export <file> --batch <items.json>`, `--batch -` (stdin) | Un décodage, plusieurs exports, chacun avec ses options : un tableau JSON de 1 à 64 `{node, out, bounds?, scale?, quality?, maxKb?, maxSize?, hide?}` (MCP `export_batch`). Affiche un manifeste `{count, failed, items}` : par élément, la sortie d'`export` ou `{error}` (les autres sont tout de même exportés) ; un élément mal formé ou un `out` répété échoue avant toute écriture |
| `preview <file> [node] [--out preview.html] [--port N] [--bind IP] [--idle-exit SECONDS]` | Viewer local qui n’écrit jamais le design ; recherche sémantique/hybride après Enable index ; `--out` écrit un snapshot hors ligne autonome ; les résultats et le context copié indiquent la page et, en direct, le chemin du fichier, le snapshot du source chargé et la revision locale (inconnus hors ligne, où le chemin n'est jamais intégré) ; le context porte aussi la coverage du scope de référence, `complete: null` si non vérifiée. Visual/Hybrid accepte un PNG de référence ou un calque sélectionné avec recadrage en pixels. Activez explicitement l’index visuel ; les images de recherche ne sont pas conservées. `--idle-exit SECONDS` l’arrête après ce délai sans page ouverte. |
| `compare <file> <nodes.json> <node>` | Diff par rapport à un export REST de la même frame |
| `compare-image <file> <node> <screenshot.png>` | Compare une capture de l’écran construit au node rendu à son échelle (`--scale`, `--limit`, `--out diff.png`, `--full-page` pour une capture pleine page plus courte que la frame, `--ignore` pour les calques dont le contenu vient des données, `--threshold` pour voir les bordures claires ; `image: true` marque les remplissages d’image ; MCP `compare_image`). Affiche `mismatch`, `nonTextMismatch` et les plus grandes régions différentes en pixels du node, chacune avec les calques les plus profonds dessous ; `text: true` signale une région sur du texte (souvent une police de substitution ; vérifiez chacune) |
| `node <file> <node>` | Un node `.fig` brut |
| `clipboard [copied.html] <out.fig>` | Quand il n'y a pas de `.fig` enregistré : sélectionnez des calques dans Figma, copiez-les (clic droit > Copy, ou Cmd/Ctrl+C), puis lancez ceci pour transformer le presse-papiers en `.fig` que toutes les commandes lisent ; collé ailleurs, il ne donne que le texte des calques. Lit le presse-papiers du système (macOS ; Windows PowerShell 5.1 et Linux `wl-paste`/`xclip` non vérifiés) ou un fichier HTML de presse-papiers enregistré. Mêmes ids de nœud, composants utilisés inclus. Signale `missingImages`, `missingComponents` et `layersWithoutOutlines` : une copie ne contient ni octets d'image ni contours vectoriels, donc exports et bundles avertissent là où ils dessinent sans eux. Ne remplace jamais un fichier existant. Avec un `.fig` enregistré, passez plutôt le lien Figma comme nœud : il conserve images et vecteurs |
| `freshness [<file.fig> [--url URL] \| --sources <mapping.json>]` | Indique si chaque copie enregistrée est plus ancienne que son fichier Figma en ligne, avant de conclure qu'il manque un frame au design : les designs du `.figlens/sources.json` le plus proche (une liste de `{name, url, file}`, `file` relatif à la racine du dépôt ; une correspondance `--sources` hors de `.figlens/` utilise son propre dossier), un fichier, ou un fichier avec son URL. Compare `exported_at` du `meta.json` de la copie au `last_touched_at` en ligne, lu avec `curl` et un personal access token dans `FIGMA_TOKEN` (scope `file_metadata:read`) ; affiche `stale`, `current` ou `unknown` avec une raison et n'échoue jamais faute de token ou de réseau. Avec un token, un nœud introuvable ou un `find` vide dans une copie associée et périmée indique aussi d'enregistrer une nouvelle copie locale (MCP `freshness`) |
| `pull [--only NAME]... [--stale] [--sources <mapping.json>] [--browser B] \| --login [--browser B] \| --logout` | Actualise les copies enregistrées associées à partir du `.figlens/sources.json` le plus proche (ou de `--sources`). Sans `--only` ni `--stale`, il ne télécharge rien et liste la fraîcheur de chaque design. `--only` choisit les designs, `--stale` ne récupère que les copies périmées ou manquantes (exige `FIGMA_TOKEN`). Un profil Chrome dédié exécute **File > Save local copy** ; le téléchargement doit se décoder et porter un `exported_at` pas plus ancien que la copie actuelle, puis il la remplace de façon atomique ; si une vérification échoue, l'ancienne copie est conservée ; un échec indique son étape (`open`, `editor`, `menu`, `export`, `download`, `install`). `--login` ouvre une fenêtre visible pour se connecter (un pull l'ouvre aussi quand la session a expiré) ; s'y connecter peut vous déconnecter de Figma sur un autre appareil. `--logout` supprime les profils du navigateur, ce qui déconnecte figlens de Figma, et s'exécute seul. Nécessite Node, Chrome et soit `playwright-core` (`npm i -g @runkids/figlens playwright-core`), soit la CLI [`agent-browser`](https://agent-browser.dev/) dans le `PATH` ; `--browser playwright\|agent-browser` en choisit un (par défaut : `playwright-core` s'il est trouvé), et chacun garde son propre profil, donc changer impose de se reconnecter ; interface Figma en anglais uniquement. CLI uniquement |
| `index prepare\|refresh\|rebuild\|clear\|status\|add\|remove\|relocate\|reset\|track\|untrack\|embed\|visual` | Workspace local dans le répertoire d'état figlens de l'utilisateur, ou sous un `--storage DIR` que le workspace mémorise ensuite. `prepare <file> --scope NODE` construit une fois l'index de recherche de cette portée ; ensuite `search --file <file>` (ou `--workspace DIR`) répond sans réanalyser, avec un classement commun aux sources du workspace. `refresh` reconstruit les fichiers modifiés, `rebuild` tous ou une `--source`/`--scope`, `clear` supprime les index inutilisés ; l'index précédent sert jusqu'à ce que le nouveau soit prêt. `reset` supprime uniquement la base de données du workspace, jamais les designs, les exports ni l'emplacement mémorisé. `track --scope NODE` enregistre les modifications de ce scope à partir d'un build baseline ; `untrack` l'arrête en gardant l'historique. `embed` encode les portées publiées avec le modèle configuré (CPU et mémoire) ; ne le lancez que pour une recherche sémantique de workspace. `visual --scope NODE` rend les écrans et composants de cette portée publiée pour la recherche par image et les encode si un profil de vision est configuré ; ne le lancez que pour une recherche visuelle de workspace. |
| `jobs status\|cancel <jobId>` | Afficher ou annuler une construction d'index ; `cancel` rend la main une fois la construction arrêtée |
| `changes --file <file>\|--id ID\|--workspace DIR [--source ID] [--scope NODE] [--consumer NAME\|--subscription ID\|--cursor C] [--revision R] [--limit N]` | Modifications enregistrées des scopes suivis, de la plus ancienne à la plus récente : une révision par build publié (baseline ou changes) avec couverture et comptes par type de changement et facette ; chaque révision changes a un résumé `impact` des calques que ses changements atteignent (null avant le schéma 7) ; `--revision` pagine les nodes modifiés de cette révision, puis les calques que liste son impact (`entry` indique lequel) ; `tracking` indique quand un scope a besoin de `index refresh` pour enregistrer un changement. Lecture seule |
| `watch ack --file <file>\|--id ID\|--workspace DIR --consumer NAME\|--subscription ID --cursor C [--source ID] [--scope NODE]` | Avance le checkpoint d'un consommateur jusqu'au `nextCursor` de la dernière page `changes` traitée avec les mêmes filtres ; le répéter est sans risque et il n'avance que vers l'avant, sauf qu'un curseur de reprise actuel ramène en arrière un checkpoint resté derrière une suppression pour relire les révisions conservées ; un curseur émis avant que la rétention supprime des révisions qu'il n'avait pas dépassées donne `event_gap` et n'écrit rien |
| `watch run --file <file>\|--id ID\|--workspace DIR --scope NODE [--source ID] [--request-id R]` | Tient au premier plan un lease sur un scope suivi : le runner partagé du workspace le rafraîchit à chaque enregistrement (interrogation toutes les 250 ms, vérification du contenu toutes les 60 s), donc `changes` montre les nouvelles révisions sans `index refresh`. Affiche le lease, puis une ligne de fin quand stdin se ferme, sur Ctrl-C ou SIGTERM, ou quand le scope n'est plus suivi. MCP `watch` `start` et `stop` tiennent des leases tant que le serveur tourne ; un lease non renouvelé prend fin après 30 s et le runner se termine après le dernier |
| `watch status --file <file>\|--id ID\|--workspace DIR` | Les leases du workspace (`active`, `unauthorized`, `untracked`, `expired`, `ended`) et son runner (`running`, `stopping`, `stopped`, `unknown`, avec son placement) ; n'écrit jamais |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]` | Suit une source, un scope ou un calque exact (`--node`) depuis sa révision la plus récente avec son propre checkpoint, lu par `changes --subscription` et acquitté par `watch ack --subscription` ; un calque absent ou ambigu donne `target_missing` ou `identity_uncertain`, jamais un calque du même nom ; `--owner` la rend temporaire pour cette tâche |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --source ID --query TEXT [--type T[,T]] [--scope NODE]` | Enregistre une recherche sur le scope d'une source : ses 50 premiers sont évalués tout de suite ; `changes --subscription` lit les évaluations stockées et n'en calcule jamais |
| `watch subscribe --file <file>\|--id ID\|--workspace DIR --consumer NAME --sources visible\|ID[,ID] --query TEXT\|--library KIND --key KEY [--namespace NS] [--definition-source ID --definition-snapshot SNAP --definition-node NODE]` | Enregistre une recherche, ou les utilisations d'un élément de bibliothèque avec leur `definitionMatch`, sur un ensemble fixe d'au plus 10 sources (`visible` est résolu maintenant ; les sources ajoutées ensuite n'y entrent jamais) : `delta.changedSourceIds` nomme les sources aux octets nouveaux, `corpus_changed` un changement de l'ensemble, `coverage.transitive` vaut `not_followed` et `delta.notFollowedSourceIds` nomme les sources modifiées sans utilisation, dont les dépendants n'ont pas été revérifiés ; retirer un membre la désactive (unsubscribe, puis subscribe à nouveau) |
| `watch evaluate --file <file>\|--id ID\|--workspace DIR --subscription ID` | Stocke l'évaluation d'un abonnement de requête sur l'index actuel du scope, sauf s'il est déjà évalué ; un watch sur le scope le fait après chaque publication |
| `watch list --file <file>\|--id ID\|--workspace DIR [--consumer NAME] [--after ID] [--subscription ID]` | Liste les abonnements visibles et leur état, une page à la fois ; `--subscription` ne lit que celui-ci (vide s'il n'est pas visible) |
| `watch unsubscribe --file <file>\|--id ID\|--workspace DIR --subscription ID\|--consumer C --owner NAME` | Supprime un abonnement et son checkpoint, ou les abonnements temporaires d'un owner ; le suivi et l'historique restent |
| `watch forget --file <file>\|--id ID\|--workspace DIR --consumer NAME [--source ID] [--scope NODE]` | Supprime les checkpoints bruts d'un consumer ; un consumer garde au plus 64 abonnements et checkpoints, un workspace 4096, et rien n'est évincé automatiquement |
| `mcp` | Serveur MCP local via stdio |
| `update` | Remplace le binaire par la dernière release |

Pour les variantes de composants liées, utilisez `inspect --states` ou `bundle --states` (MCP : `states: true`, false par défaut ; aucun changement de configuration). Les preuves conservent les sources et les marqueurs de données inconnues ou tronquées. Les bundles ajoutent `design/states.json` et un résumé dans `context.md`. Voir la [preuve des états](../wiki/states.md) pour le périmètre et les limites.

Comparez deux variantes du même component set avec `inspect <file> <variantA> --compare-variant <variantB>` (MCP `compareVariant`). `variantComparison` décrit les différences de champs et ensembles de valeurs stockés avec sources et inconnues, Les enfants sont appariés par nom unique, puis par composant principal, puis par ordre d'empilement dans des groupes de même nom et de même taille (`pairedNodes`, avec les changements par paire ; vérifiez `pairedBy`, `order` est l'indice le plus faible ; le reste n'est pas apparié), et les variables liées sont nommées. Voir les [limites](../wiki/states.md#compare-two-variants).

Pour comparer deux nœuds quelconques d'un même fichier (écran vide et rempli, breakpoints mobile et bureau, deux écrans partageant un en-tête), utilisez `inspect <file> <nodeA> --compare <nodeB>` (MCP `compare`). `comparison` a la même forme, plus `rootDelta` (taille et position relative au parent) ; les enfants sont appariés par nom unique, puis par composant principal, puis par ordre d'empilement dans des groupes de même nom et de même taille (`pairedBy` l'indique), le reste demeure non apparié avec `reason: ambiguous-name`, les instances ne sont pas développées et un nœud ne se compare pas à son propre ancêtre. `--compare-variant` exige toujours le même component set.

Un argument node accepte `12:345`, `12-345` ou une URL Figma avec `?node-id=`. `<file>` peut aussi être `gs://bucket/object`, téléchargé avec votre connexion `gcloud` et mis en cache. Consultez le [guide des snapshots GCS](../wiki/gcs.md) pour la transmission via un objet fixe, les droits requis et les limites d’actualisation. Chaque appel de la CLI décode le fichier ; MCP ne réutilise que le dernier fichier local inchangé. Les appels distants relancent l’analyse. Lancez `figlens --help` pour l’aide complète de la CLI, ou `figlens inspect --help` (`-h` aussi) pour l’entrée d’une commande. `index add gs://bucket/object` enregistre une copie épinglée de cette génération et renvoie `remote {uri, generation}` ; seul add lance gcloud.

`inspect` inclut le redimensionnement du texte, la troncature, le nombre maximal de lignes et les décorations enregistrés. Les annotations Dev Mode figurent sur leurs nœuds dans `annotations` (aperçu de `label` en texte brut, `properties` épinglées, `categoryId` ; `nodes` ajoute `labelMarkdown`) avec les `annotationCategories` nommées ; les frames et sections Ready for dev portent `devStatus`, et `--flat` marque les deux ; les mesures sont signalées dans `handoffDiagnostics`. Un parent sans auto-layout porte `childSpacing: "derived"` : les écarts entre ses enfants sont des différences de coordonnées, pas une règle d'auto-layout ; `constraints` peut encore les fixer. `handoffDiagnostics` signale les données sémantiques enregistrées mais non émises, avec des comptes, des exemples et des [limites de couverture](../wiki/semantics.md#known-handoff-gaps). `_figlens.semantics` ne liste que les champs dont la source est enregistrée (une entrée manquante signifie qu’elle ne l’est pas), et chaque réponse indique une seule fois, dans `_figlens.semantics.verification` au niveau supérieur, que ces correspondances ne sont pas comparées à Figma. Les frames en grid auto layout portent le nombre de lignes et de colonnes, les écarts et les tailles de pistes enregistrées (`_figlens.gridTracks`). Les sorties protègent la source et remplacent la destination après une écriture complète. MCP limite les lignes de requête à 1 MiB et les réponses complètes à 8 MiB ; un dépassement renvoie une erreur explicite. Le JSON complet reste disponible via la CLI.
