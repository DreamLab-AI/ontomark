//! Versioned, dry-runnable, idempotent, restartable corpus migrations
//! (ADR-2015, Migration; EXP-08).
//!
//! Markdown is authoritative, so the only write path a migration has is a
//! **guarded, lossless Markdown patch**: a pure function from one page's source
//! text to its new source text, applied against the digest the plan was made
//! from. Everything else in this module exists to make that safe.
//!
//! # The life of a migration
//!
//! 1. [`plan`] reads every page of the selected vaults, runs the migration's
//!    pure [`Migration::transform`] over each, and records a per-page
//!    [`Disposition`] with before/after SHA-256 digests. It then compiles the
//!    knowledge vault twice in memory, before and after, and reports the
//!    [`SemanticDelta`]: asserted-triple delta, inferred-subsumption delta,
//!    visibility changes, newly unsatisfiable classes and identity changes. A
//!    [`Kind::Syntax`] migration is meaning-preserving by definition, so any
//!    identity, visibility or satisfiability change makes the plan
//!    [`PlanStatus::Refused`]. The plan holds digests, never page bodies, and
//!    is deterministic: the same input yields byte-identical JSON.
//! 2. [`apply`] re-checks each page against the plan. A page whose current
//!    digest is not the planned `before` is a **conflict** and is left alone
//!    (the stale patch is invalidated, not forced); a page already at the
//!    planned `after` is skipped, which is what makes an interrupted run
//!    restartable. Each write is temp file plus atomic rename, preceded by the
//!    original bytes and an `intent` record in an fsynced journal.
//! 3. [`rollback`] restores the saved originals page by page, only where the
//!    page still has the digest the migration wrote, and verifies each restored
//!    file hashes to the planned `before`.
//! 4. [`status`] lists the journals and where each stands.
//!
//! Planning again after a successful apply yields no [`Disposition::Change`]
//! pages: every registered migration is idempotent.
//!
//! # Journals hold page bytes
//!
//! A journal lives at `<repo>/.ontomark/migrations/<migration-id>/<input-digest>/`
//! and contains the **original bytes of every page it changed**, which for the
//! working vault means private pages. `.ontomark/` must therefore stay out of
//! git and out of publication: add it to the repository's `.gitignore` and never
//! point a publish step at it.
//!
//! # Registry
//!
//! [`registry`] lists the shipped migrations; [`lookup`] finds one by id.
//! Today that is [`m0001::ObsidianBodies`], the Logseq-outline to Obsidian
//! markdown conversion of [`crate::check::bodies`].

mod apply;
mod compile;
mod journal;
pub mod m0001;
mod plan;

use sha2::{Digest, Sha256};

pub use apply::{
    apply, apply_with, rollback, status, ApplyOptions, ApplyReport, Conflict, JournalStatus,
    RollbackReport,
};
pub use plan::{
    plan, AssertedDelta, Disposition, IdentityChange, InferredDelta, MigrationRef, PagePlan, Plan,
    PlanStatus, SemanticDelta, VisibilityChange,
};

/// What a migration is allowed to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Meaning-preserving: the compiled ontology's identities, visibility and
    /// satisfiability are unchanged. A plan that shows otherwise is refused.
    Syntax,
    /// May change meaning. The plan reports the deltas for review and
    /// [`apply`] refuses it unless [`ApplyOptions::accept_semantic`] is set.
    Semantic,
}

impl Kind {
    /// The lower-case name used in plans and reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::Semantic => "semantic",
        }
    }
}

/// What a migration says about one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transform {
    /// The page already is what the migration would make it.
    Unchanged,
    /// The page's new source and a one-line summary of the change.
    Changed {
        /// The complete new file contents.
        source: String,
        /// What changed, for the plan report. Never page text.
        summary: String,
    },
    /// The migration declines this page, with the reason.
    Refused(String),
}

/// A pure, versioned page rewrite.
///
/// Implementations must be deterministic and idempotent: transforming the
/// `source` of a [`Transform::Changed`] result yields [`Transform::Unchanged`].
/// They perform no I/O, which is what lets [`apply`] recompute the exact bytes
/// a plan promised.
pub trait Migration {
    /// A stable identifier such as `m0001-obsidian-bodies`.
    fn id(&self) -> &str;
    /// The migration's version. A plan made under one version is not applied
    /// by another.
    fn version(&self) -> u32;
    /// One line saying what the migration does.
    fn description(&self) -> &str;
    /// Whether the migration claims to preserve meaning.
    fn kind(&self) -> Kind;
    /// Rewrite one page. `page_rel_path` is repository-relative with `/`
    /// separators, for example `knowledge/pages/ETSI/Domain.md`.
    fn transform(&self, page_rel_path: &str, source: &str) -> Transform;
}

/// Every shipped migration, in application order.
#[must_use]
pub fn registry() -> Vec<Box<dyn Migration>> {
    vec![Box::new(m0001::ObsidianBodies)]
}

/// The registered migration with this id.
#[must_use]
pub fn lookup(id: &str) -> Option<Box<dyn Migration>> {
    registry().into_iter().find(|m| m.id() == id)
}

/// Lower-case hex SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Why a migration operation did not complete.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    /// A filesystem operation failed.
    #[error("{path}: {source}")]
    Io {
        /// The path involved.
        path: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A plan or journal record could not be read or written as JSON.
    #[error("{what}: {source}")]
    Json {
        /// What was being (de)serialised.
        what: String,
        /// The underlying error.
        source: serde_json::Error,
    },
    /// No registered migration has this id.
    #[error("no migration `{0}` in the registry")]
    UnknownMigration(String),
    /// The plan was made by a different migration id or version than the one
    /// asked to apply it.
    #[error("plan was made by {plan} but the registry has {registry}")]
    PlanMismatch {
        /// `id@version` in the plan.
        plan: String,
        /// `id@version` being applied.
        registry: String,
    },
    /// The plan is refused; the reasons say why.
    #[error("plan is refused: {}", .0.join("; "))]
    PlanRefused(Vec<String>),
    /// A semantic migration was applied without accepting its deltas.
    #[error("migration `{0}` is semantic: review the plan deltas and pass accept_semantic")]
    SemanticNotAccepted(String),
    /// The vault inputs could not be loaded.
    #[error("{0}")]
    Input(String),
    /// A fault injected through [`ApplyOptions`], used to test restart.
    #[error("apply interrupted by an injected fault after {0} page(s)")]
    Interrupted(usize),
    /// The journal directory is missing or unreadable.
    #[error("journal: {0}")]
    Journal(String),
}

impl MigrateError {
    pub(crate) fn io(path: &std::path::Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.display().to_string(),
            source,
        }
    }

    pub(crate) fn json(what: &str, source: serde_json::Error) -> Self {
        Self::Json {
            what: what.to_owned(),
            source,
        }
    }
}
