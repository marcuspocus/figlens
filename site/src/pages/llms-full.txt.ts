// Every docs page as Markdown in one file, for language models that read a whole site at once.
import { loadDocs, SITE } from '../docs';

export function GET() {
  const body = ['# figlens documentation', '', `Source: ${SITE}/docs/`, '']
    .concat(loadDocs().flatMap((p) => [`# ${p.title}`, '', `URL: ${SITE}${p.href}`, '', p.markdown.trim(), '']))
    .join('\n');
  return new Response(body, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
