//! The core through the runtime, as a shell sees it: events in, updates out.

use super::*;
use crate::engine::image::FrameRef;
use crate::runtime::{Runtime, Update};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

struct Harness {
    rt: Runtime,
    rx: Receiver<Update>,
    last: ViewModel,
    ops: Vec<ShellOp>,
}

impl Harness {
    fn new(platform: Platform) -> Harness {
        let (tx, rx) = channel();
        let tx = std::sync::Mutex::new(tx);
        let rt = Runtime::new(move |u: Update| {
            let _ = tx.lock().unwrap().send(u);
        });
        let mut h = Harness { rt, rx, last: ViewModel::default(), ops: Vec::new() };
        h.send(Event::Start { locale: "zh-CN".into(), platform });
        h
    }

    fn send(&mut self, ev: Event) {
        self.rt.dispatch(ev);
        self.drain(Duration::from_millis(0));
    }

    /// Applies updates, answering the requests a device would answer.
    fn drain(&mut self, wait: Duration) {
        let until = Instant::now() + wait;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let Ok(u) = (if left.is_zero() { self.rx.try_recv().map_err(|_| ()) } else { self.rx.recv_timeout(left).map_err(|_| ()) }) else {
                return;
            };
            if u.view.rev >= self.last.rev {
                self.last = u.view;
            }
            for r in u.requests {
                self.ops.push(r.op.clone());
                let answer = match r.op {
                    ShellOp::LoadPrefs => Some(ShellOutput::Prefs { json: None }),
                    ShellOp::CameraStart { .. } => Some(ShellOutput::Camera {
                        ok: true,
                        error: None,
                        message: None,
                        zoom_min: 1.0,
                        zoom_max: 6.0,
                        zoom: 1.0,
                        devices: 2,
                        max_fps: 60,
                        uhd: true,
                    }),
                    ShellOp::CopyText { .. } => Some(ShellOutput::Finished { ok: true, cancelled: false }),
                    _ => None,
                };
                if let (Some(a), true) = (answer, r.id != 0) {
                    self.rt.resolve(r.id, a);
                }
            }
        }
    }

    fn wait_for(&mut self, what: &str, f: impl Fn(&ViewModel) -> bool) {
        let until = Instant::now() + Duration::from_secs(20);
        while !f(&self.last) {
            assert!(Instant::now() < until, "timed out waiting for {what}");
            self.drain(Duration::from_millis(20));
        }
    }
}

fn phone() -> Event {
    Event::Viewport { width_px: 1080.0, height_px: 2200.0, scale: 2.75, mm_per_px: 0.0577 }
}

