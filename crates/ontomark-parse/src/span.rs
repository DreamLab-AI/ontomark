//! Byte spans into a document's source, and the source digest that guards patches.

use std::fmt;

use sha2::{Digest as _, Sha256};

/// A half-open byte range `start..end` into a document's source text.
///
/// Every span the parser hands out lies on UTF-8 character boundaries of the
/// source it was taken from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    /// First byte of the range.
    pub start: usize,
    /// One past the last byte of the range.
    pub end: usize,
}

impl Span {
    /// A span from `start` to `end`.
    ///
    /// # Panics
    /// Panics when `end < start`.
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        assert!(end >= start, "span end {end} precedes start {start}");
        Self { start, end }
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// `true` for a zero-length span (an insertion point).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// The text this span covers in `source`.
    ///
    /// # Panics
    /// Panics when the span is out of range or not on character boundaries
    /// of `source`.
    #[must_use]
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }

    /// `true` when `other` lies entirely inside this span.
    #[must_use]
    pub fn contains(&self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// The span moved right by `offset` bytes.
    #[must_use]
    pub fn shift(&self, offset: usize) -> Self {
        Self::new(self.start + offset, self.end + offset)
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// The SHA-256 of a document's exact source bytes.
///
/// A [`crate::Patch`] names the digest of the source it was planned against;
/// applying it to any other source is refused. Displayed as `sha256:<hex>`.
///
/// ```
/// # use ontomark_parse::SourceDigest;
/// let d = SourceDigest::of("abc");
/// assert_eq!(
///     d.to_string(),
///     "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
/// );
/// assert_eq!(SourceDigest::parse(&d.to_string()), Some(d));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceDigest(pub [u8; 32]);

impl SourceDigest {
    /// Digest `source`.
    #[must_use]
    pub fn of(source: &str) -> Self {
        let out = Sha256::digest(source.as_bytes());
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&out);
        Self(bytes)
    }

    /// Lower-case hex of the 32 digest bytes, without the `sha256:` prefix.
    #[must_use]
    pub fn hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push(HEX[usize::from(b >> 4)] as char);
            s.push(HEX[usize::from(b & 0x0f)] as char);
        }
        s
    }

    /// Parse `sha256:<64 hex>` (or bare hex) back into a digest.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix("sha256:").unwrap_or(text);
        if hex.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let hi = (chunk[0] as char).to_digit(16)?;
            let lo = (chunk[1] as char).to_digit(16)?;
            bytes[i] = u8::try_from(hi * 16 + lo).ok()?;
        }
        Some(Self(bytes))
    }
}

impl fmt::Display for SourceDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", self.hex())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for SourceDigest {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SourceDigest {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(d)?;
        Self::parse(&s).ok_or_else(|| serde::de::Error::custom("expected sha256:<64 hex>"))
    }
}
