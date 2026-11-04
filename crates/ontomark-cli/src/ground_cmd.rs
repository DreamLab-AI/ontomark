//! `ontomark ground`: how much of a question the knowledge vault covers
//! (ADR-OM-012).
//!
//! The caller passes the names the question mentions; extracting them is the
//! agent's job, since this tool calls no model. Each name is grounded with the
//! same vocabulary tiers `ontomark ingest` uses and lands in one bucket:
//! grounded (one entity), unclear (several, or an initials conflict: ask the
//! user which) or uncovered (the corpus has nothing on it). Read-only.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use ontomark_compile::input::page::VaultKind;
use ontomark_compile::input::vocabulary::Vocabulary as VaultVocabulary;
use ontomark_compile::profile::Profile as VaultProfile;
use ontomark_ingest::{Ingest, Options};
use ontomark_model::{Iri, Model};

use crate::ingest_cmd::load_notes;
use crate::ingest_profile;

/// Arguments for `ontomark ground`.
#[derive(Debug, Args)]
pub struct GroundArgs {
    /// A name the question mentions. Repeatable; at least one.
    #[arg(required = true, value_name = "NAME")]
    pub names: Vec<String>,
}

/// Grade the names against the knowledge vault and print the coverage.
///
/// # Errors
/// Loading the vault or deriving the ingest profile.
pub fn run(
    args: &GroundArgs,
    root: &Path,
    vocab: &VaultVocabulary,
    profile: &VaultProfile,
    json: bool,
) -> anyhow::Result<ExitCode> {
    // The knowledge vault only: working notes are private and must not
    // shape what an agent is told the public corpus covers.
    let notes = load_notes(root, profile, &[VaultKind::Knowledge])?;
    let model_profile = ingest_profile::build(vocab)?;
    let model = Model::build(&model_profile, notes.iter().map(|n| (n.id.clone(), &n.doc)));
    let coverage = Ingest::new(&model_profile, &model, Options::default()).coverage(&args.names);

    if json {
        println!("{}", serde_json::to_string(&coverage)?);
        return Ok(ExitCode::SUCCESS);
    }
    match coverage.coverage {
        Some(c) => println!("coverage {c:.2}"),
        None => println!("coverage n/a (no names)"),
    }
    for g in &coverage.grounded {
        println!("  grounded  {} -> {} ({})", g.name, g.label, g.iri.as_str());
    }
    for u in &coverage.unclear {
        let c: Vec<&str> = u.candidates.iter().map(Iri::as_str).collect();
        println!("  unclear   {} -> one of {}", u.name, c.join(", "));
    }
    for u in &coverage.uncovered {
        let s: Vec<&str> = u.suggestions.iter().map(|s| s.label.as_str()).collect();
        if s.is_empty() {
            println!("  uncovered {}", u.name);
        } else {
            println!("  uncovered {} (nearest: {})", u.name, s.join(", "));
        }
    }
    Ok(ExitCode::SUCCESS)
}
