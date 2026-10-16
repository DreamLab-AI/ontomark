//! `ontomark create` — the apply side of a creation proposal.
//!
//! An **elevation** (a `working/` note promoted to a new knowledge Class) is a
//! new page, so it cannot go through `ontomark edit`, which only mutates a
//! page that exists. `vault propose` files it as `kind: "create"`; once a human
//! promotes the case, the apply path calls
//!
//! ```text
//! ontomark --repo <root> create <staged-page> --expect docs=1 \
//!       --set status=stable --set 'verified+={by: human:npub1…, at: …}' --json
//! ```
//!
//! and the page is written to `knowledge/<title>.md` in one write, the
//! `--set` keys included.
//!
//! # The guards
//!
//! Every refusal writes nothing and exits **2**:
//!
//! * `--expect docs=1` is required — a creation touches exactly one document;
//!   `blocks=N`, when given, must equal the keys `--set`/`--unset` change.
//! * The staged page must parse and must declare its `title`: the title is the
//!   filename, so a page cannot be created without naming itself.
//! * The page must not exist — checked against the loaded vault *and* by an
//!   exclusive create on disk, so a second call (or a race) is refused rather
//!   than overwriting the first.
//! * The result — the staged page with the `--set` keys applied — must pass
//!   validation, and must clear the file/IRI/slug collision checks
//!   ([`creation_blockers`]) the proposal was held to.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::check::validate;
use crate::input::page::{Page, Vault};
use crate::input::promotion::Blocker;
use crate::input::vocabulary::Vocabulary;
use crate::ir::model::Corpus;
use crate::profile::Profile;
use crate::write::edit::{apply_changes, Change, Expectation};

/// Why a creation was refused. Every variant means nothing was written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CreateError {
    /// `--expect` did not declare `docs`.
    #[error("refused: --expect must declare docs=1 for a creation")]
    UndeclaredBlastRadius,
    /// A declared guard disagrees with what the creation does.
    #[error("refused: --expect {guard}={declared}, but the creation touches {actual}")]
    GuardViolated {
        /// `docs` or `blocks`.
        guard: &'static str,
        /// What the caller declared.
        declared: usize,
        /// What the creation actually does.
        actual: usize,
    },
    /// The staged page's frontmatter does not parse.
    #[error("refused: the staged page does not parse: {0}")]
    Unparseable(String),
    /// The staged page declares no `title`, so it has no filename.
    #[error("refused: the staged page declares no `title`, which names its file")]
    Untitled,
    /// A page of that id already exists.
    #[error("refused: `{0}` already exists; a creation never overwrites")]
    Exists(String),
    /// Validation or a collision check refused the page.
    #[error("refused: {} blocker(s) on the created page: {}", .0.len(), join(.0))]
    Blocked(Vec<Blocker>),
}

impl CreateError {
    /// A stable machine code for the refusal, for `--json` consumers.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::UndeclaredBlastRadius => "UNDECLARED_BLAST_RADIUS",
            Self::GuardViolated { .. } => "GUARD_VIOLATED",
            Self::Unparseable(_) => "FRONTMATTER_INVALID",
            Self::Untitled => "UNTITLED",
            Self::Exists(_) => "EXISTS",
            Self::Blocked(_) => "BLOCKED",
        }
    }

    /// The individual blockers, when the refusal carries any.
    #[must_use]
    pub fn blockers(&self) -> &[Blocker] {
        match self {
            Self::Blocked(b) => b,
            _ => &[],
        }
    }
}

fn join(blockers: &[Blocker]) -> String {
    blockers
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// What a creation did (or, before [`write()`], would do).
#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    /// Always `true` in a returned outcome; refusals are errors.
    pub created: bool,
    /// The new page's id — its title.
    pub page: String,
    /// The new page's `resource` IRI.
    pub iri: String,
    /// The file written, relative to the repository root.
    pub path: String,
    /// Documents touched: always 1.
    pub docs: usize,
    /// Frontmatter keys the `--set`/`--unset` arguments changed.
    pub blocks: usize,
    /// Key to `(before, after)` for those keys, rendered as YAML.
    pub changes: BTreeMap<String, [Option<String>; 2]>,
    /// The page's full text.
    #[serde(skip)]
    pub rendered: String,
    /// Where to write it.
    #[serde(skip)]
    pub target: PathBuf,
}

