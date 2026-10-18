//! The runtime: one Crux core plus everything that runs in Rust on its behalf.
//!
//! ```text
//!  shell ──dispatch(event)──► core ──► EngineOp ──► runtime (sender, scanner, decoder, timers)
//!    ▲                         │                         │
//!    └──── update(view, ShellOps) ◄──────────────────────┘ facts as events
//! ```
//!
//! A platform shell sees only [`Update`]s: the view and the [`ShellOp`]s it must perform. The
//! real-time paths never cross JSON or wait on the core:
//!
//! - sending: the shell's display link calls [`Runtime::send_frame`] at the frame rate the view
//!   asks for and draws the one-pixel-per-cell image it returns;
//! - receiving: the camera thread hands every frame to [`Runtime::camera_frame`]; pacing, the
//!   region of interest and the scan happen there, packets go to the decoder thread, and the
//!   facts reach the core as events.
//!
//! [`Shell::update`] is called on whichever thread caused the change (UI, camera, decoder,
//! timer). Performing the requests is mandatory; a view whose `rev` is older than the one on
//! screen may be skipped.

use crate::app::{EngineOp, EngineOutput, Event, Lumen, ShellOp, ShellOutput, ViewModel};
use crate::content::{self, JsonLine, Kind};
use crate::engine::image::FrameRef;
use crate::engine::recv::{Plan, Prepared as PreparedImage, RecvEngine};
use crate::engine::send::{CellFrame, SendEngine, SenderInfo, StaticLayout};
use crate::engine::sink::{Sink, SinkEvent};
use crate::geometry::Geometry;
use crux_core::{Core, Request};
use lumen::scan::Scanner;
use lumen::transfer::{prepare, Prepared};
use serde::Serialize;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender as Tx};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

type Effect = crate::app::Effect;

/// Files larger than this are refused (the protocol's limit is ~400 MB at typical packet
/// sizes; a phone screen would need hours for that anyway).
pub const MAX_FILE: usize = 256 * 1024 * 1024;

/// Most view updates per second caused by the engine (a person's input is never held back).
pub const VIEW_HZ: u64 = 20;

/// Timer id that sends a coalesced view instead of answering an operation.
const FLUSH: u64 = u64::MAX;

struct Emit {
    last: Instant,
    scheduled: bool,
}

/// A request for the platform shell. `id == 0`: a notification, never answered.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ShellRequest {
    pub id: u64,
    pub op: ShellOp,
}

#[derive(Clone, Debug, Serialize)]
pub struct Update {
    pub view: ViewModel,
    pub requests: Vec<ShellRequest>,
}

pub trait Shell: Send + Sync + 'static {
    fn update(&self, update: Update);
}

impl<F: Fn(Update) + Send + Sync + 'static> Shell for F {
    fn update(&self, update: Update) {
        self(update)
    }
}

/// What the reader shows for the received file.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ReaderContent {
    pub kind: Option<Kind>,
    /// Plain text (messages, text files, raw or unhighlighted JSON), cut to the display limit.
    pub text: Option<String>,
    /// Formatted, classed JSON lines.
    pub lines: Option<Vec<JsonLine>>,
    /// Link ranges in `text` (bytes).
    pub links: Vec<(usize, usize)>,
    pub truncated: bool,
}

struct Source {
    name: String,
    bytes: Arc<Vec<u8>>,
    prepared: Option<Arc<Prepared>>,
}

struct Received {
    name: String,
    bytes: Arc<Vec<u8>>,
    text: Option<Arc<String>>,
    kind: Kind,
}

/// A prepared region of a camera frame, waiting for a scanner.
struct Job {
    plan: Plan,
    img: PreparedImage,
    dw: usize,
    dh: usize,
}

/// Scanner threads. Each owns a scanner (its own reused-header state, as each web worker
/// does); the camera thread only plans and prepares, so a slow scan never holds up capture.
struct Pool {
    txs: Vec<Tx<Job>>,
    idle: Mutex<Vec<bool>>,
    bufs: Mutex<Vec<PreparedImage>>,
}

impl Pool {
    fn idle(&self) -> Option<usize> {
        lock(&self.idle).iter().position(|&b| b)
    }

