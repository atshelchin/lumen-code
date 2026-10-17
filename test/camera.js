// Crude screen-to-camera simulator for tests.
import { homography, applyH } from '../src/core/scan.js';
import { Rng } from '../src/core/util.js';

function gaussKernel(sigma) {
  const r = Math.max(1, Math.ceil(sigma * 3));
  const k = new Float32Array(2 * r + 1);
  let s = 0;
  for (let i = -r; i <= r; i++) {
    k[i + r] = Math.exp(-(i * i) / (2 * sigma * sigma));
    s += k[i + r];
  }
  for (let i = 0; i < k.length; i++) k[i] /= s;
  return { k, r };
}

function blurPlane(src, w, h, sigma) {
  if (sigma <= 0.05) return src;
  const { k, r } = gaussKernel(sigma);
  const tmp = new Float32Array(w * h);
  const out = new Float32Array(w * h);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      let s = 0;
      for (let i = -r; i <= r; i++) {
        const xx = Math.min(w - 1, Math.max(0, x + i));
        s += src[y * w + xx] * k[i + r];
      }
      tmp[y * w + x] = s;
    }
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      let s = 0;
      for (let i = -r; i <= r; i++) {
        const yy = Math.min(h - 1, Math.max(0, y + i));
        s += tmp[yy * w + x] * k[i + r];
      }
      out[y * w + x] = s;
    }
  return out;
}

/**
 * src: {width, height, data: Uint8Array RGBA}
 * opts: camW, camH, fill (fraction of camera height covered by code height), rot (rad),
 *       tilt [tx, ty], shift [dx, dy] (fraction), k1 (barrel), blur, noise, gains [r,g,b],
 *       cross (3x3 row-major), black, vignette, chroma420, seed, bgLevel
 */
