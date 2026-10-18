import { applyI18n, t } from './i18n.js?v=5fd5612805';
import { isJson, formatJson, linkify } from './text-view.js?v=5fd5612805';

const $ = (id) => document.getElementById(id);
applyI18n();

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
  const w = new Worker(new URL('./scan-worker.js?v=5fd5612805', import.meta.url), { type: 'module' });
  w.busy = false;
  w.onmessage = (e) => onResult(w, e.data);
  w.onerror = (e) => {
    w.busy = false;
    console.error('scan worker', e.message);
  };
  workers.push(w);
}
const sink = new Worker(new URL('./sink-worker.js?v=5fd5612805', import.meta.url), { type: 'module' });
sink.onmessage = (e) => onSink(e.data);

// decoding engine: plain JavaScript or the Rust implementation compiled to WebAssembly
let engine = (() => {
  try {
    return localStorage.getItem('lumen.engine') === 'js' ? 'js' : 'wasm'; // WASM: faster, cooler
  } catch (e) {
    return 'wasm';
  }
})();
if (params.get('engine')) engine = params.get('engine') === 'wasm' ? 'wasm' : 'js';
sink.postMessage({ reset: true, engine });
let scanMs = []; // recent per-frame scan times of the current engine
let engineNoteUntil = 0;
let engineNote = '';
function wasmFailed(why) {
  if (engine !== 'wasm') return;
  engine = 'js';
  syncEngine();
  sink.postMessage({ engine }); // keeps the packets received so far
  engineNote = t('wasmFailed') + (why ? ` (${String(why).split('\n')[0].slice(0, 80)})` : '');
  engineNoteUntil = performance.now() + 8000;
  updateHud();
}
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
  session.msSum = session.msN = 0; // engine time in the summary = the engine that finished
  // both engines read the same packets: the reception continues where it is
  sink.postMessage({ engine });
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
// totals over the whole transfer, shown on the result panel
let session = null;
function resetSession() {
  session = { scans: 0, frames: 0, covOk: 0, covTot: 0, msSum: 0, msN: 0, prepSum: 0, pxSum: 0, cellSum: 0, cellN: 0, header: null, received: 0, T: 0 };
  if (params.has('debug')) self.lumenSession = session; // for measurements
}
resetSession();

// ---------- camera
async function openCamera() {
  $('startErr').textContent = '';
  if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
    $('startErr').textContent = t('needHttps');
    return;
  }
  const res = params.get('res') || '1080';
  const dims = res === '4k' ? [3840, 2160] : res === '720' ? [1280, 720] : [1920, 1080];
  // 30 fps to start; adaptCameraRate() asks for 60 when the sender plays fast enough
  videoSize = { width: { ideal: dims[0] }, height: { ideal: dims[1] } };
  camRate = 30;
  const video0 = { ...videoSize, frameRate: { ideal: 30, max: 30 } };
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

// The camera's rate follows the sender: at 30 fps every frame of a sender up to ~15 fps is
// seen twice; a faster sender needs 60 (when the camera has it) to be seen at all reliably.
// Scans stay locked to the sender, so a faster camera adds captures, not scans.
let videoSize = {};
let camRate = 30;
let camRateAt = 0;
function adaptCameraRate(sender, now) {
  if (!track || !track.applyConstraints || now - camRateAt < 2000) return;
  const max = (track.getCapabilities && track.getCapabilities().frameRate && track.getCapabilities().frameRate.max) || 30;
  let want = camRate;
  if (camRate <= 30 && sender >= 16 && max >= 59) want = 60;
  else if (camRate > 30 && sender > 0 && sender < 13) want = 30;
  if (want === camRate) return;
  camRate = want;
  camRateAt = now;
  // the whole set: applyConstraints replaces the constraints, the size included
  track.applyConstraints({ ...videoSize, frameRate: { ideal: want, max: want } }).catch(() => {});
}

