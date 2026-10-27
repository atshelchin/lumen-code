//! The root view: runs the core, shows its view model and performs what it asks for.

use crate::camera::Camera;
use crate::text_area::TextArea;
use crate::theme::{c, BG};
use crate::{home, platform, recv, send, ToggleFullScreen};
use futures::StreamExt;
use gpui::{
    div, prelude::*, ClipboardItem, Context, Entity, ExternalPaths, FocusHandle, KeyDownEvent, MouseMoveEvent, PathPromptOptions,
    RenderImage, Window,
};
use lumen_app::app::{CameraError, Event, HapticKind, Platform, Screen, ShellOp, ShellOutput};
use lumen_app::engine::send::{CellFrame, StaticLayout};
use lumen_app::i18n::{labels, Lang};
use lumen_app::runtime::{ReaderContent, Runtime, ShellRequest, Update};
use lumen_app::{FrameRef, ViewModel};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct Root {
    pub rt: Runtime,
    pub view: ViewModel,
    labels: BTreeMap<&'static str, &'static str>,
    lang: Option<Lang>,
    pub layout: Option<Arc<StaticLayout>>,
    pub frame: Option<Arc<CellFrame>>,
    frame_rev: u64,
    frame_gen: u64,
    playing: bool,
    pub text_area: Entity<TextArea>,
    pub focus: FocusHandle,
    pub camera: Option<Camera>,
    pub preview: Option<Arc<RenderImage>>,
    preview_gen: u64,
    pub reader: Option<(u64, bool, Arc<ReaderContent>)>,
    pub image: Option<(u64, Arc<RenderImage>)>,
    awake: platform::KeepAwake,
    reported: (f32, f32, f32),
    dock_poke: Instant,
    /// the bottom edge shows a hidden dock only after the pointer has been above the dock's area
    edge_armed: bool,
    pub fps_slider: crate::theme::SliderState,
    /// Test runs (LUMEN_SCRIPT): frames shown per second, logged.
    flips: (Instant, u32),
}

