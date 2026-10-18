//! The sender's frame source: the fountain-coded stream of the loaded file, turned into
//! one-pixel-per-cell images. A shell scales that image up with nearest-neighbour sampling
//! (or draws the cells as rounded quads) and puts the static mask with the finders, the
//! markers and the gaps between cells on top, so per frame only a few thousand pixels change.

use crate::geometry::{Geometry, QUIET};
use lumen::layout::{K_DATA, K_HEADER, K_MARKER};
use lumen::render::cell_style;
use lumen::transfer::{Prepared, Sender};
use lumen::Renderer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// What the core learns about a sender it asked for.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SenderInfo {
    pub ok: bool,
    /// The previous transfer (and the receiver's progress) was kept.
    pub kept: bool,
    pub tid: u16,
    pub mode: u8,
    pub ecc: u8,
    pub x: u32,
    pub y: u32,
    pub bpc: u32,
    pub bytes_per_frame: u32,
    pub payload_len: u64,
}

/// One frame: (X + 2q) × (Y + 2q) RGBA pixels, one per cell.
#[derive(Clone, Debug)]
pub struct CellFrame {
    pub seq: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The parts of the picture that never change during a transfer, for shells that draw
/// vectors (desktop): positions in cells, quiet zone excluded.
#[derive(Clone, Debug, Default)]
pub struct StaticLayout {
    pub x: usize,
    pub y: usize,
    pub quiet: usize,
    pub s: usize,
    /// Gap and corner radius of a data cell in device pixels.
    pub gap: usize,
    pub radius: usize,
    /// Per cell: true for data and header cells (drawn from the frame's colours).
    pub data: Vec<bool>,
    /// Top-left of the three finders and of the corner pattern (7×7 each).
    pub finders: [(usize, usize); 3],
    pub corner: (usize, usize),
    /// Centres of the alignment markers.
    pub markers: Vec<(usize, usize)>,
    pub ink: [u8; 3],
    pub paper: [u8; 3],
}

pub struct SendEngine {
    pub sender: Sender,
    pub renderer: Renderer,
    pub geometry: Geometry,
    pixels: Vec<u32>,
    build_ms: Vec<f64>,
}

impl SendEngine {
    pub fn new(prepared: &Prepared, mode: usize, ecc: usize, g: Geometry, tid: u16) -> Option<SendEngine> {
        let sender = Sender::new(prepared, mode, ecc, g.x, g.y, tid, 0)?;
        Some(Self::wrap(sender, g))
    }

    fn wrap(sender: Sender, g: Geometry) -> SendEngine {
        let renderer = Renderer::new(&sender.layout, sender.mode, g.s, QUIET);
        let n = (g.x + 2 * QUIET) * (g.y + 2 * QUIET);
        SendEngine { sender, renderer, geometry: g, pixels: vec![0; n], build_ms: Vec::new() }
    }

    pub fn info(&self, kept: bool, payload_len: u64) -> SenderInfo {
        SenderInfo {
            ok: true,
            kept,
            tid: self.sender.tid,
            mode: self.sender.mode as u8,
            ecc: self.sender.ecc as u8,
            x: self.sender.layout.x as u32,
            y: self.sender.layout.y as u32,
            bpc: lumen::MODES[self.sender.mode].bpc,
            bytes_per_frame: self.sender.bytes_per_frame() as u32,
            payload_len,
        }
    }

    /// New grid size, same stream.
    pub fn resize(mut self, g: Geometry) -> Result<SendEngine, SendEngine> {
        if !self.sender.resize(g.x, g.y) {
            return Err(self);
        }
        let build_ms = std::mem::take(&mut self.build_ms);
        let mut e = Self::wrap(self.sender, g);
        e.build_ms = build_ms;
        Ok(e)
    }

    /// New colours / ECC on the same stream when the packet size allows it.
    pub fn reconfigure(mut self, mode: usize, ecc: usize, g: Geometry) -> Result<SendEngine, SendEngine> {
        if !self.sender.reconfigure(mode, ecc) {
            return Err(self);
        }
        if (g.x, g.y) != (self.sender.layout.x, self.sender.layout.y) && !self.sender.resize(g.x, g.y) {
            return Err(self);
        }
        Ok(Self::wrap(self.sender, g))
    }

    pub fn next_frame(&mut self) -> CellFrame {
        let t0 = Instant::now();
        let frame = self.sender.next_frame();
        self.renderer.cell_pixels(&frame.cells, &mut self.pixels);
        self.build_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        if self.build_ms.len() > 30 {
            self.build_ms.remove(0);
        }
        let w = (self.geometry.x + 2 * QUIET) as u32;
        let h = (self.geometry.y + 2 * QUIET) as u32;
        let mut rgba = Vec::with_capacity(self.pixels.len() * 4);
        for p in &self.pixels {
            rgba.extend_from_slice(&p.to_le_bytes());
        }
        CellFrame { seq: frame.seq, width: w, height: h, rgba }
    }

    pub fn build_ms(&self) -> f64 {
        if self.build_ms.is_empty() {
            0.0
        } else {
            self.build_ms.iter().sum::<f64>() / self.build_ms.len() as f64
        }
    }

    /// Full-resolution RGBA mask: finders, markers and cell gaps opaque; cell interiors
    /// transparent (premultiplied and straight alpha are the same here).
    pub fn mask(&self) -> (u32, u32, Arc<Vec<u8>>) {
        let px = self.renderer.mask_layer();
        let mut out = Vec::with_capacity(px.len() * 4);
        for p in px {
            out.extend_from_slice(&p.to_le_bytes());
        }
        (self.renderer.width as u32, self.renderer.height as u32, Arc::new(out))
    }

    pub fn static_layout(&self) -> StaticLayout {
        let l = &self.sender.layout;
        let (gap, radius) = cell_style(self.geometry.s);
        let (nx, ny) = (l.lx.len(), l.ly.len());
        let mut markers = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let corner = (i == 0 || i == nx - 1) && (j == 0 || j == ny - 1);
                if !corner {
                    markers.push((l.lx[i], l.ly[j]));
                }
            }
        }
        debug_assert!(markers.iter().all(|&(x, y)| l.kind[y * l.x + x] == K_MARKER));
        StaticLayout {
            x: l.x,
            y: l.y,
            quiet: QUIET,
            s: self.geometry.s,
            gap,
            radius,
            data: l.kind.iter().map(|&k| k == K_DATA || k == K_HEADER).collect(),
            finders: [(0, 0), (l.x - 7, 0), (0, l.y - 7)],
            corner: (l.x - 7, l.y - 7),
            markers,
            ink: lumen::layout::INK,
            paper: lumen::layout::PAPER,
        }
    }
}
