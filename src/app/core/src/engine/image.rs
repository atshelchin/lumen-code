//! Camera frames in the platforms' native layouts, and the one pass that turns the region of
//! interest into the small RGBA image the scanner reads.
//!
//! The web demo has the GPU crop and scale the region around the code so that a cell is
//! about six pixels before a worker sees it; copying and reading back a whole 1080p frame
//! costs more than the scan. Here the same step reads the camera buffer in place (Y'CbCr
//! from AVFoundation or CameraX, BGRA, RGBA) and writes only the downscaled crop: no copy
//! of the full frame, no separate colour conversion.

/// Pixel layout of a camera frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba,
    Bgra,
    /// Y plane + chroma plane(s), 4:2:0. Covers NV12 (iOS 420f, CameraX with pixel stride 2)
    /// and I420 (pixel stride 1, separate U and V planes). Full-range BT.601.
    Yuv420,
    /// Packed 4:2:2, Y0 Cb Y1 Cr (webcams on macOS): plane 0 only.
    Yuyv,
}

#[derive(Clone, Copy, Debug)]
pub struct Plane<'a> {
    pub data: &'a [u8],
    pub row_stride: usize,
    pub pixel_stride: usize,
}

impl<'a> Plane<'a> {
    pub const EMPTY: Plane<'static> = Plane { data: &[], row_stride: 0, pixel_stride: 0 };
}

/// One camera frame, borrowed from the platform for the duration of a call.
#[derive(Clone, Copy, Debug)]
pub struct FrameRef<'a> {
    pub width: usize,
    pub height: usize,
    pub format: PixelFormat,
    /// RGBA/BGRA: plane 0 only. Yuv420: Y, Cb (U), Cr (V).
    pub planes: [Plane<'a>; 3],
    /// Clockwise rotation that shows the frame upright on screen (0, 90, 180, 270).
    pub rotation: u32,
}

impl<'a> FrameRef<'a> {
    /// A tightly packed RGBA image (stills, desktop webcams, tests).
    pub fn rgba(width: usize, height: usize, data: &'a [u8]) -> FrameRef<'a> {
        FrameRef {
            width,
            height,
            format: PixelFormat::Rgba,
            planes: [Plane { data, row_stride: width * 4, pixel_stride: 4 }, Plane::EMPTY, Plane::EMPTY],
            rotation: 0,
        }
    }

    /// Whether the planes are large enough for the stated size and strides.
    pub fn is_valid(&self) -> bool {
        let (w, h) = (self.width, self.height);
        if w < 16 || h < 16 {
            return false;
        }
        let fits = |p: &Plane, pw: usize, ph: usize, bpp: usize| {
            p.row_stride >= pw * bpp.max(1) && p.data.len() >= (ph - 1) * p.row_stride + pw * bpp.max(1)
        };
        match self.format {
            PixelFormat::Rgba | PixelFormat::Bgra => fits(&self.planes[0], w, h, 4),
            PixelFormat::Yuyv => fits(&self.planes[0], w.div_ceil(2) * 2, h, 2),
            PixelFormat::Yuv420 => {
                let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
                let ps = self.planes[1].pixel_stride.max(1);
                fits(&self.planes[0], w, h, 1)
                    && self.planes[1].data.len() >= (ch - 1) * self.planes[1].row_stride + (cw - 1) * ps + 1
                    && self.planes[2].data.len() >= (ch - 1) * self.planes[2].row_stride + (cw - 1) * self.planes[2].pixel_stride.max(1) + 1
            }
        }
    }
}

/// The part of the frame to scan and its scale (destination pixels per source pixel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crop {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub k: f64,
}

impl Crop {
    pub fn full(width: usize, height: usize) -> Crop {
        Crop { x: 0, y: 0, w: width, h: height, k: 1.0 }
    }

    pub fn out_size(&self) -> (usize, usize) {
        (((self.w as f64 * self.k).round() as usize).max(32), ((self.h as f64 * self.k).round() as usize).max(32))
    }
}

#[inline]
fn yuv_to_rgb(y: i32, u: i32, v: i32) -> [u8; 3] {
    // full-range BT.601 (JFIF), 8.8 fixed point
    let (u, v) = (u - 128, v - 128);
    let r = y + ((359 * v) >> 8);
    let g = y - ((88 * u + 183 * v) >> 8);
    let b = y + ((454 * u) >> 8);
    [r.clamp(0, 255) as u8, g.clamp(0, 255) as u8, b.clamp(0, 255) as u8]
}

