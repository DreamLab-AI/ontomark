---
id: ADR-VG-007
title: Consume a shared ontology workspace through an explicit corpus profile
date: 2026-10-11
decision_status: proposed
implementation_status: none
activation_status: inactive
owner: jjohare
review_trigger: producer extraction, vocabulary migration, explorer replacement or publication change
depends_on: [visionflow/ADR-2014, visionflow/ADR-2015, visionflow/ADR-2016]
verified_commit:
verified_paths: []
---

# ADR-VG-007 — Consume a shared ontology workspace through an explicit corpus profile

## Context

visionGraph owns the authored Markdown corpus and currently embeds a React explorer while CI obtains its producer from VisionClaw. The owner requests a reusable ontology workspace, a full local Obsidian application and consolidation across the estate, with freedom to migrate the older model. This record proposes the corpus-side change; it does not activate a producer or alter pages.

## Proposed decision

Follow the [estate workspace decision](../../../VisionFlow/docs/adr/ADR-2014-shared-ontology-workspace.md), [semantic model](../../../VisionFlow/docs/adr/ADR-2015-typed-ontology-model-and-migration.md) and [Obsidian application](../../../VisionFlow/docs/adr/ADR-2016-local-markdown-ontology-application.md).

Keep authored content and corpus-specific vocabulary/policy in visionGraph. Move reusable compiler and renderer ownership to the shared workspace; publication and Obsidian consume its tested releases. Package the estate's domains, taxonomy IDs, namespace mappings, strict OKF checks and visibility rules as a versioned profile with no private content or embedded endpoint requirement.

Replace the workflow's VisionClaw checkout/build dependency with the released shared CLI and reproducible source fallback from the same release. Replace the embedded explorer implementation with the shared view package and web host adapter. Keep site chrome, Quartz and deployment configuration here. Update both paths with exact compatible versions and a release manifest; no floating source references.

Retain validation-before-publication, exclusion-before-inference, credential checks and output-substance checks. Replace corpus-size constants only with profile-specific expectations that still detect empty or wrong-scope builds. Generic plugin tests must allow a one-note vault. The public compiler must include both intended corpus roles while respecting each page's publication disposition.

Bulk schema migration is permitted by the design but starts with a census and reversible dry run. Preserve immutable resource IRIs and original text unless an explicitly described transformation changes them. Emit semantic and visibility deltas, not just file counts. Classify syntax repairs separately from changes to axioms. Require the new native compiler, WASM compiler and estate consumers to agree on the migrated fixtures before corpus-wide application.

## Consequences and verification

This removes independent embedded implementations and makes producer updates visible across consumers. The cost is coordinated release/profile migration and a period of explicitly bounded compatibility support. Existing publication and governance decisions remain in force until replacements are accepted and activated.

The [investigation](../../planning/ontology-workspace/README.md) lists confirmed source boundaries, static defect candidates and quality gates. Twenty-one existing renderer/protocol tests passed; no new compiler, plugin, migration or deployment has been exercised. The corpus has pre-existing working changes; extraction must reconcile them rather than assume HEAD represents all current work.
