import { applyI18n, t } from './i18n.js?v=d5bf8a262c';
import { BUILD, buildText } from './version.js?v=d5bf8a262c';
import { Sender, preparePayload } from './core/transfer.js?v=d5bf8a262c';
import { Renderer } from './core/render.js?v=d5bf8a262c';
import { MIN_DIM, MAX_DIM, MODES } from './core/layout.js?v=d5bf8a262c';
import { loadWasm, WasmSender } from './core/wasm-engine.js?v=d5bf8a262c';

const $ = (id) => document.getElementById(id);
applyI18n();
// the version a user quotes in a report: in the HTML of a build, filled here on the dev server
for (const el of document.querySelectorAll('[data-build]')) if (!el.textContent) el.textContent = buildText(t('buildLabel'));
console.info('Lumen Code', BUILD);

const QUIET = 2;
// The code fills the screen in its own shape; beyond this aspect ratio (ultra-wide monitors)
// a camera would see the code too thin, so the rest stays empty.
const MAX_ASPECT = 2.4;

const prefs = load();
{
  const e = new URLSearchParams(location.search).get('engine');
  if (e) prefs.engine = e === 'wasm' ? 'wasm' : 'js';
}
function load() {
  const d = { density: 'auto', mode: 1, ecc: 1, fps: 12, engine: 'js' };
  try {
    return { ...d, ...JSON.parse(localStorage.getItem('lumen.send.v2') || '{}') };
  } catch (e) {
    return d;
  }
}
function save() {
  try {
    localStorage.setItem('lumen.send.v2', JSON.stringify(prefs));
  } catch (e) {
    /* ignore */
  }
}

// ---------- geometry: how many cells the screen should carry
function mmPerCssPx() {
  const coarse = matchMedia('(pointer: coarse)').matches;
  const minScreen = Math.min(screen.width, screen.height);
  if (coarse && minScreen < 520) return 0.165; // phone
  if (coarse) return 0.2; // tablet
  return 0.25; // laptop / desktop
}

const DENSITY = { low: 0.72, auto: 1, high: 1.3, max: 1.65 };

// The code box fills the available screen (any orientation): the more cells, the more data per
// frame. The receiver turns its phone to match a landscape code.
function computeGeometry() {
  const dpr = window.devicePixelRatio || 1;
  const pad = 6;
  const cssW = Math.max(100, window.innerWidth - 2 * pad);
  const cssH = Math.max(100, window.innerHeight - 2 * pad);
  const boxW = Math.min(cssW, cssH * MAX_ASPECT);
  const boxH = Math.min(cssH, cssW * MAX_ASPECT);
  const devW = Math.floor(boxW * dpr);
  const devH = Math.floor(boxH * dpr);
  const shortMm = Math.min(boxW, boxH) * mmPerCssPx();
  // base: ~0.75 mm per cell, clamped to what a 1080p camera can resolve across its short side
  const base = Math.max(52, Math.min(150, Math.round(shortMm / 0.75)));
  const target = Math.round(base * DENSITY[prefs.density]);
  const shortDev = Math.min(devW, devH);
  let s = Math.max(3, Math.round(shortDev / (target + 2 * QUIET)));
  const fit = () => {
    X = Math.floor(devW / s) - 2 * QUIET;
    Y = Math.floor(devH / s) - 2 * QUIET;
    if (X > Y * MAX_ASPECT) X = Math.floor(Y * MAX_ASPECT);
    if (Y > X * MAX_ASPECT) Y = Math.floor(X * MAX_ASPECT);
  };
  let X;
  let Y;
  fit();
  // stay inside format limits
  while ((X > MAX_DIM || Y > MAX_DIM) && s < 64) {
    s++;
    fit();
  }
  while ((X < MIN_DIM || Y < MIN_DIM) && s > 2) {
    s--;
    fit();
  }
  X = Math.max(MIN_DIM, Math.min(MAX_DIM, X));
  Y = Math.max(MIN_DIM, Math.min(MAX_DIM, Y));
  return { s, X, Y, dpr };
}

// ---------- state
let sender = null;
let file = null; // { name, size, prepared }
let renderer = null;
let geo = null;
let pending = null; // next frame, already in the cell canvas
let playing = true;
let lastFlip = 0;
let shownFrames = 0;
const canvas = $('code');
const ctx = canvas.getContext('2d', { alpha: false });
// Per frame only a tiny 1-pixel-per-cell image changes; the GPU scales it up and draws the
// static rounded mask on top. No per-pixel JavaScript work at screen resolution.
const cellCanvas = document.createElement('canvas');
const cellCtx = cellCanvas.getContext('2d', { alpha: false });
const maskCanvas = document.createElement('canvas');
let cellImage = null;

