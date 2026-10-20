// Generates test/vectors/test-vectors.json from the JS reference implementation.
// Every conforming implementation must reproduce these values (PROTOCOL §15).
//   node test/vectors.js           write the file
//   node test/vectors.js --check   fail if the file differs from what the code makes now
import { readFileSync, writeFileSync } from 'node:fs';
import { crc32, crc16, crc16bits, mix32, Rng } from '../src/core/util.js';
import { rsEncode, rsDecode } from '../src/core/rs.js';
import { makeLayout, packHeader, decodeHeader, chooseCodeword, codewordsPerFrame, cellsPerCodeword, HEADER_MASKS, dataCellCount } from '../src/core/layout.js';
import { fountainParams, FountainEncoder, FountainDecoder, _internals } from '../src/core/fountain.js';
import { writePacketHeader, sealPacket, encodeMeta, encodeFrameCells, parseDescriptor, PKT_HDR } from '../src/core/frame.js';
import { Sender, Receiver, fountainSeed, blockPlan } from '../src/core/transfer.js';

const PATH = new URL('./vectors/test-vectors.json', import.meta.url);
const hex = (u8) => Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength).toString('hex');
const unhex = (s) => new Uint8Array(Buffer.from(s, 'hex'));
const utf8 = (s) => new TextEncoder().encode(s);
const bytesOf = (n, seed) => { const r = new Rng(seed); return new Uint8Array(n).map(() => r.next() & 255); };
const crcOfInts = (arr) => { const b = new Uint8Array(arr.length * 4); const dv = new DataView(b.buffer); arr.forEach((v, i) => dv.setUint32(i * 4, v >>> 0, true)); return crc32(b); };
const bitsStr = (bits) => Array.from(bits).join('');
const check = (cond, what) => { if (!cond) throw new Error(`vector generator self-check failed: ${what}`); };

const V = {
  format: 'lumen-code test vectors',
  version: 2,
  spec: 'Lumen Code v1, draft 2 (docs/PROTOCOL.md)',
  notes: {
    hex: 'byte strings are lowercase hex',
    bits: 'bit strings are 70 characters 0/1, header bit 0 first, as they appear in the cells of the copy (masked)',
    cells: 'frame cells: data cells hold the palette index 0..2^bpc-1; finder, corner, separator, marker and header cells hold 8 = ink, 9 = paper',
    crcOfInts: 'a *Crc32 of an integer list is the CRC-32 of the list as little-endian u32 values',
    decode: 'an entry with "error" must be discarded; the name of the defect is informative (implementations need not report it)',
    streams: 'push = action of PROTOCOL §11.2 (discard, keep, add, accept, refuse) for the transfer key of the packet; finish = decode what can be decoded and, when every block is decoded, do the file check of §11.3 for the key of the last pushed packet; state = state of that key',
  },
};

// ---------------------------------------------------------------- primitives

V.crc32 = ['', '123456789', 'Lumen'].map((s) => ({ ascii: s, crc32: crc32(utf8(s)) }));
V.crc16 = ['', '123456789', 'Lumen'].map((s) => ({ ascii: s, crc16: crc16(utf8(s)) }));
V.mix32 = [0, 1, 0x51ed, 0xdeadbeef, 0xffffffff].map((x) => ({ x, out: mix32(x) }));
V.rng = [1, 0x12345678].map((seed) => { const r = new Rng(seed); return { seed, first8: Array.from({ length: 8 }, () => r.next()) }; });

// ---------------------------------------------------------------- Reed–Solomon

{
  const encode = [[16, 4, 1], [40, 191, 2], [32, 223, 3]].map(([nsym, k, seed]) => {
    const msg = bytesOf(k, seed);
    return { nsym, message: hex(msg), codeword: hex(rsEncode(msg, nsym, new Uint8Array(k + nsym))) };
  });
  // Damaged codewords inside the correction bound 2·errors + erasures ≤ nsym: the result is unique.
  const decode = [];
  const rng = new Rng(77);
  for (const [nsym, k, errors, erasures] of [[16, 4, 0, 0], [16, 4, 8, 0], [16, 4, 0, 16], [16, 4, 4, 8], [40, 191, 20, 0], [40, 191, 10, 20], [40, 191, 0, 40], [32, 223, 16, 0], [32, 223, 1, 30]]) {
    const cw = rsEncode(bytesOf(k, nsym * 1000 + k), nsym, new Uint8Array(k + nsym));
    const n = cw.length;
    const pos = Array.from({ length: n }, (_, i) => i);
    for (let i = n - 1; i > 0; i--) { const j = rng.next() % (i + 1); [pos[i], pos[j]] = [pos[j], pos[i]]; }
    const bad = cw.slice();
    const er = pos.slice(errors, errors + erasures).sort((a, b) => a - b);
    for (const p of pos.slice(0, errors)) bad[p] ^= 1 + (rng.next() % 255);
    for (const p of er) bad[p] = rng.next() & 255; // an erased byte can hold any value
    const fixed = bad.slice();
    check(rsDecode(fixed, nsym, er) >= 0 && hex(fixed) === hex(cw), `RS decode ${nsym} ${errors} ${erasures}`);
    decode.push({ nsym, errors, erasures: er, received: hex(bad), codeword: hex(cw) });
  }
  V.reedSolomon = { encode, decode };
}

