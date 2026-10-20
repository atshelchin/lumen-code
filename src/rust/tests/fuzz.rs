// Differential and property tests against random and hostile input. The corpora are those of
// test/corpus.js (same seeds, same order of Rng calls), so the digests of the outcomes must be
// the ones of the JS implementation in test/vectors/test-vectors.json ("differential").
//   cargo test --release --test fuzz            quick run
//   FUZZ=20 cargo test --release --test fuzz    20 times more cases
mod json;
use json::{parse, J};
use lumen::deflate::inflate;
use lumen::fountain::{Decoder, Encoder};
use lumen::frame::{encode_descriptor, parse_descriptor, seal_packet, write_packet_header, Descriptor, PKT_HDR};
use lumen::layout::{cells_per_codeword, data_cell_count, decode_header, header_mask, lattice_axis, pack_header, Layout, MODES};
use lumen::transfer::{block_plan, fountain_seed, Finish, Limits, Receiver};
use lumen::util::{crc16_bits, crc32, Rng};

fn vectors() -> J {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/vectors/test-vectors.json");
    parse(&std::fs::read_to_string(path).expect("test/vectors/test-vectors.json"))
}

fn scale(n: usize) -> usize {
    n * std::env::var("FUZZ").ok().and_then(|v| v.parse().ok()).unwrap_or(1)
}

// ------------------------------------------------------------------ corpora (test/corpus.js)

fn raw_header_bits(f: [u32; 8], copy: usize) -> [u8; 70] {
    let [version, mode, x, y, n, nsym, seq, reserved] = f;
    let mut bits = [0u8; 70];
    let mut p = 0;
    for (v, w) in [(version, 4), (mode, 2), (x, 9), (y, 9), (n, 8), (nsym >> 1, 7), (seq, 8), (reserved, 7)] {
        for i in (0..w).rev() {
            bits[p] = ((v >> i) & 1) as u8;
            p += 1;
        }
    }
    let crc = crc16_bits(&bits[..54]) as u32;
    for i in (0..16).rev() {
        bits[p] = ((crc >> i) & 1) as u8;
        p += 1;
    }
    for (i, b) in bits.iter_mut().enumerate() {
        *b ^= header_mask(copy, i);
    }
    bits
}

fn header_corpus(seed: u32, count: usize) -> Vec<([u8; 70], usize)> {
    let mut r = Rng::new(seed);
    (0..count)
        .map(|i| {
            let version = if r.next() % 4 == 0 { r.next() % 16 } else { 1 };
            let mode = r.next() % 4;
            let x = if r.next() % 2 == 0 { 40 + r.next() % 472 } else { r.next() % 512 };
            let y = if r.next() % 2 == 0 { 40 + r.next() % 472 } else { r.next() % 512 };
            let n = r.next() % 256;
            let nsym = 2 * (r.next() % 128);
            let seq = r.next() % 256;
            let reserved = r.next() % 128;
            let copy = i % 4;
            let mut bits = raw_header_bits([version, mode, x, y, n, nsym, seq, reserved], copy);
            if r.next() % 8 == 0 {
                bits[(r.next() % 70) as usize] ^= 1;
            }
            (bits, copy)
        })
        .collect()
}

fn descriptor_corpus(seed: u32, count: usize) -> Vec<Vec<u8>> {
    let mut r = Rng::new(seed);
    (0..count)
        .map(|_| {
            let t = 16 + (r.next() % 240) as usize;
            let mut d: Vec<u8> = (0..t).map(|_| r.next() as u8).collect();
            if r.next() % 8 != 0 {
                d[0] = 1;
            }
            if r.next() % 8 != 0 {
                d[1] &= 1;
            }
            if r.next() % 4 != 0 {
                d[14] = (r.next() % (t as u32 - 14)) as u8;
            }
            if r.next() % 4 != 0 {
                d[6] = 0;
                d[7] = 0;
            }
            if r.next() % 4 != 0 {
                d.copy_within(6..10, 2);
            }
            if r.next() % 4 != 0 {
                for j in 15..(15 + d[14] as usize).min(t) {
                    d[j] = 0x20 + (r.next() % 95) as u8;
                }
            }
            if r.next() % 2 == 0 {
                let from = t.min(15 + d[14] as usize);
                d[from..].fill(0);
            }
            d
        })
        .collect()
}

