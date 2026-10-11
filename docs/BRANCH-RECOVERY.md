# Branch recovery, 2026-10-11

Checked 84 pre-existing local branch heads across eight repositories, their
remote refs after fetching without pruning, detached ontomark worktree tips,
worktree changes and stash summaries. Of these heads, 37 belong to this sprint.
Unrelated feature branches and private corpus edits were left intact.

## Merge decisions

| Repository | Sprint branches | Decision |
|---|---:|---|
| ontomark | 6 | Both WP-A branches and WP-B/C/D/E are ancestors of integrated code `21cd9ff`. Local `main` was fast-forwarded to it after checks. |
| VisionClaw (`project`) | 10 | Migration tests/docs; production replacements absent. Preserve audited drafts; keep out of main until migrations pass. |
| visionGraph | 9 | Eight test preparations; `publish-cli` has the workflow replacement but release pins remain empty. No agent pushes. |
| vowl-wasm | 2 | NGG1 implementation `da21468` passes functional gates; security review unavailable. Taxonomy is still red preparation. |
| Loom | 4 | IRI/RDF/CLI are preparation contracts. Rust reloader and newer CLI parent reconciled on `resume/reload-integration` (`739f71a`); reproduced defects fixed. Parent migrations remain red. |
| agentbox | 5 | Test preparation only; legacy `vaultSrc` still pinned. Preserve audited drafts. |
| VisionFlow | 1 | Documentation and 55-topic diagram migration plan; no diagram edits. Local main has 5 unique commits and origin/main has 8; reconcile before landing. |
| knowledgeGraph | 0 | No migration branch; old VOWL 0.1.1 explorer pin awaits upstream migration and deployment. |

Every original sprint branch starts from its repository's local main. Ontomark's
five detached worktree tips are also ancestors of `21cd9ff`; none holds omitted
committed implementation. Recovered WP-B/C dirty source changes are included,
with later audit corrections. WP-E's ten dirty historical evidence files were
preserved separately on `resume/wp-e-historical-evidence` (`22641cd0e3`): those
receipts describe `6f79f58`, not the current code.

## Preserved uncommitted work

These 24 new branches commit the exact edited files without changing the source
worktree or its index. Each file's Git blob hash was checked against the recovery
commit. Full SHAs and file lists are in [the receipt](evidence/resume/branch-recovery.json).
A recovery commit records unfinished work; it does not assert a passing gate.

| Repository | Source branch | Recovery branch | Commit | Files |
|---|---|---|---|---:|
| agentbox | `ontomark/cli-publish` | `resume/prep-cli-publish` | `177c2c959b` | 2 |
| agentbox | `ontomark/iri` | `resume/prep-iri` | `4082c989af` | 2 |
| agentbox | `ontomark/parse` | `resume/prep-parse` | `b864605d1e` | 2 |
| agentbox | `ontomark/validate` | `resume/prep-validate` | `cd7eede392` | 2 |
| agentbox | `ontomark/write` | `resume/prep-write` | `e919f34189` | 2 |
| loom | `ontomark/rdf` | `resume/prep-rdf` | `47563eb1f4` | 1 |
| project | `ontomark/attribution` | `resume/prep-attribution` | `3a743f3975` | 5 |
| project | `ontomark/cli-publish` | `resume/prep-cli-publish` | `8715fbf016` | 5 |
| project | `ontomark/iri` | `resume/prep-iri` | `603b76af9d` | 5 |
| project | `ontomark/ngg1` | `resume/prep-ngg1` | `4d3ec16be6` | 5 |
| project | `ontomark/parse` | `resume/prep-parse` | `6cf10e011a` | 5 |
| project | `ontomark/rdf` | `resume/prep-rdf` | `6f96634530` | 5 |
| project | `ontomark/reason` | `resume/prep-reason` | `53cbbf805f` | 5 |
| project | `ontomark/taxonomy` | `resume/prep-taxonomy` | `3f13c6818e` | 5 |
| project | `ontomark/validate` | `resume/prep-validate` | `32378f900c` | 5 |
| project | `ontomark/write` | `resume/prep-write` | `00e4334bcb` | 5 |
| visionGraph | `ontomark/attribution` | `resume/prep-attribution` | `174f565156` | 3 |
| visionGraph | `ontomark/iri` | `resume/prep-iri` | `2fefcb7fce` | 1 |
| visionGraph | `ontomark/ngg1` | `resume/prep-ngg1` | `3743087fa7` | 5 |
| visionGraph | `ontomark/parse` | `resume/prep-parse` | `4af94692d1` | 4 |
| visionGraph | `ontomark/rdf` | `resume/prep-rdf` | `cdc35b8e04` | 3 |
| visionGraph | `ontomark/reason` | `resume/prep-reason` | `236748f7cb` | 3 |
| visionGraph | `ontomark/taxonomy` | `resume/prep-taxonomy` | `0442fb88c6` | 3 |
| visionGraph | `ontomark/validate` | `resume/prep-validate` | `c47b94dfe8` | 4 |

The strengthened visionGraph IRI test was untracked in the parse worktree;
`resume/prep-parse` preserves it. Place it on the IRI migration deliberately.
Shared helpers are repeated in the original preparation trees; consolidation
must retain each responsibility's individual test changes.

## Checks and limits

Integrated ontomark: 746 native tests passed, 7 ignored, formatting and clippy
passed. Reasoner WASM tests passed (47). Plugin: 17 unit tests, 4 WASM/harness
tests and 40 checks in a Docker container with networking disabled passed. The
plugin acceptance harness uses an Obsidian shim; desktop GUI acceptance remains
open. Full legacy corpus parity: zero differences across 32 artefact lines and
12 command outputs, using the committed corpus snapshot and compatibility mode.

The Debian-built CLI runs and builds the synthetic fixture offline on Debian 12.
Full corpus parity of the exact portable asset also passed: zero differences
over 32 artefact lines and 12 verb outputs. VOWL's security
scanner was unavailable and its wrapper's PASS receipt is invalid; do not report
that review as passed. No release, deployment or remote main push occurred during
this branch recovery. No branches were deleted or rebased.

## Loom reconciliation result

`resume/reload-integration` (`739f71a`) merges `ontomark/cli-publish` with
`ontomark/reload-generation`, retaining the Rust runbook and removing the Python
CI hook. Red regressions proved publication during identity probes could be
acknowledged, C3 identity/types were too permissive, and predictable receipt temp
files could inherit public permissions or follow a symlink. Fixes use post-probe
disk checks, exact typed identities and fresh private `NamedTempFile` receipts.
An empty-ID legacy-reader disagreement is also corrected.

Final checks: 61 integration tests plus one receipt byte-parity unit test passed;
formatting, clippy with warnings denied and diff checks passed. Seven new
regressions were reproduced red before fixes. This branch is held from main:
LM-04/LM-09 publishing contracts still have five failures (two tests pass), and
the existing build-info library test reports an unknown Loom revision in linked
worktrees. No host build, service restart or timer activation occurred.
