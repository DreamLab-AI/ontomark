---
id: ADR-001
title: Shared ontology rendering contract
date: 2026-10-11
decision_status: proposed
implementation_status: none
activation_status: inactive
owner: jjohare
review_trigger: source ownership, graph protocol or host adapter change
verified_commit:
verified_paths: []
---

# ADR-001: Shared ontology rendering contract

- Date: 2026-10-11
- Status: Proposed; inactive
- Implementation: None authorised by this record; implementation follows a separate phase.
- Parent decision: [VisionFlow ADR-2014: shared ontology workspace](../../../VisionFlow/docs/adr/ADR-2014-shared-ontology-workspace.md)
- Investigation and delivery plan: [visionGraph ontology workspace](../../../visionGraph/planning/ontology-workspace/README.md)

## Context

The proposed product is an offline Obsidian ontology plugin useful with an ordinary Markdown vault. The existing website and estate services must consume the same reusable components, without a second implementation drifting from the plugin. Rich reasoning, governance and integration capabilities can remain available to other consumers without becoming requirements for the plugin.

The current `vowl-wasm` package is an MIT Rust library with native and WASM builds. Its published crate and npm identities are already consumed by visionGraph. NGG1 support is optional. The NGG1 encoder currently belongs to VisionClaw's `vault` build, while this repository and the website contain Rust and TypeScript readers; the website also writes subgraphs. The website's React renderer includes HTTP loading, routing, fixed domain identities and worker packaging assumptions.

These are source observations, not proof that the proposed plugin runs in Obsidian. In particular, graph rendering and ontology reasoning have different portability constraints and need separate executable tests.

## Proposed decision

Adopt the shared ontology workspace as the source of reusable native/WASM processing contracts and host-independent rendering components. There must be one authoritative implementation of each semantic rule and one owner of each graph format. Obsidian, the website, the CLI and estate services are consumers with host adapters.

The existing `vowl-wasm` source may move into that workspace if the migration preserves its published crate and npm identities, compatible public APIs where promised, history and licence boundaries. Alternatively it may remain a separately released dependency with a clearly assigned contract owner. The implementation phase must choose one ownership arrangement before extraction; it must not leave independently maintained copies. A source move does not itself justify a package rename or API break.

Keep the reusable rendering/layout code under its current MIT terms. Do not copy AGPL corpus processing into an MIT package or change licences by relocation. The workspace can contain packages with distinct licences and explicit dependency boundaries.

### Graph contracts

Define graph identity independently of labels, note paths and publication slugs. Carry stable entity identifiers, typed edges, provenance where available and explicit distinctions between ordinary document links, asserted ontology relations and inferred relations. Rendering must not infer ontology axioms from a visual link.

Preserve NGG1 v1 for existing consumers through an explicit compatibility adapter. Consolidate its specification, golden fixtures, Rust codec and generated constants/types under one owner; retain efficient TypeScript buffer views where useful. Binary sections, alignment, edge codes, stable identifiers and domain/category interpretation are versioned contracts. A changed meaning requires a versioned migration, not a silently reinterpreted byte.

New generic graphs use a supplied taxonomy/domain registry, including no taxonomy at all. They must not require DreamLab domain names, six or eight fixed domains, NarrativeGoldmine IRIs or corpus-specific categories. The optional estate profile retains the historical registry order and identifiers so existing NGG1 artefacts remain readable.

### Host adapters and execution

The reusable renderer accepts graph data, navigation callbacks, theme settings and worker/resource factories. HTTP paths, React Router, note opening, Obsidian lifecycle hooks and publication policy belong to adapters. Obsidian supplies Markdown through its vault API and opens documents through its own navigation API.

Use asynchronous worker request/response contracts with generation identifiers, explicit errors, cancellation and stale-result rejection. Avoid blocking the UI during parsing, reasoning or layout. Transferable buffers are the baseline transport; shared memory is an optional optimisation after capability detection. Do not require cross-origin isolation, a particular global asset path or synchronous model callbacks for basic operation.

The same Rust semantic core serves native and WASM hosts. Rendering/layout and reasoning may use separate workers and packages; neither should drag native filesystem, signing, networking or database dependencies into a browser build. Rich estate functions remain optional crate capabilities or adapters rather than being deleted to fit the plugin.

Normal plugin operation requires no database, model, hosted service or estate account. Optional model assistance produces reviewable candidates and is separate from the accepted graph. Content scope for this phase is Markdown; OCR and document ingestion are outside this decision.

## Alternatives and tradeoffs

- Copying the renderer into the plugin is initially easy but creates duplicate fixes, domain rules and protocol evolution; reject it as a permanent arrangement.
- Importing the entire website preserves existing UI quickly but couples Obsidian to routes, HTTP endpoints and site styling; extract adapters instead.
- Replacing NGG1 immediately would simplify a fresh API but invalidate deployed consumers and fixtures; retain a tested v1 adapter while introducing generic contracts.
- Moving every component into one repository simplifies cross-package changes but increases release coordination and licence-management work. Keeping `vowl-wasm` separate preserves its independent release cadence but requires an explicit compatibility matrix and coordinated consumer tests. Either arrangement must have a single source owner.
- Shared memory can reduce copying but is not consistently available across hosts. A transferable baseline costs copies in exchange for portability.

## Acceptance evidence required before activation

1. Native and browser tests consume the same graph fixtures. The Rust encoder, Rust decoder and TypeScript reader agree on identifiers, records, edges and section bounds. Include truncation, invalid offsets, unknown versions and non-estate taxonomy fixtures.
2. Existing NGG1 v1 golden files remain compatible. Website and CLI consumer tests run against the proposed package versions; stale package or generated-type mismatches fail the build.
3. A plain Obsidian vault works without estate configuration or network access. Graph navigation opens the correct note after rename, and arbitrary frontmatter does not become an ontology assertion.
4. Real installed-plugin tests cover offline worker/WASM loading, note create/edit/rename/delete, stale generations, cancellation, unload/reload and resource disposal. Verify the transferable path with shared memory unavailable.
5. Shared native/WASM ontology tests independently establish diagnostic, asserted-triple and inferred-subsumption parity. Renderer success must not be used as evidence that reasoning works.
6. The migration records package ownership, licences, release provenance, compatibility policy and consumer upgrades; obsolete source copies are removed when their replacements are verified.

## Rollout and current effect

First establish the shared contracts and fixtures, then prove the smallest complete offline plugin slice, then migrate existing consumers against the same released components. Preserve a reversible consumer version change until parity is demonstrated. Performance budgets must be measured on representative vaults and agreed before claiming mobile support.

This ADR is a proposal only. It performs no source relocation, package publication, dependency upgrade, format migration, plugin implementation or change to current runtime behaviour.