#[test]
fn sending_a_file_shows_a_code_and_keeps_the_screen_on() {
    let mut h = Harness::new(Platform::Android);
    assert_eq!(h.last.lang, Lang::Zh);
    h.send(Event::Navigate { screen: Screen::Send });
    h.send(phone());
    let data: Vec<u8> = (0..30_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    h.rt.load_file("photo.bin", data);
    h.wait_for("sender ready", |v| v.send.phase == SendPhase::Ready);
    let v = &h.last.send;
    let g = v.geometry.expect("geometry");
    assert!(g.y > g.x, "portrait code on a portrait phone");
    // the first code waits, paused, for the photosensitivity warning to be read
    assert!(!v.playing && v.flash_ask && v.dock_visible);
    h.send(Event::AcceptFlash);
    assert!(h.last.send.playing && !h.last.send.flash_ask);
    assert!(h.ops.iter().any(|op| matches!(op, ShellOp::SavePrefs { json } if json.contains("\"flashOk\":true"))));
    let v = &h.last.send;
    assert_eq!(v.title, "photo.bin");
    assert_eq!(v.stats.len(), 5);
    assert!(h.ops.contains(&ShellOp::KeepAwake { on: true }));
    assert!(h.ops.contains(&ShellOp::Brightness { boost: true }));
    assert!(h.ops.contains(&ShellOp::Haptic { kind: HapticKind::Medium }));

    let f = h.rt.send_frame().expect("frame");
    assert_eq!((f.width as usize, f.height as usize), (g.x + 4, g.y + 4));
    let (mw, mh, mask) = h.rt.send_mask().unwrap();
    assert_eq!((mw as usize, mh as usize), (g.width_px, g.height_px));
    assert_eq!(mask.len(), g.width_px * g.height_px * 4);

    // the control bar is permanent (the code is laid out above it); settings start closed
    assert!(!h.last.send.settings_open);
    h.drain(Duration::from_millis(DOCK_IDLE_MS as u64 + 400));
    assert!(h.last.send.dock_visible, "no auto-hide");
    h.send(Event::TapCode);
    assert!(h.last.send.dock_visible, "tapping the code does not hide it");
    // the bar's line: time per pass while playing, "paused" when paused; details for settings
    assert!(h.last.send.subtitle.contains('≈'), "{}", h.last.send.subtitle);
    assert!(h.last.send.details.contains('×'), "{}", h.last.send.details);
    h.send(Event::TogglePlay);
    assert!(!h.last.send.playing && h.last.send.dock_visible);
    assert_eq!(h.last.send.subtitle, crate::i18n::t(Lang::Zh, "paused"));
    h.send(Event::TogglePlay);

    // turning the phone keeps the stream, with a landscape grid
    let rev = h.last.send.frame_rev;
    h.send(Event::Viewport { width_px: 2200.0, height_px: 1080.0, scale: 2.75, mm_per_px: 0.0577 });
    h.wait_for("regrid", |v| v.send.frame_rev > rev);
    let g2 = h.last.send.geometry.unwrap();
    assert!(g2.x > g2.y);

    h.send(Event::Navigate { screen: Screen::Home });
    assert_eq!(h.last.send.phase, SendPhase::Pick);
    assert!(h.ops.contains(&ShellOp::KeepAwake { on: false }));
    assert!(h.rt.send_frame().is_none());
}

#[test]
fn text_round_trip_through_rendered_frames() {
    // sender
    let mut tx = Harness::new(Platform::Desktop);
    tx.send(Event::Navigate { screen: Screen::Send });
    tx.send(Event::Viewport { width_px: 1400.0, height_px: 900.0, scale: 1.0, mm_per_px: 0.25 });
    let text = r#"{"hello":"世界","n":[1,2,3],"url":"https://lumen.example/x"}"#;
    tx.send(Event::SourceTab { tab: SourceTab::Text });
    tx.send(Event::TextChanged { text: text.into() });
    assert!(tx.last.send.can_send_text);
    tx.send(Event::SendText);
    tx.wait_for("sender ready", |v| v.send.phase == SendPhase::Ready);
    // the dock names a text, never shows it (the screen is filmed); the field keeps it
    assert_eq!(tx.last.send.title, "文本");
    assert_eq!(tx.last.send.text, text);
    tx.send(Event::AcceptFlash);
    assert!(tx.last.send.playing);
    // a second code plays at once: the warning was read
    tx.send(Event::CloseSend);
    assert_eq!(tx.last.send.text, text, "closing keeps the text, and the field shows it");
    tx.send(Event::SendText);
    tx.wait_for("sender ready again", |v| v.send.phase == SendPhase::Ready);
    assert!(tx.last.send.playing && !tx.last.send.flash_ask);

    // receiver: frames rendered at 3 px per cell, decoded as stills
    let mut rx = Harness::new(Platform::Ios);
    rx.send(Event::Navigate { screen: Screen::Receive });
    for _ in 0..6 {
        let f = tx.rt.send_frame().unwrap();
        let rgba = upscale(&f, 3);
        rx.rt.scan_image(&FrameRef::rgba(f.width as usize * 3, f.height as usize * 3, &rgba));
        rx.drain(Duration::from_millis(30));
        if rx.last.recv.phase == RecvPhase::Done {
            break;
        }
    }
    rx.wait_for("received", |v| v.recv.phase == RecvPhase::Done);
    let d = rx.last.recv.done.clone().unwrap();
    assert_eq!(d.kind, Some(Kind::Json));
    assert_eq!(d.title, "收到 JSON");
    assert!(d.reader && d.json && d.pretty);
    assert_eq!(rx.rt.copy_text(false).as_deref(), Some(text));
    let reader = rx.rt.reader(true).unwrap();
    assert!(reader.lines.unwrap().len() > 3);
    assert!(rx.ops.contains(&ShellOp::Haptic { kind: HapticKind::Success }));

    rx.send(Event::Copy);
    assert_eq!(rx.last.recv.done.as_ref().unwrap().copy_label.as_deref(), Some("已复制"));
    rx.wait_for("copy label back", |v| v.recv.done.as_ref().unwrap().copy_label.as_deref() == Some("复制"));

    rx.send(Event::ReceiveAnother);
    assert_eq!(rx.last.recv.phase, RecvPhase::Start);
    assert!(rx.rt.received_bytes().is_none());
}

#[test]
fn camera_flow_and_zoom() {
    let mut h = Harness::new(Platform::Ios);
    h.send(Event::Navigate { screen: Screen::Receive });
    h.send(Event::OpenCamera);
    assert!(h.last.recv.camera_on);
    assert_eq!(h.last.recv.phase, RecvPhase::Scanning);
    assert!(h.ops.contains(&ShellOp::KeepAwake { on: true }));
    h.send(Event::SetZoom { level: 9.0 });
    let z = h.last.recv.zoom.clone().unwrap();
    assert_eq!((z.level, z.active), (6.0, true));
    assert!(h.last.recv.can_switch);
    // backgrounding releases the camera and returning reopens it
    h.send(Event::AppActive { active: false });
    assert!(!h.last.recv.camera_on);
    assert!(h.ops.contains(&ShellOp::CameraStop));
    h.send(Event::AppActive { active: true });
    assert!(h.last.recv.camera_on);
    assert_eq!(h.last.recv.zoom.as_ref().unwrap().level, 6.0, "the zoom level survives a restart");
    h.send(Event::Navigate { screen: Screen::Home });
    assert!(!h.last.recv.camera_on);
    assert_eq!(h.ops.iter().filter(|o| **o == ShellOp::CameraStop).count(), 2);
}

#[test]
fn labels_used_by_the_view_exist() {
    for lang in [Lang::Zh, Lang::En] {
        let labels = crate::i18n::labels(lang);
        for k in ["dropTitle", "tabFile", "tabText", "textPh", "choose", "sendText", "openCam", "fromImage", "again", "share", "density", "colors", "ecc", "speed"] {
            assert!(labels.contains_key(k), "{k}");
        }
    }
}

/// Nearest-neighbour upscale of a one-pixel-per-cell frame (what a shell's GPU does).
fn upscale(f: &crate::engine::send::CellFrame, s: usize) -> Vec<u8> {
    let (w, h) = (f.width as usize, f.height as usize);
    let mut out = vec![0u8; w * s * h * s * 4];
    for y in 0..h * s {
        for x in 0..w * s {
            let src = ((y / s) * w + x / s) * 4;
            let dst = (y * w * s + x) * 4;
            out[dst..dst + 4].copy_from_slice(&f.rgba[src..src + 4]);
        }
    }
    out
}

#[test]
fn camera_frames_through_the_scanner_pool() {
    use crate::engine::bench::camera_image;
    use crate::engine::image::{PixelFormat, Plane};
    use crate::geometry::{compute, Density, Viewport, QUIET};
    use lumen::transfer::{prepare, Sender};
    use lumen::Renderer;

    let mut rx = Harness::new(Platform::Ios);
    rx.send(Event::Navigate { screen: Screen::Receive });
    rx.send(Event::OpenCamera);
    assert_eq!(rx.last.recv.phase, RecvPhase::Scanning);

    // a phone sender at 15 fps, filmed at 30 fps
    let g = compute(&Viewport { width_px: 1170.0, height_px: 2350.0, scale: 3.0, mm_per_px: 0.0533 }, Density::Auto);
    let data: Vec<u8> = (0..40_000u32).map(|i| (i.wrapping_mul(2246822519) >> 9) as u8).collect();
    let prepared = prepare(&data, "pool.bin", false);
    let mut tx = Sender::new(&prepared, 1, 1, g.x, g.y, 0x7001, 0).unwrap();
    let r = Renderer::new(&tx.layout, tx.mode, 7, QUIET);
    let (cw, ch) = (r.width, r.height);
    let mut code = vec![0u32; cw * ch];
    let (fw, fh) = (1920usize, 1080usize);
    let (mut y, mut uv) = (vec![0u8; fw * fh], vec![0u8; fw * fh / 2]);
    let start = Instant::now();
    let mut shown = u64::MAX;
    while rx.last.recv.phase != RecvPhase::Done {
        assert!(start.elapsed() < Duration::from_secs(20), "no file after 20 s: {:?}", rx.last.recv.diag);
        let t = start.elapsed().as_secs_f64();
        let sender_frame = (t * 15.0) as u64;
        if sender_frame != shown {
            shown = sender_frame;
            let f = tx.next_frame();
            r.draw(&f.cells, &mut code);
            camera_image(&code, cw, ch, &mut y, &mut uv, fw, fh);
        }
        let frame = FrameRef {
            width: fw,
            height: fh,
            format: PixelFormat::Yuv420,
            planes: [
                Plane { data: &y, row_stride: fw, pixel_stride: 1 },
                Plane { data: &uv, row_stride: fw, pixel_stride: 2 },
                Plane { data: &uv[1..], row_stride: fw, pixel_stride: 2 },
            ],
            rotation: 90,
        };
        rx.rt.camera_frame(&frame);
        rx.drain(Duration::from_millis(33));
    }
    let d = rx.last.recv.done.clone().unwrap();
    assert_eq!(d.name, "pool.bin");
    assert_eq!(rx.rt.received_bytes().unwrap().as_slice(), data.as_slice());
    println!("received in {:.1} s; {}", start.elapsed().as_secs_f64(), rx.last.recv.diag);
}

#[test]
fn camera_mode_follows_the_sender_and_the_heat() {
    use crate::engine::recv::{LiveStats, ScanReport};
    let mut h = Harness::new(Platform::Ios);
    h.send(Event::Navigate { screen: Screen::Receive });
    h.send(Event::OpenCamera);
    let mut at = 0.0;
    let mut report = |h: &mut Harness, sender: f32| {
        at += 2500.0;
        let stats = LiveStats { sender_fps: sender, last_found_at: at, cell: 6.0, cell_n: 5, ..Default::default() };
        h.send(Event::Scanned(Box::new(ScanReport { at, ok: true, stats, ..Default::default() })));
    };
    let modes = |h: &Harness| h.ops.iter().filter_map(|o| if let ShellOp::CameraMode { fps, .. } = o { Some(*fps) } else { None }).collect::<Vec<_>>();
    report(&mut h, 12.5);
    assert!(modes(&h).is_empty(), "a 12 fps sender keeps the camera at 30");
    report(&mut h, 20.0);
    assert_eq!(modes(&h), vec![60]);
    report(&mut h, 45.0);
    assert_eq!(modes(&h), vec![60], "the harness camera offers no more than 60");
    h.send(Event::Thermal { level: 2 });
    // a camera that never goes past 60 has given nothing up: no "held at 60" tip
    assert_ne!(h.last.recv.tip, crate::i18n::t(Lang::Zh, "tipHot"));
    report(&mut h, 8.0);
    assert_eq!(modes(&h), vec![60, 30]);
    // the engine turning frames away for the heat is what the warm tip reports
    at += 2500.0;
    let stats = LiveStats { sender_fps: 8.0, last_found_at: at, cell: 6.0, cell_n: 5, heat_limited: true, ..Default::default() };
    h.send(Event::Scanned(Box::new(ScanReport { at, ok: true, stats, ..Default::default() })));
    assert_eq!(h.last.recv.tip, crate::i18n::t(Lang::Zh, "tipHotScan"));
}

/// While data keeps arriving the receiver says so: a green outline and no warning, even when
/// each scan reads only part of a frame. The warnings come back once progress stalls.
#[test]
fn receive_feedback_follows_progress() {
    use crate::engine::recv::{LiveStats, ScanReport};
    use crate::engine::sink::ProgressFact;
    let now = 10_000.0;
    let mut m = Model { screen: Screen::Receive, now, ..Model::default() };
    m.recv.phase = RecvPhase::Scanning;
    let v = view::view(&m).recv;
    assert_eq!((v.name.as_str(), v.line.as_str()), (crate::i18n::t(Lang::En, "noData"), ""));
    let stats = LiveStats { last_found_at: now - 100.0, cov_ok: 20, cov_tot: 100, cell: 5.0, cell_n: 5, ..LiveStats::default() };
    m.recv.last = Some(ScanReport { ok: true, at: now - 100.0, corners: Some([[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]), frame_rate: 0.2, stats, ..ScanReport::default() });
    m.recv.progress = Some(ProgressFact { received: 40, needed: Some(100), t: 200, ..ProgressFact::default() });

    m.recv.prog_hist = [(now - 2000.0, 10), (now - 500.0, 40)].into_iter().collect();
    let v = view::view(&m).recv;
    assert!(v.line.contains("/s · ") && v.line.ends_with(" left"), "{}", v.line);
    assert_eq!(v.tip, crate::i18n::t(Lang::En, "tipGood"));
    assert_eq!(v.overlay.expect("outline").tone, view::Tone::Good);

    m.recv.prog_hist = [(now - 6000.0, 10), (now - 5000.0, 40)].into_iter().collect();
    let v = view::view(&m).recv;
    assert_eq!(v.tip, crate::i18n::t(Lang::En, "tipColor"), "stalled: the hint is back");
    assert_eq!(v.overlay.expect("outline").tone, view::Tone::Poor);
}
