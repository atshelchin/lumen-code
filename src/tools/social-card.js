// Draws the link-preview images (Open Graph, 1200 × 630) into src/web/img/social-<lang>.png.
// A real frame drawn by the protocol library, the name and one line of text. Needs Google Chrome
// (headless); the images are committed, so the build itself does not need Chrome.
//   npm run build && node src/tools/social-card.js [chrome binary]
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openChrome, defaultChrome } from './cdp.js';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const dist = path.join(repo, 'dist');
const out = path.join(repo, 'src/web/img');
const binary = process.argv[2] || defaultChrome();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const TEXT = {
  en: { a: 'Files from a screen', b: 'to a camera', p: 'No network, no pairing. Open protocol 1.0 · up to 96\u00a0KB/s measured · MIT' },
  zh: { a: '用屏幕和相机', b: '传文件', p: '无需网络，无需配对。开放协议 1.0 · 实测最高 96\u00a0KB/s · MIT 许可' },
};

const card = (t) => `<!doctype html><html><head><meta charset="utf-8"><style>
  html,body{margin:0;width:1200px;height:630px;background:#07090d;color:#e9edf4;
    font-family:ui-sans-serif,system-ui,-apple-system,'SF Pro Display','PingFang SC','Hiragino Sans GB','Microsoft YaHei',sans-serif}
  .wrap{display:grid;grid-template-columns:560px 1fr;gap:48px;align-items:center;height:630px;padding:0 64px;box-sizing:border-box;
    background:radial-gradient(900px 500px at 85% 50%,rgba(34,216,255,.10),transparent 60%)}
  .brand{display:flex;align-items:center;gap:14px;font-size:26px;font-weight:650;margin-bottom:34px}
  .mark{display:grid;grid-template-columns:repeat(3,12px);gap:5px;padding:10px;border-radius:14px;background:#0a0d14;border:1px solid rgba(255,255,255,.14)}
  .mark i{width:12px;height:12px;border-radius:3px;background:#1b2130}
  .mark i:nth-child(1),.mark i:nth-child(8){background:#22d8ff}.mark i:nth-child(3){background:#ff3d9a}
  .mark i:nth-child(5){background:#f2f4f8}.mark i:nth-child(6){background:#ffd60a}.mark i:nth-child(9){background:#2bdc74}
  h1{margin:0;font-size:58px;line-height:1.1;letter-spacing:-.02em;font-weight:750}
  h1 em{font-style:normal;background:linear-gradient(90deg,#22d8ff,#ff3d9a 55%,#ffd60a);-webkit-background-clip:text;color:transparent}
  p{margin:26px 0 0;color:#8b94a7;font-size:24px;line-height:1.45}
  canvas{width:100%;image-rendering:pixelated;border-radius:18px;border:1px solid rgba(255,255,255,.1);box-shadow:0 20px 60px rgba(0,0,0,.5)}
</style></head><body><div class="wrap"><div>
  <div class="brand"><span class="mark">${'<i></i>'.repeat(9)}</span>Lumen Code</div>
  <h1>${t.a}<br><em>${t.b}</em></h1><p>${t.p}</p>
</div><canvas id="c"></canvas></div>
<script type="module">
  import { Sender } from './js/core/transfer.js';
  import { Renderer } from './js/core/render.js';
  import { crc32 } from './js/core/util.js';
  const text = new TextEncoder().encode('Lumen Code: files as moving light. '.repeat(40));
  const meta = { name: 'hello.txt', fileSize: text.length, crc: crc32(text), flags: 0, payloadLength: text.length };
  const tx = new Sender({ payload: text, meta }, { mode: 1, ecc: 1, X: 64, Y: 64, tid: 0x4c43 });
  tx.seq = 7;
  const { cells, layout } = tx.nextFrame();
  const r = new Renderer(layout, 1, 6);
  const c = document.getElementById('c');
  c.width = r.width; c.height = r.height;
  const g = c.getContext('2d'); const img = g.createImageData(r.width, r.height);
  r.draw(cells, new Uint32Array(img.data.buffer)); g.putImageData(img, 0, 0);
  document.body.dataset.ready = '1';
</script></body></html>`;

// serve dist/ with the card pages in it
const port = 8600 + Math.floor(Math.random() * 300);
fs.mkdirSync(out, { recursive: true });
for (const lang of Object.keys(TEXT)) fs.writeFileSync(path.join(dist, `_card-${lang}.html`), card(TEXT[lang]));
const server = spawn(process.execPath, [path.join(repo, 'src/tools/serve.js'), '--dist', `--port=${port}`], { stdio: 'ignore' });
let chrome;
try {
  chrome = await openChrome(binary, ['--hide-scrollbars']);
  const send = chrome.send;
  await send('Emulation.setDeviceMetricsOverride', { width: 1200, height: 630, deviceScaleFactor: 1, mobile: false });
  for (const lang of Object.keys(TEXT)) {
    await send('Page.navigate', { url: `http://localhost:${port}/_card-${lang}.html` });
    for (let i = 0; i < 50; i++) {
      await sleep(100);
      const r = await send('Runtime.evaluate', { expression: 'document.body && document.body.dataset.ready', returnByValue: true });
      if (r.result && r.result.value === '1') break;
    }
    const shot = await send('Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(out, `social-${lang}.png`), Buffer.from(shot.data, 'base64'));
    console.log(`wrote src/web/img/social-${lang}.png`);
  }
} finally {
  if (chrome) chrome.close();
  server.kill();
  for (const lang of Object.keys(TEXT)) fs.rmSync(path.join(dist, `_card-${lang}.html`), { force: true });
}
process.exit(0);
