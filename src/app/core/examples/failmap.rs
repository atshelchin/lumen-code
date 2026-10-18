//! Where codewords fail on real camera frames: cargo run --release --example failmap -- <dir>
use lumen::layout::{cells_per_codeword, Layout, MODES};
use lumen::scan::{Scanner, YuvImage};

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    for i in 1..=10 {
        let Ok(d) = std::fs::read(format!("{dir}/frame-{i}.nv12")) else { continue };
        let u = |k: usize| u32::from_le_bytes(d[k * 4..k * 4 + 4].try_into().unwrap()) as usize;
        let (w, h, ys, uvs) = (u(0), u(1), u(2), u(3));
        let mut y = vec![0u8; w * h];
        for r in 0..h {
            y[r * w..(r + 1) * w].copy_from_slice(&d[16 + r * ys..16 + r * ys + w]);
        }
        let uvbase = 16 + ys * h;
        let (cw, ch) = (w / 2, h / 2);
        let mut cbcr = vec![0u8; cw * ch * 2];
        for r in 0..ch {
            cbcr[r * cw * 2..(r + 1) * cw * 2].copy_from_slice(&d[uvbase + r * uvs..uvbase + r * uvs + cw * 2]);
        }
        let mut sc = Scanner::new();
        let res = sc.scan_yuv(&YuvImage { width: w, height: h, y: &y, cbcr: &cbcr }, None, None);
        let Some(hd) = res.header else {
            println!("frame {i}: {}", res.reason);
            continue;
        };
        let l = Layout::new(hd.x as usize, hd.y as usize).unwrap();
        let cpc = cells_per_codeword(hd.n as usize, MODES[hd.mode as usize].bpc);
        let mut state = vec![0u8; l.x * l.y]; // 0 none, 1 ok, 2 fail
        let mut ok = 0;
        for c in &res.codewords {
            for s in c.index * cpc..(c.index + 1) * cpc {
                state[l.data_order[s] as usize] = if c.ok { 1 } else { 2 };
            }
            ok += c.ok as usize;
        }
        let corrected: usize = res.codewords.iter().map(|c| c.corrected).sum();
        let c = res.stats.as_ref().unwrap().corners;
        let d = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        let (t, r, b, lf) = (d(c[0], c[1]), d(c[1], c[2]), d(c[2], c[3]), d(c[3], c[0]));
        let skew = (t.max(b) / t.min(b)).max(r.max(lf) / r.min(lf));
        println!("frame {i}: grid {}x{} ok {ok}/{} corrected symbols {corrected} skew {skew:.2} edges {t:.0} {r:.0} {b:.0} {lf:.0}", l.x, l.y, res.codewords.len());
        if false {
            let k = 4;
            for gy in (0..l.y).step_by(k) {
                let mut line = String::new();
                for gx in (0..l.x).step_by(k) {
                    let (mut f, mut o) = (0, 0);
                    for yy in gy..(gy + k).min(l.y) {
                        for xx in gx..(gx + k).min(l.x) {
                            match state[yy * l.x + xx] {
                                1 => o += 1,
                                2 => f += 1,
                                _ => {}
                            }
                        }
                    }
                    line.push(if f + o == 0 { ' ' } else if f > o { '#' } else if f > 0 { '+' } else { '.' });
                }
                println!("  {line}");
            }
        }
    }
}
