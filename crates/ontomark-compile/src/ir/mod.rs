//! The resolved compile model: records, the public projection and the
//! structural closure every artefact is a pure function of.
//!
//! Relocated from VisionClaw `crates/vault/src/{model,projection,closure}.rs`
//! at `1044b0722`. The resolution rules here are the **legacy** ones (first
//! claimant, synthesised references, IRI-tail index) because the golden
//! artefacts encode them; ADR-2015's explicit resolution outcomes arrive with
//! `ontomark-model` and are recorded as divergences when adopted.

pub mod closure;
pub mod graph;
pub mod model;
pub mod projection;
