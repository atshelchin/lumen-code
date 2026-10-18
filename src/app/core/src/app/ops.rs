//! What the core asks the outside world to do. Two executors answer:
//!
//! - [`ShellOp`]: the platform shell (Swift, Kotlin, GPUI) — camera, haptics, files, screen.
//! - [`EngineOp`]: the Rust runtime ([`crate::runtime`]) — the sender stream, the scan
//!   pipeline, the decoder, timers. No platform shell ever sees these.
//!
//! Operations are sentences, not work: `CameraStart` asks for a camera; the core never
//! touches one.

use crate::engine::send::SenderInfo;
use crate::geometry::Geometry;
use crux_core::capability::Operation;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HapticKind {
    /// A light tap: a button that changes a mode (play/pause), a code coming into view
    Light,
    /// A firmer tap: a transfer starts playing
    Medium,
    /// A detent: a segmented choice, a progress milestone
    Selection,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ShellOp {
    /// Read the stored preferences (a JSON string, or nothing).
    LoadPrefs,
    /// Store the preferences. Best effort.
    SavePrefs { json: String },
    /// Keep the screen on (sending, scanning) or let it sleep.
    KeepAwake { on: bool },
    /// Raise the screen to full brightness while a code is on it, or restore it.
    Brightness { boost: bool },
    /// Hide the system bars while a code fills the screen.
    Immersive { on: bool },
    Haptic { kind: HapticKind },
    /// Open the back camera (`device` cycles through the cameras the platform offers).
    /// Frames go to the runtime's `camera_frame`.
    CameraStart { device: u32 },
    CameraStop,
    CameraZoom { level: f32 },
    /// Capture mode. `fps`: 60 when the sender flips faster than a 30 fps camera catches
    /// cleanly, else 30. `uhd`: 4K when cells are small — 4:2:0 colour then has a pixel per
    /// cell instead of half of one — else 1080p. Both cost power, so both are temporary.
    CameraMode { fps: u32, uhd: bool },
    /// The longest exposure that keeps a capture to one sender frame (µs): about a sixth of
    /// its period. Shorter exposures need more gain (noise); longer ones blend two frames.
    /// Shells that run their own exposure may ignore it.
    CameraExposure { max_us: u32 },
    /// Show a file picker; the chosen file goes to the runtime's `load_file`.
    PickFile,
    /// Show an image picker; the decoded image goes to the runtime's `scan_image`.
    PickImage,
    /// Save / share / open the received file (bytes from the runtime's `received_bytes`).
    SaveFile { name: String, mime: String },
    ShareFile { name: String, mime: String },
    OpenFile { name: String, mime: String },
    /// Copy the received text (formatted JSON when `pretty`), from the runtime's `copy_text`.
    CopyText { pretty: bool },
    OpenUrl { url: String },
    /// Open the app's page in the system settings (camera permission).
    OpenSettings,
}

impl ShellOp {
    /// Operations whose answer the core never waits for.
    pub fn is_notification(&self) -> bool {
        !matches!(
            self,
            ShellOp::LoadPrefs
                | ShellOp::CameraStart { .. }
                | ShellOp::PickFile
                | ShellOp::PickImage
                | ShellOp::SaveFile { .. }
                | ShellOp::ShareFile { .. }
                | ShellOp::OpenFile { .. }
                | ShellOp::CopyText { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraError {
    /// The person (or a policy) refused access.
    Denied,
    /// No usable camera.
    Unavailable,
    /// Anything else; the message says what.
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ShellOutput {
    Done,
    Prefs {
        #[serde(default)]
        json: Option<String>,
    },
    Camera {
        ok: bool,
        #[serde(default)]
        error: Option<CameraError>,
        #[serde(default)]
        message: Option<String>,
        #[serde(default = "one")]
        zoom_min: f32,
        #[serde(default = "one")]
        zoom_max: f32,
        #[serde(default = "one")]
        zoom: f32,
        #[serde(default = "one_u32")]
        devices: u32,
        /// The highest capture rate the camera offers at the scanning resolution.
        #[serde(default = "thirty")]
        max_fps: u32,
        /// Whether a 4K mode is available.
        #[serde(default)]
        uhd: bool,
    },
    /// A picker or a save/share sheet finished.
    Finished {
        ok: bool,
        #[serde(default)]
        cancelled: bool,
    },
}

fn one() -> f32 {
    1.0
}
fn one_u32() -> u32 {
    1
}
fn thirty() -> u32 {
    30
}

impl Operation for ShellOp {
    type Output = ShellOutput;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EngineOp {
    /// Make the typed text the source to send.
    LoadText { text: String },
    /// Build the payload of the loaded source (DEFLATE when it helps). Runs on a thread.
    Prepare,
    /// A new sender for the prepared payload. `keep_tid`: continue the current transfer id.
    StartSender { mode: u8, ecc: u8, geometry: Geometry, keep_tid: bool },
    /// Colours / ECC changed: keep the stream when the packet size allows it, else start anew.
    Reconfigure { mode: u8, ecc: u8, geometry: Geometry },
    /// The screen changed: same stream, new grid.
    Resize { geometry: Geometry },
    StopSender,
    /// Accept camera frames for scanning, or ignore them.
    RecvActive { on: bool },
    /// Forget the current reception; results of older generations are dropped.
    ResetRecv { generation: u64 },
    /// The device's heat (0 cool … 3 critical): the receiver scans less as it warms.
    Heat { level: u8 },
    /// Answer after `ms` milliseconds with the time.
    Delay { ms: u32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EngineOutput {
    Done,
    Loaded { name: String, size: u64 },
    Prepared { ok: bool, payload_len: u64 },
    Sender(SenderInfo),
    Elapsed { now: f64 },
}

impl Operation for EngineOp {
    type Output = EngineOutput;
}
