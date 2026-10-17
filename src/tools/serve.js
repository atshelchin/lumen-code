// Zero-dependency static server.
//   node src/tools/serve.js                -> http://localhost:8080, serves the sources (no build needed)
//   node src/tools/serve.js --dist         -> serves dist/ exactly as it will be deployed
//   node src/tools/serve.js --https        -> https://<lan-ip>:8443 (self-signed via openssl; phones need HTTPS for the camera)
//   --port=N overrides the port.
//
// Source mode maps the site layout onto the source tree:
//   /js/core/*   -> src/core/*     (JS protocol implementation)
//   /wasm/*      -> dist/wasm/*    (built by src/tools/build-wasm.sh)
//   /test/*      -> test/*         (browser tests)
//   /docs/*      -> docs/*        (protocol and guides)
//   everything else -> src/web/*
import http from 'node:http';
import https from 'node:https';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const useHttps = args.includes('--https');
const distMode = args.includes('--dist');
const port = Number((args.find((a) => a.startsWith('--port=')) || '').split('=')[1]) || (useHttps ? 8443 : 8080);

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.md': 'text/markdown; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.ico': 'image/x-icon',
  '.wasm': 'application/wasm',
  '.webmanifest': 'application/manifest+json',
};

function resolveSource(p) {
  const rules = [
    ['/js/core/', 'src/core/'],
    ['/wasm/', 'dist/wasm/'],
    ['/test/', 'test/'],
    ['/docs/', 'docs/'],
  ];
  for (const [prefix, dir] of rules) if (p.startsWith(prefix)) return path.join(repo, dir, p.slice(prefix.length));
  return path.join(repo, 'src/web', p);
}

function handler(req, res) {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  p = path.posix.normalize(p);
  const file = distMode ? path.join(repo, 'dist', p) : resolveSource(p);
  if (!file.startsWith(repo + path.sep)) {
    res.writeHead(403).end();
    return;
  }
  fs.readFile(file, (err, data) => {
    if (err) {
      res.writeHead(404, { 'content-type': 'text/plain' }).end('not found');
      return;
    }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-cache' });
    res.end(data);
  });
}

function lanIPs() {
  return Object.values(os.networkInterfaces())
    .flat()
    .filter((i) => i && i.family === 'IPv4' && !i.internal)
    .map((i) => i.address);
}

const what = distMode ? 'dist/' : 'sources';
if (useHttps) {
  const dir = path.join(repo, '.cert');
  const key = path.join(dir, 'key.pem');
  const cert = path.join(dir, 'cert.pem');
  if (!fs.existsSync(key) || !fs.existsSync(cert)) {
    fs.mkdirSync(dir, { recursive: true });
    const san = ['DNS:localhost', 'IP:127.0.0.1', ...lanIPs().map((ip) => `IP:${ip}`)].join(',');
    execFileSync('openssl', [
      'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '825',
      '-keyout', key, '-out', cert, '-subj', '/CN=lumen-code-local', '-addext', `subjectAltName=${san}`,
    ], { stdio: 'ignore' });
  }
  https.createServer({ key: fs.readFileSync(key), cert: fs.readFileSync(cert) }, handler).listen(port, '0.0.0.0', () => {
    console.log(`Lumen Code (${what}, https, self-signed) on:`);
    for (const ip of ['localhost', ...lanIPs()]) console.log(`  https://${ip}:${port}/`);
  });
} else {
  http.createServer(handler).listen(port, '0.0.0.0', () => {
    console.log(`Lumen Code (${what}) on http://localhost:${port}/`);
    for (const ip of lanIPs()) console.log(`  LAN (camera needs --https): http://${ip}:${port}/`);
  });
}
