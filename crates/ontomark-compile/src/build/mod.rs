//! `vault build` — pages in, one generation out (contract C3).
//!
//! The pipeline, in the order it must run:
//!
//! 1. census the inputs (refuses a non-boolean `public` flag);
//! 2. load and project the corpus (redaction precedes the closure, so a
//!    private intermediate class never enters inferred ancestry);
//! 3. validate — **errors block the bundle**;
//! 4. structural closure (`reason.py`'s BFS, whose ordering the scaffold index
//!    encodes) and Whelk's EL++ closure (which `ontology-inferred.ttl` carries);
//! 5. emit every artefact into a staging directory;
//! 6. promote the staging directory atomically.
//!
//! Step 6 matters: a failed build leaves the previous bundle intact, and a
//! successful one never leaves an obsolete page export behind, because the
//! whole tree is replaced rather than written over.

pub mod attribution;
pub mod generation;
pub mod graph_tiers;
pub mod indexes;
pub mod ngg1;
pub mod okf;
pub mod page_api;
pub mod publish;
pub mod records;
pub mod redact;
pub mod turtle;
pub mod webvowl;

pub use generation::Scope;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::input::page::{walk_pages, Page, Vault, VaultKind};
use crate::input::vocabulary::Vocabulary;
use crate::json::{to_compact, to_default_ascii, to_default_utf8, to_indented};
use anyhow::Context as _;
use serde_json::Value;

use crate::check::validate;
use crate::ir::closure;
use crate::ir::model::Corpus;
use crate::ir::projection;
use crate::profile::Profile;
use crate::reason;

/// What to build and where.
#[derive(Debug, Clone)]
pub struct Options {
    /// The vault root, e.g. `<repo>/knowledge`.
    pub vault_root: PathBuf,
    /// The bundle destination.
    pub out: PathBuf,
    /// The repository the generation sha is read from.
    pub repo_root: PathBuf,
    /// Emit `api/markdown/` — the title-form markdown mirror.
    ///
    /// Off by default (decided 2026-09-22): it has no consumer, weighs 124 MB
    /// and the Pages budget is 1 GB.
    pub with_markdown_mirror: bool,
    /// The build instant, `CPython` `isoformat()` shape. `None` reads the
    /// clock; a golden comparison supplies the recorded instant.
    pub generated_at: Option<String>,
    /// The source identity `(commit, dirty)`. `None` asks git.
    pub source: Option<(String, bool)>,
    /// Optional bundle expiry, carried into `.generation.json`.
    pub stale_after: Option<String>,
    /// Write the `publish/` staging to this directory instead of
    /// `<out>/publish`, so the markdown can be promoted separately from the C3
    /// bundle. The credential gate applies either way.
    pub publish_out: Option<PathBuf>,
    /// Also stage `working/`'s `public: true` pages under `publish/`.
    ///
    /// The ontology projections stay `knowledge/`-only regardless: publication
    /// is a per-page decision, OWL membership is not.
    pub with_working_publish: bool,
    /// Redact links to non-public pages in every public surface (EXP-04),
    /// including the machine corpus before closure and Markdown staging.
    ///
    /// `None` takes the profile's `publication.redact_private_links`;
    /// `Some(false)` is the byte-parity switch (`--no-redact-private-links`).
    pub redact_private_links: Option<bool>,
    /// Which consumer the bundle is for. [`Scope::Public`] is today's build;
    /// [`Scope::Machine`] adds `working/`'s public pages as plain records and
    /// writes no `publish/` tree (see [`ScopeConflict`] for what it refuses).
    pub scope: Scope,
    /// Write the RDFS derivation certificate of the published graph into this
    /// directory (`asserted.tsv`, `derivations.tsv`, `asserted.sha256`,
    /// `scope.tsv`, the `oo-cert/1` format), ADR-OM-009. The certified named
    /// closure must equal the classifier's or the build is refused with
    /// [`CertificateParity`]: a generation is never published with a closure
    /// its warrant does not cover. `None` writes nothing and checks nothing.
    pub certificate_out: Option<PathBuf>,
}

/// The certificate's named closure is not the classifier's (ADR-OM-009).
/// Structural today, so this is the review trigger firing, not a flake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateParity {
    /// Named subsumptions the RDFS certificate derives.
    pub certified: usize,
    /// Named subsumptions the classifier infers.
    pub classifier: usize,
}

impl std::fmt::Display for CertificateParity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the derivation certificate does not cover the classifier's closure: {} certified named subsumptions, {} inferred by the classifier; the build is refused (ADR-OM-009)",
            self.certified, self.classifier
        )
    }
}

impl std::error::Error for CertificateParity {}

/// What `--certificate-out` wrote (ADR-OM-009).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CertificateReport {
    /// Where the certificate was written.
    pub dir: PathBuf,
    /// Files, counts and the `asserted.sha256` digest.
    pub summary: crate::certificate::Summary,
    /// Wall time of the fixpoint and the write, milliseconds.
    pub certify_ms: u128,
    /// Named subsumptions the certificate derives.
    pub certified_named_subsumptions: usize,
    /// Named subsumptions the classifier inferred; equal by construction, or
    /// the build would have been refused.
    pub classifier_inferred: usize,
    /// How a reviewer re-checks it.
    pub check_with: String,
}

/// What the build produced.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Report {
    /// The generation stamp.
    pub generation: generation::Generation,
    /// Public OWL classes.
    pub class_count: usize,
    /// Public pages.
    pub page_count: usize,
    /// Asserted triples.
    pub triples: usize,
    /// Inferred `subClassOf` pairs.
    pub inferred_triples: usize,
    /// Search index entries.
    pub search_entries: usize,
    /// Graph-tier node count.
    pub graph_nodes: usize,
    /// Graph-tier resolvable edge count.
    pub graph_edges: usize,
    /// Classes Whelk found unsatisfiable. Always empty in a report that is
    /// returned: a non-empty set refuses the build with [`Unsatisfiable`]
    /// before any report exists. Kept because the JSON report is a published
    /// shape.
    pub unsatisfiable: Vec<String>,
    /// The restriction mode the reasoner ran under (`skip` or `relevant`;
    /// ADR-2124): decided by whether any class carries a `defines-as`.
    pub reasoner_mode: &'static str,
    /// Existential restrictions the reasoner saturated over.
    pub restrictions_kept: usize,
    /// Classes with a `defines-as` definition the reasoner was given.
    pub defined_classes: usize,
    /// Pages staged under `publish/`, per vault. What `--stats` prints, and
    /// the answer to "did `working/` contribute anything?".
    pub published: std::collections::BTreeMap<String, publish::VaultPublished>,
    /// Artefacts written into the bundle. A build that writes none has not
    /// built anything, and the CLI treats that as a failure.
    pub written: usize,
    /// Where the bundle was written.
    pub out: PathBuf,
    /// What private-link redaction changed; `None` when it was off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redaction: Option<redact::RedactionReport>,
    /// What the machine scope added; `None` in a public build.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<MachineSummary>,
    /// The derivation certificate, when `--certificate-out` asked for one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<CertificateReport>,
}

/// What `--scope machine` added to the knowledge projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct MachineSummary {
    /// Plain `working/` records in the bundle.
    pub working_records: usize,
    /// Public `working/` pages dropped because a public knowledge page has the
    /// same identity: the knowledge page wins, as in `VisionClaw`'s merge.
    pub yielded: usize,
}

/// Two artefacts claimed one output path. The build refuses rather than let
/// the second silently overwrite the first — which is exactly how four
/// `api/pages/<slug>.json` records went missing while the generation stamp
/// still recorded the first writer's hash.
///
/// A distinct type so the CLI exits **2**, the code for "the corpus does not
/// permit this build", naming every clash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputCollision {
    /// `(path, first claimant, second claimant)` for every clash, in order.
    pub clashes: Vec<(String, String, String)>,
}

