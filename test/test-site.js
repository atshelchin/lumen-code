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
    for (const hl of ['en', 'zh-CN', 'x-default']) expect(new RegExp(`<link rel="alternate" hreflang="${hl}" href="[^"]+"`).test(h), `${where}: alternate ${hl}`);
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

if (failures) {
  console.log(`site: ${failures} of ${checks} checks failed`);
  process.exit(1);
}
console.log(`site: all ${checks} checks passed`);
