import { rsEncode, rsDecode } from '../src/core/rs.js';
import { FountainEncoder, FountainDecoder, fountainParams } from '../src/core/fountain.js';
import { crc32, Rng } from '../src/core/util.js';

let failures = 0;
function check(cond, msg) {
  if (!cond) {
    failures++;
    console.log('FAIL', msg);
  }
}

// ---------- Reed–Solomon
{
  const rng = new Rng(1234);
  let ok = 0;
  let trials = 0;
  for (const [n, nsym] of [[255, 48], [200, 40], [64, 16], [255, 32]]) {
    for (let t = 0; t < 300; t++) {
      const k = n - nsym;
      const msg = new Uint8Array(k);
      for (let i = 0; i < k; i++) msg[i] = rng.next() & 255;
      const cw = rsEncode(msg, nsym, new Uint8Array(n));
      const ne = rng.next() % (nsym + 1);
      const maxErr = Math.floor((nsym - ne) / 2);
      const nerr = maxErr ? rng.next() % (maxErr + 1) : 0;
      const posSet = new Set();
      while (posSet.size < ne + nerr) posSet.add(rng.next() % n);
      const posArr = [...posSet];
      const eras = posArr.slice(0, ne);
      const rx = cw.slice();
      for (const p of posArr) rx[p] ^= 1 + (rng.next() % 255);
      // erased positions may happen to be corrupted or not; both fine
      const r = rsDecode(rx, nsym, eras);
      trials++;
      if (r >= 0 && rx.every((v, i) => v === cw[i])) ok++;
      else console.log('rs fail', { n, nsym, ne, nerr, r });
    }
  }
  check(ok === trials, `RS within capacity ${ok}/${trials}`);

  // beyond capacity must not silently miscorrect (mostly)
  let mis = 0;
  for (let t = 0; t < 300; t++) {
    const n = 255, nsym = 32;
    const msg = new Uint8Array(n - nsym).map(() => rng.next() & 255);
    const cw = rsEncode(msg, nsym, new Uint8Array(n));
    const rx = cw.slice();
    for (let e = 0; e < 40; e++) rx[rng.next() % n] ^= 1 + (rng.next() % 255);
    const r = rsDecode(rx, nsym, null);
    if (r >= 0 && !rx.every((v, i) => v === cw[i])) mis++;
  }
  check(mis === 0, `RS miscorrections beyond capacity: ${mis}`);
  console.log(`RS ok ${ok}/${trials}`);
}

// ---------- Fountain
function fountainTrial(size, T, seed, lossRate, rng) {
  const payload = new Uint8Array(size);
  for (let i = 0; i < size; i++) payload[i] = rng.next() & 255;
  const enc = new FountainEncoder(payload, T, seed);
  const dec = new FountainDecoder(enc.K, T, seed);
  const buf = new Uint8Array(T);
  let esi = rng.next() % 1000;
  let attempts = 0;
  const t0 = performance.now();
  let tDecode = 0;
  for (;;) {
    esi++;
    if (rng.next() / 4294967296 < lossRate) continue;
    enc.encode(esi, buf);
    dec.add(esi, buf);
    if (dec.count >= enc.K) {
      attempts++;
      const td = performance.now();
      const out = dec.decode();
      tDecode += performance.now() - td;
      if (out) {
        const same = out.subarray(0, size).every((v, i) => v === payload[i]);
        return { overhead: dec.count - enc.K, ok: same, attempts, stats: dec.lastStats, ms: performance.now() - t0, tDecode };
      }
    }
    if (dec.count > enc.K * 1.5 + 50) return { overhead: -1, ok: false, stats: dec.lastStats };
  }
}

{
  const rng = new Rng(99);
  for (const [K, T, trials] of [[1, 64, 30], [3, 64, 30], [20, 100, 50], [200, 100, 40], [1000, 200, 20], [5000, 200, 4], [20000, 200, 1]]) {
    const hist = [];
    let ok = true;
    let maxI = 0;
    let ms = 0, td = 0;
    for (let t = 0; t < trials; t++) {
      const size = K * T - (rng.next() % T);
      const r = fountainTrial(size, T, rng.next(), 0.2, rng);
      ok = ok && r.ok;
      hist.push(r.overhead);
      maxI = Math.max(maxI, r.stats ? r.stats.inactive : 0);
      ms += r.ms || 0;
      td += r.tDecode || 0;
    }
    hist.sort((a, b) => a - b);
    const mean = hist.reduce((a, b) => a + b, 0) / hist.length;
    const p = fountainParams(K);
    console.log(`fountain K=${K} S=${p.S} P=${p.P}: ok=${ok} overhead mean=${mean.toFixed(2)} max=${hist[hist.length - 1]} maxInactive=${maxI} avgTotal=${(ms / trials).toFixed(0)}ms decodeTime=${(td / trials).toFixed(0)}ms`);
    check(ok && hist[hist.length - 1] >= 0, `fountain K=${K}`);
  }
}

check(crc32(new TextEncoder().encode('123456789')) === 0xcbf43926, 'crc32');

if (failures) {
  console.log(`${failures} failures`);
  process.exit(1);
}
console.log('core: all passed');
