//! Raptor-style fountain code (normative encoding, see PROTOCOL §9) and an inactivation
//! decoder (peeling + GF(2) Gauss–Jordan on the inactive part).

use crate::util::{is_prime, mix32, next_prime, xor_into, Rng};

/// Cumulative degree distribution (out of 2^20).
pub const DEG: [u32; 31] = [
    0, 5243, 529531, 704294, 791675, 844104, 879057, 904023, 922747, 937311, 948962, 958494, 966438, 973160,
    978921, 983914, 988283, 992138, 995565, 998631, 1001391, 1003887, 1006157, 1008229, 1010129, 1011876,
    1013490, 1014983, 1016370, 1017662, 1048576,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    pub k: u32,
    pub s: u32,
    pub p: u32,
    pub p1: u32,
    pub w: u32,
    pub l: u32,
}

fn bit_length(k: u32) -> u32 {
    32 - k.leading_zeros()
}

pub fn params(k: u32) -> Params {
    let mut x: u64 = 2;
    while x * (x - 1) < 2 * k as u64 {
        x += 1;
    }
    let mut s = (k.div_ceil(100) + x as u32).max(3);
    while !is_prime(k + s) {
        s += 1;
    }
    let w = k + s;
    let p = 12 + bit_length(k);
    let p1 = next_prime(p + 1);
    Params { k, s, p, p1, w, l: w + p }
}

/// LDPC membership: rows[s] lists the source symbols of LDPC row s.
pub fn ldpc_rows(k: u32, s: u32) -> Vec<Vec<u32>> {
    let mut rows = vec![Vec::new(); s as usize];
    for i in 0..k {
        let mut a = 1 + ((i / s) % (s - 1));
        if (2 * a) % s == 0 {
            a = 1;
        }
        let b = i % s;
        rows[b as usize].push(i);
        rows[((b + a) % s) as usize].push(i);
        rows[((b + 2 * a) % s) as usize].push(i);
    }
    rows
}

/// PI row h: pseudo-random half of the source symbols.
pub fn pi_members(k: u32, h: u32, seed: u32) -> Vec<u32> {
    let mut rng = Rng::new(mix32(seed ^ (h + 1).wrapping_mul(0x5bd1_e995)));
    let mut out = Vec::with_capacity(k as usize / 2 + 8);
    let mut i = 0;
    while i < k {
        let mut bits = rng.next();
        let lim = 32.min(k - i);
        for b in 0..lim {
            if bits & 1 != 0 {
                out.push(i + b);
            }
            bits >>= 1;
        }
        i += 32;
    }
    out
}

/// Columns combined into encoded symbol `esi`: LT columns (< W) and PI columns (0..P).
pub fn tuple(p: &Params, seed: u32, esi: u32) -> (Vec<u32>, Vec<u32>) {
    let mut r = Rng::new(mix32(seed ^ (esi.wrapping_add(1)).wrapping_mul(0x9e37_79b1)));
    let v = r.next() & 0xfffff;
    let mut d = 1usize;
    while v >= DEG[d] {
        d += 1;
    }
    let d = (d as u32).min(p.w - 2).max(1);
    let a = 1 + r.next() % (p.w - 1);
    let mut b = r.next() % p.w;
    let mut lt = Vec::with_capacity(d as usize);
    lt.push(b);
    for _ in 1..d {
        b = (b + a) % p.w;
        lt.push(b);
    }
    let d1 = if d < 4 { 2 + (r.next() & 1) } else { 2 };
    let a1 = 1 + r.next() % (p.p1 - 1);
    let mut b1 = r.next() % p.p1;
    let mut pi = Vec::with_capacity(d1 as usize);
    for _ in 0..d1 {
        while b1 >= p.p {
            b1 = (b1 + a1) % p.p1;
        }
        pi.push(b1);
        b1 = (b1 + a1) % p.p1;
    }
    (lt, pi)
}

pub struct Encoder {
    pub p: Params,
    pub t: usize,
    seed: u32,
    sym: Vec<u8>, // L symbols of T bytes
}

