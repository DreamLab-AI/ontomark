//! What an operation decided: the files to write, in order, and what to tell
//! the review queue. A [`Plan`] is pure data; [`Plan::commit`] applies it.

use std::path::{Component, Path, PathBuf};

use ontomark_compile::input::page::VaultKind;
use ontomark_parse::{Receipt, SourceDigest};
use serde::Serialize;

use crate::error::{code, Refusal, Result};
use crate::store::{commit, CommitError, CommitReport, Store};

/// One file an operation writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedWrite {
    /// The vault role the file belongs to.
    #[serde(serialize_with = "ser_kind")]
    pub vault: VaultKind,
    /// The page id (vault-relative path without `.md`).
    pub id: String,
    /// The path relative to the vault root, always inside it.
    pub rel_path: PathBuf,
    /// The text the plan was made against; `None` for a creation.
    pub before_text: Option<String>,
    /// The new text.
    pub text: String,
    /// The lossless patch's proof, absent for a creation.
    pub receipt: Option<Receipt>,
}

fn ser_kind<S: serde::Serializer>(k: &VaultKind, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(k.role_name())
}

impl PlannedWrite {
    /// Digest of the text the plan was made against.
    #[must_use]
    pub fn before_digest(&self) -> Option<SourceDigest> {
        self.before_text.as_deref().map(SourceDigest::of)
    }

    /// `true` when the write changes nothing.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.before_text.as_deref() == Some(self.text.as_str())
    }
}

/// How a review item changes (ADR-VG-013 section 3). The queue itself lives in
/// agentbox governance; the engine only emits this event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewEventKind {
    /// A draft now holds disputes and had none.
    Opened,
    /// A draft's disputes changed.
    Rewritten,
    /// The draft left `draft`, or no longer holds disputes.
    Closed,
}

/// One disputed page in a review event: a `page_resource` and a time. Never a
/// title, body, reason or source (ADR-VG-013 section 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisputeRef {
    /// The disputed page's `page_resource`.
    pub page: String,
    /// When the dispute was detected.
    pub detected: String,
}

/// A references-only review-queue event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewEvent {
    /// What happened to the item.
    pub kind: ReviewEventKind,
    /// The draft's `page_resource`.
    pub draft: String,
    /// Its disputes after the write; empty for [`ReviewEventKind::Closed`].
    pub disputes: Vec<DisputeRef>,
    /// When the engine wrote.
    pub at: String,
}

/// The outcome of a planned operation: nothing has been written yet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    /// The operation name, for logs.
    pub operation: &'static str,
    /// The files to write, in the order they are written. The approved draft
    /// of an approve-then-supersede comes first, so an interruption leaves
    /// both pages approved (ADR-VG-013 section 3).
    pub writes: Vec<PlannedWrite>,
    /// The keys the request itself changed (what `--expect blocks` counts);
    /// keys the engine stamped are not here.
    pub requested_keys: Vec<String>,
    /// Client-sent clock values the engine ignored (ADR-VG-013 section 7).
    pub ignored_keys: Vec<String>,
    /// References-only events for the review queue.
    pub events: Vec<ReviewEvent>,
}

impl Plan {
    /// `true` when no file changes.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.writes.iter().all(PlannedWrite::is_noop)
    }

    /// Apply the plan to `store`: every file, or the earlier ones restored.
    ///
    /// This is ordered, not crash-atomic: each file is replaced atomically,
    /// but a kill between two files leaves a partial plan on disk (for an
    /// approve-then-supersede, `PENDING_SUPERSESSION`, finished by
    /// `complete_supersession`). Each target is first re-read and compared with the text the plan was
    /// made against; a stale file refuses the whole plan before anything is
    /// written. Writes then happen in order, and when one fails the earlier
    /// ones are restored.
    ///
    /// # Errors
    /// [`CommitError`].
    pub fn commit(&self, store: &mut dyn Store) -> std::result::Result<CommitReport, CommitError> {
        commit(self, store)
    }
}

/// Refuse a path that is absolute, empty, or climbs out of the vault.
///
/// # Errors
/// [`code::PATH_ESCAPE`].
pub(crate) fn confine(rel: &Path) -> Result<()> {
    let mut any = false;
    for c in rel.components() {
        match c {
            Component::Normal(_) => any = true,
            _ => {
                return Err(Refusal::new(
                    code::PATH_ESCAPE,
                    format!("`{}` leaves the vault", rel.display()),
                ))
            }
        }
    }
    if any {
        Ok(())
    } else {
        Err(Refusal::new(code::PATH_ESCAPE, "an empty path"))
    }
}
