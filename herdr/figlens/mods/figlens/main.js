// figlens beside your agents: open a .fig, find a screen or any layer, hand it to the agent on
// the left as Build, Plan or YOLO, look at it full size, or start a new agent on it.
// Roles (props.role): the panel (default); "toggle", the tab-bar button that opens the panel as
// the "figlens" child of the layout's "panel-body" stack; "confirm", the dialog that shows
// exactly what is pasted into which agent; "newtab", the dialog that starts an agent in a new
// tab; "quicklook", a floating window with one screen at full size.
import { View, div, image, svg } from "gpui-kit";
import { v_flex, h_flex, InputState, Input } from "gpui-base";
import { exec, hc, run, appHome, home, spaces, panes, agents, focused, shared, setShared, theme, settings, watchFolder, window, layout, pickFolder, openOverlay, closeOverlay, knownAgents, nextCommand, props as blockProps } from "herdr";
import { writeFile, mkdir, readdir } from "fs/promises";
import { Buffer } from "buffer";
import { createHash, randomUUID } from "crypto";
import { VERSION, asset, releaseBase, cacheDir, INSTALL } from "./engine.js";
import { groupFrames, thumbFile, fitBox, clean, nodeRef, intentText, appOf, latestActivity, ago, ACTIVITY } from "./frames.js";

const FILES = "figlens.files.v1"; // { [space id]: { path, on } }: each space keeps its own design
const NOTICE = "figlens.notice.v1"; // { kind, text, at }: dialogs report to the panel
const QL = "figlens.quicklook.v1"; // what the Quick Look window shows and where it is in the list
const CLEARED = "figlens.cleared.v1"; // when the image cache was last cleared: every panel reloads
const VIEWERS = "figlens.viewers.v1"; // { "<machine>:<path>": { terminal, url } }: browser viewers
const PANELS = "workspace.panels"; // Herdr's: the side panels its tabs mod restores per space
const CACHE = "mods/figlens/.cache"; // hidden: writing images does not reload the mod
const CHUNK = 12, REMOTE_CHUNK = 40, THUMB_W = 72, THUMB_H = 48, POLL = 10000, KEEP_REVISIONS = 12;
const FOLD_OVER = 40, PREVIEW_MAX = 960, PREVIEW_H = 360, LOOK_MAX = 1600, THUMB_SCALE = 3;
// Scale a small node up to the target size: --max-size only shrinks, so a 78px icon drawn at 1x
// is blurry when shown larger. Drawn at a scale, its image fills keep their own pixels.
const scaleFor = (size, max) => Math.min(16, Math.max(1, Math.ceil(max / Math.max(1, ...(size ?? [max])))));
const GONE = "This file is no longer there";
// GNU stat first: there, -f means file-system status and would print garbage. Then macOS's.
const STAT = 'stat -c "%Y %s" "$1" 2>/dev/null || stat -f "%m %z" "$1"';
const FIND = `find "$1" -maxdepth 4 \\( -name node_modules -o -name .git -o -name target \\) -prune -o -type f -name '*.fig' -print 2>/dev/null | head -20`;
// sh -c REMOTE_EXPORT <name> <bin> <file> <max> <scale> <id>...: images rendered on the design's
// machine, sent back as one base64 tar.gz; the design itself never moves.
const REMOTE_EXPORT = `set -e
bin=$1; file=$2; max=$3; scale=$4; shift 4
d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
mkdir "$d/out"; printf '%s\\n' "$@" > "$d/list"
"$bin" export "$file" --frames "$d/list" --out-dir "$d/out" --format png --bounds layout --scale "$scale" --max-size "$max" >/dev/null 2>&1 || true
tar -czf - -C "$d/out" . | base64`;
// sh -c SNAP_MAKE <name> <bin> <file> <node>: a one-screen offline viewer, gzipped into a temp
// folder there; prints "<folder> <bytes>". Snapshots embed original images and can be tens of
// MB: a remote command's output is cut at 8 MB and a mod's own file writes are capped, so
// FETCH_PART runs here and appends 4 MB at a time through hc straight into the file.
const SNAP_MAKE = `set -e
d=$(mktemp -d)
"$1" preview "$2" "$3" --out "$d/s.html" >/dev/null || { rm -rf "$d"; exit 1; }
gzip -c "$d/s.html" > "$d/s.gz"; rm "$d/s.html"
printf '%s %s' "$d" "$(wc -c < "$d/s.gz" | tr -d ' ')"`;
// sh -c FETCH_PART <name> <machine> <remote file> <skip MB> <count MB> <local file>: prints its size.
const FETCH_PART = 'h=$(command -v hc || echo "$HOME/.local/bin/hc"); "$h" exec --on "$1" dd if="$2" bs=1048576 skip="$3" count="$4" 2>/dev/null >> "$5"; wc -c < "$5"';
const PART_MB = 4;
const ACTIVITY_POLL = 5000;
const PRUNE = 'cd "$1" && ls -1t | tail -n +$2 | while IFS= read -r d; do rm -rf "./$d"; done';
const KIND = { Darwin: "macos", Linux: "linux" };
const STATUS_ICON = { done: "check", info: "copy", working: "ring", error: "alert" };
// Top-level layers are screens unless they are components; many components are icon-sized.
const KINDS = [["screens", "Screens"], ["components", "Components"], ["all", "All"]];
const isComponent = (f) => f.type === "COMPONENT" || f.type === "COMPONENT_SET";
const inKind = (f, kind) => kind === "all" || (kind === "components") === isComponent(f);
const INTENT = {
  build: { verb: "Build", title: (n, a) => `Send “${n}” to ${a}?`, go: (a) => `Send to ${a}` },
  plan: { verb: "Plan", title: (n, a) => `Plan “${n}” with ${a}?`, go: (a) => `Plan with ${a}` },
  yolo: { verb: "YOLO", title: (n, a) => `Let ${a} build “${n}” on its own?`, go: (a) => `Start YOLO in ${a}` },
};
const YOLO_NOTE = (a) => `${a} plans, edits this project, checks its work, and after a verified run deletes only the plan and exports it made. A failed run keeps them.`;

const sh = (script, args = [], on = null) => exec("sh", ["-c", script, "figlens", ...args], { on });
const lastLine = (s) => String(s ?? "").trim().split("\n").pop();
const hash = (s) => String(createHash("sha256").update(s).digest("hex")).slice(0, 16);
const base = (p) => p.split("/").pop();
const mono = () => settings().font_family || "Menlo";
const machineOf = (address) => (address?.includes("/") ? address.split("/")[0] : null);
const activeSpace = () => spaces().find((s) => s.active) ?? null;
// Type follows the person's Herdr UI size (settings ui_font_size), so it scales with theirs.
function S() {
  const b = Number(settings().ui_font_size) || 13;
  return { dialog: b + 2, title: b + 1, body: b + 0.5, button: b - 0.5, secondary: b - 0.5, label: b - 1 };
}

// ---- Look: the design system's pieces (Herdr theme tokens only) ----

const icon = (name, size, color) => svg(`mods/figlens/icons/${name}.svg`).size(size).flex_none().text_color(color);
const stop = (_e, cx) => cx.stop_propagation();
const dot = (color, size) => div().w(size).h(size).flex_none().rounded_full().bg(color);

// The agent's icon from Herdr's program table, drawn as the agents list draws it.
function programIcon(p, size, color) {
  if (!p?.icon) return null;
  return p.tint ? svg(p.icon).size(size).flex_none().text_color(color) : image(p.icon).w(size).h(size).flex_none();
}

function primary(id, label, t, onClick, iconName) {
  return h_flex().id(id).h(28).px(10).gap(6).flex_none().items_center().rounded(6).bg(t.solid)
    .text_size(S().button).font_semibold().text_color(t.solid_foreground).cursor_pointer().hover((s) => s.opacity(0.88))
    .on_mouse_down("left", stop).on_click(onClick)
    .when(Boolean(iconName), (el) => el.child(icon(iconName, 13, t.solid_foreground))).child(label);
}

function secondary(id, label, t, onClick, opts = {}) {
  const color = opts.color ?? t.foreground;
  return h_flex().id(id).h(28).px(opts.px ?? 10).gap(5).flex_none().items_center().rounded(6).border_1().border_color(t.border)
    .text_size(opts.size ?? S().button).text_color(color).cursor_pointer().hover((s) => s.bg(t.hover))
    .on_mouse_down("left", stop).on_click(onClick)
    .when(Boolean(opts.icon), (el) => el.child(icon(opts.icon, 12, opts.iconColor ?? color))).child(label);
}

const pill = (text, t) => h_flex().h(20).px(7).flex_none().items_center().rounded(10).bg(t.selected).text_size(S().label).text_color(t.muted).child(text);
const statusColor = (t, kind) => ({ done: t.ansi2, info: t.muted, working: t.working, error: t.ansi1 }[kind] ?? t.muted);
const agentColor = (t, status) => ({ working: t.working, blocked: t.blocked, done: t.done }[status] ?? t.muted);

function statusRow(t, kind, text) {
  const color = statusColor(t, kind), glyph = icon(STATUS_ICON[kind] ?? "alert", 13, color);
  return h_flex().h(34).flex_none().px(12).gap(7).items_center().border_t(1).border_color(t.border).bg(t.background)
    .text_size(S().secondary).text_color(color).child(kind === "working" ? glyph.spin(900) : glyph).child(div().min_w(0).truncate().child(text));
}

const working = (t, text) => h_flex().gap(7).items_center().text_size(S().secondary).text_color(t.muted)
  .child(icon("ring", 13, t.muted).spin(900)).child(text);

// ---- Data from Herdr and the design's machine ----

async function figlens(bin, args, on) {
  const r = await exec(bin, args, { on });
  if (r.code !== 0) throw new Error(lastLine(r.stderr) || `figlens exited with ${r.code}`);
  return r.stdout;
}

// On a machine (null: this computer): a figlens on PATH, else the npm launcher's cache
// for VERSION, else { missing }.
async function findEngine(on) {
  try {
    if ((await exec("figlens", ["--version"], { on })).code === 0) return { bin: "figlens", onPath: true, on };
  } catch { /* not on PATH */ }
  const root = (await sh('printf %s "${XDG_CACHE_HOME:-$HOME/.cache}"', [], on)).stdout;
  const dir = cacheDir(root);
  if ((await sh('test -x "$1/figlens"', [dir], on)).code === 0) return { bin: `${dir}/figlens`, on };
  return { missing: true, dir, on };
}

// Unpacks a base64 tar.gz of images (REMOTE_EXPORT's output) into dir.
async function unpackImages(b64, dir) {
  const tgz = `${dir}/.chunk-${randomUUID()}.tgz`; // calls may overlap in one folder
  await writeFile(tgz, Buffer.from(b64.replace(/\s+/g, ""), "base64"));
  await exec("tar", ["-xzf", tgz, "-C", dir]);
  await exec("rm", ["-f", tgz]);
}

// One node as a PNG in dir (named by its id), drawn where the design lives at up to max pixels
// on its longest side. True when it exists.
async function renderNode(bin, file, id, dir, max, size) {
  const scale = String(scaleFor(size, max));
  await mkdir(dir, { recursive: true });
  const name = thumbFile(id);
  if ((await readdir(dir)).includes(name)) return true;
  if (file.on) {
    const r = await sh(REMOTE_EXPORT, [bin, file.path, String(max), scale, id], file.on);
    if (r.code === 0 && r.stdout.trim()) await unpackImages(r.stdout, dir);
  } else {
    // The node's own box, not its shadows or overflow: agent boxes are drawn in that space.
    await exec(bin, ["export", file.path, id, `${dir}/${name}`, "--bounds", "layout", "--scale", scale, "--max-size", String(max)]);
  }
  return (await readdir(dir)).includes(name);
}

// The browser viewer for a design on this computer: one figlens preview per file, run as a
// hidden Herdr terminal and reused while it lives. Resolves with its URL; started(terminal)
// hears about a viewer this call started.
// A viewer the panel starts stops after 10 minutes with no page open (an open page asks every
// 2 s), when this figlens has --idle-exit; an older one runs until the design or panel closes.
async function idleArgs(bin) {
  const r = await exec(bin, ["preview", "--help"]).catch(() => null);
  return String(r?.stdout ?? "").includes("--idle-exit") ? ["--idle-exit", "600"] : [];
}

