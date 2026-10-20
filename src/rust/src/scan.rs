//! Camera image -> codewords (informative decoder; see PROTOCOL §11).
//!
//! 1. luma + block thresholds
//! 2. finder (1:1:3:1:1) and corner (1:2:1:2:1) candidates by run scanning, cross-checked
//! 3. any three of the four corners: refine centres + module sizes, read a header copy
//! 4. alignment lattice found progressively under a least-squares homography
//! 5. per-quad homographies; each cell sampled and normalised by local white/ink references
//! 6. k-means colour classification with local gain correction; RS with erasure retry

use crate::frame::{decode_codewords, Codeword};
use crate::layout::{cells_per_codeword, codewords_per_frame, header_mask, unpack_header, Header, Layout, HEADER_BITS, INK, MODES, PAPER};
use std::collections::{HashMap, HashSet};

pub type H3 = [f64; 9];

pub fn homography(src: &[[f64; 2]; 4], dst: &[[f64; 2]; 4]) -> Option<H3> {
    let mut a = [[0f64; 9]; 8];
    for i in 0..4 {
        let [x, y] = src[i];
        let [u, v] = dst[i];
        a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    solve8(&mut a)
}

fn solve8(a: &mut [[f64; 9]; 8]) -> Option<H3> {
    for c in 0..8 {
        let mut p = c;
        for r in c + 1..8 {
            if a[r][c].abs() > a[p][c].abs() {
                p = r;
            }
        }
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        for r in 0..8 {
            if r == c {
                continue;
            }
            let f = a[r][c] / a[c][c];
            if f == 0.0 {
                continue;
            }
            for k in c..9 {
                a[r][k] -= f * a[c][k];
            }
        }
    }
    let mut h = [0f64; 9];
    for i in 0..8 {
        h[i] = a[i][8] / a[i][i];
    }
    h[8] = 1.0;
    Some(h)
}

#[inline]
pub fn apply_h(h: &H3, x: f64, y: f64) -> (f64, f64) {
    let w = h[6] * x + h[7] * y + h[8];
    ((h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w)
}

/// Least-squares homography (DLT, h8 = 1) with normalisation.
pub fn fit_homography(src: &[[f64; 2]], dst: &[[f64; 2]], wts: Option<&[f64]>) -> Option<H3> {
    let n = src.len();
    if n < 4 {
        return None;
    }
    let (mut sx, mut sy, mut dx, mut dy) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        sx += src[i][0];
        sy += src[i][1];
        dx += dst[i][0];
        dy += dst[i][1];
    }
    let nf = n as f64;
    sx /= nf;
    sy /= nf;
    dx /= nf;
    dy /= nf;
    let (mut ss, mut ds) = (0.0, 0.0);
    for i in 0..n {
        ss += (src[i][0] - sx).hypot(src[i][1] - sy);
        ds += (dst[i][0] - dx).hypot(dst[i][1] - dy);
    }
    ss /= nf;
    ds /= nf;
    if ss == 0.0 {
        ss = 1.0;
    }
    if ds == 0.0 {
        ds = 1.0;
    }
    let mut nm = [[0f64; 9]; 8];
    let acc = |r: &[f64; 8], rhs: f64, w: f64, nm: &mut [[f64; 9]; 8]| {
        for a in 0..8 {
            if r[a] == 0.0 {
                continue;
            }
            nm[a][8] += w * r[a] * rhs;
            for b in 0..8 {
                nm[a][b] += w * r[a] * r[b];
            }
        }
    };
    for i in 0..n {
        let x = (src[i][0] - sx) / ss;
        let y = (src[i][1] - sy) / ss;
        let u = (dst[i][0] - dx) / ds;
        let v = (dst[i][1] - dy) / ds;
        let w = wts.map_or(1.0, |w| w[i]);
        acc(&[x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y], u, w, &mut nm);
        acc(&[0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y], v, w, &mut nm);
    }
    let h = solve8(&mut nm)?;
    let ts = [1.0 / ss, 0.0, -sx / ss, 0.0, 1.0 / ss, -sy / ss, 0.0, 0.0, 1.0];
    let tdi = [ds, 0.0, dx, 0.0, ds, dy, 0.0, 0.0, 1.0];
    let mul = |p: &H3, q: &H3| {
        let mut r = [0f64; 9];
        for i in 0..3 {
            for j in 0..3 {
                r[i * 3 + j] = p[i * 3] * q[j] + p[i * 3 + 1] * q[3 + j] + p[i * 3 + 2] * q[6 + j];
            }
        }
        r
    };
    let mut hh = mul(&tdi, &mul(&h, &ts));
    let d = hh[8];
    for v in hh.iter_mut() {
        *v /= d;
    }
    Some(hh)
}

fn solve3(m: [f64; 9], b: [f64; 3]) -> Option<[f64; 3]> {
    let [a, b1, c, d, e, f, g, h, i] = m;
    let det = a * (e * i - f * h) - b1 * (d * i - f * g) + c * (d * h - e * g);
    if det.abs() < 1e-12 {
        return None;
    }
    let x = (b[0] * (e * i - f * h) - b1 * (b[1] * i - f * b[2]) + c * (b[1] * h - e * b[2])) / det;
    let y = (a * (b[1] * i - f * b[2]) - b[0] * (d * i - f * g) + c * (d * b[2] - b[1] * g)) / det;
    let z = (a * (e * b[2] - b[1] * h) - b1 * (d * b[2] - b[1] * g) + b[0] * (d * h - e * g)) / det;
    Some([x, y, z])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameKey {
    pub seq: u32,
    pub x: u32,
    pub y: u32,
    pub mode: u32,
    pub n: u32,
    pub nsym: u32,
}

/// Codeword indices already decoded for recently seen frames.
pub type SkipMap = HashMap<FrameKey, Vec<usize>>;

pub struct Image<'a> {
    pub width: usize,
    pub height: usize,
    /// RGBA, 4 bytes per pixel
    pub rgba: &'a [u8],
}

/// A Y'CbCr 4:2:0 image as cameras deliver it (full-range BT.601): the luma plane is the
/// scanner's own luma, and colour is converted only where cells are sampled.
pub struct YuvImage<'a> {
    pub width: usize,
    pub height: usize,
    /// Luma, `width` bytes per row.
    pub y: &'a [u8],
    /// Cb, Cr interleaved at half resolution in both directions: `(width + 1) / 2` pairs per row,
    /// `(height + 1) / 2` rows.
    pub cbcr: &'a [u8],
}

/// Where the code was a moment ago (tracking): the four corners in this image's pixels and the
/// module size. Finders are then searched only in windows around those corners; the full
/// search runs only if that finds too little.
#[derive(Clone, Copy, Debug)]
pub struct Hint {
    pub corners: [[f64; 2]; 4],
    pub module: f64,
}

/// Where colours come from.
#[derive(Clone, Copy)]
enum Pixels<'a> {
    Rgba(&'a [u8]),
    Yuv { cbcr: &'a [u8], cw: usize, ch: usize },
}

#[derive(Clone, Debug, Default)]
pub struct ScanStats {
    pub cell_px: f64,
    pub corners: [[f64; 2]; 4],
    pub markers: f64,
    pub corner_found: bool,
    pub cw_ok: usize,
    pub cw_total: usize,
    pub cw_count: usize,
    pub corrected: usize,
}

pub struct ScanResult {
    pub ok: bool,
    pub skipped: bool,
    pub reason: &'static str,
    pub candidates: usize,
    pub header: Option<Header>,
    pub frame_key: Option<FrameKey>,
    pub codewords: Vec<Codeword>,
    pub stats: Option<ScanStats>,
    /// Every header copy was unreadable and the last good header was used instead
    /// (`header.seq` is then `SEQ_UNKNOWN` and there is no frame key).
    pub header_reused: bool,
}

/// `Header::seq` of a reused header: the frame number is unknown.
pub const SEQ_UNKNOWN: u32 = u32::MAX;

/// Header-less geometries tried with the last good header (bounds the extra work per frame).
const REUSE_CANDIDATES: usize = 4;

impl ScanResult {
    fn fail(reason: &'static str, candidates: usize) -> Self {
        ScanResult { ok: false, skipped: false, reason, candidates, header: None, frame_key: None, codewords: vec![], stats: None, header_reused: false }
    }
}

#[derive(Clone, Copy, Debug)]
struct Cand {
    x: f64,
    y: f64,
    m: f64,
    count: u32,
    corner: bool,
}

#[derive(Clone, Copy, Debug)]
struct Refined {
    x: f64,
    y: f64,
    mu: f64,
    mv: f64,
    white: f64,
    ink: f64,
}

#[derive(Clone, Copy)]
struct CornerSet {
    c: [Option<usize>; 4], // TL TR BL BR -> candidate index
    score: f64,
    key: [usize; 3],
}

#[derive(Clone, Copy, PartialEq)]
enum Score {
    Marker,
    Corner,
}

type V2 = [f64; 2];

#[derive(Default)]
pub struct Scanner {
    luma: Vec<u8>,
    thr: Vec<u8>,
    bmin: Vec<u8>,
    bmax: Vec<u8>,
    // Last header read correctly. X, Y, mode, n and nsym do not change within a transfer and
    // seq is not needed to decode data, so it stands in when every copy of a header is unreadable.
    last_header: Option<Header>,
    reuse_misses: u32,
    // The grid layout of the last decoded frame: a transfer keeps its size, and building a
    // layout (Hilbert order, header positions) costs as much as a tenth of a scan.
    layout: std::cell::RefCell<Option<std::sync::Arc<Layout>>>,
}

struct Ctx<'a> {
    w: usize,
    h: usize,
    bs: usize,
    bw: usize,
    luma: &'a [u8],
    thr: &'a [u8],
    px: Pixels<'a>,
    layout_cache: &'a std::cell::RefCell<Option<std::sync::Arc<Layout>>>,
    hint: Option<Hint>,
    /// Whether `thr` covers the whole image (else only the hint windows).
    thr_full: bool,
}

const TL: usize = 0;
const TR: usize = 1;
const BL: usize = 2;
const BR: usize = 3;

