// Herdr shared state the panel, its dialogs and windows use to talk: each key's shape, who
// writes it and when old values go, in one place. Callers never name a key. No Herdr imports:
// `shared(key)` and `setShared(key, value)` are the host's.

import { ACTIVE_MS } from "./journal.js";

const FILES = "figlens.files.v1"; // { [space id]: { path, on } }: each space keeps its own design
const NOTICE = "figlens.notice.v1"; // { kind, text, at }: dialogs report to the panel
const QL = "figlens.quicklook.v1"; // { file, bin, cli, key, frames, index, design, watch }: Quick Look's list
const CMP = "figlens.compare.v1"; // the comparison the compare window shows
const HANDOFFS = "figlens.handoffs.v1"; // { "<machine>:<path>": { <screen id>: { intent, address | tab + space, at, started, finished } } }
const LIVE = "figlens.live.v1"; // { "<machine>:<path>": { at, entries } }: what agents read now, for Quick Look
const CLEARED = "figlens.cleared.v1"; // when the image cache was last cleared: every panel reloads
const VIEWERS = "figlens.viewers.v1"; // { "<machine>:<path>": { terminal, url } }: browser viewers
const PANELS = "workspace.panels"; // Herdr's: the side panels its tabs mod restores per space

// A handoff shows on the board for this long.
export const HANDOFF_DAYS = 7;

// A design's key in shared state, from a file or a design ({ on, path }).
export const designKey = (d) => `${d.on ?? ""}:${d.path}`;

export function createStore(shared, setShared) {
  const all = (key) => shared(key) ?? {};
  return {
    // ---- the design each space has open ----
    fileOf(space) {
      const v = all(FILES)[space];
      return v?.path ? { path: v.path, on: v.on ?? null } : null;
    },
    // Opens `file` in `space` (null closes it). Returns the design closed there when no other
    // space still has it open, so its viewer can stop; else null.
    setFile(space, file) {
      const files = { ...all(FILES) }, was = files[space] ?? null;
      if (file) files[space] = file; else delete files[space];
      setShared(FILES, files); // every copy re-renders and loads it
      const still = was && Object.values(files).some((f) => designKey(f) === designKey(was));
      return was && !file && !still ? was : null;
    },

    // ---- notices from dialogs to the panel ----
    say(kind, text, at = Date.now()) { setShared(NOTICE, { kind, text, at }); },
    notice() { return shared(NOTICE) ?? null; },
    hush() { if (shared(NOTICE)) setShared(NOTICE, null); },

    // ---- what the person asked agents to do, by screen ----
    handoffsOf(design) { return all(HANDOFFS)[designKey(design)] ?? {}; },
    // Records a handoff for each screen; handoffs older than HANDOFF_DAYS go on the way.
    recordHandoff(design, screens, handoff, at = Date.now()) {
      const fresh = (h) => at - (h?.at ?? 0) < HANDOFF_DAYS * 86400000;
      const kept = Object.fromEntries(Object.entries(all(HANDOFFS))
        .map(([k, v]) => [k, Object.fromEntries(Object.entries(v ?? {}).filter(([, h]) => fresh(h)))])
        .filter(([, v]) => Object.keys(v).length));
      const key = designKey(design), one = { ...handoff, at };
      kept[key] = { ...(kept[key] ?? {}), ...Object.fromEntries(screens.map((id) => [id, one])) };
      setShared(HANDOFFS, kept);
    },
    // Moves a design's handoffs on with `step(h)` (frames.js handoffStep); writes only on a change.
    // True when something moved.
    stepHandoffs(design, step) {
      const key = designKey(design), mine = all(HANDOFFS)[key];
      if (!mine) return false;
      const next = Object.fromEntries(Object.entries(mine).map(([k, h]) => [k, step(h)]));
      if (Object.keys(next).every((k) => next[k] === mine[k])) return false;
      setShared(HANDOFFS, { ...all(HANDOFFS), [key]: next });
      return true;
    },

    // ---- what agents read now, for Quick Look ----
    liveOf(design) { return all(LIVE)[designKey(design)]?.entries ?? []; },
    // One key per design: two panels on two designs never overwrite each other. Other designs'
    // entries older than ACTIVE_MS go.
    shareLive(design, entries, at = Date.now()) {
      const key = designKey(design);
      const kept = Object.fromEntries(Object.entries(all(LIVE)).filter(([k, v]) => k !== key && at - (v?.at ?? 0) < ACTIVE_MS));
      setShared(LIVE, { ...kept, [key]: { at, entries } });
    },

    // ---- browser viewers, one per design ----
    viewerOf(design) { return all(VIEWERS)[designKey(design)] ?? null; },
    keepViewer(design, viewer) { setShared(VIEWERS, { ...all(VIEWERS), [designKey(design)]: viewer }); },
    // Forgets a design's viewer and returns it (to stop it), or null.
    dropViewer(design) {
      const viewers = { ...all(VIEWERS) }, key = designKey(design), v = viewers[key] ?? null;
      if (v) { delete viewers[key]; setShared(VIEWERS, viewers); }
      return v;
    },

    // ---- the image cache was cleared: every panel reads its design again ----
    cleared() { return shared(CLEARED) ?? null; },
    markCleared(at = Date.now()) { setShared(CLEARED, at); },

    // ---- the Quick Look and compare windows' content ----
    quickLook() { return shared(QL) ?? null; },
    showQuickLook(q) { setShared(QL, q); },
    comparison() { return shared(CMP) ?? null; },
    showComparison(c) { setShared(CMP, c); },

    // Herdr's tabs mod restores only the side panels listed here when you switch spaces.
    listPanel(name) {
      const panels = shared(PANELS) ?? ["files", "changes"];
      if (!panels.includes(name)) setShared(PANELS, [...panels, name]);
    },
  };
}
