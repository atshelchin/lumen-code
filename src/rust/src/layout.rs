//! Grid layout (PROTOCOL §4–§6): finders, corner pattern, separators, header copies,
//! alignment lattice, Hilbert data order and the whitening sequence.

use crate::util::{crc16_bits, mix32};

pub const VERSION: u32 = 1;
pub const MIN_DIM: usize = 40;
pub const MAX_DIM: usize = 511;
pub const HEADER_BITS: usize = 70;

pub const K_DATA: u8 = 0;
pub const K_FIXED: u8 = 1;
pub const K_MARKER: u8 = 2;
pub const K_HEADER: u8 = 3;

pub const INK: [u8; 3] = [10, 13, 20];
pub const PAPER: [u8; 3] = [242, 244, 248];

/// Colour modes: bits per cell and nominal palette (sRGB).
pub struct Mode {
    pub bpc: u32,
    pub palette: &'static [[u8; 3]],
}

pub const MODES: [Mode; 3] = [
    Mode { bpc: 1, palette: &[INK, PAPER] },
    Mode { bpc: 2, palette: &[INK, [0, 216, 255], [255, 40, 150], [255, 214, 0]] },
    Mode {
        bpc: 3,
        palette: &[INK, [44, 80, 255], [26, 222, 96], [0, 220, 255], [255, 52, 76], [250, 52, 220], [255, 214, 10], PAPER],
    },
];

/// Parity share per ECC level, in percent (encoder choice, informative).
pub const ECC_PERCENT: [u32; 4] = [10, 16, 22, 30];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub mode: u32,
    pub x: u32,
    pub y: u32,
    pub n: u32,
    pub nsym: u32,
    pub seq: u32,
}

pub struct Layout {
    pub x: usize,
    pub y: usize,
    pub kind: Vec<u8>,
    /// 0 = ink, 1 = paper for non-data cells
    pub fixed: Vec<u8>,
    /// four header copies, cell indices in bit order
    pub headers: [Vec<u32>; 4],
    pub lx: Vec<usize>,
    pub ly: Vec<usize>,
    pub data_order: Vec<u32>,
    pub whiten: Vec<u8>,
}

/// Marker lattice positions along one axis of `d` cells.
pub fn lattice_axis(d: usize) -> Vec<usize> {
    let span = d - 7;
    let mut segs = ((span + 9) / 18).max(1);
    while segs > 1 && span < 16 * segs {
        segs -= 1;
    }
    (0..=segs).map(|i| 3 + (2 * i * span + segs) / (2 * segs)).collect()
}

/// Hilbert curve: index -> (x, y) on an order×order grid (order a power of two).
pub fn hilbert_d2xy(order: usize, d: usize) -> (usize, usize) {
    let (mut x, mut y, mut t) = (0usize, 0usize, d);
    let mut s = 1;
    while s < order {
        let rx = 1 & (t >> 1);
        let ry = 1 & (t ^ rx);
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x += s * rx;
        y += s * ry;
        t >>= 2;
        s *= 2;
    }
    (x, y)
}

impl Layout {
    pub fn new(x: usize, y: usize) -> Option<Layout> {
        if !(MIN_DIM..=MAX_DIM).contains(&x) || !(MIN_DIM..=MAX_DIM).contains(&y) {
            return None;
        }
        let n = x * y;
        let mut kind = vec![K_DATA; n];
        let mut fixed = vec![0u8; n];
        let set = |cx: usize, cy: usize, k: u8, light: bool, kind: &mut Vec<u8>, fixed: &mut Vec<u8>| {
            kind[cy * x + cx] = k;
            fixed[cy * x + cx] = light as u8;
        };
        let cheb = |a: usize, b: usize| (a as i32 - 3).abs().max((b as i32 - 3).abs());
        for (ox, oy, corner) in [(0, 0, false), (x - 7, 0, false), (0, y - 7, false), (x - 7, y - 7, true)] {
            for j in 0..7 {
                for i in 0..7 {
                    let dd = cheb(i, j);
                    let light = if corner { dd == 3 || dd == 0 } else { dd == 3 || dd <= 1 };
                    set(ox + i, oy + j, K_FIXED, light, &mut kind, &mut fixed);
                }
            }
        }
        for i in 0..=7 {
            for (cx, cy) in [(7, i), (i, 7), (x - 8, i), (x - 1 - i, 7), (7, y - 1 - i), (i, y - 8), (x - 8, y - 1 - i), (x - 1 - i, y - 8)] {
                set(cx, cy, K_FIXED, false, &mut kind, &mut fixed);
            }
        }
        let mut headers: [Vec<u32>; 4] = Default::default();
        for r in 0..7 {
            for c in 0..10 {
                headers[0].push((r * x + 8 + c) as u32);
                headers[1].push(((8 + c) * x + r) as u32);
                headers[2].push((r * x + (x - 18 + c)) as u32);
                headers[3].push(((y - 18 + c) * x + r) as u32);
            }
        }
        for h in &headers {
            for &i in h {
                kind[i as usize] = K_HEADER;
            }
        }
        let lx = lattice_axis(x);
        let ly = lattice_axis(y);
        let (nx, ny) = (lx.len(), ly.len());
        for j in 0..ny {
            for i in 0..nx {
                let corner = (i == 0 || i == nx - 1) && (j == 0 || j == ny - 1);
                if corner {
                    continue;
                }
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let cx = (lx[i] as i32 + dx) as usize;
                        let cy = (ly[j] as i32 + dy) as usize;
                        set(cx, cy, K_MARKER, dx == 0 && dy == 0, &mut kind, &mut fixed);
                    }
                }
            }
        }
        let mut order = 1;
        while order < x.max(y) {
            order *= 2;
        }
        let mut data_order = Vec::new();
        for d in 0..order * order {
            let (hx, hy) = hilbert_d2xy(order, d);
            if hx < x && hy < y && kind[hy * x + hx] == K_DATA {
                data_order.push((hy * x + hx) as u32);
            }
        }
        let whiten = (0..data_order.len())
            .map(|i| (mix32(0x9e37_79b9 ^ (i as u32 + 1).wrapping_mul(0x85eb_ca6b)) & 7) as u8)
            .collect();
        Some(Layout { x, y, kind, fixed, headers, lx, ly, data_order, whiten })
    }

    pub fn data_cells(&self) -> usize {
        self.data_order.len()
    }
}