impl Scanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan(&mut self, img: &Image, skip: Option<&SkipMap>) -> ScanResult {
        self.scan_hinted(img, skip, None)
    }

    /// [`Scanner::scan`] with a tracking hint (see [`Hint`]).
    pub fn scan_hinted(&mut self, img: &Image, skip: Option<&SkipMap>, hint: Option<Hint>) -> ScanResult {
        let (w, h) = (img.width, img.height);
        if w < 16 || h < 16 || img.rgba.len() < w * h * 4 {
            return ScanResult::fail("image", 0);
        }
        // luma (buffers only grow)
        if self.luma.len() < w * h {
            self.luma.resize(w * h, 0);
        }
        let rgba = img.rgba;
        for (l, p) in self.luma[..w * h].iter_mut().zip(rgba[..w * h * 4].chunks_exact(4)) {
            *l = ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8;
        }
        let luma = std::mem::take(&mut self.luma);
        let res = self.scan_luma(w, h, &luma[..w * h], Pixels::Rgba(rgba), skip, hint);
        self.luma = luma;
        res
    }

    /// Scans a Y'CbCr image without converting it: the camera's luma is used as is.
    pub fn scan_yuv(&mut self, img: &YuvImage, skip: Option<&SkipMap>, hint: Option<Hint>) -> ScanResult {
        let (w, h) = (img.width, img.height);
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        if w < 16 || h < 16 || img.y.len() < w * h || img.cbcr.len() < cw * ch * 2 {
            return ScanResult::fail("image", 0);
        }
        self.scan_luma(w, h, &img.y[..w * h], Pixels::Yuv { cbcr: img.cbcr, cw, ch }, skip, hint)
    }

    fn scan_luma(&mut self, w: usize, h: usize, luma: &[u8], px: Pixels, skip: Option<&SkipMap>, hint: Option<Hint>) -> ScanResult {
        // block thresholds (buffers only grow)
        let bs = if w.min(h) >= 1400 { 5 } else { 4 };
        let bsz = 1usize << bs;
        let bw = w.div_ceil(bsz);
        let bh = h.div_ceil(bsz);
        let nb = bw * bh;
        if self.thr.len() < nb {
            self.thr.resize(nb, 0);
            self.bmin.resize(nb, 0);
            self.bmax.resize(nb, 0);
        }
        // With a tracking hint the finders are looked for only in windows around the expected
        // corners (find_candidates): thresholds there are enough, unless that search fails.
        let regions: Option<Vec<[usize; 4]>> = hint.map(|hn| {
            let r = hint_radius(&hn);
            // the window, plus room for the cross-checks that start near its edge, plus the
            // two blocks a threshold looks at around its own
            let m = r * 1.5;
            hn.corners
                .iter()
                .map(|&[x, y]| {
                    let bx0 = ((x - m).max(0.0) as usize >> bs).saturating_sub(2);
                    let by0 = ((y - m).max(0.0) as usize >> bs).saturating_sub(2);
                    let bx1 = ((((x + m).max(0.0) as usize).min(w - 1) >> bs) + 3).min(bw);
                    let by1 = ((((y + m).max(0.0) as usize).min(h - 1) >> bs) + 3).min(bh);
                    [bx0, by0, bx1, by1]
                })
                .collect()
        });
        self.thresholds(luma, w, h, bs, bw, bh, regions.as_deref());
        let mut ctx = Ctx { w, h, bs, bw, luma, thr: &self.thr[..nb], px, layout_cache: &self.layout, hint, thr_full: regions.is_none() };
        let (mut res, mut tried_reuse) = ctx.run(skip, self.last_header);
        if res.reason == RETHRESHOLD {
            // the windows missed: the whole image after all
            self.thresholds(luma, w, h, bs, bw, bh, None);
            ctx = Ctx { w, h, bs, bw, luma, thr: &self.thr[..nb], px, layout_cache: &self.layout, hint, thr_full: true };
            (res, tried_reuse) = ctx.run(skip, self.last_header);
        }
        if res.ok && !res.header_reused {
            self.last_header = res.header;
            self.reuse_misses = 0;
        } else if tried_reuse {
            let useful = res.ok && res.stats.as_ref().is_some_and(|st| st.cw_ok > 0);
            self.reuse_misses = if useful { 0 } else { self.reuse_misses + 1 };
            if self.reuse_misses >= 3 {
                self.last_header = None;
            }
        }
        res
    }
}

impl Scanner {
    /// Block thresholds (the midpoint of the luma range around each 2^bs-pixel block), for the
    /// whole image or only for the blocks in `regions` ([bx0, by0, bx1, by1), others stay 255:
    /// never bright).
    #[allow(clippy::too_many_arguments)]
    fn thresholds(&mut self, luma: &[u8], w: usize, h: usize, bs: usize, bw: usize, bh: usize, regions: Option<&[[usize; 4]]>) {
        let bsz = 1usize << bs;
        let all = [[0, 0, bw, bh]];
        let regions = regions.unwrap_or(&all);
        let partial = regions.len() != 1 || regions[0] != all[0];
        if partial {
            self.thr[..bw * bh].iter_mut().for_each(|t| *t = 255);
        }
        // min / max per block (every other pixel of every other row), over each region
        // widened by the two blocks a threshold looks at
        let (mut gmin, mut gmax) = (255u8, 0u8);
        for &[x0, y0, x1, y1] in regions {
            for by in y0.saturating_sub(2)..(y1 + 2).min(bh) {
                for bx in x0.saturating_sub(2)..(x1 + 2).min(bw) {
                    let (mut mn, mut mx) = (255u8, 0u8);
                    let yend = h.min((by + 1) * bsz);
                    let xend = w.min((bx + 1) * bsz);
                    let mut y = by * bsz;
                    while y < yend {
                        let row = y * w;
                        let mut x = bx * bsz;
                        while x < xend {
                            let v = luma[row + x];
                            mn = mn.min(v);
                            mx = mx.max(v);
                            x += 2;
                        }
                        y += 2;
                    }
                    self.bmax[by * bw + bx] = mx;
                    self.bmin[by * bw + bx] = mn;
                    gmin = gmin.min(mn);
                    gmax = gmax.max(mx);
                }
            }
        }
        let min_range = 40f64.max(0.3 * (gmax as f64 - gmin as f64));
        for &[x0, y0, x1, y1] in regions {
            for by in y0..y1 {
                for bx in x0..x1 {
                    let (mut mn, mut mx) = (255u8, 0u8);
                    for yy in by.saturating_sub(2)..(by + 3).min(bh) {
                        for xx in bx.saturating_sub(2)..(bx + 3).min(bw) {
                            let k = yy * bw + xx;
                            mn = mn.min(self.bmin[k]);
                            mx = mx.max(self.bmax[k]);
                        }
                    }
                    self.thr[by * bw + bx] = if ((mx - mn) as f64) < min_range { 255 } else { ((mn as u16 + mx as u16) >> 1) as u8 };
                }
            }
        }
    }
}

/// `ScanResult::reason` when a hinted search missed and needs thresholds for the whole image.
const RETHRESHOLD: &str = "rethreshold";

/// Half the side of the window a hinted finder search looks in: the 7-cell finder plus its
/// separator and some slack for perspective and motion (a window cut at the finder's own size
/// misses it often and costs a full search).
fn hint_radius(hint: &Hint) -> f64 {
    (hint.module * 10.0).max(32.0)
}

impl<'a> Ctx<'a> {
    #[inline]
    fn bright(&self, x: usize, y: usize) -> bool {
        self.luma[y * self.w + x] > self.thr[(y >> self.bs) * self.bw + (x >> self.bs)]
    }

    #[inline]
    fn luma_at(&self, x: f64, y: f64) -> f64 {
        let (w, h) = (self.w as f64, self.h as f64);
        let x = (x - 0.5).clamp(0.0, w - 1.001);
        let y = (y - 0.5).clamp(0.0, h - 1.001);
        let (ix, iy) = (x as usize, y as usize);
        let (ax, ay) = (x - ix as f64, y - iy as f64);
        let i = iy * self.w + ix;
        let l = self.luma;
        // the clamps above keep ix + 1 < w and iy + 1 < h
        assert!(i + self.w + 1 < l.len());
        // SAFETY: checked just above
        let (p00, p10, p01, p11) = unsafe { (*l.get_unchecked(i), *l.get_unchecked(i + 1), *l.get_unchecked(i + self.w), *l.get_unchecked(i + self.w + 1)) };
        let a = p00 as f64 + (p10 as f64 - p00 as f64) * ax;
        let b = p01 as f64 + (p11 as f64 - p01 as f64) * ax;
        a + (b - a) * ay
    }

    /// Cb, Cr (centred on 0) at a luma-pixel position, bilinear.
    #[inline]
    fn chroma_at(x: f64, y: f64, cbcr: &[u8], cw: usize, ch: usize) -> (f64, f64) {
        // chroma sample (cx, cy) is centred on luma (2cx + 1, 2cy + 1)
        let cx = ((x - 0.5) * 0.5 - 0.25).clamp(0.0, cw as f64 - 1.001);
        let cy = ((y - 0.5) * 0.5 - 0.25).clamp(0.0, ch as f64 - 1.001);
        let (jx, jy) = (cx as usize, cy as usize);
        let (bx, by) = (cx - jx as f64, cy - jy as f64);
        let k = (jy * cw + jx) * 2;
        let k2 = k + cw * 2;
        let (v00, v10, v01, v11) = ((1.0 - bx) * (1.0 - by), bx * (1.0 - by), (1.0 - bx) * by, bx * by);
        let cb = cbcr[k] as f64 * v00 + cbcr[k + 2] as f64 * v10 + cbcr[k2] as f64 * v01 + cbcr[k2 + 2] as f64 * v11 - 128.0;
        let cr = cbcr[k + 1] as f64 * v00 + cbcr[k + 3] as f64 * v10 + cbcr[k2 + 1] as f64 * v01 + cbcr[k2 + 3] as f64 * v11 - 128.0;
        (cb, cr)
    }

    /// One data cell's colour: six samples summed, the centre twice and four around it
    /// (`du`, `dv` away), as the reference colours are measured.
    #[inline]
    fn cell_rgb(&self, px: f64, py: f64, du: V2, dv: V2) -> [f64; 3] {
        match self.px {
            Pixels::Yuv { cbcr, cw, ch } => {
                // The clamps aside, the mean of six RGB samples is the RGB of the mean Y'CbCr, and
                // chroma (half resolution) hardly changes within a cell: average the luma, read
                // the chroma once. A fifth of the work of six full samples (native path only;
                // the RGBA path below is the one the JavaScript scanner matches).
                let l = 2.0 * self.luma_at(px, py)
                    + self.luma_at(px + du[0], py + du[1])
                    + self.luma_at(px - du[0], py - du[1])
                    + self.luma_at(px + dv[0], py + dv[1])
                    + self.luma_at(px - dv[0], py - dv[1]);
                let yv = l / 6.0;
                let (cb, cr) = Self::chroma_at(px, py, cbcr, cw, ch);
                [
                    6.0 * (yv + 1.402 * cr).clamp(0.0, 255.0),
                    6.0 * (yv - 0.344_136 * cb - 0.714_136 * cr).clamp(0.0, 255.0),
                    6.0 * (yv + 1.772 * cb).clamp(0.0, 255.0),
                ]
            }
            Pixels::Rgba(_) => {
                let mut a = [0f64; 3];
                self.rgb_at(px, py, &mut a);
                a = [a[0] * 2.0, a[1] * 2.0, a[2] * 2.0];
                self.rgb_at(px + du[0], py + du[1], &mut a);
                self.rgb_at(px - du[0], py - du[1], &mut a);
                self.rgb_at(px + dv[0], py + dv[1], &mut a);
                self.rgb_at(px - dv[0], py - dv[1], &mut a);
                a
            }
        }
    }

