---
id: ADR-OM-002
title: "shared graph code and separate Obsidian distribution"
date: 2026-10-11
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: "33924da83e4b7db2c6bc5b19162f01288e583cf8"
verified_paths: []
owner: jjohare
review_trigger: "a change to what packages/graph owns, or to how the Obsidian plugin consumes the shared graph"
repo: ontomark
---

# ADR-OM-002: shared graph code and separate Obsidian distribution

- Implementation: shared package 0.1.0 and Obsidian 0.1.0 released; installed Obsidian and public publisher browser acceptance passed
- Related: [ADR-OM-001](ADR-OM-001-sprint-decisions.md), [estate removal plan](../DEPRECATION.md)

Status mapping 2026-10-31: the record states `Implementation: shared package 0.1.0 and Obsidian 0.1.0 released; installed Obsidian and public publisher browser acceptance passed`, which is not a checker value and does not use the word "complete"; `implementation_status: partial` is the conservative reading. It states no activation, so `inactive` is taken. Owner to set the real values (the released-and-accepted statement may justify `complete` / `live`).

Verification 2026-10-31: `verified_commit` is this record's own last-modified commit, recorded when the record was moved onto the estate ledger format. It is not a re-verification of the code the record describes, and `verified_paths` is empty, so the staleness gate is not yet armed for this record.

## Context

Ontomark was established as the shared ontology core during the estate reorganisation. Keeping an interactive graph renderer in the publisher alone would recreate the duplication that the shared parser, model, reasoner and NGG1 codec are meant to remove. Obsidian users also need an installable plugin without cloning the Rust workspace or installing Node.

The existing Narrative Goldmine renderer is coupled to its React stores, routing, publication tiers and physics lifecycle. Moving that entire application into the plugin would retain the coupling. Its reusable graph interaction belongs with Ontomark; publication and vault integration remain with their host applications.

## Decision

`packages/graph` in Ontomark owns the framework-neutral graph view, typed hierarchy adapter, focus traversal, identity validation, selection, pan/zoom, fitting and resource cleanup. It accepts explicit nodes, edges and optional layout positions. It performs no network requests, IRI minting, vault reads or publication.

The shared Canvas implementation adapts the orthographic behaviour of Narrative Goldmine's former R3F renderer. It uses browser text rendering, with no font fetch. The publisher retains its tier loader, route navigation, palette and worker simulation, and supplies position snapshots through a thin host component. Its independent node/edge/label renderers have been removed after passing the shared consumer gates.

The separate [Ontomark Obsidian repository](https://github.com/DreamLab-AI/ontomark-obsidian) owns vault events, commands, settings, note opening, query views and guarded edit previews. Its graph view consumes the same shared package, adapting the typed engine index and keeping asserted and inferred edges visibly distinct. Plain prose links are not promoted to ontology axioms.

The Rust/WASM NGG1 codec remains in `ontomark-ngg1`. VOWL's existing package is a compatibility consumer of that codec and retains its physics API. The graph renderer does not implement a second binary codec.

Consumers pin immutable graph and engine release assets with integrity receipts. The Obsidian deployment publishes `main.js`, `manifest.json`, `styles.css`, a ready-to-install ZIP and third-party notices. End users require Obsidian alone; build tools run in CI.

## Verification and consequences

Library tests cover invalid identities/geometry, deterministic seed positions, induced neighbourhoods and the asserted/inferred boundary. Mounted browser acceptance covers keyboard selection, activation, edge filters, zoom, focus and cleanup. Actual Obsidian acceptance also covers note opening and plugin unload/reload; the independent offline shim remains a separate network-isolation check.

Public publisher runtime `9cf6cb87ef72f226b14bcf9a56126e1854683f6b` passed both [publisher CI](https://github.com/jjohare/visionGraph/actions/runs/38179004619) and [Pages deployment](https://github.com/DreamLab-AI/knowledgeGraph/actions/runs/38180896856). Fresh browser acceptance at the canonical [graph route](https://narrativegoldmine.com/graph/) verified the rendered graph, accessible node controls and neighbourhood focus. About exposes the full engine, renderer and JavaScript runtime notices. The 99-package inventory matches its exact build digest; the [HTTP receipt](../evidence/resume/publisher-live-http.json) records the canonical routes and notice hashes. The publisher's docs-only activation receipt is `32c81a2225f8d7e1d3186cc11c2dad09d6dbfe20`; its broader corpus-profile migration remains partial.

Nightly dreaming evaluates native core contracts without automatic merging. Browser, Obsidian and WebAssembly changes require their dedicated acceptance harnesses; the nightly configuration hands those hypotheses to their owners. Attribution travels with binary/WASM/plugin distributions; [upstream sources](../UPSTREAMS.md) and [full notices](../../THIRD-PARTY-NOTICES.md) are part of the release evidence.

This decision supersedes the original monorepo `plugin/` location and the publisher-only renderer ownership. Application routing, corpus policy and Obsidian API adaptation are deliberately host-owned.

## Status note, 31 October 2026

The owner confirmed this record as complete and live: the release shipped, acceptance passed, and the estate runs it. This replaces the conservative `partial` / `inactive` mapping made when the ledger was unified.