async function viewerUrl(bin, file, cx, started) {
  const key = `${file.on ?? ""}:${file.path}`, known = (shared(VIEWERS) ?? {})[key];
  if (known) {
    const g = await hc(["get", known.terminal, "--json"]).catch(() => null);
    try { if (g?.code === 0 && JSON.parse(g.stdout).finished_ms == null) return known.url; } catch { /* start another */ }
  }
  const r = await hc(["run", "--bg", "--json", "--no-parent", "--", bin, "preview", file.path, ...(await idleArgs(bin))]);
  if (r.code !== 0) throw new Error(lastLine(r.stderr) || "Could not start the viewer");
  const terminal = JSON.parse(r.stdout).address;
  started?.(terminal);
  for (let i = 0; i < 80; i += 1) {
    const out = await hc(["read", terminal, "--lines", "20"]).catch(() => null);
    const m = out?.stdout?.match(/Preview: (http:\/\/127\.0\.0\.1:\d+\/[0-9a-f]+\/)/);
    if (m) {
      setShared(VIEWERS, { ...(shared(VIEWERS) ?? {}), [key]: { terminal, url: m[1] } });
      return m[1];
    }
    await cx.sleep(250);
  }
  await hc(["kill", terminal]).catch(() => null);
  throw new Error("The viewer did not start");
}

// A design on another machine: its live viewer listens only there, so open a one-screen
// offline snapshot here instead.
async function openSnapshot(bin, file, id, dir, progress) {
  progress?.(`Making a snapshot on ${file.on}…`);
  const made = await sh(SNAP_MAKE, [bin, file.path, id], file.on);
  if (made.code !== 0 || !made.stdout.trim()) throw new Error(lastLine(made.stderr) || "Could not make a snapshot");
  const [tmp, bytes] = made.stdout.trim().split(" "), size = Number(bytes);
  await mkdir(dir, { recursive: true });
  const html = `${dir}/${thumbFile(id).replace(/\.png$/, ".html")}`, gz = `${html}.gz`;
  try {
    await exec("rm", ["-f", gz]);
    await copyParts(FETCH_PART, [file.on, `${tmp}/s.gz`, gz], size, PART_MB,
      (got) => progress?.(`Copying the snapshot · ${mbOf(got)} of ${mbOf(size)} MB`));
    const unpacked = await sh('gunzip -c "$1" > "$2" && rm -f "$1"', [gz, html]);
    if (unpacked.code !== 0) throw new Error("Could not unpack the snapshot");
    await exec(window()?.platform === "macos" ? "open" : "xdg-open", [html]);
  } finally {
    sh('rm -rf "$1"', [tmp], file.on).catch(() => null);
  }
}

// ---- A design on the agent's machine ----
// An agent reads files on its own machine. When the design is elsewhere, a copy goes there
// first, once per saved version, under ~/.cache/figlens/herdr-designs/<key>/ on that machine.
// hc put and hc file write stop at 16 and 8 MB, so the copy moves in parts through hc exec.
const COPIES = ".cache/figlens/herdr-designs";
const PUSH_MB = 6;
// sh -c PUSH_PART <name> <machine> <local file> <skip MB> <count MB> <remote file>: prints its size.
const PUSH_PART = `h=$(command -v hc || echo "$HOME/.local/bin/hc")
dd if="$2" bs=1048576 skip="$3" count="$4" 2>/dev/null | "$h" exec -i --on "$1" sh -c 'mkdir -p "\${1%/*}"; cat >> "$1"; wc -c < "$1"' x "$5"`;
const mbOf = (n) => (n / 1048576).toFixed(1);
const cliFor = (cli, design, agentOn) => ((design?.on ?? null) === (agentOn ?? null) ? cli : null);
async function sizeOn(path, on) {
  const r = await sh('wc -c < "$1" 2>/dev/null', [path], on).catch(() => null);
  return Number(r?.stdout?.trim()) || 0;
}

async function copyParts(script, args, total, step, say) {
  let got = 0;
  for (let part = 0; got < total && part < 1 << 16; part += step) {
    say(got);
    const r = await sh(script, [...args.slice(0, 2), String(part), String(step), args[2]]);
    const now = Number(r.stdout.trim());
    if (r.code !== 0 || !(now > got)) break;
    got = now;
  }
  if (got !== total) throw new Error(`The copy stopped at ${mbOf(got)} of ${mbOf(total)} MB`);
}

// The path the agent on agentOn (null: this computer) reads the design at, copying it there if needed.
// A design copy's folder name under COPIES, the same on every machine: one per saved version.
const copyKey = (design) => hash(`${design.on ?? ""}:${design.path}:${design.stamp}`);

async function designOn(design, agentOn, progress) {
  const from = design.on ?? null, to = agentOn ?? null;
  if (from === to) return design.path;
  const key = copyKey(design), name = base(design.path), size = design.size;
  let local = design.path;
  if (from) {
    const dir = `${home()}/${COPIES}/${key}`;
    local = `${dir}/${name}`;
    if ((await sizeOn(local, null)) !== size) {
      await sh('mkdir -p "$1" && rm -f "$2"', [dir, local]);
      await copyParts(FETCH_PART, [from, design.path, local], size, PART_MB,
        (got) => progress(`Copying ${name} from ${from} · ${mbOf(got)} of ${mbOf(size)} MB`));
    }
  }
  if (!to) return local;
  const remoteHome = (await sh('printf %s "$HOME"', [], to)).stdout.trim();
  const dest = `${remoteHome}/${COPIES}/${key}/${name}`;
  if ((await sizeOn(dest, to)) !== size) {
    await sh('rm -f "$1"', [dest], to);
    await copyParts(PUSH_PART, [to, local, dest], size, PUSH_MB,
      (got) => progress(`Copying ${name} to ${to} · ${mbOf(got)} of ${mbOf(size)} MB`));
  }
  return dest;
}

// What a dialog says before copying, and the message as it will read after.
function copyPlan(design, agentOn) {
  const from = design?.on ?? null, to = agentOn ?? null;
  if (!design || from === to) return null;
  const name = base(design.path);
  return {
    note: `Copies this design (${mbOf(design.size)} MB) to ${to ?? "this computer"} before sending, once per saved version.`,
    shown: (text) => text.split(design.path).join(`~/${COPIES}/…/${name} on ${to ?? "this computer"}`),
  };
}

// Open in the browser: the live viewer here, a snapshot for a design elsewhere.
async function openInBrowser(bin, file, id, key, cx, progress, started) {
  if (file.on) { await openSnapshot(bin, file, id, `${appHome()}/${CACHE}/${key}/snap`, progress); return "Opened a snapshot in the browser"; }
  progress?.("Starting the viewer…");
  const url = await viewerUrl(bin, file, cx, started);
  cx.open_url(`${url}#node=${encodeURIComponent(id)}`);
  return "Opened in the browser";
}

// Where the person is working: the focused pane's folder and machine, as the Files mod reads it.
function here() {
  const space = activeSpace(), f = focused();
  const pane = f?.pane ? panes().find((p) => p.pane === f.pane) : null;
  const on = f?.pane === null ? (f.terminal?.includes("/") ? f.terminal.split("/")[0] : null) : space?.machine ?? null;
  const cwd = pane?.cwd || f?.cwd || space?.folder || null;
  return cwd ? { cwd, on } : null;
}

// The system's file picker: macOS's, else zenity's or kdialog's; else a folder to look in.
async function choose() {
  const platform = window()?.platform;
  if (platform === "macos") {
    const r = await exec("osascript", ["-e", 'POSIX path of (choose file with prompt "Open a Figma design" of type {"fig"})']);
    return r.code === 0 ? { path: r.stdout.trim() } : null; // nonzero: cancelled
  }
  if (platform === "linux") {
    for (const [cmd, args] of [["zenity", ["--file-selection", "--title=Open a Figma design", "--file-filter=Figma designs | *.fig"]],
      ["kdialog", ["--getopenfilename", ".", "*.fig"]]]) {
      try {
        const r = await exec(cmd, args);
        return r.code === 0 && r.stdout.trim() ? { path: r.stdout.trim() } : null;
      } catch { /* not installed: try the next */ }
    }
  }
  const folder = await pickFolder({ prompt: "Look here" });
  return folder ? { folder } : null;
}

const agentWhere = (a) => [a.space_name, a.tab_name, machineOf(a.address)].filter(Boolean).join(" · ");

function find(node, id) {
  if (node?.id === id) return node;
  for (const child of node?.children ?? []) {
    const found = find(child, id);
    if (found) return found;
  }
  return null;
}

// This space's design.
function sharedFile() {
  const v = (shared(FILES) ?? {})[activeSpace()?.id];
  return v?.path ? { path: v.path, on: v.on ?? null } : null;
}
const same = (a, b) => (a?.path ?? null) === (b?.path ?? null) && (a?.on ?? null) === (b?.on ?? null);

// ---- The tab-bar button: same look and behavior as the tabs mod's Files and Changes ----

function toggle() {
  const t = theme(), root = layout(), panel = find(root, "panel"), body = find(root, "panel-body");
  const on = panel && !panel.hidden && body?.active === "figlens";
  return h_flex().size_full().pr(10).items_center().bg(t.chrome).border_b(1).border_color(t.border)
    .child(h_flex().id("panel-figlens").h(26).px(8).gap(5).flex_none().items_center().rounded(6)
      .text_size(12).bg(on ? t.selected : t.chrome).text_color(on ? t.foreground : t.muted)
      .hover((s) => s.bg(t.hover)).on_mouse_down("left", stop)
      .on_click(() => {
        if (on) run("layout.hide", { id: "panel" });
        else { run("layout.select", { stack: "panel-body", child: "figlens" }); run("layout.show", { id: "panel" }); }
      })
      .child(image("mods/figlens/mark.svg").w(16).h(16).flex_none()).child("figlens"));
}

// The receiving agent's card, as the confirm dialog and Quick Look show it.
function agentCard(t, agent, fallbackName) {
  const status = agent?.status ?? "ended", color = agentColor(t, status);
  return h_flex().gap(10).px(12).py(10).items_center().rounded(8).border_1().border_color(t.border).bg(t.background)
    .child(div().w(24).h(24).flex_none().flex().items_center().justify_center().rounded(6).bg(agent?.program?.tile ?? t.selected)
      .child(programIcon(agent?.program, 16, agent?.program?.color ?? t.foreground) ?? icon("arrow", 14, t.foreground)))
    .child(v_flex().flex_1().min_w(0).gap(2)
      .child(div().truncate().text_size(S().body).font_semibold().child(agent?.program?.name ?? fallbackName))
      .child(div().truncate().text_size(S().secondary).text_color(t.muted).child(agent ? agentWhere(agent) : "")))
    .child(h_flex().h(20).px(8).gap(5).flex_none().items_center().rounded(10).bg(t.selected).text_size(S().label).text_color(color)
      .child(div().w(6).h(6).rounded(3).bg(color)).child(status));
}

// Opens the confirm dialog for one intent; what it shows is exactly what is pasted.
function confirmIntent(intent, agent, text, name, design) {
  const copying = copyPlan(design, machineOf(agent.address));
  openOverlay({
    mod: "figlens", kind: "dialog", width: 480, height: (intent === "yolo" ? 470 : 410) + (copying ? 90 : 0),
    props: { role: "confirm", intent, address: agent.address, name: agent.program.name, frame: name, text, design },
  });
}

// Empties this panel's image cache and figlens's render cache on each measured machine.
async function clearAllCaches(renders, panelBytes, copies = []) {
  const root = `${appHome()}/${CACHE}`;
  const r = await exec("rm", ["-rf", root]).catch((e) => ({ code: 1, stderr: String(e.message ?? e) }));
  await mkdir(root, { recursive: true }).catch(() => null);
  let freed = r.code === 0 ? panelBytes : 0, failed = r.code === 0 ? null : lastLine(r.stderr) || "Could not clear the panel's images";
  for (const x of renders) {
    const out = await exec(x.bin, ["cache", "clear"], { on: x.on }).catch(() => null);
    try { freed += JSON.parse(out.stdout).freed || 0; } catch { failed ??= `Could not clear figlens renders${x.on ? ` on ${x.on}` : ""}`; }
  }
  for (const x of copies) {
    const out = await sh('rm -rf "$1"', [x.dir], x.on).catch(() => null);
    if (out?.code === 0) freed += x.bytes; else failed ??= `Could not remove design copies${x.on ? ` on ${x.on}` : ""}`;
  }
  return { freed, failed };
}

// Notices that only report (done, info) leave after this long; errors stay until the next action.
const NOTICE_MS = 4000;
const fading = (n) => n && (n.kind === "done" || n.kind === "info");

// Everything the handoff menu needs, worked out by whoever opens it.
function handoffProps({ agent, file, frame, page, screen, cli, design }) {
  const space = activeSpace(), agentCli = cliFor(cli, design, machineOf(agent?.address));
  const text = (intent) => intentText({ intent, app: agent?.app ?? null, file: file.path, frame, page, screen, cli: agentCli });
  return {
    address: agent?.address ?? null, agentName: agent?.program?.name ?? null, frame: frame.name,
    plan: agent ? text("plan") : null, yolo: agent ? text("yolo") : null, copy: text("build"), design,
    newtab: { role: "newtab", on: space?.machine ?? null, space: space?.id, spaceName: space?.name, file: file.path, design,
      frame: { id: frame.id, name: frame.name }, page: page ? { name: page.name } : null, screen: screen ? { name: screen.name } : null,
      cli: cliFor(cli, design, space?.machine ?? null) },
  };
}

