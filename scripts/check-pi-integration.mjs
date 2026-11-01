// Real installed Pi loader/runtime, public fixture only; no viewer socket or model call.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { copyFile, readFile, utimes } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { promisify } from 'node:util';

const [sdkInput, packageInput, fixture, cwdInput, mappingInput] = process.argv.slice(2);
if (!cwdInput) throw new Error('Usage: node scripts/check-pi-integration.mjs <installed-pi-root> <installed-figlens-root> <public.fig> <isolated-consumer>');
const [sdk, packaged, cwd] = [sdkInput, packageInput, cwdInput].map(p => path.resolve(p));
assert.ok(sdk.startsWith(cwd + path.sep) && packaged.startsWith(cwd + path.sep), 'Dependencies must belong to the isolated consumer');
assert.equal(process.env.FIGLENS_BIN, undefined, 'The packaged extension must select its adjacent wrapper');
const { loadExtensions, createExtensionRuntime } = await import(pathToFileURL(path.join(sdk, 'dist/core/extensions/loader.js')));
const { loadSkills } = await import(pathToFileURL(path.join(sdk, 'dist/core/skills.js')));
const { ExtensionRunner } = await import(pathToFileURL(path.join(sdk, 'dist/core/extensions/runner.js')));
const runtime = createExtensionRuntime();
const messages = [];
const prompts = [];
runtime.sendMessage = message => messages.push(message);
runtime.sendUserMessage = prompt => prompts.push(prompt);
const entries = [];
runtime.appendEntry = (customType, data) => entries.push({ type: 'custom', customType, data });
const loaded = await loadExtensions([path.join(packaged, 'pi/index.ts')], cwd, undefined, runtime);
assert.deepEqual(loaded.errors, []);
assert.equal(loaded.extensions.length, 1);
const extension = loaded.extensions[0];
assert.ok(extension.commands.has('figlens'));
for (const name of ['figlens_bindings', 'figlens_propose_bindings', 'figlens_preview']) assert.ok(extension.tools.has(name), name);
const ctx = { cwd, mode: 'rpc', hasUI: false, waitForIdle: async () => {}, sessionManager: { getBranch: () => entries } };
try {
  for (const handler of extension.handlers.get('session_start') || []) await handler({}, ctx);
  const registrations = runtime.mcpServers.list();
  assert.equal(registrations.length, 1);
  const server = registrations[0];
  assert.equal(server.name, 'figlens');
  assert.equal(server.config.command, process.execPath);
  assert.deepEqual(server.config.args, [path.join(packaged, 'bin/figlens.js'), 'mcp']);
  assert.equal(server.config.exposure, 'direct');
  const command = args => extension.commands.get('figlens').handler(args, ctx);
  await command('help');
  assert.match(messages.at(-1).content, /Ask Pi directly/);
  await command('plan Plan only the First success card');
  for (const name of ['SKILL.md', 'references/planning.md']) {
    assert.ok(prompts.at(-1).includes(JSON.stringify(path.join(packaged, 'skills/figlens', name))), name);
  }
  const read = await extension.tools.get('figlens_bindings').definition.execute('installed-smoke',
    { file: path.resolve(fixture), frame: '0:1' }, new AbortController().signal, undefined, ctx);
  assert.equal(read.structuredContent.frame, '0:1');
  assert.equal(read.structuredContent.name, 'First success');
  assert.equal(read.structuredContent.total, 0);
  const skills = loadSkills({ cwd, agentDir: process.env.PI_CODING_AGENT_DIR,
    skillPaths: [path.join(packaged, 'skills')], includeDefaults: false });
  assert.deepEqual(skills.diagnostics, []);
  assert.equal(skills.skills.length, 1);
  assert.equal(skills.skills[0].name, 'figlens');
  for (const name of ['SKILL.md', 'references/audit.md', 'references/planning.md', 'references/planning-details.md', 'references/yolo.md', 'references/search.md']) {
    assert.ok((await readFile(path.join(packaged, 'skills/figlens', name), 'utf8')).length > 100);
  }
  // Stale context: no message without a target; a subscription this session made is checked with the packaged binary.
  // The SDK's own runner dispatches each hook, as an agent session does before and during a run.
  const runner = new ExtensionRunner(loaded.extensions, runtime, cwd, ctx.sessionManager, undefined);
  const hookErrors = [];
  runner.onError(error => hookErrors.push(error));
  // A user prompt runs the check; the note joins each model call through context, never the session.
  const turn = async () => {
    assert.deepEqual((await runner.emitBeforeAgentStart({ messageType: 'user', prompt: 'next', images: [] }, { cwd })).messages, []);
    const prompt = { role: 'user', content: 'next', timestamp: 0 };
    const sent = await runner.emitContext([prompt]);
    assert.deepEqual(hookErrors, []);
    assert.deepEqual(sent[0], prompt);
    return sent[1];
  };
  assert.equal(await turn(), undefined, 'a session without a watch target added context');
  const cli = async args => JSON.parse((await promisify(execFile)(process.execPath, [path.join(packaged, 'bin/figlens.js'), ...args], { cwd })).stdout);
  const design = path.join(cwd, `stale-${process.pid}.fig`);
  await copyFile(path.resolve(fixture), design);
  const { workspaceId } = await cli(['index', 'prepare', design, '--scope', '0:0']);
  await cli(['index', 'track', '--id', workspaceId, '--scope', '0:0']);
  const sourceId = (await cli(['index', 'status', '--id', workspaceId])).sources[0].sourceId;
  const subscribe = { action: 'subscribe', workspaceId, consumer: `pi-check-${process.pid}`, sourceId, scope: '0:0' };
  const subscribed = await cli(['watch', 'subscribe', '--id', workspaceId, '--consumer', subscribe.consumer, '--source', sourceId, '--scope', '0:0']);
  await runner.emitToolResult({ type: 'tool_result', toolName: 'mcp__figlens__watch', toolCallId: 'c1', input: subscribe,
    content: [{ type: 'text', text: JSON.stringify(subscribed) }], isError: false, details: undefined });
  const subscriptionId = subscribed.subscription.subscriptionId;
  assert.deepEqual(entries.at(-1).data, [{ workspaceId, subscriptionId, consumer: subscribe.consumer }]);
  assert.equal(await turn(), undefined, 'a subscription follows from its newest revision, so nothing is unread yet');
  // A newer stamp the index has not verified is unknown, not unchanged.
  await utimes(design, new Date(), new Date(Date.now() + 60000));
  const facts = message => JSON.parse(message.content.split('```json\n')[1].split('\n```')[0]);
  const first = await turn();
  assert.equal(first.display, false);
  assert.equal(facts(first)[0].tracking[0].refreshRequired, true, first.content);
  assert.deepEqual(entries.map(entry => entry.customType), ['figlens-watch-targets'], 'a notice entered the session');
  await runner.emit({ type: 'session_start' });
  assert.deepEqual(facts(await turn()), facts(first), 'a resumed session lost its target');
  await cli(['index', 'refresh', '--id', workspaceId, '--scope', '0:0']);
  const page = await cli(['changes', '--id', workspaceId, '--subscription', subscriptionId]);
  if (page.revisions.length) await cli(['watch', 'ack', '--id', workspaceId, '--subscription', subscriptionId, '--cursor', page.nextCursor]);
  assert.equal(await turn(), undefined, 'a refreshed and acknowledged subscription stayed stale');
  const stale = { staleContext: { dispatchedBy: 'SDK ExtensionRunner', noTargetNoContext: true, refreshRequiredReported: true, noticeNotPersisted: true, resumedFromEntries: true, processedClears: true } };
  const pi = JSON.parse(await readFile(path.join(sdk, 'package.json'), 'utf8'));
  const pkg = JSON.parse(await readFile(path.join(packaged, 'package.json'), 'utf8'));
  let mappingRoundTrip;
  if (mappingInput) {
    const project = path.resolve(mappingInput);
    const guide = path.join(project, '.figlens/component-mappings.md');
    const before = await readFile(guide, 'utf8');
    const active = JSON.parse(before.split('```json\n')[1].split('\n```')[0]).active;
    const context = { ...ctx, cwd: project };
    const call = (name, input) => extension.tools.get(name).definition.execute('host-roundtrip', input, new AbortController().signal, undefined, context);
    const bindings = (await call('figlens_bindings', { file: 'mapping.fig', frame: '0:1' })).structuredContent;
    assert.ok(bindings.items.some(item => item.status === 'approved'), 'Pi must read the native host approval');
    assert.equal(await readFile(guide, 'utf8'), before, 'Pi read must preserve the host guide');
    await call('figlens_propose_bindings', { file: 'mapping.fig', frame: '0:1', proposals: [{ instance: '0:3',
      target: { path: 'src/Button.mjs', export: 'Button', import: 'import { Button } from "./src/Button.mjs";', example: 'Button()' },
      rationale: 'Installed Pi writer after native host review',
    }] });
    const after = await readFile(guide, 'utf8');
    assert.ok(after.endsWith('\nHuman note: preserve this line.\n'));
    const state = JSON.parse(after.split('```json\n')[1].split('\n```')[0]);
    assert.deepEqual(state.active, active);
    assert.ok(state.mappings.some(item => item.status === 'pending'));
    mappingRoundTrip = { piReadNativeApproval: true, piWriterPending: true, humanNotePreserved: true, selectionUnchanged: true };
  }
  console.log(JSON.stringify({ pi: pi.version, nodeMinimum: pi.engines.node, packageVersion: pkg.version,
    modelCalled: false, mcp: server.config, mappingRoundTrip, ...stale,
    checks: ['real installed loader', 'runtime MCP registration', 'commands and tools',
      'public frame through packaged wrapper', 'skill and workflow resources'] }));
} finally {
  for (const handler of extension.handlers.get('session_shutdown') || []) await handler({}, ctx);
}
