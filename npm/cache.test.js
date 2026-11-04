const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

test('concurrent first use keeps the verified binary while an MCP process is using it', { timeout: 5000 }, async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'figlens-cache-test-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const version = '0.3.0';
  const directory = path.join(root, 'figlens', 'npm', version);
  const binary = path.join(directory, 'figlens.exe');
  const archive = Buffer.from('verified test archive');
  const checksum = crypto.createHash('sha256').update(archive).digest('hex');
  const code = fs.readFileSync(path.join(__dirname, 'bin/figlens.js'), 'utf8');
  let running = false;
  let release;
  const gate = new Promise(resolve => { release = resolve; });
  function launch(wait) {
    const exits = [];
    const errors = [];
    const localFs = {
      ...fs,
      rmSync(file, options) {
        if (file === directory && running) throw Object.assign(new Error('binary is in use'), { code: 'EPERM' });
        return fs.rmSync(file, options);
      },
    };
    const spawnSync = (command, _args, options) => {
      if (command === 'tar' || command.endsWith('\\tar.exe')) for (const member of _args.slice(2)) fs.writeFileSync(path.join(options.cwd, member), 'verified binary');
      else {
        assert.equal(command, binary);
        assert.equal(fs.readFileSync(binary, 'utf8'), 'verified binary');
        // `figlens pull` finds its browser helper through the launcher.
        assert.match(options.env.FIGLENS_PULL_HELPER, /pull[\\/]helper\.mjs$/);
        running = true;
      }
      return { status: 0 };
    };
    const completion = vm.runInNewContext(code, {
      __dirname: path.join(__dirname, 'bin'),
      Buffer,
      require(name) {
        if (name === 'node:fs') return localFs;
        if (name === 'node:child_process') return { spawnSync };
        if (name === '../target.js') return { asset: () => ({ archive: 'test.zip', binary: 'figlens.exe', companion: 'figlens-embed.exe' }), cachedBinary: (cache, platform, arch, version) => path.join(cache, 'npm', version, 'figlens.exe') };
        if (name === '../package.json') return { version };
        return require(name);
      },
      process: { env: { XDG_CACHE_HOME: root }, platform: 'win32', arch: 'x64', argv: ['node', 'figlens'], exit: code => exits.push(code) },
      console: { error: message => errors.push(message) },
      fetch: async url => {
        if (wait) await gate;
        return { ok: true, arrayBuffer: async () => url.endsWith('SHA256SUMS') ? Buffer.from(`${checksum}  test.zip\n`) : archive };
      },
    });
    return { completion, exits, errors };
  }
  const slower = launch(true); // Both callers observe an empty cache before the faster one installs.
  const faster = launch(false);
  await faster.completion;
  assert.deepEqual(faster.exits, [0]);
  release();
  await slower.completion;
  assert.deepEqual(slower.errors, []);
  assert.deepEqual(slower.exits, [0]);
  assert.equal(fs.readFileSync(binary, 'utf8'), 'verified binary');
  assert.deepEqual(fs.readdirSync(path.dirname(directory)), [version]);
});

// Layer A: injected transport/extraction exercises installer branches, not a release download.
function installer(root, failure, env = {}) {
  const version = '0.3.1';
  const dir = path.join(root, 'figlens', 'npm', version);
  const binary = path.join(dir, 'figlens.exe');
  const bytes = Buffer.from('small controlled archive');
  const digest = crypto.createHash('sha256').update(bytes).digest('hex');
  const state = { exits: [], errors: [], downloads: 0, executions: 0 };
  state.done = vm.runInNewContext(fs.readFileSync(path.join(__dirname, 'bin/figlens.js'), 'utf8'), {
    __dirname: path.join(__dirname, 'bin'),
    Buffer,
    require(name) {
      if (name === '../package.json') return { version };
      if (name === '../target.js') return { asset: () => ({ archive: 'test.zip', binary: 'figlens.exe', companion: 'figlens-embed.exe' }), cachedBinary: (cache, platform, arch, version) => path.join(cache, 'npm', version, 'figlens.exe') };
      if (name === 'node:child_process') return { spawnSync(command, args, options) {
        if (command === 'tar' || command.endsWith('\\tar.exe')) {
          state.extractor = command;
          if (failure === 'missing-tar') return { status: null, error: Object.assign(new Error('tar missing'), { code: 'ENOENT' }) };
          // An extractor can fail after writing part of a binary; it writes every member it was asked for.
          for (const member of args.slice(2)) fs.writeFileSync(path.join(options.cwd, member), failure === 'extract' ? 'partial' : 'verified binary');
          return { status: failure === 'extract' ? 2 : 0 };
        }
        assert.equal(command, binary);
        assert.equal(fs.readFileSync(binary, 'utf8'), 'verified binary');
        state.executions++;
        return { status: 0 };
      } };
      return require(name);
    },
    process: { env: { XDG_CACHE_HOME: root, ...env }, platform: 'win32', arch: 'x64', argv: ['node', 'figlens'], exit: code => state.exits.push(code) },
    console: { error: message => state.errors.push(message) },
    fetch: async url => {
      state.downloads++;
      return { ok: failure !== 'http', status: 503,
        arrayBuffer: async () => url.endsWith('SHA256SUMS') ? Buffer.from(`${failure === 'checksum' ? '0'.repeat(64) : digest}  test.zip\n`) : bytes };
    },
  });
  return state;
}

