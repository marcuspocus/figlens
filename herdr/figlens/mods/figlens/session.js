// The design open in the panel, at the revision the panel read: the one owner of "is this work
// still current". No Herdr imports, so `node --test` can run it.
//
// Every read of the design (opening it, a new revision, a cleared cache) starts a new generation.
// Work started for a generation (the screen list, thumbnails, diffs, the watch and the activity
// loop) checks current(gen) before it starts more or keeps a result; nothing else decides that.

export const same = (a, b) => (a?.path ?? null) === (b?.path ?? null) && (a?.on ?? null) === (b?.on ?? null);

export class DesignSession {
  constructor() {
    this.file = undefined; // undefined: not read yet; null: no design open
    this.stamp = null; // "<seconds> <bytes>" of the revision read, once known
    this.gen = 0;
  }

  // Whether the panel has to read `file`: another design than the open one, or the open one
  // after stale().
  wants(file) {
    return this.file === undefined || !same(file, this.file);
  }

  // Read the open design again on the next render (its cached images were cleared).
  stale() {
    this.file = undefined;
  }

  // A new read of `file` (null: none): returns its generation; work for earlier ones is stale.
  read(file) {
    this.file = file ?? null;
    this.stamp = null;
    this.gen += 1;
    return this.gen;
  }

  current(gen) {
    return gen === this.gen;
  }

  // The revision this generation read.
  saw(gen, stamp) {
    if (this.current(gen)) this.stamp = stamp;
  }

  // After a watch event or a poll: true when the design was saved again (or is gone) since this
  // generation read it, so a new read is due. `stat(file)` resolves "<seconds> <bytes>", null
  // when the file is gone, and rejects when it cannot tell (the machine is unreachable): then
  // nothing changes.
  async changed(gen, stat) {
    if (!this.current(gen) || !this.file) return false;
    let now;
    try { now = await stat(this.file); } catch { return false; }
    return this.current(gen) && now !== this.stamp;
  }
}
