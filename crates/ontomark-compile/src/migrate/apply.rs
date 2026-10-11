//! Guarded apply, rollback and status.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use super::journal::{self, Event};
use super::plan::{Disposition, MigrationRef, Plan, PlanStatus};
use super::{lookup, sha256_hex, Kind, MigrateError, Migration, Transform};

/// Options for [`apply_with`].
#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    /// Permit a [`Kind::Semantic`] migration: the caller has reviewed the
    /// plan's deltas.
    pub accept_semantic: bool,
    /// **Fault injection for tests.** After this many pages have been written
    /// in this run, stop with [`MigrateError::Interrupted`], as a crash would.
    pub stop_after: Option<usize>,
    /// **Fault injection for tests.** With [`ApplyOptions::stop_after`] `= Some(n)`,
    /// also write page `n + 1` but die before its `applied` record, the worst
    /// moment a crash can pick.
    pub die_before_commit_record: bool,
}

/// A page the run left alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    /// Repository-relative path.
    pub path: String,
    /// Why it was left alone.
    pub reason: String,
}

/// The outcome of [`apply`].
#[derive(Debug, Clone, Serialize)]
pub struct ApplyReport {
    /// The migration, as `id@version`.
    pub migration: String,
    /// The plan's input digest.
    pub input_digest: String,
    /// Where the journal lives.
    pub journal: PathBuf,
    /// Pages written by this run.
    pub applied: usize,
    /// Pages found already at the planned bytes.
    pub already_applied: usize,
    /// Pages left alone.
    pub conflicts: Vec<Conflict>,
    /// `true` when every planned change is in place.
    pub complete: bool,
}

fn note_conflict(
    log: &mut journal::Writer,
    report: &mut ApplyReport,
    path: &str,
    reason: String,
) -> Result<(), MigrateError> {
    log.append(&Event::Conflict {
        path: path.to_owned(),
        reason: reason.clone(),
    })?;
    report.conflicts.push(Conflict {
        path: path.to_owned(),
        reason,
    });
    Ok(())
}

