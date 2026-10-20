//! Raw DEFLATE (RFC 1951): a complete inflater and a compact compressor
//! (LZ77 with hash chains + fixed Huffman codes). Zero dependencies.

// ---------------------------------------------------------------- inflate

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
    nbits: u32,
}

impl<'a> Bits<'a> {
    fn need(&mut self, n: u32) -> Option<()> {
        while self.nbits < n {
            let b = *self.data.get(self.pos)? as u32;
            self.pos += 1;
            self.bit |= b << self.nbits;
            self.nbits += 8;
        }
        Some(())
    }
    fn get(&mut self, n: u32) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        self.need(n)?;
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.nbits -= n;
        Some(v)
    }
    fn align(&mut self) {
        let drop = self.nbits % 8;
        self.bit >>= drop;
        self.nbits -= drop;
    }
}

struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Option<Huff> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        // an over-subscribed set of lengths is not a prefix code
        let mut left = 1i32;
        for &c in &counts[1..] {
            left = left * 2 - c as i32;
            if left < 0 {
                return None;
            }
        }
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + counts[i - 1];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Some(Huff { counts, symbols })
    }
    fn decode(&self, b: &mut Bits) -> Option<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.get(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return self.symbols.get((index + (code - first)) as usize).copied();
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        None
    }
}

const LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn fixed_tables() -> (Huff, Huff) {
    let mut l = [0u8; 288];
    for (i, v) in l.iter_mut().enumerate() {
        *v = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    (Huff::new(&l).unwrap(), Huff::new(&[5u8; 30]).unwrap())
}

/// Inflate raw DEFLATE data (PROTOCOL §9.1, §11.3): `data` must be exactly one stream, with
/// no byte after its final block, and the output may not be longer than `limit` bytes
/// (protection against bombs). The same rules as src/core/inflate.js.
pub fn inflate(data: &[u8], limit: usize) -> Option<Vec<u8>> {
    let mut b = Bits { data, pos: 0, bit: 0, nbits: 0 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.get(1)?;
        let typ = b.get(2)?;
        match typ {
            0 => {
                b.align();
                let len = b.get(16)?;
                let nlen = b.get(16)?;
                if len != (!nlen & 0xffff) {
                    return None;
                }
                if out.len() + len as usize > limit {
                    return None;
                }
                for _ in 0..len {
                    out.push(b.get(8)? as u8);
                }
            }
            1 | 2 => {
                let (lit, dist) = if typ == 1 {
                    fixed_tables()
                } else {
                    let hlit = b.get(5)? as usize + 257;
                    let hdist = b.get(5)? as usize + 1;
                    let hclen = b.get(4)? as usize + 4;
                    if hlit > 286 || hdist > 30 {
                        return None;
                    }
                    const ORD: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                    let mut cl = [0u8; 19];
                    for &o in ORD.iter().take(hclen) {
                        cl[o] = b.get(3)? as u8;
                    }
                    let clh = Huff::new(&cl)?;
                    let mut lens = vec![0u8; hlit + hdist];
                    let mut i = 0;
                    while i < hlit + hdist {
                        let sym = clh.decode(&mut b)?;
                        match sym {
                            0..=15 => {
                                lens[i] = sym as u8;
                                i += 1;
                            }
                            16 => {
                                let prev = *lens.get(i.checked_sub(1)?)?;
                                for _ in 0..3 + b.get(2)? {
                                    *lens.get_mut(i)? = prev;
                                    i += 1;
                                }
                            }
                            17 => i += 3 + b.get(3)? as usize,
                            _ => i += 11 + b.get(7)? as usize,
                        }
                        if i > hlit + hdist {
                            return None;
                        }
                    }
                    if i > hlit + hdist || lens[256] == 0 {
                        return None; // a repeat past the end, or no end-of-block code
                    }
                    (Huff::new(&lens[..hlit])?, Huff::new(&lens[hlit..])?)
                };
                loop {
                    let sym = lit.decode(&mut b)? as usize;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let li = sym - 257;
                        if li >= 29 {
                            return None;
                        }
                        let len = LBASE[li] as usize + b.get(LEXT[li] as u32)? as usize;
                        let di = dist.decode(&mut b)? as usize;
                        if di >= 30 {
                            return None;
                        }
                        let d = DBASE[di] as usize + b.get(DEXT[di] as u32)? as usize;
                        if d > out.len() {
                            return None;
                        }
                        let start = out.len() - d;
                        for k in 0..len {
                            let v = out[start + k];
                            out.push(v);
                        }
                    }
                    if out.len() > limit {
                        return None;
                    }
                }
            }
            _ => return None,
        }
        if out.len() > limit {
            return None;
        }
        if last == 1 {
            break;
        }
    }
    if b.pos != data.len() {
        return None; // bytes after the final block
    }
    Some(out)
}

