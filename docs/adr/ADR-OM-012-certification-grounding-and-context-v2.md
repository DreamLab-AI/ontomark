---
id: ADR-OM-012
title: Certified keys are never patched, questions are graded for coverage, and the v2 context is generated and closed over the page API
date: 2026-11-04
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: 32b3aacdd9531d06c8c7b256828601364dd34034
verified_paths: [crates/ontomark-ingest/src/certification.rs, crates/ontomark-ingest/src/accept.rs, crates/ontomark-ingest/src/judge.rs, crates/ontomark-ingest/src/coverage.rs, crates/ontomark-ingest/tests/certification.rs, crates/ontomark-ingest/tests/coverage.rs, crates/ontomark-cli/src/ground_cmd.rs, crates/ontomark-cli/tests/conformance.rs, crates/ontomark-compile/src/input/okf.rs, crates/ontomark-compile/src/ir/model.rs, crates/ontomark-compile/src/build/page_api.rs, crates/ontomark-compile/src/build/okf.rs, crates/ontomark-compile/src/build/mod.rs, crates/ontomark-compile/src/write/edit.rs, conformance/fixtures/estate-small.golden/MANIFEST, crates/ontomark-compile/src/build/attribution.rs, crates/ontomark-parse/src/certification.rs, crates/ontomark-cli/src/ingest_cmd.rs, crates/ontomark-compile/src/ir/projection.rs]
owner: jjohare
review_trigger: the first `verified` stamp written with `keys`; a release that the visionGraph publisher pins; a second actor kind admitted as human; a page API consumer that cannot take the added `certification` field
repo: ontomark
evidence_class: source+test+runtime
acceptance_boundary: released as ontomark v0.8.0 (GitHub release assets built by CI; crates ontomark-compile 0.7.0, ontomark-ingest 0.2.1, ontomark-profile-estate 0.8.0, ontomark 0.8.0 on crates.io), pinned by the visionGraph publisher and served by narrativegoldmine.com on 2026-11-04; corpus-visiongraph golden re-captured at the release
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

   > **Amended 2026-11-25 by ADR-OM-014; this text is kept as decided.** Under
   > vocabulary 3 a `verified` stamp by `did:nostr:<64 hex>` also certifies,
   > provided the attribution roster resolves that key to `kind: human`. A key
   > resolves to a principal's kind only when it is the principal's own key
   > (its canonical `did` or one listed under `also`). An instrument, swarm or
   > process key filed under a human principal keeps its own kind, so it never
   > certifies. Under vocabulary 2 the roster is not read and this paragraph's
   > rule stands unchanged. The conformance table now pins only the `human:`
   > prefix rule. See "Re-verification — 2026-11-25".

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
- **No triple changes, measured two ways over the 9,609-page visionGraph
  corpus.**
  - Same vault: this branch, ontomark `main` and v0.7.3 produce the same
    `ontology.ttl`, and the same inferred, scaffold and prose indexes once
    `generated_at` is masked.
  - Same binary (v0.7.3): visionGraph `main` (vocabulary v1) and the v2 branch
    produce the same file set. Only the two generation markers and `okf/index.md`
    differ, by `vocabulary_version` 1 → 2 and the content digest. No artefact names
    the `context/v2` URL.

  So VisionFlow ADR-2022's corpus history records nothing from this change.
- **Day-one effect: one page.** *Fungible Token* already carries a whole-page
  human stamp, so it becomes fully certified:
  - `accept` withholds every proposal for it as `certified`;
  - its page API document reports `state: certified`.

  No page carries `keys` yet. The only write path is
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

- Review freshness is now specified by ADR-OM-013: changed bound values become stale; historical protection remains. The original v2 certification field continues to describe that protection.
  The explicit `review-binding` command prepares a digest without recertifying or writing a stamp.
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

## Activation (2026-11-04)

- **Released** as `v0.8.0` (`1176316b0`). The release workflow built the static CLI;
  its sha256 is `a1dfb1e913d5fe38b7b694dac82c8e2b44b00699382999a08cf56933c6c10d64`.
  On crates.io: `ontomark-compile` 0.7.0 (the new `ClassRecord` field is breaking),
  `ontomark-ingest` 0.2.1, `ontomark-profile-estate` 0.8.0 and `ontomark` 0.8.0.
- **Pinned and served.** visionGraph's publisher pins v0.8.0 (ADR-VG-009). On
  narrativegoldmine.com, `context/v1.jsonld` keeps its bytes (`452d5ce0…`), and
  `context/v2.jsonld` and `ns/v2.jsonld` serve the same generated document
  (`4da97d77…`, 84 terms including `certification`), byte-identical to a local
  build with the release binary. Fungible Token's page document reports
  `certification: {state: certified, keys: []}`. Of 9,595 page documents, 9,594
  report `pending` and 1 reports `certified`.
