---
id: ADR-OM-014
title: Vocabulary 3 behind a version switch, one validated write path for the CLI, and estate-temporal-v2
date: 2026-11-25
decision_status: accepted
implementation_status: partial
activation_status: staged
supersedes: []
superseded_by: []
amends: [ADR-OM-012]
verified_commit: d186bfa5fc631d0fef7a55359f6d129946222009
verified_paths: [crates/ontomark-compile/src/input/vocabulary.rs, crates/ontomark-compile/src/input/time.rs, crates/ontomark-compile/src/input/principal.rs, crates/ontomark-compile/src/input/okf.rs, crates/ontomark-compile/src/check/v3, crates/ontomark-compile/src/check/validate.rs, crates/ontomark-compile/src/build/attribution.rs, crates/ontomark-compile/src/build/turtle.rs, crates/ontomark-compile/src/build/okf.rs, crates/ontomark-compile/src/build/page_api.rs, crates/ontomark-compile/src/build/redact.rs, crates/ontomark-compile/src/build/mod.rs, crates/ontomark-compile/src/ir/model.rs, crates/ontomark-compile/src/migrate, crates/ontomark-write, crates/ontomark-cli/src/main.rs, crates/ontomark-cli/src/memory_cmd.rs, crates/ontomark-cli/tests/vocab_v3.rs, crates/ontomark-cli/tests/vocab-v3-vault, crates/ontomark-cli/tests/conformance.rs, crates/ontomark-wasm/Cargo.toml, conformance/fixtures/estate-small.golden, conformance/temporal/vectors.json, conformance/temporal/vectors-v2.json, crates/ontomark-reason/tests/temporal_contract.rs]
owner: jjohare
review_trigger: the owner tags 0.10.0 or a consumer pins it; the Obsidian plugin is wired to ontomark-write; a vault other than visionGraph adopts vocabulary 3; VisionClaw adopts estate-temporal-v2 or the last consumer of v1 moves; memory-ontology changes an IRI used here
repo: ontomark
evidence_class: source+test
acceptance_boundary: implemented on release/v3 (verified at a641b81, after the human: and journal fixes; the rest as at 32b3aac) with manifests at 0.10.0; not tagged, released or published; no consumer pins it; no real vault runs vocabulary 3 yet; the Obsidian plugin is not wired to the write engine
---

# ADR-OM-014 — Vocabulary 3 behind a version switch, one validated write path for the CLI, and estate-temporal-v2

## Context

visionGraph's design records ADR-VG-010 to ADR-VG-013 define a third vocabulary
for its two vaults. The new vocabulary adds:

- authored valid time and supersession on pages;
- typed sources;
- stamps that name a principal key and the instrument it directed;
- shared working memory with visibility and a review lifecycle.

The estate settled them as one specification, with rulings B1 to B13 and later
amendments, in a locked plan of 2026-11-25. In the same pass the estate moved
its temporal contract from `estate-temporal-v1` to v2, with closure records
replacing bare end dates. VisionClaw ADR-2219 and VisionFlow ADR-2022 record
that move for their side. This record covers ontomark's part: the decisions the
code at 32b3aac takes. A plan or report is cited only where the code bears it out.

The forces:

- **Every published visionGraph build and every consumer runs vocabulary 1 or
  2 today.** A release that changes their output without an opt-in would move
  the published corpus and its corpus history (VisionFlow ADR-2022) by accident.
- **Certification had a hazard.** It counted only `human:` stamps (ADR-OM-012).
  The migration rewrites those stamps to `did:nostr:` keys. If certification
  stayed as it was, the one certified page (*Fungible Token*) would have
  reverted to `pending`, and ingest would have stopped protecting its keys.
- **The swarm keys are filed under the operator's principal in the roster.**
  A naive principal lookup would therefore let a swarm's stamp count as a
  person's review. The final review found exactly this, and it was fixed
  before merge.
- **Writes were scattered.** Before this release `edit` re-serialised
  frontmatter and validated nothing. `create` and `apply` validated in their
  own preparation, but without the other vault or the roster, and each wrote
  files its own way. None of these writes stamped who wrote, and v3's review
  lifecycle needs both stamping and full validation.
- **memory-ontology (eljeffeg, CC BY 4.0) already names the two clocks.** The
  operator chose to use its IRIs where the meaning is the same.

## Decision

