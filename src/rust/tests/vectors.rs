// Conformance: the Rust implementation against test/vectors/test-vectors.json (PROTOCOL §15).
// The JS implementation runs the same checks in test/test-vectors.js.
mod json;
use json::{parse, J};
use lumen::fountain::{self, Decoder, Encoder};
use lumen::frame::{encode_descriptor, encode_frame_cells, parse_descriptor, read_packet, seal_packet, write_packet_header, Descriptor, PKT_HDR};
use lumen::layout::{cells_per_codeword, choose_codeword, codewords_per_frame, data_cell_count, decode_header, pack_header, Header, Layout};
use lumen::transfer::{block_plan, fountain_seed, Action, Finish, Limits, Prepared, Receiver, Sender, State};
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

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn usizes(v: &[usize]) -> Vec<u64> {
    v.iter().map(|&x| x as u64).collect()
}

fn header_of(f: &J) -> Header {
    Header {
        mode: f.get("mode").u() as u32,
        x: f.get("X").u() as u32,
        y: f.get("Y").u() as u32,
        n: f.get("n").u() as u32,
        nsym: f.get("nsym").u() as u32,
        seq: f.get("seq").u() as u32,
    }
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
    let rs = v.get("reedSolomon");
    for e in rs.get("encode").arr() {
        let cw = lumen::rs::encode(&e.get("message").hex(), e.get("nsym").u() as usize);
        assert_eq!(cw, e.get("codeword").hex());
    }
    for e in rs.get("decode").arr() {
        let mut cw = e.get("received").hex();
        let erasures: Vec<usize> = e.get("erasures").us().iter().map(|&x| x as usize).collect();
        let res = lumen::rs::decode(&mut cw, e.get("nsym").u() as usize, &erasures);
        assert!(res.is_some(), "rs decode nsym={} errors={} erasures={}", e.get("nsym").u(), e.get("errors").u(), erasures.len());
        assert_eq!(cw, e.get("codeword").hex());
    }
}

#[test]
fn layouts() {
    let v = vectors();
    for e in v.get("layouts").arr() {
        let (x, y) = (e.get("X").u() as usize, e.get("Y").u() as usize);
        let l = Layout::new(x, y).unwrap();
        assert_eq!(usizes(&l.lx), e.get("lx").us());
        assert_eq!(usizes(&l.ly), e.get("ly").us());
        assert_eq!(l.data_cells() as u64, e.get("dataCells").u());
        assert_eq!(data_cell_count(x, y) as u64, e.get("dataCells").u());
        assert_eq!(crc32(&l.kind) as u64, e.get("kindCrc32").u(), "kind {x}x{y}");
        assert_eq!(crc32(&l.fixed) as u64, e.get("fixedCrc32").u());
        assert_eq!(l.data_order[..32].iter().map(|&a| a as u64).collect::<Vec<_>>(), e.get("dataOrderFirst32").us());
        assert_eq!(crc_of_u32s(&l.data_order) as u64, e.get("dataOrderCrc32").u());
        for c in 0..4 {
            assert_eq!(crc_of_u32s(&l.headers[c]) as u64, e.get("headerCellsCrc32").at(c).u());
            let h = &l.headers[c];
            assert_eq!(vec![h[0] as u64, h[1] as u64, h[69] as u64], e.get("headerCells").at(c).us());
        }
        assert_eq!(l.whiten[..32].iter().map(|&a| a as u64).collect::<Vec<_>>(), e.get("whitenFirst32").us());
        assert_eq!(crc32(&l.whiten) as u64, e.get("whitenCrc32").u());
    }
}

#[test]
fn headers() {
    let v = vectors();
    let hv = v.get("headers");
    for e in hv.get("encode").arr() {
        let h = header_of(e.get("fields"));
        for c in 0..4 {
            let bits: String = pack_header(&h, c).iter().map(|b| if *b == 1 { '1' } else { '0' }).collect();
            assert_eq!(bits, e.get("copies").at(c).s());
        }
    }
    for e in hv.get("decode").arr() {
        let bits: Vec<u8> = e.get("bits").s().bytes().map(|c| (c == b'1') as u8).collect();
        let r = decode_header(&bits, e.get("copy").u() as usize);
        match e.opt("error") {
            Some(_) => assert!(r.is_err(), "header {} must be discarded", e.get("name").s()),
            None => assert_eq!(r, Ok(header_of(e.get("header"))), "header {}", e.get("name").s()),
        }
    }
}

