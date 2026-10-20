// Pre-rendering of the pages for dist/: one set of pages per language, with every text already
// in the HTML. A page then shows its text before any script runs (no empty buttons while the
// modules load), and search engines and link previews see real content.
//
//   dist/index.html send.html recv.html        English (x-default)
//   dist/zh/index.html send.html recv.html     Chinese
//
// The templates are the pages in src/web: elements carry data-i18n (text), data-i18n-ph
// (placeholder), data-i18n-title (title, aria-label), data-i18n-content (meta content) and
// data-i18n-href (a link to a document in the page's language), and
// js/i18n.js holds the dictionary. A visitor whose saved choice (or, on the English pages, whose
// browser) is the other language is sent to the other page by a small script in <head>, before
// anything is drawn. With a site URL, the pages also get canonical and Open Graph tags, and
// sitemap.xml lists them.
import fs from 'node:fs';
import path from 'node:path';
import { DICT } from '../web/js/i18n.js';

export const PAGES = ['index.html', 'send.html', 'recv.html'];
export const LANGS = [
  { code: 'en', hreflang: 'en', dir: '' },
  { code: 'zh', hreflang: 'zh-CN', dir: 'zh/' },
];

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const text = (lang, key) => {
  const v = DICT[lang][key] ?? DICT.en[key];
  if (v === undefined) throw new Error(`prerender: no text for "${key}"`);
  return v;
};

// Where `page` of language `to` is, seen from a page of language `from`.
function link(page, from, to) {
  const file = page === 'index.html' ? '' : page;
  if (from.code === to.code) return file || './';
  return from.dir ? `../${file}` : `${to.dir}${file}`;
}
// The public address of a page. Hosts such as Cloudflare Pages redirect /page.html to /page, so
// the address search engines should keep is the one without .html.
const absolute = (site, page, lang) => `${site}/${lang.dir}${page === 'index.html' ? '' : page.replace(/\.html$/, '')}`;

// Runs in <head> before anything is drawn: open the page in the language the visitor chose
// before. Without a choice, only the English pages follow a Chinese browser (the Chinese pages
// never send anyone away, so that crawlers can read them).
const REDIRECT = `<script>
      (function () {
        try {
          var here = document.documentElement.lang.indexOf('zh') === 0 ? 'zh' : 'en';
          var want = localStorage.getItem('lumen.lang');
          if (!want && here === 'en' && (navigator.language || '').toLowerCase().indexOf('zh') === 0) want = 'zh';
          if (want && want !== here) {
            var other = document.querySelector('link[rel="alternate"][hreflang="' + (want === 'zh' ? 'zh-CN' : 'en') + '"]');
            if (other) location.replace(new URL(other.getAttribute('data-local'), location.href).href + location.search + location.hash);
          }
        } catch (e) {}
      })();
    </script>`;

