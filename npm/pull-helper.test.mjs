import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const helper = fileURLToPath(new URL('./pull/helper.mjs', import.meta.url));

const scratch = (t) => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'figlens-pull-helper-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
};

// PATH holds only `bin`, so neither a real agent-browser nor playwright-core is found.
const runHelper = (cwd, request) =>
  spawnSync(process.execPath, [helper], {
    cwd,
    input: `${JSON.stringify(request)}\n`,
    encoding: 'utf8',
    env: { ...process.env, PATH: path.join(cwd, 'bin') },
  });

test('the pull helper says how to install either browser when neither is found', (t) => {
  const cwd = scratch(t);
  const run = runHelper(cwd, { designs: [] });
  assert.equal(run.status, 1);
  assert.match(run.stderr, /npm install --save-dev playwright-core/);
  assert.match(run.stderr, /npx -y -p @runkids\/figlens -p playwright-core figlens pull/);
  assert.doesNotMatch(run.stderr, /npm i -g playwright-core/);
});

test('the pull helper names agent-browser when it is asked for and missing', (t) => {
  const cwd = scratch(t);
  const run = runHelper(cwd, { browser: 'agent-browser', designs: [] });
  assert.match(run.stderr, /needs the agent-browser CLI on PATH/);
});

test('the pull helper exits when figlens sends no request', () => {
  const run = spawnSync(process.execPath, [helper], { input: '', encoding: 'utf8' });
  assert.equal(run.status, 1);
});

// A stand-in agent-browser: a signed-in Figma whose Save local copy writes a file into the
// --download-path it was launched with. Each call is logged to `calls`.
const FAKE = `#!${process.execPath}
const fs = require('node:fs');
const path = require('node:path');
const args = process.argv.slice(2);
fs.appendFileSync(path.join(__dirname, 'calls'), JSON.stringify(args) + '\\n');
const flag = (name) => args[args.indexOf(name) + 1];
const state = path.join(__dirname, 'url');
const reply = (data) => process.stdout.write(JSON.stringify({ success: true, data }));
const skip = new Set(['--session', '--profile', '--download-path', '--headed', '--idle-timeout', '--user-agent']);
const rest = args.filter((a, i) => a !== '--json' && !skip.has(a) && !(skip.has(args[i - 1]) && (args[i - 1] !== '--headed' || a === 'false' || a === 'true')));
const [command, ...more] = rest;
if (command === 'open') fs.writeFileSync(state, more[0] ?? 'about:blank');
if (command === 'get') {
  const url = fs.readFileSync(state, 'utf8');
  reply({ url: (url.endsWith('/files') || url.endsWith('/login')) ? 'https://www.figma.com/files/recent' : url });
} else if (command === 'eval') {
  const js = more[0];
  reply({ result: js.includes('#command_line') ? (fs.existsSync(path.join(__dirname, 'headless')) ? 'chrome --headless=new --user-agent=Chrome/1' : 'chrome --remote-debugging-port=0') : js === 'navigator.userAgent' ? 'Mozilla/5.0 Chrome/1' : js.includes('outerHTML') || js.includes('aria-disabled') || js.includes('Downloading') ? false : true });
} else {
  if (command === 'find' && more.includes('Save local copy') && !fs.existsSync(path.join(__dirname, 'hold')))
    fs.writeFileSync(path.join(flag('--download-path'), 'Home.fig'), 'fig');
  reply({});
}
`;

// figlens holds stdin open while the helper works, and the helper exits when it closes.
const converse = (cwd, request) =>
  new Promise((resolve) => {
    const child = spawn(process.execPath, [helper], { cwd, env: { ...process.env, PATH: path.join(cwd, 'bin') } });
    let stdout = '';
    let stderr = '';
    child.stderr.on('data', (chunk) => (stderr += chunk));
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
      if (stdout.includes('\n')) child.stdin.end();
    });
    child.on('close', () => resolve({ stdout, stderr }));
    child.stdin.write(`${JSON.stringify(request)}\n`);
  });

