#!/bin/sh
# Strict rustdoc gate for ontomark-compile. Lives in a script so the nightly
# evaluator's `sh -c` wrapper never has to re-quote the -D warnings flag.
set -eu
RUSTDOCFLAGS="-D warnings"
export RUSTDOCFLAGS
exec cargo doc --locked -p ontomark-compile --all-features --no-deps
