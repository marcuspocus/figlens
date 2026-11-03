use std::error::Error;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use figlens::compare::{FIELDS, compare};
use figlens::variables::variables;
use figlens::{Document, Tree, normalize_id};

#[cfg(test)]
#[path = "test_support.rs"]
mod test_support;

mod command;
mod contract;
mod mcp;
mod workflow;

const USAGE: &str = "figlens — read Figma .fig files (File > Save local copy) without Figma

usage:
  figlens tree <file.fig> [node] [--depth N] [--json] [--top-frames]
                                             list ids, REST types (as in nodes) and names (default root:
                                             the document);
                                             --json prints them as nested
                                             {id,type,name,size,visible,layoutMode,children}: size is the local
                                             [width,height] (none on the document and pages), visible only when
                                             false, layoutMode only on auto-layout frames, as in nodes;
                                             nodes on a page or in a section add firstText, their first
                                             visible stored text (200 chars max), to tell labels from screens;
                                             --top-frames lists only the screens, at any depth: frames,
                                             components, component sets and instances directly on a page or in
                                             a section on one (internal pages skipped); --depth does not apply;
                                             each line starts with the id, so it pipes to --frames -; with
                                             --json, a flat array that adds page and section ids
                                             (entries carry firstText too)
  figlens find <file.fig> <query> [--type TYPE] [--node ID] [--limit N]
                                             search stored layer names and text, case-insensitive;
                                             default 20 results (1-200), with paths and truncation flag
  figlens search <file.fig> <query> [--node ID] [--type T[,T]] [--limit N] [--cursor C]
          [--include-hidden] [--include-internal] [--mode text|semantic|hybrid]
                                             ranked search of names and full text in one stored scope
                                             (default: the document), with instance overrides, text and
                                             boolean properties and swaps applied; exact refs, match
                                             reasons, UTF-16 spans, coverage and cursor pages (20, 1-200);
                                             hidden layers and internal pages need their flag
                                             --mode semantic ranks layers by meaning and hybrid fuses
                                             text and semantic ranks (RRF), both with a model set up by
                                             `figlens models configure`; without one semantic fails with
                                             mode_unavailable and hybrid answers as text; experimental;
                                             a workspace search reads the layer `index embed` stores
  figlens search --workspace DIR|--file <file.fig>|--id ws_... <query> [--source ID]... [--storage DIR] [same options]
                                           (put -- before a query that starts with -)
                                             the same search over a workspace's prepared indexes of
                                             --node (default 0:0) across its sources, without parsing;
                                             sources not ready are listed with the step that prepares them
          [--image PNG | --image-ref JSON] [--crop x,y,w,h] [--mode visual|hybrid]
                                             workspace only, experimental: rank units by how they look
                                             like a local PNG or a stored unit render (sourceId,
                                             snapshotId and nodeId of a result); the query image is
                                             never stored. With query text the mode is hybrid; the visual
                                             layers come from `index visual`, and a search builds none
  figlens models list|status                 the local model profiles: bge-m3 for semantic search,
                                             siglip-base for visual search (--image);
                                             whether each is configured
  figlens models configure <profile> --model DIR [--runtime FILE]
                                             check the profile's files you downloaded (pinned revision,
                                             SHA-256), start the figlens-embed runtime once offline
                                             (default: next to figlens) and record the profile; never
                                             downloads
  figlens models remove <profile>            forget the profile, keep its files
  figlens usages <file.fig> <node>|<resource> [--scope ID] [--relation R[,R]] [--out]
          [--snapshot S] [--include-hidden] [--limit N] [--cursor C]
                                             exact relations of a node or resource (VariableID:...,
                                             StyleKey:..., image:...) in one stored scope (default: the
                                             document), nested instances and swaps included: instance_of,
                                             variant_of, binds_variable, uses_style, uses_image,
                                             prototype_target, contains (asked for); in by default, --out
                                             for what it points at; state, basis and evidence per edge;
                                             --out names a cached library copy's library {kind,
                                             namespace, key}, the selector --library takes;
                                             coverage and cursor pages (50, 1-200), never a total
  figlens usages --workspace DIR|--file <file.fig>|--id ws_... <node>|<resource> [--source ID] [--storage DIR] [same options]
                                             the same over one source's prepared index of --scope,
                                             without parsing; a changed source is source_stale
  figlens usages --workspace DIR|--file <file.fig>|--id ws_... --library KIND [--namespace NS] --key KEY
          [--definition-source ID --definition-snapshot S --definition-node N]
          [--source ID] [--storage DIR] [--scope ID] [--include-hidden] [--limit N] [--cursor C]
                                             layers that use one library component, style or variable
                                             (KIND), as each source's cached copy records it, across
                                             every visible source's prepared --scope (--source: one);
                                             the definition stays unresolved unless --definition-*
                                             selects one by exact ref (checked, keyEvidence unverified);
                                             without --namespace only copies naming none, never
                                             complete; per-source coverage
  figlens diff <old.fig> <new.fig> [--node ID] [--limit N] [--cursor C] [--json]
                                             what changed between two saved revisions: one line per
                                             changed top-level frame (added, removed, changed, kind_changed,
                                             unknown) with counts per facet (text, layout, appearance,
                                             properties, structure, bindings); --json adds per-node old/new
                                             field values and edge changes; layers match by exact id and
                                             type only, a same-name or same-place frame is only a possible
                                             match; pages of 100 changes (1-500), a cursor rebuilds both;
                                             a side stopped at a limit prints `stopped early`, never
                                             `no changes`; narrow it with --node
  figlens inspect <file.fig> <node> [--depth N] [--images] [--states] [--compare-variant ID] [--compare ID] [--flat]
                                             [--full-text] [--colors]
                                             compact layout/text summary, instances expanded;
                                             depth 1 (0-10), at most 200 nodes, 200 characters per text;
                                             recorded text resize/truncation/decoration and known handoff gaps;
                                             Dev Mode annotations (label previews) and Ready for dev devStatus;
                                             childSpacing: derived marks gaps that are coordinates, not auto-layout;
                                             --images adds source pixels/bytes and source-pixel crop rectangles;
                                             --states lists related component variants and evidence gaps;
                                             --compare-variant compares two stored variants in the same set
                                             (children pair by unique name, main component or
                                             stacking order, see pairedBy; bound variables are named);
                                             --compare ID does the same for any two nodes of the file
                                             (frames, breakpoints, states), adds rootDelta, and marks
                                             children it cannot pair unpaired as ambiguous-name;
                                             --full-text returns each text in full (up to 10000 characters);
                                             --colors adds solidFills (hex, opacity, bound variable) for visible
                                             solid fills, also on --flat; no gradients, images or strokes;
                                             --flat prints one line per node instead (not with the above):
                                             id TYPE \"name\" @x,y WxH relative to the node, then hidden, rot,
                                             component, devStatus=…, annotations=N (count only), text,
                                             font=family size/weight/lineHeight, fill,
                                             stroke=paint/weight, r, shadow|inner-shadow=x,y,blur[,spread],color,
                                             blur, bg-blur, H|V gap or GRID RxC gap=row,col rows=… cols=…
                                             (stored sizes: flexN, px, hug or min..max), pad=top,right,bottom,left,
                                             align=main/cross and size=w/h (fixed|fill) unless min and hug,
                                             or spacing=derived, span=rows,cols on grid children,
                                             and opacity when set; children=N and
                                             `… N more` mark omissions
  figlens nodes <file.fig> <node>              print a frame as the per-node object of a Figma REST
                                             nodes response: {document, components, componentSets,
                                             styles, schemaVersion, _figlens}; instances expanded,
                                             fills/text/auto-layout, Dev Mode annotations and devStatus included
                     [--compact [--subtrees]] opt-in lossless style references; --subtrees also shares
                                             repeated templates with per-node ids and bounds;
                                             default REST shape unchanged
  figlens nodes <file.fig> --frames <list|-> | --ids <node>...
                                             one decode, many nodes: <list> has one node id per line
                                             (- reads stdin), or name ids after --ids (at most 10000);
                                             prints one JSON line per id, the nodes output with its
                                             nodeId, or {nodeId,error}; --compact [--subtrees] also works
  figlens variables <file.fig> [node]          print the variables a frame binds (design/variables.json);
                                             without a node, every local variable
  figlens variables <file.fig> --format dtcg --collection ID --mode ID --out FILE
                     [--alias-mode COLLECTION=MODE ...]
                                             write DTCG 2025.10 color/number/alias tokens for exact IDs
                                             from variables (a library collection exports from its
                                             copies cached in the file, marked libraryCopy);
                                             an alias into another collection exports its value in that
                                             collection's only mode, or the --alias-mode one, recorded
                                             in aliasOf;
                                             no node, workspace or index needed;
                                             any diagnostic fails without publishing partial tokens;
                                             prints {path,collectionId,modeId,tokenCount,diagnostics}
  figlens bundle <file.fig> <node> <out.zip> [--states] [--hash-source]
                                             write a design bundle for a frame: nodes, variables,
                                             image/SVG assets, reference render and context.md, in one zip;
                                             prints {nodeId,path,warnings}
  figlens bundle ... --file-key KEY --name N   set the manifest's file key and frame name;
                                             --states adds related variants and evidence gaps to the bundle;
                                             --hash-source records the .fig's sha256 as manifest
                                             source.sha256 (one extra full read of the file)
  figlens bundle <file.fig> --frames <list|-> --out-dir <dir> [--file-key KEY] [--jobs N] [--states]
                                             [--hash-source]
                                            one decode, many bundles: <list> has one node id per line
                                             (- reads stdin);
                                             writes <dir>/<nodeId>.zip (`:` as `-`), prints one JSON line
                                             per frame ({nodeId,path,warnings} or {nodeId,error});
                                             --jobs limits concurrent frames (0/default: all cores), not rendering
  figlens export <file.fig> <node> <out> [--bounds layout|render] [--scale N] [--quality 1-100]
          [--max-kb N] [--max-size N] [--match-dir DIR] [--hide ID[,ID...]]
                                            render a node to out.png|jpg|webp|avif|svg (default 1x,
                                             quality 80); SVG keeps vectors, scale/quality do not apply;
                                             PNGs up to 4,194,304 pixels are losslessly optimized;
                                             keeps original encoding unless smaller; larger PNGs unchanged;
                                             --max-kb lowers lossy quality until it fits;
                                             --max-size scales raster output to fit its longest side;
                                             --bounds layout exports the node's own box instead of
                                             what it draws (shadows, outside strokes, overflow);
                                             prints {path,width,height,bytes,quality,layoutBox,
                                             renderBox,warnings}; warnings say when the boxes differ and
                                             name skipped or approximated rendering, as bundle's do;
                                             --match-dir (SVG only) compares with the .svg files under DIR
                                             (subfolders included) to find an icon the project already has:
                                             matches lists those drawing the same outlines, ignoring position,
                                             paint and path syntax; similar lists up to 10 {file,score} (0.8-1)
                                             drawing a similar shape, stroked or at another size, to confirm
                                             by eye (1 for the same outlines at another size);
                                             --hide draws those layers inside the node as hidden over
                                             the same area (prints hidden): diff it with the plain
                                             export to see where that layer changes the result (no
                                             difference does not prove skipped rendering harmless)
  figlens export <file.fig> --batch <items.json|->
                                             one decode, many exports, each with its own options: a JSON
                                             array (- reads stdin) of 1-64 {node,out,bounds?,scale?,quality?,
                                             maxKb?,maxSize?,hide?} (MCP export_batch's items; format from out's
                                             extension, no matchDir); a malformed item or a repeated out fails
                                             the command before any write; prints {count,failed,items}: per
                                             item node, path and the export output, or node, path and error
                                             (an item's failure does not stop the others)
  figlens export <file.fig> --frames <list|-> --out-dir <dir> [--format png|jpg|webp|avif|svg] ...
                                             one decode, many exports (default png, same options):
                                             writes <dir>/<nodeId>.<format> (`:` as `-`), prints one
                                             JSON line per id, the export output with its nodeId, or
                                             {nodeId,error}; at most 10000 ids
  figlens verify <bundle.zip|dir> [--fonts <dir>]
                                             check a bundle, zipped or extracted, against its manifest:
                                             verified/missing/changed files, missing assets and the font
                                             faces: local_file when a font file with that name is found in
                                             this machine's font folders or --fonts <dir> (a project
                                             folder; its matches also get an @font-face), else
                                             unverified; neither shows the browser loads it. Plus
                                             findings: one entry per missing, unsupported or
                                             unverified handoff input with node ids, evidence and action
  figlens preview <file.fig> [node] [--out preview.html] [--port N] [--bind IP]
                                             local viewer that never writes the design (loopback,
                                             random port by default);
                                             --out requires a node and writes an offline snapshot;
                                             live, it also runs ranked search and exact usages of
                                             the loaded revision (snapshots search their layers only);
                                             Semantic/Visual/Hybrid searches an explicitly enabled index;
                                             choose a PNG or selected layer, optionally crop or add text;
                                             Enable visual index stores source renders, never query images;
                                             local model setup: models status; Escape cancels a model query;
                                             results and copied context name the page and, live,
                                             the loaded file path, source snapshot and local
                                             revision (offline all are unknown; no path is
                                             embedded); context carries the reference scope's
                                             coverage (complete null = unchecked)
  figlens node <file.fig> <node>               print one raw .fig node as JSON
  figlens compare <file.fig> <nodes.json> <node>   diff a frame against a REST nodes.json export
  figlens compare <file.fig> --frames <list.txt>   same for every `node|nodes.json[|variables.json]` line
  figlens compare-image <file.fig> <node> <screenshot.png> [--scale N] [--limit N] [--out diff.png] [--full-page]
                       [--ignore <id>[,<id>...]] [--threshold N]
                                             compare a PNG screenshot of the built screen with the node
                                             rendered over its layout box at the screenshot's scale (its
                                             width / the node's, or --scale); prints sizes, mismatch,
                                             nonTextMismatch and the largest differing regions (default
                                             10, 1-50) in node pixels from its top-left, each with the
                                             deepest layers under it, text: true when all are TEXT
                                             (often font fallback; review each) and image: true when only
                                             image fills; --ignore leaves layers inside the node (content
                                             from data) out of every count; --threshold (1-254, default 48)
                                             lowered to 16 sees light borders; rows past the node's height are
                                             reported, not compared; a shorter one is refused unless
                                             --full-page states it is the whole page: then its rows are
                                             compared and the missing ones are a differing region;
                                             --out writes a diff PNG
  figlens mcp [--root DIR]... [--storage-root DIR]...
                                             serve workflow guidance, tree, find, search, usages, diff, inspect,
                                             nodes, variables, bundle, export, compare_image, index, jobs,
                                             models, changes and watch as MCP tools; raster export returns an inline image;
                                             with --root, files and outputs must be under one of the folders;
                                             a custom index storage must be under a --storage-root
  figlens workflow <topic>                     print built-in agent guidance, the same text as MCP workflow:
                                             build, audit, images, search, plan, planning-details or
                                             yolo; a workflow:<topic> link means `figlens workflow <topic>`
  figlens cache [clear]                        show, or clear, the preview render cache
                                             (prints {path,bytes} or {path,freed})
  figlens index prepare <file.fig> [--workspace DIR] [--storage DIR] [--scope NODE] [--request-id ID]
                                             create or reuse a local workspace and register the file;
                                             source-scoped, or the --workspace project directory's; with
                                             --scope, build and publish that node's search index (0:0 for the
                                             whole file), progress on stderr; prints workspaceId, sourceId,
                                             incarnation, job, paths and roots. --replace-unavailable with a new
                                             --storage recreates an unusable workspace there, old files kept
  figlens index refresh <select> [--source ID] [--scope NODE] [--request-id ID]
                                             rebuild the published scopes whose .fig changed
  figlens index rebuild <select> [--source ID] [--scope NODE] [--request-id ID]
                                             rebuild published scopes even when unchanged; the old index
                                             serves until then. A reply with admission partial lists
                                             deferred items, each with the call that builds it later
  figlens index embed <select> [--source ID] [--scope NODE] [--request-id ID]
                                             embed published scopes with the configured local model
                                             (figlens models) for semantic search; text search keeps
                                             serving. A refresh with a model configured lists this step
  figlens index visual <select> --scope NODE [--source ID] [--request-id ID]
                                             render that published scope's screens and components (and
                                             the scope node below a page) for image search, and embed them
                                             with a configured vision profile; text search keeps serving.
                                             Run again, it resumes a partial layer (retrying failed units).
                                             A CLI build of a scope that has a visual layer lists this step
  figlens index clear|clear_cache <select>   delete retired and unfinished indexes; keeps published ones
  figlens index status <select>              state, sources and storage rule; creates nothing
  figlens index add <file.fig|dir> <select> [--dry-run | --manifest HEX] [--cursor C]
                                             add a source to a project workspace; a directory adds the
                                             .fig files under it (10 per call in path order, 4096 entries
                                             read), skipping links that lead out of it, and reports added,
                                             existing, skipped, truncated and nextCursor (--cursor adds the
                                             next 10); --dry-run lists that page only, with a manifest;
                                             --manifest adds nothing if that page changed
  figlens index add gs://bucket/object <select>
                                             download the object pinned to its current generation (gcloud,
                                             network) and register that copy, reporting remote {uri,
                                             generation}; the only index call that reaches GCS: refresh,
                                             status, search and watch read the copy. Add it again to fetch a
                                             newer upload under the same sourceId
  figlens index remove <sourceId> <select>   unregister a source; the .fig is never touched
  figlens index relocate <sourceId> <new.fig> <select>   keep the sourceId, change its path; a
                                             source-scoped workspace is then selected by the new path too
  figlens index reset <select>               remove the workspace database once no other figlens process
                                             uses it (else workspace_busy); designs, .figlens/, exports and
                                             other workspaces stay; prepare starts a new incarnation
                                             <select>: --workspace DIR, --file <file.fig> (its source-scoped
                                             workspace) or --id ws_...; every action accepts --storage DIR.
                                             Storage: --storage DIR (absolute; uses DIR/state, cache, data),
                                             else the location a prepare with --storage remembered for this
                                             workspace (an error when unavailable), else each XDG_*_HOME/figlens,
                                             else ~/.local/state/figlens etc. (Windows %LOCALAPPDATA%\\figlens\\state).
                                             Config, holding the remembered locations, never moves; reset keeps them
  figlens jobs status|cancel <jobId> [--storage DIR]
                                             an index job's state and progress, or stop it; cancel returns
                                             once the build stopped, and the previous index keeps serving
  figlens index track|untrack <select> --scope NODE [--source ID] [--request-id ID]
                                             record a published scope's design changes on each build of it
                                             (track builds a baseline first), or stop; history stays
  figlens changes <select> [--source ID] [--scope NODE] [--consumer NAME|--subscription ID] [--cursor C] [--limit N] [--storage DIR]
                                             recorded revisions after the cursor, the consumer's or subscription's
                                             checkpoint or the oldest kept, with tracking state; never writes;
                                             each with its impact (the layers its changes reach).
                                             --revision R [--cursor C]: one revision's changed nodes,
                                             then its impact's listed layers;
                                             a query --subscription pages its stored evaluations
  figlens watch ack <select> --consumer NAME|--subscription ID --cursor C [--source ID] [--scope NODE] [--storage DIR]
                                             move the consumer's checkpoint for these filters to C; an
                                             older C normally leaves it; a C issued before retention
                                             deleted revisions it had not passed is event_gap; a
                                             current recovery C moves a checkpoint behind such a
                                             deletion back to C, so the kept revisions replay
  figlens watch subscribe <select> --consumer NAME --source ID [--scope NODE] [--node ID] [--owner NAME]
                                             follow a source, scope or exact layer from its newest
                                             revision, with its own checkpoint (read with changes
                                             --subscription); --owner makes it temporary for that task
          [--query TEXT [--type T[,T]]]      or a saved search of the scope (default 0:0): its top 50
                                             are evaluated now, on watch evaluate and by a watch
                                             runner serving the scope; changes reads the
                                             evaluations, never computes one
          [--sources visible|ID[,ID]]        instead of --source: the saved question reads this fixed
                                             set (at most 10; visible is resolved now, never widened);
                                             causes name changed sources, corpus_changed a member's
                                             relocation, publication or shared statistics; removing a
                                             member disables it: unsubscribe, then subscribe again
          [--library KIND [--namespace NS] --key KEY [--definition-source ID --definition-snapshot S
           --definition-node ID]]            or save a library question: its uses' definitionMatch,
                                             the definition followed by node id; transitive library
                                             dependencies are not followed
  figlens watch evaluate <select> --subscription ID
                                             store a query subscription's evaluation of the scope's
                                             current index, unless it is already evaluated
  figlens watch list <select> [--consumer NAME] [--after ID] [--subscription ID]
                                             visible subscriptions and their state, a page at a time;
                                             --subscription reads only that one (empty if unknown)
  figlens watch unsubscribe <select> --subscription ID|--consumer C --owner NAME
                                             remove one subscription, or an owner's temporary ones
  figlens watch forget <select> --consumer NAME [--source ID] [--scope NODE]
                                             remove a consumer's raw checkpoints; neither untracks
  figlens watch run <select> --scope NODE [--source ID] [--request-id R]
                                             hold a lease on a tracked scope while this runs: a shared
                                             background runner refreshes it on each save; prints the
                                             lease, then its end line on stdin EOF or Ctrl-C
  figlens watch status <select>              the workspace's leases and runner; never writes
  figlens clipboard <copied.html> <out.fig>    turn a selection copied in Figma (Cmd/Ctrl+C), saved from
                                             the clipboard's HTML, into a .fig every command reads, with
                                             no Save local copy; same ids as the source file, used
                                             components included; never replaces an existing
                                             file; prints {path,sourceFileKey,layers,
                                             nodeCount,missingImages,missingComponents,
                                             layersWithoutOutlines (a count),warnings}: a copy carries no image
                                             bytes or vector outlines, so exports warn where they draw
                                             without them. macOS: osascript -e 'the clipboard as
                                             «class HTML»' | sed 's/^«data HTML//;s/»$//' | xxd -r -p > copied.html
  figlens update                               install the latest GitHub release over this binary
  figlens --help | --version

