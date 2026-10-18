//! The Lumen app as one Crux state machine: home, send and receive.
//!
//! Everything here **decides**; nothing here **does**. An [`Event`] updates the [`Model`] and
//! returns commands: a render, a [`ShellOp`] for the platform (camera, haptics, files, screen)
//! or an [`EngineOp`] for the Rust runtime (sender stream, scan pipeline, decoder, timers).
//! No clock, no I/O, no threads — which is what makes the rules testable and identical on
//! iOS, Android and the desktop.
//!
//! The rules come from the web demo (`src/web/js/*-app.js`): cell geometry, the dock that
//! hides itself, frame-locked scanning (in the engine), the dashboard and its hints, the
//! result reader. The apps add what a phone can do better: haptics, the screen kept on at
//! full brightness while a code is shown, system bars hidden, the camera released whenever
//! it is not needed.

pub mod ops;
pub mod view;

use crate::content::{Classified, Kind};
use crate::engine::recv::ScanReport;
use crate::engine::send::SenderInfo;
use crate::engine::sink::ProgressFact;
use crate::geometry::{self, Density, Geometry, Viewport};
use crate::i18n::Lang;
use crux_core::macros::effect;
use crux_core::render::{render, RenderOperation};
use crux_core::{App, Command};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub use ops::{CameraError, EngineOp, EngineOutput, HapticKind, ShellOp, ShellOutput};
pub use view::ViewModel;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    #[default]
    Ios,
    Android,
    Desktop,
}