fn inflate_corpus(seed: u32, count: usize, bases: &[Vec<u8>], file_len: usize) -> Vec<(Vec<u8>, usize)> {
    let mut r = Rng::new(seed);
    (0..count)
        .map(|_| {
            let mut s = bases[(r.next() as usize) % bases.len()].clone();
            match r.next() % 4 {
                0 => {
                    let at = r.next() as usize % s.len();
                    s[at] ^= 1 << (r.next() % 8);
                }
                1 => {
                    let len = r.next() as usize % s.len();
                    s.truncate(len);
                }
                2 => {
                    let at = r.next() as usize % s.len();
                    s[at] = r.next() as u8;
                }
                _ => s.push(r.next() as u8),
            }
            let limit = if r.next() % 2 == 0 { file_len } else { r.next() as usize % (file_len + 1) };
            (s, limit)
        })
        .collect()
}

struct FountainCase {
    k: u32,
    seed: u32,
    block: Vec<u8>,
    esis: Vec<u32>,
}

fn fountain_corpus(seed: u32, count: usize) -> Vec<FountainCase> {
    let mut r = Rng::new(seed);
    (0..count)
        .map(|_| {
            let k = 1 + r.next() % 60;
            let tid = (r.next() & 0xffff) as u16;
            let block: Vec<u8> = (0..k * 4).map(|_| r.next() as u8).collect();
            let n = (k - 1 + r.next() % 5) as usize;
            let mut esis = Vec::new();
            while esis.len() < n {
                let e = r.next() & 0xffff;
                if !esis.contains(&e) {
                    esis.push(e);
                }
            }
            FountainCase { k, seed: fountain_seed(tid, 0), block, esis }
        })
        .collect()
}

// ------------------------------------------------------------------ differential digests

#[test]
fn differential_digests_match_js() {
    let v = vectors();
    let dv = v.get("differential");

    let hv = dv.get("header");
    let mut rec = Vec::new();
    for (bits, copy) in header_corpus(hv.get("seed").u() as u32, hv.get("count").u() as usize) {
        match decode_header(&bits, copy) {
            Err(_) => rec.push(0u8),
            Ok(h) => rec.extend_from_slice(&[1, h.mode as u8, (h.x >> 8) as u8, h.x as u8, (h.y >> 8) as u8, h.y as u8, h.n as u8, h.nsym as u8, h.seq as u8]),
        }
    }
    assert_eq!(crc32(&rec) as u64, hv.get("digest").u(), "header decisions differ from JS");

    let ddv = dv.get("descriptor");
    let mut rec = Vec::new();
    for d in descriptor_corpus(ddv.get("seed").u() as u32, ddv.get("count").u() as usize) {
        match parse_descriptor(&d) {
            Err(_) => rec.push(0u8),
            Ok(x) => {
                rec.extend_from_slice(&[1, x.flags]);
                rec.extend_from_slice(&x.payload_length.to_be_bytes());
                rec.extend_from_slice(&x.file_size.to_be_bytes());
                rec.extend_from_slice(&x.crc.to_be_bytes());
                rec.push(x.name.len() as u8);
                rec.extend_from_slice(x.name.as_bytes());
                rec.push(x.records.len() as u8);
                for (t, val) in &x.records {
                    rec.extend_from_slice(&[*t, val.len() as u8]);
                    rec.extend_from_slice(val);
                }
            }
        }
    }
    assert_eq!(crc32(&rec) as u64, ddv.get("digest").u(), "descriptor decisions differ from JS");

    let iv = dv.get("inflate");
    let bases: Vec<Vec<u8>> = iv.get("bases").arr().iter().map(|b| b.hex()).collect();
    let mut rec = Vec::new();
    for (data, limit) in inflate_corpus(iv.get("seed").u() as u32, iv.get("count").u() as usize, &bases, iv.get("fileLen").u() as usize) {
        match inflate(&data, limit) {
            None => rec.push(0u8),
            Some(out) => {
                rec.push(1);
                rec.extend_from_slice(&(out.len() as u32).to_be_bytes());
                rec.extend_from_slice(&crc32(&out).to_be_bytes());
            }
        }
    }
    assert_eq!(crc32(&rec) as u64, iv.get("digest").u(), "inflate decisions differ from JS");

    let fv = dv.get("fountain");
    let rec: Vec<u8> = fountain_corpus(fv.get("seed").u() as u32, fv.get("count").u() as usize).iter().map(|c| decodes(c) as u8).collect();
    assert_eq!(crc32(&rec) as u64, fv.get("digest").u(), "fountain decodability differs from JS");
}

/// Decode a fountain case; a decoder must never return a wrong block.
fn decodes(c: &FountainCase) -> bool {
    let enc = Encoder::new(&c.block, 4, c.seed);
    let mut dec = Decoder::new(c.k, 4, c.seed);
    for &e in &c.esis {
        let mut s = [0u8; 4];
        enc.encode(e, &mut s);
        dec.add(e, &s);
    }
    match dec.decode() {
        Some(out) => {
            assert_eq!(out, c.block, "fountain decoder returned a wrong block");
            true
        }
        None => false,
    }
}