fn safe_rel(rel: &str) -> bool {
    let p = Path::new(rel);
    !rel.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Apply `plan` with the registry's migration of the same id.
///
/// # Errors
/// As [`apply_with`], plus [`MigrateError::UnknownMigration`].
pub fn apply(repo_root: &Path, plan: &Plan) -> Result<ApplyReport, MigrateError> {
    let m = lookup(&plan.migration.id)
        .ok_or_else(|| MigrateError::UnknownMigration(plan.migration.id.clone()))?;
    apply_with(repo_root, plan, m.as_ref(), &ApplyOptions::default())
}

/// Apply `plan` using `migration`, which must be the migration (id, version
/// and kind) the plan was made by.
///
/// Each page is guarded by its planned digests. A page whose current digest is
/// the planned `after` is skipped. A page whose current digest is neither is a
/// [`Conflict`] and is not written. Otherwise the page is re-transformed, the
/// result must hash to the planned `after`, the original is saved, an `intent`
/// is journalled, and the file is replaced atomically. Re-running with the same
/// plan after an interruption therefore continues where it stopped.
///
/// # Errors
/// [`MigrateError::PlanMismatch`], [`MigrateError::PlanRefused`],
/// [`MigrateError::SemanticNotAccepted`], I/O failures, or an injected
/// [`MigrateError::Interrupted`].
pub fn apply_with(
    repo_root: &Path,
    plan: &Plan,
    migration: &dyn Migration,
    opts: &ApplyOptions,
) -> Result<ApplyReport, MigrateError> {
    let current = MigrationRef::of(migration);
    if current.id != plan.migration.id
        || current.version != plan.migration.version
        || current.kind != plan.migration.kind
    {
        return Err(MigrateError::PlanMismatch {
            plan: plan.migration.label(),
            registry: current.label(),
        });
    }
    if plan.status == PlanStatus::Refused {
        return Err(MigrateError::PlanRefused(plan.reasons.clone()));
    }
    if migration.kind() == Kind::Semantic && !opts.accept_semantic {
        return Err(MigrateError::SemanticNotAccepted(migration.id().to_owned()));
    }

    let dir = journal::journal_dir(repo_root, migration.id(), &plan.input_digest);
    let mut log = journal::Writer::open(&dir)?;
    journal::write_atomic(&dir.join("plan.json"), plan.to_json()?.as_bytes())?;
    log.append(&Event::Begin {
        migration: migration.id().to_owned(),
        version: migration.version(),
        input_digest: plan.input_digest.clone(),
        time: journal::now(),
    })?;

    let mut report = ApplyReport {
        migration: current.label(),
        input_digest: plan.input_digest.clone(),
        journal: dir.clone(),
        applied: 0,
        already_applied: 0,
        conflicts: Vec::new(),
        complete: false,
    };
    for page in plan
        .pages
        .iter()
        .filter(|p| p.disposition == Disposition::Change)
    {
        let (Some(after_sha), true) = (page.after.as_deref(), safe_rel(&page.path)) else {
            note_conflict(
                &mut log,
                &mut report,
                &page.path,
                "malformed plan entry".to_owned(),
            )?;
            continue;
        };
        if opts.stop_after == Some(report.applied) && !opts.die_before_commit_record {
            return Err(MigrateError::Interrupted(report.applied));
        }
        let path = repo_root.join(&page.path);
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_symlink() => {
                note_conflict(
                    &mut log,
                    &mut report,
                    &page.path,
                    "is a symbolic link".to_owned(),
                )?;
                continue;
            }
            Ok(_) => {}
            Err(e) => {
                note_conflict(
                    &mut log,
                    &mut report,
                    &page.path,
                    format!("unreadable: {e}"),
                )?;
                continue;
            }
        }
        let bytes = std::fs::read(&path).map_err(|e| MigrateError::io(&path, e))?;
        let now = sha256_hex(&bytes);
        if now == after_sha {
            log.append(&Event::Recovered {
                path: page.path.clone(),
                after: now,
            })?;
            report.already_applied += 1;
            continue;
        }
        if now != page.before {
            note_conflict(
                &mut log,
                &mut report,
                &page.path,
                "edited since the plan was made (stale patch invalidated)".to_owned(),
            )?;
            continue;
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            note_conflict(
                &mut log,
                &mut report,
                &page.path,
                "not valid UTF-8".to_owned(),
            )?;
            continue;
        };
        let new = match migration.transform(&page.path, text) {
            Transform::Changed { source, .. } if sha256_hex(source.as_bytes()) == after_sha => {
                source
            }
            _ => {
                note_conflict(
                    &mut log,
                    &mut report,
                    &page.path,
                    "the migration no longer yields the planned bytes".to_owned(),
                )?;
                continue;
            }
        };
        // Original first, durable, then the intent, then the swap.
        let orig = dir.join("originals").join(&page.before);
        if !orig.exists() {
            journal::write_atomic(&orig, &bytes)?;
        }
        log.append(&Event::Intent {
            path: page.path.clone(),
            before: page.before.clone(),
            after: after_sha.to_owned(),
        })?;
        journal::write_atomic(&path, new.as_bytes())?;
        if opts.die_before_commit_record && opts.stop_after == Some(report.applied) {
            return Err(MigrateError::Interrupted(report.applied + 1));
        }
        log.append(&Event::Applied {
            path: page.path.clone(),
            after: after_sha.to_owned(),
        })?;
        report.applied += 1;
    }
    report.complete = report.conflicts.is_empty();
    log.append(&Event::Complete {
        applied: report.applied,
        conflicts: report.conflicts.len(),
    })?;
    Ok(report)
}

/// The outcome of [`rollback`].
#[derive(Debug, Clone, Serialize)]
pub struct RollbackReport {
    /// Pages restored to their original bytes and verified.
    pub restored: usize,
    /// Pages already at their original bytes.
    pub already_original: usize,
    /// Pages left alone because they were edited after the migration.
    pub conflicts: Vec<Conflict>,
}

