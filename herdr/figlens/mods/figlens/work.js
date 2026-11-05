// Every figlens run the panel starts goes through one queue. A figlens call decodes the whole
// design, and a large design costs seconds and gigabytes per call, so the queue keeps a bound on
// how many run at once and runs what the person is looking at first. No Herdr imports.
//
// Herdr's exec cannot stop a process once it started, so a run that went stale still finishes;
// a queued run whose `stale()` is true by its turn never starts and rejects with Stale.

// What the person is waiting on (the screen list, a preview, Quick Look, a lookup), then the
// list's thumbnails, then background comparisons.
export const NOW = 0, SOON = 1, LATER = 2;

export class Stale extends Error {
  constructor() { super("stale"); this.name = "Stale"; }
}

// `limit` runs at once, of which at most `background` are SOON or LATER: background work never
// takes every slot, so a click still starts at once.
export function createQueue({ limit = 2, background = 1 } = {}) {
  const waiting = [];
  let seq = 0, running = 0, behind = 0;

  const next = () => {
    let best = -1;
    for (let i = 0; i < waiting.length; i += 1) {
      const j = waiting[i];
      if (j.priority !== NOW && behind >= background) continue;
      if (best < 0 || j.priority < waiting[best].priority || (j.priority === waiting[best].priority && j.seq < waiting[best].seq)) best = i;
    }
    return best < 0 ? null : waiting.splice(best, 1)[0];
  };

  const pump = () => {
    while (running < limit) {
      const job = next();
      if (!job) return;
      if (job.stale?.()) { job.reject(new Stale()); continue; }
      const bg = job.priority !== NOW;
      running += 1; if (bg) behind += 1;
      Promise.resolve().then(job.run).then(job.resolve, job.reject).finally(() => {
        running -= 1; if (bg) behind -= 1;
        pump();
      });
    }
  };

  return {
    // Runs `run()` (returning a promise) in its turn; resolves or rejects with its result.
    run(run, { priority = NOW, stale = null } = {}) {
      return new Promise((resolve, reject) => {
        waiting.push({ run, priority, stale, resolve, reject, seq: seq++ });
        pump();
      });
    },
    get running() { return running; },
    get waiting() { return waiting.length; },
  };
}
