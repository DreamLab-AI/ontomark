# ontomark

Ontologies in Markdown. A Rust workspace that gives an Obsidian vault a typed
ontology model, OWL 2 EL reasoning, a compiler to RDF and graph tiers, and an
offline plugin, native and in WebAssembly.

Status: private incubation. Design record in `docs/adr`, plan in `docs/PLAN.md`.

| Crate | Owns |
|---|---|
| `ontomark-model` | typed model: Class, Individual, Property; full predicate IRIs; reference resolution states |
| `ontomark-parse` | lossless Markdown and frontmatter document model; guarded writes |
| `ontomark-reason` | OWL 2 EL profile checks and reasoning facade over `whelk-el` |
| `ontomark-compile` | RDF (Turtle, JSON-LD), SHACL shapes, NGG1 graph tiers, versioned profiles |
| `ontomark-wasm` | wasm-bindgen surface for the Obsidian plugin and browsers |
| `ontomark-cli` | the `ontomark` binary consumed by CI pipelines |
| `whelk-el` | EL reasoner derived from Whelk and whelk-rs (MIT notice in NOTICE) |

Standards consumed and produced: OWL 2 EL, RDF 1.1 Turtle, JSON-LD 1.1, SKOS,
PROV-O, SHACL, Dublin Core Terms. Estate-specific rules live in a versioned
profile, never in the crates.

Build: `source scripts/env.sh` then `cargo build`, or use `just`.

Licence: MIT OR Apache-2.0.
