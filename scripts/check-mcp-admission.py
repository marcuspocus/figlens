#!/usr/bin/env python3
"""Check warm-cache readability through a real MCP child under observable POSIX permissions."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from mcp_smoke import session

ROOT = Path(__file__).resolve().parent.parent


def check(binary):
    if os.name != 'posix':
        raise SystemExit('This permission check requires POSIX; it is not Windows evidence.')
    samples = ROOT / 'samples'
    samples.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='mcp-admission-', dir=samples) as directory:
        work = Path(directory)
        work.chmod(0o755)
        source = work / 'design.fig'
        shutil.copyfile(ROOT / 'examples/first-success/design.fig', source)
        source.chmod(0o644)
        # Docker tests run as root. Drop only this owned child, not the test runner.
        prefix = ('import os; os.setgroups([]); os.setgid(65534); os.setuid(65534); '
                  if os.geteuid() == 0 else '')
        command = [sys.executable, '-c', prefix +
                   'import os,sys; os.execv(sys.argv[1], sys.argv[1:])', str(binary), 'mcp']
        args = {'name': 'nodes', 'arguments': {'file': str(source), 'node': '0:1'}}
        # Inside a Herdr terminal the server would journal into the real state folder.
        env = {k: v for k, v in os.environ.items() if k != 'HC_TERMINAL_ID'}
        with session(command, ROOT, env) as request:
            assert request('tools/call', args)['isError'] is False
            assert request('tools/call', args)['isError'] is False
            stamp = (source.stat().st_mtime_ns, source.stat().st_size)
            source.chmod(0)
            try:
                probe = subprocess.run([sys.executable, '-c', prefix +
                    'import sys; open(sys.argv[1], "rb")', str(source)], capture_output=True, text=True)
                assert probe.returncode != 0 and 'PermissionError' in probe.stderr, probe.stderr
                assert stamp == (source.stat().st_mtime_ns, source.stat().st_size)
                failed = request('tools/call', args)
                assert failed['isError'] and failed['_meta']['figlens']['code'] == 'source_unreadable', failed
                assert request('ping', {}) == {}
            finally:
                source.chmod(0o644)
            recovered = request('tools/call', args)
            assert recovered['isError'] is False, recovered
        return {'warmCachePermissionRevocation': True, 'permissionDenialObserved': True,
                'unchangedMtimeAndLength': True, 'recovery': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    arguments = parser.parse_args()
    print(json.dumps(check(arguments.binary.resolve()), indent=2))
