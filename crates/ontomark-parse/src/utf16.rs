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
    /// `bytes[u]` = byte offset of UTF-16 offset `u`, or `u32::MAX` between
    /// the two halves of a surrogate pair.
    bytes: Vec<u32>,
}

impl Utf16Index {
    /// Index `source`.
    ///
    /// # Panics
    /// Panics when `source` is `u32::MAX` bytes (4 GiB) or longer, so no
    /// offset can reach the `u32::MAX` sentinel or wrap.
    #[must_use]
    pub fn new(source: &str) -> Self {
        assert!(
            u32::try_from(source.len()).is_ok_and(|n| n < u32::MAX),
            "Utf16Index supports sources under 4 GiB"
        );
        // UTF-16 never needs more units than UTF-8 needs bytes, so every
        // offset below fits in u32 without reaching the sentinel.
        let mut units = vec![u32::MAX; source.len() + 1];
        let mut bytes = Vec::with_capacity(source.len() + 1);
        let mut u: u32 = 0;
        for (b, c) in source.char_indices() {
            let b32 = u32::try_from(b).expect("checked above");
            units[b] = u;
            bytes.push(b32);
            if c.len_utf16() == 2 {
                bytes.push(u32::MAX);
            }
            u += u32::try_from(c.len_utf16()).expect("1 or 2");
        }
        units[source.len()] = u;
        bytes.push(u32::try_from(source.len()).expect("checked above"));
        Self { units, bytes }
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

    /// Byte offset of UTF-16 offset `unit`; `None` when out of range or
    /// between the two halves of a surrogate pair. The inverse of
    /// [`Utf16Index::offset`] at every character boundary.
    #[must_use]
    pub fn byte(&self, unit: usize) -> Option<usize> {
        match self.bytes.get(unit) {
            Some(&b) if b != u32::MAX => Some(b as usize),
            _ => None,
        }
    }

    /// `(start, end)` in UTF-16 units for a byte span; `None` when either
    /// end is not a character boundary or the span is reversed.
    #[must_use]
    pub fn span(&self, span: Span) -> Option<(usize, usize)> {
        if span.start > span.end {
            return None;
        }
        Some((self.offset(span.start)?, self.offset(span.end)?))
    }
}
