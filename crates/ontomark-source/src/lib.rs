//! The Source port for Ontomark ingest.
//!
//! A [`Source`] offers *items* (files, web pages), each with a cheap
//! change marker, and renders any item to text plus a version: the lowercase
//! hex SHA-256 of that text. The shape mirrors VisionClaw's `CorpusSource`
//! (`describe`, `list`, `fetch`) so the two ports can converge later without a
//! change of meaning. This crate reads files and, behind the `http` feature,
//! URLs. It calls no model and has no network code unless `http` is enabled.
//!
//! Two sources ship:
//!
//! - [`LocalFiles`]: a directory or a single file. Markdown and plain text are
//!   passed through verbatim (CRLF becomes LF, a UTF-8 byte-order mark is
//!   dropped). HTML is rendered to Markdown so that headings become `#`
//!   headings and heading-aware chunking works downstream.
//! - `Url` (feature `http`): blocking HTTP(S) GET. The response
//!   `Content-Type` chooses the renderer. No cookies, no credentials.
//!
//! Anything else (PDF, office documents, images) is refused with
//! [`SourceError::UnsupportedMediaType`]; a media type is never guessed.
//!
//! Re-reading is avoided with a [`Ledger`] of `(version, parser profile,
//! change marker)` per item. [`classify`] compares a fresh listing with the
//! ledger and reports each item as new, content changed, parser changed,
//! marker only, unchanged or removed. Because the version hashes the rendered
//! text, renaming or touching a file is not a re-read.
//!
//! # Example
//!
//! ```
//! use ontomark_source::{classify, Change, Ledger, LocalFiles, Source};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let dir = std::env::temp_dir().join(format!("ontomark-source-doc-{}", std::process::id()));
//! std::fs::create_dir_all(&dir)?;
//! std::fs::write(dir.join("page.html"), "<h1>Title</h1><p>Body &amp; more.</p>")?;
//!
//! let source = LocalFiles::new(&dir);
//! let mut observed = Vec::new();
//! for item in source.list()? {
//!     let fetched = source.fetch(&item)?;
//!     assert!(fetched.text.starts_with("# Title"));
//!     observed.push((item, Some(fetched)));
//! }
//!
//! let mut ledger = Ledger::new();
//! let changes = classify(&ledger, &observed, "profile-1");
//! assert!(matches!(changes[0], Change::New { .. }));
//!
//! for (item, fetched) in &observed {
//!     ledger.record(item, fetched.as_ref().unwrap(), "profile-1");
//! }
//! let changes = classify(&ledger, &observed, "profile-1");
//! assert!(matches!(changes[0], Change::Unchanged { .. }));
//! # std::fs::remove_dir_all(&dir)?;
//! # Ok(())
//! # }
//! ```

mod error;
mod ledger;
mod local;
mod render;
#[cfg(feature = "http")]
mod url;

pub use error::{Result, SourceError};
pub use ledger::{classify, Change, Ledger, LedgerEntry};
pub use local::LocalFiles;
pub use render::{render, version_of, MediaKind};
#[cfg(feature = "http")]
pub use url::Url;

/// One thing a [`Source`] offers: a file or a web page, not yet read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceItem {
    /// Stable identity within the source: a `/`-separated path relative to the
    /// root for files, the URL for web pages.
    pub id: String,
    /// Where to read it from: a filesystem path or a URL.
    pub locator: String,
    /// Cheap change marker: `mtime:size` for files, the `ETag` or
    /// `Last-Modified` header for URLs, or empty when none is available. Equal
    /// non-empty markers across two listings mean the item did not change.
    pub change_marker: String,
    /// Size in bytes when known, for telemetry only.
    pub size: Option<u64>,
}

/// An item read and rendered to text.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Fetched {
    /// The [`SourceItem::id`] this text came from.
    pub item_id: String,
    /// The media type that chose the renderer, e.g. `text/html`.
    pub media_type: String,
    /// The rendered text.
    pub text: String,
    /// Lowercase hex SHA-256 of [`Fetched::text`].
    pub version: String,
}

impl Fetched {
    /// Build a [`Fetched`], computing [`Fetched::version`] from `text`.
    pub fn new(
        item_id: impl Into<String>,
        media_type: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        let text = text.into();
        Self {
            item_id: item_id.into(),
            media_type: media_type.into(),
            version: version_of(&text),
            text,
        }
    }
}

/// Where evidence is read from.
pub trait Source {
    /// A one-line self-description for logs and ledgers, e.g. `local files at /vault`.
    fn describe(&self) -> String;

    /// Every item on offer, in a deterministic order, with its change marker.
    /// Nothing is read beyond file metadata (or response headers for URLs).
    fn list(&self) -> Result<Vec<SourceItem>>;

    /// Read one item and render it to text.
    ///
    /// # Errors
    ///
    /// [`SourceError::UnsupportedMediaType`] when the item is neither Markdown,
    /// plain text nor HTML; other variants for I/O and size failures.
    fn fetch(&self, item: &SourceItem) -> Result<Fetched>;
}
