//! Times the protocol scanner on one raw camera frame (frame-N.nv12 from a `grab`), at full
//! resolution, without and with a tracking hint: cargo run --release --example scanprof -- <file>
use lumen::scan::{Hint, Scanner, YuvImage};
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("a frame");
    let d = std::fs::read(path).unwrap();
    let u = |k: usize| u32::from_le_bytes(d[k * 4..k * 4 + 4].try_into().unwrap()) as usize;
    let (w, h, ys, uvs) = (u(0), u(1), u(2), u(3));
    let mut y = Vec::with_capacity(w * h);
    for r in 0..h {
        y.extend_from_slice(&d[16 + r * ys..16 + r * ys + w]);
    }
    let uv0 = 16 + ys * h;
    let mut cbcr = Vec::with_capacity(w * h / 2);
    for r in 0..h / 2 {
        cbcr.extend_from_slice(&d[uv0 + r * uvs..uv0 + r * uvs + w]);
    }
    let img = YuvImage { width: w, height: h, y: &y, cbcr: &cbcr };
    let mut sc = Scanner::new();
    let mut hint = None;
    for pass in 0..2 {
        let n: usize = std::env::var("N").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
        let t = Instant::now();
        let mut res = None;
        for _ in 0..n {
            res = Some(sc.scan_yuv(&img, None, hint));
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let r = res.unwrap();
        let ok = r.codewords.iter().filter(|c| c.ok).count();
        println!(
            "{} {ms:.1} ms · ok {} reason {:?} candidates {} reused {} · codewords {ok}/{}",
            if pass == 0 { "no hint " } else { "with hint" },
            r.ok, r.reason, r.candidates, r.header_reused, r.codewords.len()
        );
        if let Some(hd) = r.header {
            println!("  grid {}x{} mode {} n {} nsym {}", hd.x, hd.y, hd.mode, hd.n, hd.nsym);
        }
        if let Some(st) = r.stats {
            hint = Some(Hint { corners: st.corners, module: st.cell_px });
        }
    }
}
