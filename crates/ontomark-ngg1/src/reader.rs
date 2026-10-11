//! The zero-copy NGG1 reader.
//!
//! Provenance: derived from vowl-wasm 0.1.2 `src/ngg1.rs` (MIT, Copyright
//! (c) 2025-2026 DreamLab-AI), itself the Rust counterpart of the TypeScript
//! reader `ngg1.ts` in WasmVOWL. The public names and signatures are
//! unchanged; validation is stricter (see [`Ngg1::try_parse`]) and
//! [`Ngg1::to_graph`] is new.
//!
//! The reader borrows the input slice and decodes fields on demand with
//! `from_le_bytes`: no `unsafe`, and no alignment assumption, because a slice
//! handed over from JavaScript is not guaranteed to be 4-aligned.

use crate::encode::{Csr, Graph, GraphNode};
use crate::error::{Ngg1Error, Ngg1Fault};
use crate::{NGG1_HEADER_SIZE, NGG1_MAGIC, NGG1_NODE_STRIDE, NGG1_VERSION};

/// Byte offsets of fields within a 24-byte node record.
const N_ID: usize = 0;
const N_X: usize = 4;
const N_Y: usize = 8;
const N_DOMAIN: usize = 12;
const N_CATEGORY: usize = 14;
const N_FLAGS: usize = 16;
// bytes 17..20 are padding that aligns `degree`.
const N_DEGREE: usize = 20;

/// A single decoded node record. All fields are copied out; the record
/// carries no borrow.
///
/// ```
/// let bytes = ontomark_ngg1::worked_example();
/// let graph = ontomark_ngg1::Ngg1::parse(&bytes)?;
/// let n1 = graph.node(1);
/// assert_eq!((n1.id, n1.x, n1.degree), (1, 1.0, 2));
/// # Ok::<(), ontomark_ngg1::Ngg1Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeRecord {
    /// Stable external node id (page-API and stats identity). Not
    /// necessarily the local index.
    pub id: u32,
    /// Seed layout x; the physics worker warm-starts here.
    pub x: f32,
    /// Seed layout y.
    pub y: f32,
    /// Domain id, an index into the producer's domain list, or
    /// [`DOMAIN_NONE`](crate::DOMAIN_NONE).
    pub domain: u16,
    /// Category id, an index into the producer's category list, or
    /// [`CATEGORY_NONE`](crate::CATEGORY_NONE) if uncategorised.
    pub category: u16,
    /// Flags bitfield (`FLAG_*`).
    pub flags: u8,
    /// Total incident degree in the **full** graph: the ranking key for top-N
    /// labels and node scale, preserved across tiers even when edges are
    /// capped.
    pub degree: u32,
}

#[inline]
fn rd_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

#[inline]
fn rd_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

#[inline]
fn rd_f32(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// A parsed, borrowed NGG1 tier.
///
/// Every accessor is O(1) and allocation-free except the string decodes and
/// the explicit [`seed_positions`](Self::seed_positions),
/// [`edges`](Self::edges), [`degrees`](Self::degrees) and
/// [`to_graph`](Self::to_graph) builders. Node accessors take a *local
/// index* `i` in `[0, node_count)`.
///
/// ```
/// use ontomark_ngg1::Ngg1;
///
/// let bytes = ontomark_ngg1::worked_example();
/// let graph = Ngg1::parse(&bytes)?;
/// assert_eq!(graph.node_count(), 3);
/// assert_eq!(graph.label(2), "NLP");
/// assert_eq!(graph.iri(2), "ng:2");
/// # Ok::<(), ontomark_ngg1::Ngg1Error>(())
/// ```
pub struct Ngg1<'a> {
    bytes: &'a [u8],
    node_count: usize,
    edge_count: usize,
    off_nodes: usize,
    off_adjacency: usize,
    off_col_idx: usize,
    off_edge_types: usize,
    str_count: usize,
    blob_len: usize,
    off_str_offsets: usize,
    off_blob: usize,
}

impl core::fmt::Debug for Ngg1<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ngg1")
            .field("bytes", &self.bytes.len())
            .field("node_count", &self.node_count)
            .field("edge_count", &self.edge_count)
            .finish_non_exhaustive()
    }
}

