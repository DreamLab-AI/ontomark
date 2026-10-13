# ADR-OM-006: what Ontomark takes from Dot Triples, and the bridge between them

- Status: Accepted (owner request of 13 October 2026: "adopt the good stuff with attribution")
- Implementation: active. `bridges/dot-triples/` (this repository); the `ontomark-sparql` block in [ontomark-obsidian](https://github.com/DreamLab-AI/ontomark-obsidian) ([ADR-OBS-004](https://github.com/DreamLab-AI/ontomark-obsidian/blob/main/docs/adr/ADR-OBS-004-in-note-sparql-after-dot-triples.md)).
- Related: ADR-OM-002 (plugin boundary), ADR-OM-005 (one producer, explicit projections), [`docs/UPSTREAMS.md`](../UPSTREAMS.md).

## Context

[Dot Triples](https://github.com/cristianvasquez/dot-triples) and its [Obsidian plugin](https://github.com/cristianvasquez/dot-triples-plugin), both by Cristian Vasquez and both MIT, read a vault as RDF and query it with SPARQL. They were reviewed on 13 October 2026 (library at `99d74c8`, plugin at `2c17b41`, release 2.0.0) as the one other serious RDF-in-Obsidian project.

The two tools answer different questions about the same notes. Dot Triples turns everything into triples (frontmatter, inline `key :: value` fields, links, headings, code blocks, canvas edges) and leaves meaning deferred: keys become `urn:token:key`, names become `urn:name:Note`, and its README says "a mapping step then resolves what it can". It has no reasoning and no write path. Ontomark turns only typed frontmatter into an ontology, reasons over it, and writes back through reviewed diffs. Each stops where the other starts.

## Decisions

1. **The in-note SPARQL block is adopted.** Dot Triples' `dot-sparql` fence, with `__THIS__` for the current note and `[[Note]]` rewritten to an IRI before the query runs, is the right surface for a query people keep. Ontomark's plugin gains an `ontomark-sparql` fence with the same two tokens and a `# dataset:` directive for asserted, inferred or combined. The implementation is independent and skips string literals, `<IRI>` tokens and comments when rewriting, which Dot Triples' README lists as a known defect of its own.

2. **A bridge is published, here.** `bridges/dot-triples/dot-triples-to-ontomark.rq` is one SPARQL CONSTRUCT that lifts Dot Triples output for the keys Ontomark's profiles read into the triples the engine asserts. It is the "mapping step" Dot Triples defers. A test runs it over real `triplifier-md` output for five notes and requires equality with the engine's own reading of those notes on every bridged predicate. The bridge is a consumer-side input adapter, not a second producer (ADR-OM-005 decision 1): it neither parses Markdown nor changes any `ontomark build` artefact.

3. **Identity is joined on authored IRIs, not on names.** Ontomark derives `urn:ontomark:page:<path>`; Dot Triples knows only the note name. The bridge mints Ontomark's form from the name, which is exact at the vault root and for any note carrying `iri` (generic) or `resource` (estate). Nested notes without one are documented as a known gap, resolved by giving them an `iri`. No compatibility shim is added on either side.

4. **Evidence locations, when exported, use Web Annotation selectors.** Dot Triples anchors headings and parts with `oa:TextQuoteSelector` and RFC 5147 line ranges. Ontomark's ingest already holds a verbatim quote per accepted fact but persists only the frontmatter patch; the quote lives in the review modal. Nothing is exported today, so nothing is added today. If evidence is ever written out (a provenance sidecar, a proposal to the governance forum), it will be shaped as `oa:` selectors. This is recorded so the choice is not reinvented.

5. **Inline `key :: value` fields are not adopted.** They are the Dataview habit Dot Triples serves well. Adopting them would end Ontomark's rule that only frontmatter asserts and prose is prose, the rule that makes a diff a complete review. The plugin README says so in one sentence and points at Dot Triples and the bridge.

## Attribution

Idea credit goes to Cristian Vasquez for the in-note query surface and the deferred-identifier model that makes a bridge possible. No source code was copied from either repository; `docs/UPSTREAMS.md` and both repositories' `THIRD-PARTY-NOTICES.md` carry the method attribution. The fixture `bridges/dot-triples/fixtures/zoo.nt` is the unedited output of the MIT-licensed `triplifier-md` 0.3.0 over notes written here.

## Consequences

- Estate consumers of `ontomark build` output (VisionFlow, the Loom, Agentbox skills, visionGraph) see no change: no compiler artefact, CLI verb or wire type is touched. The bridge is an opt-in file; the block is plugin UI.
- The two plugins can share one store: Dot Triples' external mode points at an Oxigraph server; `ontology.ttl` and the bridged graph load into the same server, so its panels can ask the reasoned hierarchy that its own PIMO example wants and has no source for.
- The bridge test pins parity to the engine's current vocabulary. A vocabulary change in `ontomark-wasm::projection` fails it on purpose; update the `.rq` in the same change.
