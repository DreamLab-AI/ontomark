---
id: HANDOFF-WP-D
title: WP-D handoff (estate deprecation, repinning, docs)
date: 2026-10-11
as_of: "19:25 UTC"
owner: spec-estate (WP-D)
companion: docs/DEPRECATION.md
---

# WP-D handoff

State of every estate migration branch at the wrap-up checkpoint. `DEPRECATION.md` is the inventory and removal plan; this file is where each step's branch has got to. Step numbers are DEPRECATION §9.

## Rules still in force

- **visionGraph is never pushed by an agent.** Its `CLAUDE.md` forbids it (a push to `main` deploys narrativegoldmine.com). Its 9 branches stay local; the owner pushes or merges them.
- **Push timing (team-lead, option A).** A repo's branches are pushed only after its fix worker finishes and its Codex re-audit verdict is in. A worker not done by 19:43 UTC gets its branches pushed anyway under a commit whose subject starts `WIP prep (Codex FAIL, fix in progress):`.
- **Branches only.** Every push is an explicit `refs/heads/ontomark/X:refs/heads/ontomark/X` refspec without force. No estate `main` is ever pushed; team-lead pushes or merges after gates.
- **Worktrees.** Every branch has a worktree at `~/workspace/wt-ontomark/<repo>/<responsibility>`. The main checkouts carry the operator's uncommitted work and are never touched.
- **Evidence standard.** Each branch needs a red consumer test committed first, green after the removal, a mutation suite that stays red against partial or fake landings, and a Codex cross-vendor audit verdict before it is reported ready.

## Branch table

Codex column: round and verdict. "R1 FAIL, fixing" means the round-1 batch audit failed the branch's test as bypassable; the red result itself was correct, and a fix worker is hardening the test before round 2.

### VisionClaw (`project`, remote `dreamlab-github`)

All 10 were pushed during the wrap-up at the heads below, under team-lead's part-1 instruction, before the option-A timing rule arrived. Their fix commits will fast-forward them.

| Step | Branch | Head | Red test (`crates/visionclaw-domain/tests/`) | Codex | Remains |
|---|---|---|---|---|---|
| 1 | `ontomark/parse` | `cc29b82be` | `ontomark_step1_parse.rs` | R1 FAIL, fixing | Harden test; R2; removal must repin `crates/vault` in the same change (17 importing files) |
| 2 | `ontomark/iri` | `2067d1e9c` | `ontomark_step2_iri.rs` | R1 FAIL, fixing | Harden; R2; removal |
| 3 | `ontomark/taxonomy` | `4fe6eba13` | `ontomark_step3_taxonomy.rs` | R1 FAIL, fixing | Harden; R2; removal |
| 4 | `ontomark/reason` | `a493c1c26` | `ontomark_step4_reason.rs` | R1 FAIL, fixing | Harden; R2; closure-only path deleted only once EXP-01 passes natively (D5) |
| 5 | `ontomark/ngg1` | `c597e064c` | `ontomark_step5_ngg1.rs` | R1 FAIL, fixing | Harden; R2; repin to `ontomark-ngg1` 0.1.0 |
| 6 | `ontomark/rdf` | `4cdadff93` | `ontomark_step06_rdf.rs` | R1 FAIL, fixing | Harden; R2; removal |
| 7 | `ontomark/validate` | `da4bc4b99` | `ontomark_step07_validate.rs` | R1 FAIL, fixing | Harden; add the missed JSON-LD validator files and second OWL validator to scope; R2 |
| 8 | `ontomark/attribution` | `6078873d7` | `ontomark_step08_attribution.rs` | R1 FAIL, fixing | Harden; R2; removal |
| 9 | `ontomark/write` | `6cbe16a3b` | `ontomark_step09_write.rs` | R1 FAIL, fixing | Harden; R2; `apply.rs` survives as a thin wrapper (D4) |
| 10 | `ontomark/cli-publish` | `9039756ab` | `ontomark_step10_cli_publish.rs` | R1 FAIL, fixing | Harden; R2; thin AGPL `vault` adapter (propose, apply, panel only); delete `vault-core`; `ontology-publish.yml` on rc.1 |