// ------------------------------------------------------------------ properties

#[test]
fn grid_kinds_never_overlap() {
    // the inner markers of every axis stay clear of the header copies (x = 8 … 17 and
    // X − 18 … X − 9; the same in y), and lattice points are 16 … 24 cells apart
    for d in 40..=511 {
        let l = lattice_axis(d);
        assert_eq!((l[0], *l.last().unwrap()), (3, d - 4));
        for w in l.windows(2) {
            assert!((16..=24).contains(&(w[1] - w[0])), "gap {d}: {l:?}");
        }
        for &p in &l[1..l.len() - 1] {
            assert!(p > 18 && p < d - 19, "inner marker near a corner {d}: {l:?}"); // cells p − 1 … p + 1
        }
    }
    let mut r = Rng::new(5);
    let mut sizes = vec![(40, 40), (40, 511), (511, 40), (511, 511), (41, 57), (57, 41)];
    for _ in 0..scale(60) {
        sizes.push((40 + (r.next() % 472) as usize, 40 + (r.next() % 472) as usize));
    }
    for (x, y) in sizes {
        let l = Layout::new(x, y).unwrap();
        let mut count = [0usize; 4];
        for &k in &l.kind {
            count[k as usize] += 1;
        }
        assert_eq!(count[1..], [4 * 49 + 4 * 15, (l.lx.len() * l.ly.len() - 4) * 9, 280], "kinds {x}x{y}");
        assert_eq!(l.data_cells(), data_cell_count(x, y));
    }
}

#[test]
fn headers_random_fields() {
    for (bits, copy) in header_corpus(1001, scale(40000)) {
        let Ok(h) = decode_header(&bits, copy) else { continue };
        assert!(h.mode <= 2 && h.x >= 40 && h.y >= 40 && h.nsym >= 2 && h.n - h.nsym >= 23, "{h:?}");
        assert!(data_cell_count(h.x as usize, h.y as usize) / cells_per_codeword(h.n as usize, MODES[h.mode as usize].bpc) >= 1);
        assert_eq!(decode_header(&pack_header(&h, copy), copy), Ok(h));
    }
}

#[test]
fn descriptors_random_bytes() {
    for d in descriptor_corpus(1002, scale(40000)) {
        let Ok(x) = parse_descriptor(&d) else { continue };
        let t = d.len();
        assert!(x.name.len() == d[14] as usize && x.name.len() <= t - 15);
        assert!(x.flags & !1 == 0 && (x.flags & 1 == 1 || x.payload_length == x.file_size));
        assert!(block_plan(x.payload_length as u64, t).unwrap().z <= 128);
        let end = 15 + x.name.len() + x.records.iter().map(|(_, v)| 2 + v.len()).sum::<usize>();
        assert!(end <= t);
        let back = parse_descriptor(&encode_descriptor(&x, t)).unwrap();
        assert_eq!((back.name, back.flags, back.payload_length, back.file_size, back.crc), (x.name, x.flags, x.payload_length, x.file_size, x.crc));
    }
}

#[test]
fn reed_solomon_random_damage() {
    let mut r = Rng::new(1003);
    for _ in 0..scale(3000) {
        let nsym = 2 * (1 + (r.next() % 30)) as usize;
        let k = 1 + (r.next() as usize % (255 - nsym));
        let msg: Vec<u8> = (0..k).map(|_| r.next() as u8).collect();
        let cw = lumen::rs::encode(&msg, nsym);
        let n = cw.len();
        let within = r.next() % 2 == 0;
        let budget = if within { nsym } else { nsym + 1 + r.next() as usize % nsym };
        let f = n.min(r.next() as usize % (budget + 1));
        let e = (n - f).min((budget - f) / 2 + if within { 0 } else { 1 });
        let mut pos: Vec<usize> = (0..n).collect();
        for j in (1..n).rev() {
            let t = r.next() as usize % (j + 1);
            pos.swap(j, t);
        }
        let mut bad = cw.clone();
        for &p in &pos[..e] {
            bad[p] ^= 1 + (r.next() % 255) as u8;
        }
        let er: Vec<usize> = pos[e..e + f.min(n - e)].to_vec();
        for &p in &er {
            bad[p] = r.next() as u8;
        }
        let res = lumen::rs::decode(&mut bad, nsym, &er);
        if 2 * e + er.len() <= nsym {
            assert!(res.is_some() && bad == cw, "inside the bound n={n} nsym={nsym} e={e} f={}", er.len());
        } else if res.is_some() {
            assert_eq!(lumen::rs::encode(&bad[..k], nsym), bad, "a result outside the bound must be a codeword");
        }
    }
}

