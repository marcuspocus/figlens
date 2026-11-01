const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { cpSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');
const test = require('node:test');

// Lays out a fixture tree whose fake cargo writes each requested --bin as its own name; with
// TOUCH_CORE set, the companion build also rewrites the core, as a feature leaking into it would.
function fixtureTree(t) {
  const root = path.resolve(__dirname, '..');
  const fixture = mkdtempSync(path.join(tmpdir(), 'figlens-dist-pack-'));
  t.after(() => rmSync(fixture, { recursive: true, force: true }));
  mkdirSync(path.join(fixture, 'scripts'));
  mkdirSync(path.join(fixture, 'bin'));
  cpSync(path.join(root, 'scripts/build-dist.sh'), path.join(fixture, 'scripts/build-dist.sh'));
  cpSync(path.join(root, 'skills'), path.join(fixture, 'skills'), { recursive: true });
  cpSync(path.join(root, 'Cargo.toml'), path.join(fixture, 'Cargo.toml'));
  // Only package fixture bytes; this test never invokes a Rust toolchain.
  writeFileSync(path.join(fixture, 'bin/cargo'), `#!/bin/sh
set -eu
while [ $# -gt 0 ]; do
  case "$1" in --target) target="$2"; shift ;; --bin) bin="$2"; shift ;; esac
  shift
done
directory="target/dist-build/$target/release"
mkdir -p "$directory"
case "$target" in *windows*) exe=.exe ;; *) exe= ;; esac
printf "$bin" > "$directory/$bin$exe"
chmod +x "$directory/$bin$exe"
if [ -n "\${TOUCH_CORE:-}" ] && [ "$bin" = figlens-embed ]; then printf changed > "$directory/figlens$exe"; fi
`, { mode: 0o755 });
  const run = env => execFileSync('sh', ['scripts/build-dist.sh'], { cwd: fixture, stdio: 'pipe', env: { ...process.env, ...env, PATH: `${path.join(fixture, 'bin')}${path.delimiter}${process.env.PATH}`, FIGLENS_TARGETS: 'x86_64-unknown-linux-musl x86_64-pc-windows-gnu', FIGLENS_DIST_DIR: 'candidate' } });
  return { root, fixture, run };
}

test('native tar and Windows zip carry figlens, its model runtime and the skill references', { skip: process.platform === 'win32' }, t => {
  const { root, fixture, run } = fixtureTree(t);
  run({});
  const version = readFileSync(path.join(root, 'Cargo.toml'), 'utf8').match(/^version = "([^"]+)"/m)[1];
  const tar = path.join(fixture, `candidate/figlens-${version}-x86_64-unknown-linux-musl.tar.gz`);
  const zip = path.join(fixture, `candidate/figlens-${version}-x86_64-pc-windows-gnu.zip`);
  const verifyZip = `import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    assert z.read("figlens.exe") == b"figlens"
    assert z.read("figlens-embed.exe") == b"figlens-embed"
    for member in sys.argv[2:]:
        assert z.read(member) == open(member, "rb").read()
`;
  const resources = ['skills/figlens/SKILL.md', 'skills/figlens/references/audit.md', 'skills/figlens/references/planning.md', 'skills/figlens/references/planning-details.md', 'skills/figlens/references/yolo.md', 'skills/figlens/references/search.md'];
  for (const file of resources) {
    assert.equal(execFileSync('tar', ['-xOf', tar, file], { encoding: 'utf8' }), readFileSync(path.join(root, file), 'utf8'));
  }
  execFileSync('python3', ['-c', verifyZip, zip, ...resources], { cwd: root });
  assert.equal(execFileSync('tar', ['-xOf', tar, 'figlens'], { encoding: 'utf8' }), 'figlens');
  assert.equal(execFileSync('tar', ['-xOf', tar, 'figlens-embed'], { encoding: 'utf8' }), 'figlens-embed');
  assert.match(execFileSync('tar', ['-tvzf', tar], { encoding: 'utf8' }), /^-rwx.* figlens-embed$/m);
});

