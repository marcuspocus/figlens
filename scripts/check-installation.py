#!/usr/bin/env python3
"""Layer B: install a real candidate tarball and exercise its wrapper, native CLI and MCP.

The test-only fetch preload supplies the candidate archive to the unchanged wrapper.
This is not layer C (a published release download).
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import queue
import runpy
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import traceback
from installation import environment, run, toolchain
from mcp_smoke import check_directory, check_first_use, check_library, check_mcp, check_question, check_reuse, session, user_roots

ROOT = Path(__file__).resolve().parent.parent
# The synthetic mapping fixture the host lanes use; generate it as wiki/integrations.md says.
MAPPING = ROOT/'samples/plugin-research/fixtures/mapping-library.fig'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_runtime(cli, mcp, cwd, env):
    """Model-free: figlens finds the figlens-embed shipped beside it. configure checks the runtime
    before any model file, so an empty model directory must fail on the model (an error naming
    the profile), never on a missing runtime, and nothing is configured. Each entry is a CLI and
    its MCP command; Pi, OpenCode and OMP launch one of them."""
    with tempfile.TemporaryDirectory(prefix='runtime-', dir=cwd) as temp:
        base = Path(temp)
        empty = base/'model'
        empty.mkdir()
        env = user_roots(base, env)
        profile = json.loads(run([*cli, 'models', 'list'], cwd, env))['profiles'][0]['name']
        # Before any model is set up, status names the runtime beside the executable that ran.
        status = json.loads(run([*cli, 'models', 'status'], cwd, env))
        assert status['profiles'] == [] and status['defaultRuntime']['available'], status
        runtime = Path(status['defaultRuntime']['path'])
        assert runtime.is_file() and runtime.stem == 'figlens-embed' and runtime.with_stem('figlens').is_file(), status
        try:
            run([*cli, 'models', 'configure', profile, '--model', empty], cwd, env)
            raise AssertionError('configure accepted an empty model directory')
        except RuntimeError as error:
            assert f'{profile}:' in str(error) and 'no model runtime' not in str(error), str(error)
        with session(mcp, cwd, env, timeout=60) as request:
            request('initialize', {'protocolVersion': '2025-06-18'})
            request('notifications/initialized', {})
            reply = request('tools/call', {'name': 'models', 'arguments': {'action': 'configure', 'profile': profile, 'modelDir': str(empty)}})
            text = json.dumps(reply)
            assert reply['isError'] and f'{profile}:' in text and 'no model runtime' not in text, text
        assert json.loads(run([*cli, 'models', 'status'], cwd, env))['profiles'] == []
    return {'profile': profile, 'runtime': status['defaultRuntime'], 'configured': False}


def imported_dlls(path):
    """The DLL names a PE executable's import table names."""
    data = Path(path).read_bytes()
    pe = int.from_bytes(data[0x3c:0x40], 'little')
    assert data[pe:pe + 4] == b'PE\0\0', f'{path} is not a PE executable'
    word = lambda at, n=4: int.from_bytes(data[at:at + n], 'little')
    sections, optional = word(pe + 6, 2), pe + 24
    imports = word(optional + (112 if word(optional, 2) == 0x20b else 96) + 8)
    table = [(word(at + 12), max(word(at + 8), word(at + 16)), word(at + 20))
             for at in (optional + word(pe + 20, 2) + 40 * i for i in range(sections))]
    offset = lambda rva: next(rva - va + raw for va, size, raw in table if va <= rva < va + size)
    names, at = [], offset(imports)
    while word(at + 12):
        start = offset(word(at + 12))
        names.append(data[start:data.index(b'\0', start)].decode())
        at += 20
    return names


