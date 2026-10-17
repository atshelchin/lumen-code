//! Plain C ABI (no bindgen): used by WebAssembly (`src/core/wasm-engine.js`) and by native hosts
//! (Android JNI, iOS/Swift, desktop) through `include/lumen.h`. Enabled on wasm32, or with
//! the `ffi` feature on other targets.
//! All multi-byte values are little-endian. Handles are opaque pointers.

// C entry points take raw pointers by design; the host guarantees that handles come from
// the matching *_new function and that buffers are valid for the stated lengths.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::frame::Descriptor;
use crate::layout::Header;
use crate::scan::{FrameKey, Image, Scanner, SkipMap};
use crate::transfer::{prepare, Finish, Prepared, Receiver, Sender};
use std::alloc::{alloc, dealloc, Layout as AllocLayout};

#[no_mangle]
pub extern "C" fn lm_alloc(size: usize) -> *mut u8 {
    unsafe { alloc(AllocLayout::from_size_align(size.max(1), 8).unwrap()) }
}

#[no_mangle]
pub extern "C" fn lm_free(ptr: *mut u8, size: usize) {
    unsafe { dealloc(ptr, AllocLayout::from_size_align(size.max(1), 8).unwrap()) }
}

unsafe fn slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, len)
    }
}

// ------------------------------------------------------------------ scanner

pub struct WScanner {
    sc: Scanner,
    out: Vec<u8>,
}

#[no_mangle]
pub extern "C" fn lm_scanner_new() -> *mut WScanner {
    Box::into_raw(Box::new(WScanner { sc: Scanner::new(), out: Vec::new() }))
}

#[no_mangle]
pub extern "C" fn lm_scanner_free(h: *mut WScanner) {
    if !h.is_null() {
        unsafe { drop(Box::from_raw(h)) }
    }
}

fn reason_code(r: &str) -> u32 {
    match r {
        "" => 0,
        "finders" => 1,
        "geometry" => 2,
        "header" => 3,
        "homography" => 4,
        "image" => 5,
        _ => 6,
    }
}

fn put32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn putf(o: &mut Vec<u8>, v: f64) {
    o.extend_from_slice(&v.to_le_bytes());
}

/// skip: u32[] = count, then per frame: seq, x, y, mode, n, nsym, nIdx, idx...
/// Returns a pointer to the result record; its length is `lm_scanner_out_len`.
#[no_mangle]
pub extern "C" fn lm_scan(h: *mut WScanner, rgba: *const u8, w: u32, hgt: u32, skip: *const u32, skip_len: u32) -> *const u8 {
    let s = unsafe { &mut *h };
    let data = unsafe { slice(rgba, (w * hgt * 4) as usize) };
    let mut map = SkipMap::new();
    if skip_len > 0 {
        let sk = unsafe { std::slice::from_raw_parts(skip, skip_len as usize) };
        let mut p = 1;
        for _ in 0..sk[0] {
            if p + 7 > sk.len() {
                break;
            }
            let key = FrameKey { seq: sk[p], x: sk[p + 1], y: sk[p + 2], mode: sk[p + 3], n: sk[p + 4], nsym: sk[p + 5] };
            let ni = sk[p + 6] as usize;
            p += 7;
            let idx: Vec<usize> = sk[p..(p + ni).min(sk.len())].iter().map(|&v| v as usize).collect();
            p += ni;
            map.insert(key, idx);
        }
    }
    let res = s.sc.scan(&Image { width: w as usize, height: hgt as usize, rgba: data }, if map.is_empty() { None } else { Some(&map) });
    let o = &mut s.out;
    o.clear();
    put32(o, res.ok as u32);
    put32(o, res.skipped as u32);
    put32(o, reason_code(res.reason));
    put32(o, res.candidates as u32);
    let hd = res.header.unwrap_or(Header { mode: 0, x: 0, y: 0, n: 0, nsym: 0, seq: 0 });
    for v in [hd.mode, hd.x, hd.y, hd.n, hd.nsym, hd.seq] {
        put32(o, v);
    }
    let st = res.stats.unwrap_or_default();
    putf(o, st.cell_px);
    for c in st.corners {
        putf(o, c[0]);
        putf(o, c[1]);
    }
    putf(o, st.markers);
    for v in [st.corner_found as u32, st.cw_ok as u32, st.cw_total as u32, st.cw_count as u32, st.corrected as u32] {
        put32(o, v);
    }
    let pk: Vec<_> = res.codewords.iter().filter(|c| c.ok).collect();
    put32(o, pk.len() as u32);
    put32(o, pk.first().and_then(|c| c.bytes.as_ref()).map_or(0, |b| b.len() as u32));
    for c in pk {
        put32(o, c.index as u32);
        o.extend_from_slice(c.bytes.as_ref().unwrap());
    }
    o.as_ptr()
}

