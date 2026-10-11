# WP-B API: `whelk-el` and `ontomark-reason`

Branch `wp-b-reason`. Status: stable for consumers; the axiom input type changes
once `ontomark-model` publishes its axioms (an adapter is added, and the local
types remain as the wire format).

## Crates

| Crate | Role | Targets | Features |
|---|---|---|---|
| `whelk-el` | Whelk's EL saturation rules over owned string-IRI axioms. No I/O, clocks or threads | native, `wasm32-unknown-unknown` | `parallel` (rayon, `classify_parallel`); `horned-owl` (native adapter) |
| `ontomark-reason` | OWL 2 EL profile check, classification facade, generation-guarded jobs | native, `wasm32-unknown-unknown` | `corpus-bench` (native benchmark example only) |

`ontomark-reason` depends on `whelk-el` and `serde`. It does not depend on
`ontomark-parse`.

## `ontomark-reason`

### Input

`axiom::Axiom` is OWL 2's logical vocabulary, so that a construct outside EL can
be named when it is refused. IRIs are plain strings and are never rewritten.

```rust
use ontomark_reason::axiom::{Axiom, ClassExpression as C, Property, Characteristic};

let axioms = vec![
    Axiom::DeclareClass("urn:ngm:class:arm".into()),
    Axiom::sub_class("urn:ngm:class:arm", "urn:ngm:class:robot"),
    Axiom::SubClassOf { sub: C::class("urn:ngm:class:arm"),
                        sup: C::some("urn:ngm:prop:hasPart", C::class("urn:ngm:class:gripper")) },
    Axiom::EquivalentClasses(vec![C::class("urn:ngm:class:gripping-robot"),
        C::And(vec![C::class("urn:ngm:class:robot"),
                    C::some("urn:ngm:prop:hasPart", C::class("urn:ngm:class:gripper"))])]),
    Axiom::DisjointClasses(vec![C::class("urn:a"), C::class("urn:b")]),
    Axiom::Characteristic { property: Property::named("urn:ngm:prop:partOf"),
                            characteristic: Characteristic::Transitive },
];
```

Serde form is externally tagged in snake case, for example
`{"sub_class_of":{"sub":{"class":"urn:a"},"sup":{"class":"urn:b"}}}`; the 16
files in `crates/ontomark-reason/tests/fixtures/` are worked examples.

### Mapping from the estate frontmatter (for `spec-compile`)

| Estate (vocabulary.yaml / build::turtle) | Axiom |
|---|---|
| page `type: Class`, `resource` | `DeclareClass(iri)` |
| `is-a` (`rdfs:subClassOf`) | `SubClassOf { Class, Class }` |
| relation with `restriction: true` | `SubClassOf { Class, Some { property, Class } }` |
| `disjoint-with` (`owl:disjointWith`) | `DisjointClasses([a, b])` |
| `defines-as` (`owl:equivalentClass`) | `EquivalentClasses([Class, And([...named, Some{..}...])])`, or the lone conjunct |
| `owl:TransitiveProperty` | `Characteristic { Transitive }` |
| `rdfs:subPropertyOf` | `SubPropertyOf` |

`examples/corpus_bench.rs` is a working reference of exactly this mapping from
the built `data/ontology.ttl`.

### Classification

```rust
use ontomark_reason::{classify, classify_with, Options, ExistentialPolicy};

let c = classify(&axioms);                       // All, to the fixpoint
let c = classify_with(&axioms, Options { existentials: ExistentialPolicy::Relevant,
                                         max_steps: Some(50_000_000) })?;  // Err(Stopped::OverBudget{..})
```

`Classification` (serde, every list sorted and deduplicated):

| Field | Meaning |
|---|---|
| `profile` | `"owl2-el/whelk-el"` |
| `existentials` | the policy used |
| `subsumptions` | every entailed `(sub, sup)` between distinct named classes; no `owl:Thing`/`owl:Nothing` |
| `inferred` | `subsumptions` minus told `SubClassOf(Class, Class)`: the content of `ontology-inferred.ttl` |
| `unsatisfiable` | classes entailed `⊑ owl:Nothing` |
| `root_causes` | unsatisfiable classes without an unsatisfiable *asserted* (`SubClassOf(Class, Class)`) superclass: the `WHELK_INCONSISTENT` rule. Under `All`, a class made unsatisfiable only through a restriction's filler is also listed |
| `disjointness_violations` | `{class, disjoint: (a, b), axiom}` for each unsatisfiable class under both sides of a `DisjointClasses` |
| `refusals` | `{index, kind, construct, reason}`; `code()` gives `EL_OUTSIDE_PROFILE` or `EL_NOT_IMPLEMENTED` |
| `classes`, `existentials_dropped`, `steps` | counts; `steps` varies run to run (hash order) and is excluded from parity |

