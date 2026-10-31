//! The error type shared by every source.

/// Result alias for source operations.
pub type Result<T, E = SourceError> = std::result::Result<T, E>;

/// Why a source could not list, fetch or record an item.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// The item is not Markdown, plain text or HTML. Never guessed.
    #[error("unsupported media type {media_type:?} for {locator}")]
    UnsupportedMediaType {
        /// Path or URL of the refused item.
        locator: String,
        /// The media type found (from the extension or `Content-Type`); empty if none.
        media_type: String,
    },
    /// A filesystem operation failed.
    #[error("I/O error on {locator}: {source}")]
    Io {
        /// Path involved.
        locator: String,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// The item is not valid UTF-8.
    #[error("{locator} is not valid UTF-8")]
    NotUtf8 {
        /// Path or URL of the item.
        locator: String,
    },
    /// The item is larger than the configured limit.
    #[error("{locator} exceeds the {limit}-byte limit")]
    TooLarge {
        /// Path or URL of the item.
        locator: String,
        /// The limit in bytes.
        limit: u64,
    },
    /// The item's locator is not inside the source's root.
    #[error("{locator} is outside the source root")]
    OutsideRoot {
        /// The offending locator.
        locator: String,
    },
    /// The root does not exist or is neither a file nor a directory.
    #[error("source root {locator} is not a readable file or directory")]
    BadRoot {
        /// The root path.
        locator: String,
    },
    /// A URL was refused before any request: not http(s), or carries credentials.
    #[error("refused URL {locator}: {reason}")]
    BadUrl {
        /// The URL as given.
        locator: String,
        /// Why it was refused.
        reason: String,
    },
    /// An HTTP request failed.
    #[error("HTTP error on {locator}: {message}")]
    Http {
        /// The URL requested.
        locator: String,
        /// Status or transport failure description.
        message: String,
    },
    /// HTML could not be rendered.
    #[error("could not render HTML from {locator}: {message}")]
    Render {
        /// Path or URL of the item.
        locator: String,
        /// Renderer failure description.
        message: String,
    },
    /// A ledger file could not be read, parsed or written.
    #[error("ledger {locator}: {message}")]
    Ledger {
        /// Ledger path.
        locator: String,
        /// What went wrong.
        message: String,
    },
}
