//! The `ontomark` binary: a drop-in for `VisionClaw`'s `vault` on the verbs the
//! publication workflow and the corpus gates use (`build`, `validate`, `gate`,
//! `conflicts`, `repair bodies|fences`), plus `migrate`.
//!
//! Every subcommand accepts `--json`. Exit codes are contractual and match
//! `vault`: `0` success, `1` a failed check or execution error, `2` a refused
//! operation (a credential on a page selected for publication, two artefacts
//! claiming one path, a definition the reasoner would not finish on, an
//! unsatisfiable class — the corpus does not permit this build; or a
//! `build --scope machine` asked for alongside `--publish-out`, `--vault
//! working|all` or `--no-redact-private-links`, or on a repository without
//! `working/pages`) or an invalid argument (an unknown `--vault`, `--scope`,
//! `--tier` or `--severity` value included).
//!
//! `build --scope machine` is the bundle `VisionClaw` consumes: the public
//! knowledge projection plus `working/`'s public pages as plain records, with
//! no `publish/` tree. `--scope public` (the default) is the publication build.
//!
//! `--vault all` means **both** vaults for every verb that takes it.
//!
//! Relocated from `VisionClaw` `crates/vault/src/main.rs` at `1044b0722`; the
//! governance verbs (`propose`, `apply`, `panel`, `edit`, `create`) and the
//! graph queries (`find`, `retrieve`, `tree`) stay in `VisionClaw`.

#![deny(unsafe_code)]
#![warn(clippy::pedantic)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;
use clap::{Args, Parser, Subcommand, ValueEnum};
use ontomark_compile::check::{bodies, conflicts, gate, repair, validate};
use ontomark_compile::input::page::{load_vault, Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::model::Corpus;
use ontomark_compile::profile::Profile;

mod embed;
mod migrate_cmd;
use migrate_cmd::{vault_kinds, VAULT_VALUES};
use ontomark_compile::{build, reason};

/// Markdown ontology vaults to standard artefacts.
#[derive(Debug, Parser)]
#[command(name = "ontomark", version, about, long_about = None)]
struct Cli {
    /// The repository root holding `ontology/vocabulary.yaml`, `knowledge/`
    /// and `working/`. Defaults to the nearest ancestor that has one.
    #[arg(long, global = true)]
    repo: Option<PathBuf>,

    /// Emit machine-readable JSON instead of prose.
    #[arg(long, global = true)]
    json: bool,

    /// The corpus profile: `estate`, `generic` or the path of a profile YAML.
    /// Defaults to `estate` when the repository has `ontology/vocabulary.yaml`,
    /// else `generic`.
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
            ExitCode::from(1)
        }
    }
}

/// Find the repository root: the nearest ancestor with `ontology/vocabulary.yaml`.
fn resolve_repo(explicit: Option<&Path>) -> anyhow::Result<(PathBuf, Vocabulary)> {
    let start = match explicit {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let (path, vocab) = Vocabulary::discover(&start).with_context(|| {
        format!(
            "looking for ontology/vocabulary.yaml from {}",
            start.display()
        )
    })?;
    let root = path
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or(start);
    Ok((root, vocab))
}

/// Resolve `--profile`: a built-in name or a YAML path.
fn resolve_profile(spec: Option<&str>, root: &Path) -> anyhow::Result<Profile> {
    match spec {
        Some("estate") => Ok(ontomark_profile_estate::profile()),
        Some("generic") => Ok(Profile::generic()),
        Some(path) => {
            let text =
                std::fs::read_to_string(path).with_context(|| format!("reading profile {path}"))?;
            Profile::from_yaml(&text).with_context(|| format!("parsing profile {path}"))
        }
        None if root.join("ontology/vocabulary.yaml").is_file() => {
            Ok(ontomark_profile_estate::profile())
        }
        None => Ok(Profile::generic()),
    }
}

fn load(root: &Path, kinds: &[VaultKind]) -> anyhow::Result<Vec<Vault>> {
    kinds
        .iter()
        .map(|kind| {
            let dir = root.join(kind.dir_name());
            load_vault(&dir).with_context(|| format!("loading {}", dir.display()))
        })
        .collect()
}

fn emit(json: bool, value: &serde_json::Value, prose: impl FnOnce()) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        prose();
    }
    Ok(())
}

// One arm per subcommand. Splitting it would scatter the CLI's dispatch table
// across several functions and make the surface harder to read, not easier.
#[allow(clippy::too_many_lines)]
fn run() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let (root, vocab) = resolve_repo(cli.repo.as_deref())?;
    let profile = resolve_profile(cli.profile.as_deref(), &root)?;

    match &cli.command {
        Command::Migrate { command } => migrate_cmd::run(command, &root, &profile, cli.json),
        Command::Validate(args) => {
            let mut failed = false;
            let mut reports = serde_json::Map::new();
            for vault in load(&root, &args.selector.kinds())? {
                let corpus = Corpus::build(&vault, &vocab);
                let report = validate::validate(&profile, &vault, &corpus, &vocab);
                failed |=
                    !report.errors().is_empty() || (args.strict && !report.warnings().is_empty());
                if !cli.json {
                    println!(
                        "[{}] {} pages, {} with ontology, {} public — {} errors, {} warnings",
                        vault.kind.dir_name(),
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
                    vault.kind.dir_name().to_owned(),
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
            for vault in load(&root, &args.selector.kinds())? {
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
                verdicts.push((vault.kind.dir_name(), verdict));
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
            for vault in load(&root, &args.selector.kinds())? {
                let corpus = Corpus::build(&vault, &vocab);
                reports.push((
                    vault.kind.dir_name(),
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
                vault_root: root.join(kind.dir_name()),
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

        Command::Repair(RepairCommand::Bodies(args)) => {
            let vaults: Vec<&str> = vault_kinds(&args.vault)
                .into_iter()
                .map(VaultKind::dir_name)
                .collect();
            let report = bodies::run(&bodies::Options {
                scopes: repair::scopes_for(&root, &vaults),
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
            let vaults: Vec<&str> = vault_kinds(&args.vault)
                .into_iter()
                .map(VaultKind::dir_name)
                .collect();
            let report = repair::run(&repair::Options {
                scopes: repair::scopes_for(&root, &vaults),
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