// ---------------------------------------------------------------- grid

V.layouts = [[40, 40], [86, 152], [249, 153], [511, 40], [40, 511]].map(([X, Y]) => {
  const L = makeLayout(X, Y);
  check(dataCellCount(X, Y) === L.dataCells, `dataCellCount ${X}x${Y}`);
  return {
    X, Y, lx: L.lx, ly: L.ly, dataCells: L.dataCells,
    kindCrc32: crc32(L.kind), fixedCrc32: crc32(L.fixed),
    dataOrderFirst32: Array.from(L.dataOrder.subarray(0, 32)), dataOrderCrc32: crcOfInts(L.dataOrder),
    headerCells: L.headers.map((h) => [h[0], h[1], h[69]]), headerCellsCrc32: L.headers.map(crcOfInts),
    whitenFirst32: Array.from(L.whiten.subarray(0, 32)), whitenCrc32: crc32(L.whiten),
  };
});

// ---------------------------------------------------------------- headers

// Header bits with any field values (including invalid ones) and a correct CRC, masked for `copy`.
function rawHeader({ version = 1, mode, X, Y, n, nsym, seq, reserved = 0 }, copy, { badCrc = false } = {}) {
  const bits = new Uint8Array(70);
  let p = 0;
  const put = (v, w) => { for (let i = w - 1; i >= 0; i--) bits[p++] = (v >>> i) & 1; };
  put(version, 4); put(mode, 2); put(X, 9); put(Y, 9); put(n, 8); put(nsym >>> 1, 7); put(seq, 8); put(reserved, 7);
  put(crc16bits(bits, 54) ^ (badCrc ? 1 : 0), 16);
  for (let i = 0; i < 70; i++) bits[i] ^= HEADER_MASKS[copy][i];
  return bits;
}
{
  const encodeFields = [
    { mode: 1, X: 86, Y: 152, n: 236, nsym: 38, seq: 7 },
    { mode: 2, X: 249, Y: 153, n: 251, nsym: 40, seq: 255 },
    { mode: 0, X: 40, Y: 40, n: 120, nsym: 24, seq: 0 },
  ];
  const encode = encodeFields.map((h) => ({ fields: h, copies: [0, 1, 2, 3].map((c) => bitsStr(packHeader(h, c))) }));
  encode.forEach((e) => e.copies.forEach((b, c) => check(bitsStr(rawHeader(e.fields, c)) === b, 'rawHeader')));

  // smallest n with m = 1 on a 40 × 40 C2 grid: D = 1019 cells, so 8n ≤ 1019
  const base = { mode: 1, X: 86, Y: 152, n: 236, nsym: 38, seq: 7 };
  const cases = [
    ['valid, copy 0', base, 0],
    ['valid, copy 1', base, 1],
    ['valid, copy 2', { ...base, mode: 2, seq: 200 }, 2],
    ['valid, copy 3', { ...base, mode: 0, X: 511, Y: 511 }, 3],
    ['valid, reserved bits set (ignored)', { ...base, reserved: 0x55 }, 0],
    ['valid, T = 16', { ...base, n: 63, nsym: 40 }, 0],
    ['valid, m = 1', { mode: 0, X: 40, Y: 40, n: 127, nsym: 20, seq: 1 }, 1],
    ['valid, nsym = 2', { ...base, n: 255, nsym: 2 }, 2],
    ['crc', base, 0, { badCrc: true }],
    ['crc (copy 0 bits read as copy 1)', base, 0, { readAs: 1 }],
    ['version', { ...base, version: 2 }, 0],
    ['version', { ...base, version: 0 }, 1],
    ['mode', { ...base, mode: 3 }, 0],
    ['size', { ...base, X: 39 }, 0],
    ['size', { ...base, Y: 0 }, 3],
    ['nsym', { ...base, nsym: 0 }, 0],
    ['T', { ...base, n: 62, nsym: 40 }, 0],
    ['T', { ...base, n: 24, nsym: 22 }, 2],
    ['T', { ...base, n: 40, nsym: 40 }, 1],
    ['m', { mode: 0, X: 40, Y: 40, n: 128, nsym: 20, seq: 1 }, 0],
    ['m', { mode: 0, X: 40, Y: 40, n: 255, nsym: 40, seq: 1 }, 2],
  ];
  const decode = cases.map(([why, f, copy, opt = {}]) => {
    const bits = rawHeader(f, copy, opt);
    const readAs = opt.readAs ?? copy;
    const r = decodeHeader(bits, readAs);
    const valid = why.startsWith('valid');
    const { reserved, version, ...fields } = f;
    if (valid) check(r.header && JSON.stringify(r.header) === JSON.stringify(fields), why);
    else check(r.error === why.split(' ')[0], `${why}: got ${r.error}`);
    return valid ? { name: why, copy: readAs, bits: bitsStr(bits), header: r.header } : { name: why, copy: readAs, bits: bitsStr(bits), error: r.error };
  });
  V.headers = { encode, decode };
}

