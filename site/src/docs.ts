// Docs pages are cut at build time from the repository itself, so the site never drifts from it:
// a `<a id>` + `## heading` section of a README, a whole integration README, or the CLI's own
// `figlens --help` text read from src/main.rs.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { Marked } from 'marked';
import { REPO } from './data';
import { LOCALES, LABELS, DESCRIPTIONS, type Locale } from './i18n';

export const SITE = 'https://figlens.runkids.cc';

// The build runs in site/ (locally and in site-pages.yml); the sources live one level up.
const ROOT = resolve(process.cwd(), '..');

type Plan = { slug: string; group: string; file: string; anchor?: string; usage?: true; label?: string; description: string };
const PLAN: Plan[] = [
  { slug: '', group: 'Start here', file: 'README.md', anchor: 'start', description: 'Put design.fig in your project and pick one setup: a plugin for Claude Code, Codex, Antigravity, Cursor or Kimi Code, MCP, Pi, OpenCode V2, OMP or the CLI.' },
  { slug: 'why', label: 'Why figlens', group: 'Start here', file: 'docs/README.md', anchor: 'why', description: 'Use figlens when the design should travel as a file: remote agents, CI, offline work, agent handoff and unlimited reads. When the official Figma MCP fits better.' },
  { slug: 'install', label: 'Install the CLI', group: 'Start here', file: 'docs/README.md', anchor: 'install', description: 'Install the standalone figlens binary with the macOS and Linux script, Homebrew or PowerShell, then point any MCP client at figlens mcp.' },
  { slug: 'what-the-agent-gets', group: 'Using figlens', file: 'docs/README.md', anchor: 'bundle', description: 'One frame, one zip: structure to build from, assets to use and a reference render to check against, plus how close figlens output is to Figma exports.' },
  { slug: 'mcp', group: 'Using figlens', file: 'docs/README.md', anchor: 'mcp-and-skill', description: 'The local stdio MCP server, its tools, and the figlens skill that teaches an agent to find frames, read the bundle, plan scoped work and verify the running UI.' },
  { slug: 'viewer', label: 'Local viewer', group: 'Using figlens', file: 'docs/README.md', anchor: 'viewer', description: 'A local .fig viewer: browse pages and layers, search, measure, inspect layout, typography, variables and CSS, export layers or write an offline HTML snapshot.' },
  { slug: 'search', label: 'Search and the local index', group: 'Using figlens', file: 'docs/README.md', anchor: 'local-index', description: 'Ordinary search reads the .fig and creates nothing. An optional local index, made only when you ask, ranks across sources and finds exact usages.' },
  { slug: 'semantic-search', label: 'Semantic search', group: 'Using figlens', file: 'docs/README.md', anchor: 'semantic-search', description: 'Experimental semantic, hybrid and image search with local models you download and configure yourself. The default text search needs no model.' },
  { slug: 'pi', label: 'Pi', group: 'Integrations', file: 'pi/README.md', description: 'Build Figma screens in Pi from a local .fig: install, ask for a screen, and the /figlens plan, yolo, preview and component mapping commands.' },
  { slug: 'opencode', label: 'OpenCode V2', group: 'Integrations', file: 'opencode/README.md', description: 'Install figlens in OpenCode V2 to search designs, find usages, follow changes, search by meaning or image and compare revisions, and see what each action asks.' },
  { slug: 'omp', label: 'OMP', group: 'Integrations', file: 'omp/README.md', description: 'Install the native figlens OMP extension, use /figlens preview, open, frame, status, review and close, and ask the agent to search and follow changes.' },
  { slug: 'cli', group: 'Reference', file: 'docs/README.md', anchor: 'commands', description: 'Every figlens command and what it prints: tree, find, search, usages, diff, inspect, nodes, variables, bundle, verify, export, preview, index, watch and more.' },
  { slug: 'usage', group: 'Reference', file: 'src/main.rs', usage: true, description: 'The complete figlens --help text, command by command, with every flag, default and limit, read from the CLI source.' },
  { slug: 'limitations', group: 'Reference', file: 'README.md', anchor: 'limitations', description: 'What figlens does not do: snapshots instead of live sync, an undocumented format, fonts you supply, and output you still need to check.' },
];
// Sections of docs/README.md that now live on a fuller page.
const ALIASES: Record<string, string> = { 'docs/README.md#pi': '/docs/pi/' };

// A translated page reads the same section from README-<loc>.md or docs/README-<loc>.md. Pages with no
// translation (the integration READMEs and the CLI help) stay in the nav and link to the English page,
// except Pi, whose translated docs section stands in for pi/README.md.
function planFor(lang: Locale): (Plan & { englishOnly?: true })[] {
  if (lang === 'en') return PLAN;
  return PLAN.map((p) => {
    const t = { ...p, label: LABELS[lang][p.slug] ?? p.label, description: DESCRIPTIONS[lang][p.slug] ?? p.description };
    if (p.slug === 'pi') return { ...t, file: `docs/README-${lang}.md`, anchor: 'pi' };
    if (p.usage || !p.anchor) return { ...t, englishOnly: true as const };
    return { ...t, file: p.file.replace(/README\.md$/, `README-${lang}.md`) };
  });
}
export const hrefOf = (lang: Locale, slug: string) => `${lang === 'en' ? '' : `/${lang}`}/docs/${slug ? `${slug}/` : ''}`;

