//! uniffi bindings of the Lumen app core for the Swift (iOS) and Kotlin (Android) shells.
//!
//! Two kinds of surface:
//!
//! - **The Crux core, as JSON.** `dispatch(event)` and `resolve(id, output)` go in; every
//!   change comes back through [`ShellListener::on_update`] as
//!   `{ "view": {…}, "requests": [{ "id": 3, "op": { "type": "camera_start", … } }] }`,
//!   on whichever thread caused it. A request with `id == 0` is a notification.
//! - **The real-time paths, typed.** Frames never become JSON: [`LumenCore::send_frame`]
//!   returns the next one-pixel-per-cell image, and [`LumenCore::camera_frame`] reads the
//!   camera's own buffer in place (addresses, not copies).

use lumen_app::app::{Event, ShellOutput};
use lumen_app::content::SpanKind;
use lumen_app::i18n::{labels, Lang};
use lumen_app::runtime::{Runtime, Update};
use lumen_app::{FrameRef, PixelFormat, Plane};
use std::sync::Arc;

uniffi::setup_scaffolding!();

#[derive(Debug, uniffi::Error)]
pub enum CoreError {
    /// An event or an answer that is not valid JSON for the core's vocabulary.
    BadMessage { detail: String },
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::BadMessage { detail } => write!(f, "bad message: {detail}"),
        }
    }
}

impl std::error::Error for CoreError {}

