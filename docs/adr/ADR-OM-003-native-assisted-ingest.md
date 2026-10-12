# ADR-OM-003: native, host-driven assisted ingest

- Status: Proposed
- Date: 2026-10-12
- Implementation: first slice landed in `crates/ontomark-ingest` (facts only; native tests, strict lints and docs, wasm32 build pass). Not published; not exposed through `ontomark-wasm`; not consumed by the plugin. No release contains OntoCast code.
- Activation: inactive
- Related: [ADR-OM-002](ADR-OM-002-shared-graph-and-obsidian-boundary.md), [OntoCast triage](../ONTOCAST-TRIAGE.md), [ADR-OBS-002](https://github.com/DreamLab-AI/ontomark-obsidian/blob/main/docs/adr/ADR-OBS-002-assisted-ingest-boundary-and-estate-notes.md), [upstream credits](../UPSTREAMS.md)

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