    fn submit(&self, worker: usize, job: Job) {
        lock(&self.idle)[worker] = false;
        if self.txs[worker].send(job).is_err() {
            lock(&self.idle)[worker] = true;
        }
    }

    fn done(&self, worker: usize, buf: PreparedImage) {
        lock(&self.bufs).push(buf);
        lock(&self.idle)[worker] = true;
    }

    fn buffer(&self) -> PreparedImage {
        lock(&self.bufs).pop().unwrap_or_default()
    }
}

/// Scanners in parallel. Pacing decides how many scans run, not this: extra scanners only
/// absorb bursts (fast senders). Three on six cores and up, two on four.
fn worker_count() -> usize {
    match std::thread::available_parallelism().map_or(2, |n| n.get()) {
        n if n >= 6 => 3,
        n if n >= 4 => 2,
        _ => 1,
    }
}

fn scan_worker(id: usize, rx: std::sync::mpsc::Receiver<Job>, weak: Weak<Inner>) {
    let mut scanner = Scanner::new();
    while let Ok(job) = rx.recv() {
        let Some(inner) = weak.upgrade() else { return };
        let t0 = Instant::now();
        let res = job.img.scan(&mut scanner, job.dw, job.dh, &job.plan);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let now = inner.now();
        let (report, packets) = lock(&inner.recv).finish(&job.plan, &res, job.dw, job.dh, ms, now);
        inner.pool.done(id, job.img);
        inner.sink.push(report.generation, packets);
        inner.dispatch(Event::Scanned(Box::new(report)), false);
    }
}

#[derive(Default)]
struct Book {
    shell: HashMap<u64, Request<ShellOp>>,
    engine: HashMap<u64, Request<EngineOp>>,
    next_id: u64,
}

struct Inner {
    core: Core<Lumen>,
    /// Serialises access to the core and the request book.
    book: Mutex<Book>,
    shell: Box<dyn Shell>,
    epoch: Instant,
    source: Mutex<Option<Source>>,
    send: Mutex<Option<SendEngine>>,
    recv: Mutex<RecvEngine>,
    received: Mutex<Option<Received>>,
    sink: Sink,
    pool: Pool,
    emit: Mutex<Emit>,
    timers: Tx<(Instant, u64)>,
    stats_at: Mutex<f64>,
}

