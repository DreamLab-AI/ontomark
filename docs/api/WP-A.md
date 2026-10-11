# WP-A API: ontomark-parse and ontomark-model

Owner: `spec-model`. Branch `wp-a-model-parse`. Status per crate below; code
against these signatures. Full rustdoc: `cargo doc -p ontomark-parse -p ontomark-model --no-deps --open`.

## ontomark-parse — STABLE (commit 7176583)

Owns the Markdown format. Portable (no fs/clock/net/threads); builds for
`wasm32-unknown-unknown`. Feature `serde` derives Serialize/Deserialize on the
data types (enable it for wasm bindings).

```rust
// Reading
let doc = Document::parse(text);              // infallible, lossless
doc.source() -> &str                          // == input, always
doc.digest() -> SourceDigest                  // sha256 of source; Display "sha256:<hex>"
doc.frontmatter() -> Option<&Frontmatter>
doc.value(key) -> Option<&Value>              // top-level frontmatter value
doc.body() -> &str; doc.body_span() -> Span; doc.body_line() -> usize (1-based)
doc.body_index() -> &BodyIndex                // links, footnote_refs, footnote_defs, inline_footnotes, code
doc.line_col(byte_offset) -> (line, col)      // 1-based, col in chars

Frontmatter { open_fence, content, close_fence: Span, state: FrontmatterState,
              entries: Vec<Entry>, gaps: Vec<Span>, style: BlockStyle }
  .get(key) -> Option<&Entry>; .value(key); .has(key); .keys(); .is_parsed()
FrontmatterState::{Parsed, Unsupported(String), Malformed(String)}
Entry { key, key_span, span /* whole lines */, value: Value, items: Vec<Span> /* block-seq items */ }
  .raw(source) -> &str
  .wikilinks(source) -> Vec<FrontmatterLink { link, item, span, recovered }>
Value::{Null, Bool, Int(i64), Float(String /*lexical*/), String, Seq(Vec<Value>), Map(IndexMap<String,Value>)}
  .as_str/.as_bool/.as_f64/.as_seq/.as_map/.scalar_text/.texts/.type_name
Wikilink { target, subpath: Option<Subpath::{Heading,Block}>, alias, embed } ; Wikilink::parse("[[..]]")
BodyLink { span, link }; FootnoteRef { span, label }; FootnoteDef { span, label, text }
Span { start, end } (byte offsets into the document source)
doc.utf16_index() -> Utf16Index; .offset(byte) -> Option<usize>; .byte(utf16) -> Option<usize>; .span(Span) -> Option<(usize, usize)>  // JS hosts; None off a char boundary, inside a surrogate pair, out of range or reversed

// Writing — the only write path
let patch = Patch::new(doc.digest())
    .set("key", value)                        // in place if present, else Placement::End
    .set_at("key", value, Placement::After("title".into()))  // or ::Start / ::End
    .remove("key")
    .replace_body(span, expected_text, replacement);
let Applied { document, receipt } = doc.apply(&patch)?;  // Result<Applied, PatchError>
Receipt { before, after: SourceDigest, edits: Vec<Edit { op, before: Span, after: Span }>, preserved_bytes }
PatchError::{DigestMismatch, FrontmatterLocked, AnchorNotFound, InvalidKey, Unrepresentable, Overlap, BadRange, BodyGuard, Verification}
emit_entry(key, &value, &BlockStyle) -> String; emit_scalar(&str) -> String   // house-style YAML
```

Guarantees (tested): every byte outside the edits is identical; untouched keys
keep exact bytes and values; set keys read back equal; existing keys are never
reordered; column-0 comments belong to no key and survive single-key edits;
no-op sets/removes produce zero edits (idempotent); a body patch can neither
touch nor create frontmatter; Unsupported/Malformed blocks round-trip but
refuse key patches. Values are emitted in the corpus's serde_yaml style
(indentless `- '[[X]]'`, `[]`, single quotes), following the block's CRLF and
sequence indentation.

EXP-03 (2026-10-11, visionGraph b5e36abd1): 9,244/9,244 public pages
round-trip, match an independent yaml-rust2 load, and pass five guarded
patches each. Every one of the 163,645 real entries (maps, sequences of maps,
flat lists) re-emits and reads back equal; 163,643 re-emit byte-identically,
so a no-semantic-change rewrite produces an essentially empty diff.
Run: `just exp03 ../visionGraph/knowledge/pages`.

## ontomark-model — STABLE