<file.fig>: a local path, or gs://bucket/object (downloaded with your gcloud login, cached in
  ~/.cache/figlens until the object changes).
<node>: a Figma URL with ?node-id=..., 12-345 (URL form) or 12:345.
        nodes/inspect/export/bundle also accept full instance sublayer IDs from inspect/nodes.
<out>: always explicit. Shipped assets go where the project keeps its assets; reference and
       scratch files go in .figlens/exports/ (holding a .gitignore of *), not beside the .fig.
JSON goes to stdout, errors to stderr; exit status 1 on any error.

examples:
  figlens tree design.fig --depth 2
  figlens tree design.fig --top-frames | figlens export design.fig --frames - --out-dir shots
  figlens nodes design.fig 'https://www.figma.com/design/KEY/Name?node-id=12-345' > frame.json";

/// Rendering recurses through the node tree; rayon's 2 MiB default stacks overflow sooner than
/// the main thread's 8 MiB, so every pool gets the main thread's size.
const STACK: usize = 8 << 20;

fn main() -> ExitCode {
    // Fails only if a pool already exists, which nothing builds before this line.
    let _ = rayon::ThreadPoolBuilder::new()
        .stack_size(STACK)
        .build_global();
    let mut out = String::new();
    let result = run(std::env::args().skip(1).collect(), &mut out);
    // A reader that stops early (`figlens tree ... | head`) is not an error.
    match std::io::stdout().write_all(out.as_bytes()) {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            eprintln!("figlens: {e}");
            return ExitCode::FAILURE;
        }
        _ => {}
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if let Some(error) = e.downcast_ref::<figlens::tokens::ConversionError>() {
                eprintln!("{}", error.report);
            } else {
                eprintln!("figlens: {e}");
            }
            // Structured recovery for agents: only actions that are implemented and apply.
            if let Some(recovery) = e
                .downcast_ref::<figlens::FigError>()
                .and_then(figlens::FigError::recovery)
            {
                eprintln!("{}", serde_json::json!({"recovery": recovery}));
            }
            ExitCode::FAILURE
        }
    }
}

