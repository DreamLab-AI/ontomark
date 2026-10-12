# Ontomark: release and estate hand-off

Ontomark is the public home of the parser, typed model, supported reasoner,
compiler, NGG1 codec and shared interactive graph. The Obsidian plugin has its
own public repository. Narrative Goldmine consumes the same graph package.
Read [ADR-OM-002](adr/ADR-OM-002-shared-graph-and-obsidian-boundary.md),
[upstream credits](UPSTREAMS.md), [branch recovery](BRANCH-RECOVERY.md) and
[remaining removals](DEPRECATION.md).

## Landed core

All recovered committed core sprint work is integrated. Compiler native
classification now delegates to the shared reasoner. Its opt-in typed
`model-projection` API takes a caller model/profile and explicit public or
authoring scope, with privacy omissions and unsupported-profile refusals.
It does not silently change the legacy estate CLI semantics.

Published crates include `whelk-el` 0.1.1, `ontomark-reason` 0.1.1,
`ontomark-compile` 0.1.3 and `ontomark-ngg1` 0.1.0. The corrected Whelk
packages preserve the Rust MIT and original Scala BSD notices. Historical
published versions and release assets remain immutable.

Ontomark’s locked workspace declares Rust 1.89. Native tests, Clippy, strict rustdoc,
WASM builds and complete-history secret scanning pass in hosted core CI at
`c29c2b2` (run `38179654820`). Compiler all/no-default-feature tests are explicit
hosted gates; opt-in projection WASM compilation and locked registry package
verification also passed. The complete committed corpus comparison reports zero differences
over 32 artefact lines and 12 command outputs. Its explicit compatibility
flag is a comparison mechanism; default public exports retain privacy tests.

## Assisted ingest (landed, unreleased)