impl Encoder {
    /// `payload` is padded with zeros to K·T bytes, K = max(1, ceil(len/T)).
    pub fn new(payload: &[u8], t: usize, seed: u32) -> Self {
        let k = (payload.len().div_ceil(t)).max(1) as u32;
        let p = params(k);
        let mut sym = vec![0u8; p.l as usize * t];
        sym[..payload.len()].copy_from_slice(payload);
        let (k_, s_, w_, pp) = (k as usize, p.s as usize, p.w as usize, p.p as usize);
        for h in 0..pp {
            for i in pi_members(k, h as u32, seed) {
                let (a, b) = sym.split_at_mut((w_ + h) * t);
                xor_into(&mut b[..t], &a[i as usize * t..(i as usize + 1) * t]);
            }
        }
        let ldpc = ldpc_rows(k, p.s);
        for s in 0..s_ {
            let dst = (k_ + s) * t;
            let mut acc = vec![0u8; t];
            for &i in &ldpc[s] {
                xor_into(&mut acc, &sym[i as usize * t..(i as usize + 1) * t]);
            }
            xor_into(&mut acc, &sym[(w_ + s % pp) * t..(w_ + s % pp + 1) * t]);
            xor_into(&mut acc, &sym[(w_ + (s + 1) % pp) * t..(w_ + (s + 1) % pp + 1) * t]);
            sym[dst..dst + t].copy_from_slice(&acc);
        }
        Encoder { p, t, seed, sym }
    }

    pub fn k(&self) -> u32 {
        self.p.k
    }

    /// Writes encoded symbol `esi` (T bytes) into `out`.
    pub fn encode(&self, esi: u32, out: &mut [u8]) {
        let t = self.t;
        let (lt, pi) = tuple(&self.p, self.seed, esi);
        out[..t].fill(0);
        for c in lt {
            xor_into(&mut out[..t], &self.sym[c as usize * t..(c as usize + 1) * t]);
        }
        let w = self.p.w as usize;
        for j in pi {
            xor_into(&mut out[..t], &self.sym[(w + j as usize) * t..(w + j as usize + 1) * t]);
        }
    }
}

const ACTIVE: u8 = 0;
const PIVOT: u8 = 1;
const INACTIVE: u8 = 2;

pub struct Decoder {
    pub p: Params,
    pub t: usize,
    seed: u32,
    esis: Vec<u32>,
    data: Vec<u8>,
    seen: std::collections::HashSet<u32>,
    ldpc: Vec<Vec<u32>>,
    pi: Option<Vec<Vec<u32>>>,
}

impl Decoder {
    pub fn new(k: u32, t: usize, seed: u32) -> Self {
        let p = params(k);
        Decoder { p, t, seed, esis: Vec::new(), data: Vec::new(), seen: Default::default(), ldpc: ldpc_rows(k, p.s), pi: None }
    }

    pub fn k(&self) -> u32 {
        self.p.k
    }

    pub fn count(&self) -> usize {
        self.esis.len()
    }

    /// Adds a received symbol. Returns false for a duplicate.
    pub fn add(&mut self, esi: u32, bytes: &[u8]) -> bool {
        if !self.seen.insert(esi) {
            return false;
        }
        self.esis.push(esi);
        let start = self.data.len();
        self.data.resize(start + self.t, 0);
        let n = bytes.len().min(self.t);
        self.data[start..start + n].copy_from_slice(&bytes[..n]);
        true
    }

