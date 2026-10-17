import { applyI18n, toggleLang, t } from './i18n.js';

const $ = (id) => document.getElementById(id);
applyI18n();
$('lang').onclick = () => {
  toggleLang();
  updateHud();
};

const video = $('video');
const overlay = $('overlay');
const octx = overlay.getContext('2d');
const params = new URLSearchParams(location.search);

// ---------- workers
// phones: 2 decode workers is plenty once scanning is paced; more just burns battery
const MOBILE = matchMedia('(pointer: coarse)').matches;
const NW = Math.max(1, Math.min(MOBILE ? 2 : 3, (navigator.hardwareConcurrency || 4) - 1));
let useBitmap = typeof OffscreenCanvas !== 'undefined' && typeof createImageBitmap !== 'undefined';
let workerErrors = 0;
const workers = [];
for (let i = 0; i < NW; i++) {
  const w = new Worker(new URL('./scan-worker.js', import.meta.url), { type: 'module' });
  w.busy = false;
  w.onmessage = (e) => onResult(w, e.data);
  w.onerror = (e) => {
    w.busy = false;
    console.error('scan worker', e.message);
  };
  workers.push(w);
}
const sink = new Worker(new URL('./sink-worker.js', import.meta.url), { type: 'module' });
sink.onmessage = (e) => onSink(e.data);

// decoding engine: plain JavaScript or the Rust implementation compiled to WebAssembly
let engine = (() => {
  try {
    return localStorage.getItem('lumen.engine') === 'wasm' ? 'wasm' : 'js';
  } catch (e) {
    return 'js';
  }
})();
if (params.get('engine')) engine = params.get('engine') === 'wasm' ? 'wasm' : 'js';
sink.postMessage({ reset: true, engine });
let scanMs = []; // recent per-frame scan times of the current engine
function syncEngine() {
  for (const b of $('segEngine').querySelectorAll('button')) b.setAttribute('aria-pressed', String(b.dataset.v === engine));
}
$('segEngine').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (!b || b.dataset.v === engine) return;
  engine = b.dataset.v;
  try {
    localStorage.setItem('lumen.engine', engine);
  } catch (err) {
    /* ignore */
  }
  syncEngine();
  scanMs = [];
  // a new engine starts a fresh reception
  sink.postMessage({ reset: true, engine });
  finished = false;
  progress = null;
  activeTid = null;
  progHist = [];
  frameDone.clear();
  updateHud();
});
syncEngine();

// ---------- state
let running = false;
let stream = null;
let track = null;
let deviceIds = [];
let deviceIdx = -1;
let frameId = 0;
let fallbackCanvas = null;
let fallbackCtx = null;
let lastResult = null;
let lastFoundAt = 0;
let firstPacketAt = 0;
let progress = null;
let progHist = []; // [t, received]
let scanHist = []; // { t, ok, cwOk, cwTotal, cell }
let finished = false;
let activeTid = null;
// frames already decoded (key -> { t, ok:Set, seq }) so workers can skip repeats
const frameDone = new Map();
let camFrames = []; // timestamps of camera frames
let scanTimes = []; // timestamps of finished scans
let checksumWarnUntil = 0;

// ---------- camera
async function openCamera() {
  $('startErr').textContent = '';
  if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
    $('startErr').textContent = t('needHttps');
    return;
  }
  const res = params.get('res') || '1080';
  const dims = res === '4k' ? [3840, 2160] : res === '720' ? [1280, 720] : [1920, 1080];
  const video0 = {
    width: { ideal: dims[0] },
    height: { ideal: dims[1] },
    frameRate: { ideal: 30 },
  };
  if (deviceIdx >= 0 && deviceIds[deviceIdx]) video0.deviceId = { exact: deviceIds[deviceIdx] };
  else video0.facingMode = { ideal: 'environment' };
  try {
    stopStream();
    stream = await navigator.mediaDevices.getUserMedia({ audio: false, video: video0 });
  } catch (err) {
    $('startErr').textContent = t('camErr') + (err && err.message ? err.message : err);
    return;
  }
  track = stream.getVideoTracks()[0];
  video.srcObject = stream;
  video.removeAttribute('src');
  video.loop = false;
  await video.play().catch(() => {});
  setupTrackControls();
  try {
    const devs = await navigator.mediaDevices.enumerateDevices();
    deviceIds = devs.filter((d) => d.kind === 'videoinput').map((d) => d.deviceId);
    const cur = track.getSettings().deviceId;
    deviceIdx = Math.max(0, deviceIds.indexOf(cur));
    $('sideTools').classList.toggle('hidden', deviceIds.length < 2);
  } catch (e) {
    /* ignore */
  }
  beginScanning();
}

