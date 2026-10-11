# Evidence: W3C Turtle round trip and OWL 2 EL profile conformance

Worker WP-C (tests). Run in `/home/devuser/workspace/ontomark-wp-c`, branch `wp-c-compile`,
`CARGO_TARGET_DIR=/home/devuser/workspace/.tmp/ontomark-target-w3c`. Run started 2026-10-11T18:34:17Z.

## Pins

```
RDF_TESTS_COMMIT=42e6bb1d254bc10b3bb849e0a6872afea9ff4da3
RDF_TURTLE_MANIFEST_SHA256=52b5667f7c85743b06ab0bbee3f5d74cf4fd9c117b98cde94ad66bb5c9489b17
OWLAPI_COMMIT=b61ebe2da83daceebb3e7ba7afbd2582c9240c33
RDF_TURTLE_MANIFEST_SHA256 and the nine OWLAPI_SHA256_* digests: conformance/w3c/PINS
```

## Fetch

`conformance/w3c/fetch.sh` (2026-10-11T18:24Z): `rdf-turtle: 459 files at 42e6bb1d...`, `owl2-profile: 9 files at b61ebe2d...`.

## Turtle writer: W3C RDF 1.1 Turtle suite and property tests

`cargo test -p ontomark-compile --test w3c_turtle -- --nocapture`

```
test bug_leading_hyphen_local_name_is_not_valid_turtle ... ignored, BUG: turtle::shorten accepts a leading `-` in a local name, but PN_LOCAL cannot start with `-`
test bug_random_graphs_round_trip_unrestricted ... ignored, BUG: shorten() emits `a` for rdf:type in object/datatype position and `p:-x` for locals starting with `-`
test bug_rdf_type_as_object_is_not_valid_turtle ... ignored, BUG: turtle::shorten returns the keyword `a` for rdf:type everywhere, but `a` is only legal as a predicate
test language_and_datatype_forms_stay_distinct ... ok
w3c_turtle[generic]: 313 tests: pass=219 fail=0 unrepresentable=0 parser-rejects=0 n/a=94
w3c_turtle[estate]: 313 tests: pass=219 fail=0 unrepresentable=0 parser-rejects=0 n/a=94
test w3c_turtle_round_trip ... ok
test property_unicode_and_escapes_in_literals ... ok
test property_random_graphs_round_trip ... ok
test result: ok. 4 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 1.16s
```

Counts per profile (generic and estate prefixes): 313 manifest tests = 145 TestTurtleEval + 74 TestTurtlePositiveSyntax
(219 applicable) + 94 TestTurtleNegativeSyntax (N/A). pass 219, fail 0, unrepresentable 0, parser rejects 0.

Known writer bugs (ignored tests, fail when run): `cargo test -p ontomark-compile --test w3c_turtle -- --ignored`

```
test bug_leading_hyphen_local_name_is_not_valid_turtle ... FAILED
test bug_rdf_type_as_object_is_not_valid_turtle ... FAILED
test bug_random_graphs_round_trip_unrestricted ... FAILED
```

## OWL 2 EL profile

`cargo test -p ontomark-compile --test owl2_el_profile -- --nocapture --include-ignored`

```
test annotation_values_may_use_any_datatype ... ok
owl2-profile All   expect EL=true  (DL ontology: true ): cases=27 agree=27 disagree=0 unparsable=0 functional-syntax copies skipped=5
test el_allows_the_specified_constructs ... ok
test el_rejects_the_unsupported_constructs ... ok
owl2-profile ELDL  expect EL=true  (DL ontology: true ): cases=7 agree=7 disagree=0 unparsable=0 functional-syntax copies skipped=3
owl2-profile NOQL  expect EL=true  (DL ontology: true ): cases=14 agree=14 disagree=0 unparsable=0 functional-syntax copies skipped=9
owl2-profile NORL  expect EL=true  (DL ontology: true ): cases=8 agree=8 disagree=0 unparsable=0 functional-syntax copies skipped=0
owl2-profile NOEL  expect EL=false (DL ontology: true ): cases=8 agree=8 disagree=0 unparsable=0 functional-syntax copies skipped=6
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
test fixture_declares_every_class_it_uses ... FAILED
owl2-profile DL    expect EL=false (DL ontology: true ): cases=133 agree=133 disagree=0 unparsable=0 functional-syntax copies skipped=16
owl2-profile RLDL  expect EL=false (DL ontology: true ): cases=13 agree=13 disagree=0 unparsable=0 functional-syntax copies skipped=5
owl2-profile QLDL  expect EL=false (DL ontology: true ): cases=1 agree=1 disagree=0 unparsable=0 functional-syntax copies skipped=0
owl2-profile Full  expect EL=false (DL ontology: false): cases=60 agree=59 disagree=1 unparsable=0 functional-syntax copies skipped=0
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
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.29s
```

## Gates

- `cargo clippy --workspace --all-targets -- -D warnings`: clean at the final run.
- `cargo test --workspace`: all test binaries pass except the doctest `crates/ontomark-compile/src/shacl.rs - shacl (line 125)` (not this worker's file).
- `cargo fmt --all --check`: exit 1 from other workers' files (e.g. `src/jsonld.rs`); this worker's files are rustfmt-clean.
- Cargo.lock is not committed by this worker: it also carries another worker's change.
