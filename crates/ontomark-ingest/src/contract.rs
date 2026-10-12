//! The request/response contract between the core and a host-supplied model.

use ontomark_model::{DocId, Iri, Kind, Model, PredicateForm, Profile};
use ontomark_parse::Document;
use serde::{Deserialize, Serialize};

use crate::chunk::{chunk_document, Chunk};
use crate::error::IngestError;
use crate::Options;

/// The contract version a request carries. A request from another version
/// is refused by [`crate::Ingest::accept`].
pub const CONTRACT: &str = "ontomark-ingest/facts/1";

/// The model instructions for this contract, carried as data.
pub const INSTRUCTIONS: &str = include_str!("../prompts/facts-v1.md");

/// What kind of value a key takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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
}

/// One proposed value with its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedAssertion {
    /// A key from [`ModelRequest::keys`].
    pub key: String,
    /// A name (for [`ValueKind::Name`]) or a literal lexical form.
    pub value: String,
    /// A verbatim quote from the chunk.
    pub evidence: String,
}

/// The model's answer for one chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelResponse {
    /// The chunk index answered.
    pub chunk: usize,
    /// Proposed assertions; may be empty.
    pub assertions: Vec<ProposedAssertion>,
}

fn xsd_string() -> Iri {
    Iri::parse("http://www.w3.org/2001/XMLSchema#string").expect("constant IRI")
}

pub(crate) fn key_specs(profile: &dyn Profile, doc: &Document) -> Vec<KeySpec> {
    profile
        .predicates()
        .into_iter()
        .filter(|p| !profile.is_reserved(&p.key))
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
) -> Result<ModelRequest, IngestError> {
    let entity = model
        .entity_of(id)
        .ok_or_else(|| IngestError::NotAnEntity {
            document: id.clone(),
        })?;
    let keys = key_specs(profile, doc);
    if keys.is_empty() {
        return Err(IngestError::NoAssertionKeys {
            profile: profile.id().to_owned(),
        });
    }
    let chunks = chunk_document(doc, options.min_chunk_chars, options.max_chunk_chars);
    let response_schema = response_schema(&keys);
    Ok(ModelRequest {
        contract: CONTRACT.to_owned(),
        note: NoteRef {
            document: id.clone(),
            digest: doc.digest().hex(),
            iri: entity.iri.clone(),
            kind: entity.kind,
            label: entity.label.clone(),
        },
        instructions: INSTRUCTIONS.to_owned(),
        keys,
        chunks,
        response_schema,
    })
}