#[derive(Clone)]
pub struct Runtime(Arc<Inner>);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic elsewhere must not take the app down with it; the data are plain buffers.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Runtime {
    pub fn new(shell: impl Shell) -> Runtime {
        let inner = Arc::new_cyclic(|weak: &Weak<Inner>| {
            let w = weak.clone();
            let sink = Sink::start(move |ev| {
                if let Some(inner) = w.upgrade() {
                    inner.on_sink(ev);
                }
            });
            let (tx, rx) = channel::<(Instant, u64)>();
            let w = weak.clone();
            std::thread::Builder::new()
                .name("lumen-timers".into())
                .spawn(move || {
                    let mut heap: BinaryHeap<std::cmp::Reverse<(Instant, u64)>> = BinaryHeap::new();
                    loop {
                        let wait = heap.peek().map(|r| r.0 .0.saturating_duration_since(Instant::now()));
                        let got = match wait {
                            Some(d) => rx.recv_timeout(d),
                            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                        };
                        match got {
                            Ok(item) => heap.push(std::cmp::Reverse(item)),
                            Err(RecvTimeoutError::Timeout) => {}
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                        while heap.peek().is_some_and(|r| r.0 .0 <= Instant::now()) {
                            let std::cmp::Reverse((_, id)) = heap.pop().unwrap();
                            let Some(inner) = w.upgrade() else { return };
                            if id == FLUSH {
                                inner.flush();
                                continue;
                            }
                            let now = inner.now();
                            inner.resolve_engine(id, EngineOutput::Elapsed { now });
                        }
                    }
                })
                .expect("timer thread");
            let workers = worker_count();
            let mut txs = Vec::new();
            for id in 0..workers {
                let (tx, rx) = channel::<Job>();
                let w = weak.clone();
                std::thread::Builder::new().name(format!("lumen-scan-{id}")).spawn(move || scan_worker(id, rx, w)).expect("scan thread");
                txs.push(tx);
            }
            let pool = Pool { txs, idle: Mutex::new(vec![true; workers]), bufs: Mutex::new(Vec::new()) };
            let mut recv = RecvEngine::new();
            recv.workers = workers;
            Inner {
                core: Core::new(),
                book: Mutex::new(Book::default()),
                shell: Box::new(shell),
                epoch: Instant::now(),
                source: Mutex::new(None),
                send: Mutex::new(None),
                recv: Mutex::new(recv),
                received: Mutex::new(None),
                sink,
                pool,
                emit: Mutex::new(Emit { last: Instant::now() - Duration::from_secs(1), scheduled: false }),
                timers: tx,
                stats_at: Mutex::new(0.0),
            }
        });
        Runtime(inner)
    }

    /// Milliseconds since the runtime started (the clock every event carries).
    pub fn now(&self) -> f64 {
        self.0.now()
    }

    /// An event from the shell (a tap, a lifecycle change): its update is sent at once.
    pub fn dispatch(&self, event: Event) {
        self.0.dispatch(event, true);
    }

    /// Answer a shell request.
    pub fn resolve(&self, id: u64, output: ShellOutput) {
        self.0.resolve_shell(id, output);
    }

    pub fn view(&self) -> ViewModel {
        let _g = lock(&self.0.book);
        self.0.core.view()
    }

    // ------------------------------------------------------------------ sending

    /// A file to send (from a picker, a drop, a share sheet).
    pub fn load_file(&self, name: &str, bytes: Vec<u8>) {
        if bytes.len() > MAX_FILE {
            let lang = self.view().lang;
            let msg = crate::i18n::t(lang, "tooLarge").replace("{n}", &crate::format::bytes(MAX_FILE as f64));
            self.dispatch(Event::SourceFailed { message: msg });
            return;
        }
        let name = if name.trim().is_empty() { "file".to_string() } else { name.to_string() };
        let size = bytes.len() as u64;
        *lock(&self.0.source) = Some(Source { name: name.clone(), bytes: Arc::new(bytes), prepared: None });
        self.dispatch(Event::SourceLoaded { name, size, text: None });
    }

    /// The next frame as one RGBA pixel per cell, or `None` when nothing is being sent.
    /// Call at the frame rate the view asks for (`send.fps`), only while `send.playing`.
    pub fn send_frame(&self) -> Option<CellFrame> {
        let (frame, build_ms) = {
            let mut s = lock(&self.0.send);
            let e = s.as_mut()?;
            (e.next_frame(), e.build_ms())
        };
        let now = self.now();
        let report = {
            let mut at = lock(&self.0.stats_at);
            if now - *at >= 1000.0 {
                *at = now;
                true
            } else {
                false
            }
        };
        if report {
            self.0.dispatch(Event::SenderStats { build_ms }, false);
        }
        Some(frame)
    }

    /// The static mask at full resolution (width, height, RGBA), drawn over the scaled cells.
    pub fn send_mask(&self) -> Option<(u32, u32, Arc<Vec<u8>>)> {
        lock(&self.0.send).as_ref().map(|e| e.mask())
    }

    /// The static parts as vectors, for shells that draw cells as shapes.
    pub fn send_layout(&self) -> Option<StaticLayout> {
        lock(&self.0.send).as_ref().map(|e| e.static_layout())
    }

    // ------------------------------------------------------------------ receiving

    /// A camera frame (any thread; the platform's buffer is only read during the call).
    /// Returns whether it was scanned.
    pub fn camera_frame(&self, frame: &FrameRef) -> bool {
        if !frame.is_valid() {
            return false;
        }
        let now = self.now();
        let free = self.0.pool.idle();
        let plan = lock(&self.0.recv).plan(frame.width, frame.height, frame.rotation, now, free.is_some());
        let (Some(mut plan), Some(worker)) = (plan, free) else { return false };
        // only the region of interest leaves the camera's buffer, already scaled
        let t0 = Instant::now();
        let mut img = self.0.pool.buffer();
        let (dw, dh) = img.prepare(frame, &plan.crop);
        plan.prep_ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.0.pool.submit(worker, Job { plan, img, dw, dh });
        true
    }

    /// The camera's rolling-shutter readout (first row to last, ms), when the platform knows
    /// it: a slow readout tears captures, and the receiver then scans every one of them.
    pub fn camera_readout(&self, ms: f64) {
        lock(&self.0.recv).readout_ms = ms.max(0.0);
    }

    /// A still image to decode (a screenshot or a photo of a code).
    pub fn scan_image(&self, frame: &FrameRef) {
        let now = self.now();
        let out = lock(&self.0.recv).scan_still(frame, now);
        let (generation, found) = match out {
            Some((report, packets)) => {
                let g = report.generation;
                let found = report.ok;
                self.0.sink.push(g, packets);
                if found {
                    self.dispatch(Event::ImageScanned { generation: g, found, at: now });
                    self.dispatch(Event::Scanned(Box::new(report)));
                    return;
                }
                (g, false)
            }
            None => (lock(&self.0.recv).generation, false),
        };
        self.dispatch(Event::ImageScanned { generation, found, at: now });
    }

    pub fn received_name(&self) -> Option<String> {
        lock(&self.0.received).as_ref().map(|r| r.name.clone())
    }

    pub fn received_bytes(&self) -> Option<Arc<Vec<u8>>> {
        lock(&self.0.received).as_ref().map(|r| r.bytes.clone())
    }

    /// The text Copy puts on the clipboard: formatted JSON in the formatted view, else as received.
    pub fn copy_text(&self, pretty: bool) -> Option<String> {
        let r = lock(&self.0.received);
        let r = r.as_ref()?;
        let text = r.text.as_ref()?;
        if pretty && r.kind == Kind::Json {
            Some(content::format_json(text, false).iter().map(JsonLine::plain).collect::<Vec<_>>().join("\n"))
        } else {
            Some(text.as_str().to_owned())
        }
    }

    /// What the reader shows, for the view's current JSON mode.
    pub fn reader(&self, pretty: bool) -> Option<ReaderContent> {
        let r = lock(&self.0.received);
        let r = r.as_ref()?;
        let mut out = ReaderContent { kind: Some(r.kind), ..Default::default() };
        let Some(text) = r.text.as_ref() else { return Some(out) };
        let cut = |s: &str| -> (String, bool) {
            if s.len() <= content::TEXT_DISPLAY_LIMIT {
                return (s.to_string(), false);
            }
            let mut end = content::TEXT_DISPLAY_LIMIT;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            (s[..end].to_string(), true)
        };
        if r.kind == Kind::Json && pretty {
            if text.len() <= content::HIGHLIGHT_LIMIT {
                out.lines = Some(content::format_json(text, true));
            } else {
                let all = content::format_json(text, false).iter().map(JsonLine::plain).collect::<Vec<_>>().join("\n");
                (out.text, out.truncated) = {
                    let (t, c) = cut(&all);
                    (Some(t), c)
                };
            }
        } else {
            let (t, c) = cut(text);
            if r.kind == Kind::Message && t.len() <= 200_000 {
                out.links = content::links(&t);
            }
            out.text = Some(t);
            out.truncated = c;
        }
        Some(out)
    }
}

impl Inner {
    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64() * 1000.0
    }