pub fn cells_per_codeword(n: usize, bpc: u32) -> usize {
    (n * 8).div_ceil(bpc as usize)
}

/// Encoder choice (informative): codeword length that tiles the data area well.
pub fn choose_codeword(layout: &Layout, mode: usize, ecc_level: usize) -> (usize, usize) {
    let bpc = MODES[mode].bpc;
    let d = layout.data_cells();
    let m = d.div_ceil(cells_per_codeword(255, bpc)).max(1);
    let mut n = 255;
    while n > 24 && m * cells_per_codeword(n, bpc) > d {
        n -= 1;
    }
    let nsym = (2 * ((n as u32 * ECC_PERCENT[ecc_level] + 100) / 200)).max(4) as usize;
    (n, nsym)
}

pub fn codewords_per_frame(layout: &Layout, mode: usize, n: usize) -> usize {
    layout.data_cells() / cells_per_codeword(n, MODES[mode].bpc)
}

fn header_mask(copy: usize, i: usize) -> u8 {
    (mix32(0x4c55_4d45u32.wrapping_add(i as u32).wrapping_add(copy as u32 * 977)) & 1) as u8
}

/// The 70 header bits of one copy (already masked).
pub fn pack_header(h: &Header, copy: usize) -> [u8; HEADER_BITS] {
    let mut bits = [0u8; HEADER_BITS];
    let mut p = 0;
    let mut put = |v: u32, w: u32, bits: &mut [u8; HEADER_BITS]| {
        for i in (0..w).rev() {
            bits[p] = ((v >> i) & 1) as u8;
            p += 1;
        }
    };
    put(VERSION, 4, &mut bits);
    put(h.mode, 2, &mut bits);
    put(h.x, 9, &mut bits);
    put(h.y, 9, &mut bits);
    put(h.n, 8, &mut bits);
    put(h.nsym >> 1, 7, &mut bits);
    put(h.seq & 255, 8, &mut bits);
    put(0, 7, &mut bits);
    let crc = crc16_bits(&bits[..54]) as u32;
    put(crc, 16, &mut bits);
    for (i, b) in bits.iter_mut().enumerate() {
        *b ^= header_mask(copy, i);
    }
    bits
}

pub fn unpack_header(raw: &[u8], copy: usize) -> Option<Header> {
    let mut bits = [0u8; HEADER_BITS];
    for i in 0..HEADER_BITS {
        bits[i] = raw[i] ^ header_mask(copy, i);
    }
    let mut p = 0;
    let mut get = |w: usize| {
        let mut v = 0u32;
        for _ in 0..w {
            v = (v << 1) | bits[p] as u32;
            p += 1;
        }
        v
    };
    let ver = get(4);
    let mode = get(2);
    let x = get(9);
    let y = get(9);
    let n = get(8);
    let nsym = get(7) * 2;
    let seq = get(8);
    get(7);
    let crc = get(16);
    if crc != crc16_bits(&bits[..54]) as u32 {
        return None;
    }
    if ver != VERSION || mode > 2 {
        return None;
    }
    if (x as usize) < MIN_DIM || (y as usize) < MIN_DIM || n < 24 || nsym >= n || nsym < 2 {
        return None;
    }
    Some(Header { mode, x, y, n, nsym, seq })
}
