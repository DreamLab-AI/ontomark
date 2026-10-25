//! The `ontomark` binary: Markdown ontology vaults to standard artefacts, and
//! the verbs a corpus workflow needs around that (`build`, `validate`, `gate`,
//! `conflicts`, `repair bodies|fences`; the graph queries `find`, `retrieve`,
//! `tree`; the guarded writes `edit`, `create`, `apply`; and `migrate`). It is
//! a drop-in for `VisionClaw`'s `vault` on the verbs both have.
//!
//! # Vault layout
//!
//! A vault is an ordinary Obsidian vault: notes at the vault root, ids are the
//! root-relative path without `.md`, and the furniture folders (`templates/`,
//! `bases/`, `assets/`, `journals/`, plus the profile's `layout.excluded`) are
//! skipped and reported, never silently dropped. `--repo` names either
//!
//! * an **estate-shaped repository** — `ontology/vocabulary.yaml` beside the
//!   vaults the profile's `layout` places (`knowledge/` and `working/` by
//!   default); the estate profile is the default there — or
//! * a **plain Obsidian vault** — no `ontology/` anywhere above it. The
//!   directory itself is the knowledge vault (`layout.knowledge = "."`), there
//!   is no working vault, the generic profile and the built-in vocabulary
//!   apply, and `--vault working|all` or `--scope machine` are refused with
//!   exit 2 naming the layout (ADR-OM-008).
//!
//! The output contract is the same for both: knowledge pages stage under
//! `publish/pages/`, working pages under `publish/working/`, and the generation
//! id is `<prefix>@<commit>` where the prefix is the profile's pin
//! (`visionGraph` for the estate) or the repository's directory name.
//!
//! # Exit codes
//!
//! Every subcommand accepts `--json`. Exit codes are contractual and match
//! `vault`: `0` success, `1` a failed check or execution error, `2` a refused
//! operation (a credential on a page selected for publication, two artefacts
//! claiming one path, a definition the reasoner would not finish on, an
//! unsatisfiable class — the corpus does not permit this build; a
//! `build --scope machine` asked for alongside `--publish-out`, `--vault
//! working|all` or `--no-redact-private-links`, or on a layout without a
//! working vault; any `--vault working|all` on such a layout; a `create` or
//! `apply` whose guards refuse the write) or an invalid argument (an unknown
//! `--vault`, `--scope`, `--tier` or `--severity` value included).
//!
//! `build --scope machine` is the bundle `VisionClaw` consumes: the public
//! knowledge projection plus `working/`'s public pages as plain records, with
//! no `publish/` tree. `--scope public` (the default) is the publication build.
//!
//! `--vault all` means **both** vaults for every verb that takes it.
//!
//! Relocated from `VisionClaw` `crates/vault/src/main.rs` at `1044b0722`
//! (DEPRECATION §2.1 is the flag, JSON and exit-code contract); only the forum
//! transport (`propose`, `panel`) stays in `VisionClaw`.

#![deny(unsafe_code)]
#![warn(clippy::pedantic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;
use clap::{Args, Parser, Subcommand, ValueEnum};
use ontomark_compile::check::{bodies, conflicts, gate, repair, validate};
use ontomark_compile::input::error::VaultError;
use ontomark_compile::input::page::{Vault, VaultKind};
use ontomark_compile::input::promotion::Blocker;
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::graph::{self, VaultGraph};
use ontomark_compile::ir::model::Corpus;
use ontomark_compile::profile::{Layout, MissingVault, Profile};
use ontomark_compile::write::{apply, create, edit};
use serde_json::json;

mod embed;
#[cfg(feature = "jev")]
mod jev_create;
mod migrate_cmd;
use migrate_cmd::{vault_kinds, VAULT_VALUES};
use ontomark_compile::{build, reason};

/// Markdown ontology vaults to standard artefacts.
#[derive(Debug, Parser)]
#[command(name = "ontomark", version, about, long_about = None)]
struct Cli {
    /// The repository root: either an estate-shaped repository holding
    /// `ontology/vocabulary.yaml` beside the vaults the profile's `layout`
    /// names (`knowledge/` and `working/` by default), or a plain Obsidian
    /// vault with its notes at the root and no `ontology/` folder. Defaults to
    /// the nearest ancestor holding `ontology/vocabulary.yaml`, else the
    /// current directory as a plain vault.
    #[arg(long, global = true)]
    repo: Option<PathBuf>,

    /// Emit machine-readable JSON instead of prose.
    #[arg(long, global = true)]
    json: bool,

