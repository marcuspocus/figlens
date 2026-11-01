// Stage maintained host integrations into the existing CLI package for one release.
const { copyFileSync, mkdirSync } = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const output = process.argv[2] ? path.resolve(process.argv[2]) : path.join(root, 'npm');
for (const [directory, files] of Object.entries({
  pi: ['index.ts', 'workflow.mjs', 'viewer.mjs', 'requests.mjs', 'stale.mjs', 'package.json', 'README.md'],
  omp: ['index.ts', 'review.mjs', 'mcp.cjs', 'package.json', 'README.md'],
  opencode: ['index.mjs', 'tui.mjs', 'adapter.mjs', 'rpc.mjs', 'review-rpc.mjs', 'review-ui.mjs', 'watch-holder.mjs', 'package.json', 'README.md'],
})) {
  mkdirSync(path.join(output, directory), { recursive: true });
  for (const file of files) copyFileSync(path.join(root, directory, file), path.join(output, directory, file));
}
copyFileSync(path.join(root, 'omp/mcp.json'), path.join(output, '.mcp.json'));
for (const file of ['SKILL.md', 'references/audit.md', 'references/planning.md', 'references/planning-details.md', 'references/yolo.md', 'references/search.md']) {
  const destination = path.join(output, 'skills/figlens', file);
  mkdirSync(path.dirname(destination), { recursive: true });
  copyFileSync(path.join(root, 'skills/figlens', file), destination);
}
