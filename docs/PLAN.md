---
id: PLAN-ONTOLOGY-001
status: proposed
date: 2026-10-11
owner: jjohare
parent_adrs: [visionflow/ADR-2014, visionflow/ADR-2015, visionflow/ADR-2016, ADR-VG-007]
implementation_status: none
activation_status: inactive
---

# Shared ontology workspace investigation and delivery plan

The recommended design is a dedicated Rust workspace that owns the reusable ontology system, with native crates and WASM bindings consumed by Obsidian and estate applications. Markdown with machine-readable headers stays authoritative. The Obsidian application should be useful on ordinary vaults and offer rich ontology tools; estate consumers can use additional capabilities without imposing them on plugin users.

This is a proposed architecture and implementation plan. It includes source investigation, an existing renderer test baseline and external pattern research. It does not claim an implemented plugin, a proven WASM reasoner or a migrated estate.

## Owner requirements

- Local operation for core processing; integrations are optional.
- Human-readable Markdown and machine-readable headers are the source of truth. No database requirement for the plugin or shared core.
- Rust implementations, compiled to WASM where appropriate; native applications consume the same crates directly.
- General Obsidian users need no estate knowledge. README integration examples follow the standalone experience.
- Maximise Obsidian utility and allow still broader estate capabilities.
- Move existing components and change APIs where that produces a better design; remove parallel implementations.
- Review the older ontology model and support beneficial, proven bulk migrations.
- Use upstream crates and permissively licensed patterns where useful; preserve attribution and package boundaries.
- Scope is Markdown. OCR and non-Markdown document ingestion are excluded.
- Write the cross-repository ADR set before implementation; report trade-offs at the end.

## ADR set

| Repository | Proposed record | Responsibility |
|---|---|---|
| VisionFlow | [ADR-2014](../../../VisionFlow/docs/adr/ADR-2014-shared-ontology-workspace.md) | Shared source ownership, capabilities and estate adoption |
| VisionFlow | [ADR-2015](../../../VisionFlow/docs/adr/ADR-2015-typed-ontology-model-and-migration.md) | Typed semantics and reversible migration |
| VisionFlow | [ADR-2016](../../../VisionFlow/docs/adr/ADR-2016-local-markdown-ontology-application.md) | Offline Obsidian product and optional capabilities |
| visionGraph | [ADR-VG-007](../../docs/adr/ADR-VG-007-shared-ontology-workspace-and-profile.md) | Corpus profile, CI producer and embedded explorer migration |
| VisionClaw | [ADR-2137](../../../project/docs/adr/ADR-2137-consume-shared-ontology-workspace.md) | Parser/writer/reasoner consolidation and native consumer adoption |
| Agentbox | [ADR-2136](../../../agentbox/docs/adr/ADR-2136-pin-the-shared-ontology-toolchain.md) | Released CLI packaging and estate tooling adoption |
| Loom | [ADR-142](../../../loom/docs/design/ADR-142-consume-versioned-shared-ontology-contracts.md) | Compatible bundles and optional shared query/semantic capabilities |
| vowl-wasm | [ADR-001](../../../vowl-wasm/docs/adr/ADR-001-shared-ontology-rendering-contract.md) | Graph contract, layout ownership and portable host adapters |

All records are proposed, implementation none, activation inactive. Existing accepted governance remains in force. Source review, implementation tests and deployed-generation verification are separate evidence categories.

## What the source investigation found

Paths below are relative to `/home/devuser/workspace`; line numbers identify the inspected working source and can move. The [snapshot](evidence/source-snapshot.json) records SHAs and tracked dirty paths. This is not a clean-checkout reproducibility receipt: relevant working changes existed, and another worker may advance a repository during planning. Before extraction, capture exact source hashes and reconcile those changes.

