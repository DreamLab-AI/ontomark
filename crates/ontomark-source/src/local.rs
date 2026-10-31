//! [`LocalFiles`]: a directory or a single file on disk.

use crate::error::{Result, SourceError};
use crate::render::{render, MediaKind};
use crate::{Fetched, Source, SourceItem};
use std::path::{Path, PathBuf};

/// Extensions listed when [`LocalFiles::include`] is empty.
const DEFAULT_INCLUDE: &[&str] = &["md", "markdown", "txt", "html", "htm"];

/// Default per-file size limit: 16 MiB.
const DEFAULT_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// Walks a directory (or reads one file) and renders what it finds.
///
/// Listing is deterministic (sorted by item id) and skips every dot-file and
/// dot-directory (which covers `.obsidian/`, `.trash/` and `.git/`) below the
/// root, and symbolic links, so a walk cannot leave the root. Directory walks
/// list only files whose extension is in [`LocalFiles::include`]; a single
/// file given as the root is always listed, so a PDF named explicitly is
/// refused by [`Source::fetch`] with
/// [`SourceError::UnsupportedMediaType`] rather than silently ignored.
///
/// ```
/// use ontomark_source::LocalFiles;
/// let source = LocalFiles::new("/vault").with_include(["md", "html"]);
/// assert_eq!(source.include, ["md", "html"]);
/// ```
#[derive(Debug, Clone)]
pub struct LocalFiles {
    /// A directory to walk or a single file.
    pub root: PathBuf,
    /// Extensions to list in a directory walk, without dot or `*.` and in any
    /// case, e.g. `["md", "html"]`. Empty means `md`, `markdown`, `txt`, `html`, `htm`.
    pub include: Vec<String>,
    /// Files larger than this are refused with [`SourceError::TooLarge`].
    pub max_bytes: u64,
}

impl LocalFiles {
    /// A source over `root` with the default extensions and size limit.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            include: Vec::new(),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    /// Replace the extension list (see [`LocalFiles::include`]).
    pub fn with_include<I, S>(mut self, include: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.include = include.into_iter().map(Into::into).collect();
        self
    }

    fn wants(&self, name: &str) -> bool {
        let Some(ext) = Path::new(name).extension().and_then(|e| e.to_str()) else {
            return false;
        };
        if self.include.is_empty() {
            return DEFAULT_INCLUDE.iter().any(|d| d.eq_ignore_ascii_case(ext));
        }
        self.include.iter().any(|p| {
            p.trim_start_matches('*')
                .trim_start_matches('.')
                .eq_ignore_ascii_case(ext)
        })
    }

    fn walk(&self, dir: &Path, rel: &str, out: &mut Vec<SourceItem>) -> Result<()> {
        let io = |e| io_err(dir, e);
        for entry in std::fs::read_dir(dir).map_err(io)? {
            let entry = entry.map_err(io)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).map_err(|e| io_err(&path, e))?;
            let id = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if meta.is_dir() {
                self.walk(&path, &id, out)?;
            } else if meta.is_file() && self.wants(&name) {
                out.push(item(id, &path, &meta));
            }
        }
        Ok(())
    }
}

fn io_err(path: &Path, source: std::io::Error) -> SourceError {
    SourceError::Io {
        locator: path.display().to_string(),
        source,
    }
}

fn item(id: String, path: &Path, meta: &std::fs::Metadata) -> SourceItem {
    SourceItem {
        id,
        locator: path.display().to_string(),
        change_marker: marker(meta),
        size: Some(meta.len()),
    }
}

/// `mtime:size`, stable for an untouched file.
fn marker(meta: &std::fs::Metadata) -> String {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| format!("{}.{:09}", d.as_secs(), d.subsec_nanos()))
        .unwrap_or_else(|| "0".to_string());
    format!("{mtime}:{}", meta.len())
}

impl Source for LocalFiles {
    fn describe(&self) -> String {
        format!("local files at {}", self.root.display())
    }

