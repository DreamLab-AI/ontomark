//! Checks over a vault: schema and policy validation, semantic conflicts, the
//! continuation gate and the Logseq-body residue check.
//!
//! Relocated from VisionClaw `crates/vault/src/{validate,conflicts,gate,bodies,repair}.rs`
//! at `1044b0722`.

pub mod bodies;
pub mod conflicts;
pub mod gate;
pub mod repair;
pub mod validate;