| Finding | Evidence | Design consequence |
|---|---|---|
| In-memory parser seams already exist | `project/crates/vault-core/src/page.rs:150`; `vocabulary.rs:492` | Extract and extend, rather than rewrite parsing in TypeScript |
| Compiler mixes semantics with host operations | `project/crates/vault/src/build/mod.rs:291`; `build/generation.rs:118` | Supply document snapshots, clock/provenance and output sinks through adapters |
| Whole vault crate is a poor WASM boundary | `project/crates/vault/Cargo.toml` has native TLS, network/signing and Rayon dependencies | Split portable capability dependency closures |
| Whelk wrapper uses native timing | `project/crates/vault/src/whelk.rs:647,667,677,682` | Target-aware timing plus real browser execution gate |
| Duplicate parsing/writing remains | `project/crates/visionclaw-domain/src/vault/mod.rs:278,308`; `src/services/page_parser.rs:69` | One shared document layer, including guarded writing |
| Reasoning paths differ | `project/crates/visionclaw-adapters/src/whelk_inference_engine.rs:115`; `visionclaw-ontology/src/reasoning/custom_reasoner.rs:60` | Shared typed input and capabilities; retain explicit physics/presentation projections |
| NGG1 has three implementations | `project/crates/vault/src/build/ngg1.rs:150`; `vowl-wasm/src/ngg1.rs:128`; `visionGraph/publishing-tools/WasmVOWL/modern/src/lib/ngg1.ts:144,375,483` | One format owner, Rust codec, generated types/constants and cross-language fixtures |
| Renderer supports transfer buffers | `modern/src/workers/positionTransport.ts` | No shared-memory requirement for default graph operation |
| Renderer is tied to web routing/data | `modern/src/pages/GraphPage.tsx:209,455`; `hooks/useSimulation.ts:106` | Artefact-source, navigation and worker-factory interfaces |
| Roles/domains are estate-specific | `vault-core/src/page.rs:99`; `domains.rs`; `vault/src/build/graph_tiers.rs:58`; `modern/src/types/scope.ts:64` | Versioned profile, arbitrary vault folders and taxonomy registries |
| Producer pins already diverge | `agentbox/flake.nix:42` uses `3213e314…`; `visionGraph/.github/workflows/publish.yml:50` uses `2a2156db…` | One release manifest and visible consumer upgrade status |
| Loom consumes verified artefacts | `loom/crates/loom-facade/src/bundle.rs:16`; `loom-graph-oxigraph/src/lib.rs:38` | Preserve stage/verify/activate semantics and loaded identity |

The current estate includes query stores in VisionClaw and Loom. They are downstream implementation details, not a reason to introduce a database into the authoring model or the new plugin.

## Baseline ontology review

These are static findings requiring regression fixtures before changes:

1. **Entity types:** vocabulary declares Property, but `vault/src/model.rs:105,367` models Class/Individual and defaults non-Individuals to Class. Use explicit supported kinds and reject unsupported coercions.
2. **Predicate coverage:** the model loops frozen `RELATION_KEYS` (`model.rs:36,356`) despite a configurable vocabulary. Preserve arbitrary declared predicates through model, RDF and graph.
3. **IRI correctness:** `build/turtle.rs:824–849` preserves HTTP-like strings and maps estate URNs, but prefixes other IRIs. Preserve valid full IRIs; apply estate public-URL mapping explicitly.
4. **Identity resolution:** `model.rs:217` retains the first claimant; `:335` synthesises missing references; `:426` indexes IRI tails. Distinguish ambiguity, unresolved links and external identity, including equal tails in different namespaces.
5. **Meaning of edges:** `turtle.rs:326,334` emits both direct class-IRI property triples and restrictions for selected predicates. Model annotation/concept links, individual facts and class restrictions separately before deciding migration semantics.
6. **Inference scope:** exported statements exceed the inspected Whelk adapter's supported inputs. Document the supported axiom matrix and execution limits. A successful run does not establish complete OWL inference.
7. **Policy versus logic:** acyclicity can be an estate rule without being a universal OWL consistency condition. Domain/range inference is separate from required-field validation.
8. **Design-record drift:** `project/docs/VAULT-corpus-format.md:392` calls ADR-2124–2128 unratified while their records mark accepted/staged. Establish decided/implemented/tested/activated status per capability before freezing the new baseline.

Retain stable authored IRIs, multiple inheritance, asserted/inferred separation, explicit publication projection and consistency refusal. Review old defaults without automatically replacing OWL with SKOS or switching reasoners. Existing Python golden parity is useful for legacy artefacts but explicitly excludes inferred TTL (`vault/tests/golden_parity.rs:20`); it cannot prove new reasoning parity.

