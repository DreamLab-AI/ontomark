//! The `ontomark` binary: a drop-in for `VisionClaw`'s `vault` on the verbs the
//! publication workflow and the corpus gates use (`build`, `validate`, `gate`,
//! `conflicts`, `repair bodies|fences`), plus `migrate`.
//!
//! Every subcommand accepts `--json`. Exit codes are contractual and match
//! `vault`: `0` success, `1` a failed check or execution error, `2` a refused
//! operation (a credential on a page selected for publication, two artefacts
//! claiming one path, a definition the reasoner would not finish on) or an
//! invalid argument.
//!
//! Relocated from `VisionClaw` `crates/vault/src/main.rs` at `1044b0722`; the
//! governance verbs (`propose`, `apply`, `panel`, `edit`, `create`) and the
//! graph queries (`find`, `retrieve`, `tree`) stay in `VisionClaw`.

#![deny(unsafe_code)]
#![warn(clippy::pedantic)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;
use clap::{Args, Parser, Subcommand};
use ontomark_compile::check::{bodies, conflicts, gate, repair, validate};
use ontomark_compile::input::page::{load_vault, Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::model::Corpus;
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

/// The three legal `--vault` values. Used as clap's `value_parser` on every
/// subcommand, so an unknown value is rejected by the parser (exit 2) rather
/// than silently falling through to `knowledge`.
pub(crate) const VAULT_VALUES: [&str; 3] = ["knowledge", "working", "all"];

/// The vaults a `--vault` value names.
///
/// `all` means BOTH, everywhere — earlier commands accepted the
/// word and quietly run `knowledge/` only, which is the worst of the three
/// possible behaviours: it looks like it did what you asked.
///
/// The `_` arm is `knowledge` because the parser has already rejected anything
/// that is not one of [`VAULT_VALUES`]; it is not a silent default.
fn vault_kinds(value: &str) -> Vec<VaultKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "working" => vec![VaultKind::Working],
        "all" => vec![VaultKind::Knowledge, VaultKind::Working],
        _ => vec![VaultKind::Knowledge],
    }
}

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
    #[arg(long, default_value = "quick")]
    tier: String,
}

#[derive(Debug, Args)]
struct ConflictsArgs {
    #[command(flatten)]
    selector: VaultSelector,
    /// The severity at or above which a conflict fails the run.
    #[arg(long, default_value = "high")]
    severity: String,
}

#[derive(Debug, Args)]
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

    match &cli.command {
        Command::Validate(args) => {
            let mut failed = false;
            let mut reports = serde_json::Map::new();
            for vault in load(&root, &args.selector.kinds())? {
                let corpus = Corpus::build(&vault, &vocab);
                let report = validate::validate(&vault, &corpus, &vocab);
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
            let tier = gate::Tier::parse(&args.tier);
            let vault = load(&root, &args.selector.kinds())?
                .into_iter()
                .next()
                .context("no vault selected")?;
            let corpus = Corpus::build(&vault, &vocab);
            let report = validate::validate(&vault, &corpus, &vocab);
            let graph = (tier == gate::Tier::Full && report.errors().is_empty())
                .then(|| build::turtle::build_graph(&corpus, &vocab, false));
            let classifier = ontomark_compile::whelk::WhelkEl;
            let verdict = gate::run(
                &report,
                &corpus,
                graph
                    .as_ref()
                    .map(|g| (g, &classifier as &dyn reason::Classifier)),
                tier,
            );
            let value = serde_json::to_value(&verdict)?;
            emit(cli.json, &value, || {
                println!(
                    "[gate:{}] {} — {}",
                    verdict.tier,
                    verdict.gate.to_uppercase(),
                    verdict.caveat
                );
                for check in &verdict.checks {
                    println!("  [{}] {}: {}", check.status, check.name, check.detail);
                }
            })?;
            Ok(ExitCode::from(
                u8::try_from(verdict.exit_code()).unwrap_or(1),
            ))
        }

        Command::Conflicts(args) => {
            let vault = load(&root, &args.selector.kinds())?
                .into_iter()
                .next()
                .context("no vault selected")?;
            let corpus = Corpus::build(&vault, &vocab);
            let report = conflicts::analyse(&corpus, &args.severity);
            let value = serde_json::to_value(&report)?;
            emit(cli.json, &value, || {
                println!("Conflicts: {} total", report.summary.total);
                for conflict in report.conflicts.iter().take(40) {
                    println!(
                        "  [{:?}] {}: {}",
                        conflict.severity, conflict.kind, conflict.detail
                    );
                }
            })?;
            Ok(ExitCode::from(
                u8::try_from(report.exit_code()).unwrap_or(1),
            ))
        }

        Command::Build(args) => {
            // The ontology projections are always built from ONE vault: OWL,
            // the indexes and the graph tiers are `knowledge/`'s by definition.
            // `--vault all` widens the `publish/` staging to `working/`'s
            // `public: true` pages, which is the only thing "both vaults" can
            // mean for a build.
            let kinds = vault_kinds(&args.vault);
            let kind = if kinds == [VaultKind::Working] {
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
            };
            let report = match build::run(&options, &vocab, &ontomark_compile::whelk::WhelkEl) {
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
