// Conformance: the JS implementation against test/vectors/test-vectors.json (PROTOCOL §15).
// The Rust implementation runs the same checks in src/rust/tests/vectors.rs.
import { readFileSync } from 'node:fs';
import { crc32, crc16, mix32, Rng } from '../src/core/util.js';
import { rsEncode, rsDecode } from '../src/core/rs.js';
import { makeLayout, packHeader, decodeHeader, chooseCodeword, codewordsPerFrame, cellsPerCodeword, dataCellCount } from '../src/core/layout.js';
import { fountainParams, FountainEncoder, FountainDecoder, _internals } from '../src/core/fountain.js';
import { writePacketHeader, sealPacket, encodeMeta, encodeFrameCells, parseDescriptor, readPacket, PKT_HDR } from '../src/core/frame.js';
import { Sender, Receiver, fountainSeed, blockPlan } from '../src/core/transfer.js';

const V = JSON.parse(readFileSync(new URL('./vectors/test-vectors.json', import.meta.url), 'utf8'));
const hex = (u8) => Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength).toString('hex');
const unhex = (s) => new Uint8Array(Buffer.from(s, 'hex'));
const crcOfInts = (arr) => { const b = new Uint8Array(arr.length * 4); const dv = new DataView(b.buffer); arr.forEach((v, i) => dv.setUint32(i * 4, v >>> 0, true)); return crc32(b); };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

let failures = 0;
let checks = 0;
const expect = (cond, what) => {
  checks++;
  if (!cond) {
    failures++;
    console.log('FAIL', what);
  }
};

// primitives
for (const e of V.crc32) expect(crc32(new TextEncoder().encode(e.ascii)) === e.crc32, `crc32 ${e.ascii}`);
for (const e of V.crc16) expect(crc16(new TextEncoder().encode(e.ascii)) === e.crc16, `crc16 ${e.ascii}`);
for (const e of V.mix32) expect(mix32(e.x) === e.out, `mix32 ${e.x}`);
for (const e of V.rng) { const r = new Rng(e.seed); expect(same(e.first8.map(() => r.next()), e.first8), `rng ${e.seed}`); }

// Reed–Solomon
for (const e of V.reedSolomon.encode) {
  const msg = unhex(e.message);
  expect(hex(rsEncode(msg, e.nsym, new Uint8Array(msg.length + e.nsym))) === e.codeword, `rs encode ${e.nsym}`);
}
for (const e of V.reedSolomon.decode) {
  const cw = unhex(e.received);
  expect(rsDecode(cw, e.nsym, e.erasures) >= 0 && hex(cw) === e.codeword, `rs decode nsym=${e.nsym} errors=${e.errors} erasures=${e.erasures.length}`);
}

// grid
for (const e of V.layouts) {
  const L = makeLayout(e.X, e.Y);
  const tag = `layout ${e.X}x${e.Y}`;
  expect(same(L.lx, e.lx) && same(L.ly, e.ly), `${tag} lattice`);
  expect(L.dataCells === e.dataCells && dataCellCount(e.X, e.Y) === e.dataCells, `${tag} data cells`);
  expect(crc32(L.kind) === e.kindCrc32 && crc32(L.fixed) === e.fixedCrc32, `${tag} kinds`);
  expect(same(Array.from(L.dataOrder.subarray(0, 32)), e.dataOrderFirst32) && crcOfInts(L.dataOrder) === e.dataOrderCrc32, `${tag} data order`);
  expect(same(L.headers.map(crcOfInts), e.headerCellsCrc32) && same(L.headers.map((h) => [h[0], h[1], h[69]]), e.headerCells), `${tag} header cells`);
  expect(same(Array.from(L.whiten.subarray(0, 32)), e.whitenFirst32) && crc32(L.whiten) === e.whitenCrc32, `${tag} whitening`);
}

// headers
const bitsOf = (s) => Uint8Array.from(s, (c) => (c === '1' ? 1 : 0));
for (const e of V.headers.encode)
  e.copies.forEach((bits, c) => expect(Array.from(packHeader(e.fields, c)).join('') === bits, `header encode copy ${c}`));
for (const e of V.headers.decode) {
  const r = decodeHeader(bitsOf(e.bits), e.copy);
  if (e.error) expect(!r.header, `header ${e.name} must be discarded`);
  else expect(r.header && same(r.header, e.header), `header ${e.name}`);
}

// frames from explicit packets
for (const e of V.frames) {
  const cells = encodeFrameCells(makeLayout(e.X, e.Y), e, e.seq, e.packets.map(unhex));
  expect(crc32(cells) === e.cellsCrc32 && (!e.cells || hex(cells) === e.cells), `frame ${e.X}x${e.Y} mode ${e.mode}`);
}

// packets
for (const e of V.packets.encode) {
  const payload = unhex(e.payload);
  const msg = new Uint8Array(payload.length + 7);
  writePacketHeader(msg, e.tid, e.sbn, e.esi, e.meta);
  msg.set(payload, PKT_HDR);
  expect(hex(sealPacket(msg)) === e.bytes, `packet encode ${e.tid}`);
}
for (const e of V.packets.decode) {
  const bytes = unhex(e.bytes);
  const action = await new Receiver().push(bytes);
  if (e.error) expect(action === 'discard', `packet ${e.name} must be discarded`);
  else {
    const p = readPacket(bytes);
    expect(same({ tid: p.tid, meta: p.meta, sbn: p.sbn, esi: p.esi, T: p.data.length }, e.packet), `packet ${e.name}`);
    if (!e.packet.meta) expect(action === 'keep', `packet ${e.name} kept`);
  }
}