def check_index(binary, fixture, cwd, env):
    """The shipped binary's own SQLite: a custom-storage workspace found again without --storage,
    then reset and prepared again with a new incarnation; the source is never written."""
    base = cwd/'index-smoke'
    base.mkdir()
    source = base/'design.fig'
    shutil.copy2(fixture, source)
    before = sha(source)
    roots = {f'XDG_{kind}_HOME': str(base/kind.lower()) for kind in ['CONFIG', 'STATE', 'CACHE', 'DATA']}
    env = dict(env, **roots)
    index = lambda *args: json.loads(run([binary, 'index', *args], cwd, env))
    assert index('status', '--file', source)['state'] == 'absent'
    assert not any(Path(root).exists() for root in roots.values()), 'status of an absent workspace created storage'
    custom = base/'custom'
    first = index('prepare', source, '--storage', custom)
    assert first['initialized'] and first['storage']['rule'] == '--storage', first
    found = index('status', '--file', source)
    assert found['storage']['rule'] == 'remembered' and found['incarnation'] == first['incarnation'], found
    # <custom>/state/workspaces/<id>/index.sqlite; samefile ignores Windows \\?\ prefixes.
    assert os.path.samefile(Path(found['paths']['database']).parents[2], custom/'state'), found
    assert index('reset', '--file', source)['state'] == 'reset'
    again = index('prepare', source)
    assert again['initialized'] and again['incarnation'] != first['incarnation'], again
    assert again['storage']['rule'] == 'remembered', again
    # The shipped binary starts its own index worker, publishes, and serves search from it.
    built = index('prepare', source, '--scope', '0:0')
    assert built['job']['state'] == 'succeeded', built
    searched = json.loads(run([binary, 'search', '--file', source, 'First success'], cwd, env))
    assert searched['state'] == 'ready' and searched['results'][0]['ref']['nodeId'] == '0:1', searched
    # Wiring only: the fixture records no library reference, so the question is unresolved and has no users.
    used = json.loads(run([binary, 'usages', '--file', source, '--library', 'component', '--namespace', 'lib', '--key', 'k1'], cwd, env))
    assert used['library'] == {'kind': 'component', 'namespace': 'lib', 'key': 'k1'} and used['results'] == [], used
    assert used['definition'] == {'state': 'unresolved', 'reason': 'library_source_unproven'}, used
    assert used['coverage']['complete'] is True and [s['state'] for s in used['coverage']['sources']] == ['ready'], used
    job = json.loads(run([binary, 'jobs', 'status', built['job']['jobId']], cwd, env))
    assert job['state'] == 'succeeded', job
    # A removed remembered root is recreated by an equivalent spelling: on Windows the ordinary
    # path against the remembered \\?\ form, elsewhere a path through "..".
    shutil.rmtree(custom)
    spelling = str(custom) if os.name == 'nt' else f'{custom}{os.sep}..{os.sep}{custom.name}'
    recreated = index('prepare', source, '--storage', spelling)
    assert recreated['initialized'] and recreated['incarnation'] != again['incarnation'], recreated
    assert os.path.samefile(Path(recreated['paths']['database']).parents[2], custom/'state'), recreated
    assert sha(source) == before, 'index changed the source .fig'
    assert not (base/'state').exists(), 'a second database appeared in the default state root'
    return {'customPrepare': True, 'rediscovered': True, 'resetNewIncarnation': True, 'indexedSearch': True, 'libraryUsages': True,
            'removedRootRecreated': True, 'sourceUnchanged': True}