/// `true` when `id` can be a flat, visible knowledge page filename.
#[must_use]
pub fn is_valid_page_filename(id: &str) -> bool {
    !id.trim().is_empty()
        && id.trim() == id
        && !id.starts_with('.')
        && !id.contains(['/', '\\'])
        && !id.chars().any(char::is_control)
}

/// Everything that stops `page` — a page absent from `vault` — from being
/// created, beyond what validation reports.
///
/// * **`FILENAME_INVALID`** — the id cannot be a flat knowledge filename.
/// * **`FILENAME_COLLISION`** — an existing page's id differs only in case,
///   which is the same file on a case-insensitive checkout and the same
///   published URL on a case-folding host.
/// * **`IRI_COLLISION`** — the page's `resource` is an existing page's.
/// * **`SLUG_COLLISION`** — the page's publish slug is an existing page's, or
///   adding it would re-key an existing page's slug. This is the build's own
///   rule: [`Corpus::build`] gives each public record a unique slug by
///   re-keying all but one claimant, so a clash is detected by comparing each
///   existing record's slug before and after the page is added, plus the new
///   page's own derived slug against every existing one.
///
/// `before` is `vault`'s corpus; `after` is the corpus with the page added.
#[must_use]
pub fn creation_blockers(
    vault: &Vault,
    before: &Corpus,
    after: &Corpus,
    vocab: &Vocabulary,
    page: &Page,
) -> Vec<Blocker> {
    let mut out = Vec::new();
    let id = page.id.as_str();
    if !is_valid_page_filename(id) {
        out.push(Blocker::new(
            "FILENAME_INVALID",
            format!("`{id}` cannot be a knowledge page filename (one flat, visible name)"),
        ));
    }
    let lower = id.to_lowercase();
    if let Some(existing) = vault.pages.iter().find(|p| p.id.to_lowercase() == lower) {
        out.push(Blocker::new(
            "FILENAME_COLLISION",
            format!("`{id}` collides with the existing page `{}`", existing.id),
        ));
    }
    let iri = page.resource(&vocab.namespace);
    if let Some(existing) = before.records.iter().find(|r| r.iri == iri) {
        out.push(Blocker::new(
            "IRI_COLLISION",
            format!("`{iri}` is already the resource of `{}`", existing.page_id),
        ));
    }
    let slug = page.slug();
    let mut slug_clash: Vec<&str> = before
        .records
        .iter()
        .filter(|r| r.slug == slug)
        .map(|r| r.page_id.as_str())
        .collect();
    for (old, new) in before.records.iter().zip(&after.records) {
        if old.page_id == new.page_id && old.slug != new.slug {
            slug_clash.push(old.page_id.as_str());
        }
    }
    slug_clash.sort_unstable();
    slug_clash.dedup();
    if !slug_clash.is_empty() {
        out.push(Blocker::new(
            "SLUG_COLLISION",
            format!(
                "the publish slug `{slug}` is already claimed by {}",
                slug_clash
                    .iter()
                    .map(|p| format!("`{p}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    out
}

/// Prepare the creation of `staged` in the knowledge `vault`, with `changes`
/// applied, enforcing `expect`. Nothing is written.
///
/// # Errors
/// [`CreateError`] on any guard, parse, existence, validation or collision
/// failure.
pub fn prepare(
    profile: &Profile,
    vault: &Vault,
    vocab: &Vocabulary,
    staged: &str,
    changes: &[Change],
    expect: Expectation,
) -> Result<Outcome, CreateError> {
    match expect.docs {
        None => return Err(CreateError::UndeclaredBlastRadius),
        Some(1) => {}
        Some(declared) => {
            return Err(CreateError::GuardViolated {
                guard: "docs",
                declared,
                actual: 1,
            })
        }
    }

    let probe = Page::parse("staged.md", "staged.md", "staged", staged)
        .map_err(|e| CreateError::Unparseable(e.to_string()))?;
    let title = probe
        .frontmatter
        .text("title")
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .ok_or(CreateError::Untitled)?;
    if vault.get(&title).is_some() {
        return Err(CreateError::Exists(title));
    }

    // Pages sit at the vault root: `knowledge/<title>.md`.
    let rel_path = PathBuf::from(format!("{title}.md"));
    let target = vault.root.join(&rel_path);
    let mut page = Page::parse(&target, &rel_path, title.clone(), staged)
        .map_err(|e| CreateError::Unparseable(e.to_string()))?;
    let touched = apply_changes(&mut page, changes);
    let blocks = touched.len();
    if let Some(declared) = expect.blocks {
        if declared != blocks {
            return Err(CreateError::GuardViolated {
                guard: "blocks",
                declared,
                actual: blocks,
            });
        }
    }
    let rendered = page
        .render()
        .map_err(|e| CreateError::Unparseable(e.to_string()))?;
    // Validate what will be on disk, not the in-memory form of it.
    let page = Page::parse(&target, &rel_path, title.clone(), &rendered)
        .map_err(|e| CreateError::Unparseable(e.to_string()))?;

    let before = Corpus::build(vault, vocab);
    let mut patched = vault.clone();
    patched.pages.push(page.clone());
    let after = Corpus::build(&patched, vocab);

    let mut blockers = creation_blockers(vault, &before, &after, vocab, &page);
    for issue in validate::validate(profile, &patched, &after, vocab).errors() {
        if issue.path == title {
            let blocker = Blocker::new(issue.code.clone(), issue.message.clone());
            if !blockers.contains(&blocker) {
                blockers.push(blocker);
            }
        }
    }
    if !blockers.is_empty() {
        return Err(CreateError::Blocked(blockers));
    }

    let vault_dir = vault
        .root
        .file_name()
        .map_or_else(String::new, |n| format!("{}/", n.to_string_lossy()));
    Ok(Outcome {
        created: true,
        iri: page.resource(&vocab.namespace),
        path: format!("{vault_dir}{}", rel_path.display()),
        page: title,
        docs: 1,
        blocks,
        changes: touched,
        rendered,
        target,
    })
}

/// Write a prepared creation, **only if the file is absent**.
///
/// The create is exclusive at the filesystem level (`O_EXCL`), so a page that
/// appeared between [`prepare`] and here is refused, never overwritten.
///
/// # Errors
/// [`CreateError::Exists`] when the file already exists; any other I/O
/// failure as `std::io::Error` wrapped in `anyhow`.
pub fn write(outcome: &Outcome) -> anyhow::Result<()> {
    write_new(&outcome.target, &outcome.rendered).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            anyhow::Error::new(CreateError::Exists(outcome.page.clone()))
        } else {
            anyhow::Error::new(e).context(format!("writing {}", outcome.target.display()))
        }
    })
}

fn write_new(path: &Path, text: &str) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(e) = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
    {
        // Never leave a half-written page behind a refusal.
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::page::VaultKind;

    fn vocab() -> Vocabulary {
        Vocabulary::from_yaml_str(
            r#"
version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
scalars:
  quality: { type: number, min: 0, max: 1 }
"#,
        )
        .unwrap()
    }

    const EXISTING: &str = "---\ntype: Class\ntitle: A\npublic: true\nresource: urn:ngm:class:a\nstatus: stable\n---\nbody\n";
    const STAGED: &str = "---\ntype: Class\ntitle: New Thing\npublic: true\nresource: urn:ngm:class:new-thing\nstatus: draft\nis-a:\n- '[[A]]'\n---\nA new class.\n";

    /// The knowledge vault: pages sit at its root.
    const PAGES: &str = "knowledge";

    fn vault_in(dir: &Path) -> Vault {
        let pages = dir.join(PAGES);
        std::fs::create_dir_all(&pages).unwrap();
        std::fs::write(pages.join("A.md"), EXISTING).unwrap();
        Vault::load(dir.join("knowledge"), VaultKind::Knowledge).unwrap()
    }

    fn docs1() -> Expectation {
        Expectation::parse("docs=1").unwrap()
    }

    fn prep(
        vault: &Vault,
        staged: &str,
        changes: &[Change],
        expect: Expectation,
    ) -> Result<Outcome, CreateError> {
        prepare(
            &Profile::generic(),
            vault,
            &vocab(),
            staged,
            changes,
            expect,
        )
    }

    #[test]
    fn a_creation_writes_once_and_refuses_the_second_time() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(dir.path());
        let sets = vec![Change::parse_set("status=stable").unwrap()];
        let outcome = prep(&vault, STAGED, &sets, docs1()).unwrap();
        assert_eq!(outcome.page, "New Thing");
        assert_eq!(outcome.iri, "urn:ngm:class:new-thing");
        assert_eq!(outcome.path, "knowledge/New Thing.md");
        assert_eq!(outcome.blocks, 1);
        write(&outcome).unwrap();
        let target = dir.path().join("knowledge/New Thing.md");
        let written = std::fs::read_to_string(&target).unwrap();
        assert!(written.contains("status: stable"), "{written}");

        // The write itself is exclusive, even with a stale vault.
        let err = write(&outcome).unwrap_err();
        assert_eq!(
            err.downcast_ref::<CreateError>(),
            Some(&CreateError::Exists("New Thing".into()))
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            written,
            "the first write is untouched"
        );

        // A vault that already holds the page refuses at prepare.
        let mut held = vault.clone();
        held.pages
            .push(Page::parse(&target, "New Thing.md", "New Thing", &written).unwrap());
        let err = prep(&held, STAGED, &sets, docs1()).unwrap_err();
        assert_eq!(err, CreateError::Exists("New Thing".into()));
    }

    #[test]
    fn the_blast_radius_must_be_one_document() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(dir.path());
        let none = Expectation::default();
        assert_eq!(
            prep(&vault, STAGED, &[], none).unwrap_err(),
            CreateError::UndeclaredBlastRadius
        );
        let two = Expectation::parse("docs=2").unwrap();
        assert!(matches!(
            prep(&vault, STAGED, &[], two).unwrap_err(),
            CreateError::GuardViolated { guard: "docs", .. }
        ));
        let blocks = Expectation::parse("docs=1,blocks=2").unwrap();
        let sets = vec![Change::parse_set("status=stable").unwrap()];
        assert!(matches!(
            prep(&vault, STAGED, &sets, blocks).unwrap_err(),
            CreateError::GuardViolated {
                guard: "blocks",
                ..
            }
        ));
    }

    #[test]
    fn an_invalid_result_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(dir.path());
        // Unsetting a required key makes the result invalid.
        let unset = vec![Change::Unset {
            key: "status".into(),
        }];
        let err = prep(&vault, STAGED, &unset, docs1()).unwrap_err();
        assert_eq!(err.code(), "BLOCKED", "{err}");
        assert!(!dir.path().join("knowledge/New Thing.md").exists());

        let err = prep(&vault, "---\ntitle: [x\n---\n", &[], docs1()).unwrap_err();
        assert_eq!(err.code(), "FRONTMATTER_INVALID");
        let untitled = STAGED.replace("title: New Thing\n", "");
        assert_eq!(
            prep(&vault, &untitled, &[], docs1()).unwrap_err(),
            CreateError::Untitled
        );
    }

    #[test]
    fn an_iri_collision_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(dir.path());
        let clash = STAGED.replace("urn:ngm:class:new-thing", "urn:ngm:class:a");
        let err = prep(&vault, &clash, &[], docs1()).unwrap_err();
        assert!(
            err.blockers().iter().any(|b| b.code == "IRI_COLLISION"),
            "{err}"
        );
    }

    #[test]
    fn a_case_variant_of_an_existing_id_is_a_filename_collision() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(dir.path());
        let clash = STAGED
            .replace("title: New Thing", "title: a")
            .replace("urn:ngm:class:new-thing", "urn:ngm:class:a-lower");
        let err = prep(&vault, &clash, &[], docs1()).unwrap_err();
        let codes: Vec<&str> = err.blockers().iter().map(|b| b.code.as_str()).collect();
        assert!(codes.contains(&"FILENAME_COLLISION"), "{codes:?}");
        assert!(codes.contains(&"SLUG_COLLISION"), "{codes:?}");
    }

    #[test]
    fn filename_rule() {
        assert!(is_valid_page_filename("New Thing"));
        for bad in ["", " x", ".hidden", "a/b", "a\\b", "a\tb"] {
            assert!(!is_valid_page_filename(bad), "{bad:?}");
        }
    }
}
