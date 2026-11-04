import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const helper = fileURLToPath(new URL('./pull/helper.mjs', import.meta.url));

test('the pull helper says how to install playwright-core when it is missing', (t) => {
  const cwd = mkdtempSync(path.join(os.tmpdir(), 'figlens-pull-helper-'));
  t.after(() => rmSync(cwd, { recursive: true, force: true }));
  const run = spawnSync(process.execPath, [helper], { cwd, input: '{"designs":[]}\n', encoding: 'utf8' });
  assert.equal(run.status, 1);
  assert.match(run.stderr, /npm i -g playwright-core/);
});

test('the pull helper exits when figlens sends no request', () => {
  const run = spawnSync(process.execPath, [helper], { input: '', encoding: 'utf8' });
  assert.equal(run.status, 1);
});
