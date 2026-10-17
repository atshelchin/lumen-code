//! # Lumen Code v1
//!
//! High-throughput screen-to-camera data transfer: an animated colour 2-D code with
//! Reed–Solomon codewords and a Raptor-style fountain code. Zero dependencies.
//!
//! * [`transfer::Sender`] turns a file into an endless stream of frames (cell grids);
//!   [`render::Renderer`] rasterises a frame to RGBA.
//! * [`scan::Scanner`] turns a camera image into packets;
//!   [`transfer::Receiver`] turns packets back into the file.
//!
//! The wire format is specified in `docs/PROTOCOL.md`; `test/vectors/test-vectors.json` holds
//! conformance vectors.

// Index loops mirror the specification's pseudocode; keep the MSRV-friendly forms.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments, clippy::manual_is_multiple_of)]

pub mod deflate;
pub mod fountain;
pub mod frame;
pub mod layout;
pub mod render;
pub mod rs;
pub mod scan;
pub mod transfer;
pub mod util;

#[cfg(any(target_arch = "wasm32", feature = "ffi"))]
pub mod ffi;

pub use layout::{Header, Layout, MODES};
pub use render::Renderer;
pub use scan::{ScanResult, Scanner};
pub use transfer::{prepare, Finish, Receiver, Sender};