Owns the typed model (ADR-2015). Depends only on ontomark-parse, indexmap,
serde, thiserror; builds for wasm32. All public types are serde-serialisable.

```rust
// Identity
Iri::parse(s) -> Result<Iri, IriError>        // absolute IRI, verbatim; scheme >= 2 chars
PrefixMap::standard() /* rdf rdfs owl xsd skos dcterms prov */ .insert(p, ns)
  .expand("owl:Class" | "urn:x:y") -> Result<Iri, IriError>   // declared prefix wins; else absolute IRI kept
  .compact(&iri) -> Option<String>
DocId::new("dir/Note.md")  // "dir/Note"; .name() == "Note"   (document identity, distinct from IRI)

// Kinds
Kind::{Class, Individual, Property(PropertyKind::{Object, Data, Annotation})}; .owl_iri()

// Profile: estate rules are injected
trait Profile { id, prefixes, kind(&Document) -> KindDecision{Entity(Kind)|NotOntology|Refused},
  identity(&Document) -> IdentityDecision{Iri|Missing|Invalid}, label, aliases, is_public,
  lifecycle(&Document) -> Lifecycle{Declared|Unknown|Absent}, lifecycle_def,
  predicate(key) -> Option<PredicateDef{key, iri, form, datatype, domain, range, transitive}>,
  is_reserved(key), reference_candidates(target) -> Vec<Iri> /*default none*/,
  publication_url(&Iri) -> Option<String> /*default none*/, cycle_policy() -> CyclePolicy }
PredicateForm::{SubClassOf, ClassAssertion, Existential, ObjectAssertion, DataAssertion,
  Annotation, Skos(SkosRelation), EquivalentClass, DisjointWith, SameAs, SubPropertyOf}
ProfileSpec (serde, deny_unknown_fields): id, prefixes, type_key="type", kinds{value->KindSpec},
  untyped, identity_key="iri", title_key, aliases_key, public_key, lenient_booleans,
  lifecycle_key="status", lifecycle[LifecycleDef{id, reasoned, published}],
  predicates{key->PredicateSpec{iri, form, datatype?, domain?, range?, transitive}},
  curie_keys=true, reserved[], reference_namespaces[], publication?{namespace, base_url, suffix},
  cycle_policy{allow|warn|refuse}
DeclarativeProfile::new(spec)? ; DeclarativeProfile::generic()   // "generic/1", no estate rules

// Model
let m = Model::build(&profile, docs /* IntoIterator<(DocId, &Document)> */);
m.entities: IndexMap<Iri, Entity{iri, kind, document, label, aliases, lifecycle, public,
                                publication_url, statements, unmapped}>
m.documents: IndexMap<DocId, DocumentRecord{id, digest, disposition, public}>
  DocDisposition::{Entity(Iri), NotOntology, NoIdentity, Refused(String), IdentityConflict(Iri)}
m.conflicts: BTreeMap<Iri, Vec<DocId>>      // duplicate claims: no entity, no first-wins
m.diagnostics: Vec<Diagnostic{severity, code /* codes::* */, document, message}>
m.policy: Vec<PolicyFinding{code: "SUBCLASS_CYCLE", severity, members}>  // policy, not logic
Statement{predicate: Iri, form: StatementForm, object: Object::{Reference(Reference)|Literal(Literal)},
          disposition: Disposition::{Supported|PreservedNotReasoned(why)|Rejected(why)},
          origin: Origin{document, key, item, span}}
Reference{written, link: Option<Wikilink>, state: Resolution, document: Option<DocId>}
Resolution::{Resolved(Iri), Ambiguous(Vec<Iri>), Unresolved, External(Iri)}
// Reasoner input (WP-B): EL axioms from Supported statements only, plus profile-declared transitivity
m.axioms(&profile) -> Vec<SourcedAxiom{axiom: Axiom, origin: Option<Origin>}>
Axiom::{SubClassOf{sub,sup}, EquivalentClasses{classes}, DisjointClasses{classes}, SubPropertyOf{sub,sup},
  Transitive{property}, ClassAssertion{individual,class}, ObjectAssertion{subject,property,object}, SameIndividual{individuals}}
ClassExpr::{Named(Iri), Thing, Nothing, And(Vec), Some{property, filler: Box}}   // Iri::as_str() is free
Profile::predicates() -> Vec<PredicateDef>   // default empty
// The one minter (explicit creation only; never called when reading a vault)
Model::mint_iri(&self, profile: &dyn Profile, kind: Kind, title: &str) -> Result<Iri, MintRefusal>  // THE entry point: title guard, then the profile's rule, then the claimed-IRI guard
Profile::mint_iri(&self, kind, title, ctx: &MintContext{model: Option<&Model>}) -> Result<Iri, MintRefusal>  // the rule only; default Err(NoRule)
MintRefusal::{NoRule, EmptySlug, InvalidIri(String), IriTaken(Iri), TitleTaken{title, existing: Vec<Iri>}}  // serde tag "refusal"
Model::entities_named(&str) -> Vec<Iri>  // labels, aliases, note names; trimmed, case-insensitive
ProfileSpec.mint: Option<MintSpec{class, individual, property: Option<namespace>}>  // namespace(kind) + slugify(title)
slugify(&str) -> String   // = vault_core::slug::slugify, the current estate minter. 418 of 9,244 public resources predate it
                          // (case/digit-boundary rule, e.g. 2-d-li-dar). TitleTaken stops a second IRI for those titles.
m.is_claimed(&iri) -> bool
m.resolve_with(&profile, target) -> Reference;  m.entity_of(&DocId);  m.resolution_counts()
```

