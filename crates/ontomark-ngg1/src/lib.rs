//! `ontomark-ngg1`: the NGG1 binary graph-tier format, in one place.
//!
//! NGG1 is the compact, memory-mappable graph encoding that a publishing
//! pipeline ships to a browser explorer: a node table, a CSR adjacency, one
//! type byte per edge and a string table, all little-endian. This crate owns
//! the format end to end: the constants, the record types, the [`encode`]r and
//! the zero-copy [`Ngg1`] reader. It has no dependencies, uses only `std` and
//! contains no `unsafe` code.
//!
//! # Provenance
//!
//! - The reader, the `NGG1_*` constants and [`worked_example`] come from
//!   `vowl-wasm` 0.1.2 (`src/ngg1.rs`, MIT, Copyright (c) 2025-2026
//!   DreamLab-AI). Every public name and signature of that module is kept, so
//!   `vowl-wasm` can replace it with `pub use ontomark_ngg1::*;`.
//! - The encoder, [`Csr`], [`build_csr`] and the short constant names
//!   ([`MAGIC`], [`VERSION`], ...) come from VisionClaw's
//!   `crates/vault/src/build/ngg1.rs` at `1044b0722`, via `ontomark-compile`.
//!
//! See the `NOTICE` file shipped with the crate.
//!
//! # Byte layout
//!
//! All integers and floats are little-endian. Every section starts on a
//! 4-byte boundary, so a consumer may view `row_ptr`, `col_idx` and the string
//! offsets as `u32` arrays over a 4-aligned buffer. `N` is the node count, `E`
//! the edge count and `S = 2 * N` the string count.
//!
//! | Section | Starts at | Size (bytes) | Content |
//! |---|---|---|---|
//! | header | 0 | 32 | see the header table below |
//! | nodes | `off_nodes` (32) | `24 * N` | `N` node records |
//! | adjacency | `off_adjacency` | `4 * (N + 1) + 4 * E` | `row_ptr[N + 1]: u32`, then `col_idx[E]: u32` |
//! | edge types | `off_edge_types` | `E` plus 0 to 3 zero bytes | `edge_type[E]: u8`, padded to a multiple of 4 |
//! | strings | `off_strings` | `8 + 4 * S + blob_len` | `count: u32`, `blob_len: u32`, `offsets[S]: u32`, blob |
//!
//! The offsets are `off_nodes = 32`, `off_adjacency = 32 + 24 * N`,
//! `off_edge_types = off_adjacency + 4 * (N + 1) + 4 * E` and
//! `off_strings = off_edge_types + E + pad4(E)`, where `pad4(E)` is
//! `(4 - E % 4) % 4`. The file ends with the last byte of the blob.
//!
//! Header (32 bytes):
//!
//! | Offset | Type | Field |
//! |---|---|---|
//! | 0 | `[u8; 4]` | magic, ASCII `NGG1` (`0x3147_474E` as a little-endian `u32`) |
//! | 4 | `u16` | version, currently 1 |
//! | 6 | `u16` | padding, zero |
//! | 8 | `u32` | node count `N` |
//! | 12 | `u32` | edge count `E` |
//! | 16 | `u32` | `off_nodes` |
//! | 20 | `u32` | `off_adjacency` |
//! | 24 | `u32` | `off_edge_types` |
//! | 28 | `u32` | `off_strings` |
//!
//! Node record (24-byte stride; the original brief's "20" is arithmetically
//! impossible once `degree` is 4-aligned):
//!
//! | Offset | Type | Field |
//! |---|---|---|
//! | 0 | `u32` | stable external id, shared across tiers |
//! | 4 | `f32` | seed x position |
//! | 8 | `f32` | seed y position |
//! | 12 | `u16` | domain id, or [`DOMAIN_NONE`] |
//! | 14 | `u16` | category id, or [`CATEGORY_NONE`] |
//! | 16 | `u8` | flags (`FLAG_*`) |
//! | 17 | `[u8; 3]` | padding, zero |
//! | 20 | `u32` | degree in the full graph (ranking key) |
//!
//! Strings: the blob is the concatenation of every string's UTF-8 bytes with
//! no separators. `offsets[s]` is the start of string `s`; string `s` ends at
//! `offsets[s + 1]`, and the last string ends at `blob_len`. Node `i` owns
//! strings `2 * i` (label) and `2 * i + 1` (IRI).
//!
//! Domains and categories are only identifiers here: how many exist, and what
//! they are called, is a property of the producing taxonomy, not of the
//! format. A graph with no domains or no categories is valid.
//!
//! # Encoding and decoding
//!
//! ```
//! use ontomark_ngg1::{build_csr, encode, GraphNode, Ngg1, CATEGORY_NONE, EDGE_SUBCLASS};
//!
//! let node = |id: u32, label: &str| GraphNode {
//!     id,
//!     x: id as f32,
//!     y: 0.0,
//!     domain: 0,
//!     category: CATEGORY_NONE,
//!     flags: 0,
//!     degree: 1,
//!     label: label.to_owned(),
//!     iri: format!("https://example.org/{label}"),
//! };
//! let nodes = vec![node(0, "Animal"), node(1, "Dog")];
//! let csr = build_csr(2, &[(1, 0, EDGE_SUBCLASS)]);
//! let bytes = encode(&nodes, &csr)?;
//!
//! let graph = Ngg1::parse(&bytes)?;
//! assert_eq!((graph.node_count(), graph.edge_count()), (2, 1));
//! assert_eq!(graph.label(1), "Dog");
//! assert_eq!(graph.edges(), (vec![1], vec![0], vec![EDGE_SUBCLASS]));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Malformed input
//!
//! [`Ngg1::try_parse`] validates the whole buffer once (magic, version,
//! alignment, every section against the buffer length, CSR monotonicity, edge
//! targets, string offsets) and returns a typed [`Ngg1Fault`]. Every accessor
//! on a successfully parsed reader is then panic-free for in-range indices.

