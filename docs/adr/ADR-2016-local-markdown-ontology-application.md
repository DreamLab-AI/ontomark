---
id: ADR-2016
title: Make Obsidian a local application of the shared ontology core
date: 2026-10-11
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
depends_on: [ADR-2014, ADR-2015]
verified_commit: "not applicable — proposal without implementation"
verified_paths: []
owner: jjohare
review_trigger: plugin capability, target support, model integration or persistence change
repo: visionflow
domain: BASELINE-visionflow.md
---

# ADR-2016 — Make Obsidian a local application of the shared ontology core

## Context

The owner wants maximum useful Obsidian functionality with even broader utility for estate consumers. Users should need no knowledge of the estate. Local processing and Markdown authority are settled requirements; OCR and non-Markdown document ingestion are outside the present scope.

## Proposed decision

Ship a self-contained plugin with bundled portable Rust/WASM capabilities and Obsidian adapters. It opens ordinary vaults without imposing estate folder names, namespaces or governance headers. Installation requires no database, account, service, model download or CLI. Existing notes remain readable and editable without the plugin.

Expose substantial functionality through progressive UI: typed relation editing, vocabulary inspection, identity/reference diagnostics, supported reasoning and its explanations/limits, graph navigation, local queries, export preview and guarded Markdown changes. Keep ordinary links and formal axioms visibly distinguishable. A normal vault with no ontology metadata starts with document navigation and explicit tools to introduce typed structure; it is not silently converted into a class hierarchy.

The same crates support optional richer capabilities for either plugin or estate consumers: vocabulary profiles, query evaluation, provenance, reviewed Markdown-assisted extraction, publication and federation adapters. The plugin does not need to expose every estate feature. Optional integrations are described after the standalone README quickstart and are disabled until configured.

Markdown plus machine-readable headers are authoritative. An in-memory graph or disposable local cache may accelerate interaction; deletion of that cache must be recoverable entirely from source documents and profile configuration. No background process mutates accepted ontology assertions merely because a model assigned high confidence.

The initial query capability supports local read-only SPARQL SELECT, ASK and CONSTRUCT over explicitly selected asserted, inferred or combined datasets. Basic triple patterns, joins, FILTER, OPTIONAL, UNION, ORDER BY and LIMIT are the minimum proposed contract; enumerate any additional supported forms. Unsupported syntax receives an explicit diagnostic. Apply time/result budgets and cancellation. Disable SERVICE and remote dataset loading; SPARQL UPDATE does not mutate the source. Public-view queries operate only on the public projection. Test native/WASM result parity and dataset selection before release.

Markdown-assisted automation follows source text → evidence-bearing candidates → identity/vocabulary resolution → reviewed patches → validation/reasoning. Candidate records retain source digest, span coordinate convention, quotations, model/rule version and rejected/uncertain dispositions. Preserve the full source note; avoid invented superclass, equivalence or causal assertions. Model-free structured-header processing is always available. Optional local inference has explicit installation, capability and resource requirements; it cannot be required for normal operation.

The application uses cancellable workers, generation-tagged results and transferable buffers by default. Obsidian file events invalidate affected inputs; full rebuild remains the correctness oracle until incremental equivalence is proved. Writes verify source revisions and preserve foreign headers and prose. Unload tears down workers, listeners and views. Public export is a separate previewed operation from private local viewing.

## Consequences and tensions

Rust is the common logic layer, not a requirement to rewrite host UI APIs or force native consumers through WASM. Mobile support is a target to prove, not an automatic consequence of compiling to WASM. Node/Electron APIs are unavailable on mobile; use the vault API and test actual supported devices. Desktop can ship first with accurate manifest limits if mobile gates fail, without introducing a required server fallback.

Borrow evidence, uncertainty and extraction-contract patterns from permissive projects such as EdgeQuake and edgextract; adoption of either runtime is not decided. EdgeQuake's database architecture does not fit the default product. edgextract's browser bridge currently requires cross-origin isolation, so it is not a drop-in Obsidian component. Formal extraction schemas do not replace OWL semantics.

## Verification

Required installed-plugin tests run with network disabled on a plain vault and the estate fixture: parse, edit, rename, delete, validate, reason, inspect evidence, navigate, query, export and unload/reload. Check malformed headers, ambiguous references, interrupted updates, stale worker results and non-ASCII source spans. Model-assisted tests are separate from deterministic core tests.

WASM Whelk execution, mobile support, resource budgets and full plugin packaging are unproved. The [delivery gates](../../../visionGraph/planning/ontology-workspace/README.md) prevent a renderer-only stub being counted as completion. Consult [Obsidian mobile guidance](https://docs.obsidian.md/Plugins/Getting%20started/Mobile%20development) and [release packaging](https://docs.obsidian.md/plugins/releasing/submit-plugin) when implementing.
