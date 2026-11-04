// llms.txt (https://llmstxt.org): a short map of the site for language models.
import { loadDocs, hrefOf, SITE } from '../docs';
import { LOCALES, LANG_NAMES } from '../i18n';

export function GET() {
  const pages = loadDocs();
  const groups = [...new Set(pages.map((p) => p.group))];
  const body = [
    '# figlens',
    '',
    '> figlens reads Figma .fig files offline for coding agents: layout, text, styles, variables, assets and a reference render, from a saved local copy. No Figma API, login, Dev Mode seat or running Figma app. Native CLI, MCP server, and plugins for Claude Code, Codex, Pi, Cursor, OpenCode and OMP, and a Herdr panel. MIT licensed.',
    '',
    `The full documentation as one file: ${SITE}/llms-full.txt`,
    '',
    ...groups.flatMap((g) => [`## ${g}`, '', ...pages.filter((p) => p.group === g).map((p) => `- [${p.title}](${SITE}${p.href}): ${p.description}`), '']),
    '## Translations',
    '',
    ...LOCALES.filter((l) => l !== 'en').map((l) => `- [${LANG_NAMES[l]}](${SITE}${hrefOf(l, '')}): the docs in ${l}`),
    '',
    '## Optional',
    '',
    '- [Source code](https://github.com/runkids/figlens)',
    '- [Releases](https://github.com/runkids/figlens/releases)',
    '',
  ].join('\n');
  return new Response(body, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
