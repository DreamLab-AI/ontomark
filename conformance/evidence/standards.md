# Standards verification: JSON-LD and SHACL

Executed 2026-10-11 (UTC) in the `ontomark-wp-c` worktree, branch `wp-c-compile`,
modules `ontomark_compile::jsonld` and `ontomark_compile::shacl` at commit
`FINAL_HASH` (the `jsonld`/`shacl` modules were first committed as `757bf8a` and `b3b84c1`). Nothing here reimplements a
standard: every verdict comes from a third-party processor.

## Third-party tools

| role | tool | version |
|---|---|---|
| Turtle parser (Rust) | `oxttl` (oxigraph project) | 0.2.4 |
| JSON-LD 1.1 parser (Rust) | `oxjsonld` (oxigraph project) | 0.2.6 |
| RDF data model, RDFC-1.0 canonicalisation (Rust) | `oxrdf`, feature `rdfc-10`, SHA-256 | 0.3.4 |
| Turtle parser, JSON-LD parser, blank-node isomorphism (Python) | `rdflib` (`rdflib.compare.isomorphic`) | 7.6.0 |
| JSON-LD 1.1 processor, `toRdf` (Python) | `PyLD` | 3.3.0 |
| SHACL engine | `pyshacl` (with `meta_shacl=True`, `inference="none"`, `advanced=False`) | 0.40.1 |
| Python | CPython | 3.12.14 |

Python environment: `python3 -m venv /home/devuser/workspace/.tmp/ontomark-venv`
then `pip install rdflib pyld pyshacl pyyaml` (pulls `owlrl 7.6.2`,
`pyparsing 3.3.3`, `cachetools 7.2.1`). No Rust SHACL engine was run
(`rudof` and `shacl_validation` were not tried); pyshacl is the only SHACL
engine in this evidence.

`ontomark validate` is the reference for the SHACL comparison. Fixture and
mutation runs use the CLI built from this worktree
(`cargo build -p ontomark-cli`, commit `d5c1188` plus this work);
the corpus run uses the same binary.

## 1. JSON-LD equals Turtle

### Rust (no network, no Python): `cargo test -p ontomark-compile --test jsonld_matches_turtle`

`oxttl` parses `turtle::serialise`, `oxjsonld` parses `jsonld::to_jsonld_string`
with the embedded `okf::context`, both are RDFC-1.0 canonicalised with `oxrdf`,
and the canonical datasets are compared for equality (blank-node isomorphism).

```
test the_asserted_ontology_graph_is_the_same_in_both_syntaxes ... ok
test the_shapes_graph_is_the_same_in_both_syntaxes ... ok        (lists: sh:in, sh:or, rdf:first/rest -> @list)
test the_frontmatter_data_graphs_are_the_same_in_both_syntaxes ... ok
test the_mutation_fixture_data_graphs_are_the_same_in_both_syntaxes ... ok
test the_comparison_can_fail ... ok                               (control: a dropped node makes them differ)
test the_output_is_deterministic ... ok
test result: ok. 6 passed; 0 failed; 1 ignored
```

Full corpus (opt-in, 2026-10-11T19:06:55Z):

```
$ ONTOMARK_STANDARDS_CORPUS=/home/devuser/workspace/visionGraph \
    cargo test --release -p ontomark-compile --test jsonld_matches_turtle -- --ignored --nocapture
corpus knowledge data: 704819 triples isomorphic
corpus working data: 22098 triples isomorphic
test the_full_corpus_is_the_same_in_both_syntaxes ... ok     (also the 348081-triple asserted ontology)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 31.54s
```

### Python (`conformance/standards/run.sh`): rdflib and PyLD