def check_watch(binary, fixture, cwd, env):
    """Two `watch run` holders share one detached runner. A forced exit of the first holder (its
    PID alone, never a tree kill) leaves the runner and the second's lease; the runner records a
    save and evaluates a saved search after it. The runner is then lost: on Windows by
    terminating a test job object that the first holder ran in and that forbids breakaway, so its
    runner had to start `in_host_job`; elsewhere by killing the runner's PID. The second holder's
    renewal starts it again, and its stdin EOF ends the runner. The workspace is removed only once
    the runner is proven stopped."""
    base = Path(tempfile.mkdtemp(prefix='figlens-watch-'))
    source = base/'design.fig'
    shutil.copy2(fixture, source)
    env = dict(env, **{f'XDG_{kind}_HOME': str(base/kind.lower()) for kind in ['CONFIG', 'STATE', 'CACHE', 'DATA']})
    cli = lambda *args: json.loads(run([binary, *args], cwd, env))
    status = lambda: cli('watch', 'status', '--file', source)
    revisions = lambda: sum(r['kind'] == 'changes' for r in cli('changes', '--file', source, '--scope', '0:0')['revisions'])
    evaluation = lambda: cli('changes', '--file', source, '--subscription', saved)['evaluation']['state']

    def wait(what, done, limit=30):
        start = time.monotonic()
        while not done():
            assert time.monotonic() - start < limit, f'watch: timed out waiting for {what}'
            time.sleep(0.1)
        return round((time.monotonic() - start) * 1000)

    def holder(job=None):
        # Suspended until it is in the job, so the runner it starts is placed from inside it.
        process = subprocess.Popen([str(binary), 'watch', 'run', '--file', str(source), '--scope', '0:0'], cwd=cwd,
                                   env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                   text=True, encoding='utf-8', creationflags=0x4 if job else 0)  # CREATE_SUSPENDED
        holders.append(process)
        if job:
            # Popen keeps the process handle only in _handle; NtResumeProcess resumes all its threads.
            check(kernel32.AssignProcessToJobObject(job, int(process._handle)))
            assert resume(int(process._handle)) == 0, 'watch: resume failed'
        process.lines = queue.Queue()
        threading.Thread(target=lambda: [process.lines.put(l) for l in process.stdout] + [process.lines.put(None)],
                         daemon=True).start()
        return process

    def line(process):
        try:
            text = process.lines.get(timeout=30)
        except queue.Empty as error:
            raise TimeoutError('watch: a holder wrote no line within 30 seconds') from error
        assert text is not None, f'watch: holder exited {process.poll()} before its line'
        return json.loads(text)

    def runner_pid(workspace):
        # The OS process list, read with the caller's environment (the consumer one has no PATH).
        listed = [line.split()[0] for line in subprocess.run(['ps', '-axo', 'pid=,command='], capture_output=True,
                  text=True, check=True, timeout=60).stdout.splitlines() if '__watch-runner' in line and workspace in line]
        assert len(listed) == 1, f'watch: expected one runner for {workspace}, found {listed}'
        return int(listed[0])

    if os.name == 'nt':
        import ctypes
        from ctypes import wintypes
        kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)
        kernel32.CreateJobObjectW.restype = wintypes.HANDLE
        kernel32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
        kernel32.TerminateJobObject.argtypes = [wintypes.HANDLE, wintypes.UINT]
        kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
        resume = ctypes.WinDLL('ntdll').NtResumeProcess
        resume.argtypes, resume.restype = [wintypes.HANDLE], ctypes.c_long  # NTSTATUS
        def check(ok):
            if not ok:
                raise ctypes.WinError(ctypes.get_last_error())
        # A job without JOB_OBJECT_LIMIT_BREAKAWAY_OK: a child cannot break away from it.
        job = kernel32.CreateJobObjectW(None, None)
        check(job)
    else:
        job = None
    holders, stopped = [], False
    try:
        cli('index', 'prepare', source, '--scope', '0:0')
        cli('index', 'track', '--file', source, '--scope', '0:0')
        # A saved search the runner must evaluate after the save, with no evaluate call.
        source_id = cli('changes', '--file', source, '--scope', '0:0')['tracking'][0]['sourceId']
        saved = cli('watch', 'subscribe', '--file', source, '--consumer', 'check', '--source', source_id,
                    '--query', 'button')['subscription']['subscriptionId']
        # The first holder starts the runner; the second, outside any test job, joins it.
        first = holder(job)
        lease = line(first)
        assert lease['state'] == 'active' and lease['runner']['state'] == 'running', lease
        last = holder()
        assert line(last)['state'] == 'active'
        runner = status()['runner']
        placement = runner['placement']
        assert placement == ('in_host_job' if job else 'independent'), runner
        first.kill()
        first.wait(timeout=30)
        after = status()
        assert after['runner']['state'] == 'running' and after['runner']['attempt'] == runner['attempt'], after
        assert sorted(l['state'] for l in after['leases']) == ['active', 'active'], after
        changed = base/'next.fig'
        changed.write_bytes(source.read_bytes() + bytes(16))
        os.replace(changed, source)
        save_ms = wait('the runner to record a save', lambda: revisions() == 1)
        evaluated_ms = save_ms + wait('the runner to evaluate the saved search', lambda: evaluation() == 'current')
        if job:
            check(kernel32.TerminateJobObject(job, 1))
        else:
            os.kill(runner_pid(lease['workspaceId']), signal.SIGKILL)
        lost = time.monotonic()
        wait('a renewal to start the runner again', lambda: status()['runner'].get('attempt', 0) > runner['attempt']
             and status()['runner']['state'] == 'running')
        gap_ms = round((time.monotonic() - lost) * 1000)
        restarted = status()['runner']['placement']
        # The killed holder renewed for the last time before its kill: its lease lapses within the 30 s TTL.
        wait("the killed holder's lease to expire", lambda: sorted(l['state'] for l in status()['leases']) == ['active', 'expired'], limit=40)
        last.stdin.close()
        ended = line(last)
        assert last.wait(timeout=30) == 0 and ended == {'leaseId': ended['leaseId'], 'state': 'ended', 'reason': 'released'}, ended
        exit_ms = wait('the runner to exit after the last lease', lambda: status()['runner']['state'] == 'stopped', limit=10)
        stopped = True
    finally:
        # End this check's leases, then wait out any lease a killed holder left (TTL 30 s): a runner
        # releases runner.lock only after draining its own jobs, so `stopped` proves both.
        for process in holders:
            if process.poll() is None:
                process.stdin.close()
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=30)
        if job:
            kernel32.TerminateJobObject(job, 1)
            kernel32.CloseHandle(job)
        if not stopped:
            try:
                wait('the runner to stop before cleanup', lambda: status()['runner']['state'] == 'stopped', limit=45)
                stopped = True
            except Exception as error:
                print(f'watch: {error}', file=sys.stderr)
        if stopped:
            shutil.rmtree(base)
        else:
            print(f'watch: runner not proven stopped; kept {base}', file=sys.stderr)
    return {'placement': placement, 'firstHolderPidKilledRunnerKept': True, 'saveRecordedMs': save_ms,
            'savedSearchEvaluatedMs': evaluated_ms,
            'runnerLost': 'host job terminated' if job else 'runner PID killed', 'restartGapMs': gap_ms,
            'restartedPlacement': restarted, 'lastEofRunnerExitMs': exit_ms}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts', required=True, type=Path)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--pi-version', choices=['1.3.0', '1.4.0'])
    parser.add_argument('--opencode-bin', type=Path)
    parser.add_argument('--opencode-sdk', type=Path, help='Installed @opencode/client package root')
    parser.add_argument('--omp-sdk', type=Path)
    parser.add_argument('--bun', type=Path)
    parser.add_argument('--host-watch-only', action='store_true',
                        help='Run only the OpenCode/OMP watch and stale-context checks, on the public design')
    args = parser.parse_args()
    if bool(args.opencode_bin) != bool(args.opencode_sdk) or bool(args.omp_sdk) != bool(args.bun):
        parser.error('OpenCode requires both --opencode-bin/--opencode-sdk; OMP requires both --omp-sdk/--bun')
    artifacts = args.artifacts.resolve()
    metadata = json.loads((artifacts/'candidate.json').read_text())
    assert metadata['commit'] == args.commit, 'Candidate is from another commit'
    package = artifacts/metadata['package']['file']
    assert sha(package) == metadata['package']['sha256'], 'Candidate tarball hash differs'
    required = {'package.json', 'README.md', 'LICENSE', 'bin/figlens.js', 'target.js',
                'pi/index.ts', 'pi/workflow.mjs', 'pi/viewer.mjs', 'pi/requests.mjs', 'pi/stale.mjs', 'pi/package.json', 'pi/README.md',
                '.mcp.json', 'omp/index.ts', 'omp/review.mjs', 'omp/mcp.cjs', 'omp/package.json', 'omp/README.md',
                'opencode/index.mjs', 'opencode/tui.mjs', 'opencode/adapter.mjs', 'opencode/rpc.mjs',
                'opencode/review-rpc.mjs', 'opencode/review-ui.mjs', 'opencode/watch-holder.mjs', 'opencode/package.json', 'opencode/README.md',
                'skills/figlens/SKILL.md', 'skills/figlens/references/audit.md', 'skills/figlens/references/planning.md',
                'skills/figlens/references/planning-details.md', 'skills/figlens/references/yolo.md',
                'skills/figlens/references/search.md'}
    with tarfile.open(package) as archive:
        files = {p.name.removeprefix('package/') for p in archive.getmembers() if p.isfile()}
        assert files == required, f'Unexpected npm package files: missing={required-files}, extra={files-required}'
    node, npm = toolchain()
    with tempfile.TemporaryDirectory(prefix='figlens-consumer-') as temp:
        work = Path(temp)
        env = environment(work, node)
        run([node, '-e', "if (typeof fetch !== 'function') throw new Error('Native fetch is unavailable on this Node runtime')"], work, env)
        consumer = work/'consumer'
        consumer.mkdir()
        (consumer/'package.json').write_text('{"private":true}')
        install = npm + ['install', '--ignore-scripts', '--no-audit', '--no-fund', '--package-lock=false']
        run(install + ['--offline', '--omit=peer', package], consumer, env)
        installed = consumer/'node_modules/@runkids/figlens'
        assert installed.is_dir() and not installed.is_symlink(), 'Not an extracted tarball installation'
        manifest = json.loads((installed/'package.json').read_text())
        assert manifest['version'] == metadata['version']
        assert manifest['bin']['figlens'] == 'bin/figlens.js'
        assert manifest['pi']['extensions'] == ['./pi/index.ts'] and manifest['pi']['skills'] == ['./skills']
        asset = json.loads(run([node, '-e', 'console.log(JSON.stringify(require(process.argv[1]).asset(process.platform,process.arch,process.argv[2])))',
                                installed/'target.js', metadata['version']], consumer, env))
        archive = asset['archive']
        expected = metadata['archives'][archive]
        assert sha(artifacts/archive) == expected['sha256'], 'Candidate native archive hash differs'
        # Only these two exact release URLs can be resolved; no old GitHub release is reachable.
        feed = work/'feed.json'
        events = work/'fetch.jsonl'
        feed.write_text(json.dumps({'version': metadata['version'], 'archive': archive,
                                    'directory': str(artifacts), 'events': str(events)}))
        preload = str(ROOT/'scripts/candidate-fetch.cjs').replace('\\', '/')
        wrapped_env = dict(env, FIGLENS_TEST_FEED=str(feed), NODE_OPTIONS=f'--require="{preload}"')
        wrapper = [node, installed/'bin/figlens.js']
        cache = work/'cache/figlens/npm'/metadata['version']/asset['binary']
        assert not cache.exists(), 'Cache was not cold'
        expected_version = f'figlens {metadata["version"]}'
        assert run(wrapper + ['--version'], consumer, wrapped_env).strip() == expected_version
        assert sha(cache) == expected['binarySha256'], 'Wrapper ran a different native binary'
        # The optional model runtime is extracted beside it, executable, and starts natively
        # (no missing DLL), answering --version and --help without a model.
        companion = cache.with_name(asset['companion'])
        assert sha(companion) == expected['companionSha256'], 'Wrapper extracted a different model runtime'
        assert run([companion, '--version'], consumer, env).strip() == f'figlens-embed {metadata["version"]}'
        assert 'figlens-embed' in run([companion, '--help'], consumer, env)
        if os.name == 'nt':
            # Git Bash puts MinGW runtime DLLs on PATH, so starting proves nothing: every import must
            # be a Windows API set or a DLL Windows itself ships in System32.
            system32 = Path(os.environ['SystemRoot'])/'System32'
            for exe in [cache, companion]:
                assert 'kernel32.dll' in map(str.lower, imported_dlls(exe)), f'{exe.name}: import table unread'
                extra = [d for d in imported_dlls(exe) if not d.lower().startswith('api-ms-win-') and not (system32/d).is_file()]
                assert not extra, f'{exe.name} needs DLLs Windows does not ship: {extra}'
        downloads = events.read_text().splitlines()
        assert sorted(json.loads(line)['name'] for line in downloads) == sorted([archive, 'SHA256SUMS'])
        assert run(wrapper + ['--version'], consumer, wrapped_env).strip() == expected_version
        assert events.read_text().splitlines() == downloads, 'Warm cache downloaded again'
        # A second cold cache exercises real simultaneous wrapper processes and real extraction.
        concurrent = work/'parallel-cache'
        parallel_env = dict(wrapped_env, XDG_CACHE_HOME=str(concurrent), LOCALAPPDATA=str(concurrent))
        with ThreadPoolExecutor(max_workers=2) as pool:
            versions = list(pool.map(lambda _: run(wrapper + ['--version'], consumer, parallel_env), range(2)))
        assert all(v.strip() == expected_version for v in versions)
        assert sha(concurrent/'figlens/npm'/metadata['version']/asset['binary']) == expected['binarySha256']
        assert sha(concurrent/'figlens/npm'/metadata['version']/asset['companion']) == expected['companionSha256']
        assert not list((work/'cache/figlens/npm').glob('.tmp-*'))
        assert not list((concurrent/'figlens/npm').glob('.tmp-*'))
        fixture = consumer/'design.fig'
        shutil.copy2(ROOT/'examples/first-success/design.fig', fixture)
        print(run([sys.executable, ROOT/'scripts/first-success-smoke.py', '--binary', cache, '--example', fixture], consumer, env), end='')
        native_mcp = check_mcp([cache, 'mcp'], consumer, env, fixture, metadata['version'])
        gcs = runpy.run_path(str(ROOT/'scripts/check-gcs-snapshot.py'))['check_gcs'](cache, fixture)
        index = check_index(cache, fixture, consumer, env)
        # OpenCode and OMP launch this cached binary; Pi launches the wrapper's MCP command below.
        runtime = {'native': check_runtime([cache], [cache, 'mcp'], consumer, env),
                   'wrapper': check_runtime(wrapper, wrapper + ['mcp'], consumer, wrapped_env)}
        # Each entry starts its own first use from clean roots, the way a host would launch it.
        first_use = {'nativeMcp': check_first_use([cache, 'mcp'], work/'first-use-native', fixture, env),
                     'wrapperMcp': check_first_use(wrapper + ['mcp'], work/'first-use-wrapper', fixture, wrapped_env)}
        # A full host lane needs the fixture anyway, so it fails without one; a CLI-only or
        # watch-only job (the public design alone) records the skip.
        if not MAPPING.is_file() and (args.opencode_bin or args.omp_sdk) and not args.host_watch_only:
            raise SystemExit(f'{MAPPING} is missing; generate it as wiki/integrations.md says')
        library = lambda command, name, env: (check_library(command, work/name, MAPPING, env) if MAPPING.is_file()
                                              else 'skipped: no samples/plugin-research/fixtures/mapping-library.fig')
        first_use['nativeLibrary'] = library([cache, 'mcp'], 'library-native', env)
        first_use['wrapperLibrary'] = library(wrapper + ['mcp'], 'library-wrapper', wrapped_env)
        # The wrapper, as a second host, reuses the index the native first use enabled.
        first_use['crossHost'] = check_reuse(wrapper + ['mcp'], work/'first-use-native', wrapped_env)
        watch = check_watch(cache, fixture, consumer, env)
        # The installed OpenCode adapter holds a CLI-path lease with that cached binary itself, on this OS.
        # Its waits allow 20 s each, and Windows lists processes slowly: the host checks' budget.
        watch['opencodeHolder'] = json.loads(run([node, ROOT/'scripts/check-opencode-holder.mjs', installed, cache, fixture],
                                                 consumer, env, timeout=240))
        # Cold MCP startup must keep extraction/installation output off its protocol stdout.
        mcp_cache = work/'mcp-cache'
        mcp_env = dict(wrapped_env, XDG_CACHE_HOME=str(mcp_cache), LOCALAPPDATA=str(mcp_cache))
        wrapped_mcp = check_mcp(wrapper + ['mcp'], consumer, mcp_env, fixture, metadata['version'])
        assert sha(mcp_cache/'figlens/npm'/metadata['version']/asset['binary']) == expected['binarySha256']
        assert sha(mcp_cache/'figlens/npm'/metadata['version']/asset['companion']) == expected['companionSha256']
        pi = {'status': 'not requested', 'reason': 'CLI-only job; Pi requires Node >=22.19.0'}
        hosts = {}
        if args.pi_version:
            run(install + [f'@earendil-works/pi-coding-agent@{args.pi_version}'], consumer, env, timeout=240)
            sdk = consumer/'node_modules/@earendil-works/pi-coding-agent'
            pi = json.loads(run([node, ROOT/'scripts/check-pi-integration.mjs', sdk, installed, fixture, consumer], consumer, wrapped_env))
            assert pi['pi'] == args.pi_version and pi['packageVersion'] == metadata['version'], pi
            pi['planning'] = json.loads(run([node, ROOT/'scripts/check-pi-planning.mjs', sdk,
                                            work/'planning', installed/'pi/index.ts'], consumer, env))
            pi['preview'] = json.loads(run([node, ROOT/'scripts/check-pi-preview.mjs', sdk, cache,
                                           fixture, work/'preview', installed/'pi/index.ts'], consumer, env))
            config = pi['mcp']
            pi['stdio'] = check_mcp([config['command'], *config['args']], consumer, wrapped_env, fixture, metadata['version'])
            first_use['piMcp'] = check_first_use([config['command'], *config['args']], work/'first-use-pi', fixture, wrapped_env)
            runtime['piMcp'] = check_runtime(wrapper, [config['command'], *config['args']], consumer, wrapped_env)
            first_use['piLibrary'] = library([config['command'], *config['args']], 'library-pi', wrapped_env)
            # Pi's registered MCP command registers a directory page by page, as an agent follows nextCursor.
            first_use['piDirectory'] = check_directory([config['command'], *config['args']], work/'directory-pi', fixture, wrapped_env)
            # The same command saves a text question over two sources and names the one that changed.
            first_use['piQuestion'] = (check_question([config['command'], *config['args']], work/'question-pi', MAPPING, fixture, wrapped_env)
                                       if MAPPING.is_file() else 'skipped: no samples/plugin-research/fixtures/mapping-library.fig')
        watch_only = [f'--watch-only={fixture}'] if args.host_watch_only else []
        if args.opencode_bin:
            hosts['opencode'] = json.loads(run([node, ROOT/'scripts/check-opencode-integration.mjs',
                args.opencode_bin.resolve(), args.opencode_sdk.resolve(), installed, cache, work/'opencode', '--tui', *watch_only],
                consumer, env, timeout=240))
            if args.pi_version and not watch_only:
                hosts['opencode']['piRoundTrip'] = json.loads(run([node, ROOT/'scripts/check-pi-integration.mjs',
                    sdk, installed, fixture, consumer, work/'opencode/project'], consumer, wrapped_env))['mappingRoundTrip']
        if args.omp_sdk:
            hosts['omp'] = json.loads(run([args.bun.resolve(), ROOT/'scripts/check-omp-integration.mjs',
                args.omp_sdk.resolve(), work/'omp', cache, installed, '--tui', *watch_only], consumer, env, timeout=240))
            if args.pi_version and not watch_only:
                hosts['omp']['piRoundTrip'] = json.loads(run([node, ROOT/'scripts/check-pi-integration.mjs',
                    sdk, installed, fixture, consumer, work/'omp/project 空白'], consumer, wrapped_env))['mappingRoundTrip']
        print(json.dumps({'layer': 'B: candidate artifacts, injected local transport; not a published release',
                          'commit': metadata['commit'], 'dirty': metadata['dirty'], 'sourceDigest': metadata['sourceDigest'],
                          'package': manifest['version'], 'tarballSha256': sha(package), 'binarySha256': sha(cache),
                          'companionSha256': sha(companion), 'runtime': runtime,
                          'node': run([node, '--version'], consumer, env).strip(),
                          'npm': run(npm + ['--version'], consumer, env).strip(),
                          'coldInstall': True, 'warmCacheNoDownload': True, 'parallelFirstUse': True,
                          'nativeMcp': native_mcp, 'wrapperMcp': wrapped_mcp, 'gcsMock': gcs, 'index': index, 'firstUse': first_use, 'watch': watch, 'pi': pi, 'hosts': hosts}, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (AssertionError, OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        # A failed temporary cleanup (on Windows, a file still open) replaces the error behind it.
        context = error.__context__
        cause = ''.join(traceback.format_exception(context)) if context else ''
        timed = error if isinstance(error, subprocess.TimeoutExpired) else context
        if isinstance(timed, subprocess.TimeoutExpired):
            cause += f'{timed.output or ""}\n{timed.stderr or ""}\n'
        raise SystemExit(f'{cause}Installation smoke failed: {error}')
