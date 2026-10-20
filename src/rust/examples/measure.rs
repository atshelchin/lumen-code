//! Measurements for PROTOCOL Appendix C and the performance guide.
//!   cargo run --release --example measure -- fountain   decode failure vs. extra symbols
//!   cargo run --release --example measure -- residual   wrong packets: analysis and Monte Carlo
//!   cargo run --release --example measure -- bench      throughput of each stage
use lumen::fountain::{Decoder, Encoder};
use lumen::frame::encode_frame_cells;
use lumen::layout::{choose_codeword, codewords_per_frame, Layout};
use lumen::transfer::{fountain_seed, prepare, Receiver, Sender};
use lumen::util::Rng;
use std::time::Instant;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("fountain") => fountain(),
        Some("residual") => residual(),
        Some("bench") => bench(),
        Some("tail") => {
            let k: u32 = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(16384);
            let trials: u32 = std::env::args().nth(3).and_then(|v| v.parse().ok()).unwrap_or(1000);
            tail(k, trials)
        }
        Some("rank") => {
            let k: u32 = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(1000);
            let trials: u32 = std::env::args().nth(3).and_then(|v| v.parse().ok()).unwrap_or(100);
            rank_oracle(k, trials)
        }
        _ => eprintln!("usage: measure fountain | residual | bench | tail K trials | rank K trials"),
    }
}

/// For each K: add random distinct symbols one at a time and try to decode from K symbols on;
/// the number of extra symbols needed gives P(fail with K + e symbols) for e = 0, 1, 2, …
fn fountain() {
    println!("K       trials  P(fail) with K+0 .. K+8 symbols                               mean extra");
    for (k, trials) in [(1u32, 20000), (2, 20000), (5, 20000), (10, 20000), (30, 10000), (100, 10000), (300, 4000), (1000, 2000), (3000, 600), (10000, 200), (16384, 200)] {
        let mut need = vec![0u32; 64];
        let mut sum = 0u64;
        let mut r = Rng::new(k * 7919 + 1);
        for trial in 0..trials {
            let seed = fountain_seed((trial & 0xffff) as u16, 0);
            let mut dec = Decoder::new(k, 4, seed);
            let enc = Encoder::new(&vec![0u8; k as usize * 4], 4, seed);
            let mut extra = 0u32;
            loop {
                let mut s = [0u8; 4];
                let esi = r.next() & 0xffff;
                enc.encode(esi, &mut s);
                if !dec.add(esi, &s) {
                    continue; // repeated ESI
                }
                if dec.count() < k as usize {
                    continue;
                }
                if dec.decode().is_some() {
                    break;
                }
                extra += 1;
            }
            need[(extra as usize).min(63)] += 1;
            sum += extra as u64;
        }
        let mut row = String::new();
        let mut left = trials as f64;
        for e in 0..=8 {
            left -= need[e] as f64;
            row.push_str(&format!(" {:>7.4}", left / trials as f64));
        }
        println!("{k:<7} {trials:<7}{row}   {:.2}", sum as f64 / trials as f64);
    }
}

/// The full distribution of the extra symbols needed for one K (the tail of `fountain`).
fn tail(k: u32, trials: u32) {
    let mut need = vec![0u32; 1025];
    let mut r = Rng::new(k * 31 + 7);
    for trial in 0..trials {
        let seed = fountain_seed((trial & 0xffff) as u16, (trial >> 16) & 127);
        let mut dec = Decoder::new(k, 4, seed);
        let enc = Encoder::new(&vec![0u8; k as usize * 4], 4, seed);
        let mut extra = 0usize;
        loop {
            let mut s = [0u8; 4];
            let esi = r.next() & 0xffff;
            enc.encode(esi, &mut s);
            if !dec.add(esi, &s) || dec.count() < k as usize {
                continue;
            }
            if dec.decode().is_some() || extra == 1024 {
                break;
            }
            extra += 1;
        }
        need[extra] += 1;
    }
    let mut left = trials;
    let mut out = String::new();
    for (e, &c) in need.iter().enumerate() {
        left -= c;
        if [0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 48, 64].contains(&e) {
            out.push_str(&format!(" +{e}:{:.4}", left as f64 / trials as f64));
        }
    }
    let max = need.iter().rposition(|&c| c > 0).unwrap_or(0);
    let mean = need.iter().enumerate().map(|(e, &c)| e as f64 * c as f64).sum::<f64>() / trials as f64;
    println!("K={k} trials={trials} mean extra {mean:.2} max {max}; P(fail) after{out}");
}