/// `USAGE`'s entries for one command, continuation lines included, and a pointer to the rest;
/// `None` for a command it does not list.
fn command_usage(command: &str) -> Option<String> {
    if command.starts_with('-') {
        return None;
    }
    let mut section = String::new();
    let mut inside = false;
    for line in USAGE.lines() {
        if let Some(rest) = line.strip_prefix("  figlens ") {
            inside = rest.split_whitespace().next() == Some(command);
        } else if !line.starts_with(' ') {
            // A blank line or a heading ends the command list.
            inside = false;
        }
        if inside {
            section.push_str(line);
            section.push('\n');
        }
    }
    (!section.is_empty())
        .then(|| format!("{section}\nfiglens --help lists every command and explains <file.fig>, <node> and <out>"))
}

fn run(args: Vec<String>, out: &mut String) -> Result<(), Box<dyn Error>> {
    let command = args.first().cloned();
    dispatch(args, out).map_err(|error| {
        // A usage error shows only the command that was mistyped.
        match command.as_deref().and_then(command_usage) {
            Some(usage) if error.to_string() == USAGE => usage.into(),
            _ => error,
        }
    })
}

fn dispatch(args: Vec<String>, out: &mut String) -> Result<(), Box<dyn Error>> {
    if matches!(args.get(1).map(String::as_str), Some("-h" | "--help"))
        && let Some(usage) = args.first().and_then(|command| command_usage(command))
    {
        writeln!(out, "{usage}")?;
        return Ok(());
    }
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            writeln!(out, "{USAGE}")?;
            return Ok(());
        }
        Some("-v" | "-V" | "--version") => {
            writeln!(out, "figlens {}", env!("CARGO_PKG_VERSION"))?;
            return Ok(());
        }
        Some("update") => return update(),
        Some("cache") => {
            let dir = figlens::preview::cache_dir()?;
            let report = match args.get(1).map(String::as_str) {
                None => {
                    serde_json::json!({"path": dir, "bytes": figlens::preview::cache_bytes(&dir)})
                }
                Some("clear") => {
                    serde_json::json!({"path": dir, "freed": figlens::preview::clear_cache(&dir)?})
                }
                Some(other) => return Err(format!(
                    "unknown cache action {other:?}; use `figlens cache` or `figlens cache clear`"
                )
                .into()),
            };
            writeln!(out, "{report}")?;
            return Ok(());
        }
        Some("mcp") => return mcp(&args[1..]),
        Some("__index-worker") => {
            let (Some(dir), Some(job)) = (args.get(1), args.get(2)) else {
                return Err(USAGE.into());
            };
            // The terminal's Ctrl-C reaches the whole process group; the owner decides.
            signals::ignore();
            let report = figlens::workspace::jobs::worker(Path::new(dir), job);
            writeln!(out, "{report}")?;
            return if report["error"].is_object() {
                Err("index worker failed".into())
            } else {
                Ok(())
            };
        }
        Some("__visual-worker") => {
            let (Some(dir), Some(job)) = (args.get(1), args.get(2)) else {
                return Err(USAGE.into());
            };
            signals::ignore();
            // A hash-only job renders here; a vision job runs in its profile's figlens-embed.
            let report =
                figlens::workspace::visual_job::run_visual_worker(Path::new(dir), job, |_| {
                    Err("this figlens renders only; a vision job runs in figlens-embed".into())
                });
            writeln!(out, "{report}")?;
            return if report["error"].is_object() {
                Err("visual worker failed".into())
            } else {
                Ok(())
            };
        }
        Some("__watch-launch") => {
            let rest: Vec<std::ffi::OsString> = args[1..].iter().map(Into::into).collect();
            writeln!(out, "{}", figlens::workspace::watch::launch(&rest)?)?;
            return Ok(());
        }
        Some("__watch-runner") => {
            let (id, storage) = match &args[1..] {
                [id] => (id, None),
                [id, flag, dir] if flag == "--storage" => (id, Some(Path::new(dir))),
                _ => return Err(USAGE.into()),
            };
            // Its holders, not a terminal's Ctrl-C, decide when it ends.
            signals::ignore();
            let storage = figlens::workspace::storage(storage)?;
            return figlens::workspace::watch::serve(&storage, id);
        }
        Some("index") => {
            let report = index(&args[1..])?;
            writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
            // A partial reset prints what remains and still fails.
            if report["failed"].as_array().is_some_and(|f| !f.is_empty()) {
                return Err("reset could not remove every database file; see failed".into());
            }
            if let Some(job) = [&report["job"]]
                .into_iter()
                .chain(report["jobs"].as_array().into_iter().flatten())
                .find(|j| j.is_object() && j["state"] != "succeeded")
            {
                return Err(format!("job {} ended {}", job["jobId"], job["state"]).into());
            }
            return Ok(());
        }
        Some("changes") => {
            writeln!(out, "{}", changes(&args[1..], false)?)?;
            return Ok(());
        }
        Some("watch") if args.get(1).is_some_and(|a| a == "run") => {
            return watch_run(&args[2..]);
        }
        Some("watch") if args.get(1).is_some_and(|a| a == "status") => {
            writeln!(out, "{}", watch_status(&args[2..])?)?;
            return Ok(());
        }
        Some("watch") if args.get(1).is_some_and(|a| a == "ack") => {
            writeln!(out, "{}", changes(&args[2..], true)?)?;
            return Ok(());
        }
        Some("watch")
            if args.get(1).is_some_and(|a| {
                ["subscribe", "evaluate", "list", "unsubscribe", "forget"].contains(&a.as_str())
            }) =>
        {
            writeln!(out, "{}", watch(&args[1], &args[2..])?)?;
            return Ok(());
        }
        Some("models") => {
            let report = models(&args[1..])?;
            writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
            return Ok(());
        }
        Some("jobs") => {
            let report = jobs(&args[1..])?;
            writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
            return Ok(());
        }
        Some("search") if args.get(1).is_some_and(|a| a.starts_with("--")) => {
            let report = workspace_search(&args[1..])?;
            writeln!(out, "{report}")?;
            return Ok(());
        }
        Some("usages") if args.get(1).is_some_and(|a| a.starts_with("--")) => {
            let report = workspace_usages(&args[1..])?;
            writeln!(out, "{report}")?;
            return Ok(());
        }
        Some("diff") => {
            out.push_str(&command::diff_cli(&args[1..])?);
            return Ok(());
        }
        Some("verify") => {
            let bundle = args.get(1).ok_or(USAGE)?;
            let fonts = match &args[2..] {
                [] => None,
                [flag, dir] if flag == "--fonts" => Some(PathBuf::from(dir)),
                _ => return Err(USAGE.into()),
            };
            let report = figlens::verify::verify_with_fonts(Path::new(bundle), fonts.as_deref())?;
            writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
            return Ok(());
        }
        Some("clipboard") => {
            let [input, output] = &args[1..] else {
                return Err(USAGE.into());
            };
            let report = figlens::clipboard::convert(Path::new(input), Path::new(output))?;
            writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
            return Ok(());
        }
        Some("workflow") => {
            let topic = args.get(1).ok_or_else(|| {
                format!(
                    "figlens workflow <topic>; choose {}",
                    workflow::TOPICS.join(", ")
                )
            })?;
            writeln!(out, "{}", workflow::read_cli(topic)?)?;
            return Ok(());
        }
        _ => {}
    }
    let (command, file) = match (args.first(), args.get(1)) {
        (Some(c), Some(f)) => (c.as_str(), f),
        _ => return Err(USAGE.into()),
    };
    let search = (command == "search")
        .then(|| command::search_request(&args[2..]))
        .transpose()?;
    let usages = (command == "usages")
        .then(|| command::usages_request(&args[2..]))
        .transpose()?;
    if let Some((request, command::Query::Image(_))) = &search {
        return Err(command::image_needs_workspace(file, &request.scope));
    }
    let path = figlens::remote::local(file).map_err(|e| contract::source(file, e))?;
    if let Some((request, query)) = search {
        let command::Query::Text(mode) = query else {
            unreachable!("refused above")
        };
        // Ctrl-C stops a model run and its child instead of killing figlens around them.
        if mode != figlens::semantic::Mode::Text {
            signals::catch();
        }
        let interrupted = &signals::INTERRUPTED;
        out.push_str(&command::search(&path, &request, mode, file, interrupted)?.text()?);
        return Ok(());
    }
    if let Some(request) = usages {
        out.push_str(&command::usages(&path, &request, file)?.text()?);
        return Ok(());
    }
    if command == "preview" {
        let (mut node, mut output, mut port, mut bind) =
            (None, None, 0u16, "127.0.0.1".parse::<std::net::IpAddr>()?);
        let mut rest = args[2..].iter();
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--out" | "-o" => output = Some(rest.next().ok_or("--out needs a path")?),
                "--port" => port = rest.next().ok_or("--port needs a number")?.parse()?,
                "--bind" => bind = rest.next().ok_or("--bind needs an IP address")?.parse()?,
                _ if !arg.starts_with('-') && node.is_none() => node = Some(normalize_id(arg)),
                _ => return Err(USAGE.into()),
            }
        }
        if let Some(output) = output {
            figlens::preview::snapshot(
                &path,
                node.as_deref().ok_or("--out requires a node")?,
                Path::new(output),
            )?;
            writeln!(
                out,
                "{}",
                serde_json::json!({"path":output,"snapshot":true})
            )?;
            return Ok(());
        }
        // Ctrl+C or SIGTERM stops the viewer's model queries and owned build before it exits.
        signals::catch();
        return figlens::preview::serve_until(
            &path,
            node.as_deref(),
            std::net::SocketAddr::new(bind, port),
            &signals::INTERRUPTED,
        );
    }
    // Read before decoding, so a bundle's `source.sha256` names the bytes this tree came from.
    let decoded = figlens::search::stamp(&path).ok();
    let mut doc = Document::open(&path).map_err(|e| contract::source(file, e))?;
    let message = doc.take_message()?;
    let tree = Tree::new(&message);
    match command {
        "bundle" if args.get(2).map(String::as_str) == Some("--frames") => {
            batch(&tree, &path, decoded, &args[2..])?;
        }
        "nodes"
            if args[2..]
                .iter()
                .any(|arg| matches!(arg.as_str(), "--frames" | "--ids")) =>
        {
            let (args, compact, subtrees) = command::nodes_options(&args[2..])?;
            let frames = node_list(&args)?;
            if compact {
                batch_lines(&frames, &mut std::io::stdout().lock(), |id| {
                    figlens::rest::resolve_compact_nodes(&tree, id, subtrees, None)
                        .map_err(|e| e.to_string())
                })?;
            } else {
                nodes_batch(&tree, &frames, &mut std::io::stdout().lock())?;
            }
        }
        "export" if args.get(2).map(String::as_str) == Some("--batch") => {
            let source = args.get(3).filter(|_| args.len() == 4).ok_or(USAGE)?;
            let list = if source == "-" {
                std::io::read_to_string(std::io::stdin())?
            } else {
                std::fs::read_to_string(source)?
            };
            let items = command::export_items(&serde_json::from_str(&list)?, 0)?;
            let paths: Vec<PathBuf> = items.iter().map(|i| i.out().into()).collect();
            let manifest = command::export_batch(&tree, &path, &items, &paths)?;
            out.push_str(&serde_json::to_string_pretty(&manifest)?);
        }
        "export" if args.get(2).map(String::as_str) == Some("--frames") => {
            export_batch(&tree, &path, &args[2..], &mut std::io::stdout().lock())?;
        }
        "tree" | "find" | "inspect" | "node" | "nodes" | "variables" | "bundle" | "export" => {
            let command = command::Command::from_cli(command, &args[2..], &tree)?.decoded(decoded);
            out.push_str(&command.execute(&tree, &path)?.text()?);
        }
        "compare-image" => {
            let (node, shot, diff, opts) = command::compare_image_args(&args[2..])?;
            let report = command::compare_image(
                &tree,
                &path,
                &node,
                Path::new(&shot),
                diff.as_deref().map(Path::new),
                opts,
            )?;
            writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
        }
        "compare" => {
            // A `--frames` line may name the capture's variables.json as a third field.
            let jobs: Vec<(String, String, Option<String>)> = match args.get(2).map(String::as_str)
            {
                Some("--frames") => std::fs::read_to_string(args.get(3).ok_or(USAGE)?)?
                    .lines()
                    .filter_map(|l| {
                        let mut f = l.split('|');
                        let (id, path) = (f.next()?, f.next()?);
                        Some((
                            normalize_id(id),
                            path.to_owned(),
                            f.next().map(str::to_owned),
                        ))
                    })
                    .collect(),
                Some(path) => vec![(
                    normalize_id(args.get(3).ok_or(USAGE)?),
                    path.to_owned(),
                    None,
                )],
                None => return Err(USAGE.into()),
            };
            let (mut vars_jobs, mut vars_clean) = (0, 0);
            let (mut clean, mut fills_clean) = (0, 0);
            let mut fields_clean = [0usize; FIELDS.len()];
            for (root, path, vars_path) in &jobs {
                if !tree.nodes.contains_key(root) {
                    return Err(format!(
                        "node {root} not found; list ids with `figlens tree <file.fig> --depth 2`"
                    )
                    .into());
                }
                let oracle: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
                let report = compare(&tree, &oracle, root)?;
                let status = if report.is_clean() { "OK  " } else { "DIFF" };
                writeln!(
                    out,
                    "{status} {root} (direct {}, in instances {})",
                    report.direct, report.in_instances
                )?;
                for (label, lines) in &report.diffs {
                    writeln!(out, "  {label}: {}", lines.len())?;
                    for line in lines.iter().take(6) {
                        writeln!(out, "    {line}")?;
                    }
                }
                if let Some(vars_path) = vars_path {
                    let theirs = serde_json::from_slice(&std::fs::read(vars_path)?)?;
                    // A variables capture has no document and holds every local variable.
                    let doc = oracle["document"]
                        .is_object()
                        .then(|| figlens::rest::resolve_nodes(&tree, root))
                        .transpose()?;
                    let ours = variables(&tree, doc.as_ref().map(|d| &d["document"]));
                    let lines = figlens::variables::diff(&ours, &theirs);
                    if !lines.is_empty() {
                        writeln!(out, "  variables: {}", lines.len())?;
                        for line in lines.iter().take(6) {
                            writeln!(out, "    {line}")?;
                        }
                    }
                    vars_jobs += 1;
                    vars_clean += lines.is_empty() as usize;
                }
                clean += report.is_clean() as usize;
                fills_clean += (report.is_clean() && report.fills_clean()) as usize;
                for (n, label) in fields_clean.iter_mut().zip(FIELDS) {
                    *n += report.clean(label) as usize;
                }
            }
            writeln!(
                out,
                "identical: {clean} of {} ({fills_clean} also with identical fills)",
                jobs.len()
            )?;
            let fields: Vec<String> = FIELDS
                .iter()
                .zip(fields_clean)
                .map(|(label, n)| format!("{label} {n}"))
                .collect();
            writeln!(out, "frames with identical fields: {}", fields.join(", "))?;
            if vars_jobs > 0 {
                writeln!(out, "identical variables.json: {vars_clean} of {vars_jobs}")?;
            }
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

/// `bundle --frames <list|-> --out-dir <dir> [--file-key K] [--jobs N]`: one decode, the frames
/// written in parallel, one JSON line per frame as each finishes. A frame's failure is its own line.
fn batch(
    tree: &Tree,
    fig: &Path,
    decoded: Option<figlens::search::Stamp>,
    args: &[String],
) -> Result<(), Box<dyn Error>> {
    let (mut list, mut dir, mut file_key, mut jobs, mut states) = (None, None, None, None, false);
    let mut hash_source = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = || rest.next().cloned().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--frames" => list = Some(value()?),
            "--out-dir" => dir = Some(value()?),
            "--file-key" => file_key = Some(value()?),
            "--jobs" => jobs = Some(value()?.parse::<usize>()?),
            "--states" => states = true,
            "--hash-source" => hash_source = true,
            _ => return Err(USAGE.into()),
        }
    }
    let (list, dir) = (list.ok_or(USAGE)?, dir.ok_or("--frames needs --out-dir")?);
    let decoded = decoded.filter(|_| hash_source);
    std::fs::create_dir_all(&dir)?;
    let frames = node_list(&["--frames".to_owned(), list])?;
    let jobs = jobs
        .filter(|&n| n > 0)
        .unwrap_or_else(rayon::current_num_threads);
    let stdout = std::sync::Mutex::new(std::io::stdout());
    let write_frame = |id: &String| {
        let path = Path::new(&dir).join(format!("{}.zip", id.replace(':', "-")));
        let one = || -> Result<Vec<String>, String> {
            let key = file_key.as_deref();
            figlens::bundle::write_with_states(tree, fig, decoded, key, None, id, &path, states)
                .map_err(|e| e.to_string())
        };
        // A panic in one frame must not take the others down.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(one))
            .unwrap_or_else(|_| Err("figlens crashed on this frame".to_owned()));
        let line = match result {
            Ok(warnings) => {
                serde_json::json!({ "nodeId": id, "path": path, "warnings": warnings })
            }
            Err(e) => serde_json::json!({ "nodeId": id, "error": e }),
        };
        let mut out = stdout.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    };
    let workers = jobs.min(frames.len());
    let frames = std::sync::Mutex::new(frames.iter());
    // OS workers bound frames without confining inner rendering to a frame-sized rayon pool.
    std::thread::scope(|scope| -> std::io::Result<()> {
        for _ in 0..workers {
            std::thread::Builder::new()
                .stack_size(STACK)
                .spawn_scoped(scope, || {
                    loop {
                        let Some(id) = frames.lock().unwrap().next() else {
                            break;
                        };
                        write_frame(id);
                    }
                })?;
        }
        Ok(())
    })?;
    Ok(())
}

