# Evidence: W3C Turtle round trip and OWL 2 EL profile conformance

Worker WP-C (tests). Repository `/home/devuser/workspace/ontomark-wp-c`, branch `wp-c-compile`,
`CARGO_TARGET_DIR=/home/devuser/workspace/.tmp/ontomark-target-w3c`. Final run started 2026-10-11T18:41:52Z.

## Pins

```
RDF_TESTS_COMMIT=42e6bb1d254bc10b3bb849e0a6872afea9ff4da3
RDF_TURTLE_MANIFEST_SHA256=52b5667f7c85743b06ab0bbee3f5d74cf4fd9c117b98cde94ad66bb5c9489b17
OWLAPI_COMMIT=b61ebe2da83daceebb3e7ba7afbd2582c9240c33
(digests of all nine OWL API case files: conformance/w3c/PINS)
```

`conformance/w3c/fetch.sh` (2026-10-11T18:24Z): `rdf-turtle: 459 files at 42e6bb1d...`, `owl2-profile: 9 files at b61ebe2d...`.
The OWL 2 profile cases are the OWL API's packaging (`owlcs/owlapi`), not the W3C repository: the W3C
test-case site is no longer served.

## Turtle writer: W3C RDF 1.1 Turtle suite and property tests

`cargo test -p ontomark-compile --test w3c_turtle -- --nocapture`

```
test bug_leading_hyphen_local_name_is_not_valid_turtle ... ignored, BUG: turtle::shorten accepts a leading `-` in a local name, but PN_LOCAL cannot start with `-`
test bug_random_graphs_round_trip_unrestricted ... ignored, BUG: shorten() emits `a` for rdf:type in subject/object/datatype position and `p:-x` for locals starting with `-`
test bug_rdf_type_outside_predicate_position_is_not_valid_turtle ... ignored, BUG: turtle::shorten returns the keyword `a` for rdf:type in every position, but `a` is only legal as a predicate
test language_and_datatype_forms_stay_distinct ... ok
w3c_turtle[generic]: 313 tests: pass=219 fail=0 unrepresentable=0 parser-rejects=0 n/a=94
w3c_turtle[estate]: 313 tests: pass=219 fail=0 unrepresentable=0 parser-rejects=0 n/a=94
test w3c_turtle_round_trip ... ok
test property_unicode_and_escapes_in_literals ... ok
test property_random_graphs_round_trip ... ok
test result: ok. 4 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 1.12s
```

Per profile (generic and estate prefixes): 313 manifest tests = 145 `TestTurtleEval` + 74 `TestTurtlePositiveSyntax`
(219 applicable) + 94 `TestTurtleNegativeSyntax` (N/A for a writer). pass 219, fail 0, unrepresentable 0, parser rejects 0.
The expected-vs-written comparison is under RDFC-1.0 canonicalisation (`oxrdf`, `rdfc-10`).

Known writer bugs, ignored tests deliberately run with `-- --ignored` (they fail, as intended):

```
test bug_rdf_type_outside_predicate_position_is_not_valid_turtle ... FAILED
test bug_leading_hyphen_local_name_is_not_valid_turtle ... FAILED
test bug_random_graphs_round_trip_unrestricted ... FAILED
```

## OWL 2 EL profile

`cargo test -p ontomark-compile --test owl2_el_profile -- --nocapture` (the FINDING test is ignored in a plain run)