function setupCanvas() {
  geo = computeGeometry();
  if (sender) sender.resize(geo.X, geo.Y);
  renderer = new Renderer(sender.layout, sender.mode, geo.s, QUIET);
  canvas.width = renderer.width;
  canvas.height = renderer.height;
  canvas.style.width = `${renderer.width / geo.dpr}px`;
  canvas.style.height = `${renderer.height / geo.dpr}px`;
  maskCanvas.width = renderer.width;
  maskCanvas.height = renderer.height;
  const mctx = maskCanvas.getContext('2d');
  const mask = mctx.createImageData(renderer.width, renderer.height);
  new Uint32Array(mask.data.buffer).set(renderer.maskLayer());
  mctx.putImageData(mask, 0, 0);
  cellCanvas.width = sender.layout.X + 2 * QUIET;
  cellCanvas.height = sender.layout.Y + 2 * QUIET;
  cellImage = cellCtx.createImageData(cellCanvas.width, cellCanvas.height);
  new Uint32Array(cellImage.data.buffer).fill(renderer.ink);
  pending = null;
  prepare();
  flip();
  updateStats();
}

let buildMs = []; // per-frame encode time of the current engine
function prepare() {
  const t0 = performance.now();
  const { cells } = sender.nextFrame();
  buildMs.push(performance.now() - t0);
  if (buildMs.length > 30) buildMs.shift();
  renderer.cellPixels(cells, new Uint32Array(cellImage.data.buffer));
  cellCtx.putImageData(cellImage, 0, 0);
  pending = true;
}

function flip() {
  if (!pending) return;
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(cellCanvas, 0, 0, canvas.width, canvas.height);
  ctx.drawImage(maskCanvas, 0, 0);
  pending = null;
  shownFrames++;
}

// Flip on the display refresh closest to a fixed schedule, so any rate keeps its cadence
// (45 fps on a 60 Hz screen alternates one and two refreshes instead of falling to 30).
// Slow rates sleep until shortly before a flip; fast ones look at every refresh.
let tickTimer = 0;
let nextFlipAt = 0;
function schedule() {
  clearTimeout(tickTimer);
  if (!sender || !playing) return;
  const period = 1000 / prefs.fps;
  const wait = nextFlipAt - performance.now() - 20;
  if (period > 40 && wait > 0) tickTimer = setTimeout(() => requestAnimationFrame(tick), wait);
  else requestAnimationFrame(tick);
}
function tick(ts) {
  if (!sender || !playing) return;
  const period = 1000 / prefs.fps;
  // start, or after a pause or a stall: from now
  if (!nextFlipAt || ts - nextFlipAt > 2 * period) nextFlipAt = ts;
  if (ts + 4 >= nextFlipAt) {
    lastFlip = ts;
    flip();
    prepare();
    nextFlipAt = Math.max(nextFlipAt + period, ts + period / 2);
  }
  schedule();
}

// ---------- start / reconfigure
// Text messages travel as a UTF-8 file with this name; receivers show them as text.
const TEXT_NAME = 'message.txt';

async function startFile(name, bytes, text) {
  $('drop').classList.add('hidden');
  $('codewrap').classList.remove('hidden');
  $('dock').classList.remove('hidden');
  $('topbar').classList.add('hidden');
  $('fileName').textContent = text ? `${t('textLabel')}: ${text.replace(/\s+/g, ' ').slice(0, 40)}` : name;
  $('fileSize').textContent = fmtBytes(bytes.length);
  $('stats').textContent = t('preparing');
  const prepared = await preparePayload(bytes, name);
  file = { name, size: bytes.length, prepared };
  if (prefs.engine === 'wasm') await loadWasm();
  newSender();
  playing = true;
  syncPlay();
  wake();
  showDock();
  schedule();
}

// Both engines emit bit-identical streams, so switching keeps the transfer id and the
// receiver keeps its progress.
function newSender(keepTid) {
  geo = computeGeometry();
  if (sender && sender.free) sender.free();
  const Impl = prefs.engine === 'wasm' ? WasmSender : Sender;
  sender = new Impl(file.prepared, { mode: prefs.mode, ecc: prefs.ecc, X: geo.X, Y: geo.Y, tid: keepTid });
  buildMs = [];
  setupCanvas();
}

