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
