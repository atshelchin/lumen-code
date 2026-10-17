// Draws the figures used by docs/guide from the real layout code, so they never drift from the spec.
//   node src/tools/figures.js   -> docs/img/frame-anatomy.svg, docs/img/codeword-map.svg
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { makeLayout, K_DATA, K_FIXED, K_MARKER, K_HEADER, MODES, INK, PAPER, cellsPerCodeword } from '../core/layout.js';
import { Sender, preparePayload } from '../core/transfer.js';
import { IDX_PAPER } from '../core/frame.js';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const out = path.join(repo, 'docs/img');
fs.mkdirSync(out, { recursive: true });

const MODE = 1; // C4
const LEG = 300;
const rgb = (c) => `rgb(${c[0]},${c[1]},${c[2]})`;
const payload = await preparePayload(new Uint8Array(20000).map((_, i) => (i * 2654435761) >>> 24), 'figure.bin');

// one frame of a real transfer on an X × Y grid, drawn at S px per cell
function figure(X, Y, S, title) {
  const L = makeLayout(X, Y);
  const tx = new Sender(payload, { mode: MODE, ecc: 1, X, Y, tid: 0x4c55 });
  const { cells } = tx.nextFrame();
  const W = X * S;
  const H = Y * S;
  const s = [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W + LEG + 48} ${H + 48}" font-family="ui-sans-serif,system-ui,-apple-system,'PingFang SC','Microsoft YaHei',sans-serif">`,
    `<title>${title}</title>`,
    `<rect width="100%" height="100%" rx="16" fill="#0c1017"/>`,
    `<g transform="translate(24 24)">`,
    `<rect x="-6" y="-6" width="${W + 12}" height="${H + 12}" fill="${rgb(INK)}" rx="6"/>`,
  ];
  // cells are collected per colour and written as one path of horizontal runs (small files)
  const groups = new Map();
  const cellRect = (i, fill, extra = '') => {
    const key = fill + '|' + extra;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(i);
    return '';
  };
  const flush = () => {
    for (const [key, list] of groups) {
      const [fill, extra] = key.split('|');
      list.sort((a, b) => a - b);
      let d = '';
      for (let k = 0; k < list.length; ) {
        let e = k;
        while (e + 1 < list.length && list[e + 1] === list[e] + 1 && list[e + 1] % X !== 0) e++;
        const w = (e - k + 1) * S;
        d += `M${(list[k] % X) * S} ${Math.floor(list[k] / X) * S}h${w}v${S}h-${w}z`;
        k = e + 1;
      }
      s.push(`<path fill="${fill}"${extra} d="${d}"/>`);
    }
    groups.clear();
  };
  const legend = (items) => {
    const g = [`<g transform="translate(${W + 28} 8)">`];
    items.forEach(([fill, en, zh, stroke], k) => {
      const y = k * 46;
      g.push(`<rect x="0" y="${y}" width="22" height="22" rx="4" fill="${fill}"${stroke ? ` stroke="${stroke}" stroke-width="2"` : ''}/>`);
      g.push(`<text x="34" y="${y + 10}" fill="#e9edf4" font-size="14" font-weight="600">${en}</text>`);
      g.push(`<text x="34" y="${y + 28}" fill="#8b94a7" font-size="12.5">${zh}</text>`);
    });
    g.push('</g>');
    return g.join('\n');
  };
  return { X, Y, S, W, H, L, tx, cells, s, cellRect, flush, legend };
}