impl<'a> Ngg1<'a> {
    /// Parse and fully validate an NGG1 buffer.
    ///
    /// This is [`try_parse`](Self::try_parse) with the cause flattened into
    /// the vowl-wasm 0.1.2 [`Ngg1Error`] string.
    ///
    /// # Errors
    /// [`Ngg1Error`] when the buffer is malformed; see [`Ngg1Fault`].
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Ngg1Error> {
        Self::try_parse(bytes).map_err(Ngg1Error::from)
    }

    /// Parse and fully validate an NGG1 buffer, reporting a typed fault.
    ///
    /// Checks magic, version, alignment, every section end against the buffer
    /// length, the string count, that string offsets are in range and
    /// non-decreasing, that `row_ptr` starts at 0, never decreases and ends at
    /// the edge count, and that every edge target is a valid node index. A
    /// buffer that passes can be read through every accessor without
    /// panicking. Unknown edge types and flag bits are not errors.
    ///
    /// # Errors
    /// The first [`Ngg1Fault`] found.
    ///
    /// ```
    /// use ontomark_ngg1::{Ngg1, Ngg1Fault};
    ///
    /// let bytes = ontomark_ngg1::worked_example();
    /// assert!(Ngg1::try_parse(&bytes).is_ok());
    /// assert!(matches!(
    ///     Ngg1::try_parse(&bytes[..100]),
    ///     Err(Ngg1Fault::SectionOutOfBounds | Ngg1Fault::StringHeaderOutOfBounds)
    /// ));
    /// ```
    pub fn try_parse(bytes: &'a [u8]) -> Result<Self, Ngg1Fault> {
        let len = bytes.len();
        if len < NGG1_HEADER_SIZE {
            return Err(Ngg1Fault::TooSmall { len });
        }
        let magic = rd_u32(bytes, 0);
        if magic != NGG1_MAGIC {
            return Err(Ngg1Fault::BadMagic { found: magic });
        }
        let version = rd_u16(bytes, 4);
        if version != NGG1_VERSION {
            return Err(Ngg1Fault::UnsupportedVersion { found: version });
        }
        let node_count = rd_u32(bytes, 8) as usize;
        let edge_count = rd_u32(bytes, 12) as usize;
        let off_nodes = rd_u32(bytes, 16) as usize;
        let off_adjacency = rd_u32(bytes, 20) as usize;
        let off_edge_types = rd_u32(bytes, 24) as usize;
        let off_strings = rd_u32(bytes, 28) as usize;

        if (off_nodes | off_adjacency | off_edge_types | off_strings) & 3 != 0 {
            return Err(Ngg1Fault::Misaligned);
        }
        for (section, off) in [
            ("nodes", off_nodes),
            ("adjacency", off_adjacency),
            ("edge types", off_edge_types),
            ("strings", off_strings),
        ] {
            if off < NGG1_HEADER_SIZE {
                return Err(Ngg1Fault::OffsetInHeader { section });
            }
        }

        let ov = || Ngg1Fault::SectionOverflow;
        let nodes_end = off_nodes
            .checked_add(node_count.checked_mul(NGG1_NODE_STRIDE).ok_or_else(ov)?)
            .ok_or_else(ov)?;
        let row_ptr_len = node_count.checked_add(1).ok_or_else(ov)?;
        let off_col_idx = off_adjacency
            .checked_add(row_ptr_len.checked_mul(4).ok_or_else(ov)?)
            .ok_or_else(ov)?;
        let adj_end = off_col_idx
            .checked_add(edge_count.checked_mul(4).ok_or_else(ov)?)
            .ok_or_else(ov)?;
        let edge_types_end = off_edge_types.checked_add(edge_count).ok_or_else(ov)?;
        if off_strings.checked_add(8).ok_or_else(ov)? > len {
            return Err(Ngg1Fault::StringHeaderOutOfBounds);
        }
        if nodes_end > len || adj_end > len || edge_types_end > len {
            return Err(Ngg1Fault::SectionOutOfBounds);
        }

        let str_count = rd_u32(bytes, off_strings) as usize;
        let blob_len = rd_u32(bytes, off_strings + 4) as usize;
        if Some(str_count) != node_count.checked_mul(2) {
            return Err(Ngg1Fault::StringCountMismatch {
                count: str_count,
                nodes: node_count,
            });
        }
        let off_str_offsets = off_strings + 8;
        let off_blob = off_str_offsets
            .checked_add(str_count.checked_mul(4).ok_or_else(ov)?)
            .ok_or_else(ov)?;
        if off_blob.checked_add(blob_len).ok_or_else(ov)? > len {
            return Err(Ngg1Fault::BlobOutOfBounds);
        }

        let parsed = Self {
            bytes,
            node_count,
            edge_count,
            off_nodes,
            off_adjacency,
            off_col_idx,
            off_edge_types,
            str_count,
            blob_len,
            off_str_offsets,
            off_blob,
        };
        parsed.validate_topology_and_strings()?;
        Ok(parsed)
    }

