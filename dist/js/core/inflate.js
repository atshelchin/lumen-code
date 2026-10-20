// Raw DEFLATE (RFC 1951) decompression with an output limit. The same rules as the Rust
// inflater (src/rust/src/deflate.rs), so that both receivers accept the same payloads
// (PROTOCOL §11.3): the payload is exactly one stream, nothing may follow its final block,
// and the output may not exceed the size in the descriptor.

const LBASE = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DEXT = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

class Fail extends Error {}

// Canonical Huffman code from code lengths. An over-subscribed set is not a code.
function huffman(lengths) {
  const counts = new Uint16Array(16);
  for (const l of lengths) counts[l]++;
  counts[0] = 0;
  let left = 1;
  for (let len = 1; len < 16; len++) {
    left = left * 2 - counts[len];
    if (left < 0) throw new Fail();
  }
  const offs = new Uint16Array(16);
  for (let i = 1; i < 16; i++) offs[i] = offs[i - 1] + counts[i - 1];
  const symbols = new Uint16Array(lengths.length);
  for (let s = 0; s < lengths.length; s++) if (lengths[s]) symbols[offs[lengths[s]]++] = s;
  return { counts, symbols };
}

let FIXED = null;
function fixedCodes() {
  if (!FIXED) {
    const l = new Uint8Array(288);
    l.fill(8, 0, 144);
    l.fill(9, 144, 256);
    l.fill(7, 256, 280);
    l.fill(8, 280, 288);
    FIXED = [huffman(l), huffman(new Uint8Array(30).fill(5))];
  }
  return FIXED;
}

/**
 * Inflate raw DEFLATE data. Returns the output, or null if the data is not exactly one
 * correct stream or the output would be longer than `limit` bytes.
 */
export function inflate(data, limit) {
  let pos = 0;
  let bitBuf = 0;
  let bitCnt = 0;
  let out = new Uint8Array(Math.min(limit, Math.max(1024, data.length * 4)));
  let len = 0;

  const bits = (n) => {
    while (bitCnt < n) {
      if (pos >= data.length) throw new Fail();
      bitBuf |= data[pos++] << bitCnt;
      bitCnt += 8;
    }
    const v = bitBuf & ((1 << n) - 1);
    bitBuf >>>= n;
    bitCnt -= n;
    return v;
  };
  const decode = (h) => {
    let code = 0;
    let first = 0;
    let index = 0;
    for (let l = 1; l < 16; l++) {
      code |= bits(1);
      const count = h.counts[l];
      if (code - count < first) return h.symbols[index + (code - first)];
      index += count;
      first = (first + count) << 1;
      code <<= 1;
    }
    throw new Fail(); // a bit sequence that is not in the code
  };
  const room = (n) => {
    if (len + n > limit) throw new Fail();
    if (len + n > out.length) {
      const bigger = new Uint8Array(Math.min(limit, Math.max(len + n, out.length * 2)));
      bigger.set(out.subarray(0, len));
      out = bigger;
    }
  };

  try {
    let last = 0;
    while (!last) {
      last = bits(1);
      const type = bits(2);
      if (type === 0) {
        bitBuf = 0; // to the byte boundary
        bitCnt = 0;
        if (pos + 4 > data.length) throw new Fail();
        const n = data[pos] | (data[pos + 1] << 8);
        if ((data[pos + 2] | (data[pos + 3] << 8)) !== (~n & 0xffff)) throw new Fail();
        pos += 4;
        if (pos + n > data.length) throw new Fail();
        room(n);
        out.set(data.subarray(pos, pos + n), len);
        len += n;
        pos += n;
      } else if (type === 1 || type === 2) {
        let lit;
        let dist;
        if (type === 1) [lit, dist] = fixedCodes();
        else {
          const hlit = bits(5) + 257;
          const hdist = bits(5) + 1;
          const hclen = bits(4) + 4;
          if (hlit > 286 || hdist > 30) throw new Fail();
          const cl = new Uint8Array(19);
          for (let i = 0; i < hclen; i++) cl[ORDER[i]] = bits(3);
          const clh = huffman(cl);
          const lens = new Uint8Array(hlit + hdist);
          for (let i = 0; i < hlit + hdist; ) {
            const sym = decode(clh);
            if (sym < 16) lens[i++] = sym;
            else {
              let v = 0;
              let r;
              if (sym === 16) {
                if (i === 0) throw new Fail();
                v = lens[i - 1];
                r = 3 + bits(2);
              } else r = sym === 17 ? 3 + bits(3) : 11 + bits(7);
              if (i + r > hlit + hdist) throw new Fail();
              lens.fill(v, i, i + r);
              i += r;
            }
          }
          if (!lens[256]) throw new Fail(); // no end-of-block code
          lit = huffman(lens.subarray(0, hlit));
          dist = huffman(lens.subarray(hlit));
        }
        for (;;) {
          const sym = decode(lit);
          if (sym < 256) {
            room(1);
            out[len++] = sym;
          } else if (sym === 256) break;
          else {
            const li = sym - 257;
            if (li >= 29) throw new Fail();
            const n = LBASE[li] + bits(LEXT[li]);
            const di = decode(dist);
            if (di >= 30) throw new Fail();
            const d = DBASE[di] + bits(DEXT[di]);
            if (d > len) throw new Fail();
            room(n);
            for (let k = 0; k < n; k++, len++) out[len] = out[len - d];
          }
        }
      } else throw new Fail();
    }
    if (pos !== data.length) return null; // bytes after the final block
  } catch (e) {
    if (e instanceof Fail) return null;
    throw e;
  }
  return out.subarray(0, len);
}