export function simulateCamera(src, o) {
  const W = o.camW || 1280;
  const H = o.camH || 720;
  const rng = new Rng(o.seed || 1);
  const gauss = () => {
    let u = 0;
    let v = 0;
    while (u === 0) u = rng.next() / 4294967296;
    v = rng.next() / 4294967296;
    return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
  };
  // placement quad in ideal camera coords
  const fill = o.fill ?? 0.8;
  const scale = (H * fill) / Math.max(src.height, src.width * (H / W) * 0.9);
  const cw = src.width * scale;
  const ch = src.height * scale;
  const cx = W / 2 + (o.shift ? o.shift[0] * W : 0);
  const cy = H / 2 + (o.shift ? o.shift[1] * H : 0);
  const rot = o.rot || 0;
  const [tx, ty] = o.tilt || [0, 0];
  const base = [
    [-cw / 2, -ch / 2],
    [cw / 2, -ch / 2],
    [cw / 2, ch / 2],
    [-cw / 2, ch / 2],
  ];
  const quad = base.map(([x, y]) => {
    // perspective-ish: shrink top vs bottom (tx) and left vs right (ty)
    const fx = 1 + (y < 0 ? -tx : tx) * 0.5;
    const fy = 1 + (x < 0 ? -ty : ty) * 0.5;
    x *= fx;
    y *= fy;
    const xr = x * Math.cos(rot) - y * Math.sin(rot);
    const yr = x * Math.sin(rot) + y * Math.cos(rot);
    return [cx + xr, cy + yr];
  });
  const srcPts = [
    [0, 0],
    [src.width, 0],
    [src.width, src.height],
    [0, src.height],
  ];
  const Hinv = homography(quad, srcPts); // camera ideal -> source px
  const k1 = o.k1 || 0;
  const f = Math.max(W, H) / 2;
  const planes = [new Float32Array(W * H), new Float32Array(W * H), new Float32Array(W * H)];
  const bg = o.bgLevel ?? 30;
  const sd = src.data;
  const tmp = [0, 0];
  const sub = o.subsample || 2;
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      let r = 0;
      let g = 0;
      let b = 0;
      for (let sy = 0; sy < sub; sy++)
        for (let sx = 0; sx < sub; sx++) {
          let ux = x + (sx + 0.5) / sub;
          let uy = y + (sy + 0.5) / sub;
          if (k1) {
            const nx = (ux - W / 2) / f;
            const ny = (uy - H / 2) / f;
            const rr = nx * nx + ny * ny;
            const s = 1 / (1 + k1 * rr);
            ux = W / 2 + nx * s * f;
            uy = H / 2 + ny * s * f;
          }
          applyH(Hinv, ux, uy, tmp);
          const px = Math.floor(tmp[0]);
          const py = Math.floor(tmp[1]);
          if (px < 0 || py < 0 || px >= src.width || py >= src.height) {
            r += bg;
            g += bg;
            b += bg;
          } else {
            const i = (py * src.width + px) * 4;
            r += sd[i];
            g += sd[i + 1];
            b += sd[i + 2];
          }
        }
      const n = sub * sub;
      planes[0][y * W + x] = r / n;
      planes[1][y * W + x] = g / n;
      planes[2][y * W + x] = b / n;
    }
  // optics blur (display glow + lens)
  for (let c = 0; c < 3; c++) planes[c] = blurPlane(planes[c], W, H, o.blur ?? 0.8);
  // motion blur: horizontal/vertical box of length o.motion px
  if (o.motion) {
    const L = Math.max(1, Math.round(o.motion));
    const vert = o.motionDir === 'v';
    for (let c = 0; c < 3; c++) {
      const src = planes[c];
      const dst = new Float32Array(W * H);
      for (let y = 0; y < H; y++)
        for (let x = 0; x < W; x++) {
          let acc = 0;
          for (let k = 0; k < L; k++) {
            const xx = vert ? x : Math.min(W - 1, x + k - (L >> 1));
            const yy = vert ? Math.min(H - 1, Math.max(0, y + k - (L >> 1))) : y;
            acc += src[Math.max(0, yy) * W + Math.max(0, xx)];
          }
          dst[y * W + x] = acc / L;
        }
      planes[c] = dst;
    }
  }
  // bloom: bright areas bleed light into their surroundings (sensor/lens halation)
  if (o.bloom) {
    for (let c = 0; c < 3; c++) {
      const hi = new Float32Array(W * H);
      for (let i = 0; i < W * H; i++) hi[i] = Math.max(0, planes[c][i] - 150);
      const halo = blurPlane(hi, W, H, o.bloomRadius ?? 2.5);
      for (let i = 0; i < W * H; i++) planes[c][i] += o.bloom * halo[i];
    }
  }
  // moiré: low-frequency brightness beating between screen pixels and sensor
  if (o.moire) {
    const f1 = 0.021, f2 = 0.017;
    for (let y = 0; y < H; y++)
      for (let x = 0; x < W; x++) {
        const m = 1 + o.moire * Math.sin(2 * Math.PI * (f1 * x + f2 * y * 1.3)) * Math.sin(2 * Math.PI * (0.013 * x - 0.019 * y));
        for (let c = 0; c < 3; c++) planes[c][y * W + x] *= m;
      }
  }
  // colour pipeline
  const M = o.cross || [1, 0, 0, 0, 1, 0, 0, 0, 1];
  const gains = o.gains || [1, 1, 1];
  const black = o.black ?? 8;
  const vig = o.vignette ?? 0;
  const out = new Uint8Array(W * H * 4);
  const Yp = new Float32Array(W * H);
  const Cb = new Float32Array(W * H);
  const Cr = new Float32Array(W * H);
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      const i = y * W + x;
      const r0 = planes[0][i] / 255;
      const g0 = planes[1][i] / 255;
      const b0 = planes[2][i] / 255;
      const dx = (x - W / 2) / (W / 2);
      const dy = (y - H / 2) / (H / 2);
      const v = 1 - vig * (dx * dx + dy * dy) * 0.5;
      let r = (M[0] * r0 + M[1] * g0 + M[2] * b0) * gains[0] * v;
      let g = (M[3] * r0 + M[4] * g0 + M[5] * b0) * gains[1] * v;
      let b = (M[6] * r0 + M[7] * g0 + M[8] * b0) * gains[2] * v;
      r = black + r * (255 - black);
      g = black + g * (255 - black);
      b = black + b * (255 - black);
      Yp[i] = 0.299 * r + 0.587 * g + 0.114 * b;
      Cb[i] = -0.1687 * r - 0.3313 * g + 0.5 * b;
      Cr[i] = 0.5 * r - 0.4187 * g - 0.0813 * b;
    }
  if (o.chroma420 !== false) {
    for (let y = 0; y < H; y += 2)
      for (let x = 0; x < W; x += 2) {
        const i = y * W + x;
        const j = Math.min(x + 1, W - 1);
        const k = Math.min(y + 1, H - 1) * W;
        const cb = (Cb[i] + Cb[y * W + j] + Cb[k + x] + Cb[k + j]) / 4;
        const cr = (Cr[i] + Cr[y * W + j] + Cr[k + x] + Cr[k + j]) / 4;
        Cb[i] = Cb[y * W + j] = Cb[k + x] = Cb[k + j] = cb;
        Cr[i] = Cr[y * W + j] = Cr[k + x] = Cr[k + j] = cr;
      }
  }
  const noise = o.noise ?? 3;
  for (let i = 0; i < W * H; i++) {
    const yv = Yp[i] + gauss() * noise;
    let r = yv + 1.402 * Cr[i];
    let g = yv - 0.344136 * Cb[i] - 0.714136 * Cr[i];
    let b = yv + 1.772 * Cb[i];
    out[i * 4] = Math.max(0, Math.min(255, Math.round(r + gauss() * noise * 0.5)));
    out[i * 4 + 1] = Math.max(0, Math.min(255, Math.round(g + gauss() * noise * 0.5)));
    out[i * 4 + 2] = Math.max(0, Math.min(255, Math.round(b + gauss() * noise * 0.5)));
    out[i * 4 + 3] = 255;
  }
  if (o.mirror) {
    for (let y = 0; y < H; y++)
      for (let x = 0; x < W >> 1; x++) {
        const a = (y * W + x) * 4;
        const b = (y * W + (W - 1 - x)) * 4;
        for (let c = 0; c < 4; c++) {
          const t = out[a + c];
          out[a + c] = out[b + c];
          out[b + c] = t;
        }
      }
  }
  return { width: W, height: H, data: out, quad };
}