- **Golden.** `corpus-visiongraph` was re-captured with the v0.8.0 release build at
  visionGraph `5ba194ae3`.
- **Agentbox** pins v0.8.0 (agentbox ADR-2136). Agents get `ontomark ground` at
  the next image rebuild.


## Re-verification — 2026-11-15

ADR-OM-013 adds explicit freshness without changing v2 context meanings. Keyed,
whole-page, stale and unbound ingest protection pass the certification controls;
CLI export tests confirm legacy API separation and public projection boundaries.

## Re-verification — 2026-11-25

Re-verified at `32b3aacdd9531d06c8c7b256828601364dd34034` (release/v3, 0.10.0)
against `git diff 59e3099..32b3aac` over the governed paths, claim by claim.

1. **Who certifies: amended (see the note under Decision 1).** Humanness is
   now a test the host supplies. `ontomark_parse::certification::assess_with`
   (`crates/ontomark-parse/src/certification.rs:74`) takes it, and `assess`
   still passes the `human:` prefix test (`:38`, `:50`). The compiler's
   `Certification::for_page_with` (`input/okf.rs:456`) accepts the prefix *or*
   `Roster::is_human_by`. That resolves through `Roster::resolve_principal`
   (`build/attribution.rs:219`), where a registry principal's own keys carry its
   `kind` and a generator keeps its own stated `kind` whatever principal it is
   filed under. The build passes a roster only under vocabulary 3
   (`build/mod.rs:673-677`), on every projection path
   (`ir/projection.rs:647-868`). `Roster::human_dids`
   (`build/attribution.rs:260`) lists every principal of kind human with its
   `also` keys. It also lists any generator that states `kind: human` itself
   and that no principal lists.
   Limits at this revision:
   - Only the CLI fills ingest's roster keys from the roster
     (`ingest_cmd.rs:537-543`, vocabulary 3). `Options::human_dids` defaults to
     empty. The WASM engine accepts it in the options its host passes, but
     nothing in this repository fills it there. A host that does not pass it
     gets the prefix rule only, and a `did:nostr:` stamp does not protect its
     keys. Whether the Obsidian plugin passes it was not checked; that code is
     in another repository.
   - Under vocabulary 3 a `human:` stamp is an `INVALID_STAMP` error
     (`check/v3/stamps.rs:225-235`), but certification still counts it through
     the prefix rule. A vault with such a stamp fails `validate` and `build`,
     so nothing publishes it. Ingest still protects the stamp's keys.
   - `ingest_and_compile_agree_on_human_actors`
     (`crates/ontomark-cli/tests/conformance.rs:356`) pins only the prefix rule.
     Roster humanness is pinned by `attribution.rs`'s unit tests and by
     `vocab_v3.rs::an_instrument_key_does_not_certify_and_an_empty_stamp_is_an_error`.
2. **Ingest never adds a value to a certified key: holds.** `accept` and the
   judged path both read certification from the note, now through
   `Certification::of_with` with `Options::human_dids` (`accept.rs:390`,
   `judge.rs:665`). The withheld reason is still `certified`.
3. **The page API reports certification: holds.** The field is unchanged. It
   is now roster-aware under vocabulary 3 (`ir/model.rs:696`).
4. **Coverage grading: holds.** `coverage.rs` and `ground_cmd.rs` are unchanged.
5. **Context v2: holds.** `okf::context_v2` is unchanged. Vocabulary 3 adds
   `context/v4.jsonld` beside it (ADR-OM-014). The v3-only page keys
   (`validFrom`, `validUntil`, `recallable`, `sources`, `provenance.via`)
   appear only in `api/v3/pages` documents (`page_api.rs`, `build_v3_at`), and
   context v4 maps them. `api/pages` gains no key, and
   `context_v2_has_a_term_for_every_page_api_key` (`page_api.rs:444`) passes.
   `context/v1.jsonld` and the estate-small golden `MANIFEST` are unchanged.

Consequences re-read:
- "The only write path is `ontomark edit --set 'verified+=…'`" still holds for
  adding a stamp. `verified` is not an engine-owned key in `ontomark-write`, and
  `edit` now writes through that engine. Under vocabulary 3 the command as
  recorded, `verified+={by: human:…}`, writes a stamp that fails `validate` as
  `INVALID_STAMP`. The engine's v3 gate refuses that write, because it would add
  an error, so the stamp must name the `did:nostr:` key instead.
- m0002 rewrites the `by` of existing `verified` entries to the principal's
  `did:nostr:` form. It adds no binding (`migrate/m0002/mod.rs`, module docs).
  No bulk certification exists.

Four files are added to `verified_paths` because claim 1 now depends on them:
`build/attribution.rs`, `ontomark-parse/src/certification.rs`,
`ingest_cmd.rs` and `ir/projection.rs`.