#[test]
fn frames_from_packets() {
    let v = vectors();
    for e in v.get("frames").arr() {
        let l = Layout::new(e.get("X").u() as usize, e.get("Y").u() as usize).unwrap();
        let packets: Vec<Vec<u8>> = e.get("packets").arr().iter().map(|p| p.hex()).collect();
        let cells = encode_frame_cells(&l, e.get("mode").u() as usize, e.get("n").u() as usize, e.get("nsym").u() as usize, e.get("seq").u() as u32, &packets);
        assert_eq!(crc32(&cells) as u64, e.get("cellsCrc32").u(), "frame {}x{}", l.x, l.y);
        if let Some(c) = e.opt("cells") {
            assert_eq!(cells, c.hex());
        }
    }
}

#[test]
fn packets() {
    let v = vectors();
    let pv = v.get("packets");
    for e in pv.get("encode").arr() {
        let payload = e.get("payload").hex();
        let mut msg = vec![0u8; payload.len() + 7];
        write_packet_header(&mut msg, e.get("tid").u() as u16, e.get("sbn").u() as u32, e.get("esi").u() as u32, e.get("meta").b());
        msg[PKT_HDR..PKT_HDR + payload.len()].copy_from_slice(&payload);
        seal_packet(&mut msg);
        assert_eq!(msg, e.get("bytes").hex());
    }
    for e in pv.get("decode").arr() {
        let bytes = e.get("bytes").hex();
        let action = Receiver::new().push(&bytes);
        let name = e.get("name").s();
        match e.opt("error") {
            Some(_) => assert_eq!(action, Action::Discard, "packet {name} must be discarded"),
            None => {
                let (id, data) = read_packet(&bytes);
                let p = e.get("packet");
                assert_eq!(
                    (id.tid as u64, id.meta, id.sbn as u64, id.esi as u64, data.len() as u64),
                    (p.get("tid").u(), p.get("meta").b(), p.get("sbn").u(), p.get("esi").u(), p.get("T").u()),
                    "packet {name}"
                );
                if !id.meta {
                    assert_eq!(action, Action::Keep, "packet {name}");
                }
            }
        }
    }
}

#[test]
fn descriptors() {
    let v = vectors();
    let dv = v.get("descriptors");
    for e in dv.get("encode").arr() {
        let d = Descriptor {
            flags: e.get("flags").u() as u8,
            payload_length: e.get("payloadLength").u() as u32,
            file_size: e.get("fileSize").u() as u32,
            crc: e.get("crc").u() as u32,
            name: e.get("name").s().to_string(),
            records: Vec::new(),
        };
        let bytes = encode_descriptor(&d, e.get("T").u() as usize);
        assert_eq!(bytes, e.get("bytes").hex());
        if let Some(n) = e.opt("decodedName") {
            assert_eq!(parse_descriptor(&bytes).unwrap().name, n.s());
        }
    }
    for e in dv.get("decode").arr() {
        let r = parse_descriptor(&e.get("bytes").hex());
        let name = e.get("name").s();
        match e.opt("error") {
            Some(_) => assert!(r.is_err(), "descriptor {name} must be discarded"),
            None => {
                let d = r.unwrap_or_else(|err| panic!("descriptor {name}: {err:?}"));
                let w = e.get("descriptor");
                assert_eq!(
                    (d.flags as u64, d.payload_length as u64, d.file_size as u64, d.crc as u64, d.name.as_str()),
                    (w.get("flags").u(), w.get("payloadLength").u(), w.get("fileSize").u(), w.get("crc").u(), w.get("name").s()),
                    "descriptor {name}"
                );
                let want: Vec<(u8, Vec<u8>)> = w.get("records").arr().iter().map(|r| (r.get("type").u() as u8, r.get("value").hex())).collect();
                assert_eq!(d.records, want, "descriptor {name} records");
            }
        }
    }
}