export type DocPage = { lang: Locale; slug: string; href: string; group: string; title: string; label: string; description: string; file: string; source: string; html: string; markdown: string; toc: { id: string; text: string }[]; englishOnly?: true };

const ANCHOR = /<a id="([^"]+)"><\/a>/;
const isFence = (line: string) => /^\s*(```|~~~)/.test(line);
const read = (file: string) => readFileSync(resolve(ROOT, file), 'utf8');

function cutSection(lines: string[], anchor: string) {
  const start = lines.findIndex((l) => l.trim() === `<a id="${anchor}"></a>`);
  if (start < 0) throw new Error(`docs: anchor "${anchor}" not found`);
  const h2 = lines.findIndex((l, i) => i > start && l.startsWith('## '));
  let end = lines.length;
  let fenced = false;
  for (let i = h2 + 1; i < lines.length; i++) {
    if (isFence(lines[i])) fenced = !fenced;
    if (!fenced && lines[i].startsWith('## ')) { end = i; break; }
  }
  while (end > h2 + 1 && (lines[end - 1].trim() === '' || /^<a id="[^"]+"><\/a>\s*$/.test(lines[end - 1]))) end--;
  // Demote one level: the section's ## becomes the page h1, its ### the page's h2.
  fenced = false;
  const body = lines.slice(h2 + 1, end).map((l) => {
    if (isFence(l)) fenced = !fenced;
    return !fenced && /^#{3,6} /.test(l) ? l.slice(1) : l;
  });
  return { title: lines[h2].slice(3).trim(), body, from: start, to: end };
}

function wholeFile(lines: string[]) {
  const h1 = lines.findIndex((l) => l.startsWith('# '));
  return { title: lines[h1].slice(2).replace(/`/g, '').trim(), body: lines.slice(h1 + 1), from: 0, to: lines.length };
}

