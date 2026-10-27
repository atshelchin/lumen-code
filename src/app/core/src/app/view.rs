//! The view model: everything a shell draws, already decided and translated. A shell lays
//! it out and forwards gestures; it computes nothing (no rates, no hints, no labels).

use super::{settings_open, stats_open, Model, Platform, RecvPhase, Screen, SendPhase, SourceTab, FOUND_MS};
use crate::content::Kind;
use crate::engine::recv::ScanReport;
use crate::format;
use crate::geometry::{Density, Geometry};
use crate::i18n::{t, Lang};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewModel {
    /// Increases with every update; a shell ignores a view older than the one it shows.
    pub rev: u64,
    pub lang: Lang,
    pub platform: Platform,
    pub screen: Screen,
    pub send: SendView,
    pub recv: RecvView,
    /// The version of the core: commit, date (`crate::build`). A shell shows it with its own
    /// app version, for reports.
    pub build: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub label: String,
    pub value: String,
    /// Dropped first when the dock is narrow.
    pub wide: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendView {
    pub phase: SendPhase,
    pub source_tab: SourceTab,
    /// "12 chars · 13 B" under the text box; empty when there is no text.
    pub text_meta: String,
    pub can_send_text: bool,
    pub drop_desc: String,
    pub drop_hint: String,
    /// Photosensitivity warning, shown before a code plays (PROTOCOL §10.4).
    pub flash_warn: String,
    /// The code is ready but waits, paused, until the warning is acknowledged (`AcceptFlash`).
    pub flash_ask: bool,
    /// The text on the Text tab; the field shows this, so what is shown is what is sent.
    pub text: String,
    /// Bar: the file name, or just "Text" (never the text itself).
    pub title: String,
    /// Bar, under the title: size and time per pass ("78 KB · ≈ 3 s"), or "Paused".
    pub subtitle: String,
    /// Settings panel: raw rate, per frame, grid, bits per cell, engine time, in one line.
    pub details: String,
    pub size: String,
    pub stats: Vec<Stat>,
    pub playing: bool,
    /// Frames per second the shell flips at while playing.
    pub fps: u32,
    pub dock_visible: bool,
    pub settings_open: bool,
    pub density: Density,
    pub mode: u8,
    pub ecc: u8,
    /// What to draw; `None` until the first frame exists.
    pub geometry: Option<Geometry>,
    /// Changes when the mask / static layout must be fetched again.
    pub frame_rev: u64,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Good,
    Fair,
    Poor,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomView {
    pub level: f32,
    pub min: f32,
    pub max: f32,
    pub label: String,
    /// Zoomed in beyond the widest setting.
    pub active: bool,
}

/// The detected code outline, in camera-frame pixels (TL, TR, BR, BL). The shell maps it
/// onto its preview the same way it maps the frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Overlay {
    pub points: [[f32; 2]; 4],
    pub frame_w: u32,
    pub frame_h: u32,
    pub rotation: u32,
    pub tone: Tone,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoneView {
    pub kind: Option<Kind>,
    /// Text and images fill the screen; other files get a card.
    pub reader: bool,
    pub name: String,
    pub title: String,
    pub sub: String,
    pub stats: Vec<Metric>,
    pub code_line: Option<String>,
    pub stats_open: bool,
    pub json: bool,
    pub pretty: bool,
    pub wrap: bool,
    /// Reader text size step, -4..=8.
    pub font: i32,
    pub copy_label: Option<String>,
    pub link: Option<String>,
    pub save_label: String,
    pub save_primary: bool,
    pub can_open: bool,
    pub note: Option<String>,
    pub note_ok: bool,
    /// Changes when the shell must fetch the reader content again.
    pub content_rev: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecvView {
    pub phase: RecvPhase,
    pub camera_on: bool,
    pub camera_starting: bool,
    pub error: Option<String>,
    pub can_open_settings: bool,
    pub note: Option<String>,
    pub live: bool,
    pub status: String,
    pub name: String,
    pub pct: String,
    /// 0…1
    pub progress: f32,
    /// Under the progress bar: "32 KB/s · 2s left" (empty until data arrives).
    pub line: String,
    /// Speed, time left, cell size, success: for the Details area.
    pub metrics: Vec<Metric>,
    pub tip: String,
    pub tip_ok: bool,
    pub diag: String,
    pub zoom: Option<ZoomView>,
    pub can_switch: bool,
    pub overlay: Option<Overlay>,
    pub done: Option<DoneView>,
}

pub fn view(m: &Model) -> ViewModel {
    ViewModel { rev: m.rev, lang: m.lang, platform: m.platform, screen: m.screen, send: send_view(m), recv: recv_view(m), build: crate::build() }
}

fn send_view(m: &Model) -> SendView {
    let l = m.lang;
    let s = &m.send;
    let text_meta = if s.text.is_empty() {
        String::new()
    } else {
        format!("{} {} · {}", s.text.chars().count(), t(l, "chars"), format::bytes(s.text.len() as f64))
    };
    let (title, size) = match &s.source {
        Some(src) => {
            let title = match &src.text {
                // never the text itself: the screen is meant to be filmed (passwords, links)
                Some(_) => t(l, "textLabel").to_string(),
                None => src.name.clone(),
            };
            (title, format::bytes(src.size as f64))
        }
        None => (String::new(), String::new()),
    };
    let mut stats = Vec::new();
    // under the name in the bar: size and time per pass (or "Paused"); details in settings
    let mut subtitle = size.clone();
    let mut details = String::new();
    if s.phase == SendPhase::Preparing {
        stats.push(Stat { label: t(l, "preparing").into(), value: String::new(), wide: false });
    }
    if let (SendPhase::Ready, Some(info)) = (s.phase, &s.info) {
        let fps = m.prefs.fps as f64;
        let per = info.bytes_per_frame as f64;
        let rate = per * fps;
        let pass = if rate > 0.0 { info.payload_len as f64 / rate } else { 0.0 };
        let pass = if pass < 1.0 {
            "< 1 s".to_string()
        } else if pass < 10.0 {
            format!("{pass:.1} s")
        } else {
            format!("{pass:.0} s")
        };
        let stat = |k: &str, v: String, wide| Stat { label: t(l, k).into(), value: v, wide };
        stats.push(stat("rate", format!("{}/s", format::bytes(rate)), false));
        stats.push(stat("perFrame", format::bytes(per), false));
        stats.push(stat("loop", pass.clone(), true));
        stats.push(Stat { label: t(l, "grid").into(), value: format!("{}×{} · {} bit", info.x, info.y, info.bpc), wide: true });
        let ms = if s.build_ms > 0.0 { format!("{:.2} ms", s.build_ms) } else { "–".into() };
        stats.push(Stat { label: "Rust".into(), value: ms.clone(), wide: true });
        details = format!(
            "{} {}/s · {} {} · {} {}×{} · {} bit · Rust {}",
            t(l, "rate"),
            format::bytes(rate),
            t(l, "perFrame"),
            format::bytes(per),
            t(l, "grid"),
            info.x,
            info.y,
            info.bpc,
            ms
        );
        subtitle = if s.playing { format!("{size} · ≈ {pass}") } else { t(l, "paused").to_string() };
    }
    let mobile = m.platform.mobile();
    SendView {
        phase: s.phase,
        source_tab: m.prefs.source,
        can_send_text: !s.text.is_empty() && s.phase == SendPhase::Pick,
        text_meta,
        drop_desc: t(l, if mobile { "dropDescMobile" } else { "dropDesc" }).into(),
        drop_hint: t(l, if mobile { "dropHintMobile" } else { "dropHint" }).into(),
        flash_warn: t(l, "flashWarn").into(),
        flash_ask: s.phase == SendPhase::Ready && !m.prefs.flash_ok,
        text: s.text.clone(),
        title,
        size,
        subtitle,
        details,
        stats,
        playing: s.playing,
        fps: m.prefs.fps,
        dock_visible: s.dock_visible,
        settings_open: settings_open(m),
        density: m.prefs.density,
        mode: m.prefs.mode,
        ecc: m.prefs.ecc,
        geometry: if s.phase == SendPhase::Ready { s.geometry } else { None },
        frame_rev: s.frame_rev,
        error: s.error.clone(),
    }
}

/// How strongly the code is seen in perspective: the longer of each pair of opposite edges
/// over the shorter. A phone held at an angle to the screen has one side farther away —
/// smaller and outside the camera's shallow depth of field — and its codewords fail there.
fn skew(r: &ScanReport) -> f64 {
    let Some(c) = r.corners else { return 1.0 };
    let d = |a: [f32; 2], b: [f32; 2]| ((a[0] - b[0]) as f64).hypot((a[1] - b[1]) as f64).max(1.0);
    // TL, TR, BR, BL
    let (top, right, bottom, left) = (d(c[0], c[1]), d(c[1], c[2]), d(c[2], c[3]), d(c[3], c[0]));
    (top.max(bottom) / top.min(bottom)).max(left.max(right) / left.min(right))
}

/// Whether the camera picture is upright-portrait on screen.
fn portrait(r: &ScanReport) -> bool {
    if r.rotation % 180 == 90 {
        r.frame_w > r.frame_h
    } else {
        r.frame_h > r.frame_w
    }
}

fn recv_view(m: &Model) -> RecvView {
    let l = m.lang;
    let r = &m.recv;
    let now = m.now;
    let last = r.last.as_ref();
    let st = last.map(|x| &x.stats);
    let found = st.is_some_and(|s| s.last_found_at >= 0.0 && now - s.last_found_at < FOUND_MS);

    let mut v = RecvView {
        phase: r.phase,
        camera_on: r.camera.on,
        camera_starting: r.camera.starting,
        live: found,
        status: t(l, if found { "receiving" } else { "waiting" }).into(),
        name: t(l, "noData").into(),
        pct: "0%".into(),
        can_switch: r.camera.on && r.camera.devices > 1,
        note: r.note.map(|k| t(l, k).to_string()),
        ..Default::default()
    };
    if let Some((kind, msg)) = &r.camera.error {
        use super::CameraError::*;
        v.error = Some(match kind {
            Denied => t(l, "camDenied").to_string(),
            Unavailable => t(l, "camNone").to_string(),
            Failed => format!("{}{}", t(l, "camErr"), msg),
        });
        v.can_open_settings = *kind == Denied && m.platform.mobile();
    }
    let cam = &r.camera;
    if cam.on && cam.zoom_max > cam.zoom_min + 0.01 {
        v.zoom = Some(ZoomView {
            level: cam.zoom,
            min: cam.zoom_min,
            max: cam.zoom_max,
            label: format!("{:.1}×", cam.zoom),
            active: cam.zoom > cam.zoom_min + 0.05,
        });
    }

    // live metrics
    let (cell, cell_n) = st.map_or((0.0, 0), |s| (s.cell as f64, s.cell_n));
    let (cov_ok, cov_tot) = st.map_or((0, 0), |s| (s.cov_ok, s.cov_tot));
    let ok_rate = if cov_tot > 0 { cov_ok as f64 / cov_tot as f64 } else { 0.0 };
    let mut speed = "–".to_string();
    let mut eta = "–".to_string();
    // data is arriving: then the transfer is going well, whatever share of each frame is read
    let moving = r.prog_hist.len() >= 2
        && r.prog_hist.back().zip(r.prog_hist.front()).is_some_and(|(b, f)| b.1 > f.1 && now - b.0 < 2500.0);
    if let Some(p) = &r.progress {
        v.name = p.name.clone().unwrap_or_else(|| t(l, "receiving").into());
        let done = r.phase == RecvPhase::Done;
        let pct = match p.needed {
            Some(n) if n > 0 => (100.0 * p.received as f64 / n as f64).min(if done { 100.0 } else { 99.0 }),
            _ => 0.0,
        };
        v.pct = format!("{}%", pct.floor() as u32);
        v.progress = (pct / 100.0) as f32;
        let mut rate = 0.0;
        if r.prog_hist.len() >= 2 {
            let (t0, r0) = r.prog_hist[0];
            let (t1, r1) = r.prog_hist[r.prog_hist.len() - 1];
            if t1 > t0 {
                rate = (r1 as f64 - r0 as f64) * 1000.0 / (t1 - t0);
            }
        }
        if rate > 0.0 {
            speed = format!("{}/s", format::bytes(rate * p.t as f64));
            if let Some(n) = p.needed {
                eta = format::time((n as f64 - p.received as f64) / rate);
            }
        }
    }
    let metric = |k: &str, value: String| Metric { label: t(l, k).into(), value };
    // the panel's one line under the progress bar; cell and success go under Details
    v.line = match (speed.as_str(), eta.as_str()) {
        ("–", _) => String::new(),
        (sp, "–") => sp.to_string(),
        (sp, e) => format!("{sp} · {}", t(l, "timeLeft").replace("{t}", e)),
    };
    v.metrics = vec![
        metric("speedM", speed),
        metric("etaM", eta),
        metric("cellM", if cell_n > 0 { format!("{cell:.1}px") } else { "–".into() }),
        metric("okM", if cov_tot > 0 { format!("{}%", (ok_rate * 100.0).round() as u32) } else { "–".into() }),
    ];

    // which link limits the rate?
    if let Some(s) = st {
        let ms = if s.avg_ms > 0.0 { format!("{:.1} ms", s.avg_ms) } else { "–".into() };
        let mut d = format!(
            "{} {} · {} {:.0} · {} {:.0}/s · {} {:.1}/s",
            t(l, "native"),
            ms,
            t(l, "camFps"),
            s.cam_fps,
            t(l, "scanFps"),
            s.scan_fps,
            t(l, "frameFps"),
            s.distinct_fps
        );
        if s.sender_fps > 0.0 {
            d.push_str(&format!(" · {} {:.0}/s · {} {}%", t(l, "senderFps"), s.sender_fps, t(l, "captured"), (s.captured * 100.0).round() as u32));
        }
        v.diag = d;
    }

    // one hint at a time, most useful first
    let (tip, ok) = if now < r.checksum_until {
        (t(l, "checksum").to_string(), false)
    } else if let Some(size) = r.progress.as_ref().and_then(|p| p.refused) {
        let max = crate::engine::sink::limits().max_size as f64;
        (t(l, "recvTooLarge").replace("{size}", &format::bytes(size as f64)).replace("{max}", &format::bytes(max)), false)
    } else if !found {
        (t(l, "tipSearch").to_string(), false)
    } else if !moving && cell_n > 0 && cell < 3.4 {
        let landscape_code = last.and_then(|x| x.header).is_some_and(|h| h.x as f64 > h.y as f64 * 1.2);
        let k = if landscape_code && last.is_some_and(portrait) { "tipLandscape" } else { "tipCloser" };
        (t(l, k).to_string(), false)
    } else if !moving && cov_tot > 0 && ok_rate < 0.9 && last.is_some_and(|x| skew(x) > 1.08) {
        (t(l, "tipParallel").to_string(), false)
    } else if !moving && cov_tot > 0 && ok_rate < 0.5 {
        (t(l, if cell < 4.5 { "tipSteady" } else { "tipColor" }).to_string(), false)
    } else if m.thermal >= 2 && r.camera.on && r.camera.max_fps > 60 && r.camera.fps <= 60 {
        (t(l, "tipHot").to_string(), false)
    } else if st.is_some_and(|s| s.heat_limited) {
        (t(l, "tipHotScan").to_string(), false)
    } else if let Some(s) = st.filter(|s| s.tearing && s.span_secs > 2.0 && s.cam_fps > 0.0 && s.sender_fps > 0.95 * s.cam_fps) {
        // torn captures: past ~0.8 of the camera's rate, each tear costs more than a frame brings
        let n = (s.cam_fps * 0.8).round() as u32;
        (t(l, "tipFpsDown").replace("{cam}", &format!("{:.0}", s.cam_fps)).replace("{n}", &n.to_string()), false)
    } else if let Some(s) = st.filter(|s| {
        // what the camera can follow: two captures per frame, or ~0.8 of its rate when torn
        let target = if s.tearing { 0.8 * s.cam_fps } else { s.cam_fps / 2.0 };
        s.sender_fps > 0.0 && s.captured > 0.95 && ok_rate > 0.9 && s.span_secs > 2.0 && s.load < 0.5 && 1.1 * s.sender_fps < target
    }) {
        let target = if s.tearing { 0.8 * s.cam_fps } else { s.cam_fps / 2.0 };
        let n = target.min(s.sender_fps * 1.5).round() as u32;
        (t(l, "tipFpsUp").replace("{n}", &n.to_string()), true)
    } else {
        (t(l, "tipGood").to_string(), true)
    };
    v.tip = tip;
    v.tip_ok = ok;

    if let Some(x) = last.filter(|x| x.ok && now - x.at < FOUND_MS && r.phase == RecvPhase::Scanning) {
        if let Some(points) = x.corners {
            let tone = if moving || x.frame_rate > 0.8 {
                Tone::Good
            } else if x.frame_rate > 0.3 {
                Tone::Fair
            } else {
                Tone::Poor
            };
            v.overlay = Some(Overlay { points, frame_w: x.frame_w, frame_h: x.frame_h, rotation: x.rotation, tone });
        }
    }

    if let Some(d) = &r.done {
        v.done = Some(done_view(m, d));
        v.pct = "100%".into();
        v.progress = 1.0;
    }
    v
}

fn done_view(m: &Model, d: &super::DoneState) -> DoneView {
    let l = m.lang;
    let c = &d.classified;
    let message = c.message;
    let json = c.kind == Kind::Json;
    let text = matches!(c.kind, Kind::Message | Kind::Text | Kind::Json);
    let size = d.size as f64;
    let secs = d.secs;
    let mut what = format::bytes(size);
    if json {
        what = format!("JSON · {} {} · {}", format::grouped(c.lines.unwrap_or(0)), t(l, "lines"), what);
    } else if message {
        what = format!("{} {} · {}", format::grouped(c.chars.unwrap_or(0)), t(l, "chars"), what);
    }
    let timing = if secs >= 0.5 { format!(" · {:.1}s · {}/s", secs, format::bytes(size / secs)) } else { String::new() };
    let session = d.stats.as_ref().map(|r| &r.stats.session);
    let metric = |k: &str, value: String| Metric { label: t(l, k).into(), value };
    let wire = d.received as f64 * d.t as f64;
    let mut stats = vec![
        metric("dTime", if secs >= 0.05 { format!("{secs:.1} s") } else { "–".into() }),
        metric("dSpeed", if secs >= 0.5 { format!("{}/s", format::bytes(size / secs)) } else { "–".into() }),
        metric("dWire", if wire > 0.0 && secs >= 0.5 { format!("{}/s", format::bytes(wire / secs)) } else { "–".into() }),
    ];
    let mut code_line = None;
    if let Some(s) = session {
        stats.push(metric("dFrames", format!("{} / {}", s.frames, s.scans)));
        stats.push(metric("okM", if s.cov_tot > 0 { format!("{}%", (100.0 * s.cov_ok as f64 / s.cov_tot as f64).round() as u32) } else { "–".into() }));
        stats.push(metric("dEngine", format!("{} {:.1} ms", t(l, "native"), s.ms_avg)));
        stats.push(metric("buildLabel", crate::build()));
        if let Some(h) = s.header {
            let colours = [2, 4, 8].get(h.mode as usize).copied().unwrap_or(0);
            let cell = if s.cell_avg > 0.0 { format!("{:.1}px", s.cell_avg) } else { "–".into() };
            code_line = Some(format!(
                "{} {}×{} · {} {} · ECC {}% · {} {}",
                t(l, "dCode"),
                h.x,
                h.y,
                colours,
                t(l, "dColours"),
                (100.0 * h.nsym as f64 / h.n.max(1) as f64).round() as u32,
                t(l, "cellM"),
                cell
            ));
        }
    }
    let title = if message { t(l, if json { "jsonReceived" } else { "textReceived" }).to_string() } else { d.name.clone() };
    DoneView {
        kind: Some(c.kind),
        reader: text || c.kind == Kind::Image,
        name: d.name.clone(),
        title,
        sub: what + &timing,
        stats,
        code_line,
        stats_open: stats_open(m),
        json,
        pretty: m.prefs.reader.pretty,
        wrap: m.prefs.reader.wrap,
        font: m.prefs.reader.font,
        copy_label: if text { Some(t(l, if d.copied { "copied" } else { "copy" }).into()) } else { None },
        link: c.link.clone(),
        save_label: t(l, if message { "saveAsFile" } else { "save" }).into(),
        save_primary: !message,
        can_open: !message,
        note: d.note.map(|(_, k)| t(l, k).to_string()),
        note_ok: d.note.is_some_and(|(ok, _)| ok),
        content_rev: d.content_rev,
    }
}
