---
id: ADR-OM-003
title: "native, host-driven assisted ingest"
date: 2026-10-12
decision_status: proposed
implementation_status: partial
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: ""
verified_paths: []
owner: jjohare
review_trigger: "owner confirmation or rejection of the inferred acceptance; a second ingest slice beyond facts only; installed-Obsidian acceptance (verification item 5) recorded"
repo: ontomark
---

# ADR-OM-003: native, host-driven assisted ingest

- Status: Accepted (close-out 2026-10-30; see the note at the end)
- Implementation: complete for the engine scope (the first slice, facts only). `crates/ontomark-ingest` is published as `ontomark-ingest` 0.1.0, and `ontomark-wasm` exposes `ingest_plan` and `ingest_accept` (`crates/ontomark-wasm/src/engine.rs`). The plugin side belongs to ADR-OBS-002.
- Activation: staged (the crate is published and the plugin client calls both requests; installed-Obsidian acceptance, verification item 5, is not recorded)
- Related: [ADR-OM-002](ADR-OM-002-shared-graph-and-obsidian-boundary.md), [OntoCast triage](../ONTOCAST-TRIAGE.md), [ADR-OBS-002](https://github.com/DreamLab-AI/ontomark-obsidian/blob/main/docs/adr/ADR-OBS-002-assisted-ingest-boundary-and-estate-notes.md), [upstream credits](../UPSTREAMS.md)

Status mapping 2026-10-31: the record's header says `Accepted (close-out 2026-10-30)`, but its close-out note says that acceptance is an inference from the owner publishing `ontomark-ingest` 0.1.0, not a recorded ratification (ontomark commit 154bca9). `decision_status` is therefore `proposed` until the owner confirms. `implementation_status: partial` because the record says `complete for the engine scope` only and its plugin side belongs to ADR-OBS-002; `activation_status: staged` is as stated. Owner to confirm acceptance and the implementation scope.

## Context

The estate wants optional, assisted extraction of typed structure from Markdown notes. [OntoCast](https://github.com/growgraph/ontocast) (Apache-2.0, Python) solves the adjacent problem well, but it is a service stack: LangGraph orchestration, vector stores, a triple store, an HTTP API and a document converter. The [triage](../ONTOCAST-TRIAGE.md) shows that most of its 67,000 lines are infrastructure Ontomark already replaces or deliberately excludes. What is worth carrying across is a small set of methods: heading-aware chunking, deterministic entity-merge guards, deterministic fact checks, and the render prompt contract.

Three constraints from the existing system shape the design:

1. **Markdown is the source of truth.** Query and reasoning never bypass the notes (EXP-12). An extraction result is therefore a note change, not an RDF graph to be stored.
2. **The engine does no I/O.** `ontomark-wasm` runs in a Blob worker and compiles to wasm32. It cannot call a model.
3. **The plugin is offline by contract.** EXP-11 and its network-isolated shim forbid any socket, DNS, fetch or XHR attempt, and the manifest says "Offline".

## Decision

### 1. A pure, two-phase core in a new crate

A new crate, `ontomark-ingest`, depends only on `ontomark-parse`, `ontomark-model` and `ontomark-reason`. It performs no I/O, spawns nothing and is `wasm32`-clean. Its public surface is a state machine the host drives:

```text
plan(note, revision, profile, vocabulary, options) -> ModelRequest
accept(revision, ModelResponse)                    -> Candidates
```

```mermaid
sequenceDiagram
    participant H as Host (plugin, Loom, Agentbox)
    participant I as ontomark-ingest
    participant M as Model (host-supplied)
    participant P as ontomark-parse

    H->>I: plan(note, profile, model)
    I-->>H: ModelRequest {instructions, keys, chunks, schema, digest}
    H->>M: request as JSON
    M-->>H: ModelResponse[] {key, name or literal, evidence quote}
    H->>I: accept(note, request, responses)
    Note over I: digest check · locate evidence · ground names<br/>(exact → lexical tiers → suggestions) · XSD checks · kind guard
    I->>P: Patch bound to digest, applied once in memory to prove it
    I-->>H: Candidates {patch?, accepted[], withheld[reason], diagnostics[]}
    H->>H: diff preview; apply on confirm; undo stays available
```

- `ModelRequest` is plain data: the prompt contract (a versioned data file, not code), the chunks, the vocabulary slice the model may use, and the expected response schema. The host sends it to whatever model it chooses.
- `Candidates` holds `ontomark-parse` patches bound to the source digest, an evidence span for each asserted value, and diagnostics. A `ModelResponse` that is malformed, empty or out of vocabulary yields diagnostics and no patch, never a partial edit.
- Spans use the same UTF-16 conventions as `ontomark-parse` (`span.rs`, `utf16.rs`) so Obsidian offsets are correct.
- The "mock model" used in tests is simply recorded `ModelResponse` fixtures. Native and WASM parity is a byte-equality test over `Candidates`.

### 2. First slice: facts only, against the existing vocabulary

The first slice corresponds to OntoCast's `facts` mode. It proposes typed frontmatter values and relations using terms already in the vault's profile and vocabulary. It does not propose new classes or properties. Ontology growth collides with estate governance and is a later, separate candidate kind with its own ADR.

Identity uses `ontomark-model::mint`. Extracted source IRIs are kept as evidence, and project-local identity is re-minted, following the decision already taken in the retired RDF-to-Logseq adapter. No second identity scheme is introduced.

### 2a. Extraction contract: names in, grounded identifiers out

Following SPIRES ([Caufield et al., Bioinformatics 2024](https://academic.oup.com/bioinformatics/article/40/3/btae104/7612230), OntoGPT, BSD-3-Clause), the model is asked for **names and values, never identifiers**. The core grounds each name against the vault vocabulary by normalised string similarity, mints or resolves identity through `ontomark-model::mint`, and reports ungrounded or ambiguous names as diagnostics. A host may re-prompt the model with those diagnostics; the core does not. This removes the need to trust model-invented IRIs.

### 3. What is carried across from OntoCast

Only the **Port** rows of the [triage](../ONTOCAST-TRIAGE.md): heading-aware chunking, the deterministic merge guards, deterministic fact checks and literal repair, and the render prompt contract as data. Embedding-based candidate generation is optional and host-supplied; the lexical guards run in core. Rust building blocks are `strsim` (MIT: Jaro-Winkler, Levenshtein, Sørensen-Dice) and `textdistance` (MIT: Jaccard). `strsim` has no Jaccard and neither has Python `difflib`'s sequence ratio, so the ratio is implemented in the crate and fixture-tested, not assumed byte-equal to Python. SHACL, vector stores, converters, the HTTP API, LangChain and web grounding are not carried across. Behaviour is specified by fixtures first, not transliterated from Python.

### 4. Confidence and review

Candidates start with low confidence and an `inferred` method, as in the retired adapter. Ingest never raises confidence or promotes a statement. Applying a candidate is a user action over a diff preview, built on the same guarded-edit path as today's patches, and is undoable.

### 5. The offline contract is amended, not bypassed

Any model call is made by the **host**, never by the core. For the Obsidian plugin this is a decision for the plugin's own ADR, with these requirements recorded here so the engine API does not foreclose them:

- The adapter is off by default and calls only an endpoint the user configures.
- The README and manifest description disclose the network use.
- The credential is not stored in plaintext `data.json` without an explicit decision and warning.
- EXP-11 and the isolation shim must pass unchanged with ingest disabled. A separate, explicit test covers the enabled path.

### 6. Ownership

| Layer | Owns |
|---|---|
| `ontomark-ingest` | Request/response contract, chunking, guards, checks, candidates, evidence spans, native/WASM parity |
| `ontomark-wasm` | Exposing `plan` and `accept` to the worker |
| Host (plugin, Loom, Agentbox) | Model transport, credentials, scheduling, UI, undo |
| VisionFlow and visionGraph | Corpus vocabulary and publication policy |

### 7. Reference reading and licence cautions

- [edgextract](https://github.com/raphaelmansuy/edgextract) (Apache-2.0): ontology-constrained extraction with evidence offsets and a browser build. Design reference for `plan`/`accept`; it is sentence-level and does not emit RDF.
- [Open Ontologies](https://github.com/fabio-rovai/open-ontologies) (MIT, Rust): SHACL, plan/apply/rollback and lineage. Governance reference, not an extractor.
- [rudof](https://github.com/rudof-project/rudof) `shacl_validation` (MIT or Apache-2.0): optional native SHACL. Its wasm32 build is unverified, so it stays out of the WASM path.
- [cognee-rs](https://github.com/topoteretes/cognee-rs) (MIT or Apache-2.0): study its cassette-backed mock model; do not depend on it.
- VisionClaw is reported as AGPL-3.0-only (unverified here). Do not copy its code into this permissively licensed crate without an explicit decision.

## Consequences

- No Python, bridge or service enters a build, test or release artefact.
- The plugin can use ingest only after a new engine prerelease and an `engine.json` repin.
- Attribution is mandatory. The Apache-2.0 licence text, upstream commit `abb8d7b8d6db1a765d4f091a7423937c2cbc5d78` and the `CITATION.cff` reference are added to `UPSTREAMS.md` and `THIRD-PARTY-NOTICES.md` in the same change as the first adapted code.
- The nightly dream gates native contracts only. Ingest changes that touch WASM or the plugin need their dedicated acceptance harnesses.
- A second model pass (the critic), ontology growth, numeric and unit repair, and long-document preprocessing are deferred, not rejected.

## Deviations recorded during implementation

- **Strongest tier wins.** OntoCast merges every pair a tier accepts into one cluster. Grounding must choose one target, so terms matching at the strongest tier win and only a tie there is `Ambiguous`. Otherwise OntoCast's initials tier, which lets the one-word label `graph` alias `knowledge graph`, made every multi-word name ambiguous.
- **Lower-casing, not case-folding**, as in Python `str.lower()`: `ß` stays `ß`. The sequence ratio omits `difflib`'s junk heuristics, which only engage at 200 or more characters.
- **Suggestion scores are rounded to four decimal places** so they round-trip through any host's JSON float parser; `serde_json` without `float_roundtrip` returned a 17-digit score one ulp off.
- **Jaccard is implemented in the crate** (three lines) rather than taken from `textdistance`, to keep the WASM dependency inventory small.
- **Only `Mn` marks are dropped** in normalisation, as upstream's `unicodedata.category(c) == "Mn"` does, via `unicode-general-category`. The first cut dropped every mark (`Mn`, `Mc`, `Me`), which collapsed Indic words that differ by a vowel sign.

## Hardening after independent review

An Opus review of the first slice found seven defects, all fixed and pinned by tests before the slice was pushed:

1. **`accept` recomputes the request.** Keys, chunks and the note reference are rebuilt from the note, profile and options and must equal the request (`IngestError::RequestMismatch`). A request altered in transit or built by hand can neither name a key the profile does not grant (such as `iri`) nor carry a span outside the note, which previously panicked.
2. **Evidence must mention the value.** A located quote is not enough: for a name it has to contain the proposed text or a name of the grounded entity (as a phrase, or a whole word of two or more characters); for a literal, the value as proposed or as written. Otherwise `EvidenceDoesNotMention`. A one-letter quote no longer licenses anything.
3. **The written link must resolve back to the grounded IRI alone.** The bare note name is tried first, then the vault path (`[[a/Twin]]`); if neither resolves to exactly that entity, the value is `Unlinkable`. Candidates are keyed by IRI, not by written text, so two entities sharing a note name never merge and a re-run finds the link already present.
4. **A decimal comma is not a thousands separator.** Commas are stripped only when every group is three digits (`1,200`, `12,345,678.5`); `3,5` is refused rather than written as `35`.
5. **A key present with no value is absent.** `founded:` with nothing after it neither conflicts with a proposal nor leaves an empty list item behind.
6. **Several values for a single-valued key are all withheld** as `CompetingValues`, naming each other, rather than written as a list that a later run would have refused as a conflict.
7. **Matches never swallow trailing whitespace** when the whitespace-collapsed search reaches the end of a chunk.

The review confirmed `sequence_ratio` equals CPython `difflib.SequenceMatcher.ratio()` without autojunk, including tie-breaking, and found the offset handling otherwise correct.

A second pass from the same review, lower severity, was applied in the following commit:

- Promoting an existing scalar to a list keeps the scalar's YAML type; it is no longer re-written as a string.
- Literals are written typed (`true`, `1200`, `3.5` as YAML boolean, integer and float) so Obsidian types the property the way the RDF reads it; dates, URIs and strings stay strings.
- `xsd:int`, `short` and `byte` are range-checked; `xsd:decimal` refuses exponent forms; dates must be all digits; strings refuse C0 control characters other than tab, LF and CR.
- A name that is one of a term's own names is never that term's initials conflict; and a conflicting initial is not outranked by a match at a weaker tier than shared-label, which now withholds as `InitialsConflict` instead of silently writing the weak match.
- Frontmatter the parser could not read (`Unsupported` or `Malformed`) withholds every value as `UnpatchableKey` instead of failing the batch at the proof.
- Chunk limits are counted in characters, as the options say; a whitespace-only piece from an oversize split joins its neighbour; the module docs no longer claim that a chunk never spans two sections or that fenced code is never cut.
- The CamelCase split is ASCII-only and U+001C–U+001F count as whitespace, both as upstream.
- Third tranche (simplifications and efficiency): label tokens are computed once per term and stored, instead of re-tokenised for every assertion; `longest_match` reuses two rows; difflib's `real_quick_ratio` length bound skips the quadratic pass when it cannot reach the threshold; the oversize splitter tracks remaining characters instead of recounting; the comparison helpers are `pub(crate)` with only `jaccard`, `sequence_ratio` and the bound public; the unused profile parameter and a one-line wrapper are gone.
- API hygiene: `Term` and `Vocabulary` no longer derive `Deserialize` (their sorted private indexes cannot be rebuilt from JSON) and do not serialise those fields; `Options` derives serde with defaults so a host can pass a partial object; `EvidenceSpan::utf16` is a `{start, end}` struct like `Span`.

## Verification required before activation

1. Fixtures for every adapted guard, written failing first.
2. Native/WASM byte-equality of `Candidates` over the fixture set.
3. `cargo clippy -D warnings`, strict `cargo doc`, and a wasm32 build.
4. Repeat-run determinism, stale-revision refusal, malformed and empty responses, and cancellation.
5. For the plugin: installed-Obsidian acceptance of spans, repeat runs, stale edits, cancellation, undo and the model-unavailable case, with EXP-11 unchanged when ingest is disabled.

## Open questions

- Which section labels, if any, are worth keeping for long notes.
- Where the host-supplied embedding similarity enters the API, or whether the first slice omits it.
- Whether ingest candidates target frontmatter only, or also body text.

## Engine boundary (12 October 2026)

The portable engine (`ontomark-wasm`) exposes the contract as two generation-guarded requests, so the native API, the WASM binding and the plugin worker share one schema and the transcript proves them byte-equal:

- `ingest_plan { generation, path, options? }` → `{ path, digest, request }`. Pure; the host sends `request` to a model it controls.
- `ingest_accept { generation, path, request, responses, options? }` → `{ path, candidates, preview }`. `preview` is the `propose_patch` shape (`new_text`, `new_digest`, `diff`, `edits`) when a patch exists, so the plugin's existing diff review is reused without a second round trip.

Wire codes for the crate's errors: `not_an_entity`, `no_assertion_keys`, `stale_base`, `contract_mismatch`, `request_mismatch`, `unknown_chunk`, `patch_refused`. Everything the model got wrong is in `candidates.withheld`, never an error.

**The two generic grammars.** Wiring this exposed that the engine's released profile (`type: class` case-insensitively, `subclass-of`/`is-a`, page IRIs minted as `urn:ontomark:page:<path>`) and `ontomark-model`'s `generic/1` (exact OWL kind names, CURIE keys only, no implicit identity) are different languages. A `Model` built from the engine's documents with `generic/1` indexes nothing in an ordinary vault. The fix is the explicit mapping EXP-10 always required: each engine `Profile` now also implements `ontomark_model::Profile`, reading a vault by exactly the engine's grammar, and ingest builds its `Model` through that bridge. `crates/ontomark-wasm/tests/profile_parity.rs` asserts the engine projection and the typed model index the same `(IRI, label)` set over the fixture vault under both profiles. The engine's own axioms still come from its projection; replacing that remains gated on whole-transcript parity.

To let a profile derive identity from the note's path without breaking the published model crate, `ontomark_model::Profile` gained one additive method, `identity_for(id, doc)`, defaulting to `identity(doc)`. The declarative profiles do not override it, so `generic/1` and the estate still never mint implicitly.

**Keys are offered by kind.** `plan` now filters the profile's predicates by the note's kind using the model's own subject rules (`contract::applies_to`): a class is never offered `instance-of`, an individual never `subclass-of`, a property only its hierarchy. `accept` recomputes the same list, so a hand-built request cannot widen it. The engine bridge declares `domain` and `range` as annotations (the typed model has no property-axiom form), so a class note is still offered them; the engine's projection reads them itself, and a future property-axiom form in the model removes the looseness.

Deviations between the bridge and the engine projection, recorded rather than hidden: the engine reads a `subclass-of` key on an individual as a type assertion; the bridge declares the key as `SubClassOf`, so on an individual the model rejects it with a diagnostic (the fixture vault's individuals use `instance-of`, so parity holds). A `type` value that names no kind is an ordinary note on both sides.

## Close-out 2026-10-30

`Proposed / first slice / inactive` → `Accepted / complete (engine scope) / staged`, verified at `origin/main` `e42fc7a`. The close-out reads the owner's own publication of `ontomark-ingest` 0.1.0 as the decision; that is an inference, not a recorded ratification, and the owner should confirm it (listed in the 2026-10-30 ratification batch):

- `crates/ontomark-ingest` is on `main`. `cargo search ontomark-ingest` → `ontomark-ingest = "0.1.0"`. The crates.io API `/api/v1/crates/ontomark-ingest/0.1.0` gives `created_at` 2026-10-12 and `published_by.login` `jjohare`.
- Engine boundary: `crates/ontomark-wasm/src/engine.rs:168-179` documents `ingest_plan` and `ingest_accept`. `docs/HANDOFF.md:133` records the transcript golden re-blessed over both.
- Attribution, which Consequences makes mandatory: `THIRD-PARTY-NOTICES.md:9` and `docs/UPSTREAMS.md:13` cite OntoCast at `abb8d7b8d6db1a765d4f091a7423937c2cbc5d78`.
- Consumer: `DreamLab-AI/ontomark-obsidian` `origin/main` `src/client.ts:195,205` sends `ingest_plan` and `ingest_accept`. Its `engine.json` pins engine `v0.4.0`, which already carries them (`git show v0.4.0:crates/ontomark-wasm/src/engine.rs | grep -c ingest_plan` → 6). Activation is `staged`, not `live`, only because no installed-Obsidian acceptance (verification item 5) or EXP-11 enabled-path receipt is recorded.

The Implementation line's former "Not published; not exposed through `ontomark-wasm`" was overtaken by the engine boundary section (12 October 2026) and the 0.1.0 publication. The open questions above remain open and are not decided by this note.