/// Implemented by the shell: receives every update (any thread).
#[uniffi::export(with_foreign)]
pub trait ShellListener: Send + Sync {
    fn on_update(&self, update_json: String);
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum FrameFormat {
    Rgba,
    Bgra,
    /// 4:2:0 Y'CbCr, full range: NV12 (chroma pixel stride 2) or I420 (stride 1).
    Yuv420,
    /// Packed 4:2:2 (Y0 Cb Y1 Cr), plane 0 only.
    Yuyv,
}

/// A camera frame, described by the addresses of the platform's own buffers. The buffers
/// must stay valid and unchanged for the duration of the call.
#[derive(Clone, Debug, uniffi::Record)]
pub struct CameraFrame {
    pub width: u32,
    pub height: u32,
    pub format: FrameFormat,
    /// Clockwise rotation that shows the frame upright (0, 90, 180, 270).
    pub rotation: u32,
    pub plane0: u64,
    pub len0: u64,
    pub row_stride0: u32,
    pub plane1: u64,
    pub len1: u64,
    pub row_stride1: u32,
    pub pixel_stride1: u32,
    pub plane2: u64,
    pub len2: u64,
    pub row_stride2: u32,
    pub pixel_stride2: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SendFrame {
    pub seq: u32,
    /// (X + 4) × (Y + 4): one RGBA pixel per cell, quiet zone included.
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SendMask {
    /// Device pixels: the size to draw the code at.
    pub width: u32,
    pub height: u32,
    /// Finders, markers and the gaps between cells; transparent where a cell's colour shows.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum JsonSpanKind {
    Key,
    Str,
    Num,
    Bool,
    Null,
    Punct,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct JsonSpan {
    pub kind: JsonSpanKind,
    pub text: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct JsonLine {
    /// Nesting depth; indent by two spaces per level and hang wrapped lines under the start.
    pub depth: u32,
    pub spans: Vec<JsonSpan>,
}

/// A link in `ReaderContent::text`, in UTF-16 code units (NSRange, Kotlin indices).
#[derive(Clone, Debug, uniffi::Record)]
pub struct TextLink {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ReaderContent {
    pub text: Option<String>,
    pub lines: Vec<JsonLine>,
    pub links: Vec<TextLink>,
    pub truncated: bool,
}

#[derive(uniffi::Object)]
pub struct LumenCore {
    rt: Runtime,
}

/// The static labels of one language ("zh" or "en") as a JSON object.
#[uniffi::export]
pub fn lumen_labels(lang: String) -> String {
    serde_json::to_string(&labels(Lang::from_tag(&lang))).unwrap_or_else(|_| "{}".into())
}

/// Runs the scan pipeline on synthetic camera frames and reports the time per scan on this
/// device (see `lumen_app::engine::bench`). Takes a few seconds; call off the main thread.
/// Debug: replays raw camera frames from `dir` through the receive engine (see the shells'
/// `replay` script command).
#[uniffi::export]
pub fn lumen_replay(dir: String, rounds: u32) -> String {
    lumen_app::engine::bench::replay(&dir, rounds as usize)
}

#[uniffi::export]
pub fn lumen_selftest(camera_frames: u32) -> String {
    lumen_app::engine::bench::run(camera_frames as usize).summary()
}

#[uniffi::export]
impl LumenCore {
    #[uniffi::constructor]
    pub fn new(listener: Arc<dyn ShellListener>) -> Arc<Self> {
        let rt = Runtime::new(move |u: Update| {
            if let Ok(json) = serde_json::to_string(&u) {
                listener.on_update(json);
            }
        });
        Arc::new(LumenCore { rt })
    }

    pub fn dispatch(&self, event_json: String) -> Result<(), CoreError> {
        let ev: Event = serde_json::from_str(&event_json).map_err(|e| CoreError::BadMessage { detail: format!("{e}: {event_json}") })?;
        self.rt.dispatch(ev);
        Ok(())
    }

    pub fn resolve(&self, id: u64, output_json: String) -> Result<(), CoreError> {
        let out: ShellOutput =
            serde_json::from_str(&output_json).map_err(|e| CoreError::BadMessage { detail: format!("{e}: {output_json}") })?;
        self.rt.resolve(id, out);
        Ok(())
    }

    /// The current view model as JSON.
    pub fn view(&self) -> String {
        serde_json::to_string(&self.rt.view()).unwrap_or_else(|_| "{}".into())
    }

    /// Milliseconds on the core's clock.
    pub fn now(&self) -> f64 {
        self.rt.now()
    }

    pub fn load_file(&self, name: String, data: Vec<u8>) {
        self.rt.load_file(&name, data);
    }

    pub fn send_frame(&self) -> Option<SendFrame> {
        self.rt.send_frame().map(|f| SendFrame { seq: f.seq, width: f.width, height: f.height, rgba: f.rgba })
    }

    pub fn send_mask(&self) -> Option<SendMask> {
        self.rt.send_mask().map(|(width, height, rgba)| SendMask { width, height, rgba: Arc::unwrap_or_clone(rgba) })
    }

    /// Scans the frame when the pacing says so. Returns whether it was scanned.
    pub fn camera_frame(&self, frame: CameraFrame) -> bool {
        let format = match frame.format {
            FrameFormat::Rgba => PixelFormat::Rgba,
            FrameFormat::Bgra => PixelFormat::Bgra,
            FrameFormat::Yuv420 => PixelFormat::Yuv420,
            FrameFormat::Yuyv => PixelFormat::Yuyv,
        };
        // SAFETY: the shell passes the addresses and lengths of buffers it keeps locked
        // and unchanged until this call returns.
        let plane = |addr: u64, len: u64, row: u32, px: u32| -> Plane<'_> {
            if addr == 0 || len == 0 {
                return Plane::EMPTY;
            }
            let data = unsafe { std::slice::from_raw_parts(addr as usize as *const u8, len as usize) };
            Plane { data, row_stride: row as usize, pixel_stride: px.max(1) as usize }
        };
        let planes = match format {
            PixelFormat::Rgba | PixelFormat::Bgra => [plane(frame.plane0, frame.len0, frame.row_stride0, 4), Plane::EMPTY, Plane::EMPTY],
            PixelFormat::Yuyv => [plane(frame.plane0, frame.len0, frame.row_stride0, 2), Plane::EMPTY, Plane::EMPTY],
            PixelFormat::Yuv420 => [
                plane(frame.plane0, frame.len0, frame.row_stride0, 1),
                plane(frame.plane1, frame.len1, frame.row_stride1, frame.pixel_stride1),
                plane(frame.plane2, frame.len2, frame.row_stride2, frame.pixel_stride2),
            ],
        };
        let f = FrameRef { width: frame.width as usize, height: frame.height as usize, format, planes, rotation: frame.rotation };
        self.rt.camera_frame(&f)
    }

    /// The camera's rolling-shutter readout in ms (Android: SENSOR_ROLLING_SHUTTER_SKEW).
    pub fn camera_readout(&self, ms: f32) {
        self.rt.camera_readout(ms as f64);
    }

    /// Decodes a still image (tightly packed RGBA).
    pub fn scan_image(&self, width: u32, height: u32, rgba: Vec<u8>) {
        if rgba.len() >= width as usize * height as usize * 4 {
            self.rt.scan_image(&FrameRef::rgba(width as usize, height as usize, &rgba));
        }
    }

    pub fn received_name(&self) -> Option<String> {
        self.rt.received_name()
    }

    pub fn received_bytes(&self) -> Option<Vec<u8>> {
        self.rt.received_bytes().map(|b| b.as_ref().clone())
    }

    pub fn copy_text(&self, pretty: bool) -> Option<String> {
        self.rt.copy_text(pretty)
    }

    pub fn reader(&self, pretty: bool) -> Option<ReaderContent> {
        let r = self.rt.reader(pretty)?;
        let links = match &r.text {
            Some(text) if !r.links.is_empty() => lumen_app::content::links_utf16(text)
                .into_iter()
                .map(|(a, b)| TextLink { start: a as u32, end: b as u32 })
                .collect(),
            _ => Vec::new(),
        };
        let lines = r
            .lines
            .unwrap_or_default()
            .into_iter()
            .map(|l| JsonLine {
                depth: l.depth,
                spans: l
                    .spans
                    .into_iter()
                    .map(|s| JsonSpan {
                        kind: match s.kind {
                            SpanKind::Key => JsonSpanKind::Key,
                            SpanKind::Str => JsonSpanKind::Str,
                            SpanKind::Num => JsonSpanKind::Num,
                            SpanKind::Bool => JsonSpanKind::Bool,
                            SpanKind::Null => JsonSpanKind::Null,
                            SpanKind::Punct => JsonSpanKind::Punct,
                        },
                        text: s.text,
                    })
                    .collect(),
            })
            .collect();
        Some(ReaderContent { text: r.text, lines, links, truncated: r.truncated })
    }
}
