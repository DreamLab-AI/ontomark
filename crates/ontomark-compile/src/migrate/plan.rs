//! Planning: per-page dispositions, the input digest and the semantic deltas.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::compile::{compile_knowledge, SnapshotPage};
use super::{sha256_hex, Kind, MigrateError, Migration, Transform};
use crate::build::attribution::Roster;
use crate::input::page::{walk_pages, Page, VaultKind};
use crate::input::vocabulary::Vocabulary;
use crate::profile::Profile;

/// How many example lines each delta carries.
const EXAMPLES: usize = 10;

/// The migration a plan was made by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationRef {
    /// [`Migration::id`].
    pub id: String,
    /// [`Migration::version`].
    pub version: u32,
    /// [`Migration::kind`].
    pub kind: Kind,
    /// [`Migration::description`].
    pub description: String,
}

impl MigrationRef {
    pub(crate) fn of(m: &dyn Migration) -> Self {
        Self {
            id: m.id().to_owned(),
            version: m.version(),
            kind: m.kind(),
            description: m.description().to_owned(),
        }
    }

    /// `id@version`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }
}

/// What the migration does to one page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// Already in the migrated form.
    Unchanged,
    /// Will be rewritten.
    Change,
    /// The migration declined this page; it is never written.
    Refused,
}

/// One page's plan. Unchanged pages are counted, not listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagePlan {
    /// Repository-relative path, `/`-separated.
    pub path: String,
    /// What happens to it.
    pub disposition: Disposition,
    /// SHA-256 of the file as planned against.
    pub before: String,
    /// SHA-256 of the file after the patch (`Change` only).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub after: Option<String>,
    /// One line on the change, or the refusal reason. Never page text.
    pub summary: String,
}

/// Whether a plan may be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Applicable.
    Ready,
    /// Not applicable; see [`Plan::reasons`].
    Refused,
}

/// Asserted-triple delta of the knowledge vault's public graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertedDelta {
    /// Triples after minus before.
    pub added: usize,
    /// Triples before minus after.
    pub removed: usize,
    /// Up to ten added triples.
    pub added_examples: Vec<String>,
    /// Up to ten removed triples.
    pub removed_examples: Vec<String>,
}

/// Inferred-subsumption delta (Whelk).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferredDelta {
    /// Subsumptions entailed only after.
    pub added: usize,
    /// Subsumptions entailed only before.
    pub removed: usize,
    /// Up to ten added `sub => sup` pairs.
    pub added_examples: Vec<String>,
    /// Up to ten removed `sub => sup` pairs.
    pub removed_examples: Vec<String>,
}

/// A page whose `public` disposition changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibilityChange {
    /// Repository-relative path.
    pub path: String,
    /// `public` before.
    pub before: bool,
    /// `public` after.
    pub after: bool,
}

/// A page whose identity field changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityChange {
    /// Repository-relative path.
    pub path: String,
    /// `resource`, `page_resource`, `slug` or `title`.
    pub field: String,
    /// The value before.
    pub before: String,
    /// The value after.
    pub after: String,
}

/// What the migration changes about meaning, from compiling before and after.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticDelta {
    /// `true` when both compiles succeeded and the triple/inference deltas are real.
    pub compiled: bool,
    /// Why a compile-derived field is absent or the compile failed.
    pub notes: Vec<String>,
    /// Asserted-triple delta.
    pub asserted: AssertedDelta,
    /// Inferred-subsumption delta; `None` when no classifier is built in.
    pub inferred: Option<InferredDelta>,
    /// Pages whose `public` flag changes.
    pub visibility: Vec<VisibilityChange>,
    /// Classes unsatisfiable after and not before.
    pub newly_unsatisfiable: Vec<String>,
    /// Changed resource IRIs, page IRIs, slugs and titles.
    pub identity: Vec<IdentityChange>,
    /// Resource IRIs present after and not before.
    pub resources_added: Vec<String>,
    /// Resource IRIs present before and not after.
    pub resources_removed: Vec<String>,
}

