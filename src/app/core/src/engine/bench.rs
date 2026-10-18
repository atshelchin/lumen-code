//! A self-test any device can run: the sender's frames, as a phone camera would see a phone
//! screen (rotated 90°, ~7 pixels per cell, slightly blurred, in 1080p NV12), go through the
//! real receive engine — pacing, region of interest, colour conversion, scan — and the
//! fountain decoder. It measures what a shell cannot fake: the time per scan on this CPU.

use super::image::{FrameRef, PixelFormat, Plane};
use super::recv::RecvEngine;
use crate::geometry::{compute, Density, Viewport, QUIET};
use lumen::transfer::{prepare, Finish, Receiver, Sender};
use lumen::Renderer;
use std::time::Instant;

pub struct SelfTest {
    pub camera_frames: usize,
    pub scans: u32,
    pub avg_scan_ms: f64,
    pub max_scan_ms: f64,
    pub first_scan_ms: f64,
    pub decoded: bool,
    pub bytes: usize,
    pub seconds_to_decode: f64,
    pub grid: (usize, usize),
}

impl SelfTest {
    pub fn summary(&self) -> String {
        format!(
            "grid {}x{} · {} camera frames · {} scans · scan {:.1} ms avg / {:.1} max · first (full frame) {:.1} ms · {} ({} B in {:.1} s of camera time)",
            self.grid.0,
            self.grid.1,
            self.camera_frames,
            self.scans,
            self.avg_scan_ms,
            self.max_scan_ms,
            self.first_scan_ms,
            if self.decoded { "decoded" } else { "NOT decoded" },
            self.bytes,
            self.seconds_to_decode
        )
    }
}

pub fn run(camera_frames: usize) -> SelfTest {
    // a phone sender, portrait, 12 fps
    let vp = Viewport { width_px: 1170.0, height_px: 2350.0, scale: 3.0, mm_per_px: 0.0533 };
    let g = compute(&vp, Density::Auto);
    let data: Vec<u8> = (0..24_000u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8).collect();
    let prepared = prepare(&data, "selftest.bin", false);
    let mut tx = Sender::new(&prepared, 1, 1, g.x, g.y, 0x5e1f, 0).expect("sender");
    let s = 7usize;
    let r = Renderer::new(&tx.layout, tx.mode, s, QUIET);
    let (cw, ch) = (r.width, r.height);
    let (fw, fh) = (1920usize, 1080usize);
    let mut code = vec![0u32; cw * ch];
    let mut y_plane = vec![0u8; fw * fh];
    let mut uv_plane = vec![0u8; fw * fh / 2];
    let mut rx = RecvEngine::new();
    rx.active = true;
    let mut receiver = Receiver::new();
    let mut shown = u64::MAX;
    let (mut scans, mut total, mut max, mut first) = (0u32, 0.0f64, 0.0f64, 0.0f64);
    let mut decoded_at = None;
    let mut bytes = 0;
    for i in 0..camera_frames {
        let now = i as f64 * 33.3;
        let sender_frame = (now * 12.0 / 1000.0) as u64;
        if sender_frame != shown {
            shown = sender_frame;
            let f = tx.next_frame();
            r.draw(&f.cells, &mut code);
            camera_image(&code, cw, ch, &mut y_plane, &mut uv_plane, fw, fh);
        }
        let frame = FrameRef {
            width: fw,
            height: fh,
            format: PixelFormat::Yuv420,
            planes: [
                Plane { data: &y_plane, row_stride: fw, pixel_stride: 1 },
                Plane { data: &uv_plane, row_stride: fw, pixel_stride: 2 },
                Plane { data: &uv_plane[1..], row_stride: fw, pixel_stride: 2 },
            ],
            rotation: 90,
        };
        let t0 = Instant::now();
        let out = rx.on_frame(&frame, now);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        if let Some((_, packets)) = out {
            scans += 1;
            total += ms;
            max = max.max(ms);
            if scans == 1 {
                first = ms;
            }
            for p in &packets {
                receiver.push(p);
            }
            if decoded_at.is_none() {
                if let Some(Finish::File { bytes: b, .. }) = receiver.try_finish() {
                    bytes = b.len();
                    decoded_at = Some(now / 1000.0);
                    rx.finished = true;
                }
            }
        }
    }
    SelfTest {
        camera_frames,
        scans,
        avg_scan_ms: if scans > 0 { total / scans as f64 } else { 0.0 },
        max_scan_ms: max,
        first_scan_ms: first,
        decoded: decoded_at.is_some() && bytes == data.len(),
        bytes,
        seconds_to_decode: decoded_at.unwrap_or(0.0),
        grid: (g.x, g.y),
    }
}

