---
id: ADR-OM-012
title: Certified keys are never patched, questions are graded for coverage, and the v2 context is generated and closed over the page API
date: 2026-11-04
decision_status: accepted
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: ef485105e175f17d2697a9cda0abeb4997e097f8
verified_paths: [crates/ontomark-ingest/src/certification.rs, crates/ontomark-ingest/src/accept.rs, crates/ontomark-ingest/src/judge.rs, crates/ontomark-ingest/src/coverage.rs, crates/ontomark-ingest/tests/certification.rs, crates/ontomark-ingest/tests/coverage.rs, crates/ontomark-cli/src/ground_cmd.rs, crates/ontomark-cli/tests/conformance.rs, crates/ontomark-compile/src/input/okf.rs, crates/ontomark-compile/src/ir/model.rs, crates/ontomark-compile/src/build/page_api.rs, crates/ontomark-compile/src/build/okf.rs, crates/ontomark-compile/src/build/mod.rs, crates/ontomark-compile/src/write/edit.rs, conformance/fixtures/estate-small.golden/MANIFEST]
owner: jjohare
review_trigger: the first `verified` stamp written with `keys`; a release that the visionGraph publisher pins; a second actor kind admitted as human; a page API consumer that cannot take the added `certification` field
repo: ontomark
evidence_class: source+test
acceptance_boundary: crate code, unit, integration and golden tests, and validate/build runs against the visionGraph corpus with this branch and with the pinned v0.7.3; no release cut, no publisher pin bump, nothing served
amends: [ADR-OM-003, ADR-OM-010]
---

# ADR-OM-012 — Certified keys are never patched, questions are graded for coverage, and the v2 context is generated and closed over the page API

## Context

NVIDIA's auto-ontology (Apache-2.0) compiles database metadata into business
terms that people then certify. Two of its ideas carry over to ontomark (VisionFlow
ADR-2023 records the full comparison):

1. **Certification per field.** `name_certified`, `description_certified` and
   each attribute's `certified` flag survive recompilation (`dal/terms.py`, the
   upsert keeps a certified field). The term's state is derived:
   `certified` / `partial` / `pending`.
2. **An answerability check.** `check_answerable` grades a question by the share of
   its entities that the ontology covers and names the uncovered ones, so an agent
   asks for clarification instead of guessing.

Ontomark records human review only as page-level `verified` stamps (OKF trust
block). Ingest starts every value at `CONFIDENCE = 0.5` and `METHOD = "inferred"`
(ADR-OM-003), but nothing records which keys a reviewer confirmed. A re-ingest can
therefore extend a reviewed key.

Separately, the JSON-LD context drifted. `ontomark build` writes one document to
`context/v1.jsonld`, `api/schema/context.jsonld` and `ns/v2.jsonld`. That document
maps relation keys, but leaves most page API keys unmapped (`id`, `classIri`,
`definition`, `subClassOf`, `entityType`, `qualityScore`, `wikilinks`, …). It also
coins `vc:<key>` for every scalar, even one whose vocabulary property is something
else (`domain` is `vc:sourceDomain`). visionGraph's publisher covered this by
copying a stale hand-written `ns/v2.jsonld` over the generated file, so two
different contexts carried one name (visionGraph ADR-VG-009).

## Decision

1. **A key is certified when a human `verified` stamp covers it.** A stamp without
   `keys` covers the whole page. A stamp with `keys: [..]` covers only those keys.
   Only `human:<npub>` actors certify, and the stamp must carry `at`.
   `ontomark-ingest`'s `is_human_actor` and `ontomark-compile`'s `Actor::is_human`
   are pinned together by a conformance test over a shared table of actors.
2. **Ingest never adds a value to a certified key.**
   - `accept` reads certification from the note itself, never from the host's
     request. It runs the check before the unknown-key check and withholds the
     proposal with the new reason `certified`.
   - `accept_source` and `accept_judged` go through the same path.
   - `suggest_fields` never offers a certified key.
   - A certified key is still offered to the model in `plan`. Its proposals are
     withheld, which shows where the model disagrees with reviewed content.
   - The key name comes from `Options::verified_key` (default `verified`).
