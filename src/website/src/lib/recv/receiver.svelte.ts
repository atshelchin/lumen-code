// The receiver: camera frames go to decode workers, packets to the sink worker (the protocol's
// receiver), and the page shows the progress, the dashboard and, at the end, the file.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { t as tr, type Key, type Lang } from '#lib/i18n';
import { fmtBytes, fmtTime } from '#lib/format';
import { buildText } from '#lib/build';
import { isJson, formatJson, safeName } from '#lib/text-view';

// phones: 2 decode workers is plenty once scanning is paced; more just burns battery
const FRAME_KEY_MS = 1000; // a frame key names a frame for 1 s (PROTOCOL §11.5)
const MAX_SIZE = 256 * 1024 * 1024; // the reference receiver limit (§11.4), shown when a file is refused
// Region of interest and processing scale, from the newest result. Only this part of the
// camera frame is copied out of the GPU, already downscaled so that a cell is about
// TARGET_CELL pixels: the copy and read-back cost more than the scan itself at full size.
const TARGET_CELL = 6;
// Text messages arrive as UTF-8 "message.txt" (the sender's text tab); show them as text.
const TEXT_NAME = 'message.txt';
// Display limits only: Copy and Save always use the complete text.
export const TEXT_DISPLAY_LIMIT = 1_000_000;
export const HIGHLIGHT_LIMIT = 300_000; // larger JSON is formatted without colours to keep the DOM small
export const LINKIFY_LIMIT = 200_000;

const MIME: Record<string, string> = {
	png: 'image/png',
	jpg: 'image/jpeg',
	jpeg: 'image/jpeg',
	gif: 'image/gif',
	webp: 'image/webp',
	svg: 'image/svg+xml',
	avif: 'image/avif',
	pdf: 'application/pdf',
	txt: 'text/plain;charset=utf-8',
	md: 'text/plain;charset=utf-8',
	json: 'application/json',
	csv: 'text/csv',
	html: 'text/html',
	mp4: 'video/mp4',
	mov: 'video/quicktime',
	mp3: 'audio/mpeg',
	wav: 'audio/wav',
	zip: 'application/zip'
};
const TEXTY = /\.(txt|md|json|csv|log|js|ts|py|xml|yaml|yml|ini|conf|sh|c|h|cpp|go|rs|java|css)$/i;

type ScanWorker = Worker & { busy: boolean };
interface FrameEntry {
	t: number;
	first: number;
	ok: Set<number>;
	seq: number;
	total: number;
}
interface Crop {
	x: number;
	y: number;
	w: number;
	h: number;
	k: number;
	fw: number;
	fh: number;
}

export interface Done {
	name: string;
	title: string;
	sub: string;
	url: string;
	message: boolean; // a text message (not a file)
	text: string | null;
	json: boolean;
	image: boolean;
	lines: string[] | null; // formatted JSON
	stats: [string, string][];
	code: string;
}

export interface Reader {
	view?: 'pretty' | 'raw';
	wrap?: boolean;
	fs?: number;
	stats?: boolean;
}

export class ReceiveController {
	// ---------- what the page shows
	phase = $state<'start' | 'scanning' | 'done'>('start');
	startErr = $state('');
	engine = $state<'js' | 'wasm'>('wasm');
	found = $state(false);
	rxName = $state('');
	pct = $state(0);
	mSpeed = $state('–');
	mEta = $state('–');
	mCell = $state('–');
	mOk = $state('–');
	tip = $state('');
	tipGood = $state(false);
	diag = $state('');
	multiCam = $state(false);
	zoom = $state<{ min: number; max: number } | null>(null);
	zoomLevel = $state(0);
	done = $state<Done | null>(null);
	reader = $state<Reader>({});