The JSON-LD of each artefact is read by rdflib's JSON-LD parser and, separately,
by PyLD `toRdf` (N-Quads read back by rdflib); both are compared with the
Turtle by `rdflib.compare.isomorphic`. Results are in the three run logs below
(estate-small, estate-invalid, corpus): every check prints `isomorphic=True`.
The corpus run limits the Python legs to `shapes` and `working.data`
(`JSONLD_CHECKS`); the 348081-triple corpus ontology leg in rdflib/PyLD was
started and killed after about 25 minutes of CPU in rdflib's isomorphism check,
so the corpus ontology and the 704819-triple corpus knowledge data graph are
verified by the Rust leg above only.

## 2. SHACL: shapes, mutations, corpus

Command (per repository), all outputs written outside the repository:

```
conformance/standards/run.sh <ontomark-binary> <repo> /home/devuser/workspace/.tmp/ontomark-venv <out-dir>
```

`run.sh` runs `cargo run -p ontomark-compile --example shacl_export` (shapes,
data graphs, ontology as Turtle and JSON-LD), `ontomark --repo <repo> --json
validate --vault all`, then `shacl_check.py`: pyshacl validates each vault's
data graph with `meta_shacl=True` (the shapes graph is itself checked against
SHACL-SHACL), and every result is mapped to the validate code carried by its
shape (`ns:ruleCode`) and to a page id (`ns:meta-id`), then compared with
validate's issues as sets of (page, code).

Reading the tables: `validate`/`shacl` are issue and result counts; `pages(v/s)`
is the number of distinct pages with the code in each; `agree` is set equality
of the pages. `sh:conforms=False` is pyshacl's answer for any result of any
severity, warnings and infos included; the meaningful figure is the count of
`error` (sh:Violation) results.

### 2.1 Fixture `conformance/fixtures/estate-small`: shapes conform

Zero sh:Violation results; every SHACL-covered code agrees with validate on the
set of pages; the one disagreement, `SLUG_MISMATCH`, is a procedural rule.

```
# 2026-10-11T19:29:35Z repo=/home/devuser/workspace/ontomark-wp-c/conformance/fixtures/estate-small out=/home/devuser/workspace/.tmp/shacl/ev-small
ontology: 2297 triples
knowledge: 54 pages, 3700 triples
working: 2 pages, 44 triples
ontology: Turtle 2297 triples vs rdflib JSON-LD 2297 triples: isomorphic=True
ontology: Turtle 2297 triples vs PyLD toRdf 2297 triples: isomorphic=True
ontology: rdflib 7.6.0, PyLD 3.3.0
shapes: Turtle 691 triples vs rdflib JSON-LD 691 triples: isomorphic=True
shapes: Turtle 691 triples vs PyLD toRdf 691 triples: isomorphic=True
shapes: rdflib 7.6.0, PyLD 3.3.0
knowledge.data: Turtle 3700 triples vs rdflib JSON-LD 3700 triples: isomorphic=True
knowledge.data: Turtle 3700 triples vs PyLD toRdf 3700 triples: isomorphic=True
knowledge.data: rdflib 7.6.0, PyLD 3.3.0
working.data: Turtle 44 triples vs rdflib JSON-LD 44 triples: isomorphic=True
working.data: Turtle 44 triples vs PyLD toRdf 44 triples: isomorphic=True
working.data: rdflib 7.6.0, PyLD 3.3.0
## SHACL: knowledge
pyshacl 0.40.1 / rdflib 7.6.0: 3700 data triples, 693 shape triples; sh:conforms=False; results {'warning': 817, 'info': 7}; validate errors/warnings/info = 0/867/7
code                             validate    shacl  pages(v/s)   agree
DANGLING_LINK                         801      758     49/49     YES
MULTI_PARENT                            7        7      7/7      YES
PROFILE:ProfileMaturityLevel            0        7      0/0      -
PUBLIC_LINKS_PRIVATE                    1        1      1/1      YES
SLUG_MISMATCH                          14        0     13/0      NO
    only validate: ['2D LiDAR', '3D Content Pipeline', '3D Design', '3D Development', '3D Graphics Standard']  only shacl: []
UNVERIFIED_STABLE                      51       51     51/51     YES
DANGLING_LINK distinct (page, target) pairs: validate 758, shacl 758, identical=True
profile-only shape results (no validate rule): {'PROFILE:ProfileMaturityLevel': 7}

per page (validate codes | shacl codes | profile-only shapes | agree on shacl-covered codes):
## SHACL: working
pyshacl 0.40.1 / rdflib 7.6.0: 44 data triples, 693 shape triples; sh:conforms=False; results {'warning': 3}; validate errors/warnings/info = 0/3/0
code                             validate    shacl  pages(v/s)   agree
MISSING_DOMAIN                          2        2      2/2      YES
UNVERIFIED_STABLE                       1        1      1/1      YES
DANGLING_LINK distinct (page, target) pairs: validate 0, shacl 0, identical=True
profile-only shape results (no validate rule): {}

per page (validate codes | shacl codes | profile-only shapes | agree on shacl-covered codes):
```