## Core architecture and feature boundaries

The workspace's portable model combines standards-aware RDF/OWL structures with document/evidence ownership. Avoid another general RDF/OWL implementation. Horned-OWL supplies representation/parsing, Whelk supplies the supported classifier, and existing RDF/SPARQL crates supply terms, syntax and optional query evaluation. Current manifests use Horned-OWL 1.2 and the Whelk fork at `79a1ee21789add5a66faa0ec1a830c32dba9b539`; upstream convergence should be investigated before release. No separate crate named `sparcl` was found in inspected manifests; the evidenced query implementation is Oxigraph/SPARQL.

Dependency flow: Markdown adapter → document model → typed ontology model → validation/reasoning/compiler → graph/query/export views. Optional candidate generation feeds reviewed document patches through the same pipeline. Native and WASM adapters share implementation and fixtures, not merely similar public method names.

Use small dependency closures rather than a maximal default crate. Core parsing, identity, schema checks, supported reasoning and graph compilation must operate offline. Queries, publication, governance and model-assisted Markdown proposals are explicit capabilities. Networking/signing/storage remain host adapters. Capability selection must not silently change the semantics of the same axiom; results identify the selected profile and limits.

New package names are provisional and must be checked for registry availability. `vault-core` is already an unrelated crates.io name. Preserve existing `vowl-wasm` and `@dreamlab-ai/vowl-wasm` package identity where feasible. The shared repository must build from its own checkout and declared dependencies, without sibling paths or estate credentials. Registry publication requires full rustdoc, compiling examples, README/licence/repository metadata and successful package verification. Whelk's Git dependency needs a registry-compatible release strategy before dependent crates are published; do not substitute an unverified registry version.

## Product scope

The first complete Obsidian release includes ordinary-vault navigation, explicit ontology authoring, validation diagnostics, supported local inference with declared limits, graph exploration, local querying, export preview and guarded source updates. Opening a vault never requires public flags and never silently treats prose links as subclass axioms. Preserve unknown headers, comments, prose, aliases and non-ASCII text.

The proposed query minimum is local read-only SPARQL SELECT, ASK and CONSTRUCT with triple patterns, joins, FILTER, OPTIONAL, UNION, ORDER BY and LIMIT. Users explicitly select asserted, inferred or combined datasets; public-view queries use only the public projection. Unsupported forms fail explicitly. Disable remote SERVICE/dataset access and source-mutating UPDATE. Bound execution time/results and test native/WASM parity. Backend selection must prove this contract before the package boundary is fixed.

Reasoner explanations must distinguish a supplied derivation from a reconstructed summary; do not promise proof traces the backend cannot provide. Large-vault operation needs worker cancellation, generation tagging and an informative last-valid-result state. The first performance milestone measures a small generic vault and the estate-scale fixture, then sets supported size/latency/memory budgets before product release. No unmeasured speed promise belongs in the README.

Optional Markdown automation proposes evidence-backed assertions from existing notes. It can use deterministic structure or a separately configured local model. All candidates retain document revision, exact spans/quotes, extractor/rule identity and review state. The core remains useful without models. OCR, PDF conversion and external document ingestion are outside this programme.

## External patterns and existing work

Perplexity was queried through the requested [perplexity-research skill](/opt/agentbox/skills/perplexity-research/SKILL.md); raw results are in [search evidence](evidence/perplexity-targeted.json). Recommendations below use primary project sources, not search snippets as proof.

