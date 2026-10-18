// Sender: file bytes -> stream of frames. Receiver: packets -> file.

import { makeLayout, chooseCodeword, codewordsPerFrame, MODES, ECC_LEVELS } from './layout.js?v=5fd5612805';
import {
  encodeFrameCells,
  packetDataSize,
  writePacketHeader,
  readPacket,
  sealPacket,
  encodeMeta,
  decodeMeta,
  PKT_HDR,
  PKT_OVERHEAD,
  MAX_SBN,
} from './frame.js?v=5fd5612805';
import { FountainEncoder, FountainDecoder } from './fountain.js?v=5fd5612805';
import { crc32, mix32 } from './util.js?v=5fd5612805';

export const FLAG_DEFLATE = 1;

export const MAX_BLOCK_SYMBOLS = 16384;

// Seed of the fountain code for source block `sbn` of transfer `tid`.
export function fountainSeed(tid, sbn = 0) {
  return mix32((((tid * 0x10001 + 0x51ed) >>> 0) + Math.imul(sbn, 0x9e3779b9)) >>> 0);
}

// Split Kt = ceil(L/T) symbols into Z <= 128 source blocks of nearly equal size
// (the first ZL blocks have one symbol more than the others).
export function blockPlan(payloadLength, T) {
  const Kt = Math.max(1, Math.ceil(payloadLength / T));
  const Z = Math.ceil(Kt / MAX_BLOCK_SYMBOLS);
  if (Z > MAX_SBN + 1) throw new Error('payload too large');
  const KS = Math.floor(Kt / Z);
  const ZL = Kt - KS * Z;
  const sizes = [];
  const starts = [];
  let at = 0;
  for (let b = 0; b < Z; b++) {
    const k = b < ZL ? KS + 1 : KS;
    sizes.push(k);
    starts.push(at);
    at += k;
  }
  return { Kt, Z, sizes, starts };
}

async function streamThrough(bytes, stream) {
  const blob = new Blob([bytes]);
  const out = await new Response(blob.stream().pipeThrough(stream)).arrayBuffer();
  return new Uint8Array(out);
}

// Compress if the platform supports it and it actually helps.
export async function preparePayload(fileBytes, name) {
  const crc = crc32(fileBytes);
  let payload = fileBytes;
  let flags = 0;
  if (typeof CompressionStream !== 'undefined' && fileBytes.length > 64) {
    try {
      const z = await streamThrough(fileBytes, new CompressionStream('deflate-raw'));
      if (z.length < fileBytes.length * 0.97) {
        payload = z;
        flags |= FLAG_DEFLATE;
      }
    } catch (e) {
      /* fall back to raw */
    }
  }
  return { payload, meta: { name, fileSize: fileBytes.length, crc, flags, payloadLength: payload.length } };
}

export class Sender {
  constructor(prepared, { mode = 2, ecc = 1, X, Y, tid } = {}) {
    this.payload = prepared.payload;
    this.meta = prepared.meta;
    this.mode = mode;
    this.ecc = ecc;
    this.tid = tid ?? (Math.random() * 65536) | 0;
    this.seq = (Math.random() * 256) | 0; // random start: a restarted transfer never repeats recent seqs
    this.counter = 0; // packets emitted; packet i goes to block i mod Z with ESI floor(i / Z)
    this.frames = 0;
    this._configure(X, Y, true);
  }

  _configure(X, Y, fresh) {
    const layout = makeLayout(X, Y);
    if (fresh) {
      const { n, nsym } = chooseCodeword(layout, this.mode, this.ecc);
      this.n = n;
      this.nsym = nsym;
      this.T = packetDataSize(n, nsym);
      this.plan = blockPlan(this.payload.length, this.T);
      const padded = new Uint8Array(this.plan.Kt * this.T);
      padded.set(this.payload);
      this.encs = this.plan.sizes.map((k, b) => {
        const start = this.plan.starts[b] * this.T;
        return new FountainEncoder(padded.subarray(start, start + k * this.T), this.T, fountainSeed(this.tid, b));
      });
      this.metaBody = encodeMeta(this.meta, this.T);
    }
    this.layout = layout;
    this.perFrame = codewordsPerFrame(layout, this.mode, this.n);
    if (this.perFrame < 1) throw new Error('code area too small');
  }

  get K() {
    return this.plan.Kt;
  }

  // Resizing keeps the codeword size (and thus the fountain stream) when possible.
  resize(X, Y) {
    if (X === this.layout.X && Y === this.layout.Y) return;
    this._configure(X, Y, false);
  }

  // Change colour mode / ECC while keeping the packet size (so receivers keep their
  // progress). Returns false if that is impossible and a new transfer is needed.
  reconfigure({ mode, ecc }) {
    const r = ECC_LEVELS[ecc];
    const nsym = Math.max(4, 2 * Math.round(((this.T + PKT_OVERHEAD) * r) / (1 - r) / 2));
    const n = this.T + PKT_OVERHEAD + nsym;
    if (n > 255 || codewordsPerFrame(this.layout, mode, n) < 1) return false;
    this.mode = mode;
    this.ecc = ecc;
    this.n = n;
    this.nsym = nsym;
    this.perFrame = codewordsPerFrame(this.layout, mode, n);
    return true;
  }

  bytesPerFrame() {
    return this.perFrame * this.T;
  }

