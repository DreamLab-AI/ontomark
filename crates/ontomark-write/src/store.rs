//! Where a [`Plan`](crate::Plan) lands, and the ordered commit across files
//! (each file atomic; the set is not crash-atomic).
//!
//! The engine never touches a disk itself: it hands a [`Store`] the bytes.
//! [`MemStore`] is the in-memory store hosts and tests use;
//! [`FsStore`](crate::FsStore) is the root-confined file store.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use ontomark_compile::input::page::VaultKind;
use serde::Serialize;

use crate::plan::{Plan, PlannedWrite};

/// How a write treats an existing file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Replace an existing file atomically.
    Replace,
    /// Create the file; fail if it exists.
    CreateNew,
}

/// A place files live, addressed by vault role and vault-relative path.
pub trait Store {
    /// The current bytes, or `None` when the file does not exist.
    ///
    /// # Errors
    /// Any I/O failure other than absence.
    fn read(&self, vault: VaultKind, rel: &Path) -> io::Result<Option<Vec<u8>>>;

    /// Write `bytes` atomically: a reader sees the old file or the new one,
    /// never a mixture.
    ///
    /// # Errors
    /// Any I/O failure, or an existing file under [`WriteMode::CreateNew`].
    fn write(
        &mut self,
        vault: VaultKind,
        rel: &Path,
        bytes: &[u8],
        mode: WriteMode,
    ) -> io::Result<()>;

    /// Remove a file the commit itself created.
    ///
    /// # Errors
    /// Any I/O failure.
    fn remove(&mut self, vault: VaultKind, rel: &Path) -> io::Result<()>;
}

/// An in-memory [`Store`], with an optional injected failure for tests.
///
/// ```
/// use ontomark_write::{MemStore, Store, WriteMode};
/// use ontomark_compile::input::page::VaultKind;
/// use std::path::Path;
/// let mut s = MemStore::default();
/// s.write(VaultKind::Working, Path::new("a.md"), b"x", WriteMode::CreateNew).unwrap();
/// assert!(s.write(VaultKind::Working, Path::new("a.md"), b"y", WriteMode::CreateNew).is_err());
/// ```
#[derive(Debug, Default, Clone)]
pub struct MemStore {
    files: BTreeMap<(&'static str, PathBuf), Vec<u8>>,
    /// Fail the n-th successful-looking write (0-based), once.
    pub fail_write_number: Option<usize>,
    /// Fail every restoring write (to model a failed rollback).
    pub fail_restores: bool,
    writes_seen: usize,
}

impl MemStore {
    /// Put a file in without going through a write.
    pub fn insert(&mut self, vault: VaultKind, rel: impl Into<PathBuf>, text: &str) {
        self.files
            .insert((vault.role_name(), rel.into()), text.as_bytes().to_vec());
    }

    /// The text of a file.
    #[must_use]
    pub fn text(&self, vault: VaultKind, rel: impl AsRef<Path>) -> Option<String> {
        self.files
            .get(&(vault.role_name(), rel.as_ref().to_path_buf()))
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }

    /// How many files exist.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// `true` when no file exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl Store for MemStore {
    fn read(&self, vault: VaultKind, rel: &Path) -> io::Result<Option<Vec<u8>>> {
        Ok(self
            .files
            .get(&(vault.role_name(), rel.to_path_buf()))
            .cloned())
    }

    fn write(
        &mut self,
        vault: VaultKind,
        rel: &Path,
        bytes: &[u8],
        mode: WriteMode,
    ) -> io::Result<()> {
        let key = (vault.role_name(), rel.to_path_buf());
        if mode == WriteMode::CreateNew && self.files.contains_key(&key) {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        if self.fail_restores && mode == WriteMode::Replace && self.fail_write_number.is_none() {
            return Err(io::Error::other("injected restore failure"));
        }
        let n = self.writes_seen;
        self.writes_seen += 1;
        if self.fail_write_number == Some(n) {
            self.fail_write_number = None;
            return Err(io::Error::other("injected write failure"));
        }
        self.files.insert(key, bytes.to_vec());
        Ok(())
    }

    fn remove(&mut self, vault: VaultKind, rel: &Path) -> io::Result<()> {
        self.files.remove(&(vault.role_name(), rel.to_path_buf()));
        Ok(())
    }
}

/// What a successful commit did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitReport {
    /// Files written, in order.
    pub written: Vec<PathBuf>,
}

/// Why a commit failed. In every case the store holds the files it held before
/// the commit unless [`CommitError::Write`] says a restore also failed.
#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    /// A target no longer holds the text the plan was made against; nothing
    /// was written.
    #[error("{0} changed since the plan was made; nothing was written")]
    Stale(PathBuf),
    /// A target the plan creates already exists; nothing was written.
    #[error("{0} already exists; nothing was written")]
    Exists(PathBuf),
    /// A write failed. `restored` lists the files put back; `unrestored` lists
    /// files that could not be put back (empty on a clean rollback).
    #[error("writing {failed} failed ({source}); restored {restored:?}, could not restore {unrestored:?}")]
    Write {
        /// The file whose write failed.
        failed: PathBuf,
        /// The underlying error.
        source: io::Error,
        /// Earlier files put back to their previous text (or removed, when
        /// the commit had created them).
        restored: Vec<PathBuf>,
        /// Earlier files the rollback could not restore.
        unrestored: Vec<PathBuf>,
    },
    /// Reading a target failed.
    #[error("reading {path}: {source}")]
    Read {
        /// The file.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
}

pub(crate) fn commit(plan: &Plan, store: &mut dyn Store) -> Result<CommitReport, CommitError> {
    let live: Vec<&PlannedWrite> = plan.writes.iter().filter(|w| !w.is_noop()).collect();
    // Preflight: every target still holds what the plan saw.
    for w in &live {
        let now = store
            .read(w.vault, &w.rel_path)
            .map_err(|source| CommitError::Read {
                path: w.rel_path.clone(),
                source,
            })?;
        match (&w.before_text, now) {
            (Some(before), Some(bytes)) if bytes == before.as_bytes() => {}
            (Some(_), _) => return Err(CommitError::Stale(w.rel_path.clone())),
            (None, Some(_)) => return Err(CommitError::Exists(w.rel_path.clone())),
            (None, None) => {}
        }
    }
    let mut done: Vec<&PlannedWrite> = Vec::new();
    for w in &live {
        let mode = if w.before_text.is_some() {
            WriteMode::Replace
        } else {
            WriteMode::CreateNew
        };
        if let Err(source) = store.write(w.vault, &w.rel_path, w.text.as_bytes(), mode) {
            let mut restored = Vec::new();
            let mut unrestored = Vec::new();
            for earlier in done.iter().rev() {
                let ok = match &earlier.before_text {
                    Some(before) => store
                        .write(
                            earlier.vault,
                            &earlier.rel_path,
                            before.as_bytes(),
                            WriteMode::Replace,
                        )
                        .is_ok(),
                    None => store.remove(earlier.vault, &earlier.rel_path).is_ok(),
                };
                if ok {
                    restored.push(earlier.rel_path.clone());
                } else {
                    unrestored.push(earlier.rel_path.clone());
                }
            }
            return Err(CommitError::Write {
                failed: w.rel_path.clone(),
                source,
                restored,
                unrestored,
            });
        }
        done.push(w);
    }
    Ok(CommitReport {
        written: done.iter().map(|w| w.rel_path.clone()).collect(),
    })
}
