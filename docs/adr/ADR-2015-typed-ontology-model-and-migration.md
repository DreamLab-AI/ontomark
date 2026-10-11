---
id: ADR-2015
title: Separate ontology meaning from documents and presentation
date: 2026-10-11
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
depends_on: [ADR-2014]
verified_commit: "not applicable — proposal without implementation"
verified_paths: []
owner: jjohare
review_trigger: semantic model, vocabulary, inference profile or migration change
repo: visionflow
domain: BASELINE-visionflow.md
---

# ADR-2015 — Separate ontology meaning from documents and presentation

## Context

The existing model has useful invariants but also estate-specific identity transformations, fixed relation keys and overlapping parser/reasoner implementations. Inspection found that advertised Property pages fall through a Class/Individual-only model, and non-estate IRIs can be rewritten by the exporter. These findings require executable regressions before repair. Historical export parity is a migration baseline, not the permanent definition of ontology meaning.

## Proposed decision

Use a typed semantic representation that separates document identity, full entity IRI, publication URL, labels and aliases. Preserve existing entity IRIs on rename. New identities are minted once and stored in headers when a user explicitly creates or introduces an ontology entity, never merely by opening or indexing ordinary notes. A path or title change must not silently mint a replacement. Resolve references as resolved, ambiguous, unresolved or explicitly external, rather than choosing the first label match or inventing a target.

Represent classes, individuals and property kinds explicitly. Relations are vocabulary-driven and identified by full predicate IRI. Distinguish subclass axioms, individual membership, object/data assertions, annotations, SKOS/concept relationships and class expressions such as intersections and existential restrictions. Ordinary Markdown links remain document links unless the user explicitly maps them to ontology assertions. A graph edge is a view of these records, not the authoritative semantic representation.

Reuse standards-aware RDF terms and Horned-OWL structures where they fit; the workspace adds document/evidence ownership and supported-operation contracts rather than inventing a competing OWL language. Unknown constructs retain an explicit disposition: supported, preserved without reasoning, or rejected for the requested operation. Never silently downgrade an unsupported axiom into a generic edge.

Maintain distinct validation layers: document/schema checks, vocabulary/policy checks, logical reasoning and publication checks. Domain/range inference is not required-field validation. Subclass cycles are a configurable corpus policy concern, not automatically OWL inconsistency. Keep multiple inheritance. An optional shape-validation capability must identify its supported subset rather than claim complete SHACL support.

Expose a reasoner capability matrix and result metadata: engine/version, profile, execution mode, input generation, budget outcome and completeness limits. Preserve and test the current Whelk restriction-mode assumptions before changing them. Separate logical entailments from ordered ancestor lists, reverse-link materialisation and GPU constraints. Adding a new backend must not make weaker results indistinguishable from complete results.

Make local authoring and public export explicit operations. Public exclusion/redaction runs before inference and all derived public artefacts. Public errors/manifests must not reveal excluded content. Failed or cancelled builds retain the last valid generation and never present partial results as valid.

## Migration

First reproduce the identified gaps with fixtures: Property typing, arbitrary predicate round trips, arbitrary absolute IRIs, colliding labels and equal IRI tails across namespaces. Inventory the real corpus and affected consumers. Separate syntax-preserving migrations from changes to meaning or visibility.

Each migration is versioned, dry-runnable, idempotent and restartable. It emits an input digest, per-document disposition, identity map, guarded patch set, asserted/inferred triple deltas, visibility changes and newly unsatisfiable classes. Apply against the expected source digest; edits since planning force reconciliation. Keep originals or a recoverable Git snapshot and verify restoration. Unknown metadata, comments and prose survive unless an explicit migration owns them.

Bulk migration is supported; uncontrolled reinterpretation is not. Reclassifying every class as a SKOS concept, treating a source reference as owl:sameAs, or changing association links into existential restrictions requires a demonstrated semantic benefit and reviewed deltas. Vocabulary/profile and artefact versions advance together where meaning changes. V1 output adapters remain only for named consumers with a removal milestone.

## Consequences and verification

The typed model costs more design work than copying the current pairwise graph, but prevents information loss across consumers. Existing IRIs should normally remain stable even if the header schema and internal APIs change. The current EL implementation remains the first candidate; neither a wholesale reasoner replacement nor a wider OWL profile is justified by age alone.

Standards anchors: [OWL profiles](https://www.w3.org/TR/owl2-profiles/), [SKOS relationships](https://www.w3.org/TR/skos-reference/), and [SHACL](https://www.w3.org/TR/shacl/). SKOS broader links and OWL subclass assertions are not interchangeable. The [evidence and gates](../../../visionGraph/planning/ontology-workspace/README.md) distinguish static findings from executed checks. No corpus migration has been run under this proposal.
