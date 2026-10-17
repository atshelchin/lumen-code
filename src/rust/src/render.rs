//! Rasterise frames (informative: any rendering that keeps cell colours and pattern
//! geometry is valid). Rounded finders and "LED" cells as in the reference sender.

use crate::frame::{IDX_INK, IDX_PAPER};
use crate::layout::{Layout, INK, K_DATA, K_HEADER, MODES, PAPER};

/// RGBA packed little-endian (R in the low byte), as a canvas ImageData Uint32 view.
fn pack(rgb: [u8; 3]) -> u32 {
    0xff00_0000 | (rgb[2] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[0] as u32
}

fn rr_inside(px: f64, py: f64, cx: f64, cy: f64, half: f64, r: f64) -> bool {
    let dx = ((px - cx).abs() - (half - r)).max(0.0);
    let dy = ((py - cy).abs() - (half - r)).max(0.0);
    dx * dx + dy * dy <= r * r
}

pub struct Renderer {
    pub s: usize,
    pub q: usize,
    pub width: usize,
    pub height: usize,
    pub table: [u32; 10],
    pub ink: u32,
    spans: Vec<Option<(usize, usize)>>,
    static_layer: Vec<u32>,
    x: usize,
    y: usize,
    kind: Vec<u8>,
}

pub fn cell_style(s: usize) -> (usize, usize) {
    if s <= 4 {
        (0, 0)
    } else if s <= 7 {
        (1, 1)
    } else {
        (((s as f64 * 0.11).round() as usize).max(1), (s as f64 * 0.24).round() as usize)
    }
}

impl Renderer {
    pub fn new(layout: &Layout, mode: usize, s: usize, quiet: usize) -> Renderer {
        let (x, y) = (layout.x, layout.y);
        let width = (x + 2 * quiet) * s;
        let height = (y + 2 * quiet) * s;
        let pal = MODES[mode].palette;
        let mut table = [0u32; 10];
        for (i, t) in table.iter_mut().enumerate().take(8) {
            *t = pack(pal[i % pal.len()]);
        }
        table[IDX_INK as usize] = pack(INK);
        table[IDX_PAPER as usize] = pack(PAPER);
        let (gap, radius) = cell_style(s);
        let inner = s - gap;
        let spans = (0..s)
            .map(|yy| {
                if yy >= inner {
                    return None;
                }
                let (mut x0, mut x1) = (0usize, inner);
                if radius > 0 {
                    let (cy, r) = (yy as f64 + 0.5, radius as f64);
                    let dyc = if cy < r { r - cy } else if cy > inner as f64 - r { cy - (inner as f64 - r) } else { 0.0 };
                    if dyc > 0.0 {
                        x0 = (r - (r * r - dyc * dyc).max(0.0).sqrt()).round() as usize;
                        x1 = inner - x0;
                    }
                }
                Some((x0, x1))
            })
            .collect();
        let mut rd = Renderer { s, q: quiet, width, height, table, ink: pack(INK), spans, static_layer: vec![], x, y, kind: layout.kind.clone() };
        rd.static_layer = rd.build_static(layout);
        rd
    }

    fn build_static(&self, layout: &Layout) -> Vec<u32> {
        let (s, q, w) = (self.s, self.q, self.width);
        let mut px = vec![self.ink; self.width * self.height];
        let mut paint = |ox: usize, oy: usize, size: usize, inside: &dyn Fn(f64, f64) -> bool| {
            let (x0, y0) = ((ox + q) * s, (oy + q) * s);
            for yy in 0..size * s {
                for xx in 0..size * s {
                    let mut acc = 0;
                    for sy in 0..2 {
                        for sx in 0..2 {
                            let gx = (xx as f64 + 0.25 + sx as f64 * 0.5) / s as f64;
                            let gy = (yy as f64 + 0.25 + sy as f64 * 0.5) / s as f64;
                            acc += inside(gx, gy) as u32;
                        }
                    }
                    let a = acc as f64 / 4.0;
                    let mix = |c: usize| (INK[c] as f64 + (PAPER[c] as f64 - INK[c] as f64) * a).round() as u8;
                    px[(y0 + yy) * w + x0 + xx] = pack([mix(0), mix(1), mix(2)]);
                }
            }
        };
        let finder = |gx: f64, gy: f64| {
            if rr_inside(gx, gy, 3.5, 3.5, 1.5, 0.55) {
                true
            } else if rr_inside(gx, gy, 3.5, 3.5, 2.5, 0.95) {
                false
            } else {
                rr_inside(gx, gy, 3.5, 3.5, 3.5, 1.5)
            }
        };
        let corner = |gx: f64, gy: f64| {
            if rr_inside(gx, gy, 3.5, 3.5, 0.5, 0.5) {
                true
            } else if rr_inside(gx, gy, 3.5, 3.5, 2.5, 0.95) {
                false
            } else {
                rr_inside(gx, gy, 3.5, 3.5, 3.5, 1.5)
            }
        };
        let marker = |gx: f64, gy: f64| rr_inside(gx, gy, 1.5, 1.5, 0.5, 0.32);
        let (x, y) = (layout.x, layout.y);
        paint(0, 0, 7, &finder);
        paint(x - 7, 0, 7, &finder);
        paint(0, y - 7, 7, &finder);
        paint(x - 7, y - 7, 7, &corner);
        let (nx, ny) = (layout.lx.len(), layout.ly.len());
        for j in 0..ny {
            for i in 0..nx {
                let is_corner = (i == 0 || i == nx - 1) && (j == 0 || j == ny - 1);
                if !is_corner {
                    paint(layout.lx[i] - 1, layout.ly[j] - 1, 3, &marker);
                }
            }
        }
        px
    }

    fn is_cell(&self, i: usize) -> bool {
        let k = self.kind[i];
        k == K_DATA || k == K_HEADER
    }

    /// Full-resolution static mask: transparent interiors of data/header cells.
    pub fn mask_layer(&self) -> Vec<u32> {
        let (s, q, w) = (self.s, self.q, self.width);
        let mut px = self.static_layer.clone();
        for cy in 0..self.y {
            for cx in 0..self.x {
                if !self.is_cell(cy * self.x + cx) {
                    continue;
                }
                let base = (cy + q) * s * w + (cx + q) * s;
                for (yy, sp) in self.spans.iter().enumerate() {
                    if let Some((a, b)) = sp {
                        px[base + yy * w + a..base + yy * w + b].fill(0);
                    }
                }
            }
        }
        px
    }

    /// One pixel per cell, (X + 2q) × (Y + 2q).
    pub fn cell_pixels(&self, cells: &[u8], out: &mut [u32]) {
        let wq = self.x + 2 * self.q;
        for cy in 0..self.y {
            let row = (cy + self.q) * wq + self.q;
            for cx in 0..self.x {
                out[row + cx] = self.table[cells[cy * self.x + cx] as usize];
            }
        }
    }

    /// Full-resolution frame.
    pub fn draw(&self, cells: &[u8], out: &mut [u32]) {
        let (s, q, w) = (self.s, self.q, self.width);
        out.copy_from_slice(&self.static_layer);
        for cy in 0..self.y {
            for cx in 0..self.x {
                let i = cy * self.x + cx;
                if !self.is_cell(i) {
                    continue;
                }
                let color = self.table[cells[i] as usize];
                if color == self.ink {
                    continue;
                }
                let base = (cy + q) * s * w + (cx + q) * s;
                for (yy, sp) in self.spans.iter().enumerate() {
                    if let Some((a, b)) = sp {
                        out[base + yy * w + a..base + yy * w + b].fill(color);
                    }
                }
            }
        }
    }
}
