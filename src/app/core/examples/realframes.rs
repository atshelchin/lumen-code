//! Replays raw camera frames (frame-N.nv12 from the iOS debug "grab" command) through the
//! receive engine: cargo run --release --example realframes -- <dir> [rounds]
use lumen_app::engine::image::{FrameRef, PixelFormat, Plane};
use lumen_app::engine::recv::RecvEngine;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let rounds: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(20);
    let mut frames = Vec::new();
    for i in 1..=64 {
        let Ok(d) = std::fs::read(format!("{dir}/frame-{i}.nv12")) else { continue };
        let u = |k: usize| u32::from_le_bytes(d[k * 4..k * 4 + 4].try_into().unwrap()) as usize;
        let (w, h, ys, uvs) = (u(0), u(1), u(2), u(3));
        let y = d[16..16 + ys * h].to_vec();
        let uv = d[16 + ys * h..].to_vec();
        frames.push((w, h, ys, uvs, y, uv));
    }
    eprintln!("{} frames", frames.len());
    // preview of the first frame (raw RGB, for a PNG converter)
    if let Some((w, h, ys, uvs, y, uv)) = frames.first() {
        let mut rgb = Vec::with_capacity(w * h * 3);
        for yy in 0..*h {
            for xx in 0..*w {
                let l = y[yy * ys + xx] as f64;
                let k = (yy / 2) * uvs + (xx / 2) * 2;
                let (cb, cr) = (uv[k] as f64 - 128.0, uv[k + 1] as f64 - 128.0);
                rgb.push((l + 1.402 * cr).clamp(0.0, 255.0) as u8);
                rgb.push((l - 0.344 * cb - 0.714 * cr).clamp(0.0, 255.0) as u8);
                rgb.push((l + 1.772 * cb).clamp(0.0, 255.0) as u8);
            }
        }
        std::fs::write(format!("{dir}/preview.rgb"), &rgb).unwrap();
        eprintln!("preview {w}x{h}");
    }
    let mut rx = RecvEngine::new();
    rx.set_active(true, 0.0);
    let mut now = 0.0;
    let (mut scans, mut total, mut cw) = (0, 0.0, 0u32);
    let t0 = Instant::now();
    for r in 0..rounds {
        for (w, h, ys, uvs, y, uv) in &frames {
            let f = FrameRef {
                width: *w,
                height: *h,
                format: PixelFormat::Yuv420,
                planes: [
                    Plane { data: y, row_stride: *ys, pixel_stride: 1 },
                    Plane { data: uv, row_stride: *uvs, pixel_stride: 2 },
                    Plane { data: &uv[1..], row_stride: *uvs, pixel_stride: 2 },
                ],
                rotation: 90,
            };
            let t = Instant::now();
            if let Some((rep, packets)) = rx.on_frame(&f, now) {
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                scans += 1;
                total += ms;
                cw += packets.len() as u32;
                if r == 0 || r == rounds - 1 {
                    eprintln!(
                        "t={now:6.0} ok={} skipped={} packets={} cell={:.1} ms={ms:.1} rate={:.2} corners={:?}",
                        rep.ok, rep.skipped, packets.len(), rep.stats.cell, rep.frame_rate,
                        rep.corners.map(|c| c.map(|[x, y]| [x.round() as i32, y.round() as i32]))
                    );
                }
            }
            now += 33.3;
        }
    }
    println!("{scans} scans, {:.2} ms avg, {} packets, {:.1} s wall", total / scans.max(1) as f64, cw, t0.elapsed().as_secs_f64());
}
