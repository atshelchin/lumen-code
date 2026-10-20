//! The receiver's real-time loop: which camera frames to scan, which part of them, and what
//! the scans add up to. A port of the web demo's `recv-app.js` (frame-locked pacing, region
//! of interest, per-frame codeword coverage, rolling statistics), so phones and desktops
//! behave like the browser, minus the browser's copies.
//!
//! Each sender frame needs one good capture; more scans of the same frame only cost battery.
//! After a frame is fully decoded the next scan waits ~3/4 of a sender frame period; after a
//! repeat ~1/3; after a partial decode it runs at once. While searching it runs 5 times a
//! second, and twice a second once nothing has been in view for 10 s (phone put down).

use super::image::{prepare, prepare_yuv, Crop, FrameRef};
use lumen::scan::{FrameKey, Hint, Image, ScanResult, Scanner, SkipMap, YuvImage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Pixels per cell the region of interest is scaled to.
const TARGET_CELL: f64 = 6.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderInfo {
    pub x: u32,
    pub y: u32,
    pub mode: u32,
    pub n: u32,
    pub nsym: u32,
}

/// Totals over the whole transfer (the result panel).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub scans: u32,
    pub frames: u32,
    pub cov_ok: u32,
    pub cov_tot: u32,
    pub ms_avg: f32,
    pub cell_avg: f32,
    pub header: Option<HeaderInfo>,
}

/// Rolling numbers for the live dashboard.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStats {
    /// Time of the last scan that found a code (ms, runtime clock); negative = never.
    pub last_found_at: f64,
    /// Mean cell size in camera pixels over the last 2.5 s, and how many scans it covers.
    pub cell: f32,
    pub cell_n: u32,
    /// Codewords recovered / present in the frames first seen in the last 2.5 s.
    pub cov_ok: u32,
    pub cov_tot: u32,
    pub cam_fps: f32,
    pub scan_fps: f32,
    pub distinct_fps: f32,
    pub sender_fps: f32,
    /// Share of the sender's frames that were captured.
    pub captured: f32,
    /// Seconds of history behind sender_fps.
    pub span_secs: f32,
    pub avg_ms: f32,
    /// Fraction of time the scan thread is busy.
    pub load: f32,
    /// Captures are torn (slow sensor readout, or a camera barely faster than the sender).
    pub tearing: bool,
    /// The heat budget turned frames away in the last second.
    pub heat_limited: bool,
    pub session: SessionStats,
}

/// What one scan saw, for the core.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub generation: u64,
    pub at: f64,
    pub ok: bool,
    pub skipped: bool,
    /// Code outline TL, TR, BR, BL in camera-frame pixels.
    pub corners: Option<[[f32; 2]; 4]>,
    pub frame_w: u32,
    pub frame_h: u32,
    pub rotation: u32,
    /// Codeword coverage of this frame (outline colour).
    pub frame_rate: f32,
    pub header: Option<HeaderInfo>,
    pub stats: LiveStats,
    /// Packets this scan recovered.
    pub packets: u32,
}

/// A region of a camera frame ready to scan: Y'CbCr planes for camera formats (no colour
/// conversion), RGBA otherwise. Buffers are reused from frame to frame.
#[derive(Default)]
pub struct Prepared {
    rgba: Vec<u8>,
    y: Vec<u8>,
    cbcr: Vec<u8>,
    yuv: bool,
}

impl Prepared {
    pub fn prepare(&mut self, frame: &FrameRef, crop: &Crop) -> (usize, usize) {
        if let Some(size) = prepare_yuv(frame, crop, &mut self.y, &mut self.cbcr) {
            self.yuv = true;
            return size;
        }
        self.yuv = false;
        prepare(frame, crop, &mut self.rgba)
    }

    pub fn scan(&self, scanner: &mut Scanner, dw: usize, dh: usize, plan: &Plan) -> ScanResult {
        // the hint is in camera pixels; the scanner wants this image's pixels
        let (kx, ky) = (dw as f64 / plan.crop.w as f64, dh as f64 / plan.crop.h as f64);
        let hint = plan.hint.map(|(c, cell)| Hint {
            corners: c.map(|[x, y]| [(x - plan.crop.x as f64) * kx, (y - plan.crop.y as f64) * ky]),
            module: cell * (kx + ky) / 2.0,
        });
        if self.yuv {
            scanner.scan_yuv(&YuvImage { width: dw, height: dh, y: &self.y, cbcr: &self.cbcr }, Some(&plan.skip), hint)
        } else {
            scanner.scan_hinted(&Image { width: dw, height: dh, rgba: &self.rgba[..dw * dh * 4] }, Some(&plan.skip), hint)
        }
    }
}