impl Root {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Update>();
        let tx = std::sync::Mutex::new(tx);
        let rt = Runtime::new(move |u: Update| {
            if let Ok(tx) = tx.lock() {
                let _ = tx.unbounded_send(u);
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            while let Some(u) = rx.next().await {
                if this.update_in(cx, |root, window, cx| root.apply(u, window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let rt2 = rt.clone();
        let text_area = cx.new(|cx| TextArea::new(cx, move |text| rt2.dispatch(Event::TextChanged { text })));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        rt.dispatch(Event::Start { locale: platform::locale(), platform: Platform::Desktop });
        if let Ok(script) = std::env::var("LUMEN_SCRIPT") {
            cx.spawn_in(window, async move |this, cx| crate::script::run(this, cx, script).await).detach();
        }
        let view = rt.view();
        Root {
            rt,
            view,
            labels: BTreeMap::new(),
            lang: None,
            layout: None,
            frame: None,
            frame_rev: 0,
            frame_gen: 0,
            playing: false,
            text_area,
            focus,
            camera: None,
            preview: None,
            preview_gen: 0,
            reader: None,
            image: None,
            awake: platform::KeepAwake::default(),
            reported: (0., 0., 0.),
            dock_poke: Instant::now(),
            edge_armed: false,
            fps_slider: Default::default(),
            flips: (Instant::now(), 0),
        }
    }

    pub fn t(&self, key: &str) -> &'static str {
        self.labels.get(key).copied().unwrap_or_else(|| lumen_app::i18n::t(self.view.lang, key))
    }

    pub fn dispatch(&self, ev: Event) {
        self.rt.dispatch(ev);
    }

    fn apply(&mut self, u: Update, window: &mut Window, cx: &mut Context<Self>) {
        if u.view.rev >= self.view.rev {
            if self.lang != Some(u.view.lang) {
                self.lang = Some(u.view.lang);
                self.labels = labels(u.view.lang);
            }
            let s = &u.view.send;
            if s.frame_rev != self.frame_rev {
                self.frame_rev = s.frame_rev;
                self.layout = self.rt.send_layout().map(Arc::new);
                self.frame = self.rt.send_frame().map(Arc::new);
            }
            if s.geometry.is_none() {
                self.layout = None;
                self.frame = None;
            }
            // The text box shows the core's text. Its own edits reach the core at once, so the
            // core differs from the box only after a change made elsewhere (a script); updates
            // still queued from earlier keystrokes are stale, hence the core's current text.
            if s.text != self.view.send.text && s.text != self.text_area.read(cx).content {
                let text = self.rt.view().send.text;
                self.text_area.update(cx, |t, cx| t.set_text(text, cx));
            }
            let playing = s.playing && s.geometry.is_some();
            if playing != self.playing {
                self.playing = playing;
                if playing {
                    self.start_frames(window, cx);
                } else {
                    self.frame_gen += 1;
                }
            }
            self.view = u.view;
        }
        for r in u.requests {
            self.perform(r, window, cx);
        }
        cx.notify();
    }

    /// Flips frames at the sender rate (the core's `send.fps`), keeping the cadence.
    fn start_frames(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.frame_gen += 1;
        let generation = self.frame_gen;
        cx.spawn_in(window, async move |this, cx| {
            let mut next = Instant::now();
            loop {
                let wait = next.saturating_duration_since(Instant::now());
                cx.background_executor().timer(wait).await;
                let fps = this
                    .update(cx, |root, cx| {
                        if root.frame_gen != generation {
                            return None;
                        }
                        // A timer and a plain redraw request, at every rate: GPUI throttles
                        // animation-frame callbacks to 30 fps whenever the window is not
                        // focused (the usual case while a phone is held up to it), plain
                        // redraws it does not; and nothing is redrawn between frames.
                        root.frame = root.rt.send_frame().map(Arc::new);
                        root.count_flip();
                        cx.notify();
                        Some(root.view.send.fps.max(1))
                    })
                    .ok()
                    .flatten();
                let Some(fps) = fps else { break };
                next += Duration::from_secs_f64(1.0 / fps as f64);
                let now = Instant::now();
                if next < now {
                    next = now;
                }
            }
        })
        .detach();
    }

    fn count_flip(&mut self) {
        if std::env::var_os("LUMEN_SCRIPT").is_none() {
            return;
        }
        self.flips.1 += 1;
        if self.flips.0.elapsed() >= Duration::from_secs(1) {
            eprintln!("[lumen] {} frames/s (fps setting {})", self.flips.1, self.view.send.fps);
            self.flips = (Instant::now(), 0);
        }
    }

    fn answer(&self, id: u64, out: ShellOutput) {
        if id != 0 {
            self.rt.resolve(id, out);
        }
    }

    fn perform(&mut self, r: ShellRequest, window: &mut Window, cx: &mut Context<Self>) {
        let id = r.id;
        let rt = self.rt.clone();
        match r.op {
            ShellOp::LoadPrefs => self.answer(id, ShellOutput::Prefs { json: platform::load_prefs() }),
            ShellOp::SavePrefs { json } => platform::save_prefs(&json),
            ShellOp::KeepAwake { on } => self.awake.set(on),
            ShellOp::Brightness { .. } | ShellOp::Haptic { .. } => {}
            ShellOp::Immersive { .. } => {}
            ShellOp::CameraStart { device } => {
                self.camera = None;
                let (camera, ready) = Camera::start(device as usize, self.rt.clone());
                self.camera = Some(camera);
                self.preview_gen += 1;
                let generation = self.preview_gen;
                cx.spawn_in(window, async move |this, cx| {
                    let answer = ready.await.unwrap_or(Err((CameraError::Failed, "camera thread ended".into())));
                    let ok = answer.is_ok();
                    let out = match answer {
                        // the webcam already runs at its fastest mode: report it as the most it offers
                        Ok((devices, fps)) => ShellOutput::Camera { ok: true, error: None, message: None, zoom_min: 1.0, zoom_max: 1.0, zoom: 1.0, devices, max_fps: fps.max(1), uhd: false },
                        Err((error, message)) => ShellOutput::Camera { ok: false, error: Some(error), message: Some(message), zoom_min: 1.0, zoom_max: 1.0, zoom: 1.0, devices: 0, max_fps: 30, uhd: false },
                    };
                    let _ = this.update(cx, |root, _| root.answer(id, out));
                    if !ok {
                        return;
                    }
                    // the preview: the newest frame, ~30 times a second
                    loop {
                        cx.background_executor().timer(Duration::from_millis(33)).await;
                        let alive = this
                            .update_in(cx, |root, window, cx| {
                                if root.preview_gen != generation {
                                    return false;
                                }
                                let Some(frame) = root.camera.as_ref().and_then(|c| c.take_preview()) else { return root.camera.is_some() };
                                let image = Arc::new(frame);
                                if let Some(old) = root.preview.replace(image) {
                                    let _ = window.drop_image(old);
                                }
                                cx.notify();
                                true
                            })
                            .unwrap_or(false);
                        if !alive {
                            break;
                        }
                    }
                })
                .detach();
            }
            ShellOp::CameraStop => {
                self.camera = None;
                self.preview_gen += 1;
                if let Some(old) = self.preview.take() {
                    let _ = window.drop_image(old);
                }
            }
            ShellOp::CameraZoom { .. } | ShellOp::CameraMode { .. } | ShellOp::CameraExposure { .. } => {}
            ShellOp::PickFile => {
                let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
                cx.spawn(async move |_, _| {
                    let picked = paths.await.ok().and_then(|r| r.ok()).flatten().and_then(|p| p.into_iter().next());
                    let out = match picked {
                        Some(path) => {
                            let ok = load_path(&rt, &path);
                            ShellOutput::Finished { ok, cancelled: false }
                        }
                        None => ShellOutput::Finished { ok: false, cancelled: true },
                    };
                    if id != 0 {
                        rt.resolve(id, out);
                    }
                })
                .detach();
            }
            ShellOp::PickImage => {
                let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
                cx.spawn(async move |_, cx| {
                    let picked = paths.await.ok().and_then(|r| r.ok()).flatten().and_then(|p| p.into_iter().next());
                    let out = match picked {
                        Some(path) => {
                            let rt2 = rt.clone();
                            let ok = cx
                                .background_executor()
                                .spawn(async move {
                                    let Ok(img) = image::open(&path) else { return false };
                                    let img = if img.width().max(img.height()) > 2400 { img.resize(2400, 2400, image::imageops::FilterType::Triangle) } else { img };
                                    let rgba = img.to_rgba8();
                                    rt2.scan_image(&FrameRef::rgba(rgba.width() as usize, rgba.height() as usize, rgba.as_raw()));
                                    true
                                })
                                .await;
                            ShellOutput::Finished { ok, cancelled: false }
                        }
                        None => ShellOutput::Finished { ok: false, cancelled: true },
                    };
                    rt.resolve(id, out);
                })
                .detach();
            }
            ShellOp::SaveFile { name, .. } | ShellOp::ShareFile { name, .. } => {
                let Some(bytes) = self.rt.received_bytes() else { return self.answer(id, ShellOutput::Finished { ok: false, cancelled: false }) };
                let dir = platform::downloads_dir();
                let path = cx.prompt_for_new_path(&dir, Some(&name));
                cx.spawn(async move |_, _| {
                    let out = match path.await.ok().and_then(|r| r.ok()).flatten() {
                        Some(p) => ShellOutput::Finished { ok: std::fs::write(&p, bytes.as_slice()).is_ok(), cancelled: false },
                        None => ShellOutput::Finished { ok: false, cancelled: true },
                    };
                    rt.resolve(id, out);
                })
                .detach();
            }
            ShellOp::OpenFile { name, .. } => {
                let ok = match (self.rt.received_bytes(), platform::temp_file(&name)) {
                    (Some(bytes), Some(path)) if std::fs::write(&path, bytes.as_slice()).is_ok() => {
                        cx.open_with_system(&path);
                        true
                    }
                    _ => false,
                };
                self.answer(id, ShellOutput::Finished { ok, cancelled: false });
            }
            ShellOp::CopyText { pretty } => {
                let text = self.rt.copy_text(pretty);
                if let Some(t) = &text {
                    cx.write_to_clipboard(ClipboardItem::new_string(t.clone()));
                }
                self.answer(id, ShellOutput::Finished { ok: text.is_some(), cancelled: false });
            }
            ShellOp::OpenUrl { url } => cx.open_url(&url),
            ShellOp::OpenSettings => {
                // the system's camera privacy page (Linux desktops have no common one)
                #[cfg(target_os = "macos")]
                cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Camera");
                #[cfg(target_os = "windows")]
                cx.open_url("ms-settings:privacy-webcam");
            }
        }
        let _ = HapticKind::Light;
    }

    /// Reader content for the current JSON mode, cached per content revision.
    pub fn reader_content(&mut self, rev: u64, pretty: bool) -> Option<Arc<ReaderContent>> {
        if let Some((r, p, c)) = &self.reader {
            if *r == rev && *p == pretty {
                return Some(c.clone());
            }
        }
        let c = Arc::new(self.rt.reader(pretty)?);
        self.reader = Some((rev, pretty, c.clone()));
        Some(c)
    }

    /// The received image as a GPU image, decoded once.
    pub fn received_image(&mut self, rev: u64, window: &mut Window) -> Option<Arc<RenderImage>> {
        if let Some((r, img)) = &self.image {
            if *r == rev {
                return Some(img.clone());
            }
        }
        let bytes = self.rt.received_bytes()?;
        let img = image::load_from_memory(&bytes).ok()?;
        let img = if img.width().max(img.height()) > 4096 { img.resize(4096, 4096, image::imageops::FilterType::Triangle) } else { img };
        let image = Arc::new(platform::render_image(img.to_rgba8()));
        if let Some((_, old)) = self.image.replace((rev, image.clone())) {
            let _ = window.drop_image(old);
        }
        Some(image)
    }

    /// The space the code may use, in device pixels (titlebar strip excluded).
    fn report_viewport(&mut self, window: &mut Window) {
        let scale = window.scale_factor();
        let size = window.viewport_size();
        let top = crate::theme::title_inset(window);
        let (w, h) = (f32::from(size.width), f32::from(size.height) - top);
        if (w, h, scale) != self.reported {
            self.reported = (w, h, scale);
            // ~0.25 mm per point: a typical laptop or desktop monitor
            if std::env::var("LUMEN_SCRIPT").is_ok() {
                eprintln!("[lumen] viewport {w}x{h} pt @{scale}");
            }
            self.dispatch(Event::Viewport { width_px: (w * scale) as f64, height_px: (h * scale) as f64, scale: scale as f64, mm_per_px: 0.25 / scale as f64 });
        }
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.text_area.read(cx).focus_handle.is_focused(window) {
            return;
        }
        let key = ev.keystroke.key.as_str();
        let send = &self.view.send;
        if self.view.screen == Screen::Send && send.flash_ask {
            // the warning card: Enter or Space plays, Esc closes; nothing else
            match key {
                "enter" | "space" => self.dispatch(Event::AcceptFlash),
                "escape" => self.dispatch(Event::CloseSend),
                _ => {}
            }
        } else if self.view.screen == Screen::Send && send.geometry.is_some() {
            match key {
                "space" => self.dispatch(Event::TogglePlay),
                "f" => window.toggle_fullscreen(),
                "escape" => self.dispatch(Event::HideDock),
                "h" => self.dispatch(if send.dock_visible { Event::HideDock } else { Event::ShowDock }),
                "right" | "up" | "left" | "down" => {
                    let step = if ev.keystroke.modifiers.shift { 5 } else { 1 };
                    let fps = send.fps as i32 + if matches!(key, "right" | "up") { step } else { -step };
                    self.dispatch(Event::SetFps { value: fps.clamp(1, 120) as u32 });
                    self.dispatch(Event::ShowDock);
                }
                _ => self.dispatch(Event::ShowDock),
            }
        } else if key == "escape" && self.view.screen != Screen::Home {
            self.dispatch(Event::Navigate { screen: Screen::Home });
        }
    }

    fn on_mouse_move(&mut self, ev: &MouseMoveEvent, window: &mut Window) {
        let send = &self.view.send;
        if self.view.screen != Screen::Send || send.geometry.is_none() {
            return;
        }
        let h = f32::from(window.viewport_size().height);
        let y = f32::from(ev.position.y);
        // right after the hide button the pointer is still over the dock's area: stay hidden
        if send.dock_visible {
            self.edge_armed = false;
        } else if y < h - 220.0 {
            self.edge_armed = true;
        }
        if !send.dock_visible && y > h - 90.0 && self.edge_armed {
            self.dispatch(Event::ShowDock);
        } else if send.dock_visible && y > h - 220.0 && self.dock_poke.elapsed() > Duration::from_millis(500) {
            self.dock_poke = Instant::now();
            self.dispatch(Event::DockActivity);
        }
    }
}

fn load_path(rt: &Runtime, path: &std::path::Path) -> bool {
    match std::fs::read(path) {
        Ok(bytes) => {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
            rt.load_file(&name, bytes);
            true
        }
        Err(_) => false,
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.report_viewport(window);
        let screen = match self.view.screen {
            Screen::Home => home::render(self, window, cx).into_any_element(),
            Screen::Send => send::render(self, window, cx).into_any_element(),
            Screen::Receive => recv::render(self, window, cx).into_any_element(),
        };
        div()
            .id("root")
            .size_full()
            .bg(c(BG))
            .text_color(c(crate::theme::TEXT))
            .font_family(".SystemUIFont")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| this.on_key(ev, window, cx)))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, _| this.on_mouse_move(ev, window)))
            .on_action(cx.listener(|_, _: &ToggleFullScreen, window, _| window.toggle_fullscreen()))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, _| {
                if let Some(p) = paths.paths().first() {
                    load_path(&this.rt, p);
                }
            }))
            .child(screen)
    }
}