### 2.2 Mutation fixture `conformance/fixtures/estate-invalid`

A copy of `vault.toml` and `ontology/`, three real estate-small pages, a clean
parent (`Mut Base Parent`), three clean controls (`C01`..`C03`) and one defect
per page (`M01`..`M26`), journals and two working pages. Agreement per mutation
is on the codes the shapes cover; codes validate reports that SHACL Core cannot
express are listed as procedural.

| mutation | validate reports | SHACL reports (shape code) | agree |
|---|---|---|---|
| M01 missing `resource` | nothing (identity is minted from the slug) | profile-only `RequiredKey-knowledge-resource` | yes, and a documented divergence (see 4) |
| M02 `public: "yes"` | INVALID_BOOLEAN | INVALID_BOOLEAN | yes |
| M03 `status: half-baked` | INVALID_STATUS | INVALID_STATUS | yes |
| M04 `generated.by: robot:fixture` | MISSING_GENERATED | MISSING_GENERATED (`GeneratedStamp`, actor `sh:pattern`) | yes |
| M05 relation to a non-existent page | DANGLING_LINK | DANGLING_LINK | yes |
| M06 no `type` | MISSING_TYPE | MISSING_TYPE | yes |
| M07 `type: Gadget` | INVALID_TYPE | INVALID_TYPE | yes |
| M08 unknown key `colour` | UNKNOWN_KEY | UNKNOWN_KEY | yes |
| M09 `quality: 1.5` | OUT_OF_RANGE | OUT_OF_RANGE | yes |
| M10 `quality: abc` | INVALID_NUMBER | INVALID_NUMBER | yes |
| M11 `maturity: vintage` | INVALID_ENUM | INVALID_ENUM | yes |
| M12 no `status` | MISSING_REQUIRED, MISSING_STATUS | MISSING_REQUIRED, MISSING_STATUS | yes |
| M13 no `generated` | MISSING_GENERATED | MISSING_GENERATED | yes |
| M14 `is-a` itself | SELF_REFERENCE | SELF_REFERENCE | yes |
| M15 two pages, one `resource` | DUPLICATE_IRI (first owner) | DUPLICATE_IRI | yes |
| M16 no `domain` | MISSING_DOMAIN | MISSING_DOMAIN | yes |
| M17 `domain: gastronomy` | INVALID_DOMAIN | INVALID_DOMAIN | yes |
| M18 public page links a private page | PUBLIC_LINKS_PRIVATE | PUBLIC_LINKS_PRIVATE | yes |
| M19 two parents | MULTI_PARENT (info) | MULTI_PARENT (info) | yes |
| M20 no `title`, foreign `resource` | nothing | profile-only `RequiredKey-knowledge-title`, `ResourceIri` | yes (profile-only) |
| M21 `public: 3` | INVALID_BOOLEAN | INVALID_BOOLEAN | yes |
| M22 `status: stable`, no human verifier | UNVERIFIED_STABLE | UNVERIFIED_STABLE | yes |
| M23 Logseq `key::` in the body | LOGSEQ_PROPERTY | nothing (procedural) | yes (procedural) |
| M24 `defines-as: [{only: ...}]` | NON_EL_DEFINITION | nothing (procedural) | yes (procedural) |
| M25 `disjoint-with` a non-sibling | DISJOINT_NOT_SIBLINGS | nothing (the sibling join is procedural) | yes (procedural) |
| M26 `resource: ngm:compact-look-alike` (a compact-IRI look-alike) | nothing | profile-only `ResourceIri`; the JSON-LD of its data graph is isomorphic to the Turtle (`iris_that_look_like_compact_iris_stay_absolute`) | yes (profile-only) |
| C01 clean | nothing | nothing | yes |
| C02 `status: Stable` plus a human `verified` | nothing | nothing (status normalised as validate does) | yes |
| C03 `public: "true"` | INVALID_PUBLIC_FLAG | plain literal `"true"`, no datatype (a quoted boolean is not a boolean; S1.3, 2026-10-12, was: accepted) | yes |
| journals: undated name / `type: Gadget` | JOURNAL_NOT_DATED / INVALID_TYPE | same | yes |
| working: `type: Gadget`; unknown key allowed | INVALID_TYPE; nothing | INVALID_TYPE; nothing | yes |