    /// `urgent`: the person caused it. Otherwise (scan results, progress, statistics) views
    /// that change nothing else are coalesced to at most [`VIEW_HZ`] per second.
    fn dispatch(self: &Arc<Self>, event: Event, urgent: bool) {
        let effects = {
            let _g = lock(&self.book);
            self.core.process_event(event)
        };
        self.process_with(effects, urgent);
    }

    /// Sends the latest view (a coalesced update whose time has come).
    fn flush(self: &Arc<Self>) {
        {
            let mut e = lock(&self.emit);
            e.scheduled = false;
            e.last = Instant::now();
        }
        let view = {
            let _g = lock(&self.book);
            self.core.view()
        };
        self.shell.update(Update { view, requests: Vec::new() });
    }

    fn resolve_shell(self: &Arc<Self>, id: u64, output: ShellOutput) {
        let effects = {
            let mut book = lock(&self.book);
            let Some(mut req) = book.shell.remove(&id) else { return };
            // an error means the command that asked is gone: the answer outlived the question
            self.core.resolve(&mut req, output).unwrap_or_default()
        };
        self.process(effects);
    }

    fn resolve_engine(self: &Arc<Self>, id: u64, output: EngineOutput) {
        let effects = {
            let mut book = lock(&self.book);
            let Some(mut req) = book.engine.remove(&id) else { return };
            self.core.resolve(&mut req, output).unwrap_or_default()
        };
        self.process(effects);
    }

