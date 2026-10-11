# ontomark-ngg1

The NGG1 binary graph-tier format in one dependency-free crate: format
constants, record types, an encoder and a zero-copy reader. `std` only, no
`unsafe`.

NGG1 is the compact, little-endian encoding a publishing pipeline ships to a
browser explorer: a node table (24-byte records), a CSR adjacency, one type byte
per edge and a string table. The byte-layout table is in the crate-level
documentation (`cargo doc -p ontomark-ngg1 --open`).

```rust
use ontomark_ngg1::{build_csr, encode, GraphNode, Ngg1, CATEGORY_NONE, DOMAIN_NONE, EDGE_SUBCLASS};

let node = |id: u32, label: &str| GraphNode {
    id, x: 0.0, y: 0.0, domain: DOMAIN_NONE, category: CATEGORY_NONE,
    flags: 0, degree: 1, label: label.into(), iri: format!("urn:{label}"),
};
let bytes = encode(&[node(0, "a"), node(1, "b")], &build_csr(2, &[(1, 0, EDGE_SUBCLASS)]))?;

let graph = Ngg1::try_parse(&bytes)?;      // typed Ngg1Fault on malformed input
assert_eq!(graph.label(1), "b");
assert_eq!(graph.to_graph()?.encode()?, bytes); // decode and re-encode is byte-identical
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance and licence

Licensed under MIT OR Apache-2.0. The reader and the `NGG1_*` constants derive
from `vowl-wasm` 0.1.2 (MIT, Copyright (c) 2025-2026 DreamLab-AI); the encoder
derives from VisionClaw's `crates/vault/src/build/ngg1.rs` at `1044b0722`. See
`NOTICE`.

The public API of `vowl-wasm`'s `ngg1` module is kept name for name, so that
crate can replace its module with `pub use ontomark_ngg1::*;`.
