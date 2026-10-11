//! Text utilities shared by the engine: host coordinates, IRI minting and diffs.

use std::collections::BTreeMap;

use oxrdf::Triple;
use serde::{Deserialize, Serialize};

/// A deduplicated set of triples kept in N-Triples order, so every build
/// (native or wasm) emits the same sequence.
#[derive(Debug, Clone, Default)]
pub struct TripleSet(BTreeMap<String, Triple>);

impl TripleSet {
    /// Insert `t`; returns `false` when it was already present.
    pub fn insert(&mut self, t: Triple) -> bool {
        self.0.insert(t.to_string(), t).is_none()
    }

    /// The triples in canonical order.
    #[must_use]
    pub fn into_vec(self) -> Vec<Triple> {
        self.0.into_values().collect()
    }
}

/// A source range in both coordinate systems a host needs.
///
/// Rust spans are UTF-8 byte offsets; JavaScript strings, CodeMirror and the
/// Obsidian editor index UTF-16 code units. Both are carried so a host never
/// converts, and never mis-highlights a non-ASCII page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSpan {
    /// UTF-8 byte offset of the start.
    pub byte_start: usize,
    /// UTF-8 byte offset one past the end.
    pub byte_end: usize,
    /// UTF-16 code-unit offset of the start.
    pub utf16_start: usize,
    /// UTF-16 code-unit offset one past the end.
    pub utf16_end: usize,
    /// 1-based line of the start.
    pub line: usize,
    /// 1-based column of the start, in characters.
    pub col: usize,
}

impl HostSpan {
    /// Coordinates of `start..end` (byte offsets on character boundaries) in `source`.
    #[must_use]
    pub fn of(source: &str, start: usize, end: usize) -> Self {
        let start = start.min(source.len());
        let end = end.clamp(start, source.len());
        let before = &source[..start];
        let line = 1 + before.matches('\n').count();
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let col = 1 + source[line_start..start].chars().count();
        Self {
            byte_start: start,
            byte_end: end,
            utf16_start: utf16_len(before),
            utf16_end: utf16_len(&source[..end]),
            line,
            col,
        }
    }
}

/// Length of `s` in UTF-16 code units.
#[must_use]
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Percent-encode a vault path for use inside a URN: unreserved characters
/// and `/` stay, every other byte of the UTF-8 encoding becomes `%XX`.
#[must_use]
pub fn pct_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A unified diff of `before` → `after`, labelled with `path`, three lines of context.
#[must_use]
pub fn unified_diff(path: &str, before: &str, after: &str) -> String {
    similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{path}"), &format!("b/{path}"))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_span_counts_utf16_for_astral_and_accented_text() {
        // "é" is 2 bytes / 1 unit; "𝔸" is 4 bytes / 2 units.
        let src = "é𝔸\nx[[y]]";
        let at = src.find("[[").unwrap();
        let span = HostSpan::of(src, at, at + 5);
        assert_eq!(span.byte_start, 8);
        assert_eq!(span.utf16_start, 5);
        assert_eq!(span.utf16_end, 10);
        assert_eq!((span.line, span.col), (2, 2));
        let js_like: Vec<u16> = src.encode_utf16().collect();
        assert_eq!(
            String::from_utf16(&js_like[span.utf16_start..span.utf16_end]).unwrap(),
            "[[y]]"
        );
    }

    #[test]
    fn pct_encoding_keeps_slashes_and_encodes_unicode() {
        assert_eq!(pct_encode_path("Dir/Café note"), "Dir/Caf%C3%A9%20note");
    }

    #[test]
    fn diff_has_headers_and_hunks() {
        let d = unified_diff("p.md", "a\nb\n", "a\nc\n");
        assert!(d.starts_with("--- a/p.md\n+++ b/p.md\n"), "{d}");
        assert!(d.contains("-b\n+c\n"), "{d}");
    }
}