    fn process(self: &Arc<Self>, effects: Vec<Effect>) {
        self.process_with(effects, true);
    }

    /// Runs engine requests, books shell requests, and tells the shell.
    fn process_with(self: &Arc<Self>, effects: Vec<Effect>, urgent: bool) {
        let mut queue: VecDeque<Effect> = effects.into();
        let mut requests = Vec::new();
        let mut dirty = false;
        while let Some(effect) = queue.pop_front() {
            match effect {
                Effect::Render(_) => dirty = true,
                Effect::Shell(req) => {
                    let op = req.operation.clone();
                    let id = if op.is_notification() {
                        0
                    } else {
                        let mut book = lock(&self.book);
                        book.next_id += 1;
                        let id = book.next_id;
                        book.shell.insert(id, req);
                        id
                    };
                    requests.push(ShellRequest { id, op });
                }
                Effect::Engine(mut req) => {
                    if let Some(out) = self.run(&req.operation) {
                        let more = {
                            let _g = lock(&self.book);
                            self.core.resolve(&mut req, out).unwrap_or_default()
                        };
                        queue.extend(more);
                    } else if expects_answer(&req.operation) {
                        // answered later, from another thread
                        let id = {
                            let mut book = lock(&self.book);
                            book.next_id += 1;
                            let id = book.next_id;
                            book.engine.insert(id, req);
                            id
                        };
                        self.run_later(id);
                    }
                }
            }
        }
        if dirty || !requests.is_empty() {
            {
                // a hot phone redraws the dashboard half as often (the UI is a good part of the load)
                let hz = if lock(&self.recv).heat >= 2 { VIEW_HZ / 2 } else { VIEW_HZ };
                let mut e = lock(&self.emit);
                let gap = Duration::from_millis(1000 / hz);
                if !urgent && requests.is_empty() && e.last.elapsed() < gap {
                    // the dashboard does not need every scan; the latest view follows shortly
                    if !e.scheduled {
                        e.scheduled = true;
                        let _ = self.timers.send((e.last + gap, FLUSH));
                    }
                    return;
                }
                e.last = Instant::now();
            }
            let view = {
                let _g = lock(&self.book);
                self.core.view()
            };
            self.shell.update(Update { view, requests });
        }
    }

    /// Runs an engine operation now; `None` when it answers later (or never).
    fn run(self: &Arc<Self>, op: &EngineOp) -> Option<EngineOutput> {
        match op {
            EngineOp::LoadText { text } => {
                let bytes = text.as_bytes().to_vec();
                let size = bytes.len() as u64;
                *lock(&self.source) = Some(Source { name: content::TEXT_NAME.into(), bytes: Arc::new(bytes), prepared: None });
                Some(EngineOutput::Loaded { name: content::TEXT_NAME.into(), size })
            }
            EngineOp::Prepare | EngineOp::Delay { .. } => None,
            EngineOp::StartSender { mode, ecc, geometry, keep_tid } => {
                let tid = if *keep_tid { lock(&self.send).as_ref().map(|e| e.sender.tid) } else { None };
                Some(EngineOutput::Sender(self.start_sender(*mode, *ecc, *geometry, tid)))
            }
            EngineOp::Reconfigure { mode, ecc, geometry } => {
                let current = lock(&self.send).take();
                let info = match current {
                    None => SenderInfo::default(),
                    Some(e) => match e.reconfigure(*mode as usize, *ecc as usize, *geometry) {
                        Ok(e) => self.keep(e),
                        // the packet size cannot be kept: a new transfer
                        Err(_) => self.start_sender(*mode, *ecc, *geometry, None),
                    },
                };
                Some(EngineOutput::Sender(info))
            }
            EngineOp::Resize { geometry } => {
                let current = lock(&self.send).take();
                let info = match current {
                    None => SenderInfo::default(),
                    Some(e) => {
                        let (mode, ecc, tid) = (e.sender.mode as u8, e.sender.ecc as u8, e.sender.tid);
                        match e.resize(*geometry) {
                            Ok(e) => self.keep(e),
                            Err(_) => self.start_sender(mode, ecc, *geometry, Some(tid)),
                        }
                    }
                };
                Some(EngineOutput::Sender(info))
            }
            EngineOp::StopSender => {
                *lock(&self.send) = None;
                *lock(&self.source) = None;
                None
            }
            EngineOp::RecvActive { on } => {
                let now = self.now();
                lock(&self.recv).set_active(*on, now);
                None
            }
            EngineOp::Heat { level } => {
                lock(&self.recv).heat = *level;
                None
            }
            EngineOp::ResetRecv { generation } => {
                lock(&self.recv).reset(*generation);
                self.sink.reset(*generation);
                *lock(&self.received) = None;
                None
            }
        }
    }

