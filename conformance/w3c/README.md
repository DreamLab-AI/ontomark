# Third-party conformance corpora

Fetched, never committed (`.gitignore`); revisions and digests are in `PINS`.

```sh
conformance/w3c/fetch.sh          # both corpora (or: turtle | owl)
cargo test -p ontomark-compile --test w3c_turtle --test owl2_el_profile -- --nocapture
```

Without the fetch the suite tests print a note and pass, so `cargo test`
stays offline. The unit and property tests need no corpus.

| Directory | Source | Used by |
|---|---|---|
| `rdf-turtle/` | W3C RDF 1.1 Turtle tests, `w3c/rdf-tests` at the pinned commit | `tests/w3c_turtle.rs` |
| `owl2-profile/` | OWL 2 profile-identification cases as packaged by the OWL API contract tests, `owlcs/owlapi` at the pinned commit | `tests/owl2_el_profile.rs` |

## Turtle round trip

ontomark is a Turtle writer, so each `TestTurtleEval` and
`TestTurtlePositiveSyntax` action is parsed with `oxttl`, loaded into
`turtle::Graph`, written with `turtle::serialise`, re-parsed and compared with
the expected N-Triples (or the first parse) under blank-node isomorphism
(`oxrdf` RDFC-1.0 canonicalisation). Negative tests are N/A for a writer.
A construct `Graph` cannot carry is counted as *unrepresentable*, not as a pass.

## OWL 2 profile cases

The W3C OWL 2 test-case site is no longer served, so the labelled profile
cases come from the OWL API repository (`contract/src/test/resources/.../apitest/profile*TestCases.txt`).
The expectation for each file is the one in the OWL API's `Profile*TestCase`
classes: `All`, `ELDL`, `NOQL`, `NORL` are in EL; `NOEL`, `DL`, `RLDL`, `QLDL`
are not; `Full` is not even OWL 2 DL. Each case is stored in RDF/XML and in
functional syntax; the RDF/XML copy is classified.
