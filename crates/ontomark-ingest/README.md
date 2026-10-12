# ontomark-ingest

Assisted ingest for Ontomark: a pure, host-driven `plan`/`accept` contract
that turns a model's proposed names and values into evidence-bearing, guarded
Markdown patch candidates. Specified by
[ADR-OM-003](../../docs/adr/ADR-OM-003-native-assisted-ingest.md).

The crate never calls a model and performs no I/O. A host (the Obsidian
plugin, Loom, Agentbox) owns transport, credentials and the user's
confirmation; this crate owns everything deterministic in between.

```mermaid
sequenceDiagram
    participant H as Host (plugin, Loom, Agentbox)
    participant I as ontomark-ingest
    participant M as Model (host-supplied)
    participant P as ontomark-parse

    H->>I: plan(note, profile, model)
    I-->>H: ModelRequest {instructions, keys, chunks, schema, digest}
    H->>M: request as JSON
    M-->>H: ModelResponse[] {key, name or literal, evidence quote}
    H->>I: accept(note, request, responses)
    Note over I: digest check · locate evidence · ground names<br/>(exact → lexical tiers → suggestions) · XSD checks · kind guard
    I->>P: Patch::new(digest).set(key, value) then apply once to prove it
    P-->>I: Applied (discarded) or PatchError
    I-->>H: Candidates {patch?, accepted[], withheld[reason], diagnostics[]}
    H->>H: show diff preview; on confirm apply patch; undo stays available
```

```rust
use ontomark_ingest::{Ingest, ModelResponse, Options, ProposedAssertion};
use ontomark_model::{DeclarativeProfile, DocId, Model, ProfileSpec};
use ontomark_parse::Document;

let spec: ProfileSpec = serde_json::from_value(serde_json::json!({
    "id": "demo/1",
    "kinds": { "Class": "class" },
    "predicates": { "is-a": { "iri": "rdfs:subClassOf", "form": "sub_class_of" } }
})).unwrap();
let profile = DeclarativeProfile::new(spec).unwrap();

let graph = Document::parse("---\ntype: Class\niri: urn:x:Graph\n---\n");
let kg = Document::parse("---\ntype: Class\niri: urn:x:KG\n---\nA knowledge graph is a graph.\n");
let model = Model::build(&profile, [
    (DocId::new("Graph.md"), &graph),
    (DocId::new("Knowledge graph.md"), &kg),
]);

let ingest = Ingest::new(&profile, &model, Options::default());
let request = ingest.plan(&DocId::new("Knowledge graph.md"), &kg).unwrap();

// The host's model answers with names and verbatim evidence, never identifiers.
let response = ModelResponse { chunk: 0, assertions: vec![ProposedAssertion {
    key: "is-a".into(), value: "graph".into(), evidence: "is a graph".into(),
}]};
let out = ingest.accept(&kg, &request, &[response]).unwrap();
assert_eq!(out.accepted[0].written, "[[Graph]]");
let patched = kg.apply(out.patch.as_ref().unwrap()).unwrap();
assert!(patched.document.source().contains("is-a:\n- '[[Graph]]'\n"));
```

## What `plan` produces

A `ModelRequest` is plain, serialisable data:

- `instructions`: the prompt contract, carried as a versioned data file
  ([`prompts/facts-v1.md`](prompts/facts-v1.md)), not code;
- `keys`: the frontmatter keys the model may fill, derived from the profile's
  declared predicates, each with its predicate IRI, form, value kind (`name`
  or `literal` with datatype) and the values already present;
- `chunks`: the note body cut at headings, merged below `min_chunk_chars`
  and split above `max_chunk_chars`, each the exact byte slice of its span;
- `response_schema`: the JSON schema one `ModelResponse` must satisfy;
- `note.digest`: the source digest everything is bound to.

## What `accept` guarantees

- **Stale revisions are refused.** If the note changed since `plan`, the
  result is `IngestError::StaleRevision`, not a patch against the wrong bytes.
- **Names are grounded, never trusted.** Following SPIRES, the model gives
  names; the core resolves them against the vault's labels, aliases and note
  names, then the lexical tiers adapted from OntoCast's merge guards (exact,
  shared label, initials, label Jaccard, sequence ratio, token Jaccard). The
  strongest tier wins; a tie is `Ambiguous`; a name that differs from an
  entity's only by a conflicting initial (`Company S` against `Company T`) is
  an `InitialsConflict`; anything else is `Ungrounded` with Jaro-Winkler
  suggestions for the host to re-prompt with.
- **Every accepted value has evidence.** The model's quote is located in the
  chunk (exactly, then whitespace-collapsed, then ASCII case-insensitively)
  and reported as a byte span and a UTF-16 span. No quote, no value.
- **Deterministic checks guard the write.** Literals are checked against
  their XSD datatype after trimming, unquoting and removing thousands
  separators (each repair is a diagnostic). A grounded entity must have the
  kind the predicate form needs. Self-references, values already present and
  a differing single value on a data key are withheld with the reason.
- **One patch, already proven.** Accepted values become one
  `ontomark_parse::Patch` bound to the digest, keys in profile order,
  existing lists extended rather than replaced. The patch is applied once in
  memory to prove it before it is returned. Applying it a second time is
  refused by the parser's own digest guard.
- **Confidence starts low.** Every candidate carries `confidence: 0.5` and
  `method: "inferred"`, the retired RDF-to-Logseq adapter's starting
  provenance. Only a reviewer raises them.

## Scope of this slice

Facts only, against the vocabulary the vault already has: no new classes or
properties are proposed. A second model pass (the critic), ontology growth,
numeric and unit repair and long-document preprocessing are deferred in
ADR-OM-003, not rejected.

## Portability

No filesystem, clock, network or threads. Builds for
`wasm32-unknown-unknown` unchanged; CI compiles it for that target. Native and
WASM parity of `Candidates` is a byte-equality property over recorded
`ModelResponse` fixtures.

## Attribution

Methods are adapted from [OntoCast](https://github.com/growgraph/ontocast)
(growgraph, Apache-2.0, reviewed at `abb8d7b8d6db1a765d4f091a7423937c2cbc5d78`)
and from SPIRES (Caufield et al., *Bioinformatics* 2024; OntoGPT,
BSD-3-Clause). No upstream code is copied; see [NOTICE](NOTICE) and the
workspace [UPSTREAMS](../../docs/UPSTREAMS.md). Dependencies: `strsim` (MIT)
and `unicode-normalization` (MIT OR Apache-2.0).

## Licence

MIT OR Apache-2.0.
