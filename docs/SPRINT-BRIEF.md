# Sprint brief: ontomark single sprint to conclusion

Queen: the lead session (Fable). Specialists: Opus, one per slice, each the
owner of its crate boundaries and acceptance tests. Workers: Sonnet, spawned by
specialists for implementation. Researchers: Haiku `Explore`, for census and
lookup. Method: build-with-quality (EDD before TDD; executed evidence only).

## Non-negotiables

1. Markdown is authoritative; the model is derived. No write path exists that is
   not a guarded, lossless Markdown patch.
2. One owner per format: parse (Markdown), model (types), reason (EL),
   compile (RDF, SHACL, NGG1, profiles). No second implementation anywhere in
   the estate survives this sprint (EXP-10).
3. Standards first, superset as a versioned profile: OWL 2 EL, RDF 1.1 Turtle,
   JSON-LD 1.1, SKOS, PROV-O, SHACL, DCTERMS. Estate rules (frontmatter grammar,
   lifecycle, governance kinds, NGG1 tiers, `urn:ngm:` and `resource:` IRIs,
   attribution roster, 53 taxonomy categories) live in `profiles/estate/`.
4. Identity continuity: every class keeps its `urn:ngm:class:` IRI and
   `resource:` IRI byte-for-byte; vowl-wasm keeps its published JS API and
   crate name as a re-export. Golden fixtures prove it (EXP-02, EXP-09).
5. Licence: MIT OR Apache-2.0 for every crate here. AGPL code (anything
   descending from solid-pod or sidestr, in practice `vault/src/nostr.rs` and
   its governance posting) stays in VisionClaw as the estate adapter and calls
   into these crates. Nothing from it is copied here.
6. Signing is a trait (`Signer`) with did:nostr as one implementation in the
   estate profile; the generic surface signs with nothing or with a caller's
   implementation.
7. Deprecate and remove as you go. When a slice reaches parity for a
   responsibility, the specialist for WP-DEP deletes the superseded code in
   VisionClaw, visionGraph, vowl-wasm, Loom and Agentbox in the same sprint,
   repins consumers, and updates every repo's docs and diagram corpus. Removal
   is gated on the consumer's existing tests passing against the new crate.
8. Bulk migration of the synthetic corpus is allowed when the typed model needs
   it (ADR-2015). It runs once through `ontomark migrate`, emits a page-level
   report, is reversible (EXP-08), and is captured by golden fixtures before.
9. Private repo now; public later. No private page bodies, no secrets, no
   working-vault fixtures in history. Fixtures are synthesised or public.
10. Agents never push visionGraph or VisionClaw main; they commit on branches
    and report. The queen pushes after gates. The ontomark repo itself may be
    pushed by specialists to their own branches.

## Slices and owners

| Slice | Specialist | Crates | Acceptance |
|---|---|---|---|
| WP-A model+parse | `spec-model` | ontomark-model, ontomark-parse | lossless round-trip on every public page of visionGraph (EXP-03); typed model with the 8 defects from ADR-2015 fixed; reference states |
| WP-B reasoner | `spec-reason` | whelk-el, ontomark-reason | native == wasm32 entailments on `el_profile` and `defined_classes` fixtures (EXP-01); rayon optional; no std::time in lib |
| WP-C compiler+contracts | `spec-compile` | ontomark-compile, ontomark-cli, profiles/estate | Turtle, JSON-LD, SHACL, NGG1 byte-identical to VisionClaw golden at `ontomark-census-baseline` (EXP-02, 04, 05, 09); CLI drop-in for `vault build/validate/gate/conflicts` |
| WP-D estate deprecation+docs | `spec-estate` | VisionClaw, visionGraph, vowl-wasm, Loom, Agentbox | duplicates removed, consumers repinned, diagrams and docs updated in every repo (EXP-10); publish.yml consumes a released `ontomark` CLI |
| WP-E plugin+wasm | `spec-plugin` | ontomark-wasm, plugin/ | offline Obsidian plugin reading a vault, running EL, local SPARQL subset, guarded write (EXP-06, 11, 12); desktop first |

Sequencing: A and B start now and are independent. C starts on the census and
golden capture now, codes against A and B as their APIs land. D starts the
estate census now; removal waits on C's parity per responsibility. E starts on
the worker lifecycle and plugin shell now; semantics arrive from A, B, C.

## Shared facts

- Census baseline: VisionClaw tag `ontomark-census-baseline` (= 1044b0722).
- Source to extract from: `project/crates/vault-core` (portable, 6.3k lines)
  and `project/crates/vault` (20.5k lines; keep nostr.rs out). Whelk fork
  source: `~/workspace/.tmp/.../scratchpad/whelk-probe` or clone
  `github.com/jjohare/whelk-rs` rev `79a1ee2`.
- vowl-wasm 0.1.2 at `~/workspace/vowl-wasm`, MIT, frozen JS API, NGG1 tests.
- Golden fixtures: `project/crates/vault/tests/golden/`.
- Toolchain: `source scripts/env.sh` (repo-local CARGO_HOME). wasm32 std is
  baked. `just check` runs the full gate.
- Advisor (Opus) is consulted by the queen at ADR-2134 checkpoints; specialists
  may request a consult through the queen.
- Evidence: executed, with command, output, timestamp and sha (EDD). The
  cross-vendor auditor is Codex; a Claude audit is degraded and must say so.
