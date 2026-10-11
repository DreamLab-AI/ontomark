# Hand-off: resumed ontomark sprint

Read [the branch census](BRANCH-RECOVERY.md), [RUNLOG](RUNLOG.md),
[ADR-OM-001](adr/ADR-OM-001-sprint-decisions.md) and
[the removal plan](DEPRECATION.md). The older WP-D tables describe the original
interruption; the branch census records what was recovered and what can land.

## Integrated core

Local `main` was fast-forwarded from `506e401` to `21cd9ff` after the recovered
WP-B/C/D/E merges. WP-A remains the canonical parser/model. All original
ontomark sprint refs and detached worktree tips are ancestors of this code;
there is no omitted committed core slice.

| Slice | Current state |
|---|---|
| WP-A model/parse | Original audited slice retained; later branch copies do not overwrite it. |
| WP-B reasoner | Audited facade merged. Unsupported assertion/role constructs are refused; dropped existentials make classification incomplete. Partial EL capability is explicit. |
| WP-C compiler/CLI | Public projection, alias/link privacy and Turtle writer corrections merged; JSON-LD/SHACL conformance work recovered. Legacy corpus parity passes. Shared reasoner adoption is pending. |
| WP-D estate | Census, removal plan and guard merged. Estate migrations remain open. |
| WP-E engine/plugin | Offline plugin merged, capability metadata corrected, JSON ordering stable with combined workspace features. Desktop Obsidian acceptance remains open. |

`ontomark-ngg1` 0.1.0 was already published from `291e0de`, with tag
`ontomark-ngg1-v0.1.0`. The CLI prerelease has not yet been published.

## Verified checks

`just check`: 746 tests passed, 7 ignored; fmt and clippy pass. Reasoner WASM:
47 tests passed. Plugin: 17 unit tests, 4 WASM/harness tests, bundle checks and
40 offline Docker checks passed. The Docker plugin harness uses an Obsidian
runtime shim. Legacy fixture parity and full committed corpus parity pass;
full corpus receipt reports zero differences over 32 artefact lines and
12 verb outputs. Current logs are in [evidence/resume](evidence/resume/).

Default-redaction corpus counts, from committed visionGraph `b5e36abd1`:
258 knowledge publication pages changed, 301 frontmatter entries removed,
5 keys removed and 50 body links rewritten; 63 working publication pages
changed with 195 body links rewritten; 32 mirror pages changed with 50 links.
Public example titles: 1Hive, 1inch, 2D LiDAR. Private page bodies and target
names are not included in the evidence bundle.

## CLI release

A clean source archive of `21cd9ff` was built using `rust:1.96-bookworm`.
The resulting `ontomark-x86_64-unknown-linux-gnu` uses the ordinary Linux ELF
loader; the local Nix release binary must not be uploaded. The portable asset
runs and builds the synthetic fixture on Debian 12 with networking disabled.
Its SHA-256 is
`7b15c2858f1715e561c0889dbaa50164e5a404ee7f8ed1d6cfd9da41daeb1d45`.

Parity of that exact asset also passes: zero differences over 32 artefact lines
and 12 verb outputs. Remaining release work: final receipts and the prerelease
tag/upload. Then fill visionGraph `ontomark/publish-cli` release tag
and checksum, run its 17 contract tests, and prepare the owner's push. Agents
must not push visionGraph: its main push deploys the public site.

## Estate branch work

Thirty-one estate migration branches were checked. Twenty-four dirty test/audit
preparations were preserved as `resume/prep-*` commits without changing their
original worktrees or indexes. WP-E historical evidence was preserved separately.
See the census for every recovery commit and landing blocker. Preparation tests
remain deliberately red until their consumer implementations are replaced.

VOWL NGG1 implementation `da21468` passes its functional gates and actual WASM
smoke/bundle tests. Its security review was unavailable, not passed. Taxonomy is
still preparation. Loom's Rust reloader is reconciled with its hardened CLI
parent on `resume/reload-integration` (`739f71a`). Marker, receipt and
concurrent-publication regressions are fixed; 61 integration tests plus one unit
test pass with fmt/clippy. Parent publishing contracts still have five failures,
and the existing build-info test reports unknown Loom revision in worktrees. VisionFlow main has diverged from origin/main and needs
reconciliation before its diagram migration plan can land.

The guard still reports the original 876 residues. Estate completion requires
that count to reach zero with each consumer's behavioural gates passing.

## Next implementation work

After the CLI prerelease, replace compiler legacy classification with the shared
reasoner, add the typed/in-memory projection API, declare the 690 SKOS-only
classes as `owl:Class`, and implement the remaining CLI commands. Follow the
consumer removal order in DEPRECATION section 9; repin importers in the same
change as removing their old library.

Use `CARGO_TARGET_DIR` and `TMPDIR` under the roomy
`/home/devuser/workspace/project/.git/` mount. The workspace volume is nearly full;
`/tmp` accepts logs/data but is mounted without execution permission. Stream
source archives into Docker rather than binding container paths on the host.
