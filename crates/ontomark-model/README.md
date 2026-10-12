# ontomark-model

Typed ontology entities and assertions derived from losslessly parsed Markdown,
with profile-defined identity, kinds, predicates, visibility and lifecycle.

If `ontomark-parse` owns the format, this crate owns the *meaning*: it reads
`Document`s through a `Profile` and produces a read-only `Model`. The model is
derived, never edited; changes go back to Markdown through `ontomark_parse::Patch`.

## Rules the model enforces

These are the decisions that make a vault safe to reason over, and the crate
refuses rather than guesses on each of them:

- **Identity is an IRI**, stored verbatim from the page header. The model never
  mints one, re-prefixes one or derives one from a title. The document id,
  label, aliases and publication URL are separate fields. A page with no
  identity is reported, not given one.
- **Kinds are explicit.** `Kind::Class`, `Kind::Individual` and
  `Kind::Property` (object, data or annotation). An undeclared type is refused,
  never coerced to a class.
- **Predicates are full IRIs** chosen by the profile per frontmatter key, each
  with one `PredicateForm`: subclass, existential restriction, class, object
  or data assertion, annotation, SKOS relation, equivalence, disjointness,
  sameness or sub-property. The key `subclass-of` means something because the
  profile says so, not because the model guessed from the name.
- **References resolve to one of four states**: `Resolved`, `Ambiguous`,
  `Unresolved` or `External`. A wikilink that matches two pages is `Ambiguous`,
  never a first match. A link to a page the vault does not have is
  `Unresolved`, never a synthesised target.
- **Every statement carries a `Disposition`**: in the supported reasoning
  fragment, preserved without reasoning, or rejected with a reason.
  `Model::axioms` selects the supported fragment for the reasoner.
- **Corpus policies are not logic.** Subclass acyclicity and duplicate-IRI
  claims are `PolicyFinding`s. They are reported alongside, and kept separate
  from, OWL inconsistency.

```rust
use ontomark_model::{DeclarativeProfile, DocId, Kind, Model, Resolution};
use ontomark_parse::Document;

let animal = Document::parse("---\ntype: Class\niri: urn:x:Animal\n---\n");
let dog = Document::parse(
    "---\ntype: Class\niri: urn:x:Dog\nsubclass-of: [\"[[Animal]]\"]\n---\n",
);
let model = Model::build(
    &DeclarativeProfile::generic(),
    [(DocId::new("Animal"), &animal), (DocId::new("Dog"), &dog)],
);

let profile = DeclarativeProfile::generic();
let dog_entity = model.entity_of(&DocId::new("Dog")).unwrap();
assert_eq!(dog_entity.kind, Kind::Class);
assert_eq!(dog_entity.iri.as_str(), "urn:x:Dog");
// A reference target (the text inside a wikilink) resolves to exactly one
// page, by the profile's rules, or says why not.
let reference = model.resolve_with(&profile, "Animal");
assert!(matches!(reference.state, Resolution::Resolved(_)));
assert!(matches!(model.resolve_with(&profile, "Nowhere").state, Resolution::Unresolved));
```

## Profiles

A `Profile` is the vocabulary and policy for a vault: which keys carry which
predicates, how `public:` is read, what a missing identity means, which
lifecycle statuses exist. `DeclarativeProfile::generic()` contains no estate
rules and is what the Obsidian plugin uses by default; the DreamLab estate
injects its own. The trait is small enough to implement for another corpus
convention in an afternoon.

`mint` provides deterministic IRI minting *for callers that choose to use it*
(the estate profile does, for working notes); the model itself never calls it.

Builds for native and `wasm32-unknown-unknown`; no I/O.

Licensed under MIT OR Apache-2.0. See `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