    fn keep(&self, e: SendEngine) -> SenderInfo {
        let len = self.payload_len();
        let info = e.info(true, len);
        *lock(&self.send) = Some(e);
        info
    }

    fn payload_len(&self) -> u64 {
        lock(&self.source).as_ref().and_then(|s| s.prepared.as_ref()).map_or(0, |p| p.payload.len() as u64)
    }

    fn start_sender(&self, mode: u8, ecc: u8, g: Geometry, tid: Option<u16>) -> SenderInfo {
        let prepared = lock(&self.source).as_ref().and_then(|s| s.prepared.clone());
        let Some(prepared) = prepared else { return SenderInfo::default() };
        let tid = tid.unwrap_or_else(random_tid);
        match SendEngine::new(&prepared, mode as usize, ecc as usize, g, tid) {
            Some(e) => {
                let info = e.info(false, prepared.payload.len() as u64);
                *lock(&self.send) = Some(e);
                info
            }
            None => SenderInfo::default(),
        }
    }

    fn run_later(self: &Arc<Self>, id: u64) {
        let op = lock(&self.book).engine.get(&id).map(|r| r.operation.clone());
        match op {
            Some(EngineOp::Delay { ms }) => {
                let _ = self.timers.send((Instant::now() + Duration::from_millis(ms as u64), id));
            }
            Some(EngineOp::Prepare) => {
                let me = self.clone();
                std::thread::Builder::new()
                    .name("lumen-prepare".into())
                    .spawn(move || {
                        let src = lock(&me.source).as_ref().map(|s| (s.name.clone(), s.bytes.clone()));
                        let out = match src {
                            Some((name, bytes)) => {
                                let p = Arc::new(prepare(&bytes, &name, true));
                                let len = p.payload.len() as u64;
                                let mut s = lock(&me.source);
                                match s.as_mut() {
                                    // still the same file?
                                    Some(cur) if Arc::ptr_eq(&cur.bytes, &bytes) => {
                                        cur.prepared = Some(p);
                                        EngineOutput::Prepared { ok: true, payload_len: len }
                                    }
                                    _ => EngineOutput::Prepared { ok: false, payload_len: 0 },
                                }
                            }
                            None => EngineOutput::Prepared { ok: false, payload_len: 0 },
                        };
                        me.resolve_engine(id, out);
                    })
                    .expect("prepare thread");
            }
            _ => {}
        }
    }

    fn on_sink(self: &Arc<Self>, ev: SinkEvent) {
        let at = self.now();
        match ev {
            SinkEvent::Progress(fact) => self.dispatch(Event::Progress { fact, at }, false),
            SinkEvent::ChecksumError { generation } => self.dispatch(Event::ChecksumError { generation, at }, true),
            SinkEvent::Done { generation, name, bytes } => {
                if lock(&self.recv).generation != generation {
                    return;
                }
                let (classified, text) = content::classify(&name, &bytes);
                let size = bytes.len() as u64;
                lock(&self.recv).finished = true;
                *lock(&self.received) =
                    Some(Received { name: name.clone(), bytes: Arc::new(bytes), text: text.map(Arc::new), kind: classified.kind });
                self.dispatch(Event::Received { generation, name, size, classified, at }, true);
            }
        }
    }
}

fn expects_answer(op: &EngineOp) -> bool {
    !matches!(op, EngineOp::StopSender | EngineOp::RecvActive { .. } | EngineOp::ResetRecv { .. } | EngineOp::Heat { .. })
}

fn random_tid() -> u16 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(Instant::now().elapsed().as_nanos());
    h.write_u64(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64));
    (h.finish() & 0xffff) as u16
}