| Source | Useful pattern | Adoption boundary |
|---|---|---|
| [EdgeQuake](https://github.com/raphaelmansuy/edgequake) | Staged extraction, provider contracts and explicit review states; Apache-2.0 project | Learn from or selectively reuse components. Its PostgreSQL/GraphRAG deployment is not the plugin architecture |
| [edgextract Rust model](https://github.com/raphaelmansuy/edgextract/blob/master/rust/edgextract/src/types.rs) | Mention spans, evidence, confidence and accept/review/reject bands | Optional candidate protocol inspiration; extraction type constraints are not OWL semantics |
| [edgextract browser bridge](https://github.com/raphaelmansuy/edgextract/blob/master/web/src/tev1/bridge.ts) | Native/WASM separation and browser inference integration | Current synchronous bridge requires SharedArrayBuffer/Atomics and cross-origin isolation. Do not assume Obsidian support |
| [Horned-OWL](https://github.com/phillord/horned-owl) | Typed OWL representation | Already used; prefer extending shared adapters over parallel axiom encodings |
| [Oxigraph component crates](https://github.com/oxigraph/oxigraph) | RDF terms, parsers/serialisers and separable SPARQL utilities | Select components without requiring database persistence; verify WASM feature closure |
| [Sophia](https://github.com/pchampin/sophia_rs) | RDF dataset/term abstractions | Design comparison; avoid introducing a second term model unless it solves a measured gap |

The earlier memory entry `knowledgegraph-ontocast-integration-2026-08-16` led to `knowledgeGraph/docs/integrations/ontocast.md`. Its adapter is now archived and emits retired Logseq syntax; it is historical design evidence, not a ready plugin component. docBox has evidence-span/reconciliation ideas but is a clinical pipeline, not a proven general converter. No document-ingestion implementation is planned here.

There is already direct upstream reuse: [EdgeQuake's pipeline manifest](https://github.com/raphaelmansuy/edgequake/blob/edgequake-main/edgequake/crates/edgequake-pipeline/Cargo.toml) depends on edgextract 0.1.0 with defaults disabled. That demonstrates a selective component boundary worth evaluating if optional extraction is implemented. It does not establish maturity or Obsidian runtime compatibility; the inspected edgextract source was `3f08b1dca84ebacffd5152cee37639a62df0fb93` and was not executed.

Formal distinctions come from standards: [OWL EL profiles](https://www.w3.org/TR/owl2-profiles/), [SKOS](https://www.w3.org/TR/skos-reference/) and [SHACL](https://www.w3.org/TR/shacl/). An extraction schema's allowed pair types, an OWL domain/range axiom and a validation shape are different contracts.

## Delivery sequence with completion gates

| Work package | Concrete output | Exit condition |
|---|---|---|
| WP-01 Baseline contract | Capability census, duplicate inventory, regression fixtures and explicit status reconciliation | Reproduce gaps; identify intentional semantic differences and every current consumer |
| WP-02 Portable vertical slice | In-memory Markdown → typed model → Whelk → RDF/graph, native and WASM | Browser-worker execution and native parity on positive/negative fixtures; no service fallback |
| WP-03 Shared ownership | Independently buildable workspace, licences, versioned profiles, packages and bindings | Clean checkout builds; package verification; no sibling paths; exact release manifest |
| WP-04 Core consolidation | Common parser/writer, resolution and reasoning adapters | VisionClaw consumers pass round-trip/semantic tests; superseded implementations deleted |
| WP-05 Complete plugin | Offline authoring, validation, inference, query, graph and export | Installed Obsidian tests on plain and estate vaults, edits/rename/delete and worker lifecycle |
| WP-06 Estate adoption | visionGraph CI/web view, Agentbox CLI, VisionClaw publication, Loom contracts | All use compatible released inputs and identify the generation actually loaded |
| WP-07 Model migration | Versioned dry run, identity map, guarded batch patches and rollback | Reviewed semantic/visibility deltas; idempotence; interrupted-run recovery; consumer acceptance |
| WP-08 Optional automation | Evidence-bearing Markdown proposals using shared types | Deterministic baseline plus separately evaluated optional model; no silent source mutation |

WP-02 is an early feasibility gate, not a shippable stub. WP-05 cannot be declared complete after only embedding the renderer. WP-07 can move earlier for a proven semantic repair, but never before its replacement model and rollback tests. No new repository, crate publication, corpus mutation or deployment has been performed during this planning task.

## Expectations and anti-drift tests

| ID | Required behaviour | Counter-example and permanent test |
|---|---|---|
| EXP-01 | Native and WASM agree for supported axioms | Defined-class, disjointness and restriction fixtures must produce the same entailments/refusals; closure-only substitute fails |
| EXP-02 | Full identity survives all representations | Arbitrary URN, same tail/different namespace and rename fixtures; no estate prefix insertion |
| EXP-03 | Unknown metadata/prose survives editing | Parse/patch/parse with comments, aliases and Unicode; unrelated bytes remain unchanged |
| EXP-04 | Public export excludes private derivations | Public child/private ancestor plus prose/diagnostic leaks tested across every artefact |
| EXP-05 | Generic and estate profiles both work | Plain one-note vault accepted; estate invalid schema still refused |
| EXP-06 | Worker output belongs to current source | Edit/delete during reasoning; cancelled old result cannot replace new generation |
| EXP-07 | Model output remains attributable and reviewable | Stale quotes, unsupported predicates and ambiguous identities cannot auto-promote |
| EXP-08 | Migration is reversible and idempotent | Second run has no changes; interrupted run recovers; source edits invalidate stale patch |
| EXP-09 | One release has one tested contract | All consumers load a released fixture bundle; deletion/equal-count replacement updates identity |
| EXP-10 | No duplicate semantic implementations remain | Inventory linked to removed code and replacement call sites, plus consumer regression tests |
| EXP-11 | Offline plugin is complete | Network-disabled installed release performs all core operations; no missing WASM asset fetch |
| EXP-12 | Querying does not bypass Markdown authority | Rebuild empty cache from headers; read queries cannot mutate source or reveal excluded projection |

Carry forward `vault/tests/{golden_parity,build_cli,publish_gate,defined_classes,el_profile,probe_consistency,whelk_scaling,apply_cli,propose_cli}.rs`, native mutation round-trip tests and Loom generation/allowlist tests. Add cross-language malformed-buffer and arbitrary-taxonomy fixtures. Normalise RDF blank-node identity appropriately; compare triple meaning separately from legacy byte/order contracts. Use seeded tolerances for layout tests rather than asserting cross-platform floating-point byte identity without justification.

Release checks include formatting, Clippy, rustdoc/package verification, native/WASM feature matrices, dependency/licence review, plugin accessibility, consumer fixtures and evidence receipts. Every regression-critical expectation needs a named stabilising test. Cross-family evidence audit is a future implementation gate; the planning reviewers were same-family. No implementation quality gate is represented as passed here.

## Executed evidence and limits

[Renderer baseline receipt](evidence/renderer-baseline.txt): 3 files and 21 tests passed, covering NGG1 serialisation, worker protocol and transferable position buffers. The receipt includes command, time, SHA and output. It does not cover the new design or in-app behaviour. External code was inspected, not built or benchmarked. Whelk's full transitive WASM compatibility remains unknown. Existing dirty source must be captured before reproducible extraction tests.

## Trade-offs and remaining decisions

1. **General product versus estate richness:** use profiles and optional capability crates. This adds configuration/schema discipline but preserves both audiences; do not lower the core to the least expressive plugin feature.
2. **WASM portability versus full native capability:** portable semantics and native adapters share Rust source. Browser resource/host limits remain real. Prove local reasoning early; optional integrations must not conceal a failed core requirement.
3. **Model improvements versus compatibility:** correct the concrete representation gaps with versioned fixtures and migrations. Preserve useful IRIs and meanings; retire historical byte/order constraints only after their consumers migrate.
4. **Package reuse versus licence boundaries:** existing processor crates are AGPL while other components are permissive. Decide ownership/licence strategy from provenance; dependency selection alone does not make copied code permissive. Preserve the existing MIT renderer boundary.
5. **Rich automation versus trust:** evidence-backed proposals maximise utility without treating model confidence as truth. Optional model-assisted features add memory/download/runtime costs; Markdown core operation remains model-free.
6. **Fast responsiveness versus authoritative recomputation:** begin with bounded full rebuilds in workers and measure; introduce incremental reasoning only with equivalence tests for additions, deletions and retractions.
7. **Broad reasoning versus predictable local operation:** retain a declared EL capability first. Wider semantics need measured benefit and a backend with explicit coverage, rather than optimistic success flags.
8. **Source consolidation versus coordinated rollout:** one repository removes forks but cannot atomically upgrade every deployment. Use release manifests, consumer status and rollback; distinguish source merged, artefact published and generation activated.

Decisions still to settle before implementation are the new repository/package names, the package licence/ownership arrangement, the first supported desktop/mobile release matrix, and acceptance of the proposed semantic/migration contract. No further architectural permission is needed to finish this ADR set.
