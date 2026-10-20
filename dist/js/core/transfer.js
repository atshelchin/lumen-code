// Sender: file bytes -> stream of frames. Receiver: packets -> file.

import { makeLayout, chooseCodeword, codewordsPerFrame, MODES, ECC_LEVELS, MIN_K, MIN_T } from './layout.js?v=c839a7fac2';
import {
  encodeFrameCells,
  packetDataSize,
  writePacketHeader,
  readPacket,
  sealPacket,
  packetIntact,
  encodeMeta,
  parseDescriptor,
  PKT_HDR,
  PKT_OVERHEAD,
  MAX_BLOCKS,
  MAX_BLOCK_SYMBOLS,
  FLAG_DEFLATE,
} from './frame.js?v=c839a7fac2';
import { FountainEncoder, FountainDecoder } from './fountain.js?v=c839a7fac2';
import { inflate } from './inflate.js?v=c839a7fac2';
import { crc32, mix32 } from './util.js?v=c839a7fac2';

export { FLAG_DEFLATE, MAX_BLOCK_SYMBOLS };

// Seed of the fountain code for source block `sbn` of transfer `tid`.
export function fountainSeed(tid, sbn = 0) {
  return mix32((((tid * 0x10001 + 0x51ed) >>> 0) + Math.imul(sbn, 0x9e3779b9)) >>> 0);
}

