#!/usr/bin/env python3
"""Pack the real npm artifact beside freshly built release-format candidate archives."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import zipfile
from installation import environment, run, toolchain

ROOT = Path(__file__).resolve().parent.parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    out = args.directory.resolve()
    if (out/'candidate.json').exists() or list(out.glob('*.tgz')):
        raise SystemExit('Refusing to overwrite an existing candidate package or manifest')
    package = json.loads((ROOT/'npm/package.json').read_text())
    version = package['version']
    assert version == re.search(r'^version = "([^"]+)"', (ROOT/'Cargo.toml').read_text(), re.M)[1]
    sums = {line.split()[1]: line.split()[0] for line in (out/'SHA256SUMS').read_text().splitlines()}
    archives = {}
    for name, expected in sums.items():
        assert Path(name).name == name and name.startswith(f'figlens-{version}-'), name
        artifact = out/name
        assert digest(artifact.read_bytes()) == expected, name
        if name.endswith('.zip'):
            with zipfile.ZipFile(artifact) as archive:
                binary, companion = archive.read('figlens.exe'), archive.read('figlens-embed.exe')
        else:
            with tarfile.open(artifact) as archive:
                binary, companion = (archive.extractfile(n).read() for n in ['figlens', 'figlens-embed'])
        archives[name] = {'sha256': expected, 'binarySha256': digest(binary), 'companionSha256': digest(companion)}
    assert archives, 'No candidate archives'
    with tempfile.TemporaryDirectory(prefix='figlens-pack-') as work:
        node, npm = toolchain()
        env = environment(Path(work), node)
        stage = Path(work)/'package'
        stage.mkdir()
        for name in ['package.json', 'README.md', 'target.js']:
            shutil.copy2(ROOT/'npm'/name, stage/name)
        shutil.copytree(ROOT/'npm/bin', stage/'bin')
        shutil.copytree(ROOT/'npm/pull', stage/'pull')
        shutil.copy2(ROOT/'LICENSE', stage/'LICENSE')
        run([node, ROOT/'scripts/prepare-npm.cjs', stage], ROOT, env)
        packed = json.loads(run(npm + ['pack', '--ignore-scripts', '--offline', '--json', '--pack-destination', str(out)], stage, env))[0]['filename']
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard'], cwd=ROOT, text=True).splitlines()
    inputs = {p: digest((ROOT/p).read_bytes()) for p in sorted(paths) if (ROOT/p).is_file()}
    metadata = {
        'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'dirty': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True)),
        'sourceDigest': digest(json.dumps(inputs, sort_keys=True).encode()),
        'version': version, 'package': {'file': packed, 'sha256': digest((out/packed).read_bytes())},
        'archives': archives,
    }
    with (out/'candidate.json').open('x') as output:
        json.dump(metadata, output, indent=2)
    print(json.dumps(metadata, indent=2))


if __name__ == '__main__':
    main()
