//! cargo run --release --example bench -- [camera frames]       the self-test
//! cargo run --release --example bench -- profile [iterations]  tight scan loop, for a profiler
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("profile") {
        lumen_app::engine::bench::profile(args.get(2).and_then(|a| a.parse().ok()).unwrap_or(400));
        return;
    }
    let n = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(300);
    let t = std::time::Instant::now();
    let r = lumen_app::engine::bench::run(n);
    println!("{} ({:.1} s wall)", r.summary(), t.elapsed().as_secs_f64());
}
