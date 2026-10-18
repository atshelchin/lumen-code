//! The real-time half: work that runs at frame rate and must not wait on the core's lock or
//! cross a JSON boundary. The core decides *what* happens; these do it.

pub mod bench;
pub mod image;
pub mod recv;
pub mod send;
pub mod sink;
