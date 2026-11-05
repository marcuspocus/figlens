// What the panel keeps on disk, where it goes and how long it stays: drawn images under Herdr's
// app home, saved versions of designs (for "changed since built") and copies made for agents on
// other machines under $HOME there. Callers never build one of these paths. No Herdr imports.
//
// host: { sh(script, args, on), exec(cmd, args, { on }), mkdir(dir, { recursive }), readdir(dir),
//         appHome(), home(), hash(text) }

import { thumbFile } from "./frames.js";

// Under Herdr's app home; hidden, so writing images does not reload the mod.
const IMAGES = "mods/figlens/.cache";
// Under $HOME on each machine.
export const COPIES = ".cache/figlens/herdr-designs";
const REVISIONS = ".cache/figlens/herdr-revisions";
// Image folders kept, one per revision of a design, across all designs.
export const KEEP_IMAGES = 12;
const PRUNE = 'cd "$1" && ls -1t | tail -n +$2 | while IFS= read -r d; do rm -rf "./$d"; done';
// sh -c KEEP_REVISION <design> <folder key> <revision key> <revision>: keeps this saved version of
// the design beside the others agents read (a copy-on-write clone where the file system has one),
// so a later save can be compared with what an agent built from; the newest 8 per design stay. A
// save during the copy leaves none rather than other bytes under this revision's name.
const KEEP_REVISION = `d="$HOME/${REVISIONS}/$2"; f="$d/$3.fig"; mkdir -p "$d"
now() { stat -L -c "%Y %s" "$1" 2>/dev/null || stat -L -f "%m %z" "$1"; }
[ -e "$f" ] && exit 0
[ "$(now "$1")" = "$4" ] || exit 0
cp -c "$1" "$f" 2>/dev/null || cp --reflink=auto "$1" "$f" 2>/dev/null || cp "$1" "$f" || { rm -f "$f"; exit 0; }
[ "$(now "$1")" = "$4" ] || rm -f "$f"
cd "$d" && ls -1t | tail -n +9 | while IFS= read -r x; do rm -f "./$x"; done`;
const KEPT = `f="$HOME/${REVISIONS}/$1/$2.fig"; [ -e "$f" ] && printf %s "$f"`;
const DU = 'du -sk "$1" 2>/dev/null | cut -f1';

const base = (p) => p.split("/").pop();
const lastLine = (s) => String(s ?? "").trim().split("\n").pop();

export function createCache(host) {
  const root = () => `${host.appHome()}/${IMAGES}`;
  // A kept revision's folder and file names: the design's key, the revision with "-" for spaces.
  const revisionKeys = (file, rev) => [host.hash(`${file.on ?? ""}:${file.path}`), rev.replace(/\s+/g, "-")];
  const kb = (r) => (Number(r?.stdout?.trim()) || 0) * 1024;

  return {
    // ---- drawn images: one folder per revision of a design (`key`) ----
    // `kind`: "" for thumbnails, "preview", "full" (Quick Look), "look" (compare), "snap".
    folder: (key, kind = "") => [root(), key, kind].filter(Boolean).join("/"),
    // A drawn node, relative to Herdr's app home, as image() takes it.
    image: (key, kind, id) => [IMAGES, key, kind, thumbFile(id)].filter(Boolean).join("/"),
    // A compare window's build image: { rel (for image()), abs (to write) }.
    compareImage(id, name) {
      const rel = `${IMAGES}/compare/${id}-${name}.png`;
      return { rel, abs: `${host.appHome()}/${rel}`, folder: `${root()}/compare` };
    },

    // Opens revision `key` of the design whose keys start with `prefix`: its older revisions'
    // images go, and only the newest KEEP_IMAGES folders of all designs stay. Resolves with the
    // names of the images already drawn for it.
    async openRevision(prefix, key) {
      const dir = `${root()}/${key}`;
      await host.mkdir(dir, { recursive: true });
      await host.exec("touch", [dir]);
      const old = (await host.readdir(root())).filter((d) => d.startsWith(`${prefix}-`) && d !== key);
      if (old.length) await host.exec("rm", ["-rf", ...old.map((d) => `${root()}/${d}`)]);
      await host.sh(PRUNE, [root(), String(KEEP_IMAGES + 1)]);
      return new Set(await host.readdir(dir));
    },

    // ---- saved versions, on the design's machine ----
    // Keeps the design's revision `stamp`. Best effort: the panel does not wait on it.
    keep(file, stamp) {
      return host.sh(KEEP_REVISION, [file.path, ...revisionKeys(file, stamp), stamp], file.on).catch(() => null);
    },
    // The path of the kept copy of revision `rev` there, or "" when none was kept.
    async kept(file, rev) {
      const r = await host.sh(KEPT, revisionKeys(file, rev), file.on);
      return r.stdout.trim();
    },

    // ---- copies of a design made for agents on other machines ----
    // Under $HOME on any machine: one folder per saved version.
    copyPath: (design) => `${COPIES}/${host.hash(`${design.on ?? ""}:${design.path}:${design.stamp}`)}/${base(design.path)}`,

    // ---- measuring and clearing ----
    // { panelBytes, renders: [{ on, bin, bytes }], copies: [{ on, dir, bytes, saved }] } on each
    // machine; `engineOn(on)` finds figlens there (null: none).
    async measure(machines, engineOn) {
      const panelBytes = kb(await host.sh(DU, [root()]).catch(() => null)), renders = [], copies = [];
      for (const on of machines) {
        const engine = await engineOn(on).catch(() => null);
        if (!engine?.bin) continue;
        const out = await host.exec(engine.bin, ["cache"], { on }).catch(() => null);
        try { renders.push({ on, bin: engine.bin, bytes: JSON.parse(out.stdout).bytes || 0 }); } catch { /* no render cache there */ }
      }
      for (const on of machines) {
        const home = on ? (await host.sh('printf %s "$HOME"', [], on).catch(() => ({ stdout: "" }))).stdout.trim() : host.home();
        for (const [dir, saved] of [[`${home}/${COPIES}`, false], [`${home}/${REVISIONS}`, true]]) {
          const bytes = kb(await host.sh(DU, [dir], on).catch(() => null));
          if (bytes) copies.push({ on, dir, bytes, saved });
        }
      }
      return { panelBytes, renders, copies };
    },

    // Empties what measure found: this panel's images, figlens's render cache and the copies.
    // Resolves { freed, failed }: bytes freed, and the first thing that could not be cleared.
    async clear({ renders = [], panelBytes = 0, copies = [] }) {
      const r = await host.exec("rm", ["-rf", root()]).catch((e) => ({ code: 1, stderr: String(e.message ?? e) }));
      await host.mkdir(root(), { recursive: true }).catch(() => null);
      let freed = r.code === 0 ? panelBytes : 0, failed = r.code === 0 ? null : lastLine(r.stderr) || "Could not clear the panel's images";
      for (const x of renders) {
        const out = await host.exec(x.bin, ["cache", "clear"], { on: x.on }).catch(() => null);
        try { freed += JSON.parse(out.stdout).freed || 0; } catch { failed ??= `Could not clear figlens renders${x.on ? ` on ${x.on}` : ""}`; }
      }
      for (const x of copies) {
        const out = await host.sh('rm -rf "$1"', [x.dir], x.on).catch(() => null);
        if (out?.code === 0) freed += x.bytes; else failed ??= `Could not remove design copies${x.on ? ` on ${x.on}` : ""}`;
      }
      return { freed, failed };
    },
  };
}
