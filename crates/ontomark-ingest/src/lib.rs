//! Assisted ingest for Ontomark (ADR-OM-003): a pure, host-driven contract
//! that turns a model's proposed names and values into evidence-bearing,
//! guarded Markdown patch candidates.
//!
//! The crate performs no I/O and never calls a model. A host drives two
//! calls:
//!
//! 1. [`Ingest::plan`] reads one note through its [`Profile`] and the vault
//!    [`Model`] and returns a [`ModelRequest`]: the instructions (versioned
//!    prompt data, not code), the heading-aware [`Chunk`]s of the body, the
//!    frontmatter keys the model may fill ([`KeySpec`]) and the JSON schema
//!    its answer must satisfy. The host sends it to whatever model it chooses.
//! 2. [`Ingest::accept`] takes the [`ModelResponse`]s back and returns
//!    [`Candidates`]: one [`ontomark_parse::Patch`] bound to the source digest
//!    the request was planned against, an [`EvidenceSpan`] for every accepted
//!    value, and a [`Withheld`] record with a reason for everything refused.
//!
//! Following SPIRES (Caufield et al., 2024), the model is asked for **names,
//! never identifiers**. Names are grounded against the vault vocabulary: an
//! exact label, alias or note-name match first, then the lexical alias tiers
//! adapted from OntoCast's merge guards ([`similarity`]), with Jaro-Winkler
//! near-misses reported as suggestions when nothing grounds. An ungrounded,
//! ambiguous or out-of-vocabulary name yields a diagnostic, never an edit.
//!
//! Every accepted value carries the retired RDF-to-Logseq adapter's starting
//! provenance: [`CONFIDENCE`] and [`METHOD`]. Ingest never raises them; a
//! reviewer does.
//!
//! ```
//! use ontomark_ingest::{Ingest, ModelResponse, Options, ProposedAssertion};
//! use ontomark_model::{DeclarativeProfile, DocId, Model, ProfileSpec};
//! use ontomark_parse::Document;
//!
//! // A profile that maps one frontmatter key to a predicate.
//! let spec: ProfileSpec = serde_json::from_value(serde_json::json!({
//!     "id": "demo/1",
//!     "kinds": { "Class": "class" },
//!     "predicates": { "is-a": { "iri": "rdfs:subClassOf", "form": "sub_class_of" } }
//! })).unwrap();
//! let profile = DeclarativeProfile::new(spec).unwrap();
//!
//! let graph = Document::parse("---\ntype: Class\niri: urn:x:Graph\n---\n");
//! let kg = Document::parse("---\ntype: Class\niri: urn:x:KG\n---\nA knowledge graph is a graph.\n");
//! let model = Model::build(&profile, [
//!     (DocId::new("Graph.md"), &graph),
//!     (DocId::new("Knowledge graph.md"), &kg),
//! ]);
//!
//! let ingest = Ingest::new(&profile, &model, Options::default());
//! let request = ingest.plan(&DocId::new("Knowledge graph.md"), &kg).unwrap();
//! assert_eq!(request.chunks.len(), 1);
//!
//! // The host's model answers with names and verbatim evidence.
//! let response = ModelResponse { chunk: 0, assertions: vec![ProposedAssertion {
//!     key: "is-a".into(), value: "graph".into(), evidence: "is a graph".into(),
//! }]};
//! let out = ingest.accept(&kg, &request, &[response]).unwrap();
//! assert_eq!(out.accepted.len(), 1);
//! assert_eq!(out.accepted[0].written, "[[Graph]]");
//! // The patch is bound to the note's digest and writes only the new key.
//! let patched = kg.apply(out.patch.as_ref().unwrap()).unwrap();
//! assert!(patched.document.source().contains("iri: urn:x:KG\nis-a:\n- '[[Graph]]'\n---\n"));
//! assert_eq!(patched.document.body(), kg.body());
//! ```
//!
//! The crate is portable: no filesystem, clock, network or threads, so it
//! builds for `wasm32-unknown-unknown` unchanged.

