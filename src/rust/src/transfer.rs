//! Sender (file -> frames) and Receiver (packets -> file), PROTOCOL §9–§10.

use crate::deflate;
use crate::fountain::{Decoder, Encoder};
use crate::frame::{
    encode_descriptor, encode_frame_cells, packet_data_size, packet_intact, parse_descriptor, read_packet, seal_packet,
    write_packet_header, Descriptor, FLAG_DEFLATE, MAX_BLOCKS, PKT_HDR, PKT_OVERHEAD,
};
use crate::layout::{choose_codeword, codewords_per_frame, Layout, ECC_PERCENT, MIN_K, MIN_T};
use crate::util::{crc32, mix32};
use std::collections::HashMap;

pub use crate::frame::MAX_BLOCK_SYMBOLS;

/// Fountain seed of source block `sbn` of transfer `tid`.
pub fn fountain_seed(tid: u16, sbn: u32) -> u32 {
    mix32((tid as u32).wrapping_mul(0x10001).wrapping_add(0x51ed).wrapping_add(sbn.wrapping_mul(0x9e37_79b9)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockPlan {
    pub kt: u32,
    pub z: u32,
    pub sizes: Vec<u32>,
    pub starts: Vec<u32>,
}

pub fn block_plan(payload_length: u64, t: usize) -> Option<BlockPlan> {
    let kt = (payload_length.div_ceil(t as u64)).max(1);
    let z = kt.div_ceil(MAX_BLOCK_SYMBOLS);
    if z > MAX_BLOCKS {
        return None;
    }
    let (kt, z) = (kt as u32, z as u32);
    let ks = kt / z;
    let zl = kt - ks * z;
    let mut sizes = Vec::new();
    let mut starts = Vec::new();
    let mut at = 0;
    for b in 0..z {
        let k = if b < zl { ks + 1 } else { ks };
        sizes.push(k);
        starts.push(at);
        at += k;
    }
    Some(BlockPlan { kt, z, sizes, starts })
}

pub struct Prepared {
    pub payload: Vec<u8>,
    pub descriptor: Descriptor,
}

/// Build the payload (optionally DEFLATE-compressed when that helps) and the descriptor.
pub fn prepare(file: &[u8], name: &str, compress: bool) -> Prepared {
    let crc = crc32(file);
    let mut payload = file.to_vec();
    let mut flags = 0u8;
    if compress && file.len() > 64 {
        let z = deflate::deflate(file);
        if (z.len() as f64) < file.len() as f64 * 0.97 {
            payload = z;
            flags |= FLAG_DEFLATE;
        }
    }
    let descriptor = Descriptor { flags, payload_length: payload.len() as u32, file_size: file.len() as u32, crc, name: name.into(), records: Vec::new() };
    Prepared { payload, descriptor }
}

pub struct Frame {
    pub cells: Vec<u8>,
    pub seq: u32,
}

pub struct Sender {
    pub tid: u16,
    pub mode: usize,
    pub ecc: usize,
    pub n: usize,
    pub nsym: usize,
    pub t: usize,
    pub seq: u32,
    pub layout: Layout,
    pub per_frame: usize,
    pub plan: BlockPlan,
    encs: Vec<Encoder>,
    desc: Vec<u8>,
    counter: u64,
    frames: u64,
    /// descriptors sent: the next one goes in codeword mix32(count) mod m
    descriptors: u32,
}

impl Sender {
    pub fn new(prepared: &Prepared, mode: usize, ecc: usize, x: usize, y: usize, tid: u16, seq: u32) -> Option<Sender> {
        let layout = Layout::new(x, y)?;
        let (n, nsym) = choose_codeword(&layout, mode, ecc);
        let t = packet_data_size(n, nsym);
        if t < MIN_T {
            return None;
        }
        let plan = block_plan(prepared.payload.len() as u64, t)?;
        let mut padded = prepared.payload.clone();
        padded.resize(plan.kt as usize * t, 0);
        let encs = (0..plan.z as usize)
            .map(|b| {
                let s = plan.starts[b] as usize * t;
                Encoder::new(&padded[s..s + plan.sizes[b] as usize * t], t, fountain_seed(tid, b as u32))
            })
            .collect();
        let desc = encode_descriptor(&prepared.descriptor, t);
        let per_frame = codewords_per_frame(&layout, mode, n);
        if per_frame == 0 {
            return None;
        }
        Some(Sender { tid, mode, ecc, n, nsym, t, seq: seq & 255, layout, per_frame, plan, encs, desc, counter: 0, frames: 0, descriptors: 0 })
    }

    pub fn k(&self) -> u32 {
        self.plan.kt
    }

    pub fn bytes_per_frame(&self) -> usize {
        self.per_frame * self.t
    }

    /// Keep the packet size (and the stream) on a new grid size.
    pub fn resize(&mut self, x: usize, y: usize) -> bool {
        if x == self.layout.x && y == self.layout.y {
            return true;
        }
        let Some(layout) = Layout::new(x, y) else { return false };
        let pf = codewords_per_frame(&layout, self.mode, self.n);
        if pf == 0 {
            return false;
        }
        self.layout = layout;
        self.per_frame = pf;
        true
    }

    /// Change colour mode / ECC level and keep the packet size, when possible.
    pub fn reconfigure(&mut self, mode: usize, ecc: usize) -> bool {
        let pct = ECC_PERCENT[ecc] as usize;
        let base = self.t + PKT_OVERHEAD;
        let nsym = (2 * ((base * pct + (100 - pct)) / (2 * (100 - pct)))).max(4);
        let n = base + nsym;
        if n > 255 || codewords_per_frame(&self.layout, mode, n) == 0 {
            return false;
        }
        self.mode = mode;
        self.ecc = ecc;
        self.n = n;
        self.nsym = nsym;
        self.per_frame = codewords_per_frame(&self.layout, mode, n);
        true
    }

    /// The packets of the next frame (reference schedule, §10.2): the descriptor in every
    /// frame of a tiny transfer, otherwise in every 4th frame; its codeword moves, so that a
    /// corner hidden by glare cannot hide every copy.
    pub fn next_packets(&mut self) -> Vec<Vec<u8>> {
        let k = self.n - self.nsym;
        let m = self.per_frame;
        let meta_at = if self.k() as usize <= 2 * m || self.frames % 4 == 0 {
            self.descriptors += 1;
            mix32(self.descriptors - 1) as usize % m
        } else {
            usize::MAX
        };
        let z = self.plan.z as u64;
        let mut msgs = Vec::with_capacity(m);
        for i in 0..m {
            let mut msg = vec![0u8; k];
            if i == meta_at {
                write_packet_header(&mut msg, self.tid, 0, 0, true);
                msg[PKT_HDR..PKT_HDR + self.t].copy_from_slice(&self.desc);
            } else {
                let b = (self.counter % z) as usize;
                let esi = ((self.counter / z) & 0xffff) as u32;
                write_packet_header(&mut msg, self.tid, b as u32, esi, false);
                self.encs[b].encode(esi, &mut msg[PKT_HDR..PKT_HDR + self.t]);
                self.counter += 1;
            }
            seal_packet(&mut msg);
            msgs.push(msg);
        }
        self.frames += 1;
        msgs
    }

    pub fn next_frame(&mut self) -> Frame {
        let msgs = self.next_packets();
        let seq = self.seq;
        self.seq = (self.seq + 1) & 255;
        Frame { cells: encode_frame_cells(&self.layout, self.mode, self.n, self.nsym, seq, &msgs), seq }
    }
}

/// Limits of a receiver (§11.4). The defaults are the reference values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// a transfer with a larger payload or file is refused (bytes)
    pub max_size: u64,
    /// data packets kept for each transfer before its descriptor arrives
    pub max_kept: usize,
    /// transfer keys kept at the same time
    pub max_transfers: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_size: 256 << 20, max_kept: 4096, max_transfers: 4 }
    }
}