3. **The page API reports certification.** Every page document gains
   `certification: {state: certified|partial|pending, keys}`.
4. **Coverage grading.**
   - `Vocabulary::coverage` / `Ingest::coverage` and `ontomark ground <NAME>...`
     ground each distinct name a question mentions with ingest's own tiers.
   - Each name lands in exactly one bucket: grounded, unclear (ambiguous or an
     initials conflict) or uncovered (with near-miss suggestions).
   - `coverage` is grounded / distinct names. It is `null` when no names are
     given, because `0.0` would read as "the corpus has nothing".
   - Name extraction is the host's job.
   - `ground` reads the knowledge vault only.
5. **Context v2 is generated and closed.**
   - `okf::context_v2` is written to `context/v2.jsonld` and to `ns/v2.jsonld`.
   - Every page API key has a term, enforced by a test. Scalars use their
     vocabulary `owl:` property and datatype. Link objects `{id, label}` are
     nodes. `certification` and `provenance` have scoped contexts.
   - `context/v1.jsonld` and `api/schema/context.jsonld` keep their exact bytes,
     because a versioned URL does not change.

## Consequences

- **Page API bytes change for every page.** The legacy-parity golden was
  re-blessed in a targeted way (MANIFEST `reblessed_note_3`): only page documents,
  `ns/v2`, `context/v2` and the generation markers changed. `ontology.ttl`,
  `context/v1`, `publish/` and the verbs are byte-identical.
- **No triple changes.** Across the 9,609-page visionGraph corpus, this branch,
  `main` and v0.7.3 produce the same `ontology.ttl`. They also produce the same
  inferred, scaffold and prose indexes once `generated_at` is masked. So VisionFlow
  ADR-2022's corpus history records nothing from this change.
- **The day-one effect is nil.** One visionGraph page carries a human stamp
  (whole page) and none carries `keys`. The only write path is
  `ontomark edit --set 'verified+={by: human:…, at: …, keys: [..]}'`, run by a
  human. No bulk certification is possible or intended.
- **`ground` costs a vault load:** about 3.7 s cold over visionGraph. An agent
  should batch a question's names into one call.
- **Consumers that build `ClassRecord` with a struct literal must add
  `certification`.** This is a breaking change for a 0.x minor release.
- **Activation needs:**
  - an ontomark release;
  - the visionGraph publisher bumping `ONTOMARK_RELEASE_TAG` and
    `ONTOMARK_SHA256`;
  - `context/v2.jsonld` added to the publisher's required-artefact gate and to
    `quartz/scripts/check-urls.sh`.

  Until then v0.7.3 serves `ns/v2.jsonld` as a copy of v1, because the stale
  static override is gone.
- **`corpus-visiongraph` golden hashes** must be re-captured at that release.

## Open questions

- A hand edit to a value after it was certified leaves the key marked certified.
  Binding a stamp to a digest of the values it covers would close that gap. It
  would also make `keys` awkward to write by hand, so it is not done yet.
- `ground` grades names lexically. An embedding tier (RuVector, already in the
  estate) would catch paraphrases. It would also make the grade
  non-deterministic, which this ADR avoids.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo doc --no-deps` (ingest, compile) with `-D warnings`, and
  `cargo build -p ontomark-ingest --target wasm32-unknown-unknown`: all pass at
  `ef48510`.
- visionGraph (vocabulary v2 branch), with a page carrying
  `verified: [{by, at, keys: [is-a, enables]}]`:
  - v0.7.3 and this branch both validate with 0 errors and 28,805 warnings,
    unchanged.
  - Both build.
  - `api/pages/fungible-token.json` gives `certification: {state: certified,
    keys: [enables, is-a]}`.
  - No page API key is unmapped in v2.