    /// Returns K·T bytes when the received symbols determine the source block.
    pub fn decode(&mut self) -> Option<Vec<u8>> {
        let Params { k, s, p: pp, w, .. } = self.p;
        let (k, s, pp, w) = (k as usize, s as usize, pp as usize, w as usize);
        let t = self.t;
        if self.count() < k {
            return None;
        }
        if self.pi.is_none() {
            self.pi = Some((0..pp).map(|h| pi_members(k as u32, h as u32, self.seed)).collect());
        }
        let n = self.count();
        let r_total = s + n;
        let mut row_lt: Vec<Vec<u32>> = Vec::with_capacity(r_total);
        let mut row_pi: Vec<Vec<u32>> = Vec::with_capacity(r_total);
        for si in 0..s {
            let mut v = self.ldpc[si].clone();
            v.push((k + si) as u32);
            row_lt.push(v);
            row_pi.push(vec![(si % pp) as u32, ((si + 1) % pp) as u32]);
        }
        for i in 0..n {
            let (lt, pi) = tuple(&self.p, self.seed, self.esis[i]);
            row_lt.push(lt);
            row_pi.push(pi);
        }

        // column -> rows (CSR)
        let mut col_start = vec![0usize; w + 1];
        for r in &row_lt {
            for &c in r {
                col_start[c as usize + 1] += 1;
            }
        }
        for c in 0..w {
            col_start[c + 1] += col_start[c];
        }
        let mut col_rows = vec![0u32; col_start[w]];
        let mut fill = col_start.clone();
        for (ri, r) in row_lt.iter().enumerate() {
            for &c in r {
                col_rows[fill[c as usize]] = ri as u32;
                fill[c as usize] += 1;
            }
        }

        // phase 1: peeling with inactivation
        let mut deg: Vec<i32> = row_lt.iter().map(|r| r.len() as i32).collect();
        let max_deg = deg.iter().copied().max().unwrap_or(0) as usize;
        let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); max_deg + 1];
        for ri in (0..r_total).rev() {
            buckets[deg[ri] as usize].push(ri as u32);
        }
        let mut used = vec![false; r_total];
        let mut state = vec![ACTIVE; w];
        let mut inact_idx = vec![u32::MAX; w];
        let mut pivot_row = vec![u32::MAX; w];
        let mut pos = vec![u32::MAX; w];
        let mut order: Vec<u32> = Vec::new();
        let mut n_inact = 0u32;
        let mut active = w;

        fn touch(c: usize, except: usize, col_start: &[usize], col_rows: &[u32], used: &[bool], deg: &mut [i32], buckets: &mut [Vec<u32>]) {
            for &r2 in &col_rows[col_start[c]..col_start[c + 1]] {
                let r2 = r2 as usize;
                if r2 == except || used[r2] {
                    continue;
                }
                deg[r2] -= 1;
                if deg[r2] >= 1 {
                    buckets[deg[r2] as usize].push(r2 as u32);
                }
            }
        }

        while active > 0 {
            let mut pick = usize::MAX;
            'outer: for bkt in 1..=max_deg {
                while let Some(cand) = buckets[bkt].pop() {
                    let cand = cand as usize;
                    if !used[cand] && deg[cand] as usize == bkt {
                        pick = cand;
                        break 'outer;
                    }
                }
            }
            if pick == usize::MAX {
                break;
            }
            let r = pick;
            if deg[r] > 1 {
                let mut keep = usize::MAX;
                let mut best = usize::MAX;
                for &c in &row_lt[r] {
                    let c = c as usize;
                    if state[c] != ACTIVE {
                        continue;
                    }
                    let cnt = col_start[c + 1] - col_start[c];
                    if cnt < best {
                        best = cnt;
                        keep = c;
                    }
                }
                let cols: Vec<u32> = row_lt[r].clone();
                for c in cols {
                    let c = c as usize;
                    if state[c] == ACTIVE && c != keep {
                        state[c] = INACTIVE;
                        inact_idx[c] = n_inact;
                        n_inact += 1;
                        active -= 1;
                        touch(c, usize::MAX, &col_start, &col_rows, &used, &mut deg, &mut buckets);
                    }
                }
            }
            let mut c = usize::MAX;
            for &cc in &row_lt[r] {
                if state[cc as usize] == ACTIVE {
                    c = cc as usize;
                }
            }
            if c == usize::MAX {
                continue;
            }
            state[c] = PIVOT;
            used[r] = true;
            pivot_row[c] = r as u32;
            pos[c] = order.len() as u32;
            order.push(c as u32);
            active -= 1;
            touch(c, r, &col_start, &col_rows, &used, &mut deg, &mut buckets);
        }
        for c in 0..w {
            if state[c] == ACTIVE {
                state[c] = INACTIVE;
                inact_idx[c] = n_inact;
                n_inact += 1;
            }
        }

        let ni = pp + n_inact as usize;
        let wd = ni.div_ceil(32);
        let npiv = order.len();

        // phase 2 (symbolic): inactive-dependency bitsets of the pivots
        let mut bits = vec![0u32; npiv * wd];
        for kk in 0..npiv {
            let c = order[kk] as usize;
            let r = pivot_row[c] as usize;
            let off = kk * wd;
            for &c2 in &row_lt[r] {
                let c2 = c2 as usize;
                if c2 == c {
                    continue;
                }
                if state[c2] == PIVOT {
                    let o2 = pos[c2] as usize * wd;
                    for x in 0..wd {
                        bits[off + x] ^= bits[o2 + x];
                    }
                } else {
                    let b = pp + inact_idx[c2] as usize;
                    bits[off + (b >> 5)] ^= 1 << (b & 31);
                }
            }
            for &j in &row_pi[r] {
                bits[off + (j as usize >> 5)] ^= 1 << (j & 31);
            }
        }
        // leftover equations: unused sparse rows + PI rows
        let pi_rows = self.pi.as_ref().unwrap();
        let mut eq: Vec<(usize, bool)> = Vec::new(); // (index, is_pi)
        for (r, &u) in used.iter().enumerate() {
            if !u {
                eq.push((r, false));
            }
        }
        for h in 0..pp {
            eq.push((h, true));
        }
        let e_n = eq.len();
        if e_n < ni {
            return None;
        }
        let mut m = vec![0u32; e_n * wd];
        for (e, &(idx, is_pi)) in eq.iter().enumerate() {
            let off = e * wd;
            let (lt, pis): (&[u32], Vec<u32>) =
                if is_pi { (&pi_rows[idx], vec![idx as u32]) } else { (&row_lt[idx], row_pi[idx].clone()) };
            for &c in lt {
                let c = c as usize;
                if state[c] == PIVOT {
                    let o2 = pos[c] as usize * wd;
                    for x in 0..wd {
                        m[off + x] ^= bits[o2 + x];
                    }
                } else {
                    let b = pp + inact_idx[c] as usize;
                    m[off + (b >> 5)] ^= 1 << (b & 31);
                }
            }
            for j in pis {
                m[off + (j as usize >> 5)] ^= 1 << (j & 31);
            }
        }
        // Gauss–Jordan, recording row operations
        let mut perm: Vec<usize> = (0..e_n).collect();
        let mut ops: Vec<(u32, u32)> = Vec::new();
        for col in 0..ni {
            let wi = col >> 5;
            let mask = 1u32 << (col & 31);
            let mut pr = usize::MAX;
            for i in col..e_n {
                if m[perm[i] * wd + wi] & mask != 0 {
                    pr = i;
                    break;
                }
            }
            if pr == usize::MAX {
                return None; // rank deficient: need more symbols
            }
            perm.swap(pr, col);
            let tp = perm[col];
            let po = tp * wd;
            for i in 0..e_n {
                if i == col {
                    continue;
                }
                let q = perm[i];
                let qo = q * wd;
                if m[qo + wi] & mask != 0 {
                    for x in wi..wd {
                        m[qo + x] ^= m[po + x];
                    }
                    ops.push((q as u32, tp as u32));
                }
            }
        }

        // numeric phase
        let rhs_of = |r: usize| -> Option<&[u8]> { if r >= s { Some(&self.data[(r - s) * t..(r - s + 1) * t]) } else { None } };
        let mut dbuf = vec![0u8; npiv * t];
        for kk in 0..npiv {
            let c = order[kk] as usize;
            let r = pivot_row[c] as usize;
            let (done, rest) = dbuf.split_at_mut(kk * t);
            let dst = &mut rest[..t];
            if let Some(rhs) = rhs_of(r) {
                dst.copy_from_slice(rhs);
            }
            for &c2 in &row_lt[r] {
                let c2 = c2 as usize;
                if c2 != c && state[c2] == PIVOT {
                    let o = pos[c2] as usize * t;
                    xor_into(dst, &done[o..o + t]);
                }
            }
        }
        let mut rv = vec![0u8; e_n * t];
        for (e, &(idx, is_pi)) in eq.iter().enumerate() {
            let dst = &mut rv[e * t..(e + 1) * t];
            let lt: &[u32] = if is_pi { &pi_rows[idx] } else { &row_lt[idx] };
            if !is_pi {
                if let Some(rhs) = rhs_of(idx) {
                    dst.copy_from_slice(rhs);
                }
            }
            for &c in lt {
                let c = c as usize;
                if state[c] == PIVOT {
                    let o = pos[c] as usize * t;
                    xor_into(dst, &dbuf[o..o + t]);
                }
            }
        }
        for &(dst, src) in &ops {
            let (dst, src) = (dst as usize, src as usize);
            if dst < src {
                let (a, b) = rv.split_at_mut(src * t);
                xor_into(&mut a[dst * t..dst * t + t], &b[..t]);
            } else {
                let (a, b) = rv.split_at_mut(dst * t);
                xor_into(&mut b[..t], &a[src * t..src * t + t]);
            }
        }
        let yoff = |kk: usize| perm[kk] * t;
        // back-substitute pivots
        for kk in 0..npiv {
            let c = order[kk] as usize;
            let r = pivot_row[c] as usize;
            let mut acc = vec![0u8; t];
            if let Some(rhs) = rhs_of(r) {
                acc.copy_from_slice(rhs);
            }
            for &c2 in &row_lt[r] {
                let c2 = c2 as usize;
                if c2 == c {
                    continue;
                }
                if state[c2] == PIVOT {
                    let o = pos[c2] as usize * t;
                    xor_into(&mut acc, &dbuf[o..o + t]);
                } else {
                    let o = yoff(pp + inact_idx[c2] as usize);
                    xor_into(&mut acc, &rv[o..o + t]);
                }
            }
            for &j in &row_pi[r] {
                let o = yoff(j as usize);
                xor_into(&mut acc, &rv[o..o + t]);
            }
            dbuf[kk * t..(kk + 1) * t].copy_from_slice(&acc);
        }
        let mut out = vec![0u8; k * t];
        for i in 0..k {
            let src = if state[i] == PIVOT {
                let o = pos[i] as usize * t;
                &dbuf[o..o + t]
            } else {
                let o = yoff(pp + inact_idx[i] as usize);
                &rv[o..o + t]
            };
            out[i * t..(i + 1) * t].copy_from_slice(src);
        }
        Some(out)
    }
}
