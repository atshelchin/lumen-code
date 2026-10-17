/* Lumen Code v1 — C ABI of the Rust implementation (build with `--features ffi`,
 * or target wasm32). All handles are opaque. Multi-byte values in returned records are
 * little-endian. Memory returned by the library stays valid until the next call on the
 * same handle. */
#ifndef LUMEN_H
#define LUMEN_H
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef struct WScanner WScanner;
typedef struct WSender WSender;
typedef struct WReceiver WReceiver;

uint8_t *lm_alloc(size_t size);
void lm_free(uint8_t *ptr, size_t size);

/* ---- scanner: camera image (RGBA) -> packets ----
 * skip: optional u32 list [count, then per frame: seq, X, Y, mode, n, nsym, nIdx, idx...]
 * Result record (see rust/src/ffi.rs, lm_scan): ok, skipped, reason, candidates (u32),
 * header mode, X, Y, n, nsym, seq (u32; 0xFFFFFFFF = unknown: no header copy was readable and the
 * scanner reused the last good header, so the frame cannot be identified), cellPx (f64), corners[8] (f64), markers (f64),
 * cornerFound, cwOk, cwTotal, cwCount, corrected (u32), nPackets, packetLen (u32),
 * then nPackets x (u32 codeword index, packetLen bytes). */
WScanner *lm_scanner_new(void);
void lm_scanner_free(WScanner *h);
const uint8_t *lm_scan(WScanner *h, const uint8_t *rgba, uint32_t width, uint32_t height,
                       const uint32_t *skip, uint32_t skip_len);
uint32_t lm_scanner_out_len(WScanner *h);

/* ---- sender: prepared payload -> frames (cell values) ---- */
WSender *lm_sender_new(const uint8_t *payload, uint32_t len, const uint8_t *name, uint32_t name_len,
                       uint32_t flags, uint32_t file_size, uint32_t crc32, uint32_t mode, uint32_t ecc,
                       uint32_t x, uint32_t y, uint32_t tid, uint32_t seq);
/* the same, from the file itself: computes CRC-32 and DEFLATE-compresses when useful (compress = 1) */
WSender *lm_sender_new_file(const uint8_t *file, uint32_t len, const uint8_t *name, uint32_t name_len,
                            uint32_t compress, uint32_t mode, uint32_t ecc, uint32_t x, uint32_t y,
                            uint32_t tid, uint32_t seq);
void lm_sender_free(WSender *h);
/* u32[10]: n, nsym, T, perFrame, K, X, Y, mode, ecc, blocks */
const uint32_t *lm_sender_info(WSender *h);
/* X*Y cell values of the next frame; afterwards slot 0 of the info array holds its seq */
const uint8_t *lm_sender_next_frame(WSender *h);
uint32_t lm_sender_resize(WSender *h, uint32_t x, uint32_t y);
uint32_t lm_sender_reconfigure(WSender *h, uint32_t mode, uint32_t ecc);

/* ---- receiver: packets -> file ---- */
WReceiver *lm_receiver_new(void);
void lm_receiver_free(WReceiver *h);
void lm_receiver_push(WReceiver *h, const uint8_t *packet, uint32_t len);
/* u32[8]: valid, tid, received, needed, blocks, done, T, fileSize */
const uint32_t *lm_receiver_progress(WReceiver *h);
const uint8_t *lm_receiver_name(WReceiver *h);
uint32_t lm_receiver_name_len(WReceiver *h);
/* 0 = not yet, 1 = file ready, 2 = checksum error */
uint32_t lm_receiver_finish(WReceiver *h);
const uint8_t *lm_receiver_file(WReceiver *h);
uint32_t lm_receiver_file_len(WReceiver *h);

#ifdef __cplusplus
}
#endif
#endif
