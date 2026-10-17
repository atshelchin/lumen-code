// Codewords <-> cells, packet and metadata formats.

import { MODES, cellsPerCodeword, packHeader, K_FIXED, K_MARKER } from './layout.js';
import { rsEncode, rsDecode } from './rs.js';
import { mix32, crc16 } from './util.js';

export const IDX_INK = 8;
export const IDX_PAPER = 9;

// Packet = one RS message of k bytes:
//   [0..1]  transfer id (big-endian)
//   [2..4]  M(1) | SBN(7) | ESI(16)     M = descriptor/metadata packet, SBN = source block
//   [5..k-3] T = k-7 payload bytes
//   [k-2..k-1] CRC-16/CCITT-FALSE of bytes 0..k-3 (guards the fountain decoder against
//              RS miscorrection)
export const PKT_HDR = 5;
export const PKT_OVERHEAD = PKT_HDR + 2;
export const MAX_SBN = 127;
export const MAX_ESI = 0xffff;

export function packetDataSize(n, nsym) {
  return n - nsym - PKT_OVERHEAD;
}

export function sealPacket(msg) {
  const c = crc16(msg, msg.length - 2);
  msg[msg.length - 2] = c >>> 8;
  msg[msg.length - 1] = c & 255;
  return msg;
}

export function packetIntact(msg) {
  return crc16(msg, msg.length - 2) === ((msg[msg.length - 2] << 8) | msg[msg.length - 1]);
}

export function writePacketHeader(buf, tid, sbn, esi, isMeta) {
  buf[0] = (tid >>> 8) & 255;
  buf[1] = tid & 255;
  buf[2] = (isMeta ? 0x80 : 0) | (sbn & 0x7f);
  buf[3] = (esi >>> 8) & 255;
  buf[4] = esi & 255;
}

export function readPacket(buf) {
  return {
    tid: (buf[0] << 8) | buf[1],
    meta: (buf[2] & 0x80) !== 0,
    sbn: buf[2] & 0x7f,
    esi: (buf[3] << 8) | buf[4],
    data: buf.subarray(PKT_HDR, buf.length - 2),
  };
}

// File descriptor (payload of the metadata packet with ESI 0):
//   [0] version = 1   [1] flags (bit 0: payload is raw DEFLATE)
//   [2..5] payload length  [6..9] file size  [10..13] CRC-32 of the file  (big-endian)
//   [14] name length N  [15..15+N) name, UTF-8
//   then optional TLV records (type u8, length u8, value); type 0 ends the list.
export const DESC_FIXED = 15;

function utf8Truncate(bytes, max) {
  if (bytes.length <= max) return bytes;
  let end = max;
  while (end > 0 && (bytes[end] & 0xc0) === 0x80) end--; // do not split a code point
  return bytes.subarray(0, end);
}

export function encodeMeta(meta, T) {
  const out = new Uint8Array(T);
  const name = utf8Truncate(new TextEncoder().encode(meta.name || 'file'), Math.max(0, Math.min(255, T - DESC_FIXED)));
  out[0] = 1;
  out[1] = meta.flags & 255;
  const dv = new DataView(out.buffer);
  dv.setUint32(2, meta.payloadLength);
  dv.setUint32(6, meta.fileSize);
  dv.setUint32(10, meta.crc >>> 0);
  out[14] = name.length;
  out.set(name, DESC_FIXED);
  return out;
}

export function decodeMeta(data) {
  if (data.length < DESC_FIXED || data[0] !== 1) return null;
  const dv = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const nameLen = Math.min(data[14], data.length - DESC_FIXED);
  let name = 'file';
  try {
    name = new TextDecoder('utf-8', { fatal: false }).decode(data.subarray(DESC_FIXED, DESC_FIXED + nameLen)) || 'file';
  } catch (e) {
    /* keep default */
  }
  // TLV records after the name are reserved for extensions; unknown types are skipped.
  return {
    flags: data[1],
    payloadLength: dv.getUint32(2),
    fileSize: dv.getUint32(6),
    crc: dv.getUint32(10),
    name,
  };
}

/**
 * Build the per-cell colour index array for one frame.
 * codewords: array of Uint8Array(k) messages (RS parity is added here).
 */
