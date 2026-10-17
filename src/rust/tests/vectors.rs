// Conformance: reproduce test/vectors/test-vectors.json exactly.
mod json;
use json::{parse, J};
use lumen::fountain::{self, Encoder};
use lumen::frame::{encode_descriptor, seal_packet, write_packet_header, Descriptor, PKT_HDR};
use lumen::layout::{cells_per_codeword, choose_codeword, codewords_per_frame, pack_header, Header, Layout};
use lumen::transfer::{block_plan, fountain_seed, Prepared, Sender};
use lumen::util::{crc16, crc32, mix32, Rng};

fn vectors() -> J {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/vectors/test-vectors.json");
    parse(&std::fs::read_to_string(path).expect("test/vectors/test-vectors.json"))
}

fn crc_of_u32s(v: &[u32]) -> u32 {
    let mut b = Vec::with_capacity(v.len() * 4);
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
    crc32(&b)
}

#[test]
fn checksums_hash_rng() {
    let v = vectors();
    for e in v.get("crc32").arr() {
        assert_eq!(crc32(e.get("ascii").s().as_bytes()) as u64, e.get("crc32").u());
    }
    for e in v.get("crc16").arr() {
        assert_eq!(crc16(e.get("ascii").s().as_bytes()) as u64, e.get("crc16").u());
    }
    for e in v.get("mix32").arr() {
        assert_eq!(mix32(e.get("x").u() as u32) as u64, e.get("out").u());
    }
    for e in v.get("rng").arr() {
        let mut r = Rng::new(e.get("seed").u() as u32);
        for x in e.get("first8").us() {
            assert_eq!(r.next() as u64, x);
        }
    }
}

#[test]
fn reed_solomon() {
    let v = vectors();
    for e in v.get("reedSolomon").arr() {
        let nsym = e.get("nsym").u() as usize;
        let cw = lumen::rs::encode(&e.get("message").hex(), nsym);
        assert_eq!(cw, e.get("codeword").hex());
        // decode with errors
        let mut bad = cw.clone();
        for i in 0..nsym / 2 {
            bad[i * 3 % cw.len()] ^= 0x5a;
        }
        assert!(lumen::rs::decode(&mut bad, nsym, &[]).is_some());
        assert_eq!(bad, cw);
    }
}

#[test]
fn layouts_and_headers() {
    let v = vectors();
    for e in v.get("layouts").arr() {
        let (x, y) = (e.get("X").u() as usize, e.get("Y").u() as usize);
        let l = Layout::new(x, y).unwrap();
        assert_eq!(l.lx.iter().map(|&a| a as u64).collect::<Vec<_>>(), e.get("lx").us());
        assert_eq!(l.ly.iter().map(|&a| a as u64).collect::<Vec<_>>(), e.get("ly").us());
        assert_eq!(l.data_cells() as u64, e.get("dataCells").u());
        assert_eq!(crc32(&l.kind) as u64, e.get("kindCrc32").u(), "kind {x}x{y}");
        assert_eq!(crc32(&l.fixed) as u64, e.get("fixedCrc32").u());
        assert_eq!(l.data_order[..32].iter().map(|&a| a as u64).collect::<Vec<_>>(), e.get("dataOrderFirst32").us());
        assert_eq!(crc_of_u32s(&l.data_order) as u64, e.get("dataOrderCrc32").u());
        for c in 0..4 {
            assert_eq!(crc_of_u32s(&l.headers[c]) as u64, e.get("headerCellsCrc32").at(c).u());
        }
        assert_eq!(crc32(&l.whiten) as u64, e.get("whitenCrc32").u());
    }
    for e in v.get("headers").arr() {
        let f = e.get("fields");
        let h = Header {
            mode: f.get("mode").u() as u32,
            x: f.get("X").u() as u32,
            y: f.get("Y").u() as u32,
            n: f.get("n").u() as u32,
            nsym: f.get("nsym").u() as u32,
            seq: f.get("seq").u() as u32,
        };
        for c in 0..4 {
            let bits: String = pack_header(&h, c).iter().map(|b| if *b == 1 { '1' } else { '0' }).collect();
            assert_eq!(bits, e.get("copies").at(c).s());
            assert_eq!(lumen::layout::unpack_header(&pack_header(&h, c), c), Some(h));
        }
    }
    for e in v.get("codewordChoice").arr() {
        let l = Layout::new(e.get("X").u() as usize, e.get("Y").u() as usize).unwrap();
        let mode = e.get("mode").u() as usize;
        let (n, nsym) = choose_codeword(&l, mode, e.get("eccLevel").u() as usize);
        assert_eq!((n as u64, nsym as u64), (e.get("n").u(), e.get("nsym").u()));
        assert_eq!(cells_per_codeword(n, lumen::MODES[mode].bpc) as u64, e.get("cellsPerCodeword").u());
        assert_eq!(codewords_per_frame(&l, mode, n) as u64, e.get("codewordsPerFrame").u());
    }
}