`crates/ontomark-ingest` implements the first slice of
[ADR-OM-003](adr/ADR-OM-003-native-assisted-ingest.md): a pure `plan`/`accept`
contract with no I/O and no model call. `plan` returns the prompt contract as
data, heading-aware chunks, the profile's assertion keys and a response JSON
schema; `accept` grounds model-proposed names against the vault (exact, then
OntoCast's lexical tiers, strongest tier wins), checks literals against their
XSD datatype, requires a located evidence quote for every value, and returns one
digest-bound `Patch` that has already been applied once in memory to prove it.
Facts only, against the existing vocabulary; ontology growth, the critic pass and
numeric repair are deferred. 20 unit, 8 integration and 6 doc tests pass; strict
Clippy and rustdoc pass; the crate compiles for `wasm32-unknown-unknown` and CI
builds it for that target. It is not published, not exposed through
`ontomark-wasm`, and not consumed by the plugin; those are the next steps and
each needs its own gate (engine prerelease and `engine.json` repin, installed
Obsidian acceptance with EXP-11 unchanged when ingest is off). Methods adapted
from OntoCast and SPIRES are credited in [UPSTREAMS](UPSTREAMS.md) and the
crate's NOTICE.

## Releases and consumers

| Component | Release and status |
| --- | --- |
| CLI | GitHub `v0.1.0-rc.1`, clean source `21cd9ff`; Linux x86-64 binary verified on Debian 12 without networking. |
| Plugin engine | Same prerelease, SHA-256 `f810962b1b71e28653737f2cbbe3a06a5e1a8e6c059e452a2292b6af05b2023a`; embedded in the plugin. |
| Shared renderer | `ontomark-graph-v0.1.0`, immutable npm-format tarball, SHA-256 `de491e2d7d947503d3c49c140193c88a9d17cfec5fbb334166002049c8d56aa5`. |
| Obsidian | Separate `DreamLab-AI/ontomark-obsidian`; released installable Linux ZIP at tag `0.1.0`; real desktop and offline gates pass. |
| VOWL | crates.io `vowl-wasm` 0.2.0 and GitHub browser tarball; main `8c78a25` passes all 18 hosted jobs (run `38177385305`), including the reviewed release-notice guard. npm trusted-publisher configuration still prevents npm registry publication. |
| Narrative Goldmine | Runtime `9cf6cb8` uses the released CLI, shared renderer and WASM codec. Its 92 unit tests, six native notice-tool tests, 23 publisher contracts and built-browser acceptance pass. [Publisher run](https://github.com/jjohare/visionGraph/actions/runs/38179004619) and [Pages deployment](https://github.com/DreamLab-AI/knowledgeGraph/actions/runs/38180896856) succeeded; public graph/About routes and notices are verified. |
| Loom | Main `eef1bec`, shared C3 contracts and registry compiler 0.1.1; 441 tests, Clippy, rustdoc and hosted Rust 1.98 checks passed. ADR-142 remains partial/inactive; no first served activation claimed. |

Released CLI ELF was built with Rust 1.96.1 in the Bookworm image and uses
the ordinary Linux loader. Its SHA-256 is
`7b15c2858f1715e561c0889dbaa50164e5a404ee7f8ed1d6cfd9da41daeb1d45`.
Do not replace it with the local Nix binary. Dependency licence inventories are
scoped to the exact release assets; see UPSTREAMS and THIRD-PARTY-NOTICES.

The public [graph](https://narrativegoldmine.com/graph/) and
[About page](https://narrativegoldmine.com/about/) have canonical HTTP 200 routes.
All four attribution files are served: full VOWL and renderer notices plus
JavaScript notices and the 99-package runtime inventory. The live inventory's
SHA-256 matches the exact build, as recorded in the
[public HTTP receipt](evidence/resume/publisher-live-http.json).
Independent audits verify all 76 upstream repositories and 97 locked npm
source archives in that inventory; see the
[repository](evidence/resume/publisher-runtime-source-links.json) and
[archive](evidence/resume/publisher-npm-source-archives.json) receipts.
Fresh public browser acceptance renders 61 nodes and 350 edges, exposes 61
accessible node controls and supports neighbourhood focus. Cold interaction
settled in 2,729 ms including the isolation service-worker reload. The
publisher's docs-only activation receipt is `32c81a222`; the tested runtime
remains `9cf6cb8`. Its broader corpus-profile migration remains partial.

## Obsidian evidence

Installed Obsidian 1.14.4 on Linux passed real Blob-worker initialisation,
indexing, classification, query UI, guarded edit preview/application,
concurrent-edit refusal and unload/reload. The graph passed search, keyboard
selection/activation, zoom, neighbourhood focus, inferred-edge filtering,
note opening and view cleanup. [Three screenshots](https://github.com/DreamLab-AI/ontomark-obsidian#screenshots)
and acceptance receipts live in the plugin repository. The separate network-isolated shim passed 40 checks; unit tests
passed 17 and actual WASM harness tests passed four. Mobile remains unsupported.

## Nightly dreaming

[dream.config.json](../dream.config.json) nominates the canonical checkout in
the existing 01:00–05:00 UTC cycle. Four native contract slots require workspace
and compiler feature tests, Clippy and strict rustdoc. Automatic merging is
disabled. Discovery/admission and the baked engine's dispatch-free dry run were
verified; a real nightly cycle has not been claimed. Browser, Obsidian and WASM
execution need their dedicated harnesses. See [nightly docs](dream-cycle/README.md).

## Remaining estate work

Broader estate removals remain open: corpus-wide typed/profile migration,
other consumer replacements and the migration guard's original 876 residues
are not declared complete. VisionFlow/Agentbox ADR reconciliation is landed.
VisionFlow’s release roster
now includes the two public Ontomark repositories (main `7cb004d`); its count,
schema, seven governance suites and ADR gates pass. Agentbox’s ownership docs
landed at `5bca6b9a6`, with existing unrelated ADR staleness unchanged.
Security agent analysis was unavailable; functional
and dependency checks are not described as an independent security approval.

Original dirty operator worktrees, branches and private corpus content remain
preserved. The user has explicitly authorised estate pushes, repins and
experimental deployments; no new permission is required for those actions.

Build targets and temporary executables belong under the roomy
`/home/devuser/workspace/project` mount. `/tmp` is suitable for logs/data but
is mounted without execution permission. Stream archives to host Docker;
container bind paths do not address the edited container filesystem.

## Typed projection privacy correction

Compiler 0.1.3 corrects private literal filtering for Unicode case variants, using the model resolver’s comparison rule. Sharp-s and final-sigma canaries failed before the fix; authoring and unrelated-public-value controls pass afterwards. All 413 compiler unit tests and integration suites pass, including 10 typed projection cases; 36 doctests, strict Clippy/rustdoc and wasm32 compilation pass. An initial doctest link failure came from the full workspace temporary filesystem; rerunning those doctests with the roomy executable TMPDIR passed.

Compiler 0.1.3 passed locked registry verification and is published. Version 0.1.2 is yanked so new dependency resolution selects the corrected implementation. Existing pinned users should upgrade. Hosted implementation CI at 49c6325 passed; compiler all/no-default-feature suites are now explicit CI gates.

## Assisted ingest and the engine boundary (12 October 2026, unreleased)

`crates/ontomark-ingest` (ADR-OM-003) is in `main`: a pure plan/accept contract, three independent review passes applied, 43 tests, strict Clippy/rustdoc and wasm32 clean. The portable engine exposes it as `ingest_plan` and `ingest_accept`; the transcript fixture exercises both and the golden is re-blessed, so native/WASM parity now covers ingest. Wiring it exposed that the engine's released profile and `ontomark-model`'s `generic/1` were different grammars; each engine profile now also implements `ontomark_model::Profile` (the EXP-10 mapping) and `tests/profile_parity.rs` proves both sides index the same entities. `ontomark_model::Profile` gained one additive method (`identity_for`); no published crate's API was broken, so no version bump was forced. ADR-OM-004 records why OWL 2 EL is the local profile and where non-EL constructs are refused by name.

Not yet done, in order: engine prerelease and `engine.json` repin (ADR-OBS-003 gate), the plugin command, isolated-run and installed-Obsidian acceptance for ingest. The plugin harness already understands the `@plan:N` transcript substitution; its fixture copies are refreshed at the repin. Publishing `ontomark-ingest` and republishing `ontomark-model`/`ontomark-wasm` is a separate, operator-triggered step.

## Estate alignment (12 October 2026)

Owner decisions recorded in `docs/adr/ADR-OM-005` and `DEPRECATION.md` §13: VisionClaw is a consumer (force superset = explicit projection); VC-60 retired, gated on VisionClaw ADR-2106; `working/` stays in the GPU view via a private `--vault all` machine bundle from the same producer; generation identity `visionGraph@<sha>` separable from distribution, re-minting permitted; extraction of `vault`/`vault-core` verbs is a port under the existing grant. Estate ADR snapshots replaced by `docs/adr/ESTATE-ADRS.md`. Fleet edits landed the same day in VisionFlow (ADR-2014 + BASELINE), VisionClaw (ADR-2137), Agentbox (ADR-2136), Loom (ADR-141/142), visionGraph (VG-007). No code changed.