// Where the caret was pressed, in window coordinates: the menu opens under it. Herdr's default
// "pointer" placement lands elsewhere when the button sits in a floating window.
let caretAt = null;
const MENU_W = 236;

// The split button: its main part builds with the agent (or opens a new tab without one); its
// caret opens the handoff menu at the pointer, styled as Herdr's own menus.
// In a column it hugs its content (self_start); in a row such as Quick Look's title bar it centers.
function handoffButton(t, key, agent, build, menuProps, newTab, inRow = false) {
  const main = agent ? { label: `Build with ${agent.program.name}…`, icon: "arrow", go: build } : { label: "New tab…", icon: "plus", go: newTab };
  const items = agent ? 4 : 2;
  return h_flex().when(!inRow, (el) => el.self_start()).when(inRow, (el) => el.self_center()).flex_none().h(28).rounded(6).bg(t.solid).text_color(t.solid_foreground).text_size(S().button).font_semibold()
    .child(h_flex().id(`main-${key}`).h_full().pl(10).pr(9).gap(6).items_center().rounded_l(6).cursor_pointer().hover((s) => s.opacity(0.88))
      .on_mouse_down("left", stop).on_click(() => main.go()).child(icon(main.icon, 13, t.solid_foreground)).child(main.label))
    .child(div().w(1).h(16).bg(t.solid_foreground).opacity(0.2))
    .child(h_flex().id(`more-${key}`).h_full().px(7).items_center().rounded_r(6).cursor_pointer().hover((s) => s.opacity(0.88))
      .on_mouse_down("left", (e, cx) => { caretAt = e.position ?? null; cx.stop_propagation(); })
      .on_click(() => openOverlay({
        mod: "figlens", kind: "popup", width: MENU_W, height: items * 31 + (agent ? 9 : 0) + 10,
        at: caretAt ? { x: Math.max(8, caretAt.x - MENU_W + 16), y: caretAt.y + 18 } : "pointer",
        props: { role: "menu", ...menuProps() },
      }))
      .child(icon("caret", 13, t.solid_foreground)));
}

export default class Figlens extends View {
  init(_props, cx) {
    this.role = blockProps()?.role ?? "panel";
    if (this.role === "toggle") {
      // Herdr's tabs mod remembers each space's side panel, but restores only the panels named
      // in shared "workspace.panels" (it draws buttons for files and changes alone): listing
      // figlens there keeps the panel when you switch spaces and come back.
      const panels = shared(PANELS) ?? ["files", "changes"];
      if (!panels.includes("figlens")) setShared(PANELS, [...panels, "figlens"]);
      return;
    }
    if (this.role !== "panel") {
      this.handle = cx.focus_handle(); this.handle.focus();
      if (this.role === "confirm") { this.sending = false; this.failed = null; this.progress = null; }
      if (this.role === "quicklook") { this.note = null; this.looks = new Map(); this.width = 920; this.height = 640; this.lastAgent = blockProps()?.agent ?? null; }
      if (this.role === "newtab") this.initNewTab(cx);
      if (this.role === "menu") this.at = 0;
      if (this.role === "clear") this.clearing = false;
      return;
    }
    this.handle = cx.focus_handle();
    this.engine = null; this.installing = false; this.installError = null; this.error = null; this.notice = null;
    this.file = undefined; this.groups = []; this.thumbs = new Set(); this.key = null; this.gone = false;
    this.selected = null; this.loading = false; this.token = 0; this.lastAgent = null; this.visible = []; this.listed = [];
    this.cleared = shared(CLEARED) ?? null; this.cacheBytes = null; this.measuring = false;
    this.found = []; this.foundKey = null; this.picked = null; this.queue = []; this.tried = new Set(); this.drawing = false;
    this.kind = "screens"; this.query = ""; this.fold = new Map(); this.previews = new Map(); this.width = 368;
    this.raw = ""; this.hit = null; this.layers = null; this.lookToken = 0; this.looking = false;
    this.activity = new Map();
    this.filter = InputState.new({ placeholder: "Filter by name, text, node ID or Figma link" });
    this.filter.on("change", (_e, cx) => {
      this.raw = this.filter.value().trim(); this.query = this.raw.toLowerCase();
      this.lookup(cx); cx.notify();
    });
    // Enter in the filter selects the first screen and hands the keyboard to the list.
    this.filter.on("submit", (_e, cx) => {
      const first = this.visible[0];
      if (first) { this.select(first, cx); this.handle.focus(); }
      cx.notify();
    });
    // A shown done or info notice leaves on its own: redraw once it is due, not on a fast timer.
    this.showing = null;
    cx.timer.every(1000, (cx) => { if (this.showing && Date.now() - this.showing > NOTICE_MS) { this.showing = null; cx.notify(); } });
    // pane.go onto this panel (mod.json focusable) gives it the keyboard.
    cx.spawn(async (cx) => {
      for (;;) { if ((await nextCommand()) === "focus") this.handle.focus(); cx.notify(); }
    });
  }

  // Cleanup on the panel's way out: the browser viewers this panel started, not other panels'.
  async close() {
    if (this.role !== "panel") return;
    for (const terminal of this.started ?? []) run("terminal.kill", { terminal });
  }

  // ---- Confirm dialog ----

  sendNow(p, agent, cx) {
    if (this.sending) return;
    this.sending = true; this.failed = null; cx.notify();
    cx.spawn(async (cx) => {
      let text = p.text;
      if (copyPlan(p.design, machineOf(p.address))) {
        try {
          const path = await designOn(p.design, machineOf(p.address), (note) => { this.progress = note; cx.notify(); });
          text = p.text.split(p.design.path).join(path);
        } catch (e) { this.sending = false; this.progress = null; this.failed = String(e.message ?? e); cx.notify(); return; }
        this.progress = null;
      }
      const r = await hc(["send", p.address, text]).catch((e) => ({ code: 1, stderr: String(e.message ?? e) }));
      if (r.code === 0) {
        const verb = p.intent === "plan" ? "Asked" : p.intent === "yolo" ? "Started YOLO in" : "Sent to";
        setShared(NOTICE, { kind: "done", text: `${verb} ${agent.program.name}${agent.space_name ? ` in ${agent.space_name}` : ""}`, at: Date.now() });
        closeOverlay(p.overlay);
        run("agent.focus", { terminal: p.address });
        return;
      }
      this.sending = false; this.failed = lastLine(r.stderr) || "Could not send"; cx.notify();
    });
  }