/// Is the system of §9.7 (LDPC rows, PI rows, one row per received symbol) of full column rank?
/// Plain dense Gaussian elimination over GF(2): an oracle independent of the reference decoder.
fn full_rank(k: u32, seed: u32, esis: &[u32]) -> bool {
    let p = lumen::fountain::params(k);
    let (w, pp, l) = (p.w as usize, p.p as usize, p.l as usize);
    let words = l.div_ceil(64);
    let mut rows: Vec<Vec<u64>> = Vec::new();
    let mut row = |cols: &mut dyn Iterator<Item = usize>| {
        let mut r = vec![0u64; words];
        for c in cols {
            r[c / 64] ^= 1 << (c % 64);
        }
        rows.push(r);
    };
    for (s, members) in lumen::fountain::ldpc_rows(k, p.s).iter().enumerate() {
        let mut it = members.iter().map(|&i| i as usize).chain([k as usize + s, w + s % pp, w + (s + 1) % pp]);
        row(&mut it);
    }
    for h in 0..pp {
        let mut it = lumen::fountain::pi_members(k, h as u32, seed).into_iter().map(|i| i as usize).chain([w + h]);
        row(&mut it);
    }
    for &e in esis {
        let (lt, pi) = lumen::fountain::tuple(&p, seed, e);
        let mut it = lt.into_iter().map(|c| c as usize).chain(pi.into_iter().map(|c| w + c as usize));
        row(&mut it);
    }
    let mut rank = 0;
    for c in 0..l {
        let (wd, bit) = (c / 64, 1u64 << (c % 64));
        let Some(piv) = (rank..rows.len()).find(|&i| rows[i][wd] & bit != 0) else { return false };
        rows.swap(rank, piv);
        let pr = rows[rank].clone();
        for r in rows.iter_mut().skip(rank + 1) {
            if r[wd] & bit != 0 {
                for x in wd..words {
                    r[x] ^= pr[x];
                }
            }
        }
        rank += 1;
    }
    true
}

/// The reference decoder against the rank oracle: at the first symbol count where it decodes the
/// system must have full rank, and one symbol earlier it must not.
fn rank_oracle(k: u32, trials: u32) {
    let mut r = Rng::new(k * 13 + 5);
    let (mut agree, mut tails) = (0, 0);
    for trial in 0..trials {
        let seed = fountain_seed((trial & 0xffff) as u16, 3);
        let enc = Encoder::new(&vec![0u8; k as usize * 4], 4, seed);
        let mut dec = Decoder::new(k, 4, seed);
        let mut esis = Vec::new();
        loop {
            let mut s = [0u8; 4];
            let esi = r.next() & 0xffff;
            enc.encode(esi, &mut s);
            if !dec.add(esi, &s) {
                continue;
            }
            esis.push(esi);
            if esis.len() >= k as usize && dec.decode().is_some() {
                break;
            }
        }
        let ok_at = full_rank(k, seed, &esis);
        let before = esis.len() > k as usize && full_rank(k, seed, &esis[..esis.len() - 1]);
        assert!(ok_at, "trial {trial}: the decoder decoded a system without full rank");
        assert!(!before, "trial {trial}: the system had full rank one symbol before the decoder succeeded");
        agree += 1;
        if esis.len() - k as usize >= 8 {
            tails += 1;
        }
    }
    println!("K={k}: the decoder agrees with the rank oracle in {agree} of {trials} trials ({tails} needed 8 or more extra symbols)");
}

/// log10 of the number of words within Hamming distance t of a word of length n over GF(256).
fn log10_ball(n: usize, t: usize) -> f64 {
    let mut terms = Vec::new();
    for i in 0..=t.min(n) {
        let mut l = 0.0;
        for j in 0..i {
            l += ((n - j) as f64).log10() - ((j + 1) as f64).log10();
        }
        terms.push(l + i as f64 * 255f64.log10());
    }
    let m = terms.iter().cloned().fold(f64::MIN, f64::max);
    m + terms.iter().map(|x| 10f64.powf(x - m)).sum::<f64>().log10()
}