`is_complete()` is false when anything was refused; refused axioms are left out
whole, never weakened.

`ExistentialPolicy::Relevant` generalises the estate reasoner's
`Restrictions::{Skip, Relevant}`: it drops `Named ⊑ ∃R.D` axioms whose
properties cannot reach a left-position restriction through sub-properties,
chains or transitivity. Named subsumptions are unchanged (on the corpus:
identical `inferred` set to the estate, 81,238 pairs); unsatisfiability is not
carried back from an unsatisfiable filler through a dropped axiom (the filler
itself is still reported). Use `All` for completeness, `Relevant` for corpus
builds and interactive use.

### Profile check alone

`ontomark_reason::check(&axioms) -> ProfileReport { accepted: Vec<(index, ElAxiom)>, refusals }`
and `profile::check_one(&axiom)`. Refused constructs: `ObjectAllValuesFrom`,
`Object{Min,Max,Exact}Cardinality`, `ObjectUnionOf`, `ObjectComplementOf`,
`ObjectOneOf` (more than one individual), `ObjectInverseOf`,
`DisjointUnion`, `DisjointObjectProperties`, `InverseObjectProperties`,
`Functional/InverseFunctional/Symmetric/Asymmetric/IrreflexiveObjectProperty`
(outside EL); `ObjectPropertyRange`, `ReflexiveObjectProperty`, `ObjectHasSelf`,
`NegativeObjectPropertyAssertion`, `SameIndividual`, `DifferentIndividuals`,
`HasKey`, data-property constructs (EL, not implemented). `ObjectHasValue`
and a one-individual `ObjectOneOf` are accepted and reasoned as nominals.

### Worker contract (EXP-06, for `spec-plugin`)

```rust
use ontomark_reason::session::{Session, Job, Step};

let mut session = Session::new();          // main thread / controller
let ticket = session.begin();              // on every source change; cancels the previous ticket
// in the worker (Job is !Send: build it where it runs; Ticket is Send):
// Relevant + a budget: All on the full corpus is the configuration the estate
// measured as not finishing (see Evidence), so a host never uses it unbounded.
let options = Options { existentials: ExistentialPolicy::Relevant, max_steps: Some(5_000_000) };
let mut job = Job::new(ticket, &axioms, options);
loop {
    match job.step(20_000) {               // one slice; yield to the event loop between slices
        Step::Pending { steps } => { /* report progress, check messages */ }
        Step::Done(outcome) => { /* post outcome (serde) to the controller */ break }
        Step::Stopped(why) => { /* Cancelled or OverBudget: drop it */ break }
        Step::Spent => unreachable!(),
    }
}
// controller:
session.accept(outcome)?;                  // Err(Rejected::Stale{..}) if a newer generation began,
                                           // Err(Rejected::ForeignSession{..}) if another Session issued the ticket
```

`CancelToken` is an `Arc<AtomicBool>`: a host thread can cancel. In a
single-threaded wasm worker, cancellation arrives as a message handled between
slices (call `ticket.cancel.cancel()` or just drop the job). An outcome is
accepted only by the session that issued its ticket (`SessionId`, unique
within the process) and only for that session's current generation, so an
old result that raced to completion cannot replace a newer one, and a
result from a discarded session cannot be mistaken for a current one.

Latency: a slice of 20,000 saturation steps is about 25 ms on the corpus
natively (302,861 steps in about 340 ms), but two phases are not sliced:
`Job::new` (profile check, policy filter, translation, role indexing; about
40 ms on the corpus) and the `Done` slice, which runs `finish` (collecting and
sorting about 125,000 pairs; about 200 ms, the remainder of the 589 ms median). Cancellation is noticed at the
next slice boundary, so its worst-case latency is one of those phases, not
25 ms. Expect wasm32 to be slower than these native figures.

## `whelk-el`

