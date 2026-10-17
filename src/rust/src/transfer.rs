//! Sender (file -> frames) and Receiver (packets -> file), PROTOCOL §9–§10.

use crate::deflate;
use crate::fountain::{Decoder, Encoder};
use crate::frame::{
    decode_descriptor, encode_descriptor, encode_frame_cells, packet_data_size, read_packet, seal_packet, write_packet_header,
    Descriptor, FLAG_DEFLATE, MAX_SBN, PKT_HDR, PKT_OVERHEAD,
};
use crate::layout::{choose_codeword, codewords_per_frame, Layout, ECC_PERCENT};
use crate::util::{crc32, mix32};
use std::collections::HashMap;

pub const MAX_BLOCK_SYMBOLS: u32 = 16384;

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
    let z = kt.div_ceil(MAX_BLOCK_SYMBOLS as u64);
    if z > MAX_SBN as u64 + 1 {
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
    let descriptor = Descriptor { flags, payload_length: payload.len() as u32, file_size: file.len() as u32, crc, name: name.into() };
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
}

impl Sender {
    pub fn new(prepared: &Prepared, mode: usize, ecc: usize, x: usize, y: usize, tid: u16, seq: u32) -> Option<Sender> {
        let layout = Layout::new(x, y)?;
        let (n, nsym) = choose_codeword(&layout, mode, ecc);
        let t = packet_data_size(n, nsym);
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
        Some(Sender { tid, mode, ecc, n, nsym, t, seq: seq & 255, layout, per_frame, plan, encs, desc, counter: 0, frames: 0 })
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

    pub fn next_packets(&mut self) -> Vec<Vec<u8>> {
        let k = self.n - self.nsym;
        let m = self.per_frame;
        let with_meta = self.k() as usize <= 2 * m || self.frames % 4 == 0;
        let z = self.plan.z as u64;
        let mut msgs = Vec::with_capacity(m);
        for i in 0..m {
            let mut msg = vec![0u8; k];
            if with_meta && i == 0 {
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

struct Inbound {
    tid: u16,
    t: usize,
    desc: Option<Descriptor>,
    plan: Option<BlockPlan>,
    pending: HashMap<(u32, u32), Vec<u8>>,
    decs: Vec<Option<Decoder>>,
    outs: Vec<Option<Vec<u8>>>,
    last_try: Vec<usize>,
    done: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub tid: u16,
    pub name: Option<String>,
    pub size: Option<u32>,
    pub received: u32,
    pub needed: Option<u32>,
    pub blocks: u32,
    pub done: bool,
    pub t: usize,
}

pub enum Finish {
    File { name: String, bytes: Vec<u8> },
    ChecksumError,
}

#[derive(Default)]
pub struct Receiver {
    transfers: HashMap<(u16, usize), Inbound>,
    active: Option<(u16, usize)>,
}

impl Receiver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, packet: &[u8]) {
        if packet.len() < PKT_OVERHEAD + 1 {
            return;
        }
        let (id, data) = read_packet(packet);
        let key = (id.tid, data.len());
        let tr = self.transfers.entry(key).or_insert_with(|| Inbound {
            tid: id.tid,
            t: data.len(),
            desc: None,
            plan: None,
            pending: HashMap::new(),
            decs: Vec::new(),
            outs: Vec::new(),
            last_try: Vec::new(),
            done: false,
        });
        self.active = Some(key);
        if tr.done {
            return;
        }
        if id.meta {
            if tr.desc.is_none() && id.esi == 0 {
                if let Some(d) = decode_descriptor(data) {
                    let Some(plan) = block_plan(d.payload_length as u64, tr.t) else { return };
                    tr.decs = (0..plan.z).map(|b| Some(Decoder::new(plan.sizes[b as usize], tr.t, fountain_seed(tr.tid, b)))).collect();
                    tr.outs = vec![None; plan.z as usize];
                    tr.last_try = vec![usize::MAX; plan.z as usize];
                    for ((sbn, esi), bytes) in tr.pending.drain() {
                        if let Some(Some(dec)) = tr.decs.get_mut(sbn as usize) {
                            dec.add(esi, &bytes);
                        }
                    }
                    tr.plan = Some(plan);
                    tr.desc = Some(d);
                }
            }
        } else if tr.plan.is_some() {
            if let Some(Some(dec)) = tr.decs.get_mut(id.sbn as usize) {
                dec.add(id.esi, data);
            }
        } else if tr.pending.len() < 200_000 {
            tr.pending.entry((id.sbn, id.esi)).or_insert_with(|| data.to_vec());
        }
    }

    pub fn progress(&self) -> Option<Progress> {
        let tr = self.transfers.get(self.active.as_ref()?)?;
        let received = match &tr.plan {
            None => tr.pending.len() as u32,
            Some(plan) => (0..plan.z as usize)
                .map(|b| if tr.outs[b].is_some() { plan.sizes[b] } else { tr.decs[b].as_ref().map_or(0, |d| d.count() as u32) })
                .sum(),
        };
        Some(Progress {
            tid: tr.tid,
            name: tr.desc.as_ref().map(|d| d.name.clone()),
            size: tr.desc.as_ref().map(|d| d.file_size),
            received,
            needed: tr.plan.as_ref().map(|p| p.kt),
            blocks: tr.plan.as_ref().map_or(0, |p| p.z),
            done: tr.done,
            t: tr.t,
        })
    }

    /// Try to complete the active transfer.
    pub fn try_finish(&mut self) -> Option<Finish> {
        let key = self.active?;
        let tr = self.transfers.get_mut(&key)?;
        if tr.done {
            return None;
        }
        let plan = tr.plan.clone()?;
        for b in 0..plan.z as usize {
            if tr.outs[b].is_some() {
                continue;
            }
            let dec = tr.decs[b].as_mut()?;
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
        let mut all = Vec::with_capacity(plan.kt as usize * tr.t);
        for o in tr.outs.iter() {
            all.extend_from_slice(o.as_ref().unwrap());
        }
        let d = tr.desc.as_ref()?;
        all.truncate(d.payload_length as usize);
        let bytes = if d.flags & FLAG_DEFLATE != 0 {
            match deflate::inflate(&all, d.file_size as usize) {
                Some(b) => b,
                None => return Some(Finish::ChecksumError),
            }
        } else {
            all
        };
        if bytes.len() != d.file_size as usize || crc32(&bytes) != d.crc {
            return Some(Finish::ChecksumError);
        }
        tr.done = true;
        Some(Finish::File { name: d.name.clone(), bytes })
    }
}
