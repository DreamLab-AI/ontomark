# source this: repo-local cargo home (the shared agentbox CARGO_HOME has root-owned sources)
export CARGO_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/.cargo-home"
