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
//!
//! # Vault layout
//!
//! A vault is an ordinary Obsidian vault (ADR-OM-007): pages sit at the vault
//! root and a page's id is its root-relative path without `.md`
//! (`ETSI/Domain.md` is `ETSI/Domain`). Top-level folders that hold Obsidian
//! furniture rather than pages — [`input::page::EXCLUDED_DIRS`] (`templates`,
//! `bases`, `assets`, `journals`), or the profile's `layout.excluded` in their
//! place — are skipped by the page walk and reported as
//! [`input::page::SkipReason::ExcludedDirectory`], never silently dropped;
//! dot-folders (`.obsidian/`) are hidden regardless. `journals/` is loaded as
//! its own tree.
//!
//! Where the vaults sit under the repository root is the profile's
//! [`profile::Layout`] (ADR-OM-008): `layout.knowledge` (default `knowledge`)
//! and `layout.working` (default `working`, or `null` for a layout with no
//! working vault). `knowledge: "."` means the repository root *is* the vault —
//! a plain Obsidian vault opened directly, which is how `ontomark` treats a
//! directory with no `ontology/vocabulary.yaml` above it. Code that needs a
//! vault's folder asks the layout ([`profile::Layout::root_of`]); a role the
//! layout does not place is refused by name through
//! [`profile::MissingVault`].
//!
//! The **output** contract does not move with the input layout: knowledge
//! pages stage under `publish/pages/`, working pages under `publish/working/`,
//! and every report, digest path and staging count is keyed by the role name
//! ([`input::page::VaultKind::role_name`]: `knowledge`, `working`), whatever
//! folders the pages came from. A build's generation id is `<prefix>@<commit>`
//! ([`build::generation::generation_id`]); the prefix is the profile's
//! [`profile::GenerationNaming`] — pinned to `visionGraph` by the estate
//! profile, the repository root's directory name otherwise.
//!
//! ```
//! use ontomark_compile::input::page::VaultKind;
//! use ontomark_compile::profile::Profile;
//!
//! // A profile for a vault that keeps its notes at the root and has no
//! // working vault: the shape `ontomark --repo <vault>` selects on its own.
//! let p = Profile::from_yaml("layout:\n  knowledge: .\n  working: null\n").unwrap();
//! assert_eq!(p.layout.dir_of(VaultKind::Knowledge), Some("."));
//! assert_eq!(p.layout.dir_of(VaultKind::Working), None);
//! // The role name is the output key and never moves.
//! assert_eq!(VaultKind::Knowledge.role_name(), "knowledge");
//! ```

#![allow(clippy::module_name_repetitions)]

pub mod build;
pub mod bundle;
pub mod check;
pub mod input;
pub mod ir;
pub mod json;
pub mod jsonld;
pub mod migrate;
/// The NGG1 graph-tier format: constants, encoder and zero-copy reader.
pub use ontomark_ngg1 as ngg1;
pub mod profile;
#[cfg(feature = "model-projection")]
pub mod projection;
pub mod reason;
pub mod shacl;
#[cfg(feature = "whelk-el")]
pub mod whelk;
pub mod write;