// ---------------------------------------------------------------- frames (encoder conformance)

// Frame cells as a function of (X, Y, mode, n, nsym, seq, packets) alone (§7, §15).
{
  const frame = (X, Y, mode, n, nsym, seq, seed, full) => {
    const L = makeLayout(X, Y);
    const m = codewordsPerFrame(L, mode, n);
    const packets = Array.from({ length: m }, (_, j) => sealPacket(bytesOf(n - nsym, seed * 1000 + j)));
    const cells = encodeFrameCells(L, { mode, n, nsym }, seq, packets.map((p) => p.slice()));
    const out = { X, Y, mode, n, nsym, seq, packets: packets.map(hex), cellsCrc32: crc32(cells) };
    if (full) out.cells = hex(cells);
    return out;
  };
  V.frames = [
    frame(48, 64, 1, 205, 32, 9, 1, true),
    frame(40, 40, 0, 120, 24, 0, 2, true),
    frame(86, 152, 2, 253, 40, 255, 3, false),
    frame(140, 240, 1, 255, 82, 100, 4, false),
  ];
}

// ---------------------------------------------------------------- packets and descriptors

function packet(tid, sbn, esi, meta, payload) {
  const msg = new Uint8Array(payload.length + 7);
  writePacketHeader(msg, tid, sbn, esi, meta);
  msg.set(payload, PKT_HDR);
  return sealPacket(msg);
}
// A descriptor body from raw parts (for invalid ones too).
function rawDescriptor({ version = 1, flags = 0, L, F, crc = 0, name = new Uint8Array(0), N = name.length, tail = [] }, T) {
  const d = new Uint8Array(T);
  const dv = new DataView(d.buffer);
  d[0] = version; d[1] = flags;
  dv.setUint32(2, L); dv.setUint32(6, F); dv.setUint32(10, crc >>> 0);
  d[14] = N;
  d.set(name.subarray(0, Math.min(name.length, T - 15)), 15);
  d.set(tail, 15 + Math.min(name.length, T - 15));
  return d;
}
{
  const pv = packet(0xbeef, 3, 0x1234, false, bytesOf(23, 4));
  V.packets = {
    encode: [
      { tid: 0xbeef, meta: false, sbn: 3, esi: 0x1234, payload: hex(bytesOf(23, 4)), bytes: hex(pv) },
      { tid: 0x0001, meta: true, sbn: 0, esi: 0, payload: hex(bytesOf(16, 5)), bytes: hex(packet(1, 0, 0, true, bytesOf(16, 5))) },
    ],
  };
  const bad = pv.slice(); bad[9] ^= 4;
  const short = packet(7, 0, 1, false, bytesOf(15, 6)); // T = 15
  for (const p of [bad, short, packet(1, 5, 0, true, bytesOf(16, 5)), packet(1, 0, 1, true, bytesOf(16, 5))]) check((await new Receiver().push(p)) === 'discard', 'packet discard');
  check((await new Receiver().push(pv)) === 'keep', 'packet keep');
  V.packets.decode = [
    { name: 'data packet', bytes: hex(pv), packet: { tid: 0xbeef, meta: false, sbn: 3, esi: 0x1234, T: 23 } },
    { name: 'data packet, T = 16', bytes: hex(packet(7, 127, 65535, false, bytesOf(16, 6))), packet: { tid: 7, meta: false, sbn: 127, esi: 65535, T: 16 } },
    { name: 'metadata packet 0', bytes: hex(packet(1, 0, 0, true, bytesOf(16, 5))), packet: { tid: 1, meta: true, sbn: 0, esi: 0, T: 16 } },
    { name: 'crc', bytes: hex(bad), error: 'crc' },
    { name: 'length (T = 15)', bytes: hex(short), error: 'length' },
    { name: 'metadata packet with SBN != 0', bytes: hex(packet(1, 5, 0, true, bytesOf(16, 5))), error: 'metadata-sbn' },
    { name: 'metadata packet with index 1 (reserved)', bytes: hex(packet(1, 0, 1, true, bytesOf(16, 5))), error: 'metadata-index' },
  ];

  const meta = { name: '测试 file.txt', fileSize: 123456, payloadLength: 65432, crc: 0xcafebabe, flags: 1 };
  const encode = [
    { ...meta, T: 40, bytes: hex(encodeMeta(meta, 40)) },
    { name: 'a-very-long-name-that-does-not-fit.bin', fileSize: 10, payloadLength: 10, crc: 1, flags: 0, T: 30, bytes: hex(encodeMeta({ name: 'a-very-long-name-that-does-not-fit.bin', fileSize: 10, payloadLength: 10, crc: 1, flags: 0 }, 30)) },
    { name: 'ééééé', fileSize: 0, payloadLength: 0, crc: 0, flags: 0, T: 24, bytes: hex(encodeMeta({ name: 'ééééé', fileSize: 0, payloadLength: 0, crc: 0, flags: 0 }, 24)) },
  ];
  // the last two are cut: at 15 bytes, and at a character boundary (é is 2 bytes; 9 bytes fit)
  check(parseDescriptor(unhex(encode[1].bytes)).descriptor.name === 'a-very-long-nam', 'cut name');
  check(parseDescriptor(unhex(encode[2].bytes)).descriptor.name === 'éééé', 'cut name at boundary');
  encode[1].decodedName = 'a-very-long-nam';
  encode[2].decodedName = 'éééé';

  const T = 40;
  const ok = { L: 1000, F: 1000, crc: 0x01020304, name: utf8('report.pdf') };
  const cases = [
    ['valid', ok],
    ['valid, DEFLATE, L != F', { ...ok, flags: 1, L: 300 }],
    ['valid, no name', { ...ok, name: new Uint8Array(0) }],
    ['valid, name fills the descriptor', { ...ok, name: utf8('n'.repeat(T - 15)) }],
    ['valid, byte-order mark kept in the name', { ...ok, name: utf8('﻿a') }],
    ['valid, unknown TLV records ignored', { ...ok, tail: [7, 3, 1, 2, 3, 200, 0, 9, 1, 0xff] }],
    ['valid, bytes after the end of the TLV list ignored', { ...ok, tail: [0, 5, 6, 7] }],
    ['valid, TLV record ends at the last byte', { ...ok, name: utf8('x'), tail: Array.from({ length: T - 16 }, (_, i) => (i === 0 ? 9 : i === 1 ? T - 18 : 0xaa)) }],
    ['valid, Z = 128', { ...ok, L: 128 * 16384 * T, F: 128 * 16384 * T }],
    ['version', { ...ok, version: 2 }],
    ['flags', { ...ok, flags: 2 }],
    ['flags', { ...ok, flags: 0x81 }],
    ['name-length', { ...ok, N: T - 14 }],
    ['name-utf8', { ...ok, name: Uint8Array.of(0x61, 0xc3, 0x28) }],
    ['name-utf8', { ...ok, name: Uint8Array.of(0xc0, 0xaf) }],
    ['name-utf8', { ...ok, name: Uint8Array.of(0xed, 0xa0, 0x80) }],
    ['name-utf8', { ...ok, name: Uint8Array.of(0x61, 0xe6, 0xb5) }],
    ['tlv', { ...ok, name: utf8('x'), tail: Array.from({ length: T - 16 }, (_, i) => (i === T - 17 ? 9 : 0xaa)).map((v, i) => (i === 0 ? 9 : i === 1 ? T - 19 : v)) }],
    ['tlv', { ...ok, tail: [9, 200] }],
    ['length', { ...ok, L: 999 }],
    ['blocks', { ...ok, L: 128 * 16384 * T + 1, F: 128 * 16384 * T + 1 }],
  ];
  const decode = cases.map(([why, f]) => {
    const body = rawDescriptor(f, T);
    const r = parseDescriptor(body);
    if (why.startsWith('valid')) {
      check(r.descriptor, `${why}: ${r.error}`);
      const d = r.descriptor;
      return { name: why, T, bytes: hex(body), descriptor: { flags: d.flags, payloadLength: d.payloadLength, fileSize: d.fileSize, crc: d.crc, name: d.name, records: d.records.map((x) => ({ type: x.type, value: hex(x.value) })) } };
    }
    check(r.error === why, `${why}: got ${r.error}`);
    return { name: why, T, bytes: hex(body), error: r.error };
  });
  V.descriptors = { encode, decode };
}

