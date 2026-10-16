//! In-memory compilation of a vault snapshot, for the before/after deltas.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::build::attribution::Roster;
use crate::build::turtle::{self, Subject, Term};
use crate::input::page::{Page, Vault, VaultKind};
use crate::input::vocabulary::Vocabulary;
use crate::ir::model::Corpus;
use crate::ir::projection;
use crate::profile::Profile;
#[cfg(feature = "whelk-el")]
use crate::reason;

/// One page of a snapshot: where it lives, and its text.
#[derive(Debug, Clone)]
pub(crate) struct SnapshotPage {
    /// Repository-relative path, `/`-separated.
    pub rel: String,
    /// The page source (lossily decoded when the file is not UTF-8).
    pub text: String,
}

/// What a compile of the knowledge vault yields, reduced to comparable data.
#[derive(Debug, Default)]
pub(crate) struct Compiled {
    /// Canonical asserted triples with multiplicity. Blank-node labels are
    /// replaced by `_:b`, because they are allocation counters that shift when
    /// an unrelated triple appears.
    pub triples: BTreeMap<String, usize>,
    /// Entailed, non-asserted `(sub, sup)` pairs. `None` without a classifier.
    pub inferred: Option<BTreeSet<(String, String)>>,
    /// Unsatisfiable classes. `None` without a classifier.
    pub unsatisfiable: Option<BTreeSet<String>>,
}

fn is_unpublished_page(rel_under_pages: &str) -> bool {
    crate::input::page::is_unpublished(Path::new(rel_under_pages))
}

/// Build a [`Vault`] from in-memory page texts.
pub(crate) fn vault_from(
    repo_root: &Path,
    kind: VaultKind,
    pages: &[SnapshotPage],
) -> Result<Vault, String> {
    let root = repo_root.join(kind.dir_name());
    let mut out = Vault {
        root: root.clone(),
        kind,
        pages: Vec::new(),
        journals: Vec::new(),
        skipped: Vec::new(),
    };
    for p in pages {
        let under_vault = p
            .rel
            .strip_prefix(&format!("{}/", kind.dir_name()))
            .ok_or_else(|| format!("{} is not under {}", p.rel, kind.dir_name()))?;
        let abs: PathBuf = repo_root.join(&p.rel);
        // Pages sit at the vault root; `journals/` is the one sibling tree.
        // The snapshot was read by `read_considered`, which already applied
        // the excluded-folder rule, so nothing else needs filtering here.
        if let Some(r) = under_vault.strip_prefix("journals/") {
            let id = r.trim_end_matches(".md").to_owned();
            let page = Page::parse(&abs, under_vault, id, &p.text).map_err(|e| e.to_string())?;
            out.journals.push(page);
        } else {
            let id = under_vault.trim_end_matches(".md").to_owned();
            let page = Page::parse(&abs, under_vault, id, &p.text).map_err(|e| e.to_string())?;
            out.pages.push(page);
        }
    }
    Ok(out)
}

/// Compile the knowledge vault exactly as `build::run` does up to the
/// reasoner, in memory.
///
/// # Errors
/// A message when a page does not parse, the projection refuses the corpus, or
/// the reasoner refuses the axioms.
pub(crate) fn compile_knowledge(
    repo_root: &Path,
    pages: &[SnapshotPage],
    profile: &Profile,
    vocab: &Vocabulary,
    roster: &Roster,
) -> Result<Compiled, String> {
    let vault = vault_from(repo_root, VaultKind::Knowledge, pages)?;
    let raw = Corpus::build(&vault, vocab);
    let mut excluded: HashSet<String> = HashSet::new();
    for page in &vault.pages {
        if is_unpublished_page(&page.id) {
            excluded.insert(page.id.clone());
            excluded.insert(page.slug());
            if let Some(r) = page.frontmatter.text("resource") {
                excluded.insert(r);
            }
            excluded.insert(page.title());
        }
    }
    let corpus = projection::project(profile, &raw, &excluded).map_err(|e| e.to_string())?;
    let graph = turtle::build_graph_attributed(profile, &corpus, vocab, true, roster);
    let mut triples: BTreeMap<String, usize> = BTreeMap::new();
    for (s, p, o) in graph.iter() {
        let s = match s {
            Subject::Iri(i) => i.clone(),
            Subject::Blank(_) => "_:b".to_owned(),
        };
        let o = match o {
            Term::Blank(_) => "_:b".to_owned(),
            other => format!("{other:?}"),
        };
        *triples.entry(format!("{s} {p} {o}")).or_insert(0) += 1;
    }
    let compiled = Compiled {
        triples,
        ..Compiled::default()
    };
    #[cfg(feature = "whelk-el")]
    {
        let mut compiled = compiled;
        let reasoning =
            reason::reason(&graph, &crate::whelk::WhelkEl).map_err(|e| e.to_string())?;
        compiled.inferred = Some(reasoning.inferred.iter().cloned().collect());
        compiled.unsatisfiable = Some(reasoning.unsatisfiable.iter().cloned().collect());
        Ok(compiled)
    }
    #[cfg(not(feature = "whelk-el"))]
    Ok(compiled)
}
