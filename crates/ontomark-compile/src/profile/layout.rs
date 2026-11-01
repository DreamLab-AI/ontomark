//! Filesystem checks shared by compiler hosts that place vault roles on disk.

use std::path::{Component, Path, PathBuf};

use super::Layout;
use crate::input::page::VaultKind;

/// A layout whose role roots cannot be used consistently within one repository.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LayoutError {
    /// A filesystem operation failed while resolving a root.
    #[error("cannot resolve vault path {path}: {source}")]
    Io {
        /// The path being resolved.
        path: PathBuf,
        /// The underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// A declared folder is not a permitted relative path.
    #[error("invalid vault folder {path}: {reason}")]
    Invalid {
        /// The authored folder value.
        path: PathBuf,
        /// The violated path rule.
        reason: &'static str,
    },
    /// A symlink places a declared role outside its repository.
    #[error("vault {path} is outside repository {repository}")]
    Outside {
        /// The resolved role path.
        path: PathBuf,
        /// The canonical repository root.
        repository: PathBuf,
    },
    /// Two role roots are equal or one contains the other.
    #[error("vault roots overlap: {first} and {second}")]
    Overlap {
        /// The first canonical root.
        first: PathBuf,
        /// The second canonical root.
        second: PathBuf,
    },
    /// A caller supplied a directory that the layout does not assign a role.
    #[error("vault root {path} is not a root declared by this layout")]
    UnknownRoot {
        /// The unassigned root.
        path: PathBuf,
    },
}

/// Resolve existing ancestors without requiring an optional vault to exist yet.
fn resolve_path(path: &Path) -> Result<PathBuf, LayoutError> {
    let mut ancestor = path;
    let mut missing = Vec::new();
    loop {
        match std::fs::canonicalize(ancestor) {
            Ok(mut resolved) => {
                for part in missing.into_iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = ancestor.file_name() else {
                    return Err(LayoutError::Io {
                        path: path.to_path_buf(),
                        source,
                    });
                };
                missing.push(name.to_os_string());
                let Some(parent) = ancestor.parent() else {
                    return Err(LayoutError::Io {
                        path: path.to_path_buf(),
                        source,
                    });
                };
                ancestor = parent;
            }
            Err(source) => {
                return Err(LayoutError::Io {
                    path: path.to_path_buf(),
                    source,
                })
            }
        }
    }
}

fn role_root(repository: &Path, path: &Path) -> Result<PathBuf, LayoutError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(LayoutError::Invalid {
            path: path.to_path_buf(),
            reason: "use a non-empty repository-relative path without parent components",
        });
    }
    let mut joined = repository.to_path_buf();
    for part in path.components() {
        if let Component::Normal(name) = part {
            joined.push(name);
        }
    }
    let resolved = resolve_path(&joined)?;
    if !resolved.starts_with(repository) {
        return Err(LayoutError::Outside {
            path: resolved,
            repository: repository.to_path_buf(),
        });
    }
    Ok(resolved)
}

/// Resolve repository-relative corpus roots and reject aliases, overlap and escapes.
///
/// Results preserve the input order. Existing symlinks and ancestors are
/// resolved; a missing leaf is permitted so hosts can decide whether an
/// optional source is absent. Parent components and absolute paths are refused.
/// A single `.` root is valid. These checks do not prevent concurrent filesystem
/// replacement and do not grant access to any files.
///
/// # Errors
/// Returns [`LayoutError`] for invalid or overlapping paths, escaping symlinks
/// or filesystem failures while resolving an existing ancestor.
///
/// # Examples
/// ```no_run
/// use std::path::{Path, PathBuf};
/// use ontomark_compile::profile::resolve_roots;
/// let roots = resolve_roots(Path::new("/srv/corpus"), &[
///     PathBuf::from("knowledge"), PathBuf::from("working"),
/// ])?;
/// assert_eq!(roots.len(), 2);
/// # Ok::<(), ontomark_compile::profile::LayoutError>(())
/// ```
pub fn resolve_roots(repo_root: &Path, roots: &[PathBuf]) -> Result<Vec<PathBuf>, LayoutError> {
    let repository = std::fs::canonicalize(repo_root).map_err(|source| LayoutError::Io {
        path: repo_root.to_path_buf(),
        source,
    })?;
    if !repository.is_dir() {
        return Err(LayoutError::Invalid {
            path: repository,
            reason: "the repository root must be a directory",
        });
    }
    let mut resolved: Vec<PathBuf> = Vec::with_capacity(roots.len());
    for root in roots {
        let path = role_root(&repository, root)?;
        for previous in &resolved {
            if path.starts_with(previous) || previous.starts_with(&path) {
                return Err(LayoutError::Overlap {
                    first: previous.clone(),
                    second: path,
                });
            }
        }
        resolved.push(path);
    }
    Ok(resolved)
}

