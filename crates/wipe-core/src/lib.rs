//! Core storage engine and domain model for **wipe**.
//!
//! `wipe-core` owns everything that touches a `.wipe` board on disk: the domain
//! model ([`model`]), deterministic JSON persistence and project discovery
//! ([`store`]), stable ID formatting ([`id`]), and the error type ([`error`]).
//!
//! Every mutation of a `.wipe` directory in the entire project MUST go through this
//! crate so that serialization stays deterministic (stable key order, trailing
//! newline, atomic writes) and git diffs remain minimal and merge-friendly.

pub mod error;
pub mod git;
pub mod id;
pub mod model;
pub mod ops;
pub mod store;

pub use error::{Error, Result};
pub use store::{Store, WIPE_DIR};