Vocabulary 3 is opt-in per vault through `ontology/vocabulary.yaml`'s
`version`. Below 3 the tool's validate, build, conflicts and repair output is
what 0.9.0 produced, except for three changes named under Consequences. At 3 or
above, ontomark enforces ADR-VG-010 to ADR-VG-013 as ruled:

- the stamp grammar, with humanness taken from the roster principal's own key;
- supersession that closes one named clock per reason;
- typed sources;
- working-page visibility.

It emits the new facts with memory-ontology's time terms. Migration m0002
rewrites a vocabulary 2 vault to version 3 losslessly, or refuses. The CLI's
page writes (`edit`, `create`, `apply` and the new `memory` verbs) go through
one engine, `ontomark-write`. Under vocabulary 3 that engine validates every
write and stamps working-page writes. ontomark carries estate-temporal-v2 as
its executable contract, and keeps v1 byte-identical until its last consumer
leaves.

### 1. The version switch

- `Vocabulary::is_v3` (`input/vocabulary.rs:573`, `V3 = 3` at `:374`) is the
  only switch. Every v3 rule, emission and roster read checks it. Under version
  2 the roster is not read for rules, certification or emission.
- The v3 knowledge keys (`valid-from`, `valid-to`, `supersedes`,
  `superseded-by`, `statements`) are known keys only under version 3. Below
  that they remain `UNKNOWN_KEY`.
- Guard: the estate-small golden (`conformance/fixtures/estate-small.golden`, a
  vocabulary 1 fixture) is checked by
  `conformance.rs::build_retains_legacy_bytes_except_versioned_provenance_additions`
  and `verbs_print_the_same_bytes_and_exit_codes`. The fixture and the test are
  unchanged since 59e3099 and pass at 32b3aac. `vocab_v3.rs` has two tests:
  `the_fixture_vault_exercises_every_v3_code` asserts the 43 v3 codes fire, and
  `the_same_vault_under_version_2_reports_none_of_them` asserts that no v3 code
  except `INVALID_STAMP` fires under version 2. It allows `INVALID_STAMP` but
  does not require it.

### 2. Stamps and the roster rule

- Under version 3 the `by` of a stamp (`generated`, `verified[]`,
  `contributors[]`, and entries of `supersedes` and `statements`) is
  `did:nostr:` followed by 64 lowercase hex characters. `Actor::Did` parses it
  (`input/okf.rs`).
- A `human:` value in `by` is an `INVALID_STAMP` error that names the migration
  (`check/v3/stamps.rs:225-235`). `via` names an `agent:` or `process:`
  instrument.
- `npub` and hex convert losslessly through `bech32`, pinned `=0.11.1`
  (`input/principal.rs`).
- Codes: `INVALID_STAMP` (a warning under version 2, an error under version 3),
  `NON_CANONICAL_PRINCIPAL` (warning), `VERIFIED_NOT_HUMAN` (error) and
  `VERIFIED_PRINCIPAL_UNKNOWN` (warning).
- **Roster rule.** `Roster::resolve_principal` (`build/attribution.rs:219`) is
  the one place a key's kind is decided:
  - A registry principal's own keys (its canonical `did`, or one listed under
    `also`) carry the principal's `kind`.
  - A generator entry gives only the canonical DID it acts for. Its kind is its
    own stated `kind`, so a swarm, generator or process key filed under a human
    is never human.
- The roster rule feeds `VERIFIED_NOT_HUMAN`, certification (`is_human_actor`,
  `is_human_by`, `:247` and `:294`), ingest's key list (`human_dids`, `:264`)
  and the write engine's approver check. m0002 builds its own list of human
  keys from the roster file. It is consistent with the rule but separate code.
- Certification under version 3 therefore counts a `verified` stamp by such a
  key as human, and counts nothing else: the old `human:<npub or label>`
  spelling does not certify (it is an `INVALID_STAMP` error), with a roster or
  without one. Under version 2 the `human:` prefix rule is as released. This
  amends ADR-OM-012 Decision 1; that record carries the note and the details.

### 3. Supersession and the two clocks

- `input/time.rs::clocks_closed` (`:74`) is the only reason-to-clock function
  (ruling B1):
  - `world-changed` closes valid time, and the superseded page carries
    `valid-to`;
  - `corrected` and `withdrawn` close recorded time, so every withdrawal closes
    a clock;
  - `merged` and `renamed` are identity curation and close neither.