/// What to scan of one camera frame, decided under the engine's lock; the scan itself runs
/// on any thread.
#[derive(Clone)]
pub struct Plan {
    pub crop: Crop,
    pub captured_at: f64,
    pub generation: u64,
    pub skip: SkipMap,
    pub fw: usize,
    pub fh: usize,
    pub rotation: u32,
    /// Whether the result steers pacing and the region of interest (not for stills).
    pub track: bool,
    /// Time spent preparing the image, added to the reported engine time.
    pub prep_ms: f64,
    /// Where the code was (corners and cell size in camera pixels), when recently seen.
    pub hint: Option<([[f64; 2]; 4], f64)>,
}

/// seq repeats every 256 frames, so a frame key names a frame for this long after it was
/// first seen (PROTOCOL §11.5); entries stay a little longer for the statistics.
const FRAME_KEY_MS: f64 = 1000.0;

struct FrameEntry {
    t: f64,
    first: f64,
    ok: HashSet<usize>,
    seq: u32,
    total: usize,
    scans: u32,
}

struct Rec {
    t: f64,
    ok: bool,
    cell: f64,
}

#[derive(Clone, Copy)]
struct RoiState {
    crop: Crop,
    fw: usize,
    fh: usize,
}

#[derive(Default)]
struct Session {
    scans: u32,
    frames: u32,
    cov_ok: u32,
    cov_tot: u32,
    ms_sum: f64,
    ms_n: u32,
    cell_sum: f64,
    cell_n: u32,
    header: Option<HeaderInfo>,
}

pub struct RecvEngine {
    scanner: Scanner,
    pub active: bool,
    pub generation: u64,
    pub finished: bool,
    last_dispatch: f64,
    next_scan_at: f64,
    last_found_at: f64,
    sender_fps_est: f64,
    roi: Option<RoiState>,
    frame_done: HashMap<FrameKey, FrameEntry>,
    cam_frames: VecDeque<f64>,
    scan_times: VecDeque<f64>,
    scan_hist: VecDeque<Rec>,
    scan_ms: VecDeque<f64>,
    session: Session,
    img: Prepared,
    searches: u32,
    last_decision_at: f64,
    activated_at: f64,
    /// The last code seen: corners, cell size (camera pixels), when, frame size.
    tracked: Option<([[f64; 2]; 4], f64, f64, usize, usize)>,
    /// Scanners working in parallel (load = busy share of all of them).
    pub workers: usize,
    /// Rolling shutter: how long the sensor takes from its first row to its last, in ms, when
    /// the camera says (0 = unknown).
    pub readout_ms: f64,
    /// Share of codewords recovered by recent scans that found the code (moving average).
    yield_avg: f64,
    /// The device's heat, 0 (cool) to 3 (critical): caps the scan rate.
    pub heat: u8,
    /// Scans the heat budget still allows (a token bucket), and when it was last filled.
    tokens: f64,
    tokens_at: f64,
    /// When the heat budget last turned a frame away.
    heat_refused_at: f64,
}

impl Default for RecvEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RecvEngine {
    pub fn new() -> Self {
        RecvEngine {
            scanner: Scanner::new(),
            active: false,
            generation: 0,
            finished: false,
            last_dispatch: f64::NEG_INFINITY,
            next_scan_at: 0.0,
            last_found_at: f64::NEG_INFINITY,
            sender_fps_est: 0.0,
            roi: None,
            frame_done: HashMap::new(),
            cam_frames: VecDeque::new(),
            scan_times: VecDeque::new(),
            scan_hist: VecDeque::new(),
            scan_ms: VecDeque::new(),
            session: Session::default(),
            img: Prepared::default(),
            searches: 0,
            last_decision_at: f64::NEG_INFINITY,
            activated_at: f64::NEG_INFINITY,
            tracked: None,
            workers: 1,
            readout_ms: 0.0,
            yield_avg: 1.0,
            heat: 0,
            tokens: 0.0,
            tokens_at: 0.0,
            heat_refused_at: f64::NEG_INFINITY,
        }
    }