impl std::fmt::Display for OutputCollision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} output path(s) claimed twice; nothing was written: ",
            self.clashes.len()
        )?;
        for (path, first, second) in &self.clashes {
            write!(f, "{path} <- `{first}` and `{second}`; ")?;
        }
        Ok(())
    }
}

impl std::error::Error for OutputCollision {}

/// The reasoner found at least one unsatisfiable class (ADR-2125 item 4). An
/// unsatisfiable class is a broken corpus, never a warning: nothing is written
/// and the root causes (not every class that inherits `owl:Nothing` from them)
/// are named.
///
/// A distinct type so the CLI exits **2**: the corpus does not permit this
/// build, the same refusal class as [`OutputCollision`] (S2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsatisfiable {
    /// Every unsatisfiable class IRI, in reasoner order.
    pub unsatisfiable: Vec<String>,
    /// The subset whose unsatisfiability is not inherited from another
    /// unsatisfiable class; empty when they all sit under one another.
    pub root_causes: Vec<String>,
}

impl std::fmt::Display for Unsatisfiable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const SHOWN: usize = 20;
        let roots = if self.root_causes.is_empty() {
            &self.unsatisfiable
        } else {
            &self.root_causes
        };
        let mut named = roots
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if roots.len() > SHOWN {
            named = format!("{named} (+{} more)", roots.len() - SHOWN);
        }
        write!(
            f,
            "UNSATISFIABLE_CLASSES: {} unsatisfiable class(es); no bundle written. \
             {} root cause(s): {named}",
            self.unsatisfiable.len(),
            roots.len()
        )
    }
}

impl std::error::Error for Unsatisfiable {}

/// `--scope machine` was asked for alongside something it cannot honour, or on
/// a repository it cannot read. The build refuses before reading any page,
/// naming every conflict.
///
/// A distinct type so the CLI exits **2**, the code for "the corpus does not
/// permit this build". The machine bundle's inputs are fixed (`knowledge/` plus
/// `working/`), it never writes the Quartz `publish/` tree, and private-link
/// redaction is not optional in it, so each of those is a refusal rather than a
/// quiet override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeConflict {
    /// One line per conflict, in the order they were found.
    pub conflicts: Vec<String>,
}

impl std::fmt::Display for ScopeConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SCOPE_CONFLICT: `--scope machine` cannot be built as asked; nothing was written: {}",
            self.conflicts.join("; ")
        )
    }
}

impl std::error::Error for ScopeConflict {}

/// Replace `out` with the fully written `stage` directory: `out` is moved
/// aside, `stage` is renamed into its place, and the old tree is removed only
/// once the rename succeeded. On failure the old tree is put back. The window
/// in which `out` does not exist is the gap between two renames, and nothing
/// is ever deleted before the new tree is complete (S2.3).
pub(crate) fn swap_in(stage: &Path, out: &Path) -> std::io::Result<()> {
    let parent = out.parent().unwrap_or(Path::new("."));
    let retired = parent.join(format!(".publication-retired-{}", std::process::id()));
    if retired.exists() {
        std::fs::remove_dir_all(&retired)?;
    }
    if out.exists() {
        std::fs::rename(out, &retired)?;
    }
    match std::fs::rename(stage, out) {
        Ok(()) => {
            if retired.exists() {
                let _ = std::fs::remove_dir_all(&retired);
            }
            Ok(())
        }
        Err(e) => {
            // Put the previous tree back before surfacing the failure.
            if retired.exists() {
                let _ = std::fs::rename(&retired, out);
            }
            let _ = std::fs::remove_dir_all(stage);
            Err(e)
        }
    }
}

/// The claimant named for an artefact the build itself emits, rather than
/// one page.
const BUILD_OWNER: &str = "the build";

/// Files staged for one bundle, kept in memory so the promotion is atomic.
///
/// Every path is **write-once**: a second claim on a path is recorded, not
/// applied, and [`Staged::ensure_single_claims`] refuses the build before
/// anything is stamped or promoted. No emitter can overwrite another silently.
struct Staged {
    files: Vec<(String, Vec<u8>)>,
    owners: HashMap<String, String>,
    clashes: Vec<(String, String, String)>,
    /// Artefacts this build wrote somewhere other than `out` (`--publish-out`),
    /// listed in the generation stamp but not promoted with the bundle (S2.5).
    external: Vec<generation::Artifact>,
}

impl Staged {
    fn new() -> Self {
        Self {
            files: Vec::new(),
            owners: HashMap::new(),
            clashes: Vec::new(),
            external: Vec::new(),
        }
    }

    /// Record an artefact written outside the bundle, so the stamp lists
    /// everything the build produced.
    fn record_external(&mut self, path: impl Into<String>, content: &[u8]) {
        self.external.push(generation::Artifact::of(path, content));
    }

    /// Stage a build-level artefact.
    fn add(&mut self, path: impl Into<String>, content: impl Into<Vec<u8>>) {
        self.add_for(path, content, BUILD_OWNER);
    }

    /// Stage an artefact on behalf of `owner` (a page id, where there is one),
    /// so a clash names both claimants.
    fn add_for(
        &mut self,
        path: impl Into<String>,
        content: impl Into<Vec<u8>>,
        owner: impl Into<String>,
    ) {
        let (path, owner) = (path.into(), owner.into());
        match self.owners.entry(path.clone()) {
            std::collections::hash_map::Entry::Occupied(first) => {
                self.clashes.push((path, first.get().clone(), owner));
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(owner);
                self.files.push((path, content.into()));
            }
        }
    }

    /// Refuse the build if any path was claimed twice.
    fn ensure_single_claims(&self) -> Result<(), OutputCollision> {
        if self.clashes.is_empty() {
            Ok(())
        } else {
            Err(OutputCollision {
                clashes: self.clashes.clone(),
            })
        }
    }

    fn artifacts(&self) -> Vec<generation::Artifact> {
        self.files
            .iter()
            .map(|(name, content)| generation::Artifact::of(name.clone(), content))
            .chain(self.external.iter().cloned())
            .collect()
    }

    /// Write into a fresh staging directory beside `out`, then swap it in
    /// ([`swap_in`]): the previous bundle survives any failure.
    fn promote(&self, out: &Path) -> anyhow::Result<()> {
        let parent = out.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
        let stage = parent.join(format!(".publication-{}", std::process::id()));
        if stage.exists() {
            std::fs::remove_dir_all(&stage)?;
        }
        std::fs::create_dir_all(&stage)?;

        for (name, content) in &self.files {
            let target = stage.join(name);
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&target, content)
                .with_context(|| format!("writing {}", target.display()))?;
        }

        swap_in(&stage, out)
            .with_context(|| format!("promoting the bundle into {}", out.display()))?;
        Ok(())
    }
}

/// An ISO-8601 UTC instant in `CPython`'s `datetime.isoformat()` shape.
#[must_use]
pub fn now_iso() -> String {
    let now = time::OffsetDateTime::now_utc();
    let format = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]+00:00"
    );
    now.format(&format)
        .unwrap_or_else(|_| "1970-01-01T00:00:00.000000+00:00".to_owned())
}

/// Today in `YYYY-MM-DD`, matching Python's `date.today().isoformat()`.
#[must_use]
pub fn today() -> String {
    let now = time::OffsetDateTime::now_utc();
    let format = time::macros::format_description!("[year]-[month]-[day]");
    now.format(&format)
        .unwrap_or_else(|_| "1970-01-01".to_owned())
}