#[test]
fn inflate_hostile_streams() {
    let v = vectors();
    let bases: Vec<Vec<u8>> = v.get("differential").get("inflate").get("bases").arr().iter().map(|b| b.hex()).collect();
    for (data, limit) in inflate_corpus(1004, scale(10000), &bases, 588) {
        if let Some(out) = inflate(&data, limit) {
            assert!(out.len() <= limit);
        }
    }
    let mut r = Rng::new(1005);
    for _ in 0..scale(20000) {
        let data: Vec<u8> = (0..1 + r.next() % 64).map(|_| r.next() as u8).collect();
        if let Some(out) = inflate(&data, 1000) {
            assert!(out.len() <= 1000);
        }
    }
    // the crate's own compressor round trips through the strict inflater
    for i in 0..scale(40) {
        let words = ["lumen ", "code ", "frame ", "camera ", "\n", "fountain ", "0123 "];
        let text: String = (0..50 + (r.next() % 2000)).map(|_| words[r.next() as usize % words.len()]).collect();
        let z = lumen::deflate::deflate(text.as_bytes());
        assert_eq!(inflate(&z, text.len()).as_deref(), Some(text.as_bytes()), "round trip {i}");
        assert!(inflate(&z, text.len() - 1).is_none());
    }
}

#[test]
fn fountain_random_symbol_sets() {
    for c in fountain_corpus(1006, scale(1500)) {
        decodes(&c);
    }
}

#[test]
fn receiver_hostile_streams() {
    let mut r = Rng::new(1007);
    let t = 24usize;
    let limits = Limits { max_size: 5000, max_kept: 50, max_transfers: 3 };
    let (mut delivered, mut wrong) = (0, 0);
    for run in 0..scale(60) {
        struct F {
            tid: u16,
            file: Vec<u8>,
            kt: u32,
            enc: Encoder,
            desc: Vec<u8>,
        }
        let tids = [(r.next() & 0xffff) as u16, (r.next() & 0xffff) as u16];
        let files: Vec<F> = (0..3)
            .map(|j| {
                let size = if j == 2 && run % 3 == 0 { 6000 } else { 1 + (r.next() % 2000) as usize };
                let file: Vec<u8> = (0..size).map(|_| r.next() as u8).collect();
                let tid = tids[if j == 2 { 0 } else { j }];
                let kt = block_plan(size as u64, t).unwrap().kt;
                let mut padded = file.clone();
                padded.resize(kt as usize * t, 0);
                let d = Descriptor { flags: 0, payload_length: size as u32, file_size: size as u32, crc: crc32(&file), name: format!("f{j}"), records: Vec::new() };
                F { tid, kt, enc: Encoder::new(&padded, t, fountain_seed(tid, 0)), desc: encode_descriptor(&d, t), file }
            })
            .collect();
        let mut rx = Receiver::with_limits(limits);
        for _ in 0..1500 {
            let f = &files[(r.next() % 3) as usize];
            let kind = r.next() % 16;
            let mut msg = vec![0u8; t + 7];
            match kind {
                0 => msg.iter_mut().for_each(|b| *b = r.next() as u8),
                1 => {
                    write_packet_header(&mut msg, f.tid, r.next() % 128, r.next() & 0xffff, true);
                    seal_packet(&mut msg);
                }
                2 => {
                    write_packet_header(&mut msg, f.tid, 0, r.next() & 0xffff, false);
                    msg[PKT_HDR..PKT_HDR + t].iter_mut().for_each(|b| *b = r.next() as u8);
                    seal_packet(&mut msg);
                }
                3..=6 => {
                    write_packet_header(&mut msg, f.tid, 0, 0, true);
                    msg[PKT_HDR..PKT_HDR + t].copy_from_slice(&f.desc);
                    if kind == 3 {
                        let at = PKT_HDR + 2 + (r.next() % 12) as usize;
                        msg[at] ^= 1 << (r.next() % 8);
                    }
                    seal_packet(&mut msg);
                }
                _ => {
                    let esi = r.next() % (f.kt * 3);
                    let sbn = if r.next() % 16 == 0 { 1 } else { 0 };
                    write_packet_header(&mut msg, f.tid, sbn, esi, false);
                    f.enc.encode(esi, &mut msg[PKT_HDR..PKT_HDR + t]);
                    seal_packet(&mut msg);
                }
            }
            rx.push(&msg);
            if let Some(Finish::File { bytes, .. }) = rx.try_finish() {
                delivered += 1;
                match files.iter().find(|x| x.file == bytes) {
                    Some(x) => assert!(x.file.len() as u64 <= limits.max_size),
                    None => wrong += 1,
                }
            }
        }
    }
    assert_eq!(wrong, 0, "wrong files delivered");
    assert!(delivered > 0);
}