    /// The corpus profile: `estate`, `generic` or the path of a profile YAML.
    /// Defaults to `estate` when the repository has `ontology/vocabulary.yaml`,
    /// else `generic`. On a plain vault (no vocabulary file) a built-in profile
    /// takes the flat layout — the root is the knowledge vault and there is no
    /// working vault; a profile YAML keeps the `layout` it declares.
    #[arg(long, global = true, value_name = "estate|generic|PATH")]
    profile: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// OKF conformance, vocabulary agreement, link integrity and the public gate.
    Validate(ValidateArgs),
    /// The autonomous continuation gate.
    Gate(GateArgs),
    /// Semantic conflict detection.
    Conflicts(ConflictsArgs),
    /// Pages to one generation.
    Build(BuildArgs),
    /// Search titles and aliases.
    Find(FindArgs),
    /// Expand from seed pages along declared edge types.
    Retrieve(RetrieveArgs),
    /// The taxonomy tree beneath a page.
    Tree(TreeArgs),
    /// Guarded mutation; refused without `--expect`.
    Edit(EditArgs),
    /// Write a new knowledge page from a staged file; never overwrites.
    Create(CreateArgs),
    /// Apply a human-approved `kind: amend` proposal to its page.
    Apply(ApplyArgs),
    /// Fix corpus defects in place.
    #[command(subcommand)]
    Repair(RepairCommand),
    /// Plan, apply, roll back and inspect corpus migrations (reversible,
    /// idempotent, page-level report).
    Migrate {
        #[command(subcommand)]
        command: migrate_cmd::MigrateCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RepairCommand {
    /// Close or remove unmatched code-fence openers.
    Fences(RepairArgs),
    /// Rewrite Logseq outliner bodies (heading bullets, `{:height}` image
    /// sizing, repeated inlined lines) as Obsidian markdown.
    Bodies(BodiesArgs),
}

#[derive(Debug, Args)]
struct BodiesArgs {
    /// Which vault to convert: `knowledge`, `working` or `all`.
    #[arg(long, default_value = "all", value_parser = VAULT_VALUES)]
    vault: String,
    /// Compute everything, write nothing.
    #[arg(long)]
    dry_run: bool,
    /// Write nothing and exit 1 when any page still needs converting — the
    /// residue gate.
    #[arg(long)]
    check: bool,
    /// Write the converted page list here.
    #[arg(long)]
    report: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RepairArgs {
    /// Which vault to repair: `knowledge`, `working` or `all`.
    #[arg(long, default_value = "all", value_parser = VAULT_VALUES)]
    vault: String,
    /// Compute everything, write nothing.
    #[arg(long)]
    dry_run: bool,
    /// Write the full change list here.
    #[arg(long)]
    report: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct VaultSelector {
    /// Which vault to read: `knowledge`, `working` or `all`.
    #[arg(long, default_value = "knowledge", value_parser = VAULT_VALUES)]
    vault: String,
}

impl VaultSelector {
    fn kinds(&self) -> Vec<VaultKind> {
        vault_kinds(&self.vault)
    }
}

/// `build --scope`. A command-line twin of [`build::Scope`], which lives in a
/// crate that must not depend on clap (it compiles to `wasm32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ScopeArg {
    /// The publication build (default): knowledge plus the `publish/` tree.
    Public,
    /// The machine bundle: knowledge plus `working/`'s public pages as plain
    /// records, and no `publish/` tree.
    Machine,
}

impl From<ScopeArg> for build::Scope {
    fn from(scope: ScopeArg) -> Self {
        match scope {
            ScopeArg::Public => Self::Public,
            ScopeArg::Machine => Self::Machine,
        }
    }
}

/// The legal `--tier` values, rejected by the parser otherwise (S2.4).
const TIER_VALUES: [&str; 2] = ["quick", "full"];
/// The legal `--severity` values, rejected by the parser otherwise (S2.4).
const SEVERITY_VALUES: [&str; 4] = ["high", "medium", "low", "all"];

#[derive(Debug, Args)]
struct ValidateArgs {
    #[command(flatten)]
    selector: VaultSelector,
    /// Treat warnings as failures.
    #[arg(long)]
    strict: bool,
}

#[derive(Debug, Args)]
struct FindArgs {
    #[command(flatten)]
    selector: VaultSelector,
    /// The search text.
    #[arg(long)]
    query: String,
    /// Restrict to one OKF `type`.
    #[arg(long, value_name = "T")]
    r#type: Option<String>,
    /// Maximum hits.
    #[arg(long, default_value_t = 20)]
    limit: usize,
    /// Also match on token overlap.
    #[arg(long)]
    fuzzy: bool,
}

#[derive(Debug, Args)]
struct RetrieveArgs {
    #[command(flatten)]
    selector: VaultSelector,
    /// Seed page ids, titles or aliases.
    #[arg(required = true)]
    ids: Vec<String>,
    /// Per-edge-type depths, e.g. `is-a=2,requires=1`. An edge type absent
    /// from this list is never traversed.
    #[arg(long, default_value = "is-a=1")]
    expand: String,
    /// Cap on expanded documents.
    #[arg(long, default_value_t = 50)]
    max_documents: usize,
}

#[derive(Debug, Args)]
struct TreeArgs {
    #[command(flatten)]
    selector: VaultSelector,
    /// The root page.
    id: String,
    /// How many levels to descend.
    #[arg(long, default_value_t = 3)]
    depth: usize,
    /// The edge type to follow, inverted.
    #[arg(long, default_value = "is-a")]
    predicate: String,
}

#[derive(Debug, Args)]
struct EditArgs {
    #[command(flatten)]
    selector: VaultSelector,
    /// The page id.
    id: String,
    /// `key=value`, or `key+=value` to append to a list.
    #[arg(long = "set", value_name = "K=V")]
    sets: Vec<String>,
    /// Remove a key.
    #[arg(long = "unset", value_name = "K")]
    unsets: Vec<String>,
    /// The declared blast radius, e.g. `docs=1,blocks=1`. Required.
    #[arg(long)]
    expect: Option<String>,
    /// Show the result without writing it.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct CreateArgs {
    /// The staged page in full. Its `title` names the file:
    /// `knowledge/<title>.md`.
    file: PathBuf,
    /// `key=value`, or `key+=value` to append to a list, applied in the same
    /// write (e.g. `status=stable`, `verified+={by: …, at: …}`).
    #[arg(long = "set", value_name = "K=V")]
    sets: Vec<String>,
    /// Remove a key from the staged page before writing.
    #[arg(long = "unset", value_name = "K")]
    unsets: Vec<String>,
    /// The declared blast radius: `docs=1`, optionally `,blocks=N` for the
    /// keys `--set`/`--unset` change. Required.
    #[arg(long)]
    expect: Option<String>,
    /// Check everything and report, without writing.
    #[arg(long)]
    dry_run: bool,
    /// Jev option: also print advisory domain, type and parent suggestions for
    /// the new page. Needs `ONTOMARK_JEV=1` and a local endpoint; never
    /// writes or changes what `create` does. Off by default.
    #[cfg(feature = "jev")]
    #[arg(long)]
    suggest: bool,
}

#[derive(Debug, Args)]
struct ApplyArgs {
    /// The approved proposal: the `PatchProposal` JSON (the 31402's content),
    /// or the whole signed 31402 event.
    ///
    /// Only `kind: amend` is applied; a creation goes through `ontomark
    /// create`. The proposal's `digest` must be the one `vault propose`
    /// computes over its fields, its page must exist with the proposal's
    /// `iri`, and its unified diff must apply EXACTLY to the page as it is now
    /// — every context and removed line at the line its hunk states, with no
    /// fuzz and no offset search. That exact context is the staleness guard;
    /// the proposal's `generation` is deliberately not compared, because
    /// generations are routinely `+dirty`. Refusals write nothing and exit 2
    /// with a code: `MALFORMED`, `NOT_AMEND`, `DIGEST_MISMATCH`, `EXPECT`,
    /// `NO_PAGE`, `IRI_MISMATCH`, `STALE`, `INVALID` or `ARGUMENT` (a bad
    /// `--set`).
    file: PathBuf,
    /// `key=value`, or `key+=value` to append to a list, applied to the
    /// patched page in the same write (`edit`'s grammar), e.g.
    /// `verified+={by: human:npub1…, at: 2026-10-05T19:00:00Z}`.
    #[arg(long = "set", value_name = "K=V")]
    sets: Vec<String>,
    /// Remove a key from the patched page in the same write.
    #[arg(long = "unset", value_name = "K")]
    unsets: Vec<String>,
    /// The declared blast radius: `docs=1`, optionally `,blocks=N` for the
    /// keys `--set`/`--unset` change. Required.
    #[arg(long)]
    expect: Option<String>,
}

#[derive(Debug, Args)]
struct GateArgs {
    #[command(flatten)]
    selector: VaultSelector,
    /// `quick` (validate only) or `full` (+ conflicts + Whelk).
    #[arg(long, default_value = "quick", value_parser = TIER_VALUES)]
    tier: String,
}

#[derive(Debug, Args)]
struct ConflictsArgs {
    #[command(flatten)]
    selector: VaultSelector,
    /// The severity at or above which a conflict fails the run: `high`,
    /// `medium`, `low` or `all` (= `low`).
    #[arg(long, default_value = "high", value_parser = SEVERITY_VALUES)]
    severity: String,
}

#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // independent command-line switches
struct BuildArgs {
    /// The bundle destination.
    #[arg(long)]
    out: PathBuf,
    /// Which vault to build: `knowledge`, `working` or `all`.
    ///
    /// `all` keeps the ontology projections on `knowledge/` — OWL membership is
    /// not a per-page decision — and widens the `publish/` staging to
    /// `working/`'s `public: true` pages.
    #[arg(long, default_value = "knowledge", value_parser = VAULT_VALUES)]
    vault: String,
    /// Which consumer the bundle is for: `public` or `machine`.
    ///
    /// `machine` adds `working/`'s `public: true` pages as plain records (no
    /// OWL) to the knowledge projection and writes no `publish/` tree. It fixes
    /// its inputs and always redacts private links, so it refuses (exit 2)
    /// `--publish-out`, `--vault working|all` and `--no-redact-private-links`.
    #[arg(long, value_enum, default_value_t = ScopeArg::Public)]
    scope: ScopeArg,
    /// Print the counts after the build.
    #[arg(long)]
    stats: bool,
    /// Write the RDFS derivation certificate of the published graph into this
    /// directory (`oo-cert/1`: asserted.tsv, derivations.tsv, asserted.sha256,
    /// scope.tsv), ADR-OM-009. An independent checker (`oo-cert`, Lean 4)
    /// re-derives every line. The build is refused if the certified named
    /// closure is not the classifier's.
    #[arg(long)]
    certificate_out: Option<PathBuf>,
    /// Write the `publish/` staging to this directory instead of `<out>/publish`.
    ///
    /// For a publication step that promotes the markdown separately from the
    /// C3 bundle; the credential gate applies either way.
    #[arg(long)]
    publish_out: Option<PathBuf>,
    /// Also emit `api/markdown/` — the title-form markdown mirror. Off by
    /// default: no consumer, 124 MB, and the Pages budget is 1 GB.
    #[arg(long)]
    with_markdown_mirror: bool,
    /// Embed the corpus and emit the portable `data/ontology-corpus.records.jsonl`
    /// (contract C3). Needs the embedder at `--embed-endpoint`; the rest of the
    /// build never does.
    #[arg(long)]
    with_rvdb: bool,
    /// The OpenAI-compatible `/v1/embeddings` endpoint `--with-rvdb` calls
    /// (`bge-small-en-v1.5`, 384 dimensions, locked).
    #[arg(long, default_value = embed::DEFAULT_ENDPOINT, env = "ONTOMARK_EMBED_ENDPOINT")]
    embed_endpoint: String,
    /// An expiry to stamp into `.generation.json`.
    #[arg(long)]
    stale_after: Option<String>,
    /// Stamp this instant (`CPython` `isoformat()` shape) instead of reading
    /// the clock: reproducible builds and golden comparison.
    #[arg(long, env = "ONTOMARK_GENERATED_AT")]
    generated_at: Option<String>,
    /// Stamp this source commit instead of asking git (with `--source-dirty`).
    #[arg(long)]
    source_commit: Option<String>,
    /// With `--source-commit`: record the source as dirty.
    #[arg(long, requires = "source_commit")]
    source_dirty: bool,
    /// Keep links to non-public pages in the staged `publish/` Markdown and
    /// the markdown mirror, as the legacy `vault build` did.
    ///
    /// By default (profile `publication.redact_private_links`) such links are
    /// redacted so a private page's title never reaches the published site.
    /// This switch exists for byte-parity runs against the legacy build only.
    #[arg(long)]
    no_redact_private_links: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ontomark: {e:#}");
            // A vault role the layout does not place (`--vault working` on a
            // plain vault) is a refused request, not a failure: exit 2.
            if e.downcast_ref::<MissingVault>().is_some() {
                return ExitCode::from(2);
            }
            ExitCode::from(1)
        }
    }
}

/// What `--repo` (or the current directory) turned out to be.
struct Repo {
    /// The repository root: the directory holding `ontology/` for an estate
    /// repository, or the vault itself for a plain one.
    root: PathBuf,
    /// The vocabulary found there, or the built-in one for a plain vault.
    vocab: Vocabulary,
    /// `true` when `ontology/vocabulary.yaml` was found: the estate shape.
    estate_shape: bool,
}

/// Find the repository root: the nearest ancestor with
/// `ontology/vocabulary.yaml`. Without one, the start directory is a plain
/// Obsidian vault and is the root itself (ADR-OM-008). A vocabulary file that
/// exists but does not parse is still an error.
fn resolve_repo(explicit: Option<&Path>) -> anyhow::Result<Repo> {
    let start = match explicit {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    match Vocabulary::discover(&start) {
        Ok((path, vocab)) => {
            let root = path
                .parent()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
                .unwrap_or(start);
            Ok(Repo {
                root,
                vocab,
                estate_shape: true,
            })
        }
        Err(VaultError::NotAVault { .. }) => {
            anyhow::ensure!(
                start.is_dir(),
                "{} is not a directory (no ontology/vocabulary.yaml above it either)",
                start.display()
            );
            Ok(Repo {
                root: start,
                vocab: Vocabulary::generic(),
                estate_shape: false,
            })
        }
        Err(e) => Err(e).with_context(|| {
            format!(
                "looking for ontology/vocabulary.yaml from {}",
                start.display()
            )
        }),
    }
}

/// Resolve `--profile`: a built-in name or a YAML path.
///
/// On a plain vault a built-in profile takes [`Layout::flat`]: the root is the
/// knowledge vault and there is no working vault. A profile YAML is taken as
/// written, `layout` included.
fn resolve_profile(spec: Option<&str>, repo: &Repo) -> anyhow::Result<Profile> {
    let mut profile = match spec {
        Some("estate") => ontomark_profile_estate::profile(),
        Some("generic") => Profile::generic(),
        Some(path) => {
            let text =
                std::fs::read_to_string(path).with_context(|| format!("reading profile {path}"))?;
            return Profile::from_yaml(&text).with_context(|| format!("parsing profile {path}"));
        }
        None if repo.estate_shape => ontomark_profile_estate::profile(),
        None => Profile::generic(),
    };
    if !repo.estate_shape {
        profile.layout = Layout {
            excluded: profile.layout.excluded,
            ..Layout::flat()
        };
    }
    Ok(profile)
}

/// Where the vault of `kind` sits, by the profile's layout.
///
/// # Errors
/// [`MissingVault`] (exit 2) when the layout declares no such vault or its
/// folder is absent.
fn vault_root(root: &Path, profile: &Profile, kind: VaultKind) -> anyhow::Result<PathBuf> {
    Ok(profile.layout.root_of(root, kind)?)
}

fn load(root: &Path, profile: &Profile, kinds: &[VaultKind]) -> anyhow::Result<Vec<Vault>> {
    let excluded = profile.layout.excluded_dirs();
    kinds
        .iter()
        .map(|kind| {
            let dir = vault_root(root, profile, *kind)?;
            Vault::load_excluding(&dir, *kind, &excluded)
                .with_context(|| format!("loading {}", dir.display()))
        })
        .collect()
}

/// The repair scopes for the selected vault roles, each placed by the layout.
fn repair_scopes(
    root: &Path,
    profile: &Profile,
    kinds: &[VaultKind],
) -> anyhow::Result<Vec<repair::Scope>> {
    let roots: Vec<(VaultKind, PathBuf)> = kinds
        .iter()
        .map(|kind| Ok((*kind, vault_root(root, profile, *kind)?)))
        .collect::<anyhow::Result<_>>()?;
    let borrowed: Vec<(VaultKind, &Path)> = roots.iter().map(|(k, p)| (*k, p.as_path())).collect();
    Ok(repair::scopes_for(
        &borrowed,
        &profile.layout.excluded_dirs(),
    ))
}

fn emit(json: bool, value: &serde_json::Value, prose: impl FnOnce()) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        prose();
    }
    Ok(())
}

/// Parse `is-a=2,requires=1` into per-edge-type depths.
fn parse_expansion(raw: &str) -> anyhow::Result<BTreeMap<String, usize>> {
    let mut out = BTreeMap::new();
    for part in raw.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (key, depth) = part
            .split_once('=')
            .with_context(|| format!("`{part}` is not `edge-type=depth`"))?;
        out.insert(
            key.trim().to_owned(),
            depth
                .trim()
                .parse()
                .with_context(|| format!("`{depth}` is not a depth"))?,
        );
    }
    Ok(out)
}

