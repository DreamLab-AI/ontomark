//! The NGG1 encoder and the record types it consumes.
//!
//! Provenance: the byte-writing code, [`Csr`] and [`build_csr`] are the
//! VisionClaw `crates/vault/src/build/ngg1.rs` writer at `1044b0722`, moved
//! here from `ontomark-compile`; the output is held byte-identical to it by
//! the conformance suite. Compared with that code, the sanity checks that were
//! `assert!`s are now typed [`EncodeError`]s. The layout algorithms stayed in
//! `ontomark-compile`. Derived in part from vowl-wasm 0.1.2 (MIT, Copyright
//! (c) 2025-2026 DreamLab-AI).

use core::fmt;

use crate::{HEADER_SIZE, MAGIC, NODE_STRIDE, VERSION};

/// One graph node as stored in a tier, in local-index order.
///
/// This is the owned form of [`NodeRecord`](crate::NodeRecord) plus the two
/// strings. Positions are `f32` because that is what the file stores;
/// producers that lay out in `f64` narrow with `as f32` exactly once.
///
/// ```
/// use ontomark_ngg1::{GraphNode, CATEGORY_NONE, DOMAIN_NONE};
///
/// let node = GraphNode {
///     id: 7,
///     x: 1.5,
///     y: -2.0,
///     domain: DOMAIN_NONE,
///     category: CATEGORY_NONE,
///     flags: 0,
///     degree: 0,
///     label: "Café".to_owned(),
///     iri: "https://example.org/cafe".to_owned(),
/// };
/// assert_eq!(node.label.chars().count(), 4);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    /// Stable external id, shared across tiers.
    pub id: u32,
    /// Seed x position.
    pub x: f32,
    /// Seed y position.
    pub y: f32,
    /// Domain id, or [`DOMAIN_NONE`](crate::DOMAIN_NONE).
    pub domain: u16,
    /// Category id, or [`CATEGORY_NONE`](crate::CATEGORY_NONE).
    pub category: u16,
    /// Flag bits (`FLAG_*`).
    pub flags: u8,
    /// Full-graph incident degree.
    pub degree: u32,
    /// Display label.
    pub label: String,
    /// Canonical IRI.
    pub iri: String,
}

/// Compressed sparse row adjacency.
///
/// ```
/// let csr = ontomark_ngg1::build_csr(2, &[(0, 1, 0)]);
/// assert_eq!(csr.row_ptr, vec![0, 1, 1]);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Csr {
    /// `nodes + 1` offsets into [`Csr::col_idx`].
    pub row_ptr: Vec<u32>,
    /// Target local indices.
    pub col_idx: Vec<u32>,
    /// One edge type per entry of [`Csr::col_idx`].
    pub edge_type: Vec<u8>,
}

/// A decoded tier: nodes plus adjacency. Produced by
/// [`Ngg1::to_graph`](crate::Ngg1::to_graph) and written by [`Graph::encode`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Graph {
    /// Nodes in local-index order.
    pub nodes: Vec<GraphNode>,
    /// Adjacency over the local indices of [`Graph::nodes`].
    pub csr: Csr,
}

impl Graph {
    /// Serialise with [`encode`].
    ///
    /// # Errors
    /// As [`encode`].
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        encode(&self.nodes, &self.csr)
    }
}

