//! Why the engine refused a write.
//!
//! A refusal is a value, never a panic: it carries a stable machine `code`, one
//! line a human can act on, and, when validation refused the result, the errors
//! the write would have introduced. Nothing is written when an operation
//! returns one.

use serde::Serialize;

/// The stable refusal codes. Each is the `code` of a [`Refusal`].
pub mod code {
    /// The write needs vocabulary version 3 and the vocabulary is older.
    pub const VOCABULARY_V2: &str = "VOCABULARY_V2";
    /// The host supplied no vocabulary, so the engine cannot tell the version.
    pub const NO_VOCABULARY: &str = "NO_VOCABULARY";
    /// A status change outside ADR-VG-013 section 1's table.
    pub const ILLEGAL_TRANSITION: &str = "ILLEGAL_TRANSITION";
    /// A request set a key only the engine writes.
    pub const ENGINE_OWNED_KEY: &str = "ENGINE_OWNED_KEY";
    /// The result would carry an error the page did not carry before.
    pub const WOULD_INTRODUCE_ERRORS: &str = "WOULD_INTRODUCE_ERRORS";
    /// The page changed since the request was planned.
    pub const STALE_BASE: &str = "STALE_BASE";
    /// A disputed page changed after the dispute was recorded.
    pub const STALE_DISPUTE: &str = "STALE_DISPUTE";
    /// A disputed page was itself replaced; the dispute must name its successor.
    pub const DISPUTE_TARGET_MOVED: &str = "DISPUTE_TARGET_MOVED";
    /// A dispute names something that is not an approved Belief or Procedure.
    pub const DISPUTE_TARGET: &str = "DISPUTE_TARGET";
    /// An approval of a disputed draft did not state a reason for every dispute.
    pub const MISSING_RESOLUTION: &str = "MISSING_RESOLUTION";
    /// An approval or confirmation carried an instrument, or the writer may not approve.
    pub const APPROVER: &str = "APPROVER";
    /// The page already exists; a creation never overwrites.
    pub const EXISTS: &str = "EXISTS";
    /// No such page.
    pub const NOT_FOUND: &str = "NOT_FOUND";
    /// A path that would leave the vault.
    pub const PATH_ESCAPE: &str = "PATH_ESCAPE";
    /// A malformed request: bad timestamp, bad principal, bad key.
    pub const INVALID_REQUEST: &str = "INVALID_REQUEST";
    /// The page's frontmatter is one the lossless patch cannot edit.
    pub const UNPATCHABLE: &str = "UNPATCHABLE";
    /// A content change to a page in a terminal status.
    pub const TERMINAL: &str = "TERMINAL";
    /// An earlier approve-then-supersede was interrupted and must be completed first.
    pub const PENDING_SUPERSESSION: &str = "PENDING_SUPERSESSION";
    /// A v3 working-page write that names no principal.
    pub const WRITER_REQUIRED: &str = "WRITER_REQUIRED";
}

/// One validation finding quoted in a refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssueRecord {
    /// The page the finding is about.
    pub path: String,
    /// The validator's code.
    pub code: String,
    /// The validator's message.
    pub message: String,
}

/// A refused write. Nothing was written.
///
/// ```
/// use ontomark_write::{code, Refusal};
/// let r = Refusal::new(code::ILLEGAL_TRANSITION, "draft to archived");
/// assert_eq!(r.code, "ILLEGAL_TRANSITION");
/// assert!(r.to_string().contains("draft to archived"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Refusal {
    /// A stable machine code, one of [`code`].
    pub code: &'static str,
    /// One line a human can act on.
    pub message: String,
    /// The errors the result would have introduced, for
    /// [`code::WOULD_INTRODUCE_ERRORS`]; empty otherwise.
    pub issues: Vec<IssueRecord>,
}

impl Refusal {
    /// A refusal with no quoted findings.
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            issues: Vec::new(),
        }
    }
}

/// Shorthand used inside the crate.
pub(crate) type Result<T> = std::result::Result<T, Refusal>;