Every row is taken from the log below (the `per page` section).

```
# 2026-10-11T19:29:51Z repo=/home/devuser/workspace/ontomark-wp-c/conformance/fixtures/estate-invalid out=/home/devuser/workspace/.tmp/shacl/ev-invalid
ontology: 548 triples
knowledge: 35 pages, 1397 triples
working: 4 pages, 64 triples
ontology: Turtle 548 triples vs rdflib JSON-LD 548 triples: isomorphic=True
ontology: Turtle 548 triples vs PyLD toRdf 548 triples: isomorphic=True
ontology: rdflib 7.6.0, PyLD 3.3.0
shapes: Turtle 691 triples vs rdflib JSON-LD 691 triples: isomorphic=True
shapes: Turtle 691 triples vs PyLD toRdf 691 triples: isomorphic=True
shapes: rdflib 7.6.0, PyLD 3.3.0
knowledge.data: Turtle 1397 triples vs rdflib JSON-LD 1397 triples: isomorphic=True
knowledge.data: Turtle 1397 triples vs PyLD toRdf 1397 triples: isomorphic=True
knowledge.data: rdflib 7.6.0, PyLD 3.3.0
working.data: Turtle 64 triples vs rdflib JSON-LD 64 triples: isomorphic=True
working.data: Turtle 64 triples vs PyLD toRdf 64 triples: isomorphic=True
working.data: rdflib 7.6.0, PyLD 3.3.0
## SHACL: knowledge
pyshacl 0.40.1 / rdflib 7.6.0: 1397 data triples, 693 shape triples; sh:conforms=False; results {'warning': 99, 'error': 12, 'info': 2}; validate errors/warnings/info = 15/101/2
code                             validate    shacl  pages(v/s)   agree
DANGLING_LINK                          89       82      4/4      YES
DISJOINT_NOT_SIBLINGS                   1        0      1/0      NO
    only validate: ['M25 Disjoint Not Siblings']  only shacl: []
DUPLICATE_IRI                           1        1      1/1      YES
INVALID_BOOLEAN                         2        2      2/2      YES
INVALID_DOMAIN                          1        1      1/1      YES
INVALID_ENUM                            1        1      1/1      YES
INVALID_NUMBER                          1        1      1/1      YES
INVALID_STATUS                          1        1      1/1      YES
INVALID_TYPE                            2        2      2/2      YES
JOURNAL_NOT_DATED                       1        1      1/1      YES
LOGSEQ_PROPERTY                         1        0      1/0      NO
    only validate: ['M23 Logseq Property In Body']  only shacl: []
MISSING_DOMAIN                          1        1      1/1      YES
MISSING_GENERATED                       2        2      2/2      YES
MISSING_REQUIRED                        1        1      1/1      YES
MISSING_STATUS                          1        1      1/1      YES
MISSING_TYPE                            1        1      1/1      YES
MULTI_PARENT                            2        2      2/2      YES
NON_EL_DEFINITION                       1        0      1/0      NO
    only validate: ['M24 Non EL Definition']  only shacl: []
OUT_OF_RANGE                            1        1      1/1      YES
PROFILE:ProfileMaturityLevel            0        1      0/0      -
PROFILE:RequiredKey-knowledge-resource        0        1      0/0      -
PROFILE:RequiredKey-knowledge-title        0        1      0/0      -
PROFILE:ResourceIri                     0        2      0/0      -
PUBLIC_LINKS_PRIVATE                    1        1      1/1      YES
SELF_REFERENCE                          1        1      1/1      YES
UNKNOWN_KEY                             1        1      1/1      YES
UNVERIFIED_STABLE                       4        4      4/4      YES
DANGLING_LINK distinct (page, target) pairs: validate 82, shacl 82, identical=True
profile-only shape results (no validate rule): {'PROFILE:ResourceIri': 2, 'PROFILE:RequiredKey-knowledge-resource': 1, 'PROFILE:ProfileMaturityLevel': 1, 'PROFILE:RequiredKey-knowledge-title': 1}

per page (validate codes | shacl codes | profile-only shapes | agree on shacl-covered codes):
  M01 Missing Resource                       V=- | S=- | P=RequiredKey-knowledge-resource | AGREE
  M02 Public Yes                             V=INVALID_BOOLEAN | S=INVALID_BOOLEAN | P=- | AGREE
  M03 Unknown Status                         V=INVALID_STATUS | S=INVALID_STATUS | P=- | AGREE
  M04 Malformed Generated By                 V=MISSING_GENERATED | S=MISSING_GENERATED | P=- | AGREE
  M05 Dangling Relation                      V=DANGLING_LINK | S=DANGLING_LINK | P=- | AGREE
  M06 Missing Type                           V=MISSING_TYPE | S=MISSING_TYPE | P=- | AGREE
  M07 Invalid Type                           V=INVALID_TYPE | S=INVALID_TYPE | P=- | AGREE
  M08 Unknown Key                            V=UNKNOWN_KEY | S=UNKNOWN_KEY | P=- | AGREE
  M09 Quality Out Of Range                   V=OUT_OF_RANGE | S=OUT_OF_RANGE | P=- | AGREE
  M10 Quality Not A Number                   V=INVALID_NUMBER | S=INVALID_NUMBER | P=- | AGREE
  M11 Unknown Maturity                       V=INVALID_ENUM | S=INVALID_ENUM | P=ProfileMaturityLevel | AGREE
  M12 Missing Status                         V=MISSING_REQUIRED,MISSING_STATUS | S=MISSING_REQUIRED,MISSING_STATUS | P=- | AGREE
  M13 Missing Generated                      V=MISSING_GENERATED | S=MISSING_GENERATED | P=- | AGREE
  M14 Self Reference                         V=SELF_REFERENCE | S=SELF_REFERENCE | P=- | AGREE
  M15 Duplicate Iri A                        V=DUPLICATE_IRI | S=DUPLICATE_IRI | P=- | AGREE
  M16 Missing Domain                         V=MISSING_DOMAIN | S=MISSING_DOMAIN | P=- | AGREE
  M17 Invalid Domain                         V=INVALID_DOMAIN | S=INVALID_DOMAIN | P=- | AGREE
  M18 Public Links Private                   V=PUBLIC_LINKS_PRIVATE | S=PUBLIC_LINKS_PRIVATE | P=- | AGREE
  M19 Multi Parent                           V=MULTI_PARENT | S=MULTI_PARENT | P=- | AGREE
  M20 No Title And Foreign Resource          V=- | S=- | P=RequiredKey-knowledge-title,ResourceIri | AGREE
  M21 Non Boolean Number Public              V=INVALID_BOOLEAN | S=INVALID_BOOLEAN | P=- | AGREE
  M22 Stable Without Human                   V=UNVERIFIED_STABLE | S=UNVERIFIED_STABLE | P=- | AGREE
  M23 Logseq Property In Body                V=LOGSEQ_PROPERTY | S=- | P=- | AGREE (procedural only in validate: LOGSEQ_PROPERTY)
  M24 Non EL Definition                      V=NON_EL_DEFINITION | S=- | P=- | AGREE (procedural only in validate: NON_EL_DEFINITION)
  M25 Disjoint Not Siblings                  V=DISJOINT_NOT_SIBLINGS | S=- | P=- | AGREE (procedural only in validate: DISJOINT_NOT_SIBLINGS)
  M26 Compact Iri Look Alike                 V=- | S=- | P=ResourceIri | AGREE
## SHACL: working
pyshacl 0.40.1 / rdflib 7.6.0: 64 data triples, 693 shape triples; sh:conforms=False; results {'warning': 5, 'error': 1}; validate errors/warnings/info = 1/5/0
code                             validate    shacl  pages(v/s)   agree
INVALID_TYPE                            1        1      1/1      YES
MISSING_DOMAIN                          4        4      4/4      YES
UNVERIFIED_STABLE                       1        1      1/1      YES
DANGLING_LINK distinct (page, target) pairs: validate 0, shacl 0, identical=True
profile-only shape results (no validate rule): {}

per page (validate codes | shacl codes | profile-only shapes | agree on shacl-covered codes):
  W01 Bad Type                               V=INVALID_TYPE,MISSING_DOMAIN | S=INVALID_TYPE,MISSING_DOMAIN | P=- | AGREE
  W02 Unknown Key Allowed                    V=MISSING_DOMAIN | S=MISSING_DOMAIN | P=- | AGREE
```

