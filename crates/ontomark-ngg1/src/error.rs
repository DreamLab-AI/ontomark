//! Error types for reading and encoding NGG1.
//!
//! Provenance: [`Ngg1Error`] and the message texts are from vowl-wasm 0.1.2
//! `src/ngg1.rs` (MIT, Copyright (c) 2025-2026 DreamLab-AI); [`Ngg1Fault`] is
//! the typed refinement added in `ontomark-ngg1`.

use core::fmt;

/// Parse error for an NGG1 buffer. Displays a stable, greppable reason.
///
/// This is the vowl-wasm 0.1.2 error type, kept for compatibility. Callers
/// that want to branch on the cause use [`Ngg1::try_parse`](crate::Ngg1::try_parse)
/// and [`Ngg1Fault`] instead.
///
/// ```
/// let err = ontomark_ngg1::Ngg1::parse(&[0u8; 4]).unwrap_err();
/// assert!(err.to_string().starts_with("NGG1: buffer too small"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ngg1Error(pub String);

impl fmt::Display for Ngg1Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NGG1: {}", self.0)
    }
}

impl std::error::Error for Ngg1Error {}

impl From<Ngg1Fault> for Ngg1Error {
    fn from(fault: Ngg1Fault) -> Self {
        Self(fault.to_string())
    }
}

/// The typed reason an NGG1 buffer was rejected.
///
/// ```
/// use ontomark_ngg1::{Ngg1, Ngg1Fault};
///
/// let mut bytes = ontomark_ngg1::worked_example();
/// bytes[0] = 0;
/// assert!(matches!(Ngg1::try_parse(&bytes), Err(Ngg1Fault::BadMagic { .. })));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Ngg1Fault {
    /// The buffer is shorter than the 32-byte header.
    TooSmall {
        /// Buffer length.
        len: usize,
    },
    /// The first four bytes are not `NGG1`.
    BadMagic {
        /// The magic as read, little-endian.
        found: u32,
    },
    /// The version field is not one this crate supports.
    UnsupportedVersion {
        /// The version as read.
        found: u16,
    },
    /// A section offset is not a multiple of four.
    Misaligned,
    /// A section offset points into the header.
    OffsetInHeader {
        /// Section name.
        section: &'static str,
    },
    /// Section arithmetic overflowed `usize`.
    SectionOverflow,
    /// The string table header does not fit in the buffer.
    StringHeaderOutOfBounds,
    /// The nodes, adjacency or edge-type section extends past the buffer.
    SectionOutOfBounds,
    /// The string count is not twice the node count.
    StringCountMismatch {
        /// String count as read.
        count: usize,
        /// Node count as read.
        nodes: usize,
    },
    /// The string blob extends past the buffer.
    BlobOutOfBounds,
    /// A string offset lies past the blob or before its predecessor.
    BadStringOffset {
        /// Index of the offending string.
        index: usize,
    },
    /// `row_ptr` does not start at 0, decreases, or does not end at the edge count.
    BadRowPtr {
        /// Index into `row_ptr` where the rule first fails.
        index: usize,
    },
    /// An edge target is not a valid node index.
    EdgeTargetOutOfRange {
        /// Edge slot.
        edge: usize,
        /// Target as read.
        target: u32,
        /// Node count.
        nodes: usize,
    },
    /// A string is not valid UTF-8 (reported only by strict decoding).
    InvalidUtf8 {
        /// Index of the offending string.
        index: usize,
    },
}

impl fmt::Display for Ngg1Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall { len } => {
                write!(f, "buffer too small ({len} < {})", crate::NGG1_HEADER_SIZE)
            }
            Self::BadMagic { found } => write!(
                f,
                "bad magic 0x{found:08X} (expected 0x{:08X})",
                crate::NGG1_MAGIC
            ),
            Self::UnsupportedVersion { found } => write!(
                f,
                "unsupported version {found} (reader supports {})",
                crate::NGG1_VERSION
            ),
            Self::Misaligned => f.write_str("a section offset is not 4-byte aligned"),
            Self::OffsetInHeader { section } => {
                write!(f, "the {section} section starts inside the header")
            }
            Self::SectionOverflow => f.write_str("section arithmetic overflow"),
            Self::StringHeaderOutOfBounds => f.write_str("string table header exceeds buffer"),
            Self::SectionOutOfBounds => {
                f.write_str("section bounds exceed buffer length (truncated artifact)")
            }
            Self::StringCountMismatch { count, nodes } => write!(
                f,
                "string count {count} != 2 * node_count {}",
                nodes.saturating_mul(2)
            ),
            Self::BlobOutOfBounds => f.write_str("string blob exceeds buffer length"),
            Self::BadStringOffset { index } => write!(f, "string offset {index} is out of order"),
            Self::BadRowPtr { index } => write!(f, "row_ptr[{index}] breaks the CSR invariants"),
            Self::EdgeTargetOutOfRange {
                edge,
                target,
                nodes,
            } => write!(
                f,
                "edge {edge} targets node {target}, but there are {nodes} nodes"
            ),
            Self::InvalidUtf8 { index } => write!(f, "string {index} is not valid UTF-8"),
        }
    }
}

impl std::error::Error for Ngg1Fault {}
