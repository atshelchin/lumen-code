// Builds dist/: a self-contained static site that can be deployed as is.
//   node src/tools/build.js          copy the web app, the JS core and the protocol docs
//   node src/tools/build.js --wasm   also rebuild dist/wasm/lumen.wasm from src/rust (needs cargo)
//
// dist/
//   index.html send.html recv.html  css/  js/ (app)  js/core/ (protocol library)
//   wasm/lumen.wasm  PROTOCOL.md  PROTOCOL.zh-CN.md  LICENSE
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

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
for (const f of ['PROTOCOL.md', 'PROTOCOL.zh-CN.md', 'LICENSE']) fs.copyFileSync(path.join(repo, f), path.join(dist, f));

if (process.argv.includes('--wasm')) {
  execFileSync('sh', [path.join(repo, 'src/tools/build-wasm.sh')], { stdio: 'inherit' });
}
if (!fs.existsSync(path.join(dist, 'wasm/lumen.wasm'))) {
  console.warn('warning: dist/wasm/lumen.wasm is missing; run `npm run build:wasm` (the WASM engine will be unavailable)');
}

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