- `check/v3/temporal.rs` enforces the rest:
  - strict `YYYY-MM-DDTHH:MM:SSZ` times;
  - `valid-to` only with a closing status;
  - `deprecated` needs a successor or its own withdrawal entry;
  - no supersession cycles;
  - no live relation to a deprecated page;
  - closed `statements` absent from the live list.
- Known gaps in `temporal.rs`:
  - A `world-changed` entry whose target has no `valid-to` is only a warning
    (`SUPERSESSION_CLOCK`).
  - Cycles and superseded targets are reported by `validate`, not by
    `conflicts`.

### 4. Typed sources and `BRIEF_AS_EVIDENCE`

- Under version 3 a `sources` entry may carry `rel` (one of `origin`,
  `episode`, `evidence`, `derived-from`, `standard` or `reference`), `at` and
  `locator`. Fields the type does not model are preserved
  (`input/okf.rs`, `Source`).
- The four resolving rels need a `resource` that resolves. A wikilink and a
  page `urn:visionflow:page:` id get the same checks (`DANGLING_SOURCE` is an
  error when both vaults are loaded).
- A journal is a page for this purpose. The sibling index holds journals, so
  one resolves by its `page_resource` urn and by wikilink (`[[2026-11-25]]`,
  or `journals/2026-11-25`), for every `rel`. ADR-VG-010 section 5 has one
  target-type rule, which is the Brief rule below, so a journal may be the
  `origin`, `evidence` or any other `rel` of a source; a Proposal's `origin` is
  the motivating case. The locator checks apply to `Episode` ledgers only and
  `EVIDENCE_NOT_APPROVED` to memory kinds, so a journal gets neither.
- A journal is never public. Under version 3 the staged copy of a public page
  drops a whole `sources` entry (its `id`, `rel`, `at` and `locator` as well as
  the `resource`) when the target is private or names no page the build can
  see, and removes that `id` from every `statements[].sources` list (the list
  goes when it empties). `ontology.ttl`, the page API, `history.ttl` and the context omit it
  (`build/redact.rs`, `PrivateLinks::source_withheld`;
  `v3_sources.rs::a_public_page_citing_a_journal_publishes_none_of_it`).
- `BRIEF_AS_EVIDENCE` is an error for `rel: evidence` or `origin` pointing at a
  `Brief` (`check/v3/sources.rs:196-200`): a brief summarises evidence and is
  not itself evidence.
- Pins are ontomark's own `sha256-v1:` page digest, the value `review-binding`
  prints. A stale `derived-from` pin is a warning, and so is an Episode without
  `rel: origin` (`MISSING_ORIGIN`, until the migration lists no unmatched
  ledgers).

### 5. Visibility

- `check/v3/state.rs::is_public_under` (`:89`) is the publish test:
  - under version 3, a working page is public only when it says
    `visibility: public`;
  - knowledge pages, and every page under version 2, are public by
    `public: true`.
- The publisher, redaction, SHACL, the link graph, migration planning and the
  projection all call it.
- `visibility: private` is an error (`PRIVATE_UNSUPPORTED`) because no private
  store exists yet (ADR-VG-012 §5).
- Under version 3 a working page must carry `page_resource`, `visibility` and
  `generated`, unless the vocabulary names other required keys. Journals are
  included.

### 6. Emission under version 3

The terms emitted:

- **`data/ontology.ttl`:**
  - a page's authored start and end as memory-ontology's `temporal:validFrom`
    and `temporal:validUntil` (`http://nova.dev/temporal#`,
    `xsd:dateTimeStamp`; `build/turtle.rs:45`, `:865-933`);
  - `owl:deprecated`, `dcterms:isReplacedBy`, and `prov:wasRevisionOf` with the
    reason as an `owl:Axiom` annotation;
  - `prov:wasAttributedTo` for the principal;
  - `prov:actedOnBehalfOf` from each instrument the roster resolves to its
    principal;
  - sources by rel (`prov:wasDerivedFrom`, `prov:hadPrimarySource`,
    `dcterms:source`).
- **The reasoner and certificate input:** a deprecated class and its axioms
  leave it (`build/mod.rs:822`).
- **`data/history.ttl`:** closed statements are written here as non-asserting
  `rdf:Statement` records, never into the ontology the reasoner reads
  (`build/mod.rs:936-948`, `build/turtle.rs:1104`). The file is always written
  under version 3, as a prologue only when no page has a closed statement.