    /// A new reception: forget every frame, statistic and header.
    pub fn reset(&mut self, generation: u64) {
        let (active, workers, activated_at, readout, heat) = (self.active, self.workers, self.activated_at, self.readout_ms, self.heat);
        *self = RecvEngine { img: std::mem::take(&mut self.img), ..RecvEngine::new() };
        self.active = active;
        self.workers = workers;
        self.readout_ms = readout;
        self.heat = heat;
        self.activated_at = activated_at;
        self.generation = generation;
    }

    /// Starts (or stops) accepting frames. A fresh start searches at the quick rate for a
    /// while, as if a code had just been in view.
    pub fn set_active(&mut self, on: bool, now: f64) {
        if on && !self.active {
            self.activated_at = now;
        }
        self.active = on;
    }

    fn sender_period(&self) -> f64 {
        1000.0 / if self.sender_fps_est > 0.0 { self.sender_fps_est } else { 12.0 }
    }

    /// Whether captures are torn: the sensor reads its rows over a good part of a sender frame
    /// (a capture then holds the end of one frame and the start of the next), or the camera
    /// is too slow to see each sender frame in more than one capture anyway. Every capture
    /// then carries data no other one has: scan them all, skip nothing.
    fn tearing(&self) -> bool {
        let p = self.sender_period();
        let cam = match (self.cam_frames.front(), self.cam_frames.back()) {
            (Some(a), Some(b)) if self.cam_frames.len() > 5 && b > a => (self.cam_frames.len() - 1) as f64 * 1000.0 / (b - a),
            _ => 0.0,
        };
        self.readout_ms > 0.4 * p || (self.sender_fps_est > 0.0 && cam > 0.0 && cam < 1.6 * self.sender_fps_est)
    }

    fn scan_due(&self, now: f64) -> bool {
        let since = now - self.last_dispatch;
        if self.finished {
            return since >= 1000.0 / 3.0;
        }
        // the first two seconds after the camera opens: look hard, so a code that is already
        // in view locks at once (half-resolution searches are cheap)
        if !self.last_found_at.is_finite() && now - self.activated_at < 2000.0 {
            return since >= 1000.0 / 15.0;
        }
        // idle times count from the last code seen, or from the camera opening
        let seen = self.last_found_at.max(self.activated_at);
        if now - seen > 10_000.0 {
            return since >= 1000.0 / 2.0;
        }
        if now - self.last_found_at > 2_000.0 {
            return since >= 1000.0 / 5.0;
        }
        // the code is in view but nothing decodes (blur, glare, a bad exposure): such scans
        // are the slowest (every reading is tried) and give nothing; look a few times a second
        if self.yield_avg < 0.1 && since < 1000.0 / 8.0 {
            return false;
        }
        if self.tearing() {
            return true;
        }
        // locked to the sender: never more often than three times a sender frame
        if since < self.sender_period() / 3.0 {
            return false;
        }
        now >= self.next_scan_at
    }

    /// A warm phone throttles itself (and gets slower and hotter still): scan less, so it can
    /// cool down first. The budget is scanning time, not scans: a phone that scans in 12 ms
    /// heats little at 30 a second, one that needs 60 ms does at 25. A token bucket, so 24 a
    /// second from a 30 fps camera skips one frame in five rather than every other one.
    fn heat_allows(&mut self, now: f64) -> bool {
        // seconds of scanning per second, all scanners together
        let busy = match self.heat {
            0 => return true,
            1 => 1.5,
            2 => 0.8,
            _ => 0.4,
        };
        let ms = if self.scan_ms.len() >= 5 { self.scan_ms.iter().sum::<f64>() / self.scan_ms.len() as f64 } else { 40.0 };
        let budget = (busy * 1000.0 / ms.max(1.0)).max(5.0);
        self.tokens = (self.tokens + (now - self.tokens_at).max(0.0) * budget / 1000.0).min(2.0);
        self.tokens_at = now;
        if self.tokens < 1.0 {
            self.heat_refused_at = now;
            return false;
        }
        self.tokens -= 1.0;
        true
    }