function stopStream() {
  if (stream) for (const tr of stream.getTracks()) tr.stop();
  stream = null;
  track = null;
}

function setupTrackControls() {
  const caps = track && track.getCapabilities ? track.getCapabilities() : {};
  if (caps.focusMode && caps.focusMode.includes('continuous')) {
    track.applyConstraints({ advanced: [{ focusMode: 'continuous' }] }).catch(() => {});
  }
  const box = $('zoomBox');
  if (caps.zoom && caps.zoom.max > caps.zoom.min) {
    const z = $('zoom');
    z.min = caps.zoom.min;
    z.max = Math.min(caps.zoom.max, caps.zoom.min * 8);
    z.step = caps.zoom.step || 0.1;
    z.value = track.getSettings().zoom || caps.zoom.min;
    $('zoomVal').textContent = `${Number(z.value).toFixed(1)}×`;
    z.oninput = () => {
      $('zoomVal').textContent = `${Number(z.value).toFixed(1)}×`;
      track.applyConstraints({ advanced: [{ zoom: Number(z.value) }] }).catch(() => {});
    };
    box.classList.remove('hidden');
  } else box.classList.add('hidden');
}

$('switchCam').onclick = () => {
  if (deviceIds.length < 2) return;
  deviceIdx = (deviceIdx + 1) % deviceIds.length;
  openCamera();
};

// ---------- media file (image or recorded video) as a source, for testing
$('fileBtn').onclick = () => $('mediaFile').click();
$('mediaFile').onchange = async () => {
  const f = $('mediaFile').files[0];
  if (!f) return;
  stopStream();
  const url = URL.createObjectURL(f);
  if (f.type.startsWith('video/')) {
    video.srcObject = null;
    video.src = url;
    video.loop = true;
    await video.play().catch(() => {});
    beginScanning();
    return;
  }
  const img = new Image();
  img.onload = () => {
    video.srcObject = null;
    video.poster = url;
    beginScanning(img);
  };
  img.src = url;
};

let stillImage = null;
function beginScanning(still) {
  $('start').classList.add('hidden');
  $('hud').classList.remove('hidden');
  stillImage = still || null;
  running = true;
  if (!firstPacketAt) firstPacketAt = 0;
  wake();
  updateHud();
  if (stillImage) {
    // decode the still a few times (each worker once)
    for (const w of workers) grab(w);
    return;
  }
  pump();
}

function sourceSize() {
  if (stillImage) return [stillImage.naturalWidth, stillImage.naturalHeight];
  return [video.videoWidth, video.videoHeight];
}

// Scan pacing: about 2.2x the sender's distinct frame rate is enough to catch every frame;
// scanning every camera frame mostly re-reads frames already decoded and heats the phone.
let lastDispatch = 0;
let senderFpsEst = 0;
function scanInterval(now) {
  if (finished) return 1000 / 3;
  if (now - lastFoundAt > 2000) return 1000 / 5; // searching
  const rate = senderFpsEst > 0 ? Math.min(30, Math.max(6, senderFpsEst * 2.2)) : 15;
  return 1000 / rate;
}

function pump() {
  if (!running || stillImage) return;
  const now = performance.now();
  camFrames.push(now);
  while (camFrames.length && now - camFrames[0] > 2000) camFrames.shift();
  if (video.readyState >= 2 && video.videoWidth && now - lastDispatch >= scanInterval(now) - 4) {
    const free = workers.find((w) => !w.busy);
    if (free) {
      lastDispatch = now;
      grab(free);
    }
  }
  if ('requestVideoFrameCallback' in video) video.requestVideoFrameCallback(pump);
  else requestAnimationFrame(pump);
}