#[no_mangle]
pub extern "C" fn lm_scanner_out_len(h: *mut WScanner) -> u32 {
    unsafe { (*h).out.len() as u32 }
}

// ------------------------------------------------------------------ sender

pub struct WSender {
    tx: Sender,
    cells: Vec<u8>,
    info: [u32; 10],
}

/// Payload is already prepared (possibly DEFLATE-compressed) by the caller.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn lm_sender_new(
    payload: *const u8,
    len: u32,
    name: *const u8,
    name_len: u32,
    flags: u32,
    file_size: u32,
    crc: u32,
    mode: u32,
    ecc: u32,
    x: u32,
    y: u32,
    tid: u32,
    seq: u32,
) -> *mut WSender {
    let payload = unsafe { slice(payload, len as usize) }.to_vec();
    let name = String::from_utf8_lossy(unsafe { slice(name, name_len as usize) }).into_owned();
    let prepared = Prepared { descriptor: Descriptor { flags: flags as u8, payload_length: len, file_size, crc, name }, payload };
    match Sender::new(&prepared, mode as usize, ecc as usize, x as usize, y as usize, tid as u16, seq) {
        Some(tx) => Box::into_raw(Box::new(WSender { tx, cells: Vec::new(), info: [0; 10] })),
        None => std::ptr::null_mut(),
    }
}

