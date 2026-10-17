// Reed–Solomon over GF(256), primitive polynomial 0x11d, first consecutive root α^0.
// Systematic encoding; decoding handles errors and erasures (2e + f <= nsym).

const EXP = new Uint8Array(512);
const LOG = new Uint8Array(256);
(() => {
  let x = 1;
  for (let i = 0; i < 255; i++) {
    EXP[i] = x;
    LOG[x] = i;
    x <<= 1;
    if (x & 0x100) x ^= 0x11d;
  }
  for (let i = 255; i < 512; i++) EXP[i] = EXP[i - 255];
})();

export function gfMul(a, b) {
  return a && b ? EXP[LOG[a] + LOG[b]] : 0;
}

function gfInv(a) {
  return EXP[255 - LOG[a]];
}

// Generator polynomial, highest degree first, g[0] = 1.
const genCache = new Map();
function generator(nsym) {
  let g = genCache.get(nsym);
  if (g) return g;
  let p = [1];
  for (let i = 0; i < nsym; i++) {
    const q = new Array(p.length + 1).fill(0);
    for (let j = 0; j < p.length; j++) {
      q[j] ^= p[j];
      q[j + 1] ^= gfMul(p[j], EXP[i]);
    }
    p = q;
  }
  // store log of each coefficient (skipping the leading 1) for the LFSR
  g = new Int16Array(nsym);
  for (let j = 0; j < nsym; j++) g[j] = p[j + 1] ? LOG[p[j + 1]] : -1;
  genCache.set(nsym, g);
  return g;
}

// out[0..k) = msg, out[k..k+nsym) = parity. out may alias msg's buffer start.
export function rsEncode(msg, nsym, out) {
  const k = msg.length;
  const g = generator(nsym);
  const par = new Uint8Array(nsym + 1);
  for (let i = 0; i < k; i++) {
    const coef = msg[i] ^ par[0];
    par.copyWithin(0, 1);
    par[nsym - 1] = 0;
    if (coef) {
      const lc = LOG[coef];
      for (let j = 0; j < nsym; j++) {
        const gj = g[j];
        if (gj >= 0) par[j] ^= EXP[lc + gj];
      }
    }
  }
  if (out !== msg) out.set(msg, 0);
  out.set(par.subarray(0, nsym), k);
  return out;
}

function syndromes(cw, n, nsym, S) {
  let nonzero = false;
  for (let j = 0; j < nsym; j++) {
    let s = 0;
    for (let i = 0; i < n; i++) {
      s = (s ? EXP[LOG[s] + j] : 0) ^ cw[i];
    }
    S[j] = s;
    if (s) nonzero = true;
  }
  return nonzero;
}

// polynomial helpers, lowest degree first
function polyEval(p, x) {
  // Horner from highest
  let y = 0;
  const lx = x ? LOG[x] : -1;
  for (let i = p.length - 1; i >= 0; i--) {
    y = (y && lx >= 0 ? EXP[LOG[y] + lx] : 0) ^ p[i];
  }
  return y;
}

const _S = new Uint8Array(256);

/**
 * Decode in place. Returns number of corrected symbols, or -1 on failure.
 * erasures: optional array of byte positions known to be unreliable.
 */
export function rsDecode(cw, nsym, erasures) {
  const n = cw.length;
  const S = _S;
  if (!syndromes(cw, n, nsym, S)) return 0;

  const ne = erasures ? erasures.length : 0;
  if (ne > nsym) return -1;

  // erasure locator Γ(x) = Π (1 + X_k x)
  let lambda = [1];
  for (let k = 0; k < ne; k++) {
    const X = EXP[(n - 1 - erasures[k]) % 255];
    const q = new Array(lambda.length + 1).fill(0);
    for (let i = 0; i < lambda.length; i++) {
      q[i] ^= lambda[i];
      q[i + 1] ^= gfMul(lambda[i], X);
    }
    lambda = q;
  }

  // Berlekamp–Massey with erasures (Blahut)
  let B = lambda.slice();
  let L = ne;
  for (let r = ne + 1; r <= nsym; r++) {
    let delta = 0;
    for (let i = 0; i <= L && i < lambda.length; i++) {
      if (r - 1 - i >= 0) delta ^= gfMul(lambda[i], S[r - 1 - i]);
    }
    // xB
    const xB = [0, ...B];
    if (delta === 0) {
      B = xB;
      continue;
    }
    const len = Math.max(lambda.length, xB.length);
    const T = new Array(len).fill(0);
    for (let i = 0; i < lambda.length; i++) T[i] = lambda[i];
    for (let i = 0; i < xB.length; i++) T[i] ^= gfMul(delta, xB[i]);
    if (2 * L <= r - 1 + ne) {
      const inv = gfInv(delta);
      B = lambda.map((c) => gfMul(c, inv));
      L = r + ne - L;
    } else {
      B = xB;
    }
    lambda = T;
  }
  while (lambda.length > 1 && lambda[lambda.length - 1] === 0) lambda.pop();
  const deg = lambda.length - 1;
  if (deg !== L || 2 * (L - ne) + ne > nsym) return -1;

  // Chien search
  const pos = [];
  for (let p = 0; p < n; p++) {
    const xinv = EXP[(255 - ((n - 1 - p) % 255)) % 255];
    if (polyEval(lambda, xinv) === 0) pos.push(p);
  }
  if (pos.length !== deg) return -1;

  // Ω(x) = S(x)Λ(x) mod x^nsym
  const omega = new Array(nsym).fill(0);
  for (let i = 0; i < lambda.length; i++) {
    if (!lambda[i]) continue;
    for (let j = 0; i + j < nsym; j++) omega[i + j] ^= gfMul(lambda[i], S[j]);
  }
  // Λ'(x): odd terms
  const dl = [];
  for (let i = 1; i < lambda.length; i += 2) {
    dl[i - 1] = lambda[i];
    dl[i] = 0;
  }

  for (const p of pos) {
    const X = EXP[(n - 1 - p) % 255];
    const xinv = gfInv(X);
    const num = polyEval(omega, xinv);
    const den = polyEval(dl, xinv);
    if (!den) return -1;
    const e = gfMul(X, gfMul(num, gfInv(den)));
    cw[p] ^= e;
  }

  // guard against miscorrection
  if (syndromes(cw, n, nsym, S)) return -1;
  return pos.length;
}
