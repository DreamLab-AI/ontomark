//! `ontomark migrate`: plan, apply, roll back and inspect corpus migrations.
//!
//! The lead wires this into `main.rs` with one variant,
//! `Migrate { #[command(subcommand)] command: MigrateCommand }`, dispatching to
//! [`run`]. Everything here goes through `ontomark_compile::migrate`, whose only
//! write path is a guarded, journalled, lossless Markdown patch.
//!
//! Journals under `.ontomark/` hold the original bytes of migrated pages,
//! private ones included: keep that directory out of git and out of publication.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context as _};
use clap::Subcommand;
use ontomark_compile::input::page::VaultKind;
use ontomark_compile::migrate::{self, ApplyOptions, Plan, PlanStatus};
use ontomark_compile::profile::Profile;

/// `ontomark migrate` subcommands.
#[derive(Debug, Subcommand)]
pub enum MigrateCommand {
    /// List the registered migrations.
    List,
    /// Dry run: read the vaults, compute per-page dispositions and the
    /// before/after semantic deltas. Writes nothing but `--out`.
    Plan {
        /// Migration id, for example `m0001-obsidian-bodies`.
        migration: String,
        /// Vaults to consider: `knowledge`, `working` or `all`.
        #[arg(long, default_value = "all", value_parser = ["knowledge", "working", "all"])]
        vault: String,
        /// Write the plan JSON here (needed to apply it).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Apply a plan file. Pages edited since the plan was made are conflicts
    /// and are left alone; an interrupted run is finished by running it again.
    Apply {
        /// The plan JSON written by `plan --out`.
        #[arg(long)]
        plan: PathBuf,
        /// Allow a semantic migration after reviewing the plan's deltas.
        #[arg(long)]
        accept_semantic: bool,
    },
    /// Restore the originals a journal saved, where the page still has the
    /// bytes the migration wrote.
    Rollback {
        /// Migration id.
        migration: String,
        /// The plan's input digest (see `status`).
        digest: String,
    },
    /// Show every journal and where it stands.
    Status,
}

fn kinds(value: &str) -> Vec<VaultKind> {
    match value {
        "knowledge" => vec![VaultKind::Knowledge],
        "working" => vec![VaultKind::Working],
        _ => vec![VaultKind::Knowledge, VaultKind::Working],
    }
}

fn emit(json: bool, value: &serde_json::Value, prose: impl FnOnce()) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        prose();
    }
    Ok(())
}

/// Run one `migrate` subcommand against the repository at `repo_root`,
/// compiling with `profile` (the global `--profile`) for the semantic deltas.
///
/// Exit codes: `0` success; `1` the run left conflicts; `2` the plan is
/// refused.
///
/// # Errors
/// An unknown migration, an unreadable plan or vault, or an I/O failure.
// One arm per subcommand, mirroring the dispatch table in `main.rs`.
#[allow(clippy::too_many_lines)]
pub fn run(
    cmd: &MigrateCommand,
    repo_root: &Path,
    profile: &Profile,
    json: bool,
) -> anyhow::Result<ExitCode> {
    match cmd {
        MigrateCommand::List => {
            let all = migrate::registry();
            let value = serde_json::Value::Array(
                all.iter()
                    .map(|m| {
                        serde_json::json!({
                            "id": m.id(), "version": m.version(),
                            "kind": m.kind().as_str(), "description": m.description(),
                        })
                    })
                    .collect(),
            );
            emit(json, &value, || {
                for m in &all {
                    println!(
                        "{}@{} ({}): {}",
                        m.id(),
                        m.version(),
                        m.kind().as_str(),
                        m.description()
                    );
                }
            })?;
            Ok(ExitCode::SUCCESS)
        }

        MigrateCommand::Plan {
            migration,
            vault,
            out,
        } => {
            let m = migrate::lookup(migration)
                .with_context(|| format!("no migration `{migration}` (see `migrate list`)"))?;
            let plan = migrate::plan(repo_root, &kinds(vault), m.as_ref(), profile)?;
            if let Some(path) = out {
                std::fs::write(path, plan.to_json()?)
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            emit(json, &serde_json::to_value(&plan)?, || {
                print!("{}", plan.report());
            })?;
            Ok(ExitCode::from(if plan.status == PlanStatus::Ready {
                0
            } else {
                2
            }))
        }

        MigrateCommand::Apply {
            plan,
            accept_semantic,
        } => {
            let text = std::fs::read_to_string(plan)
                .with_context(|| format!("reading {}", plan.display()))?;
            let plan = Plan::from_json(&text)?;
            let m = migrate::lookup(&plan.migration.id)
                .with_context(|| format!("no migration `{}`", plan.migration.id))?;
            let opts = ApplyOptions {
                accept_semantic: *accept_semantic,
                ..ApplyOptions::default()
            };
            let report = migrate::apply_with(repo_root, &plan, m.as_ref(), &opts)?;
            emit(json, &serde_json::to_value(&report)?, || {
                println!(
                    "{}: {} applied, {} already applied, {} conflict(s); journal {}",
                    report.migration,
                    report.applied,
                    report.already_applied,
                    report.conflicts.len(),
                    report.journal.display()
                );
                for c in &report.conflicts {
                    println!("  conflict {}: {}", c.path, c.reason);
                }
            })?;
            Ok(ExitCode::from(u8::from(!report.complete)))
        }

        MigrateCommand::Rollback { migration, digest } => {
            if migration.contains(['/', '\\'])
                || digest.contains(['/', '\\'])
                || digest.contains("..")
            {
                bail!("migration id and digest must be plain names");
            }
            let dir = repo_root
                .join(".ontomark/migrations")
                .join(migration)
                .join(digest);
            let report = migrate::rollback(repo_root, &dir)?;
            emit(json, &serde_json::to_value(&report)?, || {
                println!(
                    "{} restored, {} already original, {} conflict(s)",
                    report.restored,
                    report.already_original,
                    report.conflicts.len()
                );
                for c in &report.conflicts {
                    println!("  conflict {}: {}", c.path, c.reason);
                }
            })?;
            Ok(ExitCode::from(u8::from(!report.conflicts.is_empty())))
        }

        MigrateCommand::Status => {
            let all = migrate::status(repo_root)?;
            emit(json, &serde_json::to_value(&all)?, || {
                if all.is_empty() {
                    println!("no migration journals");
                }
                for s in &all {
                    println!(
                        "{} {}  {}  intents {} applied {} conflicts {} restored {}",
                        s.migration,
                        s.input_digest,
                        s.state,
                        s.intents,
                        s.applied,
                        s.conflicts,
                        s.restored
                    );
                }
            })?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
