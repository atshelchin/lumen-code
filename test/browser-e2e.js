// End to end in a real browser: headless Chrome plays a .y4m video as its webcam, the built
// receiver page (dist/recv.html) opens that camera and must deliver the file in it.
//   npm run build && node test/make-y4m.js test/out/cam.y4m 60000 1 100 170 8 1280 720
//   node test/browser-e2e.js [engine: wasm|js] [language: en|zh] [chrome binary]   (or CHROME=<binary>)
// Zero dependencies: the DevTools protocol over the WebSocket of Node 22 (src/tools/cdp.js).
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openChrome, defaultChrome } from '../src/tools/cdp.js';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const engine = process.argv[2] || 'wasm';
const dir = process.argv[3] === 'zh' ? 'zh/' : ''; // the pages of the other language live in zh/
const binary = process.argv[4] || defaultChrome();
const video = path.join(repo, 'test/out/cam.y4m');
const want = createHash('sha256').update(readFileSync(video + '.src')).digest('hex');
const port = 8000 + Math.floor(Math.random() * 900);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const server = spawn(process.execPath, [path.join(repo, 'src/tools/serve.js'), '--dist', `--port=${port}`], { stdio: 'ignore' });
let chrome;
try {
  chrome = await openChrome(binary, ['--use-fake-device-for-media-stream', `--use-file-for-fake-video-capture=${video}`, '--use-fake-ui-for-media-stream']);
} catch (e) {
  console.log(`browser e2e: ${e.message}`);
  server.kill();
  process.exit(1);
}
const stop = (code) => {
  chrome.close();
  server.kill();
  process.exit(code);
};
const errors = [];
chrome.on((msg) => {
  if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails.text + ' ' + (msg.params.exceptionDetails.exception?.description || ''));
  if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') errors.push(msg.params.args.map((a) => a.value ?? a.description).join(' '));
});
const send = chrome.send;
const evaluate = async (expr) => (await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true })).result?.value;

await send('Runtime.enable');
await send('Page.enable');
await send('Page.navigate', { url: `http://localhost:${port}/${dir}recv.html` });
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
  console.log(`browser e2e (${engine}, ${dir || 'en/'}): file received in ${((Date.now() - t0) / 1000).toFixed(1)} s, SHA-256 ${ok ? 'matches' : 'DIFFERS'}`);
} else console.log(`browser e2e (${engine}, ${dir || 'en/'}): no file within 90 s`);
if (errors.length) console.log('page errors:', errors.slice(0, 5));

// the home page draws one still frame with the protocol library
await send('Page.navigate', { url: `http://localhost:${port}/${dir}index.html` });
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
await send('Page.navigate', { url: `http://localhost:${port}/${dir}send.html` });
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

// the language versions: a saved choice, or a Chinese browser on the English pages, opens the
// other page before anything is drawn; the language button switches pages
const go = async (url, pre = '') => {
  await send('Page.navigate', { url: `http://localhost:${port}/${url}` });
  await sleep(1200);
  if (pre) {
    await evaluate(pre);
    await sleep(1500);
  }
  return evaluate('location.pathname');
};
const langChecks = [
  ['saved Chinese choice: send.html opens zh/send.html', (await go('send.html', "localStorage.setItem('lumen.lang','zh'); location.reload(); true")) === '/zh/send.html'],
  ['saved English choice: zh/index.html opens the English home', (await go('zh/index.html', "localStorage.setItem('lumen.lang','en'); location.reload(); true")) === '/'],
  ['home page button switches to Chinese', (await go('index.html', "document.getElementById('lang').click(); true")) === '/zh/'],
  ['…and back to English', (await go('zh/', "document.getElementById('lang').click(); true")) === '/'],
];
await evaluate("localStorage.removeItem('lumen.lang'); true");
await send('Emulation.setUserAgentOverride', { userAgent: 'Mozilla/5.0 (lumen e2e)', acceptLanguage: 'zh-CN', platform: 'MacIntel' });
langChecks.push(['Chinese browser, no choice: recv.html opens zh/recv.html', (await go('recv.html')) === '/zh/recv.html']);
langChecks.push(['Chinese browser: the Chinese pages stay', (await go('zh/send.html')) === '/zh/send.html']);
await send('Emulation.setUserAgentOverride', { userAgent: 'Mozilla/5.0 (lumen e2e)', acceptLanguage: 'en-US', platform: 'MacIntel' });
langChecks.push(['English browser, no choice: zh/recv.html stays (crawlers read it)', (await go('zh/recv.html')) === '/zh/recv.html']);
const langOk = langChecks.every(([, pass]) => pass);
for (const [what, pass] of langChecks) if (!pass) console.log(`language FAIL: ${what}`);
console.log(`browser languages: ${langOk ? `all ${langChecks.length} checks pass` : 'FAILED'}`);

if (errors.length) console.log('page errors:', errors.slice(0, 5));
stop(ok && homeOk && senderOk && langOk && !errors.length ? 0 : 1);
