//! A page: its exact source, the frontmatter block and the body index.

use crate::body::BodyIndex;
use crate::frontmatter::{self, Frontmatter};
use crate::span::{SourceDigest, Span};
use crate::value::Value;

/// A Markdown page held losslessly.
///
/// The source bytes are the document. Everything else is an index into them:
/// `Document::to_string` (via `Display`) and [`Document::source`] return the
/// input unchanged, whatever it contained — a BOM, `\r\n` line endings, a
/// missing final newline, comments, odd quoting or YAML this crate cannot
/// patch.
///
/// ```
/// # use ontomark_parse::{Document, Value};
/// let src = "---\ntitle: Café\n# kept\naliases:\n- Cafe\n---\nSee [[Menu]].\n";
/// let doc = Document::parse(src);
/// assert_eq!(doc.source(), src);
/// assert_eq!(doc.value("title"), Some(&Value::string("Café")));
/// assert_eq!(doc.body(), "See [[Menu]].\n");
/// assert_eq!(doc.body_index().links[0].link.target, "Menu");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    source: String,
    bom: bool,
    frontmatter: Option<Frontmatter>,
    body: Span,
    body_line: usize,
    body_index: BodyIndex,
}

impl Document {
    /// Parse `source`. Never fails: problems in the frontmatter are reported
    /// through [`Frontmatter::state`] and the bytes are kept regardless.
    ///
    /// The frontmatter opening fence must be the very first line (after an
    /// optional BOM), exactly `---`; a `---` later in the page is a
    /// horizontal rule. A block with no closing fence is not frontmatter.
    #[must_use]
    pub fn parse(source: impl Into<String>) -> Self {
        let source = source.into();
        let bom = source.starts_with('\u{feff}');
        let offset = if bom { '\u{feff}'.len_utf8() } else { 0 };
        let (frontmatter, body_start) = match frontmatter::locate(&source, offset) {
            Some((open, content, close)) => {
                let fm = frontmatter::parse_block(&source, open, content, close);
                (Some(fm), close.end)
            }
            None => (None, offset),
        };
        let body = Span::new(body_start, source.len());
        let body_line = 1 + source[..body_start].matches('\n').count();
        let body_index = BodyIndex::scan(body.slice(&source), body_start);
        Self {
            source,
            bom,
            frontmatter,
            body,
            body_line,
            body_index,
        }
    }

    /// The exact source text.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Consume the document, returning its source text.
    #[must_use]
    pub fn into_source(self) -> String {
        self.source
    }

    /// SHA-256 of the source bytes; the guard a [`crate::Patch`] names.
    #[must_use]
    pub fn digest(&self) -> SourceDigest {
        SourceDigest::of(&self.source)
    }

    /// `true` when the source starts with a UTF-8 byte-order mark.
    #[must_use]
    pub fn has_bom(&self) -> bool {
        self.bom
    }

    /// The frontmatter block, when the page has one.
    #[must_use]
    pub fn frontmatter(&self) -> Option<&Frontmatter> {
        self.frontmatter.as_ref()
    }

    /// The value under a top-level frontmatter `key`.
    #[must_use]
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.frontmatter.as_ref()?.value(key)
    }

    /// The body text: everything after the closing fence (or after the BOM
    /// when there is no frontmatter).
    #[must_use]
    pub fn body(&self) -> &str {
        self.body.slice(&self.source)
    }

    /// The body's span in the source.
    #[must_use]
    pub fn body_span(&self) -> Span {
        self.body
    }

    /// The 1-based source line the body starts on.
    #[must_use]
    pub fn body_line(&self) -> usize {
        self.body_line
    }

    /// Wikilinks, footnotes and code regions in the body.
    #[must_use]
    pub fn body_index(&self) -> &BodyIndex {
        &self.body_index
    }

    /// The 1-based line and column (in characters) of byte `offset`.
    ///
    /// # Panics
    /// Panics when `offset` is beyond the source or not on a character
    /// boundary.
    #[must_use]
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let before = &self.source[..offset];
        let line = 1 + before.matches('\n').count();
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        (line, 1 + before[line_start..].chars().count())
    }
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::FrontmatterState;

    #[test]
    fn bom_crlf_and_no_final_newline_round_trip() {
        let src = "\u{feff}---\r\na: 1\r\n---\r\nbody";
        let d = Document::parse(src);
        assert_eq!(d.source(), src);
        assert!(d.has_bom());
        assert_eq!(d.value("a"), Some(&Value::Int(1)));
        assert_eq!(d.body(), "body");
        assert!(d.frontmatter().unwrap().style.crlf);
        assert_eq!(d.body_line(), 4);
    }

    #[test]
    fn closing_fence_at_eof_without_newline() {
        let d = Document::parse("---\na: 1\n---");
        assert_eq!(d.body(), "");
        assert_eq!(d.value("a"), Some(&Value::Int(1)));
    }

    #[test]
    fn unclosed_block_is_body() {
        let d = Document::parse("---\na: 1\n");
        assert!(d.frontmatter().is_none());
        assert_eq!(d.body(), "---\na: 1\n");
    }

    #[test]
    fn horizontal_rule_is_not_a_fence() {
        let d = Document::parse("# T\n\n---\n\nx\n");
        assert!(d.frontmatter().is_none());
    }

    #[test]
    fn malformed_yaml_still_round_trips() {
        let src = "---\na: [unclosed\n---\nbody\n";
        let d = Document::parse(src);
        assert!(matches!(
            d.frontmatter().unwrap().state,
            FrontmatterState::Malformed(_)
        ));
        assert_eq!(d.source(), src);
        assert_eq!(d.body(), "body\n");
    }
}
