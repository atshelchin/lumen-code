// Pseudo-random input corpora for differential testing. src/rust/tests/corpus.rs makes the
// same inputs from the same seeds (every rng.next() call below is in a fixed order), so the
// two implementations can be compared through a digest of their outcomes
// (test-vectors.json "differential"), and test-fuzz.js checks invariants on larger runs.
import { crc32, crc16bits, Rng } from '../src/core/util.js';
import { decodeHeader, HEADER_MASKS } from '../src/core/layout.js';
import { parseDescriptor } from '../src/core/frame.js';
import { inflate } from '../src/core/inflate.js';
import { FountainEncoder, FountainDecoder } from '../src/core/fountain.js';
import { fountainSeed } from '../src/core/transfer.js';

// The 70 masked header bits of copy `copy` for any field values, with a correct CRC.
export function rawHeaderBits(f, copy) {
  const bits = new Uint8Array(70);
  let p = 0;
  const put = (v, w) => { for (let i = w - 1; i >= 0; i--) bits[p++] = (v >>> i) & 1; };
  put(f.version, 4); put(f.mode, 2); put(f.X, 9); put(f.Y, 9); put(f.n, 8); put(f.nsym >>> 1, 7); put(f.seq, 8); put(f.reserved, 7);
  put(crc16bits(bits, 54), 16);
  for (let i = 0; i < 70; i++) bits[i] ^= HEADER_MASKS[copy][i];
  return bits;
}

// Headers with random, often invalid fields; one in eight has a flipped bit.
export function* headerCorpus(seed, count) {
  const r = new Rng(seed);
  for (let i = 0; i < count; i++) {
    const version = r.next() % 4 === 0 ? r.next() % 16 : 1;
    const mode = r.next() % 4;
    const X = r.next() % 2 === 0 ? 40 + (r.next() % 472) : r.next() % 512;
    const Y = r.next() % 2 === 0 ? 40 + (r.next() % 472) : r.next() % 512;
    const n = r.next() % 256;
    const nsym = 2 * (r.next() % 128);
    const seq = r.next() % 256;
    const reserved = r.next() % 128;
    const copy = i % 4;
    const bits = rawHeaderBits({ version, mode, X, Y, n, nsym, seq, reserved }, copy);
    if (r.next() % 8 === 0) bits[r.next() % 70] ^= 1;
    yield { bits, copy };
  }
}

// Descriptors of random length and content, pushed towards the valid ones.
export function* descriptorCorpus(seed, count) {
  const r = new Rng(seed);
  for (let i = 0; i < count; i++) {
    const T = 16 + (r.next() % 240);
    const d = new Uint8Array(T);
    for (let j = 0; j < T; j++) d[j] = r.next() & 255;
    if (r.next() % 8 !== 0) d[0] = 1;
    if (r.next() % 8 !== 0) d[1] &= 1;
    if (r.next() % 4 !== 0) d[14] = r.next() % (T - 14);
    if (r.next() % 4 !== 0) d[6] = d[7] = 0; // F < 65536
    if (r.next() % 4 !== 0) d.copyWithin(2, 6, 10); // L = F
    if (r.next() % 4 !== 0) for (let j = 15; j < 15 + d[14] && j < T; j++) d[j] = 0x20 + (r.next() % 95);
    if (r.next() % 2 === 0) d.fill(0, Math.min(T, 15 + d[14]));
    yield d;
  }
}

// Mutated DEFLATE streams: one bit flip, cut, overwritten byte or appended byte; random limit.
export function* inflateCorpus(seed, count, bases, fileLen) {
  const r = new Rng(seed);
  for (let i = 0; i < count; i++) {
    let s = bases[r.next() % bases.length].slice();
    const op = r.next() % 4;
    if (op === 0) s[r.next() % s.length] ^= 1 << (r.next() % 8);
    else if (op === 1) s = s.subarray(0, r.next() % s.length);
    else if (op === 2) s[r.next() % s.length] = r.next() & 255;
    else s = Uint8Array.from([...s, r.next() & 255]);
    const limit = r.next() % 2 === 0 ? fileLen : r.next() % (fileLen + 1);
    yield { data: s, limit };
  }
}

// Random sets of K - 1 … K + 3 distinct symbols of a random block (T = 4).
export function* fountainCorpus(seed, count) {
  const r = new Rng(seed);
  for (let i = 0; i < count; i++) {
    const K = 1 + (r.next() % 60);
    const tid = r.next() & 0xffff;
    const block = new Uint8Array(K * 4);
    for (let j = 0; j < block.length; j++) block[j] = r.next() & 255;
    const n = K - 1 + (r.next() % 5);
    const esis = [];
    while (esis.length < n) {
      const e = r.next() & 0xffff;
      if (!esis.includes(e)) esis.push(e);
    }
    yield { K, seed: fountainSeed(tid, 0), block, esis };
  }
}

// ---- outcome digests (CRC-32 of a byte record per case)

const u32be = (v) => [(v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255];

export function headerDigest(seed, count) {
  const rec = [];
  for (const { bits, copy } of headerCorpus(seed, count)) {
    const h = decodeHeader(bits, copy).header;
    if (!h) rec.push(0);
    else rec.push(1, h.mode, h.X >> 8, h.X & 255, h.Y >> 8, h.Y & 255, h.n, h.nsym, h.seq);
  }
  return crc32(Uint8Array.from(rec));
}

export function descriptorDigest(seed, count) {
  const rec = [];
  for (const d of descriptorCorpus(seed, count)) {
    const r = parseDescriptor(d).descriptor;
    if (!r) {
      rec.push(0);
      continue;
    }
    const name = new TextEncoder().encode(r.name);
    rec.push(1, r.flags, ...u32be(r.payloadLength), ...u32be(r.fileSize), ...u32be(r.crc), name.length, ...name, r.records.length);
    for (const x of r.records) rec.push(x.type, x.value.length, ...x.value);
  }
  return crc32(Uint8Array.from(rec));
}

export function inflateDigest(seed, count, bases, fileLen) {
  const rec = [];
  for (const { data, limit } of inflateCorpus(seed, count, bases, fileLen)) {
    const out = inflate(data, limit);
    if (!out) rec.push(0);
    else rec.push(1, ...u32be(out.length), ...u32be(crc32(out)));
  }
  return crc32(Uint8Array.from(rec));
}

export function fountainDigest(seed, count) {
  const rec = [];
  for (const { K, seed: s, block, esis } of fountainCorpus(seed, count)) {
    const enc = new FountainEncoder(block, 4, s);
    const dec = new FountainDecoder(K, 4, s);
    for (const e of esis) dec.add(e, enc.encode(e, new Uint8Array(4)));
    const out = dec.decode();
    if (out && crc32(out) !== crc32(block)) throw new Error('fountain decoder returned a wrong block');
    rec.push(out ? 1 : 0);
  }
  return crc32(Uint8Array.from(rec));
}
