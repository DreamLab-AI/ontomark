//! A file [`Store`] confined to two vault roots.
//!
//! Every write goes to a temporary file in the target's own directory and is
//! renamed into place, so a reader sees the old file or the new one. Nothing
//! follows a symlink: a target, or any directory on the way to it, that is a
//! symlink is refused, and the resolved parent must lie inside the vault root.
//! `working/assets` is such a symlink in the estate, which is why.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};

use ontomark_compile::input::page::VaultKind;

use crate::store::{Store, WriteMode};

/// A [`Store`] over real directories.
#[derive(Debug, Clone)]
pub struct FsStore {
    working: Option<PathBuf>,
    knowledge: Option<PathBuf>,
}

impl FsStore {
    /// A store over the two vault roots. Roots are resolved once, here; a root
    /// that is itself reached through a symlink is fine, a path *inside* it
    /// that is a symlink is not.
    ///
    /// # Errors
    /// When a given root does not exist.
    pub fn new(working: Option<&Path>, knowledge: Option<&Path>) -> io::Result<Self> {
        Ok(Self {
            working: working.map(fs::canonicalize).transpose()?,
            knowledge: knowledge.map(fs::canonicalize).transpose()?,
        })
    }

    fn root(&self, vault: VaultKind) -> io::Result<&Path> {
        match vault {
            VaultKind::Working => self.working.as_deref(),
            VaultKind::Knowledge => self.knowledge.as_deref(),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "that vault root was not given"))
    }

    /// The absolute path of `rel` under the vault root, after refusing any
    /// escape. With `create_dirs` the missing directories on the way are made.
    fn resolve(&self, vault: VaultKind, rel: &Path, create_dirs: bool) -> io::Result<PathBuf> {
        let root = self.root(vault)?;
        let denied = |what: &str| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{}: {what}", rel.display()),
            )
        };
        let mut path = root.to_path_buf();
        let mut parts = rel.components().peekable();
        while let Some(part) = parts.next() {
            let Component::Normal(name) = part else {
                return Err(denied("leaves the vault"));
            };
            path.push(name);
            let last = parts.peek().is_none();
            match fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(denied("is or passes through a symlink"));
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    if !last && create_dirs {
                        fs::create_dir(&path)?;
                    }
                }
                Err(e) => return Err(e),
            }
        }
        if let Some(parent) = path.parent() {
            if parent.exists() && !fs::canonicalize(parent)?.starts_with(root) {
                return Err(denied("resolves outside the vault"));
            }
        }
        Ok(path)
    }
}

impl Store for FsStore {
    fn read(&self, vault: VaultKind, rel: &Path) -> io::Result<Option<Vec<u8>>> {
        let path = self.resolve(vault, rel, false)?;
        match fs::read(path) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn write(
        &mut self,
        vault: VaultKind,
        rel: &Path,
        bytes: &[u8],
        mode: WriteMode,
    ) -> io::Result<()> {
        let path = self.resolve(vault, rel, mode == WriteMode::CreateNew)?;
        let dir = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no parent directory"))?;
        let mut tmp = tempfile::Builder::new()
            .prefix(".ontomark-write-")
            .suffix(".tmp")
            .tempfile_in(dir)?;
        tmp.write_all(bytes)?;
        tmp.as_file().sync_all()?;
        match mode {
            WriteMode::Replace => {
                tmp.persist(&path).map_err(|e| e.error)?;
            }
            WriteMode::CreateNew => {
                tmp.persist_noclobber(&path).map_err(|e| e.error)?;
            }
        }
        Ok(())
    }

    fn remove(&mut self, vault: VaultKind, rel: &Path) -> io::Result<()> {
        let path = self.resolve(vault, rel, false)?;
        match fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}