/// Replays raw camera frames (`frame-N.nv12`, the debug `grab` format) through the receive
/// engine, `rounds` times, on the calling thread: the real cost of real captures on a device.
pub fn replay(dir: &str, rounds: usize) -> String {
    let mut frames = Vec::new();
    for i in 1..=64 {
        let Ok(d) = std::fs::read(format!("{dir}/frame-{i}.nv12")) else { continue };
        if d.len() < 16 {
            continue;
        }
        let u = |k: usize| u32::from_le_bytes([d[k * 4], d[k * 4 + 1], d[k * 4 + 2], d[k * 4 + 3]]) as usize;
        let (w, h, ys, uvs) = (u(0), u(1), u(2), u(3));
        if d.len() < 16 + ys * h + uvs * (h / 2) {
            continue;
        }
        frames.push((w, h, ys, uvs, d));
    }
    if frames.is_empty() {
        return format!("no frames in {dir}");
    }
    let mut rx = RecvEngine::new();
    rx.set_active(true, 0.0);
    let (mut scans, mut total, mut max, mut packets) = (0u32, 0.0f64, 0.0f64, 0usize);
    let mut now = 0.0;
    for _ in 0..rounds {
        for (w, h, ys, uvs, d) in &frames {
            let uv0 = 16 + ys * h;
            let frame = FrameRef {
                width: *w,
                height: *h,
                format: PixelFormat::Yuv420,
                planes: [
                    Plane { data: &d[16..uv0], row_stride: *ys, pixel_stride: 1 },
                    Plane { data: &d[uv0..], row_stride: *uvs, pixel_stride: 2 },
                    Plane { data: &d[uv0 + 1..], row_stride: *uvs, pixel_stride: 2 },
                ],
                rotation: 90,
            };
            let t0 = Instant::now();
            if let Some((_, p)) = rx.on_frame(&frame, now) {
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                scans += 1;
                total += ms;
                max = max.max(ms);
                packets += p.len();
            }
            now += 33.3;
        }
    }
    format!("{} frames x {rounds} · {scans} scans · {:.1} ms avg / {:.1} max · {packets} packets", frames.len(), total / scans.max(1) as f64, max)
}

/// What a rolling-shutter camera passes on to the fountain decoder.
pub struct Rolling {
    /// Distinct packets per second, after the first second (search and lock).
    pub packets_per_sec: f64,
    /// The same as whole sender frames per second.
    pub frames_per_sec: f64,
    pub bytes_per_sec: f64,
    pub scans: u32,
    /// Captures holding parts of more than one sender frame, in percent.
    pub torn_pct: u32,
}

/// A camera at `cam_fps` whose rows are exposed one after another over `skew` ms watches a
/// sender at `sender_fps` (its clock a little off the camera's, so the tear wanders through
/// the frame). `readout` is what the engine is told about the skew (0: nothing).
pub fn rolling_shutter(sender_fps: f64, cam_fps: f64, skew: f64, secs: f64, readout: f64) -> Rolling {
    let sfps_true = sender_fps * 1.0037;
    let vp = Viewport { width_px: 1170.0, height_px: 2350.0, scale: 3.0, mm_per_px: 0.0533 };
    let g = compute(&vp, Density::Auto);
    let data: Vec<u8> = (0..600_000u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8).collect();
    let prepared = prepare(&data, "big.bin", false);
    let mut tx = Sender::new(&prepared, 1, 1, g.x, g.y, 0x5e1f, 0).expect("sender");
    let r = Renderer::new(&tx.layout, tx.mode, 7, QUIET);
    let (cw, ch) = (r.width, r.height);
    let (fw, fh) = (1920usize, 1080usize);
    let mut code = vec![0u32; cw * ch];
    // camera views of the sender frames on screen during the current capture
    let mut views: std::collections::VecDeque<(u64, Vec<u8>, Vec<u8>)> = Default::default();
    let mut next_sender = 0u64;
    let mut y_plane = vec![0u8; fw * fh];
    let mut uv_plane = vec![0u8; fw * fh / 2];
    let mut rx = RecvEngine::new();
    rx.set_active(true, 0.0);
    rx.readout_ms = readout;
    let mut receiver = Receiver::new();
    let (mut scans, mut torn) = (0u32, 0u32);
    let frames = (secs * cam_fps) as usize;
    let at = |k: usize| 300.0 + k as f64 * 1000.0 / cam_fps;
    let idx_at = |ts: f64| (ts * sfps_true / 1000.0) as u64;
    let mut base = None;
    for k in 0..frames {
        let t = at(k);
        let last = idx_at(t + skew);
        while next_sender <= last {
            let f = tx.next_frame();
            r.draw(&f.cells, &mut code);
            let mut yp = vec![0u8; fw * fh];
            let mut uv = vec![0u8; fw * fh / 2];
            camera_image(&code, cw, ch, &mut yp, &mut uv, fw, fh);
            views.push_back((next_sender, yp, uv));
            next_sender += 1;
        }
        let first = idx_at(t);
        while views.front().is_some_and(|v| v.0 < first) {
            views.pop_front();
        }
        torn += (last != first) as u32;
        for y in 0..fh {
            let i = idx_at(t + y as f64 / fh as f64 * skew);
            let v = views.iter().find(|v| v.0 == i).expect("view");
            y_plane[y * fw..(y + 1) * fw].copy_from_slice(&v.1[y * fw..(y + 1) * fw]);
            if y % 2 == 0 {
                uv_plane[(y / 2) * fw..(y / 2 + 1) * fw].copy_from_slice(&v.2[(y / 2) * fw..(y / 2 + 1) * fw]);
            }
        }
        let frame = FrameRef {
            width: fw,
            height: fh,
            format: PixelFormat::Yuv420,
            planes: [
                Plane { data: &y_plane, row_stride: fw, pixel_stride: 1 },
                Plane { data: &uv_plane, row_stride: fw, pixel_stride: 2 },
                Plane { data: &uv_plane[1..], row_stride: fw, pixel_stride: 2 },
            ],
            rotation: 90,
        };
        if let Some((_, packets)) = rx.on_frame(&frame, t) {
            scans += (t >= 1000.0) as u32;
            for p in &packets {
                receiver.push(p);
            }
        }
        if t >= 1000.0 && base.is_none() {
            base = Some((t, receiver.progress().map_or(0, |p| p.received)));
        }
    }
    let (t0, r0) = base.expect("longer than a second");
    let r1 = receiver.progress().map_or(0, |p| p.received);
    let pps = (r1 - r0) as f64 * 1000.0 / (at(frames - 1) - t0);
    Rolling {
        packets_per_sec: pps,
        frames_per_sec: pps / tx.per_frame as f64,
        bytes_per_sec: pps * tx.t as f64,
        scans,
        torn_pct: torn * 100 / frames as u32,
    }
}

