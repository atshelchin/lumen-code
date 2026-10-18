#!/usr/bin/env python3
"""App icon: the brand mark (a 3x3 code) as PNG, zero dependencies.

usage: icon.py <size> <out.png> [--round] [--foreground]
  --round       circular mask (Android legacy round icon)
  --foreground  transparent background, mark only, inset for an adaptive icon
"""
import struct, sys, zlib, math

BG = (10, 13, 20)
DARK = (27, 33, 48)
COLS = [(34, 216, 255), DARK, (255, 61, 154), DARK, (242, 244, 248), (255, 214, 10), DARK, (34, 216, 255), (43, 220, 116)]

def rr(px, py, x0, y0, s, r):
    cx, cy = x0 + s / 2, y0 + s / 2
    dx = max(abs(px - cx) - (s / 2 - r), 0)
    dy = max(abs(py - cy) - (s / 2 - r), 0)
    return dx * dx + dy * dy <= r * r

def render(size, round_mask=False, foreground=False):
    ss = 3
    scale = size / 1024
    inset = 0.62 if foreground else 0.78
    grid = 1024 * inset
    cell = grid / (3 + 2 * 0.3)
    gap = cell * 0.3
    off = (1024 - grid) / 2
    rad = cell * 0.24
    rows = []
    for y in range(size):
        row = bytearray()
        for x in range(size):
            acc = [0, 0, 0, 0]
            for sy in range(ss):
                for sx in range(ss):
                    px = (x + (sx + 0.5) / ss) / scale
                    py = (y + (sy + 0.5) / ss) / scale
                    col, a = None, 0
                    if not foreground:
                        if round_mask and (px - 512) ** 2 + (py - 512) ** 2 > 512 ** 2:
                            col, a = (0, 0, 0), 0
                        else:
                            # soft glow behind the mark
                            d = math.hypot(px - 512, py - 430) / 720
                            g = max(0, 1 - d) ** 2 * 18
                            col, a = (BG[0] + g * 0.6, BG[1] + g * 0.9, BG[2] + g * 1.4), 255
                    for i in range(9):
                        cx0 = off + (i % 3) * (cell + gap)
                        cy0 = off + (i // 3) * (cell + gap)
                        if rr(px, py, cx0, cy0, cell, rad) and a != 0 or (foreground and rr(px, py, cx0, cy0, cell, rad)):
                            col, a = COLS[i], 255
                    if col is None:
                        col, a = (0, 0, 0), 0
                    acc[0] += col[0] * a / 255
                    acc[1] += col[1] * a / 255
                    acc[2] += col[2] * a / 255
                    acc[3] += a
            n = ss * ss
            alpha = acc[3] / n
            if alpha > 0:
                k = 255 / alpha
                row += bytes([min(255, int(acc[0] / n * k + 0.5)), min(255, int(acc[1] / n * k + 0.5)), min(255, int(acc[2] / n * k + 0.5)), int(alpha + 0.5)])
            else:
                row += b"\0\0\0\0"
        rows.append(bytes(row))
    return rows

def png(rows, size, path, opaque):
    raw = b"".join((b"\0" + (bytes(b for i, b in enumerate(r) if i % 4 != 3) if opaque else r)) for r in rows)
    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    ct = 2 if opaque else 6
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, ct, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(data)

if __name__ == "__main__":
    size, out = int(sys.argv[1]), sys.argv[2]
    rnd, fg = "--round" in sys.argv, "--foreground" in sys.argv
    png(render(size, rnd, fg), size, out, opaque=not (rnd or fg))
