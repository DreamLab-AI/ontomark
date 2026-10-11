# Repo-local cargo home: the shared agentbox CARGO_HOME has root-owned sources.
export CARGO_HOME := justfile_directory() + "/.cargo-home"

default: check

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

wasm:
    cargo build -p ontomark-wasm --target wasm32-unknown-unknown --release

# Offline: ontomark against the legacy golden for the in-repo fixture
# (artefact bytes, verb output bytes and exit codes).
conformance:
    cargo test -p ontomark-cli --test conformance

# Full corpus parity against the hashes in conformance/golden/corpus-visiongraph.
# Needs a visionGraph checkout at the golden's source commit (see its MANIFEST).
parity-corpus vg="../visionGraph": 
    cargo build -p ontomark-cli
    ./conformance/parity.sh target/debug/ontomark {{vg}} conformance/golden/corpus-visiongraph

doc:
    cargo doc --workspace --no-deps