function setupTrackControls() {
  const caps = track && track.getCapabilities ? track.getCapabilities() : {};
  if (caps.focusMode && caps.focusMode.includes('continuous')) {
    track.applyConstraints({ advanced: [{ focusMode: 'continuous' }] }).catch(() => {});
  }
  if (caps.zoom && caps.zoom.max > caps.zoom.min) {
    zoomRange = { min: caps.zoom.min, max: Math.min(caps.zoom.max, caps.zoom.min * 8) };
    // a restarted camera (after "receive another") keeps the level the user chose
    setZoom(zoomLevel ? zoomLevel : track.getSettings().zoom || caps.zoom.min);
  } else zoomRange = null;
  $('zoomChip').classList.toggle('hidden', !zoomRange);
}

// ---------- zoom: pinch on the camera image; ctrl + wheel (trackpad pinch) on desktop
let zoomRange = null;
let zoomLevel = 0;
let zoomPending = false;
function setZoom(z) {
  if (!zoomRange || !track) return;
  zoomLevel = Math.min(zoomRange.max, Math.max(zoomRange.min, z));
  $('zoomVal').textContent = `${zoomLevel.toFixed(1)}×`;
  $('zoomChip').classList.toggle('active', zoomLevel > zoomRange.min + 0.05);
  if (zoomPending) return; // one constraint update per animation frame
  zoomPending = true;
  requestAnimationFrame(() => {
    zoomPending = false;
    if (track) track.applyConstraints({ advanced: [{ zoom: zoomLevel }] }).catch(() => {});
  });
}
{
  const stage = $('stage');
  const onImage = (e) => !e.target.closest('.topbar, .recv-bottom, .side-tools, .zoom-chip, .start, .done');
  let pinch = null;
  const spread = (t) => Math.hypot(t[0].clientX - t[1].clientX, t[0].clientY - t[1].clientY);
  stage.addEventListener('touchstart', (e) => {
    if (e.touches.length === 2 && zoomRange && onImage(e)) pinch = { d: spread(e.touches), z: zoomLevel };
  }, { passive: true });
  stage.addEventListener('touchmove', (e) => {
    if (!pinch || e.touches.length !== 2) return;
    e.preventDefault();
    setZoom(pinch.z * (spread(e.touches) / pinch.d));
  }, { passive: false });
  stage.addEventListener('touchend', (e) => {
    if (e.touches.length < 2) pinch = null;
  });
  // iOS Safari: keep the page itself from zooming
  for (const ev of ['gesturestart', 'gesturechange']) stage.addEventListener(ev, (e) => onImage(e) && e.preventDefault());
  stage.addEventListener('wheel', (e) => {
    if (!zoomRange || !onImage(e)) return;
    e.preventDefault();
    setZoom(zoomLevel * Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.002)));
  }, { passive: false });
  $('zoomChip').onclick = () => zoomRange && setZoom(zoomRange.min);
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
  crop = null;
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

// Frame-locked scanning. Each sender frame needs one good capture; more scans of the same
// frame only cost battery. After a frame is fully decoded, wait ~3/4 of a sender frame
// period; after a repeat, ~1/3; after a partial decode, rescan at once.
let lastDispatch = 0;
let senderFpsEst = 0;
let nextScanAt = 0;
let lastDecisionAt = 0; // capture time of the newest result used for scheduling
const captureAt = new Map(); // scan id -> capture time
function senderPeriod() {
  return 1000 / (senderFpsEst > 0 ? senderFpsEst : 12);
}
function scanDue(now) {
  if (finished) return now - lastDispatch >= 1000 / 3;
  // searching: 5 scans/s, and 2/s once nothing has been in view for 10 s (phone put down)
  if (now - lastFoundAt > 10000) return now - lastDispatch >= 1000 / 2;
  if (now - lastFoundAt > 2000) return now - lastDispatch >= 1000 / 5;
  if (now - lastDispatch < Math.min(1000 / 30, senderPeriod() / 3)) return false;
  return now >= nextScanAt;
}
function schedule(d, capturedAt) {
  if (capturedAt < lastDecisionAt) return; // an older capture finished late
  lastDecisionAt = capturedAt;
  const P = senderPeriod();
  if (!d.ok) nextScanAt = 0;
  else if (d.skipped) nextScanAt = capturedAt + 0.33 * P;
  else {
    const e = d.frameKey && frameDone.get(d.frameKey);
    nextScanAt = e && e.total && e.ok.size >= e.total ? capturedAt + 0.75 * P : 0;
  }
}

