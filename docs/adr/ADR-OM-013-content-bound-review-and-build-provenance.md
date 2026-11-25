---
id: ADR-OM-013
title: Bind human review to source content and identify the bundle compiler separately
date: 2026-11-15
decision_status: accepted
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
amends: [ADR-OM-012]
verified_commit: 32b3aacdd9531d06c8c7b256828601364dd34034
verified_paths: [crates/ontomark-parse/src/certification.rs, crates/ontomark-compile/src/input/okf.rs, crates/ontomark-ingest/src/certification.rs]
owner: jjohare
review_trigger: Digest encoding changes, consumer trust interpretation changes, or a new provenance role is introduced
repo: ontomark
evidence_class: source+test
acceptance_boundary: Producer implemented and verified; parser 0.1.4, ingest 0.2.2 and compiler/profile/CLI 0.9.0 published; consumer rollout and runtime activation recorded separately
---

# ADR-OM-013 — Content-bound review and build provenance

VisionFlow ADR-2024 resolves the open freshness question in ADR-OM-012. A human
stamp with a content digest claims review of specific values. A historical stamp
without a digest cannot claim a current binding. Compiler identity, content origin,
logical derivation, human review and governance admission remain separate claims.

The portable parser owns SHA-256 v1 scope encoding and stamp assessment. Ingest,
compiler, CLI, WASM and consumer adapters use that contract. Key-scoped review covers
only the named values; whole-page review covers frontmatter except `verified` and
the exact body. Types, missing values and sequence order matter; map order does not.
Float spelling is preserved to avoid losing precision through reserialisation.

Current, stale and unbound stamps retain historical ingest protection. A malformed
scope cannot claim current review, but its identifiable legacy keys remain protected.
Build and ordinary edit operations never recertify. `review-binding` prepares a
binding without writing a stamp. A digest is not a signature or proof of review.

Generation markers add optional `built_by` compiler name/version. Missing legacy
metadata remains unknown. Explicit source attestations survive parsing and export,
subject to the existing public redaction boundary. Source review summaries do not
claim that redacted projection bytes match the source digest.

The v3 page API and context expose freshness separately from legacy protection.
Existing v1/v2 contexts remain unchanged. Scaffold records carry review summaries
and retain source attestations; absent legacy fields retain their meaning.

The wire and digest specification is [review-bindings.md](../api/review-bindings.md).
Tests cover native/WASM agreement, exact-source numeric precision, malformed scopes,
history, consumer round-trips, CLI exports and private canaries. Delivery evidence
is recorded in VisionFlow ADR-2024; implementation, release, adoption and deployment
are recorded separately.

Verification commands, revisions and results: [ADR-OM-013 receipt](../evidence/ADR-OM-013.md).

## Re-verification — 2026-11-25

Re-verified at `32b3aacdd9531d06c8c7b256828601364dd34034` (release/v3, 0.10.0)
against `git diff 59e3099..32b3aac` over the three governed paths. The record
still holds.

- `crates/ontomark-parse/src/certification.rs` moved the "is this `by` a human"
  test into a parameter. `assess` now calls `assess_with(doc, key,
  &is_human_prefixed)` (`:38`), and the predicate is the expression the loop
  used before. For a caller of `assess`, the SHA-256 v1 scope encoding, the
  scope rules and the current, stale and unbound classification are unchanged.
- `crates/ontomark-ingest/src/certification.rs` adds `Certification::of_with`.
  It keeps `of`'s freshness handling and only widens who counts as human
  (ADR-OM-012 re-verification, ADR-OM-014).
- `crates/ontomark-compile/src/input/okf.rs` adds the `did:nostr:` actor, `via`
  on stamps, and `rel`, `at`, `locator` and preserved extensions on `sources`.
  It also adds `Certification::from_verified_with` and `for_page_with`. The
  freshness fields (`current`, `stale`, `unbound`) are still copied from the
  parser's assessment. Source attestations survive parsing better than before:
  fields the type does not model are now written back.

`certification_freshness` and `review_binding_cli` pass at this revision (see
the ADR-OM-014 verification section for the run).