/// A migration plan: deterministic, serialisable, free of page bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// Plan schema version.
    pub schema: u32,
    /// The migration that made the plan.
    pub migration: MigrationRef,
    /// The vaults considered, `knowledge` and/or `working`.
    pub vaults: Vec<String>,
    /// SHA-256 over every considered page's sorted `path`, NUL, `sha256`, LF.
    pub input_digest: String,
    /// Pages considered.
    pub pages_considered: usize,
    /// Pages already in the migrated form.
    pub unchanged: usize,
    /// Pages the migration will rewrite.
    pub changes: usize,
    /// Pages the migration refused.
    pub refused: usize,
    /// Whether the plan may be applied.
    pub status: PlanStatus,
    /// Why the plan is refused.
    pub reasons: Vec<String>,
    /// Every `Change` and `Refused` page, sorted by path.
    pub pages: Vec<PagePlan>,
    /// The semantic deltas.
    pub deltas: SemanticDelta,
}

impl Plan {
    /// Stable JSON: struct field order is fixed and there are no maps or
    /// timestamps, so the same input yields the same bytes.
    ///
    /// # Errors
    /// Only if serialisation fails, which the plan's types cannot cause.
    pub fn to_json(&self) -> Result<String, MigrateError> {
        let mut s =
            serde_json::to_string_pretty(self).map_err(|e| MigrateError::json("plan", e))?;
        s.push('\n');
        Ok(s)
    }

    /// Parse a plan written by [`Plan::to_json`].
    ///
    /// # Errors
    /// The text is not a plan.
    pub fn from_json(text: &str) -> Result<Self, MigrateError> {
        serde_json::from_str(text).map_err(|e| MigrateError::json("plan", e))
    }

    /// A human report: the verdict, the counts, the deltas and the page list.
    #[must_use]
    pub fn report(&self) -> String {
        let mut s = String::new();
        let m = &self.migration;
        let _ = writeln!(
            s,
            "migration {} ({}): {}",
            m.label(),
            m.kind.as_str(),
            m.description
        );
        let _ = writeln!(s, "vaults: {}", self.vaults.join(", "));
        let _ = writeln!(s, "input digest: {}", self.input_digest);
        let _ = writeln!(
            s,
            "pages: {} considered, {} to change, {} unchanged, {} refused",
            self.pages_considered, self.changes, self.unchanged, self.refused
        );
        let _ = writeln!(
            s,
            "status: {}",
            match self.status {
                PlanStatus::Ready => "READY",
                PlanStatus::Refused => "REFUSED",
            }
        );
        for r in &self.reasons {
            let _ = writeln!(s, "  reason: {r}");
        }
        let d = &self.deltas;
        let _ = writeln!(s, "deltas (knowledge vault, compiled before/after):");
        for n in &d.notes {
            let _ = writeln!(s, "  note: {n}");
        }
        let _ = writeln!(
            s,
            "  asserted triples: +{} -{}",
            d.asserted.added, d.asserted.removed
        );
        for e in &d.asserted.added_examples {
            let _ = writeln!(s, "    + {e}");
        }
        for e in &d.asserted.removed_examples {
            let _ = writeln!(s, "    - {e}");
        }
        match &d.inferred {
            Some(i) => {
                let _ = writeln!(s, "  inferred subsumptions: +{} -{}", i.added, i.removed);
            }
            None => {
                let _ = writeln!(s, "  inferred subsumptions: not computed");
            }
        }
        let _ = writeln!(s, "  visibility changes: {}", d.visibility.len());
        for v in &d.visibility {
            let _ = writeln!(s, "    {}: public {} -> {}", v.path, v.before, v.after);
        }
        let _ = writeln!(s, "  newly unsatisfiable: {}", d.newly_unsatisfiable.len());
        let _ = writeln!(
            s,
            "  identity changes: {} (resources +{} -{})",
            d.identity.len(),
            d.resources_added.len(),
            d.resources_removed.len()
        );
        for c in &d.identity {
            let _ = writeln!(s, "    {}: {} {} -> {}", c.path, c.field, c.before, c.after);
        }
        let _ = writeln!(s, "pages:");
        for p in &self.pages {
            let tag = match p.disposition {
                Disposition::Change => "change ",
                Disposition::Refused => "refused",
                Disposition::Unchanged => "same   ",
            };
            let _ = writeln!(s, "  {tag} {}  ({})", p.path, p.summary);
        }
        s
    }
}