#[test]
fn fountain_code() {
    let v = vectors();
    for e in v.get("fountainParams").arr() {
        let p = fountain::params(e.get("K").u() as u32);
        assert_eq!(
            (p.s, p.p, p.p1, p.w, p.l),
            (e.get("S").u() as u32, e.get("P").u() as u32, e.get("P1").u() as u32, e.get("W").u() as u32, e.get("L").u() as u32)
        );
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
    assert_eq!(pi0[..16].iter().map(|&x| x as u64).collect::<Vec<_>>(), rv.get("piRow0First16").us());
    let sv = v.get("fountainSymbols");
    let t = sv.get("T").u() as usize;
    let enc = Encoder::new(&sv.get("payload").hex(), t, sv.get("seed").u() as u32);
    for s in sv.get("symbols").arr() {
        let mut out = vec![0u8; t];
        enc.encode(s.get("esi").u() as u32, &mut out);
        assert_eq!(out, s.get("data").hex(), "esi {}", s.get("esi").u());
    }
    for e in v.get("fountainDecode").arr() {
        let mut dec = Decoder::new(e.get("K").u() as u32, e.get("T").u() as usize, e.get("seed").u() as u32);
        let syms = e.get("symbols").arr();
        for s in syms {
            dec.add(s.get("esi").u() as u32, &s.get("data").hex());
        }
        let out = dec.decode();
        let what = format!("fountain decode K={} with {} symbols", e.get("K").u(), syms.len());
        if e.get("decodable").b() {
            assert_eq!(out, Some(e.get("block").hex()), "{what}");
        } else {
            assert!(out.is_none(), "{what} must not decode");
        }
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
fn streams() {
    let v = vectors();
    for st in v.get("streams").arr() {
        let name = st.get("name").s();
        let mut limits = Limits::default();
        if let Some(l) = st.opt("limits") {
            if let Some(x) = l.opt("maxSize") {
                limits.max_size = x.u();
            }
            if let Some(x) = l.opt("maxKept") {
                limits.max_kept = x.u() as usize;
            }
            if let Some(x) = l.opt("maxTransfers") {
                limits.max_transfers = x.u() as usize;
            }
        }
        let mut rx = Receiver::with_limits(limits);
        for (i, s) in st.get("steps").arr().iter().enumerate() {
            let at = format!("stream \"{name}\" step {i}");
            if let Some(p) = s.opt("push") {
                let want = match s.get("result").s() {
                    "discard" => Action::Discard,
                    "keep" => Action::Keep,
                    "add" => Action::Add,
                    "accept" => Action::Accept,
                    "refuse" => Action::Refuse,
                    other => panic!("{at}: unknown action {other}"),
                };
                assert_eq!(rx.push(&p.hex()), want, "{at}");
            } else if let Some(f) = s.opt("finish") {
                let got = rx.try_finish();
                if f.is_null() {
                    assert!(got.is_none(), "{at}: nothing expected");
                } else if let Some(file) = f.opt("file") {
                    match got {
                        Some(Finish::File { name: n, bytes }) => {
                            assert_eq!(hex(&bytes), file.s(), "{at}");
                            assert_eq!(n, f.get("name").s(), "{at}");
                        }
                        _ => panic!("{at}: a file expected"),
                    }
                } else {
                    assert!(matches!(got, Some(Finish::ChecksumError)), "{at}: a failed check expected");
                }
            } else if let Some(want) = s.opt("state") {
                let got = match rx.progress().map(|p| p.state) {
                    None => "new",
                    Some(State::Collecting) => "collecting",
                    Some(State::Receiving) => "receiving",
                    Some(State::Refused) => "refused",
                    Some(State::Complete) => "complete",
                };
                assert_eq!(got, want.s(), "{at}");
            }
        }
    }
}

#[test]
fn reference_sender() {
    let v = vectors();
    let rs = v.get("referenceSender");
    for e in rs.get("codewordChoice").arr() {
        let l = Layout::new(e.get("X").u() as usize, e.get("Y").u() as usize).unwrap();
        let mode = e.get("mode").u() as usize;
        let (n, nsym) = choose_codeword(&l, mode, e.get("eccLevel").u() as usize);
        assert_eq!((n as u64, nsym as u64), (e.get("n").u(), e.get("nsym").u()));
        assert_eq!(cells_per_codeword(n, lumen::MODES[mode].bpc) as u64, e.get("cellsPerCodeword").u());
        assert_eq!(codewords_per_frame(&l, mode, n) as u64, e.get("codewordsPerFrame").u());
    }
    let fv = rs.get("frames");
    let file = fv.get("file").hex();
    let prepared = Prepared {
        descriptor: Descriptor { flags: 0, payload_length: file.len() as u32, file_size: file.len() as u32, crc: crc32(&file), name: fv.get("name").s().into(), records: Vec::new() },
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
    assert_eq!(crc32(&tx.next_frame().cells) as u64, fv.get("frame0").get("cellsCrc32").u());
    assert_eq!(crc32(&tx.next_frame().cells) as u64, fv.get("frame1").get("cellsCrc32").u());

    // the descriptor schedule of a larger transfer
    let sv = rs.get("schedule");
    let mut r = Rng::new(11);
    let big: Vec<u8> = (0..40000).map(|_| r.next() as u8).collect();
    let prepared = Prepared {
        descriptor: Descriptor { flags: 0, payload_length: big.len() as u32, file_size: big.len() as u32, crc: crc32(&big), name: "big.bin".into(), records: Vec::new() },
        payload: big,
    };
    let mut tx = Sender::new(&prepared, sv.get("mode").u() as usize, sv.get("eccLevel").u() as usize, sv.get("X").u() as usize, sv.get("Y").u() as usize, 0x0303, 0).unwrap();
    for (f, want) in sv.get("descriptorCodeword").arr().iter().enumerate() {
        let at = tx.next_packets().iter().position(|p| p[2] & 0x80 != 0).map_or(-1, |i| i as i64);
        assert_eq!(at, want.num() as i64, "descriptor position in frame {f}");
    }
}