/// The code rotated 90° into a 1080p frame on a dim background, blurred a little (optics),
/// as full-range NV12.
pub fn camera_image(code: &[u32], cw: usize, ch: usize, yp: &mut [u8], uv: &mut [u8], fw: usize, fh: usize) {
    // rotated: the code's height runs along the frame's width
    let (ow, oh) = (ch, cw);
    let ox = (fw.saturating_sub(ow)) / 2 + 37;
    let oy = (fh.saturating_sub(oh)) / 2 - 11;
    let mut rgb = vec![[38u8, 40, 46]; fw * fh];
    for y in 0..oh.min(fh - oy) {
        for x in 0..ow.min(fw - ox) {
            // rotate: frame (x, y) <- code (y, ch - 1 - x)
            let p = code[(ch - 1 - x) * cw + y].to_le_bytes();
            rgb[(oy + y) * fw + ox + x] = [p[0], p[1], p[2]];
        }
    }
    for y in 0..fh {
        for x in 0..fw {
            // 3×3 box blur
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for dy in 0..3 {
                let yy = (y + dy).saturating_sub(1).min(fh - 1);
                for dx in 0..3 {
                    let xx = (x + dx).saturating_sub(1).min(fw - 1);
                    let p = rgb[yy * fw + xx];
                    r += p[0] as u32;
                    g += p[1] as u32;
                    b += p[2] as u32;
                }
            }
            let (r, g, b) = (r as i32 / 9, g as i32 / 9, b as i32 / 9);
            yp[y * fw + x] = ((77 * r + 150 * g + 29 * b) >> 8).clamp(0, 255) as u8;
            if y % 2 == 0 && x % 2 == 0 {
                let u = 128 + ((-43 * r - 85 * g + 128 * b) >> 8);
                let v = 128 + ((128 * r - 107 * g - 21 * b) >> 8);
                uv[(y / 2) * fw + x] = u.clamp(0, 255) as u8;
                uv[(y / 2) * fw + x + 1] = v.clamp(0, 255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn selftest_decodes() {
        let t = super::run(150);
        println!("{}", t.summary());
        assert!(t.decoded, "{}", t.summary());
    }

    /// A camera reading its rows over a whole frame (a common phone sensor at 30 fps) still
    /// passes on nearly every sender frame: every capture is scanned, none skipped. Frame-locked
    /// pacing used to drop the captures holding the second half of a frame (12 of 20 frames/s).
    #[test]
    fn torn_captures_are_all_used() {
        let r = super::rolling_shutter(20.0, 30.0, 32.7, 2.5, 0.0);
        println!("{:.1} frames/s, {} scans, {}% torn", r.frames_per_sec, r.scans, r.torn_pct);
        assert!(r.torn_pct > 50 && r.frames_per_sec > 17.0, "{:.1} frames/s", r.frames_per_sec);
    }
}

#[cfg(test)]
mod split {
    use super::super::image::{prepare, Crop, FrameRef, PixelFormat, Plane};
    #[test]
    fn prepare_cost() {
        let (fw, fh) = (1920usize, 1080usize);
        let y = vec![100u8; fw * fh];
        let uv = vec![128u8; fw * fh / 2];
        let f = FrameRef {
            width: fw,
            height: fh,
            format: PixelFormat::Yuv420,
            planes: [
                Plane { data: &y, row_stride: fw, pixel_stride: 1 },
                Plane { data: &uv, row_stride: fw, pixel_stride: 2 },
                Plane { data: &uv[1..], row_stride: fw, pixel_stride: 2 },
            ],
            rotation: 90,
        };
        let mut out = Vec::new();
        for (name, c) in [("full", Crop::full(fw, fh)), ("roi k=.86", Crop { x: 400, y: 200, w: 1200, h: 640, k: 0.86 }), ("roi k=.5", Crop { x: 400, y: 200, w: 1200, h: 640, k: 0.5 })] {
            let t = std::time::Instant::now();
            for _ in 0..20 {
                prepare(&f, &c, &mut out);
            }
            println!("prepare {name}: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
        }
    }
}

/// Scans the same pre-rendered camera frames `iters` times (every scan a full decode of a new
/// frame, the worst case), for profiling. Returns ms per scan.
pub fn profile(iters: usize) -> f64 {
    use super::image::{prepare as prep, Crop};
    use lumen::scan::Scanner;
    let vp = Viewport { width_px: 1170.0, height_px: 2350.0, scale: 3.0, mm_per_px: 0.0533 };
    let g = compute(&vp, Density::Auto);
    let data: Vec<u8> = (0..24_000u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8).collect();
    let prepared = prepare(&data, "p.bin", false);
    let mut tx = Sender::new(&prepared, 1, 1, g.x, g.y, 0x5e1f, 0).expect("sender");
    let r = Renderer::new(&tx.layout, tx.mode, 7, QUIET);
    let (cw, ch) = (r.width, r.height);
    let (fw, fh) = (1920usize, 1080usize);
    let mut frames = Vec::new();
    let mut code = vec![0u32; cw * ch];
    for _ in 0..6 {
        let f = tx.next_frame();
        r.draw(&f.cells, &mut code);
        let mut y = vec![0u8; fw * fh];
        let mut uv = vec![0u8; fw * fh / 2];
        camera_image(&code, cw, ch, &mut y, &mut uv, fw, fh);
        frames.push((y, uv));
    }
    // the region a tracking receiver would scan (code + margin), at ~6 px per cell
    let k: f64 = std::env::var("LUMEN_K").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0 / 7.0);
    let crop = Crop { x: 300, y: 150, w: 1320, h: 780, k };
    let modes: &[bool] = if std::env::var("LUMEN_YUV_ONLY").is_ok() { &[true] } else { &[false, true] };
    for &yuv in modes {
    let mut scanner = Scanner::new();
    let mut buf = Vec::new();
    let mut img = super::recv::Prepared::default();
    let t0 = Instant::now();
    let mut ok = 0;
    for i in 0..iters {
        let (y, uv) = &frames[i % frames.len()];
        let frame = FrameRef {
            width: fw,
            height: fh,
            format: PixelFormat::Yuv420,
            planes: [
                Plane { data: y, row_stride: fw, pixel_stride: 1 },
                Plane { data: uv, row_stride: fw, pixel_stride: 2 },
                Plane { data: &uv[1..], row_stride: fw, pixel_stride: 2 },
            ],
            rotation: 90,
        };
        let res = if yuv {
            let (dw, dh) = img.prepare(&frame, &crop);
            let plan = super::recv::Plan { crop, captured_at: 0.0, generation: 0, skip: Default::default(), fw, fh, rotation: 90, track: false, prep_ms: 0.0, hint: None };
            img.scan(&mut scanner, dw, dh, &plan)
        } else {
            let (dw, dh) = prep(&frame, &crop, &mut buf);
            scanner.scan(&lumen::scan::Image { width: dw, height: dh, rgba: &buf[..dw * dh * 4] }, None)
        };
        ok += res.codewords.iter().filter(|c| c.ok).count();
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    eprintln!("profile {}: {ms:.2} ms per scan, {:.2} codewords ok per scan", if yuv { "yuv " } else { "rgba" }, ok as f64 / iters.max(1) as f64);
    }
    0.0
}