function fmtBytes(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`;
  return `${(n / 1048576).toFixed(1)} MB`;
}

function updateStats() {
  if (!sender) return;
  const per = sender.bytesPerFrame();
  const rate = per * prefs.fps;
  const passSec = file.prepared.payload.length / rate;
  const L = sender.layout;
  $('stats').innerHTML =
    `<span>${t('rate')} <b class="num">${fmtBytes(rate)}/s</b></span>` +
    `<span>${t('perFrame')} <b class="num">${fmtBytes(per)}</b></span>` +
    `<span class="wide">${t('loop')} <b class="num">${passSec < 1 ? '<1' : passSec.toFixed(passSec < 10 ? 1 : 0)} s</b></span>` +
    `<span class="wide">${t('grid')} <b class="num">${L.X}×${L.Y}</b> · ${MODES[sender.mode].bpc} bit</span>` +
    `<span class="wide">${prefs.engine.toUpperCase()} <b class="num" id="buildMs">–</b></span>`;
}
setInterval(() => {
  const el = document.getElementById('buildMs');
  if (el && buildMs.length) el.textContent = `${(buildMs.reduce((a, b) => a + b, 0) / buildMs.length).toFixed(2)} ms`;
}, 1000);

// ---------- controls
function bindSeg(id, key, parse, onChange) {
  const el = $(id);
  const sync = () => {
    for (const b of el.querySelectorAll('button')) b.setAttribute('aria-pressed', String(parse(b.dataset.v) === prefs[key]));
  };
  el.addEventListener('click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    const v = parse(b.dataset.v);
    if (v === prefs[key]) return;
    prefs[key] = v;
    save();
    sync();
    onChange();
  });
  sync();
}

bindSeg('segDensity', 'density', String, () => {
  if (sender) setupCanvas();
});
bindSeg('segMode', 'mode', Number, () => {
  if (sender) reconfigure();
});
bindSeg('segEcc', 'ecc', Number, () => {
  if (sender) reconfigure();
});
bindSeg('segEngine', 'engine', String, async () => {
  if (!sender) return;
  if (prefs.engine === 'wasm') await loadWasm();
  newSender(sender.tid);
});

// Mode/ECC changes keep the same transfer (and the receiver's progress) when the
// packet size can be preserved; otherwise a new transfer starts.
function reconfigure() {
  const keep = sender.reconfigure({ mode: prefs.mode, ecc: prefs.ecc });
  if (!keep) newSender();
  else setupCanvas();
}

const fps = $('fps');
fps.value = prefs.fps;
$('fpsVal').textContent = prefs.fps;
fps.oninput = () => {
  prefs.fps = Number(fps.value);
  $('fpsVal').textContent = prefs.fps;
  schedule();
  save();
  updateStats();
};

function syncPlay() {
  $('icoPause').classList.toggle('hidden', !playing);
  $('icoPlay').classList.toggle('hidden', playing);
  $('liveDot').className = `dot ${playing ? 'live' : 'warn'}`;
}
$('playBtn').onclick = () => {
  playing = !playing;
  syncPlay();
  schedule();
  armDockTimer();
};

const setBtn = $('setBtn');
function showSettings(on) {
  $('settings').classList.toggle('hidden', !on);
  setBtn.setAttribute('aria-expanded', String(on));
}
showSettings(window.innerWidth >= 900);
setBtn.onclick = () => showSettings($('settings').classList.contains('hidden'));

$('fsBtn').onclick = () => {
  if (document.fullscreenElement) document.exitFullscreen();
  else document.documentElement.requestFullscreen?.().catch(() => {});
};

$('closeBtn').onclick = () => {
  sender = null;
  file = null;
  $('drop').classList.remove('hidden');
  $('codewrap').classList.add('hidden');
  $('dock').classList.add('hidden');
  $('topbar').classList.remove('hidden');
  $('file').value = '';
};

let resizeTimer = 0;
window.addEventListener('resize', () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => sender && setupCanvas(), 150);
});

// ---------- dock visibility
// Hidden by: the hide button, a tap/click on the code, H/Esc, or 2.6 s of inactivity while playing.
// Shown by: a tap/click on the code, the mouse reaching the bottom edge, or any key.
const dock = $('dock');
let dockTimer = 0;
function dockVisible() {
  return !dock.classList.contains('away');
}
function hideDock() {
  clearTimeout(dockTimer);
  dock.classList.add('away');
}
function showDock() {
  dock.classList.remove('away');
  armDockTimer();
}
function armDockTimer() {
  clearTimeout(dockTimer);
  dockTimer = setTimeout(() => {
    if (sender && playing && !dock.matches(':hover') && !dock.contains(document.activeElement)) hideDock();
    else if (dockVisible()) armDockTimer();
  }, 2600);
}
function pokeDock() {
  if (dockVisible()) armDockTimer();
}
$('hideBtn').onclick = (e) => {
  e.stopPropagation();
  hideDock();
};
$('codewrap').addEventListener('click', () => (dockVisible() ? hideDock() : showDock()));
dock.addEventListener('pointermove', pokeDock, { passive: true });
dock.addEventListener('pointerdown', pokeDock, { passive: true });
window.addEventListener(
  'pointermove',
  (e) => {
    if (e.pointerType === 'mouse' && sender && e.clientY > window.innerHeight - 90 && !dockVisible()) showDock();
  },
  { passive: true },
);

window.addEventListener('keydown', (e) => {
  if (!sender || e.target.tagName === 'TEXTAREA') return;
  if (e.code === 'Space') {
    playing = !playing;
    syncPlay();
    schedule();
    showDock();
    e.preventDefault();
  } else if (e.key === 'f') $('fsBtn').onclick();
  else if (e.key === 'Escape') hideDock();
  else if (e.key === 'h') dockVisible() ? hideDock() : showDock();
  else showDock();
});

// keep the screen on
let wakeLock = null;
async function wake() {
  try {
    if ('wakeLock' in navigator && !wakeLock) {
      wakeLock = await navigator.wakeLock.request('screen');
      wakeLock.addEventListener('release', () => (wakeLock = null));
    }
  } catch (e) {
    /* not available */
  }
}
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible' && sender) wake();
});

// ---------- inputs
$('pick').onclick = () => $('file').click();
$('file').onchange = async () => {
  const f = $('file').files[0];
  if (f) startFile(f.name, new Uint8Array(await f.arrayBuffer()));
};
const drop = $('drop');
for (const ev of ['dragenter', 'dragover']) {
  window.addEventListener(ev, (e) => {
    e.preventDefault();
    drop.classList.add('over');
  });
}
for (const ev of ['dragleave', 'drop']) window.addEventListener(ev, () => drop.classList.remove('over'));
window.addEventListener('drop', async (e) => {
  e.preventDefault();
  const f = e.dataTransfer.files[0];
  if (f) return startFile(f.name, new Uint8Array(await f.arrayBuffer()));
  const text = e.dataTransfer.getData('text/plain');
  if (text && !sender) {
    setSource('text');
    textInput.value = text;
    updateTextCount();
  }
});
window.addEventListener('paste', async (e) => {
  if (sender || e.target === textInput) return;
  const f = e.clipboardData.files[0];
  if (f) return startFile(f.name || 'pasted', new Uint8Array(await f.arrayBuffer()));
  const text = e.clipboardData.getData('text');
  if (text) {
    e.preventDefault();
    setSource('text');
    textInput.value = text;
    updateTextCount();
  }
});

// ---------- text input
const textInput = $('textInput');
function setSource(v) {
  prefs.source = v;
  save();
  for (const b of $('srcTabs').querySelectorAll('button')) b.setAttribute('aria-pressed', String(b.dataset.v === v));
  $('paneFile').classList.toggle('hidden', v !== 'file');
  $('paneText').classList.toggle('hidden', v !== 'text');
  if (v === 'text') setTimeout(() => textInput.focus(), 0);
}
$('srcTabs').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) setSource(b.dataset.v);
});
function updateTextCount() {
  const text = textInput.value;
  const bytes = new TextEncoder().encode(text).length;
  $('textCount').textContent = text ? `${[...text].length} ${t('chars')} · ${fmtBytes(bytes)}` : '';
  $('textSend').disabled = !text;
}
textInput.addEventListener('input', updateTextCount);
function sendText() {
  const text = textInput.value;
  if (!text) return;
  startFile(TEXT_NAME, new TextEncoder().encode(text), text);
}
$('textSend').onclick = sendText;
textInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    sendText();
  }
});
setSource(prefs.source === 'text' ? 'text' : 'file');
updateTextCount();

// test hook: ?demo=<bytes> starts with random data
const demo = new URLSearchParams(location.search).get('demo');
if (demo) {
  const n = Math.max(1, Number(demo) || 20000);
  const bytes = new Uint8Array(n);
  crypto.getRandomValues(bytes.subarray(0, Math.min(n, 65536)));
  for (let i = 65536; i < n; i++) bytes[i] = bytes[i % 65536] ^ (i & 255);
  startFile(`demo-${n}.bin`, bytes);
}
