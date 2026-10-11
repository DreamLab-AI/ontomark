//! `ontomark-compile` — Markdown ontology vaults to standard artefacts.
//!
//! Pages in, one generation out: RDF 1.1 Turtle (OWL 2 EL axioms, SKOS labels,
//! DCTERMS, PROV-O attribution), JSON-LD 1.1, SHACL shapes for the validation
//! rules, the NGG1 graph tiers, the page API, the search, scaffold and prose
//! indexes and the OKF bundle. Corpus rules (frontmatter grammar, IRIs,
//! taxonomy, attribution) come from a [`profile::Profile`]; the estate profile
//! lives in `profiles/estate`.
//!
//! The pipeline is relocated from VisionClaw's `vault build` at tag
//! `ontomark-census-baseline` (`1044b0722`) and is held byte-identical to it
//! by the conformance suite (`conformance/`).

#![allow(clippy::module_name_repetitions)]

pub mod build;
pub mod check;
pub mod input;
pub mod ir;
pub mod json;
pub mod reason;
#[cfg(feature = "whelk-el")]
pub mod whelk;
