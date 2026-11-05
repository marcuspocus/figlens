// Where a design lives: this computer, or another machine Herdr reaches (`file.on`). Callers ask
// for the design's revision, wait for it to change, run a script there or draw nodes, and do not
// branch on the machine themselves. No Herdr imports: the host's calls come in `host`.
//
// host: { sh(script, args, on), exec(cmd, args, { on }), watchFolder(folder), writeFile(path, text),
//         readdir(dir), unpack(base64TarGz, dir) }

// GNU stat first: there, -f means file-system status and would print garbage. Then macOS's.
export const STAT = 'stat -L -c "%Y %s" "$1" 2>/dev/null || stat -L -f "%m %z" "$1"';
// sh -c REMOTE_EXPORT <name> <bin> <file> <max> <scale> <id>...: images rendered on the design's
// machine, sent back as one base64 tar.gz; the design itself never moves.
export const REMOTE_EXPORT = `set -e
unset HC_TERMINAL_ID
bin=$1; file=$2; max=$3; scale=$4; shift 4
d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
mkdir "$d/out"; printf '%s\\n' "$@" > "$d/list"
"$bin" export "$file" --frames "$d/list" --out-dir "$d/out" --format png --bounds layout --scale "$scale" --max-size "$max" >/dev/null 2>&1 || true
tar -czf - -C "$d/out" . | base64`;
// How often a design on another machine is looked at for a new save (it cannot be watched).
export const POLL = 10000;
// Nodes per figlens run: one run decodes the design once for all of them. A remote run also pays
// a round trip, so it takes more.
export const CHUNK = 12, REMOTE_CHUNK = 40;

// The panel's own figlens calls: never journaled as an agent's, whatever environment exec has.
export const engineArgs = (bin, args) => ["-u", "HC_TERMINAL_ID", bin, ...args];

// `export --frames --out-dir` names each file by its node id with `:` as `-`.
const fileOf = (id) => `${id.replaceAll(":", "-")}.png`;
let lists = 0;

export function placeOf(file, host) {
  const on = file.on ?? null, folder = file.path.slice(0, file.path.lastIndexOf("/")) || "/";
  let polling = Boolean(on);
  return {
    on,
    chunk: on ? REMOTE_CHUNK : CHUNK,

    // A script on the design's machine.
    run: (script, args = []) => host.sh(script, args, on),

    // "<seconds> <bytes>" of the saved design; null when it is gone. Rejects when the machine
    // cannot be asked.
    async stat() {
      const r = await host.sh(STAT, [file.path], on);
      return r.code === 0 ? r.stdout.trim() : null;
    },

    // Waits until the design may have been saved again: a watch on its folder here, a poll there
    // (and here too once a folder is too large to watch). True when a batch named the design or
    // the folder itself (Herdr's "refresh everything"); other files in the folder do not count.
    // `sleep(ms)` is the caller's (a Herdr cx lives only as long as its callback).
    async changes(sleep) {
      if (!polling) {
        try {
          const paths = await host.watchFolder(folder);
          return paths.some((p) => p === file.path || p === folder);
        } catch { polling = true; }
      }
      await sleep(POLL);
      return true;
    },

    // Draws nodes as PNGs named by id into `dir` (which exists), where the design lives. Resolves
    // with the ids drawn; a node that could not be drawn is left out.
    async render(bin, ids, dir, { max, scale }) {
      if (on) {
        const r = await host.sh(REMOTE_EXPORT, [bin, file.path, String(max), String(scale), ...ids], on);
        if (r.code === 0 && r.stdout.trim()) await host.unpack(r.stdout, dir);
      } else {
        lists += 1;
        const list = `${dir}/.frames-${lists}`;
        await host.writeFile(list, ids.join("\n"));
        try {
          await host.exec("env", engineArgs(bin, ["export", file.path, "--frames", list, "--out-dir", dir,
            "--format", "png", "--bounds", "layout", "--scale", String(scale), "--max-size", String(max)]), { on: null });
        } finally {
          await host.exec("rm", ["-f", list], { on: null }).catch(() => null);
        }
      }
      const have = new Set(await host.readdir(dir));
      return new Set(ids.filter((id) => have.has(fileOf(id))));
    },
  };
}