```
test fixture_declares_every_class_it_uses ... ignored, FINDING: ontology.ttl uses undeclared classes as rdfs:subClassOf targets (not OWL 2 DL typed); EL axioms are unaffected
test annotation_values_may_use_any_datatype ... ok
owl2-profile All   expect EL=true  (DL ontology: true ): cases=27 agree=27 disagree=0 unparsable=0 functional-syntax documents not classified=5
test el_allows_the_specified_constructs ... ok
test el_rejects_the_unsupported_constructs ... ok
owl2-profile ELDL  expect EL=true  (DL ontology: true ): cases=7 agree=7 disagree=0 unparsable=0 functional-syntax documents not classified=3
owl2-profile NOQL  expect EL=true  (DL ontology: true ): cases=14 agree=14 disagree=0 unparsable=0 functional-syntax documents not classified=9
owl2-profile NORL  expect EL=true  (DL ontology: true ): cases=8 agree=8 disagree=0 unparsable=0 functional-syntax documents not classified=0
owl2-profile NOEL  expect EL=false (DL ontology: true ): cases=8 agree=8 disagree=0 unparsable=0 functional-syntax documents not classified=6
estate-small fixture: 2252 axioms
  AnnotationAssertion                           761  flagged 0
  ClassAssertion                                466  flagged 0
  Declaration                                    81  flagged 0
  ObjectPropertyAssertion                       864  flagged 0
  ObjectPropertyRange                             1  flagged 0
  OntologyHeader                                  1  flagged 0
  SubClassOf                                     72  flagged 51
  SubObjectPropertyOf                             4  flagged 0
  TransitiveObjectProperty                        2  flagged 0
  EL violations: 0; OWL 2 DL typing issues (undeclared entities): 43 distinct, 51 axioms
test estate_small_fixture_is_owl2_el ... ok
owl2-profile DL    expect EL=false (DL ontology: true ): cases=133 agree=133 disagree=0 unparsable=0 functional-syntax documents not classified=16
owl2-profile RLDL  expect EL=false (DL ontology: true ): cases=13 agree=13 disagree=0 unparsable=0 functional-syntax documents not classified=5
owl2-profile QLDL  expect EL=false (DL ontology: true ): cases=1 agree=1 disagree=0 unparsable=0 functional-syntax documents not classified=0
owl2-profile Full  expect EL=false (DL ontology: false): cases=60 agree=59 disagree=1 unparsable=0 functional-syntax documents not classified=0
    DISAGREE Full#16 (classified EL)
owl2-profile total: cases=271 agree=270
test profile_identification_agreement ... ok
visiongraph corpus: 278961 axioms
  AnnotationAssertion                         74717  flagged 0
  ClassAssertion                              15230  flagged 0
  Declaration                                  9273  flagged 0
  DisjointClasses                                 1  flagged 0
  ObjectPropertyAssertion                    143174  flagged 0
  ObjectPropertyRange                             1  flagged 0
  OntologyHeader                                  1  flagged 0
  SubClassOf                                  36558  flagged 881
  SubObjectPropertyOf                             4  flagged 0
  TransitiveObjectProperty                        2  flagged 0
  EL violations: 0; OWL 2 DL typing issues (undeclared entities): 690 distinct, 881 axioms
test full_corpus_is_owl2_el ... ok
test result: ok. 6 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 5.39s
```

Profile-identification agreement: 270 of 271 RDF/XML cases classified as the OWL API labels say (all 211 labelled
OWL 2 DL agree; Full 59 of 60). 44 functional-syntax documents were not run (All 5, ELDL 3, NOQL 9, NOEL 6, DL 16,
RLDL 5; the two syntaxes are interleaved, not paired). `Full#16` (`x owl:differentFrom x`) is undiagnosed.
Annotation assertions are non-logical and not datatype-checked, which covers the 52 `xsd:float` `vc:qualityScore` literals in the fixture.

The undeclared-class finding, run deliberately with `-- --ignored`:

```
test fixture_declares_every_class_it_uses ... FAILED
43 undeclared-entity issues, e.g. ["SubClassOf: class https://narrativegoldmine.com/class/academic-conference is not declared (not OWL 2 DL)", "SubClassOf: class https://narrativegoldmine.com/class/access-control is not declared (not OWL 2 DL)", "SubClassOf: class https://narrativegoldmine.com/class/agent-communication-protocol is not declared (not OWL 2 DL)"]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.06s
```

## Gates (final)

- `cargo clippy --workspace --all-targets -- -D warnings`: `Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.47s`. (The first commit's run needed `-A clippy::unnecessary_map_or` for another worker's `migrate/journal.rs`.)
- `cargo test --workspace`: every test binary passes; the only failure is the doctest `crates/ontomark-compile/src/shacl.rs - shacl (line 125)`, not this worker's file. Doctests took 377 s under concurrent builds.
- `cargo fmt --all --check`: exit 1, from other workers' files: crates/ontomark-compile/examples/shacl_export.rs, crates/ontomark-compile/src/jsonld.rs, crates/ontomark-compile/src/lib.rs, crates/ontomark-compile/src/shacl.rs, crates/ontomark-compile/tests/jsonld_matches_turtle.rs, crates/ontomark-compile/tests/shacl_projection.rs. This worker's files are rustfmt-clean.
- `Cargo.lock` not committed by this worker: it also carries another worker's change.
- Offline: with `conformance/w3c/rdf-turtle` and `owl2-profile` moved aside, both binaries print the skip note and pass.
