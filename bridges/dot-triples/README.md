# Dot Triples bridge

[Dot Triples](https://github.com/cristianvasquez/dot-triples) by Cristian Vasquez reads every Markdown note and JSON Canvas board in a vault as RDF: files, notes, headings, inline `key :: value` fields, links and code blocks, with Web Annotation selectors saying where each came from. Its [Obsidian plugin](https://github.com/cristianvasquez/dot-triples-plugin) stores that graph in an in-process Oxigraph and queries it with SPARQL from `dot-sparql` blocks in notes. Both are MIT.

Dot Triples stops deliberately short of meaning. Field keys stay as `urn:token:key`, note names as `urn:name:Note`, and its README says a mapping step downstream "gives them classes and datatypes". Ontomark is that step for the ontology subset: the frontmatter keys its profiles read (`type`, `subclass-of`/`is-a`, `instance-of`, `domain`, `range`, `equivalent-to`, `disjoint-with`, `subproperty-of`, `iri`/`resource`, `title`, `aliases`) are exactly the tokens the bridge resolves.

## What is here

| File | Purpose |
| --- | --- |
| `dot-triples-to-ontomark.rq` | SPARQL 1.1 CONSTRUCT that lifts Dot Triples output into the OWL/RDFS/SKOS triples the Ontomark engine asserts for the same notes. |
| `fixtures/zoo-vault/` | Five notes: a class hierarchy, an individual, a property, and a class with an authored `iri`, `title` and `aliases`. |
| `fixtures/zoo.nt` | What `triplifier-md` 0.3.0 (dot-triples `99d74c8`) prints for those notes, unedited. |
| `../../crates/ontomark-wasm/tests/bridge_dot_triples.rs` | Runs the CONSTRUCT over the fixture with the engine's own SPARQL evaluator and checks the result against the engine's reading of the same five notes. |

## Identity, and where the two graphs join

Ontomark names a note without an authored IRI `urn:ontomark:page:<path minus .md>`; Dot Triples names it `urn:name:<name>`. The bridge mints Ontomark's form from the name, so the two coincide exactly for notes at the vault root and for any note that carries an `iri` (generic profile) or `resource` (estate profile) key, which both tools honour as written. A nested note without one is `urn:ontomark:page:Zoo/Dog` to Ontomark and `urn:ontomark:page:Dog` to the bridge. When the two graphs must join across folders, give those notes an `iri`, or reconcile on `rdfs:label`.

Properties are typed `owl:ObjectProperty`; the engine chooses `owl:DatatypeProperty` when a range is a datatype, which the bridge cannot see from the tokens alone.

## Running it

Over the fixture, with the Oxigraph CLI:

```sh
oxigraph load --location /tmp/zoo --file bridges/dot-triples/fixtures/zoo.nt
oxigraph query --location /tmp/zoo --query bridges/dot-triples/dot-triples-to-ontomark.rq --results-format ttl
```

Over your own vault: run the dot-triples CLI on each note, concatenate, and apply the same query. The result can be loaded next to `ontology.ttl` from `ontomark build`, or into the Oxigraph server the Dot Triples plugin's external mode points at, so one store answers both tools' questions.

## The other direction

Nothing is needed. `ontomark build` writes `ontology.ttl` and `ontology-inferred.ttl` as standard RDF; load them into the same store and Dot Triples' panels can query the reasoned hierarchy its own PIMO panel asks for (`rdfs:subClassOf+`), which Dot Triples alone has no source for.

## What Ontomark took from Dot Triples, and what it did not

Taken, with thanks, into [ontomark-obsidian](https://github.com/DreamLab-AI/ontomark-obsidian): the in-note SPARQL block that rewrites `__THIS__` and `[[Note]]` before running (its `ontomark-sparql` fence), and the decision to express evidence locations, when they are ever exported, with Web Annotation selectors as Dot Triples does. The decision record is [ADR-OM-006](../../docs/adr/ADR-OM-006-dot-triples-interop.md).

Not taken: inline `key :: value` fields. Ontomark asserts from frontmatter only and treats prose links as prose, which is what lets a diff be the whole review. Users with Dataview habits have Dot Triples for the rest of the vault, and this bridge to join the two.
