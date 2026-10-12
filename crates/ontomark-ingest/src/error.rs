//! Errors that stop a plan or an acceptance outright. Anything the model got
//! wrong is a [`crate::Withheld`] record, not an error.

use ontomark_model::DocId;
use ontomark_parse::PatchError;

/// A plan or acceptance could not proceed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IngestError {
    /// The note declares no entity in the model, so there is nothing to
    /// assert about.
    #[error("{document} declares no ontology entity")]
    NotAnEntity {
        /// The note.
        document: DocId,
    },
    /// The profile declares no predicate keys, so the model could fill
    /// nothing.
    #[error("profile {profile} declares no assertion keys")]
    NoAssertionKeys {
        /// The profile id.
        profile: String,
    },
    /// The note changed since the request was planned.
    #[error("note revision {found} does not match the planned {expected}")]
    StaleRevision {
        /// Digest the request was planned against (hex).
        expected: String,
        /// Digest of the note now (hex).
        found: String,
    },
    /// The request was produced by another contract version.
    #[error("request contract {found} is not {expected}")]
    ContractMismatch {
        /// This crate's contract.
        expected: String,
        /// The request's contract.
        found: String,
    },
    /// A field of the request does not match what `plan` produces for this
    /// note with this profile and options, so the request was altered or
    /// built by hand. Only `plan`'s output is accepted.
    #[error("request {field} do not match what plan produces for this note")]
    RequestMismatch {
        /// Which part disagreed: `note`, `keys` or `chunks`.
        field: &'static str,
    },
    /// A response names a chunk the request does not have.
    #[error("response names chunk {chunk} but the request has {count}")]
    UnknownChunk {
        /// The chunk index named.
        chunk: usize,
        /// How many chunks the request has.
        count: usize,
    },
    /// The candidate patch failed its own proof against the note.
    #[error("candidate patch refused: {0}")]
    Patch(#[from] PatchError),
}
