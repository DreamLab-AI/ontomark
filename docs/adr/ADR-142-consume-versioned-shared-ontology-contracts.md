---
id: ADR-142
title: Consume versioned shared ontology contracts without moving authored authority
date: 2026-10-11
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit:
verified_paths: []
owner: jjohare
review_trigger: Before shared workspace extraction or any consumer dependency or generation migration
repo: loom
---

# ADR-142 — Consume versioned shared ontology contracts without moving authored authority

## Context

Loom consumes asserted/inferred Turtle, scaffold/prose indexes and generation markers produced elsewhere. It verifies an activated bundle and reports loaded content. A shared ontology workspace can align these contracts without moving authorship into Loom or making its graph store mandatory for authoring.

Estate proposal: [VisionFlow ADR-2014](../../../VisionFlow/docs/adr/ADR-2014-shared-ontology-workspace.md). Evidence: [workspace investigation](../../../visionGraph/planning/ontology-workspace/README.md). Proposed extension to [ADR-141](ADR-141-loom-consumes-the-vault-build.md) and [pipeline contract](ONTOLOGY-LOOM-PIPELINE.md); no accepted contract is superseded.

## Decision

This is a proposed extension. Existing accepted invariants remain in force until a separately reviewed implementation and migration establish the new baseline.

1. Consume versioned bundle contracts from the independent ontology workspace. Markdown and machine-readable headers remain the sole authored ontology authority. Oxigraph, lexical/vector indexes and loaded bundles remain rebuildable serving projections; Loom does not become an alternate ontology authoring database.
2. Prefer the existing artefact boundary initially. Shared contract crates may replace duplicated schema/identity logic only after licence and API review. This MIT/Apache workspace must not silently import the producer's AGPL implementation. No copy of ontology parsing or reasoning is introduced to avoid that boundary.
3. Preserve rich optional estate capabilities independently of the Obsidian plugin. Consumers negotiate or reject unsupported contract versions explicitly. Open-world truth, asserted/inferred basis, provenance and ontology version IRI survive projection; an empty response does not become a negative assertion.
4. Retain stage, digest verification and activation as separate operations. Report the loaded generation and ontology version IRI, not merely the latest files on disk. Coordinate producer release, reader compatibility and reload receipt; retain the prior bundle for rollback. Source commit alone is insufficient identity when vocabulary, policy or emitter versions change.
5. Test the same released fixture bundle across native, WASM and serving consumers. A compatible optimisation or richer native reasoner must label its semantic profile and completeness. Existing-vault proposal automation may use established external governance; OCR and document ingestion are out of scope.

## Consequences

Loom keeps its serving architecture and licence boundary while gaining an explicit producer compatibility contract. Shared types reduce drift only when they describe actual consumed bytes; wholesale dependency merger would increase coupling. Optional indexes remain valuable, but rebuilding them must never require reconstructing authored knowledge from a database.

## Verification

No implementation or activation is claimed. Source inspected at `37a320ce3f15787a1ee10fac0faae043ab1ac12e`: `crates/loom-graph-oxigraph/src/lib.rs:38` fixes the Turtle allowlist; `crates/loom-facade/src/bundle.rs:16` defines verification and loaded identity. Before adoption run graph allowlist/inferred-closure integration tests, `mirror_generation`, MCP ancestor/provenance tests and producer-fixture compatibility. Add vocabulary-only and emitter-only identity changes, unsupported-version rejection, mixed-bundle rejection and rollback/restart tests. A reload receipt must prove the intended generation is served; no deployment was exercised here.
