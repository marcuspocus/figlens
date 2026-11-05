// The figlens activity journal: what agents in Herdr terminals did with a design, as figlens
// writes it (src/activity.rs: one JSON object per line) and as the panel shows it. The format,
// the read on each machine and what the panel derives from it live here. No Herdr imports.

import { clean } from "./frames.js";

// From a design's activity journal on this machine (its rotated half first): each terminal's
// newest line, each screen's newest line and newest comparison, and the last 40 lines, so a busy
// agent hides neither another agent nor a screen's history. figlens names the journal by the
// SHA-256 of the design's canonical path, under an absolute XDG_STATE_HOME or ~/.local/state. A
// path not starting with / is under $HOME (a design copy).
export const ACTIVITY = `case "$1" in /*) p="$1" ;; *) p="$HOME/$1" ;; esac
[ -e "$p" ] || exit 0
p=$(realpath "$p" 2>/dev/null) || p="$(cd "\${p%/*}" && pwd -P)/\${p##*/}"
case "$XDG_STATE_HOME" in /*) s="$XDG_STATE_HOME" ;; *) s="$HOME/.local/state" ;; esac
k=$(printf %s "$p" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-16)
for f in "$s/figlens/activity/$k.1.jsonl" "$s/figlens/activity/$k.jsonl"; do [ -f "$f" ] && cat "$f"; done |
  awk '{ ring[NR % 40] = $0; n = NR }
    match($0, /"terminal":"[^"]*"/) { keep["t" substr($0, RSTART, RLENGTH)] = $0 }
    match($0, /"screen":"[^"]*"/) { s = substr($0, RSTART, RLENGTH); keep["s" s] = $0; if ($0 ~ /"tool":"compare_image"/) keep["c" s] = $0 }
    END { for (k in keep) print keep[k]; for (i = n - 39; i <= n; i++) if (i > 0) print ring[i % 40] }'
exit 0`;
export const ACTIVE_MS = 10 * 60 * 1000;

// Every journal line (deduplicated), read on machine `on` (null: this computer); a terminal there
// is addressed as `<machine>/<id>` from here. Design strings never appear in a line; ids and
// paths are bounded anyway.
export function parseActivity(text, on) {
  const out = [], seen = new Set();
  for (const line of String(text ?? "").split("\n")) {
    let e;
    try { e = JSON.parse(line); } catch { continue; }
    if (e?.v !== 1 || typeof e.terminal !== "string" || !Number.isFinite(e.at)) continue;
    const address = on && !e.terminal.includes("/") ? `${on}/${e.terminal}` : e.terminal;
    const id = `${address} ${e.at} ${e.tool}`;
    if (seen.has(id)) continue;
    seen.add(id);
    const box = Array.isArray(e.box) && e.box.length === 4 && e.box.every(Number.isFinite) ? e.box : null;
    const r = e.result && typeof e.result === "object" ? e.result : null;
    out.push({
      address, at: e.at, tool: clean(e.tool, 40), box, on: on ?? null,
      revision: typeof e.revision === "string" ? clean(e.revision, 40) : null,
      nodes: Array.isArray(e.nodes) ? e.nodes.map((n) => clean(n, 80)) : [],
      screen: typeof e.screen === "string" ? clean(e.screen, 80) : null,
      result: r && {
        mismatch: Number.isFinite(r.mismatch) ? r.mismatch : null, nonTextMismatch: Number.isFinite(r.nonTextMismatch) ? r.nonTextMismatch : null,
        regionCount: Number.isFinite(r.regionCount) ? r.regionCount : null,
        screenshot: typeof r.screenshot === "string" ? clean(r.screenshot, 400) : null, diff: typeof r.diff === "string" ? clean(r.diff, 400) : null,
      },
    });
  }
  return out;
}

// The newest parsed line per terminal from the last `maxAge`.
export function latestActivity(lines, now, maxAge = ACTIVE_MS) {
  const out = new Map();
  for (const e of lines) {
    if (!(now - e.at <= maxAge) || (out.get(e.address)?.at ?? -Infinity) > e.at) continue;
    out.set(e.address, e);
  }
  return out;
}

// The newest lines of every agent, newest first: the panel's "Activity on this file".
export const recentActivity = (entries, limit = 8) =>
  [...entries].sort((a, b) => b.at - a.at).slice(0, limit);