    #[inline]
    fn rgb_at(&self, x: f64, y: f64, out: &mut [f64; 3]) {
        let (w, h) = (self.w as f64, self.h as f64);
        let x = (x - 0.5).clamp(0.0, w - 1.001);
        let y = (y - 0.5).clamp(0.0, h - 1.001);
        let (ix, iy) = (x as usize, y as usize);
        let (ax, ay) = (x - ix as f64, y - iy as f64);
        let i = (iy * self.w + ix) * 4;
        let j = i + self.w * 4;
        let (w00, w10, w01, w11) = ((1.0 - ax) * (1.0 - ay), ax * (1.0 - ay), (1.0 - ax) * ay, ax * ay);
        match self.px {
            Pixels::Rgba(p) => {
                for c in 0..3 {
                    out[c] += p[i + c] as f64 * w00 + p[i + 4 + c] as f64 * w10 + p[j + c] as f64 * w01 + p[j + 4 + c] as f64 * w11;
                }
            }
            Pixels::Yuv { cbcr, cw, ch } => {
                let l = self.luma;
                let (i, j) = (i / 4, j / 4);
                let yv = l[i] as f64 * w00 + l[i + 1] as f64 * w10 + l[j] as f64 * w01 + l[j + 1] as f64 * w11;
                // chroma sample (cx, cy) is centred on luma (2cx + 1, 2cy + 1)
                let cx = (x * 0.5 - 0.25).clamp(0.0, cw as f64 - 1.001);
                let cy = (y * 0.5 - 0.25).clamp(0.0, ch as f64 - 1.001);
                let (jx, jy) = (cx as usize, cy as usize);
                let (bx, by) = (cx - jx as f64, cy - jy as f64);
                let k = (jy * cw + jx) * 2;
                let k2 = k + cw * 2;
                let (v00, v10, v01, v11) = ((1.0 - bx) * (1.0 - by), bx * (1.0 - by), (1.0 - bx) * by, bx * by);
                let cb = cbcr[k] as f64 * v00 + cbcr[k + 2] as f64 * v10 + cbcr[k2] as f64 * v01 + cbcr[k2 + 2] as f64 * v11 - 128.0;
                let cr = cbcr[k + 1] as f64 * v00 + cbcr[k + 3] as f64 * v10 + cbcr[k2 + 1] as f64 * v01 + cbcr[k2 + 3] as f64 * v11 - 128.0;
                out[0] += (yv + 1.402 * cr).clamp(0.0, 255.0);
                out[1] += (yv - 0.344_136 * cb - 0.714_136 * cr).clamp(0.0, 255.0);
                out[2] += (yv + 1.772 * cb).clamp(0.0, 255.0);
            }
        }
    }

    fn ratio_ok(c: &[usize; 5]) -> bool {
        let total: usize = c.iter().sum();
        if total < 7 {
            return false;
        }
        let m = total as f64 / 7.0;
        let (lo, hi) = (0.3 * m, 1.9 * m);
        let f = |v: usize| v as f64;
        f(c[0]) > lo && f(c[0]) < hi && f(c[1]) > lo * 0.8 && f(c[1]) < hi && f(c[3]) > lo * 0.8 && f(c[3]) < hi && f(c[4]) > lo && f(c[4]) < hi
            && f(c[2]) > 1.7 * m && f(c[2]) < 4.6 * m
    }

    fn ratio_corner(c: &[usize; 5]) -> bool {
        let total: usize = c.iter().sum();
        if total < 7 {
            return false;
        }
        let m = total as f64 / 7.0;
        let f = |v: usize| v as f64;
        f(c[0]) > 0.35 * m && f(c[0]) < 1.9 * m && f(c[4]) > 0.35 * m && f(c[4]) < 1.9 * m && f(c[1]) > 1.1 * m && f(c[1]) < 3.2 * m
            && f(c[3]) > 1.1 * m && f(c[3]) < 3.2 * m && f(c[2]) > 0.3 * m && f(c[2]) < 1.9 * m
    }

    /// Runs along (dx, dy) through (cx, cy). Returns (centre offset, total length).
    fn cross_check(&self, cx: i64, cy: i64, dx: i64, dy: i64, max_count: f64, orig_total: usize, corner: bool) -> Option<(f64, usize)> {
        let (w, h) = (self.w as i64, self.h as i64);
        let inb = |x: i64, y: i64| x >= 0 && y >= 0 && x < w && y < h;
        let br = |x: i64, y: i64| self.bright(x as usize, y as usize);
        let mut c = [0usize; 5];
        let (mut x, mut y) = (cx, cy);
        while inb(x, y) && br(x, y) {
            c[2] += 1;
            x -= dx;
            y -= dy;
        }
        if !inb(x, y) {
            return None;
        }
        while inb(x, y) && !br(x, y) && (c[1] as f64) <= max_count {
            c[1] += 1;
            x -= dx;
            y -= dy;
        }
        if !inb(x, y) || c[1] as f64 > max_count {
            return None;
        }
        while inb(x, y) && br(x, y) && (c[0] as f64) <= max_count {
            c[0] += 1;
            x -= dx;
            y -= dy;
        }
        if c[0] as f64 > max_count {
            return None;
        }
        x = cx + dx;
        y = cy + dy;
        let mut f = 1usize;
        while inb(x, y) && br(x, y) {
            c[2] += 1;
            x += dx;
            y += dy;
            f += 1;
        }
        if !inb(x, y) {
            return None;
        }
        while inb(x, y) && !br(x, y) && (c[3] as f64) <= max_count {
            c[3] += 1;
            x += dx;
            y += dy;
            f += 1;
        }
        if !inb(x, y) || c[3] as f64 > max_count {
            return None;
        }
        while inb(x, y) && br(x, y) && (c[4] as f64) <= max_count {
            c[4] += 1;
            x += dx;
            y += dy;
            f += 1;
        }
        if c[4] as f64 > max_count {
            return None;
        }
        let total: usize = c.iter().sum();
        if 5 * (total as i64 - orig_total as i64).unsigned_abs() as usize >= 3 * orig_total {
            return None;
        }
        let ok = if corner { Self::ratio_corner(&c) } else { Self::ratio_ok(&c) };
        if !ok {
            return None;
        }
        Some((f as f64 - c[4] as f64 - c[3] as f64 - c[2] as f64 / 2.0, total))
    }

    /// Finder and corner candidates: everywhere, or (tracking) only near the hinted corners,
    /// falling back to everywhere when the windows hold fewer than three.
    /// Finder candidates; None when a hinted search missed and only the windows have thresholds.
    fn find_candidates(&self) -> Option<Vec<Cand>> {
        if let Some(hint) = self.hint {
            let r = hint_radius(&hint);
            let windows: Vec<(usize, usize, usize, usize)> = hint
                .corners
                .iter()
                .map(|&[x, y]| {
                    let x0 = (x - r).floor().max(0.0) as usize;
                    let y0 = (y - r).floor().max(0.0) as usize;
                    let x1 = ((x + r).ceil() as usize).min(self.w);
                    let y1 = ((y + r).ceil() as usize).min(self.h);
                    (x0, y0, x1, y1)
                })
                .filter(|&(x0, y0, x1, y1)| x1 > x0 + 8 && y1 > y0 + 8)
                .collect();
            let mut cands = Vec::new();
            for &(x0, y0, x1, y1) in &windows {
                self.find_in(x0, y0, x1, y1, 1, &mut cands);
            }
            if cands.len() >= 3 {
                return Some(cands);
            }
            if !self.thr_full {
                return None;
            }
        }
        let mut cands = Vec::new();
        self.find_in(0, 0, self.w, self.h, (self.h / 360).max(1), &mut cands);
        Some(cands)
    }

    /// Row scan for 1:1:3:1:1 finders and 1:2:1:2:1 corner patterns within a window.
    fn find_in(&self, x0: usize, y0: usize, x1: usize, y1: usize, step: usize, cands: &mut Vec<Cand>) {
        let (w, h) = (x1 - x0, self.h);
        let _ = h;
        let mut run_len = vec![0usize; w + 1];
        let mut run_start = vec![0usize; w + 1];
        let mut run_bright = vec![false; w + 1];
        // One row binarised at a time, block by block (one threshold per block): the same
        // test as `bright`, in a form the compiler vectorises.
        let mut bits = vec![0u8; w];
        let bsz = 1usize << self.bs;
        let mut y = y0 + (step >> 1);
        while y < y1 {
            let row = &self.luma[y * self.w + x0..y * self.w + x1];
            let trow = &self.thr[(y >> self.bs) * self.bw..];
            if x0 == 0 {
                for ((out, lum), &t) in bits.chunks_mut(bsz).zip(row.chunks(bsz)).zip(trow) {
                    for (o, &l) in out.iter_mut().zip(lum) {
                        *o = (l > t) as u8;
                    }
                }
            } else {
                for (i, (o, &l)) in bits.iter_mut().zip(row).enumerate() {
                    *o = (l > trow[(x0 + i) >> self.bs]) as u8;
                }
            }
            let mut n = 0usize;
            let mut prev = 2u8;
            for (x, &b) in bits.iter().enumerate() {
                if b != prev {
                    run_start[n] = x0 + x;
                    run_len[n] = 1;
                    run_bright[n] = b == 1;
                    n += 1;
                    prev = b;
                } else {
                    run_len[n - 1] += 1;
                }
            }
            let mut i = 0;
            while i + 4 < n {
                if !run_bright[i] {
                    i += 1;
                    continue;
                }
                let c = [run_len[i], run_len[i + 1], run_len[i + 2], run_len[i + 3], run_len[i + 4]];
                let is_f = Self::ratio_ok(&c);
                let is_c = !is_f && Self::ratio_corner(&c);
                if !is_f && !is_c {
                    i += 1;
                    continue;
                }
                let total: usize = c.iter().sum();
                let m = total as f64 / 7.0;
                if (i > 0 && (run_len[i - 1] as f64) < 0.5 * m) || (i + 5 < n && (run_len[i + 5] as f64) < 0.5 * m) {
                    i += 1;
                    continue;
                }
                let cx0 = (run_start[i + 2] as f64 + c[2] as f64 / 2.0).floor() as i64;
                let max_count = if is_f { c[2] as f64 * 1.5 } else { total as f64 * 0.6 };
                if let Some((voff, vtot)) = self.cross_check(cx0, y as i64, 0, 1, max_count, total, is_c) {
                    let cy = (y as f64 - 1.0 + voff + 0.5).floor() as i64;
                    if cy >= 0 && (cy as usize) < h {
                        if let Some((hoff, htot)) = self.cross_check(cx0, cy, 1, 0, max_count, total, is_c) {
                            let cx = cx0 as f64 - 1.0 + hoff;
                            let module = (vtot + htot) as f64 / 14.0;
                            if module >= 2.0 {
                                add_candidate(cands, cx + 0.5, cy as f64 + 0.5, module, is_c);
                            }
                        }
                    }
                }
                i += 1;
            }
            y += step;
        }
    }