  renderConfirm() {
    const t = theme(), p = blockProps() ?? {}, intent = INTENT[p.intent] ?? INTENT.build;
    // Read again on every render: the agent may have ended or started asking a question.
    const agent = agents().find((a) => a.address === p.address) ?? null;
    const name = agent?.program?.name ?? p.name ?? "the agent";
    const problem = !agent ? `${name} is no longer running. Nothing was sent.`
      : agent.status === "blocked" ? `${name} is waiting on a question. A paste could answer it, so sending is off until it moves on.` : null;
    const close = () => closeOverlay(p.overlay);
    const go = (cx) => { if (!problem) this.sendNow(p, agent, cx); };
    const native = String(p.text ?? "").startsWith("/figlens ");
    const note = problem ?? this.failed, plan = copyPlan(p.design, machineOf(p.address));
    const button = (id, label, bg, fg, border, onClick, enabled = true) =>
      h_flex().id(id).h(32).px(14).items_center().rounded(8).border_1().border_color(border).bg(bg).text_size(S().button).text_color(fg)
        .when(id === "send", (el) => el.font_semibold())
        .when(enabled, (el) => el.cursor_pointer().hover((s) => s.opacity(0.88)).on_click(onClick)).child(label);
    return v_flex().id("confirm").track_focus(this.handle)
      .on_key_down((e, cx) => {
        // YOLO authorizes edits: only the button confirms it.
        if (e.key === "enter" && p.intent !== "yolo") go(cx);
        else if (e.key === "escape") close();
        else return;
        cx.stop_propagation();
      })
      .size_full().min_h(0).p(18).gap(14).bg(t.popover).text_size(S().body).text_color(t.foreground)
      .child(h_flex().flex_none().items_center().gap(8)
        .child(div().flex_1().min_w(0).truncate().text_size(S().dialog).font_semibold().child(intent.title(clean(p.frame, 60), name)))
        .child(div().text_size(S().label).text_color(t.muted).child("Esc")))
      .when(p.intent === "yolo", (el) => el.child(h_flex().flex_none().gap(10).px(12).py(10).items_start().rounded(8).bg(t.selected)
        .child(icon("bolt", 15, t.working)).child(div().flex_1().min_w(0).text_size(S().secondary).child(YOLO_NOTE(name)))))
      .child(div().flex_none().child(agentCard(t, agent, name)))
      .child(v_flex().flex_1().min_h(64).gap(6)
        .child(div().text_size(S().label).font_semibold().text_color(t.muted).child("Message"))
        .child(div().id("message").flex_1().min_h(0).overflow_y_scrollbar().px(12).py(10).rounded(8).border_1().border_color(t.border)
          .bg(t.background).font_family(mono()).text_size(S().secondary).child(plan ? plan.shown(p.text ?? "") : p.text ?? "")))
      .when(Boolean(plan), (el) => el.child(h_flex().flex_none().gap(8).items_start().text_size(S().secondary).text_color(this.progress ? t.working : t.muted)
        .child(this.progress ? icon("ring", 13, t.working).spin(900) : icon("upload", 13, t.muted))
        .child(div().flex_1().min_w(0).child(this.progress ?? plan.note))))
      .child(note
        ? h_flex().flex_none().gap(7).items_center().text_size(S().secondary).text_color(t.ansi1).child(icon("alert", 13, t.ansi1)).child(div().min_w(0).child(note))
        : div().flex_none().text_size(S().secondary).text_color(t.muted)
          .child(`${native ? `Runs ${name}'s figlens command (needs the figlens extension). ` : ""}Pastes into ${name} and presses Enter.`))
      .child(h_flex().flex_none().justify_end().gap(8)
        .child(button("cancel", "Cancel", t.raised, t.foreground, t.input, close))
        .child(problem || this.sending
          ? button("send", this.progress ? "Copying…" : this.sending ? "Sending…" : intent.go(name), t.raised, t.muted, t.border, null, false)
          : button("send", intent.go(name), t.solid, t.solid_foreground, t.solid, (_e, cx) => go(cx))));
  }

  // ---- Clear-cache dialog: says what goes before anything is deleted ----

  renderClear() {
    const t = theme(), p = blockProps() ?? {}, close = () => closeOverlay(p.overlay);
    const mb = (n) => `${mbOf(n)} MB`, renders = p.renders ?? [];
    const where = ["this computer", ...renders.filter((x) => x.on).map((x) => x.on)].join(" and ");
    const go = (cx) => {
      if (this.clearing) return;
      this.clearing = true; cx.notify();
      cx.spawn(async () => {
        const { freed, failed } = await clearAllCaches(renders, p.panelBytes ?? 0, p.copies ?? []);
        setShared(CLEARED, Date.now()); // panels with a design open draw it again
        setShared(NOTICE, failed ? { kind: "error", text: failed, at: Date.now() } : { kind: "done", text: `Cleared ${mb(freed)}`, at: Date.now() });
        close();
      });
    };
    const button = (id, label, bg, fg, border, onClick, enabled = true) =>
      h_flex().id(id).h(32).px(14).items_center().rounded(8).border_1().border_color(border).bg(bg).text_size(S().button).text_color(fg)
        .when(id === "go", (el) => el.font_semibold())
        .when(enabled, (el) => el.cursor_pointer().hover((s) => s.opacity(0.88)).on_click(onClick)).child(label);
    return v_flex().id("clear").track_focus(this.handle)
      .on_key_down((e, cx) => {
        if (e.key === "enter") go(cx);
        else if (e.key === "escape") close();
        else return;
        cx.stop_propagation();
      })
      .size_full().min_h(0).p(18).gap(12).bg(t.popover).text_color(t.foreground)
      .child(h_flex().items_center().gap(8)
        .child(div().flex_1().min_w(0).text_size(S().dialog).font_semibold().child("Clear cached images?"))
        .child(div().text_size(S().label).text_color(t.muted).child("Esc")))
      .child(v_flex().gap(6).text_size(S().secondary).text_color(t.muted)
        .child(div().child(`This panel's thumbnails, previews, Quick Look images and snapshots · ${mb(p.panelBytes ?? 0)}`))
        .child(div().child(`figlens's render cache on ${where} · ${mb(renders.reduce((n, x) => n + x.bytes, 0))}`))
        .when(Boolean(p.copies?.length), (el) => el.child(div().child(`Copies of designs made for agents on other machines · ${mb(p.copies.reduce((n, x) => n + x.bytes, 0))}; an agent still working from one loses it`)))
        .child(div().child("Your own designs are not touched. Images are drawn and copies made again when needed.")))
      .child(div().flex_1())
      .child(h_flex().justify_end().gap(8)
        .child(button("cancel", "Cancel", t.raised, t.foreground, t.input, close))
        .child(this.clearing
          ? button("go", "Clearing…", t.raised, t.muted, t.border, null, false)
          : button("go", `Clear ${mb(p.total ?? 0)}`, t.solid, t.solid_foreground, t.solid, (_e, cx) => go(cx))));
  }

  // ---- Handoff menu: the other ways to hand a screen or layer over ----

  renderMenu() {
    const t = theme(), p = blockProps() ?? {}, close = () => closeOverlay(p.overlay);
    const agent = p.address ? agents().find((a) => a.address === p.address) ?? null : null;
    const ask = (intent) => () => { close(); if (agent) confirmIntent(intent, agent, p[intent], p.frame, p.design); };
    const list = [
      ...(agent ? [
        { glyph: "list", color: t.muted, label: `Plan with ${p.agentName}…`, go: ask("plan") },
        { glyph: "bolt", color: t.working, label: `YOLO with ${p.agentName}…`, go: ask("yolo"), rule: true },
      ] : []),
      { glyph: "plus", color: t.muted, label: "Start in a new tab…", go: () => { close(); openOverlay({ mod: "figlens", kind: "dialog", width: 380, height: 540, props: p.newtab }); } },
      { glyph: "copy", color: t.muted, label: "Copy prompt", go: (cx) => { cx.write_to_clipboard(p.copy); setShared(NOTICE, { kind: "info", text: "Prompt copied", at: Date.now() }); close(); } },
    ];
    return v_flex().id("menu").track_focus(this.handle).w_full().min_h(0).p(5).bg(t.popover).text_size(S().body)
      .on_key_down((e, cx) => {
        if (e.key === "down") this.at = (this.at + 1) % list.length;
        else if (e.key === "up") this.at = (this.at + list.length - 1) % list.length;
        else if (e.key === "enter") list[this.at].go(cx);
        else if (e.key === "escape") close();
        else return;
        cx.notify();
      })
      .children(list.map((it, i) => v_flex()
        .child(h_flex().id(`mi-${i}`).h(31).px(8).gap(9).rounded(7).items_center().text_color(t.foreground).cursor_pointer()
          .when(i === this.at, (el) => el.bg(t.selected)).hover((s) => s.bg(t.hover))
          .on_click((_e, cx) => it.go(cx))
          .child(icon(it.glyph, 14, it.color)).child(div().flex_1().min_w(0).truncate().child(it.label)))
        .when(Boolean(it.rule), (el) => el.child(div().mx(7).my(4).h(1).bg(t.border)))));
  }

  // ---- New tab dialog: a fresh agent, in this space, on one screen or layer ----

  initNewTab(cx) {
    const p = blockProps() ?? {};
    this.choices = null; this.pick = null; this.intent = "plan"; this.progress = null; this.failed = null;
    cx.spawn(async (cx) => {
      const list = await knownAgents({ on: p.on ?? null }).catch(() => []);
      this.choices = list; this.pick = list.find((a) => a.installed)?.key ?? null;
      cx.notify();
    });
  }

  renderNewTab() {
    const t = theme(), p = blockProps() ?? {}, list = this.choices;
    const agent = list?.find((a) => a.key === this.pick) ?? null;
    const close = () => closeOverlay(p.overlay);
    const open = (cx) => {
      if (!agent || this.progress) return;
      this.progress = "Starting…"; this.failed = null; cx.notify();
      const text = intentText({ intent: this.intent, app: appOf(agent.key), file: p.file, frame: p.frame, page: p.page, screen: p.screen, cli: p.cli });
      cx.spawn(async (cx) => {
        let task = text;
        if (copyPlan(p.design, p.on)) {
          try { task = text.split(p.design.path).join(await designOn(p.design, p.on, (note) => { this.progress = note; cx.notify(); })); }
          catch (e) { this.progress = null; this.failed = String(e.message ?? e); cx.notify(); return; }
        }
        this.progress = null;
        run("tab.new", { name: `${clean(p.frame.name, 40)} · ${this.intent}`, space: p.space, argv: agent.run, task });
        setShared(NOTICE, { kind: "done", text: `Opened ${agent.name} in a new tab`, at: Date.now() });
        close();
      });
    };
    const plan = copyPlan(p.design, p.on);
    const message = agent ? intentText({ intent: this.intent, app: appOf(agent.key), file: p.file, frame: p.frame, page: p.page, screen: p.screen, cli: p.cli }) : "";
    return v_flex().id("newtab").track_focus(this.handle)
      .on_key_down((e, cx) => { if (e.key === "escape") { close(); cx.stop_propagation(); } })
      .size_full().min_h(0).p(14).gap(10).bg(t.popover).text_color(t.foreground)
      .child(div().truncate().text_size(S().title).font_semibold().child(`Start an agent on “${clean(p.frame?.name, 50)}”`))
      .child(v_flex().id("choices").flex_1().min_h(0).overflow_y_scrollbar().gap(2)
        .when(!list, (el) => el.child(working(t, "Looking for installed agents…")))
        .children((list ?? []).map((a) => {
          const on = a.key === this.pick;
          return h_flex().id(`agent-${a.key}`).h(34).px(8).gap(10).items_center().rounded(7)
            .bg(on ? t.selected : t.popover).text_size(S().body).text_color(a.installed ? t.foreground : t.muted)
            .when(a.installed, (el) => el.cursor_pointer().hover((s) => s.bg(on ? t.selected : t.hover))
              .on_click((_e, cx) => { this.pick = a.key; cx.notify(); }))
            .child(div().w(18).h(18).flex_none().flex().items_center().justify_center().rounded(5).bg(a.tile ?? t.selected)
              .child(programIcon(a, 13, a.color ?? t.foreground) ?? div()))
            .child(div().flex_1().min_w(0).truncate().child(a.name))
            .when(!a.installed, (el) => el.child(div().text_size(S().label).child("not installed")));
        })))
      .child(div().text_size(S().label).font_semibold().text_color(t.muted).child("First message"))
      .child(h_flex().p(2).gap(2).rounded(7).bg(t.chrome).children(["plan", "build", "yolo"].map((id) => {
        const on = this.intent === id;
        return h_flex().id(`intent-${id}`).flex_1().h(26).items_center().justify_center().rounded(5)
          .text_size(S().secondary).text_color(on ? t.foreground : t.muted).bg(on ? t.selected : t.chrome).when(on, (el) => el.font_semibold())
          .cursor_pointer().on_click((_e, cx) => { this.intent = id; cx.notify(); }).child(INTENT[id].verb);
      })))
      .when(Boolean(message), (el) => el.child(div().id("nt-message").flex_none().max_h(96).overflow_y_scrollbar().px(10).py(8).rounded(8)
        .border_1().border_color(t.border).bg(t.background).font_family(mono()).text_size(S().label)
        .child(plan ? plan.shown(message) : message)))
      .child(div().text_size(S().secondary).text_color(this.intent === "yolo" ? t.working : t.muted)
        .child(this.intent === "yolo" ? YOLO_NOTE(agent?.name ?? "The agent")
          : `Opens tab “${clean(p.frame?.name, 30)} · ${this.intent}” in ${p.spaceName ?? "this space"}.`))
      .when(Boolean(plan || this.failed || this.progress), (el) => el.child(div().text_size(S().secondary)
        .text_color(this.failed ? t.ansi1 : this.progress ? t.working : t.muted).child(this.failed ?? this.progress ?? plan?.note ?? "")))
      .child(h_flex().justify_end().gap(8)
        .child(secondary("nt-cancel", "Cancel", t, close))
        .child(agent ? primary("nt-open", this.progress ? "Starting…" : `Open tab with ${agent.name}`, t, (_e, cx) => open(cx), "plus")
          : secondary("nt-open", "No agent installed", t, () => {}, { color: t.muted })));
  }

  // ---- Quick Look: one screen at full size in a floating window ----

  renderQuickLook(cx) {
    const t = theme(), p = blockProps() ?? {}, q = shared(QL);
    const close = () => closeOverlay(p.overlay);
    if (!q?.frames?.length) return v_flex().size_full().bg(t.popover).p(18).child(div().text_color(t.muted).child("Nothing to show"));
    const index = Math.max(0, Math.min(q.index, q.frames.length - 1)), f = q.frames[index];
    const move = (step) => setShared(QL, { ...q, index: (index + step + q.frames.length) % q.frames.length });
    const dir = `${appHome()}/${CACHE}/${q.key}/full`, look = `${q.key}:${f.id}`, state = this.looks.get(look);
    if (!state) {
      this.looks.set(look, "loading");
      cx.spawn(async (cx) => {
        const ok = await renderNode(q.bin, q.file, f.id, dir, LOOK_MAX, f.size).catch(() => false);
        this.looks.set(look, ok ? "ready" : "error"); cx.notify();
      });
    }
    const agent = agents().find((a) => a.focused) ?? agents().find((a) => a.address === this.lastAgent) ?? null;
    const [w, h] = f.size ?? [];
    const box = fitBox(f.size, Math.max(200, this.width - 40), Math.max(160, this.height - 42 - 40 - 36));
    const prev = q.frames[(index - 1 + q.frames.length) % q.frames.length], next = q.frames[(index + 1) % q.frames.length];
    return v_flex().id("quicklook").track_focus(this.handle)
      .on_size_change((size, cx) => {
        if (Math.abs(size.width - this.width) > 1 || Math.abs(size.height - this.height) > 1) { this.width = size.width; this.height = size.height; cx.notify(); }
      })
      .on_key_down((e, cx) => {
        if (e.key === "escape" || e.key === "space") close();
        else if (e.key === "left") move(-1);
        else if (e.key === "right") move(1);
        else return;
        cx.stop_propagation();
      })
      .size_full().min_h(0).bg(t.popover).text_color(t.foreground)
      .child(h_flex().h(42).flex_none().pl(14).pr(8).gap(10).items_center().border_b(1).border_color(t.border)
        .child(div().flex_none().max_w(260).truncate().text_size(S().title).font_semibold().child(clean(f.name)))
        .child(div().flex_1().min_w(0).truncate().text_size(S().secondary).text_color(t.muted)
          .child([w > 0 ? `${Math.round(w)}×${Math.round(h)}` : null, f.page?.name ? clean(f.page.name) : null, base(q.file.path)].filter(Boolean).join(" · ")))
        .child(secondary("ql-browser", q.file.on ? "Open snapshot" : "Open in browser", t, (_e, cx) => {
          if (this.note?.kind === "working") return;
          this.note = { kind: "working", text: "Starting…" }; cx.notify();
          cx.spawn(async (cx) => {
            const progress = (text) => { this.note = { kind: "working", text }; cx.notify(); };
            try { this.note = { kind: "done", text: await openInBrowser(q.bin, q.file, f.id, q.key, cx, progress) }; }
            catch (e) { this.note = { kind: "error", text: String(e.message ?? e) }; }
            cx.notify();
          });
        }, { icon: "external" }))
        .child(handoffButton(t, "ql", agent,
          () => confirmIntent("build", agent, intentText({ intent: "build", app: agent.app, file: q.file.path, frame: f, page: f.page, screen: f.screen, cli: cliFor(q.cli, q.design, machineOf(agent.address)) }), f.name, q.design),
          () => handoffProps({ agent, file: q.file, frame: f, page: f.page, screen: f.screen, cli: q.cli, design: q.design }),
          () => openOverlay({ mod: "figlens", kind: "dialog", width: 380, height: 540, props: handoffProps({ agent: null, file: q.file, frame: f, page: f.page, screen: f.screen, cli: q.cli, design: q.design }).newtab }),
          true))
        .child(h_flex().id("ql-close").w(26).h(26).flex_none().items_center().justify_center().rounded(6).cursor_pointer()
          .hover((s) => s.bg(t.hover)).on_mouse_down("left", stop).on_click(close).child(icon("close", 14, t.muted))))
      .child(h_flex().flex_1().min_h(0).items_center().justify_center().bg(t.chrome)
        .child(this.looks.get(look) === "ready"
          ? image(`${CACHE}/${q.key}/full/${thumbFile(f.id)}`).w(box.w).h(box.h)
          : this.looks.get(look) === "error" ? div().text_size(S().secondary).text_color(t.muted).child("This layer could not be drawn")
            : working(t, "Drawing at full size…")))
      .child(h_flex().h(40).flex_none().px(10).gap(6).items_center().border_t(1).border_color(t.border).text_size(S().secondary).text_color(t.muted)
        .when(q.frames.length > 1, (el) => el
          .child(secondary("ql-prev", `‹ ${clean(prev.name, 24)}`, t, () => move(-1), { px: 8 }))
          .child(secondary("ql-next", `${clean(next.name, 24)} ›`, t, () => move(1), { px: 8 })))
        .child(div().flex_1())
        .child(this.note
          ? h_flex().gap(7).items_center().min_w(0).text_color(statusColor(t, this.note.kind))
            .child(this.note.kind === "working" ? icon("ring", 13, statusColor(t, "working")).spin(900) : icon(STATUS_ICON[this.note.kind] ?? "alert", 13, statusColor(t, this.note.kind)))
            .child(div().min_w(0).truncate().child(this.note.text))
          : div().child(q.frames.length > 1 ? `${index + 1} of ${q.frames.length} · Space or Esc closes · ← → moves` : "Space or Esc closes")));
  }

  // ---- Panel: engine, file, screens ----

  // Downloads figlens VERSION on the machine the open design is on, then reads it again.
  install(cx) {
    const engine = this.engine, file = this.file;
    this.installing = true; this.installError = null; cx.notify();
    cx.spawn(async (cx) => {
      try {
        const [kind, arch] = (await sh("uname -s; uname -m", [], engine.on)).stdout.trim().split("\n");
        const archive = asset(KIND[kind], arch);
        if (!archive) throw new Error(`figlens has no build for ${kind}/${arch}`);
        // exec stops at 30 s, too short for a slow download: run it as a hidden Herdr terminal.
        const run = await hc(["run", "--bg", "--json", "--no-parent", ...(engine.on ? ["--on", engine.on] : []),
          "--", "sh", "-c", INSTALL, "figlens", engine.dir, archive, releaseBase()]);
        if (run.code !== 0) throw new Error(lastLine(run.stderr) || "Could not start the download");
        const terminal = JSON.parse(run.stdout).address;
        let code = null;
        for (let i = 0; i < 900 && code === null; i += 1) {
          await cx.sleep(1000);
          const g = await hc(["get", terminal, "--json"]).catch(() => null);
          try { const d = JSON.parse(g.stdout); if (d.finished_ms != null) code = d.exit_code ?? 1; } catch { /* still running */ }
        }
        const log = await hc(["read", terminal, "--lines", "20"]).catch(() => null);
        await hc(["rm", terminal]).catch(() => null);
        if (code !== 0) throw new Error(code === null ? "The download took longer than 15 minutes" : lastLine(log?.stdout) || "Download failed");
        if (same(file, this.file)) this.load(file, cx);
      } catch (e) { this.installError = String(e.message ?? e); }
      finally { this.installing = false; cx.notify(); }
    });
  }

  // "<mtime> <size>" of the design where it lives.
  async stat(file) {
    const r = await sh(STAT, [file.path], file.on);
    if (r.code !== 0) throw new Error(GONE);
    return r.stdout.trim();
  }

  // Reads the design's screens with figlens on the design's own machine.
  // Called from render: state set here shows in this pass, so it must not notify.
  load(file, cx) {
    const token = ++this.token;
    if (!same(file, this.file)) this.activity = new Map();
    this.file = file; this.groups = []; this.thumbs = new Set(); this.selected = null;
    this.error = null; this.gone = false; this.queue = []; this.drawing = false; this.fold = new Map(); this.previews = new Map();
    this.lookToken += 1; this.hit = null; this.layers = null; this.looking = false;
    if (!file) return;
    this.loading = true;
    cx.spawn(async (cx) => {
      try {
        const stamp = await this.stat(file);
        if (token !== this.token) return;
        this.stamp = stamp;
        if (this.engine?.on !== file.on || !this.engine?.bin) {
          const engine = await findEngine(file.on);
          if (token !== this.token) return;
          this.engine = engine;
        }
        if (this.engine.missing) return;
        const bin = this.engine.bin;
        const prefix = hash(`${file.on ?? ""}:${file.path}`), key = `${prefix}-${hash(stamp)}`;
        const [doc, top] = await Promise.all([
          figlens(bin, ["tree", file.path, "--json", "--depth", "1"], file.on),
          figlens(bin, ["tree", file.path, "--top-frames", "--json"], file.on),
        ]);
        if (token !== this.token) return;
        this.groups = groupFrames(JSON.parse(doc), JSON.parse(top));
        for (const g of this.groups) for (const f of g.frames) f.find = `${clean(f.name)}\n${clean(f.firstText)}`.toLowerCase();
        await this.prepareThumbs(prefix, key);
        if (token !== this.token) return;
        this.key = key; this.loading = false; cx.notify();
        const q = shared(QL);
        if (q && same(q.file, file) && q.key !== key) setShared(QL, { ...q, key, design: this.design() });
        this.watch(file, stamp, cx);
        this.follow(file, cx);
        if (this.raw) this.lookup(cx);
      } catch (e) {
        if (token === this.token) {
          const message = String(e.message ?? e);
          if (message === GONE) this.gone = true; else this.error = message;
        }
      } finally {
        if (token === this.token) this.loading = false;
        cx.notify();
      }
    });
  }

  // This revision's image folder: older revisions of the file go, drawn images are kept, and
  // only the newest revisions of all designs stay on disk.
  async prepareThumbs(prefix, key) {
    const root = `${appHome()}/${CACHE}`, dir = `${root}/${key}`;
    await mkdir(dir, { recursive: true });
    await exec("touch", [dir]);
    const old = (await readdir(root)).filter((d) => d.startsWith(`${prefix}-`) && d !== key);
    if (old.length) await exec("rm", ["-rf", ...old.map((d) => `${root}/${d}`)]);
    await sh(PRUNE, [root, String(KEEP_REVISIONS + 1)]);
    const have = new Set(await readdir(dir));
    for (const g of this.groups) for (const f of g.frames) if (have.has(thumbFile(f.id))) this.thumbs.add(f.id);
    this.queue = []; this.tried = new Set(); this.drawing = false; this.batchDone = 0; this.batchTotal = 0;
  }

  // Thumbnails are drawn only for screens in open pages, in list order: a folded page, a hidden
  // kind or a filtered-out row costs nothing. Called from render: it only queues and starts work.
  want(ids, cx) {
    if (!this.key || !this.engine?.bin) return;
    let added = 0;
    for (const id of ids) {
      if (this.thumbs.has(id) || this.tried.has(id)) continue;
      this.tried.add(id); this.queue.push(id); added += 1;
    }
    this.batchTotal += added;
    if (this.queue.length && !this.drawing) this.draw(cx);
  }

  draw(cx) {
    const token = this.token, file = this.file, bin = this.engine.bin, dir = `${appHome()}/${CACHE}/${this.key}`;
    const max = String(THUMB_W * 3), step = file.on ? REMOTE_CHUNK : CHUNK;
    this.drawing = true;
    cx.spawn(async (cx) => {
      try {
        while (this.queue.length && token === this.token) {
          const chunk = this.queue.splice(0, step);
          if (file.on) {
            const r = await sh(REMOTE_EXPORT, [bin, file.path, max, String(THUMB_SCALE), ...chunk], file.on);
            if (token !== this.token) return;
            if (r.code === 0 && r.stdout.trim()) await unpackImages(r.stdout, dir);
            const now = new Set(await readdir(dir));
            for (const id of chunk) if (now.has(thumbFile(id))) this.thumbs.add(id);
          } else {
            await writeFile(`${dir}/.frames`, chunk.join("\n"));
            const out = await exec(bin, ["export", file.path, "--frames", `${dir}/.frames`, "--out-dir", dir, "--format", "png", "--bounds", "layout", "--scale", String(THUMB_SCALE), "--max-size", max]);
            if (token !== this.token) return;
            for (const line of out.stdout.split("\n")) {
              try { const r = JSON.parse(line); if (r.nodeId && !r.error) this.thumbs.add(r.nodeId); } catch { /* not a result line */ }
            }
          }
          this.batchDone += chunk.length;
          cx.notify();
        }
      } catch { /* rows without a thumbnail keep their outline */ }
      finally {
        if (token === this.token) { this.drawing = false; this.batchDone = 0; this.batchTotal = 0; }
        cx.notify();
      }
    });
  }

  // Reloads when the design is saved again: a folder watch here, a stat poll on another machine.
  watch(file, stamp, cx) {
    const id = `${file.on ?? ""}:${file.path}`;
    if (this.watching === id) return;
    this.watching = id;
    const still = () => this.watching === id && same(this.file, file);
    const poll = async (cx) => {
      for (let last = stamp; still();) {
        await cx.sleep(POLL);
        const now = await this.stat(file).catch(() => last);
        if (still() && now !== last) { last = now; this.load(file, cx); cx.notify(); }
      }
    };
    cx.spawn(async (cx) => {
      try {
        if (file.on) { await poll(cx); return; }
        const folder = file.path.slice(0, file.path.lastIndexOf("/")) || "/";
        try {
          while (still()) {
            const changed = await watchFolder(folder);
            if (still() && changed.some((p) => p === file.path || p === folder)) { this.load(file, cx); cx.notify(); }
          }
        } catch { await poll(cx); } // a folder too large to watch (Linux): poll it instead
      } catch { /* the watch ends; reopening the file starts another */ }
      finally { if (this.watching === id) this.watching = null; }
    });
  }

  // What agents read in this design: the figlens activity journal on the design's machine, and
  // for its copy on each other agent machine. Polled; redraws while anyone is active.
  follow(file, cx) {
    const id = `${file.on ?? ""}:${file.path}`;
    if (this.following === id) return;
    // A generation, not the id: reopening a file while an older loop for it sleeps must not
    // leave two loops polling.
    const gen = (this.followGen ?? 0) + 1;
    this.followGen = gen; this.following = id;
    const still = () => this.followGen === gen && same(this.file, file);
    cx.spawn(async (cx) => {
      try {
        while (still()) {
          // Only machines with agents: lines from terminals that are not agents never show.
          const from = file.on ?? null, copy = `${COPIES}/${copyKey(this.design())}/${base(file.path)}`;
          const machines = new Set(agents().map((a) => machineOf(a.address)));
          const all = new Map(), now = Date.now();
          const reads = await Promise.all([...machines].map((m) => sh(ACTIVITY, [m === from ? file.path : copy], m).catch(() => null)));
          if (!still()) return;
          [...machines].forEach((m, i) => latestActivity(reads[i]?.stdout, m, now, undefined, all));
          // Redraw while anyone is active (the "12s ago" moves) and once more when the last one goes.
          if (all.size || this.activity.size) { this.activity = all; cx.notify(); }
          await cx.sleep(ACTIVITY_POLL);
        }
      } catch { /* the follow ends; reopening the file starts another */ }
      finally { if (this.followGen === gen) this.following = null; }
    });
  }

  // Live agents with recent reads in this design, newest first, each with its color.
  readers(t) {
    const colors = [t.ansi4, t.ansi5, t.ansi6, t.ansi3, t.ansi2, t.ansi1];
    const live = agents();
    return [...this.activity.values()]
      .map((e) => ({ e, agent: live.find((a) => a.address === e.address) }))
      .filter((r) => r.agent)
      // Colors follow terminal order, so an agent keeps its color while others come and go less often.
      .sort((a, b) => a.e.address.localeCompare(b.e.address))
      .map((r, i) => ({ ...r, color: colors[i % colors.length] }))
      .sort((a, b) => b.e.at - a.e.at);
  }

  // Shows the screen an agent is reading as the filter's node card: the one card, its preview
  // and the agent's box, however long the list. Clearing the filter brings the list back.
  reveal(r, cx) {
    const id = r.e.screen;
    if (!id || !this.topIndex().has(id)) return;
    this.filter.set_value(id);
    this.raw = id; this.query = id.toLowerCase();
    this.lookup(cx);
  }

  // One line per agent reading this design: who, which tool, which screen, how long ago.
  readersStrip(t) {
    const list = this.reading ?? [];
    if (!list.length) return null;
    const top = this.topIndex(), now = Date.now();
    return v_flex().gap(1).children(list.map((r) => {
      const place = r.e.screen && top.get(r.e.screen);
      // A big design repeats screen names: the page tells them apart.
      const what = place ? `${clean(place.frame.name, 60)} · ${clean(place.page.name, 40)}`
        : r.e.nodes[0] && r.e.nodes[0] !== "0:0" ? `node ${r.e.nodes[0]}` : "the file";
      return h_flex().id(`reader-${r.e.address}`).h(24).px(6).gap(7).items_center().rounded(5).text_size(S().secondary)
        .when(Boolean(place), (el) => el.cursor_pointer().hover((s) => s.bg(t.hover)).on_click((_e, cx) => { this.reveal(r, cx); cx.notify(); }))
        .child(dot(r.color, 8))
        .child(div().flex_none().font_semibold().child(r.agent.program?.name ?? r.e.address))
        .child(div().flex_1().min_w(0).truncate().text_color(t.muted).child(`${r.e.tool} · ${what}`))
        .child(div().flex_none().text_size(S().label).text_color(t.muted).child(ago(now - r.e.at)));
    }));
  }

  // .fig files under a folder (the picked one, else where the person is working).
  // Called from render: it only starts work.
  discover(place, cx) {
    this.foundKey = JSON.stringify(place); this.found = [];
    if (!place) return;
    // A scan number, not the place: a refresh while a scan runs must win over it.
    const scan = (this.scan ?? 0) + 1;
    this.scan = scan; this.scanning = true;
    cx.spawn(async (cx) => {
      const r = await sh(FIND, [place.cwd], place.on).catch(() => null);
      if (this.scan !== scan) return;
      this.found = (r?.stdout ?? "").split("\n").filter(Boolean); this.scanning = false;
      cx.notify();
    });
  }

  pick(cx) {
    cx.spawn(async (cx) => {
      const r = await choose().catch((e) => { this.say("error", String(e.message ?? e)); return null; });
      if (r?.path) this.open({ path: r.path, on: null });
      else if (r?.folder) this.picked = { cwd: r.folder, on: null };
      cx.notify();
    });
  }

  // Opens a design for this space (null closes it); a closed design's browser viewer ends.
  open(file) {
    this.say(null);
    const space = activeSpace()?.id, all = { ...(shared(FILES) ?? {}) }, was = all[space];
    if (!file && was) {
      const key = `${was.on ?? ""}:${was.path}`, viewers = { ...(shared(VIEWERS) ?? {}) };
      const stillOpen = Object.entries(all).some(([s, f]) => s !== space && same(f, was));
      if (viewers[key] && !stillOpen) { hc(["kill", viewers[key].terminal]).catch(() => null); delete viewers[key]; setShared(VIEWERS, viewers); }
    }
    if (file) all[space] = file; else delete all[space];
    setShared(FILES, all); // every copy re-renders and loads it
  }

  // A notice for the status bar; the newest of this copy's and the dialogs' shows.
  say(kind, text) {
    this.notice = kind ? { kind, text, at: Date.now() } : null;
    if (!kind && shared(NOTICE)) setShared(NOTICE, null);
  }

  // The engine runs on the design's machine, so its CLI path is valid there.
  cli() { return this.engine?.onPath ? null : this.engine?.bin; }

  // The agent the actions go to: the one focused on the left, else the last one that was,
  // since using this panel's keyboard takes focus away from it.
  receiver() {
    const a = agents().find((x) => x.focused);
    if (a) { this.lastAgent = a.address; return a; }
    return agents().find((x) => x.address === this.lastAgent) ?? null;
  }

  textFor(intent, agent, target, page, screen) {
    const cli = cliFor(this.cli(), this.design(), machineOf(agent?.address));
    return intentText({ intent, app: agent?.app ?? null, file: this.file.path, frame: target, page, screen, cli });
  }

  // The open design as a dialog needs it to copy it to an agent's machine.
  design() {
    return this.file ? { path: this.file.path, on: this.file.on ?? null, stamp: this.stamp, size: Number(String(this.stamp ?? "").split(" ")[1]) || 0 } : null;
  }

  // ---- Finding a node: an id or Figma link, or any layer by name or text ----

  topIndex() {
    const top = new Map();
    for (const g of this.groups) for (const f of g.frames) top.set(f.id, { frame: f, page: g });
    return top;
  }

  // The screen and page that hold a layer, from find's ancestor path.
  placeOf(path, top) {
    for (const p of path) if (top.has(p.id)) return top.get(p.id);
    const page = this.groups.find((g) => g.id === path[1]?.id) ?? (path[1] ? { id: path[1].id, name: path[1].name } : null);
    return { frame: null, page };
  }

  // Filter changes run here (an event, so it may notify). An id or link resolves at once;
  // text searches every layer after a short pause.
  lookup(cx) {
    const raw = this.raw, token = ++this.lookToken;
    this.hit = null; this.layers = null; this.looking = false;
    if (!raw || !this.file || !this.engine?.bin || !this.key) return;
    const ref = nodeRef(raw);
    if (!ref && raw.length < 2) return;
    this.looking = true;
    cx.spawn(async (cx) => {
      if (!ref) await cx.sleep(300);
      if (token !== this.lookToken) return;
      let hit = null, layers = null;
      try {
        if (ref) hit = await this.resolve(ref);
        else layers = await this.findLayers(raw);
      } catch { if (ref) hit = { ref, missing: true }; }
      if (token !== this.lookToken) return;
      this.hit = hit; this.layers = layers; this.looking = false;
      if (hit?.node) this.preview(hit.node, cx);
      cx.notify();
    });
  }

  // A node by id: what it is, then which screen holds it. find narrows by the node's type, and
  // a text layer is also looked up by its text, so a common name still finds its own node.
  async resolve(ref) {
    const bin = this.engine.bin, f = this.file, top = this.topIndex();
    const n = JSON.parse(await figlens(bin, ["inspect", f.path, ref], f.on)).node;
    if (!n?.id) return { ref, missing: true };
    const box = n.absoluteBoundingBox;
    const node = { id: n.id, name: n.name ?? n.id, type: n.type, size: box ? [box.width, box.height] : null };
    if (top.has(n.id)) return { ref, node, ...top.get(n.id), isScreen: true };
    const queries = [n.name, n.type === "TEXT" ? n.characters : null].filter((s) => s && s.trim());
    for (const q of queries) {
      const args = ["find", f.path, q, "--limit", "200", ...(n.type ? ["--type", n.type] : [])];
      const r = (JSON.parse(await figlens(bin, args, f.on)).results ?? []).find((x) => x.id === n.id);
      if (r) return { ref, node, ...this.placeOf(r.path ?? [], top) };
    }
    return { ref, node, frame: null, page: null, unplaced: true };
  }

  async findLayers(text) {
    const bin = this.engine.bin, f = this.file, top = this.topIndex();
    const found = JSON.parse(await figlens(bin, ["find", f.path, text, "--limit", "30"], f.on));
    const rows = (found.results ?? []).filter((r) => !top.has(r.id) && r.type !== "CANVAS").map((r) => ({
      node: { id: r.id, name: r.name ?? r.id, type: r.type, size: r.size ? [r.size.width, r.size.height] : null, text: r.characters },
      ...this.placeOf(r.path ?? [], top),
    }));
    return { rows, truncated: Boolean(found.truncated) };
  }

  // ---- Preview, Quick Look, browser, new tab ----

  // A large render of the selected screen or layer, made once per revision.
  preview(target, cx) {
    if (this.previews.has(target.id) || !this.engine?.bin) return;
    const file = this.file, key = this.key, bin = this.engine.bin;
    this.previews.set(target.id, "loading");
    cx.spawn(async (cx) => {
      const ok = await renderNode(bin, file, target.id, `${appHome()}/${CACHE}/${key}/preview`, PREVIEW_MAX, target.size).catch(() => false);
      if (!same(file, this.file) || key !== this.key) return;
      this.previews.set(target.id, ok ? "ready" : "error");
      cx.notify();
    });
  }

  previewBox(target, t) {
    const width = Math.max(160, this.width - 34), box = fitBox(target.size, width, PREVIEW_H), state = this.previews.get(target.id);
    // Where agents read inside this screen: a box in each agent's color.
    const scale = box.w / (target.size?.[0] || box.w);
    // Only the part inside the screen is drawn; a box wholly outside it is not.
    const marks = (this.reading ?? []).filter((r) => r.e.screen === target.id && r.e.box).flatMap((r) => {
      const [x, y, w, h] = r.e.box.map((v) => v * scale);
      const left = Math.max(0, x), top = Math.max(0, y);
      const width = Math.min(box.w, x + w) - left, height = Math.min(box.h, y + h) - top;
      if (!(width >= 1 && height >= 1)) return [];
      return [div().absolute().left(left).top(top).w(Math.max(4, width)).h(Math.max(4, height))
        .border_2().border_color(r.color).rounded(2)];
    });
    return h_flex().h(box.h + 16).flex_none().items_center().justify_center().rounded(6).bg(t.chrome)
      .child(state === "ready"
        ? div().relative().w(box.w).h(box.h).child(image(`${CACHE}/${this.key}/preview/${thumbFile(target.id)}`).w(box.w).h(box.h)).children(marks)
        : state === "error"
          ? div().text_size(S().secondary).text_color(t.muted).child("Preview unavailable for this layer")
          : working(t, "Rendering preview…"));
  }

  // Quick Look over the visible list, starting at the target.
  quickLook(target, page, screen) {
    const frames = this.listed.map((v) => ({ id: v.target.id, name: v.target.name, size: v.target.size, page: { name: v.page?.name }, screen: v.screen ? { name: v.screen.name } : null }));
    let index = frames.findIndex((f) => f.id === target.id);
    if (index < 0) { frames.unshift({ id: target.id, name: target.name, size: target.size, page: { name: page?.name }, screen: screen ? { name: screen.name } : null }); index = 0; }
    setShared(QL, { file: this.file, bin: this.engine.bin, cli: this.cli(), key: this.key, frames, index, design: this.design() });
    openOverlay({ mod: "figlens", kind: "window", key: "figlens-quicklook", width: 920, height: 660, props: { role: "quicklook", agent: this.lastAgent } });
  }

  browser(target, cx) {
    const bin = this.engine.bin, file = this.file, key = this.key;
    this.say("working", file.on ? `Making a snapshot on ${file.on}…` : "Starting the viewer…"); cx.notify();
    cx.spawn(async (cx) => {
      const progress = (text) => { this.say("working", text); cx.notify(); };
      const started = (terminal) => { (this.started ??= new Set()).add(terminal); };
      try { this.say("done", await openInBrowser(bin, file, target.id, key, cx, progress, started)); }
      catch (e) { this.say("error", String(e.message ?? e)); }
      cx.notify();
    });
  }

  newTab(target, page, screen) {
    openOverlay({ mod: "figlens", kind: "dialog", width: 380, height: 540,
      props: handoffProps({ agent: null, file: this.file, frame: target, page, screen, cli: this.cli(), design: this.design() }).newtab });
  }


  // The selected thing's details: a large preview, the handoff split button, and quiet tools.
  details(target, page, t, screen) {
    const agent = this.receiver(), id = target.id, name = agent?.program?.name;
    const copyId = (_e, cx) => { cx.write_to_clipboard(id); this.say("info", `Copied node ID ${id}`); cx.notify(); };
    const tool = (key, label, glyph, onClick) => secondary(`${key}-${id}`, label, t, onClick, { icon: glyph, size: S().secondary, px: 8, color: t.muted });
    return [
      this.previewBox(target, t),
      handoffButton(t, id, agent,
        () => confirmIntent("build", agent, this.textFor("build", agent, target, page, screen), target.name, this.design()),
        () => handoffProps({ agent, file: this.file, frame: target, page, screen, cli: this.cli(), design: this.design() }),
        () => this.newTab(target, page, screen)),
      div().text_size(S().secondary).text_color(t.muted).child(agent
        ? `${name}${agentWhere(agent) ? ` · ${agentWhere(agent)}` : ""} · ${agent.focused ? "focused on the left" : "last focused on the left"}`
        : `Focus an agent on the left to hand this ${screen ? "layer" : "screen"} over, or start one in a new tab.`),
      h_flex().gap(6).flex_wrap()
        .child(tool("look", "Quick Look", "eye", () => this.quickLook(target, page, screen)))
        .child(tool("browser", this.file.on ? "Snapshot" : "Browser", "external", (_e, cx) => this.browser(target, cx)))
        .child(secondary(`id-${id}`, h_flex().gap(5).child(div().child("Node")).child(div().font_family(mono()).text_color(t.foreground).child(id)),
          t, copyId, { icon: "copy", size: S().secondary, px: 8, color: t.muted })),
    ];
  }

  select(entry, cx) {
    this.selected = entry.target.id; this.say(null); this.preview(entry.target, cx);
  }

  // ---- Panel drawing ----

  frameRow(frame, page, t) {
    const on = this.selected === frame.id, box = fitBox(frame.size, THUMB_W, THUMB_H), ready = this.thumbs.has(frame.id);
    const [w, h] = frame.size ?? [];
    const row = h_flex().id(`frame-${frame.id}`).h(60).flex_none().gap(10).px(8).py(6).items_center().cursor_pointer()
      .when(on, (el) => el.bg(t.selected).rounded_t(7)).when(!on, (el) => el.rounded(7).hover((s) => s.bg(t.hover)))
      .on_click((_e, cx) => { if (on) this.selected = null; else this.select({ target: frame, page }, cx); this.say(null); cx.notify(); })
      .child(div().w(THUMB_W).h(THUMB_H).flex_none().flex().items_center().justify_center().rounded(4).bg(t.chrome)
        .child(ready
          ? image(`${CACHE}/${this.key}/${thumbFile(frame.id)}`).w(box.w).h(box.h)
          : div().w(box.w).h(box.h).rounded(2).border_1().border_dashed().border_color(t.border)))
      .child(v_flex().flex_1().min_w(0).gap(3)
        .child(h_flex().gap(6).items_center()
          .child(div().min_w(0).truncate().text_size(S().body).when(on, (el) => el.font_semibold()).child(clean(frame.name)))
          .children((this.reading ?? []).filter((r) => r.e.screen === frame.id).map((r) => dot(r.color, 7))))
        .child(div().truncate().text_size(S().secondary).text_color(t.muted)
          .child([w > 0 ? `${Math.round(w)}×${Math.round(h)}` : null, clean(frame.firstText, 60) || null].filter(Boolean).join(" · "))));
    if (!on) return row;
    return v_flex().flex_none().child(row)
      .child(v_flex().gap(10).px(8).pt(2).pb(10).bg(t.selected).rounded_b(7).children(this.details(frame, page, t)));
  }

  // An id or link's node: what it is, where it sits, and the same actions as a screen.
  nodeCard(t) {
    const ref = nodeRef(this.raw);
    if (this.looking) return h_flex().mx(8).my(6).child(working(t, `Looking up node ${ref}…`));
    const hit = this.hit;
    if (!hit) return div();
    if (hit.missing) {
      return h_flex().mx(8).my(6).gap(7).items_center().text_size(S().secondary).text_color(t.muted)
        .child(icon("alert", 13, t.muted)).child(`No node ${hit.ref} in this file`);
    }
    const n = hit.node, screen = hit.isScreen ? null : hit.frame;
    const where = hit.unplaced ? "figlens could not place this layer in a screen"
      : [screen ? `in ${clean(screen.name)}` : null, hit.page ? clean(hit.page.name) : null].filter(Boolean).join(" · ");
    const [w, h] = n.size ?? [];
    return v_flex().mx(4).my(4).px(8).pt(8).pb(10).gap(10).rounded(8).bg(t.selected)
      .child(v_flex().gap(3)
        .child(h_flex().gap(8).items_center()
          .child(pill(n.type ?? "NODE", t))
          .child(div().flex_1().min_w(0).truncate().text_size(S().body).font_semibold().child(clean(n.name))))
        .child(div().truncate().text_size(S().secondary).text_color(t.muted)
          .child([where, w > 0 ? `${Math.round(w)}×${Math.round(h)}` : null].filter(Boolean).join(" · "))))
      .children(this.details(n, hit.page, t, screen));
  }

  // Layers inside screens that match the text, below the matching screens.
  layersSection(t) {
    if (this.looking && !nodeRef(this.raw)) return h_flex().mx(8).my(6).child(working(t, "Searching layers…"));
    const found = this.layers;
    if (!found?.rows.length) return div();
    return v_flex().gap(2).pt(6)
      .child(h_flex().h(30).px(8).gap(6).items_center().text_size(S().label).font_semibold().text_color(t.muted)
        .child(div().flex_1().child("Layers inside screens"))
        .child(div().font_normal().child(found.truncated ? `first ${found.rows.length}` : String(found.rows.length))))
      .children(found.rows.map((r, i) => {
        const on = this.hit?.node?.id === r.node.id;
        const label = r.node.type === "TEXT" && r.node.text ? clean(r.node.text, 80) : clean(r.node.name);
        const where = [r.frame ? clean(r.frame.name) : null, r.page ? clean(r.page.name) : null].filter(Boolean).join(" · ");
        return v_flex().flex_none()
          .child(h_flex().id(`layer-${i}`).h(44).px(8).gap(8).items_center().cursor_pointer()
            .when(on, (el) => el.bg(t.selected).rounded_t(7)).when(!on, (el) => el.rounded(7).hover((s) => s.bg(t.hover)))
            .on_click((_e, cx) => {
              this.hit = on ? null : { ref: r.node.id, node: r.node, frame: r.frame, page: r.page };
              if (!on) this.preview(r.node, cx);
              cx.notify();
            })
            .child(div().w(46).flex_none().truncate().text_size(S().label).text_color(t.muted).child(r.node.type ?? ""))
            .child(v_flex().flex_1().min_w(0).gap(2)
              .child(div().truncate().text_size(S().body).when(on, (el) => el.font_semibold()).child(label))
              .child(div().truncate().text_size(S().secondary).text_color(t.muted).child(where))))
          .when(on, (el) => el.child(v_flex().gap(10).px(8).pt(2).pb(10).bg(t.selected).rounded_b(7)
            .children(this.details(r.node, r.page, t, r.frame))));
      }));
  }

  // Nothing open: one target that opens the picker, and the .fig files found nearby.
  start(t, cx) {
    const place = this.picked ?? here();
    if (JSON.stringify(place) !== this.foundKey) this.discover(place, cx);
    const label = place ? (this.picked ? `In ${base(place.cwd) || place.cwd}` : `Near your focus · ${base(place.cwd) || place.cwd}`) : null;
    const platform = window()?.platform;
    return v_flex().p(12).gap(14)
      .child(v_flex().id("choose").h(104).flex_none().gap(6).items_center().justify_center().rounded(8)
        .border_1().border_dashed().border_color(t.input).cursor_pointer().hover((s) => s.bg(t.hover))
        .on_click((_e, cx) => this.pick(cx))
        .child(icon("upload", 18, t.muted))
        .child(div().text_size(S().body).font_semibold().child("Open a .fig file"))
        .child(div().text_size(S().secondary).text_color(t.muted)
          .child(platform === "macos" || platform === "linux" ? "Drop it here or click to choose" : "Drop it here or click to choose a folder")))
      .when(Boolean(label), (el) => el.child(v_flex().gap(2)
        .child(h_flex().h(26).px(8).gap(6).items_center().text_size(S().label).font_semibold().text_color(t.muted)
          .child(div().min_w(0).truncate().child(label))
          .when(Boolean(place?.on), (el) => el.child(pill(place.on, t)))
          // Files saved there since the last look show on a refresh.
          .child(h_flex().id("rescan").w(22).h(22).flex_none().items_center().justify_center().rounded(6).cursor_pointer()
            .hover((s) => s.bg(t.hover)).on_click((_e, cx) => { this.foundKey = null; cx.notify(); })
            .child(icon("refresh", 12, t.muted)))
          .when(Boolean(this.picked), (el) => el.child(div().flex_1()).child(h_flex().id("unpick").h(22).px(6).gap(4).items_center().rounded(6)
            .text_size(S().secondary).text_color(t.foreground).cursor_pointer().hover((s) => s.bg(t.hover))
            .on_click((_e, cx) => { this.picked = null; cx.notify(); })
            .child(icon("crosshair", 12, t.foreground)).child("Follow focus"))))
        .when(!this.found.length, (el) => el.child(div().px(8).py(4).text_size(S().secondary).text_color(t.muted).child(this.scanning ? "Looking…" : "No .fig files here")))
        .children(this.found.map((p, i) => {
          const rel = p.startsWith(`${place.cwd}/`) ? p.slice(place.cwd.length + 1) : p;
          const cut = rel.lastIndexOf("/");
          return h_flex().id(`found-${i}`).h(32).px(8).gap(8).items_center().rounded(7).cursor_pointer()
            .hover((s) => s.bg(t.hover)).on_click(() => this.open({ path: p, on: place.on }))
            .child(icon("file", 14, t.dim))
            .child(div().flex_none().max_w(200).truncate().text_size(S().body).child(cut < 0 ? rel : rel.slice(cut + 1)))
            .child(div().flex_1().min_w(0).truncate().text_right().text_size(S().secondary).text_color(t.muted).child(cut < 0 ? "" : rel.slice(0, cut)));
        }))));
  }

  // The caches, pinned to the panel's bottom while nothing is open: this panel's images
  // (thumbnails, previews, Quick Look images, snapshots) and figlens's own render cache, here
  // and on the focused space's machine, where viewers and snapshots render.
  // Called from render: measuring only starts work.
  cacheRow(t, cx) {
    if (this.cacheBytes === null && !this.measuring) {
      this.measuring = true;
      cx.spawn(async (cx) => {
        const r = await sh('du -sk "$1" 2>/dev/null | cut -f1', [`${appHome()}/${CACHE}`]).catch(() => null);
        const machines = [...new Set([null, here()?.on ?? null])], renders = [];
        for (const on of machines) {
          const engine = await findEngine(on).catch(() => null);
          if (!engine?.bin) continue;
          const out = await exec(engine.bin, ["cache"], { on }).catch(() => null);
          try { renders.push({ on, bin: engine.bin, bytes: JSON.parse(out.stdout).bytes || 0 }); } catch { /* no render cache there */ }
        }
        const copies = [];
        for (const on of machines) {
          const dir = on ? `${(await sh('printf %s "$HOME"', [], on).catch(() => ({ stdout: "" }))).stdout.trim()}/${COPIES}` : `${home()}/${COPIES}`;
          const d = await sh('du -sk "$1" 2>/dev/null | cut -f1', [dir], on).catch(() => null);
          const bytes = (Number(d?.stdout?.trim()) || 0) * 1024;
          if (bytes) copies.push({ on, dir, bytes });
        }
        this.cacheBytes = (Number(r?.stdout?.trim()) || 0) * 1024; this.renders = renders; this.copies = copies; this.measuring = false; cx.notify();
      });
    }
    const mb = (n) => `${mbOf(n)} MB`, renders = this.renders ?? [];
    const renderBytes = renders.reduce((n, x) => n + x.bytes, 0), copyBytes = (this.copies ?? []).reduce((n, x) => n + x.bytes, 0);
    const total = (this.cacheBytes ?? 0) + renderBytes + copyBytes;
    const text = this.cacheBytes === null ? "Cache · measuring…"
      : `Images ${mb(this.cacheBytes)} · figlens ${mb(renderBytes)}${copyBytes ? ` · copies ${mb(copyBytes)}` : ""}`;
    return h_flex().h(42).flex_none().px(12).gap(8).items_center().border_t(1).border_color(t.border)
      .child(div().flex_1().min_w(0).truncate().text_size(S().secondary).text_color(t.muted).child(text))
      .child(total > 0
        ? h_flex().id("clear-cache").w(28).h(28).flex_none().items_center().justify_center().rounded(6).border_1().border_color(t.border)
          .cursor_pointer().hover((s) => s.bg(t.hover)).on_mouse_down("left", stop).on_click(() => this.clearCache())
          .child(icon("trash", 14, t.muted))
        : div());
  }

  // Asks first: the dialog lists what goes, then clears.
  clearCache() {
    const renders = this.renders ?? [], copies = this.copies ?? [], panelBytes = this.cacheBytes ?? 0;
    const total = panelBytes + renders.reduce((n, x) => n + x.bytes, 0) + copies.reduce((n, x) => n + x.bytes, 0);
    openOverlay({ mod: "figlens", kind: "dialog", width: 460, height: copies.length ? 280 : 250, props: { role: "clear", renders, copies, panelBytes, total } });
  }

  engineCard(t) {
    const where = this.engine.on ? ` on ${this.engine.on}` : "";
    if (this.installing) return h_flex().p(18).child(working(t, `Downloading figlens ${VERSION}${where}…`));
    return v_flex().px(16).py(18).gap(10)
      .child(div().text_size(S().body).font_semibold().child(`figlens needs its engine${where || " here"}`))
      .child(div().text_size(S().secondary).text_color(t.muted).child(`One download of figlens ${VERSION} from GitHub Releases, about 10 MB, checked against the release's SHA-256 sums. It reads the design where it is; the file never moves.`))
      .child(div().truncate().text_size(S().label).font_family(mono()).text_color(t.muted).child(`${this.engine.dir.replace(/^\/(Users|home)\/[^/]+/, "~")} · shared with npx`))
      .when(Boolean(this.installError), (el) => el.child(h_flex().gap(7).items_center().text_size(S().button).text_color(t.ansi1)
        .child(icon("alert", 14, t.ansi1)).child(div().min_w(0).child(this.installError))))
      .child(h_flex().pt(4).child(this.installError
        ? secondary("install", "Try again", t, (_e, cx) => this.install(cx))
        : primary("install", `Download figlens ${VERSION}`, t, (_e, cx) => this.install(cx), "download")));
  }

  skeleton(t) {
    const bars = [["62%", "40%"], ["48%", "52%"], ["70%", "36%"], ["55%", "44%"], ["64%", "30%"], ["50%", "46%"]];
    return v_flex().p(4).gap(2)
      .child(h_flex().h(28).px(8).items_center().child(div().w(64).h(8).rounded(4).bg(t.selected)))
      .children(bars.map(([a, b]) => h_flex().h(60).px(8).gap(10).items_center()
        .child(div().w(THUMB_W).h(THUMB_H).flex_none().rounded(4).bg(t.chrome))
        .child(v_flex().flex_1().gap(7).child(div().h(9).w(a).rounded(4).bg(t.selected)).child(div().h(8).w(b).rounded(4).bg(t.hover)))));
  }

  goneState(t) {
    return v_flex().px(28).py(40).gap(10).items_center()
      .child(icon("alert", 22, t.ansi1))
      .child(div().text_size(S().body).font_semibold().child(GONE))
      .child(div().text_center().text_size(S().secondary).text_color(t.muted)
        .child(`${base(this.file.path)} was moved or deleted${this.file.on ? ` on ${this.file.on}` : ""}.`))
      .child(h_flex().pt(6).child(secondary("open-another", "Open another file", t, () => this.open(null))));
  }

  // Which screens show and which pages are open, for the toolbar and the list alike.
  listing() {
    const q = this.query;
    const match = (f) => inKind(f, this.kind) && (!q || f.find.includes(q));
    const pages = this.groups.map((g) => ({ ...g, shown: g.frames.filter(match) })).filter((g) => g.shown.length);
    const shown = pages.reduce((n, g) => n + g.shown.length, 0);
    // A long list starts folded to its pages; a filter opens every page that matches.
    const foldDefault = shown <= FOLD_OVER;
    const isOpen = (g) => Boolean(q) || (this.fold.has(g.id) ? this.fold.get(g.id) : foldDefault);
    return { q, pages, shown, isOpen };
  }

  // True once the design's screens are known: the toolbar stays above the list from then on.
  listReady() {
    return Boolean(this.file && !this.gone && !(this.engine?.missing && this.engine.on === this.file.on) && !this.loading && this.groups.length);
  }

  body(t, cx, view) {
    this.visible = []; this.listed = [];
    if (!this.file) return this.start(t, cx);
    if (this.gone) return this.goneState(t);
    if (this.engine?.missing && this.engine.on === this.file.on) return this.engineCard(t);
    if (this.loading) return this.skeleton(t);
    if (!this.groups.length) return div().p(14).text_size(S().button).text_color(t.muted).child(this.error ? "" : "No screens found in this file");
    // An id or a Figma link: that node alone.
    if (!view) return v_flex().p(4).gap(2).child(this.nodeCard(t));
    const { q, pages, isOpen } = view;
    this.listed = pages.flatMap((g) => g.shown.map((f) => ({ target: f, page: g })));
    this.visible = pages.filter(isOpen).flatMap((g) => g.shown.map((f) => ({ target: f, page: g })));
    this.want(this.visible.map((v) => v.target.id), cx);
    return v_flex().p(4).gap(2)
      .when(!pages.length && !(q && (this.looking || this.layers?.rows.length)), (el) => el.child(div().px(12).py(10).text_size(S().secondary).text_color(t.muted)
        .child(q ? "Nothing matches this filter" : this.kind === "components" ? "No components in this file" : "No screens in this file")))
      .children(pages.map((page) => {
        const open = isOpen(page);
        return v_flex().gap(2)
          .child(h_flex().id(`page-${page.id}`).h(30).px(8).gap(6).items_center().rounded(7).cursor_pointer()
            .hover((s) => s.bg(t.hover)).text_size(S().label).font_semibold().text_color(t.muted)
            .on_click((_e, cx) => { if (!q) { this.fold.set(page.id, !open); cx.notify(); } })
            .child(icon(open ? "chevron-down" : "chevron-right", 12, t.muted))
            .child(div().flex_1().min_w(0).truncate().child(clean(page.name)))
            // Agents reading a screen on this page, seen even while it is folded.
            .children((this.reading ?? []).filter((r) => page.frames.some((f) => f.id === r.e.screen)).map((r) => dot(r.color, 7)))
            .child(div().font_normal().child(String(page.shown.length))))
          .when(open, (el) => el.children(page.shown.map((f) => this.frameRow(f, page, t))));
      }))
      .when(Boolean(q), (el) => el.child(this.layersSection(t)));
  }

  // Kind tabs, the filter and fold-all, pinned above the list: how a file with hundreds of
  // top-level layers stays usable.
  toolbar(t, view) {
    const all = this.groups.flatMap((g) => g.frames), count = { screens: 0, components: 0, all: all.length };
    for (const f of all) count[isComponent(f) ? "components" : "screens"] += 1;
    // Fold-all: opens every page while any is closed, else closes them all. A filter opens them anyway.
    const pages = view?.pages ?? [], canFold = Boolean(view && !view.q && pages.length);
    const allOpen = canFold && pages.every((g) => view.isOpen(g)), strip = this.readersStrip(t);
    return v_flex().flex_none().px(12).pt(10).pb(8).gap(8).border_b(1).border_color(t.border).bg(t.background)
      .child(h_flex().p(2).gap(2).rounded(7).bg(t.chrome).children(KINDS.map(([id, label]) => {
        const on = this.kind === id;
        return h_flex().id(`kind-${id}`).flex_1().h(26).gap(5).items_center().justify_center().rounded(5)
          .text_size(S().secondary).text_color(on ? t.foreground : t.muted).bg(on ? t.selected : t.chrome)
          .when(on, (el) => el.font_semibold()).cursor_pointer().hover((s) => s.bg(on ? t.selected : t.hover))
          .on_click((_e, cx) => { this.kind = id; cx.notify(); })
          .child(label).child(div().font_normal().text_color(t.muted).child(String(count[id])));
      })))
      .child(h_flex().gap(6).items_center()
        .child(h_flex().id("filter").flex_1().min_w(0).h(30).pl(10).pr(4).gap(8).items_center().rounded(7).border_1().border_color(this.query ? t.ring : t.border)
          .bg(t.chrome).cursor_text().on_mouse_down("left", stop)
          .child(icon("search", 13, t.muted))
          .child(Input.new(this.filter).flex_1().min_w(0).h_full().text_size(S().body).text_color(t.foreground))
          .when(Boolean(this.query) && !nodeRef(this.raw), (el) => el.child(div().flex_none().text_size(S().label).text_color(t.muted).child(`${view?.shown ?? 0} of ${count[this.kind]}`)))
          .when(Boolean(this.raw), (el) => el.child(h_flex().id("clear-filter").w(22).h(22).flex_none().items_center().justify_center()
            .rounded(5).cursor_pointer().hover((s) => s.bg(t.hover)).on_mouse_down("left", stop)
            .on_click((_e, cx) => this.clearFilter(cx))
            .child(icon("close", 13, t.muted)))))
        .child(h_flex().id("fold-all").w(30).h(30).flex_none().items_center().justify_center().rounded(7).border_1().border_color(t.border)
          .when(canFold, (el) => el.cursor_pointer().hover((s) => s.bg(t.hover)).on_mouse_down("left", stop)
            .on_click((_e, cx) => { for (const g of pages) this.fold.set(g.id, !allOpen); cx.notify(); }))
          .child(icon(allOpen ? "fold" : "unfold", 14, canFold ? t.muted : t.border))))
      .when(Boolean(strip), (el) => el.child(strip));
  }

  // Empties the filter and drops any lookup still running.
  clearFilter(cx) {
    this.filter.set_value("");
    this.raw = ""; this.query = ""; this.lookToken += 1; this.hit = null; this.layers = null; this.looking = false;
    cx.notify();
  }

  // The status bar: errors first, then work in progress, then the newest notice.
  status() {
    if (this.error) return ["error", this.error];
    if (this.file && this.loading && !this.engine?.missing) {
      return ["working", `Reading ${base(this.file.path)}${this.file.on ? ` on ${this.file.on}` : ""}…`];
    }
    if (this.drawing && this.batchTotal) return ["working", `Drawing thumbnails · ${this.batchDone} of ${this.batchTotal}`];
    const s = shared(NOTICE), n = !s ? this.notice : !this.notice || s.at > this.notice.at ? s : this.notice;
    if (!n || (fading(n) && Date.now() - n.at > NOTICE_MS)) return null;
    this.showing = fading(n) ? n.at : null;
    return [n.kind, n.text];
  }

  // Keys on the panel: / filters, ↑ ↓ move between screens, Enter builds, Space looks closer,
  // Esc clears the filter, then the selection.
  key(e, cx) {
    const at = this.visible.findIndex((v) => v.target.id === this.selected);
    const cur = this.visible[at];
    if (e.key === "/" || e.key_char === "/") this.filter.focus();
    else if (e.key === "down" || e.key === "up") {
      const next = this.visible[Math.max(0, Math.min(this.visible.length - 1, at < 0 ? 0 : at + (e.key === "down" ? 1 : -1)))];
      if (!next) return;
      this.select(next, cx);
    } else if (e.key === "enter" && cur) {
      const agent = this.receiver();
      if (agent) confirmIntent("build", agent, this.textFor("build", agent, cur.target, cur.page), cur.target.name, this.design());
    } else if (e.key === "space" && cur) this.quickLook(cur.target, cur.page);
    else if (e.key === "escape") {
      if (this.raw) this.clearFilter(cx);
      else if (this.selected) this.selected = null;
      else return;
    } else return;
    cx.stop_propagation();
    cx.notify();
  }

  render(cx) {
    if (this.role === "toggle") return toggle();
    if (this.role === "confirm") return this.renderConfirm();
    if (this.role === "newtab") return this.renderNewTab();
    if (this.role === "menu") return this.renderMenu();
    if (this.role === "clear") return this.renderClear();
    if (this.role === "quicklook") return this.renderQuickLook(cx);
    const t = theme(), file = sharedFile(), cleared = shared(CLEARED) ?? null;
    if (cleared !== this.cleared) { this.cleared = cleared; this.cacheBytes = null; this.renders = null; if (this.file) this.file = undefined; }
    if (this.file === undefined || !same(file, this.file)) this.load(file, cx);
    this.receiver(); // remembers the agent focused on the left while it is
    this.reading = this.readers(t); // once per frame: rows, pages, the strip and the preview read it
    const status = this.status();
    const view = this.listReady() && !nodeRef(this.raw) ? this.listing() : null;
    return v_flex().id("figlens").track_focus(this.handle).size_full().min_h(0).rounded(10).overflow_hidden().bg(t.background)
      .border_1().border_color(t.border).text_color(t.foreground).text_size(S().body)
      .on_mouse_down("left", () => this.handle.focus())
      .on_key_down((e, cx) => this.key(e, cx))
      .on_size_change((size, cx) => { if (Math.abs(size.width - this.width) > 1) { this.width = size.width; cx.notify(); } })
      .on_drop((e, cx) => {
        const fig = e.paths.find((p) => p.toLowerCase().endsWith(".fig"));
        if (fig) this.open({ path: fig, on: null });
        else { this.say("error", "Only .fig files can be opened"); cx.notify(); }
      })
      .child(h_flex().h(42).flex_none().pl(14).pr(8).gap(8).items_center().border_b(1).border_color(t.border)
        .child(image("mods/figlens/mark.svg").w(18).h(18).flex_none())
        .child(div().flex_none().font_semibold().text_size(S().title).child("figlens"))
        .child(div().flex_1().min_w(0).truncate().text_right().text_size(S().secondary).text_color(t.muted).child(this.file ? base(this.file.path) : ""))
        .when(Boolean(this.file?.on), (el) => el.child(pill(this.file.on, t)))
        .when(Boolean(this.file), (el) => el.child(h_flex().id("close-file").w(24).h(24).flex_none().items_center().justify_center()
          .rounded(6).cursor_pointer().hover((s) => s.bg(t.hover)).on_mouse_down("left", stop).on_click(() => this.open(null))
          .child(icon("close", 14, t.muted)))))
      .when(this.listReady(), (el) => el.child(this.toolbar(t, view)))
      .child(v_flex().id("figlens-body").flex_1().min_h(0).overflow_y_scrollbar().child(this.body(t, cx, view)))
      .when(!this.file, (el) => el.child(this.cacheRow(t, cx)))
      .when(Boolean(status), (el) => el.child(statusRow(t, status[0], status[1])));
  }
}
