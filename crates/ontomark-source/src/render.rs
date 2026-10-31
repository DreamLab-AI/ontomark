//! Rendering fetched bytes to text, and the version hash.

use crate::error::{Result, SourceError};
use sha2::{Digest, Sha256};

/// The renderer a media type selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// Markdown: passed through verbatim after newline and BOM normalisation.
    Markdown,
    /// Plain text: handled like Markdown.
    PlainText,
    /// HTML: rendered to Markdown with `#` headings.
    Html,
}

impl MediaKind {
    /// The canonical media type string for this kind.
    pub fn media_type(self) -> &'static str {
        match self {
            MediaKind::Markdown => "text/markdown",
            MediaKind::PlainText => "text/plain",
            MediaKind::Html => "text/html",
        }
    }

    /// Map a file extension (without the dot, any case) to a kind.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(MediaKind::Markdown),
            "txt" | "text" => Some(MediaKind::PlainText),
            "html" | "htm" | "xhtml" => Some(MediaKind::Html),
            _ => None,
        }
    }

    /// Map a `Content-Type` header value (parameters ignored) to a kind.
    pub fn from_content_type(value: &str) -> Option<Self> {
        let essence = value.split(';').next().unwrap_or("").trim();
        match essence.to_ascii_lowercase().as_str() {
            "text/markdown" | "text/x-markdown" => Some(MediaKind::Markdown),
            "text/plain" => Some(MediaKind::PlainText),
            "text/html" | "application/xhtml+xml" => Some(MediaKind::Html),
            _ => None,
        }
    }
}

/// Lowercase hex SHA-256 of `text`: the version of a rendered item.
///
/// ```
/// assert_eq!(
///     ontomark_source::version_of("abc"),
///     "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
/// );
/// ```
pub fn version_of(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut out = String::with_capacity(64);
    for b in digest.iter() {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Elements dropped with their content when rendering HTML.
const DROPPED: &[&str] = &[
    "script", "style", "noscript", "template", "nav", "head", "svg",
];

/// Render `bytes` of the given kind to text. `locator` is used in errors only.
///
/// Markdown and plain text: UTF-8 BOM stripped, CRLF and lone CR become LF,
/// otherwise verbatim. HTML: converted to Markdown (headings as `#`), with
/// script, style, nav and similar chrome dropped and entities decoded.
///
/// # Errors
///
/// [`SourceError::NotUtf8`] for invalid UTF-8; [`SourceError::Render`] if the
/// HTML converter fails.
pub fn render(kind: MediaKind, bytes: &[u8], locator: &str) -> Result<String> {
    let text = std::str::from_utf8(bytes).map_err(|_| SourceError::NotUtf8 {
        locator: locator.to_string(),
    })?;
    let text = normalise(text);
    match kind {
        MediaKind::Markdown | MediaKind::PlainText => Ok(text),
        MediaKind::Html => {
            let converter = htmd::HtmlToMarkdown::builder()
                .skip_tags(DROPPED.to_vec())
                .build();
            let md = converter.convert(&text).map_err(|e| SourceError::Render {
                locator: locator.to_string(),
                message: e.to_string(),
            })?;
            let md = normalise(&md);
            let md = md.trim();
            Ok(if md.is_empty() {
                String::new()
            } else {
                format!("{md}\n")
            })
        }
    }
}

/// Strip a leading BOM and convert CRLF / CR to LF.
fn normalise(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            version_of(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn markdown_normalises_bom_and_newlines_only() {
        let out = render(MediaKind::Markdown, b"\xef\xbb\xbf# T\r\nline\rx\r\n", "t").unwrap();
        assert_eq!(out, "# T\nline\nx\n");
    }

    #[test]
    fn html_headings_entities_and_chrome() {
        let html = "<html><head><title>x</title><style>p{}</style></head><body>\
            <nav><a href='/'>Home</a></nav><script>alert(1)</script>\
            <h1>Top</h1><p>A &amp; B &lt; C &eacute;</p><h2>Sub</h2><p>Text</p>\
            </body></html>";
        let out = render(MediaKind::Html, html.as_bytes(), "t").unwrap();
        assert!(out.starts_with("# Top\n"), "{out:?}");
        assert!(out.contains("A & B < C é"), "{out:?}");
        assert!(out.contains("## Sub"), "{out:?}");
        for gone in ["alert", "Home", "p{}", "x\n"] {
            assert!(!out.contains(gone), "{gone} survived: {out:?}");
        }
    }

    #[test]
    fn invalid_utf8_is_refused() {
        assert!(matches!(
            render(MediaKind::PlainText, &[0xff, 0xfe, 0x41], "t"),
            Err(SourceError::NotUtf8 { .. })
        ));
    }

    #[test]
    fn content_type_parameters_are_ignored() {
        assert_eq!(
            MediaKind::from_content_type("Text/HTML; charset=utf-8"),
            Some(MediaKind::Html)
        );
        assert_eq!(MediaKind::from_content_type("application/pdf"), None);
    }
}
