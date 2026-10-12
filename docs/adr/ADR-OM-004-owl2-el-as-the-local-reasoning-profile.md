# ADR-OM-004: OWL 2 EL is the local reasoning profile, and non-EL is named, not dropped

- Status: Accepted (records a decision already in force since `whelk-el` 0.1.0; written down on 12 October 2026 when the question was asked directly)
- Implementation: active. `ontomark-reason::profile` (refusal codes), `ontomark-compile` `NON_EL_VOCABULARY` (ADR-2126 Decision 2), engine `Classification::unsupported`.

## Question

The estate runs full-OWL tooling elsewhere. Was it a mistake for Ontomark to adopt only the OWL 2 EL profile?

## Decision

No. OWL 2 EL stays the profile for every local reasoning path (plugin engine, Loom, VisionClaw's vault compile), for three reasons that are measurable rather than tasteful:

1. **Tractability is the product constraint.** Subsumption in EL remains polynomial even with general concept inclusions (Baader, Brandt & Lutz, *Pushing the EL Envelope*, IJCAI 2005); most additional constructors make it EXPTIME-complete or worse. The plugin classifies inside a browser worker in resumable 25 ms slices (`ontomark-wasm` `DEFAULT_STEP_BUDGET`); that is only possible because the algorithm is a saturation, not a tableau. ELK classifies the roughly 300,000-concept SNOMED CT in under 10 s (Kazakov, Krötzsch & Simančík, JAR 2014); the full-OWL reasoners (HermiT, Pellet, Openllet) are JVM tableau systems with no Rust or WASM port and could not run in Obsidian at all.
2. **The vault grammar is EL.** Subsumption, class assertion, equivalence, disjointness (via `owl:Nothing`), existential restrictions, property hierarchies and domain/range are all EL++. The estate corpus (9,258 typed notes, 13,638 asserted `is-a`) authors nothing outside it.
3. **Full OWL has a home already.** Consistency audits with universal restrictions, cardinalities, `unionOf`, complement, inverse or functional properties belong to the estate's JVM/Python tooling, run on a server over a compiled bundle. Ontomark's job is to produce that bundle faithfully, not to reproduce HermiT in a text editor.

## The real risk, and why it is already closed

The failure mode worth guarding against is not "EL" but **silent weakening**: an authored axiom quietly dropped or approximated. Ontomark refuses that in three places, each with a stable code:

| Layer | Behaviour | Code |
|---|---|---|
| `ontomark-reason::profile::check` | An axiom outside EL is refused **whole**, never weakened, with the responsible constructor named (`ObjectAllValuesFrom`, …). `Classification::is_complete()` is `false` whenever anything was refused. | `EL_OUTSIDE_PROFILE` |
| same | An axiom that *is* EL but `whelk-el` does not implement is refused separately, so a different backend could take it. | `EL_NOT_IMPLEMENTED` |
| `ontomark-compile` vocabulary load | A profile vocabulary entry mapping a key onto a non-EL construct fails the load by name (ADR-2126 Decision 2). A vault cannot be *configured* into silent approximation. | `NON_EL_VOCABULARY` |

The engine carries refusals to the host as `Classification::unsupported` and marks the result incomplete; the plugin's index view shows that state. The `whelk-el` crate doc example demonstrates the path end to end (a universal restriction refused and named, `is_complete() == false`).

## Consequences

- A future "send this bundle to a full reasoner" feature is a host-driven, off-by-default remote call over the compiled bundle, the same shape as assisted ingest (ADR-OM-003), and its results would be a *second* classification shown alongside the local one, never merged into it silently.
- Property axioms the model lacks a form for (`rdfs:domain`, `rdfs:range` as axioms rather than annotations) are EL and belong in `ontomark-model` as a `PredicateForm`; that is a model gap, not a profile gap (see ADR-OM-003, engine boundary).
- Anyone proposing a non-EL authoring key must first add the construct to `ontomark-reason`'s refusal table with a name, so the author sees exactly what will not be reasoned locally.

## References

- Baader, Brandt & Lutz, *Pushing the EL Envelope*, IJCAI 2005 — `docs/GROUNDING-EVIDENCE.md` row 6a.
- Kazakov, Krötzsch & Simančík, *The Incredible ELK*, JAR 2014 — row 6b.
- OWL 2 Web Ontology Language Profiles, W3C Recommendation, §2 (EL).
- `crates/ontomark-reason/src/profile.rs`, `crates/ontomark-compile/src/input/vocabulary.rs` (`check_el_profile`), `crates/ontomark-wasm/src/reason.rs`.