// ---------------------------------------------------------------- deflate

struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, v: u32, bits: u32) {
        self.acc |= (v as u64) << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// Huffman codes are written most-significant bit first.
    fn put_rev(&mut self, code: u32, len: u32) {
        let mut r = 0u32;
        for i in 0..len {
            r |= ((code >> i) & 1) << (len - 1 - i);
        }
        self.put(r, len);
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

fn put_lit(w: &mut BitWriter, sym: u32) {
    match sym {
        0..=143 => w.put_rev(0x30 + sym, 8),
        144..=255 => w.put_rev(0x190 + sym - 144, 9),
        256..=279 => w.put_rev(sym - 256, 7),
        _ => w.put_rev(0xc0 + sym - 280, 8),
    }
}

/// Compress with LZ77 (32 KiB window, hash chains) and fixed Huffman codes.
pub fn deflate(data: &[u8]) -> Vec<u8> {
    const WIN: usize = 32768;
    const HBITS: u32 = 15;
    let mut head = vec![u32::MAX; 1 << HBITS];
    let mut prev = vec![u32::MAX; WIN];
    let hash = |i: usize| -> usize {
        let v = (data[i] as u32) | (data[i + 1] as u32) << 8 | (data[i + 2] as u32) << 16;
        (v.wrapping_mul(0x9e37_79b1) >> (32 - HBITS)) as usize
    };
    let mut w = BitWriter { out: Vec::with_capacity(data.len() / 2 + 16), acc: 0, n: 0 };
    w.put(1, 1); // final block
    w.put(1, 2); // fixed Huffman
    let mut i = 0;
    let insert = |pos: usize, head: &mut Vec<u32>, prev: &mut Vec<u32>| {
        if pos + 2 < data.len() {
            let h = hash(pos);
            prev[pos % WIN] = head[h];
            head[h] = pos as u32;
        }
    };
    while i < data.len() {
        let (mut best_len, mut best_d) = (0usize, 0usize);
        if i + 2 < data.len() {
            let mut cand = head[hash(i)];
            let mut chain = 48;
            while cand != u32::MAX && chain > 0 {
                let c = cand as usize;
                if i - c > WIN - 1 {
                    break;
                }
                let max = (data.len() - i).min(258);
                let mut l = 0;
                while l < max && data[c + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    best_len = l;
                    best_d = i - c;
                    if l == max {
                        break;
                    }
                }
                cand = prev[c % WIN];
                chain -= 1;
            }
        }
        if best_len >= 3 {
            let li = LBASE.iter().rposition(|&b| b as usize <= best_len).unwrap();
            put_lit(&mut w, 257 + li as u32);
            w.put((best_len - LBASE[li] as usize) as u32, LEXT[li] as u32);
            let di = DBASE.iter().rposition(|&b| b as usize <= best_d).unwrap();
            w.put_rev(di as u32, 5);
            w.put((best_d - DBASE[di] as usize) as u32, DEXT[di] as u32);
            for k in 0..best_len {
                insert(i + k, &mut head, &mut prev);
            }
            i += best_len;
        } else {
            put_lit(&mut w, data[i] as u32);
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
    put_lit(&mut w, 256);
    w.finish()
}
