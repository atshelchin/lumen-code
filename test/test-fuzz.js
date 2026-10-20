// Property and fuzz tests: parsers and the receiver against random and hostile input.
//   node test/test-fuzz.js            quick run (part of npm test)
//   FUZZ=20 node test/test-fuzz.js    20 times more cases
import { crc32, Rng } from '../src/core/util.js';
import { rsEncode, rsDecode } from '../src/core/rs.js';
import { makeLayout, latticeAxis, dataCellCount, decodeHeader, packHeader, MODES, cellsPerCodeword } from '../src/core/layout.js';
import { parseDescriptor, encodeMeta, writePacketHeader, sealPacket, PKT_HDR } from '../src/core/frame.js';
import { inflate } from '../src/core/inflate.js';
import { FountainEncoder } from '../src/core/fountain.js';
import { Receiver, fountainSeed, blockPlan, RECEIVER_LIMITS } from '../src/core/transfer.js';
import { headerCorpus, descriptorCorpus, inflateCorpus, fountainDigest } from './corpus.js';

const SCALE = Number(process.env.FUZZ || 1);
const N = (n) => Math.round(n * SCALE);
let failures = 0;
const expect = (cond, what) => {
  if (!cond) {
    failures++;
    if (failures < 20) console.log('FAIL', what);
  }
};
const section = (name, fn) => {
  const t0 = performance.now();
  const before = failures;
  return Promise.resolve(fn()).then((info) => console.log(`${name.padEnd(34)} ${failures === before ? 'ok' : 'FAILED'}  ${info || ''} (${((performance.now() - t0) / 1000).toFixed(1)} s)`));
};

// ---- grid: kinds never overlap, for every size (§4.1)
await section('grid, all 472 sizes per axis', () => {
  // Markers sit on the lattice of each axis; the patterns, separators and header copies are
  // within 18 cells of a corner. So it is enough that every inner marker stays clear of the
  // header copies on its axis, and that lattice points are far apart.
  for (let D = 40; D <= 511; D++) {
    const L = latticeAxis(D);
    expect(L[0] === 3 && L[L.length - 1] === D - 4, `lattice ends ${D}`);
    for (let i = 1; i < L.length; i++) expect(L[i] - L[i - 1] >= 16 && L[i] - L[i - 1] <= 24, `lattice gap ${D}: ${L}`);
    // header copies A and C span x = 8 … 17 and X − 18 … X − 9 (B and D the same in y)
    for (let i = 1; i < L.length - 1; i++) expect(L[i] - 1 >= 18 && L[i] + 1 <= D - 19, `inner marker near a corner ${D}: ${L}`);
  }
  // ... and the closed form of D agrees with real layouts (it counts each kind once)
  const r = new Rng(5);
  const sizes = [[40, 40], [40, 511], [511, 40], [511, 511], [41, 57], [57, 41]];
  for (let i = 0; i < N(60); i++) sizes.push([40 + (r.next() % 472), 40 + (r.next() % 472)]);
  for (const [X, Y] of sizes) {
    const L = makeLayout(X, Y);
    const count = [0, 0, 0, 0];
    for (const k of L.kind) count[k]++;
    const markers = (L.lx.length * L.ly.length - 4) * 9;
    expect(count[1] === 4 * 49 + 4 * 15 && count[2] === markers && count[3] === 280, `kind counts ${X}x${Y}`);
    expect(L.dataCells === dataCellCount(X, Y), `data cells ${X}x${Y}`);
  }
  return `${sizes.length} full layouts`;
});

// ---- headers (§6.3)
await section('headers, random fields', () => {
  let valid = 0;
  for (const { bits, copy } of headerCorpus(1001, N(40000))) {
    const r = decodeHeader(bits, copy);
    if (!r.header) continue;
    valid++;
    const h = r.header;
    expect(h.mode <= 2 && h.X >= 40 && h.Y >= 40 && h.nsym >= 2 && h.n - h.nsym >= 23, `valid header ${JSON.stringify(h)}`);
    expect(Math.floor(dataCellCount(h.X, h.Y) / cellsPerCodeword(h.n, MODES[h.mode].bpc)) >= 1, `m >= 1 ${JSON.stringify(h)}`);
    // the sender makes the same bits again (with the reserved bits 0, the CRC changes too)
    const again = decodeHeader(packHeader(h, copy), copy).header;
    expect(again && JSON.stringify(again) === JSON.stringify(h), 'header round trip');
  }
  return `${valid} valid`;
});

