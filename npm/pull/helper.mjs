#!/usr/bin/env node
// figlens pull's browser helper (#225). figlens owns the mapping, lock, validation and replace;
// this only drives Figma's Main menu > File > Save local copy in a dedicated Chrome profile.
// stdin: one JSON line {profile, downloads, login, designs: [{name, url}]}, then held open; when
// figlens goes away it closes and this exits, so no orphaned Chrome holds the profile.
// stdout: one JSON line per design, {name, path} or {name, stage, error}, where stage is open,
// editor, menu, export or download; it stops at the first error.
// stderr: progress for the person. Flow ported from a project script that ran it in production:
// the English UI menu path, the regular Chrome user agent in both modes (one login cookie), a
// key-only URL (a percent-encoded name slug made CloudFront answer 403) and a 10 minute login.
import { createRequire } from 'node:module';
import path from 'node:path';

const LOGIN_WAIT = 10 * 60_000;
const EXPORT_WAIT = 10 * 60_000;
const EDITOR_WAIT = 120_000;

const log = (line) => process.stderr.write(`figlens pull: ${line}\n`);
const emit = (event) => process.stdout.write(`${JSON.stringify(event)}\n`);

// The package keeps no dependencies; playwright-core comes from beside it (a global install)
// or from the project the pull runs in.
function playwright() {
  for (const from of [import.meta.url, path.join(process.cwd(), 'package.json')]) {
    try {
      return createRequire(from)('playwright-core');
    } catch {}
  }
  throw new Error('pull needs playwright-core: npm i -g playwright-core, or add it to this project');
}

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

async function main() {
  const request = await readRequest();
  const { chromium } = playwright();
  let userAgent;
  const launch = async (headless) => {
    const context = await chromium.launchPersistentContext(request.profile, {
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
    return { context, page };
  };
  const loggedIn = async (page) => {
    await page.goto('https://www.figma.com/files', { waitUntil: 'domcontentloaded' });
    // No redirect (a challenge page, a changed route) counts as signed out: sign in headed.
    await page
      .waitForURL(
        (u) => u.hostname !== 'www.figma.com' || u.pathname.startsWith('/login') || u.pathname.startsWith('/files/'),
        { timeout: EDITOR_WAIT },
      )
      .catch(() => {});
    return onFiles(page.url());
  };
  const signIn = async () => {
    const { context, page } = await launch(false);
    try {
      await page.goto('https://www.figma.com/login', { waitUntil: 'domcontentloaded' });
      log('sign in to Figma in the window that opened; this continues by itself (up to 10 minutes)');
      const deadline = Date.now() + LOGIN_WAIT;
      while (!onFiles(page.url())) {
        if (Date.now() > deadline) throw new Error('the Figma login did not finish within 10 minutes');
        await page.waitForTimeout(2000);
      }
      log('signed in');
    } finally {
      await context.close();
    }
  };

  if (request.login) return signIn();
  let { context, page } = await launch(true);
  try {
    if (!(await loggedIn(page))) {
      await context.close();
      await signIn();
      ({ context, page } = await launch(true));
      if (!(await loggedIn(page))) throw new Error('the Figma login was not kept; nothing was downloaded');
    }
    for (const [index, design] of request.designs.entries()) {
      const step = `[${index + 1}/${request.designs.length}] ${design.name}`;
      try {
        log(`${step}: opening the design`);
        await page.goto(design.url, { waitUntil: 'domcontentloaded' }).catch((e) => { throw at('open', e); });
        if ((await page.content()).includes('ERROR: The request could not be satisfied'))
          throw at('open', new Error('Figma answered CloudFront 403 before the editor opened'));
        await page
          .locator('button[aria-label="Main menu"]')
          .waitFor({ timeout: EDITOR_WAIT })
          .catch((e) => { throw at('editor', e); });
        const item = page.getByRole('menuitem', { name: /^Save local copy(?:…|\.\.\.)?$/ });
        try {
          await page.getByRole('button', { name: 'Main menu', exact: true }).click();
          await page.getByRole('menuitem', { name: 'File', exact: true }).hover();
          await item.waitFor({ timeout: EDITOR_WAIT });
        } catch (e) {
          throw at('menu', e);
        }
        if ((await item.getAttribute('aria-disabled')) === 'true')
          throw at('menu', new Error('Save local copy is disabled: this account cannot export the file'));
        const downloaded = page.waitForEvent('download', { timeout: EXPORT_WAIT });
        // Observed now, so a failed click cannot leave it rejecting unhandled.
        downloaded.catch(() => {});
        await item.click().catch((e) => { throw at('menu', e); });
        log(`${step}: exporting (Figma prepares images first; this can take minutes)`);
        let last;
        const progress = setInterval(async () => {
          const text = await page
            .evaluate(() => document.body.innerText.match(/Downloading images[^\n]*/)?.[0] ?? null)
            .catch(() => null);
          if (text && text !== last) log(`${step}: ${(last = text)}`);
        }, 2000);
        let download;
        try {
          download = await downloaded;
        } catch (e) {
          throw at('export', e);
        } finally {
          clearInterval(progress);
        }
        const failure = await download.failure();
        if (failure) throw at('download', new Error(`the download failed: ${failure}`));
        const file = path.join(request.downloads, `${index}-${path.basename(download.suggestedFilename())}`);
        await download.saveAs(file);
        log(`${step}: downloaded`);
        emit({ name: design.name, path: file });
      } catch (error) {
        emit({ name: design.name, stage: error.stage ?? 'helper', error: error.message.split('\n')[0] });
        process.exitCode = 1;
        return;
      }
    }
  } finally {
    await context.close().catch((error) => log(`closing the browser failed: ${error.message}`));
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
