// Small shared helpers: CRCs, hashing PRNG, primes.

const CRC32_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

export function crc32(buf, crc = 0) {
  crc = ~crc >>> 0;
  for (let i = 0; i < buf.length; i++) crc = CRC32_TABLE[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
  return ~crc >>> 0;
}

// CRC-16/CCITT-FALSE over a bit array (0/1 values), MSB-first.
export function crc16bits(bits, len) {
  let crc = 0xffff;
  for (let i = 0; i < len; i++) {
    const top = ((crc >>> 15) & 1) ^ bits[i];
    crc = (crc << 1) & 0xffff;
    if (top) crc ^= 0x1021;
  }
  return crc;
}

// CRC-16/CCITT-FALSE over bytes
export function crc16(buf, len = buf.length) {
  let crc = 0xffff;
  for (let i = 0; i < len; i++) {
    crc ^= buf[i] << 8;
    for (let k = 0; k < 8; k++) crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
  }
  return crc;
}

// lowbias32 integer hash
export function mix32(x) {
  x = Math.imul(x ^ (x >>> 16), 0x7feb352d);
  x = Math.imul(x ^ (x >>> 15), 0x846ca68b);
  return (x ^ (x >>> 16)) >>> 0;
}

// splitmix-style 32-bit generator
export class Rng {
  constructor(seed) {
    this.s = seed >>> 0;
  }
  next() {
    this.s = (this.s + 0x9e3779b9) >>> 0;
    return mix32(this.s);
  }
}

export function isPrime(n) {
  if (n < 2) return false;
  if (n % 2 === 0) return n === 2;
  for (let d = 3; d * d <= n; d += 2) if (n % d === 0) return false;
  return true;
}

export function nextPrime(n) {
  while (!isPrime(n)) n++;
  return n;
}

// XOR src into dst (both Uint32Array views of equal length)
export function xor32(dst, src) {
  for (let i = 0; i < dst.length; i++) dst[i] ^= src[i];
}