    /// Counts a camera frame and decides whether to scan it: when the pacing says so and a
    /// scanner is free (`free`). Cheap; called under the engine's lock for every frame.
    pub fn plan(&mut self, fw: usize, fh: usize, rotation: u32, now: f64, free: bool) -> Option<Plan> {
        self.cam_frames.push_back(now);
        while self.cam_frames.front().is_some_and(|&t| now - t > 2000.0) {
            self.cam_frames.pop_front();
        }
        if !self.active || !free || !self.scan_due(now) || !self.heat_allows(now) {
            return None;
        }
        self.last_dispatch = now;
        Some(self.make_plan(fw, fh, rotation, now, true))
    }

    fn make_plan(&mut self, fw: usize, fh: usize, rotation: u32, now: f64, track: bool) -> Plan {
        // a region measured on another frame size (the phone was turned) no longer applies
        let crop = match self.roi {
            Some(r) if r.fw == fw && r.fh == fh => r.crop,
            _ if track && now - self.last_found_at > 700.0 && fw.min(fh) >= 720 => {
                // Searching a whole frame: two of three at half resolution (a finder is found
                // at three pixels per cell), every third at full resolution for small codes.
                self.searches += 1;
                Crop { k: if self.searches % 3 == 0 { 1.0 } else { 0.5 }, ..Crop::full(fw, fh) }
            }
            _ => Crop::full(fw, fh),
        };
        let hint = match self.tracked {
            Some((c, cell, at, w, h)) if track && now - at < 250.0 && (w, h) == (fw, fh) => Some((c, cell)),
            _ => None,
        };
        // (also forgets frames not seen for a while: their 8-bit seq comes round again)
        let skip = self.skip_map(now);
        // a torn capture's codewords may belong to the next frame, whatever its header says
        let skip = if self.tearing() { SkipMap::new() } else { skip };
        Plan { crop, captured_at: now, generation: self.generation, skip, fw, fh, rotation, track, prep_ms: 0.0, hint }
    }

    /// Scans a frame synchronously when the pacing says so (single-threaded hosts, tests).
    pub fn on_frame(&mut self, frame: &FrameRef, now: f64) -> Option<(ScanReport, Vec<Vec<u8>>)> {
        let plan = self.plan(frame.width, frame.height, frame.rotation, now, frame.is_valid())?;
        Some(self.run(frame, plan, now))
    }

    /// Scans a still image (all of it, at full resolution) regardless of pacing.
    pub fn scan_still(&mut self, frame: &FrameRef, now: f64) -> Option<(ScanReport, Vec<Vec<u8>>)> {
        if !frame.is_valid() {
            return None;
        }
        self.roi = None;
        let plan = self.make_plan(frame.width, frame.height, frame.rotation, now, false);
        Some(self.run(frame, plan, now))
    }

    fn run(&mut self, frame: &FrameRef, mut plan: Plan, now: f64) -> (ScanReport, Vec<Vec<u8>>) {
        let t0 = std::time::Instant::now();
        let mut img = std::mem::take(&mut self.img);
        let (dw, dh) = img.prepare(frame, &plan.crop);
        plan.prep_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = std::time::Instant::now();
        let res = img.scan(&mut self.scanner, dw, dh, &plan);
        let ms = t1.elapsed().as_secs_f64() * 1000.0;
        self.img = img;
        self.finish(&plan, &res, dw, dh, ms, now)
    }