// ---------------------------------------------------------------- fountain code

V.fountainParams = [1, 2, 10, 100, 1000, 16384].map((K) => ({ K, ...fountainParams(K) }));
V.fountainSeed = [[0, 0], [1, 0], [0x1234, 0], [0x1234, 1], [0xffff, 127]].map(([tid, sbn]) => ({ tid, sbn, seed: fountainSeed(tid, sbn) }));
{
  const seed = fountainSeed(0x1234, 0);
  const K = 10;
  const p = fountainParams(K);
  V.fountainTuples = { K, seed, tuples: [0, 1, 2, 99, 65535].map((esi) => ({ esi, ..._internals.tuple(p, seed, esi) })) };
  V.fountainRows = { K: 100, seed, ldpcRow0: _internals.ldpcRows(100, fountainParams(100).S)[0], piRow0Crc32: crcOfInts(_internals.hdpcMembers(100, 0, seed)), piRow0First16: _internals.hdpcMembers(100, 0, seed).slice(0, 16) };
  const T = 16;
  const payload = bytesOf(T * 7 - 3, 9);
  const enc = new FountainEncoder(payload, T, seed);
  V.fountainSymbols = { T, seed, payload: hex(payload), symbols: [0, 1, 5, 6, 7, 40, 65535].map((esi) => ({ esi, data: hex(enc.encode(esi, new Uint8Array(T))) })) };
}
// Decodability depends only on the set of symbols (§9.7). For each case: the smallest prefix of
// a random ESI list that decodes, the prefix one shorter (which does not), and the block.
{
  const T = 16;
  const decode = [];
  for (const [K, tid, sbn, rseed] of [[1, 3, 0, 1], [2, 3, 1, 2], [10, 0x1234, 0, 3], [40, 0xabcd, 5, 4], [100, 0x0042, 0, 5], [100, 0x0042, 0, 6]]) {
    const seed = fountainSeed(tid, sbn);
    const block = bytesOf(K * T, rseed * 31);
    const enc = new FountainEncoder(block, T, seed);
    const rng = new Rng(rseed);
    const esis = [];
    const used = new Set();
    let ok = null;
    while (!ok) {
      let e;
      do e = rng.next() & 0xffff; while (used.has(e));
      used.add(e);
      esis.push(e);
      if (esis.length < K) continue;
      const dec = new FountainDecoder(K, T, seed);
      for (const x of esis) dec.add(x, enc.encode(x, new Uint8Array(T)));
      ok = dec.decode();
    }
    check(hex(ok) === hex(block), 'fountain decode');
    const sym = (x) => ({ esi: x, data: hex(enc.encode(x, new Uint8Array(T))) });
    decode.push({ K, T, tid, sbn, seed, block: hex(block), symbols: esis.map(sym), decodable: true });
    if (esis.length > K) decode.push({ K, T, tid, sbn, seed, block: hex(block), symbols: esis.slice(0, -1).map(sym), decodable: false });
  }
  V.fountainDecode = decode;
}