### 2.3 Full corpus (read-only; graphs generated under `/home/devuser/workspace/.tmp`, never into a repository)

pyshacl reports **0 sh:Violation results** on both vaults, matching
`ontomark validate`'s 0 errors; the warning and info counts are compared by
code below.

```
# 2026-10-11T19:30:01Z repo=/home/devuser/workspace/visionGraph out=/home/devuser/workspace/.tmp/shacl/ev-corpus
ontology: 348081 triples
knowledge: 9258 pages, 704819 triples
working: 786 pages, 22098 triples
shapes: Turtle 691 triples vs rdflib JSON-LD 691 triples: isomorphic=True
shapes: Turtle 691 triples vs PyLD toRdf 691 triples: isomorphic=True
shapes: rdflib 7.6.0, PyLD 3.3.0
working.data: Turtle 22098 triples vs rdflib JSON-LD 22098 triples: isomorphic=True
working.data: Turtle 22098 triples vs PyLD toRdf 22098 triples: isomorphic=True
working.data: rdflib 7.6.0, PyLD 3.3.0
## SHACL: knowledge
pyshacl 0.40.1 / rdflib 7.6.0: 704819 data triples, 693 shape triples; sh:conforms=False; results {'warning': 29191, 'info': 2363}; validate errors/warnings/info = 0/28840/2378
code                             validate    shacl  pages(v/s)   agree
DANGLING_LINK                       20475    19851   2928/2928   YES
INVALID_ENUM                           38       38     38/38     YES
MULTI_PARENT                         2378     2363   2378/2363   NO
    only validate: ['AI-Assisted Developer Tooling', 'AI-Assisted Development Practice', 'Adaptive Control', 'Data Fabric Architecture', 'Data Integration Interface']  only shacl: []
PROFILE:ProfileMaturityLevel            0      977      0/0      -
PUBLIC_LINKS_PRIVATE                    2        2      2/2      YES
UNTERMINATED_FENCE                      2        0      2/0      NO
    only validate: ['AI User', 'Bias']  only shacl: []
UNVERIFIED_STABLE                    8323     8323   8323/8323   YES
DANGLING_LINK distinct (page, target) pairs: validate 19851, shacl 19851, identical=True
profile-only shape results (no validate rule): {'PROFILE:ProfileMaturityLevel': 977}

per page (validate codes | shacl codes | profile-only shapes | agree on shacl-covered codes):
  C4 Model                                   V=UNVERIFIED_STABLE | S=UNVERIFIED_STABLE | P=- | AGREE
## SHACL: working
pyshacl 0.40.1 / rdflib 7.6.0: 22098 data triples, 693 shape triples; sh:conforms=False; results {'warning': 1709, 'info': 1}; validate errors/warnings/info = 0/1687/1
code                             validate    shacl  pages(v/s)   agree
DANGLING_LINK                          14       14     10/10     YES
JOURNAL_NOT_DATED                     125      125    125/125    YES
MISSING_DOMAIN                        765      765    765/765    YES
MULTI_PARENT                            1        1      1/1      YES
PROFILE:RequiredKey-working-title        0        2      0/0      -
PROFILE:RequiredKey-working-type        0        2      0/0      -
PROFILE:ResourceIri                     0       19      0/0      -
UNTERMINATED_FENCE                      1        0      1/0      NO
    only validate: ['misc/INDEX']  only shacl: []
UNVERIFIED_STABLE                     782      782    782/782    YES
DANGLING_LINK distinct (page, target) pairs: validate 14, shacl 14, identical=True
profile-only shape results (no validate rule): {'PROFILE:RequiredKey-working-title': 2, 'PROFILE:ResourceIri': 19, 'PROFILE:RequiredKey-working-type': 2}

per page (validate codes | shacl codes | profile-only shapes | agree on shacl-covered codes):
```