test('the pull helper saves a download through agent-browser', async (t) => {
  const cwd = scratch(t);
  const bin = path.join(cwd, 'bin');
  const downloads = path.join(cwd, 'downloads');
  mkdirSync(bin);
  mkdirSync(downloads);
  writeFileSync(path.join(bin, 'agent-browser'), FAKE);
  chmodSync(path.join(bin, 'agent-browser'), 0o755);
  const run = await converse(cwd, {
    profiles: { playwright: path.join(cwd, 'pw'), 'agent-browser': path.join(cwd, 'ab') },
    browser: null,
    downloads,
    login: false,
    designs: [{ name: 'home', url: 'https://www.figma.com/design/abc/' }],
  });
  assert.match(run.stderr, /browser: agent-browser.*playwright-core was not found.*npx -y -p @runkids\/figlens -p playwright-core/);
  const event = JSON.parse(run.stdout);
  const calls = readFileSync(path.join(bin, 'calls'), 'utf8');
  assert.deepEqual(
    [event, readFileSync(event.path, 'utf8'), calls.includes(JSON.stringify(path.join(cwd, 'ab')))],
    [{ name: 'home', path: path.join(downloads, '0-Home.fig') }, 'fig', true],
    run.stderr,
  );
});

test('the pull helper closes its agent-browser session when interrupted', async (t) => {
  const cwd = scratch(t);
  const bin = path.join(cwd, 'bin');
  mkdirSync(bin);
  mkdirSync(path.join(cwd, 'downloads'));
  writeFileSync(path.join(bin, 'agent-browser'), FAKE);
  chmodSync(path.join(bin, 'agent-browser'), 0o755);
  // The export never finishes, so the helper is still waiting when it is interrupted.
  writeFileSync(path.join(bin, 'hold'), '');
  const child = spawn(process.execPath, [helper], { cwd, env: { ...process.env, PATH: bin } });
  child.stdin.write(`${JSON.stringify({
    profiles: { playwright: path.join(cwd, 'pw'), 'agent-browser': path.join(cwd, 'ab') },
    browser: 'agent-browser',
    downloads: path.join(cwd, 'downloads'),
    login: false,
    designs: [{ name: 'home', url: 'https://www.figma.com/design/abc/' }],
  })}\n`);
  const calls = () => {
    try {
      return readFileSync(path.join(bin, 'calls'), 'utf8');
    } catch {
      return '';
    }
  };
  let progress = '';
  child.stderr.on('data', (chunk) => (progress += chunk));
  while (!progress.includes('exporting (')) await new Promise((resolve) => setTimeout(resolve, 50));
  const closed = new Promise((resolve) => child.on('close', resolve));
  child.kill('SIGINT');
  await closed;
  assert.match(calls().trim().split('\n').at(-1), /"close"/);
});

for (const headless of [false, true]) {
  test(`agent-browser login verifies headed mode before prompting (headless=${headless})`, { timeout: 8000 }, async (t) => {
    const cwd = scratch(t);
    const bin = path.join(cwd, 'bin');
    mkdirSync(bin);
    mkdirSync(path.join(cwd, 'downloads'));
    writeFileSync(path.join(bin, 'agent-browser'), FAKE);
    chmodSync(path.join(bin, 'agent-browser'), 0o755);
    if (headless) writeFileSync(path.join(bin, 'headless'), '');
    const run = await converse(cwd, {
      profiles: { 'agent-browser': path.join(cwd, 'ab') },
      browser: 'agent-browser', downloads: path.join(cwd, 'downloads'), login: true, designs: [],
    });
    const calls = readFileSync(path.join(bin, 'calls'), 'utf8');
    assert.match(calls, /chrome:\/\/version/);
    if (headless) {
      assert.match(run.stderr, /could not verify a headed Chrome window.*--browser playwright/);
      assert.doesNotMatch(run.stderr, /window that opened/);
      assert.doesNotMatch(calls, /figma.com\/login/);
    } else {
      assert.match(run.stderr, /window that opened/);
      const open = calls.split('\n').filter(Boolean).map(JSON.parse).find((args) => args.includes('about:blank'));
      assert.equal(open[open.indexOf('--headed') + 1], '--idle-timeout');
    }
    assert.match(calls.trim().split('\n').at(-1), /"close"/);
  });
}
