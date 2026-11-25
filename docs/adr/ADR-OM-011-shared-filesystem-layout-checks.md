---
id: ADR-OM-011
title: Shared filesystem layout checks precede role selection
date: 2026-11-01
decision_status: accepted
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: 32b3aacdd9531d06c8c7b256828601364dd34034
verified_paths: [crates/ontomark-compile/src/profile/layout.rs, crates/ontomark-compile/src/profile.rs, crates/ontomark-compile/src/input/page.rs, crates/ontomark-compile/src/input/vocabulary.rs, crates/ontomark-compile/src/build/mod.rs, crates/ontomark-compile/src/migrate/plan.rs, crates/ontomark-cli/src/main.rs, crates/ontomark-cli/src/migrate_cmd.rs, crates/ontomark-cli/tests/layout_contract.rs, crates/ontomark-compile/src/check/bodies.rs, crates/ontomark-compile/src/check/repair.rs, crates/ontomark-compile/src/migrate/apply.rs, crates/ontomark-compile/src/migrate/journal.rs, crates/ontomark-compile/tests/migrate.rs, crates/ontomark-compile/src/migrate/m0002/mod.rs]
owner: jjohare
review_trigger: a change to root discovery, role placement, exclusions or a consumer adopting these checks
repo: ontomark
evidence_class: source+test
acceptance_boundary: shared owner checks and package gates; no runtime deployment or complete producer migration
amends: [ADR-OM-007, ADR-OM-008]
---

# ADR-OM-011 — Shared filesystem layout checks precede role selection

## Context

The estate audit reproduced validation/build disagreement when a custom knowledge
folder ended in the working folder's name. Aliased or nested roots could load one
page into both roles; an empty exclusion list made journals graph pages. Relative
repository discovery missed ancestors, and migration planning silently accepted an
absent selected role. VisionClaw needs the same checks while its producer migration
retains the parity boundary in ADR-OM-005.

## Decision

- `ontomark-compile::profile::resolve_roots` owns repository-relative root checks.
  Canonical paths must stay inside the repository and must not equal or contain
  another root. Absolute paths and parent components are refused. Existing
  symlinks are resolved; optional missing roots use their nearest existing ancestor.
- `Layout::validate` applies those rules to declared roles. CLI dispatch, builds
  and migration planning check the layout before work. `kind_of_at` matches the
  complete resolved root. The old lexical `kind_of` remains for source compatibility.
  Selecting an absent role still returns `MissingVault`; the CLI refuses it with 2.
- Furniture exclusions are additive: profile entries cannot reclassify templates,
  bases, assets or journals as graph pages. Journals retain their separate tree.
- Separately selected journal roots and repair scope roots cannot be symlinks.
  Migration apply/rollback recheck target ancestors and journal/log/originals
  containment before access; a directory moved outside the repository after
  planning becomes a conflict. This extends the same owner path checks.
- Rollback refuses parent-component migration names before journal access, matching
  the existing digest guard.
- `Vocabulary::discover_path` shares canonical ancestor discovery independently
  of vocabulary parsing. Consumers use the published crate with default features
  disabled, rather than copying the checks or importing a repository path.

## Consequences

Valid disjoint estate layouts keep their role names, generation and output paths.
Previously accepted overlapping/escaping layouts are now refused. A flat `.`
knowledge root therefore requires no working root. Missing optional roots remain
permitted configuration, preserving consumers that mount only one corpus half.
These are configuration checks, not a race-safe filesystem sandbox.

This change does not expand `Role.required_keys` runtime enforcement beyond the
existing knowledge `type`/`status` rules. The broader list also feeds schema output;
full enforcement requires a separate policy and corpus migration. Neither the
producer migration nor publication/certificate findings are closed by these checks.

## Verification

Permanent CLI regressions in `crates/ontomark-cli/tests/layout_contract.rs` cover
role suffixes, aliases, nested roots, relative discovery, furniture, parent escapes,
root symlinks and missing migration roles. `profile/layout.rs` tests the public
API, including missing ancestors and non-directory repositories. Compiler feature
legs include defaults, all features and no defaults. The [verification receipt](../evidence/2026-11-01-layout-checks.md) records the
executed commands and acceptance boundary.

## Journal-root follow-up — 2026-11-01

A fresh security review after the first patch exposed a second root boundary:
`journals/` is walked independently, so child-link settings did not protect it.
Six failing probes reproduced loader/planner traversal, body-repair writes,
apply/rollback writes through replaced parents and redirected migration logs.
The repair rejects linked journal/repair roots and reuses `resolve_roots` for
migration targets and bookkeeping paths. Two further tests cover fence repair
and an existing log-file symlink. See the updated verification receipt.

