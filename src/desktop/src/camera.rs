//! The webcam (macOS: AVFoundation through nokhwa), feeding the core's scan pipeline.
//!
//! Capture runs on its own thread at the camera's fastest mode (a desktop has no heat budget to
//! protect; the runtime decides itself which frames to scan). Frames go to the runtime in the
//! camera's own Y'CbCr layout (YUYV or NV12) — no conversion; only every second frame is
//! converted, at half size, for the preview.

use futures::channel::oneshot;
use gpui::RenderImage;
use lumen_app::app::CameraError;
use lumen_app::runtime::Runtime;
use lumen_app::{FrameRef, PixelFormat, Plane};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Devices and the capture rate in force, or why the camera could not open.
type Ready = Result<(u32, u32), (CameraError, String)>;

pub struct Camera {
    stop: Arc<AtomicBool>,
    preview: Arc<Mutex<Option<image::RgbaImage>>>,
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Camera {
    pub fn take_preview(&self) -> Option<RenderImage> {
        let img = self.preview.lock().ok()?.take()?;
        Some(crate::platform::render_image(img))
    }

    #[cfg(target_os = "macos")]
    pub fn start(index: usize, rt: Runtime) -> (Camera, oneshot::Receiver<Ready>) {
        let (tx, rx) = oneshot::channel::<Ready>();
        let stop = Arc::new(AtomicBool::new(false));
        let preview = Arc::new(Mutex::new(None));
        let (stop2, preview2) = (stop.clone(), preview.clone());
        std::thread::Builder::new()
            .name("lumen-webcam".into())
            .spawn(move || capture(index, rt, stop2, preview2, tx))
            .expect("webcam thread");
        (Camera { stop, preview }, rx)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn start(_index: usize, _rt: Runtime) -> (Camera, oneshot::Receiver<Ready>) {
        let (tx, rx) = oneshot::channel::<Ready>();
        let _ = tx.send(Err((CameraError::Unavailable, String::new())));
        (Camera { stop: Arc::new(AtomicBool::new(true)), preview: Arc::new(Mutex::new(None)) }, rx)
    }
}

#[cfg(target_os = "macos")]
fn capture(index: usize, rt: Runtime, stop: Arc<AtomicBool>, preview: Arc<Mutex<Option<image::RgbaImage>>>, ready: oneshot::Sender<Ready>) {
    use nokhwa::pixel_format::RgbAFormat;
    use nokhwa::utils::{ApiBackend, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType};

    // The permission prompt must be answered before a device is opened.
    let granted = Arc::new(AtomicBool::new(false));
    let answered = Arc::new(AtomicBool::new(false));
    {
        let (g, a) = (granted.clone(), answered.clone());
        nokhwa::nokhwa_initialize(move |ok| {
            g.store(ok, Ordering::SeqCst);
            a.store(true, Ordering::SeqCst);
        });
    }
    for _ in 0..600 {
        if answered.load(Ordering::SeqCst) || stop.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if !granted.load(Ordering::SeqCst) {
        let _ = ready.send(Err((CameraError::Denied, String::new())));
        return;
    }
    let devices = nokhwa::query(ApiBackend::Auto).map(|d| d.len()).unwrap_or(0);
    if devices == 0 {
        let _ = ready.send(Err((CameraError::Unavailable, String::new())));
        return;
    }
    let format = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = match nokhwa::Camera::new(CameraIndex::Index((index % devices) as u32), format) {
        Ok(c) => c,
        Err(e) => {
            let _ = ready.send(Err((CameraError::Failed, e.to_string())));
            return;
        }
    };
    if let Err(e) = camera.open_stream() {
        let _ = ready.send(Err((CameraError::Failed, e.to_string())));
        return;
    }
    let _ = ready.send(Ok((devices as u32, camera.camera_format().frame_rate())));
    let mut n = 0u64;
    while !stop.load(Ordering::SeqCst) {
        let Ok(buffer) = camera.frame() else {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        };
        let (w, h) = (buffer.resolution().width() as usize, buffer.resolution().height() as usize);
        let raw = buffer.buffer();
        n += 1;
        let want_preview = n % 2 == 0;
        match buffer.source_frame_format() {
            FrameFormat::YUYV if raw.len() >= w * h * 2 => {
                let f = FrameRef {
                    width: w,
                    height: h,
                    format: PixelFormat::Yuyv,
                    planes: [Plane { data: raw, row_stride: w * 2, pixel_stride: 2 }, Plane::EMPTY, Plane::EMPTY],
                    rotation: 0,
                };
                rt.camera_frame(&f);
                if want_preview {
                    set_preview(&preview, preview_yuyv(raw, w, h));
                }
            }
            FrameFormat::NV12 if raw.len() >= w * h * 3 / 2 => {
                let (y, uv) = raw.split_at(w * h);
                let f = FrameRef {
                    width: w,
                    height: h,
                    format: PixelFormat::Yuv420,
                    planes: [
                        Plane { data: y, row_stride: w, pixel_stride: 1 },
                        Plane { data: uv, row_stride: w, pixel_stride: 2 },
                        Plane { data: &uv[1..], row_stride: w, pixel_stride: 2 },
                    ],
                    rotation: 0,
                };
                rt.camera_frame(&f);
                if want_preview {
                    set_preview(&preview, preview_nv12(y, uv, w, h));
                }
            }
            _ => {
                let Ok(rgba) = buffer.decode_image::<RgbAFormat>() else { continue };
                rt.camera_frame(&FrameRef::rgba(w, h, rgba.as_raw()));
                if want_preview {
                    set_preview(&preview, image::imageops::resize(&rgba, (w / 2) as u32, (h / 2) as u32, image::imageops::FilterType::Nearest));
                }
            }
        }
    }
    let _ = camera.stop_stream();
}

#[cfg(target_os = "macos")]
fn set_preview(slot: &Arc<Mutex<Option<image::RgbaImage>>>, img: image::RgbaImage) {
    if let Ok(mut p) = slot.lock() {
        *p = Some(img);
    }
}

#[inline]
fn rgb(y: i32, u: i32, v: i32) -> [u8; 4] {
    let (u, v) = (u - 128, v - 128);
    [
        (y + ((359 * v) >> 8)).clamp(0, 255) as u8,
        (y - ((88 * u + 183 * v) >> 8)).clamp(0, 255) as u8,
        (y + ((454 * u) >> 8)).clamp(0, 255) as u8,
        255,
    ]
}

/// Half-size RGBA preview of a YUYV frame (one pixel per 2×2 block).
#[cfg(target_os = "macos")]
fn preview_yuyv(raw: &[u8], w: usize, h: usize) -> image::RgbaImage {
    let (pw, ph) = (w / 2, h / 2);
    let mut out = vec![0u8; pw * ph * 4];
    for py in 0..ph {
        let row = &raw[py * 2 * w * 2..];
        for px in 0..pw {
            let q = &row[px * 4..px * 4 + 4];
            out[(py * pw + px) * 4..(py * pw + px) * 4 + 4].copy_from_slice(&rgb(q[0] as i32, q[1] as i32, q[3] as i32));
        }
    }
    image::RgbaImage::from_raw(pw as u32, ph as u32, out).unwrap_or_default()
}

/// Half-size RGBA preview of an NV12 frame.
#[cfg(target_os = "macos")]
fn preview_nv12(y: &[u8], uv: &[u8], w: usize, h: usize) -> image::RgbaImage {
    let (pw, ph) = (w / 2, h / 2);
    let mut out = vec![0u8; pw * ph * 4];
    for py in 0..ph {
        for px in 0..pw {
            let l = y[py * 2 * w + px * 2] as i32;
            let k = py * w + px * 2;
            out[(py * pw + px) * 4..(py * pw + px) * 4 + 4].copy_from_slice(&rgb(l, uv[k] as i32, uv[k + 1] as i32));
        }
    }
    image::RgbaImage::from_raw(pw as u32, ph as u32, out).unwrap_or_default()
}
