# ADR-OM-013 verification — 15 November 2026

Implementation: `59e30991886d3d6e808a3ad99281a8a64401d28a`.
Independent digest vectors: `8e10cf1`.
Toolchain: rustc 1.99.1, native x86_64 Linux and wasm32-unknown-unknown;
wasm-bindgen 0.2.127, Node 22.23.3.

| Check | Result |
| --- | --- |
| `RUST_TEST_THREADS=4 CARGO_BUILD_JOBS=8 cargo test --offline --workspace` | 1,154 passed; 8 existing diagnostic tests ignored |
| `cargo test --offline -p ontomark-compile --all-features` | 705 passed; 7 existing diagnostics ignored |
| `cargo test --offline -p ontomark-compile --no-default-features` | 600 passed; 2 existing diagnostics ignored |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | Passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --offline --workspace --no-deps` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `node --test packages/graph/test/*.test.mjs` | 28 passed; 1 existing skip |
| wasm32 build, matching wasm-bindgen Node wrapper, `node scripts/test-review-binding-wasm.cjs` | Five native/WASM vectors, current/stale controls and invalid scope passed |
| `cargo publish --locked` package verification | Parser 0.1.4, ingest 0.2.2, compiler/profile/CLI 0.9.0 published |

The digest vectors were encoded independently with typed byte framing and Python's
standard-library SHA-256. They are fixed constants, not outputs recomputed by a
second call to the Rust encoder. CLI tests build actual public artefacts, check
explicit source stamps and compiler identity, and exclude a private-page canary.
Existing public/private and machine-scope controls also pass.

The original legacy golden files remain unchanged. Compatibility comparisons allow
only the named new provenance fields and v3 paths, and continue checking every
legacy payload field. Separate export tests verify the added metadata. The WASM
transcript golden changed only its engine version strings for the release.

An initial high-concurrency documentation-test run stopped making progress alongside
other Cargo processes. It was terminated and rerun with four test threads and eight
build jobs; the complete workspace run above exited successfully. Earlier failures
found and fixed lost float precision, malformed-scope protection and expected legacy
fixture differences. Raw logs are retained in the workspace consolidation archive
as `provenance-*.log`.

Consumers use the registry packages, not temporary local Cargo patches. VisionFlow
ADR-2024 records their revisions, tests and release adoption. Runtime activation is
separate from source and package publication; this receipt does not claim a running
service has reloaded or an Agentbox image has been rebuilt.
