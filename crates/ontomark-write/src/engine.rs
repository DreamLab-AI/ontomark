//! The engine's context: a snapshot of the vault(s), the vocabulary, the
//! profile and the roster, and the validation gate every result passes.

use std::cell::OnceCell;
use std::collections::BTreeSet;
use std::path::PathBuf;

use ontomark_compile::build::attribution::Roster;
use ontomark_compile::check::validate::{validate_with, Severity, Siblings};
use ontomark_compile::input::page::{Page, Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::model::Corpus;
use ontomark_compile::profile::Profile;

use crate::actor::Actor;
use crate::error::{code, IssueRecord, Refusal, Result};
use crate::plan::{confine, PlannedWrite};

type ErrorKey = (String, String, String);

/// The context every operation runs in.
///
/// It owns a snapshot: the working vault, optionally the knowledge vault (so
/// `about` links and cross-vault rules resolve), the vocabulary, the profile
/// and the attribution roster. An operation plans against the snapshot and
/// returns a [`Plan`](crate::Plan); the host rebuilds the engine (or applies
/// the plan to its own index) after a commit.
///
/// The vocabulary is a required input: without it the engine cannot tell
/// version 2 from version 3, and guessing would silently downgrade a v3 vault
/// to unvalidated patches.
pub struct Engine {
    pub(crate) profile: Profile,
    pub(crate) vocab: Vocabulary,
    pub(crate) roster: Option<Roster>,
    pub(crate) working: Vault,
    pub(crate) knowledge: Option<Vault>,
    baseline_working: OnceCell<BTreeSet<ErrorKey>>,
    baseline_knowledge: OnceCell<BTreeSet<ErrorKey>>,
}

/// A page the operation will replace or add, after the change.
pub(crate) struct Replacement {
    pub kind: VaultKind,
    pub page: Page,
}

impl Engine {
    /// Build an engine over a snapshot.
    #[must_use]
    pub fn new(
        profile: Profile,
        vocab: Vocabulary,
        roster: Option<Roster>,
        working: Vault,
        knowledge: Option<Vault>,
    ) -> Self {
        Self {
            profile,
            vocab,
            roster,
            working,
            knowledge,
            baseline_working: OnceCell::new(),
            baseline_knowledge: OnceCell::new(),
        }
    }

    /// The vocabulary the engine enforces.
    #[must_use]
    pub fn vocabulary(&self) -> &Vocabulary {
        &self.vocab
    }

    /// `true` when vocabulary version 3 rules apply.
    #[must_use]
    pub fn is_v3(&self) -> bool {
        self.vocab.is_v3()
    }

    pub(crate) fn vault(&self, kind: VaultKind) -> Option<&Vault> {
        match kind {
            VaultKind::Working => Some(&self.working),
            VaultKind::Knowledge => self.knowledge.as_ref(),
        }
    }

    /// The page `id` of `kind`, including journals.
    pub(crate) fn find(&self, kind: VaultKind, id: &str) -> Option<&Page> {
        let v = self.vault(kind)?;
        v.pages.iter().chain(v.journals.iter()).find(|p| p.id == id)
    }

    /// The working page whose `page_resource` is `urn`.
    pub(crate) fn find_resource(&self, urn: &str) -> Option<&Page> {
        self.working
            .pages
            .iter()
            .chain(self.working.journals.iter())
            .find(|p| p.frontmatter.text("page_resource").as_deref() == Some(urn))
    }

    /// Resolve a dispute or supersession reference: a `page_resource` urn, or
    /// a page id or title.
    pub(crate) fn find_reference(&self, reference: &str) -> Option<&Page> {
        let r = reference.trim();
        self.find_resource(r).or_else(|| {
            let inner = r
                .strip_prefix("[[")
                .and_then(|s| s.strip_suffix("]]"))
                .unwrap_or(r);
            let name = inner.split(['|', '#']).next().unwrap_or(inner);
            self.working
                .pages
                .iter()
                .find(|p| p.id == name || p.title() == name)
        })
    }

    pub(crate) fn roster_known(&self) -> bool {
        self.roster
            .as_ref()
            .is_some_and(|r| !r.principals().is_empty())
    }

    pub(crate) fn actor(&self, writer: &crate::Writer) -> Result<Actor> {
        Actor::resolve(writer, self.roster.as_ref())
    }

    /// Refuse a vocabulary below version 3 for operations that are v3 concepts.
    pub(crate) fn require_v3(&self, what: &str) -> Result<()> {
        if self.vocab.is_v3() {
            Ok(())
        } else {
            Err(Refusal::new(
                code::VOCABULARY_V2,
                format!(
                    "{what} needs vocabulary version 3 (stamps, clocks, lifecycle and page identity); this vault is on version {}",
                    self.vocab.version
                ),
            ))
        }
    }

    fn baseline(&self, kind: VaultKind) -> &BTreeSet<ErrorKey> {
        let cell = match kind {
            VaultKind::Working => &self.baseline_working,
            VaultKind::Knowledge => &self.baseline_knowledge,
        };
        cell.get_or_init(|| {
            self.vault(kind)
                .map(|v| self.error_keys(v, self.sibling(kind)))
                .unwrap_or_default()
        })
    }

    fn sibling(&self, kind: VaultKind) -> Option<&Vault> {
        match kind {
            VaultKind::Working => self.knowledge.as_ref(),
            VaultKind::Knowledge => Some(&self.working),
        }
    }

    fn error_keys(&self, vault: &Vault, sibling: Option<&Vault>) -> BTreeSet<ErrorKey> {
        let corpus = Corpus::build_with(vault, &self.vocab, self.roster.as_ref());
        let siblings = Siblings {
            vaults: sibling.into_iter().collect(),
            roster: self.roster.as_ref(),
        };
        let report = validate_with(&self.profile, vault, &corpus, &self.vocab, &siblings);
        report
            .issues
            .into_iter()
            .filter(|i| i.severity == Severity::Error)
            .map(|i| (i.path, i.code, i.message))
            .collect()
    }

    /// Validate the snapshot with `changes` substituted and refuse if the
    /// result carries an error the snapshot did not.
    ///
    /// # Errors
    /// [`code::WOULD_INTRODUCE_ERRORS`], quoting each new error.
    pub(crate) fn gate(&self, changes: &[Replacement]) -> Result<()> {
        for kind in [VaultKind::Working, VaultKind::Knowledge] {
            let mine: Vec<&Replacement> = changes.iter().filter(|c| c.kind == kind).collect();
            if mine.is_empty() {
                continue;
            }
            let Some(base) = self.vault(kind) else {
                return Err(Refusal::new(
                    code::NOT_FOUND,
                    format!("the {} vault is not loaded", kind.role_name()),
                ));
            };
            let mut vault = base.clone();
            for c in mine {
                let slot = vault
                    .pages
                    .iter_mut()
                    .chain(vault.journals.iter_mut())
                    .find(|p| p.id == c.page.id);
                match slot {
                    Some(old) => *old = c.page.clone(),
                    None => vault.pages.push(c.page.clone()),
                }
            }
            let after = self.error_keys(&vault, self.sibling(kind));
            let before = self.baseline(kind);
            let introduced: Vec<IssueRecord> = after
                .difference(before)
                .map(|(path, code, message)| IssueRecord {
                    path: path.clone(),
                    code: code.clone(),
                    message: message.clone(),
                })
                .collect();
            if !introduced.is_empty() {
                let first = &introduced[0];
                return Err(Refusal {
                    code: code::WOULD_INTRODUCE_ERRORS,
                    message: format!(
                        "the write would introduce {} validation error(s), first: {} on {}: {}",
                        introduced.len(),
                        first.code,
                        first.path,
                        first.message
                    ),
                    issues: introduced,
                });
            }
        }
        Ok(())
    }

    /// Build the [`PlannedWrite`] for a page whose text becomes `text`.
    pub(crate) fn write_for(
        &self,
        kind: VaultKind,
        old: &Page,
        text: String,
        receipt: ontomark_parse::Receipt,
    ) -> Result<PlannedWrite> {
        confine(&old.rel_path)?;
        Ok(PlannedWrite {
            vault: kind,
            id: old.id.clone(),
            rel_path: old.rel_path.clone(),
            before_text: Some(old.source.clone()),
            text,
            receipt: Some(receipt),
        })
    }

    /// Parse `text` as the new version of `old`.
    pub(crate) fn reparse(&self, old: &Page, text: &str) -> Result<Page> {
        Page::parse(old.path.clone(), old.rel_path.clone(), old.id.clone(), text).map_err(|e| {
            Refusal::new(
                code::UNPATCHABLE,
                format!("the result does not parse as a page: {e}"),
            )
        })
    }

    /// The path a new page `id` of `kind` is written to.
    pub(crate) fn path_for(&self, kind: VaultKind, id: &str) -> Result<(PathBuf, PathBuf)> {
        let rel = PathBuf::from(format!("{id}.md"));
        confine(&rel)?;
        let root = self
            .vault(kind)
            .map(|v| v.root.clone())
            .ok_or_else(|| Refusal::new(code::NOT_FOUND, "that vault is not loaded"))?;
        Ok((root.join(&rel), rel))
    }
}