Summary of the corpus comparison:

| code | validate | pyshacl results | pages | note |
|---|---|---|---|---|
| UNVERIFIED_STABLE | 8323 + 782 | 8323 + 782 | identical | |
| INVALID_ENUM | 38 | 38 | identical | |
| PUBLIC_LINKS_PRIVATE | 2 | 2 | identical | |
| MISSING_DOMAIN (working) | 765 | 765 | identical | |
| JOURNAL_NOT_DATED (working) | 125 | 125 | identical | |
| DANGLING_LINK | 20475 + 14 | 19851 + 14 | identical; distinct (page, target) pairs identical (19851, 14) | validate counts one issue per (key, link) occurrence, the SHACL path is an alternative path so repeats collapse |
| MULTI_PARENT | 2378 + 1 | 2363 + 1 | 15 pages differ | validate counts `is-a` link occurrences, so a page listing one parent twice (10 pages) or two spellings that resolve to one IRI (5 pages) is "bridging"; an RDF graph is a set, so SHACL sees one parent. Info severity |
| UNTERMINATED_FENCE | 2 + 1 | 0 | n/a | procedural (page body) |

## 3. Rust tests (no network, no Python)

`cargo test --workspace` (2026-10-11): 579 passed, 0 failed, 11 ignored.
`cargo clippy --workspace --all-targets -- -D warnings`: clean.
`cargo fmt --all -- --check`: clean. Tests added:

