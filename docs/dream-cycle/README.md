# Ontomark nightly experiments

Ontomark is nominated by its root `dream.config.json`. Agentbox's running
Dream Machine discovers nominated checkouts directly under its workspace; no
central repository registry or supervisor restart is required. It archives the
committed HEAD, so a local, uncommitted configuration does not provision an
experiment.

Nightly cycles run in the existing estate window, 01:00–05:00 UTC, with at most
five repositories per night. Selection favours repositories least recently
evaluated; a nomination over the cap is deferred. This configuration does not
change the window, cap or other
repositories' pause and standby controls. Keep only the canonical Ontomark
checkout active; park temporary nominated worktrees with `.dream-standby`.

Four rotating slots cover typed identity, public projection, supported
reasoning, and bundle/RDF contracts. Every slot requires workspace tests, compiler
tests with and without default features, compiler Clippy, and strict compiler
rustdoc. These commands emit named test cases or source diagnostics and exit
non-zero when their contracts fail. Existing privacy tests include both private
canaries and controls proving those canaries fire without redaction. The connected
node needs Cargo/Rust compatible with the workspace's declared MSRV; the engine
archives this self-contained workspace without sibling repositories. A missing
toolchain or failed build is an environment fault, not a validated finding.

These gates cover native Rust execution. Browser, Obsidian and WebAssembly
execution need dedicated tests; consumer runtime and private corpus changes
are handed to the owning repositories. Nightly reports use committed synthetic
or standards fixtures and must not include private page bodies. Releases,
publication, estate repins and deployment are outside the nightly patch scope.

`autoMerge` is false. Candidate patches are evaluated against their archived
baseline before a human reviews an accepted proposal. The engine creates
`LEDGER.md` here when it records a cycle; there is no fabricated initial verdict.

To park or revive this repository, use the Dream Machine's `/dream standby
ontomark` or `/dream revive ontomark` controls. `/dream status` reports eligibility,
the roster, and the last night's receipts. A nomination does not promise that a
cycle has run or that the model/provider and connected node are healthy.