    /// Books a finished scan. Results may arrive out of order from several scanners; an
    /// older capture never overrides the pacing or the region a newer one decided.
    pub fn finish(&mut self, plan: &Plan, res: &ScanResult, dw: usize, dh: usize, scan_ms: f64, now: f64) -> (ScanReport, Vec<Vec<u8>>) {
        let crop = plan.crop;
        let (fw, fh) = (plan.fw, plan.fh);
        let kx = dw as f64 / crop.w as f64;
        let ky = dh as f64 / crop.h as f64;
        let ms = plan.prep_ms + scan_ms;
        let current = plan.generation == self.generation;

        let mut packets = Vec::new();
        let mut ok_idx = Vec::new();
        if res.ok {
            for c in &res.codewords {
                if let (true, Some(b)) = (c.ok, &c.bytes) {
                    packets.push(b.clone());
                    ok_idx.push(c.index);
                }
            }
        }
        let header = res.header.map(|h| HeaderInfo { x: h.x, y: h.y, mode: h.mode, n: h.n, nsym: h.nsym });
        let stats = res.stats.as_ref().map(|st| {
            let corners = st.corners.map(|[x, y]| [x / kx + crop.x as f64, y / ky + crop.y as f64]);
            (corners, st.cell_px / ((kx + ky) / 2.0), st.cw_count)
        });
        let mut frame_rate = 0.0;
        if current {
            // bookkeeping, as onResult() in recv-app.js
            self.scan_ms.push_back(ms);
            if self.scan_ms.len() > 30 {
                self.scan_ms.pop_front();
            }
            self.session.ms_sum += ms;
            self.session.ms_n += 1;
            self.session.scans += 1;
            self.scan_times.push_back(now);
            while self.scan_times.front().is_some_and(|&t| now - t > 2000.0) {
                self.scan_times.pop_front();
            }
            if let Some(key) = res.frame_key {
                let total = stats.map_or(0, |s| s.2);
                if self.frame_done.get(&key).is_some_and(|e| now - e.first > FRAME_KEY_MS) {
                    self.frame_done.remove(&key); // the seq came round: a new frame
                }
                let e = self.frame_done.entry(key).or_insert_with(|| {
                    self.session.frames += 1;
                    self.session.cov_tot += total as u32;
                    FrameEntry { t: now, first: now, ok: HashSet::new(), seq: key.seq, total, scans: 0 }
                });
                e.t = now;
                e.scans += 1;
                for &i in &ok_idx {
                    if e.ok.insert(i) {
                        self.session.cov_ok += 1;
                    }
                }
                if e.total > 0 {
                    frame_rate = e.ok.len() as f64 / e.total as f64;
                }
                self.session.header = header;
            } else if res.skipped {
                frame_rate = 1.0;
            } else if let Some(s) = stats {
                if s.2 > 0 {
                    frame_rate = ok_idx.len() as f64 / s.2 as f64;
                }
            }
            if let (true, Some(s), false) = (res.ok, stats, res.skipped) {
                self.session.cell_sum += s.1;
                self.session.cell_n += 1;
                if s.2 > 0 {
                    self.yield_avg = 0.7 * self.yield_avg + 0.3 * ok_idx.len() as f64 / s.2 as f64;
                }
            }
            if let (true, true, Some(st)) = (plan.track, res.ok, stats) {
                self.tracked = Some((st.0, st.1, now, fw, fh));
            }
            if plan.track && plan.captured_at >= self.last_decision_at {
                self.last_decision_at = plan.captured_at;
                self.schedule(res.ok, res.skipped, res.frame_key, plan.captured_at);
                self.update_roi(res.ok, stats.map(|s| (s.0, s.1)), fw, fh);
            }
            if res.ok {
                self.last_found_at = now;
            }
            self.scan_hist.push_back(Rec { t: now, ok: res.ok, cell: stats.map_or(0.0, |s| s.1) });
            while self.scan_hist.front().is_some_and(|r| now - r.t > 2500.0) {
                self.scan_hist.pop_front();
            }
        }

        let report = ScanReport {
            generation: plan.generation,
            at: now,
            ok: res.ok,
            skipped: res.skipped,
            corners: if res.ok { stats.map(|s| s.0.map(|[x, y]| [x as f32, y as f32])) } else { None },
            frame_w: fw as u32,
            frame_h: fh as u32,
            rotation: plan.rotation,
            frame_rate: frame_rate as f32,
            header,
            stats: self.live(now),
            packets: packets.len() as u32,
        };
        (report, packets)
    }

    fn skip_map(&mut self, now: f64) -> SkipMap {
        self.frame_done.retain(|_, e| now - e.first <= 4000.0);
        self.frame_done.iter().filter(|(_, e)| now - e.first <= FRAME_KEY_MS).map(|(k, e)| (*k, e.ok.iter().copied().collect())).collect()
    }

    fn schedule(&mut self, ok: bool, skipped: bool, key: Option<FrameKey>, at: f64) {
        let p = self.sender_period();
        // After a partial decode the web demo rescans at once. Measured on real phone captures,
        // the codewords a good capture misses fail the same way in the next ones (a region of
        // glare, moiré or blur): rescans recovered ~0.3 packets each against ~34 from the next
        // sender frame. So only a poor capture (likely torn or blurred) gets one quick retry.
        self.next_scan_at = if !ok {
            0.0
        } else if skipped {
            at + 0.33 * p
        } else {
            match key.and_then(|k| self.frame_done.get(&k)) {
                Some(e) if e.total > 0 && e.ok.len() < e.total && e.scans < 2 && (e.ok.len() as f64) < 0.6 * e.total as f64 => 0.0,
                Some(_) => at + 0.75 * p,
                None => 0.0,
            }
        };
    }

