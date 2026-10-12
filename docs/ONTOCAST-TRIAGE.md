# OntoCast triage: what Ontomark adapts, replaces or drops

- Upstream: [growgraph/ontocast](https://github.com/growgraph/ontocast), Apache-2.0, 0.6.7, reviewed at `abb8d7b8d6db1a765d4f091a7423937c2cbc5d78` (CITATION.cff cites Belikov, GrowGraph, DOI 10.5281/zenodo.17796467).
- Companion decision: [ADR-OM-003](adr/ADR-OM-003-native-assisted-ingest.md).
- Size: about 67,000 lines of Python under `ontocast/`. The package ships no `NOTICE` file and no per-file copyright headers, so attribution rests on `LICENSE` and `CITATION.cff`.
- Method: module sizes, `pyproject.toml` dependencies, import counts and the upstream concept pages. The code was not executed. Anything marked **Port** still needs a line-by-line read of the named file before adaptation (see "Before any code is adapted").

## What the dependencies tell us

| Dependency | Where it shows | Consequence |
|---|---|---|
| `rdflib`, `oxrdflib`, `pyoxigraph`, `pyld` | 92 `rdflib` imports across the package | The graph layer is a library call, not an algorithm. `ontomark-model` replaces it. Nothing here is ported. |
| `pydantic`, `pydantic-settings` | 67 imports, `config/settings.py` is 3,347 lines | Schemas become Rust types. Settings become a small typed options struct. |
| `langchain-core`, `langgraph` | 33 imports | Orchestration framework. The host drives a two-phase state machine instead. |
| `numpy`, `scikit-learn` | 5 and 1 imports | Embedding clustering only. Optional, host-supplied in the Rust design. |
| `difflib` | 3 imports | Lexical sequence ratio. Re-implement and fixture-test; do not assume byte equality with Python. |
| `qdrant_client`, LanceDB | vector store modules | Dropped. |

## Triage

Bucket meanings: **Drop** is not carried across; **Replace** is already covered by an Ontomark crate; **Port** is a method re-implemented in Rust under attribution.

| Upstream path | Lines | Bucket | Ontomark home or reason |
|---|---:|---|---|
| `api/`, `cli/` | ~2,500 | Drop | HTTP service and CLI. The host (plugin, Loom, Agentbox) owns transport. |
| `integrations/langchain.py` | 727 | Drop | Framework glue. |
| `config/settings.py`, `config/section_labels/` | ~3,750 | Drop | Replaced by a small options struct. Section-label vocabulary is reconsidered only if section filtering is wanted. |
| `tool/vector_store/*` | ~4,200 | Drop | Retrieval is a host concern. |
| `tool/triple_manager/*` (Fuseki, in-memory) | ~1,400 | Drop | The vault is the store. |
| `tool/cache.py`, `tool/llm.py`, telemetry, tenancy, `util/loop*` | ~2,100 | Drop | Model access is host-supplied. |
| `agent/convert_document.py` and docling path | ~150+ | Drop | The input is already a Markdown note. |
| `onto/rdfgraph.py`, `onto/ontology*.py`, `onto/model.py`, `onto/triple_index.py`, `onto/sparql_models.py`, `tool/sparql.py` | ~7,300 | Replace | `ontomark-model` and `ontomark-compile` (`ontomark-wasm` query). |
| `onto/ontology_apply.py` and the `GraphUpdate` insert/delete patch | 299 | Replace | `ontomark-parse::patch` (source-digest-bound, UTF-16 span-safe). |
| `tool/agg/uri_builder.py`, `cd:` facts namespace | 445 | Replace | `ontomark-model::mint`. ADR-OBS-002 forbids a second identity scheme. |
| `tool/ontology_manager.py`, `ontology_condense.py`, `agent/render_ontology.py`, `criticise_ontology.py`, `normalize_ontology.py`, `tool/ontology_validation/*` | ~3,500 | Defer | Ontology growth. A later, separate candidate kind that needs governance. Not in the first slice. |
| `tool/chunk/chunker.py`, `sections.py`, `sizing.py`, `non_content.py`, `segment.py` | ~1,250 | **Port** | Heading-aware Markdown chunking for long notes. |
| `tool/chunk/{prepare,proposition,schema_detect,outline,section_llm,density,bibliography,util}.py` | ~3,100 | Defer | Document-level preprocessing. Revisit only for long-note support. |
| `tool/agg/{aggregate,rewriter,normalizer,signatures,clustering,entity_aligner,unit_scope,match_*}.py` | ~5,000 | **Port (guards only)** | Deterministic merge guards: sibling, literal conflict, functional by empirical use, initials, role, type, lexical, cluster veto, split on validation. Embedding candidate generation stays host-supplied or optional. |
| `tool/facts_validation/{gate,terms,literal_repair,unit_findings,acceptance}.py` | ~3,200 | **Port** | Deterministic fact checks and literal repair, expressed as diagnostics on a candidate. |
| `tool/facts_validation/shacl.py`, `prompt/shapes_contract.py` | ~1,050 | Drop | Wraps the SHACL engine. Use Ontomark profile and policy validation instead. |
| `tool/facts_validation/critic_patch.py`, `agent/criticise_facts.py` | ~1,270 | Defer | Second model pass. Added only after the single-pass slice is proven. |
| `agent/render_facts.py`, `complete_facts.py`, `stategraph/unit_pipeline.py`, `prompt/{render_facts,facts_guidelines,graph_format,term_sheet,llm_json_schema}.py` | ~2,900 | **Port (as data and contract)** | The render prompt contract and the unit-level request/response shape. Prompts are carried as versioned data, not code. |
| `onto/run_manifest.py`, `onto/unit_states.py`, `onto/state.py` | ~2,200 | Replace | A smaller per-run record in the ingest crate. |
| `util/numeric_inventory.py`, `util/measurement_lexicon.py` | ~1,070 | Defer | Numeric and unit repair. Domain-specific; revisit with evidence from real vault runs. |
| `agent/external_evidence.py`, `prompt/web_grounding.py` | ~490 | Drop | Web grounding has no place in a local, user-invoked action. |
| RDF 1.2 provenance emission | n/a | Replace | Evidence spans on candidates, bound to a source revision. |

## Mapping to the estate

The retired RDF-to-Logseq adapter (`knowledgeGraph/docs/integrations/ontocast.md`) already decided several things worth keeping:

1. Extraction output is a **candidate**, never a direct write.
2. Source IRIs are retained as evidence; project-local identity is re-minted.
3. Unsupported predicates stay machine-readable for review instead of widening the vocabulary.
4. Confidence starts low (`0.5`, method `inferred`); only review raises it.

The Markdown frontmatter form of that mapping is specified in ADR-OM-003.

## Before any code is adapted

1. Read each **Port** file in full and record the algorithm, thresholds and edge cases in a per-file note.
2. For each guard, write a failing fixture first, taken from the upstream documented behaviour (`docs/concepts/entity_disambiguation.md`), then implement.
3. Do not translate the Python line by line. Treat it as a specification for behaviour, not a source to transliterate.
4. Add the Apache-2.0 licence text, the upstream commit and the `CITATION.cff` reference to `UPSTREAMS.md` and `THIRD-PARTY-NOTICES.md` in the same change as the first adapted code.
5. Where a file's logic is substantially derived, note it in the rustdoc of the Rust item.

## Survey of alternatives (12 Oct 2026)

A Perplexity survey, with crate licence, version and update dates checked against crates.io and GitHub, found no Rust tool covering OntoCast's combination of ontology-guided LLM extraction, insert/delete patches, cross-chunk disambiguation, SHACL and passage provenance. Building blocks:

| Need | Choice | Licence |
|---|---|---|
| String scoring | `strsim` 0.11.1 | MIT |
| Jaccard | `textdistance` 1.1.1 | MIT |
| RDF model and I/O (if needed) | `oxrdf`, `oxrdfio` (not archived Rio) | MIT or Apache-2.0 |
| SHACL, native only | `rudof` `shacl_validation` | MIT or Apache-2.0 |
| Extraction method | SPIRES (OntoGPT) | BSD-3-Clause |
| Design references | edgextract, Open Ontologies, cognee-rs | Apache-2.0, MIT, MIT or Apache-2.0 |

Python methods worth reading, with licences as reported and not individually verified: Graphiti (Apache-2.0), KGGen (MIT; a third-party inventory disagrees, so verify the exact revision), iText2KG (Apache-2.0), Neo4j GraphRAG (Apache-2.0 with marked PSF-2.0 portions), LightRAG and Microsoft GraphRAG (MIT).