#[test]
fn fountain_code() {
    let v = vectors();
    for e in v.get("fountainParams").arr() {
        let p = fountain::params(e.get("K").u() as u32);
        assert_eq!((p.s, p.p, p.p1, p.w, p.l) as (u32, u32, u32, u32, u32), (
            e.get("S").u() as u32,
            e.get("P").u() as u32,
            e.get("P1").u() as u32,
            e.get("W").u() as u32,
            e.get("L").u() as u32
        ));
    }
    for e in v.get("fountainSeed").arr() {
        assert_eq!(fountain_seed(e.get("tid").u() as u16, e.get("sbn").u() as u32) as u64, e.get("seed").u());
    }
    let tv = v.get("fountainTuples");
    let p = fountain::params(tv.get("K").u() as u32);
    for t in tv.get("tuples").arr() {
        let (lt, pi) = fountain::tuple(&p, tv.get("seed").u() as u32, t.get("esi").u() as u32);
        assert_eq!(lt.iter().map(|&x| x as u64).collect::<Vec<_>>(), t.get("lt").us());
        assert_eq!(pi.iter().map(|&x| x as u64).collect::<Vec<_>>(), t.get("pi").us());
    }
    let rv = v.get("fountainRows");
    let k = rv.get("K").u() as u32;
    let rows = fountain::ldpc_rows(k, fountain::params(k).s);
    assert_eq!(rows[0].iter().map(|&x| x as u64).collect::<Vec<_>>(), rv.get("ldpcRow0").us());
    let pi0 = fountain::pi_members(k, 0, rv.get("seed").u() as u32);
    assert_eq!(crc_of_u32s(&pi0) as u64, rv.get("piRow0Crc32").u());
    let sv = v.get("fountainSymbols");
    let t = sv.get("T").u() as usize;
    let enc = Encoder::new(&sv.get("payload").hex(), t, sv.get("seed").u() as u32);
    for s in sv.get("symbols").arr() {
        let mut out = vec![0u8; t];
        enc.encode(s.get("esi").u() as u32, &mut out);
        assert_eq!(out, s.get("data").hex(), "esi {}", s.get("esi").u());
    }
    for e in v.get("blockPlans").arr() {
        let p = block_plan(e.get("payloadLength").u(), e.get("T").u() as usize).unwrap();
        assert_eq!(p.kt as u64, e.get("Kt").u());
        assert_eq!(p.z as u64, e.get("Z").u());
        assert_eq!(p.sizes.iter().map(|&x| x as u64).collect::<Vec<_>>(), e.get("sizes").us());
        assert_eq!(p.starts.iter().map(|&x| x as u64).collect::<Vec<_>>(), e.get("starts").us());
    }
}

#[test]
fn packets_descriptor_frames() {
    let v = vectors();
    let pv = v.get("packet");
    let want = pv.get("bytes").hex();
    let mut msg = vec![0u8; want.len()];
    write_packet_header(&mut msg, pv.get("tid").u() as u16, pv.get("sbn").u() as u32, pv.get("esi").u() as u32, false);
    msg[PKT_HDR..want.len() - 2].copy_from_slice(&want[PKT_HDR..want.len() - 2]);
    seal_packet(&mut msg);
    assert_eq!(msg, want);

    let dv = v.get("descriptor");
    let m = dv.get("meta");
    let d = Descriptor {
        flags: m.get("flags").u() as u8,
        payload_length: m.get("payloadLength").u() as u32,
        file_size: m.get("fileSize").u() as u32,
        crc: m.get("crc").u() as u32,
        name: m.get("name").s().to_string(),
    };
    assert_eq!(encode_descriptor(&d, dv.get("T").u() as usize), dv.get("bytes").hex());

    let fv = v.get("frames");
    let file = fv.get("file").hex();
    let prepared = Prepared {
        descriptor: Descriptor { flags: 0, payload_length: file.len() as u32, file_size: file.len() as u32, crc: crc32(&file), name: fv.get("name").s().into() },
        payload: file,
    };
    let mut tx = Sender::new(
        &prepared,
        fv.get("mode").u() as usize,
        fv.get("eccLevel").u() as usize,
        fv.get("X").u() as usize,
        fv.get("Y").u() as usize,
        fv.get("tid").u() as u16,
        fv.get("frame0").get("seq").u() as u32,
    )
    .unwrap();
    assert_eq!((tx.n as u64, tx.nsym as u64), (fv.get("n").u(), fv.get("nsym").u()));
    let f0 = tx.next_frame();
    assert_eq!(f0.cells, fv.get("frame0").get("cells").hex());
    let f1 = tx.next_frame();
    assert_eq!(crc32(&f1.cells) as u64, fv.get("frame1").get("cellsCrc32").u());
}