/// The first whitespace-separated field of each non-blank line, as a node id.
fn frame_ids(list: &str) -> Vec<String> {
    list.lines()
        .filter_map(|l| l.split_whitespace().next().map(normalize_id))
        .collect()
}

/// `--frames <list>` (one id per line; `-` reads stdin) or `--ids <id>...`.
fn node_list(args: &[String]) -> Result<Vec<String>, Box<dyn Error>> {
    Ok(match args {
        [flag, list] if flag == "--frames" && list == "-" => {
            frame_ids(&std::io::read_to_string(std::io::stdin())?)
        }
        [flag, list] if flag == "--frames" => frame_ids(&std::fs::read_to_string(list)?),
        [flag, ids @ ..] if flag == "--ids" && !ids.is_empty() => {
            ids.iter().map(|id| normalize_id(id)).collect()
        }
        _ => return Err(USAGE.into()),
    })
}

const MAX_NODE_FRAMES: usize = 10_000;

/// `nodes --frames|--ids`: one decode, one JSON line per id, written as each finishes so the
/// whole list's JSON is never held at once. A missing, failing or panicking id is its own line.
fn nodes_batch(
    tree: &Tree,
    frames: &[String],
    out: &mut impl std::io::Write,
) -> Result<(), Box<dyn Error>> {
    batch_lines(frames, out, |id| {
        figlens::rest::resolve_nodes(tree, id).map_err(|e| e.to_string())
    })
}