V.blockPlans = [[0, 16], [1, 200], [3276800, 200], [3276801, 200], [5000000, 191], [128 * 16384 * 16, 16]].map(([L, T]) => ({ payloadLength: L, T, ...blockPlan(L, T) }));

// ---------------------------------------------------------------- packet streams (receiver state machine)

// Deterministic DEFLATE for the vectors (platform zlib output can change between versions):
// stored blocks, and one fixed-Huffman block with literals and distance-1 runs.
function deflateStored(data) {
  const out = [];
  for (let i = 0; i === 0 || i < data.length; i += 65535) {
    const n = Math.min(65535, data.length - i);
    out.push(i + 65535 >= data.length ? 1 : 0, n & 255, n >> 8, ~n & 255, (~n >> 8) & 255, ...data.subarray(i, i + n));
  }
  return Uint8Array.from(out);
}
function deflateFixed(data) {
  const out = [];
  let acc = 0;
  let nb = 0;
  const put = (v, n) => { acc |= v << nb; nb += n; while (nb >= 8) { out.push(acc & 255); acc >>>= 8; nb -= 8; } };
  const rev = (c, n) => { let r = 0; for (let i = 0; i < n; i++) r |= ((c >> i) & 1) << (n - 1 - i); put(r, n); };
  const lit = (s) => (s < 144 ? rev(0x30 + s, 8) : s < 256 ? rev(0x190 + s - 144, 9) : s < 280 ? rev(s - 256, 7) : rev(0xc0 + s - 280, 8));
  put(1, 1); put(1, 2);
  for (let i = 0; i < data.length; ) {
    let run = 0;
    while (i > 0 && run < 10 && i + run < data.length && data[i + run] === data[i - 1]) run++;
    if (run >= 3) { lit(257 + run - 3); rev(0, 5); i += run; } // lengths 3..10, distance 1
    else lit(data[i++]);
  }
  lit(256);
  if (nb) out.push(acc & 255);
  return Uint8Array.from(out);
}

