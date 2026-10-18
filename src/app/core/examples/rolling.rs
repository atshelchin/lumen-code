//! A rolling-shutter camera watching a sender: each camera row is exposed at its own moment
//! (`skew` ms from the first row to the last), so a capture can hold parts of two or three
//! sender frames. Prints what reaches the fountain decoder.
//!
//!     cargo run --release --example rolling -- <sender fps> <camera fps> <skew ms> [seconds]
//!
//! READOUT=<ms> tells the engine the skew, as an Android camera does.
use lumen_app::engine::bench::rolling_shutter;

fn main() {
    let a: Vec<f64> = std::env::args().skip(1).map(|s| s.parse().expect("a number")).collect();
    let (sfps, cfps, skew) = (a[0], a[1], a[2]);
    let readout = std::env::var("READOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let r = rolling_shutter(sfps, cfps, skew, a.get(3).copied().unwrap_or(6.0), readout);
    println!(
        "sender {sfps:>3} cam {cfps:>3} skew {skew:>4.1}ms | {:>6.1} KB/s = {:>4.1} frames/s of {sfps} | {} scans, {}% torn",
        r.bytes_per_sec / 1024.0,
        r.frames_per_sec,
        r.scans,
        r.torn_pct
    );
}
