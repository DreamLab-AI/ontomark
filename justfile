# Repo-local cargo home: the shared agentbox CARGO_HOME has root-owned sources.
export CARGO_HOME := justfile_directory() + "/.cargo-home"

default: check

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

wasm:
    cargo build -p ontomark-wasm --target wasm32-unknown-unknown --release

conformance:
    cargo test -p ontomark-compile --test conformance

doc:
    cargo doc --workspace --no-deps

# EXP-03: lossless round-trip over every public page of a vault (read-only).
exp03 corpus="../visionGraph/knowledge/pages":
    ONTOMARK_CORPUS={{absolute_path(corpus)}} cargo test --release -p ontomark-parse --test exp03_corpus -- --nocapture
# EXP-01/EXP-06 on wasm32 under Node; the runner (wasm-bindgen-test-runner
# 0.2.127) must match the locked wasm-bindgen.
wasm-test *ARGS:
    CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test -p whelk-el -p ontomark-reason --target wasm32-unknown-unknown {{ARGS}}

# EXP-01 native; then compare the `EXP-01 <name> sha256=` lines with `just wasm-test -- --nocapture`.
exp01:
    cargo test -p ontomark-reason --test exp01 -- --nocapture