- **`context/v4.jsonld` and `ns/v4.jsonld`:** written under version 3 only
  (`build/okf.rs:203`, `build/mod.rs:1019-1025`). They map the `api/v3/pages`
  keys `validFrom`, `validUntil`, `recallable`, `sources` and `provenance.via`.
  For a version 2 vocabulary, contexts v1 to v3 keep their bytes.

The two clock concepts `memory:ValidTime` and `memory:RecordedTime` are not
emitted by the compiler. They appear by IRI only in the temporal contract
(§9). `NOTICE` credits the memory-ontology terms under CC BY 4.0 and states
that no text or axioms are copied. No conformance to memory-ontology's shapes is
claimed: its records reify with RDF 1.2 `rdf:reifies`, and none of ontomark's
output does.

### 7. Migration m0002-vocabulary-v3

`ontomark migrate` now has two migrations, m0001 and m0002, in that order
(`migrate/mod.rs:175`). m0002 (`migrate/m0002/`):

- **Dry run first.** `migrate plan` writes nothing to the vault. It reports a
  classified diff and lists what it would not migrate. m0002 is
  `Kind::Semantic`, so `migrate apply` needs `--accept-semantic`.
- **Lossless.** Every page edit is a guarded `ontomark-parse` patch. Unnamed
  keys, unknown keys and key order are kept, and the body is never touched. The
  vocabulary and roster are edited as text, with comments kept. Both are
  planned, journalled and rolled back like pages.
- **Idempotent.** A migrated page transforms to `Unchanged`.
- **Refuses on an invalid result.** The plan validates the whole migrated tree
  under the migrated vocabulary and roster, and refuses on any error
  (`migrate/plan.rs:632`, `:708`). A page it cannot derive is refused with the
  reason and left alone; it never guesses.
- **Supersessions input.** The list of merged losers comes from
  `ontology/m0002-supersessions.yaml`, or from the file named by
  `ONTOMARK_M0002_SUPERSESSIONS`. That file must lie outside the repository,
  and when the variable is set the in-tree file is neither read nor listed.
  Its SHA-256 is bound into the plan, and `apply` refuses a different or
  missing input (`migrate/apply.rs:140-146`).
- A `verified` entry keeps its absent review binding: m0002 rewrites its `by`
  and adds no digest.

### 8. The write engine

- `crates/ontomark-write` is a new, publishable crate (0.1.0). Its core does no
  I/O, reads no clock and draws no randomness, and it builds for `wasm32` with
  `--no-default-features`.
- Under vocabulary 3 a write goes through these steps:
  1. refuse engine-owned keys and illegal transitions;
  2. build the new text through a lossless patch;
  3. on working pages only, stamp `contributors` (one entry per write) and the
     clocks;
  4. validate before and after with siblings and roster;
  5. refuse any error the page did not already carry.
  A knowledge-page `patch` under vocabulary 3 is therefore validated, and
  `generated` and `page_resource` cannot be changed once present, but it is not
  stamped. The gate in `ops/patch.rs:237-242` is keyed on `is_v3()` alone. The
  method's doc comment (`ops/patch.rs:95-101`) still says a knowledge page gets
  "no new validation". The code wins; the comment is stale.
- `Plan::commit` writes through a root-confined `FsStore`. Each file is written
  atomically; a plan is ordered, not crash-atomic.
- **CLI.** `edit` takes its bytes from the engine (`main.rs`, `Command::Edit`),
  and under version 3 a working-page edit needs `--by` or `ONTOMARK_PRINCIPAL`.
  `create` and `apply` keep their own preparation, validation
  (`prepare_with`, now given the other vault and the roster under version 3)
  and errors. Only their final write is committed through the engine's store
  (`memory_cmd.rs:422`, `:446`); they are not stamped. The
  `memory` verbs (`create`, `adopt`, `approve`, `reject`, `confirm`,
  `send-back`, `withdraw`, `archive`, `dispute`, `complete`, `resource`) call
  the engine directly.
- **Version 2.** `patch` is a lossless pass-through: no stamps and no new
  validation. Every other operation is refused with `VOCABULARY_V2`.
- **Not routed, on purpose:**
  - `migrate apply` and `rollback` have their own journal, digest guard and
    rollback. Wrapping them would duplicate that recovery path.
  - `repair bodies` and `repair fences` rewrite bodies in place under their own
    report.
  - `ingest` writes a proposal bundle under `.ontomark/ingest/`, outside the
    vaults, never a page.