// ---- descriptors (§8.4)
await section('descriptors, random bytes', () => {
  let valid = 0;
  for (const d of descriptorCorpus(1002, N(40000))) {
    const r = parseDescriptor(d);
    if (!r.descriptor) continue;
    valid++;
    const x = r.descriptor;
    const T = d.length;
    const nameBytes = new TextEncoder().encode(x.name);
    expect(nameBytes.length === d[14] && nameBytes.length <= T - 15, 'name length');
    expect((x.flags & ~1) === 0 && (x.flags & 1 || x.payloadLength === x.fileSize), 'flags and length');
    expect(blockPlan(x.payloadLength, T).Z <= 128, 'block count');
    let end = 15 + nameBytes.length;
    for (const rec of x.records) end += 2 + rec.value.length;
    expect(end <= T, 'records inside the descriptor');
    // encoding the parsed fields gives a descriptor that parses to the same fields
    const back = parseDescriptor(encodeMeta(x, T)).descriptor;
    expect(back && back.name === x.name && back.flags === x.flags && back.payloadLength === x.payloadLength && back.fileSize === x.fileSize && back.crc === x.crc, 'descriptor round trip');
  }
  return `${valid} valid`;
});

// ---- Reed–Solomon: always corrects inside the bound, never returns a non-codeword outside it
await section('Reed–Solomon, random damage', () => {
  const r = new Rng(1003);
  let inside = 0;
  let outsideAccepted = 0;
  for (let i = 0; i < N(3000); i++) {
    const nsym = 2 * (1 + (r.next() % 30));
    const k = 1 + (r.next() % (255 - nsym));
    const msg = new Uint8Array(k).map(() => r.next() & 255);
    const cw = rsEncode(msg, nsym, new Uint8Array(k + nsym));
    const n = cw.length;
    const within = r.next() % 2 === 0;
    const budget = within ? nsym : nsym + 1 + (r.next() % nsym);
    const f = Math.min(n, r.next() % (budget + 1));
    const e = Math.max(0, Math.min(n - f, Math.floor((budget - f) / 2) + (within ? 0 : 1)));
    const pos = Array.from({ length: n }, (_, j) => j);
    for (let j = n - 1; j > 0; j--) { const t = r.next() % (j + 1); [pos[j], pos[t]] = [pos[t], pos[j]]; }
    const bad = cw.slice();
    for (const p of pos.slice(0, e)) bad[p] ^= 1 + (r.next() % 255);
    const er = pos.slice(e, e + Math.min(f, n - e));
    for (const p of er) bad[p] = r.next() & 255;
    const res = rsDecode(bad, nsym, er);
    if (2 * e + er.length <= nsym) {
      inside++;
      expect(res >= 0 && bad.every((v, j) => v === cw[j]), `RS inside the bound n=${n} nsym=${nsym} e=${e} f=${er.length}`);
    } else if (res >= 0) {
      outsideAccepted++;
      const re = rsEncode(bad.subarray(0, k), nsym, new Uint8Array(n));
      expect(re.every((v, j) => v === bad[j]), 'RS result outside the bound is a codeword');
    }
  }
  return `${inside} inside the bound, ${outsideAccepted} miscorrections outside it (expected: a few)`;
});

// ---- inflate: hostile streams never throw and never exceed the limit; zlib streams round trip
await section('inflate, mutated and zlib streams', async () => {
  const text = new TextEncoder().encode('Lumen Code moves a file from a screen to a camera. '.repeat(40));
  const z = new Uint8Array(await new Response(new Blob([text]).stream().pipeThrough(new CompressionStream('deflate-raw'))).arrayBuffer());
  let ok = 0;
  for (const { data, limit } of inflateCorpus(1004, N(10000), [z], text.length)) {
    let out;
    try {
      out = inflate(data, limit);
    } catch (e) {
      expect(false, `inflate threw ${e}`);
      continue;
    }
    if (out) ok++;
    expect(!out || out.length <= limit, 'inflate output within the limit');
  }
  const r = new Rng(1005);
  let rand = 0;
  for (let i = 0; i < N(20000); i++) {
    const data = new Uint8Array(1 + (r.next() % 64)).map(() => r.next() & 255);
    const out = inflate(data, 1000);
    if (out) rand++;
    expect(!out || out.length <= 1000, 'random stream within the limit');
  }
  // zlib (dynamic Huffman, all levels) round trip on random text
  for (let i = 0; i < N(40); i++) {
    const words = ['lumen ', 'code ', 'frame ', 'camera ', '\n', 'fountain ', '0123 '];
    const t = new TextEncoder().encode(Array.from({ length: 50 + (r.next() % 2000) }, () => words[r.next() % words.length]).join(''));
    const zz = new Uint8Array(await new Response(new Blob([t]).stream().pipeThrough(new CompressionStream('deflate-raw'))).arrayBuffer());
    const out = inflate(zz, t.length);
    expect(out && crc32(out) === crc32(t) && out.length === t.length, 'zlib round trip');
    expect(!inflate(zz, t.length - 1), 'zlib stream over the limit');
  }
  return `${ok} mutated streams still decode, ${rand} of ${N(20000)} random byte strings decode`;
});

