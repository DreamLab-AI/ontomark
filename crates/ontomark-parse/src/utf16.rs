//! UTF-16 code-unit offsets for byte spans, for hosts (JavaScript, CodeMirror,
//! Obsidian) whose string indices are UTF-16.

use crate::span::Span;

/// A byte → UTF-16 offset table for one source string.
///
/// Built once per document in O(n); each lookup is O(1).
///
/// ```
/// # use ontomark_parse::{Utf16Index, Span};
/// let s = "é𝄞x"; // 2 + 4 + 1 bytes; 1 + 2 + 1 UTF-16 units
/// let idx = Utf16Index::new(s);
/// assert_eq!(idx.offset(2), Some(1));
/// assert_eq!(idx.offset(6), Some(3));
/// assert_eq!(idx.span(Span::new(2, 7)), Some((1, 4)));
/// assert_eq!(idx.offset(1), None); // inside `é`
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utf16Index {
    /// `units[b]` = UTF-16 offset of byte `b`, or `u32::MAX` inside a character.
    units: Vec<u32>,
}

impl Utf16Index {
    /// Index `source`.
    ///
    /// # Panics
    /// Panics when `source` is 4 GiB or longer.
    #[must_use]
    pub fn new(source: &str) -> Self {
        let mut units = vec![u32::MAX; source.len() + 1];
        let mut u: u32 = 0;
        for (b, c) in source.char_indices() {
            units[b] = u;
            u += u32::try_from(c.len_utf16()).expect("1 or 2");
        }
        units[source.len()] = u;
        Self { units }
    }

    /// UTF-16 offset of byte offset `byte`; `None` when out of range or not
    /// on a character boundary.
    #[must_use]
    pub fn offset(&self, byte: usize) -> Option<usize> {
        match self.units.get(byte) {
            Some(&u) if u != u32::MAX => Some(u as usize),
            _ => None,
        }
    }

    /// `(start, end)` in UTF-16 units for a byte span.
    #[must_use]
    pub fn span(&self, span: Span) -> Option<(usize, usize)> {
        Some((self.offset(span.start)?, self.offset(span.end)?))
    }
}