// figlens --help: the USAGE string literal in src/main.rs, one section per command.
function usage() {
  const src = read('src/main.rs');
  const open = 'const USAGE: &str = "';
  const i = src.indexOf(open) + open.length;
  let j = i;
  for (;;) {
    j = src.indexOf('"', j);
    let k = j - 1;
    while (src[k] === '\\') k--;
    if ((j - 1 - k) % 2 === 0) break;
    j++;
  }
  const text = src.slice(i, j).replace(/\\(["\\nt])/g, (_, c) => ({ n: '\n', t: '\t' } as Record<string, string>)[c] ?? c);
  const lines = text.split('\n');
  const u = lines.indexOf('usage:');
  let end = lines.findIndex((l, n) => n > u && l.trim() !== '' && !l.startsWith(' '));
  if (end < 0) end = lines.length;
  const groups = new Map<string, string[]>();
  let current = '';
  for (const l of lines.slice(u + 1, end)) {
    const m = l.match(/^ {2}figlens (\S+)/);
    if (m) current = m[1] === '--help' ? 'help' : m[1];
    if (!current) continue;
    if (!groups.has(current)) groups.set(current, []);
    groups.get(current)!.push(l.slice(2));
  }
  const fence = (body: string[]) => ['```text', ...body.map((l) => l.replace(/\s+$/, '')), '```'];
  const esc = (t: string) => t.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  // Each entry is a syntax line plus `[...]` continuation lines; text after a run of 3+ spaces, and
  // every other line, is the entry's description.
  const entries = (ls: string[]) => {
    const out: { syntax: string[]; desc: string[] }[] = [];
    for (const l of ls) {
      const t = l.trim();
      if (!t) continue;
      const head = l.startsWith('figlens ');
      if (!head && !out.length) continue;
      if (head) out.push({ syntax: [], desc: [] });
      const e = out[out.length - 1];
      const opts = t.split(/\s{3,}/)[0];
      if (head || (l.search(/\S/) < 30 && /^(\[|--)/.test(t)) || /^(\[[^\]]*\]\s*)+$/.test(opts)) {
        const [syntax, ...rest] = t.split(/\s{3,}/);
        e.syntax.push(syntax);
        if (rest.length) e.desc.push(rest.join(' '));
      } else e.desc.push(t);
    }
    return out.flatMap((e) => ['```sh', ...e.syntax, '```', '', e.desc.length ? `<p class="usage-desc">${esc(e.desc.join(' '))}</p>` : '', '']);
  };
  const body = [
    lines[0],
    '',
    'This is the text of `figlens --help`, taken from the CLI source when this site was built.',
    '',
    ...[...groups].flatMap(([cmd, ls]) => [`## ${cmd}`, '', ...entries(ls)]),
    '## Output and examples',
    '',
    ...fence(lines.slice(end)),
  ];
  return { title: 'Command usage', body, from: 0, to: 0 };
}

const slugify = (s: string) => s.toLowerCase().replace(/<[^>]+>|`/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');

let cache: Map<Locale, DocPage[]> | undefined;
export function loadDocs(lang: Locale = 'en'): DocPage[] {
  cache ??= loadAll();
  return cache.get(lang)!;
}

function loadAll() {
  const text = new Map<string, string[]>();
  const linesOf = (file: string) => {
    if (!text.has(file)) text.set(file, read(file).split('\n'));
    return text.get(file)!;
  };
  const cut = LOCALES.flatMap((lang) => planFor(lang).map((p) => {
    if (p.englishOnly) return { ...p, lang, href: hrefOf('en', p.slug), title: p.label ?? p.slug, body: [] as string[], from: 0, to: 0 };
    const part = p.usage ? usage() : p.anchor ? cutSection(linesOf(p.file), p.anchor) : wholeFile(linesOf(p.file));
    return { ...p, lang, href: hrefOf(lang, p.slug), ...part };
  }));
  const rendered = cut.filter((c) => !c.englishOnly);
  // A section's own anchor is cut with its heading, so a link to it opens the page itself.
  const tops = new Set(rendered.filter((c) => c.anchor).map((c) => `${c.file}#${c.anchor}`));

  // Every anchor in a source file points at the page whose section holds it; whole files map to their page.
  const owner = new Map<string, string>(Object.entries(ALIASES));
  const wholePage = new Map<string, string>();
  for (const c of rendered) {
    if (c.usage) continue;
    if (!c.anchor) wholePage.set(c.file, c.href);
    linesOf(c.file).forEach((l, i) => {
      const m = l.match(ANCHOR);
      if (m && i >= c.from && i < c.to) owner.set(`${c.file}#${m[1]}`, c.href);
    });
  }

  const link = (file: string, href: string, here: string) => {
    if (/^[a-z]+:/i.test(href) || href.startsWith('//')) return href;
    if (href.startsWith('#') && file === 'src/main.rs') return href;
    const url = new URL(href, `https://repo/${file}`);
    const path = url.pathname.slice(1);
    const hash = url.hash.slice(1);
    const docsHome = path.match(/^docs\/README(?:-(.+))?\.md$/);
    const page = (hash && owner.get(`${path}#${hash}`)) || (!hash || wholePage.has(path) ? wholePage.get(path) ?? (docsHome ? hrefOf((docsHome[1] ?? 'en') as Locale, '') : undefined) : undefined);
    if (page && (ALIASES[`${path}#${hash}`] || tops.has(`${path}#${hash}`))) return page;
    if (page) {
      const target = hash ? `${page}#${hash}` : page;
      return page === here ? (hash ? `#${hash}` : page) : target;
    }
    return `${REPO}/blob/main/${path}${url.hash}`;
  };
  const asset = (file: string, src: string) => (/^[a-z]+:/i.test(src) ? src : `https://raw.githubusercontent.com/runkids/figlens/main/${new URL(src, `https://repo/${file}`).pathname.slice(1)}`);

  const pages = cut.map((c): DocPage => {
    const base = { lang: c.lang, slug: c.slug, href: c.href, group: c.group, title: c.title, label: c.label ?? c.title, description: c.description, file: c.file };
    if (c.englishOnly) return { ...base, source: '', html: '', markdown: '', toc: [], englishOnly: true };
    const md = c.body.join('\n');
    const toc: DocPage['toc'] = [];
    const marked = new Marked({ gfm: true });
    marked.use({
      walkTokens(token) {
        if (token.type === 'link') token.href = link(c.file, token.href, c.href);
        if (token.type === 'image') token.href = asset(c.file, token.href);
        if (token.type === 'html') {
          token.text = token.text
            .replace(/href="([^"]+)"/g, (_: string, h: string) => `href="${link(c.file, h, c.href)}"`)
            .replace(/src="([^"]+)"/g, (_: string, s: string) => `src="${asset(c.file, s)}"`);
        }
      },
      renderer: {
        heading({ tokens, depth, text }) {
          const id = slugify(text);
          if (depth === 2) toc.push({ id, text: text.replace(/`/g, '') });
          return `<h${depth} id="${id}">${this.parser.parseInline(tokens)}</h${depth}>\n`;
        },
      },
    });
    const html = marked.parse(md, { async: false }) as string;
    // A Markdown copy for llms-full.txt, with links made absolute.
    const markdown = md.replace(/\]\(([^)\s]+)\)/g, (_, h) => {
      const to = link(c.file, h, '');
      return `](${to.startsWith('/') ? SITE + to : to.startsWith('#') ? SITE + c.href + to : to})`;
    });
    const source = c.usage ? `${REPO}/blob/main/src/main.rs` : `${REPO}/blob/main/${c.file}${c.anchor ? `#${c.anchor}` : ''}`;
    return { ...base, source, html, markdown, toc };
  });
  return new Map(LOCALES.map((lang) => [lang, pages.filter((p) => p.lang === lang)]));
}

// The same page in every language that has it, for hreflang and the language menu.
export function alternatesOf(page: DocPage) {
  return LOCALES.flatMap((lang) => {
    const twin = loadDocs(lang).find((p) => p.slug === page.slug && !p.englishOnly);
    return twin ? [{ lang, href: twin.href }] : [];
  });
}