// Packets of one transfer with T = 24 (one source block).
function transfer(tid, file, { name = 'f.txt', payload = file, flags = 0, F = file.length, crc = crc32(file), T = 24 } = {}) {
  const plan = blockPlan(payload.length, T);
  check(plan.Z === 1, 'one block');
  const padded = new Uint8Array(plan.Kt * T);
  padded.set(payload);
  const enc = new FountainEncoder(padded, T, fountainSeed(tid, 0));
  const desc = rawDescriptor({ flags, L: payload.length, F, crc, name: utf8(name) }, T);
  return {
    K: plan.Kt,
    desc: hex(packet(tid, 0, 0, true, desc)),
    data: (esi, sbn = 0) => hex(packet(tid, sbn, esi, false, enc.encode(esi, new Uint8Array(T)))),
    raw: (esi, bytes) => hex(packet(tid, 0, esi, false, bytes)),
    descWith: (d) => hex(packet(tid, 0, 0, true, rawDescriptor({ L: payload.length, F, crc, name: utf8(name), flags, ...d }, T))),
  };
}
// Data packets ESI = from, from + 1, … until the block decodes (checked with a decoder).
function enough(tr, tid, from, T = 24) {
  const dec = new FountainDecoder(tr.K, T, fountainSeed(tid, 0));
  const out = [];
  for (let esi = from; ; esi++) {
    const h = tr.data(esi);
    out.push(h);
    dec.add(esi, unhex(h).subarray(5, 5 + T));
    if (dec.count >= tr.K && dec.decode()) return out;
  }
}
async function runStream(steps, limits) {
  const rx = new Receiver(limits);
  for (const s of steps) {
    if (s.push) {
      const got = await rx.push(unhex(s.push));
      check(got === s.result, `push: want ${s.result}, got ${got}`);
    } else if ('finish' in s) {
      const got = await rx.tryFinish();
      const norm = got && (got.bytes ? { file: hex(got.bytes), name: got.name } : { error: 'check' });
      check(JSON.stringify(norm) === JSON.stringify(s.finish), `finish: want ${JSON.stringify(s.finish)}, got ${JSON.stringify(norm)}`);
    } else if (s.state) {
      const p = rx.progress();
      check((p ? p.state : 'new') === s.state, `state: want ${s.state}, got ${p && p.state}`);
    }
  }
}
const pushes = (list, result) => list.map((p) => ({ push: p, result }));
{
  const text = utf8('Lumen Code stream vector. '.repeat(6));
  const other = utf8('A different file with the same transfer key. '.repeat(3));
  const streams = [];
  const add = async (name, steps, limits) => {
    await runStream(steps, limits);
    streams.push(limits ? { name, limits, steps } : { name, steps });
  };

  {
    const tr = transfer(0x0101, text);
    const d = enough(tr, 0x0101, 0);
    await add('descriptor first', [{ state: 'new' }, { push: tr.desc, result: 'accept' }, { state: 'receiving' }, { finish: null }, ...pushes(d, 'add'), { finish: { file: hex(text), name: 'f.txt' } }, { state: 'complete' }, { push: tr.desc, result: 'discard' }, { push: tr.data(999), result: 'discard' }, { finish: null }]);
  }
  {
    const tr = transfer(0x0102, text);
    const d = enough(tr, 0x0102, 100);
    await add('data before the descriptor', [...pushes(d, 'keep'), { state: 'collecting' }, { finish: null }, { push: tr.data(100), result: 'discard' }, { push: tr.desc, result: 'accept' }, { finish: { file: hex(text), name: 'f.txt' } }]);
  }
  {
    const tr = transfer(0x0103, text);
    const d = enough(tr, 0x0103, 0);
    const badCrc = unhex(d[0]); badCrc[8] ^= 1;
    await add('discarded packets', [
      { push: hex(badCrc), result: 'discard' },
      { state: 'new' },
      { push: hex(packet(0x0103, 1, 0, true, unhex(tr.desc).subarray(5, 29))), result: 'discard' },
      { push: hex(packet(0x0103, 0, 1, true, unhex(tr.desc).subarray(5, 29))), result: 'discard' },
      { push: tr.descWith({ flags: 2 }), result: 'discard' },
      { push: tr.descWith({ version: 2 }), result: 'discard' },
      { state: 'new' },
      { push: tr.desc, result: 'accept' },
      { push: d[0], result: 'add' },
      { push: d[0], result: 'discard' },
      { push: tr.data(5, 1), result: 'discard' },
      ...pushes(d.slice(1), 'add'),
      { finish: { file: hex(text), name: 'f.txt' } },
    ]);
  }
  {
    // a wrong symbol with a correct packet CRC: the check fails, the key is reset, and the
    // transfer completes from new packets
    const tr = transfer(0x0104, text);
    const d = enough(tr, 0x0104, 0);
    const wrong = unhex(d[0]).subarray(5, 29).slice(); wrong[3] ^= 0x40;
    const d2 = enough(tr, 0x0104, 500);
    await add('wrong symbol: check fails, transfer starts again', [
      { push: tr.desc, result: 'accept' },
      { push: tr.raw(0, wrong), result: 'add' },
      ...pushes(d.slice(1), 'add'),
      { finish: { error: 'check' } },
      { state: 'collecting' },
      ...pushes(d2, 'keep'),
      { push: tr.desc, result: 'accept' },
      { finish: { file: hex(text), name: 'f.txt' } },
    ]);
  }
  {
    const a = transfer(0x0105, text, { name: 'a.txt' });
    const b = transfer(0x0105, other, { name: 'b.txt' });
    const da = enough(a, 0x0105, 0);
    const db = enough(b, 0x0105, 0);
    await add('different descriptor for the same key', [
      { push: a.desc, result: 'accept' },
      ...pushes(da.slice(0, 3), 'add'),
      { push: b.desc, result: 'accept' },
      { state: 'receiving' },
      ...pushes(db, 'add'),
      { finish: { file: hex(other), name: 'b.txt' } },
      { push: a.desc, result: 'accept' },
      ...pushes(da, 'add'),
      { finish: { file: hex(text), name: 'a.txt' } },
    ]);
  }
  {
    const tr = transfer(0x0106, text);
    await add('refused: larger than the receiver limit', [{ push: tr.data(1), result: 'keep' }, { push: tr.desc, result: 'refuse' }, { state: 'refused' }, { push: tr.data(2), result: 'discard' }, { finish: null }], { maxSize: 100 });
  }
  {
    const tr = transfer(0x0107, text, { name: 'z.txt', payload: deflateFixed(text), flags: 1 });
    await add('limit on kept packets', [...pushes([tr.data(0), tr.data(1), tr.data(2)], 'keep'), { push: tr.data(3), result: 'discard' }, { push: tr.desc, result: 'accept' }, { push: tr.data(3), result: 'add' }], { maxKept: 3 });
  }
  {
    const t1 = transfer(0x0201, text);
    const t2 = transfer(0x0202, text);
    const t3 = transfer(0x0203, text);
    await add('limit on transfer keys: the least recent key is reset', [
      { push: t1.data(0), result: 'keep' },
      { push: t2.data(0), result: 'keep' },
      { push: t1.data(1), result: 'keep' },
      { push: t3.data(0), result: 'keep' },
      { push: t2.data(0), result: 'keep' },
      { push: t1.data(1), result: 'keep' },
    ], { maxTransfers: 2 });
  }
  for (const [name, payload, F, ok] of [
    ['DEFLATE, stored block', deflateStored(text), text.length, true],
    ['DEFLATE, fixed Huffman block', deflateFixed(text), text.length, true],
    ['DEFLATE, output longer than the file size', deflateFixed(text), text.length - 1, false],
    ['DEFLATE, byte after the final block', Uint8Array.from([...deflateFixed(text), 0]), text.length, false],
    ['DEFLATE, stream ends before the final block', deflateFixed(text).subarray(0, 20), text.length, false],
    ['DEFLATE, block type 3', Uint8Array.from([0x07, 0, 0, 0]), text.length, false],
  ]) {
    const tid = 0x0300 + streams.length;
    const tr = transfer(tid, text, { name: 'z.txt', payload, flags: 1, F });
    await add(name, [{ push: tr.desc, result: 'accept' }, ...pushes(enough(tr, tid, 0), 'add'), { finish: ok ? { file: hex(text), name: 'z.txt' } : { error: 'check' } }]);
  }
  {
    const tr = transfer(0x0401, new Uint8Array(0), { name: '' });
    await add('empty file, empty name', [{ push: tr.desc, result: 'accept' }, ...pushes(enough(tr, 0x0401, 7), 'add'), { finish: { file: '', name: '' } }]);
  }
  {
    const tr = transfer(0x0402, text, { crc: crc32(text) ^ 1 });
    await add('wrong CRC-32 in the descriptor', [{ push: tr.desc, result: 'accept' }, ...pushes(enough(tr, 0x0402, 0), 'add'), { finish: { error: 'check' } }, { state: 'collecting' }]);
  }
  V.streams = streams;
}