/// The reference codeword decoder (frame::decode_codewords): plain RS decoding, then two
/// retries with the nsym/4 and nsym/2 least reliable bytes as erasures.
fn reference_accepts(cw: &[u8], nsym: usize, order: &[usize]) -> bool {
    let mut w = cw.to_vec();
    if lumen::rs::decode(&mut w, nsym, &[]).is_some() {
        return true;
    }
    for f in [nsym / 4, nsym / 2] {
        if f == 0 {
            continue;
        }
        let mut w = cw.to_vec();
        if lumen::rs::decode(&mut w, nsym, &order[..f]).is_some() {
            return true;
        }
    }
    false
}

/// Probability that a codeword read from garbage (a misread frame) gives a packet that passes
/// all checks: the decoder accepts it (bounded-distance analysis, union over the three tries),
/// and then its CRC-16 is correct (2^-16).
fn residual() {
    println!("Analysis: P(accept) of random input for the reference decoder, and with the packet CRC-16");
    println!("n    nsym  plain        f=nsym/4     f=nsym/2     total        x 2^-16");
    let rows = [(255, 26), (255, 40), (255, 56), (255, 76), (127, 20), (63, 8), (40, 4)];
    for (n, nsym) in rows {
        let q = 256f64.log10();
        let plain = log10_ball(n, nsym / 2) - nsym as f64 * q;
        let f1 = nsym / 4;
        let quarter = if f1 > 0 { log10_ball(n - f1, (nsym - f1) / 2) - (nsym - f1) as f64 * q } else { f64::NEG_INFINITY };
        let f2 = nsym / 2;
        let half = log10_ball(n - f2, (nsym - f2) / 2) - (nsym - f2) as f64 * q;
        let total = (10f64.powf(plain) + 10f64.powf(quarter) + 10f64.powf(half)).log10();
        let fmt = |l: f64| if l == f64::NEG_INFINITY { "0".to_string() } else { format!("{:.1e}", 10f64.powf(l)) };
        println!("{n:<4} {nsym:<5} {:<12} {:<12} {:<12} {:<12} {}", fmt(plain), fmt(quarter), fmt(half), fmt(total), fmt(total - 16.0 * 2f64.log10()));
    }
    println!();
    println!("Monte Carlo of the same decoder on random input (rates large enough to measure):");
    let mut r = Rng::new(42);
    for (n, nsym, trials) in [(40usize, 4usize, 400_000u32), (63, 8, 400_000), (100, 10, 400_000)] {
        let mut accepted = 0u32;
        let mut order: Vec<usize> = (0..n).collect();
        for _ in 0..trials {
            let cw: Vec<u8> = (0..n).map(|_| r.next() as u8).collect();
            for j in (1..n).rev() {
                order.swap(j, r.next() as usize % (j + 1));
            }
            if reference_accepts(&cw, nsym, &order) {
                accepted += 1;
            }
        }
        let q = 256f64.log10();
        let f1 = nsym / 4;
        let f2 = nsym / 2;
        let mut pred = 10f64.powf(log10_ball(n, nsym / 2) - nsym as f64 * q) + 10f64.powf(log10_ball(n - f2, (nsym - f2) / 2) - (nsym - f2) as f64 * q);
        if f1 > 0 {
            pred += 10f64.powf(log10_ball(n - f1, (nsym - f1) / 2) - (nsym - f1) as f64 * q);
        }
        println!("n={n:<4} nsym={nsym:<3} measured {:.2e} ({accepted} of {trials}), analysis (union bound) {:.2e}", accepted as f64 / trials as f64, pred);
    }
}

fn time<F: FnMut()>(reps: u32, mut f: F) -> f64 {
    f();
    let t0 = Instant::now();
    for _ in 0..reps {
        f();
    }
    t0.elapsed().as_secs_f64() / reps as f64
}