/// Every alias declared by each page, keyed by page id — the search index's
/// `labels` source.
fn alias_map(vault: &Vault) -> HashMap<String, Vec<String>> {
    vault
        .pages
        .iter()
        .map(|p| (p.id.clone(), p.frontmatter.strings("aliases")))
        .collect()
}

/// The working vault root the layout places under the repository, when it
/// declares one. Nothing is checked on disk here.
fn working_root(options: &Options, profile: &Profile) -> Option<PathBuf> {
    profile
        .layout
        .path_of(&options.repo_root, VaultKind::Working)
}

/// Refuse the options and repository `--scope machine` cannot build from,
/// naming every conflict rather than the first.
fn ensure_machine_scope(options: &Options, profile: &Profile) -> Result<(), ScopeConflict> {
    let mut conflicts = Vec::new();
    if options.publish_out.is_some() {
        conflicts.push(
            "`--publish-out`: a machine bundle writes no publish/ tree to put anywhere".to_owned(),
        );
    }
    if options.with_working_publish {
        conflicts.push(
            "`--vault working|all`: the machine bundle always reads knowledge/ and working/"
                .to_owned(),
        );
    }
    if !options
        .redact_private_links
        .unwrap_or(profile.publication.redact_private_links)
    {
        conflicts.push(
            "`--no-redact-private-links` (or a profile with publication.redact_private_links \
             off): the machine bundle is always redacted"
                .to_owned(),
        );
    }
    if profile.layout.kind_of(&options.vault_root) != VaultKind::Knowledge {
        conflicts.push(format!(
            "the vault root {} is not a knowledge vault",
            options.vault_root.display()
        ));
    }
    if let Err(missing) = profile
        .layout
        .root_of(&options.repo_root, VaultKind::Working)
    {
        conflicts.push(format!(
            "a machine bundle reads both vaults, but there is {missing}"
        ));
    }
    if conflicts.is_empty() {
        Ok(())
    } else {
        Err(ScopeConflict { conflicts })
    }
}

/// Run the whole build.
///
/// # Errors
/// The census refusing an input, an identity collision, a validation error
/// (in `working/` too, under [`Scope::Machine`]), a [`ScopeConflict`], a
/// [`reason::ReasoningError`] (a definition the reasoner would not finish
/// on), or any I/O failure while staging or promoting the bundle.
pub fn run(
    options: &Options,
    profile: &Profile,
    vocab: &Vocabulary,
    classifier: &dyn reason::Classifier,
) -> anyhow::Result<Report> {
    run_with(options, profile, vocab, classifier, None)
}