function skipMap() {
  const now = performance.now();
  const out = {};
  for (const [key, e] of frameDone) {
    if (now - e.t > 4000) frameDone.delete(key);
    else out[key] = [...e.ok];
  }
  return out;
}

async function grab(w) {
  w.busy = true;
  const id = ++frameId;
  const skip = skipMap();
  const src = stillImage || video;
  const [sw, sh] = sourceSize();
  try {
    if (useBitmap) {
      const bmp = await createImageBitmap(src);
      w.postMessage({ id, bitmap: bmp, skip, engine }, [bmp]);
    } else {
      if (!fallbackCanvas || fallbackCanvas.width !== sw || fallbackCanvas.height !== sh) {
        fallbackCanvas = document.createElement('canvas');
        fallbackCanvas.width = sw;
        fallbackCanvas.height = sh;
        fallbackCtx = fallbackCanvas.getContext('2d', { willReadFrequently: true });
      }
      fallbackCtx.drawImage(src, 0, 0, sw, sh);
      const im = fallbackCtx.getImageData(0, 0, sw, sh);
      w.postMessage({ id, image: { width: sw, height: sh, data: im.data }, skip, engine }, [im.data.buffer]);
    }
  } catch (err) {
    w.busy = false;
    console.warn('grab failed', err);
  }
}

// ---------- results
function onResult(w, d) {
  w.busy = false;
  if (d.reason === 'error') {
    console.warn(d.error);
    if (useBitmap && ++workerErrors >= 2) useBitmap = false; // no OffscreenCanvas 2D in workers
    return;
  }
  const now = performance.now();
  lastResult = d;
  if (d.engine === engine && d.ms) {
    scanMs.push(d.ms);
    if (scanMs.length > 30) scanMs.shift();
  }
  scanTimes.push(now);
  while (scanTimes.length && now - scanTimes[0] > 2000) scanTimes.shift();
  if (d.frameKey) {
    let e = frameDone.get(d.frameKey);
    if (!e) {
      e = { t: now, first: now, ok: new Set(), seq: d.header.seq, total: d.stats ? d.stats.cwCount : 0 };
      frameDone.set(d.frameKey, e);
    }
    e.t = now;
    for (const i of d.okIdx || []) e.ok.add(i);
  }
  const rec = { t: now, ok: d.ok, cwOk: 0, cwTotal: 0, cell: 0, skipped: !!d.skipped };
  if (d.ok) {
    lastFoundAt = now;
    rec.cwOk = d.stats.cwOk;
    rec.cwTotal = d.stats.cwTotal;
    rec.cell = d.stats.cellPx;
  }
  scanHist.push(rec);
  while (scanHist.length && now - scanHist[0].t > 2500) scanHist.shift();
  drawOverlay(d);
  if (d.packets && d.packets.length && !finished) {
    if (!firstPacketAt) firstPacketAt = now;
    sink.postMessage({ packets: d.packets }, d.packets.map((p) => p.buffer));
  }
  updateHud();
}

function onSink(m) {
  if (m.type === 'progress' && m.prog) {
    const now = performance.now();
    if (m.prog.tid !== activeTid) {
      activeTid = m.prog.tid;
      progHist = [];
      firstPacketAt = now;
    }
    progress = m.prog;
    progHist.push([now, m.prog.received]);
    while (progHist.length > 2 && now - progHist[0][0] > 3000) progHist.shift();
    updateHud();
  } else if (m.type === 'done') {
    showDone(m.name, m.bytes);
  } else if (m.type === 'error') {
    checksumWarnUntil = performance.now() + 3000;
    console.warn('sink', m.error);
  }
}

