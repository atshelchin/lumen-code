// Builds dist/: a self-contained static site that can be deployed as is.
//   node src/tools/build.js                  copy the web app, the JS core and the protocol docs
//   node src/tools/build.js --wasm           also rebuild dist/wasm/lumen.wasm from src/rust (needs cargo)
//   node src/tools/build.js --site=https://… the site's public URL (default: "homepage" in package.json):
//                                            adds canonical and Open Graph tags and sitemap.xml
//
// dist/
//   index.html send.html recv.html  English pages, text pre-rendered (src/tools/prerender.js)
//   zh/index.html send.html recv.html  the same in Chinese
//   css/  js/ (app)  js/core/ (protocol library)  img/  wasm/lumen.wasm
//   docs/ (protocol + guides)  robots.txt  sitemap.xml (with a site URL)  LICENSE
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { prerender } from './prerender.js';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const dist = path.join(repo, 'dist');

function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    if (e.name.startsWith('.')) continue;
    const a = path.join(from, e.name);
    const b = path.join(to, e.name);
    if (e.isDirectory()) copyDir(a, b);
    else fs.copyFileSync(a, b);
  }
}

// clean everything except the WebAssembly build output
fs.mkdirSync(dist, { recursive: true });
for (const e of fs.readdirSync(dist)) if (e !== 'wasm') fs.rmSync(path.join(dist, e), { recursive: true, force: true });

copyDir(path.join(repo, 'src/web'), dist);
copyDir(path.join(repo, 'src/core'), path.join(dist, 'js/core'));
copyDir(path.join(repo, 'docs'), path.join(dist, 'docs'));
fs.rmSync(path.join(dist, 'docs/STATUS.md'), { force: true }); // working notes, not part of the site
fs.copyFileSync(path.join(repo, 'LICENSE'), path.join(dist, 'LICENSE'));

if (process.argv.includes('--wasm')) {
  execFileSync('sh', [path.join(repo, 'src/tools/build-wasm.sh')], { stdio: 'inherit' });
}
// The version shown on the pages: the last commit that changed a source of this site (the web
// app, the JS library, the Rust code of the WASM module, the build tools, the docs it copies),
// and "dirty" when these sources have uncommitted changes. So commit the sources first, then
// build and commit dist/: the pages then name that commit, and a later rebuild gives the same
// files (a change to the native apps or the tests does not change the site's version).
const SOURCES = ['src/web', 'src/core', 'src/rust', 'src/tools', 'docs', ':!docs/STATUS.md', 'LICENSE', 'package.json'];
const pkg = JSON.parse(fs.readFileSync(path.join(repo, 'package.json'), 'utf8'));
const build = (() => {
  const git = (...args) => execFileSync('git', args, { cwd: repo, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  try {
    const full = git('log', '-1', '--format=%H', '--', ...SOURCES);
    return {
      commit: full.slice(0, 7),
      full,
      date: git('log', '-1', '--format=%cs', '--', ...SOURCES),
      dirty: git('status', '--porcelain', '--', ...SOURCES) !== '',
      protocol: '1.0',
    };
  } catch (e) {
    return { commit: 'unknown', full: '', date: '', dirty: false, protocol: '1.0' }; // not a git checkout
  }
})();
fs.writeFileSync(
  path.join(dist, 'js/version.js'),
  fs.readFileSync(path.join(dist, 'js/version.js'), 'utf8').replace(/export const BUILD = \{[^}]*\};/, `export const BUILD = ${JSON.stringify(build)};`),
);
if (build.dirty) console.warn(`warning: the sources have uncommitted changes; the pages say ${build.commit}+dev (commit first, then build)`);

// the pages, one set per language, with their text in the HTML
const siteUrl = (process.argv.find((a) => a.startsWith('--site=')) || '').slice(7) || pkg.homepage || '';
const pre = prerender(dist, { site: siteUrl, build, repoUrl: (pkg.repository && pkg.repository.url) || '' });
console.log(`pre-rendered ${pre.pages} pages${pre.site ? ` for ${pre.site}` : ' (no site URL: no canonical, Open Graph or sitemap)'}, version ${build.commit}${build.dirty ? '+dev' : ''} ${build.date}`);

if (!fs.existsSync(path.join(dist, 'wasm/lumen.wasm'))) {
  console.warn('warning: dist/wasm/lumen.wasm is missing; run `npm run build:wasm` (the WASM engine will be unavailable)');
}

// Cache busting. Static hosts and browsers cache JS modules; after a deploy, a new page could run
// with old cached modules. Every relative .js/.css/.wasm reference gets ?v=<hash of the build>,
// so a new build is always fetched in full and an unchanged build keeps its cache.
const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : [path.join(d, e.name)]));
const site = walk(dist).filter((f) => !f.includes(`${path.sep}docs${path.sep}`)).sort();
const hash = createHash('sha256');
for (const f of site) hash.update(path.relative(dist, f)).update(fs.readFileSync(f));
const stamp = hash.digest('hex').slice(0, 10);
const REL = String.raw`\.{0,2}\/?[\w./-]+?\.(?:js|css|wasm)`;
const rules = [
  new RegExp(String.raw`(\bfrom\s*)(['"])(\.{1,2}\/[\w./-]+?\.js)\2`, 'g'), // static imports
  new RegExp(String.raw`(\bimport\(\s*)(['"])(\.{1,2}\/[\w./-]+?\.js)\2`, 'g'), // dynamic imports
  new RegExp(String.raw`(\bnew URL\(\s*)(['"])(\.{1,2}\/[\w./-]+?\.(?:js|wasm))\2`, 'g'), // workers, wasm
  new RegExp(String.raw`(\b(?:href|src)=)(")(${REL})"`, 'g'), // html attributes
];
let stamped = 0;
for (const f of site.filter((x) => /\.(?:js|html)$/.test(x))) {
  const before = fs.readFileSync(f, 'utf8');
  let after = before;
  for (const re of rules) after = after.replace(re, (m, pre, q, url) => (/^(?:https?:|data:|\/\/)/.test(url) ? m : `${pre}${q}${url}?v=${stamp}${q}`));
  if (after !== before) {
    fs.writeFileSync(f, after);
    stamped++;
  }
}
console.log(`cache stamp ?v=${stamp} in ${stamped} files`);

let files = 0;
let bytes = 0;
(function count(d) {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) count(p);
    else {
      files++;
      bytes += fs.statSync(p).size;
    }
  }
})(dist);
console.log(`dist/: ${files} files, ${(bytes / 1024).toFixed(0)} KB`);