// ---- fountain: a decoder never returns a wrong block
await section('fountain, random symbol sets', () => {
  fountainDigest(1006, N(1500)); // throws on a wrong block
});

// ---- receiver: hostile packet streams (§11, §13)
await section('receiver, hostile streams', async () => {
  const r = new Rng(1007);
  const T = 24;
  const limits = { maxSize: 5000, maxKept: 50, maxTransfers: 3 };
  let delivered = 0;
  let wrong = 0;
  for (let run = 0; run < N(60); run++) {
    // three real transfers, two with the same TID (collision), one over the size limit
    const files = [];
    const tids = [r.next() & 0xffff, r.next() & 0xffff];
    for (let j = 0; j < 3; j++) {
      const size = j === 2 && run % 3 === 0 ? 6000 : 1 + (r.next() % 2000);
      const file = new Uint8Array(size).map(() => r.next() & 255);
      const tid = tids[j === 2 ? 0 : j];
      const plan = blockPlan(size, T);
      const padded = new Uint8Array(plan.Kt * T);
      padded.set(file);
      files.push({ tid, file, plan, enc: new FountainEncoder(padded, T, fountainSeed(tid, 0)), desc: encodeMeta({ name: `f${j}`, fileSize: size, payloadLength: size, crc: crc32(file), flags: 0 }, T) });
    }
    const valid = new Set(files.map((f) => crc32(f.file)));
    const rx = new Receiver(limits);
    for (let i = 0; i < 1500; i++) {
      const f = files[r.next() % 3];
      const kind = r.next() % 16;
      const msg = new Uint8Array(T + 7);
      if (kind === 0) {
        msg.forEach((_, j) => (msg[j] = r.next() & 255)); // garbage, CRC almost always wrong
      } else if (kind === 1) {
        writePacketHeader(msg, f.tid, r.next() % 128, r.next() & 0xffff, true); // reserved metadata
        sealPacket(msg);
      } else if (kind === 2) {
        writePacketHeader(msg, f.tid, 0, r.next() & 0xffff, false); // wrong symbol, correct CRC
        msg.forEach((_, j) => j >= PKT_HDR && j < T + PKT_HDR && (msg[j] = r.next() & 255));
        sealPacket(msg);
      } else if (kind === 3) {
        writePacketHeader(msg, f.tid, 0, 0, true); // random descriptor with a correct CRC
        msg.set(f.desc, PKT_HDR);
        msg[PKT_HDR + 2 + (r.next() % 12)] ^= 1 << (r.next() % 8);
        sealPacket(msg);
      } else if (kind < 7) {
        writePacketHeader(msg, f.tid, 0, 0, true);
        msg.set(f.desc, PKT_HDR);
        sealPacket(msg);
      } else {
        const esi = r.next() % (f.plan.Kt * 3);
        writePacketHeader(msg, f.tid, r.next() % 16 === 0 ? 1 : 0, esi, false);
        f.enc.encode(esi, msg.subarray(PKT_HDR, PKT_HDR + T));
        sealPacket(msg);
      }
      let out;
      try {
        await rx.push(msg);
        out = await rx.tryFinish();
      } catch (e) {
        expect(false, `receiver threw ${e.stack}`);
        break;
      }
      expect(rx.transfers.size <= limits.maxTransfers, 'transfer limit');
      for (const t of rx.transfers.values()) expect(t.kept.size <= limits.maxKept, 'kept limit');
      if (out && out.bytes) {
        delivered++;
        if (!valid.has(crc32(out.bytes))) wrong++;
        const src = files.find((x) => crc32(x.file) === crc32(out.bytes));
        expect(src && src.file.length <= limits.maxSize && out.bytes.every((v, j) => v === src.file[j]), 'delivered file is a real file under the limit');
      }
    }
  }
  expect(wrong === 0, `${wrong} wrong files delivered`);
  return `${delivered} files delivered, ${wrong} wrong`;
});

if (failures) {
  console.log(`fuzz: ${failures} failures`);
  process.exit(1);
}
console.log('fuzz: all passed');