- **The Obsidian plugin is not wired to it.** `ontomark-wasm` does not depend
  on `ontomark-write` (`crates/ontomark-wasm/Cargo.toml`). Editor writes
  therefore still bypass the engine's stamps and validation.

### 9. estate-temporal-v2 in `conformance/temporal/`

`vectors-v2.json` holds 17 versions, 12 closures and 41 cases; its first 9 cases
and 3 versions are v1's, verbatim. The README defines:

- **Versions**: a version never carries an end. It has `temporal:validFrom`,
  and `temporal:recordedFrom` as its recorded time.
- **Closure records**: `dl:Closure`, `dl:closesVersion`, and `dl:closes` with
  one or both of `memory:ValidTime` and `memory:RecordedTime`. A closure
  carries `dl:validUntil` if and only if it closes valid time, and its own
  `prov:generatedAtTime`. A closure is never closed.
- **The state rule**: a version is in `state(t, r)` if and only if all four
  clauses hold. A recorded-time closure applies once recorded; a valid-time
  closure applies once recorded and once its end has passed. The effective end
  is the minimum.

`v1` (`vectors.json`, `estate-temporal-v1`) is kept byte for byte and marked
superseded, because VisionClaw still pins it. Both pins are checked against the
files and against the README by `temporal_contract.rs::pins_match_files_and_readme`:

```
334425f34523d63224fff92cde1c4268c18cdb989b479c731d271516c905a873  vectors-v2.json
6973aeef35e538412375dd6e4a9d9539f12f88fa7513d237371098da3bc46f75  vectors.json
```

Negative tests show the evaluator can fail (`the_check_can_fail`,
`the_added_vectors_kill_the_wrong_readings`).

## Consequences

**Easier.**

- A vault opts in by changing one number, and the migration that gets it there
  can be planned, reviewed, applied and rolled back.
- Who is answerable is a key, not a spelling. Certification, validation, ingest
  and approval all take humanness from one function.
- The CLI's page writes are lossless, stamped and validated in one place.

**v2-visible changes (the exceptions to §1).** Each is deliberate:

- `INVALID_STAMP` is a new warning for a `generated` or `verified` value that
  0.9.0 silently dropped. visionGraph has none.
- `edit` output is lossless instead of re-serialised: comments, odd YAML styles
  and key order now survive.
- `edit`, `create` and `apply` refuse to write through an in-vault symlink,
  which 0.9.0's `edit` followed.

**Harder or foreclosed.**

- Under version 3 the legacy `human:` spelling, `visibility: private` and a
  dangling source are errors, so a vault cannot carry them through.
- `ClassRecord` gains `v3: Option<Box<V3Facts>>`: a struct-literal consumer must
  add it.
- Under version 3 every published page's `provenance.wasAttributedTo` becomes
  the principal, not the instrument's DID. A consumer keyed on swarm DIDs must
  read `via` or `prov:actedOnBehalfOf`.

**Known limits at 32b3aac.**

- The plugin is not wired to `ontomark-write` (§8).
- `create::write`, `apply::write` and `edit::apply`'s rendered output remain
  public and unused by the CLI.
- `Engine::patch`'s doc comment misdescribes knowledge-page validation under
  vocabulary 3 (§8).
- The CLI depends on `ontomark-write`, so publishing the CLI to crates.io
  needs `ontomark-write` 0.1.0 published first, after `ontomark-compile` 0.10.0.
- A `repair` of a working memory page bypasses `contributors` and
  `content-updated`.
- Who writes is asserted, not proved. With no roster, anyone may approve.
- Ingest's vocabulary 3 inputs (`human_dids`, `legacy_human_prefix`) are set
  only by the CLI. A host that embeds `ontomark-ingest` or the WASM engine and
  does not pass them keeps the vocabulary 2 rule (ADR-OM-012 re-verification).
  Every entry point that takes no vocabulary keeps the prefix rule too:
  `Certification::from_verified`, `for_page`, `for_page_with`,
  `OkfBlock::is_human_verified` and `is_human_verified_with` (the readings
  VisionClaw's oracle called, which is why the vocabulary-aware `_under` forms
  exist), parse's `assess`, ingest's `Certification::of`, `of_with` and
  `is_human_actor(&str)`, the WASM `review_binding` counts and
  `input/promotion.rs`. No non-test caller on a version 3 path uses one.
