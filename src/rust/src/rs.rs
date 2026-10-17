//! Reed–Solomon over GF(256), primitive polynomial 0x11D, generator roots α^0 … α^(nsym−1).
//! Systematic encoding. Decoding corrects e errors and f erasures when 2e + f <= nsym.

const fn make_tables() -> ([u8; 512], [u8; 256]) {
    let mut exp = [0u8; 512];
    let mut log = [0u8; 256];
    let mut x: u32 = 1;
    let mut i = 0;
    while i < 255 {
        exp[i] = x as u8;
        log[x as usize] = i as u8;
        x <<= 1;
        if x & 0x100 != 0 {
            x ^= 0x11d;
        }
        i += 1;
    }
    while i < 512 {
        exp[i] = exp[i - 255];
        i += 1;
    }
    (exp, log)
}
static TABLES: ([u8; 512], [u8; 256]) = make_tables();

#[inline]
fn exp(i: usize) -> u8 {
    TABLES.0[i]
}
#[inline]
fn log(x: u8) -> usize {
    TABLES.1[x as usize] as usize
}
#[inline]
pub fn gf_mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        0
    } else {
        exp(log(a) + log(b))
    }
}
#[inline]
fn gf_inv(a: u8) -> u8 {
    exp(255 - log(a))
}

/// Generator polynomial coefficients (highest degree first, leading 1 omitted) as logs, -1 = zero.
fn generator(nsym: usize) -> Vec<i16> {
    let mut p: Vec<u8> = vec![1];
    for i in 0..nsym {
        let mut q = vec![0u8; p.len() + 1];
        for j in 0..p.len() {
            q[j] ^= p[j];
            q[j + 1] ^= gf_mul(p[j], exp(i));
        }
        p = q;
    }
    (0..nsym).map(|j| if p[j + 1] == 0 { -1 } else { log(p[j + 1]) as i16 }).collect()
}

/// Returns `msg` followed by `nsym` parity bytes.
pub fn encode(msg: &[u8], nsym: usize) -> Vec<u8> {
    let g = generator(nsym);
    let mut par = vec![0u8; nsym + 1];
    for &m in msg {
        let coef = m ^ par[0];
        par.copy_within(1.., 0);
        par[nsym - 1] = 0;
        if coef != 0 {
            let lc = log(coef);
            for j in 0..nsym {
                if g[j] >= 0 {
                    par[j] ^= exp(lc + g[j] as usize);
                }
            }
        }
    }
    let mut out = Vec::with_capacity(msg.len() + nsym);
    out.extend_from_slice(msg);
    out.extend_from_slice(&par[..nsym]);
    out
}

fn syndromes(cw: &[u8], nsym: usize, s: &mut [u8]) -> bool {
    let mut nonzero = false;
    for j in 0..nsym {
        let mut acc = 0u8;
        for &c in cw {
            acc = (if acc != 0 { exp(log(acc) + j) } else { 0 }) ^ c;
        }
        s[j] = acc;
        nonzero |= acc != 0;
    }
    nonzero
}

fn poly_eval(p: &[u8], x: u8) -> u8 {
    let mut y = 0u8;
    for i in (0..p.len()).rev() {
        y = (if y != 0 && x != 0 { exp(log(y) + log(x)) } else { 0 }) ^ p[i];
    }
    y
}

/// Decode in place. Returns the number of corrected positions, or None on failure.
pub fn decode(cw: &mut [u8], nsym: usize, erasures: &[usize]) -> Option<usize> {
    let n = cw.len();
    let mut s = vec![0u8; nsym];
    if !syndromes(cw, nsym, &mut s) {
        return Some(0);
    }
    let ne = erasures.len();
    if ne > nsym {
        return None;
    }
    // erasure locator
    let mut lambda: Vec<u8> = vec![1];
    for &pos in erasures {
        let x = exp((n - 1 - pos) % 255);
        let mut q = vec![0u8; lambda.len() + 1];
        for i in 0..lambda.len() {
            q[i] ^= lambda[i];
            q[i + 1] ^= gf_mul(lambda[i], x);
        }
        lambda = q;
    }
    // Berlekamp–Massey with erasures
    let mut b = lambda.clone();
    let mut l = ne;
    for r in (ne + 1)..=nsym {
        let mut delta = 0u8;
        for i in 0..=l.min(lambda.len() - 1) {
            if r > i {
                delta ^= gf_mul(lambda[i], s[r - 1 - i]);
            }
        }
        let mut xb = Vec::with_capacity(b.len() + 1);
        xb.push(0);
        xb.extend_from_slice(&b);
        if delta == 0 {
            b = xb;
            continue;
        }
        let len = lambda.len().max(xb.len());
        let mut t = vec![0u8; len];
        t[..lambda.len()].copy_from_slice(&lambda);
        for i in 0..xb.len() {
            t[i] ^= gf_mul(delta, xb[i]);
        }
        if 2 * l < r + ne {
            let inv = gf_inv(delta);
            b = lambda.iter().map(|&c| gf_mul(c, inv)).collect();
            l = r + ne - l;
        } else {
            b = xb;
        }
        lambda = t;
    }
    while lambda.len() > 1 && *lambda.last().unwrap() == 0 {
        lambda.pop();
    }
    let deg = lambda.len() - 1;
    if deg != l || 2 * (l - ne.min(l)) + ne > nsym {
        return None;
    }
    // Chien search
    let mut pos = Vec::new();
    for p in 0..n {
        let xinv = exp((255 - ((n - 1 - p) % 255)) % 255);
        if poly_eval(&lambda, xinv) == 0 {
            pos.push(p);
        }
    }
    if pos.len() != deg {
        return None;
    }
    // Ω = S·Λ mod x^nsym ; Λ' (odd terms)
    let mut omega = vec![0u8; nsym];
    for i in 0..lambda.len() {
        if lambda[i] == 0 {
            continue;
        }
        for j in 0..(nsym - i.min(nsym)) {
            omega[i + j] ^= gf_mul(lambda[i], s[j]);
        }
    }
    let mut dl = vec![0u8; lambda.len()];
    let mut i = 1;
    while i < lambda.len() {
        dl[i - 1] = lambda[i];
        i += 2;
    }
    for &p in &pos {
        let x = exp((n - 1 - p) % 255);
        let xinv = gf_inv(x);
        let num = poly_eval(&omega, xinv);
        let den = poly_eval(&dl, xinv);
        if den == 0 {
            return None;
        }
        cw[p] ^= gf_mul(x, gf_mul(num, gf_inv(den)));
    }
    if syndromes(cw, nsym, &mut s) {
        return None;
    }
    Some(pos.len())
}