mod accept;
mod contract;
mod error;
mod vocabulary;

pub mod chunk;
pub mod normalize;
pub mod similarity;

pub use accept::{
    codes, CandidateAssertion, Candidates, EvidenceSpan, IngestDiagnostic, Utf16Span, Withheld,
    WithholdReason, CONFIDENCE, METHOD,
};
pub use chunk::Chunk;
pub use contract::{
    KeySpec, ModelRequest, ModelResponse, NoteRef, ProposedAssertion, ValueKind, CONTRACT,
    INSTRUCTIONS,
};
pub use error::IngestError;
pub use similarity::{AliasTier, LexicalThresholds};
pub use vocabulary::{Grounding, Suggestion, Term, Vocabulary};

use ontomark_model::{DocId, Model, Profile};
use ontomark_parse::Document;
use serde::{Deserialize, Serialize};

/// Tunable limits. Defaults follow OntoCast's chunking and lexical settings.
/// Every field has a default, so a host may pass a partial object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Chunks shorter than this merge with their neighbour (OntoCast
    /// `CHUNK_MIN_SIZE`, 3000 characters).
    pub min_chunk_chars: usize,
    /// Chunks longer than this split at paragraph, then line, boundaries
    /// (OntoCast `CHUNK_MAX_SIZE`, 12000 characters).
    pub max_chunk_chars: usize,
    /// Thresholds for the lexical grounding tiers.
    pub lexical: LexicalThresholds,
    /// How many near-miss suggestions to report for an ungrounded name.
    pub suggestions: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            min_chunk_chars: 3000,
            max_chunk_chars: 12000,
            lexical: LexicalThresholds::default(),
            suggestions: 3,
        }
    }
}

/// The ingest engine for one vault state: a profile, a built model and the
/// vocabulary derived from them.
pub struct Ingest<'a> {
    profile: &'a dyn Profile,
    model: &'a Model,
    vocabulary: Vocabulary,
    options: Options,
}

impl<'a> Ingest<'a> {
    /// Build the engine. The vocabulary is derived once from `model`.
    #[must_use]
    pub fn new(profile: &'a dyn Profile, model: &'a Model, options: Options) -> Self {
        let vocabulary = Vocabulary::from_model(profile, model);
        Self {
            profile,
            model,
            vocabulary,
            options,
        }
    }

    /// The vocabulary the model is allowed to use.
    #[must_use]
    pub fn vocabulary(&self) -> &Vocabulary {
        &self.vocabulary
    }

    /// The options in force.
    #[must_use]
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// Plan an extraction request for one note.
    ///
    /// # Errors
    /// [`IngestError::NotAnEntity`] when the note declares no entity in the
    /// model; [`IngestError::NoAssertionKeys`] when the profile declares no
    /// predicate keys the model could fill.
    pub fn plan(&self, id: &DocId, doc: &Document) -> Result<ModelRequest, IngestError> {
        contract::plan(self.profile, self.model, &self.options, id, doc)
    }

    /// Turn the model's responses to `request` into guarded candidates.
    ///
    /// # Errors
    /// [`IngestError::StaleRevision`] when `doc` no longer matches the digest
    /// the request was planned against; [`IngestError::ContractMismatch`]
    /// when the request was produced by another contract version;
    /// [`IngestError::UnknownChunk`] when a response names a chunk the
    /// request does not have; [`IngestError::Patch`] when the resulting patch
    /// fails its own proof.
    pub fn accept(
        &self,
        doc: &Document,
        request: &ModelRequest,
        responses: &[ModelResponse],
    ) -> Result<Candidates, IngestError> {
        accept::accept(
            self.profile,
            self.model,
            &self.vocabulary,
            &self.options,
            doc,
            request,
            responses,
        )
    }
}

impl std::fmt::Debug for Ingest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ingest")
            .field("profile", &self.profile.id())
            .field("terms", &self.vocabulary.terms().len())
            .field("options", &self.options)
            .finish()
    }
}
