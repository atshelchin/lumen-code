// Every page in both languages, each with its language versions.
import { LANGS, htmlLang, path, type Lang } from '#lib/i18n';

export const prerender = true;

const PAGES = ['', 'send', 'recv', 'privacy', 'terms'] as const;

export function GET() {
	const abs = (lang: Lang, page: (typeof PAGES)[number]) => `${__SITE__}${path(lang, page)}`;
	const urls = PAGES.flatMap((page) =>
		LANGS.map((lang) => {
			const alts = LANGS.map(
				(l) => `    <xhtml:link rel="alternate" hreflang="${htmlLang(l)}" href="${abs(l, page)}" />`
			);
			alts.push(
				`    <xhtml:link rel="alternate" hreflang="x-default" href="${abs('en', page)}" />`
			);
			return `  <url>\n    <loc>${abs(lang, page)}</loc>\n${alts.join('\n')}\n  </url>`;
		})
	);
	const xml = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls.join('\n')}\n</urlset>\n`;
	return new Response(xml, { headers: { 'content-type': 'application/xml; charset=utf-8' } });
}