// ---------- overlay: detected code outline
function drawOverlay(d) {
  const dpr = window.devicePixelRatio || 1;
  const W = overlay.clientWidth;
  const H = overlay.clientHeight;
  if (overlay.width !== Math.round(W * dpr) || overlay.height !== Math.round(H * dpr)) {
    overlay.width = Math.round(W * dpr);
    overlay.height = Math.round(H * dpr);
  }
  octx.setTransform(dpr, 0, 0, dpr, 0, 0);
  octx.clearRect(0, 0, W, H);
  if (!d.ok || !d.stats) return;
  const [iw, ih] = [d.w, d.h];
  const scale = Math.max(W / iw, H / ih); // object-fit: cover
  const ox = (W - iw * scale) / 2;
  const oy = (H - ih * scale) / 2;
  const pts = d.stats.corners.map(([x, y]) => [ox + x * scale, oy + y * scale]);
  const e = d.frameKey && frameDone.get(d.frameKey);
  const rate = e && e.total ? e.ok.size / e.total : d.skipped ? 1 : 0;
  const color = rate > 0.8 ? '#2bdc74' : rate > 0.3 ? '#ffd60a' : '#ff5468';
  octx.lineWidth = 3;
  octx.strokeStyle = color;
  octx.lineJoin = 'round';
  octx.shadowColor = color;
  octx.shadowBlur = 12;
  octx.beginPath();
  pts.forEach(([x, y], i) => (i ? octx.lineTo(x, y) : octx.moveTo(x, y)));
  octx.closePath();
  octx.stroke();
}

// ---------- HUD
function fmtBytes(n) {
  if (n < 1024) return `${Math.round(n)} B`;
  if (n < 1048576) return `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`;
  return `${(n / 1048576).toFixed(1)} MB`;
}
function fmtTime(s) {
  if (!isFinite(s) || s < 0) return '–';
  if (s < 60) return `${Math.ceil(s)}s`;
  return `${Math.floor(s / 60)}m${String(Math.ceil(s % 60)).padStart(2, '0')}s`;
}

function updateHud() {
  const now = performance.now();
  const found = now - lastFoundAt < 700;
  $('statusDot').className = `dot ${found ? 'live' : ''}`;
  $('statusText').textContent = found ? t('receiving') : t('waiting');

  let okRate = 0;
  let cell = 0;
  let cellN = 0;
  let cwOk = 0;
  let cwTot = 0;
  for (const r of scanHist) {
    cwOk += r.cwOk;
    cwTot += r.cwTotal;
    if (r.ok) {
      cell += r.cell;
      cellN++;
    }
  }
  // success = share of each recently seen frame's codewords that were recovered
  let covOk = 0;
  let covTot = 0;
  for (const e of frameDone.values())
    if (now - e.first < 2500 && e.total) {
      covOk += e.ok.size;
      covTot += e.total;
    }
  okRate = covTot ? covOk / covTot : 0;
  cwTot = covTot;
  cell = cellN ? cell / cellN : 0;
  $('mCell').textContent = cellN ? `${cell.toFixed(1)}px` : '–';
  $('mOk').textContent = covTot ? `${Math.round(okRate * 100)}%` : '–';

  if (progress) {
    $('rxName').textContent = progress.name || t('receiving');
    const pct = progress.needed ? Math.min(progress.done ? 100 : 99, (100 * progress.received) / progress.needed) : 0;
    $('rxPct').textContent = `${Math.floor(pct)}%`;
    $('rxBar').style.width = `${pct}%`;
    let rate = 0;
    if (progHist.length >= 2) {
      const [t0, r0] = progHist[0];
      const [t1, r1] = progHist[progHist.length - 1];
      if (t1 > t0) rate = ((r1 - r0) * 1000) / (t1 - t0);
    }
    $('mSpeed').textContent = rate > 0 ? `${fmtBytes(rate * (progress.T || 0))}/s` : '–';
    $('mEta').textContent = progress.needed && rate > 0 ? fmtTime((progress.needed - progress.received) / rate) : '–';
  }

  // capture / decode diagnostics: which link limits the rate?
  const camFps = camFrames.length > 1 ? ((camFrames.length - 1) * 1000) / (camFrames[camFrames.length - 1] - camFrames[0]) : 0;
  const scanFps = scanTimes.length > 1 ? ((scanTimes.length - 1) * 1000) / (scanTimes[scanTimes.length - 1] - scanTimes[0]) : 0;
  const recent = [...frameDone.values()].filter((e) => now - e.first < 3000).sort((a, b) => a.first - b.first);
  let span = 0;
  for (let i = 1; i < recent.length; i++) {
    const dlt = (recent[i].seq - recent[i - 1].seq + 256) & 255;
    if (dlt > 0 && dlt < 64) span += dlt;
  }
  const dt = recent.length > 1 ? (recent[recent.length - 1].first - recent[0].first) / 1000 : 0;
  const senderFps = dt > 0.5 ? span / dt : 0;
  if (senderFps > 0) senderFpsEst = senderFps;
  const distinctFps = dt > 0.5 ? (recent.length - 1) / dt : 0;
  const missed = span > 0 ? Math.max(0, 1 - (recent.length - 1) / span) : 0;
  const avgMs = scanMs.length ? scanMs.reduce((a, b) => a + b, 0) / scanMs.length : 0;
  $('diag').textContent =
    `${engine.toUpperCase()} ${avgMs ? avgMs.toFixed(1) + ' ms' : '–'} · ${t('camFps')} ${camFps.toFixed(0)} · ${t('scanFps')} ${scanFps.toFixed(0)}/s · ${t('frameFps')} ${distinctFps.toFixed(1)}/s` +
    (senderFps ? ` · ${t('missed')} ${Math.round(missed * 100)}%` : '');

  const tip = $('tip');
  let msg = '';
  let good = false;
  if (now < checksumWarnUntil) msg = t('checksum');
  else if (!found) msg = t('tipSearch');
  else if (cellN && cell < 3.4) {
    const h = lastResult && lastResult.header;
    const portrait = video.videoHeight > video.videoWidth;
    msg = h && h.X > h.Y * 1.2 && portrait ? t('tipLandscape') : t('tipCloser');
  } else if (cwTot && okRate < 0.5) msg = cell < 4.5 ? t('tipSteady') : t('tipColor');
  else if (senderFps && missed > 0.3 && dt > 1.5) msg = t('tipFpsDown').replace('{n}', Math.max(4, Math.round(distinctFps * 1.1)));
  else if (senderFps && missed < 0.05 && okRate > 0.85 && dt > 2 && camFps > 2.6 * senderFps && scanFps > 2 * senderFps) {
    msg = t('tipFpsUp').replace('{n}', Math.round(Math.min(camFps / 2.2, senderFps * 1.5)));
    good = true;
  } else {
    msg = t('tipGood');
    good = true;
  }
  tip.textContent = msg;
  tip.classList.toggle('ok', good);
}
setInterval(() => running && updateHud(), 500);

