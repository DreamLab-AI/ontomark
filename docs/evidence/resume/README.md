# Resume evidence

`verification.json` records the integrated code commit, commands, counts and
SHA-256 hashes of the logs. `branch-recovery.json` records all 24 recovery commits
and exact file lists. `wp-e-recovery.json` identifies separately preserved old
receipts; those historical logs do not verify the resumed code.

`native-check.log` is the combined formatting, clippy and test gate.
`plugin-tests.log`, `plugin-wasm.log` and `plugin-offline.log` contain the executed
plugin gates; the latter is a Docker runtime shim with networking disabled.
`corpus-parity-nix.log` verifies the original local build; the release asset is
verified by `corpus-parity-portable.log`, `portable-build.log` and
`portable-runtime.log`. Both corpus runs use a clean committed snapshot and
compatibility mode with public-link redaction disabled. Privacy has its own
regression coverage in the native gate.

No private corpus bodies or published output trees are included. Security review
was not successfully executed; these receipts make no security PASS claim.

Loom evidence includes failing marker/probe, receipt and reader-dispatch
regressions; the final passing integration, receipt and clippy logs; and
`loom-parent-contracts-pending.log`, which is deliberately labelled as failing.
The parent migration and existing build-info failure prevent a full Loom gate
claim.