fn bench() {
    let mut r = Rng::new(9);
    // Reed–Solomon, n = 255, nsym = 40
    let msg: Vec<u8> = (0..215).map(|_| r.next() as u8).collect();
    let cw = lumen::rs::encode(&msg, 40);
    let enc = time(20000, || {
        std::hint::black_box(lumen::rs::encode(&msg, 40));
    });
    let mut bad = cw.clone();
    for i in 0..20 {
        bad[i * 12] ^= 0x5a;
    }
    let dec = time(5000, || {
        let mut w = bad.clone();
        std::hint::black_box(lumen::rs::decode(&mut w, 40, &[]));
    });
    let clean = time(20000, || {
        let mut w = cw.clone();
        std::hint::black_box(lumen::rs::decode(&mut w, 40, &[]));
    });
    println!("RS(255,215) encode {:.2} us, decode clean {:.2} us, decode 20 errors {:.1} us", enc * 1e6, clean * 1e6, dec * 1e6);

    // fountain code, T = 200
    for k in [100u32, 1000, 16384] {
        let t = 200usize;
        let data: Vec<u8> = (0..k as usize * t).map(|_| r.next() as u8).collect();
        let seed = fountain_seed(1, 0);
        let mut e = None;
        let pre = time(3, || e = Some(Encoder::new(&data, t, seed)));
        let e = e.unwrap();
        let mut out = vec![0u8; t];
        let sym = time(20000, || e.encode(r.next() & 0xffff, &mut out));
        let mut dec = Decoder::new(k, t, seed);
        let mut esi = 0;
        while dec.count() < k as usize + 10 {
            e.encode(esi, &mut out);
            dec.add(esi, &out);
            esi += 1;
        }
        let d = time(3, || {
            let mut d2 = Decoder::new(k, t, seed);
            for x in 0..esi {
                e.encode(x, &mut out);
                d2.add(x, &out);
            }
            std::hint::black_box(d2.decode());
        });
        println!(
            "fountain K={k:<6} T={t}: precode {:.1} ms, one symbol {:.2} us, decode of K+10 symbols {:.1} ms ({:.0} MB/s)",
            pre * 1e3,
            sym * 1e6,
            d * 1e3,
            (k as f64 * t as f64) / d / 1e6
        );
    }

    // frame encoding: a 13" laptop grid (C4) and a phone grid
    for (x, y) in [(150, 240), (90, 160)] {
        let layout = Layout::new(x, y).unwrap();
        let (n, nsym) = choose_codeword(&layout, 1, 1);
        let m = codewords_per_frame(&layout, 1, n);
        let packets: Vec<Vec<u8>> = (0..m).map(|_| (0..n - nsym).map(|_| r.next() as u8).collect()).collect();
        let f = time(500, || {
            std::hint::black_box(encode_frame_cells(&layout, 1, n, nsym, 7, &packets));
        });
        println!("frame {x}x{y} C4: {m} codewords of n={n}, cells {:.2} ms", f * 1e3);
    }

    // the whole sender and receiver without the camera: 2 MB of random data
    let file: Vec<u8> = (0..2_000_000).map(|_| r.next() as u8).collect();
    let prepared = prepare(&file, "bench.bin", true);
    let t0 = Instant::now();
    let mut tx = Sender::new(&prepared, 1, 1, 150, 240, 0x4242, 0).unwrap();
    let setup = t0.elapsed().as_secs_f64();
    let mut rx = Receiver::new();
    let t0 = Instant::now();
    let mut frames = 0;
    let mut packets = 0;
    let mut sending = 0.0;
    loop {
        let t1 = Instant::now();
        let ps = tx.next_packets();
        sending += t1.elapsed().as_secs_f64();
        frames += 1;
        for p in &ps {
            rx.push(p);
            packets += 1;
        }
        if frames % 8 == 0 {
            if let Some(lumen::transfer::Finish::File { bytes, .. }) = rx.try_finish() {
                assert_eq!(bytes, file);
                break;
            }
        }
    }
    let total = t0.elapsed().as_secs_f64();
    println!(
        "2 MB transfer, packets only: sender setup {:.0} ms, {frames} frames / {packets} packets, sender {:.0} ms, receiver {:.0} ms ({:.0} MB/s)",
        setup * 1e3,
        sending * 1e3,
        (total - sending) * 1e3,
        2.0 / (total - sending)
    );
}