    fn select_sets(&self, cands: &[Cand]) -> Vec<CornerSet> {
        let by_strength = |a: &usize, b: &usize| {
            let (ca, cb) = (&cands[*a], &cands[*b]);
            cb.count.cmp(&ca.count).then(cb.m.partial_cmp(&ca.m).unwrap_or(std::cmp::Ordering::Equal))
        };
        let mut fall: Vec<usize> = (0..cands.len()).filter(|&i| !cands[i].corner).collect();
        fall.sort_by(by_strength);
        let mut cs: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].corner && cands[i].count >= 2).collect();
        cs.sort_by(by_strength);
        cs.truncate(4);
        let d = |p: usize, q: usize| (cands[p].x - cands[q].x).hypot(cands[p].y - cands[q].y);
        let cross = |o: usize, a: usize, b: usize| {
            (cands[a].x - cands[o].x) * (cands[b].y - cands[o].y) - (cands[a].y - cands[o].y) * (cands[b].x - cands[o].x)
        };
        let consider = |p: [usize; 3], with_corner: bool, out: &mut Vec<CornerSet>| {
            let ms = p.map(|i| cands[i].m);
            let mmax = ms.iter().cloned().fold(f64::MIN, f64::max);
            let mmin = ms.iter().cloned().fold(f64::MAX, f64::min);
            if mmax / mmin > 3.0 {
                return;
            }
            let sides = [d(p[1], p[2]), d(p[0], p[2]), d(p[0], p[1])];
            let mut rv = 0;
            if sides[1] > sides[rv] {
                rv = 1;
            }
            if sides[2] > sides[rv] {
                rv = 2;
            }
            let (v, a, b) = (p[rv], p[(rv + 1) % 3], p[(rv + 2) % 3]);
            let (la, lb) = (d(v, a), d(v, b));
            let cos = ((cands[a].x - cands[v].x) * (cands[b].x - cands[v].x) + (cands[a].y - cands[v].y) * (cands[b].y - cands[v].y)) / (la * lb);
            if cos.abs() > 0.65 || la < 18.0 * mmin || lb < 18.0 * mmin || la / lb > 2.6 || lb / la > 2.6 {
                return;
            }
            let mut c = [None; 4];
            if !with_corner {
                c[TL] = Some(v);
                let pos = cross(v, a, b) > 0.0;
                c[TR] = Some(if pos { a } else { b });
                c[BL] = Some(if pos { b } else { a });
            } else {
                let corner = *p.iter().find(|&&i| cands[i].corner).unwrap();
                if v == corner {
                    c[BR] = Some(v);
                    let neg = cross(v, a, b) < 0.0;
                    c[TR] = Some(if neg { a } else { b });
                    c[BL] = Some(if neg { b } else { a });
                } else {
                    let other = if a == corner { b } else { a };
                    c[BR] = Some(corner);
                    c[TL] = Some(other);
                    if cross(other, v, corner) > 0.0 {
                        c[TR] = Some(v);
                    } else {
                        c[BL] = Some(v);
                    }
                }
            }
            let score = p.iter().map(|&i| 4.0 * (1.0 + cands[i].count.min(15) as f64).log2()).sum::<f64>() - 8.0 * cos.abs() - 2.0 * (mmax / mmin - 1.0);
            let mut key = p;
            key.sort();
            out.push(CornerSet { c, score, key });
        };
        let build = |f: &[usize], with_corners: bool| {
            let mut out = Vec::new();
            for a in 0..f.len() {
                for b in a + 1..f.len() {
                    if !with_corners {
                        for c in b + 1..f.len() {
                            consider([f[a], f[b], f[c]], false, &mut out);
                        }
                    } else {
                        for &k in &cs {
                            consider([f[a], f[b], k], true, &mut out);
                        }
                    }
                }
            }
            out.sort_by(|x: &CornerSet, y: &CornerSet| y.score.partial_cmp(&x.score).unwrap_or(std::cmp::Ordering::Equal));
            out
        };
        let strong: Vec<usize> = fall.iter().copied().filter(|&i| cands[i].count >= 2).take(14).collect();
        let all: Vec<usize> = fall.iter().copied().take(16).collect();
        let mut res = Vec::new();
        let mut seen = HashSet::new();
        for (list, lim) in [(build(&strong, false), 8), (build(&strong, true), 4), (build(&all, false), 8), (build(&all, true), 3)] {
            for st in list.into_iter().take(lim) {
                if seen.insert(st.key) {
                    res.push(st);
                }
            }
        }
        res
    }

    fn measure_axis(&self, cx: f64, cy: f64, ux: f64, uy: f64, m0: f64) -> Option<(f64, f64)> {
        let step = 0.25;
        let ns = (5.0 * m0 / step).ceil() as i64;
        let n = (2 * ns + 1) as usize;
        let mut prof = vec![0f64; n];
        for (i, p) in prof.iter_mut().enumerate() {
            let t = (i as i64 - ns) as f64 * step;
            *p = self.luma_at(cx + ux * t, cy + uy * t);
        }
        let hh = 1usize;
        let mut der = vec![0f64; n];
        for i in hh..n.saturating_sub(hh) {
            der[i] = prof[i + hh] - prof[i - hh];
        }
        let (mut m, mut c0) = (m0, 0.0);
        for pass in 0..2 {
            let mut ks: Vec<f64> = Vec::new();
            let mut ts: Vec<f64> = Vec::new();
            for k in [-3.5f64, -2.5, -1.5, 1.5, 2.5, 3.5] {
                let outward_down = k.abs() != 2.5;
                let sign = if k > 0.0 { if outward_down { -1.0 } else { 1.0 } } else if outward_down { 1.0 } else { -1.0 };
                let tc = c0 + m * k;
                let win = if pass == 0 { 0.5 } else { 0.35 } * m;
                let i0 = ((hh + 1) as i64).max(((tc - win) / step + 0.5).floor() as i64 + ns);
                let i1 = (n as i64 - hh as i64 - 2).min(((tc + win) / step + 0.5).floor() as i64 + ns);
                let (mut bi, mut bv) = (-1i64, 0.0);
                let mut i = i0;
                while i <= i1 {
                    let v = der[i as usize] * sign;
                    if v > bv {
                        bv = v;
                        bi = i;
                    }
                    i += 1;
                }
                if bi < 0 {
                    continue;
                }
                let bi_u = bi as usize;
                let (a, b, c) = (der[bi_u - 1] * sign, der[bi_u] * sign, der[bi_u + 1] * sign);
                let den = a - 2.0 * b + c;
                let off = if den < 0.0 { 0.5 * (a - c) / den } else { 0.0 };
                ks.push(k);
                ts.push(((bi - ns) as f64 + off.clamp(-0.5, 0.5)) * step);
            }
            if ks.len() < 5 {
                return None;
            }
            let mut s = [0f64; 9];
            let mut bb = [0f64; 3];
            for i in 0..ks.len() {
                let k = ks[i];
                let e = k.signum() * if k.abs() == 2.5 { -1.0 } else { 1.0 };
                let t = ts[i];
                s[0] += 1.0;
                s[1] += k;
                s[2] += e;
                s[4] += k * k;
                s[5] += k * e;
                s[8] += e * e;
                bb[0] += t;
                bb[1] += k * t;
                bb[2] += e * t;
            }
            s[3] = s[1];
            s[6] = s[2];
            s[7] = s[5];
            let sol = solve3(s, bb)?;
            c0 = sol[0];
            m = sol[1];
            if !(m > 0.55 * m0 && m < 1.7 * m0) || c0.abs() > 1.5 * m0 {
                return None;
            }
        }
        Some((c0, m))
    }

    fn centroid(&self, x: f64, y: f64, r: f64, thr: f64) -> Option<V2> {
        let x0 = (x - r).floor().max(0.0) as usize;
        let x1 = ((x + r).ceil() as usize).min(self.w - 1);
        let y0 = (y - r).floor().max(0.0) as usize;
        let y1 = ((y + r).ceil() as usize).min(self.h - 1);
        let (mut sw, mut sx, mut sy) = (0.0, 0.0, 0.0);
        let r2 = r * r;
        for yy in y0..=y1 {
            let dy = yy as f64 + 0.5 - y;
            for xx in x0..=x1 {
                let dx = xx as f64 + 0.5 - x;
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                let wgt = self.luma[yy * self.w + xx] as f64 - thr;
                if wgt <= 0.0 {
                    continue;
                }
                sw += wgt;
                sx += wgt * (xx as f64 + 0.5);
                sy += wgt * (yy as f64 + 0.5);
            }
        }
        if sw <= 0.0 {
            None
        } else {
            Some([sx / sw, sy / sw])
        }
    }

    fn refine_finder(&self, f: &Cand, u: V2, v: V2) -> Option<Refined> {
        let (mut x, mut y, m) = (f.x, f.y, f.m);
        let white = self.luma_at(x, y);
        let ink = (self.luma_at(x + 2.0 * m * u[0], y + 2.0 * m * u[1])
            + self.luma_at(x - 2.0 * m * u[0], y - 2.0 * m * u[1])
            + self.luma_at(x + 2.0 * m * v[0], y + 2.0 * m * v[1])
            + self.luma_at(x - 2.0 * m * v[0], y - 2.0 * m * v[1]))
            / 4.0;
        if white - ink < 20.0 {
            return None;
        }
        if let Some(c) = self.centroid(x, y, 2.1 * m, (white + ink) / 2.0) {
            x = c[0];
            y = c[1];
        }
        let (cu, mu) = self.measure_axis(x, y, u[0], u[1], m)?;
        let (cv, mv) = self.measure_axis(x, y, v[0], v[1], m)?;
        Some(Refined { x: x + cu * u[0] + cv * v[0], y: y + cu * u[1] + cv * v[1], mu, mv, white, ink })
    }

    fn marker_score(&self, px: f64, py: f64, du: V2, dv: V2) -> f64 {
        let c = self.luma_at(px, py);
        let (mut ring, mut rmax) = (0.0, 0f64);
        for a in -1..=1 {
            for b in -1..=1 {
                if a == 0 && b == 0 {
                    continue;
                }
                let (a, b) = (a as f64, b as f64);
                let v = self.luma_at(px + a * du[0] + b * dv[0], py + a * du[1] + b * dv[1]);
                ring += v;
                rmax = rmax.max(v);
            }
        }
        c - (ring / 8.0 + rmax) / 2.0
    }

    fn corner_score(&self, px: f64, py: f64, du: V2, dv: V2) -> f64 {
        let c = self.luma_at(px, py);
        let (mut ink, mut outer) = (0.0, 0.0);
        for a in -1..=1 {
            for b in -1..=1 {
                if a == 0 && b == 0 {
                    continue;
                }
                let (a, b) = (a as f64, b as f64);
                ink += self.luma_at(px + 1.5 * (a * du[0] + b * dv[0]), py + 1.5 * (a * du[1] + b * dv[1]));
                outer += self.luma_at(px + 3.0 * (a * du[0] + b * dv[0]), py + 3.0 * (a * du[1] + b * dv[1]));
            }
        }
        (c + outer / 8.0) / 2.0 - ink / 8.0
    }

    fn search(&self, pred: V2, du: V2, dv: V2, radius: f64, score: Score) -> (f64, f64, f64) {
        let cell = du[0].hypot(du[1]).max(dv[0].hypot(dv[1]));
        let step = (cell / 4.0).min(radius / 6.0).max(0.5);
        let (mut best, mut bx, mut by) = (-1e9, pred[0], pred[1]);
        let mut oy = -radius;
        while oy <= radius {
            let mut ox = -radius;
            while ox <= radius {
                if ox * ox + oy * oy <= radius * radius {
                    let s = match score {
                        Score::Marker => self.marker_score(pred[0] + ox, pred[1] + oy, du, dv),
                        Score::Corner => self.corner_score(pred[0] + ox, pred[1] + oy, du, dv),
                    };
                    if s > best {
                        best = s;
                        bx = pred[0] + ox;
                        by = pred[1] + oy;
                    }
                }
                ox += step;
            }
            oy += step;
        }
        (bx, by, best)
    }

    fn refine_corner(&self, cc: &Cand, u: V2, v: V2) -> Option<Refined> {
        let m = cc.m;
        let du = [u[0] * m, u[1] * m];
        let dv = [v[0] * m, v[1] * m];
        let (rx, ry, _) = self.search([cc.x, cc.y], du, dv, 0.9 * m, Score::Corner);
        let (fx, fy, _) = self.search([rx, ry], du, dv, 0.3 * m, Score::Corner);
        let at = |a: f64, b: f64| self.luma_at(fx + a * du[0] + b * dv[0], fy + a * du[1] + b * dv[1]);
        let (mut white, mut ink) = (0.0, 0.0);
        for sg in [-1.0, 1.0] {
            white += at(3.0 * sg, 0.0) + at(0.0, 3.0 * sg);
            ink += at(1.5 * sg, 0.0) + at(0.0, 1.5 * sg);
        }
        white /= 4.0;
        ink /= 4.0;
        if white - ink < 20.0 {
            return None;
        }
        let mut min_light = self.luma_at(fx, fy);
        for (a, b) in [(3.0, 0.0), (-3.0, 0.0), (0.0, 3.0), (0.0, -3.0), (3.0, 1.5), (3.0, -1.5), (-3.0, 1.5), (-3.0, -1.5), (1.5, 3.0), (-1.5, 3.0), (1.5, -3.0), (-1.5, -3.0)] {
            min_light = min_light.min(at(a, b));
        }
        let mut max_dark = 0f64;
        for r in [1.1, 2.0] {
            for (a, b) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.75, 0.75), (-0.75, 0.75), (0.75, -0.75), (-0.75, -0.75)] {
                max_dark = max_dark.max(at(a * r, b * r));
            }
        }
        if min_light - max_dark < 0.12 * (white - ink) {
            return None;
        }
        let c = self.centroid(fx, fy, 1.3 * m, (self.luma_at(fx, fy) + ink) / 2.0);
        let (x, y) = c.map_or((fx, fy), |c| (c[0], c[1]));
        Some(Refined { x, y, mu: m, mv: m, white, ink })
    }

    /// Reads one header copy: Ok(header) when its CRC is correct, otherwise Err(soft bits of the
    /// best-aligned hypothesis, normalised and unmasked) for combining with other copies.
    #[allow(clippy::result_large_err)] // soft bits by value: no allocation per header attempt
    fn read_header(&self, f: &Refined, u: V2, v: V2, which: usize) -> Result<Header, [f64; HEADER_BITS]> {
        let thr = (f.white + f.ink) / 2.0;
        let mut offs = [(0f64, 0f64); HEADER_BITS];
        for r in 0..7 {
            for c in 0..10 {
                let (rf, cf) = (r as f64, c as f64);
                offs[r * 10 + c] = match which {
                    0 => (cf + 5.0, rf - 3.0),
                    1 => (rf - 3.0, cf + 5.0),
                    2 => (cf - 14.0, rf - 3.0),
                    _ => (rf - 3.0, cf - 14.0),
                };
            }
        }
        let horiz = which == 0 || which == 2;
        let mut vals = [0f64; HEADER_BITS];
        let mut raw = [0u8; HEADER_BITS];
        let mut best = [0f64; HEADER_BITS];
        let mut best_q = -1.0;
        for hyp in 0..17 {
            let ds = if hyp == 0 { 0.0 } else { (((hyp + 1) / 2) as f64) * 0.0125 * if hyp % 2 == 1 { 1.0 } else { -1.0 } };
            let su = if horiz { 1.0 + ds } else { 1.0 };
            let sv = if horiz { 1.0 } else { 1.0 + ds };
            let (ux, uy) = (u[0] * f.mu * su, u[1] * f.mu * su);
            let (vx, vy) = (v[0] * f.mv * sv, v[1] * f.mv * sv);
            for b in 0..HEADER_BITS {
                let (dx, dy) = offs[b];
                let px = f.x + dx * ux + dy * vx;
                let py = f.y + dx * uy + dy * vy;
                let mut acc = self.luma_at(px, py) * 2.0;
                acc += self.luma_at(px + 0.2 * ux, py + 0.2 * uy) + self.luma_at(px - 0.2 * ux, py - 0.2 * uy);
                acc += self.luma_at(px + 0.2 * vx, py + 0.2 * vy) + self.luma_at(px - 0.2 * vx, py - 0.2 * vy);
                vals[b] = acc / 6.0;
            }
            let (mut t, mut m0, mut m1) = (thr, thr, thr);
            for _ in 0..4 {
                let (mut s0, mut n0, mut s1, mut n1) = (0.0, 0, 0.0, 0);
                for &val in vals.iter() {
                    if val > t {
                        s1 += val;
                        n1 += 1;
                    } else {
                        s0 += val;
                        n0 += 1;
                    }
                }
                if n0 == 0 || n1 == 0 {
                    break;
                }
                m0 = s0 / n0 as f64;
                m1 = s1 / n1 as f64;
                t = (m0 + m1) / 2.0;
            }
            let both = [t, thr];
            let ts: &[f64] = if t == thr { &both[1..] } else { &both };
            for &tt in ts {
                for b in 0..HEADER_BITS {
                    raw[b] = (vals[b] > tt) as u8;
                }
                if let Some(hd) = unpack_header(&raw, which) {
                    return Ok(hd);
                }
            }
            let half = (m1 - m0).max(1.0) / 2.0;
            let q: f64 = vals.iter().map(|&x| ((x - t).abs() / half).min(1.0)).sum();
            if q > best_q {
                best_q = q;
                for b in 0..HEADER_BITS {
                    let s2 = ((vals[b] - t) / half).clamp(-1.0, 1.0);
                    best[b] = if header_mask(which, b) == 1 { -s2 } else { s2 };
                }
            }
        }
        Err(best)
    }

    /// Soft majority of header copies; copies that disagree with the rest are left out.
    fn combine_headers(softs: &[[f64; HEADER_BITS]]) -> Option<Header> {
        if softs.len() < 2 {
            return None;
        }
        let mut use_idx: Vec<usize> = (0..softs.len()).collect();
        if softs.len() >= 3 {
            use_idx.retain(|&i| {
                let agree = (0..HEADER_BITS)
                    .filter(|&b| {
                        let others: f64 = (0..softs.len()).filter(|&j| j != i).map(|j| softs[j][b]).sum();
                        (softs[i][b] > 0.0) == (others > 0.0)
                    })
                    .count();
                agree as f64 >= 0.75 * HEADER_BITS as f64
            });
            if use_idx.len() < 2 {
                return None;
            }
        }
        let mut raw = [0u8; HEADER_BITS];
        for b in 0..HEADER_BITS {
            let s: f64 = use_idx.iter().map(|&i| softs[i][b]).sum();
            raw[b] = ((s > 0.0) as u8) ^ header_mask(0, b);
        }
        unpack_header(&raw, 0)
    }

    /// Refines a corner set and reads its header. `Some((None, r))`: the geometry is usable but
    /// no header copy could be read (kept for header reuse).
    #[allow(clippy::type_complexity)]
    fn try_corners(&self, cands: &[Cand], set: &CornerSet, mirrored: bool) -> Option<(Option<Header>, [Option<Refined>; 4])> {
        let mut k = set.c;
        if mirrored {
            k.swap(TR, BL);
        }
        let pos = |i: Option<usize>| i.map(|i| [cands[i].x, cands[i].y]);
        let kp = [pos(k[0]), pos(k[1]), pos(k[2]), pos(k[3])];
        let rp = |r: &[Option<Refined>; 4]| r.map(|o| o.map(|f| [f.x, f.y]));
        let mut r: [Option<Refined>; 4] = [None; 4];
        let mut ax = axes(&kp);
        for key in [TL, TR, BL] {
            if let Some(ci) = k[key] {
                r[key] = self.refine_finder(&cands[ci], ax[key].0?, ax[key].1?);
            }
        }
        if let Some(ci) = k[BR] {
            r[BR] = self.refine_corner(&cands[ci], ax[BR].0?, ax[BR].1?);
        }
        if r.iter().filter(|o| o.is_some()).count() < 3 {
            return None;
        }
        ax = axes(&rp(&r));
        let mut hdr = None;
        let mut softs: Vec<[f64; HEADER_BITS]> = Vec::new();
        for (key, which) in [(TL, 0), (TL, 1), (TR, 2), (BL, 3)] {
            if let Some(f) = &r[key] {
                if let (Some(u), Some(v)) = ax[key] {
                    match self.read_header(f, u, v, which) {
                        Ok(h) => {
                            hdr = Some(h);
                            break;
                        }
                        Err(soft) => softs.push(soft),
                    }
                }
            }
        }
        let Some(hdr) = hdr.or_else(|| Self::combine_headers(&softs)) else { return Some((None, r)) };
        if !Self::fits(&hdr, &r) {
            return None;
        }
        Some((Some(hdr), r))
    }

    /// Sanity check: header dimensions against the measured corner spacing.
    fn fits(hdr: &Header, r: &[Option<Refined>; 4]) -> bool {
        let dist = |a: &Refined, b: &Refined| (a.x - b.x).hypot(a.y - b.y);
        let xest = match (&r[TL], &r[TR], &r[BL], &r[BR]) {
            (Some(a), Some(b), _, _) => dist(a, b) / ((a.mu + b.mu) / 2.0) + 7.0,
            (_, _, Some(a), Some(b)) => dist(a, b) / a.mu + 7.0,
            _ => hdr.x as f64,
        };
        let yest = match (&r[TL], &r[TR], &r[BL], &r[BR]) {
            (Some(a), _, Some(b), _) => dist(a, b) / ((a.mv + b.mv) / 2.0) + 7.0,
            (_, Some(a), _, Some(b)) => dist(a, b) / a.mv + 7.0,
            _ => hdr.y as f64,
        };
        (hdr.x as f64 - xest).abs() <= 0.2 * xest + 3.0 && (hdr.y as f64 - yest).abs() <= 0.2 * yest + 3.0
    }

    /// Returns the result and whether the last good header (`reuse`) was tried.
    fn run(&self, skip: Option<&SkipMap>, reuse: Option<Header>) -> (ScanResult, bool) {
        let Some(cands) = self.find_candidates() else { return (ScanResult::fail(RETHRESHOLD, 0), false) };
        if cands.len() < 3 {
            return (ScanResult::fail("finders", cands.len()), false);
        }
        let sets = self.select_sets(&cands);
        let mut headerless = Vec::new();
        for set in &sets {
            for mir in [false, true] {
                match self.try_corners(&cands, set, mir) {
                    Some((Some(hdr), r)) => {
                        let res = self.decode(hdr, r, skip, false);
                        if res.ok || res.reason != "lattice" {
                            return (res, false);
                        }
                    }
                    Some((None, r)) => headerless.push(r),
                    None => {}
                }
            }
        }
        // Only after the normal search failed: decode the located grid with the last good header.
        // A wrong reuse is caught by the size check, the marker lattice and the packet CRCs.
        if let Some(last) = reuse {
            if !headerless.is_empty() {
                let hdr = Header { seq: SEQ_UNKNOWN, ..last };
                for r in headerless.into_iter().take(REUSE_CANDIDATES) {
                    if !Self::fits(&hdr, &r) {
                        continue;
                    }
                    let res = self.decode(hdr, r, None, true);
                    if res.ok {
                        return (res, true);
                    }
                }
                return (ScanResult::fail("header", cands.len()), true);
            }
        }
        (ScanResult::fail(if sets.is_empty() { "geometry" } else { "header" }, cands.len()), false)
    }

    #[allow(clippy::needless_range_loop)]
    fn decode(&self, hdr: Header, r: [Option<Refined>; 4], skip: Option<&SkipMap>, reused: bool) -> ScanResult {
        let (xg, yg) = (hdr.x as usize, hdr.y as usize);
        let (mode, n, nsym) = (hdr.mode as usize, hdr.n as usize, hdr.nsym as usize);
        let cached = self.layout_cache.borrow().as_ref().filter(|l| l.x == xg && l.y == yg).cloned();
        let layout = match cached {
            Some(l) => l,
            None => {
                let Some(l) = Layout::new(xg, yg) else { return ScanResult::fail("layout", 0) };
                let l = std::sync::Arc::new(l);
                *self.layout_cache.borrow_mut() = Some(l.clone());
                l
            }
        };
        let layout = &*layout;
        let (lx, ly) = (&layout.lx, &layout.ly);
        let (nx, ny) = (lx.len(), ly.len());
        let avail: Vec<usize> = (0..4).filter(|&k| r[k].is_some()).collect();
        let contrast = avail.iter().map(|&k| r[k].unwrap().white - r[k].unwrap().ink).sum::<f64>() / avail.len() as f64;
        let rr = |k: usize| r[k].unwrap();

        let nl = nx * ny;
        let mut pp = vec![0f64; nl * 2];
        let mut state = vec![0u8; nl];
        let mut refs = vec![0f64; nl * 6];
        let mut ldu = vec![0f64; nl * 4];
        let gx_of = |k: usize| lx[k % nx] as f64 + 0.5;
        let gy_of = |k: usize| ly[k / nx] as f64 + 0.5;

        let pair_u = if r[TL].is_some() && r[TR].is_some() { (rr(TL), rr(TR)) } else { (rr(BL), rr(BR)) };
        let pair_v = if r[TL].is_some() && r[BL].is_some() { (rr(TL), rr(BL)) } else { (rr(TR), rr(BR)) };
        let du0 = [(pair_u.1.x - pair_u.0.x) / (xg as f64 - 7.0), (pair_u.1.y - pair_u.0.y) / (xg as f64 - 7.0)];
        let dv0 = [(pair_v.1.x - pair_v.0.x) / (yg as f64 - 7.0), (pair_v.1.y - pair_v.0.y) / (yg as f64 - 7.0)];
        let cell_px = (du0[0].hypot(du0[1]) + dv0[0].hypot(dv0[1])) / 2.0;
        let lu = du0[0].hypot(du0[1]);
        let lv = dv0[0].hypot(dv0[1]);

        // a reused header has no seq: such a frame cannot be told apart from others (no skipping)
        let frame_key = if reused { None } else { Some(FrameKey { seq: hdr.seq, x: hdr.x, y: hdr.y, mode: hdr.mode, n: hdr.n, nsym: hdr.nsym }) };
        let count = codewords_per_frame(layout, mode, n);
        let mut done = vec![false; count];
        let mut done_n = 0;
        if let Some(list) = frame_key.as_ref().and_then(|k| skip.and_then(|s| s.get(k))) {
            for &i in list {
                if i < count && !done[i] {
                    done[i] = true;
                    done_n += 1;
                }
            }
        }
        if done_n >= count {
            let (key, gx, gy) = if r[TL].is_some() { (TL, 3.5, 3.5) } else if r[TR].is_some() { (TR, xg as f64 - 3.5, 3.5) } else { (BL, 3.5, yg as f64 - 3.5) };
            let f = rr(key);
            let corners = [[0.0, 0.0], [xg as f64, 0.0], [xg as f64, yg as f64], [0.0, yg as f64]]
                .map(|[cx, cy]| [f.x + (cx - gx) * du0[0] + (cy - gy) * dv0[0], f.y + (cx - gx) * du0[1] + (cy - gy) * dv0[1]]);
            return ScanResult {
                ok: true,
                skipped: true,
                reason: "",
                candidates: 0,
                header: Some(hdr),
                frame_key,
                codewords: vec![],
                header_reused: reused,
                stats: Some(ScanStats { cell_px, corners, cw_count: count, ..Default::default() }),
            };
        }

        let sample6 = |x: f64, y: f64, du: V2, dv: V2, spread: f64| -> [f64; 3] {
            // the centre counts twice: sampled once and doubled (v + v == 2v exactly)
            let mut a = [0f64; 3];
            self.rgb_at(x, y, &mut a);
            a = [a[0] + a[0], a[1] + a[1], a[2] + a[2]];
            self.rgb_at(x + spread * du[0], y + spread * du[1], &mut a);
            self.rgb_at(x - spread * du[0], y - spread * du[1], &mut a);
            self.rgb_at(x + spread * dv[0], y + spread * dv[1], &mut a);
            self.rgb_at(x - spread * dv[0], y - spread * dv[1], &mut a);
            [a[0] / 6.0, a[1] / 6.0, a[2] / 6.0]
        };
        let ring_ink = |x: f64, y: f64, du: V2, dv: V2, rad: f64| -> [f64; 3] {
            let mut a = [0f64; 3];
            for i in -1..=1 {
                for j in -1..=1 {
                    if i == 0 && j == 0 {
                        continue;
                    }
                    let (i, j) = (i as f64, j as f64);
                    self.rgb_at(x + rad * (i * du[0] + j * dv[0]), y + rad * (i * du[1] + j * dv[1]), &mut a);
                }
            }
            [a[0] / 8.0, a[1] / 8.0, a[2] / 8.0]
        };
        let set_refs = |refs: &mut Vec<f64>, k: usize, white: [f64; 3], ink: [f64; 3]| {
            refs[k * 6..k * 6 + 3].copy_from_slice(&white);
            refs[k * 6 + 3..k * 6 + 6].copy_from_slice(&ink);
        };
        let corner_refs = |refs: &mut Vec<f64>, k: usize, px: f64, py: f64, du: V2, dv: V2| {
            let mut wacc = [0f64; 3];
            for s2 in [-3.0, 3.0] {
                self.rgb_at(px + s2 * du[0], py + s2 * du[1], &mut wacc);
                self.rgb_at(px + s2 * dv[0], py + s2 * dv[1], &mut wacc);
            }
            set_refs(refs, k, [wacc[0] / 4.0, wacc[1] / 4.0, wacc[2] / 4.0], ring_ink(px, py, du, dv, 1.5));
        };

        let kk = [0, nx - 1, (ny - 1) * nx, nl - 1]; // TL TR BL BR lattice indices
        let mut missing: Vec<usize> = Vec::new();
        for key in [TL, TR, BL] {
            let k = kk[key];
            let Some(f) = r[key] else {
                missing.push(k);
                continue;
            };
            pp[k * 2] = f.x;
            pp[k * 2 + 1] = f.y;
            state[k] = 1;
            let du = [du0[0] / lu * f.mu, du0[1] / lu * f.mu];
            let dv = [dv0[0] / lv * f.mv, dv0[1] / lv * f.mv];
            ldu[k * 4..k * 4 + 4].copy_from_slice(&[du[0], du[1], dv[0], dv[1]]);
            let white = sample6(f.x, f.y, du, dv, 0.35);
            set_refs(&mut refs, k, white, ring_ink(f.x, f.y, du, dv, 2.0));
        }
        if let Some(b) = r[BR] {
            let k = kk[BR];
            pp[k * 2] = b.x;
            pp[k * 2 + 1] = b.y;
            state[k] = 1;
            corner_refs(&mut refs, k, b.x, b.y, du0, dv0);
        }

        // seed homography: corners + virtual points along lines with known directions
        let mut seed_src: Vec<[f64; 2]> = Vec::new();
        let mut seed_dst: Vec<[f64; 2]> = Vec::new();
        let known_u = [r[TL].is_some() && r[TR].is_some(), r[TL].is_some() && r[TR].is_some(), r[BL].is_some() && r[BR].is_some()];
        let known_v = [r[TL].is_some() && r[BL].is_some(), r[TR].is_some() && r[BR].is_some(), r[TL].is_some() && r[BL].is_some()];
        for key in [TL, TR, BL] {
            let Some(f) = r[key] else { continue };
            let k = kk[key];
            let dd = [ldu[k * 4], ldu[k * 4 + 1], ldu[k * 4 + 2], ldu[k * 4 + 3]];
            let (gx, gy) = (gx_of(k), gy_of(k));
            let mut offs = vec![(0.0, 0.0)];
            if known_u[key] {
                offs.extend([(3.0, 0.0), (-3.0, 0.0)]);
            }
            if known_v[key] {
                offs.extend([(0.0, 3.0), (0.0, -3.0)]);
            }
            for (a, b) in offs {
                seed_src.push([gx + a, gy + b]);
                seed_dst.push([f.x + a * dd[0] + b * dd[2], f.y + a * dd[1] + b * dd[3]]);
            }
        }
        if let Some(b) = r[BR] {
            seed_src.push([gx_of(kk[BR]), gy_of(kk[BR])]);
            seed_dst.push([b.x, b.y]);
        }
        let Some(mut hcur) = fit_homography(&seed_src, &seed_dst, None) else { return ScanResult::fail("homography", 0) };
        let mut fit_count = avail.len();

        let mut found = 0usize;
        let mut br_found = r[BR].is_some();
        for &k in &missing {
            state[k] = 2;
        }
        loop {
            let mut pick = usize::MAX;
            let mut pscore = -1i64;
            for k in 0..nl {
                if state[k] != 0 {
                    continue;
                }
                let (i, j) = ((k % nx) as i64, (k / nx) as i64);
                let mut sc = 0i64;
                for jj in (j - 2).max(0)..=(j + 2).min(ny as i64 - 1) {
                    for ii in (i - 2).max(0)..=(i + 2).min(nx as i64 - 1) {
                        if state[jj as usize * nx + ii as usize] == 1 {
                            sc += if (jj - j).abs() <= 1 && (ii - i).abs() <= 1 { 5 } else { 1 };
                        }
                    }
                }
                if k == kk[BR] {
                    sc -= 3;
                }
                if sc > pscore {
                    pscore = sc;
                    pick = k;
                }
            }
            if pick == usize::MAX {
                break;
            }
            let k = pick;
            // predict: homography + residual interpolation
            let (i, j) = ((k % nx) as i64, (k / nx) as i64);
            let (gx, gy) = (gx_of(k), gy_of(k));
            let (bx, by) = apply_h(&hcur, gx, gy);
            let (mut sw, mut rx, mut ry) = (0.0, 0.0, 0.0);
            for jj in (j - 2).max(0)..=(j + 2).min(ny as i64 - 1) {
                for ii in (i - 2).max(0)..=(i + 2).min(nx as i64 - 1) {
                    let k2 = jj as usize * nx + ii as usize;
                    if state[k2] != 1 {
                        continue;
                    }
                    let (hx, hy) = apply_h(&hcur, gx_of(k2), gy_of(k2));
                    let wgt = 1.0 / (((ii - i) * (ii - i) + (jj - j) * (jj - j)) as f64);
                    sw += wgt;
                    rx += wgt * (pp[k2 * 2] - hx);
                    ry += wgt * (pp[k2 * 2 + 1] - hy);
                }
            }
            let pred = if sw > 0.0 { [bx + rx / sw, by + ry / sw] } else { [bx, by] };
            let (ax_, ay_) = apply_h(&hcur, gx + 1.0, gy);
            let (bx_, by_) = apply_h(&hcur, gx, gy + 1.0);
            let du = [ax_ - bx, ay_ - by];
            let dv = [bx_ - bx, by_ - by];
            let cell = (du[0].hypot(du[1]) + dv[0].hypot(dv[1])) / 2.0;
            let is_br = k == kk[BR];
            let sc = if is_br { Score::Corner } else { Score::Marker };
            let (sx, sy, sbest) = self.search(pred, du, dv, cell * if is_br { 1.6 } else { 0.95 }, sc);
            if sbest > 0.3 * contrast {
                let (fx, fy, fscore) = self.search([sx, sy], du, dv, cell * 0.3, sc);
                let thr = if is_br {
                    (self.luma_at(fx, fy) + self.luma_at(fx + 1.5 * du[0], fy + 1.5 * du[1])) / 2.0
                } else {
                    self.luma_at(fx, fy) - fscore / 2.0
                };
                let c = self.centroid(fx, fy, if is_br { 1.3 } else { 1.15 } * cell, thr);
                let (px, py) = c.map_or((fx, fy), |c| (c[0], c[1]));
                pp[k * 2] = px;
                pp[k * 2 + 1] = py;
                state[k] = 1;
                if is_br {
                    br_found = true;
                    corner_refs(&mut refs, k, px, py, du, dv);
                } else {
                    found += 1;
                    let white = sample6(px, py, du, dv, 0.2);
                    set_refs(&mut refs, k, white, ring_ink(px, py, du, dv, 1.0));
                }
                if found + avail.len() >= fit_count + 2.max(fit_count >> 2) {
                    let mut src = seed_src.clone();
                    let mut dst = seed_dst.clone();
                    let mut wts = vec![0.3; src.len()];
                    for k2 in 0..nl {
                        if state[k2] == 1 {
                            src.push([gx_of(k2), gy_of(k2)]);
                            dst.push([pp[k2 * 2], pp[k2 * 2 + 1]]);
                            wts.push(1.0);
                        }
                    }
                    if let Some(hn) = fit_homography(&src, &dst, Some(&wts)) {
                        hcur = hn;
                    }
                    fit_count = found + avail.len();
                }
            } else {
                pp[k * 2] = pred[0];
                pp[k * 2 + 1] = pred[1];
                state[k] = 2;
            }
        }
        // a false corner predicts the lattice in the wrong place: reject the geometry
        if nl >= 8 && (found as f64) < 0.6 * (nl as f64 - 4.0) {
            return ScanResult::fail("lattice", 0);
        }
        for &k in &missing {
            let (x, y) = apply_h(&hcur, gx_of(k), gy_of(k));
            pp[k * 2] = x;
            pp[k * 2 + 1] = y;
        }
        let gpt = |k: usize| [pp[k * 2], pp[k * 2 + 1]];
        let (xf, yf) = (xg as f64, yg as f64);
        let Some(hg) = homography(&[[3.5, 3.5], [xf - 3.5, 3.5], [xf - 3.5, yf - 3.5], [3.5, yf - 3.5]], &[gpt(kk[TL]), gpt(kk[TR]), gpt(kk[BR]), gpt(kk[BL])]) else {
            return ScanResult::fail("homography", 0);
        };
        // refs of unfound points from the nearest found ring
        for k in 0..nl {
            if state[k] == 1 {
                continue;
            }
            let (i, j) = ((k % nx) as i64, (k / nx) as i64);
            let mut acc = [0f64; 6];
            let mut cnt = 0;
            let mut rad = 1i64;
            while rad < nx.max(ny) as i64 && cnt == 0 {
                for jj in (j - rad).max(0)..=(j + rad).min(ny as i64 - 1) {
                    for ii in (i - rad).max(0)..=(i + rad).min(nx as i64 - 1) {
                        let k2 = jj as usize * nx + ii as usize;
                        if state[k2] != 1 {
                            continue;
                        }
                        for c in 0..6 {
                            acc[c] += refs[k2 * 6 + c];
                        }
                        cnt += 1;
                    }
                }
                rad += 1;
            }
            if cnt > 0 {
                for c in 0..6 {
                    refs[k * 6 + c] = acc[c] / cnt as f64;
                }
            }
        }

        // per-quad homographies and average cell vectors
        let qn = (nx - 1) * (ny - 1);
        let mut qh: Vec<H3> = Vec::with_capacity(qn);
        let mut qdu = vec![0f64; qn * 4];
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let k00 = j * nx + i;
                let src = [
                    [lx[i] as f64 + 0.5, ly[j] as f64 + 0.5],
                    [lx[i + 1] as f64 + 0.5, ly[j] as f64 + 0.5],
                    [lx[i + 1] as f64 + 0.5, ly[j + 1] as f64 + 0.5],
                    [lx[i] as f64 + 0.5, ly[j + 1] as f64 + 0.5],
                ];
                let dst = [gpt(k00), gpt(k00 + 1), gpt(k00 + nx + 1), gpt(k00 + nx)];
                qh.push(homography(&src, &dst).unwrap_or(hg));
                let (sx, sy) = ((lx[i + 1] - lx[i]) as f64, (ly[j + 1] - ly[j]) as f64);
                let q = (j * (nx - 1) + i) * 4;
                qdu[q] = (pp[(k00 + 1) * 2] - pp[k00 * 2] + pp[(k00 + nx + 1) * 2] - pp[(k00 + nx) * 2]) / (2.0 * sx);
                qdu[q + 1] = (pp[(k00 + 1) * 2 + 1] - pp[k00 * 2 + 1] + pp[(k00 + nx + 1) * 2 + 1] - pp[(k00 + nx) * 2 + 1]) / (2.0 * sx);
                qdu[q + 2] = (pp[(k00 + nx) * 2] - pp[k00 * 2] + pp[(k00 + nx + 1) * 2] - pp[(k00 + 1) * 2]) / (2.0 * sy);
                qdu[q + 3] = (pp[(k00 + nx) * 2 + 1] - pp[k00 * 2 + 1] + pp[(k00 + nx + 1) * 2 + 1] - pp[(k00 + 1) * 2 + 1]) / (2.0 * sy);
            }
        }
        let mut qi_of = vec![0usize; xg];
        let mut i = 0;
        for (x, q) in qi_of.iter_mut().enumerate() {
            while i < nx - 2 && x >= lx[i + 1] {
                i += 1;
            }
            *q = i;
        }
        let mut qj_of = vec![0usize; yg];
        let mut j = 0;
        for (y, q) in qj_of.iter_mut().enumerate() {
            while j < ny - 2 && y >= ly[j + 1] {
                j += 1;
            }
            *q = j;
        }

        // sample the slots of codewords still missing
        let dorder = &layout.data_order;
        let d = dorder.len();
        let cpc = cells_per_codeword(n, MODES[mode].bpc);
        let mut slots: Vec<usize> = Vec::new();
        for jw in 0..count {
            if !done[jw] {
                slots.extend(jw * cpc..(jw + 1) * cpc);
            }
        }
        let mut norm = vec![0f32; d * 3];
        for &s in &slots {
            let cidx = dorder[s] as usize;
            let (cx, cy) = (cidx % xg, cidx / xg);
            let (qi, qj) = (qi_of[cx], qj_of[cy]);
            let q = qj * (nx - 1) + qi;
            let (px, py) = apply_h(&qh[q], cx as f64 + 0.5, cy as f64 + 0.5);
            let du = [qdu[q * 4] * 0.18, qdu[q * 4 + 1] * 0.18];
            let dv = [qdu[q * 4 + 2] * 0.18, qdu[q * 4 + 3] * 0.18];
            let a = self.cell_rgb(px, py, du, dv);
            let fx = ((cx as f64 - lx[qi] as f64) / (lx[qi + 1] - lx[qi]) as f64).clamp(0.0, 1.0);
            let fy = ((cy as f64 - ly[qj] as f64) / (ly[qj + 1] - ly[qj]) as f64).clamp(0.0, 1.0);
            let k00 = qj * nx + qi;
            let (w00, w10, w01, w11) = ((1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy);
            for ch in 0..3 {
                let rf = |o: usize| refs[k00 * 6 + o] * w00 + refs[(k00 + 1) * 6 + o] * w10 + refs[(k00 + nx) * 6 + o] * w01 + refs[(k00 + nx + 1) * 6 + o] * w11;
                let (wr, kr) = (rf(ch), rf(3 + ch));
                norm[s * 3 + ch] = ((a[ch] / 6.0 - kr) / (wr - kr).max(12.0)) as f32;
            }
        }

        // classification: palette-seeded k-means + local gain correction
        let pal = MODES[mode].palette;
        let nc = pal.len();
        let mut cent = vec![0f32; nc * 3];
        for k in 0..nc {
            for ch in 0..3 {
                cent[k * 3 + ch] = (pal[k][ch] as f32 - INK[ch] as f32) / (PAPER[ch] as f32 - INK[ch] as f32);
            }
        }
        let mut symbols = vec![0u8; d];
        let mut conf = vec![0f32; d];
        let mut work = norm.clone();
        let assign = |update: bool, work: &[f32], cent: &mut [f32], symbols: &mut [u8], conf: &mut [f32]| {
            let mut sums = vec![0f64; nc * 4];
            for &s in &slots {
                let (rv, gv, bv) = (work[s * 3], work[s * 3 + 1], work[s * 3 + 2]);
                let (mut b1, mut b2, mut bi) = (f32::MAX, f32::MAX, 0usize);
                for k in 0..nc {
                    let (dr, dg, db) = (rv - cent[k * 3], gv - cent[k * 3 + 1], bv - cent[k * 3 + 2]);
                    let dd = dr * dr + dg * dg + db * db;
                    if dd < b1 {
                        b2 = b1;
                        b1 = dd;
                        bi = k;
                    } else if dd < b2 {
                        b2 = dd;
                    }
                }
                symbols[s] = bi as u8;
                if !update {
                    // only the final pass's confidences are used
                    conf[s] = b2.sqrt() - b1.sqrt();
                }
                sums[bi * 4] += rv as f64;
                sums[bi * 4 + 1] += gv as f64;
                sums[bi * 4 + 2] += bv as f64;
                sums[bi * 4 + 3] += 1.0;
            }
            if update {
                for k in 0..nc {
                    let cnt = sums[k * 4 + 3];
                    if cnt >= 8.0 {
                        for ch in 0..3 {
                            cent[k * 3 + ch] = (sums[k * 4 + ch] / cnt) as f32;
                        }
                    }
                }
            }
        };
        const GB: usize = 4;
        let (gbw, gbh) = (xg.div_ceil(GB), yg.div_ceil(GB));
        // where each slot sits among the gain blocks: its own block, and the four it blends
        // with their weights (the same for both passes, so worked out once)
        struct GainAt {
            b: usize,
            bi: [usize; 4],
            ax: f64,
            ay: f64,
        }
        let gain_at: Vec<GainAt> = slots
            .iter()
            .map(|&s| {
                let c = dorder[s] as usize;
                let b = (c / xg / GB) * gbw + (c % xg) / GB;
                let (cx, cy) = ((c % xg) as f64, (c / xg) as f64);
                let fx = ((cx + 0.5) / GB as f64 - 0.5).clamp(0.0, gbw as f64 - 1.0001);
                let fy = ((cy + 0.5) / GB as f64 - 0.5).clamp(0.0, gbh as f64 - 1.0001);
                let ix = (fx as usize).min(gbw.saturating_sub(2));
                let iy = (fy as usize).min(gbh.saturating_sub(2));
                let ax = if gbw > 1 { fx - ix as f64 } else { 0.0 };
                let ay = if gbh > 1 { fy - iy as f64 } else { 0.0 };
                let b00 = iy * gbw + ix;
                let b10 = if gbw > 1 { b00 + 1 } else { b00 };
                let b01 = if gbh > 1 { b00 + gbw } else { b00 };
                let b11 = if gbw > 1 { b01 + 1 } else { b01 };
                GainAt { b, bi: [b00, b10, b01, b11], ax, ay }
            })
            .collect();
        let gain_pass = |symbols: &[u8], cent: &[f32], work: &mut [f32]| {
            let mut gacc = vec![0f64; gbw * gbh * 6];
            for (&s, at) in slots.iter().zip(&gain_at) {
                let b = at.b;
                let k = symbols[s] as usize;
                for ch in 0..3 {
                    let cv = cent[k * 3 + ch] as f64;
                    gacc[b * 6 + ch] += norm[s * 3 + ch] as f64 * cv;
                    gacc[b * 6 + 3 + ch] += cv * cv;
                }
            }
            let lam = 0.6;
            let mut gain = vec![0f64; gbw * gbh * 3];
            for b in 0..gbw * gbh {
                for ch in 0..3 {
                    gain[b * 3 + ch] = (gacc[b * 6 + ch] + lam) / (gacc[b * 6 + 3 + ch] + lam);
                }
            }
            for (&s, at) in slots.iter().zip(&gain_at) {
                let [b00, b10, b01, b11] = at.bi;
                let (ax, ay) = (at.ax, at.ay);
                for ch in 0..3 {
                    // evaluated in the same order as the JavaScript scanner (bit-identical)
                    let g = gain[b00 * 3 + ch] * (1.0 - ax) * (1.0 - ay) + gain[b10 * 3 + ch] * ax * (1.0 - ay) + gain[b01 * 3 + ch] * (1.0 - ax) * ay + gain[b11 * 3 + ch] * ax * ay;
                    work[s * 3 + ch] = (norm[s * 3 + ch] as f64 / g.max(0.25)) as f32;
                }
            }
        };
        assign(true, &work, &mut cent, &mut symbols, &mut conf);
        gain_pass(&symbols, &cent, &mut work);
        assign(true, &work, &mut cent, &mut symbols, &mut conf);
        gain_pass(&symbols, &cent, &mut work);
        assign(false, &work, &mut cent, &mut symbols, &mut conf);

        let cws = decode_codewords(layout, mode, n, nsym, &symbols, &conf, count, &done);
        let corners = [[0.0, 0.0], [xf, 0.0], [xf, yf], [0.0, yf]].map(|[cx, cy]| {
            let (x, y) = apply_h(&hg, cx, cy);
            [x, y]
        });
        let cw_ok = cws.iter().filter(|c| c.ok).count();
        let corrected = cws.iter().filter(|c| c.ok).map(|c| c.corrected).sum();
        ScanResult {
            ok: true,
            skipped: false,
            reason: "",
            candidates: 0,
            header: Some(hdr),
            frame_key,
            codewords: cws,
            header_reused: reused,
            stats: Some(ScanStats {
                cell_px,
                corners,
                markers: found as f64 / (nl as f64 - 4.0).max(1.0),
                corner_found: br_found,
                cw_ok,
                cw_total: count - done_n,
                cw_count: count,
                corrected,
            }),
        }
    }
}

