// End to end in a real browser: headless Chrome plays a .y4m video as its webcam, the built
// receiver page (dist/recv.html) opens that camera and must deliver the file in it.
//   npm run build && node test/make-y4m.js test/out/cam.y4m 60000 1 100 170 8 1280 720
//   node test/browser-e2e.js [engine: wasm|js] [chrome binary]   (or CHROME=<binary>)
// Zero dependencies: the DevTools protocol over the WebSocket of Node 22.
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const engine = process.argv[2] || 'wasm';
const chrome = process.argv[3] || process.env.CHROME || (process.platform === 'darwin' ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' : 'google-chrome');
const video = path.join(repo, 'test/out/cam.y4m');
const want = createHash('sha256').update(readFileSync(video + '.src')).digest('hex');
const port = 8000 + Math.floor(Math.random() * 900);
const dbg = 9300 + Math.floor(Math.random() * 600);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const server = spawn(process.execPath, [path.join(repo, 'src/tools/serve.js'), '--dist', `--port=${port}`], { stdio: 'ignore' });
const browser = spawn(chrome, [
  '--headless=new',
  `--remote-debugging-port=${dbg}`,
  `--user-data-dir=${mkdtempSync(path.join(tmpdir(), 'lumen-e2e-'))}`,
  '--use-fake-device-for-media-stream',
  `--use-file-for-fake-video-capture=${video}`,
  '--use-fake-ui-for-media-stream',
  '--no-first-run',
  'about:blank',
], { stdio: 'ignore' });
const stop = (code) => {
  browser.kill();
  server.kill();
  process.exit(code);
};

let ws;
for (let i = 0; i < 50 && !ws; i++) {
  await sleep(200);
  try {
    const pages = await (await fetch(`http://127.0.0.1:${dbg}/json`)).json();
    const page = pages.find((p) => p.type === 'page');
    if (page) ws = new WebSocket(page.webSocketDebuggerUrl);
  } catch (e) {
    /* not up yet */
  }
}
if (!ws) {
  console.log('browser e2e: Chrome did not start');
  stop(1);
}
await new Promise((r) => (ws.onopen = r));
let id = 0;
const waiting = new Map();
const errors = [];
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && waiting.has(msg.id)) waiting.get(msg.id)(msg.result);
  if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails.text + ' ' + (msg.params.exceptionDetails.exception?.description || ''));
  if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') errors.push(msg.params.args.map((a) => a.value ?? a.description).join(' '));
};
const send = (method, params = {}) => new Promise((r) => { waiting.set(++id, r); ws.send(JSON.stringify({ id, method, params })); });
const evaluate = async (expr) => (await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true })).result?.value;

await send('Runtime.enable');
await send('Page.enable');
await send('Page.navigate', { url: `http://localhost:${port}/recv.html` });
await sleep(1500);
// pick the engine (read at page load; the receiver defaults to WASM), then open the camera
await evaluate(`localStorage.setItem('lumen.engine', ${JSON.stringify(engine)}); true`);
await send('Page.reload');
await sleep(2500);
await evaluate(`document.getElementById('startBtn').click(); true`);
const t0 = Date.now();
let done = false;
let last = '';
while (Date.now() - t0 < 90000) {
  await sleep(1000);
  const st = await evaluate(`(() => {
    const d = document.getElementById('done');
    return { done: !d.classList.contains('hidden'), pct: document.getElementById('rxPct')?.textContent, diag: document.getElementById('diag')?.textContent, tip: document.getElementById('tip')?.textContent };
  })()`);
  if (!st) continue;
  const line = `${st.pct} ${st.tip} | ${st.diag}`;
  if (line !== last) console.log(`  ${((Date.now() - t0) / 1000).toFixed(0).padStart(3)} s  ${line}`);
  last = line;
  if (st.done) {
    done = true;
    break;
  }
}
let ok = false;
if (done) {
  const got = await evaluate(`(async () => {
    const b = await (await fetch(document.getElementById('saveBtn').href)).arrayBuffer();
    return [...new Uint8Array(await crypto.subtle.digest('SHA-256', b))].map((x) => x.toString(16).padStart(2, '0')).join('');
  })()`);
  ok = got === want;
  console.log(`browser e2e (${engine}): file received in ${((Date.now() - t0) / 1000).toFixed(1)} s, SHA-256 ${ok ? 'matches' : 'DIFFERS'}`);
} else console.log(`browser e2e (${engine}): no file within 90 s`);
if (errors.length) console.log('page errors:', errors.slice(0, 5));

// the home page draws one still frame with the protocol library
await send('Page.navigate', { url: `http://localhost:${port}/index.html` });
await sleep(2000);
const home = await evaluate(`(() => {
  const c = document.getElementById('sample');
  const px = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
  let colours = 0;
  for (let i = 0; i < px.length; i += 4) if (px[i] > 200 && px[i + 2] < 60) colours++; // yellow cells of C4
  return { w: c.width, h: c.height, colours };
})()`);
const homeOk = home && home.w > 300 && home.colours > 1000;
console.log(`browser home: sample frame ${homeOk ? `drawn (${home.w}x${home.h} px)` : 'NOT drawn ' + JSON.stringify(home)}`);

// the sender page: send typed text and check that frames are drawn
await send('Page.navigate', { url: `http://localhost:${port}/send.html` });
await sleep(2500);
const sent = await evaluate(`(async () => {
  document.querySelector('#srcTabs [data-v="text"], #srcTabs button:nth-child(2)').click();
  const ta = document.getElementById('textInput');
  ta.value = 'Lumen Code browser test ' + 'x'.repeat(5000);
  ta.dispatchEvent(new Event('input', { bubbles: true }));
  document.getElementById('textSend').click();
  await new Promise((r) => setTimeout(r, 2000));
  const c = document.getElementById('code');
  const visible = !document.getElementById('codewrap').classList.contains('hidden');
  return { visible, w: c.width, h: c.height };
})()`);
const senderOk = sent && sent.visible && sent.w > 100 && sent.h > 100;
console.log(`browser sender: code ${senderOk ? `drawn (${sent.w}x${sent.h} px)` : 'NOT drawn ' + JSON.stringify(sent)}`);
if (errors.length) console.log('page errors:', errors.slice(0, 5));
stop(ok && homeOk && senderOk && !errors.length ? 0 : 1);