/// A considered page, read from disk.
struct Considered {
    rel: String,
    bytes: Vec<u8>,
}

fn vault_kind_name(kind: VaultKind) -> &'static str {
    kind.dir_name()
}

fn read_considered(repo_root: &Path, kinds: &[VaultKind]) -> Result<Vec<Considered>, MigrateError> {
    let mut out = Vec::new();
    let mut kinds: Vec<VaultKind> = kinds.to_vec();
    kinds.sort_by_key(|k| vault_kind_name(*k));
    kinds.dedup();
    for kind in kinds {
        let vault_dir = repo_root.join(kind.dir_name());
        for tree in ["pages", "journals"] {
            let dir = vault_dir.join(tree);
            if !dir.is_dir() {
                continue;
            }
            let walk = walk_pages(&dir).map_err(|e| MigrateError::Input(e.to_string()))?;
            for path in walk.files {
                let rel = path
                    .strip_prefix(repo_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = std::fs::read(&path).map_err(|e| MigrateError::io(&path, e))?;
                out.push(Considered { rel, bytes });
            }
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

/// A page's identity-bearing fields and `public` flag, from its source.
fn identity_of(vocab: &Vocabulary, rel: &str, text: &str) -> Option<(bool, [(String, String); 4])> {
    let under = rel.split_once('/').map_or(rel, |(_, r)| r);
    let id = under
        .split_once('/')
        .map_or(under, |(_, r)| r)
        .trim_end_matches(".md");
    let page = Page::parse(rel, under, id, text).ok()?;
    Some((
        page.is_public(),
        [
            ("resource".to_owned(), page.resource(&vocab.namespace)),
            (
                "page_resource".to_owned(),
                page.frontmatter.text("page_resource").unwrap_or_default(),
            ),
            ("slug".to_owned(), page.slug()),
            ("title".to_owned(), page.title()),
        ],
    ))
}

fn multiset_delta(
    before: &BTreeMap<String, usize>,
    after: &BTreeMap<String, usize>,
) -> AssertedDelta {
    let mut d = AssertedDelta::default();
    for (k, &n) in after {
        let b = before.get(k).copied().unwrap_or(0);
        if n > b {
            d.added += n - b;
            if d.added_examples.len() < EXAMPLES {
                d.added_examples.push(k.clone());
            }
        }
    }
    for (k, &n) in before {
        let a = after.get(k).copied().unwrap_or(0);
        if n > a {
            d.removed += n - a;
            if d.removed_examples.len() < EXAMPLES {
                d.removed_examples.push(k.clone());
            }
        }
    }
    d
}

fn pairs_delta(
    before: &BTreeSet<(String, String)>,
    after: &BTreeSet<(String, String)>,
) -> InferredDelta {
    let show = |(a, b): &(String, String)| format!("{a} => {b}");
    let added: Vec<_> = after.difference(before).collect();
    let removed: Vec<_> = before.difference(after).collect();
    InferredDelta {
        added: added.len(),
        removed: removed.len(),
        added_examples: added.iter().take(EXAMPLES).map(|p| show(p)).collect(),
        removed_examples: removed.iter().take(EXAMPLES).map(|p| show(p)).collect(),
    }
}

/// Plan `migration` over `vaults` of the repository at `repo_root`.
///
/// Reads only; writes nothing. The knowledge vault is compiled twice in memory
/// (before and after) under `profile` and the repository's
/// `ontology/vocabulary.yaml`, unless no page changes.
///
/// A [`Kind::Syntax`] plan is refused when the after-compile shows an identity
/// change, a visibility change, a newly unsatisfiable class, or fails where the
/// before-compile succeeded. A non-zero asserted-triple delta is reported but
/// does not by itself refuse: body text legitimately feeds definitions and
/// links, and the plan shows exactly which triples moved.
///
/// # Errors
/// The vault cannot be read or the vocabulary cannot be loaded.
pub fn plan(
    repo_root: &Path,
    vaults: &[VaultKind],
    migration: &dyn Migration,
    profile: &Profile,
) -> Result<Plan, MigrateError> {
    let vocab = Vocabulary::load(repo_root.join("ontology/vocabulary.yaml"))
        .map_err(|e| MigrateError::Input(format!("ontology/vocabulary.yaml: {e}")))?;
    let roster = Roster::discover(repo_root);
    let considered = read_considered(repo_root, vaults)?;

    let mut digest_input = String::new();
    let mut pages: Vec<PagePlan> = Vec::new();
    let mut unchanged = 0usize;
    let (mut changes, mut refused) = (0usize, 0usize);
    // Post-migration source for every changed page, to compile the "after".
    let mut after_text: BTreeMap<String, String> = BTreeMap::new();
    let mut texts: Vec<(String, String)> = Vec::new();
    for c in &considered {
        let sha = sha256_hex(&c.bytes);
        let _ = writeln!(digest_input, "{}\0{}", c.rel, sha);
        let Ok(text) = std::str::from_utf8(&c.bytes) else {
            refused += 1;
            pages.push(PagePlan {
                path: c.rel.clone(),
                disposition: Disposition::Refused,
                before: sha,
                after: None,
                summary: "not valid UTF-8".to_owned(),
            });
            texts.push((
                c.rel.clone(),
                String::from_utf8_lossy(&c.bytes).into_owned(),
            ));
            continue;
        };
        texts.push((c.rel.clone(), text.to_owned()));
        match migration.transform(&c.rel, text) {
            Transform::Unchanged => unchanged += 1,
            Transform::Changed { source, summary } => {
                if source.as_bytes() == c.bytes.as_slice() {
                    unchanged += 1;
                    continue;
                }
                changes += 1;
                pages.push(PagePlan {
                    path: c.rel.clone(),
                    disposition: Disposition::Change,
                    before: sha,
                    after: Some(sha256_hex(source.as_bytes())),
                    summary,
                });
                after_text.insert(c.rel.clone(), source);
            }
            Transform::Refused(reason) => {
                refused += 1;
                pages.push(PagePlan {
                    path: c.rel.clone(),
                    disposition: Disposition::Refused,
                    before: sha,
                    after: None,
                    summary: reason,
                });
            }
        }
    }
    pages.sort_by(|a, b| a.path.cmp(&b.path));
    let input_digest = sha256_hex(digest_input.as_bytes());

    let deltas = semantic_delta(repo_root, &vocab, &roster, profile, &texts, &after_text);

    let mut reasons = Vec::new();
    if migration.kind() == Kind::Syntax && !after_text.is_empty() {
        if !deltas.identity.is_empty()
            || !deltas.resources_added.is_empty()
            || !deltas.resources_removed.is_empty()
        {
            reasons.push(format!(
                "syntax migration changes identity: {} field(s), {} resource(s) added, {} removed",
                deltas.identity.len(),
                deltas.resources_added.len(),
                deltas.resources_removed.len()
            ));
        }
        if !deltas.visibility.is_empty() {
            reasons.push(format!(
                "syntax migration changes visibility of {} page(s)",
                deltas.visibility.len()
            ));
        }
        if !deltas.newly_unsatisfiable.is_empty() {
            reasons.push(format!(
                "syntax migration makes {} class(es) unsatisfiable",
                deltas.newly_unsatisfiable.len()
            ));
        }
        if deltas
            .notes
            .iter()
            .any(|n| n.starts_with("after-compile failed"))
        {
            reasons.push("the knowledge vault no longer compiles after the migration".to_owned());
        }
    }

    Ok(Plan {
        schema: 1,
        migration: MigrationRef::of(migration),
        vaults: {
            let mut v: Vec<String> = vaults.iter().map(|k| k.dir_name().to_owned()).collect();
            v.sort();
            v.dedup();
            v
        },
        input_digest,
        pages_considered: considered.len(),
        unchanged,
        changes,
        refused,
        status: if reasons.is_empty() {
            PlanStatus::Ready
        } else {
            PlanStatus::Refused
        },
        reasons,
        pages,
        deltas,
    })
}

/// A zero inferred delta, when a classifier is built in.
fn zero_inferred() -> Option<InferredDelta> {
    cfg!(feature = "whelk-el").then(InferredDelta::default)
}

fn semantic_delta(
    repo_root: &Path,
    vocab: &Vocabulary,
    roster: &Roster,
    profile: &Profile,
    texts: &[(String, String)],
    after_text: &BTreeMap<String, String>,
) -> SemanticDelta {
    let mut d = SemanticDelta::default();
    if after_text.is_empty() {
        d.compiled = true;
        d.inferred = zero_inferred();
        d.notes
            .push("no page changes: nothing to compile".to_owned());
        return d;
    }

    // Visibility and identity are read straight off the page sources, for both
    // vaults.
    let mut res_before = BTreeSet::new();
    let mut res_after = BTreeSet::new();
    for (rel, before) in texts {
        let Some(new) = after_text.get(rel) else {
            if let Some((_, ids)) = identity_of(vocab, rel, before) {
                res_before.insert(ids[0].1.clone());
                res_after.insert(ids[0].1.clone());
            }
            continue;
        };
        let (Some((pb, ib)), Some((pa, ia))) = (
            identity_of(vocab, rel, before),
            identity_of(vocab, rel, new),
        ) else {
            d.notes.push(format!(
                "after-compile failed: {rel} does not parse before or after"
            ));
            continue;
        };
        if pb != pa {
            d.visibility.push(VisibilityChange {
                path: rel.clone(),
                before: pb,
                after: pa,
            });
        }
        res_before.insert(ib[0].1.clone());
        res_after.insert(ia[0].1.clone());
        for (b, a) in ib.iter().zip(ia.iter()) {
            if b.1 != a.1 {
                d.identity.push(IdentityChange {
                    path: rel.clone(),
                    field: b.0.clone(),
                    before: b.1.clone(),
                    after: a.1.clone(),
                });
            }
        }
    }
    d.resources_added = res_after.difference(&res_before).cloned().collect();
    d.resources_removed = res_before.difference(&res_after).cloned().collect();

    // Triples and inference come from compiling the knowledge vault.
    let knowledge: Vec<SnapshotPage> = texts
        .iter()
        .filter(|(rel, _)| rel.starts_with("knowledge/"))
        .map(|(rel, text)| SnapshotPage {
            rel: rel.clone(),
            text: text.clone(),
        })
        .collect();
    if !after_text.keys().any(|r| r.starts_with("knowledge/")) {
        d.compiled = true;
        d.inferred = zero_inferred();
        d.notes
            .push("no knowledge page changes: ontology compile deltas are zero".to_owned());
        return d;
    }
    let migrated: Vec<SnapshotPage> = knowledge
        .iter()
        .map(|p| SnapshotPage {
            rel: p.rel.clone(),
            text: after_text
                .get(&p.rel)
                .cloned()
                .unwrap_or_else(|| p.text.clone()),
        })
        .collect();
    let before = compile_knowledge(repo_root, &knowledge, profile, vocab, roster);
    let after = compile_knowledge(repo_root, &migrated, profile, vocab, roster);
    match (before, after) {
        (Ok(b), Ok(a)) => {
            d.compiled = true;
            d.asserted = multiset_delta(&b.triples, &a.triples);
            if let (Some(bi), Some(ai)) = (&b.inferred, &a.inferred) {
                d.inferred = Some(pairs_delta(bi, ai));
            } else {
                d.notes
                    .push("no classifier built in: inferred delta not computed".to_owned());
            }
            if let (Some(bu), Some(au)) = (&b.unsatisfiable, &a.unsatisfiable) {
                d.newly_unsatisfiable = au.difference(bu).cloned().collect();
            }
        }
        (Err(e), Ok(_)) => {
            d.notes.push(format!("before-compile failed: {e}"));
        }
        (Ok(_), Err(e)) => {
            d.notes.push(format!("after-compile failed: {e}"));
        }
        (Err(b), Err(a)) => {
            d.notes.push(format!("before-compile failed: {b}"));
            d.notes.push(format!("compile still fails after: {a}"));
        }
    }
    d
}