    /// The structural checks that need the section bounds already proven.
    fn validate_topology_and_strings(&self) -> Result<(), Ngg1Fault> {
        let mut prev = 0usize;
        for s in 0..self.str_count {
            let off = rd_u32(self.bytes, self.off_str_offsets + s * 4) as usize;
            if off > self.blob_len || off < prev {
                return Err(Ngg1Fault::BadStringOffset { index: s });
            }
            prev = off;
        }
        if self.row_ptr(0) != 0 {
            return Err(Ngg1Fault::BadRowPtr { index: 0 });
        }
        let mut last = 0u32;
        for i in 1..=self.node_count {
            let v = self.row_ptr(i);
            if v < last || v as usize > self.edge_count {
                return Err(Ngg1Fault::BadRowPtr { index: i });
            }
            last = v;
        }
        if last as usize != self.edge_count {
            return Err(Ngg1Fault::BadRowPtr {
                index: self.node_count,
            });
        }
        for e in 0..self.edge_count {
            let target = self.col_idx(e);
            if target as usize >= self.node_count {
                return Err(Ngg1Fault::EdgeTargetOutOfRange {
                    edge: e,
                    target,
                    nodes: self.node_count,
                });
            }
        }
        Ok(())
    }

    /// Number of node records.
    #[inline]
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Number of directed edges.
    #[inline]
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    /// Decode node record `i`.
    ///
    /// # Panics
    /// When `i >= node_count()`.
    #[inline]
    #[must_use]
    pub fn node(&self, i: usize) -> NodeRecord {
        assert!(i < self.node_count, "NGG1: node index {i} out of range");
        let o = self.off_nodes + i * NGG1_NODE_STRIDE;
        NodeRecord {
            id: rd_u32(self.bytes, o + N_ID),
            x: rd_f32(self.bytes, o + N_X),
            y: rd_f32(self.bytes, o + N_Y),
            domain: rd_u16(self.bytes, o + N_DOMAIN),
            category: rd_u16(self.bytes, o + N_CATEGORY),
            flags: self.bytes[o + N_FLAGS],
            degree: rd_u32(self.bytes, o + N_DEGREE),
        }
    }

    /// `row_ptr[i]`, the start of node `i`'s out-edge slot range. `i` is in
    /// `[0, node_count]`.
    #[inline]
    #[must_use]
    pub fn row_ptr(&self, i: usize) -> u32 {
        debug_assert!(i <= self.node_count);
        rd_u32(self.bytes, self.off_adjacency + i * 4)
    }

    /// `col_idx[e]`, the target local index of edge slot `e`.
    #[inline]
    #[must_use]
    pub fn col_idx(&self, e: usize) -> u32 {
        debug_assert!(e < self.edge_count);
        rd_u32(self.bytes, self.off_col_idx + e * 4)
    }

    /// `edge_type[e]` ([`EDGE_SUBCLASS`](crate::EDGE_SUBCLASS) or
    /// [`EDGE_RELATION`](crate::EDGE_RELATION)).
    #[inline]
    #[must_use]
    pub fn edge_type(&self, e: usize) -> u8 {
        debug_assert!(e < self.edge_count);
        self.bytes[self.off_edge_types + e]
    }

    /// Raw string `s` in `[0, 2 * node_count)`, UTF-8 decoded lossily: a
    /// corrupt blob degrades to replacement characters rather than panicking.
    ///
    /// # Panics
    /// When `s >= 2 * node_count()`.
    #[must_use]
    pub fn string(&self, s: usize) -> String {
        assert!(s < self.str_count, "NGG1: string index {s} out of range");
        String::from_utf8_lossy(self.string_bytes(s)).into_owned()
    }