  nextFrame() {
    const msgs = this.nextPackets();
    const seq = this.seq;
    this.seq = (this.seq + 1) & 255;
    const cells = encodeFrameCells(this.layout, { mode: this.mode, n: this.n, nsym: this.nsym }, seq, msgs);
    return { cells, layout: this.layout, seq };
  }

  // The packets (RS messages) of the next frame, without the visual layer.
  nextPackets() {
    const k = this.n - this.nsym;
    const msgs = [];
    const m = this.perFrame;
    // descriptor: every frame for tiny transfers, otherwise every 4th frame
    const withMeta = this.K <= 2 * m || this.frames % 4 === 0;
    const Z = this.plan.Z;
    for (let i = 0; i < m; i++) {
      const msg = new Uint8Array(k);
      if (withMeta && i === 0) {
        writePacketHeader(msg, this.tid, 0, 0, true);
        msg.set(this.metaBody, PKT_HDR);
      } else {
        const b = this.counter % Z;
        const esi = Math.floor(this.counter / Z) & 0xffff; // wraps after 65536 symbols per block
        writePacketHeader(msg, this.tid, b, esi, false);
        this.encs[b].encode(esi, msg.subarray(PKT_HDR, PKT_HDR + this.T));
        this.counter++;
      }
      msgs.push(sealPacket(msg));
    }
    this.frames++;
    return msgs;
  }
}

class Inbound {
  constructor(tid, T) {
    this.tid = tid;
    this.T = T;
    this.meta = null;
    this.plan = null;
    this.pending = new Map(); // "sbn:esi" -> data, packets that arrive before the descriptor
    this.decs = null;
    this.outs = null; // decoded block bytes
    this.lastTry = null;
    this.done = false;
  }

  received() {
    if (!this.decs) return this.pending.size;
    let r = 0;
    for (let b = 0; b < this.decs.length; b++) r += this.outs[b] ? this.plan.sizes[b] : this.decs[b].count;
    return r;
  }
}

export class Receiver {
  constructor() {
    this.transfers = new Map();
    this.active = null;
  }

  async push(bytes) {
    const p = readPacket(bytes);
    const key = `${p.tid}:${p.data.length}`;
    let tr = this.transfers.get(key);
    if (!tr) {
      tr = new Inbound(p.tid, p.data.length);
      this.transfers.set(key, tr);
    }
    this.active = tr;
    if (tr.done) return { duplicate: true };
    if (p.meta) {
      if (!tr.meta && p.esi === 0) {
        const m = decodeMeta(p.data);
        if (m) {
          let plan;
          try {
            plan = blockPlan(m.payloadLength, tr.T);
          } catch (e) {
            return null;
          }
          tr.meta = m;
          tr.plan = plan;
          tr.decs = plan.sizes.map((k, b) => new FountainDecoder(k, tr.T, fountainSeed(tr.tid, b)));
          tr.outs = new Array(plan.Z).fill(null);
          tr.lastTry = new Int32Array(plan.Z).fill(-1);
          for (const [k2, data] of tr.pending) {
            const [sbn, esi] = k2.split(':').map(Number);
            if (sbn < plan.Z) tr.decs[sbn].add(esi, data);
          }
          tr.pending.clear();
        }
      }
    } else if (tr.decs) {
      if (p.sbn < tr.plan.Z && !tr.outs[p.sbn]) tr.decs[p.sbn].add(p.esi, p.data);
    } else {
      const k2 = `${p.sbn}:${p.esi}`;
      if (tr.pending.size < 200000 && !tr.pending.has(k2)) tr.pending.set(k2, p.data.slice());
    }
    return null;
  }

  progress() {
    const tr = this.active;
    if (!tr) return null;
    return {
      tid: tr.tid,
      name: tr.meta ? tr.meta.name : null,
      size: tr.meta ? tr.meta.fileSize : null,
      received: tr.received(),
      needed: tr.plan ? tr.plan.Kt : null,
      blocks: tr.plan ? tr.plan.Z : null,
      done: tr.done,
    };
  }

  // Try to finish the active transfer. Returns { name, bytes }, { error }, or null.
  async tryFinish() {
    const tr = this.active;
    if (!tr || tr.done || !tr.decs) return null;
    for (let b = 0; b < tr.plan.Z; b++) {
      const dec = tr.decs[b];
      if (tr.outs[b] || dec.count < dec.K || dec.count === tr.lastTry[b]) continue;
      tr.lastTry[b] = dec.count;
      const out = dec.decode();
      if (out) {
        tr.outs[b] = out;
        tr.decs[b] = { count: dec.K, K: dec.K }; // free the decoder state
      }
    }
    if (tr.outs.some((o) => !o)) return null;
    const all = new Uint8Array(tr.plan.Kt * tr.T);
    tr.outs.forEach((o, b) => all.set(o, tr.plan.starts[b] * tr.T));
    let bytes = all.subarray(0, tr.meta.payloadLength);
    if (tr.meta.flags & FLAG_DEFLATE) {
      bytes = await streamThrough(bytes, new DecompressionStream('deflate-raw'));
    }
    if (bytes.length !== tr.meta.fileSize || crc32(bytes) !== tr.meta.crc) {
      return { error: 'checksum', name: tr.meta.name };
    }
    tr.done = true;
    return { name: tr.meta.name, bytes };
  }
}

export { MODES };