test('Windows ZIP extraction uses system bsdtar instead of a tar from PATH', { timeout: 5000 }, async t => {
  for (const env of [{}, { SystemRoot: 'D:\\Test Windows' }, { SYSTEMROOT: 'D:\\Test Windows' }]) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'figlens-system-tar-'));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    const state = installer(root, undefined, { PATH: 'C:\\Program Files\\Git\\usr\\bin', ...env });
    await state.done;
    assert.deepEqual(state.exits, [0]);
    assert.equal(state.extractor, Object.keys(env).length ? 'D:\\Test Windows\\System32\\tar.exe' : 'C:\\Windows\\System32\\tar.exe');
  }
});

test('cold install then warm start reuses the valid binary without downloads', { timeout: 5000 }, async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'figlens-installer-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const first = installer(root);
  await first.done;
  assert.deepEqual(first.exits, [0]);
  assert.equal(first.downloads, 2);
  // The model runtime lands beside the binary, where figlens looks for it.
  assert.equal(fs.readFileSync(path.join(root, 'figlens/npm/0.3.1/figlens-embed.exe'), 'utf8'), 'verified binary');
  const again = installer(root, 'http');
  await again.done;
  assert.deepEqual(again.exits, [0]);
  assert.equal(again.downloads, 0);
  assert.equal(again.executions, 1);
});

test('a cache holding the binary but not the runtime gets the runtime and keeps its binary', { timeout: 5000 }, async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'figlens-installer-repair-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const dir = path.join(root, 'figlens/npm/0.3.1');
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'figlens.exe'), 'verified binary');
  const fixed = installer(root);
  await fixed.done;
  assert.deepEqual(fixed.exits, [0]);
  assert.equal(fixed.downloads, 2);
  assert.equal(fs.readFileSync(path.join(dir, 'figlens-embed.exe'), 'utf8'), 'verified binary');
  assert.deepEqual(fs.readdirSync(path.dirname(dir)), ['0.3.1']);
});

for (const failure of ['checksum', 'http', 'extract', 'missing-tar']) {
  test(`${failure} fails without publishing partial files or damaging a valid older cache`, { timeout: 5000 }, async t => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'figlens-installer-failure-'));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    const parent = path.join(root, 'figlens/npm');
    const prior = path.join(parent, '0.3.0/figlens.exe');
    fs.mkdirSync(path.dirname(prior), { recursive: true });
    fs.writeFileSync(prior, 'valid older binary');
    const failed = installer(root, failure);
    await failed.done;
    assert.deepEqual(failed.exits, [1]);
    assert.equal(failed.executions, 0);
    assert.match(failed.errors[0], failure === 'checksum' ? /SHA256SUMS/ : failure === 'http' ? /HTTP 503/ : /tar could not extract/);
    assert.equal(fs.readFileSync(prior, 'utf8'), 'valid older binary');
    assert.deepEqual(fs.readdirSync(parent), ['0.3.0']);
    const retry = installer(root);
    await retry.done;
    assert.deepEqual(retry.exits, [0]);
    assert.equal(fs.readFileSync(prior, 'utf8'), 'valid older binary');
    assert.deepEqual(fs.readdirSync(parent).sort(), ['0.3.0', '0.3.1']);
  });
}
