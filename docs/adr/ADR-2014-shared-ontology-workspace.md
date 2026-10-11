---
id: ADR-2014
title: Own shared ontology capabilities in one Rust workspace
date: 2026-10-11
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
depends_on: [ADR-2013]
verified_commit: "not applicable — proposal without implementation"
verified_paths: []
owner: jjohare
review_trigger: workspace creation, package ownership, licence or consumer contract change
repo: visionflow
domain: BASELINE-visionflow.md
---

# ADR-2014 — Own shared ontology capabilities in one Rust workspace

## Context

The owner requests a general-purpose, local Obsidian application and greater reuse across the estate. Markdown with machine-readable headers remains authoritative. Existing boundaries may move, obsolete implementations may be removed, and beneficial schema changes may be bulk-migrated. This proposal follows a source investigation of VisionClaw, visionGraph, Agentbox, Loom and vowl-wasm; it does not certify the running estate.

## Proposed decision

Create one independently buildable ontology workspace as the source owner for shared semantics, Markdown processing, validation, reasoning orchestration, graph contracts and host bindings. Repository and new package names remain to be selected. Obsidian is a first-class application of this workspace; its needs do not cap the capabilities available to other consumers.

Use Rust libraries directly in native consumers and compile portable capabilities to WASM for Obsidian and browser consumers. TypeScript supplies host APIs and presentation. Native consumers need not execute WASM to share the same implementation. Dependency direction is host adapter → capability → semantic model, with no reverse dependency on Obsidian, VisionClaw, Loom, a database or an estate endpoint.

The proposed capability boundaries are:

| Boundary | Responsibility |
|---|---|
| Markdown document model | Parse headers and source spans; preserve unknown metadata and prose; lossless guarded patches |
| Ontology model | Typed entities, axioms, evidence, stable identities and explicit resolution outcomes |
| Validation and reasoning | Schema/policy validation separately from declared logical capabilities; shared Whelk adapter |
| Compilation | Local/public projection, canonical artefacts, generation identity and diagnostics |
| Graph protocol and layout | Versioned binary/typed graph contract; common codecs and the existing layout engine |
| Optional capabilities | Queries, Markdown-assisted proposals, import/export formats and provenance |
| Host adapters | CLI filesystem/Git, Obsidian vault, web artefacts, service APIs and governed promotion |

These are dependency boundaries, not a requirement to publish a crate for every row. Split packages when target support, dependency weight, licensing or independent consumers justify it. Keep native networking, signing, storage and threading out of the portable default dependency closure. Generate binding schemas/types and test serialisation across targets.

Keep mature upstream primitives as dependencies: Horned-OWL for OWL representation, Whelk for the supported reasoning fragment, and RDF/SPARQL components as needed. Reuse permissive patterns with attribution and tests. Do not fork a standards implementation merely to put its source in this repository. Existing vowl-wasm crate/npm identities can survive a source relocation; there must be one maintained source owner, not two patched copies.

Profiles contain vocabulary, layout, namespace projections, validation policy and publication policy. An ordinary vault works without estate configuration; the estate profile preserves or deliberately migrates its richer semantics. Optional Rust capabilities can exceed the Obsidian surface. Markdown and headers are the durable source; graph/query caches and published bundles are derived and rebuildable.

Migrate consumers through released crates, bindings and CLI artefacts with a compatibility manifest and shared conformance fixtures. Temporary compatibility adapters have named removal criteria. Completion includes deletion of superseded parsers, writers and semantic transformations after their consumers pass replacement tests. Preserve distinct logical entailments and presentation/physics constraints even when they share a model.

## Consequences and tensions

One source owner reduces implementation drift but introduces release coordination. Preserve package identities where useful; permit breaking internal APIs behind explicit migrations. A mixed licence workspace must preserve existing notices and per-package dependency boundaries: the current processor is AGPL, vowl-wasm is MIT, and Loom is MIT/Apache. Relocation does not settle redistribution choices or permit automatic relicensing. Review provenance before choosing the public package licence set.

WASM-first core design is appropriate; mandatory WASM execution for every native consumer is not. Large model runtimes and platform integration are adapters. No service or model is required for parsing, editing, validation, supported reasoning or graph rendering.

## Verification and delivery

The [investigation and delivery gates](../../../visionGraph/planning/ontology-workspace/README.md) record source findings, the existing 21-test renderer baseline and open target checks. The [semantic-model proposal](ADR-2015-typed-ontology-model-and-migration.md) and [Obsidian capability proposal](ADR-2016-local-markdown-ontology-application.md) define the remaining contracts. All three records are proposed, with no implementation or activation claim. Cross-family implementation audit remains a future gate; this investigation used same-family reviewers.

Acceptance requires clean-checkout native/WASM builds, supported-inference parity, installed offline plugin tests, consumer conformance, migration/rollback receipts, and removal of the inventoried duplicate implementations. Source completion, release publication and runtime activation are tracked separately.
