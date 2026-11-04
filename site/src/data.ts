// Every value here comes from figlens output or the README; nothing is illustrative.
export const REPO = 'https://github.com/runkids/figlens';

export const outcomes = [
  { title: 'Build from the source details', description: 'Read layout, text, styles, variables and Dev Mode annotations; export vector icons as SVG and images as assets. Bundle a frame’s data, assets and reference render, then check for missing files, font requirements and handoff limitations.' },
  { title: 'Compare the running UI', description: 'Compare a screenshot with figlens’s render and get the layers under differing regions. Review those differences and rendering warnings: this is not a pixel-perfect guarantee.' },
  { title: 'Follow design changes', description: 'Compare saved revisions with diff, or use changes and watch to report updates to tracked local scopes. Save a new .fig from Figma to pick up design edits; this is not live Figma sync.' },
];

// An illustrative frame (not a real customer design), drawn in src/scripts/screen.ts.
// box is x, y, w, h inside the 320 × 200 frame; depth is the layer's place in the exploded stack.
export type Layer = { id: string; box: [number, number, number, number]; depth: number; type: string; name: string; values: string };
export const layers: Layer[] = [
  { id: 'frame', box: [0, 0, 320, 200], depth: 0, type: 'FRAME', name: 'Handoff', values: '320×200 #e9e7f3' },
  { id: 'card', box: [16, 16, 288, 168], depth: 1, type: 'FRAME', name: 'Card', values: 'r14 #ffffff' },
  { id: 'logo', box: [32, 32, 96, 32], depth: 5, type: 'IMAGE', name: 'logo.webp', values: '96×32' },
  { id: 'title', box: [32, 82, 200, 28], depth: 4, type: 'TEXT', name: 'Title', values: '22/600 #16151a' },
  { id: 'body', box: [32, 114, 168, 30], depth: 3, type: 'TEXT', name: 'Body', values: '11/400 #55535c' },
  { id: 'button', box: [204, 144, 84, 28], depth: 2, type: 'INSTANCE', name: 'Button', values: '84×28 r7' },
  { id: 'check', box: [260, 32, 28, 28], depth: 6, type: 'VECTOR', name: 'Check', values: '→ check.svg' },
];

export type Host = { id: string; label: string; lines: string[]; note?: string };

// The three hosts the site leads with; every other setup sits under "More hosts".
export const primaryHosts: Host[] = [
  { id: 'claude', label: 'Claude Code', lines: ['claude plugin marketplace add runkids/figlens', 'claude plugin install figlens@figlens'] },
  { id: 'codex', label: 'Codex', lines: ['codex plugin marketplace add runkids/figlens', 'codex plugin add figlens@figlens'] },
  { id: 'pi', label: 'Pi', lines: ['pi install npm:@runkids/figlens'] },
];

// Alternatives, not steps: each gets its own line and its own Copy.
export const cliOptions = [
  { label: 'Homebrew', line: 'brew install runkids/tap/figlens' },
  { label: 'macOS and Linux script', line: 'curl -fsSL https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.sh | sh' },
  { label: 'Windows PowerShell', line: 'irm https://raw.githubusercontent.com/runkids/figlens/main/scripts/install.ps1 | iex' },
  { label: 'Try without installing', line: 'npx @runkids/figlens --help' },
];

export const moreHosts: Host[] = [
  { id: 'cursor', label: 'Cursor', lines: ['git clone --depth 1 https://github.com/runkids/figlens ~/.cursor/plugins/local/figlens'], note: 'Then reload the window.' },
  { id: 'opencode', label: 'OpenCode V2', lines: ['opencode plugin add @runkids/figlens'], note: 'Restart, then run /figlens help.' },
  { id: 'omp', label: 'OMP', lines: ['omp plugin install @runkids/figlens'], note: 'Restart, then run /figlens help.' },
  { id: 'herdr', label: 'Herdr', lines: ['Read ~/.config/herdr-app/AGENTS.md, then add this layout: https://github.com/runkids/figlens/tree/main/herdr/figlens-layout'], note: 'Ask the agent running in Herdr. It adds a figlens panel beside Files and Changes; no figlens install needed.' },
  { id: 'agy', label: 'Antigravity', lines: ['agy plugin install https://github.com/runkids/figlens/tree/main'] },
  { id: 'kimi', label: 'Kimi Code', lines: ['/plugins install https://github.com/runkids/figlens/tree/main'], note: 'Run it in a session, then /reload.' },
  { id: 'skillshare', label: 'skillshare', lines: ['skillshare plugin add https://github.com/runkids/figlens.git --target claude --target codex --target cursor -g'], note: 'Installs the same plugin on several hosts at once.' },
  { id: 'mcp', label: 'Any MCP client', lines: ['{ "mcpServers": { "figlens": { "command": "npx", "args": ["-y", "@runkids/figlens", "mcp"] } } }'], note: 'Node.js 18+. Without Node.js, install the CLI and use "command": "figlens", "args": ["mcp"].' },
];