// descriptors
for (const e of V.descriptors.encode) {
  expect(hex(encodeMeta(e, e.T)) === e.bytes, `descriptor encode ${e.name}`);
  if (e.decodedName) expect(parseDescriptor(unhex(e.bytes)).descriptor.name === e.decodedName, `descriptor cut ${e.decodedName}`);
}
for (const e of V.descriptors.decode) {
  const r = parseDescriptor(unhex(e.bytes));
  if (e.error) expect(!r.descriptor, `descriptor ${e.name} must be discarded`);
  else {
    const d = r.descriptor;
    const got = d && { flags: d.flags, payloadLength: d.payloadLength, fileSize: d.fileSize, crc: d.crc, name: d.name, records: d.records.map((x) => ({ type: x.type, value: hex(x.value) })) };
    expect(same(got, e.descriptor), `descriptor ${e.name}`);
  }
}

// fountain code
for (const e of V.fountainParams) {
  const p = fountainParams(e.K);
  expect(same([p.S, p.P, p.P1, p.W, p.L], [e.S, e.P, e.P1, e.W, e.L]), `fountain params K=${e.K}`);
}
for (const e of V.fountainSeed) expect(fountainSeed(e.tid, e.sbn) === e.seed, `fountain seed ${e.tid}/${e.sbn}`);
{
  const tv = V.fountainTuples;
  const p = fountainParams(tv.K);
  for (const t of tv.tuples) expect(same(_internals.tuple(p, tv.seed, t.esi), { lt: t.lt, pi: t.pi }), `fountain tuple ${t.esi}`);
  const rv = V.fountainRows;
  expect(same(_internals.ldpcRows(rv.K, fountainParams(rv.K).S)[0], rv.ldpcRow0), 'fountain LDPC row');
  const pi0 = _internals.hdpcMembers(rv.K, 0, rv.seed);
  expect(crcOfInts(pi0) === rv.piRow0Crc32 && same(pi0.slice(0, 16), rv.piRow0First16), 'fountain PI row');
  const sv = V.fountainSymbols;
  const enc = new FountainEncoder(unhex(sv.payload), sv.T, sv.seed);
  for (const s of sv.symbols) expect(hex(enc.encode(s.esi, new Uint8Array(sv.T))) === s.data, `fountain symbol ${s.esi}`);
}
for (const e of V.fountainDecode) {
  const dec = new FountainDecoder(e.K, e.T, e.seed);
  for (const s of e.symbols) dec.add(s.esi, unhex(s.data));
  const out = dec.decode();
  expect(e.decodable ? out && hex(out) === e.block : !out, `fountain decode K=${e.K} with ${e.symbols.length} symbols: ${e.decodable ? 'decodable' : 'not decodable'}`);
}
for (const e of V.blockPlans) {
  const p = blockPlan(e.payloadLength, e.T);
  expect(same([p.Kt, p.Z, p.sizes, p.starts], [e.Kt, e.Z, e.sizes, e.starts]), `block plan ${e.payloadLength}/${e.T}`);
}

// packet streams through the receiver state machine
for (const st of V.streams) {
  const rx = new Receiver(st.limits);
  let i = 0;
  for (const s of st.steps) {
    const where = `stream "${st.name}" step ${i++}`;
    if (s.push) expect((await rx.push(unhex(s.push))) === s.result, `${where}: push ${s.result}`);
    else if ('finish' in s) {
      const got = await rx.tryFinish();
      const norm = got && (got.bytes ? { file: hex(got.bytes), name: got.name } : { error: 'check' });
      expect(same(norm, s.finish), `${where}: finish ${JSON.stringify(s.finish)}`);
    } else if (s.state) {
      const p = rx.progress();
      expect((p ? p.state : 'new') === s.state, `${where}: state ${s.state}`);
    }
  }
}

// reference sender (informative, but the JS sender is the reference)
{
  const rs = V.referenceSender;
  for (const e of rs.codewordChoice) {
    const L = makeLayout(e.X, e.Y);
    const c = chooseCodeword(L, e.mode, e.eccLevel);
    expect(same([c.n, c.nsym, cellsPerCodeword(c.n, [1, 2, 3][e.mode]), codewordsPerFrame(L, e.mode, c.n)], [e.n, e.nsym, e.cellsPerCodeword, e.codewordsPerFrame]), `codeword choice ${e.X}x${e.Y}`);
  }
  const fv = rs.frames;
  const file = unhex(fv.file);
  const tx = new Sender({ payload: file, meta: { name: fv.name, fileSize: file.length, crc: crc32(file), flags: 0, payloadLength: file.length } }, { mode: fv.mode, ecc: fv.eccLevel, X: fv.X, Y: fv.Y, tid: fv.tid });
  tx.seq = fv.frame0.seq;
  expect(crc32(tx.nextFrame().cells) === fv.frame0.cellsCrc32 && crc32(tx.nextFrame().cells) === fv.frame1.cellsCrc32, 'reference sender frames');
}

if (failures) {
  console.log(`vectors: ${failures} of ${checks} checks failed`);
  process.exit(1);
}
console.log(`vectors: all ${checks} checks passed`);
