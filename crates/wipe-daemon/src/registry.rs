//! The project registry now lives in `wipe-core` so the CLI (`wipe scan`) and the
//! daemon share one implementation and one on-disk file. Re-exported here for the
//! daemon's existing call sites.

pub use wipe_core::registry::{default_scan_roots, list, prune, register, scan, ProjectEntry};