function pump() {
  if (!running || stillImage) return;
  const now = performance.now();
  camFrames.push(now);
  while (camFrames.length && now - camFrames[0] > 2000) camFrames.shift();
  if (video.readyState >= 2 && video.videoWidth && scanDue(now)) {
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

// Region of interest and processing scale, from the newest result. Only this part of the
// camera frame is copied out of the GPU, already downscaled so that a cell is about
// TARGET_CELL pixels: the copy and read-back cost more than the scan itself at full size.
const TARGET_CELL = 6;
let crop = null; // { x, y, w, h, k } in camera pixels
let cropAt = 0;
function updateCrop(d, capturedAt) {
  if (capturedAt < cropAt || stillImage) return;
  cropAt = capturedAt;
  if (!d.ok || !d.stats || !d.w) {
    crop = null; // lost: whole frame, full resolution
    return;
  }
  const xs = d.stats.corners.map((c) => c[0]);
  const ys = d.stats.corners.map((c) => c[1]);
  const mx = (Math.max(...xs) - Math.min(...xs)) * 0.18 + 24;
  const my = (Math.max(...ys) - Math.min(...ys)) * 0.18 + 24;
  const x0 = Math.max(0, Math.floor(Math.min(...xs) - mx));
  const y0 = Math.max(0, Math.floor(Math.min(...ys) - my));
  const x1 = Math.min(d.w, Math.ceil(Math.max(...xs) + mx));
  const y1 = Math.min(d.h, Math.ceil(Math.max(...ys) + my));
  const k = Math.min(1, Math.max(0.35, TARGET_CELL / d.stats.cellPx));
  if (x1 - x0 < 64 || y1 - y0 < 64) crop = null;
  else crop = { x: x0, y: y0, w: x1 - x0, h: y1 - y0, k, fw: d.w, fh: d.h };
}

async function grab(w) {
  w.busy = true;
  const id = ++frameId;
  captureAt.set(id, performance.now());
  const skip = skipMap();
  const src = stillImage || video;
  const [sw, sh] = sourceSize();
  // a crop measured on another frame size (the phone was turned) no longer applies
  const r = crop && crop.fw === sw && crop.fh === sh ? crop : { x: 0, y: 0, w: sw, h: sh, k: 1 };
  const dw = Math.max(32, Math.round(r.w * r.k));
  const dh = Math.max(32, Math.round(r.h * r.k));
  const region = { x: r.x, y: r.y, kx: dw / r.w, ky: dh / r.h, fullW: sw, fullH: sh };
  try {
    if (useBitmap) {
      // crop + resize happen on the GPU; the worker receives only the small bitmap
      const opts = dw < r.w ? { resizeWidth: dw, resizeHeight: dh, resizeQuality: 'medium' } : undefined;
      const bmp = await createImageBitmap(src, r.x, r.y, r.w, r.h, opts);
      w.postMessage({ id, bitmap: bmp, region, dw, dh, skip, engine }, [bmp]);
    } else {
      if (!fallbackCanvas) {
        fallbackCanvas = document.createElement('canvas');
        fallbackCtx = fallbackCanvas.getContext('2d', { willReadFrequently: true });
      }
      if (fallbackCanvas.width < dw || fallbackCanvas.height < dh) {
        fallbackCanvas.width = Math.max(fallbackCanvas.width, dw);
        fallbackCanvas.height = Math.max(fallbackCanvas.height, dh);
      }
      fallbackCtx.drawImage(src, r.x, r.y, r.w, r.h, 0, 0, dw, dh);
      const im = fallbackCtx.getImageData(0, 0, dw, dh);
      w.postMessage({ id, image: { width: dw, height: dh, data: im.data }, region, skip, engine }, [im.data.buffer]);
    }
  } catch (err) {
    w.busy = false;
    console.warn('grab failed', err);
  }
}

// ---------- results
function onResult(w, d) {
  w.busy = false;
  const capturedAt = captureAt.get(d.id) ?? performance.now();
  captureAt.delete(d.id);
  if (d.reason === 'error') {
    console.warn(d.error);
    if (d.wasmError) wasmFailed(d.error);
    else if (useBitmap && ++workerErrors >= 2) useBitmap = false; // no OffscreenCanvas 2D in workers
    nextScanAt = 0;
    return;
  }
  const now = performance.now();
  lastResult = d;
  if (d.prepMs !== undefined) {
    session.prepSum += d.prepMs;
    session.pxSum += d.px || 0;
  }
  if (d.engine === engine && d.ms) {
    scanMs.push(d.ms);
    if (scanMs.length > 30) scanMs.shift();
    session.msSum += d.ms;
    session.msN++;
  }
  session.scans++;
  scanTimes.push(now);
  while (scanTimes.length && now - scanTimes[0] > 2000) scanTimes.shift();
  if (d.frameKey) {
    let e = frameDone.get(d.frameKey);
    if (!e) {
      e = { t: now, first: now, ok: new Set(), seq: d.header.seq, total: d.stats ? d.stats.cwCount : 0 };
      frameDone.set(d.frameKey, e);
      session.frames++;
      session.covTot += e.total;
    }
    e.t = now;
    for (const i of d.okIdx || []) {
      if (!e.ok.has(i)) session.covOk++;
      e.ok.add(i);
    }
    session.header = d.header;
  }
  if (d.ok && d.stats && !d.skipped) {
    session.cellSum += d.stats.cellPx;
    session.cellN++;
  }
  schedule(d, capturedAt);
  updateCrop(d, capturedAt);
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
    session.received = m.prog.received;
    session.T = m.prog.T || session.T;
    progHist.push([now, m.prog.received]);
    while (progHist.length > 2 && now - progHist[0][0] > 3000) progHist.shift();
    updateHud();
  } else if (m.type === 'done') {
    showDone(m.name, m.bytes);
  } else if (m.type === 'error') {
    if (m.wasmError) return wasmFailed(m.error);
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
  const rate = e && e.total ? e.ok.size / e.total : d.skipped ? 1 : d.stats.cwCount ? (d.okIdx || []).length / d.stats.cwCount : 0;
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
  if (senderFps > 0 && !stillImage) adaptCameraRate(senderFps, now);
  const distinctFps = dt > 0.5 ? (recent.length - 1) / dt : 0;
  // share of the sender's frames that were captured; with a fountain code a missed frame
  // costs only its own data, nothing has to be resent
  const captured = span > 0 ? Math.min(1, (recent.length - 1) / span) : 0;
  const avgMs = scanMs.length ? scanMs.reduce((a, b) => a + b, 0) / scanMs.length : 0;
  // fraction of time the decode workers are busy
  const load = (avgMs * scanFps) / 1000 / NW;
  $('diag').textContent =
    `${engine.toUpperCase()} ${avgMs ? avgMs.toFixed(1) + ' ms' : '–'} · ${t('camFps')} ${camFps.toFixed(0)} · ${t('scanFps')} ${scanFps.toFixed(0)}/s · ${t('frameFps')} ${distinctFps.toFixed(1)}/s` +
    (senderFps ? ` · ${t('senderFps')} ${senderFps.toFixed(0)}/s · ${t('captured')} ${Math.round(captured * 100)}%` : '');

  const tip = $('tip');
  let msg = '';
  let good = false;
  if (now < engineNoteUntil) msg = engineNote;
  else if (now < checksumWarnUntil) msg = t('checksum');
  else if (!found) msg = t('tipSearch');
  else if (cellN && cell < 3.4) {
    const h = lastResult && lastResult.header;
    const portrait = video.videoHeight > video.videoWidth;
    msg = h && h.X > h.Y * 1.2 && portrait ? t('tipLandscape') : t('tipCloser');
  } else if (cwTot && okRate < 0.5) msg = cell < 4.5 ? t('tipSteady') : t('tipColor');
  else if (engine === 'js' && senderFps && captured < 0.8 && load > 0.8) msg = t('tipWasm');
  else if (senderFps && captured > 0.95 && okRate > 0.9 && dt > 2 && load < 0.5 && camFps > 2.2 * senderFps) {
    msg = t('tipFpsUp').replace('{n}', Math.round(Math.min(camFps / 2, senderFps * 1.5)));
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

// Display limits only: Copy and Save always use the complete text.
const TEXT_DISPLAY_LIMIT = 1_000_000;
const HIGHLIGHT_LIMIT = 300_000; // larger JSON is formatted without colours to keep the DOM small
const LINKIFY_LIMIT = 200_000;
let truncated = false;

// Reader view (received text or image fills the screen). Choices persist per browser.
const reader = (() => {
  try {
    return JSON.parse(localStorage.getItem('lumen.reader')) || {};
  } catch (e) {
    return {};
  }
})();
function saveReader() {
  try {
    localStorage.setItem('lumen.reader', JSON.stringify(reader));
  } catch (e) {
    /* ignore */
  }
}
let shown = null; // { text, json, plain, lines }

function renderText() {
  const pv = $('preview');
  const c = shown;
  const pretty = c.json && reader.view !== 'raw';
  const pre = document.createElement('pre');
  truncated = false;
  if (pretty) {
    const colour = c.text.length <= HIGHLIGHT_LIMIT;
    c.lines = c.lines || formatJson(c.text, { html: colour });
    if (colour) {
      pre.className = 'json';
      pre.style.setProperty('--gut', `${String(c.lines.length).length + 2}ch`);
      // indentation becomes padding, so a wrapped line continues under its own start (hanging indent)
      pre.innerHTML = c.lines
        .map((l) => {
          const sp = l.search(/\S|$/);
          return `<span class="l" style="--d:${sp / 2}">${l.slice(sp)}</span>`;
        })
        .join('');
    } else {
      const all = c.lines.join('\n');
      truncated = all.length > TEXT_DISPLAY_LIMIT;
      pre.textContent = truncated ? all.slice(0, TEXT_DISPLAY_LIMIT) : all;
    }
  } else {
    truncated = c.text.length > TEXT_DISPLAY_LIMIT;
    const body = truncated ? c.text.slice(0, TEXT_DISPLAY_LIMIT) : c.text;
    if (c.plain && body.length <= LINKIFY_LIMIT) linkify(pre, body);
    else pre.textContent = body;
  }
  pv.className = 'preview';
  pv.classList.toggle('text', c.plain);
  pv.classList.toggle('nowrap', !c.plain && reader.wrap === false);
  pv.style.setProperty('--rfs-d', reader.fs || 0);
  pv.replaceChildren(pre);
  pv.scrollTop = 0;
  pv.scrollLeft = 0;
  for (const b of $('segView').querySelectorAll('button')) b.setAttribute('aria-pressed', String((b.dataset.v === 'pretty') === pretty));
  $('wrapBtn').classList.toggle('hidden', c.plain);
  $('wrapBtn').setAttribute('aria-pressed', String(reader.wrap !== false));
  requestAnimationFrame(updateMoreHint);
}

function applyStatsToggle() {
  const on = reader.stats ?? innerWidth >= 720; // open by default on wide screens, folded on phones
  $('done').classList.toggle('show-stats', on);
  $('statsBtn').setAttribute('aria-expanded', String(on));
}
$('statsBtn').onclick = () => {
  reader.stats = !$('done').classList.contains('show-stats');
  saveReader();
  applyStatsToggle();
};
$('segView').addEventListener('click', (e) => {
  const v = e.target.closest('button')?.dataset.v;
  if (!v || !shown) return;
  reader.view = v;
  saveReader();
  renderText();
});
$('wrapBtn').onclick = () => {
  reader.wrap = reader.wrap === false;
  saveReader();
  if (shown) renderText();
};
for (const [id, d] of [['fontDown', -1], ['fontUp', 1]]) {
  $(id).onclick = () => {
    reader.fs = Math.max(-4, Math.min(8, (reader.fs || 0) + d));
    saveReader();
    $('preview').style.setProperty('--rfs-d', reader.fs);
  };
}

// Long previews scroll; phones show no scrollbar, so fade the edge (and say so in the small card).
function updateMoreHint() {
  const pv = $('preview');
  const hint = $('moreHint');
  const overflow = !pv.classList.contains('hidden') && pv.scrollHeight > pv.clientHeight + 4;
  const atEnd = pv.scrollTop + pv.clientHeight >= pv.scrollHeight - 8;
  pv.classList.toggle('fade', overflow && !atEnd);
  const msgs = [];
  if (overflow && !$('done').classList.contains('reader')) msgs.push(t('scrollHint'));
  if (truncated) msgs.push(t('truncatedHint').replace('{n}', TEXT_DISPLAY_LIMIT.toLocaleString()));
  hint.textContent = msgs.join(' · ');
  hint.classList.toggle('hidden', !msgs.length);
}
$('preview').addEventListener('scroll', updateMoreHint, { passive: true });
window.addEventListener('resize', () => !$('done').classList.contains('hidden') && updateMoreHint());

// The same numbers as the live dashboard, summed over the whole transfer.
function renderDoneStats(size, secs) {
  const s = session;
  const h = s.header;
  const wire = s.received * (s.T || 0); // fountain symbols collected (payload after compression)
  const items = [
    [t('dTime'), secs >= 0.05 ? `${secs.toFixed(1)} s` : '–'],
    [t('dSpeed'), secs >= 0.5 ? `${fmtBytes(size / secs)}/s` : '–'],
    [t('dWire'), wire ? `${fmtBytes(wire / Math.max(secs, 0.001))}/s` : '–'],
    [t('dFrames'), `${s.frames} / ${s.scans}`],
    [t('okM'), s.covTot ? `${Math.round((100 * s.covOk) / s.covTot)}%` : '–'],
    [t('dEngine'), `${engine.toUpperCase()} ${s.msN ? (s.msSum / s.msN).toFixed(1) + ' ms' : ''}`],
  ];
  const box = $('doneStats');
  box.innerHTML = '';
  for (const [k, v] of items) {
    const d = document.createElement('div');
    d.textContent = k;
    const b = document.createElement('b');
    b.textContent = v;
    d.appendChild(b);
    box.appendChild(d);
  }
  if (h) {
    const c = document.createElement('div');
    c.className = 'code';
    const colours = [2, 4, 8][h.mode] || '?';
    c.textContent = `${t('dCode')} ${h.X}×${h.Y} · ${colours} ${t('dColours')} · ECC ${Math.round((100 * h.nsym) / h.n)}% · ${t('cellM')} ${s.cellN ? (s.cellSum / s.cellN).toFixed(1) + 'px' : '–'}`;
    box.appendChild(c);
  }
}

let cameraWasOn = false;
function showDone(name, bytes) {
  finished = true;
  truncated = false;
  running = false;
  // the camera is not needed while the result is on screen: turn it off (power, heat)
  cameraWasOn = !!stream;
  if (stream) {
    stopStream();
    video.srcObject = null;
  }
  if (wakeLock) wakeLock.release().catch(() => {});
  const ext = (name.split('.').pop() || '').toLowerCase();
  const blob = new Blob([bytes], { type: MIME[ext] || 'application/octet-stream' });
  if (lastUrl) URL.revokeObjectURL(lastUrl);
  lastUrl = URL.createObjectURL(blob);
  const text = asText(name, bytes);
  const message = text !== null && name === TEXT_NAME;
  const json = text !== null && isJson(text);
  const image = (MIME[ext] || '').startsWith('image/');
  const secs = firstPacketAt ? (performance.now() - firstPacketAt) / 1000 : 0;
  renderDoneStats(bytes.length, secs);
  const timing = secs >= 0.5 ? ` · ${secs.toFixed(1)}s · ${fmtBytes(bytes.length / secs)}/s` : '';
  shown = text !== null ? { text, json, plain: message && !json, lines: null } : null;
  if (json) shown.lines = formatJson(text, { html: text.length <= HIGHLIGHT_LIMIT });
  let what = fmtBytes(bytes.length);
  if (json) what = `JSON · ${shown.lines.length.toLocaleString()} ${t('lines')} · ${what}`;
  else if (message) what = `${[...text].length.toLocaleString()} ${t('chars')} · ${what}`;
  $('doneName').textContent = message ? t(json ? 'jsonReceived' : 'textReceived') : name;
  $('doneSub').textContent = what + timing;

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
    // copies what is on screen: formatted JSON in the Formatted view; Save keeps the original bytes
    const out = json && reader.view !== 'raw' ? formatJson(text).join('\n') : text;
    if (await copyText(out)) {
      copy.textContent = t('copied');
      setTimeout(() => (copy.textContent = t('copy')), 1500);
    }
  };
  const link = $('linkBtn');
  const url = text !== null ? text.trim() : '';
  const isLink = /^https?:\/\/\S+$/i.test(url);
  link.classList.toggle('hidden', !isLink);
  if (isLink) link.href = url;

  // text and images open full screen; other files get the small card
  const done = $('done');
  done.classList.toggle('reader', image || text !== null);
  $('readerBar').classList.toggle('hidden', !shown || shown.plain);
  $('headTools').classList.toggle('hidden', !shown);
  $('segView').classList.toggle('hidden', !json);
  applyStatsToggle();
  const pv = $('preview');
  truncated = false;
  pv.replaceChildren();
  pv.className = 'preview hidden';
  if (text !== null) {
    renderText();
  } else if (image) {
    const img = document.createElement('img');
    img.src = lastUrl;
    img.alt = name;
    pv.appendChild(img);
    pv.className = 'preview image';
  }
  done.classList.remove('hidden');
  requestAnimationFrame(updateMoreHint);
  $('rxPct').textContent = '100%';
  $('rxBar').style.width = '100%';
  octx.clearRect(0, 0, overlay.width, overlay.height);
}

$('againBtn').onclick = () => {
  $('done').classList.add('hidden');
  $('preview').replaceChildren();
  shown = null;
  sink.postMessage({ reset: true, engine });
  finished = false;
  progress = null;
  activeTid = null;
  progHist = [];
  scanHist = [];
  frameDone.clear();
  resetSession();
  firstPacketAt = 0;
  $('rxName').textContent = t('waiting');
  $('rxPct').textContent = '0%';
  $('rxBar').style.width = '0';
  $('mSpeed').textContent = $('mEta').textContent = '–';
  if (cameraWasOn) {
    cameraWasOn = false;
    openCamera(); // same camera as before; starts scanning again
  } else if (video.src) {
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