// Each screen's place on the board from the journal: `building` while an agent used a building
// tool on it in the last `activeMs` after its last comparison (an inspect alone is a read), else
// `verified` once compare_image ran on it (with that result), else `read`; `changed` leads when a
// newer saved design changed what an agent worked on.
export const BUILDING_MS = 30 * 60 * 1000;
// Tools that take a screen's material to build it; an inspect alone (a look, or Point's "what is
// this") is a read, not building.
const BUILDING_TOOLS = new Set(["bundle", "export", "export_batch", "nodes", "variables", "compare_image"]);
// With `current` (the design's revision, as `stat` prints it) and `changes` (revision → the
// screens `diff` found changed from it to now), a screen whose newest line read an older revision
// that changed it is `changed`, ahead of every other state.
export function screenStates(entries, now, { activeMs = BUILDING_MS, current = null, changes = new Map() } = {}) {
  const last = new Map();
  for (const e of entries) {
    if (!e.screen) continue;
    const s = last.get(e.screen) ?? { read: null, compare: null, built: null };
    const slot = e.tool === "compare_image" ? "compare" : "read";
    if ((s[slot]?.at ?? -Infinity) < e.at) s[slot] = e;
    if (BUILDING_TOOLS.has(e.tool) && (s.built?.at ?? -Infinity) < e.at) s.built = e;
    last.set(e.screen, s);
  }
  const out = new Map();
  for (const [screen, { read, compare, built }] of last) {
    const newest = !compare || (read && read.at > compare.at) ? read : compare;
    const change = current && newest?.revision && newest.revision !== current && changes.get(newest.revision)?.get(screen);
    if (change) { out.set(screen, { state: "changed", entry: newest, change }); continue; }
    if (read && built && now - built.at <= activeMs && (!compare || built.at > compare.at)) out.set(screen, { state: "building", entry: built, since: built.at });
    else if (compare) out.set(screen, { state: "verified", entry: compare });
    else out.set(screen, { state: "read", entry: read });
  }
  return out;
}

// Every line from a design's journal on each machine: `sh(script, args, on)` is the host's shell,
// `design` the design's path on its own machine (`from`), `copy` its copy's path (under $HOME) on
// the others. A machine that cannot be read adds nothing.
export async function readJournals(sh, design, copy, from, machines) {
  const list = [...machines];
  const reads = await Promise.all(list.map((m) => sh(ACTIVITY, [m === from ? design : copy], m).catch(() => null)));
  return list.flatMap((m, i) => parseActivity(reads[i]?.stdout, m));
}

// What the panel shows of a design's journal, read again on every poll: who is active now, the
// newest lines, and each screen's state. `update` says what moved, so a poll that found nothing
// new redraws nothing and writes nothing.
export class Journal {
  constructor() {
    this.activity = new Map(); // address → newest line from the last ACTIVE_MS
    this.recent = []; // newest lines first
    this.states = new Map(); // screen → screenStates entry
    this.sigs = { live: null, recent: null, states: null };
  }

  // A comparison finished: states are worked out again on the next update.
  restate() { this.sigs.states = null; }

  // `entries`: every line (readJournals); `current`: the design's revision; `changes`: revision
  // → screens diff found changed since. Returns { share, redraw }: share when the active agents
  // changed (or, while any are, every 10 s, so "12s ago" moves in Quick Look); redraw when the
  // panel shows something new.
  update(entries, now, { current = null, changes = new Map() } = {}) {
    const activity = latestActivity(entries, now, ACTIVE_MS);
    const live = `${[...activity.values()].map((e) => `${e.address}${e.at}`).join()}|${activity.size ? Math.floor(now / 10000) : 0}`;
    const share = live !== this.sigs.live;
    this.sigs.live = live;
    const recent = recentActivity(entries), recentSig = recent.map((e) => `${e.address}${e.at}`).join();
    // While anyone is active the panel redraws every poll: its "12s ago" moves.
    let redraw = Boolean(activity.size || this.activity.size || recentSig !== this.sigs.recent);
    this.activity = activity; this.recent = recent; this.sigs.recent = recentSig;
    const states = screenStates(entries, now, { current, changes });
    const statesSig = [...states].map(([s, v]) => `${s}${v.state}${v.entry?.at}`).join();
    if (statesSig !== this.sigs.states) { this.states = states; this.sigs.states = statesSig; redraw = true; }
    return { share, redraw };
  }
}
