# ADR-OM-002: shared graph code and separate Obsidian distribution

- Status: Accepted
- Date: 2026-10-11
- Implementation: consumer acceptance and release in progress
- Related: [ADR-OM-001](ADR-OM-001-sprint-decisions.md), [estate removal plan](../DEPRECATION.md)

## Context

Ontomark was established as the shared ontology core during the estate reorganisation. Keeping an interactive graph renderer in the publisher alone would recreate the duplication that the shared parser, model, reasoner and NGG1 codec are meant to remove. Obsidian users also need an installable plugin without cloning the Rust workspace or installing Node.

The existing Narrative Goldmine renderer is coupled to its React stores, routing, publication tiers and physics lifecycle. Moving that entire application into the plugin would retain the coupling. Its reusable graph interaction belongs with Ontomark; publication and vault integration remain with their host applications.

## Decision

`packages/graph` in Ontomark owns the framework-neutral graph view, typed hierarchy adapter, focus traversal, identity validation, selection, pan/zoom, fitting and resource cleanup. It accepts explicit nodes, edges and optional layout positions. It performs no network requests, IRI minting, vault reads or publication.

The shared Canvas implementation adapts the orthographic behaviour of Narrative Goldmine's former R3F renderer. It uses browser text rendering, with no font fetch. The publisher retains its tier loader, route navigation, palette and worker simulation, and supplies position snapshots through a thin host component. Its independent node/edge/label renderers are removed once the shared consumer gates pass.

The separate [Ontomark Obsidian repository](https://github.com/DreamLab-AI/ontomark-obsidian) owns vault events, commands, settings, note opening, query views and guarded edit previews. Its graph view consumes the same shared package, adapting the typed engine index and keeping asserted and inferred edges visibly distinct. Plain prose links are not promoted to ontology axioms.

The Rust/WASM NGG1 codec remains in `ontomark-ngg1`. VOWL's existing package is a compatibility consumer of that codec and retains its physics API. The graph renderer does not implement a second binary codec.

Consumers pin immutable graph and engine release assets with integrity receipts. The Obsidian deployment publishes `main.js`, `manifest.json`, `styles.css`, a ready-to-install ZIP and third-party notices. End users require Obsidian alone; build tools run in CI.

## Verification and consequences

Library tests cover invalid identities/geometry, deterministic seed positions, induced neighbourhoods and the asserted/inferred boundary. Mounted browser acceptance covers keyboard selection, activation, edge filters, zoom, focus and cleanup. Actual Obsidian acceptance also covers note opening and plugin unload/reload; the independent offline shim remains a separate network-isolation check.

Nightly dreaming evaluates native core contracts without automatic merging. Browser, Obsidian and WebAssembly changes require their dedicated acceptance harnesses; the nightly configuration hands those hypotheses to their owners. Attribution travels with binary/WASM/plugin distributions; [upstream sources](../UPSTREAMS.md) and [full notices](../../THIRD-PARTY-NOTICES.md) are part of the release evidence.

This decision supersedes the original monorepo `plugin/` location and the publisher-only renderer ownership. Application routing, corpus policy and Obsidian API adaptation are deliberately host-owned.