impl Layout {
    fn resolved_roots(&self, repo_root: &Path) -> Result<(PathBuf, Option<PathBuf>), LayoutError> {
        let mut roots = vec![PathBuf::from(&self.knowledge)];
        if let Some(working) = &self.working {
            roots.push(PathBuf::from(working));
        }
        let mut resolved = resolve_roots(repo_root, &roots)?.into_iter();
        let knowledge = resolved
            .next()
            .expect("one declared knowledge root produces one result");
        Ok((knowledge, resolved.next()))
    }

    /// Check that declared roles occupy distinct, non-overlapping paths inside the repository.
    ///
    /// Existing symlinks are resolved before comparing paths. Missing optional
    /// folders are checked through their nearest existing ancestor; selecting a
    /// missing role still requires [`Self::root_of`]. This is configuration
    /// validation, not a sandbox against concurrent filesystem replacement.
    ///
    /// # Errors
    /// Returns [`LayoutError`] for invalid paths, escaping symlinks, overlapping
    /// roles or an unreadable repository root.
    pub fn validate(&self, repo_root: &Path) -> Result<(), LayoutError> {
        self.resolved_roots(repo_root).map(|_| ())
    }

    /// Identify a vault by its complete resolved root, rather than its directory suffix.
    ///
    /// # Errors
    /// Returns [`LayoutError`] if the layout is invalid, the path cannot be
    /// resolved or neither declared root matches it.
    pub fn kind_of_at(&self, repo_root: &Path, root: &Path) -> Result<VaultKind, LayoutError> {
        let (knowledge, working) = self.resolved_roots(repo_root)?;
        let actual = std::fs::canonicalize(root).map_err(|source| LayoutError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        if actual == knowledge {
            Ok(VaultKind::Knowledge)
        } else if working.as_ref() == Some(&actual) {
            Ok(VaultKind::Working)
        } else {
            Err(LayoutError::UnknownRoot { path: actual })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_roots_keep_their_order_and_flat_roots_are_valid() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path().canonicalize().unwrap();
        let paths = resolve_roots(repo.path(), &["notes".into(), "drafts/new".into()]).unwrap();
        assert_eq!(paths, [root.join("notes"), root.join("drafts/new")]);
        assert_eq!(resolve_roots(repo.path(), &[".".into()]).unwrap(), [root]);
    }

    #[test]
    fn rejects_invalid_components_and_overlapping_missing_roots() {
        let repo = tempfile::tempdir().unwrap();
        for path in ["", "../outside", "notes/../drafts", "/absolute"] {
            assert!(matches!(
                resolve_roots(repo.path(), &[path.into()]),
                Err(LayoutError::Invalid { .. })
            ));
        }
        for paths in [
            ["notes", "./notes"],
            ["notes", "notes/nested"],
            [".", "drafts"],
        ] {
            assert!(matches!(
                resolve_roots(repo.path(), &paths.map(PathBuf::from)),
                Err(LayoutError::Overlap { .. })
            ));
        }
    }

    #[test]
    fn a_file_cannot_be_the_repository() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(matches!(
            resolve_roots(file.path(), &[".".into()]),
            Err(LayoutError::Invalid { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn resolves_existing_ancestors_before_accepting_optional_roots() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), repo.path().join("linked")).unwrap();
        assert!(matches!(
            resolve_roots(repo.path(), &["linked/missing".into()]),
            Err(LayoutError::Outside { .. })
        ));
        std::fs::create_dir(repo.path().join("notes")).unwrap();
        std::os::unix::fs::symlink("notes", repo.path().join("alias")).unwrap();
        assert!(matches!(
            resolve_roots(repo.path(), &["notes".into(), "alias".into()]),
            Err(LayoutError::Overlap { .. })
        ));
    }
}