export function encodeFrameCells(layout, cfg, seq, messages) {
  const { X, Y, kind, fixed, headers, dataOrder, whiten } = layout;
  const { mode, n, nsym } = cfg;
  const bpc = MODES[mode].bpc;
  const mask = (1 << bpc) - 1;
  const cells = new Uint8Array(X * Y);
  for (let i = 0; i < cells.length; i++) {
    if (kind[i] === K_FIXED || kind[i] === K_MARKER) cells[i] = fixed[i] ? IDX_PAPER : IDX_INK;
  }
  headers.forEach((h, copy) => {
    const hbits = packHeader({ mode, X, Y, n, nsym, seq }, copy);
    for (let b = 0; b < h.length; b++) cells[h[b]] = hbits[b] ? IDX_PAPER : IDX_INK;
  });

  const cpc = cellsPerCodeword(n, bpc);
  const cw = new Uint8Array(n);
  let slot = 0;
  for (const msg of messages) {
    rsEncode(msg, nsym, cw);
    let acc = 0;
    let accBits = 0;
    let byte = 0;
    for (let c = 0; c < cpc; c++) {
      while (accBits < bpc) {
        acc = (acc << 8) | (byte < n ? cw[byte] : 0);
        byte++;
        accBits += 8;
      }
      const sym = (acc >>> (accBits - bpc)) & mask;
      accBits -= bpc;
      acc &= (1 << accBits) - 1;
      cells[dataOrder[slot]] = sym ^ (whiten[slot] & mask);
      slot++;
    }
  }
  // filler
  let r = mix32(seq + 0x1234567);
  for (; slot < dataOrder.length; slot++) {
    r = mix32(r + slot);
    cells[dataOrder[slot]] = r & mask;
  }
  return cells;
}

/**
 * symbols: Uint8Array(dataCells) raw symbol per data slot (before un-whitening)
 * conf: Float32Array(dataCells) confidence per slot (higher = better)
 * Returns array of { index, ok, bytes(k), corrected }
 */
export function decodeCodewords(layout, cfg, symbols, conf, count, skip) {
  const { whiten } = layout;
  const { mode, n, nsym } = cfg;
  const bpc = MODES[mode].bpc;
  const mask = (1 << bpc) - 1;
  const cpc = cellsPerCodeword(n, bpc);
  const out = [];
  const cw = new Uint8Array(n);
  const byteConf = new Float32Array(n);
  for (let j = 0; j < count; j++) {
    if (skip && skip.has(j)) {
      out.push({ index: j, ok: false, skipped: true, corrected: 0, bytes: null });
      continue;
    }
    cw.fill(0);
    byteConf.fill(1e9);
    let bitPos = 0;
    for (let c = 0; c < cpc; c++) {
      const slot = j * cpc + c;
      const sym = (symbols[slot] ^ whiten[slot]) & mask;
      const cf = conf[slot];
      for (let b = bpc - 1; b >= 0; b--) {
        const byte = bitPos >>> 3;
        if (byte < n) {
          if ((sym >>> b) & 1) cw[byte] |= 0x80 >>> (bitPos & 7);
          if (cf < byteConf[byte]) byteConf[byte] = cf;
        }
        bitPos++;
      }
    }
    const work = cw.slice();
    let res = rsDecode(work, nsym, null);
    if (res < 0) {
      // retry treating least-confident bytes as erasures
      const idx = Array.from({ length: n }, (_, i) => i).sort((a, b) => byteConf[a] - byteConf[b]);
      // keep >= nsym/2 redundancy so the decoder cannot be talked into a miscorrection
      for (const f of [nsym >> 2, nsym >> 1]) {
        if (f <= 0) continue;
        const w2 = cw.slice();
        const r2 = rsDecode(w2, nsym, idx.slice(0, f));
        if (r2 >= 0) {
          res = r2;
          work.set(w2);
          break;
        }
      }
    }
    const ok = res >= 0 && packetIntact(work.subarray(0, n - nsym));
    out.push({ index: j, ok, corrected: res, bytes: ok ? work.slice(0, n - nsym) : null });
  }
  return out;
}