/// State of one transfer key (TID, T), §11.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Collecting,
    Receiving,
    Refused,
    Complete,
}

/// What the receiver did with a packet (§11.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Discard,
    /// kept until the descriptor arrives
    Keep,
    /// given to a block decoder
    Add,
    /// descriptor accepted
    Accept,
    /// descriptor accepted, transfer over a limit
    Refuse,
}

struct Inbound {
    tid: u16,
    t: usize,
    touched: u64,
    state: State,
    /// the accepted descriptor: all T bytes, and parsed
    desc_bytes: Vec<u8>,
    desc: Option<Descriptor>,
    plan: Option<BlockPlan>,
    /// data packets before the descriptor, by (SBN, ESI)
    kept: HashMap<(u32, u32), Vec<u8>>,
    /// one decoder per block, None once the block is decoded
    decs: Vec<Option<Decoder>>,
    outs: Vec<Option<Vec<u8>>>,
    /// symbol count of the last failed decode of each block
    last_try: Vec<usize>,
}

impl Inbound {
    fn new(tid: u16, t: usize) -> Inbound {
        Inbound {
            tid,
            t,
            touched: 0,
            state: State::Collecting,
            desc_bytes: Vec::new(),
            desc: None,
            plan: None,
            kept: HashMap::new(),
            decs: Vec::new(),
            outs: Vec::new(),
            last_try: Vec::new(),
        }
    }

