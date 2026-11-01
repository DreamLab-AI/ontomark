# Shared layout checks — verification, 2026-11-01

Compiler release: `ontomark-compile 0.6.2`; CLI: `ontomark 0.7.1`.
Other workspace crate versions remain unchanged.

| Executed gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --locked --offline --workspace` | 1,105 passed, 8 ignored |
| `cargo test --locked --offline -p ontomark-compile --all-features` | 685 passed, 7 ignored |
| `cargo test --locked --offline -p ontomark-compile --no-default-features` | 581 passed, 2 ignored |
| `RUSTDOCFLAGS="-D warnings" cargo doc --locked --offline --workspace --no-deps` | Pass |
| `cargo build --locked --offline -p ontomark-wasm -p ontomark-ingest --target wasm32-unknown-unknown --release` | Pass |
| `cargo package -p ontomark-compile --allow-dirty` | Pass, including compilation against registry dependencies |

The final test matrix used `RUST_TEST_THREADS=4 CARGO_BUILD_JOBS=8` after an
unbounded parallel doctest run stalled in linkers and was stopped. No failing
assertion was ignored. Existing ignored cases retain their declared skips.

Eight actual CLI/loader regressions first reproduced layout defects, then passed
with shared validation, exact role resolution, canonical discovery and additive
furniture exclusions. Four public-API unit tests cover optional roots, invalid
components, repository type and linked ancestors. A security review's medium
rollback finding was separately reproduced by a failing CLI test and fixed; all
nine permanent layout/rollback regressions pass. The security gate met its HIGH
threshold (zero high/critical findings); this does not claim exhaustive security.

Generic required-key enforcement was tried and removed after it broke existing
publication behaviour. The profile list also supplies schema output; runtime
knowledge type/status enforcement remains unchanged. That mismatch is an open
policy/corpus migration, not one of the repaired layout defects.

These are synthetic fixture and build results. No real-corpus producer parity,
consumer deployment, concurrent-filesystem sandbox or complete privacy guarantee
is established. VisionClaw adoption has its own feature and acceptance record.
