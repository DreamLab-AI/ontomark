# Hand-off: ontomark sprint, checkpoint 2026-10-11 evening

Read this first, then `docs/RUNLOG.md` (the append-only ledger), then
`docs/adr/ADR-OM-001-sprint-decisions.md` (21 decisions) and
`docs/DEPRECATION.md` on branch `wp-d-estate` (the estate removal plan).

## State of main

`main` holds the scaffold, the merged WP-A slice (`ontomark-parse`,
`ontomark-model`), and the documentation. WP-A is closed: 9,244 of 9,244
public pages round-trip byte-identically, two Codex cross-vendor audits ran,
six defects were fixed test-first, no gates pending.

`ontomark-ngg1` 0.1.0 is published on crates.io from `291e0de`, tagged
`ontomark-ngg1-v0.1.0`. It is the single owner of the NGG1 format.

## Slices

| Slice | Branch | State at hand-off |
|---|---|---|
| WP-A model and parse | merged to main | closed |
| WP-B reasoner | `wp-b-reason` | never reported; see `docs/api/WP-B.md` if present |
| WP-C compile and CLI | `wp-c-compile` | parity proven at `291e0de`; checkpoint `d5c1188` unverified; rc.1 pending |
| WP-D estate | `wp-d-estate` | census done; guard baseline 876; 29 prep branches with red tests |
| WP-E plugin | `wp-e-plugin` | never reported; see `docs/api/WP-E.md` if present |

## WP-C: finishing rc.1 (about 90 minutes, in this order)

1. Take the redaction count with three public example titles from the
   worker; record in RUNLOG.
2. `cargo fmt`, commit `Cargo.lock`.
3. Full parity with `--no-redact-private-links` on fixture and corpus:
   must be 0 differences over 32 artefact lines.
4. Release build from a clean worktree: `cargo build --release -p ontomark-cli`.
5. Codex audit of the parity claim; verdict into `docs/api/WP-C.md`.
6. Tag `v0.1.0-rc.1`, upload asset `ontomark-x86_64-unknown-linux-gnu`
   with its sha256.

After rc.1: rebase onto main (WP-A), switch compile to `ontomark-reason`,
divergence 3 (declare the 690 `skos:Concept`-only classes as `owl:Class`),
writer `shorten` fixes, the in-memory projection API for WP-E, then
`find`, `retrieve`, `tree`.

## WP-D: landing Phase 2

Each `ontomark/<responsibility>` branch in each estate repo has a red
consumer test. Round-one Codex audits failed all 29 (the tests checked
spelling, not behaviour); fix workers are rewriting them to parse manifests,
strip comments, never self-scan, and carry a mutation suite. The visionGraph
branches live only in the worktrees under `~/workspace/wt-ontomark/`; the
repo rule forbids agents pushing visionGraph, so the owner pushes those.
VisionClaw branches push to remote `dreamlab-github`. See
`docs/HANDOFF-WP-D.md` on `wp-d-estate` for the per-branch table. Order is `docs/DEPRECATION.md` section 9. First to land:
visionGraph `ontomark/publish-cli` (`ea812a287`): fill
`ONTOMARK_RELEASE_TAG` and `ONTOMARK_SHA256` from rc.1, run its contract
test (11 of 11), push visionGraph, watch the deploy. Done when
`scripts/estate-guard.sh` exits 0 across all repos.

## Lessons for the next queen

- `SendMessage` bodies over about 1,300 characters truncate; send in parts.
- Check `git branch --show-current` before every commit; the shared
  checkout can be switched by a teammate.
- Codex's sandbox (`bwrap`) cannot start in this container; run audits
  unsandboxed on exported copies, never on worktrees.
- Run a slice's own gate yourself in a clean worktree before merging.
- `Write` of a long file was silently truncated once; verify with `wc -l`
  after writing anything over about 100 lines.
- Count declarations by parsing, never by grepping lines.
