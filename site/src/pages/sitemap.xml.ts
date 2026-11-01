import { loadDocs, alternatesOf, SITE } from '../docs';
import { LOCALES } from '../i18n';

// Every page, with its translations as hreflang alternates.
export function GET() {
  const docs = LOCALES.flatMap((lang) => loadDocs(lang).filter((p) => !p.englishOnly));
  const entry = (loc: string, alts: { lang: string; href: string }[]) => {
    const links = alts.length > 1 ? alts.map((a) => `\n    <xhtml:link rel="alternate" hreflang="${a.lang}" href="${SITE}${a.href}"/>`).join('') : '';
    return `  <url>\n    <loc>${SITE}${loc}</loc>${links}\n  </url>`;
  };
  const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${[entry('/', []), ...docs.map((p) => entry(p.href, alternatesOf(p)))].join('\n')}\n</urlset>\n`;
  return new Response(body, { headers: { 'Content-Type': 'application/xml; charset=utf-8' } });
}