### visionGraph (never pushed by agents)

| Step | Branch | Head | Red test | Codex | Remains |
|---|---|---|---|---|---|
| 10 | `ontomark/publish-cli` | `5d911301c` | `scripts/tests/test_publish_cli.py` (17 tests) | R1 PASS WITH FINDINGS; R2 FAIL; fixed; R3 running | R3 verdict; fill `ONTOMARK_RELEASE_TAG` and `ONTOMARK_SHA256` once team-lead cuts v0.1.0-rc.1. 16/17 green now, only the pin red. Team-lead's "step 1". |
| 1 | `ontomark/parse` | `0c0d8b340` | `modern/tests/migration/ontomark-step1-parse.test.ts` | R1 FAIL, fixing | `fmlib.py` can't go before the step-7 scripts that import it |
| 2 | `ontomark/iri` | `da5a9e4ec` | `…/ontomark-step2-iri.test.ts` | R1 FAIL, fixing | Harden; R2 |
| 3 | `ontomark/taxonomy` | `3eaca0ae2` | `…/ontomark-step3-taxonomy.test.ts` | R1 FAIL, fixing | Generated registry path is an assumption to confirm with spec-compile |
| 4 | `ontomark/reason` | `56611f79a` | `…/ontomark-step4-reason.test.ts` | R1 FAIL, fixing | Harden; R2 |
| 5 | `ontomark/ngg1` | `96859622e` | `…/ontomark-step5-ngg1.test.ts` | R1 FAIL, fixing | Explorer onto vowl-wasm 0.2 after vowl-wasm step 5 releases; delete `ngg1.ts`; load `ngg1-3n2e.bin` in a test |
| 6 | `ontomark/rdf` | `786f0d1ed` | `…/ontomark-step6-rdf.test.ts` | R1 FAIL, fixing | Harden; R2 |
| 7 | `ontomark/validate` | `f52395fae` | `…/ontomark-step7-validate.test.ts` | R1 FAIL, fixing | Port hygiene scripts to `ontomark lint\|migrate\|attribute\|assets` (D7), then delete |
| 8 | `ontomark/attribution` | `867b0f366` | `…/ontomark-step8-attribution.test.ts` | R1 FAIL, fixing | Harden; R2 |

`…` is `publishing-tools/WasmVOWL/modern/tests/migration`.

### vowl-wasm (remote `origin`)

| Step | Branch | Head | Red test | Codex | Remains |
|---|---|---|---|---|---|
| 5 | `ontomark/ngg1` | `20ae479` | `tests/ontomark_ngg1_migration.rs` | R1 FAIL, fixing | See "vowl-wasm step 5" below |
| 3 | `ontomark/taxonomy` | `fcbfd53` | `tests/ontomark_taxonomy_migration.rs` | R1 FAIL, fixing | Harden; R2 |

### Loom (remote `origin`)

| Step | Branch | Head | Red test (`crates/…/tests/`) | Codex | Remains |
|---|---|---|---|---|---|
| 2 | `ontomark/iri` | `3a2b094` | `loom-domain/…/ontomark_iri_migration.rs` | R1 FAIL, fixing | Four `urn:ngm:class:` minting sites onto one ontomark-model function (D9) |
| 6 | `ontomark/rdf` | `e03933a` | `loom-domain/…/ontomark_rdf_migration.rs` | R1 FAIL, fixing | Consume the versioned bundle contract (ADR-142) |
| 10 | `ontomark/cli-publish` | `0096d0b` | `loom-domain/…/ontomark_cli_publish_migration.rs` | R1 FAIL, fixing | LM-06 assertions go green with step 11 |
| 11 | `ontomark/reload-generation` | `eeea81b` | `loom-facade/…/reload_published_generation.rs` | not yet audited | See "Loom reload binary" below |

### agentbox (remote `origin`)