/// Restore the originals recorded in the journal directory `journal`.
///
/// A page is restored only if its current digest is the one the migration
/// wrote; a page edited since is reported and left alone. Every restored file
/// is re-read and must hash to the planned `before`.
///
/// # Errors
/// An unreadable journal or original, or an I/O failure.
pub fn rollback(repo_root: &Path, journal: &Path) -> Result<RollbackReport, MigrateError> {
    let events = journal::read(journal)?;
    let mut seen = BTreeSet::new();
    let mut intents = Vec::new();
    for e in &events {
        if let Event::Intent {
            path,
            before,
            after,
        } = e
        {
            if seen.insert(path.clone()) {
                intents.push((path.clone(), before.clone(), after.clone()));
            }
        }
    }
    let mut log = journal::Writer::open(journal)?;
    log.append(&Event::RollbackBegin {
        time: journal::now(),
    })?;
    let mut report = RollbackReport {
        restored: 0,
        already_original: 0,
        conflicts: Vec::new(),
    };
    for (rel, before, after) in intents.iter().rev() {
        let mut leave = |log: &mut journal::Writer, reason: String| -> Result<(), MigrateError> {
            log.append(&Event::RollbackConflict {
                path: rel.clone(),
                reason: reason.clone(),
            })?;
            report.conflicts.push(Conflict {
                path: rel.clone(),
                reason,
            });
            Ok(())
        };
        if !safe_rel(rel) {
            leave(&mut log, "malformed journal entry".to_owned())?;
            continue;
        }
        let path = repo_root.join(rel);
        let Ok(bytes) = std::fs::read(&path) else {
            leave(&mut log, "unreadable or missing".to_owned())?;
            continue;
        };
        let now = sha256_hex(&bytes);
        if now == *before {
            report.already_original += 1;
            continue;
        }
        if now != *after {
            leave(
                &mut log,
                "edited since the migration; not restored".to_owned(),
            )?;
            continue;
        }
        let orig_path = journal.join("originals").join(before);
        let original = std::fs::read(&orig_path).map_err(|e| MigrateError::io(&orig_path, e))?;
        if sha256_hex(&original) != *before {
            leave(
                &mut log,
                "saved original is corrupt; not restored".to_owned(),
            )?;
            continue;
        }
        journal::write_atomic(&path, &original)?;
        let check = std::fs::read(&path).map_err(|e| MigrateError::io(&path, e))?;
        if sha256_hex(&check) != *before {
            leave(&mut log, "restored file failed verification".to_owned())?;
            continue;
        }
        log.append(&Event::Restored {
            path: rel.clone(),
            before: before.clone(),
        })?;
        report.restored += 1;
    }
    log.append(&Event::RollbackComplete {
        restored: report.restored,
        conflicts: report.conflicts.len(),
    })?;
    Ok(report)
}

/// Where one journal stands.
#[derive(Debug, Clone, Serialize)]
pub struct JournalStatus {
    /// The migration id.
    pub migration: String,
    /// The plan's input digest.
    pub input_digest: String,
    /// The journal directory.
    pub dir: PathBuf,
    /// Pages with a saved original (an apply reached them).
    pub intents: usize,
    /// Pages confirmed written.
    pub applied: usize,
    /// Pages left alone by apply.
    pub conflicts: usize,
    /// Pages restored by a rollback.
    pub restored: usize,
    /// `complete`, `complete-with-conflicts`, `interrupted`, `rolled-back` or
    /// `rollback-partial`.
    pub state: String,
}

/// List every journal under `<repo_root>/.ontomark/migrations`.
///
/// # Errors
/// An unreadable journal directory.
pub fn status(repo_root: &Path) -> Result<Vec<JournalStatus>, MigrateError> {
    let base = repo_root.join(".ontomark").join("migrations");
    let mut out = Vec::new();
    let Ok(ids) = std::fs::read_dir(&base) else {
        return Ok(out);
    };
    let mut id_dirs: Vec<PathBuf> = ids.filter_map(Result::ok).map(|e| e.path()).collect();
    id_dirs.sort();
    for id_dir in id_dirs {
        let Ok(digests) = std::fs::read_dir(&id_dir) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = digests.filter_map(Result::ok).map(|e| e.path()).collect();
        dirs.sort();
        for dir in dirs {
            if !dir.join("journal.jsonl").is_file() {
                continue;
            }
            let events = journal::read(&dir)?;
            // Distinct pages, so re-runs of one plan do not inflate the counts.
            let paths = |f: fn(&Event) -> Option<&str>| {
                events
                    .iter()
                    .filter_map(f)
                    .collect::<BTreeSet<&str>>()
                    .len()
            };
            let intents = paths(|e| match e {
                Event::Intent { path, .. } => Some(path),
                _ => None,
            });
            let applied = paths(|e| match e {
                Event::Applied { path, .. } | Event::Recovered { path, .. } => Some(path),
                _ => None,
            });
            let conflicts = paths(|e| match e {
                Event::Conflict { path, .. } => Some(path),
                _ => None,
            });
            let restored = paths(|e| match e {
                Event::Restored { path, .. } => Some(path),
                _ => None,
            });
            let last_rollback = events
                .iter()
                .rposition(|e| matches!(e, Event::RollbackBegin { .. }));
            let state = match last_rollback {
                Some(i) => match events[i..].iter().find_map(|e| match e {
                    Event::RollbackComplete { conflicts, .. } => Some(*conflicts),
                    _ => None,
                }) {
                    Some(0) => "rolled-back",
                    _ => "rollback-partial",
                },
                None => match events.last() {
                    Some(Event::Complete { conflicts: 0, .. }) => "complete",
                    Some(Event::Complete { .. }) => "complete-with-conflicts",
                    _ => "interrupted",
                },
            };
            out.push(JournalStatus {
                migration: id_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                input_digest: dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                dir,
                intents,
                applied,
                conflicts,
                restored,
                state: state.to_owned(),
            });
        }
    }
    Ok(out)
}
