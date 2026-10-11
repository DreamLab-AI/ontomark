# ontomark-model

Typed ontology entities and assertions derived from losslessly parsed Markdown,
with profile-defined identity, kinds, predicates, visibility and lifecycle.
Identity is a stored IRI. Missing identities, ambiguous references and duplicate
IRI claims are reported without silently minting or choosing a first claimant.

```rust
use ontomark_model::{DeclarativeProfile, DocId, Kind, Model};
use ontomark_parse::Document;
let document = Document::parse("---\ntype: Class\niri: urn:Example\n---\n");
let model = Model::build(&DeclarativeProfile::generic(), [(DocId::new("Example"), &document)]);
assert_eq!(model.entities.values().next().unwrap().kind, Kind::Class);
```

Each statement records its source and whether it is supported, preserved without
reasoning, or rejected. `Model::axioms` selects the supported logical fragment;
diagnostics and corpus policy findings remain separate from OWL inconsistency.
The generic profile contains no estate rules. Inject an estate profile when
working with a corpus that has its own metadata and governance conventions.

Licensed under MIT OR Apache-2.0. See `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