// ---------- 1. frame anatomy
{
  const { X, Y, S, W, H, L, cells, s, cellRect, flush, legend } = figure(48, 84, 7, 'Lumen Code frame anatomy');
  const HDR = ['#3b82f6', '#a855f7', '#f97316', '#14b8a6'];
  const hdrOf = new Int8Array(X * Y).fill(-1);
  L.headers.forEach((h, c) => h.forEach((i) => (hdrOf[i] = c)));
  const pal = MODES[MODE].palette;
  for (let i = 0; i < X * Y; i++) {
    const k = L.kind[i];
    if (k === K_DATA) cellRect(i, rgb(pal[cells[i]]), ' opacity="0.5"');
    else if (k === K_HEADER) cellRect(i, HDR[hdrOf[i]], cells[i] === IDX_PAPER ? '' : ' opacity="0.4"');
    else cellRect(i, rgb(L.fixed[i] ? PAPER : INK));
  }
  flush();
  // outlines: finders/corner, separators are the ink band inside them, markers
  const box = (x, y, w, h, c, r = 3) =>
    s.push(`<rect x="${x * S - 1.5}" y="${y * S - 1.5}" width="${w * S + 3}" height="${h * S + 3}" rx="${r}" fill="none" stroke="${c}" stroke-width="2.5"/>`);
  for (const [x, y] of [[0, 0], [X - 7, 0], [0, Y - 7]]) box(x, y, 7, 7, '#22d8ff');
  box(X - 7, Y - 7, 7, 7, '#ff3d9a');
  for (let i = 0; i < X * Y; i++)
    if (L.kind[i] === K_MARKER && L.fixed[i]) box((i % X) - 1, Math.floor(i / X) - 1, 3, 3, '#2bdc74', 2);
  s.push('</g>');
  s.push(
    legend([
      ['#f2f4f8', 'Finder × 3 (1:1:3:1:1)', '定位图案 ×3：TL、TR、BL', '#22d8ff'],
      ['#f2f4f8', 'Corner pattern (1:2:1:2:1)', '角标：BR，确定方向与透视', '#ff3d9a'],
      [rgb(INK), 'Separators (ink)', '分隔带：墨色，隔开图案与数据', '#5b6476'],
      [HDR[0], 'Header copy A (TL, right)', '帧头副本 A：70 格，独立掩码'],
      [HDR[1], 'Header copy B (TL, below)', '帧头副本 B'],
      [HDR[2], 'Header copy C (TR, left)', '帧头副本 C'],
      [HDR[3], 'Header copy D (BL, above)', '帧头副本 D'],
      ['#f2f4f8', 'Alignment markers (3×3)', '校准点阵：跟踪透视与畸变', '#2bdc74'],
      [rgb(pal[1]), 'Data cells (C4: 2 bits each)', '数据格子：码字沿 Hilbert 曲线排布'],
    ])
  );
  s.push(`<text x="${W + 52}" y="${H + 30}" fill="#5b6476" font-size="12">${X}×${Y} grid, C4 · ${(100 * L.dataCells / (X * Y)).toFixed(1)}% data cells</text>`);
  s.push('</svg>');
  fs.writeFileSync(path.join(out, 'frame-anatomy.svg'), s.join('\n') + '\n');
}

// ---------- 2. which cells belong to which codeword (Hilbert order keeps each one compact)
{
  const { X, Y, S, W, L, tx, s, cellRect, flush, legend } = figure(90, 160, 4, 'Lumen Code codeword placement');
  const cpc = cellsPerCodeword(tx.n, MODES[MODE].bpc);
  const m = tx.perFrame;
  const owner = new Int32Array(X * Y).fill(-2);
  L.dataOrder.forEach((cell, slot) => (owner[cell] = slot < m * cpc ? Math.floor(slot / cpc) : -1));
  for (let i = 0; i < X * Y; i++) {
    const o = owner[i];
    if (o >= 0) cellRect(i, `hsl(${(o * 137.508) % 360} 70% ${o % 2 ? 58 : 46}%)`);
    else if (o === -1) cellRect(i, '#2a3142');
    else cellRect(i, L.kind[i] === K_FIXED || L.kind[i] === K_MARKER ? (L.fixed[i] ? '#3a4255' : '#11151e') : '#1b2130');
  }
  flush();
  // a horizontal tear line: only the codewords it crosses are lost
  const ty = Math.round(Y * 0.56);
  const hit = new Set();
  for (let x = 0; x < X; x++) for (const yy of [ty - 1, ty]) if (owner[yy * X + x] >= 0) hit.add(owner[yy * X + x]);
  s.push(`<line x1="-6" x2="${W + 6}" y1="${ty * S}" y2="${ty * S}" stroke="#fff" stroke-width="2" stroke-dasharray="6 4"/>`);
  s.push('</g>');
  s.push(
    legend([
      ['hsl(200 70% 52%)', `Codeword = ${cpc} cells`, `每个码字 ${cpc} 格（n=${tx.n}，nsym=${tx.nsym}）`],
      ['hsl(337 70% 52%)', `${m} codewords per frame`, `每帧 ${m} 个码字，各自独立纠错`],
      ['#2a3142', 'Filler (padding)', '填充：剩余不足一个码字的格子'],
      ['#ffffff', `Tear line: ${hit.size} of ${m} lost`, `撕裂线：只丢失经过的 ${hit.size} 个码字`],
    ])
  );
  s.push('</svg>');
  fs.writeFileSync(path.join(out, 'codeword-map.svg'), s.join('\n') + '\n');
  console.log(`codeword map: ${m} codewords of ${cpc} cells; tear line crosses ${hit.size}`);
}
console.log('wrote docs/img/frame-anatomy.svg, docs/img/codeword-map.svg');
