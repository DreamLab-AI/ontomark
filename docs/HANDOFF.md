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
`ontomark-compile` 0.1.2 and `ontomark-ngg1` 0.1.0. The corrected Whelk
packages preserve the Rust MIT and original Scala BSD notices. Historical
published versions and release assets remain immutable.

Ontomark’s locked workspace declares Rust 1.89. Native tests, Clippy, strict rustdoc,
WASM builds and complete-history secret scanning pass in hosted core CI at
`7fd8ae2` (run `38175300206`). Compiler all/no-default-feature tests, opt-in
projection WASM compilation and locked registry package verification passed
separately. The complete committed corpus comparison reports zero differences
over 32 artefact lines and 12 command outputs. Its explicit compatibility
flag is a comparison mechanism; default public exports retain privacy tests.

## Releases and consumers

| Component | Release and status |
| --- | --- |
| CLI | GitHub `v0.1.0-rc.1`, clean source `21cd9ff`; Linux x86-64 binary verified on Debian 12 without networking. |
| Plugin engine | Same prerelease, SHA-256 `f810962b1b71e28653737f2cbbe3a06a5e1a8e6c059e452a2292b6af05b2023a`; embedded in the plugin. |
| Shared renderer | `ontomark-graph-v0.1.0`, immutable npm-format tarball, SHA-256 `de491e2d7d947503d3c49c140193c88a9d17cfec5fbb334166002049c8d56aa5`. |
| Obsidian | Separate `DreamLab-AI/ontomark-obsidian`; released installable Linux ZIP at tag `0.1.0`; real desktop and offline gates pass. |
| VOWL | crates.io `vowl-wasm` 0.2.0 and GitHub browser tarball; all 17 hosted checks pass. npm trusted-publisher configuration still prevents npm registry publication. |
| Narrative Goldmine | CLI pin already deployed at `366236b38`; shared renderer and WASM codec adoption passed 88 tests and built-browser acceptance, final deployment being monitored. |
| Loom | Main `eef1bec`, shared C3 contracts and registry compiler 0.1.1; 441 tests, Clippy, rustdoc and hosted Rust 1.98 checks passed. ADR-142 remains partial/inactive; no first served activation claimed. |

Released CLI ELF was built with Rust 1.96.1 in the Bookworm image and uses
the ordinary Linux loader. Its SHA-256 is
`7b15c2858f1715e561c0889dbaa50164e5a404ee7f8ed1d6cfd9da41daeb1d45`.
Do not replace it with the local Nix binary. Dependency licence inventories are
scoped to the exact release assets; see UPSTREAMS and THIRD-PARTY-NOTICES.

## Obsidian evidence

Installed Obsidian 1.14.4 on Linux passed real Blob-worker initialisation,
indexing, classification, query UI, guarded edit preview/application,
concurrent-edit refusal and unload/reload. The graph passed search, keyboard
selection/activation, zoom, neighbourhood focus, inferred-edge filtering,
note opening and view cleanup. Screenshots and receipts live in the plugin
repository. The separate network-isolated shim passed 40 checks; unit tests
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
are not declared complete. VisionFlow/Agentbox ADR reconciliation is part of
this release continuation. Security agent analysis was unavailable; functional
and dependency checks are not described as an independent security approval.

Original dirty operator worktrees, branches and private corpus content remain
preserved. The user has explicitly authorised estate pushes, repins and
experimental deployments; no new permission is required for those actions.

Build targets and temporary executables belong under the roomy
`/home/devuser/workspace/project` mount. `/tmp` is suitable for logs/data but
is mounted without execution permission. Stream archives to host Docker;
container bind paths do not address the edited container filesystem.