/// One JSON line per id: `one(id)` with `nodeId` added, or `{nodeId,error}`. A missing, failing
/// or panicking id is its own line; a reader that stops early (`... | head`) is not an error.
fn batch_lines(
    frames: &[String],
    out: &mut impl std::io::Write,
    one: impl Fn(&str) -> Result<serde_json::Value, String>,
) -> Result<(), Box<dyn Error>> {
    if frames.len() > MAX_NODE_FRAMES {
        return Err(format!(
            "{} node ids listed; at most {MAX_NODE_FRAMES} per call",
            frames.len()
        )
        .into());
    }
    for id in frames {
        let line = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| one(id)))
            .unwrap_or_else(|_| Err("figlens crashed on this node".to_owned()))
        {
            Ok(mut doc) => {
                doc["nodeId"] = id.as_str().into();
                doc
            }
            Err(e) => serde_json::json!({ "nodeId": id, "error": e }),
        };
        match writeln!(out, "{line}") {
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => return Ok(()),
            result => result?,
        }
    }
    Ok(())
}

/// `export --frames <list|-> --out-dir <dir> [--format F] [export options]`: one decode, each id
/// exported as a single `export` would be, one JSON line per id.
fn export_batch(
    tree: &Tree,
    fig: &Path,
    args: &[String],
    out: &mut impl std::io::Write,
) -> Result<(), Box<dyn Error>> {
    let (mut list, mut dir, mut format, mut options) = (None, None, "png".to_owned(), Vec::new());
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = || rest.next().cloned().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--frames" => list = Some(value()?),
            "--out-dir" => dir = Some(value()?),
            "--format" => format = value()?.to_ascii_lowercase(),
            "--bounds" | "--scale" | "--quality" | "--max-kb" | "--max-size" | "--match-dir" => {
                options.extend([arg.clone(), value()?]);
            }
            "--hide" => return Err("--hide applies to a single export, not --frames".into()),
            _ => return Err(USAGE.into()),
        }
    }
    let frames = node_list(&["--frames".to_owned(), list.ok_or(USAGE)?])?;
    let dir = PathBuf::from(dir.ok_or("--frames needs --out-dir")?);
    // Bad options fail once, before any file is written, instead of on every line.
    let export_format = figlens::export::Format::from_ext(&format)
        .ok_or("--format takes png, jpg, webp, avif or svg")?;
    let (layout, scale, quality, max_kb, max_size, match_dir, _) =
        command::export_options(&options)?;
    // Every export compares against the folder as it was before the batch wrote anything.
    let match_index = command::match_index(match_dir, export_format)?;
    std::fs::create_dir_all(&dir)?;
    // shortcut: frames export one after another; add bundle's --jobs if large batches need it.
    batch_lines(&frames, out, |id| {
        let export = command::Command::Export {
            node: id.to_owned(),
            out: dir.join(format!("{}.{format}", id.replace(':', "-"))),
            format: export_format,
            layout,
            scale,
            quality,
            max_kb,
            max_size,
            match_index: match_index.clone(),
            hide: Vec::new(),
        };
        let output = export.execute(tree, fig).map_err(|e| e.to_string())?;
        Ok(output.json().cloned().unwrap_or_default())
    })
}

#[cfg(test)]
use contract::TOOLS;
#[cfg(test)]
use mcp::in_request_order;
#[cfg(test)]
use mcp::{call_tool, mcp_reply, serve};

/// Start stdio after canonicalizing configured roots.
fn mcp(args: &[String]) -> Result<(), Box<dyn Error>> {
    let (mut roots, mut storage_roots) = (Vec::new(), Vec::new());
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let (list, dir) = match (arg.as_str(), args.next()) {
            ("--root", Some(dir)) => (&mut roots, dir),
            ("--storage-root", Some(dir)) => (&mut storage_roots, dir),
            _ => return Err(USAGE.into()),
        };
        list.push(std::fs::canonicalize(dir).map_err(|e| format!("{arg} {dir}: {e}"))?);
    }
    // A host may signal right after closing stdin (OMP does), before the reader sees the end:
    // this server's leases end then too, instead of waiting to expire. The thread blocks until
    // a signal, so a server that never watches wakes for nothing.
    let waiter =
        signals::waiter().map_err(|e| format!("figlens mcp: cannot catch signals: {e}"))?;
    std::thread::spawn(move || {
        if waiter.wait() {
            figlens::workspace::watch::release_at_signal();
            std::process::exit(143);
        }
    });
    // The reader thread needs a Send input; stdout stays locked on this thread.
    mcp::serve_with(
        std::io::BufReader::new(std::io::stdin()),
        std::io::stdout().lock(),
        &roots,
        &storage_roots,
    )
}

/// Flags of `figlens index`, `jobs` and workspace `search`: each value flag at most once.
#[derive(Default)]
struct IndexArgs<'a> {
    positional: Vec<&'a str>,
    storage: Option<&'a str>,
    workspace: Option<&'a str>,
    file: Option<&'a str>,
    id: Option<&'a str>,
    scope: Option<&'a str>,
    source: Vec<&'a str>,
    request: Option<&'a str>,
    replace: bool,
    /// `index add <dir> --dry-run`: list what it would register, writing nothing.
    dry_run: bool,
    /// `index add <dir> --manifest HEX`: add only if the dry run's file set is unchanged.
    manifest: Option<&'a str>,
    /// `index add <dir> --cursor C`: the page after the previous add's `nextCursor`.
    cursor: Option<&'a str>,
    /// Search flags passed through to the search request parser.
    search: Vec<String>,
}

fn index_args<'a>(args: &'a [String], search: bool) -> Result<IndexArgs<'a>, Box<dyn Error>> {
    let mut parsed = IndexArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let slot = match arg.as_str() {
            "--storage" => &mut parsed.storage,
            "--workspace" => &mut parsed.workspace,
            "--file" => &mut parsed.file,
            "--id" => &mut parsed.id,
            "--scope" if !search => &mut parsed.scope,
            "--request-id" if !search => &mut parsed.request,
            "--manifest" if !search => &mut parsed.manifest,
            "--cursor" if !search => &mut parsed.cursor,
            "--replace-unavailable" if !search => {
                parsed.replace = true;
                continue;
            }
            "--dry-run" if !search => {
                parsed.dry_run = true;
                continue;
            }
            "--source" => {
                let value = rest.next().ok_or("--source needs a value")?;
                parsed.source.push(value);
                continue;
            }
            // Everything after `--` is the query, even when it starts with `-`.
            "--" if search => {
                parsed.positional.extend(rest.by_ref().map(String::as_str));
                break;
            }
            flag if search && flag.starts_with('-') => {
                parsed.search.push(arg.clone());
                if !matches!(flag, "--include-hidden" | "--include-internal" | "--out") {
                    parsed
                        .search
                        .push(rest.next().ok_or(format!("{arg} needs a value"))?.clone());
                }
                continue;
            }
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown flag {flag}; usage: figlens index|jobs|search, see `figlens --help`"
                )
                .into());
            }
            _ => {
                parsed.positional.push(arg.as_str());
                continue;
            }
        };
        // A repeated selector must not quietly pick one workspace for a destructive reset.
        if slot.is_some() {
            return Err(format!("{arg} is given twice").into());
        }
        let value = rest.next().filter(|v| !v.starts_with("--"));
        *slot = Some(
            value
                .ok_or_else(|| format!("{arg} needs a value"))?
                .as_str(),
        );
    }
    Ok(parsed)
}