// ---------------------------------------------------------------- reference sender (informative)

V.referenceSender = {
  note: 'the choices of the reference sender (PROTOCOL §10.2, §10.3); not part of conformance',
  codewordChoice: [[86, 152, 1, 1], [249, 153, 2, 1], [40, 40, 0, 2], [140, 240, 1, 0], [140, 240, 1, 3]].map(([X, Y, mode, ecc]) => {
    const L = makeLayout(X, Y); const c = chooseCodeword(L, mode, ecc);
    return { X, Y, mode, eccLevel: ecc, n: c.n, nsym: c.nsym, cellsPerCodeword: cellsPerCodeword(c.n, [1, 2, 3][mode]), codewordsPerFrame: codewordsPerFrame(L, mode, c.n) };
  }),
};
{
  const file = utf8('Lumen Code v1 test frame. '.repeat(20));
  const prepared = { payload: file, meta: { name: 'frame.txt', fileSize: file.length, crc: crc32(file), flags: 0, payloadLength: file.length } };
  const tx = new Sender(prepared, { mode: 1, ecc: 1, X: 48, Y: 64, tid: 0x0102 });
  tx.seq = 9;
  const f0 = tx.nextFrame();
  const f1 = tx.nextFrame();
  V.referenceSender.frames = {
    file: hex(file), name: 'frame.txt', tid: 0x0102, mode: 1, eccLevel: 1, X: 48, Y: 64, n: tx.n, nsym: tx.nsym,
    frame0: { seq: 9, cellsCrc32: crc32(f0.cells) },
    frame1: { seq: 10, cellsCrc32: crc32(f1.cells) },
  };
  // the schedule of a larger transfer: which codeword of each frame holds the descriptor
  const big = bytesOf(40000, 11);
  const tx2 = new Sender({ payload: big, meta: { name: 'big.bin', fileSize: big.length, crc: crc32(big), flags: 0, payloadLength: big.length } }, { mode: 1, ecc: 1, X: 100, Y: 180, tid: 0x0303 });
  const where = [];
  const firstEsis = [];
  for (let f = 0; f < 12; f++) {
    const pk = tx2.nextPackets();
    where.push(pk.findIndex((p) => p[2] & 0x80));
    if (f === 0) firstEsis.push(...pk.slice(0, 6).map((p) => [p[2] & 0x7f, (p[3] << 8) | p[4]]));
  }
  V.referenceSender.schedule = { X: 100, Y: 180, mode: 1, eccLevel: 1, K: tx2.K, blocks: tx2.plan.Z, codewordsPerFrame: tx2.perFrame, descriptorCodeword: where, frame0FirstSbnEsi: firstEsis };
}

const json = JSON.stringify(V, null, 1) + '\n';
if (process.argv.includes('--check')) {
  const old = readFileSync(PATH, 'utf8');
  if (old !== json) {
    console.log('test/vectors/test-vectors.json is out of date: run node test/vectors.js');
    process.exit(1);
  }
  console.log('vectors: file matches the implementation');
} else {
  writeFileSync(PATH, json);
  console.log('wrote test/vectors/test-vectors.json', Object.keys(V).join(', '), `(${(json.length / 1024).toFixed(0)} KB)`);
}