| Step | Branch | Head | Red test (`management-api/tests/`) | Codex | Remains |
|---|---|---|---|---|---|
| 1 | `ontomark/parse` | `b98297e5a` | `ontomark-parse-migration.test.js` | R1 FAIL, fixing | Harden; R2 |
| 2 | `ontomark/iri` | `d77c9d8be` | `ontomark-iri-migration.test.js` | R1 FAIL, fixing | Harden; R2 |
| 7 | `ontomark/validate` | `f9e8704f9` | `ontomark-validate-migration.test.js` | R1 FAIL, fixing | Harden; R2 |
| 9 | `ontomark/write` | `d104c22f8` | `ontomark-write-migration.test.js` | R1 FAIL, fixing | Harden; R2 |
| 10 | `ontomark/cli-publish` | `2433159e5` | `ontomark-cli-publish-migration.test.js` | R1 FAIL, fixing | Flake bakes the ontomark CLI and the thin `vault` adapter (D4); ADR-2136 to accepted |

### VisionFlow (remote `origin`)

| Step | Branch | Head | Content | Codex | Remains |
|---|---|---|---|---|---|
| 12 | `ontomark/docs` | `c0fdb59` | `terminology.md`, sovereign-corpus docs, ADR-2014/15/16 status notes, `docs/engineering/ontomark-diagram-migration.md` (55 topics: 48 change, 7 no change) | R1 FAIL, fixing | Topic restamps only after code lands; 6 stale citations listed in plan §5.1 |

### ontomark

| Branch | Head | Content |
|---|---|---|
| `wp-d-estate` | pushed | `docs/DEPRECATION.md`, `docs/HANDOFF-WP-D.md`, `scripts/estate-guard.sh` and its tests (baseline 876 residues, exit 1) |

## vowl-wasm step 5

- **Implemented** on `ontomark/ngg1`. `src/ngg1.rs` re-exports `ontomark-ngg1` (`23c1fcf`), the `markdown-ontology` feature and its parser, `Domain` enum and OWL validator are deleted (`79667d9`), and the dependency is the registry crate `ontomark-ngg1 = { version = "0.1.0", optional = true }` (`0aef092`). No patch override remains.
- **Breaking change recorded** (`20ae479`): malformed buffers now return an error instead of panicking.
- **Not landable yet.** Two red assertions are scope errors in the migration test itself, and the round-1 audit found the test bypassable. A fix worker is correcting both and measuring the real `wasm-pack` bundle on `main` and on the branch (`--features ngg1,interaction`; README baseline 232,052 bytes).
- **Remains:** the hardened test green, Codex round 2, then the release. The explorer's vowl-wasm 0.2 pin (visionGraph step 5) follows that release.

## Loom reload binary (step 11)

- **Branch** `ontomark/reload-generation`, built on Loom's `ontomark/cli-publish`.
- **Committed:** the red test `crates/loom-facade/tests/reload_published_generation.rs` (`eeea81b`).
- **In progress, uncommitted** in the worktree: `src/reload.rs`, `src/bin/reload_published_generation.rs` and edits to `mirror.rs`, `lib.rs` and `Cargo.toml`. A Sonnet worker is still running.
- **Still present:** `scripts/reload-published-generation.py`. It is deleted in the same branch only once the Rust test passes.
- **Remains:** the Rust test green with all 16 Python behaviours mapped; systemd unit `ExecStart` repointed; Python script and its test deleted; LM-06 assertions green; Codex audit.

## Outside WP-D, needed for landing

- **v0.1.0-rc.1** (team-lead, from `wp-c-compile`): asset `ontomark-x86_64-unknown-linux-gnu` plus its sha256. That unblocks `publish-cli`, VisionClaw step 10 and the publish half of every step's test.
- **Parity receipt.** `wp-c-compile` must be pushed so `d7863ff`, which the docs cite, resolves on GitHub. Codex round 2 got a 404 for it.
- **Later CLI verbs** (`find`, `retrieve`, `tree`, `edit`, `create`, `apply`) gate agentbox step 10 (DEPRECATION §2.1).
