//! Lumen Code app core, shared by the iOS, Android and desktop shells.
//!
//! - [`app`] — the Crux state machine: every decision and every string on screen
//! - [`runtime`] — runs the core and the real-time work it asks for (sender stream, scan
//!   pipeline, decoder thread, timers) and talks to a platform [`runtime::Shell`]
//! - [`engine`] — the real-time parts themselves
//! - [`geometry`], [`content`], [`format`], [`i18n`] — rules the core and the shells share
//!
//! The wire protocol itself is the zero-dependency `lumen-code` crate (`src/rust`).

pub mod app;
pub mod content;
pub mod engine;
pub mod format;
pub mod geometry;
pub mod i18n;
pub mod runtime;

pub use app::{Event, Platform, Screen, ViewModel};
pub use engine::image::{FrameRef, PixelFormat, Plane};
pub use runtime::{Runtime, Shell, ShellRequest, Update};