/// [`run`], optionally embedding the corpus into
/// `data/ontology-corpus.records.jsonl` (contract C3, `--with-rvdb`) through
/// the host's [`records::Embedder`]. The compile crate never opens a socket:
/// the embedder is the one place the build reaches the network, and only when
/// the host hands one in.
///
/// # Errors
/// As [`run`], plus the embedder's failure or a vector of the wrong width.
#[allow(clippy::too_many_lines)] // The pipeline, in the order it must run.
pub fn run_with(
    options: &Options,
    profile: &Profile,
    vocab: &Vocabulary,
    classifier: &dyn reason::Classifier,
    embedder: Option<&dyn records::Embedder>,
) -> anyhow::Result<Report> {
    // The machine bundle's inputs are fixed, so what cannot be honoured is
    // refused before a page is read.
    let machine = options.scope == Scope::Machine;
    if machine {
        ensure_machine_scope(options, profile)?;
    }

    // One walk of the vault root feeds the census, the load and the held-page
    // identities (OM-00 step 1): three filters over one listing rather than
    // three listings with three filters that drifted apart. The profile's
    // `layout.excluded` names the root folders that are not pages.
    let excluded = profile.layout.excluded_dirs();
    anyhow::ensure!(
        options.vault_root.is_dir(),
        "{} is not a directory",
        options.vault_root.display()
    );
    let walk = walk_pages(&options.vault_root, &excluded)?;
    let census = projection::inspect_walk(&options.vault_root, &walk)?;
    let vault = Vault::load_with_walk(
        &options.vault_root,
        profile.layout.kind_of(&options.vault_root),
        walk,
    )?;
    let aliases = alias_map(&vault);
    let raw = Corpus::build(&vault, vocab);

    // --- the credential gate, before validation -------------------------------
    //
    // The union of both vaults' `public: true` pages, and the last gate before
    // bytes leave the machine. It runs *before* validation on purpose. A
    // credential on a page selected for publication is a specific, actionable
    // refusal with its own exit code (2), and running it second would let the
    // general "validation rejected N errors" message (exit 1) swallow it — the
    // reader would be told the corpus is invalid rather than that it was about
    // to publish a key. It is also the cheapest check, so failing here skips
    // the reasoner entirely. (The census runs earlier still: it is the input
    // accounting every later step relies on.)
    //
    // The machine bundle reads `working/` as well, and under the same census:
    // a `public` flag that is not a boolean refuses the build there too. Its
    // census is not published (`api/census.json` stays `knowledge/`'s), so the
    // result is dropped once it has passed. Loaded once, here, so the digest
    // lists each of its pages once.
    let working = if machine {
        // `ensure_machine_scope` has already refused a layout without one.
        let root = profile
            .layout
            .root_of(&options.repo_root, VaultKind::Working)?;
        let walk = walk_pages(&root, &excluded)?;
        projection::inspect_walk(&root, &walk)?;
        Some(Vault::load_with_walk(&root, VaultKind::Working, walk)?)
    } else if options.with_working_publish {
        publish::sibling_vault(&options.repo_root, &profile.layout, VaultKind::Working)?
    } else {
        None
    };
    let mut publish_vaults: Vec<&Vault> = vec![&vault];
    publish_vaults.extend(working.as_ref());

    // EXP-04: links to pages that are not public are named in clear text by the
    // raw-Markdown surfaces. Resolution needs both vaults, so the one not
    // already loaded is read here whether or not its pages are published.
    let redact_links = options
        .redact_private_links
        .unwrap_or(profile.publication.redact_private_links);
    let other_vault = if redact_links && working.is_none() {
        let other = match vault.kind {
            VaultKind::Knowledge => VaultKind::Working,
            VaultKind::Working => VaultKind::Knowledge,
        };
        publish::sibling_vault(&options.repo_root, &profile.layout, other)?
    } else {
        None
    };
    let private_links = redact_links.then(|| {
        let mut all: Vec<&Vault> = vec![&vault];
        all.extend(working.as_ref());
        all.extend(other_vault.as_ref());
        redact::PrivateLinks::new(&all)
    });
    let staging = publish::stage_redacted(&publish_vaults, private_links.as_ref())?;

    let report = validate::validate(profile, &vault, &raw, vocab);
    anyhow::ensure!(
        report.errors().is_empty(),
        "validation rejected {} error(s); no bundle written. First: {}",
        report.errors().len(),
        report.errors()[0]
    );

    // `validate` pairs `vault.pages` with the corpus built from it by position,
    // so `working/` is checked on its own, under its own role.
    if machine {
        if let Some(working) = &working {
            let working_report =
                validate::validate(profile, working, &Corpus::build(working, vocab), vocab);
            anyhow::ensure!(
                working_report.errors().is_empty(),
                "validation rejected {} error(s) in working/; no bundle written. First: {}",
                working_report.errors().len(),
                working_report.errors()[0]
            );
        }
    }

    let excluded = projection::excluded_identities_of(&vault);
    let mut merged: Option<Corpus> = None;
    let mut machine_summary = None;
    let (corpus, prose_redactor) =
        if let (true, Some(links), Some(working)) = (machine, &private_links, &working) {
            let projected =
                projection::project_machine(profile, &vault, working, vocab, &excluded, links)?;
            merged = Some(projected.merged());
            machine_summary = Some(MachineSummary {
                working_records: projected.plain.len(),
                yielded: projected.yielded,
            });
            (projected.knowledge, projected.redactor)
        } else if let Some(links) = &private_links {
            projection::project_vault_with_links(profile, &vault, vocab, &excluded, links)?
        } else {
            // Explicit compatibility mode retains the legacy projection bytes.
            projection::project_with_redactor(profile, &raw, &excluded)?
        };

    let structural = closure::compute(profile, &corpus);
    let backlinks = indexes::backlink_index(&corpus);
    let roster = options
        .vault_root
        .parent()
        .map(attribution::Roster::discover)
        .unwrap_or_default();
    let graph = turtle::build_graph_attributed(profile, &corpus, vocab, true, &roster);
    // ADR-2124: a definition over a transitive hierarchy is refused here,
    // before the reasoner runs, with the named `WHELK_RELEVANT_CAP` error —
    // never a hang. The error is returned unwrapped so the CLI can downcast it.
    let reasoning = reason::reason(&graph, classifier)?;
    // ADR-OM-009: the warrant for the closure, from the very graph that is
    // reasoned and published, before anything is written.
    let certificate = match &options.certificate_out {
        Some(dir) => {
            let started = std::time::Instant::now();
            let cert = crate::certificate::certify(&graph);
            let (certified, classifier_n, same) = crate::certificate::parity(&cert, &reasoning);
            if !same {
                return Err(CertificateParity {
                    certified,
                    classifier: classifier_n,
                }
                .into());
            }
            let generation_label = generation::generation_id(
                &profile.generation.prefix_for(&options.repo_root),
                &options.source.as_ref().map_or_else(
                    || generation::git_sha(&options.repo_root),
                    |(c, _)| c.clone(),
                ),
                options.source.as_ref().map_or_else(
                    || generation::git_dirty(&options.repo_root, &[&options.vault_root]),
                    |(_, d)| *d,
                ),
            );
            let summary = cert
                .write(dir, &generation_label)
                .with_context(|| format!("writing the certificate to {}", dir.display()))?;
            Some(CertificateReport {
                dir: dir.clone(),
                summary,
                certify_ms: started.elapsed().as_millis(),
                certified_named_subsumptions: certified,
                classifier_inferred: classifier_n,
                check_with: format!(
                    "oo-cert {d}/asserted.tsv {d}/derivations.tsv",
                    d = dir.display()
                ),
            })
        }
        None => None,
    };
    // ADR-2124 decision 4: the mode is decided by the graph, so say which.
    eprintln!(
        "vault build: whelk restrictions mode {} ({} kept, {} skipped, {} defined class(es))",
        reasoning.mode.as_str(),
        reasoning.restrictions_kept,
        reasoning.restrictions_skipped,
        reasoning.defined_classes
    );
    // ADR-2125 item 4: the zero-unsatisfiable gate. An unsatisfiable class is
    // a broken corpus, never a warning; nothing is written and the root causes
    // (not every class that inherits `owl:Nothing` from them) are named.
    if !reasoning.is_consistent() {
        return Err(Unsatisfiable {
            unsatisfiable: reasoning.unsatisfiable.clone(),
            root_causes: reasoning.root_causes.clone(),
        }
        .into());
    }
    let generated = options.generated_at.clone().unwrap_or_else(now_iso);
    // `today()` in the legacy build: the same instant's date, so one
    // injected clock fixes both.
    let today = generated.get(..10).map_or_else(today, str::to_owned);

    let class_count = corpus.public_classes().count();
    // What the page API and the search index are built from: in the machine
    // scope the knowledge records plus the plain `working/` ones. Everything
    // ontological (Turtle, reasoner, indexes Loom reads, graph tiers, OKF) is
    // built from `corpus`, so a plain record cannot reach it.
    let listed = merged.as_ref().unwrap_or(&corpus);
    let page_count = listed.records.len();

    let mut staged = Staged::new();
    // `publish/` either rides inside the bundle or is promoted on its own —
    // the latter only after the write-once check below passes.
    if machine {
        // The credential gate has scanned `working/`'s public pages; the
        // staged Markdown is not part of this bundle.
    } else if options.publish_out.is_none() {
        for file in &staging.files {
            staged.add_for(file.path.clone(), file.content.clone(), file.path.clone());
        }
    } else {
        // Written to `publish_out` below, so not promoted with the bundle, but
        // still produced by this build and therefore still stamped (S2.5).
        for file in &staging.files {
            staged.record_external(file.path.clone(), file.content.as_bytes());
        }
    }
    staged.add("data/ontology.ttl", turtle::serialise(profile, &graph));
    staged.add(
        "data/ontology-inferred.ttl",
        reason::inferred_turtle(profile, &reasoning, &generated),
    );

    let scaffold = indexes::scaffold_index(&corpus, &structural, &backlinks, &generated);
    staged.add("data/scaffold-index.json", to_compact(&scaffold)?);
    let prose = indexes::prose_index(&corpus, &generated);
    staged.add("data/prose-index.json", to_default_utf8(&prose)?);

    let search = indexes::search_index(listed, &aliases);
    let search_entries = search.as_array().map_or(0, Vec::len);
    staged.add("api/search-index.json", to_indented(&search)?);

    // The page documents' backlinks see the plain records too; the scaffold
    // index above does not (D4), so `data/` is the same in both scopes.
    let page_backlinks = merged.as_ref().map(indexes::backlink_index);
    let (documents, domain_index) = page_api::build_attributed(
        listed,
        &structural,
        page_backlinks.as_ref().unwrap_or(&backlinks),
        &roster,
    );
    for doc in &documents {
        staged.add_for(
            format!("api/pages/{}.json", doc.slug),
            to_indented(&doc.value)?,
            doc.page_id.clone(),
        );
    }
    staged.add("api/pages/_domain-index.json", to_indented(&domain_index)?);
    staged.add("api/census.json", to_indented(&census)?);
    staged.add(
        "api/validation-report.json",
        to_indented(&report.summary())?,
    );
    // The context is served from /ns/v2.jsonld; the property IRIs inside it
    // still cite the profile's vocabulary namespace. Every path ships, byte-identical,
    // because consumers exist for each (contract C3). `/api/schema/context.jsonld`
    // is the old site's pinned URL — its five prefixes are a strict subset of
    // this document — and the publish workflow requires it.
    let context = okf::context(profile, vocab);
    let context_json = to_indented(&context)?;
    staged.add("context/v1.jsonld", context_json.clone());
    staged.add("api/schema/context.jsonld", context_json.clone());
    staged.add("ns/v2.jsonld", context_json);

    // WebVOWL graph — the explorer's build input.
    staged.add(
        "data/ontology.json",
        to_default_ascii(&webvowl::build(profile, &corpus))?,
    );

    // NGG1 graph tiers — the explorer physics worker reads these from the
    // published site, so the `.bin` layout is byte-frozen.
    let tiers = graph_tiers::emit(profile, &corpus, &today);
    for file in &tiers.files {
        staged.add(format!("data/graph/{}", file.name), file.bytes.clone());
    }

    let mut mirror_redaction = redact::SurfaceRedaction::default();
    if options.with_markdown_mirror {
        // A page's own name (its page id, in both `/` encodings) always wins.
        // Its title is written too, because the explorer requests
        // `/api/markdown/<title>.md` with the search-index title, which can
        // differ from the page id (`ai agents` vs `AI Agents`). But a title is
        // only an alias: where it is another page's own name, or two pages
        // share it, the file would answer for the wrong page, so no page gets
        // it as an alias. Those pages keep their id-named file.
        let encodings = |key: &str| {
            let mut v = vec![key.replace('/', "___"), key.replace('/', "%2F")];
            v.dedup();
            v
        };
        let mut owned = std::collections::HashSet::new();
        let mut alias_claims: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for record in corpus.public() {
            owned.extend(encodings(&record.page_id));
            if !record.title.is_empty() && record.title != record.page_id {
                for name in encodings(&record.title) {
                    *alias_claims.entry(name).or_default() += 1;
                }
            }
        }
        let pages_by_id: std::collections::HashMap<&str, &Page> =
            vault.pages.iter().map(|p| (p.id.as_str(), p)).collect();
        for record in corpus.public() {
            // Generated from the projected record, never copied from source: a
            // public page can reference a private one.
            let mut text = String::new();
            // Since the frontmatter migration the definition IS the body's
            // leading paragraph (definition_placement: leading-paragraph), so
            // prepend it only for a page whose body does not already open
            // with it; otherwise every mirrored page states it twice.
            let definition = record.definition.trim();
            let prepend_definition =
                !definition.is_empty() && !record.body.trim_start().starts_with(definition);
            let (mut body_text, mut definition_text) = (record.body.clone(), definition.to_owned());
            if let (Some(links), Some(page)) = (
                private_links.as_ref(),
                pages_by_id.get(record.page_id.as_str()),
            ) {
                // Rewrite the links in the author's text, then apply the same
                // marker masking the projection applied to `record`.
                let mut tally = redact::Tally::default();
                body_text = prose_redactor.redact(&links.body(vault.kind, &page.body, &mut tally));
                if prepend_definition {
                    let raw_definition = page
                        .frontmatter
                        .text("definition")
                        .filter(|d| !d.trim().is_empty())
                        .unwrap_or_else(|| page.leading_paragraph());
                    definition_text = prose_redactor
                        .redact(&links.body(vault.kind, &raw_definition, &mut tally))
                        .trim()
                        .to_owned();
                }
                mirror_redaction.record(&tally);
            }
            if prepend_definition {
                text.push_str(&definition_text);
                text.push_str("\n\n");
            }
            text.push_str(&body_text);
            text.push('\n');
            let mut names = encodings(&record.page_id);
            if !record.title.is_empty() && record.title != record.page_id {
                names.extend(
                    encodings(&record.title)
                        .into_iter()
                        .filter(|name| !owned.contains(name) && alias_claims.get(name) == Some(&1)),
                );
            }
            for name in names {
                staged.add_for(
                    format!("api/markdown/{name}.md"),
                    text.clone(),
                    record.page_id.clone(),
                );
            }
        }
    }

    let (commit, dirty) = if let Some((commit, dirty)) = &options.source {
        (commit.clone(), *dirty)
    } else {
        let commit = generation::git_sha(&options.repo_root);
        // The paths a build reads: the vault, the vocabulary beside it, and the
        // sibling vault whenever it was loaded (for publication, for machine
        // scope, or for link redaction), so a dirty sibling never stamps a
        // clean generation while `content_digest` covers its bytes.
        let vocabulary_dir = options.repo_root.join("ontology");
        let working_root = working_root(options, profile);
        let mut read_paths: Vec<&Path> =
            vec![options.vault_root.as_path(), vocabulary_dir.as_path()];
        if working.is_some() || other_vault.is_some() {
            read_paths.extend(working_root.as_deref());
        }
        let dirty = generation::git_dirty(&options.repo_root, &read_paths);
        (commit, dirty)
    };
    let prefix = profile.generation.prefix_for(&options.repo_root);
    let generation_id = generation::generation_id(&prefix, &commit, dirty);
    for file in okf::bundle(profile, &corpus, vocab, &generation_id) {
        staged.add(format!("okf/{}", file.path), file.content);
    }

    if let Some(embedder) = embedder {
        let mut concept_records = records::concept_records(&scaffold, &prose, &generation_id);
        records::embed_all(&mut concept_records, embedder)
            .context("embedding the corpus for --with-rvdb")?;
        staged.add(records::RECORDS_PATH, records::to_jsonl(&concept_records)?);
    }

    // Write-once: refuse before anything is stamped or promoted.
    staged.ensure_single_claims()?;

    // Every byte the build read, hashed as it was parsed (S2.2): the pages and
    // journals of every vault it loaded (the knowledge vault, `working/` when
    // its pages are staged, the sibling read for link resolution) and the
    // vocabulary, which fixes the Turtle as surely as the pages do.
    let mut sources: Vec<(String, String)> = Vec::new();
    for v in [Some(&vault), working.as_ref(), other_vault.as_ref()]
        .into_iter()
        .flatten()
    {
        for page in v.all_pages() {
            sources.push((
                // The role name, not the folder: the digest's paths are an
                // output contract and stay put when the layout moves.
                format!("{}/{}", v.kind.role_name(), page.rel_path.to_string_lossy()),
                page.source_sha256.clone(),
            ));
        }
    }
    sources.push((
        "ontology/vocabulary.yaml".to_owned(),
        hex::encode(<sha2::Sha256 as sha2::Digest>::digest(
            serde_yaml::to_string(vocab).unwrap_or_default().as_bytes(),
        )),
    ));
    let content_digest = generation::content_digest_of_hashes(sources);
    // Pages alone do not fix the Turtle (the vocabulary and the emitter do
    // too), so the ontology carries its own digest — the tail of its
    // `owl:versionIRI` — recorded here beside the page digest (ADR-2128).
    let ontology_digest = turtle::ontology_digest(profile, &graph);

    let mut gen = generation::generation(
        &prefix,
        commit,
        dirty,
        content_digest,
        ontology_digest,
        generated,
        class_count,
        page_count,
        vocab.version,
        options.stale_after.clone(),
        staged.artifacts(),
    );
    gen.scope = options.scope;
    // Loom serves `data/` and reads its marker from there, verifying each
    // artefact relative to that directory; the root marker keeps the whole
    // bundle's view. Both carry the same identity. A marker cannot list its
    // own hash, so the rule is: no marker lists itself, and the root marker
    // lists the data marker (S2.5).
    let data_marker = to_indented(&gen.scoped("data"))?;
    let mut artifacts = gen.artifacts.clone();
    artifacts.push(generation::Artifact::of(
        "data/.generation.json",
        data_marker.as_bytes(),
    ));
    let root_marker = generation::Generation { artifacts, ..gen };
    staged.add("data/.generation.json", data_marker);
    staged.add(".generation.json", to_indented(&root_marker)?);
    staged.ensure_single_claims()?;
    staged.promote(&options.out)?;
    if let Some(dir) = &options.publish_out {
        publish::write_to(&staging, dir)?;
    }

    Ok(Report {
        generation: root_marker,
        class_count,
        page_count,
        triples: graph.len(),
        inferred_triples: reasoning.inferred.len(),
        search_entries,
        graph_nodes: tiers.nodes,
        graph_edges: tiers.edges,
        reasoner_mode: reasoning.mode.as_str(),
        restrictions_kept: reasoning.restrictions_kept,
        defined_classes: reasoning.defined_classes,
        unsatisfiable: reasoning.unsatisfiable,
        // The machine bundle stages no Markdown, so it publishes nothing.
        published: if machine {
            std::collections::BTreeMap::new()
        } else {
            staging.per_vault
        },
        // Both destinations count: with `--publish-out` the markdown is
        // promoted separately, and it is still something that was written.
        written: staged.files.len()
            + if options.publish_out.is_some() {
                staging.files.len()
            } else {
                0
            },
        out: options.out.clone(),
        redaction: staging
            .redaction
            .clone()
            .map(|publish| redact::RedactionReport {
                publish: if machine {
                    std::collections::BTreeMap::new()
                } else {
                    publish
                },
                mirror: mirror_redaction,
            }),
        machine: machine_summary,
        certificate,
    })
}