Semantics consumers rely on:

- One authored value, one statement, one form. `Existential` on a class is
  `C ⊑ ∃P.D`; the same predicate on an individual is an object assertion. No
  second direct triple is implied (defect 5). WP-C decides Turtle shape from
  `form` + `disposition` only.
- `Supported` = in the EL fragment with a resolved, kind-compatible target.
  Annotation, SKOS, data, external, unresolved, ambiguous and non-reasoned
  lifecycle → `PreservedNotReasoned`; kind mismatches → `Rejected`. WP-B feeds
  only `Supported` statements to Whelk.
- Resolution never guesses: a CURIE/IRI is an identity (`Resolved` if declared,
  else `External`); names (label, alias, note name, path, profile candidates)
  give exactly one → `Resolved`, several → `Ambiguous`, none → `Unresolved`.
  IRI tails are never indexed.
- Body wikilinks never become statements; frontmatter keys the profile does
  not declare are kept in `Entity::unmapped` with an `UNMAPPED_KEY` info.
- Typed pages without an identity header get `NoIdentity`; nothing is minted.

Corpus run (scratch estate-like spec derived from vocabulary.yaml; the real
estate profile is WP-C's): 9,244 public pages → 9,244 entities, 0 conflicts,
references resolved 133,316 / ambiguous 9 / unresolved 20,180 / external 99,
0.9 s release. Including the 42 non-public pages moves unresolved to 20,211,
so the unresolved figure is not a public-projection effect. Run: `ONTOMARK_CORPUS=… ONTOMARK_PROFILE=spec.json cargo test
--release -p ontomark-model --test model_corpus -- --nocapture`.

## Audit status

| Gate | Status |
|---|---|
| Cross-vendor evidence audit (Codex, `gpt-6-astra`, EDD step 4) | **NOT RUN, pending.** On 2026-10-11, `codex exec` exited with no output (refresh token expired, reproduced by team-lead). Re-run on this branch once `codex login` is restored. |
| Same-family adversarial review (Claude Opus, `code-reviewer`, at a55f80c) | **Degraded and supplementary.** It does not substitute for the Codex audit. It confirmed 5 defects plus 6 minor ones by running falsifiers. All 11 now have a regression test (`ontomark-parse/tests/review_regressions.rs`, plus 3 tests in `ontomark-model/tests/adr2015.rs`). In `177e5b9` the `Value::Float("1_0.0")` item was not actually fixed: its emitter change was a no-op, because the reader and the emitter already agree. It is now a named refusal, `PatchError::Unrepresentable`. **Scope:** the review covered `a55f80c`. |
| Same-family review, second pass (Claude Opus, `code-reviewer`, at 177e5b9; `axiom.rs`, `utf16.rs`, `mint.rs`) | **Degraded and supplementary.** It confirmed 6 findings, and each is fixed with a regression test. A predicate IRI whose page is a data property or a class was projected as an object property: now Rejected, with no Transitive axiom. Data⊑Data was projected as SubObjectPropertyOf: now PreservedNotReasoned. The axiom doc claim is narrowed. Utf16Index wrapped at 4 GiB (now an asserted limit), accepted a reversed span (now None), and had no reverse direction (`byte()` added). Not falsified: minting determinism, refusals, Unicode slugs, and the forward UTF-16 mapping. Not falsified: lossless tiling (C1), the char-to-byte span mapping, digest and lock guards, the emitter on about 80 adversarial strings, IRI-tail isolation, and one form plus one disposition per statement. |