	private t: (key: Key) => string;
	private video!: HTMLVideoElement;
	private overlay!: HTMLCanvasElement;
	private octx!: CanvasRenderingContext2D;
	private params = new URLSearchParams(location.search);
	private workers: ScanWorker[] = [];
	private sink!: Worker;
	private NW: number;
	private useBitmap =
		typeof OffscreenCanvas !== 'undefined' && typeof createImageBitmap !== 'undefined';
	private workerErrors = 0;
	private scanMs: number[] = [];
	private engineNoteUntil = 0;
	private engineNote = '';
	private running = false;
	private stream: MediaStream | null = null;
	private track: MediaStreamTrack | null = null;
	private deviceIds: string[] = [];
	private deviceIdx = -1;
	private frameId = 0;
	private fallbackCanvas: HTMLCanvasElement | null = null;
	private fallbackCtx: CanvasRenderingContext2D | null = null;
	private lastResult: any = null;
	private lastFoundAt = 0;
	private firstPacketAt = 0;
	private progress: any = null;
	private progHist: [number, number][] = [];
	private scanHist: {
		t: number;
		ok: boolean;
		cwOk: number;
		cwTotal: number;
		cell: number;
		skipped: boolean;
	}[] = [];
	private finished = false;
	private activeTid: number | null = null;
	// frames already decoded, so workers can skip repeats; kept a little longer for the statistics
	private frameDone = new Map<string, FrameEntry>();
	private camFrames: number[] = [];
	private scanTimes: number[] = [];
	private checksumWarnUntil = 0;
	private session: any = null;
	private stillImage: HTMLImageElement | null = null;
	private videoSize: MediaTrackConstraints = {};
	private camRate = 30;
	private camRateAt = 0;
	private zoomPending = false;
	private lastDispatch = 0;
	private senderFpsEst = 0;
	private nextScanAt = 0;
	private lastDecisionAt = 0; // capture time of the newest result used for scheduling
	private captureAt = new Map<number, number>();
	private crop: Crop | null = null;
	private cropAt = 0;
	private cameraWasOn = false;
	private lastUrl: string | null = null;
	private wakeLock: WakeLockSentinel | null = null;
	private hudTimer = 0;

	constructor(lang: Lang) {
		this.t = (key) => tr(lang, key);
		const mobile = matchMedia('(pointer: coarse)').matches;
		this.NW = Math.max(1, Math.min(mobile ? 2 : 3, (navigator.hardwareConcurrency || 4) - 1));
		this.rxName = this.t('noData');
		try {
			this.engine = localStorage.getItem('lumen.engine') === 'js' ? 'js' : 'wasm'; // WASM: faster, cooler
			this.reader = JSON.parse(localStorage.getItem('lumen.reader') || '{}') || {};
		} catch {
			/* storage unavailable */
		}
		const e = this.params.get('engine');
		if (e) this.engine = e === 'wasm' ? 'wasm' : 'js';
		this.resetSession();
	}

	attach(video: HTMLVideoElement, overlay: HTMLCanvasElement) {
		this.video = video;
		this.overlay = overlay;
		this.octx = overlay.getContext('2d')!;
		for (let i = 0; i < this.NW; i++) {
			const w = new Worker(new URL('./scan.worker.ts', import.meta.url), {
				type: 'module'
			}) as ScanWorker;
			w.busy = false;
			w.onmessage = (ev) => this.onResult(w, ev.data);
			w.onerror = (ev) => {
				w.busy = false;
				console.error('scan worker', ev.message);
			};
			this.workers.push(w);
		}
		this.sink = new Worker(new URL('./sink.worker.ts', import.meta.url), { type: 'module' });
		this.sink.onmessage = (ev) => this.onSink(ev.data);
		this.sink.postMessage({ reset: true, engine: this.engine });
		if (!window.isSecureContext) this.startErr = this.t('needHttps');
		this.hudTimer = window.setInterval(() => this.running && this.updateHud(), 500);
	}

	destroy() {
		this.running = false;
		clearInterval(this.hudTimer);
		this.stopStream();
		for (const w of this.workers) w.terminate();
		this.sink?.terminate();
		if (this.lastUrl) URL.revokeObjectURL(this.lastUrl);
		if (this.wakeLock) this.wakeLock.release().catch(() => {});
	}

	private resetSession() {
		this.session = {
			scans: 0,
			frames: 0,
			covOk: 0,
			covTot: 0,
			msSum: 0,
			msN: 0,
			prepSum: 0,
			pxSum: 0,
			cellSum: 0,
			cellN: 0,
			header: null,
			received: 0,
			T: 0
		};
		if (this.params.has('debug')) (self as any).lumenSession = this.session; // for measurements
	}

	// ---------- engine
	setEngine(engine: 'js' | 'wasm') {
		if (engine === this.engine) return;
		this.engine = engine;
		try {
			localStorage.setItem('lumen.engine', engine);
		} catch {
			/* ignore */
		}
		this.scanMs = [];
		this.session.msSum = this.session.msN = 0; // engine time in the summary = the engine that finished
		// both engines read the same packets: the reception continues where it is
		this.sink.postMessage({ engine });
		this.updateHud();
	}