/// Build CSR from directed `(src_local, tgt_local, edge_type)` triples.
///
/// Edges arrive pre-ordered per source (backbone before relations); this only
/// groups them, preserving arrival order within a source.
///
/// # Panics
/// When a source index is not below `num_nodes`.
///
/// ```
/// let csr = ontomark_ngg1::build_csr(3, &[(0, 1, 0), (0, 2, 1), (2, 0, 0)]);
/// assert_eq!(csr.row_ptr, vec![0, 2, 2, 3]);
/// assert_eq!(csr.col_idx, vec![1, 2, 0]);
/// assert_eq!(csr.edge_type, vec![0, 1, 0]);
/// ```
#[must_use]
pub fn build_csr(num_nodes: usize, edges: &[(u32, u32, u8)]) -> Csr {
    let mut counts = vec![0u32; num_nodes];
    for (src, _, _) in edges {
        counts[*src as usize] += 1;
    }
    let mut row_ptr = vec![0u32; num_nodes + 1];
    for i in 0..num_nodes {
        row_ptr[i + 1] = row_ptr[i] + counts[i];
    }
    let mut col_idx = vec![0u32; edges.len()];
    let mut edge_type = vec![0u8; edges.len()];
    let mut cursor: Vec<u32> = row_ptr[..num_nodes].to_vec();
    for (src, tgt, t) in edges {
        let p = cursor[*src as usize] as usize;
        col_idx[p] = *tgt;
        edge_type[p] = *t;
        cursor[*src as usize] += 1;
    }
    Csr {
        row_ptr,
        col_idx,
        edge_type,
    }
}

/// Why [`encode`] refused its input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// `row_ptr` is not `nodes + 1` long.
    RowPtrLength {
        /// Length found.
        found: usize,
        /// Length required.
        expected: usize,
    },
    /// `edge_type` is not as long as `col_idx`.
    EdgeTypeLength {
        /// Length found.
        found: usize,
        /// Length required.
        expected: usize,
    },
    /// `row_ptr` does not start at 0, decreases, or does not end at the edge count.
    RowPtr {
        /// Index into `row_ptr` where the rule first fails.
        index: usize,
    },
    /// An edge target is not a node index.
    EdgeTarget {
        /// Edge slot.
        edge: usize,
        /// Target found.
        target: u32,
    },
    /// A count or offset does not fit the format's `u32` fields.
    TooLarge,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RowPtrLength { found, expected } => {
                write!(f, "row_ptr length {found}, expected {expected}")
            }
            Self::EdgeTypeLength { found, expected } => {
                write!(f, "edge_type length {found}, expected {expected}")
            }
            Self::RowPtr { index } => write!(f, "row_ptr[{index}] breaks the CSR invariants"),
            Self::EdgeTarget { edge, target } => {
                write!(f, "edge {edge} targets node {target}, which does not exist")
            }
            Self::TooLarge => f.write_str("graph exceeds the u32 limits of the NGG1 format"),
        }
    }
}

impl std::error::Error for EncodeError {}

const fn pad4(n: usize) -> usize {
    (4 - (n & 3)) & 3
}

fn to_u32(v: usize) -> Result<u32, EncodeError> {
    u32::try_from(v).map_err(|_| EncodeError::TooLarge)
}

