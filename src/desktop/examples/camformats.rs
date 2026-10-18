//! Lists the webcam's formats: cargo run --release --example camformats
#[cfg(target_os = "macos")]
fn main() {
    use nokhwa::pixel_format::RgbAFormat;
    use nokhwa::utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let done = Arc::new(AtomicBool::new(false));
    let ok = Arc::new(AtomicBool::new(false));
    {
        let (d, o) = (done.clone(), ok.clone());
        nokhwa::nokhwa_initialize(move |g| {
            o.store(g, Ordering::SeqCst);
            d.store(true, Ordering::SeqCst);
        });
    }
    for _ in 0..300 {
        if done.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    println!("permission: {}", ok.load(Ordering::SeqCst));
    for d in nokhwa::query(ApiBackend::Auto).unwrap_or_default() {
        println!("device {}: {}", d.index(), d.human_name());
    }
    for i in 0..3u32 {
        let Ok(mut cam) = nokhwa::Camera::new(CameraIndex::Index(i), RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate)) else { continue };
        println!("[{i}] highest fps: {:?}", cam.camera_format());
        let r = cam.set_frame_rate(60);
        println!("[{i}] set 60 -> {:?} now {:?}", r.err().map(|e| e.to_string()), cam.camera_format());
        if cam.open_stream().is_ok() {
            let t = std::time::Instant::now();
            let mut n = 0;
            while t.elapsed().as_secs_f64() < 2.0 {
                if let Ok(b) = cam.frame() {
                    n += 1;
                    if n == 1 {
                        println!("[{i}] frame {:?} {:?} {} bytes", b.resolution(), b.source_frame_format(), b.buffer().len());
                    }
                }
            }
            println!("[{i}] measured {:.1} fps", n as f64 / 2.0);
            let _ = cam.stop_stream();
        }
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {}