// ---------- completion
const MIME = {
  png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif', webp: 'image/webp', svg: 'image/svg+xml',
  avif: 'image/avif', pdf: 'application/pdf', txt: 'text/plain;charset=utf-8', md: 'text/plain;charset=utf-8',
  json: 'application/json', csv: 'text/csv', html: 'text/html', mp4: 'video/mp4', mov: 'video/quicktime',
  mp3: 'audio/mpeg', wav: 'audio/wav', zip: 'application/zip',
};
const TEXTY = /\.(txt|md|json|csv|log|js|ts|py|xml|yaml|yml|ini|conf|sh|c|h|cpp|go|rs|java|css)$/i;

let lastUrl = null;
// Text messages arrive as UTF-8 "message.txt" (the sender's text tab); show them as text.
const TEXT_NAME = 'message.txt';
function asText(name, bytes) {
  if ((name !== TEXT_NAME && !TEXTY.test(name)) || bytes.length > 4_000_000) return null;
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch (e) {
    return null;
  }
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (e) {
    // fallback for browsers without async clipboard access
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.cssText = 'position:fixed;opacity:0;top:0;left:0';
    document.body.appendChild(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand('copy');
    } catch (err) {
      /* ignore */
    }
    ta.remove();
    return ok;
  }
}

// Display limit only: Copy and Save always use the complete text.
const TEXT_DISPLAY_LIMIT = 1_000_000;
let truncated = false;

// Long previews scroll inside the panel; phones show no scrollbar, so say so and fade the edge.
function updateMoreHint() {
  const pv = $('preview');
  const hint = $('moreHint');
  const overflow = !pv.classList.contains('hidden') && pv.scrollHeight > pv.clientHeight + 4;
  const atEnd = pv.scrollTop + pv.clientHeight >= pv.scrollHeight - 8;
  pv.classList.toggle('fade', overflow && !atEnd);
  const msgs = [];
  if (overflow) msgs.push(t('scrollHint'));
  if (truncated) msgs.push(t('truncatedHint').replace('{n}', TEXT_DISPLAY_LIMIT.toLocaleString()));
  hint.textContent = msgs.join(' · ');
  hint.classList.toggle('hidden', !msgs.length);
}
$('preview').addEventListener('scroll', updateMoreHint, { passive: true });
window.addEventListener('resize', () => !$('done').classList.contains('hidden') && updateMoreHint());

