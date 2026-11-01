---
id: ADR-OM-011
title: Shared filesystem layout checks precede role selection
date: 2026-11-01
decision_status: accepted
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: 395c18b3c8f037bd9fe883121b515506f4430d8d
verified_paths: [crates/ontomark-compile/src/profile/layout.rs, crates/ontomark-compile/src/profile.rs, crates/ontomark-compile/src/input/page.rs, crates/ontomark-compile/src/input/vocabulary.rs, crates/ontomark-compile/src/build/mod.rs, crates/ontomark-compile/src/migrate/plan.rs, crates/ontomark-cli/src/main.rs, crates/ontomark-cli/src/migrate_cmd.rs, crates/ontomark-cli/tests/layout_contract.rs, crates/ontomark-compile/src/check/bodies.rs, crates/ontomark-compile/src/check/repair.rs, crates/ontomark-compile/src/migrate/apply.rs, crates/ontomark-compile/src/migrate/journal.rs, crates/ontomark-compile/tests/migrate.rs]
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