// Split Kt = ceil(L/T) symbols into Z <= 128 source blocks of nearly equal size
// (the first ZL blocks have one symbol more than the others).
export function blockPlan(payloadLength, T) {
  const Kt = Math.max(1, Math.ceil(payloadLength / T));
  const Z = Math.ceil(Kt / MAX_BLOCK_SYMBOLS);
  if (Z > MAX_BLOCKS) throw new Error('payload too large');
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
    this.descriptors = 0; // descriptors sent: the next one goes in codeword mix32(count) mod m
    this._configure(X, Y, true);
  }

  _configure(X, Y, fresh) {
    const layout = makeLayout(X, Y);
    if (fresh) {
      const { n, nsym } = chooseCodeword(layout, this.mode, this.ecc);
      this.n = n;
      this.nsym = nsym;
      this.T = packetDataSize(n, nsym);
      if (this.T < MIN_T) throw new Error('code area too small');
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

  // The packets (RS messages) of the next frame, without the visual layer (§10.2).
  nextPackets() {
    const k = this.n - this.nsym;
    const msgs = [];
    const m = this.perFrame;
    // descriptor: every frame for tiny transfers, otherwise every 4th frame; its codeword
    // moves, so that a corner hidden by glare cannot hide every copy
    const metaAt = this.K <= 2 * m || this.frames % 4 === 0 ? mix32(this.descriptors++) % m : -1;
    const Z = this.plan.Z;
    for (let i = 0; i < m; i++) {
      const msg = new Uint8Array(k);
      if (i === metaAt) {
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

// Reference limits of a receiver (§11.4).
export const RECEIVER_LIMITS = {
  maxSize: 256 * 1024 * 1024, // bytes; a transfer with a larger payload or file is refused
  maxKept: 4096, // data packets kept for each transfer before its descriptor arrives
  maxTransfers: 4, // transfer keys kept at the same time
};

// State of one transfer key (TID, T), §11.2: 'collecting', 'receiving', 'refused' or 'complete'.
class Inbound {
  constructor(tid, T) {
    this.tid = tid;
    this.T = T;
    this.touched = 0;
    this.reset();
  }

  reset() {
    this.state = 'collecting';
    this.descBytes = null; // the accepted descriptor, all T bytes
    this.meta = null;
    this.plan = null;
    this.kept = new Map(); // sbn * 65536 + esi -> payload, data packets before the descriptor
    this.decs = null; // one fountain decoder per block, null once the block is decoded
    this.outs = null; // decoded blocks
    this.lastTry = null; // symbol count of the last failed decode of each block
  }

  received() {
    if (this.state === 'collecting') return this.kept.size;
    if (this.state !== 'receiving') return this.state === 'complete' ? this.plan.Kt : 0;
    let r = 0;
    for (let b = 0; b < this.plan.Z; b++) r += this.outs[b] ? this.plan.sizes[b] : this.decs[b].count;
    return r;
  }
}

const sameBytes = (a, b) => a.length === b.length && a.every((v, i) => v === b[i]);

export class Receiver {
  constructor(limits = {}) {
    this.limits = { ...RECEIVER_LIMITS, ...limits };
    this.transfers = new Map(); // "tid:T" -> Inbound
    this.active = null; // the transfer of the newest packet
    this.clock = 0;
  }

  /**
   * Give one packet (k bytes, the message of a codeword) to the transfer state machine.
   * Returns the action of §11.2: 'discard', 'keep' (kept until the descriptor arrives),
   * 'add' (given to a block decoder), 'accept' (descriptor accepted) or 'refuse' (descriptor
   * accepted, transfer over a limit).
   */
  async push(bytes) {
    if (bytes.length < MIN_K || !packetIntact(bytes)) return 'discard';
    const p = readPacket(bytes);
    let desc = null;
    if (p.meta) {
      if (p.sbn !== 0 || p.esi !== 0) return 'discard'; // reserved metadata (§8.3)
      const r = parseDescriptor(p.data);
      if (r.error) return 'discard';
      desc = r.descriptor;
    }
    const key = `${p.tid}:${p.data.length}`;
    let tr = this.transfers.get(key);
    if (!tr) {
      if (this.transfers.size >= this.limits.maxTransfers) {
        let old = null;
        for (const [k, t] of this.transfers) if (!old || t.touched < this.transfers.get(old).touched) old = k;
        this.transfers.delete(old);
      }
      tr = new Inbound(p.tid, p.data.length);
      this.transfers.set(key, tr);
    }
    tr.touched = ++this.clock;
    this.active = tr;

    if (desc) {
      if (tr.descBytes) {
        if (sameBytes(tr.descBytes, p.data)) return 'discard';
        tr.reset(); // another transfer with the same key, or a wrong descriptor (§11.2)
      }
      return this._accept(tr, desc, p.data.slice());
    }
    if (tr.state === 'collecting') {
      const k = p.sbn * 65536 + p.esi;
      if (tr.kept.has(k) || tr.kept.size >= this.limits.maxKept) return 'discard';
      tr.kept.set(k, p.data.slice());
      return 'keep';
    }
    if (tr.state !== 'receiving' || p.sbn >= tr.plan.Z || tr.outs[p.sbn]) return 'discard';
    return tr.decs[p.sbn].add(p.esi, p.data) ? 'add' : 'discard';
  }

  _accept(tr, meta, bytes) {
    tr.descBytes = bytes;
    tr.meta = meta;
    tr.plan = blockPlan(meta.payloadLength, tr.T);
    if (Math.max(meta.payloadLength, meta.fileSize) > this.limits.maxSize) {
      tr.state = 'refused';
      tr.kept.clear();
      return 'refuse';
    }
    const { Z, sizes } = tr.plan;
    tr.decs = sizes.map((k, b) => new FountainDecoder(k, tr.T, fountainSeed(tr.tid, b)));
    tr.outs = new Array(Z).fill(null);
    tr.lastTry = new Int32Array(Z).fill(-1);
    for (const [k, data] of tr.kept) {
      const sbn = Math.floor(k / 65536);
      if (sbn < Z) tr.decs[sbn].add(k % 65536, data);
    }
    tr.kept.clear();
    tr.state = 'receiving';
    return 'accept';
  }

  progress() {
    const tr = this.active;
    if (!tr) return null;
    return {
      tid: tr.tid,
      T: tr.T,
      state: tr.state,
      name: tr.meta ? tr.meta.name : null,
      size: tr.meta ? tr.meta.fileSize : null,
      payloadLength: tr.meta ? tr.meta.payloadLength : null,
      received: tr.received(),
      needed: tr.plan ? tr.plan.Kt : null,
      blocks: tr.plan ? tr.plan.Z : null,
      done: tr.state === 'complete',
    };
  }

  /**
   * Decode what can be decoded in the active transfer and, when every block is decoded,
   * do the file check (§11.3). Returns { name, bytes } once, { error: 'checksum', name }
   * when the check fails (the transfer then starts again), or null.
   */
  async tryFinish() {
    const tr = this.active;
    if (!tr || tr.state !== 'receiving') return null;
    const { Z, sizes, starts, Kt } = tr.plan;
    for (let b = 0; b < Z; b++) {
      const dec = tr.decs[b];
      if (!dec || dec.count < sizes[b] || dec.count === tr.lastTry[b]) continue;
      tr.lastTry[b] = dec.count;
      const out = dec.decode();
      if (out) {
        tr.outs[b] = out;
        tr.decs[b] = null;
      }
    }
    if (tr.outs.some((o) => !o)) return null;
    const { payloadLength, fileSize, flags, crc, name } = tr.meta;
    const all = new Uint8Array(Kt * tr.T);
    tr.outs.forEach((o, b) => all.set(o, starts[b] * tr.T));
    const payload = all.subarray(0, payloadLength);
    const bytes = flags & FLAG_DEFLATE ? inflate(payload, fileSize) : payload;
    if (!bytes || bytes.length !== fileSize || crc32(bytes) !== crc) {
      tr.reset();
      return { error: 'checksum', name };
    }
    tr.state = 'complete';
    tr.decs = null;
    tr.outs = null;
    return { name, bytes: bytes.slice() };
  }
}

export { MODES };