* `crates/ontomark-compile/src/jsonld.rs`: unit tests (lists, shared and cyclic chains, typed literals, `@type`, context forms);
* `crates/ontomark-compile/tests/jsonld_matches_turtle.rs`: section 1;
* `crates/ontomark-compile/tests/shacl_projection.rs`: the rule table equals every code in `check/validate.rs` (plus the four codes reached through other modules' constants) and the module-doc table; every `shacl` rule has a shape and no `procedural` rule has a full one; every shape has a severity, one origin annotation and a target; severities follow validate and profile-only shapes are warnings; the actor `sh:pattern` accepts exactly what `Actor::from_str` accepts on 14 samples; the data graph reads values the way validate does.

## 4. Findings and limits

* **The profile's maturity levels are narrower than the vocabulary's.**
  `profile.lifecycle.maturity_levels` lists five levels (`established`,
  `emerging`, `draft`, `stub`, `deprecated`); `ontology/vocabulary.yaml`, which
  `validate` checks (`INVALID_ENUM`), also admits `mature`. The shape
  `ProfileMaturityLevel` (profile-only, `sh:Warning`) checks the profile list:
  it reports 977 knowledge pages in the corpus (about 955 are `mature`, the
  rest `experimental`, `growing` and similar) and 7 in the estate-small fixture,
  where `validate` reports 38 and 0. The profile and the vocabulary disagree and
  the lead should decide which is right; `InvalidEnum-maturity` keeps parity
  with `validate` either way.
* **`turtle::serialise` writes `rdf:type` as `a` in object position.** `shorten()`
  is used for predicates and objects alike, so a graph holding the IRI
  `rdf:type` as an object (for example in an `sh:ignoredProperties` list) is
  written as `rdf:first a ;`, which no Turtle parser accepts (found by `oxttl`:
  "a is not a valid RDF object"). The shapes avoid the construct (unknown keys
  are an `sh:in` over `ns:meta-key`, not `sh:closed`). The ontology graph never
  holds that term as an object, so it is unaffected.
* **A page without `resource` is not an error in validate.** `Page::resource`
  falls back to `namespace + slug`, so `MISSING_CLASS_IRI` and the `resource`
  half of `MISSING_REQUIRED` cannot fire. The shapes test the same minted
  identity (so they agree by not firing) and add a separate profile-only
  `RequiredKey-knowledge-resource` warning. Likewise required `title`, required
  `public` and the namespace of `resource`/`page_resource` are profile rules
  validate never had; they are `sh:Warning` and carry no `ruleCode`. On the full
  knowledge vault they produce no results; on the working vault they produce 23
  (2 + 2 missing `type`/`title`, 19 `resource` values outside the estate namespaces, a real data finding).
* **Compact-IRI look-alikes.** An IRI such as `ngm:foo` (a scheme that is also a
  context term) would be expanded by a JSON-LD processor. `to_jsonld` writes any
  node object holding one with `"@context": null`; without that the independent
  check fails (control run: the test `iris_that_look_like_compact_iris_stay_absolute`
  fails with the reset disabled, passes with it).
* **Vacuous coverage.** `MISSING_CLASS_IRI` is counted as `shacl` only vacuously:
  neither validate nor the shapes can fire it (the identity is minted).
* **`DUPLICATE_IRI` page attribution.** validate names the first owner in vault
  walk order; the harness takes the first owner by id order. They agree on this
  data but could differ on other data.
* **Sets, not multisets.** Duplicate links and multiple relation keys collapse in RDF (see MULTI_PARENT and DANGLING_LINK above).
* **Unknown to SHACL Core.** `MISSING_LABEL`, `SLUG_MISMATCH`, `DISJOINT_NOT_SIBLINGS` (the sibling join), `NON_EL_DEFINITION`, `DEFINITION_OVER_TRANSITIVE`, `DEFINITION_NOT_EXISTENTIAL` and the six body-text rules stay procedural. 21 of the 33 codes are expressed in SHACL Core, 12 are procedural.
* **Null-valued frontmatter keys** are recorded as `ns:meta-key` (so `UNKNOWN_KEY` sees them, like validate) but carry no `ns:fm-<key>` triple, so a required key set to null counts as absent.
* **A Rust SHACL engine was not run**; only pyshacl evaluated the shapes. pyshacl treats `sh:conforms` as false for warnings and infos, so conformance is stated in terms of Violation results.
* The `generated.by` pattern is derived from the profile's actor templates and is an XSD-style regular expression; the Rust test compares it with `Actor::from_str` using the `regex` crate, whose syntax agrees with XSD on this subset.