/// Like `lm_sender_new`, but starts from the file itself: computes the CRC-32 and compresses
/// with DEFLATE when that helps (`compress` = 1), so a native host needs no zlib.
#[no_mangle]
pub extern "C" fn lm_sender_new_file(
    file: *const u8,
    len: u32,
    name: *const u8,
    name_len: u32,
    compress: u32,
    mode: u32,
    ecc: u32,
    x: u32,
    y: u32,
    tid: u32,
    seq: u32,
) -> *mut WSender {
    let file = unsafe { slice(file, len as usize) };
    let name = String::from_utf8_lossy(unsafe { slice(name, name_len as usize) }).into_owned();
    let prepared = prepare(file, &name, compress != 0);
    match Sender::new(&prepared, mode as usize, ecc as usize, x as usize, y as usize, tid as u16, seq) {
        Some(tx) => Box::into_raw(Box::new(WSender { tx, cells: Vec::new(), info: [0; 10] })),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn lm_sender_free(h: *mut WSender) {
    if !h.is_null() {
        unsafe { drop(Box::from_raw(h)) }
    }
}

/// u32[10]: n, nsym, T, perFrame, K, X, Y, mode, ecc, blocks
#[no_mangle]
pub extern "C" fn lm_sender_info(h: *mut WSender) -> *const u32 {
    let s = unsafe { &mut *h };
    let t = &s.tx;
    s.info = [t.n as u32, t.nsym as u32, t.t as u32, t.per_frame as u32, t.k(), t.layout.x as u32, t.layout.y as u32, t.mode as u32, t.ecc as u32, t.plan.z];
    s.info.as_ptr()
}

/// Cells of the next frame (X·Y bytes); the frame's seq is returned by `lm_sender_last_seq`.
#[no_mangle]
pub extern "C" fn lm_sender_next_frame(h: *mut WSender) -> *const u8 {
    let s = unsafe { &mut *h };
    let f = s.tx.next_frame();
    s.info[0] = f.seq;
    s.cells = f.cells;
    s.cells.as_ptr()
}

#[no_mangle]
pub extern "C" fn lm_sender_resize(h: *mut WSender, x: u32, y: u32) -> u32 {
    unsafe { (*h).tx.resize(x as usize, y as usize) as u32 }
}

#[no_mangle]
pub extern "C" fn lm_sender_reconfigure(h: *mut WSender, mode: u32, ecc: u32) -> u32 {
    unsafe { (*h).tx.reconfigure(mode as usize, ecc as usize) as u32 }
}

// ------------------------------------------------------------------ receiver

pub struct WReceiver {
    rx: Receiver,
    file: Vec<u8>,
    name: Vec<u8>,
    prog: [u32; 8],
}

#[no_mangle]
pub extern "C" fn lm_receiver_new() -> *mut WReceiver {
    Box::into_raw(Box::new(WReceiver { rx: Receiver::new(), file: Vec::new(), name: Vec::new(), prog: [0; 8] }))
}

#[no_mangle]
pub extern "C" fn lm_receiver_free(h: *mut WReceiver) {
    if !h.is_null() {
        unsafe { drop(Box::from_raw(h)) }
    }
}

#[no_mangle]
pub extern "C" fn lm_receiver_push(h: *mut WReceiver, ptr: *const u8, len: u32) {
    let r = unsafe { &mut *h };
    r.rx.push(unsafe { slice(ptr, len as usize) });
}

/// u32[8]: valid, tid, received, needed (0 = unknown), blocks, done, T, fileSize; name via lm_receiver_name.
#[no_mangle]
pub extern "C" fn lm_receiver_progress(h: *mut WReceiver) -> *const u32 {
    let r = unsafe { &mut *h };
    r.prog = [0; 8];
    if let Some(p) = r.rx.progress() {
        r.prog = [1, p.tid as u32, p.received, p.needed.unwrap_or(0), p.blocks, p.done as u32, p.t as u32, p.size.unwrap_or(0)];
        r.name = p.name.unwrap_or_default().into_bytes();
    }
    r.prog.as_ptr()
}

#[no_mangle]
pub extern "C" fn lm_receiver_name(h: *mut WReceiver) -> *const u8 {
    unsafe { (*h).name.as_ptr() }
}

#[no_mangle]
pub extern "C" fn lm_receiver_name_len(h: *mut WReceiver) -> u32 {
    unsafe { (*h).name.len() as u32 }
}

/// 0 = not yet, 1 = file ready (lm_receiver_file*), 2 = checksum error.
#[no_mangle]
pub extern "C" fn lm_receiver_finish(h: *mut WReceiver) -> u32 {
    let r = unsafe { &mut *h };
    match r.rx.try_finish() {
        Some(Finish::File { name, bytes }) => {
            r.file = bytes;
            r.name = name.into_bytes();
            1
        }
        Some(Finish::ChecksumError) => 2,
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn lm_receiver_file(h: *mut WReceiver) -> *const u8 {
    unsafe { (*h).file.as_ptr() }
}

#[no_mangle]
pub extern "C" fn lm_receiver_file_len(h: *mut WReceiver) -> u32 {
    unsafe { (*h).file.len() as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_from_file_matches_prepared_payload() {
        let file: Vec<u8> = (0..5000u32).map(|i| b"lumen code "[(i % 11) as usize]).collect();
        let name = b"note.txt";
        let p = prepare(&file, "note.txt", true);
        let d = &p.descriptor;
        let a = lm_sender_new(p.payload.as_ptr(), p.payload.len() as u32, name.as_ptr(), 8, d.flags as u32, d.file_size, d.crc, 1, 1, 90, 150, 0x1234, 7);
        let b = lm_sender_new_file(file.as_ptr(), file.len() as u32, name.as_ptr(), 8, 1, 1, 1, 90, 150, 0x1234, 7);
        assert!(!a.is_null() && !b.is_null());
        for _ in 0..3 {
            let fa = unsafe { std::slice::from_raw_parts(lm_sender_next_frame(a), 90 * 150) }.to_vec();
            let fb = unsafe { std::slice::from_raw_parts(lm_sender_next_frame(b), 90 * 150) }.to_vec();
            assert_eq!(fa, fb);
        }
        lm_sender_free(a);
        lm_sender_free(b);
    }
}
