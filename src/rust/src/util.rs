//! Checksums, the integer hash and the pseudo-random generator used by the protocol.

const fn make_crc32_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
}
static CRC32_TABLE: [u32; 256] = make_crc32_table();

/// CRC-32 (IEEE 802.3, reflected, init and xorout 0xFFFFFFFF).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc = CRC32_TABLE[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

/// CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF, no reflection, no xorout).
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

/// CRC-16/CCITT-FALSE over a sequence of bits (0/1), most significant first.
pub fn crc16_bits(bits: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &bit in bits {
        let top = ((crc >> 15) & 1) as u8 ^ bit;
        crc <<= 1;
        if top != 0 {
            crc ^= 0x1021;
        }
    }
    crc
}

/// The protocol hash ("lowbias32").
#[inline]
pub fn mix32(x: u32) -> u32 {
    let mut x = x;
    x = (x ^ (x >> 16)).wrapping_mul(0x7feb_352d);
    x = (x ^ (x >> 15)).wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// The protocol pseudo-random generator.
#[derive(Clone)]
pub struct Rng {
    s: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng { s: seed }
    }
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u32 {
        self.s = self.s.wrapping_add(0x9e37_79b9);
        mix32(self.s)
    }
}

pub fn is_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    if n % 2 == 0 {
        return n == 2;
    }
    let mut d = 3u32;
    while (d as u64) * (d as u64) <= n as u64 {
        if n % d == 0 {
            return false;
        }
        d += 2;
    }
    true
}

pub fn next_prime(mut n: u32) -> u32 {
    while !is_prime(n) {
        n += 1;
    }
    n
}

/// XOR `src` into `dst` (same length).
#[inline]
pub fn xor_into(dst: &mut [u8], src: &[u8]) {
    // process 8 bytes at a time; the compiler vectorises this well
    let n = dst.len().min(src.len());
    let (d8, dr) = dst[..n].split_at_mut(n & !7);
    let (s8, sr) = src[..n].split_at(n & !7);
    for (d, s) in d8.chunks_exact_mut(8).zip(s8.chunks_exact(8)) {
        let a = u64::from_ne_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]);
        let b = u64::from_ne_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]);
        d.copy_from_slice(&(a ^ b).to_ne_bytes());
    }
    for (d, s) in dr.iter_mut().zip(sr.iter()) {
        *d ^= *s;
    }
}
