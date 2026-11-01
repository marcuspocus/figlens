import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('the existing CLI npm package includes the maintained Pi entry and its adjacent launcher', t => {
  const root = fileURLToPath(new URL('..', import.meta.url));
  const cache = mkdtempSync(path.join(tmpdir(), 'figlens-npm-pack-'));
  t.after(() => rmSync(cache, { recursive: true, force: true }));
  execFileSync(process.execPath, ['scripts/prepare-npm.cjs'], { cwd: root });
  const [packed] = JSON.parse(execFileSync(process.platform === 'win32' ? 'npm.cmd' : 'npm',
    ['pack', '--dry-run', '--ignore-scripts', '--json', '--cache', cache],
    { cwd: path.join(root, 'npm'), shell: process.platform === 'win32', encoding: 'utf8' }));
  const manifest = JSON.parse(readFileSync(path.join(root, 'npm/package.json'), 'utf8'));
  assert.equal(manifest.name, '@runkids/figlens');
  assert.ok(manifest.keywords.includes('pi-package'));
  assert.deepEqual(manifest.pi.extensions, ['./pi/index.ts']);
  assert.deepEqual(manifest.pi.skills, ['./skills']);
  assert.deepEqual(manifest.omp.extensions, ['./omp/index.ts']);
  assert.equal(manifest.type, undefined, 'The CLI root retains CommonJS semantics');
  assert.deepEqual(manifest.exports, { '.': './opencode/index.mjs', './tui': './opencode/tui.mjs', './rpc': './opencode/rpc.mjs', './package.json': './package.json' });
  assert.ok(manifest.pi.image.endsWith('/assets/taskflow-demo-poster.jpg'));
  assert.ok(manifest.pi.video.endsWith('/assets/taskflow-demo.mp4'));
  const files = packed.files.map(file => file.path);
  assert.ok(files.includes('bin/figlens.js'));
  for (const file of ['SKILL.md', 'references/audit.md', 'references/planning.md', 'references/planning-details.md', 'references/yolo.md', 'references/search.md']) {
    assert.ok(files.includes(`skills/figlens/${file}`));
    assert.equal(readFileSync(path.join(root, 'npm/skills/figlens', file), 'utf8'), readFileSync(path.join(root, 'skills/figlens', file), 'utf8'));
  }
  for (const file of ['index.ts', 'workflow.mjs', 'viewer.mjs', 'requests.mjs', 'stale.mjs', 'package.json', 'README.md']) {
    assert.ok(files.includes(`pi/${file}`));
    assert.equal(readFileSync(path.join(root, 'npm/pi', file), 'utf8'), readFileSync(path.join(root, 'pi', file), 'utf8'));
  }
  for (const [directory, names] of Object.entries({
    omp: ['index.ts', 'review.mjs', 'mcp.cjs', 'package.json', 'README.md'],
    opencode: ['index.mjs', 'tui.mjs', 'adapter.mjs', 'rpc.mjs', 'review-rpc.mjs', 'review-ui.mjs', 'watch-holder.mjs', 'package.json', 'README.md'],
  })) {
    for (const file of names) {
      assert.ok(files.includes(`${directory}/${file}`));
      assert.equal(readFileSync(path.join(root, 'npm', directory, file), 'utf8'), readFileSync(path.join(root, directory, file), 'utf8'));
    }
    assert.equal(JSON.parse(readFileSync(path.join(root, directory, 'package.json'))).type, 'module');
  }
  assert.ok(files.includes('.mcp.json'));
  assert.equal(readFileSync(path.join(root, 'npm/.mcp.json'), 'utf8'), readFileSync(path.join(root, 'omp/mcp.json'), 'utf8'));
  assert.doesNotMatch(readFileSync(path.join(root, 'omp/index.ts'), 'utf8'), /@earendil-works/);
  assert.doesNotMatch(readFileSync(path.join(root, 'opencode/index.mjs'), 'utf8'), /@earendil-works|@oh-my-pi/);
  const skill = readFileSync(path.join(root, 'npm/skills/figlens/SKILL.md'), 'utf8');
  for (const [, target] of skill.matchAll(/\]\(([^)]+)\)/g)) {
    if (/^(https?:|#)/.test(target)) continue;
    const local = path.posix.normalize(path.posix.join('skills/figlens', target.split('#')[0]));
    assert.ok(files.includes(local), `Packaged skill link is missing: ${target}`);
  }
  assert.ok(files.every(file => !file.endsWith('.fig') && !file.includes('samples/') && !file.endsWith('.test.mjs')));
  assert.ok(Object.values(manifest.peerDependenciesMeta).every(peer => peer.optional));
});