    fn update_roi(&mut self, ok: bool, stats: Option<([[f64; 2]; 4], f64)>, fw: usize, fh: usize) {
        let Some((corners, cell)) = stats.filter(|_| ok) else {
            self.roi = None; // lost: whole frame, full resolution
            return;
        };
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for [x, y] in corners {
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        let mx = (x1 - x0) * 0.18 + 24.0;
        let my = (y1 - y0) * 0.18 + 24.0;
        let cx0 = (x0 - mx).floor().max(0.0) as usize;
        let cy0 = (y0 - my).floor().max(0.0) as usize;
        let cx1 = ((x1 + mx).ceil() as usize).min(fw);
        let cy1 = ((y1 + my).ceil() as usize).min(fh);
        let k = (TARGET_CELL / cell.max(0.1)).clamp(0.35, 1.0);
        // close to 1: no resampling at all (a row copy, and a sharper image)
        let k = if k > 0.8 { 1.0 } else { k };
        self.roi = if cx1 <= cx0 + 64 || cy1 <= cy0 + 64 {
            None
        } else {
            Some(RoiState { crop: Crop { x: cx0, y: cy0, w: cx1 - cx0, h: cy1 - cy0, k }, fw, fh })
        };
    }

    fn live(&mut self, now: f64) -> LiveStats {
        let mut st = LiveStats { last_found_at: if self.last_found_at.is_finite() { self.last_found_at } else { -1.0 }, ..Default::default() };
        let (mut cell, mut cell_n) = (0.0, 0u32);
        for r in &self.scan_hist {
            if r.ok {
                cell += r.cell;
                cell_n += 1;
            }
        }
        st.cell = if cell_n > 0 { (cell / cell_n as f64) as f32 } else { 0.0 };
        st.cell_n = cell_n;
        for e in self.frame_done.values() {
            if now - e.first < 2500.0 && e.total > 0 {
                st.cov_ok += e.ok.len() as u32;
                st.cov_tot += e.total as u32;
            }
        }
        let rate = |q: &VecDeque<f64>| -> f32 {
            match (q.front(), q.back()) {
                (Some(a), Some(b)) if q.len() > 1 && b > a => ((q.len() - 1) as f64 * 1000.0 / (b - a)) as f32,
                _ => 0.0,
            }
        };
        st.cam_fps = rate(&self.cam_frames);
        st.scan_fps = rate(&self.scan_times);
        let mut recent: Vec<(f64, u32)> =
            self.frame_done.values().filter(|e| now - e.first < 3000.0).map(|e| (e.first, e.seq)).collect();
        recent.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut span = 0u32;
        for w in recent.windows(2) {
            let d = (w[1].1 + 256 - w[0].1) & 255;
            if d > 0 && d < 64 {
                span += d;
            }
        }
        let dt = if recent.len() > 1 { (recent[recent.len() - 1].0 - recent[0].0) / 1000.0 } else { 0.0 };
        if dt > 0.5 {
            st.sender_fps = (span as f64 / dt) as f32;
            st.distinct_fps = ((recent.len() - 1) as f64 / dt) as f32;
        }
        if st.sender_fps > 0.0 {
            self.sender_fps_est = st.sender_fps as f64;
        }
        st.captured = if span > 0 { ((recent.len() - 1) as f32 / span as f32).min(1.0) } else { 0.0 };
        st.span_secs = dt as f32;
        st.avg_ms = if self.scan_ms.is_empty() { 0.0 } else { (self.scan_ms.iter().sum::<f64>() / self.scan_ms.len() as f64) as f32 };
        st.load = st.avg_ms * st.scan_fps / 1000.0 / self.workers.max(1) as f32;
        st.tearing = self.tearing();
        st.heat_limited = now - self.heat_refused_at < 1000.0;
        let s = &self.session;
        st.session = SessionStats {
            scans: s.scans,
            frames: s.frames,
            cov_ok: s.cov_ok,
            cov_tot: s.cov_tot,
            ms_avg: if s.ms_n > 0 { (s.ms_sum / s.ms_n as f64) as f32 } else { 0.0 },
            cell_avg: if s.cell_n > 0 { (s.cell_sum / s.cell_n as f64) as f32 } else { 0.0 },
            header: s.header,
        };
        st
    }
}