function showDone(name, bytes) {
  finished = true;
  truncated = false;
  running = false;
  const ext = (name.split('.').pop() || '').toLowerCase();
  const blob = new Blob([bytes], { type: MIME[ext] || 'application/octet-stream' });
  if (lastUrl) URL.revokeObjectURL(lastUrl);
  lastUrl = URL.createObjectURL(blob);
  const text = asText(name, bytes);
  const message = text !== null && name === TEXT_NAME;
  const secs = firstPacketAt ? (performance.now() - firstPacketAt) / 1000 : 0;
  const timing = secs >= 0.5 ? ` · ${secs.toFixed(1)}s · ${fmtBytes(bytes.length / secs)}/s` : '';
  $('doneName').textContent = message ? t('textReceived') : name;
  $('doneSub').textContent = (message ? `${[...text].length} ${t('chars')} · ${fmtBytes(bytes.length)}` : fmtBytes(bytes.length)) + timing;

  const save = $('saveBtn');
  save.href = lastUrl;
  save.download = name;
  save.textContent = message ? t('saveAsFile') : t('save');
  save.className = message ? 'btn' : 'btn primary';
  $('openBtn').href = lastUrl;
  $('openBtn').classList.toggle('hidden', message);

  const copy = $('copyBtn');
  copy.classList.toggle('hidden', text === null);
  copy.textContent = t('copy');
  copy.onclick = async () => {
    if (await copyText(text)) {
      copy.textContent = t('copied');
      setTimeout(() => (copy.textContent = t('copy')), 1500);
    }
  };
  const link = $('linkBtn');
  const url = text !== null ? text.trim() : '';
  const isLink = /^https?:\/\/\S+$/i.test(url);
  link.classList.toggle('hidden', !isLink);
  if (isLink) link.href = url;

  const pv = $('preview');
  pv.innerHTML = '';
  pv.className = 'preview hidden';
  if ((MIME[ext] || '').startsWith('image/')) {
    const img = document.createElement('img');
    img.src = lastUrl;
    pv.appendChild(img);
    pv.classList.remove('hidden');
  } else if (text !== null) {
    const pre = document.createElement('pre');
    truncated = text.length > TEXT_DISPLAY_LIMIT;
    pre.textContent = truncated ? text.slice(0, TEXT_DISPLAY_LIMIT) : text;
    pv.appendChild(pre);
    pv.classList.remove('hidden');
    if (message) pv.classList.add('text');
  }
  $('done').querySelector('.panel').classList.toggle('wide', text !== null);
  $('done').classList.remove('hidden');
  requestAnimationFrame(updateMoreHint);
  $('rxPct').textContent = '100%';
  $('rxBar').style.width = '100%';
  octx.clearRect(0, 0, overlay.width, overlay.height);
}

$('againBtn').onclick = () => {
  $('done').classList.add('hidden');
  sink.postMessage({ reset: true, engine });
  finished = false;
  progress = null;
  activeTid = null;
  progHist = [];
  scanHist = [];
  frameDone.clear();
  firstPacketAt = 0;
  $('rxName').textContent = t('waiting');
  $('rxPct').textContent = '0%';
  $('rxBar').style.width = '0';
  $('mSpeed').textContent = $('mEta').textContent = '–';
  if (stream || video.src) {
    running = true;
    pump();
  } else {
    $('start').classList.remove('hidden');
    $('hud').classList.add('hidden');
  }
};

let wakeLock = null;
async function wake() {
  try {
    if ('wakeLock' in navigator && !wakeLock) {
      wakeLock = await navigator.wakeLock.request('screen');
      wakeLock.addEventListener('release', () => (wakeLock = null));
    }
  } catch (e) {
    /* ignore */
  }
}
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible' && running) wake();
});

$('startBtn').onclick = openCamera;
if (!window.isSecureContext) $('startErr').textContent = t('needHttps');