fn print_tree(node: &graph::TreeNode, indent: usize) {
    println!("{:indent$}{}", "", node.id, indent = indent * 2);
    for child in &node.children {
        print_tree(child, indent + 1);
    }
}

/// `--set K=V…` and `--unset K…` as one ordered change list.
fn changes_of(sets: &[String], unsets: &[String]) -> Result<Vec<edit::Change>, String> {
    let mut changes: Vec<edit::Change> = sets
        .iter()
        .map(|s| edit::Change::parse_set(s))
        .collect::<Result<_, _>>()?;
    changes.extend(
        unsets
            .iter()
            .map(|k| edit::Change::Unset { key: k.clone() }),
    );
    Ok(changes)
}

/// The `key: before -> after` lines `edit`, `create` and `apply` print.
fn print_changes(changes: &BTreeMap<String, [Option<String>; 2]>) {
    for (key, [before, after]) in changes {
        println!(
            "  {key}: {} -> {}",
            before.as_deref().unwrap_or("(absent)"),
            after.as_deref().unwrap_or("(removed)")
        );
    }
}

// One arm per subcommand. Splitting it would scatter the CLI's dispatch table
// across several functions and make the surface harder to read, not easier.
#[allow(clippy::too_many_lines)]
fn run() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let repo = resolve_repo(cli.repo.as_deref())?;
    let profile = resolve_profile(cli.profile.as_deref(), &repo)?;
    let Repo { root, vocab, .. } = repo;

    match &cli.command {
        Command::Migrate { command } => migrate_cmd::run(command, &root, &profile, cli.json),
        Command::Validate(args) => {
            let mut failed = false;
            let mut reports = serde_json::Map::new();
            for vault in load(&root, &profile, &args.selector.kinds())? {
                let corpus = Corpus::build(&vault, &vocab);
                let report = validate::validate(&profile, &vault, &corpus, &vocab);
                failed |=
                    !report.errors().is_empty() || (args.strict && !report.warnings().is_empty());
                if !cli.json {
                    println!(
                        "[{}] {} pages, {} with ontology, {} public — {} errors, {} warnings",
                        vault.kind.role_name(),
                        report.total_pages,
                        report.pages_with_ontology,
                        report.public_pages,
                        report.errors().len(),
                        report.warnings().len()
                    );
                    for issue in report.errors().iter().take(30) {
                        println!("  {issue}");
                    }
                }
                reports.insert(
                    vault.kind.role_name().to_owned(),
                    serde_json::to_value(report.detailed())?,
                );
            }
            emit(cli.json, &serde_json::Value::Object(reports), || {})?;
            Ok(ExitCode::from(u8::from(failed)))
        }

        Command::Gate(args) => {
            // Every selected vault is gated; `all` is both (S1.5). One vault
            // prints the legacy shape byte for byte (the conformance golden);
            // several print a verdict per vault and a JSON map keyed by vault,
            // and the exit code is the worst verdict.
            let tier = gate::Tier::parse(&args.tier);
            let classifier = ontomark_compile::whelk::WhelkEl;
            let mut verdicts: Vec<(&'static str, gate::Verdict)> = Vec::new();
            for vault in load(&root, &profile, &args.selector.kinds())? {
                let corpus = Corpus::build(&vault, &vocab);
                let report = validate::validate(&profile, &vault, &corpus, &vocab);
                let graph = (tier == gate::Tier::Full && report.errors().is_empty())
                    .then(|| build::turtle::build_graph(&profile, &corpus, &vocab, false));
                let verdict = gate::run(
                    &report,
                    &corpus,
                    graph
                        .as_ref()
                        .map(|g| (g, &classifier as &dyn reason::Classifier)),
                    tier,
                );
                verdicts.push((vault.kind.role_name(), verdict));
            }
            let worst = verdicts
                .iter()
                .map(|(_, v)| v.exit_code())
                .max()
                .unwrap_or(0);
            let several = verdicts.len() > 1;
            let value = if several {
                let mut map = serde_json::Map::new();
                for (name, verdict) in &verdicts {
                    map.insert((*name).to_owned(), serde_json::to_value(verdict)?);
                }
                serde_json::Value::Object(map)
            } else {
                serde_json::to_value(verdicts.first().map(|(_, v)| v))?
            };
            emit(cli.json, &value, || {
                for (name, verdict) in &verdicts {
                    if several {
                        println!(
                            "[gate:{} {name}] {} — {}",
                            verdict.tier,
                            verdict.gate.to_uppercase(),
                            verdict.caveat
                        );
                    } else {
                        println!(
                            "[gate:{}] {} — {}",
                            verdict.tier,
                            verdict.gate.to_uppercase(),
                            verdict.caveat
                        );
                    }
                    for check in &verdict.checks {
                        println!("  [{}] {}: {}", check.status, check.name, check.detail);
                    }
                }
            })?;
            Ok(ExitCode::from(u8::try_from(worst).unwrap_or(1)))
        }

        Command::Conflicts(args) => {
            // Every selected vault is analysed; `all` is both (S1.5). The same
            // one-vault legacy shape / several-vault keyed shape as `gate`.
            let mut reports: Vec<(&'static str, conflicts::Report)> = Vec::new();
            for vault in load(&root, &profile, &args.selector.kinds())? {
                let corpus = Corpus::build(&vault, &vocab);
                reports.push((
                    vault.kind.role_name(),
                    conflicts::analyse(&corpus, &args.severity),
                ));
            }
            let worst = reports
                .iter()
                .map(|(_, r)| r.exit_code())
                .max()
                .unwrap_or(0);
            let several = reports.len() > 1;
            let value = if several {
                let mut map = serde_json::Map::new();
                for (name, report) in &reports {
                    map.insert((*name).to_owned(), serde_json::to_value(report)?);
                }
                serde_json::Value::Object(map)
            } else {
                serde_json::to_value(reports.first().map(|(_, r)| r))?
            };
            emit(cli.json, &value, || {
                for (name, report) in &reports {
                    if several {
                        println!("Conflicts in {name}: {} total", report.summary.total);
                    } else {
                        println!("Conflicts: {} total", report.summary.total);
                    }
                    for conflict in report.conflicts.iter().take(40) {
                        println!(
                            "  [{:?}] {}: {}",
                            conflict.severity, conflict.kind, conflict.detail
                        );
                    }
                }
            })?;
            Ok(ExitCode::from(u8::try_from(worst).unwrap_or(1)))
        }

        Command::Build(args) => {
            // The ontology projections are always built from ONE vault: OWL,
            // the indexes and the graph tiers are `knowledge/`'s by definition.
            // `--vault all` widens the `publish/` staging to `working/`'s
            // `public: true` pages, which is the only thing "both vaults" can
            // mean for a build.
            let kinds = vault_kinds(&args.vault);
            // A machine bundle always reads both vaults from the knowledge
            // root, so `--vault working|all` is left for the build to refuse
            // by name rather than quietly moving the root.
            let machine = args.scope == ScopeArg::Machine;
            let kind = if kinds == [VaultKind::Working] && !machine {
                VaultKind::Working
            } else {
                VaultKind::Knowledge
            };
            let options = build::Options {
                with_working_publish: kinds.contains(&VaultKind::Working)
                    && kind == VaultKind::Knowledge,
                publish_out: args.publish_out.clone(),
                vault_root: vault_root(&root, &profile, kind)?,
                out: args.out.clone(),
                repo_root: root.clone(),
                with_markdown_mirror: args.with_markdown_mirror,
                generated_at: args.generated_at.clone(),
                source: args
                    .source_commit
                    .clone()
                    .map(|commit| (commit, args.source_dirty)),
                stale_after: args.stale_after.clone(),
                redact_private_links: args.no_redact_private_links.then_some(false),
                scope: args.scope.into(),
                certificate_out: args.certificate_out.clone(),
            };
            // `--with-rvdb` is the only network the build ever touches, and the
            // transport lives here, not in the compile crate.
            let embedder: Option<Box<dyn build::records::Embedder>> = args.with_rvdb.then(|| {
                Box::new(embed::Xinference::new(args.embed_endpoint.clone()))
                    as Box<dyn build::records::Embedder>
            });
            let report = match build::run_with(
                &options,
                &profile,
                &vocab,
                &ontomark_compile::whelk::WhelkEl,
                embedder.as_deref(),
            ) {
                Ok(report) => report,
                Err(e) => {
                    // A credential on a page selected for publication is not a
                    // generic failure: it is the corpus refusing to publish
                    // something it must not. Exit 2 denotes this refusal.
                    if let Some(secrets) = e.downcast_ref::<build::publish::SecretsFound>() {
                        eprintln!("ontomark: {secrets}");
                        return Ok(ExitCode::from(2));
                    }
                    // Two artefacts claiming one output path: the same refusal
                    // class — the corpus does not permit this build as it stands.
                    if let Some(clash) = e.downcast_ref::<build::OutputCollision>() {
                        eprintln!("ontomark: {clash}");
                        return Ok(ExitCode::from(2));
                    }
                    // A definition the reasoner would not finish on (ADR-2124):
                    // refused before reasoning, same refusal class.
                    if let Some(cap) = e.downcast_ref::<reason::ReasoningError>() {
                        eprintln!("ontomark: {cap}; no bundle written");
                        return Ok(ExitCode::from(2));
                    }
                    // A machine bundle asked for with something it cannot honour:
                    // the same refusal class, naming every conflict.
                    if let Some(conflict) = e.downcast_ref::<build::ScopeConflict>() {
                        eprintln!("ontomark: {conflict}");
                        return Ok(ExitCode::from(2));
                    }
                    // An unsatisfiable class is a broken corpus: the corpus does
                    // not permit this build, the same refusal class (S2.1).
                    if let Some(unsat) = e.downcast_ref::<build::Unsatisfiable>() {
                        eprintln!("ontomark: {unsat}");
                        return Ok(ExitCode::from(2));
                    }
                    // A closure the certificate does not cover: the corpus is
                    // not published without its warrant (ADR-OM-009).
                    if let Some(parity) = e.downcast_ref::<build::CertificateParity>() {
                        eprintln!("ontomark: {parity}");
                        return Ok(ExitCode::from(2));
                    }
                    return Err(e);
                }
            };
            let value = serde_json::to_value(&report)?;
            emit(cli.json || args.stats, &value, || {
                println!(
                    "{} -> {} ({} artefact(s) written)",
                    report.generation.id,
                    report.out.display(),
                    report.written
                );
                for (vault, counts) in &report.published {
                    println!(
                        "  published {vault}: {} of {} page(s)",
                        counts.published, counts.considered
                    );
                }
            })?;
            // "Nothing built" is a failure, not a success. A build that writes
            // no artefact and exits 0 is indistinguishable from one that worked.
            anyhow::ensure!(
                report.written > 0,
                "the build wrote no artefacts to {}; refusing to report success",
                report.out.display()
            );
            Ok(ExitCode::SUCCESS)
        }

        Command::Find(args) => {
            let mut hits = Vec::new();
            for vault in load(&root, &profile, &args.selector.kinds())? {
                let graph = VaultGraph::build(&vault, &vocab);
                hits.extend(graph.find(
                    &args.query,
                    args.r#type.as_deref(),
                    args.limit,
                    args.fuzzy,
                ));
            }
            graph::sort_hits(&mut hits);
            hits.truncate(args.limit);
            let value = serde_json::to_value(&hits)?;
            emit(cli.json, &value, || {
                for hit in &hits {
                    println!("{:.2}  {}", hit.score, hit.id);
                }
            })?;
            Ok(ExitCode::SUCCESS)
        }

        Command::Retrieve(args) => {
            let expand = parse_expansion(&args.expand)?;
            let vault = load(&root, &profile, &args.selector.kinds())?
                .into_iter()
                .next()
                .context("no vault selected")?;
            let graph = VaultGraph::build(&vault, &vocab);
            let result = graph.retrieve(&args.ids, &expand, args.max_documents);
            let value = serde_json::to_value(&result)?;
            emit(cli.json, &value, || {
                for seed in &result.seeds {
                    println!("seed  {}", seed.id);
                }
                for node in &result.expanded {
                    println!(
                        "  +{}  {}  (via {} from {})",
                        node.depth, node.id, node.via, node.from
                    );
                }
                if result.truncated {
                    println!("(truncated at --max-documents)");
                }
            })?;
            Ok(ExitCode::SUCCESS)
        }

        Command::Tree(args) => {
            let vault = load(&root, &profile, &args.selector.kinds())?
                .into_iter()
                .next()
                .context("no vault selected")?;
            let graph = VaultGraph::build(&vault, &vocab);
            let tree = graph
                .tree(&args.id, &args.predicate, args.depth)
                .with_context(|| format!("no page matching `{}`", args.id))?;
            let value = serde_json::to_value(&tree)?;
            emit(cli.json, &value, || print_tree(&tree, 0))?;
            Ok(ExitCode::SUCCESS)
        }

        Command::Edit(args) => {
            let Some(expect_raw) = &args.expect else {
                anyhow::bail!(
                    "refused: --expect must declare docs and blocks; \
                     e.g. --expect docs=1,blocks=1"
                );
            };
            let expect = edit::Expectation::parse(expect_raw).map_err(anyhow::Error::msg)?;
            let changes = changes_of(&args.sets, &args.unsets).map_err(anyhow::Error::msg)?;

            let vault = load(&root, &profile, &args.selector.kinds())?
                .into_iter()
                .next()
                .context("no vault selected")?;
            let page = vault
                .get(&args.id)
                .with_context(|| format!("no page with id `{}`", args.id))?;
            let outcome = edit::apply(page, &changes, expect).map_err(anyhow::Error::msg)?;
            if !args.dry_run {
                std::fs::write(&page.path, &outcome.rendered)
                    .with_context(|| format!("writing {}", page.path.display()))?;
            }
            let value = serde_json::to_value(&outcome)?;
            emit(cli.json, &value, || {
                println!(
                    "{}: {} document(s), {} key(s){}",
                    outcome.page,
                    outcome.docs,
                    outcome.blocks,
                    if args.dry_run { " (dry run)" } else { "" }
                );
                print_changes(&outcome.changes);
            })?;
            Ok(ExitCode::SUCCESS)
        }

        Command::Create(args) => {
            let expect = edit::Expectation::parse(args.expect.as_deref().unwrap_or(""))
                .map_err(anyhow::Error::msg)?;
            let changes = changes_of(&args.sets, &args.unsets).map_err(anyhow::Error::msg)?;
            let staged = std::fs::read_to_string(&args.file)
                .with_context(|| format!("reading {}", args.file.display()))?;
            let vault = load(&root, &profile, &[VaultKind::Knowledge])?
                .into_iter()
                .next()
                .context("no knowledge vault")?;

            // Every refusal is exit 2 with nothing written, and under `--json`
            // stdout is still one JSON document naming why.
            let refuse = |error: &create::CreateError| -> anyhow::Result<ExitCode> {
                let value = json!({
                    "created": false,
                    "file": args.file.display().to_string(),
                    "code": error.code(),
                    "message": error.to_string(),
                    "blockers": error.blockers(),
                });
                emit(cli.json, &value, || {})?;
                eprintln!("ontomark: {error}");
                Ok(ExitCode::from(2))
            };
            let outcome = match create::prepare(&profile, &vault, &vocab, &staged, &changes, expect)
            {
                Ok(outcome) => outcome,
                Err(error) => return refuse(&error),
            };
            if !args.dry_run {
                if let Err(e) = create::write(&outcome) {
                    return match e.downcast_ref::<create::CreateError>() {
                        Some(error) => refuse(error),
                        None => Err(e),
                    };
                }
            }
            let mut value = serde_json::to_value(&outcome)?;
            value["dry_run"] = json!(args.dry_run);
            #[cfg(feature = "jev")]
            let suggestion = if args.suggest {
                jev_create::suggest(&staged, &outcome.page, &vault, &vocab)
            } else {
                None
            };
            #[cfg(feature = "jev")]
            if let Some(s) = &suggestion {
                value["jev_suggestion"] = jev_create::to_json(s);
            }
            emit(cli.json, &value, || {
                println!(
                    "{} {}{}",
                    if args.dry_run {
                        "would create"
                    } else {
                        "created"
                    },
                    outcome.path,
                    if args.dry_run { " (dry run)" } else { "" }
                );
                print_changes(&outcome.changes);
                #[cfg(feature = "jev")]
                if let Some(s) = &suggestion {
                    jev_create::print(s);
                }
            })?;
            Ok(ExitCode::SUCCESS)
        }

        Command::Apply(args) => {
            // Every refusal is exit 2 with nothing written, and under `--json`
            // stdout is still one JSON document naming why — as `create` does.
            let refuse =
                |code: &str, message: &str, blockers: &[Blocker]| -> anyhow::Result<ExitCode> {
                    let value = json!({
                        "applied": false,
                        "code": code,
                        "message": message,
                        "blockers": blockers,
                    });
                    emit(cli.json, &value, || {})?;
                    eprintln!("ontomark: {message}");
                    Ok(ExitCode::from(2))
                };
            let refuse_with = |error: &apply::ApplyError| {
                refuse(error.code(), &error.to_string(), error.blockers())
            };

            let Some(expect_raw) = &args.expect else {
                return refuse_with(&apply::ApplyError::Expect(
                    "--expect must declare docs=1 for an apply".into(),
                ));
            };
            let expect = match edit::Expectation::parse(expect_raw) {
                Ok(expect) => expect,
                Err(e) => return refuse_with(&apply::ApplyError::Expect(e)),
            };
            let changes = match changes_of(&args.sets, &args.unsets) {
                Ok(changes) => changes,
                Err(e) => return refuse("ARGUMENT", &format!("refused: {e}"), &[]),
            };
            let text = std::fs::read_to_string(&args.file)
                .with_context(|| format!("reading {}", args.file.display()))?;
            let proposal = match apply::read_proposal(&text) {
                Ok(proposal) => proposal,
                Err(error) => return refuse_with(&error),
            };
            let vault = load(&root, &profile, &[VaultKind::Knowledge])?
                .into_iter()
                .next()
                .context("no knowledge vault")?;
            let outcome =
                match apply::prepare(&profile, &vault, &vocab, &proposal, &changes, expect) {
                    Ok(outcome) => outcome,
                    Err(error) => return refuse_with(&error),
                };
            if let Err(e) = apply::write(&outcome) {
                return match e.downcast_ref::<apply::ApplyError>() {
                    Some(error) => refuse_with(error),
                    None => Err(e),
                };
            }
            let value = serde_json::to_value(&outcome)?;
            emit(cli.json, &value, || {
                println!("applied {} to {}", outcome.digest, outcome.path);
                print_changes(&outcome.changes);
            })?;
            Ok(ExitCode::SUCCESS)
        }

        Command::Repair(RepairCommand::Bodies(args)) => {
            let scopes = repair_scopes(&root, &profile, &vault_kinds(&args.vault))?;
            let report = bodies::run(&bodies::Options {
                scopes,
                dry_run: args.dry_run || args.check,
            })?;
            if let Some(path) = &args.report {
                let mut text = serde_json::to_string_pretty(&report)?;
                text.push('\n');
                std::fs::write(path, text)
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            let value = serde_json::to_value(&report)?;
            emit(cli.json, &value, || {
                let verb = if args.check {
                    "need converting"
                } else if args.dry_run {
                    "would be converted"
                } else {
                    "converted"
                };
                println!(
                    "{} page(s) examined, {} {verb}",
                    report.pages_examined, report.pages_converted
                );
                for change in report.changes.iter().take(10) {
                    println!("  {}/{}", change.vault, change.id);
                }
                if report.changes.len() > 10 {
                    println!(
                        "  … {} more; pass --report for the full list",
                        report.changes.len() - 10
                    );
                }
            })?;
            if args.check && report.pages_converted > 0 {
                return Ok(ExitCode::FAILURE);
            }
            Ok(ExitCode::SUCCESS)
        }

        Command::Repair(RepairCommand::Fences(args)) => {
            let scopes = repair_scopes(&root, &profile, &vault_kinds(&args.vault))?;
            let report = repair::run(&repair::Options {
                scopes,
                dry_run: args.dry_run,
            })?;
            if let Some(path) = &args.report {
                let mut text = serde_json::to_string_pretty(&report)?;
                text.push('\n');
                std::fs::write(path, text)
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            let value = serde_json::to_value(&report)?;
            emit(cli.json, &value, || {
                println!(
                    "{} page(s) examined, {} repaired: {} spurious opener(s) removed, \
                     {} closing fence(s) inserted{}",
                    report.pages_examined,
                    report.pages_repaired,
                    report.openers_removed,
                    report.closers_inserted,
                    if args.dry_run { " (dry run)" } else { "" }
                );
                for change in report.changes.iter().take(10) {
                    println!("  {}/{}: {:?}", change.vault, change.id, change.action);
                }
                if report.changes.len() > 10 {
                    println!(
                        "  … {} more; pass --report for the full list",
                        report.changes.len() - 10
                    );
                }
                if !report.ambiguous.is_empty() {
                    println!(
                        "  {} page(s) NOT repaired — the unmatched marker closes the code \
                         above it, so the missing one is earlier in the file and a human \
                         has to place it. These still fail UNTERMINATED_FENCE, by design.",
                        report.ambiguous.len()
                    );
                    for a in report.ambiguous.iter().take(5) {
                        println!("    {}/{}:{}", a.vault, a.id, a.line);
                    }
                }
            })?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
