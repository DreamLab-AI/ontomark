---
id: EXP-05
linked_adrs: [ADR-2014, ADR-2016]
priority: high
regression_critical: true
evidence_category: executable
status: proven
evidence: docs/evidence/WP-E/README.md
stabilized_by: "crates/ontomark-wasm/src/projection.rs::tests; crates/ontomark-wasm/tests/transcript.rs::transcript_semantics; plugin/test/e2e/offline.e2e.cjs"
authored_by: pair
slice: WP-E (plugin surface of EXP-05)
---

## Expectation: the plugin works on a plain vault with the generic profile; the estate profile is opt-in

With default settings the plugin selects the `generic` profile. A vault of
ordinary notes indexes without error. A note with no frontmatter, or with
frontmatter but no ontology keys, yields zero classes, zero individuals, zero
properties and zero axioms. Wikilinks in prose are document links and are never
promoted to `subClassOf`. Selecting `estate` in settings is the only way estate
rules apply, and an estate-invalid page is refused under that profile.

### In scope
- Plain note with `[[links]]` and tags: navigable, not an entity.
- Generic typed page (`type: class`, `subclass-of: [[B]]`): one class, one axiom.
- Profile switch re-indexes and bumps the generation.

### Counter-examples (must NOT happen)
- Any axiom derived from a prose wikilink under either profile.
- Estate folder names, `urn:ngm:` IRIs or governance keys required by default.
- Estate branding in the default UI, manifest or README quickstart.
