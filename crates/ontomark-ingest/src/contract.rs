//! The request/response contract between the core and a host-supplied model.

use ontomark_model::{DocId, Iri, Kind, Model, PredicateForm, Profile};
use ontomark_parse::{Document, SourceDigest};
use serde::{Deserialize, Serialize};

use crate::chunk::{chunk_document, chunk_text, Chunk};
use crate::error::IngestError;
use crate::Options;

/// The contract version a request carries. A request from another version
/// is refused by [`crate::Ingest::accept`].
pub const CONTRACT: &str = "ontomark-ingest/facts/1";

/// The model instructions for this contract, carried as data.
pub const INSTRUCTIONS: &str = include_str!("../prompts/facts-v1.md");

/// The contract version of a request planned from an external source (see
/// [`crate::Ingest::plan_source`]). It differs from [`CONTRACT`] so that a
/// source request can never be accepted as a note-only one, or the reverse.
pub const CONTRACT_SOURCE: &str = "ontomark-ingest/facts-source/1";

/// The model instructions for a source request, carried as data.
pub const INSTRUCTIONS_SOURCE: &str = include_str!("../prompts/facts-source-v1.md");

/// Which external source a piece of evidence came from, and which version
/// of it. The host supplies both; the crate never reads the source itself.
///
/// ```
/// # use ontomark_ingest::SourceRef;
/// let r = SourceRef::new("file:notes/paper.md", "ab".repeat(32));
/// assert_eq!(r.id, "file:notes/paper.md");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SourceRef {
    /// The host's stable identifier for the source (a path, URL or item id).
    pub id: String,
    /// The lower-case hex SHA-256 of the source text, so a quote can be
    /// re-checked against exactly the text it was found in.
    pub version: String,
}

impl SourceRef {
    /// A reference from an id and a version hash.
    #[must_use]
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
        }
    }
}

/// External source text with its reference. Nothing in the crate fetches or
/// renders it.
///
/// ```
/// # use ontomark_ingest::SourceText;
/// let s = SourceText::from_text("urn:doc:1", "Alpha is a beta.");
/// assert_eq!(s.reference.version.len(), 64);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SourceText {
    /// Where the text came from.
    pub reference: SourceRef,
    /// The text: plain text or Markdown. Read whole; a leading `---` block is
    /// not frontmatter here.
    pub text: String,
}

impl SourceText {
    /// Pair a reference with its text. The version is checked against the
    /// text when a plan or acceptance uses it; see [`Self::from_text`] to
    /// compute it.
    #[must_use]
    pub fn new(reference: SourceRef, text: impl Into<String>) -> Self {
        Self {
            reference,
            text: text.into(),
        }
    }

    /// A source whose version is the SHA-256 of `text`.
    #[must_use]
    pub fn from_text(id: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let version = SourceDigest::of(&text).hex();
        Self {
            reference: SourceRef::new(id, version),
            text,
        }
    }

    /// Whether the reference's version is the SHA-256 of the text.
    #[must_use]
    pub fn version_matches(&self) -> bool {
        self.reference.version == SourceDigest::of(&self.text).hex()
    }
}

/// What kind of value a key takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ValueKind {
    /// The name of an entity in the vault; grounded by the core.
    Name,
    /// A literal in the lexical form of `datatype`.
    Literal {
        /// The literal datatype IRI.
        datatype: Iri,
    },
}

/// One frontmatter key the model may fill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KeySpec {
    /// The frontmatter key.
    pub key: String,
    /// The predicate it asserts.
    pub predicate: Iri,
    /// How the profile reads it.
    pub form: PredicateForm,
    /// What values it takes.
    pub value: ValueKind,
    /// Declared `rdfs:range`, informational.
    pub range: Option<Iri>,
    /// Values the note already holds, as written.
    pub present: Vec<String>,
}

/// The note a request is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NoteRef {
    /// Vault path.
    pub document: DocId,
    /// Source digest (hex) the request was planned against.
    pub digest: String,
    /// The entity the note declares.
    pub iri: Iri,
    /// Its kind.
    pub kind: Kind,
    /// Its label.
    pub label: String,
}

/// Everything a host needs to ask a model, as plain data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelRequest {
    /// [`CONTRACT`].
    pub contract: String,
    /// The note.
    pub note: NoteRef,
    /// [`INSTRUCTIONS`].
    pub instructions: String,
    /// Keys the model may fill, in profile order.
    pub keys: Vec<KeySpec>,
    /// The note body in chunks.
    pub chunks: Vec<Chunk>,
    /// JSON schema one [`ModelResponse`] must satisfy.
    pub response_schema: serde_json::Value,
    /// The external source the chunks were cut from, when the request was
    /// planned with [`crate::Ingest::plan_source`]. Omitted for a note-only
    /// request, so those serialise as they always did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
}

/// One proposed value with its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProposedAssertion {
    /// A key from [`ModelRequest::keys`].
    pub key: String,
    /// A name (for [`ValueKind::Name`]) or a literal lexical form.
    pub value: String,
    /// A verbatim quote from the chunk.
    pub evidence: String,
}

impl ProposedAssertion {
    /// A proposal of `value` for `key`, supported by the verbatim `evidence`.
    #[must_use]
    pub fn new(
        key: impl Into<String>,
        value: impl Into<String>,
        evidence: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            evidence: evidence.into(),
        }
    }
}

/// The model's answer for one chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelResponse {
    /// The chunk index answered.
    pub chunk: usize,
    /// Proposed assertions; may be empty.
    pub assertions: Vec<ProposedAssertion>,
}