    fn list(&self) -> Result<Vec<SourceItem>> {
        let meta = std::fs::metadata(&self.root).map_err(|e| io_err(&self.root, e))?;
        let mut out = Vec::new();
        if meta.is_file() {
            let id = self
                .root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.root.display().to_string());
            out.push(item(id, &self.root, &meta));
        } else if meta.is_dir() {
            self.walk(&self.root, "", &mut out)?;
        } else {
            return Err(SourceError::BadRoot {
                locator: self.root.display().to_string(),
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn fetch(&self, item: &SourceItem) -> Result<Fetched> {
        let path = Path::new(&item.locator);
        let kind = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(MediaKind::from_extension)
            .ok_or_else(|| SourceError::UnsupportedMediaType {
                locator: item.locator.clone(),
                media_type: path
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default(),
            })?;

        // The locator must resolve inside the root (or be the root file).
        let real = path.canonicalize().map_err(|e| io_err(path, e))?;
        let root = self
            .root
            .canonicalize()
            .map_err(|e| io_err(&self.root, e))?;
        if real != root && !real.starts_with(&root) {
            return Err(SourceError::OutsideRoot {
                locator: item.locator.clone(),
            });
        }

        let len = std::fs::metadata(&real).map_err(|e| io_err(path, e))?.len();
        if len > self.max_bytes {
            return Err(SourceError::TooLarge {
                locator: item.locator.clone(),
                limit: self.max_bytes,
            });
        }
        let bytes = std::fs::read(&real).map_err(|e| io_err(path, e))?;
        let text = render(kind, &bytes, &item.locator)?;
        Ok(Fetched::new(&item.id, kind.media_type(), text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir_all(p.join("sub")).unwrap();
        fs::create_dir_all(p.join(".obsidian")).unwrap();
        fs::create_dir_all(p.join(".trash")).unwrap();
        fs::write(p.join("b.md"), "# B\r\n").unwrap();
        fs::write(p.join("a.txt"), "\u{feff}plain\r\n").unwrap();
        fs::write(p.join("sub/c.HTML"), "<h2>C</h2>").unwrap();
        fs::write(p.join(".hidden.md"), "x").unwrap();
        fs::write(p.join(".obsidian/app.md"), "x").unwrap();
        fs::write(p.join(".trash/old.md"), "x").unwrap();
        fs::write(p.join("doc.pdf"), b"%PDF-1.7").unwrap();
        d
    }

    #[test]
    fn lists_sorted_and_skips_hidden_and_other_types() {
        let d = tree();
        let ids: Vec<_> = LocalFiles::new(d.path())
            .list()
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(ids, ["a.txt", "b.md", "sub/c.HTML"]);
    }

    #[test]
    fn include_filters_by_extension() {
        let d = tree();
        let src = LocalFiles::new(d.path()).with_include(["*.md", ".PDF"]);
        let ids: Vec<_> = src.list().unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["b.md", "doc.pdf"]);
    }

    #[test]
    fn fetch_renders_and_versions() {
        let d = tree();
        let src = LocalFiles::new(d.path());
        let items = src.list().unwrap();
        let a = src.fetch(&items[0]).unwrap();
        assert_eq!(
            (a.text.as_str(), a.media_type.as_str()),
            ("plain\n", "text/plain")
        );
        assert_eq!(a.version, crate::version_of("plain\n"));
        let c = src.fetch(&items[2]).unwrap();
        assert_eq!(c.text, "## C\n");
        assert_eq!(c.media_type, "text/html");
    }

    #[test]
    fn pdf_is_refused_by_name() {
        let d = tree();
        let src = LocalFiles::new(d.path()).with_include(["pdf"]);
        let items = src.list().unwrap();
        match src.fetch(&items[0]) {
            Err(SourceError::UnsupportedMediaType { media_type, .. }) => {
                assert_eq!(media_type, "pdf")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn single_file_root_is_listed_even_if_unsupported() {
        let d = tree();
        let src = LocalFiles::new(d.path().join("doc.pdf"));
        let items = src.list().unwrap();
        assert_eq!(items.len(), 1);
        assert!(matches!(
            src.fetch(&items[0]),
            Err(SourceError::UnsupportedMediaType { .. })
        ));
        let md = LocalFiles::new(d.path().join("b.md"));
        let items = md.list().unwrap();
        assert_eq!(items[0].id, "b.md");
        assert_eq!(md.fetch(&items[0]).unwrap().text, "# B\n");
    }

    #[test]
    fn marker_changes_when_file_is_rewritten() {
        let d = tree();
        let src = LocalFiles::new(d.path());
        let before = src.list().unwrap();
        fs::write(d.path().join("b.md"), "# B changed").unwrap();
        let after = src.list().unwrap();
        assert_eq!(before[0].change_marker, after[0].change_marker);
        assert_ne!(before[1].change_marker, after[1].change_marker);
    }

    #[test]
    fn locator_outside_root_is_refused() {
        let d = tree();
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("x.md"), "secret").unwrap();
        let src = LocalFiles::new(d.path());
        let rogue = SourceItem {
            id: "x.md".into(),
            locator: other.path().join("x.md").display().to_string(),
            change_marker: String::new(),
            size: None,
        };
        assert!(matches!(
            src.fetch(&rogue),
            Err(SourceError::OutsideRoot { .. })
        ));
    }

    #[test]
    fn size_limit_applies() {
        let d = tree();
        let mut src = LocalFiles::new(d.path());
        src.max_bytes = 3;
        let items = src.list().unwrap();
        assert!(matches!(
            src.fetch(&items[1]),
            Err(SourceError::TooLarge { .. })
        ));
    }

    #[test]
    fn missing_root_is_an_error() {
        assert!(matches!(
            LocalFiles::new("/definitely/not/here").list(),
            Err(SourceError::Io { .. })
        ));
    }
}
