//! How many cells the sender's screen carries (the web demo's computeGeometry).
//!
//! The code fills the space it is given, in its own shape, up to an aspect ratio of
//! [`MAX_ASPECT`]. The cell size follows the physical size of the screen: about 0.75 mm per
//! cell, clamped to what a 1080p camera resolves across its short side, scaled by the
//! density choice.

use lumen::layout::{MAX_DIM, MIN_DIM};
use serde::{Deserialize, Serialize};

/// Cells of quiet zone on each side of the grid.
pub const QUIET: usize = 2;
/// Beyond this aspect ratio a camera would see the code too thin; the rest stays empty.
pub const MAX_ASPECT: f64 = 2.4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Low,
    #[default]
    Auto,
    High,
    Max,
}

impl Density {
    pub fn factor(self) -> f64 {
        match self {
            Density::Low => 0.72,
            Density::Auto => 1.0,
            Density::High => 1.3,
            Density::Max => 1.65,
        }
    }
}

/// The space the code may use, in device pixels, and how big a device pixel is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    /// Usable width and height for the code in device pixels (safe area already removed).
    pub width_px: f64,
    pub height_px: f64,
    /// Device pixels per point (dp / CSS pixel).
    pub scale: f64,
    /// Physical size of one device pixel in millimetres.
    pub mm_per_px: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Viewport { width_px: 1170.0, height_px: 2400.0, scale: 3.0, mm_per_px: 0.055 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    /// Device pixels per cell.
    pub s: usize,
    /// Grid size in cells (without the quiet zone).
    pub x: usize,
    pub y: usize,
    /// Rendered size in device pixels, quiet zone included.
    pub width_px: usize,
    pub height_px: usize,
}

pub fn compute(vp: &Viewport, density: Density) -> Geometry {
    let scale = vp.scale.max(0.5);
    let pad = 6.0 * scale;
    let dev_w = (vp.width_px - 2.0 * pad).max(100.0 * scale);
    let dev_h = (vp.height_px - 2.0 * pad).max(100.0 * scale);
    let box_w = dev_w.min(dev_h * MAX_ASPECT).floor();
    let box_h = dev_h.min(dev_w * MAX_ASPECT).floor();
    let short_mm = box_w.min(box_h) * vp.mm_per_px;
    let base = (short_mm / 0.75).round().clamp(52.0, 150.0);
    let target = (base * density.factor()).round();
    let short_dev = box_w.min(box_h);
    let mut s = ((short_dev / (target + 2.0 * QUIET as f64)).round() as usize).max(3);
    let fit = |s: usize| -> (usize, usize) {
        let mut x = (box_w as usize / s).saturating_sub(2 * QUIET);
        let mut y = (box_h as usize / s).saturating_sub(2 * QUIET);
        if x as f64 > y as f64 * MAX_ASPECT {
            x = (y as f64 * MAX_ASPECT).floor() as usize;
        }
        if y as f64 > x as f64 * MAX_ASPECT {
            y = (x as f64 * MAX_ASPECT).floor() as usize;
        }
        (x, y)
    };
    let (mut x, mut y) = fit(s);
    while (x > MAX_DIM || y > MAX_DIM) && s < 64 {
        s += 1;
        (x, y) = fit(s);
    }
    while (x < MIN_DIM || y < MIN_DIM) && s > 2 {
        s -= 1;
        (x, y) = fit(s);
    }
    let x = x.clamp(MIN_DIM, MAX_DIM);
    let y = y.clamp(MIN_DIM, MAX_DIM);
    Geometry { s, x, y, width_px: (x + 2 * QUIET) * s, height_px: (y + 2 * QUIET) * s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_portrait_matches_the_web_demo() {
        // iPhone-class screen: 390×844 pt at 3×, 0.165 mm per point
        let vp = Viewport { width_px: 1170.0, height_px: 2532.0, scale: 3.0, mm_per_px: 0.055 };
        let g = compute(&vp, Density::Auto);
        assert!(g.y > g.x, "portrait code on a portrait screen");
        assert!((52..=90).contains(&g.x), "x = {}", g.x);
        assert!(g.width_px as f64 <= vp.width_px && g.height_px as f64 <= vp.height_px);
        assert!(g.y as f64 <= g.x as f64 * MAX_ASPECT);
    }

    #[test]
    fn laptop_landscape() {
        let vp = Viewport { width_px: 2880.0, height_px: 1800.0, scale: 2.0, mm_per_px: 0.125 };
        let g = compute(&vp, Density::Auto);
        assert!(g.x > g.y);
        let hi = compute(&vp, Density::Max);
        assert!(hi.x > g.x && hi.s <= g.s);
    }
}