/// Read a JSON artefact back out of a bundle, for `--verify` and the tests.
///
/// # Errors
/// I/O or JSON decoding failure.
pub fn read_artifact(out: &Path, name: &str) -> anyhow::Result<Value> {
    let path = out.join(name);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The legacy tests, verbatim, against the default classifier.
    #[cfg(feature = "whelk-el")]
    fn run(options: &Options, vocab: &Vocabulary) -> anyhow::Result<Report> {
        let profile = crate::profile::test_estate();
        super::run(options, &profile, vocab, &crate::whelk::WhelkEl)
    }

    #[cfg(feature = "whelk-el")]
    fn fixture_vault(dir: &Path) {
        let pages = dir.join("knowledge");
        std::fs::create_dir_all(&pages).unwrap();
        std::fs::write(
            pages.join("Root Concept.md"),
            "---\ntype: Class\npublic: true\nresource: urn:ngm:class:root-concept\nstatus: stable\ndomain: infrastructure\ndefinition: The root.\nmaturity: established\nquality: 0.4\n---\n",
        )
        .unwrap();
        std::fs::write(
            pages.join("Knowledge Graph.md"),
            "---\ntype: Class\npublic: true\nresource: urn:ngm:class:knowledge-graph\nstatus: stable\ndomain: infrastructure\ndefinition: A graph.\nmaturity: established\nquality: 0.35\nis-a: [\"[[Root Concept]]\"]\nlinks: [\"[[Root Concept]]\"]\n---\n### Current Landscape (2026)\n\n- Widely adopted.\n",
        )
        .unwrap();
        std::fs::write(
            pages.join("Secret.md"),
            "---\ntype: Class\npublic: false\nresource: urn:ngm:class:secret\nstatus: draft\n---\n",
        )
        .unwrap();
    }

    #[cfg(feature = "whelk-el")]
    fn vocab() -> Vocabulary {
        Vocabulary::from_yaml_str(
            r#"
version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource] }
relations:
  is-a:     { owl: "rdfs:subClassOf", characteristics: [transitive] }
  requires: { owl: "vc:requires" }
scalars:
  domain:     { type: text, enum: [infrastructure] }
  maturity:   { type: text }
  quality:    { type: number, min: 0, max: 1 }
  definition: { type: text }
"#,
        )
        .unwrap()
    }

    #[cfg(feature = "whelk-el")]
    fn build_fixture() -> (tempfile::TempDir, Report) {
        let dir = tempfile::tempdir().unwrap();
        fixture_vault(dir.path());
        let options = Options {
            vault_root: dir.path().join("knowledge"),
            out: dir.path().join("www"),
            repo_root: dir.path().to_path_buf(),
            with_markdown_mirror: false,
            with_working_publish: false,
            redact_private_links: None,
            publish_out: None,
            generated_at: None,
            source: None,
            stale_after: None,
            scope: Scope::Public,
            certificate_out: None,
        };
        let report = run(&options, &vocab()).unwrap();
        (dir, report)
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn emits_every_contract_c3_artefact() {
        let (dir, _) = build_fixture();
        let out = dir.path().join("www");
        for name in [
            "data/ontology.ttl",
            "data/ontology-inferred.ttl",
            "data/scaffold-index.json",
            "data/prose-index.json",
            "api/search-index.json",
            "api/census.json",
            "api/validation-report.json",
            "api/pages/_domain-index.json",
            "context/v1.jsonld",
            "okf/index.md",
            ".generation.json",
        ] {
            assert!(out.join(name).is_file(), "missing {name}");
        }
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn private_pages_reach_no_artefact() {
        let (dir, report) = build_fixture();
        let out = dir.path().join("www");
        assert_eq!(report.class_count, 2);
        assert!(!out.join("api/pages/secret.json").exists());
        let ttl = std::fs::read_to_string(out.join("data/ontology.ttl")).unwrap();
        assert!(!ttl.contains("ngm:secret"));
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn the_scaffold_index_is_compact_and_ascii_only() {
        let (dir, _) = build_fixture();
        let raw = std::fs::read_to_string(dir.path().join("www/data/scaffold-index.json")).unwrap();
        assert!(raw.is_ascii());
        assert!(!raw.contains(": "), "compact separators expected");
        assert!(raw.starts_with(r#"{"version":1,"generated":"#));
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn the_generation_stamps_every_artefact() {
        let (dir, report) = build_fixture();
        let gen: Value = read_artifact(&dir.path().join("www"), ".generation.json").unwrap();
        assert!(gen["id"].as_str().unwrap().starts_with("visionGraph@"));
        assert_eq!(gen["class_count"], 2);
        assert_eq!(gen["vocabulary_version"], 1);
        assert_eq!(gen["content_digest"], report.generation.content_digest);
        let names: Vec<&str> = gen["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"data/scaffold-index.json"));
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn the_generation_records_the_ontology_version_iri_digest() {
        let (dir, report) = build_fixture();
        let out = dir.path().join("www");
        let digest = report.generation.ontology_digest.clone();
        assert!(digest.starts_with("sha256-12-"), "{digest}");
        assert_ne!(digest, report.generation.content_digest);
        for marker in [".generation.json", "data/.generation.json"] {
            let gen: Value = read_artifact(&out, marker).unwrap();
            assert_eq!(gen["ontology_digest"], digest.as_str(), "{marker}");
        }
        let ttl = std::fs::read_to_string(out.join("data/ontology.ttl")).unwrap();
        assert!(
            ttl.contains(&format!(
                "owl:versionIRI <{}/{digest}>",
                crate::profile::test_estate().iris.ontology_iri
            )),
            "the Turtle names the digest .generation.json records"
        );
        let (_again, rebuilt) = build_fixture();
        assert_eq!(
            rebuilt.generation.ontology_digest, digest,
            "identical rebuild"
        );
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn the_data_marker_verifies_every_data_artefact_relative_to_data() {
        let (dir, report) = build_fixture();
        let data = dir.path().join("www/data");
        let marker: Value = read_artifact(&data, ".generation.json").unwrap();
        assert_eq!(marker["id"], report.generation.id.as_str());
        assert_eq!(marker["commit"], report.generation.commit.as_str());
        assert_eq!(
            marker["content_digest"],
            report.generation.content_digest.as_str()
        );
        assert_eq!(marker["class_count"], 2);

        let listed = marker["artifacts"].as_array().unwrap();
        let mut names: Vec<String> = Vec::new();
        for artifact in listed {
            let name = artifact["name"].as_str().unwrap();
            assert!(
                !name.starts_with("data/"),
                "{name} must be relative to data/"
            );
            let bytes = std::fs::read(data.join(name)).unwrap();
            let sha = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&bytes));
            assert_eq!(artifact["sha256"], sha.as_str(), "{name} hash");
            assert_eq!(artifact["bytes"], bytes.len() as u64, "{name} size");
            names.push(name.to_owned());
        }
        assert!(names.contains(&"scaffold-index.json".to_owned()));
        assert!(
            names.iter().any(|n| n.starts_with("graph/")),
            "graph tiers listed"
        );

        // Every file under data/ (bar the marker itself) is accounted for.
        let mut on_disk: Vec<String> = walkdir::WalkDir::new(&data)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                e.path()
                    .strip_prefix(&data)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .filter(|n| n != ".generation.json")
            .collect();
        on_disk.sort();
        names.sort();
        assert_eq!(names, on_disk);
    }

    #[test]
    fn a_second_claim_on_a_path_is_recorded_not_applied() {
        let mut staged = Staged::new();
        staged.add_for("api/pages/x.json", "first", "Page One");
        staged.add_for("api/pages/x.json", "second", "Page Two");
        assert_eq!(staged.files.len(), 1);
        assert_eq!(staged.files[0].1, b"first");
        let clash = staged.ensure_single_claims().unwrap_err();
        assert_eq!(
            clash.clashes,
            vec![(
                "api/pages/x.json".to_owned(),
                "Page One".to_owned(),
                "Page Two".to_owned()
            )]
        );
        assert!(clash.to_string().contains("Page One"));
    }

    #[cfg(feature = "whelk-el")]
    fn build_pages(pages: &[(&str, &str)]) -> (tempfile::TempDir, anyhow::Result<Report>) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("knowledge");
        std::fs::create_dir_all(&root).unwrap();
        for (id, text) in pages {
            std::fs::write(root.join(format!("{id}.md")), text).unwrap();
        }
        let options = Options {
            vault_root: dir.path().join("knowledge"),
            out: dir.path().join("www"),
            repo_root: dir.path().to_path_buf(),
            with_markdown_mirror: true,
            with_working_publish: false,
            redact_private_links: None,
            publish_out: None,
            generated_at: None,
            source: None,
            stale_after: None,
            scope: Scope::Public,
            certificate_out: None,
        };
        let result = run(&options, &vocab());
        (dir, result)
    }

    #[cfg(feature = "whelk-el")]
    fn class_page(extra: &str) -> String {
        format!(
            "---\ntype: Class\npublic: true\nstatus: stable\ndomain: infrastructure\n{extra}---\nA page.\n"
        )
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn the_markdown_mirror_is_written_under_the_title_the_explorer_requests() {
        // The explorer fetches /api/markdown/<search-index title>.md; the title
        // can differ from the file name.
        let (dir, result) = build_pages(&[
            ("AI Agents", &class_page("title: ai agents\n")),
            ("A-B Testing", &class_page("title: A/B Testing\n")),
        ]);
        result.expect("builds");
        let md = dir.path().join("www/api/markdown");
        for name in [
            "AI Agents.md",
            "ai agents.md",
            "A-B Testing.md",
            "A___B Testing.md",
            "A%2FB Testing.md",
        ] {
            assert!(md.join(name).is_file(), "missing {name}");
        }
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn a_title_that_is_another_pages_name_is_not_an_alias() {
        // `bc-cryptographic-primitive` is titled like the page named
        // `Cryptographic Primitive`: the name stays that page's alone.
        let (dir, result) = build_pages(&[
            ("Cryptographic Primitive", &class_page("")),
            (
                "bc-cryptographic-primitive",
                &class_page(
                    "title: Cryptographic Primitive\nresource: urn:ngm:class:bc-cryptographic-primitive\n",
                ),
            ),
        ]);
        result.expect("builds instead of refusing on the shared title");
        let md = dir.path().join("www/api/markdown");
        assert!(md.join("bc-cryptographic-primitive.md").is_file());
        let owner = std::fs::read_to_string(md.join("Cryptographic Primitive.md")).unwrap();
        let other = std::fs::read_to_string(md.join("bc-cryptographic-primitive.md")).unwrap();
        assert_eq!(
            owner, other,
            "fixture bodies are identical; the claim is what is tested"
        );
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn the_markdown_mirror_states_a_leading_paragraph_definition_once() {
        let (dir, result) = build_pages(&[(
            "Once",
            &format!(
                "---\ntype: Class\npublic: true\nstatus: stable\ndomain: infrastructure\n---\n{}\n\nMore text.\n",
                "A page."
            ),
        )]);
        result.expect("builds");
        let text = std::fs::read_to_string(dir.path().join("www/api/markdown/Once.md")).unwrap();
        assert_eq!(text.matches("A page.").count(), 1, "{text}");
        assert!(text.contains("More text."));
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn a_title_slug_colliding_with_a_declared_slug_gets_its_own_file() {
        // The real pair: `ML Experiment Tracking` declares the slug that
        // `Experiment Tracking` derives from its title.
        let (dir, result) = build_pages(&[
            (
                "ML Experiment Tracking",
                &class_page(
                    "slug: experiment-tracking\nresource: urn:ngm:class:experiment-tracking\n",
                ),
            ),
            (
                "Experiment Tracking",
                &class_page("resource: urn:ngm:class:empirical-experimental-design-tracking\n"),
            ),
        ]);
        result.expect("the pair is disambiguated, not refused");
        let out = dir.path().join("www");
        let kept: Value = read_artifact(&out, "api/pages/experiment-tracking.json").unwrap();
        assert_eq!(kept["title"], "ML Experiment Tracking");
        let moved: Value = read_artifact(
            &out,
            "api/pages/empirical-experimental-design-tracking.json",
        )
        .unwrap();
        assert_eq!(moved["title"], "Experiment Tracking");

        // Every artefact the root marker records verifies against its file.
        let marker: Value = read_artifact(&out, ".generation.json").unwrap();
        for artifact in marker["artifacts"].as_array().unwrap() {
            let name = artifact["name"].as_str().unwrap();
            let bytes = std::fs::read(out.join(name)).unwrap();
            let sha = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&bytes));
            assert_eq!(artifact["sha256"], sha.as_str(), "{name}");
        }
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn an_unresolvable_collision_refuses_the_build_and_names_both_pages() {
        // `Foo` loses `foo` to the page declaring it and falls back to its
        // resource tail `bar` — which `Bar` already derives from its title.
        let (dir, result) = build_pages(&[
            (
                "Declared",
                &class_page("slug: foo\nresource: urn:ngm:class:declared\n"),
            ),
            ("Foo", &class_page("resource: urn:ngm:class:bar\n")),
            ("Bar", &class_page("resource: urn:ngm:class:bar-page\n")),
        ]);
        let error = result.expect_err("two pages claim api/pages/bar.json");
        let clash = error
            .downcast_ref::<OutputCollision>()
            .expect("a typed collision, for exit code 2");
        let text = clash.to_string();
        assert!(text.contains("api/pages/bar.json"), "{text}");
        assert!(text.contains("`Foo`") && text.contains("`Bar`"), "{text}");
        assert!(
            !dir.path().join("www").exists(),
            "nothing is promoted when the build refuses"
        );
    }

    // ── ADR-2125 item 4: the zero-unsatisfiable build gate ──────────────────

    #[cfg(feature = "whelk-el")]
    fn build_with_disjointness(
        pages: &[(&str, String)],
    ) -> (tempfile::TempDir, anyhow::Result<Report>) {
        let vocab = Vocabulary::from_yaml_str(
            r#"
version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource] }
relations:
  is-a:          { owl: "rdfs:subClassOf", characteristics: [transitive] }
  disjoint-with: { owl: "owl:disjointWith", status: provisional }
scalars:
  domain:     { type: text, enum: [infrastructure] }
"#,
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("knowledge");
        std::fs::create_dir_all(&root).unwrap();
        for (id, text) in pages {
            std::fs::write(root.join(format!("{id}.md")), text).unwrap();
        }
        let options = Options {
            vault_root: dir.path().join("knowledge"),
            out: dir.path().join("www"),
            repo_root: dir.path().to_path_buf(),
            with_markdown_mirror: false,
            with_working_publish: false,
            redact_private_links: None,
            publish_out: None,
            generated_at: None,
            source: None,
            stale_after: None,
            scope: Scope::Public,
            certificate_out: None,
        };
        let result = run(&options, &vocab);
        (dir, result)
    }

    /// `Parent ⊐ A, B`, `A disjoint-with B`, and `P ⊑ A` (plus `P ⊑ B` when
    /// `clash`), with `Q ⊑ P` hanging below.
    #[cfg(feature = "whelk-el")]
    fn disjoint_corpus(clash: bool) -> Vec<(&'static str, String)> {
        let p_parents = if clash {
            "[\"[[A]]\", \"[[B]]\"]"
        } else {
            "[\"[[A]]\"]"
        };
        vec![
            ("Parent", class_page("resource: urn:ngm:class:parent\n")),
            (
                "A",
                class_page(
                    "resource: urn:ngm:class:a\nis-a: [\"[[Parent]]\"]\ndisjoint-with: [\"[[B]]\"]\n",
                ),
            ),
            ("B", class_page("resource: urn:ngm:class:b\nis-a: [\"[[Parent]]\"]\n")),
            ("P", class_page(&format!("resource: urn:ngm:class:p\nis-a: {p_parents}\n"))),
            ("Q", class_page("resource: urn:ngm:class:q\nis-a: [\"[[P]]\"]\n")),
        ]
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn an_unsatisfiable_class_fails_the_build_naming_only_the_root_cause() {
        let (dir, result) = build_with_disjointness(&disjoint_corpus(true));
        let error = result.expect_err("P ⊑ A ⊓ B with A disjoint B must refuse the build");
        let text = error.to_string();
        assert!(text.contains("UNSATISFIABLE_CLASSES"), "{text}");
        assert!(text.contains("2 unsatisfiable"), "{text}");
        assert!(text.contains("/class/p"), "the root cause is named: {text}");
        let roots = text.split("root cause").nth(1).unwrap_or_default();
        assert!(
            !roots.contains("/class/q"),
            "Q is derived from P, not a root: {text}"
        );
        assert!(
            !dir.path().join("www").exists(),
            "nothing is promoted when the build refuses"
        );
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn a_satisfiable_disjointness_builds_and_emits_the_axiom() {
        let (dir, result) = build_with_disjointness(&disjoint_corpus(false));
        let report = result.expect("siblings A, B disjoint with no common subclass is fine");
        assert_eq!(report.unsatisfiable, [] as [String; 0]);
        let ttl = std::fs::read_to_string(dir.path().join("www/data/ontology.ttl")).unwrap();
        assert!(ttl.contains("owl:disjointWith ngm:b"), "{ttl}");
    }

    /// ADR-2124 end to end: one `defines-as` page, a real `vault build`. The
    /// derived membership lands in `ontology-inferred.ttl` only (decision 5),
    /// and the report carries the restriction mode and kept count.
    #[test]
    #[cfg(feature = "whelk-el")]
    fn a_defined_class_is_classified_into_by_the_build() {
        let vocab = Vocabulary::from_yaml_str(
            r#"
version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource] }
relations:
  is-a:       { owl: "rdfs:subClassOf", characteristics: [transitive] }
  has-part:   { owl: "vc:hasPart", restriction: true }
  defines-as: { owl: "owl:equivalentClass", status: provisional }
scalars:
  domain:     { type: text, enum: [infrastructure] }
"#,
        )
        .unwrap();
        let pages = [
            ("Robot", class_page("resource: urn:ngm:class:robot\n")),
            ("Gripper", class_page("resource: urn:ngm:class:gripper\n")),
            (
                "Arm",
                class_page(
                    "resource: urn:ngm:class:arm\nis-a: [\"[[Robot]]\"]\nhas-part: [\"[[Gripper]]\"]\n",
                ),
            ),
            (
                "Gripping Robot",
                class_page(
                    "resource: urn:ngm:class:gripping-robot\ndefines-as:\n  - \"[[Robot]]\"\n  - has-part: \"[[Gripper]]\"\n",
                ),
            ),
        ];
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("knowledge");
        std::fs::create_dir_all(&root).unwrap();
        for (id, text) in &pages {
            std::fs::write(root.join(format!("{id}.md")), text).unwrap();
        }
        let options = Options {
            vault_root: dir.path().join("knowledge"),
            out: dir.path().join("www"),
            repo_root: dir.path().to_path_buf(),
            with_markdown_mirror: false,
            with_working_publish: false,
            redact_private_links: None,
            publish_out: None,
            generated_at: None,
            source: None,
            stale_after: None,
            scope: Scope::Public,
            certificate_out: None,
        };
        let report = run(&options, &vocab).expect("a defined class builds");
        assert_eq!(report.reasoner_mode, "relevant");
        assert_eq!(report.restrictions_kept, 1);
        assert_eq!(report.defined_classes, 1);
        let www = dir.path().join("www/data");
        let inferred = std::fs::read_to_string(www.join("ontology-inferred.ttl")).unwrap();
        assert!(
            inferred.contains("ngm:arm rdfs:subClassOf ngm:gripping-robot"),
            "{inferred}"
        );
        let asserted = std::fs::read_to_string(www.join("ontology.ttl")).unwrap();
        assert!(asserted.contains("owl:equivalentClass"), "{asserted}");
        assert!(!asserted.contains("ngm:gripping-robot ;\n    rdfs:subClassOf"));
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn a_corpus_without_definitions_reasons_with_skip() {
        let (_dir, report) = build_fixture();
        assert_eq!(report.reasoner_mode, "skip");
        assert_eq!(report.restrictions_kept, 0);
        assert_eq!(report.defined_classes, 0);
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn the_closure_and_the_reasoner_agree_on_the_taxonomy() {
        let (dir, report) = build_fixture();
        let scaffold: Value =
            read_artifact(&dir.path().join("www"), "data/scaffold-index.json").unwrap();
        assert_eq!(
            scaffold["classes"]["knowledge-graph"]["sup"][0],
            "root-concept"
        );
        assert_eq!(report.unsatisfiable, [] as [String; 0]);
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn the_prose_index_captures_the_current_landscape() {
        let (dir, _) = build_fixture();
        let prose: Value = read_artifact(&dir.path().join("www"), "data/prose-index.json").unwrap();
        assert_eq!(prose["pages"]["knowledge-graph"]["cl"], "Widely adopted.");
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn promotion_replaces_the_previous_bundle_wholesale() {
        let (dir, _) = build_fixture();
        let out = dir.path().join("www");
        std::fs::write(out.join("api/pages/stale.json"), "{}").unwrap();
        let options = Options {
            vault_root: dir.path().join("knowledge"),
            out: out.clone(),
            repo_root: dir.path().to_path_buf(),
            with_markdown_mirror: false,
            with_working_publish: false,
            redact_private_links: None,
            publish_out: None,
            generated_at: None,
            source: None,
            stale_after: None,
            scope: Scope::Public,
            certificate_out: None,
        };
        run(&options, &vocab()).unwrap();
        assert!(!out.join("api/pages/stale.json").exists());
    }

    #[test]
    #[cfg(feature = "whelk-el")]
    fn a_validation_error_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fixture_vault(dir.path());
        std::fs::write(
            dir.path().join("knowledge/Duplicate.md"),
            "---\ntype: Class\npublic: true\nresource: urn:ngm:class:root-concept\nstatus: draft\n---\n",
        )
        .unwrap();
        let out = dir.path().join("www");
        let options = Options {
            vault_root: dir.path().join("knowledge"),
            out: out.clone(),
            repo_root: dir.path().to_path_buf(),
            with_markdown_mirror: false,
            with_working_publish: false,
            redact_private_links: None,
            publish_out: None,
            generated_at: None,
            source: None,
            stale_after: None,
            scope: Scope::Public,
            certificate_out: None,
        };
        assert!(run(&options, &vocab()).is_err());
        assert!(!out.exists());
    }

    #[test]
    fn the_timestamp_has_python_isoformat_shape() {
        let s = now_iso();
        assert!(s.ends_with("+00:00"), "{s}");
        assert_eq!(s.len(), "2026-09-22T00:00:00.000000+00:00".len(), "{s}");
    }
}
