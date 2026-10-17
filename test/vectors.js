// Generates test/vectors/test-vectors.json from the JS reference implementation.
// Every conforming implementation must reproduce these values exactly.
import { writeFileSync } from 'node:fs';
import { crc32, crc16, mix32, Rng } from '../src/core/util.js';
import { rsEncode } from '../src/core/rs.js';
import { makeLayout, packHeader, chooseCodeword, codewordsPerFrame, cellsPerCodeword, K_DATA } from '../src/core/layout.js';
import { fountainParams, FountainEncoder, _internals } from '../src/core/fountain.js';
import { writePacketHeader, sealPacket, encodeMeta, encodeFrameCells, PKT_HDR } from '../src/core/frame.js';
import { Sender, fountainSeed, blockPlan } from '../src/core/transfer.js';

const hex = (u8) => Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength).toString('hex');
const bytesOf = (n, seed) => { const r = new Rng(seed); return new Uint8Array(n).map(() => r.next() & 255); };
const crcOfInts = (arr) => { const b = new Uint8Array(arr.length * 4); const dv = new DataView(b.buffer); arr.forEach((v, i) => dv.setUint32(i * 4, v >>> 0, true)); return crc32(b); };
const V = { format: 'lumen-code test vectors', version: 1 };

V.crc32 = ['', '123456789', 'Lumen'].map((s) => ({ ascii: s, crc32: crc32(new TextEncoder().encode(s)) }));
V.crc16 = ['', '123456789', 'Lumen'].map((s) => ({ ascii: s, crc16: crc16(new TextEncoder().encode(s)) }));
V.mix32 = [0, 1, 0x51ed, 0xdeadbeef, 0xffffffff].map((x) => ({ x, out: mix32(x) }));
V.rng = [1, 0x12345678].map((seed) => { const r = new Rng(seed); return { seed, first8: Array.from({ length: 8 }, () => r.next()) }; });

V.reedSolomon = [[16, 4, 1], [40, 191, 2], [255 - 0, 223, 3]].map(([nsym, k, seed]) => {
  const msg = bytesOf(k, seed);
  const cw = rsEncode(msg, nsym, new Uint8Array(k + nsym));
  return { nsym, message: hex(msg), codeword: hex(cw) };
}).map((v) => ({ ...v, nsym: v.nsym === 255 ? 32 : v.nsym }));
// fix the third vector to a real (223+32) code
{ const msg = bytesOf(223, 3); V.reedSolomon[2] = { nsym: 32, message: hex(msg), codeword: hex(rsEncode(msg, 32, new Uint8Array(255))) }; }

V.layouts = [[40, 40], [86, 152], [249, 153]].map(([X, Y]) => {
  const L = makeLayout(X, Y);
  return {
    X, Y, lx: L.lx, ly: L.ly, dataCells: L.dataCells,
    kindCrc32: crc32(L.kind), fixedCrc32: crc32(L.fixed),
    dataOrderFirst32: Array.from(L.dataOrder.subarray(0, 32)), dataOrderCrc32: crcOfInts(L.dataOrder),
    headerCells: L.headers.map((h) => [h[0], h[1], h[69]]), headerCellsCrc32: L.headers.map(crcOfInts),
    whitenFirst32: Array.from(L.whiten.subarray(0, 32)), whitenCrc32: crc32(L.whiten),
  };
});

V.headers = [{ mode: 1, X: 86, Y: 152, n: 236, nsym: 38, seq: 7 }, { mode: 2, X: 249, Y: 153, n: 251, nsym: 40, seq: 255 }, { mode: 0, X: 40, Y: 40, n: 150, nsym: 24, seq: 0 }].map((h) => ({
  fields: h, copies: [0, 1, 2, 3].map((c) => Array.from(packHeader(h, c)).join('')),
}));

V.codewordChoice = [[86, 152, 1, 1], [249, 153, 2, 1], [40, 40, 0, 2]].map(([X, Y, mode, ecc]) => {
  const L = makeLayout(X, Y); const c = chooseCodeword(L, mode, ecc);
  return { X, Y, mode, eccLevel: ecc, n: c.n, nsym: c.nsym, cellsPerCodeword: cellsPerCodeword(c.n, [1, 2, 3][mode]), codewordsPerFrame: codewordsPerFrame(L, mode, c.n) };
});

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

V.blockPlans = [[1, 200], [3276800, 200], [3276801, 200], [5000000, 191]].map(([L, T]) => ({ payloadLength: L, T, ...blockPlan(L, T) }));

{
  const msg = new Uint8Array(30);
  writePacketHeader(msg, 0xbeef, 3, 0x1234, false);
  msg.set(bytesOf(23, 4), PKT_HDR);
  sealPacket(msg);
  V.packet = { tid: 0xbeef, sbn: 3, esi: 0x1234, meta: false, bytes: hex(msg) };
  V.descriptor = { meta: { name: '测试 file.txt', fileSize: 123456, payloadLength: 65432, crc: 0xcafebabe, flags: 1 }, T: 40, bytes: hex(encodeMeta({ name: '测试 file.txt', fileSize: 123456, payloadLength: 65432, crc: 0xcafebabe, flags: 1 }, 40)) };
}

// a complete frame: raw payload, fixed transfer id and seq
{
  const file = new TextEncoder().encode('Lumen Code v1 test frame. '.repeat(20));
  const prepared = { payload: file, meta: { name: 'frame.txt', fileSize: file.length, crc: crc32(file), flags: 0, payloadLength: file.length } };
  const tx = new Sender(prepared, { mode: 1, ecc: 1, X: 48, Y: 64, tid: 0x0102 });
  tx.seq = 9;
  const f0 = tx.nextFrame();
  const f1 = tx.nextFrame();
  V.frames = {
    file: hex(file), name: 'frame.txt', tid: 0x0102, mode: 1, eccLevel: 1, X: 48, Y: 64, n: tx.n, nsym: tx.nsym,
    frame0: { seq: 9, cells: hex(f0.cells), cellsCrc32: crc32(f0.cells) },
    frame1: { seq: 10, cellsCrc32: crc32(f1.cells) },
    cellValues: 'data cells: palette index 0..2^bpc-1; finder, corner, separator, marker and header cells: 8 = ink, 9 = paper',
  };
}

writeFileSync('test/vectors/test-vectors.json', JSON.stringify(V, null, 1));
console.log('wrote test/vectors/test-vectors.json', Object.keys(V).join(', '));