The same review found that the legacy `FrontmatterGrammar.public_gate` field is
not consumed by the hard-coded `public` visibility check. This remains an open
policy finding alongside generic required-key enforcement; this filesystem
patch does not imply either has been implemented.

## Re-verification — 2026-11-04

Re-verified at `ef485105e175f17d2697a9cda0abeb4997e097f8` after ADR-OM-012 touched two governed paths:
`crates/ontomark-cli/src/main.rs` gained the `ground` subcommand's dispatch line,
and `crates/ontomark-compile/src/build/mod.rs` emits `context/v2.jsonld`. Neither
changes root discovery, role placement or exclusions. `layout_contract` and the
`migrate` integration tests pass unchanged.

## Re-verification — 2026-11-15

The new review-binding CLI dispatch and v3 build outputs preserve filesystem
root discovery and role separation. The layout, migration and machine-scope
CLI controls pass with the ADR-2024 implementation.

## Re-verification — 2026-11-25

Re-verified at `32b3aacdd9531d06c8c7b256828601364dd34034` (release/v3, 0.10.0)
against `git diff 59e3099..32b3aac` over the governed paths. The release
(ADR-OM-014) touched six of them. The diff leaves root discovery, role placement
and exclusions as they were. The Decision holds, with one qualification on
m0002, given below.

- `crates/ontomark-cli/src/main.rs` gains the `memory` subcommand and
  `V3Context`. Under vocabulary 3, `validate`, `gate`, `create` and `apply`
  also load the role that was not selected (`main.rs:609-627`), through the same
  `load`. A load failure of that role is dropped (`.ok()`), not reported: the
  rules that need the sibling vault then see none. The layout is still checked
  at dispatch, before any of this. `edit`, `create` and `apply` now make their
  final write through `ontomark-write`'s `FsStore` (`memory_cmd.rs:395`,
  `:422`, `:446`). `FsStore` refuses a symlink anywhere below the vault root and
  a resolved parent outside it (`crates/ontomark-write/src/fs.rs:1-29`). That
  is stricter than 0.9.0's `edit`, which followed symlinks.
- `crates/ontomark-compile/src/build/mod.rs` reads the roster and writes
  `data/history.ttl` and `context/v4.jsonld` under vocabulary 3 only
  (`build/mod.rs:673-677`, `:936-948`, `:1019-1025`). No path resolution
  changed.
- `crates/ontomark-compile/src/input/vocabulary.rs` gains the version switch and
  the v3 working-type rules. `Vocabulary::discover_path` is unchanged.
- `crates/ontomark-compile/src/migrate/plan.rs` and `apply.rs` gain
  `Migration::prepare`, `extra_paths`, the classified diff, whole-tree
  validation of the result and an external-input digest. `apply` runs
  `checked_path` and the symlink refusal on every plan entry, so the
  non-page files m0002 rewrites (`ontology/vocabulary.yaml`,
  `ontology/attribution.yaml`, templates) get the same target checks as pages
  (`apply.rs:174-200`). `tests/migrate.rs` changes only to list both shipped
  migrations.

Qualification (true of the code at this revision, recorded rather than fixed):

- `plan` calls `migration.prepare(repo_root)` *before* `Layout::validate`
  (`plan.rs:498-502`). For m0002 this reads the roster, the ledgers under
  `working/podcast-evidence/`, the templates, `transcripts/` and the
  supersessions input. It reads them at fixed repository-relative paths, not
  through the profile's roles, using plain `std::fs`, which follows symlinks
  (`m0002/mod.rs:85-300`). These are reads only. Page enumeration and every
  write still follow the layout check and the apply-time checks. The Decision's
  "migration planning check[s] the layout before work" therefore holds for
  pages and writes, but not for m0002's context read.
- `plan` reads each `extra_paths` file with `is_file()` and `fs::read`, so a
  symlinked vocabulary or roster is read through. `apply` then refuses to
  write it, as a conflict.
- `ONTOMARK_M0002_SUPERSESSIONS` must canonicalise to a file outside the
  repository, or the plan errors (`m0002/mod.rs:234-258`). Its SHA-256 is bound
  into the plan, and `apply` refuses a different or missing input
  (`apply.rs:140-146`).

`m0002/mod.rs` is added to `verified_paths` because the qualification depends
on it. At this revision `layout_contract`, `migrate`, `m0002` and
`m0002_external_input` pass (see the ADR-OM-014 verification section for the
run).
