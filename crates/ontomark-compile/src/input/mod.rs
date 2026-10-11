//! Vault input: pages, frontmatter, vocabulary and the OKF lifecycle types.
//!
//! **Temporary stand-in for WP-A.** Relocated from VisionClaw
//! `crates/vault-core/src` at `1044b0722` (tag `ontomark-census-baseline`) so
//! the compiler can be proven byte-identical before `ontomark-parse` and
//! `ontomark-model` land. Removal criterion: the compiler's input adapter reads
//! `ontomark-model` and the conformance suite stays green; this module is then
//! deleted (EXP-10).

pub mod code;
pub mod consistency;
pub mod definition;
pub mod error;
pub mod frontmatter;
pub mod okf;
pub mod page;
pub mod promotion;
pub mod secrets;
pub mod slug;
pub mod vocabulary;