fn add_candidate(cands: &mut Vec<Cand>, x: f64, y: f64, m: f64, corner: bool) {
    for c in cands.iter_mut() {
        if c.corner == corner && (c.x - x).abs() <= c.m * 1.5 && (c.y - y).abs() <= c.m * 1.5 && (c.m - m).abs() <= 1f64.max(0.5 * c.m) {
            let n = c.count as f64;
            c.x = (c.x * n + x) / (n + 1.0);
            c.y = (c.y * n + y) / (n + 1.0);
            c.m = (c.m * n + m) / (n + 1.0);
            c.count += 1;
            return;
        }
    }
    cands.push(Cand { x, y, m, count: 1, corner });
}

type Axes = (Option<V2>, Option<V2>);

fn axes(p: &[Option<V2>; 4]) -> [Axes; 4] {
    let nrm = |a: Option<V2>, b: Option<V2>| -> Option<V2> {
        let (a, b) = (a?, b?);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy);
        Some([dx / l, dy / l])
    };
    let u_top = nrm(p[TL], p[TR]);
    let u_bot = nrm(p[BL], p[BR]);
    let v_left = nrm(p[TL], p[BL]);
    let v_right = nrm(p[TR], p[BR]);
    [(u_top.or(u_bot), v_left.or(v_right)), (u_top.or(u_bot), v_right.or(v_left)), (u_bot.or(u_top), v_left.or(v_right)), (u_bot.or(u_top), v_right.or(v_left))]
}
