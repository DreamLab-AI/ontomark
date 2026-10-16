//! Guarded writes to a vault: `edit`, `create` and `apply`, plus the
//! [`proposal::PatchProposal`] they land and the exact unified-diff applier
//! that is the staleness guard.
//!
//! One rule runs through every module here, borrowed from IWE's blast-radius
//! guards: **a write that has not declared its blast radius is refused.** The
//! caller says `--expect docs=1,blocks=2`; if the write would touch a
//! different number of documents or keys, nothing is written and the refusal
//! names the guard that was wrong. Every refusal is a typed error with a stable
//! `code()` string, so a `--json` consumer reads `code`, `message` and
//! `blockers` rather than parsing prose.
//!
//! | module | verb | what it guarantees |
//! |---|---|---|
//! | [`edit`] | `ontomark edit` | key mutation on one existing page; refused without `--expect` |
//! | [`create`] | `ontomark create` | a new page written once, `O_EXCL`; never overwrites |
//! | [`apply`] | `ontomark apply` | an approved amendment's diff applied exactly, atomically |
//! | [`diff`] | — | `unified_diff` and its strict inverse, no fuzz, no offset search |
//! | [`proposal`] | — | the C4 `PatchProposal` and its digest, bit-identical to `vault propose`'s |
//!
//! # Provenance
//!
//! Ported from VisionClaw `crates/vault/src/{edit,create,apply,diff}.rs` and
//! `crates/vault-core/src/proposal.rs` (DEPRECATION rows VC-29 and VC-30,
//! ADR-OM-005 decision 5). Those files are DreamLab-authored and covered by the
//! relicensing grant recorded in this repository's `NOTICE`, so this is a port
//! onto ontomark's typed model, not a clean-room rewrite; the logic, error
//! codes and JSON shapes are kept identical so the agentbox callers
//! (`management-api/lib/ontology-apply.js`) switch binaries without a change.
//! The forum transport that posts a proposal (`vault propose`, `nostr.rs`)
//! stays AGPL in VisionClaw.

pub mod apply;
pub mod create;
pub mod diff;
pub mod edit;
pub mod proposal;