/// Source span [a, b) of each destination index along one axis.
fn spans(origin: usize, len: usize, out: usize, limit: usize) -> Vec<(usize, usize)> {
    let step = len as f64 / out as f64;
    (0..out)
        .map(|i| {
            let a = origin + (i as f64 * step).floor() as usize;
            let b = (origin + ((i + 1) as f64 * step).floor() as usize).max(a + 1).min(limit);
            (a.min(limit - 1), b)
        })
        .collect()
}

/// Writes the crop, scaled by box averaging, as RGBA into `out` (resized as needed).
/// Returns the output size.
pub fn prepare(frame: &FrameRef, crop: &Crop, out: &mut Vec<u8>) -> (usize, usize) {
    let (dw, dh) = crop.out_size();
    let need = dw * dh * 4;
    if out.len() < need {
        out.resize(need, 0);
    }
    if frame.format == PixelFormat::Yuyv {
        // the scanner takes Y'CbCr directly (prepare_yuv); RGBA only for completeness
        let (mut y, mut c) = (Vec::new(), Vec::new());
        prepare_yuv(frame, crop, &mut y, &mut c);
        let cw = dw.div_ceil(2);
        for yy in 0..dh {
            for xx in 0..dw {
                let k = ((yy / 2) * cw + xx / 2) * 2;
                let rgb = yuv_to_rgb(y[yy * dw + xx] as i32, c[k] as i32, c[k + 1] as i32);
                out[(yy * dw + xx) * 4..(yy * dw + xx) * 4 + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        return (dw, dh);
    }
    if dw == crop.w && dh == crop.h && crop.x + dw <= frame.width && crop.y + dh <= frame.height {
        copy_1to1(frame, crop, out);
    } else {
        scale_box(frame, crop, dw, dh, out);
    }
    (dw, dh)
}

/// 16.16 reciprocals of box sizes, so averaging needs no division per pixel.
fn recip(n: usize) -> u32 {
    (65536 + n as u32 / 2) / n.max(1) as u32
}

/// Box-filtered downscale, separable: the source rows of each output row are summed into an
/// accumulator (contiguous and vectorisable), then each output pixel adds a 1–3 wide span of
/// it and scales by a reciprocal.
fn scale_box(frame: &FrameRef, crop: &Crop, dw: usize, dh: usize, out: &mut [u8]) {
    let xs = spans(crop.x, crop.w, dw, frame.width);
    let ys = spans(crop.y, crop.h, dh, frame.height);
    let x0 = xs[0].0;
    let x1 = xs[dw - 1].1;
    let p0 = &frame.planes[0];
    // span widths and their offsets into the accumulator, once per call
    let xoff: Vec<(usize, usize)> = xs.iter().map(|&(a, b)| (a - x0, b - a)).collect();
    let mut inv = [0u32; 65];
    for (n, v) in inv.iter_mut().enumerate() {
        *v = recip(n);
    }
    let inv_of = |n: usize| if n < inv.len() { inv[n] } else { recip(n) };
    match frame.format {
        // packed 4:2:2 goes through prepare_yuv (see prepare)
        PixelFormat::Yuyv => {}
        PixelFormat::Rgba | PixelFormat::Bgra => {
            let (ri, bi) = if frame.format == PixelFormat::Rgba { (0, 2) } else { (2, 0) };
            let wsrc = x1 - x0;
            let mut acc = vec![0u32; wsrc * 3];
            for (oy, &(y0, y1)) in ys.iter().enumerate() {
                acc.iter_mut().for_each(|a| *a = 0);
                for y in y0..y1 {
                    let row = &p0.data[y * p0.row_stride + x0 * 4..][..wsrc * 4];
                    for (a, px) in acc.chunks_exact_mut(3).zip(row.chunks_exact(4)) {
                        a[0] += px[ri] as u32;
                        a[1] += px[1] as u32;
                        a[2] += px[bi] as u32;
                    }
                }
                let rows = y1 - y0;
                let dst = &mut out[oy * dw * 4..(oy + 1) * dw * 4];
                for (o, &(off, n)) in dst.chunks_exact_mut(4).zip(&xoff) {
                    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                    for a in acc[off * 3..(off + n) * 3].chunks_exact(3) {
                        r += a[0];
                        g += a[1];
                        b += a[2];
                    }
                    let k = inv_of(n * rows);
                    o[0] = ((r * k) >> 16) as u8;
                    o[1] = ((g * k) >> 16) as u8;
                    o[2] = ((b * k) >> 16) as u8;
                    o[3] = 255;
                }
            }
        }
        PixelFormat::Yuv420 => {
            let (pu, pv) = (&frame.planes[1], &frame.planes[2]);
            let (psu, psv) = (pu.pixel_stride.max(1), pv.pixel_stride.max(1));
            // chroma sampled at the centre of each box (it is half resolution anyway)
            let cu: Vec<usize> = xs.iter().map(|&(a, b)| ((a + b - 1) / 2 >> 1) * psu).collect();
            let cv: Vec<usize> = xs.iter().map(|&(a, b)| ((a + b - 1) / 2 >> 1) * psv).collect();
            let wsrc = x1 - x0;
            let mut acc = vec![0u32; wsrc];
            for (oy, &(y0, y1)) in ys.iter().enumerate() {
                acc.iter_mut().for_each(|a| *a = 0);
                for y in y0..y1 {
                    let row = &p0.data[y * p0.row_stride + x0..][..wsrc];
                    for (a, &l) in acc.iter_mut().zip(row) {
                        *a += l as u32;
                    }
                }
                let rows = y1 - y0;
                let cy = ((y0 + y1 - 1) / 2) >> 1;
                let urow = &pu.data[cy * pu.row_stride..];
                let vrow = &pv.data[cy * pv.row_stride..];
                let dst = &mut out[oy * dw * 4..(oy + 1) * dw * 4];
                for (ox, (o, &(off, n))) in dst.chunks_exact_mut(4).zip(&xoff).enumerate() {
                    let sum: u32 = acc[off..off + n].iter().sum();
                    let l = ((sum * inv_of(n * rows)) >> 16) as i32;
                    let rgb = yuv_to_rgb(l, urow[cu[ox]] as i32, vrow[cv[ox]] as i32);
                    o[0] = rgb[0];
                    o[1] = rgb[1];
                    o[2] = rgb[2];
                    o[3] = 255;
                }
            }
        }
    }
}

/// For 4:2:0 frames: the crop scaled into a luma plane (dw × dh, box filtered) and interleaved
/// chroma at half that resolution (sampled at each 2×2 block's centre). No colour conversion
/// and a third of the bytes of RGBA; the scanner converts colour only where it samples cells.
/// Returns `None` for frames that are not Y'CbCr.
pub fn prepare_yuv(frame: &FrameRef, crop: &Crop, y_out: &mut Vec<u8>, cbcr_out: &mut Vec<u8>) -> Option<(usize, usize)> {
    // luma plane and its pixel stride, chroma planes, chroma rows per luma row (4:2:0 halves)
    let (p0, ps0, pu, pv, vshift) = match frame.format {
        PixelFormat::Yuv420 => (frame.planes[0], 1usize, frame.planes[1], frame.planes[2], 1usize),
        PixelFormat::Yuyv => {
            let d = frame.planes[0];
            if d.data.len() < 4 {
                return None;
            }
            let at = |o: usize, ps: usize| Plane { data: &d.data[o..], row_stride: d.row_stride, pixel_stride: ps };
            (at(0, 2), 2usize, at(1, 4), at(3, 4), 0usize)
        }
        _ => return None,
    };
    let (dw, dh) = crop.out_size();
    let (cw, ch) = (dw.div_ceil(2), dh.div_ceil(2));
    if y_out.len() < dw * dh {
        y_out.resize(dw * dh, 0);
    }
    if cbcr_out.len() < cw * ch * 2 {
        cbcr_out.resize(cw * ch * 2, 0);
    }
    let xs = spans(crop.x, crop.w, dw, frame.width);
    let ys = spans(crop.y, crop.h, dh, frame.height);
    let p0 = &p0;
    let x0 = xs[0].0;
    let wsrc = xs[dw - 1].1 - x0;
    if dw == crop.w && dh == crop.h {
        for (oy, &(y, _)) in ys.iter().enumerate() {
            let src = &p0.data[y * p0.row_stride + x0 * ps0..];
            let dst = &mut y_out[oy * dw..(oy + 1) * dw];
            if ps0 == 1 {
                dst.copy_from_slice(&src[..dw]);
            } else {
                for (o, px) in dst.iter_mut().zip(src.chunks_exact(ps0)) {
                    *o = px[0];
                }
            }
        }
    } else {
        let xoff: Vec<(usize, usize)> = xs.iter().map(|&(a, b)| (a - x0, b - a)).collect();
        let mut inv = [0u32; 65];
        for (n, v) in inv.iter_mut().enumerate() {
            *v = recip(n);
        }
        let mut acc = vec![0u32; wsrc];
        for (oy, &(y0, y1)) in ys.iter().enumerate() {
            acc.iter_mut().for_each(|a| *a = 0);
            for y in y0..y1 {
                let row = &p0.data[y * p0.row_stride + x0 * ps0..];
                if ps0 == 1 {
                    for (a, &l) in acc.iter_mut().zip(&row[..wsrc]) {
                        *a += l as u32;
                    }
                } else {
                    for (a, px) in acc.iter_mut().zip(row.chunks_exact(ps0)) {
                        *a += px[0] as u32;
                    }
                }
            }
            let rows = y1 - y0;
            for (o, &(off, n)) in y_out[oy * dw..(oy + 1) * dw].iter_mut().zip(&xoff) {
                let sum: u32 = acc[off..off + n].iter().sum();
                let k = if n * rows < inv.len() { inv[n * rows] } else { recip(n * rows) };
                *o = ((sum * k) >> 16) as u8;
            }
        }
    }
    // chroma: one sample per 2×2 output block, from the source position at its centre
    let (psu, psv) = (pu.pixel_stride.max(1), pv.pixel_stride.max(1));
    let centre = |sp: &[(usize, usize)], i: usize, shift: usize| {
        let (a, _) = sp[2 * i];
        let (_, b) = sp[(2 * i + 1).min(sp.len() - 1)];
        ((a + b - 1) / 2) >> shift
    };
    let cxs: Vec<usize> = (0..cw).map(|i| centre(&xs, i, 1)).collect();
    for j in 0..ch {
        let cy = centre(&ys, j, vshift);
        let urow = &pu.data[cy * pu.row_stride..];
        let vrow = &pv.data[cy * pv.row_stride..];
        let out = &mut cbcr_out[j * cw * 2..(j + 1) * cw * 2];
        for (o, &cx) in out.chunks_exact_mut(2).zip(&cxs) {
            o[0] = urow[cx * psu];
            o[1] = vrow[cx * psv];
        }
    }
    Some((dw, dh))
}

/// The crop at its own resolution (the full-frame search): one conversion per pixel.
fn copy_1to1(frame: &FrameRef, crop: &Crop, out: &mut [u8]) {
    let (w, h) = (crop.w, crop.h);
    let p0 = &frame.planes[0];
    match frame.format {
        // packed 4:2:2 goes through prepare_yuv (see prepare)
        PixelFormat::Yuyv => {}
        PixelFormat::Rgba => {
            for y in 0..h {
                let src = &p0.data[(crop.y + y) * p0.row_stride + crop.x * 4..][..w * 4];
                out[y * w * 4..(y + 1) * w * 4].copy_from_slice(src);
            }
        }
        PixelFormat::Bgra => {
            for y in 0..h {
                let src = &p0.data[(crop.y + y) * p0.row_stride + crop.x * 4..][..w * 4];
                let dst = &mut out[y * w * 4..(y + 1) * w * 4];
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = 255;
                }
            }
        }
        PixelFormat::Yuv420 => {
            let (pu, pv) = (&frame.planes[1], &frame.planes[2]);
            let (psu, psv) = (pu.pixel_stride.max(1), pv.pixel_stride.max(1));
            for y in 0..h {
                let sy = crop.y + y;
                let yrow = &p0.data[sy * p0.row_stride + crop.x..][..w];
                let urow = &pu.data[(sy >> 1) * pu.row_stride..];
                let vrow = &pv.data[(sy >> 1) * pv.row_stride..];
                let dst = &mut out[y * w * 4..(y + 1) * w * 4];
                for (x, (d, &l)) in dst.chunks_exact_mut(4).zip(yrow).enumerate() {
                    let cx = (crop.x + x) >> 1;
                    let rgb = yuv_to_rgb(l as i32, urow[cx * psu] as i32, vrow[cx * psv] as i32);
                    d[0] = rgb[0];
                    d[1] = rgb[1];
                    d[2] = rgb[2];
                    d[3] = 255;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_to_one_matches_the_general_path() {
        let (w, h) = (48usize, 40usize);
        let y: Vec<u8> = (0..w * h).map(|i| (i * 7 % 251) as u8).collect();
        let uv: Vec<u8> = (0..w * h / 2).map(|i| (i * 13 % 241) as u8).collect();
        let f = FrameRef {
            width: w,
            height: h,
            format: PixelFormat::Yuv420,
            planes: [
                Plane { data: &y, row_stride: w, pixel_stride: 1 },
                Plane { data: &uv, row_stride: w, pixel_stride: 2 },
                Plane { data: &uv[1..], row_stride: w, pixel_stride: 2 },
            ],
            rotation: 0,
        };
        let crop = Crop { x: 2, y: 3, w: 34, h: 34, k: 1.0 };
        let (mut a, mut b) = (vec![0u8; 34 * 34 * 4], vec![0u8; 34 * 34 * 4]);
        copy_1to1(&f, &crop, &mut a);
        scale_box(&f, &crop, 34, 34, &mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn yuyv_matches_nv12() {
        // the same picture as packed 4:2:2 and as 4:2:0 gives the same luma and chroma
        let (w, h) = (64usize, 48usize);
        let luma = |x: usize, y: usize| ((x * 3 + y * 5) % 251) as u8;
        let cb = |x: usize, y: usize| (100 + (x / 2) % 50 + (y / 2) % 7) as u8;
        let cr = |x: usize, y: usize| (140 - (x / 2) % 40 + (y / 2) % 5) as u8;
        let mut yuyv = vec![0u8; w * h * 2];
        let (mut yp, mut uv) = (vec![0u8; w * h], vec![0u8; w * h / 2]);
        for y in 0..h {
            for x in 0..w {
                yuyv[y * w * 2 + x * 2] = luma(x, y);
                yp[y * w + x] = luma(x, y);
                if x % 2 == 0 {
                    yuyv[y * w * 2 + x * 2 + 1] = cb(x, y);
                    yuyv[y * w * 2 + x * 2 + 3] = cr(x, y);
                    if y % 2 == 0 {
                        uv[(y / 2) * w + x] = cb(x, y);
                        uv[(y / 2) * w + x + 1] = cr(x, y);
                    }
                }
            }
        }
        let fy = FrameRef { width: w, height: h, format: PixelFormat::Yuyv, planes: [Plane { data: &yuyv, row_stride: w * 2, pixel_stride: 2 }, Plane::EMPTY, Plane::EMPTY], rotation: 0 };
        let fn12 = FrameRef {
            width: w,
            height: h,
            format: PixelFormat::Yuv420,
            planes: [
                Plane { data: &yp, row_stride: w, pixel_stride: 1 },
                Plane { data: &uv, row_stride: w, pixel_stride: 2 },
                Plane { data: &uv[1..], row_stride: w, pixel_stride: 2 },
            ],
            rotation: 0,
        };
        assert!(fy.is_valid());
        for crop in [Crop::full(w, h), Crop { x: 4, y: 2, w: 50, h: 40, k: 0.7 }] {
            let (mut a, mut ac, mut b, mut bc) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            let sa = prepare_yuv(&fy, &crop, &mut a, &mut ac).unwrap();
            let sb = prepare_yuv(&fn12, &crop, &mut b, &mut bc).unwrap();
            assert_eq!(sa, sb);
            assert_eq!(a[..sa.0 * sa.1], b[..sb.0 * sb.1]);
            let n = sa.0.div_ceil(2) * sa.1.div_ceil(2) * 2;
            // chroma constant over each 2×2 block here, so both layouts agree
            assert_eq!(ac[..n], bc[..n]);
        }
    }

    #[test]
    fn nv12_and_bgra_match_rgba() {
        // a 64×48 RGBA gradient, its BGRA twin and an NV12 rendition of flat colour blocks
        let (w, h) = (64usize, 48usize);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[(x * 4) as u8, (y * 5) as u8, 200, 255]);
            }
        }
        let bgra: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
        let crop = Crop { x: 8, y: 4, w: 40, h: 30, k: 0.5 };
        let (mut a, mut b) = (Vec::new(), Vec::new());
        let sa = prepare(&FrameRef::rgba(w, h, &rgba), &crop, &mut a);
        let fb = FrameRef { format: PixelFormat::Bgra, ..FrameRef::rgba(w, h, &bgra) };
        let sb = prepare(&fb, &crop, &mut b);
        assert_eq!(sa, (32, 32));
        assert_eq!(sa, sb);
        assert_eq!(a[..sa.0 * sa.1 * 4], b[..sb.0 * sb.1 * 4]);

        // grey NV12: Y = 128, U = V = 128 -> RGB 128
        let y = vec![128u8; w * h];
        let uv = vec![128u8; w * h / 2];
        let f = FrameRef {
            width: w,
            height: h,
            format: PixelFormat::Yuv420,
            planes: [
                Plane { data: &y, row_stride: w, pixel_stride: 1 },
                Plane { data: &uv, row_stride: w, pixel_stride: 2 },
                Plane { data: &uv[1..], row_stride: w, pixel_stride: 2 },
            ],
            rotation: 90,
        };
        assert!(f.is_valid());
        let mut c = Vec::new();
        let (cw, ch) = prepare(&f, &Crop::full(w, h), &mut c);
        assert_eq!((cw, ch), (w, h));
        assert!(c[..cw * ch * 4].chunks(4).all(|p| p == [128, 128, 128, 255]));
    }
}