impl ModelResponse {
    /// The answer for chunk `chunk`.
    #[must_use]
    pub fn new(chunk: usize, assertions: Vec<ProposedAssertion>) -> Self {
        Self { chunk, assertions }
    }
}

fn xsd_string() -> Iri {
    Iri::parse("http://www.w3.org/2001/XMLSchema#string").expect("constant IRI")
}

/// Whether a predicate form can be asserted on a page of `kind`, mirroring
/// `ontomark-model`'s own subject rules: subsumption, equivalence and
/// disjointness need a class; class assertion, `sameAs` and object assertion
/// need an individual; an existential is a restriction on a class and an
/// object assertion on an individual; sub-property needs a property; data
/// assertions and annotations apply anywhere.
pub(crate) fn applies_to(form: PredicateForm, kind: Kind) -> bool {
    use PredicateForm as F;
    match form {
        F::SubClassOf | F::EquivalentClass | F::DisjointWith => kind == Kind::Class,
        F::ClassAssertion | F::SameAs | F::ObjectAssertion => kind == Kind::Individual,
        F::Existential => matches!(kind, Kind::Class | Kind::Individual),
        F::SubPropertyOf => matches!(kind, Kind::Property(_)),
        F::DataAssertion | F::Annotation | F::Skos(_) => true,
    }
}

pub(crate) fn key_specs(profile: &dyn Profile, doc: &Document, kind: Kind) -> Vec<KeySpec> {
    profile
        .predicates()
        .into_iter()
        .filter(|p| !profile.is_reserved(&p.key) && applies_to(p.form, kind))
        .map(|p| {
            let value = match p.form {
                PredicateForm::DataAssertion => ValueKind::Literal {
                    datatype: p.datatype.clone().unwrap_or_else(xsd_string),
                },
                PredicateForm::Annotation => ValueKind::Literal {
                    datatype: xsd_string(),
                },
                _ => ValueKind::Name,
            };
            let present = doc.value(&p.key).map(|v| v.texts()).unwrap_or_default();
            KeySpec {
                key: p.key,
                predicate: p.iri,
                form: p.form,
                value,
                range: p.range,
                present,
            }
        })
        .collect()
}

fn response_schema(keys: &[KeySpec]) -> serde_json::Value {
    let names: Vec<&str> = keys.iter().map(|k| k.key.as_str()).collect();
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "ontomark-ingest facts response",
        "type": "object",
        "required": ["chunk", "assertions"],
        "additionalProperties": false,
        "properties": {
            "chunk": { "type": "integer", "minimum": 0 },
            "assertions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["key", "value", "evidence"],
                    "additionalProperties": false,
                    "properties": {
                        "key": { "type": "string", "enum": names },
                        "value": { "type": "string", "minLength": 1 },
                        "evidence": { "type": "string", "minLength": 1 }
                    }
                }
            }
        }
    })
}

pub(crate) fn plan(
    profile: &dyn Profile,
    model: &Model,
    options: &Options,
    id: &DocId,
    doc: &Document,
    source: Option<&SourceText>,
) -> Result<ModelRequest, IngestError> {
    let entity = model
        .entity_of(id)
        .ok_or_else(|| IngestError::NotAnEntity {
            document: id.clone(),
        })?;
    let keys = key_specs(profile, doc, entity.kind);
    if keys.is_empty() {
        return Err(IngestError::NoAssertionKeys {
            profile: profile.id().to_owned(),
        });
    }
    if let Some(s) = source {
        if !s.version_matches() {
            return Err(IngestError::SourceVersion {
                id: s.reference.id.clone(),
                declared: s.reference.version.clone(),
                actual: SourceDigest::of(&s.text).hex(),
            });
        }
    }
    let chunks = chunks_for(doc, source, options);
    let response_schema = response_schema(&keys);
    Ok(ModelRequest {
        contract: if source.is_some() {
            CONTRACT_SOURCE
        } else {
            CONTRACT
        }
        .to_owned(),
        note: NoteRef {
            document: id.clone(),
            digest: doc.digest().hex(),
            iri: entity.iri.clone(),
            kind: entity.kind,
            label: entity.label.clone(),
        },
        instructions: if source.is_some() {
            INSTRUCTIONS_SOURCE
        } else {
            INSTRUCTIONS
        }
        .to_owned(),
        keys,
        chunks,
        response_schema,
        source: source.map(|s| s.reference.clone()),
    })
}

/// The chunks a request carries: the source's when there is one, else the
/// note body's. `accept` recomputes them with this same function.
pub(crate) fn chunks_for(
    doc: &Document,
    source: Option<&SourceText>,
    options: &Options,
) -> Vec<Chunk> {
    match source {
        Some(s) => chunk_text(&s.text, options.min_chunk_chars, options.max_chunk_chars),
        None => chunk_document(doc, options.min_chunk_chars, options.max_chunk_chars),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ontomark_model::PropertyKind;

    #[test]
    fn forms_are_offered_only_where_they_can_be_asserted() {
        use PredicateForm as F;
        assert!(applies_to(F::SubClassOf, Kind::Class));
        assert!(!applies_to(F::ClassAssertion, Kind::Class));
        assert!(applies_to(F::ClassAssertion, Kind::Individual));
        assert!(!applies_to(F::SubClassOf, Kind::Individual));
        assert!(applies_to(
            F::SubPropertyOf,
            Kind::Property(PropertyKind::Object)
        ));
        assert!(!applies_to(
            F::SubClassOf,
            Kind::Property(PropertyKind::Object)
        ));
        for k in [
            Kind::Class,
            Kind::Individual,
            Kind::Property(PropertyKind::Data),
        ] {
            assert!(applies_to(F::Annotation, k));
            assert!(applies_to(F::DataAssertion, k));
        }
    }
}