test('a model runtime build that changes the model-free core fails the release build', { skip: process.platform === 'win32' }, t => {
  const { run } = fixtureTree(t);
  assert.throws(() => run({ TOUCH_CORE: '1' }), e => /building figlens-embed changed figlens for x86_64-unknown-linux-musl/.test(e.stderr));
});

// install.sh against a fake curl serving local release assets: a release with the model runtime
// installs both executables; an older one without it still installs figlens. Neither touches
// other files in the bin directory, such as a model or a design kept there.
function install(t, members) {
  const root = mkdtempSync(path.join(tmpdir(), 'figlens-install-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const os = { Darwin: 'apple-darwin', Linux: 'unknown-linux-musl' }[execFileSync('uname', ['-s'], { encoding: 'utf8' }).trim()];
  const arch = /arm64|aarch64/.test(execFileSync('uname', ['-m'], { encoding: 'utf8' })) ? 'aarch64' : 'x86_64';
  const archive = `figlens-9.9.9-${arch}-${os}.tar.gz`;
  const stage = path.join(root, 'stage');
  const feed = path.join(root, 'feed');
  mkdirSync(stage);
  mkdirSync(feed);
  for (const name of members) writeFileSync(path.join(stage, name), `#!/bin/sh\necho "${name} new"\n`, { mode: 0o755 });
  execFileSync('tar', ['-czf', path.join(feed, archive), '-C', stage, ...members]);
  const sum = execFileSync('shasum', ['-a', '256', archive], { cwd: feed, encoding: 'utf8' });
  writeFileSync(path.join(feed, 'SHA256SUMS'), sum);
  const fake = path.join(root, 'fake');
  mkdirSync(fake);
  writeFileSync(path.join(fake, 'curl'), `#!/bin/sh
out=
while [ $# -gt 1 ]; do [ "$1" = -o ] && out="$2"; shift; done
case "$1" in
  */releases/latest) echo '{"tag_name": "v9.9.9"}' ;;
  *) cp "${feed}/$(basename "$1")" "$out" ;;
esac
`, { mode: 0o755 });
  const bin = path.join(root, 'bin');
  mkdirSync(path.join(bin, 'model'), { recursive: true });
  writeFileSync(path.join(bin, 'figlens'), 'old', { mode: 0o755 });
  writeFileSync(path.join(bin, 'model/config.json'), 'model');
  writeFileSync(path.join(bin, 'design.fig'), 'design');
  execFileSync('sh', [path.resolve(__dirname, 'install.sh')], { stdio: 'pipe', env: { ...process.env, FIGLENS_BIN_DIR: bin, PATH: `${fake}${path.delimiter}${process.env.PATH}` } });
  return bin;
}

test('install.sh puts figlens-embed beside figlens and keeps other files', { skip: process.platform === 'win32' }, t => {
  const bin = install(t, ['figlens', 'figlens-embed']);
  for (const name of ['figlens', 'figlens-embed']) {
    assert.equal(execFileSync(path.join(bin, name), { encoding: 'utf8' }), `${name} new\n`);
  }
  assert.deepEqual([readFileSync(path.join(bin, 'model/config.json'), 'utf8'), readFileSync(path.join(bin, 'design.fig'), 'utf8')], ['model', 'design']);
  assert.deepEqual(readdirSync(bin).sort(), ['design.fig', 'figlens', 'figlens-embed', 'model']);
});

test('install.sh still installs a release that ships no figlens-embed', { skip: process.platform === 'win32' }, t => {
  const bin = install(t, ['figlens']);
  assert.equal(execFileSync(path.join(bin, 'figlens'), { encoding: 'utf8' }), 'figlens new\n');
  assert.deepEqual(readdirSync(bin).sort(), ['design.fig', 'figlens', 'model']);
});