mod encode;
mod error;
mod example;
mod reader;

pub use encode::{build_csr, encode, Csr, EncodeError, Graph, GraphNode};
pub use error::{Ngg1Error, Ngg1Fault};
pub use example::worked_example;
pub use reader::{Ngg1, NodeRecord};

/// `NGG1` as a little-endian `u32`: `0x3147_474E` (bytes `4E 47 47 31`).
pub const NGG1_MAGIC: u32 = 0x3147_474E;
/// Format version this crate reads and writes.
pub const NGG1_VERSION: u16 = 1;
/// Fixed header size in bytes.
pub const NGG1_HEADER_SIZE: usize = 32;
/// Node record stride in bytes. **24, not 20.**
pub const NGG1_NODE_STRIDE: usize = 24;

/// The magic as the four bytes at the start of every file, `NGG1`.
pub const MAGIC: &[u8; 4] = b"NGG1";
/// Format version; the same value as [`NGG1_VERSION`].
pub const VERSION: u16 = NGG1_VERSION;
/// Header length in bytes; the same value as [`NGG1_HEADER_SIZE`].
pub const HEADER_SIZE: usize = NGG1_HEADER_SIZE;
/// Node record stride in bytes; the same value as [`NGG1_NODE_STRIDE`].
pub const NODE_STRIDE: usize = NGG1_NODE_STRIDE;

/// `edge_type` value for a `subClassOf` edge, the taxonomy backbone.
pub const EDGE_SUBCLASS: u8 = 0;
/// `edge_type` value for an `objectProperty` relation.
pub const EDGE_RELATION: u8 = 1;

/// Node flag: the node is a declared domain root.
pub const FLAG_DOMAIN_ROOT: u8 = 0x01;
/// Node flag: the node is a taxonomy category root.
pub const FLAG_CATEGORY_ROOT: u8 = 0x02;
/// Node flag: the node has an authored page behind it.
pub const FLAG_HAS_PAGE: u8 = 0x04;
/// Node flag: the node ships at least one cross-domain `bridges-to` edge.
pub const FLAG_BRIDGE: u8 = 0x08;
/// Node flag: the node is an OWL individual, not a class.
pub const FLAG_INDIVIDUAL: u8 = 0x10;
/// Node flag: a client-minted collapsed-category metanode. Never written by
/// a producer; the TypeScript explorer sets it on synthetic nodes.
pub const FLAG_META: u8 = 0x40;

/// Domain id for a node with no resolvable domain.
pub const DOMAIN_NONE: u16 = 0xFFFF;
/// Category id for an uncategorised node.
pub const CATEGORY_NONE: u16 = 0xFFFF;
