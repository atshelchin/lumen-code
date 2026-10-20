// The built site (dist/): every page of every language has its text in the HTML, links that
// resolve, and the tags that tell browsers and search engines about the language versions.
//   npm run build && node test/test-site.js
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DICT } from '../src/web/js/i18n.js';
import { PAGES, LANGS } from '../src/tools/prerender.js';

const dist = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../dist');
let failures = 0;
let checks = 0;
const expect = (cond, what) => {
  checks++;
  if (!cond) {
    failures++;
    console.log('FAIL', what);
  }
};
const unesc = (s) => s.replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');

for (const lang of LANGS) {
  for (const page of PAGES) {
    const where = `${lang.dir}${page}`;
    const file = path.join(dist, lang.dir, page);
    expect(fs.existsSync(file), `${where} exists`);
    if (!fs.existsSync(file)) continue;
    const h = fs.readFileSync(file, 'utf8');

    expect(new RegExp(`<html lang="${lang.hreflang}" data-prerendered>`).test(h), `${where}: lang ${lang.hreflang}`);
    // every translatable text is in the HTML, and it is the dictionary's text
    let n = 0;
    for (const m of h.matchAll(/<([a-zA-Z][\w-]*)\b[^>]*?\sdata-i18n="(\w+)"[^>]*>([^<]*)<\/\1>/g)) {
      n++;
      expect(unesc(m[3]) === DICT[lang.code][m[2]], `${where}: text of ${m[2]}`);
    }
    expect(n === (h.match(/\sdata-i18n="/g) || []).length, `${where}: every data-i18n element holds plain text (${n})`);
    for (const m of h.matchAll(/data-i18n-ph="(\w+)" placeholder="([^"]*)"/g)) expect(unesc(m[2]) === DICT[lang.code][m[1]], `${where}: placeholder ${m[1]}`);
    expect((h.match(/data-i18n-ph="/g) || []).length === (h.match(/data-i18n-ph="\w+" placeholder="/g) || []).length, `${where}: every placeholder filled`);
    expect((h.match(/data-i18n-title="/g) || []).length === (h.match(/data-i18n-title="\w+" title="[^"]+" aria-label="/g) || []).length, `${where}: every title filled`);
    const title = h.match(/<title[^>]*>([^<]+)<\/title>/);
    const desc = h.match(/<meta name="description"[^>]*content="([^"]+)"/);
    expect(title && title[1].length > 10, `${where}: title`);
    expect(desc && desc[1].length > 40, `${where}: meta description`);

    // the language versions name each other
    for (const hl of ['en', 'zh-CN', 'x-default']) {
      const m = h.match(new RegExp(`<link rel="alternate" hreflang="${hl}" href="[^"]+" data-local="([^"]+)"`));
      expect(!!m, `${where}: alternate ${hl}`);
      if (m) expect(fs.existsSync(path.join(dist, lang.dir, m[1].endsWith('/') ? `${m[1]}index.html` : m[1])), `${where}: local ${hl} page ${m[1]} exists`);
    }
    expect(h.indexOf('hreflang="x-default"') < h.indexOf('location.replace') && h.indexOf('location.replace') < h.indexOf('rel="stylesheet"'), `${where}: the language redirect runs before the stylesheet`);

    // every relative reference resolves to a file of the site
    for (const m of h.matchAll(/\s(?:href|src)="([^"#]+)"/g)) {
      const ref = m[1];
      if (/^(?:https?:|data:|mailto:|\/\/)/.test(ref)) continue;
      let target = path.join(dist, lang.dir, ref.split('?')[0]);
      if (ref.endsWith('/') || ref === './' || ref === '../') target = path.join(target, 'index.html');
      expect(fs.existsSync(target), `${where}: ${ref} resolves`);
    }
    for (const m of h.matchAll(/\bfrom\s*['"]([^'"]+)['"]/g)) expect(fs.existsSync(path.join(dist, lang.dir, m[1].split('?')[0])), `${where}: import ${m[1]} resolves`);
  }
}
expect(fs.existsSync(path.join(dist, 'robots.txt')), 'robots.txt');

// every reference to a script, style or module carries the build's ?v= stamp: _headers lets
// browsers keep these files for a year, so an unstamped reference could load a stale file
const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : [path.join(d, e.name)]));
const stamp = (fs.readFileSync(path.join(dist, 'index.html'), 'utf8').match(/\?v=([0-9a-f]{10})/) || [])[1];
expect(!!stamp, 'the build stamp is found');
for (const f of walk(dist).filter((x) => /\.(?:html|js)$/.test(x) && !x.includes(`${path.sep}docs${path.sep}`))) {
  const src = fs.readFileSync(f, 'utf8');
  const refs = [
    ...src.matchAll(/\bfrom\s*['"](\.{1,2}\/[^'"]+)['"]/g),
    ...src.matchAll(/\bimport\(\s*['"](\.{1,2}\/[^'"]+)['"]/g),
    ...src.matchAll(/\bnew URL\(\s*['"](\.{1,2}\/[^'"]+)['"]/g),
    ...src.matchAll(/\s(?:href|src)="([^"]+\.(?:js|css|wasm)(?:\?[^"]*)?)"/g),
  ].map((m) => m[1]);
  for (const ref of refs) if (/\.(?:js|css|wasm)/.test(ref) && !/^https?:/.test(ref)) expect(ref.endsWith(`?v=${stamp}`), `${path.relative(dist, f)}: ${ref} carries ?v=${stamp}`);
}

// with a site URL: canonical, previews, sitemap
const site = JSON.parse(fs.readFileSync(path.join(dist, '..', 'package.json'), 'utf8')).homepage;
if (site) {
  for (const lang of LANGS)
    for (const page of PAGES) {
      const h = fs.readFileSync(path.join(dist, lang.dir, page), 'utf8');
      const url = `${site}/${lang.dir}${page === 'index.html' ? '' : page.replace(/\.html$/, '')}`;
      const where = `${lang.dir}${page}`;
      expect(h.includes(`<link rel="canonical" href="${url}" />`), `${where}: canonical ${url}`);
      expect(h.includes(`<meta property="og:url" content="${url}" />`), `${where}: og:url`);
      expect(h.includes(`<meta property="og:image" content="${site}/img/social-${lang.code}.png" />`), `${where}: og:image`);
      expect(new RegExp(`hreflang="zh-CN" href="${site}/zh/`).test(h), `${where}: absolute hreflang`);
    }
  const map = fs.readFileSync(path.join(dist, 'sitemap.xml'), 'utf8');
  expect((map.match(/<loc>/g) || []).length === PAGES.length * LANGS.length, 'sitemap lists every page');
  expect(!/\.html</.test(map), 'sitemap addresses have no .html');
  expect(fs.readFileSync(path.join(dist, 'robots.txt'), 'utf8').includes(`Sitemap: ${site}/sitemap.xml`), 'robots.txt names the sitemap');
  const ld = fs.readFileSync(path.join(dist, 'index.html'), 'utf8').match(/<script type="application\/ld\+json">([^<]+)<\/script>/);
  expect(ld && JSON.parse(ld[1])['@type'] === 'SoftwareApplication', 'JSON-LD on the home page');
}
expect(fs.existsSync(path.join(dist, '404.html')) && fs.existsSync(path.join(dist, '_headers')), '404.html and _headers');

// the version: dist/js/version.js names the commit, and every page shows it
const build = JSON.parse(fs.readFileSync(path.join(dist, 'js/version.js'), 'utf8').match(/export const BUILD = (\{[^}]*\});/)[1]);
expect(/^[0-9a-f]{7}$/.test(build.commit) && /^[0-9a-f]{40}$/.test(build.full) && /^\d{4}-\d{2}-\d{2}$/.test(build.date), `version.js names a commit (${build.commit} ${build.date})`);
for (const lang of LANGS)
  for (const page of PAGES) {
    const h = fs.readFileSync(path.join(dist, lang.dir, page), 'utf8');
    const want = `${DICT[lang.code].buildLabel} ${build.commit}${build.dirty ? '+dev' : ''} · ${build.date}`;
    expect(h.includes(`/commit/${build.full}">${want}</a>`), `${lang.dir}${page}: shows ${want}`);
  }

if (failures) {
  console.log(`site: ${failures} of ${checks} checks failed`);
  process.exit(1);
}
console.log(`site: all ${checks} checks passed`);