/// Runs the jobs this process owns to a terminal state in the foreground, printing progress to
/// stderr, and waits for jobs another live process runs. A joined job whose owner dies is
/// taken over. Each job report in `report` is replaced by its final state.
fn finish_jobs(
    storage: &figlens::workspace::Storage,
    outcome: figlens::workspace::Outcome,
) -> Result<serde_json::Value, Box<dyn Error>> {
    use figlens::workspace::{Select, jobs};
    let mut report = outcome.report;
    let ws = report["workspaceId"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let stop = &signals::INTERRUPTED;
    signals::catch();
    let mut progress = |job: &serde_json::Value| {
        eprintln!(
            "figlens: job {} {} {}/{}",
            job["jobId"].as_str().unwrap_or(""),
            job["stage"].as_str().unwrap_or(""),
            job["progress"]["units"],
            job["progress"]["total"]
        );
    };
    let mut fin = |job: &serde_json::Value| -> Result<serde_json::Value, Box<dyn Error>> {
        let id = job["jobId"].as_str().unwrap_or_default().to_owned();
        if outcome.run.contains(&id) {
            return jobs::run(storage, &ws, &id, stop, &mut progress);
        }
        loop {
            let now = jobs::status(storage, &id)?;
            match now["state"].as_str() {
                // Another process owns it; an interrupt stops only the waiting.
                Some("queued" | "running") if stop.load(std::sync::atomic::Ordering::Relaxed) => {
                    return Ok(now);
                }
                Some("queued" | "running") => {
                    std::thread::sleep(std::time::Duration::from_millis(200))
                }
                Some("interrupted") => {
                    // Retried as the same kind: a rebuild of an unchanged scope must not
                    // become a refresh that reuses it, nor an embed a text refresh.
                    let select = Select::Id(ws.clone());
                    let (source, scope) = (now["sourceId"].as_str(), now["scope"].as_str());
                    let again = match now["kind"].as_str() {
                        Some("rebuild") => {
                            jobs::rebuild_scope(storage, &select, source, scope, None)?
                        }
                        Some("embed") => jobs::embed(storage, &select, source, scope, None)?,
                        Some("visual") => jobs::visual(storage, &select, source, scope, None)?,
                        _ => figlens::workspace::refresh(storage, &select, source, scope, None)?,
                    };
                    // Another process may have taken it over first; then keep waiting.
                    if let Some(id) = again.run.first() {
                        return jobs::run(storage, &ws, id, stop, &mut progress);
                    }
                }
                _ => return Ok(now),
            }
        }
    };
    // This process runs no model job after it exits, so a text build that could have one says
    // which call embeds or renders it instead of leaving it queued for a dead owner.
    let mut fin = |job: &serde_json::Value| -> Result<serde_json::Value, Box<dyn Error>> {
        let mut done = fin(job)?;
        let steps = [
            jobs::embed_step(storage, &done),
            jobs::visual_step(storage, &done),
        ];
        if let Some(next) = done["nextSteps"].as_array_mut() {
            next.extend(steps.into_iter().flatten());
        }
        Ok(done)
    };
    if report["job"].is_object() {
        report["job"] = fin(&report["job"])?;
    }
    if let Some(list) = report["jobs"].as_array().cloned() {
        let mut done = Vec::new();
        for job in &list {
            done.push(fin(job)?);
        }
        report["jobs"] = serde_json::json!(done);
    }
    if !outcome.run.is_empty() || report["job"].is_object() || report["jobs"].is_array() {
        let now = figlens::workspace::status(storage, &Select::Id(ws))?;
        report["sources"] = now["sources"].clone();
    }
    Ok(report)
}

/// Ctrl-C and SIGTERM while the CLI runs its own index jobs cancel them: each job's owner kills
/// its worker, waits for it, and records `cancelled`, so the previous index keeps serving.
mod signals {
    use std::sync::atomic::{AtomicBool, Ordering};

    pub static INTERRUPTED: AtomicBool = AtomicBool::new(false);

    #[cfg(unix)]
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
        #[cfg_attr(target_os = "linux", link_name = "__errno_location")]
        #[cfg_attr(not(target_os = "linux"), link_name = "__error")]
        fn errno() -> *mut i32;
    }

    /// The nonblocking write end of `Waiter`'s socket, once it exists.
    #[cfg(unix)]
    static WAKE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

    #[cfg(unix)]
    extern "C" fn interrupted(_: i32) {
        // Only the first signal writes, so repeats never fill the socket. Repeats are swallowed, not
        // fatal: a wrapper forwards the host's group SIGTERM, which must not cut the release short.
        if INTERRUPTED.swap(true, Ordering::SeqCst) {
            return;
        }
        let fd = WAKE.load(Ordering::SeqCst);
        if fd >= 0 {
            // SAFETY: write(2) is async-signal-safe; nonblocking, so a full buffer drops the byte
            // instead of blocking the handler, and one byte already wakes the waiter. The
            // interrupted thread's errno is put back, as a handler must.
            unsafe {
                let saved = *errno();
                write(fd, [1u8].as_ptr(), 1);
                *errno() = saved;
            }
        }
    }

    /// Wakes once `catch`'s handler runs, and for nothing else.
    #[cfg(unix)]
    pub struct Waiter(std::os::unix::net::UnixStream);

    /// Sets up the wakeup, then catches the signals: a failure leaves the default handlers.
    #[cfg(unix)]
    pub fn waiter() -> std::io::Result<Waiter> {
        use std::os::fd::IntoRawFd;
        // std opens both ends close-on-exec, so no child inherits them.
        let (read, write) = std::os::unix::net::UnixStream::pair()?;
        write.set_nonblocking(true)?;
        WAKE.store(write.into_raw_fd(), Ordering::SeqCst);
        catch();
        Ok(Waiter(read))
    }

    #[cfg(unix)]
    impl Waiter {
        /// Blocks until a signal is caught; false if the wakeup failed, leaving the defaults
        /// again so a signal still ends the process.
        pub fn wait(mut self) -> bool {
            use std::io::Read;
            let mut byte = [0u8];
            while !INTERRUPTED.load(Ordering::SeqCst) {
                match self.0.read(&mut byte) {
                    Ok(1) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    _ => {
                        // SAFETY: SIG_DFL (0) for SIGINT and SIGTERM.
                        unsafe {
                            signal(2, 0);
                            signal(15, 0);
                        }
                        return INTERRUPTED.load(Ordering::SeqCst);
                    }
                }
            }
            true
        }
    }

    #[cfg(unix)]
    pub fn catch() {
        let handler = interrupted as extern "C" fn(i32) as usize;
        // SAFETY: the handler only swaps an atomic and writes one byte, both async-signal-safe.
        unsafe {
            signal(2, handler);
            signal(15, handler);
        }
    }

    #[cfg(unix)]
    pub fn ignore() {
        // SAFETY: SIG_IGN (1) for SIGINT.
        unsafe {
            signal(2, 1);
        }
    }

    /// `wait`'s wakeup: Windows runs the handler on an ordinary thread, so it may lock.
    #[cfg(windows)]
    static WAKE: (std::sync::Mutex<()>, std::sync::Condvar) =
        (std::sync::Mutex::new(()), std::sync::Condvar::new());

    #[cfg(windows)]
    unsafe extern "system" fn interrupted(_: u32) -> i32 {
        let _held = WAKE.0.lock();
        INTERRUPTED.store(true, Ordering::SeqCst);
        WAKE.1.notify_all();
        1
    }

    /// Wakes once `catch`'s handler runs, and for nothing else.
    #[cfg(windows)]
    pub struct Waiter;

    #[cfg(windows)]
    pub fn waiter() -> std::io::Result<Waiter> {
        catch();
        Ok(Waiter)
    }

    #[cfg(windows)]
    impl Waiter {
        /// Blocks until a signal is caught; false only if the lock was poisoned.
        pub fn wait(self) -> bool {
            let Ok(mut held) = WAKE.0.lock() else {
                return false;
            };
            while !INTERRUPTED.load(Ordering::SeqCst) {
                match WAKE.1.wait(held) {
                    Ok(next) => held = next,
                    Err(_) => return false,
                }
            }
            true
        }
    }

    #[cfg(windows)]
    pub fn catch() {
        // SAFETY: the handler runs on its own thread, so it may lock and notify.
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(interrupted), 1);
        }
    }

    #[cfg(windows)]
    pub fn ignore() {
        // SAFETY: a null handler with TRUE makes this process ignore Ctrl-C.
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 1);
        }
    }
}

