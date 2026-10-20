//! The decoder thread: collects packets and runs the fountain decode off the camera thread
//! (the web demo's sink worker). A block decode of a large file takes a noticeable time; it
//! must not hold up scanning.

use lumen::transfer::{Finish, Limits, Receiver, State};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver as Rx, Sender as Tx};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressFact {
    pub generation: u64,
    pub tid: u16,
    pub name: Option<String>,
    pub size: Option<u32>,
    pub received: u32,
    pub needed: Option<u32>,
    /// Symbol size in bytes (received × t = payload bytes collected).
    pub t: u32,
    /// The transfer is larger than the receiver accepts (PROTOCOL §11.4): the larger of
    /// payload and file size, in bytes.
    pub refused: Option<u32>,
}

/// The receiver limits of the apps: the reference values (PROTOCOL §11.4).
pub fn limits() -> Limits {
    Limits::default()
}

pub enum SinkEvent {
    Progress(ProgressFact),
    Done { generation: u64, name: String, bytes: Vec<u8> },
    ChecksumError { generation: u64 },
}

enum Msg {
    Packets(u64, Vec<Vec<u8>>),
    Reset(u64),
}

pub struct Sink {
    tx: Tx<Msg>,
}

impl Sink {
    /// Starts the thread; `emit` is called on it for every event.
    pub fn start(emit: impl Fn(SinkEvent) + Send + 'static) -> Sink {
        let (tx, rx) = channel::<Msg>();
        std::thread::Builder::new()
            .name("lumen-decoder".into())
            .spawn(move || run(rx, emit))
            .expect("decoder thread");
        Sink { tx }
    }

    pub fn push(&self, generation: u64, packets: Vec<Vec<u8>>) {
        if !packets.is_empty() {
            let _ = self.tx.send(Msg::Packets(generation, packets));
        }
    }

    pub fn reset(&self, generation: u64) {
        let _ = self.tx.send(Msg::Reset(generation));
    }
}

fn run(rx: Rx<Msg>, emit: impl Fn(SinkEvent)) {
    let mut generation = 0u64;
    let mut receiver = Receiver::with_limits(limits());
    let mut last: Option<ProgressFact> = None;
    while let Ok(first) = rx.recv() {
        // take everything queued, then report once
        let mut dirty = false;
        let mut next = Some(first);
        while let Some(msg) = next.take().or_else(|| rx.try_recv().ok()) {
            match msg {
                Msg::Reset(g) => {
                    generation = g;
                    receiver = Receiver::with_limits(limits());
                    last = None;
                    dirty = false;
                }
                Msg::Packets(g, packets) if g == generation => {
                    for p in &packets {
                        receiver.push(p);
                    }
                    dirty = true;
                }
                Msg::Packets(..) => {}
            }
        }
        if !dirty {
            continue;
        }
        if let Some(p) = receiver.progress() {
            let fact = ProgressFact {
                generation,
                tid: p.tid,
                name: p.name.clone(),
                size: p.size,
                received: p.received,
                needed: p.needed,
                t: p.t as u32,
                refused: (p.state == State::Refused).then(|| p.size.unwrap_or(0).max(p.payload_length.unwrap_or(0))),
            };
            if last.as_ref() != Some(&fact) {
                last = Some(fact.clone());
                emit(SinkEvent::Progress(fact));
            }
        }
        match receiver.try_finish() {
            Some(Finish::File { name, bytes }) => emit(SinkEvent::Done { generation, name, bytes }),
            Some(Finish::ChecksumError) => emit(SinkEvent::ChecksumError { generation }),
            None => {}
        }
    }
}
