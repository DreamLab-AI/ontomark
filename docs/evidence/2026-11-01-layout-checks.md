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

## Journal-root follow-up: compiler 0.6.3, CLI 0.7.2

The fresh review at `a55033c` found that separately walked journal roots still
followed symlinks, and migration apply/rollback checked the leaf but not a
redirected parent. Six new tests failed against that release. The repair uses a
shared journal/repair-root refusal and the existing `resolve_roots` API for
migration target and bookkeeping containment. Two additional regressions cover
fence repair and a linked existing log file. All eight pass in the full suites.

The same gate commands were executed for this patch: workspace **1,113 passed,
8 ignored**; all features **693 passed, 7 ignored**; no defaults **589 passed,
2 ignored**. Strict Clippy, rustfmt, warning-free workspace rustdoc, both release
WASM builds and compiler package verification pass. No source or fixture was
substituted for these runs. Stable redirected parents are checked; concurrent
replacement during an operation remains outside the guarantee.

The first release's raw security receipt was lost in a worktree scratch-archive
sequencing error. Its stdout and completed test logs survived. A fresh clean-tree
review at `a55033c` was retained before this follow-up; it met the HIGH threshold
and reported the two journal findings plus the existing ignored `public_gate`
configuration field. That last policy finding remains open. The incident and raw
receipts are retained with the VisionFlow audit evidence.