/// `figlens index <action> ...`: CLI syntax and foreground execution of shared index actions.
fn index(args: &[String]) -> Result<serde_json::Value, Box<dyn Error>> {
    use command::index::{Reply, Request};
    use figlens::workspace::{self, Select};
    let usage = || -> Box<dyn Error> {
        "usage: figlens index prepare|status|add|refresh|remove|relocate|clear|rebuild|track|untrack|embed|visual|reset ...; see `figlens --help`"
            .into()
    };
    let action = args.first().ok_or_else(usage)?.as_str();
    let a = index_args(&args[1..], false)?;
    let storage = workspace::storage(a.storage.map(Path::new))?;
    let select = Select::from_choice(a.workspace.map(Path::new), a.file.map(Path::new), a.id)?;
    let scope = a.scope.map(normalize_id);
    let mut request = Request {
        action: if action == "clear" {
            "clear_cache"
        } else {
            action
        },
        select,
        scope: scope.as_deref(),
        request: a.request,
        replace: a.replace,
        cursor: a.cursor,
        manifest: a.manifest,
        dry_run: a.dry_run,
        ..Default::default()
    };
    match (action, a.positional.as_slice()) {
        _ if action != "add" && (a.dry_run || a.manifest.is_some() || a.cursor.is_some()) => {
            return Err(usage());
        }
        ("prepare", [path]) if a.file.is_none() && a.id.is_none() && a.source.is_empty() => {
            request.file = Some(Path::new(path));
            request.project = a.workspace.map(Path::new);
        }
        ("refresh" | "rebuild" | "track" | "untrack" | "embed" | "visual", []) => {
            request.source = a.source.first().copied();
        }
        ("add", [path]) if a.source.is_empty() => request.file = Some(Path::new(path)),
        ("remove", [source]) if a.source.is_empty() => request.source = Some(source),
        ("relocate", [source, path]) if a.source.is_empty() => {
            request.source = Some(source);
            request.file = Some(Path::new(path));
        }
        ("status" | "clear" | "clear_cache" | "reset", []) if a.source.is_empty() => {}
        _ => return Err(usage()),
    }
    if !request.applicable() || (matches!(action, "track" | "untrack") && scope.is_none()) {
        return Err(usage());
    }
    if action != "prepare" && request.select.is_none() {
        return Err("choose the workspace with --workspace, --file or --id".into());
    }
    if a.source.len() > 1 {
        return Err("--source is given twice".into());
    }
    let result = match request.run(&storage) {
        Ok(Reply::Report(value)) => Ok(value),
        Ok(Reply::Jobs(outcome)) => finish_jobs(&storage, outcome),
        Err(e) => Err(e),
    };
    result.map_err(|e| workspace::rooted(&storage, e))
}

/// `figlens jobs status|cancel <jobId> [--storage DIR]`.
fn jobs(args: &[String]) -> Result<serde_json::Value, Box<dyn Error>> {
    use figlens::workspace::{self, jobs};
    let a = index_args(args.get(1..).unwrap_or_default(), false)?;
    let storage = workspace::storage(a.storage.map(Path::new))?;
    match (args.first().map(String::as_str), a.positional.as_slice()) {
        (Some("status"), [id]) => jobs::status(&storage, id),
        (Some("cancel"), [id]) => jobs::cancel(&storage, id),
        _ => Err("usage: figlens jobs status|cancel <jobId> [--storage DIR]".into()),
    }
}

const MODELS: &str = "usage: figlens models list|status | configure <profile> --model DIR [--runtime FILE] | remove <profile>";

/// `figlens models ...`: the local model profiles semantic and visual search use. Nothing is
/// downloaded; `configure` checks files the user already has.
fn models(args: &[String]) -> Result<serde_json::Value, Box<dyn Error>> {
    use figlens::semantic::profile;
    let storage = figlens::workspace::storage(None)?;
    let (mut model, mut runtime, mut rest) = (None, None, Vec::new());
    let mut it = args.iter().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--model" => model = Some(PathBuf::from(it.next().ok_or(MODELS)?)),
            "--runtime" => runtime = Some(PathBuf::from(it.next().ok_or(MODELS)?)),
            _ if arg.starts_with("--") => return Err(MODELS.into()),
            _ => rest.push(arg.as_str()),
        }
    }
    match (args.first().map(String::as_str), rest.as_slice(), model) {
        (Some("list"), [], None) => profile::list(&storage),
        (Some("status"), [], None) => profile::status(&storage),
        (Some("configure"), [name], Some(model)) => {
            signals::catch();
            profile::configure(
                &storage,
                profile::known(name)?,
                &model,
                runtime.as_deref(),
                &signals::INTERRUPTED,
            )
        }
        (Some("remove"), [name], None) => profile::remove(&storage, name),
        _ => Err(MODELS.into()),
    }
}

/// `figlens search --workspace DIR|--file <file.fig>|--id ws_... <query> [--source ID]...`:
/// search the workspace's published indexes; nothing is built.
fn workspace_search(args: &[String]) -> Result<serde_json::Value, Box<dyn Error>> {
    use figlens::workspace::{self, Select};
    let a = index_args(args, true)?;
    let image = a
        .search
        .iter()
        .any(|f| f == "--image" || f == "--image-ref");
    let query = match a.positional.as_slice() {
        [query] => query.to_string(),
        [] if image => String::new(),
        _ => return Err("usage: figlens search --workspace DIR|--file <file.fig>|--id ws_... <query> [--source ID]... [--node ID] | [query] --image PNG|--image-ref JSON [--crop X,Y,W,H]".into()),
    };
    let storage = workspace::storage(a.storage.map(Path::new))?;
    let select = Select::from_choice(a.workspace.map(Path::new), a.file.map(Path::new), a.id)?
        .ok_or("choose the workspace with --workspace, --file or --id")?;
    let options: Vec<String> = a.search.iter().map(|s| s.to_string()).collect();
    let (request, query) = command::search_options(Some(query), &options)?;
    let only: Vec<String> = a.source.iter().map(|s| s.to_string()).collect();
    match query {
        command::Query::Text(figlens::semantic::Mode::Text) => {
            workspace::query::search(&storage, &select, &only, &request)
        }
        command::Query::Text(mode) => {
            signals::catch();
            workspace::query::search_mode(
                &storage,
                &select,
                &only,
                &request,
                mode,
                &signals::INTERRUPTED,
            )
        }
        command::Query::Image(image) => {
            signals::catch();
            workspace::query::search_image(
                &storage,
                &select,
                &only,
                &request,
                &image,
                &signals::INTERRUPTED,
            )
        }
    }
}

const WATCH_RUN: &str = "usage: figlens watch run --workspace DIR|--file <file.fig>|--id ws_... --scope NODE [--source ID] [--request-id R] [--storage DIR]";

/// The workspace a `watch` command names, with its storage.
fn watch_select(
    a: &IndexArgs,
) -> Result<(figlens::workspace::Storage, figlens::workspace::Select), Box<dyn Error>> {
    use figlens::workspace::{self, Select};
    let storage = workspace::storage(a.storage.map(Path::new))?;
    let select = Select::from_choice(a.workspace.map(Path::new), a.file.map(Path::new), a.id)?
        .ok_or("choose the workspace with --workspace, --file or --id")?;
    Ok((storage, select))
}

/// `watch run`: holds one lease in the foreground, renewing it, until Ctrl-C, SIGTERM or the
/// end of stdin; then releases it. Prints the lease, then the release, as they happen.
fn watch_run(args: &[String]) -> Result<(), Box<dyn Error>> {
    use figlens::workspace::watch;
    use std::sync::atomic::Ordering;
    let a = index_args(args, false)?;
    let (Some(scope), true) = (a.scope, a.positional.is_empty() && a.source.len() <= 1) else {
        return Err(WATCH_RUN.into());
    };
    let (storage, select) = watch_select(&a)?;
    signals::catch();
    let lease = watch::start(
        &storage,
        &select,
        a.source.first().copied(),
        scope,
        a.request,
    )?;
    let mut out = std::io::stdout();
    if let Err(e) = writeln!(out, "{lease}").and_then(|()| out.flush()) {
        // Nobody reads this lease: end it now rather than leave it to expire.
        watch::release();
        return Err(e.into());
    }
    if lease["state"] != "active" {
        return Err(format!("lease {} ended ({})", lease["leaseId"], lease["reason"]).into());
    }
    // The end of stdin is the portable way a host stops it; a signal is the other.
    std::thread::spawn(|| {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
        signals::INTERRUPTED.store(true, Ordering::Relaxed);
    });
    let tick = std::time::Duration::from_millis(100);
    let mut renew_at = std::time::Instant::now() + watch::RENEW;
    while !signals::INTERRUPTED.load(Ordering::Relaxed) {
        if std::time::Instant::now() >= renew_at {
            renew_at = std::time::Instant::now() + watch::RENEW;
            // A failed renewal keeps the lease until it expires; the next one retries.
            let _ = watch::renew();
            if !watch::holding() {
                // Ended elsewhere (stopped, source removed, untracked or the workspace reset).
                let reason = watch::status(&storage, &select)
                    .ok()
                    .and_then(|s| {
                        s["leases"]
                            .as_array()?
                            .iter()
                            .find(|l| l["leaseId"] == lease["leaseId"])
                            .cloned()
                    })
                    // An expired row has no reason; its state says why.
                    .map_or("workspace_gone".into(), |l| {
                        l.get("reason").unwrap_or(&l["state"]).clone()
                    });
                writeln!(
                    out,
                    "{}",
                    serde_json::json!({"leaseId": lease["leaseId"], "state": "ended", "reason": reason})
                )?;
                return Ok(());
            }
        }
        std::thread::sleep(tick);
    }
    watch::release();
    writeln!(
        out,
        "{}",
        serde_json::json!({"leaseId": lease["leaseId"], "state": "ended", "reason": "released"})
    )?;
    Ok(())
}

/// `watch status`: the workspace's leases, runner and monitors, read-only.
fn watch_status(args: &[String]) -> Result<serde_json::Value, Box<dyn Error>> {
    let a = index_args(args, false)?;
    if !a.positional.is_empty() || a.scope.is_some() || !a.source.is_empty() {
        return Err("usage: figlens watch status --workspace DIR|--file <file.fig>|--id ws_... [--storage DIR]".into());
    }
    let (storage, select) = watch_select(&a)?;
    figlens::workspace::watch::status(&storage, &select)
}