    fn string_bytes(&self, s: usize) -> &'a [u8] {
        let start = rd_u32(self.bytes, self.off_str_offsets + s * 4) as usize;
        let end = if s + 1 < self.str_count {
            rd_u32(self.bytes, self.off_str_offsets + (s + 1) * 4) as usize
        } else {
            self.blob_len
        };
        let a = self.off_blob + start.min(self.blob_len);
        let b = self.off_blob + end.min(self.blob_len);
        &self.bytes[a..b.max(a)]
    }

    /// Label of node `i`, string `2 * i`.
    ///
    /// # Panics
    /// When `i >= node_count()`.
    #[inline]
    #[must_use]
    pub fn label(&self, i: usize) -> String {
        self.string(i * 2)
    }

    /// IRI of node `i`, string `2 * i + 1`.
    ///
    /// # Panics
    /// When `i >= node_count()`.
    #[inline]
    #[must_use]
    pub fn iri(&self, i: usize) -> String {
        self.string(i * 2 + 1)
    }

    /// Packed seed positions `[x0, y0, x1, y1, ...]`, length
    /// `node_count * 2`: the warm-start buffer for a simulation.
    #[must_use]
    pub fn seed_positions(&self) -> Vec<f32> {
        let mut out = vec![0.0f32; self.node_count * 2];
        for i in 0..self.node_count {
            let o = self.off_nodes + i * NGG1_NODE_STRIDE;
            out[i * 2] = rd_f32(self.bytes, o + N_X);
            out[i * 2 + 1] = rd_f32(self.bytes, o + N_Y);
        }
        out
    }

    /// The directed edge list as parallel `(src, tgt, edge_type)` arrays,
    /// walked from the CSR.
    #[must_use]
    pub fn edges(&self) -> (Vec<u32>, Vec<u32>, Vec<u8>) {
        let e = self.edge_count;
        let mut src = Vec::with_capacity(e);
        let mut tgt = Vec::with_capacity(e);
        let mut ty = Vec::with_capacity(e);
        for i in 0..self.node_count {
            let lo = self.row_ptr(i) as usize;
            let hi = self.row_ptr(i + 1) as usize;
            for slot in lo..hi.min(e) {
                // A node count above u32::MAX cannot parse (the header field is u32).
                src.push(u32::try_from(i).unwrap_or(u32::MAX));
                tgt.push(self.col_idx(slot));
                ty.push(self.edge_type(slot));
            }
        }
        (src, tgt, ty)
    }

    /// Per-node full-graph degree vector.
    #[must_use]
    pub fn degrees(&self) -> Vec<u32> {
        (0..self.node_count).map(|i| self.node(i).degree).collect()
    }

    /// Decode the whole tier into an owned [`Graph`] that
    /// [`encode`](crate::encode) writes back byte for byte.
    ///
    /// Unlike [`string`](Self::string) this is strict about UTF-8.
    ///
    /// # Errors
    /// [`Ngg1Fault::InvalidUtf8`] when a label or IRI is not valid UTF-8.
    ///
    /// ```
    /// let bytes = ontomark_ngg1::worked_example();
    /// let graph = ontomark_ngg1::Ngg1::parse(&bytes)?.to_graph()?;
    /// assert_eq!(graph.encode()?, bytes);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn to_graph(&self) -> Result<Graph, Ngg1Fault> {
        let text = |s: usize| {
            std::str::from_utf8(self.string_bytes(s))
                .map(str::to_owned)
                .map_err(|_| Ngg1Fault::InvalidUtf8 { index: s })
        };
        let mut nodes = Vec::with_capacity(self.node_count);
        for i in 0..self.node_count {
            let r = self.node(i);
            nodes.push(GraphNode {
                id: r.id,
                x: r.x,
                y: r.y,
                domain: r.domain,
                category: r.category,
                flags: r.flags,
                degree: r.degree,
                label: text(i * 2)?,
                iri: text(i * 2 + 1)?,
            });
        }
        let row_ptr = (0..=self.node_count).map(|i| self.row_ptr(i)).collect();
        let col_idx = (0..self.edge_count).map(|e| self.col_idx(e)).collect();
        let edge_type = (0..self.edge_count).map(|e| self.edge_type(e)).collect();
        Ok(Graph {
            nodes,
            csr: Csr {
                row_ptr,
                col_idx,
                edge_type,
            },
        })
    }
}