	private wasmFailed(why: string) {
		if (this.engine !== 'wasm') return;
		this.engine = 'js';
		this.sink.postMessage({ engine: 'js' }); // keeps the packets received so far
		this.engineNote =
			this.t('wasmFailed') + (why ? ` (${String(why).split('\n')[0].slice(0, 80)})` : '');
		this.engineNoteUntil = performance.now() + 8000;
		this.updateHud();
	}

	// ---------- camera
	async openCamera() {
		this.startErr = '';
		if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
			this.startErr = this.t('needHttps');
			return;
		}
		const res = this.params.get('res') || '1080';
		const dims = res === '4k' ? [3840, 2160] : res === '720' ? [1280, 720] : [1920, 1080];
		// 30 fps to start; adaptCameraRate() asks for 60 when the sender plays fast enough
		this.videoSize = { width: { ideal: dims[0] }, height: { ideal: dims[1] } };
		this.camRate = 30;
		const video0: MediaTrackConstraints = { ...this.videoSize, frameRate: { ideal: 30, max: 30 } };
		if (this.deviceIdx >= 0 && this.deviceIds[this.deviceIdx])
			video0.deviceId = { exact: this.deviceIds[this.deviceIdx] };
		else video0.facingMode = { ideal: 'environment' };
		try {
			this.stopStream();
			this.stream = await navigator.mediaDevices.getUserMedia({ audio: false, video: video0 });
		} catch (err) {
			this.startErr = this.t('camErr') + (err instanceof Error ? err.message : String(err));
			return;
		}
		this.track = this.stream.getVideoTracks()[0];
		const video = this.video;
		video.srcObject = this.stream;
		video.removeAttribute('src');
		video.loop = false;
		await video.play().catch(() => {});
		this.setupTrackControls();
		try {
			const devs = await navigator.mediaDevices.enumerateDevices();
			this.deviceIds = devs.filter((d) => d.kind === 'videoinput').map((d) => d.deviceId);
			const cur = this.track.getSettings().deviceId || '';
			this.deviceIdx = Math.max(0, this.deviceIds.indexOf(cur));
			this.multiCam = this.deviceIds.length >= 2;
		} catch {
			/* ignore */
		}
		this.beginScanning();
	}

	switchCamera() {
		if (this.deviceIds.length < 2) return;
		this.deviceIdx = (this.deviceIdx + 1) % this.deviceIds.length;
		this.openCamera();
	}

	private stopStream() {
		if (this.stream) for (const tr of this.stream.getTracks()) tr.stop();
		this.stream = null;
		this.track = null;
	}

	// The camera's rate follows the sender: at 30 fps every frame of a sender up to ~15 fps is
	// seen twice; a faster sender needs 60 (when the camera has it) to be seen at all reliably.
	// Scans stay locked to the sender, so a faster camera adds captures, not scans.
	private adaptCameraRate(sender: number, now: number) {
		const track = this.track;
		if (!track || !track.applyConstraints || now - this.camRateAt < 2000) return;
		const caps: any = track.getCapabilities ? track.getCapabilities() : {};
		const max = (caps.frameRate && caps.frameRate.max) || 30;
		let want = this.camRate;
		if (this.camRate <= 30 && sender >= 16 && max >= 59) want = 60;
		else if (this.camRate > 30 && sender > 0 && sender < 13) want = 30;
		if (want === this.camRate) return;
		this.camRate = want;
		this.camRateAt = now;
		// the whole set: applyConstraints replaces the constraints, the size included
		track
			.applyConstraints({ ...this.videoSize, frameRate: { ideal: want, max: want } })
			.catch(() => {});
	}

	private setupTrackControls() {
		const track = this.track!;
		const caps: any = track.getCapabilities ? track.getCapabilities() : {};
		if (caps.focusMode && caps.focusMode.includes('continuous')) {
			track.applyConstraints({ advanced: [{ focusMode: 'continuous' } as any] }).catch(() => {});
		}
		if (caps.zoom && caps.zoom.max > caps.zoom.min) {
			this.zoom = { min: caps.zoom.min, max: Math.min(caps.zoom.max, caps.zoom.min * 8) };
			// a restarted camera (after "receive another") keeps the level the user chose
			this.setZoom(
				this.zoomLevel ? this.zoomLevel : (track.getSettings() as any).zoom || caps.zoom.min
			);
		} else this.zoom = null;
	}

	// ---------- zoom: pinch on the camera image; ctrl + wheel (trackpad pinch) on desktop
	setZoom(z: number) {
		if (!this.zoom || !this.track) return;
		this.zoomLevel = Math.min(this.zoom.max, Math.max(this.zoom.min, z));
		if (this.zoomPending) return; // one constraint update per animation frame
		this.zoomPending = true;
		requestAnimationFrame(() => {
			this.zoomPending = false;
			if (this.track)
				this.track
					.applyConstraints({ advanced: [{ zoom: this.zoomLevel } as any] })
					.catch(() => {});
		});
	}

	// ---------- media file (image or recorded video) as a source, for testing
	async openMedia(f: File) {
		this.stopStream();
		const url = URL.createObjectURL(f);
		const video = this.video;
		if (f.type.startsWith('video/')) {
			video.srcObject = null;
			video.src = url;
			video.loop = true;
			await video.play().catch(() => {});
			this.beginScanning();
			return;
		}
		const img = new Image();
		img.onload = () => {
			video.srcObject = null;
			video.poster = url;
			this.beginScanning(img);
		};
		img.src = url;
	}

	private beginScanning(still?: HTMLImageElement) {
		this.phase = 'scanning';
		this.stillImage = still || null;
		this.crop = null;
		this.running = true;
		this.wake();
		this.updateHud();
		if (this.stillImage) {
			// decode the still a few times (each worker once)
			for (const w of this.workers) this.grab(w);
			return;
		}
		this.pump();
	}

	private sourceSize(): [number, number] {
		if (this.stillImage) return [this.stillImage.naturalWidth, this.stillImage.naturalHeight];
		return [this.video.videoWidth, this.video.videoHeight];
	}

	// Frame-locked scanning. Each sender frame needs one good capture; more scans of the same
	// frame only cost battery. After a frame is fully decoded, wait ~3/4 of a sender frame
	// period; after a repeat, ~1/3; after a partial decode, rescan at once.
	private senderPeriod() {
		return 1000 / (this.senderFpsEst > 0 ? this.senderFpsEst : 12);
	}
	private scanDue(now: number) {
		if (this.finished) return now - this.lastDispatch >= 1000 / 3;
		// searching: 5 scans/s, and 2/s once nothing has been in view for 10 s (phone put down)
		if (now - this.lastFoundAt > 10000) return now - this.lastDispatch >= 1000 / 2;
		if (now - this.lastFoundAt > 2000) return now - this.lastDispatch >= 1000 / 5;
		if (now - this.lastDispatch < Math.min(1000 / 30, this.senderPeriod() / 3)) return false;
		return now >= this.nextScanAt;
	}
	private schedule(d: any, capturedAt: number) {
		if (capturedAt < this.lastDecisionAt) return; // an older capture finished late
		this.lastDecisionAt = capturedAt;
		const P = this.senderPeriod();
		if (!d.ok) this.nextScanAt = 0;
		else if (d.skipped) this.nextScanAt = capturedAt + 0.33 * P;
		else {
			const e = d.frameKey && this.frameDone.get(d.frameKey);
			this.nextScanAt = e && e.total && e.ok.size >= e.total ? capturedAt + 0.75 * P : 0;
		}
	}

	private pump = () => {
		if (!this.running || this.stillImage) return;
		const now = performance.now();
		const video = this.video;
		this.camFrames.push(now);
		while (this.camFrames.length && now - this.camFrames[0] > 2000) this.camFrames.shift();
		if (video.readyState >= 2 && video.videoWidth && this.scanDue(now)) {
			const free = this.workers.find((w) => !w.busy);
			if (free) {
				this.lastDispatch = now;
				this.grab(free);
			}
		}
		if ('requestVideoFrameCallback' in video) video.requestVideoFrameCallback(this.pump);
		else requestAnimationFrame(this.pump);
	};

	private skipMap() {
		const now = performance.now();
		const out: Record<string, number[]> = {};
		for (const [key, e] of this.frameDone) {
			if (now - e.first > 4000) this.frameDone.delete(key);
			else if (now - e.first <= FRAME_KEY_MS) out[key] = [...e.ok];
		}
		return out;
	}

	private updateCrop(d: any, capturedAt: number) {
		if (capturedAt < this.cropAt || this.stillImage) return;
		this.cropAt = capturedAt;
		if (!d.ok || !d.stats || !d.w) {
			this.crop = null; // lost: whole frame, full resolution
			return;
		}
		const xs = d.stats.corners.map((c: number[]) => c[0]);
		const ys = d.stats.corners.map((c: number[]) => c[1]);
		const mx = (Math.max(...xs) - Math.min(...xs)) * 0.18 + 24;
		const my = (Math.max(...ys) - Math.min(...ys)) * 0.18 + 24;
		const x0 = Math.max(0, Math.floor(Math.min(...xs) - mx));
		const y0 = Math.max(0, Math.floor(Math.min(...ys) - my));
		const x1 = Math.min(d.w, Math.ceil(Math.max(...xs) + mx));
		const y1 = Math.min(d.h, Math.ceil(Math.max(...ys) + my));
		const k = Math.min(1, Math.max(0.35, TARGET_CELL / d.stats.cellPx));
		if (x1 - x0 < 64 || y1 - y0 < 64) this.crop = null;
		else this.crop = { x: x0, y: y0, w: x1 - x0, h: y1 - y0, k, fw: d.w, fh: d.h };
	}

	private async grab(w: ScanWorker) {
		w.busy = true;
		const id = ++this.frameId;
		this.captureAt.set(id, performance.now());
		const skip = this.skipMap();
		const src = this.stillImage || this.video;
		const [sw, sh] = this.sourceSize();
		// a crop measured on another frame size (the phone was turned) no longer applies
		const c = this.crop;
		const r = c && c.fw === sw && c.fh === sh ? c : { x: 0, y: 0, w: sw, h: sh, k: 1 };
		const dw = Math.max(32, Math.round(r.w * r.k));
		const dh = Math.max(32, Math.round(r.h * r.k));
		const region = { x: r.x, y: r.y, kx: dw / r.w, ky: dh / r.h, fullW: sw, fullH: sh };
		const engine = this.engine;
		try {
			if (this.useBitmap) {
				// crop + resize happen on the GPU; the worker receives only the small bitmap
				const opts: ImageBitmapOptions | undefined =
					dw < r.w ? { resizeWidth: dw, resizeHeight: dh, resizeQuality: 'medium' } : undefined;
				const bmp = await createImageBitmap(src, r.x, r.y, r.w, r.h, opts);
				w.postMessage({ id, bitmap: bmp, region, dw, dh, skip, engine }, [bmp]);
			} else {
				if (!this.fallbackCanvas) {
					this.fallbackCanvas = document.createElement('canvas');
					this.fallbackCtx = this.fallbackCanvas.getContext('2d', { willReadFrequently: true });
				}
				const fc = this.fallbackCanvas;
				if (fc.width < dw || fc.height < dh) {
					fc.width = Math.max(fc.width, dw);
					fc.height = Math.max(fc.height, dh);
				}
				this.fallbackCtx!.drawImage(src, r.x, r.y, r.w, r.h, 0, 0, dw, dh);
				const im = this.fallbackCtx!.getImageData(0, 0, dw, dh);
				w.postMessage(
					{ id, image: { width: dw, height: dh, data: im.data }, region, skip, engine },
					[im.data.buffer]
				);
			}
		} catch (err) {
			w.busy = false;
			console.warn('grab failed', err);
		}
	}

	// ---------- results
	private onResult(w: ScanWorker, d: any) {
		w.busy = false;
		const capturedAt = this.captureAt.get(d.id) ?? performance.now();
		this.captureAt.delete(d.id);
		if (d.reason === 'error') {
			console.warn(d.error);
			if (d.wasmError) this.wasmFailed(d.error);
			else if (this.useBitmap && ++this.workerErrors >= 2) this.useBitmap = false; // no OffscreenCanvas 2D in workers
			this.nextScanAt = 0;
			return;
		}
		const now = performance.now();
		const s = this.session;
		this.lastResult = d;
		if (d.prepMs !== undefined) {
			s.prepSum += d.prepMs;
			s.pxSum += d.px || 0;
		}
		if (d.engine === this.engine && d.ms) {
			this.scanMs.push(d.ms);
			if (this.scanMs.length > 30) this.scanMs.shift();
			s.msSum += d.ms;
			s.msN++;
		}
		s.scans++;
		this.scanTimes.push(now);
		while (this.scanTimes.length && now - this.scanTimes[0] > 2000) this.scanTimes.shift();
		if (d.frameKey) {
			let e = this.frameDone.get(d.frameKey);
			if (e && now - e.first > FRAME_KEY_MS) e = undefined; // the seq came round: a new frame
			if (!e) {
				e = {
					t: now,
					first: now,
					ok: new Set(),
					seq: d.header.seq,
					total: d.stats ? d.stats.cwCount : 0
				};
				this.frameDone.set(d.frameKey, e);
				s.frames++;
				s.covTot += e.total;
			}
			e.t = now;
			for (const i of d.okIdx || []) {
				if (!e.ok.has(i)) s.covOk++;
				e.ok.add(i);
			}
			s.header = d.header;
		}
		if (d.ok && d.stats && !d.skipped) {
			s.cellSum += d.stats.cellPx;
			s.cellN++;
		}
		this.schedule(d, capturedAt);
		this.updateCrop(d, capturedAt);
		const rec = { t: now, ok: d.ok, cwOk: 0, cwTotal: 0, cell: 0, skipped: !!d.skipped };
		if (d.ok) {
			this.lastFoundAt = now;
			rec.cwOk = d.stats.cwOk;
			rec.cwTotal = d.stats.cwTotal;
			rec.cell = d.stats.cellPx;
		}
		this.scanHist.push(rec);
		while (this.scanHist.length && now - this.scanHist[0].t > 2500) this.scanHist.shift();
		this.drawOverlay(d);
		if (d.packets && d.packets.length && !this.finished) {
			if (!this.firstPacketAt) this.firstPacketAt = now;
			this.sink.postMessage(
				{ packets: d.packets },
				d.packets.map((p: Uint8Array) => p.buffer)
			);
		}
		this.updateHud();
	}

	private onSink(m: any) {
		if (m.type === 'progress' && m.prog) {
			const now = performance.now();
			if (m.prog.tid !== this.activeTid) {
				this.activeTid = m.prog.tid;
				this.progHist = [];
				this.firstPacketAt = now;
			}
			this.progress = m.prog;
			this.session.received = m.prog.received;
			this.session.T = m.prog.T || this.session.T;
			this.progHist.push([now, m.prog.received]);
			while (this.progHist.length > 2 && now - this.progHist[0][0] > 3000) this.progHist.shift();
			this.updateHud();
		} else if (m.type === 'done') {
			this.showDone(m.name, m.bytes);
		} else if (m.type === 'error') {
			if (m.wasmError) return this.wasmFailed(m.error);
			this.checksumWarnUntil = performance.now() + 3000;
			console.warn('sink', m.error);
		}
	}

	// ---------- overlay: detected code outline
	private drawOverlay(d: any) {
		const overlay = this.overlay;
		const octx = this.octx;
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
		const pts = d.stats.corners.map(([x, y]: number[]) => [ox + x * scale, oy + y * scale]);
		const e = d.frameKey && this.frameDone.get(d.frameKey);
		const rate =
			e && e.total
				? e.ok.size / e.total
				: d.skipped
					? 1
					: d.stats.cwCount
						? (d.okIdx || []).length / d.stats.cwCount
						: 0;
		const color = this.moving() || rate > 0.8 ? '#2bdc74' : rate > 0.3 ? '#ffd60a' : '#ff5468';
		octx.lineWidth = 3;
		octx.strokeStyle = color;
		octx.lineJoin = 'round';
		octx.shadowColor = color;
		octx.shadowBlur = 12;
		octx.beginPath();
		pts.forEach(([x, y]: number[], i: number) => (i ? octx.lineTo(x, y) : octx.moveTo(x, y)));
		octx.closePath();
		octx.stroke();
	}

	// ---------- HUD
	// Data is arriving: then the transfer is going well, whatever share of each frame is read.
	private moving() {
		const h = this.progHist;
		return (
			h.length >= 2 && h[h.length - 1][1] > h[0][1] && performance.now() - h[h.length - 1][0] < 2500
		);
	}

	updateHud() {
		const t = this.t;
		const now = performance.now();
		const found = now - this.lastFoundAt < 700;
		const moving = this.moving();
		this.found = found;

		let cell = 0;
		let cellN = 0;
		for (const r of this.scanHist) {
			if (r.ok) {
				cell += r.cell;
				cellN++;
			}
		}
		// success = share of each recently seen frame's codewords that were recovered
		let covOk = 0;
		let covTot = 0;
		for (const e of this.frameDone.values())
			if (now - e.first < 2500 && e.total) {
				covOk += e.ok.size;
				covTot += e.total;
			}
		const okRate = covTot ? covOk / covTot : 0;
		cell = cellN ? cell / cellN : 0;
		this.mCell = cellN ? `${cell.toFixed(1)}px` : '–';
		this.mOk = covTot ? `${Math.round(okRate * 100)}%` : '–';

		const progress = this.progress;
		if (progress) {
			this.rxName = progress.name ? safeName(progress.name) : t('receiving');
			this.pct = progress.needed
				? Math.min(progress.done ? 100 : 99, (100 * progress.received) / progress.needed)
				: 0;
			let rate = 0;
			const h = this.progHist;
			if (h.length >= 2) {
				const [t0, r0] = h[0];
				const [t1, r1] = h[h.length - 1];
				if (t1 > t0) rate = ((r1 - r0) * 1000) / (t1 - t0);
			}
			this.mSpeed = rate > 0 ? `${fmtBytes(rate * (progress.T || 0))}/s` : '–';
			this.mEta =
				progress.needed && rate > 0 ? fmtTime((progress.needed - progress.received) / rate) : '–';
		}

		// capture / decode diagnostics: which link limits the rate?
		const cf = this.camFrames;
		const st = this.scanTimes;
		const camFps = cf.length > 1 ? ((cf.length - 1) * 1000) / (cf[cf.length - 1] - cf[0]) : 0;
		const scanFps = st.length > 1 ? ((st.length - 1) * 1000) / (st[st.length - 1] - st[0]) : 0;
		const recent = [...this.frameDone.values()]
			.filter((e) => now - e.first < 3000)
			.sort((a, b) => a.first - b.first);
		let span = 0;
		for (let i = 1; i < recent.length; i++) {
			const dlt = (recent[i].seq - recent[i - 1].seq + 256) & 255;
			if (dlt > 0 && dlt < 64) span += dlt;
		}
		const dt = recent.length > 1 ? (recent[recent.length - 1].first - recent[0].first) / 1000 : 0;
		const senderFps = dt > 0.5 ? span / dt : 0;
		if (senderFps > 0) this.senderFpsEst = senderFps;
		if (senderFps > 0 && !this.stillImage) this.adaptCameraRate(senderFps, now);
		const distinctFps = dt > 0.5 ? (recent.length - 1) / dt : 0;
		// share of the sender's frames that were captured; with a fountain code a missed frame
		// costs only its own data, nothing has to be resent
		const captured = span > 0 ? Math.min(1, (recent.length - 1) / span) : 0;
		const avgMs = this.scanMs.length
			? this.scanMs.reduce((a, b) => a + b, 0) / this.scanMs.length
			: 0;
		// fraction of time the decode workers are busy
		const load = (avgMs * scanFps) / 1000 / this.NW;
		this.diag =
			`${this.engine.toUpperCase()} ${avgMs ? avgMs.toFixed(1) + ' ms' : '–'} · ${t('camFps')} ${camFps.toFixed(0)} · ${t('scanFps')} ${scanFps.toFixed(0)}/s · ${t('frameFps')} ${distinctFps.toFixed(1)}/s` +
			(senderFps
				? ` · ${t('senderFps')} ${senderFps.toFixed(0)}/s · ${t('captured')} ${Math.round(captured * 100)}%`
				: '');

		let msg: string;
		let good = false;
		if (now < this.engineNoteUntil) msg = this.engineNote;
		else if (now < this.checksumWarnUntil) msg = t('checksum');
		else if (progress && progress.state === 'refused')
			msg = t('tooLarge')
				.replace('{size}', fmtBytes(Math.max(progress.size, progress.payloadLength)))
				.replace('{max}', fmtBytes(MAX_SIZE));
		else if (!found) msg = t('tipSearch');
		else if (!moving && cellN && cell < 3.4) {
			const h = this.lastResult && this.lastResult.header;
			const portrait = this.video.videoHeight > this.video.videoWidth;
			msg = h && h.X > h.Y * 1.2 && portrait ? t('tipLandscape') : t('tipCloser');
		} else if (!moving && covTot && okRate < 0.5) msg = cell < 4.5 ? t('tipSteady') : t('tipColor');
		else if (this.engine === 'js' && senderFps && captured < 0.8 && load > 0.8) msg = t('tipWasm');
		else if (
			senderFps &&
			captured > 0.95 &&
			okRate > 0.9 &&
			dt > 2 &&
			load < 0.5 &&
			camFps > 2.2 * senderFps
		) {
			msg = t('tipFpsUp').replace('{n}', String(Math.round(Math.min(camFps / 2, senderFps * 1.5))));
			good = true;
		} else {
			msg = t('tipGood');
			good = true;
		}
		this.tip = msg;
		this.tipGood = good;
	}

	// ---------- completion
	private asText(name: string, bytes: Uint8Array) {
		if ((name !== TEXT_NAME && !TEXTY.test(name)) || bytes.length > 4_000_000) return null;
		try {
			return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
		} catch {
			return null;
		}
	}

	// The same numbers as the live dashboard, summed over the whole transfer.
	private doneStats(size: number, secs: number): { stats: [string, string][]; code: string } {
		const t = this.t;
		const s = this.session;
		const h = s.header;
		const wire = s.received * (s.T || 0); // fountain symbols collected (payload after compression)
		const stats: [string, string][] = [
			[t('dTime'), secs >= 0.05 ? `${secs.toFixed(1)} s` : '–'],
			[t('dSpeed'), secs >= 0.5 ? `${fmtBytes(size / secs)}/s` : '–'],
			[t('dWire'), wire ? `${fmtBytes(wire / Math.max(secs, 0.001))}/s` : '–'],
			[t('dFrames'), `${s.frames} / ${s.scans}`],
			[t('okM'), s.covTot ? `${Math.round((100 * s.covOk) / s.covTot)}%` : '–'],
			[
				t('dEngine'),
				`${this.engine.toUpperCase()} ${s.msN ? (s.msSum / s.msN).toFixed(1) + ' ms' : ''}`
			],
			[t('buildLabel'), buildText()]
		];
		const colours = h ? [2, 4, 8][h.mode] || '?' : '';
		const code = h
			? `${t('dCode')} ${h.X}×${h.Y} · ${colours} ${t('dColours')} · ECC ${Math.round((100 * h.nsym) / h.n)}% · ${t('cellM')} ${s.cellN ? (s.cellSum / s.cellN).toFixed(1) + 'px' : '–'}`
			: '';
		return { stats, code };
	}

	private showDone(rawName: string, bytes: Uint8Array) {
		const t = this.t;
		const name = safeName(rawName); // a descriptor can also have no name (§8.4)
		this.finished = true;
		this.running = false;
		// the camera is not needed while the result is on screen: turn it off (power, heat)
		this.cameraWasOn = !!this.stream;
		if (this.stream) {
			this.stopStream();
			this.video.srcObject = null;
		}
		if (this.wakeLock) this.wakeLock.release().catch(() => {});
		const ext = (name.split('.').pop() || '').toLowerCase();
		const blob = new Blob([bytes as BlobPart], { type: MIME[ext] || 'application/octet-stream' });
		if (this.lastUrl) URL.revokeObjectURL(this.lastUrl);
		const url = (this.lastUrl = URL.createObjectURL(blob));
		const text = this.asText(name, bytes);
		const message = text !== null && name === TEXT_NAME;
		const json = text !== null && isJson(text);
		const image = (MIME[ext] || '').startsWith('image/');
		const secs = this.firstPacketAt ? (performance.now() - this.firstPacketAt) / 1000 : 0;
		const { stats, code } = this.doneStats(bytes.length, secs);
		const timing = secs >= 0.5 ? ` · ${secs.toFixed(1)}s · ${fmtBytes(bytes.length / secs)}/s` : '';
		const lines = json ? formatJson(text!, { html: text!.length <= HIGHLIGHT_LIMIT }) : null;
		let what = fmtBytes(bytes.length);
		if (json) what = `JSON · ${lines!.length.toLocaleString()} ${t('lines')} · ${what}`;
		else if (message) what = `${[...text!].length.toLocaleString()} ${t('chars')} · ${what}`;
		this.done = {
			name,
			title: message ? t(json ? 'jsonReceived' : 'textReceived') : name,
			sub: what + timing,
			url,
			message,
			text,
			json,
			image,
			lines,
			stats,
			code
		};
		this.pct = 100;
		this.phase = 'done';
		this.octx.clearRect(0, 0, this.overlay.width, this.overlay.height);
	}

	saveReader() {
		try {
			localStorage.setItem('lumen.reader', JSON.stringify(this.reader));
		} catch {
			/* ignore */
		}
	}

	again() {
		this.done = null;
		this.sink.postMessage({ reset: true, engine: this.engine });
		this.finished = false;
		this.progress = null;
		this.activeTid = null;
		this.progHist = [];
		this.scanHist = [];
		this.frameDone.clear();
		this.resetSession();
		this.firstPacketAt = 0;
		this.rxName = this.t('noData');
		this.pct = 0;
		this.mSpeed = this.mEta = '–';
		if (this.cameraWasOn) {
			this.cameraWasOn = false;
			this.openCamera(); // same camera as before; starts scanning again
		} else if (this.video.src) {
			this.phase = 'scanning';
			this.running = true;
			this.pump();
		} else this.phase = 'start';
	}

	async wake() {
		try {
			if ('wakeLock' in navigator && !this.wakeLock) {
				this.wakeLock = await navigator.wakeLock.request('screen');
				this.wakeLock.addEventListener('release', () => (this.wakeLock = null));
			}
		} catch {
			/* ignore */
		}
	}

	visible() {
		if (document.visibilityState === 'visible' && this.running) this.wake();
	}
}