/// `figlens changes <select> [filters] [--consumer C] [--cursor X] [--revision R] [--limit N]`,
/// or with `ack`, `figlens watch ack <select> --consumer C --cursor X [filters]`.
fn changes(args: &[String], ack: bool) -> Result<serde_json::Value, Box<dyn Error>> {
    use figlens::workspace::{self, Select, changes};
    let usage = match ack {
        true => {
            "usage: figlens watch ack --workspace DIR|--file <file.fig>|--id ws_... --consumer NAME|--subscription ID --cursor C [--source ID] [--scope NODE] [--storage DIR]"
        }
        false => {
            "usage: figlens changes --workspace DIR|--file <file.fig>|--id ws_... [--source ID] [--scope NODE] [--consumer NAME] [--subscription ID] [--cursor C] [--revision R] [--limit N] [--storage DIR]"
        }
    };
    let a = index_args(args, true)?;
    if !a.positional.is_empty() || a.source.len() > 1 {
        return Err(usage.into());
    }
    let mut flags = std::collections::HashMap::new();
    for pair in a.search.chunks(2) {
        let allowed: &[&str] = match ack {
            true => &["--scope", "--consumer", "--subscription", "--cursor"],
            false => &[
                "--scope",
                "--consumer",
                "--subscription",
                "--cursor",
                "--revision",
                "--limit",
            ],
        };
        if !allowed.contains(&pair[0].as_str()) || pair.len() < 2 {
            return Err(format!("unknown flag {}; {usage}", pair[0]).into());
        }
        if flags.insert(pair[0].as_str(), pair[1].as_str()).is_some() {
            return Err(format!("{} is given twice", pair[0]).into());
        }
    }
    let limit = flags
        .get("--limit")
        .map(|l| {
            l.parse::<usize>()
                .map_err(|_| "--limit needs a whole number")
        })
        .transpose()?;
    let storage = workspace::storage(a.storage.map(Path::new))?;
    let select = Select::from_choice(a.workspace.map(Path::new), a.file.map(Path::new), a.id)?
        .ok_or("choose the workspace with --workspace, --file or --id")?;
    let request = changes::Request {
        source: a.source.first().copied(),
        scope: flags.get("--scope").copied(),
        consumer: flags.get("--consumer").copied(),
        cursor: flags.get("--cursor").copied(),
        revision: flags.get("--revision").copied(),
        limit,
        subscription: flags.get("--subscription").copied(),
    };
    match ack {
        true => changes::ack(&storage, &select, &request),
        false => changes::changes(&storage, &select, &request),
    }
}

/// `figlens watch subscribe|evaluate|list|unsubscribe|forget <select> [flags]`: durable subscriptions
/// and checkpoint views.
fn watch(action: &str, args: &[String]) -> Result<serde_json::Value, Box<dyn Error>> {
    use figlens::workspace::{self, Select, changes::subscriptions as subs};
    let (usage, allowed): (&str, &[&str]) = match action {
        "subscribe" => (
            "usage: figlens watch subscribe <select> --consumer NAME --source ID|--sources visible|ID[,ID] [--scope NODE] [--node ID|--query TEXT [--type T[,T]]|--library KIND [--namespace NS] --key KEY [--definition-source ID --definition-snapshot SNAP --definition-node ID]] [--owner NAME] [--storage DIR]",
            &[
                "--consumer",
                "--scope",
                "--node",
                "--owner",
                "--query",
                "--type",
                "--sources",
                "--library",
                "--namespace",
                "--key",
                "--definition-source",
                "--definition-snapshot",
                "--definition-node",
            ],
        ),
        "evaluate" => (
            "usage: figlens watch evaluate <select> --subscription ID [--storage DIR]",
            &["--subscription"],
        ),
        "list" => (
            "usage: figlens watch list <select> [--consumer NAME] [--after ID] [--subscription ID] [--storage DIR]",
            &["--consumer", "--after", "--subscription"],
        ),
        "unsubscribe" => (
            "usage: figlens watch unsubscribe <select> --subscription ID|--consumer NAME --owner NAME [--storage DIR]",
            &["--subscription", "--consumer", "--owner"],
        ),
        _ => (
            "usage: figlens watch forget <select> --consumer NAME [--source ID] [--scope NODE] [--storage DIR]",
            &["--consumer", "--scope"],
        ),
    };
    let a = index_args(args, true)?;
    if !a.positional.is_empty() || a.source.len() > 1 {
        return Err(usage.into());
    }
    // evaluate runs on the subscription's own source; a --source it would ignore is refused.
    if action == "evaluate" && !a.source.is_empty() {
        return Err(format!("unknown flag --source; {usage}").into());
    }
    let mut flags = std::collections::HashMap::new();
    for pair in a.search.chunks(2) {
        if !allowed.contains(&pair[0].as_str()) || pair.len() < 2 {
            return Err(format!("unknown flag {}; {usage}", pair[0]).into());
        }
        if flags.insert(pair[0].as_str(), pair[1].as_str()).is_some() {
            return Err(format!("{} is given twice", pair[0]).into());
        }
    }
    let storage = workspace::storage(a.storage.map(Path::new))?;
    let select = Select::from_choice(a.workspace.map(Path::new), a.file.map(Path::new), a.id)?
        .ok_or("choose the workspace with --workspace, --file or --id")?;
    let (source, flag) = (a.source.first().copied(), |k: &str| flags.get(k).copied());
    // A library question's own flags never silently drop from a saved search.
    if flag("--library").is_none()
        && let Some(f) = flags
            .keys()
            .find(|f| ["--namespace", "--key"].contains(f) || f.starts_with("--definition-"))
    {
        return Err(format!("{f} applies only with --library; {usage}").into());
    }
    match action {
        "subscribe" => subs::subscribe(
            &storage,
            &select,
            &subs::Subscribe {
                consumer: flag("--consumer"),
                source,
                scope: flag("--scope"),
                node: flag("--node"),
                owner: flag("--owner"),
                query: flag("--query"),
                types: flag("--type")
                    .map(command::search_types)
                    .into_iter()
                    .flatten()
                    .collect(),
                sources: flag("--sources"),
                library: match flag("--library") {
                    None => None,
                    Some(_) => Some(command::library_request(
                        &a.search
                            .chunks(2)
                            .filter(|p| {
                                p[0].starts_with("--library")
                                    || ["--namespace", "--key"].contains(&p[0].as_str())
                                    || p[0].starts_with("--definition-")
                            })
                            .flatten()
                            .cloned()
                            .collect::<Vec<_>>(),
                    )?),
                },
            },
        ),
        "evaluate" => workspace::changes::evaluate(&storage, &select, flag("--subscription")),
        "list" => subs::list(
            &storage,
            &select,
            flag("--consumer"),
            flag("--after"),
            flag("--subscription"),
        ),
        "unsubscribe" => subs::unsubscribe(
            &storage,
            &select,
            flag("--subscription"),
            flag("--consumer"),
            flag("--owner"),
        ),
        _ => subs::forget(
            &storage,
            &select,
            flag("--consumer"),
            source,
            flag("--scope"),
        ),
    }
}

/// `figlens usages --workspace DIR|--file <file.fig>|--id ws_... <node>|<resource> [--source ID] [options]`:
/// usages over one source's published index of `--scope`, or with `--library KIND --namespace NS
/// --key KEY` every visible source's; nothing is built.
fn workspace_usages(args: &[String]) -> Result<serde_json::Value, Box<dyn Error>> {
    use figlens::workspace::{self, Select};
    let a = index_args(args, true)?;
    let library = a.search.iter().any(|f| f == "--library");
    let target = match a.positional.as_slice() {
        [] if library => None,
        [target] if !library => Some(target),
        [_] => return Err("--library replaces the node or resource; pass one, not both".into()),
        _ => return Err("usage: figlens usages --workspace DIR|--file <file.fig>|--id ws_... <node>|<resource>|--library KIND --namespace NS --key KEY [--source ID] [--scope ID]".into()),
    };
    let storage = workspace::storage(a.storage.map(Path::new))?;
    let select = Select::from_choice(a.workspace.map(Path::new), a.file.map(Path::new), a.id)?
        .ok_or("choose the workspace with --workspace, --file or --id")?;
    let one_source = || {
        if a.source.len() > 1 {
            return Err("usages reads one source; pass --source once");
        }
        Ok(a.source.first().copied())
    };
    let Some(target) = target else {
        let request = command::library_request(&a.search)?;
        return workspace::library::library(&storage, &select, one_source()?, &request);
    };
    let mut usages_args = vec![target.to_string()];
    usages_args.extend(a.search.iter().cloned());
    let request = command::usages_request(&usages_args)?;
    let source = one_source()?;
    workspace::usages::usages(&storage, &select, source, &request)
}

/// Reruns the release installer (scripts/install.sh, or install.ps1 on Windows) into this
/// binary's directory.
fn update() -> Result<(), Box<dyn Error>> {
    let exe = std::env::current_exe()?;
    // Follow a symlink to the real binary; Windows paths stay plain (no `\\?\` prefix).
    let exe = if cfg!(windows) {
        exe
    } else {
        exe.canonicalize()?
    };
    let dir = exe.parent().ok_or("cannot locate the figlens binary")?;
    let script = if cfg!(windows) {
        "install.ps1"
    } else {
        "install.sh"
    };
    // Download completely before running anything: a cut-off script must not run half.
    let fetched = Command::new("curl")
        .args(["-fsSL"])
        .arg(format!(
            "https://raw.githubusercontent.com/runkids/figlens/main/scripts/{script}"
        ))
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| {
            format!(
                "update needs `curl`: {e}; without it, download the archive from \
                 https://github.com/runkids/figlens/releases/latest"
            )
        })?;
    if !fetched.status.success() {
        return Err("could not download the install script".into());
    }
    let ran = if cfg!(windows) {
        // PowerShell runs a multi-line script reliably only from a file. %TEMP% is per user.
        let path = std::env::temp_dir().join(format!("figlens-{}-{script}", std::process::id()));
        std::fs::write(&path, &fetched.stdout)?;
        let ran = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&path)
            .env("FIGLENS_BIN_DIR", dir)
            .status();
        let _ = std::fs::remove_file(&path);
        ran?
    } else {
        // Piped from memory: no file in a shared /tmp that another user could swap.
        let mut sh = Command::new("sh")
            .env("FIGLENS_BIN_DIR", dir)
            .stdin(Stdio::piped())
            .spawn()?;
        sh.stdin
            .take()
            .ok_or("sh has no stdin")?
            .write_all(&fetched.stdout)?;
        sh.wait()?
    };
    if !ran.success() {
        return Err("install script failed".into());
    }
    Ok(())
}