`ElAxiom`/`ClassExpr` (owned, `Send`), `translate`, `classify(&[ElAxiom]) ->
Vec<(String, String)>` (all named pairs incl. reflexive, `⊤`, `⊥`),
`reasoner::Saturation::{new, append, run(budget) -> Progress, into_state}`,
`ReasonerState::{named_subsumption_ids, subsumers}`, `assert`, `assert_append`,
`components`, and `classify_parallel` (feature `parallel`).

Defects fixed against whelk-rs 79a1ee2, each pinned by a test that fails on
the upstream behaviour:

| Defect | Effect upstream | Test |
|---|---|---|
| `rule_bottom_left` tested the subclass for `⊥` | an unsatisfiable filler derived after its link left the subject satisfiable (missed unsatisfiability) | `an_unsatisfiable_filler_makes_its_restricted_class_unsatisfiable` |
| chain ids joined with `:` | `[p:q, r, …]` and `[p, q:r, …]` shared an intermediate role: **unsound** links between unrelated chains | `colliding_chain_prefixes_do_not_share_an_intermediate_role` |
| composition pruning on any distinct super-role | equivalent roles pruned each other; chain lost | `a_chain_into_equivalent_roles_survives_pruning` |
| `group_by` over a hash set | a chain super-property could be dropped | `a_chain_with_two_super_properties_implies_both` |
| complement rule asserted `C ⊑ ⊥` for every `¬C` | unsound; removed with the unused disjunction/self variants (non-EL input is refused instead) | profile refusal of `ObjectComplementOf` |

Consequence for consumers moving from whelk-rs (WP-D): under `All`, whelk-el
can report unsatisfiable classes whelk-rs missed. The estate today runs
Skip/Relevant with no definitions, where nothing changes (corpus parity
above), and its only chains are transitivity (length 2), so the chain-id
defect never fired there.

## Evidence

| Claim | Command | Result |
|---|---|---|
| EXP-01 native | `cargo test -p ontomark-reason --test exp01` | 16 fixtures meet expectations and equal goldens |
| EXP-01 wasm32 | `just wasm-test` (wasm-bindgen-test-runner 0.2.127, Node) | same 16 goldens; sorted digest lists identical, sha256 `ebdf57c1…` |
| EXP-01 negative | `the_closure_only_substitute_fails_exp01` | substitute fails 13/16, every definition/restriction/disjointness fixture |
| EXP-06 | `cargo test -p ontomark-reason --test exp06` (native and wasm32) | 8 passed |
| Upstream conformance | `WHELK_RS_INFERENCE_TESTS=… cargo test -p whelk-el --all-features --test upstream_owx` | skeletons 5+2, go-extract 694 entailments |
| Corpus | `cargo run --release -p ontomark-reason --features corpus-bench --example corpus_bench -- data/ontology.ttl data/ontology-inferred.ttl` | 9,936 classes; Relevant: classify median 589 ms, saturation 336 ms (estate whelk-rs 360 ms); `inferred` = estate's 81,238, 0 missing, 0 extra |
| Corpus, All | same, mode `all` | did not finish within 900 s wall clock (14 min 51 s CPU; 23,040 existentials over 15 properties, 2 transitive), as the estate measured for whelk-rs `Include`. Hosts must use `Relevant` or a `max_steps` budget on this corpus |
| `parallel` on the corpus | `corpus_bench` components line | 5 components, the largest 22,763 of 22,771 axioms: the taxonomy is one component, so `classify_parallel` gives no speed-up here; it helps only on inputs with independent modules |

## Audit

Cross-vendor audit by Codex (gpt-6-astra) in two rounds, with source given
inline because the Codex sandbox cannot start in this container. Round 1
confirmed the `rule_bottom_left` fix and refuted four claims (chain pruning
with equivalent roles, `⊥ ⊑ B` components, Relevant dropping whole
conjunctions and uninitialised classes, generation wrap-around). Round 2
refuted three more (chain-id collision, `DisjointClasses(⊤, ⊤)` components,
cross-session outcomes) and confirmed the rest. All seven are fixed, each
with its counterexample as a regression test (`whelk-el/tests/entailments.rs`,
`ontomark-reason/tests/audit_regressions.rs`, `exp06.rs`) that fails on the
earlier code. A third round on the final code has not been run.

Toolchain note: the lock pins `wasm-bindgen = 0.2.127` and
`wasm-bindgen-test = 0.3.77` to match the baked `wasm-bindgen-test-runner`;
any crate here using `wasm-bindgen` must stay on 0.2.127 until the CLI moves.