    /// Discard all state of the key (§11.2).
    fn reset(&mut self) {
        let touched = self.touched;
        *self = Inbound::new(self.tid, self.t);
        self.touched = touched;
    }

    fn received(&self) -> u32 {
        match (self.state, &self.plan) {
            (State::Collecting, _) => self.kept.len() as u32,
            (State::Complete, Some(plan)) => plan.kt,
            (State::Receiving, Some(plan)) => (0..plan.z as usize)
                .map(|b| if self.outs[b].is_some() { plan.sizes[b] } else { self.decs[b].as_ref().map_or(0, |d| d.count() as u32) })
                .sum(),
            _ => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub tid: u16,
    pub t: usize,
    pub state: State,
    pub name: Option<String>,
    pub size: Option<u32>,
    pub payload_length: Option<u32>,
    pub received: u32,
    pub needed: Option<u32>,
    pub blocks: u32,
    pub done: bool,
}

pub enum Finish {
    File { name: String, bytes: Vec<u8> },
    /// The file check failed (§11.3); the transfer starts again from new packets.
    ChecksumError,
}

#[derive(Default)]
pub struct Receiver {
    limits: Limits,
    transfers: HashMap<(u16, usize), Inbound>,
    active: Option<(u16, usize)>,
    clock: u64,
}

impl Receiver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limits(limits: Limits) -> Self {
        Receiver { limits, ..Self::default() }
    }

    /// Give one packet (k bytes, the message of a codeword) to the transfer state machine.
    pub fn push(&mut self, packet: &[u8]) -> Action {
        if packet.len() < MIN_K || !packet_intact(packet) {
            return Action::Discard;
        }
        let (id, data) = read_packet(packet);
        let mut desc = None;
        if id.meta {
            if id.sbn != 0 || id.esi != 0 {
                return Action::Discard; // reserved metadata (§8.3)
            }
            match parse_descriptor(data) {
                Ok(d) => desc = Some(d),
                Err(_) => return Action::Discard,
            }
        }
        let key = (id.tid, data.len());
        if !self.transfers.contains_key(&key) && self.transfers.len() >= self.limits.max_transfers {
            if let Some(old) = self.transfers.iter().min_by_key(|(_, t)| t.touched).map(|(k, _)| *k) {
                self.transfers.remove(&old);
            }
        }
        self.clock += 1;
        let limits = self.limits;
        let tr = self.transfers.entry(key).or_insert_with(|| Inbound::new(id.tid, data.len()));
        tr.touched = self.clock;
        self.active = Some(key);

        if let Some(d) = desc {
            if tr.desc.is_some() {
                if tr.desc_bytes == data {
                    return Action::Discard;
                }
                tr.reset(); // another transfer with the same key, or a wrong descriptor (§11.2)
            }
            return accept(tr, d, data, &limits);
        }
        match tr.state {
            State::Collecting => {
                let k = (id.sbn, id.esi);
                if tr.kept.contains_key(&k) || tr.kept.len() >= limits.max_kept {
                    return Action::Discard;
                }
                tr.kept.insert(k, data.to_vec());
                Action::Keep
            }
            State::Receiving => match tr.decs.get_mut(id.sbn as usize) {
                Some(Some(dec)) => {
                    if dec.add(id.esi, data) {
                        Action::Add
                    } else {
                        Action::Discard
                    }
                }
                _ => Action::Discard,
            },
            State::Refused | State::Complete => Action::Discard,
        }
    }

    pub fn progress(&self) -> Option<Progress> {
        let tr = self.transfers.get(self.active.as_ref()?)?;
        Some(Progress {
            tid: tr.tid,
            t: tr.t,
            state: tr.state,
            name: tr.desc.as_ref().map(|d| d.name.clone()),
            size: tr.desc.as_ref().map(|d| d.file_size),
            payload_length: tr.desc.as_ref().map(|d| d.payload_length),
            received: tr.received(),
            needed: tr.plan.as_ref().map(|p| p.kt),
            blocks: tr.plan.as_ref().map_or(0, |p| p.z),
            done: tr.state == State::Complete,
        })
    }

    /// Decode what can be decoded in the active transfer and, when every block is decoded,
    /// do the file check (§11.3). A failed check resets the transfer.
    pub fn try_finish(&mut self) -> Option<Finish> {
        let key = self.active?;
        let tr = self.transfers.get_mut(&key)?;
        if tr.state != State::Receiving {
            return None;
        }
        let plan = tr.plan.clone()?;
        for b in 0..plan.z as usize {
            let Some(dec) = tr.decs[b].as_mut() else { continue };
            if dec.count() < dec.k() as usize || dec.count() == tr.last_try[b] {
                continue;
            }
            tr.last_try[b] = dec.count();
            if let Some(out) = dec.decode() {
                tr.outs[b] = Some(out);
                tr.decs[b] = None;
            }
        }
        if tr.outs.iter().any(|o| o.is_none()) {
            return None;
        }
        let d = tr.desc.clone()?;
        let mut payload = Vec::with_capacity(plan.kt as usize * tr.t);
        for o in tr.outs.iter() {
            payload.extend_from_slice(o.as_ref().unwrap());
        }
        payload.truncate(d.payload_length as usize);
        let bytes = if d.flags & FLAG_DEFLATE != 0 { deflate::inflate(&payload, d.file_size as usize) } else { Some(payload) };
        match bytes {
            Some(bytes) if bytes.len() == d.file_size as usize && crc32(&bytes) == d.crc => {
                tr.state = State::Complete;
                tr.decs = Vec::new();
                tr.outs = Vec::new();
                Some(Finish::File { name: d.name, bytes })
            }
            _ => {
                tr.reset();
                Some(Finish::ChecksumError)
            }
        }
    }
}

/// Accept a descriptor (§11.2): plan the blocks, refuse a transfer over the limits, or make
/// the decoders and give them the kept packets.
fn accept(tr: &mut Inbound, d: Descriptor, bytes: &[u8], limits: &Limits) -> Action {
    let Some(plan) = block_plan(d.payload_length as u64, tr.t) else { return Action::Discard };
    tr.desc_bytes = bytes.to_vec();
    let too_large = (d.payload_length.max(d.file_size) as u64) > limits.max_size;
    tr.desc = Some(d);
    if too_large {
        tr.plan = Some(plan);
        tr.state = State::Refused;
        tr.kept.clear();
        return Action::Refuse;
    }
    tr.decs = (0..plan.z).map(|b| Some(Decoder::new(plan.sizes[b as usize], tr.t, fountain_seed(tr.tid, b)))).collect();
    tr.outs = vec![None; plan.z as usize];
    tr.last_try = vec![usize::MAX; plan.z as usize];
    for ((sbn, esi), bytes) in tr.kept.drain() {
        if let Some(Some(dec)) = tr.decs.get_mut(sbn as usize) {
            dec.add(esi, &bytes);
        }
    }
    tr.plan = Some(plan);
    tr.state = State::Receiving;
    Action::Accept
}
