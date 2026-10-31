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
//! ambiguous or out-of-vocabulary name is withheld with its reason, never
//! written.
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
//! let response = ModelResponse::new(0, vec![ProposedAssertion::new(
//!     "is-a", "graph", "is a graph",
//! )]);
//! let out = ingest.accept(&kg, &request, &[response]).unwrap();
//! assert_eq!(out.accepted.len(), 1);
//! assert_eq!(out.accepted[0].written, "[[Graph]]");
//! // The patch is bound to the note's digest and writes only the new key.
//! let patched = kg.apply(out.patch.as_ref().unwrap()).unwrap();
//! assert!(patched.document.source().contains("iri: urn:x:KG\nis-a:\n- '[[Graph]]'\n---\n"));
//! assert_eq!(patched.document.body(), kg.body());
//! ```
//!
//! # Evidence from outside the vault
//!
//! [`Ingest::plan_source`] and [`Ingest::accept_source`] take a **target**
//! note and external [`SourceText`]. Evidence is found in the source, the
//! patch lands on the target, and every [`EvidenceSpan`] names its source
//! and version. The host owns reading the source and deciding what to do with
//! the patch; ADR-OM-010 has it propose, not write.
//!
//! ```
//! use ontomark_ingest::{Ingest, ModelResponse, Options, ProposedAssertion, SourceText};
//! use ontomark_model::{DeclarativeProfile, DocId, Model, ProfileSpec};
//! use ontomark_parse::Document;
//!
//! let spec: ProfileSpec = serde_json::from_value(serde_json::json!({
//!     "id": "demo/1",
//!     "kinds": { "Class": "class" },
//!     "predicates": { "is-a": { "iri": "rdfs:subClassOf", "form": "sub_class_of" } }
//! })).unwrap();
//! let profile = DeclarativeProfile::new(spec).unwrap();
//! let graph = Document::parse("---\ntype: Class\niri: urn:x:Graph\n---\n");
//! let kg = Document::parse("---\ntype: Class\niri: urn:x:KG\n---\n");
//! let model = Model::build(&profile, [
//!     (DocId::new("Graph.md"), &graph),
//!     (DocId::new("Knowledge graph.md"), &kg),
//! ]);
//! let ingest = Ingest::new(&profile, &model, Options::default());
//!
//! // The source is a document outside the vault; its version is its hash.
//! let source = SourceText::from_text("file:paper.md", "A knowledge graph is a graph.\n");
//! let request = ingest
//!     .plan_source(&DocId::new("Knowledge graph.md"), &kg, &source)
//!     .unwrap();
//! let response = ModelResponse::new(0, vec![ProposedAssertion::new(
//!     "is-a", "graph", "is a graph",
//! )]);
//! let out = ingest.accept_source(&kg, &source, &request, &[response]).unwrap();
//!
//! assert_eq!(out.accepted[0].written, "[[Graph]]");
//! // The evidence points into the source, which it names.
//! let ev = &out.accepted[0].evidence[0];
//! assert_eq!(ev.source.as_ref().unwrap().id, "file:paper.md");
//! assert_eq!(ev.span.slice(&source.text), ev.quote);
//! // The patch changes the target note only.
//! let patched = kg.apply(out.patch.as_ref().unwrap()).unwrap();
//! assert!(patched.document.source().contains("is-a:"));
//! // Counts say which rung decided each value.
//! assert_eq!(out.counts.as_ref().unwrap().names.values().sum::<usize>(), 1);
//! ```
//!
//! The crate is portable: no filesystem, clock, network or threads, so it
//! builds for `wasm32-unknown-unknown` unchanged.

mod accept;
mod contract;
mod error;
mod judge;
mod vocabulary;

pub mod chunk;
pub mod normalize;
pub mod similarity;

pub use accept::{
    codes, CandidateAssertion, Candidates, Counts, EvidenceSpan, IngestDiagnostic, Utf16Span,
    Withheld, WithholdReason, CONFIDENCE, METHOD,
};
pub use chunk::Chunk;
pub use contract::{
    KeySpec, ModelRequest, ModelResponse, NoteRef, ProposedAssertion, SourceRef, SourceText,
    ValueKind, CONTRACT, CONTRACT_SOURCE, INSTRUCTIONS, INSTRUCTIONS_SOURCE,
};
pub use error::IngestError;
pub use judge::{
    CandidateSelector, FactClaim, FactJudge, FieldSuggester, FieldSuggestion, HostCandidate,
    JudgeOutcome, Judged, NoJudge, NoSelector, NoSuggester, RawSuggestion, Selected, Selection,
    SelectionQuestion, SuggestionCandidate, SuggestionQuestion, Suggestions, Unavailable, Verdict,
};
pub use similarity::{AliasTier, LexicalThresholds};
pub use vocabulary::{Grounding, Suggestion, Term, Vocabulary};

use ontomark_model::{DocId, Model, Profile};
use ontomark_parse::Document;
use serde::{Deserialize, Serialize};

/// Tunable limits. Defaults follow OntoCast's chunking and lexical settings.
/// Every field has a default, so a host may pass a partial object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
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
        let vocabulary = Vocabulary::from_model(model);
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
        contract::plan(self.profile, self.model, &self.options, id, doc, None)
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
            None,
            request,
            responses,
        )
    }

    /// Plan an extraction request for the note `id`, reading its evidence
    /// from external `source` text instead of the note's own body
    /// (ADR-OM-010).
    ///
    /// `doc` is the **target**: its frontmatter decides which keys are open
    /// and which values are already present, and the patch that
    /// [`Ingest::accept_source`] returns is bound to its digest. The request's
    /// chunks are cut from `source.text`, heading-aware, with the same
    /// limits as a note body, and the request carries the source's reference.
    ///
    /// # Errors
    /// As [`Ingest::plan`], and [`IngestError::SourceVersion`] when the
    /// source's declared version is not the SHA-256 of its text.
    pub fn plan_source(
        &self,
        id: &DocId,
        doc: &Document,
        source: &SourceText,
    ) -> Result<ModelRequest, IngestError> {
        contract::plan(
            self.profile,
            self.model,
            &self.options,
            id,
            doc,
            Some(source),
        )
    }

    /// Turn the model's responses to a [`Ingest::plan_source`] request into
    /// guarded candidates for the target note.
    ///
    /// Quotes are verified against the chunks of `source`, never the note.
    /// Every [`EvidenceSpan`] carries `source` and its `chunk`, `span` and
    /// `utf16` index the source's text. The patch changes the target `doc`
    /// only, is bound to its digest and is proven to apply. A value that
    /// conflicts with one the note already holds is withheld as
    /// [`WithholdReason::ConflictingValue`], never overwritten.
    /// [`Candidates::counts`] is filled in.
    ///
    /// # Errors
    /// As [`Ingest::accept`]; also [`IngestError::RequestMismatch`] with
    /// field `source` when the request was not planned from this source, and
    /// [`IngestError::ContractMismatch`] when it is a note-only request.
    pub fn accept_source(
        &self,
        doc: &Document,
        source: &SourceText,
        request: &ModelRequest,
        responses: &[ModelResponse],
    ) -> Result<Candidates, IngestError> {
        accept::accept(
            self.profile,
            self.model,
            &self.vocabulary,
            &self.options,
            doc,
            Some(source),
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
