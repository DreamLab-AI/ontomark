# vowl-wasm re-exporting ontomark-ngg1

Proof that `vowl-wasm` 0.1.2's `ngg1` module can be replaced by
`pub use ontomark_ngg1::*;` with every existing test still passing. The
published crate at `/home/devuser/workspace/vowl-wasm` was **not** modified;
the proof ran on a copy (2026-10-11).

## Procedure

```sh
mkdir -p /home/devuser/workspace/.tmp/vowl-wasm-reexport
cd /home/devuser/workspace/vowl-wasm
tar --exclude=./target --exclude=./pkg --exclude=./.git -cf - . \
  | tar -xf - -C /home/devuser/workspace/.tmp/vowl-wasm-reexport
cd /home/devuser/workspace/.tmp/vowl-wasm-reexport
# apply the diff below (src/ngg1.rs body and the Cargo.toml dependency), then:
export CARGO_TARGET_DIR=/home/devuser/workspace/.tmp/vowl-reexport-target
cargo test --features ngg1
cargo build --target wasm32-unknown-unknown --features ngg1
```

## Results

| Run | Command | Result |
|---|---|---|
| Baseline (unmodified crate) | `cargo test --locked --features ngg1` | lib 147, integration 5, ngg1_explorer 4, ontology_metadata 11, phase3 7 (+2 ignored), doctests 6: all pass |
| Re-export | `cargo test --features ngg1` | lib 138, integration 5, ngg1_explorer 4, ontology_metadata 11, phase3 7 (+2 ignored), doctests 6: all pass |
| Re-export, wasm | `cargo build --target wasm32-unknown-unknown --features ngg1` | builds |

The nine fewer lib tests are the unit tests that lived inside `src/ngg1.rs`
(`parses_worked_example_header`, `node_stride_24_fields`, `strings_pair_label_iri`,
`csr_topology`, `seed_positions_packed`, `rejects_bad_magic`, `rejects_bad_version`,
`rejects_truncated`, `rejects_too_small`). They move to `ontomark-ngg1`
(`tests/malformed.rs`, `tests/roundtrip.rs`, doctests on `worked_example`, `Ngg1`
and `NodeRecord`). `tests/ngg1_explorer_test.rs` (the `NggExplorer` binding over
`Ngg1`) and `layout::csr_sim`'s tests (which use `crate::ngg1::worked_example`,
exported publicly by `ontomark-ngg1`) pass unchanged.

## The diff a WP-D maintainer applies

`src/ngg1.rs` becomes the whole file:

```rust
//! NGG1 binary graph-tier format. The implementation now lives in
//! [`ontomark-ngg1`](https://docs.rs/ontomark-ngg1); this module re-exports it
//! unchanged so the `vowl_wasm::ngg1` path and every public name keep working.

pub use ontomark_ngg1::*;
```

`Cargo.toml` (use a crates.io version once `ontomark-ngg1` is published; the
proof used a path dependency):

```diff
@@ -47,6 +47,7 @@ nalgebra = "0.32"
 # was 68% of the 0.1.0 bundle's code section.
 regex = { version = "1.10", optional = true }
 rayon = { version = "1", optional = true }
+ontomark-ngg1 = { path = "/home/devuser/workspace/ontomark-wp-c/crates/ontomark-ngg1", optional = true }
 
 [dev-dependencies]
 wasm-bindgen-test = "0.3"
@@ -63,7 +64,7 @@ default = []
 # NGG1 binary tier format plus the zero-copy `NggExplorer` binding. Estate
 # specific (DreamLab's NarrativeGoldmine pipeline artefact), not something a
 # general ontology-visualisation consumer needs.
-ngg1 = []
+ngg1 = ["dep:ontomark-ngg1"]
 
 # Markdown-fronted ontology pages: the `ontology::markdown_parser` and the
 # filesystem `ontology::loader` built on it, plus the `parseMarkdownOntology`
```

`lib.rs` needs no change: its `#[cfg(feature = "ngg1")] pub mod ngg1;` stays.
The `NggExplorer` binding and `csr_sim` use only `Ngg1`, `Ngg1Error` and
`worked_example`, all re-exported.

## Differences a consumer can observe

- `Ngg1::parse` is stricter: it additionally rejects a `row_ptr` that does not
  start at 0, decreases or does not end at the edge count, an edge target that
  is not a node index, string offsets that are out of order or past the blob,
  and a section offset inside the header. vowl-wasm 0.1.2 read such buffers
  without error (and could panic later). Buffers produced by the NGG1 writer
  never trip these checks.
- Additions only: `Ngg1::try_parse` and `Ngg1Fault` (typed errors),
  `Ngg1::to_graph`, `Graph`, `GraphNode`, `Csr`, `build_csr`, `encode`,
  `EncodeError`, the short constants (`MAGIC`, `VERSION`, ...), `DOMAIN_NONE`,
  `FLAG_INDIVIDUAL`, `FLAG_META`, `impl Debug for Ngg1`, and `worked_example`
  as a public (was `pub(crate)`, test-only) function. `#[must_use]` is now on
  the pure accessors.
- `Ngg1Error(pub String)`, its `Display` ("NGG1: ..."), and every message text
  for the pre-existing failure modes are unchanged.
