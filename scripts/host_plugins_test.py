"""Host plugin manifests and install docs stay in lockstep with the npm package."""
import json
import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HOST_FILES = ['.claude-plugin/plugin.json', '.claude-plugin/marketplace.json', '.codex-plugin/plugin.json',
              '.codex-plugin/mcp.json', '.agents/plugins/marketplace.json', '.cursor-plugin/plugin.json',
              '.kimi-plugin/plugin.json', '.antigravity-plugin/plugin.json', 'mcp_config.json']
# Every tracked Markdown file; CHANGELOG.md records past releases, not install commands.
DOC_FILES = [p for p in subprocess.run(['git', 'ls-files', '*.md'], cwd=ROOT, capture_output=True, text=True,
                                       check=True).stdout.split('\n') if p and p != 'CHANGELOG.md']


def load(path):
    return json.loads((ROOT / path).read_text())


def resolve(data, jsonpath):
    # The release config only uses `$.key.key[0]` paths.
    for key, index in re.findall(r'\.([^.\[]+)(?:\[(\d+)\])?', jsonpath):
        data = data[key]
        if index:
            data = data[int(index)]
    return data


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


class HostPluginsTest(unittest.TestCase):
    version = load('npm/package.json')['version']
    extra = load('.github/release-please-config.json')['packages']['.']['extra-files']

    def test_release_bumps_every_version_and_pinned_package(self):
        bumped = {(e['path'], e['jsonpath']) for e in self.extra if e['type'] == 'json'}
        for path, jsonpath in bumped:
            value = resolve(load(path), jsonpath)
            self.assertIn(value, (self.version, f'v{self.version}', f'@runkids/figlens@{self.version}'), f'{path} {jsonpath}')
        for path in HOST_FILES:
            data = load(path)
            pinned = [s for s in strings(data) if s.startswith('@runkids/figlens')]
            versions = ['version' in data, *('version' in p for p in data.get('plugins', []))]
            paths = {jp for p, jp in bumped if p == path}
            self.assertEqual(len(paths), len(pinned) + sum(versions), f'{path}: an unbumped version or package')

    def test_registry_entry_names_the_npm_package(self):
        # The MCP Registry reads mcpName from the published package and rejects a different name.
        server, package = load('server.json'), load('npm/package.json')
        self.assertEqual((server['name'], server['packages'][0]['identifier']), (package['mcpName'], package['name']))

    def test_manifests_point_at_existing_paths(self):
        for path in HOST_FILES:
            data = load(path)
            for key in ('skills', 'mcpServers'):
                if isinstance(data.get(key), str):
                    self.assertTrue((ROOT / data[key]).exists(), f'{path} {key}')
        self.assertTrue((ROOT / 'skills/figlens/SKILL.md').is_file())

    def test_marketplaces_install_from_the_repository_root(self):
        # The relative form is what other plugin repositories and tools such as Skillshare read.
        for path in ('.claude-plugin/marketplace.json', '.agents/plugins/marketplace.json'):
            self.assertEqual(load(path)['plugins'][0]['source'], './', path)

    def test_docs_do_not_pin_a_version(self):
        # A release edits no doc: install commands follow the latest package and default branch.
        pinned = re.compile(r'@runkids/figlens@\d|figlens/tree/v\d|--branch v\d|x-release-please')
        for path in DOC_FILES:
            for number, line in enumerate((ROOT / path).read_text().split('\n'), 1):
                self.assertIsNone(pinned.search(line), f'{path}:{number} pins a version')
        # generic rewrites a marked line: only code may carry one (the Herdr mod's engine.js), never a doc.
        self.assertFalse([e for e in self.extra if e['type'] == 'generic' and not e['path'].endswith('.js')])


if __name__ == '__main__':
    unittest.main()