// Static imports of a module, recursively: the files to preload so that they download in
// parallel instead of one level of imports after the other.
function importGraph(dist, entries) {
  const seen = new Set();
  const visit = (rel) => {
    if (seen.has(rel)) return;
    seen.add(rel);
    const src = fs.readFileSync(path.join(dist, rel), 'utf8');
    for (const m of src.matchAll(/\bfrom\s*['"](\.{1,2}\/[\w./-]+\.js)['"]/g)) visit(path.posix.normalize(path.posix.join(path.posix.dirname(rel), m[1])));
  };
  entries.forEach(visit);
  return [...seen];
}

function renderPage(tpl, page, lang, { dist, site }) {
  let h = tpl;
  const t = (key) => esc(text(lang.code, key));

  // text, placeholders, titles, meta content
  h = h.replace(/(<([a-zA-Z][\w-]*)\b[^>]*?\sdata-i18n="(\w+)"[^>]*>)[^<]*(<\/\2>)/g, (m, open, tag, key, close) => open + t(key) + close);
  h = h.replace(/\sdata-i18n-ph="(\w+)"/g, (m, key) => `${m} placeholder="${t(key)}"`);
  h = h.replace(/\sdata-i18n-title="(\w+)"/g, (m, key) => `${m} title="${t(key)}" aria-label="${t(key)}"`);
  h = h.replace(/\sdata-i18n-content="(\w+)" content="[^"]*"/g, (m, key) => ` data-i18n-content="${key}" content="${t(key)}"`);
  h = h.replace(/\shref="[^"]*"(\sdata-i18n-href="(\w+)")/g, (m, attr, key) => ` href="${t(key)}"${attr}`);
  h = h.replace(/<html lang="[^"]*">/, `<html lang="${lang.hreflang}" data-prerendered>`);

  // a page in a subdirectory reaches the shared files one level up
  const up = lang.dir ? '../' : '';
  if (up) {
    h = h.replace(/(\s(?:href|src)=")((?:css|js|docs|img|wasm)\/)/g, `$1${up}$2`);
    h = h.replace(/(\bfrom\s*['"])\.\/js\//g, `$1../js/`);
  }

  // <head>: language versions and the redirect first, then canonical, previews and preloads
  // href: the public address, for search engines; data-local: the same page on this host, for
  // the redirect and the language button (a copy on another host stays there)
  const alt = (hreflang, l) => `<link rel="alternate" hreflang="${hreflang}" href="${site ? absolute(site, page, l) : link(page, lang, l)}" data-local="${link(page, lang, l)}" />`;
  const alternates = [...LANGS.map((l) => alt(l.hreflang, l)), alt('x-default', LANGS[0])];
  const head = [...alternates, REDIRECT];
  h = h.replace(/(<meta name="viewport"[^>]*>)/, `$1\n    ${head.join('\n    ')}`);

  const key = { 'index.html': 'Home', 'send.html': 'Send', 'recv.html': 'Recv' }[page];
  const tail = [];
  if (site) {
    const url = absolute(site, page, lang);
    tail.push(`<link rel="canonical" href="${url}" />`);
    tail.push(`<meta property="og:type" content="website" />`);
    tail.push(`<meta property="og:site_name" content="Lumen Code" />`);
    tail.push(`<meta property="og:title" content="${t(`title${key}`)}" />`);
    tail.push(`<meta property="og:description" content="${t(`desc${key}`)}" />`);
    tail.push(`<meta property="og:url" content="${url}" />`);
    tail.push(`<meta property="og:locale" content="${lang.code === 'zh' ? 'zh_CN' : 'en_US'}" />`);
    if (fs.existsSync(path.join(dist, `img/social-${lang.code}.png`))) {
      tail.push(`<meta property="og:image" content="${site}/img/social-${lang.code}.png" />`);
      tail.push(`<meta property="og:image:width" content="1200" />`);
      tail.push(`<meta property="og:image:height" content="630" />`);
      tail.push(`<meta name="twitter:card" content="summary_large_image" />`);
    }
    if (page === 'index.html') {
      const app = {
        '@context': 'https://schema.org',
        '@type': 'SoftwareApplication',
        name: 'Lumen Code',
        description: text(lang.code, 'descHome'),
        url,
        applicationCategory: 'UtilitiesApplication',
        operatingSystem: 'Web, macOS, Windows, Linux, iOS, Android',
        license: 'https://opensource.org/licenses/MIT',
        offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
      };
      tail.push(`<script type="application/ld+json">${JSON.stringify(app)}</script>`);
    }
  }
  // the page's modules, fetched in parallel
  const entries = [...h.matchAll(/<script type="module" src="(?:\.\.\/)?(js\/[\w./-]+\.js)"/g)].map((m) => m[1]);
  for (const m of h.matchAll(/<script type="module">([\s\S]*?)<\/script>/g))
    for (const i of m[1].matchAll(/\bfrom\s*['"](?:\.\.?\/)(js\/[\w./-]+\.js)['"]/g)) entries.push(i[1]);
  for (const rel of importGraph(dist, entries)) tail.push(`<link rel="modulepreload" href="${up}${rel}" />`);
  h = h.replace(/(\n\s*)<\/head>/, `$1  ${tail.join('$1  ')}$1</head>`);
  return h;
}

export function prerender(dist, { site = '' } = {}) {
  site = site.replace(/\/+$/, '');
  const templates = Object.fromEntries(PAGES.map((p) => [p, fs.readFileSync(path.join(dist, p), 'utf8')]));
  for (const lang of LANGS) {
    fs.mkdirSync(path.join(dist, lang.dir), { recursive: true });
    for (const page of PAGES) fs.writeFileSync(path.join(dist, lang.dir, page), renderPage(templates[page], page, lang, { dist, site }));
  }
  const robots = ['User-agent: *', 'Allow: /'];
  if (site) {
    const urls = PAGES.flatMap((page) =>
      LANGS.map((lang) => {
        const alts = LANGS.map((l) => `    <xhtml:link rel="alternate" hreflang="${l.hreflang}" href="${absolute(site, page, l)}" />`).join('\n');
        return `  <url>\n    <loc>${absolute(site, page, lang)}</loc>\n${alts}\n  </url>`;
      }),
    );
    fs.writeFileSync(
      path.join(dist, 'sitemap.xml'),
      `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls.join('\n')}\n</urlset>\n`,
    );
    robots.push(`Sitemap: ${site}/sitemap.xml`);
  }
  fs.writeFileSync(path.join(dist, 'robots.txt'), robots.join('\n') + '\n');
  return { pages: PAGES.length * LANGS.length, site: site || null };
}