/// Serialise one tier to NGG1 bytes.
///
/// `nodes` are in local-index order and `csr.col_idx` holds local indices.
/// The input is checked first: whatever this returns, [`Ngg1::parse`] accepts.
///
/// # Errors
/// An [`EncodeError`] when `csr` is inconsistent with `nodes` or the graph is
/// too large for the format's `u32` fields.
///
/// ```
/// use ontomark_ngg1::{build_csr, encode, GraphNode, Ngg1, DOMAIN_NONE, CATEGORY_NONE};
///
/// let nodes: Vec<GraphNode> = (0..3)
///     .map(|i| GraphNode {
///         id: i,
///         x: 0.0,
///         y: 0.0,
///         domain: DOMAIN_NONE,
///         category: CATEGORY_NONE,
///         flags: 0,
///         degree: 0,
///         label: format!("N{i}"),
///         iri: format!("urn:n:{i}"),
///     })
///     .collect();
/// let bytes = encode(&nodes, &build_csr(3, &[(0, 1, 0), (1, 2, 1), (2, 0, 1)]))?;
/// // 32 header + 3 * 24 nodes + 16 row_ptr + 12 col_idx + (3 types + 1 pad) + strings
/// assert_eq!(&bytes[..4], b"NGG1");
/// assert_eq!(Ngg1::parse(&bytes)?.edge_count(), 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// [`Ngg1::parse`]: crate::Ngg1::parse
pub fn encode(nodes: &[GraphNode], csr: &Csr) -> Result<Vec<u8>, EncodeError> {
    let node_count = nodes.len();
    let edge_count = csr.col_idx.len();
    if csr.row_ptr.len() != node_count + 1 {
        return Err(EncodeError::RowPtrLength {
            found: csr.row_ptr.len(),
            expected: node_count + 1,
        });
    }
    if csr.edge_type.len() != edge_count {
        return Err(EncodeError::EdgeTypeLength {
            found: csr.edge_type.len(),
            expected: edge_count,
        });
    }
    let mut last = 0u32;
    if csr.row_ptr[0] != 0 {
        return Err(EncodeError::RowPtr { index: 0 });
    }
    for (i, &v) in csr.row_ptr.iter().enumerate().skip(1) {
        if v < last || v as usize > edge_count {
            return Err(EncodeError::RowPtr { index: i });
        }
        last = v;
    }
    if last as usize != edge_count {
        return Err(EncodeError::RowPtr { index: node_count });
    }
    if let Some((edge, &target)) = csr
        .col_idx
        .iter()
        .enumerate()
        .find(|&(_, &t)| t as usize >= node_count)
    {
        return Err(EncodeError::EdgeTarget { edge, target });
    }

    // Layout arithmetic in u64 so a 32-bit target cannot wrap before the check.
    let n = node_count as u64;
    let e = edge_count as u64;
    let off_nodes = HEADER_SIZE as u64;
    let off_adjacency = off_nodes + n * NODE_STRIDE as u64;
    let off_edge_types = off_adjacency + (n + 1) * 4 + e * 4;
    let off_strings = off_edge_types + e + pad4(edge_count) as u64;
    let off_nodes = HEADER_SIZE as u32;
    let off_adjacency = u32::try_from(off_adjacency).map_err(|_| EncodeError::TooLarge)?;
    let off_edge_types = u32::try_from(off_edge_types).map_err(|_| EncodeError::TooLarge)?;
    let off_strings = u32::try_from(off_strings).map_err(|_| EncodeError::TooLarge)?;

    let mut out: Vec<u8> = Vec::with_capacity(off_strings as usize + 8 + node_count * 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // pad
    for v in [
        to_u32(node_count)?,
        to_u32(edge_count)?,
        off_nodes,
        off_adjacency,
        off_edge_types,
        off_strings,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }

    for node in nodes {
        out.extend_from_slice(&node.id.to_le_bytes());
        out.extend_from_slice(&node.x.to_le_bytes());
        out.extend_from_slice(&node.y.to_le_bytes());
        out.extend_from_slice(&node.domain.to_le_bytes());
        out.extend_from_slice(&node.category.to_le_bytes());
        out.push(node.flags);
        out.extend_from_slice(&[0u8; 3]); // pad
        out.extend_from_slice(&node.degree.to_le_bytes());
    }

    for v in &csr.row_ptr {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in &csr.col_idx {
        out.extend_from_slice(&v.to_le_bytes());
    }

    out.extend_from_slice(&csr.edge_type);
    out.extend(std::iter::repeat_n(0u8, pad4(edge_count)));

    let mut blob: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(node_count * 2);
    for node in nodes {
        offsets.push(to_u32(blob.len())?);
        blob.extend_from_slice(node.label.as_bytes());
        offsets.push(to_u32(blob.len())?);
        blob.extend_from_slice(node.iri.as_bytes());
    }
    out.extend_from_slice(&to_u32(offsets.len())?.to_le_bytes());
    out.extend_from_slice(&to_u32(blob.len())?.to_le_bytes());
    for v in &offsets {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&blob);

    debug_assert_eq!(
        out.len(),
        off_strings as usize + 8 + offsets.len() * 4 + blob.len()
    );
    Ok(out)
}