- The source IRIs differ between surfaces: `context/v4.jsonld` maps `sources`
  to `vc:source*`, while `ontology.ttl` uses `dcterms:source`,
  `prov:wasDerivedFrom` and `prov:hadPrimarySource`.
- `via` has no SHACL shape.
- Revalidation intervals (ruling B10) are implemented nowhere. Per-type status
  sets are built-in defaults unless the vocabulary states them.
- m0002 is estate-specific. It hard-codes the canonical principal
  (`CANONICAL_PRINCIPAL`), and reads its context at fixed visionGraph paths
  before the layout check (ADR-OM-011 re-verification).
- Episode `merged` and `renamed` curation is not offered by the engine.
- VisionClaw's adoption of estate-temporal-v2 is pending, so `vectors.json`
  cannot yet be deleted.

**Revisit if:**

- the plugin is wired;
- a second vault adopts vocabulary 3, since m0002's estate constants would then
  need to come from configuration;
- receipts or signatures arrive, since "asserted" would then become "proved";
- memory-ontology changes an IRI;
- VisionClaw moves to v2.

### Pending upstream move of the memory-ontology IRIs (noted 2026-11-26)

The `temporal:` and `memory:` IRIs used in emission and in the temporal
contract are under `http://nova.dev/`. memory-ontology's author has said that
namespace is a placeholder he does not own, and that he will register one under
w3id.org. His open change (eljeffeg/memory-ontology#5, not merged as of this
note) also moves the two clock concepts from `memory:` to `temporal:`, makes a
closing record the source of an end, and accepts classic reification in its
shapes. When the namespace is registered, the IRIs in emission, in
`context/v4.jsonld` and in `conformance/temporal/README.md` change together, in
one release. Until then they stay as they are, and nothing published under them
resolves.

## Rejected alternatives

- **Leaving certification prefix-only and treating any `Did` as human.**
  Rejected: an organisation's or a swarm's key would certify.
- **Taking the principal's kind for a generator filed under it.** This was the
  first implementation and was rejected in review: it launders an instrument's
  stamp into a person's review.
- **Writing closed statements as `owl:Axiom` in `ontology.ttl`.** Rejected: an
  OWL parser reads an annotated axiom back as asserted, re-asserting the closed
  fact.
- **Typing principal DIDs as `prov:Person` or `prov:SoftwareAgent`.**
  Rejected: a class assertion makes each DID an individual in a corpus that is
  pure TBox by design. `prov:actedOnBehalfOf` separates them instead.
- **Putting `temporal:validUntil` on versions in the temporal contract.**
  Rejected: an end on the earlier record, with no recorded time of its own, is
  the v1 defect. The published `ontology.ttl` may carry `temporal:validUntil`
  because it is a static projection with no recorded-time query.
- **Routing `migrate`, `repair` and `ingest` through the engine.** Rejected for
  now (see §8).
- **Minting `page_resource` from a title slug.** Rejected: a rename would
  change identity. m0002 derives it from the page id (journals too), and
  `memory create` draws it from host-supplied entropy.

## Verification

At `32b3aacdd9531d06c8c7b256828601364dd34034`, with
`CARGO_TARGET_DIR=wt/.target-om-release`:
`cargo test --workspace --offline --no-fail-fast` gave 1,363 passed, 1 failed
and 9 ignored. The failure is `ontomark-wasm`'s `transcript_matches_golden`.
Its golden transcript (`crates/ontomark-wasm/tests/fixtures/transcript.golden.jsonl`)
records version `0.9.0`, and 32b3aac bumped the crate to `0.10.0` without
re-blessing it. Apart from that string (7 occurrences) the transcript is
identical. The test does not exercise anything in this record, but it fails the
release's `cargo test --workspace` until the golden is re-blessed.

The run covers the tests cited above, including `conformance`, `vocab_v3`,
`m0002`, `m0002_external_input`, `migrate`, `layout_contract`,
`certification_freshness`, `review_binding_cli`, `memory_cli` and
`temporal_contract`.

Evidence from the implementers that this record relies on but did not re-run
(2026-11-25, out of repository):

- v2 golden: `validate`, `conflicts` and `repair` byte-identical to released
  0.9.0 on the live visionGraph tree.
- v2 build parity: 38,584 files, identical except for timestamp lines.
- A trial migration of a copy of visionGraph: 0 errors after apply, and a
  second plan with 0 changes.
- The real-vault lossless walk: 11,658 pages, 0 lossy.
