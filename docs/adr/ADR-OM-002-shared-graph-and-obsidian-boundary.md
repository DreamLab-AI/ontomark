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

## Status note, 14 November 2026

Graph package 0.3.0 adds a third consumer and a layout driver, both inside the boundary above.

- **Vellum** (a VS Code web extension) draws note links with the shared renderer. Its integration decision is [ADR-VEL-001](https://github.com/DreamLab-AI/vellum-markdown-editor/blob/main/docs/adr/ADR-VEL-001-shared-graph-and-rust-wasm.md). The renderer gains two additive options for it: `ariaLabel`, because a note-link graph is not an "Ontology graph", and `preservePositions` on `setGraph`, because an update otherwise reseeds every node. Defaults are unchanged, so the Obsidian and publisher consumers see the same behaviour as 0.2.0.
- **Layout** stays outside the renderer. `packages/graph/src/layout.js` is a separate entry point (`@dreamlab-ai/ontomark-graph/layout`) that drives an engine the host supplies and reports snapshots for `setPositions`. It performs no I/O and loads no module. The renderer still imports nothing and starts no worker.
- **The engine is `vowl-wasm`'s existing force simulation**, through an adapter. No second layout implementation and no second graph codec were written: bundles without `loadArrays` take the graph through the module's own NGG1 encoder.

Verified here: 29 package tests, one of them against the released `vowl-wasm` 0.2.3 bundle and against a `vowl-wasm` build with `loadArrays`. An independent review of the driver found five defects (a drag stopping the layout on bundles before 0.2.2, a second failed graph going unreported, an unchanged graph leaving a reseeded renderer unarranged, a throwing host callback stalling unseen, and an early settle); each is fixed with a regression test. Not verified here: a browser run of the mounted renderer with the driver, and any measurement of layout cost on large graphs.

## Status note, 24 November 2026

Graph package 0.4.0 (released 2026-11-23, PR #8) passes layout engine parameters through the driver. This stays inside the boundary above, and the note on 0.3.0 still holds: no second layout implementation and no codec, and the renderer is unchanged. Everything new is opt-in. Without `params` the layout is exactly the 0.3.0 one.

- **`createLayout` option `params`, and live `layout.setParams(params)`.** These pass parameters to an engine that has `setParams`. The values merge over earlier ones, and a loaded layout restarts warm. Keys the engine cannot apply come back as `{applied, ignored}` and reach the option `onIgnoredParams`.
- **`vowlEngine` engines gain `setParams` and `supportedParams`.** `NggExplorer.setParam` ignores keys it does not know and has no getter, so the adapter reads the bundle's `version()` and applies only the keys that version has:
  - `charge`, `linkDistance`, `linkStrength`, `damping`, `centerGravity` and `alphaMin` from 0.2.0;
  - `theta` from 0.2.1;
  - the collision keys `collideRadius`, `collideStrength` and `collideIterations`, plus `componentGravity`, from `vowl-wasm` 0.4.0.

  A bundle with no readable version gets the 0.2.0 keys. Applied values are set again after every load, and `reheat` is never treated as a parameter.
- **`RECOMMENDED_LAYOUT_PARAMS`** (`collideRadius` 21, `collideStrength` 0.7, `collideIterations` 3, `componentGravity` 0.1) carries the `vowl-wasm` 0.4.0 `NggExplorer` collision and component-gravity settings, for nodes drawn at up to 12 px radius plus a label.

Verified here at `8222170`: 36 package tests pass (`node --test`), including those that run the driver against the real `@dreamlab-ai/vowl-wasm` 0.4.0 bundle. That bundle is a dev dependency pinned with its integrity, and CI requires it (`ONTOMARK_REQUIRE_VOWL`). Without params the 0.3.0 layout is reproduced exactly, and with the recommended params every pair of nodes is at least two collision radii apart. Downstream, WasmVOWL's modern explorer lays out on `NggExplorer` through this driver and adapter. The README version table and `packages/graph/README.md` already name 0.4.0 as current. Not verified here: a browser run of the mounted renderer with the driver, or any measurement of layout cost on large graphs.