impl Platform {
    pub fn mobile(self) -> bool {
        self != Platform::Desktop
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    #[default]
    Home,
    Send,
    Receive,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceTab {
    #[default]
    File,
    Text,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendPhase {
    /// Choosing a file or typing text
    #[default]
    Pick,
    /// Building the payload
    Preparing,
    /// The code is on screen (playing or paused)
    Ready,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecvPhase {
    #[default]
    Start,
    Scanning,
    Done,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderPrefs {
    pub pretty: bool,
    pub wrap: bool,
    pub font: i32,
    /// Transfer statistics unfolded in the reader; unset = by screen size.
    pub stats: Option<bool>,
}

impl Default for ReaderPrefs {
    fn default() -> Self {
        ReaderPrefs { pretty: true, wrap: true, font: 0, stats: None }
    }
}

/// Choices that persist across launches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    pub lang: Option<Lang>,
    pub density: Density,
    /// 1 = 4 colours, 2 = 8 colours
    pub mode: u8,
    /// 0..=2 (low, mid, high)
    pub ecc: u8,
    pub fps: u32,
    pub source: SourceTab,
    pub reader: ReaderPrefs,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { lang: None, density: Density::Auto, mode: 1, ecc: 1, fps: 12, source: SourceTab::File, reader: ReaderPrefs::default() }
    }
}

impl Prefs {
    /// Stored values are checked, never trusted.
    fn sanitized(mut self) -> Prefs {
        if !(1..=2).contains(&self.mode) {
            self.mode = 1;
        }
        self.ecc = self.ecc.min(2);
        self.fps = self.fps.clamp(MIN_FPS, MAX_FPS);
        self.reader.font = self.reader.font.clamp(-4, 8);
        self
    }
}

pub const MIN_FPS: u32 = 4;
/// 60 needs a 120 Hz screen on the sender for even frame times and a 120 fps camera on the
/// receiver; the core switches the receiver's camera up as the sender's rate rises.
pub const MAX_FPS: u32 = 60;
/// The dock hides itself after this long without a touch while the code plays.
pub const DOCK_IDLE_MS: u32 = 2600;
/// Dashboard refresh while scanning (numbers that age: status, speed).
pub const TICK_MS: u32 = 500;
/// A code counts as in view for this long after the last scan that found it.
pub const FOUND_MS: f64 = 700.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Loaded,
    Prepared,
    Started,
    Adjusted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerKind {
    Dock,
    Tick,
    Copied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    Pick,
    Save,
    Share,
    Open,
    Copy,
    PickImage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum Event {
    // ---- from the shell
    /// App launch: the system language and the platform.
    Start { locale: String, platform: Platform },
    Navigate { screen: Screen },
    ToggleLang,
    /// The space the sender's code may use (device pixels, safe area removed).
    Viewport { width_px: f64, height_px: f64, scale: f64, mm_per_px: f64 },
    /// The app went to the background (or came back).
    AppActive { active: bool },
    /// The device's thermal state: 0 nominal, 1 fair, 2 serious, 3 critical.
    Thermal { level: u8 },
    SourceTab { tab: SourceTab },
    TextChanged { text: String },
    PickFile,
    SendText,
    SetDensity { value: Density },
    SetMode { value: u8 },
    SetEcc { value: u8 },
    SetFps { value: u32 },
    TogglePlay,
    ToggleSettings,
    /// A tap on the code: toggles the dock.
    TapCode,
    HideDock,
    ShowDock,
    /// A touch inside the dock: restart its idle timer.
    DockActivity,
    CloseSend,
    OpenCamera,
    SwitchCamera,
    SetZoom { level: f32 },
    ResetZoom,
    PickImage,
    ReceiveAnother,
    Copy,
    OpenLink,
    Save,
    Share,
    OpenFile,
    OpenSettings,
    SetJsonView { pretty: bool },
    ToggleWrap,
    SetFont { delta: i32 },
    ToggleStats,

    // ---- from the runtime and from answered operations (never on the wire)
    #[serde(skip)]
    PrefsLoaded(ShellOutput),
    #[serde(skip)]
    SourceLoaded { name: String, size: u64, text: Option<String> },
    #[serde(skip)]
    SourceFailed { message: String },
    #[serde(skip)]
    Engine { step: Step, attempt: u64, out: EngineOutput },
    #[serde(skip)]
    CameraAnswer { attempt: u64, out: ShellOutput },
    #[serde(skip)]
    Finished { action: FileAction, out: ShellOutput },
    #[serde(skip)]
    Scanned(Box<ScanReport>),
    #[serde(skip)]
    ImageScanned { generation: u64, found: bool, at: f64 },
    #[serde(skip)]
    Progress { fact: ProgressFact, at: f64 },
    #[serde(skip)]
    Received { generation: u64, name: String, size: u64, classified: Classified, at: f64 },
    #[serde(skip)]
    ChecksumError { generation: u64, at: f64 },
    #[serde(skip)]
    SenderStats { build_ms: f64 },
    #[serde(skip)]
    Timer { kind: TimerKind, attempt: u64, out: EngineOutput },
}

#[effect]
pub enum Effect {
    Render(RenderOperation),
    Shell(ShellOp),
    Engine(EngineOp),
}

pub(crate) struct SourceMeta {
    pub name: String,
    pub size: u64,
    /// The text, for a message typed on the sender (label only).
    pub text: Option<String>,
}

#[derive(Default)]
pub(crate) struct SendState {
    pub phase: SendPhase,
    pub text: String,
    pub source: Option<SourceMeta>,
    pub info: Option<SenderInfo>,
    pub geometry: Option<Geometry>,
    /// Bumps whenever the shell must fetch a new mask / layout.
    pub frame_rev: u64,
    pub playing: bool,
    pub settings_open: Option<bool>,
    pub dock_visible: bool,
    pub dock_attempt: u64,
    pub build_ms: f64,
    pub error: Option<String>,
    pub attempt: u64,
}

#[derive(Default)]
pub(crate) struct CameraState {
    pub on: bool,
    pub starting: bool,
    pub error: Option<(CameraError, String)>,
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub zoom: f32,
    pub devices: u32,
    pub device: u32,
    /// Capture mode in force, and what the camera offers.
    pub fps: u32,
    pub max_fps: u32,
    pub uhd: bool,
    pub uhd_ok: bool,
    /// When the mode last changed (a new mode needs time to show its effect).
    pub mode_at: f64,
    /// The exposure limit last sent (µs; 0: none yet).
    pub exposure_us: u32,
    /// Restart when the app returns to the foreground / after "receive another".
    pub resume: bool,
    pub attempt: u64,
}

pub(crate) struct DoneState {
    pub name: String,
    pub size: u64,
    pub secs: f64,
    pub classified: Classified,
    pub stats: Option<ScanReport>,
    pub received: u32,
    pub t: u32,
    pub copied: bool,
    pub copied_attempt: u64,
    pub note: Option<(bool, &'static str)>,
    pub content_rev: u64,
}

#[derive(Default)]
pub(crate) struct RecvState {
    pub phase: RecvPhase,
    pub generation: u64,
    pub camera: CameraState,
    pub last: Option<ScanReport>,
    pub progress: Option<ProgressFact>,
    pub prog_hist: VecDeque<(f64, u32)>,
    pub first_packet_at: f64,
    pub active_tid: Option<u16>,
    pub checksum_until: f64,
    pub milestone: u8,
    pub found: bool,
    pub lost_at: f64,
    pub done: Option<DoneState>,
    /// A one-line note on the start screen (e.g. no code in the chosen image).
    pub note: Option<&'static str>,
    pub tick_attempt: u64,
    pub ticking: bool,
    pub from_image: bool,
}

#[derive(Default)]
pub(crate) struct DeviceState {
    pub awake: bool,
    pub bright: bool,
    pub immersive: bool,
}

#[derive(Default)]
pub struct Model {
    pub(crate) started: bool,
    pub(crate) platform: Platform,
    pub(crate) lang: Lang,
    pub(crate) screen: Screen,
    pub(crate) prefs: Prefs,
    pub(crate) viewport: Viewport,
    pub(crate) now: f64,
    pub(crate) send: SendState,
    pub(crate) recv: RecvState,
    pub(crate) device: DeviceState,
    pub(crate) thermal: u8,
    pub(crate) rev: u64,
}

#[derive(Default)]
pub struct Lumen;

type Cmd = Command<Effect, Event>;

fn notify(op: ShellOp) -> Cmd {
    Command::notify_shell(op).build()
}

fn haptic(kind: HapticKind) -> Cmd {
    notify(ShellOp::Haptic { kind })
}

fn engine(op: EngineOp, step: Step, attempt: u64) -> Cmd {
    Command::request_from_shell(op).then_send(move |out| Event::Engine { step, attempt, out })
}

fn timer(kind: TimerKind, attempt: u64, ms: u32) -> Cmd {
    Command::request_from_shell(EngineOp::Delay { ms }).then_send(move |out| Event::Timer { kind, attempt, out })
}

fn engine_notify(op: EngineOp) -> Cmd {
    Command::notify_shell(op).build()
}

fn save_prefs(model: &Model) -> Cmd {
    match serde_json::to_string(&model.prefs) {
        Ok(json) => notify(ShellOp::SavePrefs { json }),
        Err(_) => Command::done(),
    }
}

impl App for Lumen {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Cmd {
        let cmd = update(event, model);
        let device = sync_device(model);
        model.rev += 1;
        Command::all([cmd, device, render()])
    }

    fn view(&self, model: &Model) -> ViewModel {
        view::view(model)
    }
}

/// Screen on, brightness and system bars follow what is on screen.
fn sync_device(model: &mut Model) -> Cmd {
    let showing_code = model.screen == Screen::Send && model.send.phase == SendPhase::Ready;
    let scanning = model.screen == Screen::Receive && model.recv.phase == RecvPhase::Scanning;
    let awake = showing_code || scanning;
    let bright = showing_code && model.platform.mobile();
    let immersive = showing_code;
    let mut cmds = Vec::new();
    if awake != model.device.awake {
        model.device.awake = awake;
        cmds.push(notify(ShellOp::KeepAwake { on: awake }));
    }
    if bright != model.device.bright {
        model.device.bright = bright;
        cmds.push(notify(ShellOp::Brightness { boost: bright }));
    }
    if immersive != model.device.immersive {
        model.device.immersive = immersive;
        cmds.push(notify(ShellOp::Immersive { on: immersive }));
    }
    Command::all(cmds)
}

fn update(event: Event, model: &mut Model) -> Cmd {
    match event {
        Event::Start { locale, platform } => {
            if model.started {
                return Command::done();
            }
            model.started = true;
            model.platform = platform;
            model.lang = Lang::from_tag(&locale);
            Command::request_from_shell(ShellOp::LoadPrefs).then_send(Event::PrefsLoaded)
        }
        Event::PrefsLoaded(out) => {
            if let ShellOutput::Prefs { json: Some(json) } = out {
                if let Ok(p) = serde_json::from_str::<Prefs>(&json) {
                    model.prefs = p.sanitized();
                    if let Some(l) = model.prefs.lang {
                        model.lang = l;
                    }
                }
            }
            Command::done()
        }
        Event::Navigate { screen } => navigate(model, screen),
        Event::ToggleLang => {
            model.lang = model.lang.toggled();
            model.prefs.lang = Some(model.lang);
            save_prefs(model)
        }
        Event::Viewport { width_px, height_px, scale, mm_per_px } => {
            let vp = Viewport { width_px, height_px, scale, mm_per_px };
            if vp == model.viewport || width_px < 50.0 || height_px < 50.0 {
                return Command::done();
            }
            model.viewport = vp;
            regrid(model)
        }
        Event::Thermal { level } => {
            model.thermal = level.min(3);
            let heat = engine_notify(EngineOp::Heat { level: model.thermal });
            // hot: the fast capture modes go first (they cost the most)
            let cam = &mut model.recv.camera;
            if model.thermal >= 2 && cam.on && cam.fps > 60 {
                cam.fps = 60;
                cam.mode_at = model.now;
                return Command::all([heat, notify(ShellOp::CameraMode { fps: 60, uhd: cam.uhd })]);
            }
            heat
        }
        Event::AppActive { active } => {
            let cam = &mut model.recv.camera;
            if !active && (cam.on || cam.starting) {
                cam.on = false;
                cam.starting = false;
                cam.resume = true;
                return Command::all([notify(ShellOp::CameraStop), engine_notify(EngineOp::RecvActive { on: false })]);
            }
            if active && cam.resume && model.screen == Screen::Receive && model.recv.phase != RecvPhase::Done {
                cam.resume = false;
                return start_camera(model);
            }
            Command::done()
        }

        // ------------------------------------------------------------------ send
        Event::SourceTab { tab } => {
            model.prefs.source = tab;
            save_prefs(model)
        }
        Event::TextChanged { text } => {
            model.send.text = text;
            Command::done()
        }
        Event::PickFile => Command::request_from_shell(ShellOp::PickFile).then_send(move |out| Event::Finished { action: FileAction::Pick, out }),
        Event::SendText => {
            if model.send.text.is_empty() || model.send.phase != SendPhase::Pick {
                return Command::done();
            }
            model.send.attempt += 1;
            engine(EngineOp::LoadText { text: model.send.text.clone() }, Step::Loaded, model.send.attempt)
        }
        Event::SourceLoaded { name, size, text } => {
            model.screen = Screen::Send;
            source_loaded(model, name, size, text)
        }
        Event::SourceFailed { message } => {
            model.send.error = Some(message);
            model.send.phase = SendPhase::Pick;
            haptic(HapticKind::Error)
        }
        Event::Engine { step, attempt, out } => engine_answer(model, step, attempt, out),
        Event::SenderStats { build_ms } => {
            model.send.build_ms = build_ms;
            Command::done()
        }
        Event::SetDensity { value } => {
            if model.prefs.density == value {
                return Command::done();
            }
            model.prefs.density = value;
            Command::all([save_prefs(model), haptic(HapticKind::Selection), poke_dock(model), regrid(model)])
        }
        Event::SetMode { value } => {
            if !(1..=2).contains(&value) || model.prefs.mode == value {
                return Command::done();
            }
            model.prefs.mode = value;
            Command::all([save_prefs(model), haptic(HapticKind::Selection), poke_dock(model), reconfigure(model)])
        }
        Event::SetEcc { value } => {
            if value > 2 || model.prefs.ecc == value {
                return Command::done();
            }
            model.prefs.ecc = value;
            Command::all([save_prefs(model), haptic(HapticKind::Selection), poke_dock(model), reconfigure(model)])
        }
        Event::SetFps { value } => {
            let v = value.clamp(MIN_FPS, MAX_FPS);
            if v == model.prefs.fps {
                return Command::done();
            }
            model.prefs.fps = v;
            Command::all([save_prefs(model), haptic(HapticKind::Selection), poke_dock(model)])
        }
        Event::TogglePlay => {
            if model.send.phase != SendPhase::Ready {
                return Command::done();
            }
            model.send.playing = !model.send.playing;
            Command::all([haptic(HapticKind::Light), show_dock(model)])
        }
        Event::ToggleSettings => {
            let open = !settings_open(model);
            model.send.settings_open = Some(open);
            poke_dock(model)
        }
        Event::TapCode => {
            if model.send.dock_visible {
                model.send.dock_visible = false;
                model.send.dock_attempt += 1;
                Command::done()
            } else {
                show_dock(model)
            }
        }
        Event::HideDock => {
            model.send.dock_visible = false;
            model.send.dock_attempt += 1;
            Command::done()
        }
        Event::ShowDock => show_dock(model),
        Event::DockActivity => poke_dock(model),
        Event::CloseSend => close_send(model),

        // ------------------------------------------------------------------ receive
        Event::OpenCamera => {
            model.recv.note = None;
            start_camera(model)
        }
        Event::CameraAnswer { attempt, out } => camera_answer(model, attempt, out),
        Event::SwitchCamera => {
            let cam = &mut model.recv.camera;
            if cam.devices < 2 || !cam.on {
                return Command::done();
            }
            cam.device = (cam.device + 1) % cam.devices;
            Command::all([haptic(HapticKind::Light), start_camera(model)])
        }
        Event::SetZoom { level } => {
            let cam = &mut model.recv.camera;
            if !cam.on || cam.zoom_max <= cam.zoom_min || !level.is_finite() {
                return Command::done();
            }
            let z = level.clamp(cam.zoom_min, cam.zoom_max);
            if (z - cam.zoom).abs() < 0.005 {
                return Command::done();
            }
            cam.zoom = z;
            notify(ShellOp::CameraZoom { level: z })
        }
        Event::ResetZoom => {
            let cam = &mut model.recv.camera;
            if !cam.on {
                return Command::done();
            }
            cam.zoom = cam.zoom_min;
            Command::all([notify(ShellOp::CameraZoom { level: cam.zoom_min }), haptic(HapticKind::Selection)])
        }
        Event::PickImage => {
            model.recv.note = None;
            Command::request_from_shell(ShellOp::PickImage).then_send(move |out| Event::Finished { action: FileAction::PickImage, out })
        }
        Event::ImageScanned { generation, found, at } => {
            if generation != model.recv.generation {
                return Command::done();
            }
            model.now = at;
            if !found {
                if model.recv.phase == RecvPhase::Start {
                    model.recv.note = Some("imageNoCode");
                }
                return haptic(HapticKind::Warning);
            }
            if model.recv.phase == RecvPhase::Start {
                model.recv.phase = RecvPhase::Scanning;
                model.recv.from_image = true;
                return start_tick(model);
            }
            Command::done()
        }
        Event::Scanned(report) => scanned(model, *report),
        Event::Progress { fact, at } => progress(model, fact, at),
        Event::Received { generation, name, size, classified, at } => received(model, generation, name, size, classified, at),
        Event::ChecksumError { generation, at } => {
            if generation != model.recv.generation {
                return Command::done();
            }
            model.now = at;
            model.recv.checksum_until = at + 3000.0;
            haptic(HapticKind::Warning)
        }
        Event::Timer { kind, attempt, out } => {
            if let EngineOutput::Elapsed { now } = out {
                model.now = model.now.max(now);
            }
            match kind {
                TimerKind::Dock => {
                    if attempt == model.send.dock_attempt && model.send.playing && model.send.phase == SendPhase::Ready {
                        model.send.dock_visible = false;
                    }
                    Command::done()
                }
                TimerKind::Tick => {
                    if attempt != model.recv.tick_attempt || model.recv.phase != RecvPhase::Scanning || model.screen != Screen::Receive {
                        model.recv.ticking = false;
                        return Command::done();
                    }
                    timer(TimerKind::Tick, attempt, TICK_MS)
                }
                TimerKind::Copied => {
                    if let Some(d) = &mut model.recv.done {
                        if d.copied_attempt == attempt {
                            d.copied = false;
                        }
                    }
                    Command::done()
                }
            }
        }
        Event::ReceiveAnother => receive_another(model),
        Event::Copy => {
            let pretty = model.prefs.reader.pretty;
            match &model.recv.done {
                Some(d) if matches!(d.classified.kind, Kind::Message | Kind::Text | Kind::Json) => {
                    Command::request_from_shell(ShellOp::CopyText { pretty }).then_send(move |out| Event::Finished { action: FileAction::Copy, out })
                }
                _ => Command::done(),
            }
        }
        Event::OpenLink => match model.recv.done.as_ref().and_then(|d| d.classified.link.clone()) {
            Some(url) => notify(ShellOp::OpenUrl { url }),
            None => Command::done(),
        },
        Event::Save | Event::Share | Event::OpenFile => {
            let Some(d) = &model.recv.done else { return Command::done() };
            let (name, mime) = (d.name.clone(), d.classified.mime.clone());
            let (op, action) = match event {
                Event::Save => (ShellOp::SaveFile { name, mime }, FileAction::Save),
                Event::Share => (ShellOp::ShareFile { name, mime }, FileAction::Share),
                _ => (ShellOp::OpenFile { name, mime }, FileAction::Open),
            };
            Command::request_from_shell(op).then_send(move |out| Event::Finished { action, out })
        }
        Event::OpenSettings => notify(ShellOp::OpenSettings),
        Event::Finished { action, out } => finished(model, action, out),
        Event::SetJsonView { pretty } => {
            if model.prefs.reader.pretty == pretty {
                return Command::done();
            }
            model.prefs.reader.pretty = pretty;
            bump_content(model);
            Command::all([save_prefs(model), haptic(HapticKind::Selection)])
        }
        Event::ToggleWrap => {
            model.prefs.reader.wrap = !model.prefs.reader.wrap;
            save_prefs(model)
        }
        Event::SetFont { delta } => {
            let f = (model.prefs.reader.font + delta.signum()).clamp(-4, 8);
            if f == model.prefs.reader.font {
                return Command::done();
            }
            model.prefs.reader.font = f;
            save_prefs(model)
        }
        Event::ToggleStats => {
            let open = !stats_open(model);
            model.prefs.reader.stats = Some(open);
            save_prefs(model)
        }
    }
}

// ---------------------------------------------------------------------- navigation

fn navigate(model: &mut Model, screen: Screen) -> Cmd {
    if screen == model.screen {
        return Command::done();
    }
    let mut cmds = Vec::new();
    match model.screen {
        Screen::Send if model.send.phase != SendPhase::Pick => cmds.push(close_send(model)),
        Screen::Receive => cmds.push(leave_receive(model)),
        _ => {}
    }
    model.screen = screen;
    Command::all(cmds)
}

fn leave_receive(model: &mut Model) -> Cmd {
    let mut cmds = Vec::new();
    let cam = &mut model.recv.camera;
    if cam.on || cam.starting {
        cmds.push(notify(ShellOp::CameraStop));
    }
    cam.on = false;
    cam.starting = false;
    cam.resume = false;
    cam.attempt += 1;
    cam.error = None;
    let generation = model.recv.generation + 1;
    let camera = std::mem::take(&mut model.recv.camera);
    model.recv = RecvState { generation, camera, ..RecvState::default() };
    cmds.push(engine_notify(EngineOp::RecvActive { on: false }));
    cmds.push(engine_notify(EngineOp::ResetRecv { generation }));
    Command::all(cmds)
}

// ---------------------------------------------------------------------- send

fn settings_open(model: &Model) -> bool {
    model.send.settings_open.unwrap_or(!model.platform.mobile())
}

fn source_loaded(model: &mut Model, name: String, size: u64, text: Option<String>) -> Cmd {
    let s = &mut model.send;
    s.attempt += 1;
    s.phase = SendPhase::Preparing;
    s.error = None;
    s.info = None;
    s.source = Some(SourceMeta { name, size, text });
    engine(EngineOp::Prepare, Step::Prepared, s.attempt)
}

fn engine_answer(model: &mut Model, step: Step, attempt: u64, out: EngineOutput) -> Cmd {
    if attempt != model.send.attempt {
        return Command::done(); // superseded (closed, or a newer file)
    }
    match (step, out) {
        (Step::Loaded, EngineOutput::Loaded { name, size }) => {
            let text = Some(model.send.text.clone());
            model.screen = Screen::Send;
            source_loaded(model, name, size, text)
        }
        (Step::Prepared, EngineOutput::Prepared { ok: true, .. }) => {
            let g = geometry::compute(&model.viewport, model.prefs.density);
            model.send.geometry = Some(g);
            let op = EngineOp::StartSender { mode: model.prefs.mode, ecc: model.prefs.ecc, geometry: g, keep_tid: false };
            engine(op, Step::Started, attempt)
        }
        (Step::Started | Step::Adjusted, EngineOutput::Sender(info)) if info.ok => {
            let first = step == Step::Started;
            model.send.info = Some(info);
            model.send.frame_rev += 1;
            model.send.phase = SendPhase::Ready;
            if first {
                model.send.playing = true;
                Command::all([haptic(HapticKind::Medium), show_dock(model)])
            } else {
                Command::done()
            }
        }
        (_, EngineOutput::Prepared { ok: false, .. }) | (_, EngineOutput::Sender(_)) => {
            model.send.phase = SendPhase::Pick;
            model.send.error = Some(crate::i18n::t(model.lang, "readFailed").to_string());
            model.send.source = None;
            haptic(HapticKind::Error)
        }
        _ => Command::done(),
    }
}

/// The screen or the density changed: same stream on a new grid.
fn regrid(model: &mut Model) -> Cmd {
    if model.send.phase != SendPhase::Ready {
        return Command::done();
    }
    let g = geometry::compute(&model.viewport, model.prefs.density);
    if Some(g) == model.send.geometry {
        return Command::done();
    }
    model.send.geometry = Some(g);
    engine(EngineOp::Resize { geometry: g }, Step::Adjusted, model.send.attempt)
}

/// Colours or ECC changed: keep the transfer when the packet size allows it.
fn reconfigure(model: &mut Model) -> Cmd {
    if model.send.phase != SendPhase::Ready {
        return Command::done();
    }
    let g = geometry::compute(&model.viewport, model.prefs.density);
    model.send.geometry = Some(g);
    engine(EngineOp::Reconfigure { mode: model.prefs.mode, ecc: model.prefs.ecc, geometry: g }, Step::Adjusted, model.send.attempt)
}

fn close_send(model: &mut Model) -> Cmd {
    let s = &mut model.send;
    s.attempt += 1;
    s.dock_attempt += 1;
    s.phase = SendPhase::Pick;
    s.source = None;
    s.info = None;
    s.geometry = None;
    s.playing = false;
    s.error = None;
    engine_notify(EngineOp::StopSender)
}

fn show_dock(model: &mut Model) -> Cmd {
    model.send.dock_visible = true;
    poke_dock(model)
}

fn poke_dock(model: &mut Model) -> Cmd {
    if !model.send.dock_visible || model.send.phase != SendPhase::Ready {
        return Command::done();
    }
    model.send.dock_attempt += 1;
    timer(TimerKind::Dock, model.send.dock_attempt, DOCK_IDLE_MS)
}

// ---------------------------------------------------------------------- receive

fn start_camera(model: &mut Model) -> Cmd {
    let cam = &mut model.recv.camera;
    cam.attempt += 1;
    cam.starting = true;
    cam.error = None;
    let attempt = cam.attempt;
    Command::request_from_shell(ShellOp::CameraStart { device: cam.device }).then_send(move |out| Event::CameraAnswer { attempt, out })
}

fn camera_answer(model: &mut Model, attempt: u64, out: ShellOutput) -> Cmd {
    if attempt != model.recv.camera.attempt || model.screen != Screen::Receive {
        // the person left (or asked again) while the camera was opening
        if let ShellOutput::Camera { ok: true, .. } = out {
            if !model.recv.camera.on {
                return notify(ShellOp::CameraStop);
            }
        }
        return Command::done();
    }
    let cam = &mut model.recv.camera;
    cam.starting = false;
    match out {
        ShellOutput::Camera { ok: true, zoom_min, zoom_max, zoom, devices, max_fps, uhd, .. } => {
            cam.on = true;
            cam.fps = 30;
            cam.max_fps = max_fps;
            cam.uhd = false;
            cam.uhd_ok = uhd;
            cam.mode_at = model.now;
            cam.exposure_us = 0;
            cam.zoom_min = zoom_min;
            cam.zoom_max = zoom_max.max(zoom_min);
            // a restarted camera keeps the level the person chose
            cam.zoom = if cam.zoom > 0.0 { cam.zoom.clamp(cam.zoom_min, cam.zoom_max) } else { zoom };
            cam.devices = devices.max(1);
            cam.device %= cam.devices;
            let z = cam.zoom;
            let rezoom = (z - zoom).abs() > 0.01;
            model.recv.phase = RecvPhase::Scanning;
            model.recv.from_image = false;
            let mut cmds = vec![engine_notify(EngineOp::RecvActive { on: true }), start_tick(model)];
            if rezoom {
                cmds.push(notify(ShellOp::CameraZoom { level: z }));
            }
            Command::all(cmds)
        }
        ShellOutput::Camera { error, message, .. } => {
            cam.on = false;
            let kind = error.unwrap_or(CameraError::Failed);
            cam.error = Some((kind, message.unwrap_or_default()));
            if model.recv.phase == RecvPhase::Scanning && !model.recv.from_image {
                model.recv.phase = RecvPhase::Start;
            }
            haptic(HapticKind::Error)
        }
        _ => Command::done(),
    }
}

fn start_tick(model: &mut Model) -> Cmd {
    if model.recv.ticking {
        return Command::done();
    }
    model.recv.ticking = true;
    model.recv.tick_attempt += 1;
    timer(TimerKind::Tick, model.recv.tick_attempt, TICK_MS)
}

fn scanned(model: &mut Model, report: ScanReport) -> Cmd {
    if report.generation != model.recv.generation || model.recv.phase == RecvPhase::Done {
        return Command::done();
    }
    model.now = model.now.max(report.at);
    let r = &mut model.recv;
    let mut cmd = Command::done();
    // The capture mode follows the code. Frame rate: each sender frame needs about two
    // captures, so the camera runs at the lowest of 30/60/120/240 fps that gives that (a fast
    // mode also reads the sensor out faster: fewer torn frames). 4K for small cells (4:2:0
    // colour at 1080p gives a 5-pixel cell 2.5 colour pixels and neighbours bleed; at 5 px it
    // gains nothing measurable and only costs power, so only genuinely tiny cells qualify).
    // Wide hysteresis (the estimate of a 12 fps sender jitters around 12–14), and two seconds
    // for a new mode to settle.
    let sender = report.stats.sender_fps;
    let hot = model.thermal >= 2;
    let cam = &mut r.camera;
    if cam.on && report.at - cam.mode_at > 2000.0 {
        let mut fps = cam.fps;
        // a hot phone gives up the fast modes before iOS/Android throttle it mid-transfer
        let max_fps = if hot { cam.max_fps.min(60) } else { cam.max_fps };
        if sender > 0.0 {
            const BANDS: [(u32, f32, f32); 4] = [(30, 0.0, 13.0), (60, 16.0, 27.0), (120, 32.0, 54.0), (240, 64.0, f32::MAX)];
            let cur = BANDS.iter().position(|b| b.0 == fps).unwrap_or(0);
            let mut band = cur;
            while band + 1 < BANDS.len() && sender >= BANDS[band + 1].1 && BANDS[band + 1].0 <= max_fps {
                band += 1;
            }
            while band > 0 && (sender < BANDS[band].1 - 3.0 || BANDS[band].0 > max_fps) {
                band -= 1;
            }
            fps = BANDS[band].0;
        }
        let mut uhd = cam.uhd;
        let cell = report.stats.cell as f64;
        if report.stats.cell_n >= 3 && report.ok {
            if !uhd && cam.uhd_ok && cell < 4.2 {
                uhd = true;
            } else if uhd && cell > 16.0 {
                uhd = false;
            }
        }
        // a fast sender needs captures more than resolution (4K stops at 60 fps on most phones)
        if fps > 60 {
            uhd = false;
        }
        if (fps, uhd) != (cam.fps, cam.uhd) {
            cam.fps = fps;
            cam.uhd = uhd;
            cam.mode_at = report.at;
            cmd = notify(ShellOp::CameraMode { fps, uhd });
        }
    }
    // exposure: as long as a sixth of the sender's frame allows (less noise), in steps
    if cam.on && sender > 0.0 && report.stats.span_secs > 1.0 {
        let fit = 1_000_000.0 / sender as f64 / 6.0;
        let max_us = [8000u32, 4000, 2000].into_iter().find(|&b| b as f64 <= fit).unwrap_or(1000);
        if max_us != cam.exposure_us {
            cam.exposure_us = max_us;
            cmd = Command::all([cmd, notify(ShellOp::CameraExposure { max_us })]);
        }
    }
    if report.ok {
        // a code came into view after a while without one
        if !r.found && report.at - r.lost_at > 1500.0 {
            cmd = Command::all([cmd, haptic(HapticKind::Light)]);
        }
        r.found = true;
    } else if r.found && report.at - report.stats.last_found_at > FOUND_MS {
        r.found = false;
        r.lost_at = report.at;
    }
    r.last = Some(report);
    cmd
}

fn progress(model: &mut Model, fact: ProgressFact, at: f64) -> Cmd {
    if fact.generation != model.recv.generation || model.recv.phase == RecvPhase::Done {
        return Command::done();
    }
    model.now = model.now.max(at);
    let r = &mut model.recv;
    if r.active_tid != Some(fact.tid) {
        r.active_tid = Some(fact.tid);
        r.prog_hist.clear();
        r.first_packet_at = at;
        r.milestone = 0;
    }
    if r.first_packet_at <= 0.0 {
        r.first_packet_at = at;
    }
    r.prog_hist.push_back((at, fact.received));
    while r.prog_hist.len() > 2 && at - r.prog_hist[0].0 > 3000.0 {
        r.prog_hist.pop_front();
    }
    let quarter = match fact.needed {
        Some(n) if n > 0 => ((4 * fact.received as u64) / n as u64).min(3) as u8,
        _ => 0,
    };
    r.progress = Some(fact);
    if quarter > r.milestone {
        r.milestone = quarter;
        return haptic(HapticKind::Selection);
    }
    Command::done()
}

fn received(model: &mut Model, generation: u64, name: String, size: u64, classified: Classified, at: f64) -> Cmd {
    if generation != model.recv.generation || model.recv.phase == RecvPhase::Done {
        return Command::done();
    }
    model.now = model.now.max(at);
    let r = &mut model.recv;
    let secs = if r.first_packet_at > 0.0 { (at - r.first_packet_at) / 1000.0 } else { 0.0 };
    let (received, t) = r.progress.as_ref().map_or((0, 0), |p| (p.received, p.t));
    r.done = Some(DoneState {
        name,
        size,
        secs,
        classified,
        stats: r.last.clone(),
        received,
        t,
        copied: false,
        copied_attempt: 0,
        note: None,
        content_rev: 1,
    });
    r.phase = RecvPhase::Done;
    r.ticking = false;
    r.tick_attempt += 1;
    // the camera is not needed while the result is on screen: power, heat
    let cam = &mut r.camera;
    let was_on = cam.on || cam.starting;
    cam.resume = was_on;
    cam.on = false;
    cam.starting = false;
    cam.attempt += 1;
    let mut cmds = vec![haptic(HapticKind::Success), engine_notify(EngineOp::RecvActive { on: false })];
    if was_on {
        cmds.push(notify(ShellOp::CameraStop));
    }
    Command::all(cmds)
}

fn receive_another(model: &mut Model) -> Cmd {
    let generation = model.recv.generation + 1;
    let resume = model.recv.camera.resume;
    let mut camera = std::mem::take(&mut model.recv.camera);
    camera.resume = false;
    model.recv = RecvState { generation, camera, ..RecvState::default() };
    let mut cmds = vec![engine_notify(EngineOp::ResetRecv { generation })];
    if resume {
        // same camera as before; scanning starts again when it answers
        cmds.push(start_camera(model));
    }
    Command::all(cmds)
}

fn finished(model: &mut Model, action: FileAction, out: ShellOutput) -> Cmd {
    let ShellOutput::Finished { ok, cancelled } = out else { return Command::done() };
    match action {
        FileAction::Copy if ok => {
            if let Some(d) = &mut model.recv.done {
                d.copied = true;
                d.copied_attempt += 1;
                let attempt = d.copied_attempt;
                return Command::all([haptic(HapticKind::Success), timer(TimerKind::Copied, attempt, 1500)]);
            }
            Command::done()
        }
        FileAction::Save => {
            if let Some(d) = &mut model.recv.done {
                if cancelled {
                    d.note = None;
                    return Command::done();
                }
                d.note = Some((ok, if ok { "saved" } else { "saveFailed" }));
                return haptic(if ok { HapticKind::Success } else { HapticKind::Error });
            }
            Command::done()
        }
        FileAction::Pick if !ok && !cancelled => {
            model.send.error = Some(crate::i18n::t(model.lang, "readFailed").to_string());
            haptic(HapticKind::Error)
        }
        _ => Command::done(),
    }
}

fn bump_content(model: &mut Model) {
    if let Some(d) = &mut model.recv.done {
        d.content_rev += 1;
    }
}

fn stats_open(model: &Model) -> bool {
    model.prefs.reader.stats.unwrap_or(model.viewport.width_px / model.viewport.scale.max(1.0) >= 720.0)
}

#[cfg(test)]
mod tests;
