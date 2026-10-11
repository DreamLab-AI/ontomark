//! The migration journal: originals plus an fsynced, append-only event log.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::MigrateError;

/// One journal line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum Event {
    /// An apply run started (or restarted).
    Begin {
        migration: String,
        version: u32,
        input_digest: String,
        time: String,
    },
    /// About to replace `path`: its original is saved under `originals/`.
    Intent {
        path: String,
        before: String,
        after: String,
    },
    /// `path` now has the planned bytes.
    Applied { path: String, after: String },
    /// `path` was found already at the planned bytes (interrupted earlier run).
    Recovered { path: String, after: String },
    /// `path` was left alone.
    Conflict { path: String, reason: String },
    /// An apply run finished.
    Complete { applied: usize, conflicts: usize },
    /// A rollback started.
    RollbackBegin { time: String },
    /// `path` was restored and verified.
    Restored { path: String, before: String },
    /// `path` was left alone by the rollback.
    RollbackConflict { path: String, reason: String },
    /// A rollback finished.
    RollbackComplete { restored: usize, conflicts: usize },
}

/// The journal directory of a plan.
pub(crate) fn journal_dir(repo_root: &Path, migration_id: &str, input_digest: &str) -> PathBuf {
    repo_root
        .join(".ontomark")
        .join("migrations")
        .join(migration_id)
        .join(input_digest)
}

pub(crate) fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// Make the directory entry durable (best effort: some filesystems refuse).
pub(crate) fn sync_dir(dir: &Path) {
    if let Ok(f) = File::open(dir) {
        let _ = f.sync_all();
    }
}

/// Write `bytes` to `target` through a dot-prefixed temp file in the same
/// directory, fsync it, rename it over `target` and fsync the directory.
/// Permissions are copied from `target` when it exists.
pub(crate) fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), MigrateError> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.ontomark-tmp"));
    let result = (|| -> std::io::Result<()> {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        if let Ok(meta) = std::fs::metadata(target) {
            let _ = f.set_permissions(meta.permissions());
        }
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, target)?;
        sync_dir(dir);
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| MigrateError::io(target, e))
}

/// Append-only journal writer; every record is fsynced before `append` returns.
pub(crate) struct Writer {
    file: File,
    path: PathBuf,
}

impl Writer {
    pub(crate) fn open(dir: &Path) -> Result<Self, MigrateError> {
        std::fs::create_dir_all(dir.join("originals")).map_err(|e| MigrateError::io(dir, e))?;
        let path = dir.join("journal.jsonl");
        let created = !path.exists();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| MigrateError::io(&path, e))?;
        let mut w = Self { file, path };
        // A crash can leave a truncated final line; terminate it so the next
        // record starts on its own line (the reader skips the damaged one).
        if !created
            && std::fs::read(&w.path).is_ok_and(|b| b.last().is_some_and(|c| *c != b'\n'))
        {
            w.file
                .write_all(b"\n")
                .map_err(|e| MigrateError::io(&w.path, e))?;
        }
        sync_dir(dir);
        Ok(w)
    }

    pub(crate) fn append(&mut self, event: &Event) -> Result<(), MigrateError> {
        let mut line =
            serde_json::to_string(event).map_err(|e| MigrateError::json("journal event", e))?;
        line.push('\n');
        self.file
            .write_all(line.as_bytes())
            .and_then(|()| self.file.sync_data())
            .map_err(|e| MigrateError::io(&self.path, e))
    }
}

/// Read a journal. A line that does not parse is skipped: records are fsynced
/// before the action they announce, so a torn record belongs to an action that
/// never happened.
pub(crate) fn read(dir: &Path) -> Result<Vec<Event>, MigrateError> {
    let path = dir.join("journal.jsonl");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| MigrateError::Journal(format!("{}: {e}", path.display())))?;
    Ok(text
        .lines()
        .filter_map(|l| serde_json::from_str::<Event>(l).ok())
        .collect())
}
