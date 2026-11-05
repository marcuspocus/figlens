#!/usr/bin/env node
// figlens pull's browser helper (#225). figlens owns the mapping, lock, validation and replace;
// this only drives Figma's Main menu > File > Save local copy in a dedicated Chrome profile,
// through playwright-core or the agent-browser CLI, each with its own profile.
// stdin: one JSON line {profiles: {playwright, agent-browser}, browser, downloads, login,
// designs: [{name, url}]}, then held open; when figlens goes away it closes and this exits, so no
// orphaned Chrome holds the profile. browser null picks playwright-core when it is found.
// stdout: one JSON line per design, {name, path} or {name, stage, error}, where stage is open,
// editor, menu, export or download; it stops at the first error.
// stderr: progress for the person. Flow ported from a project script that ran it in production:
// the English UI menu path, the regular Chrome user agent in both modes (one login cookie), a
// key-only URL (a percent-encoded name slug made CloudFront answer 403) and a 10 minute login.
import { execFile, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const LOGIN_WAIT = 10 * 60_000;
const EXPORT_WAIT = 10 * 60_000;
const EDITOR_WAIT = 120_000;
const INSTALL = 'npm i -g playwright-core, or add it to this project';

const log = (line) => process.stderr.write(`figlens pull: ${line}\n`);
const emit = (event) => process.stdout.write(`${JSON.stringify(event)}\n`);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// The package keeps no dependencies; playwright-core comes from beside it (a global install)
// or from the project the pull runs in.
function findPlaywright() {
  for (const from of [import.meta.url, path.join(process.cwd(), 'package.json')]) {
    try {
      return createRequire(from)('playwright-core');
    } catch {}
  }
  return null;
}

const hasAgentBrowser = () => spawnSync('agent-browser', ['--version'], { stdio: 'ignore' }).status === 0;

function readRequest() {
  return new Promise((resolve, reject) => {
    let text = '';
    const onData = (chunk) => {
      text += chunk;
      const end = text.indexOf('\n');
      if (end < 0) return;
      process.stdin.off('data', onData);
      process.stdin.on('end', () => process.exit(1));
      resolve(JSON.parse(text.slice(0, end)));
    };
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', onData);
    process.stdin.on('end', () => reject(new Error('figlens sent no request')));
  });
}

// An error that names the step it came from.
const at = (stage, error) => Object.assign(error, { stage });

const onFiles = (url) => {
  const u = new URL(url);
  return u.origin === 'https://www.figma.com' && u.pathname.startsWith('/files/');
};

// Polls check until it returns a truthy value, or fails with what after timeout milliseconds.
async function until(check, timeout, what) {
  const deadline = Date.now() + timeout;
  for (;;) {
    const value = await check();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(what);
    await sleep(1000);
  }
}

// Each browser launches a page with: goto, url, evaluate (an expression), waitForEditor,
// openFileMenu (to Save local copy, whose disabled state it returns), save (clicks it and returns
// a function that waits for the download and saves it under a path prefix) and close.
function playwrightBrowser(request, playwright) {
  if (!playwright) throw new Error(`pull needs playwright-core: ${INSTALL}`);
  let userAgent;
  const launch = async (headless) => {
    const context = await playwright.chromium.launchPersistentContext(request.profiles.playwright, {
      channel: 'chrome',
      headless,
      acceptDownloads: true,
      downloadsPath: request.downloads,
      viewport: { width: 1440, height: 900 },
      ...(userAgent ? { userAgent } : {}),
    });
    const page = context.pages()[0] ?? (await context.newPage());
    if (!userAgent) {
      const agent = await page.evaluate(() => navigator.userAgent);
      userAgent = agent.replace('HeadlessChrome', 'Chrome');
      // Headless announces itself; relaunch so login and downloads present one browser to Figma.
      if (agent !== userAgent) {
        await context.close();
        return launch(headless);
      }
    }
    const item = page.getByRole('menuitem', { name: /^Save local copy(?:…|\.\.\.)?$/ });
    return {
      goto: (url) => page.goto(url, { waitUntil: 'domcontentloaded' }),
      url: () => page.url(),
      evaluate: (expression) => page.evaluate(expression),
      waitForEditor: (timeout) => page.locator('button[aria-label="Main menu"]').waitFor({ timeout }),
      openFileMenu: async (timeout) => {
        await page.getByRole('button', { name: 'Main menu', exact: true }).click();
        await page.getByRole('menuitem', { name: 'File', exact: true }).hover();
        await item.waitFor({ timeout });
        return (await item.getAttribute('aria-disabled')) === 'true';
      },
      save: async (timeout) => {
        const downloaded = page.waitForEvent('download', { timeout });
        // Observed now, so a failed click cannot leave it rejecting unhandled.
        downloaded.catch(() => {});
        await item.click().catch((e) => { throw at('menu', e); });
        return async (prefix) => {
          let download;
          try {
            download = await downloaded;
          } catch (e) {
            throw at('export', e);
          }
          const failure = await download.failure();
          if (failure) throw at('download', new Error(`the download failed: ${failure}`));
          const file = `${prefix}${path.basename(download.suggestedFilename())}`;
          await download.saveAs(file);
          return file;
        };
      },
      close: () => context.close(),
    };
  };
  return launch;
}

// The agent-browser CLI drives a daemon per session. The session is closed when the helper exits
// or is interrupted; one killed outright (SIGKILL) leaves it to the next pull, which closes the
// same fixed session before it launches (the pull lock allows one pull at a time).
function agentBrowser(request, found) {
  if (!found) throw new Error('pull --browser agent-browser needs the agent-browser CLI on PATH');
  const session = 'figlens-pull';
  // Chrome saves into its own folder, so a file there is this design's download and nothing else.
  const saving = path.join(request.downloads, 'agent-browser');
  fs.mkdirSync(saving, { recursive: true });
  let userAgent;
  let flags = [];
  const run = (args) =>
    new Promise((resolve, reject) => {
      // Its 25 s default would fail a navigation to a heavy design before the editor wait starts.
      const env = { ...process.env, AGENT_BROWSER_DEFAULT_TIMEOUT: String(EDITOR_WAIT) };
      execFile('agent-browser', ['--session', session, '--json', ...flags, ...args], { env, maxBuffer: 16 << 20 }, (error, stdout) => {
        let reply;
        try {
          reply = JSON.parse(stdout);
        } catch {}
        if (reply?.success) return resolve(reply.data);
        reject(new Error(`agent-browser ${args[0]}: ${reply?.error ?? error?.message ?? 'no reply'}`));
      });
    });
  const close = () => spawnSync('agent-browser', ['--session', session, 'close'], { stdio: 'ignore' });
  process.on('exit', close);
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(signal, () => process.exit(1));
  const SAVE = `[...document.querySelectorAll('[role="menuitem"]')].find((e) => /^Save local copy(?:…|\\.\\.\\.)?$/.test(e.textContent.trim()))`;
  const launch = async (headless) => {
    // Launch options take effect when the session's browser starts, so each launch starts one.
    close();
    flags = [
      '--profile', request.profiles['agent-browser'],
      '--download-path', saving,
      '--headed', String(!headless),
      // Polling keeps the session busy; this only ends a daemon this helper could not close.
      '--idle-timeout', '10m',
      ...(userAgent ? ['--user-agent', userAgent] : []),
    ];
    await run(['open', 'about:blank']);
    await run(['set', 'viewport', '1440', '900']);
    if (!userAgent) {
      const agent = (await run(['eval', 'navigator.userAgent'])).result;
      userAgent = agent.replace('HeadlessChrome', 'Chrome');
      // Headless announces itself; relaunch so login and downloads present one browser to Figma.
      if (agent !== userAgent) return launch(headless);
    }
    const evaluate = async (expression) => (await run(['eval', expression])).result;
    // A poll during a navigation can fail; the wait goes on until its own timeout.
    const poll = (expression) => evaluate(expression).catch(() => false);
    return {
      goto: (url) => run(['open', url]),
      url: async () => (await run(['get', 'url'])).url,
      evaluate,
      waitForEditor: (timeout) =>
        until(() => poll(`!!document.querySelector('button[aria-label="Main menu"]')`), timeout, `the editor did not open within ${timeout} ms`),
      openFileMenu: async (timeout) => {
        await run(['find', 'role', 'button', 'click', '--name', 'Main menu', '--exact']);
        await run(['find', 'role', 'menuitem', 'hover', '--name', 'File', '--exact']);
        await until(() => poll(`!!${SAVE}`), timeout, 'Save local copy did not appear in the File menu');
        return evaluate(`${SAVE}.getAttribute('aria-disabled') === 'true'`);
      },
      save: async (timeout) => {
        for (const name of fs.readdirSync(saving)) fs.rmSync(path.join(saving, name), { force: true });
        await run(['find', 'role', 'menuitem', 'click', '--name', 'Save local copy']).catch((e) => { throw at('menu', e); });
        // One agent-browser command cannot wait minutes for an export, so watch the folder: done
        // when Chrome has no partial file left and the file's size held for one poll.
        return async (prefix) => {
          let last;
          const name = await until(
            () => {
              const names = fs.readdirSync(saving);
              if (names.length !== 1 || names[0].endsWith('.crdownload')) return null;
              const size = fs.statSync(path.join(saving, names[0])).size;
              const done = size > 0 && size === last;
              last = size;
              return done && names[0];
            },
            timeout,
            `no download finished within ${timeout / 60_000} minutes`,
          ).catch((e) => { throw at('export', e); });
          const file = `${prefix}${path.basename(name)}`;
          fs.renameSync(path.join(saving, name), file);
          return file;
        };
      },
      close: async () => close(),
    };
  };
  return launch;
}

function chooseBrowser(request) {
  if (request.browser === 'agent-browser') return agentBrowser(request, hasAgentBrowser());
  const playwright = findPlaywright();
  if (request.browser === 'playwright' || playwright) return playwrightBrowser(request, playwright);
  if (hasAgentBrowser()) return agentBrowser(request, true);
  throw new Error(`pull needs playwright-core or the agent-browser CLI: ${INSTALL}, or put agent-browser on PATH`);
}

async function main() {
  const request = await readRequest();
  const launch = chooseBrowser(request);
  const loggedIn = async (page) => {
    await page.goto('https://www.figma.com/files');
    // No redirect (a challenge page, a changed route) counts as signed out: sign in headed.
    await until(
      async () => {
        const u = new URL(await page.url());
        return u.hostname !== 'www.figma.com' || u.pathname.startsWith('/login') || u.pathname.startsWith('/files/');
      },
      EDITOR_WAIT,
      'timeout',
    ).catch(() => {});
    return onFiles(await page.url());
  };
  const signIn = async () => {
    const page = await launch(false);
    try {
      await page.goto('https://www.figma.com/login');
      log('sign in to Figma in the window that opened; this continues by itself (up to 10 minutes)');
      await until(async () => onFiles(await page.url()), LOGIN_WAIT, 'the Figma login did not finish within 10 minutes');
      log('signed in');
    } finally {
      await page.close();
    }
  };

  if (request.login) return signIn();
  let page = await launch(true);
  try {
    if (!(await loggedIn(page))) {
      await page.close();
      await signIn();
      page = await launch(true);
      if (!(await loggedIn(page))) throw new Error('the Figma login was not kept; nothing was downloaded');
    }
    for (const [index, design] of request.designs.entries()) {
      const step = `[${index + 1}/${request.designs.length}] ${design.name}`;
      try {
        log(`${step}: opening the design`);
        await page.goto(design.url).catch((e) => { throw at('open', e); });
        if (await page.evaluate(`document.documentElement.outerHTML.includes('ERROR: The request could not be satisfied')`))
          throw at('open', new Error('Figma answered CloudFront 403 before the editor opened'));
        await page.waitForEditor(EDITOR_WAIT).catch((e) => { throw at('editor', e); });
        let disabled;
        try {
          disabled = await page.openFileMenu(EDITOR_WAIT);
        } catch (e) {
          throw at('menu', e);
        }
        if (disabled) throw at('menu', new Error('Save local copy is disabled: this account cannot export the file'));
        const finish = await page.save(EXPORT_WAIT);
        log(`${step}: exporting (Figma prepares images first; this can take minutes)`);
        // Each check starts after the last returns, so a slow agent-browser call cannot pile up.
        let last;
        let exporting = true;
        const progress = async () => {
          await sleep(2000);
          if (!exporting) return;
          const text = await page
            .evaluate(`document.body.innerText.match(/Downloading images[^\\n]*/)?.[0] ?? null`)
            .catch(() => null);
          if (text && text !== last && exporting) log(`${step}: ${(last = text)}`);
          if (exporting) return progress();
        };
        progress();
        let file;
        try {
          file = await finish(path.join(request.downloads, `${index}-`));
        } finally {
          // A check still running ends by itself; waiting for it would delay the next design.
          exporting = false;
        }
        log(`${step}: downloaded`);
        emit({ name: design.name, path: file });
      } catch (error) {
        emit({ name: design.name, stage: error.stage ?? 'helper', error: error.message.split('\n')[0] });
        process.exitCode = 1;
        return;
      }
    }
  } finally {
    await page.close().catch((error) => log(`closing the browser failed: ${error.message}`));
  }
}

// Let go of stdin rather than calling process.exit, which can drop a pending stdout line.
const finish = (code) => {
  process.exitCode = code;
  process.stdin.removeAllListeners('end');
  process.stdin.destroy();
};
main().then(
  () => finish(process.exitCode ?? 0),
  (error) => {
    log(error.message.split('\n')[0]);
    finish(1);
  },
);
