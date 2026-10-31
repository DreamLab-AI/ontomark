---
id: ADR-OM-010
title: "External-source ingest: a Source port, a host CLI, and proposals rather than writes"
date: 2026-10-31
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit: ""
verified_paths: []
owner: jjohare
review_trigger: "an `ontomark ingest` bundle submitted through `vault propose` and signed on the forum (activation); a second Source kind beyond local files and URLs; any change to the ontomark-ingest wire contract the plugin pins"
repo: ontomark
domain: ingest
lineage: "ADR-OM-003 (native assisted ingest), ADR-OM-005 (one producer; vault keeps propose), ADR-OBS-003 (ingest action; open question on the proposal route), VisionClaw ADR-2114 (CorpusSource), Agentbox ADR-2107 (no corpus MCP)"
---

# ADR-OM-010 — External-source ingest: a Source port, a host CLI, and proposals rather than writes

## Context

`ontomark-ingest` (ADR-OM-003) proposes frontmatter values for one note from that note's own body. A model names values and quotes evidence; `accept()` grounds names, verifies quotes and returns one digest-bound patch. It reads no files, calls no network and compiles to wasm32 for the Obsidian plugin. Nothing yet lets evidence come from outside the vault: a document, a web page, a dataset.

Two projects were read on 31 October 2026 for mechanisms, not code: FengZhenfei/carrel @2a5a63e (MIT) and HelixDB/helix-foundry @ca9c9b7 (Apache-2.0). No code, prompt text or schema is copied from either, so neither licence attaches; they are credited in [UPSTREAMS.md](../UPSTREAMS.md). What they showed:

- **Change classification with a reason** (carrel `detect_changes.py`): new, content changed, parser changed, metadata only, unchanged. A version is the hash of the rendered text, so a rename is not a re-read.
- **A small connector contract** (foundry `connector.go`, `documents.go`): discover, then sync behind an opaque cursor; documents carry a content-hash version, and the host skips versions it already holds.
- **Typed evidence and a named resolution ladder** (foundry's evidence union and resolution rungs): each value carries where it came from; a name that resolves to several things is counted and refused, never guessed.
- **Conflicts become reviews** (foundry `prop_conflicts`, carrel `currency.py`): a source that disagrees with the corpus raises a question instead of overwriting.
- **Budgets and idempotency** (foundry's turn limits and request id).

Ontomark already has most of the core: `EvidenceSpan` records the chunk, byte span, UTF-16 span and quote; `Grounding` ranks matches by `AliasTier` and refuses ambiguous names and initials conflicts. Two gaps are real. Names that differ only in a number (`ISO 9001` and `ISO 14001`, `Llama 3.1 8B` and `Llama 3.1 70B`) can pass the fuzzy tiers. And the result does not say how many values each tier decided.

VisionClaw already has a source port, `CorpusSource` (ADR-2114: `describe`, `list_pages`, `fetch_page`, with a change marker per page). A second, differently shaped port would split the estate.

## Decision

1. **`ontomark-ingest` stays pure and grows additively.**
   - Below `SharedLabel`, a fuzzy tier never aliases two names whose numeric tokens differ. `Exact` and `SharedLabel` are author assertions and stay unguarded.
   - `Candidates` reports per-tier counts.
   - `EvidenceSpan` gains an optional source reference (source id, version hash). It is omitted when absent, so a note-only ingest serialises byte-identically and the plugin's golden transcript holds.
   - A plan can take a target note plus external source text: the patch binds to the target's digest, and evidence spans point into the source.
   - The public structs become `#[non_exhaustive]` in the same release, so this one semver break (0.2.0) is the last for additive fields.

2. **A new crate, `ontomark-source`, is the Source port.** It mirrors `CorpusSource`'s shape: `describe`, `list`, `fetch`, and a change marker per item. A fetched item is rendered text with a version (sha256 of the text) and a media type.
   - Two kinds ship: local files (Markdown, plain text, HTML rendered to text) and URLs. URLs use blocking `ureq` behind a feature; the crate has no network code without it.
   - Unsupported media types are refused by name, not guessed.
   - Change classification compares a ledger of (item, version, parser profile) and reports new, content changed, parser changed, unchanged or removed.

3. **`ontomark ingest` is the host.** It reads sources, picks target notes, calls the model, accepts, and writes a bundle.
   - **Targets** are named explicitly, or discovered from exact label or alias mentions in the source; the fuzzy tiers are not used for discovery. A cap applies.
   - **The model endpoint** is explicit, and local-only by default, reusing Jev's policy and its `ONTOMARK_JEV_ALLOW_CLOUD=1` opt-in. Notes in `working/` are private.
   - **Runs are budgeted** by model calls and characters sent.
   - **Responses are cached** by request digest, so a re-run with nothing changed calls no model.
   - **The request id** is derived from source versions, target digests, model and options.

4. **External evidence lands as a proposal, never as a write.** This closes ADR-OBS-003's open question for external sources.
   - `ontomark ingest` writes the directory manifest that `vault propose --diff <DIR>` already accepts: one full proposed page per target, named by page id. Beside it goes an evidence ledger listing each accepted value, its quote, the source id and version, and the target digest, plus everything withheld and why.
   - It prints the `vault propose` command and never runs it.
   - A value that conflicts with the corpus is withheld as `ConflictingValue` and listed for review.
   - Quotes stay in the ledger, not in page bodies, so text from a third-party source does not enter the public corpus or the relay post.
   - Provenance keys written into pages are limited to those `ontology/vocabulary.yaml` already defines.
   - Notes ingested from their own body (the plugin) keep the reviewed-patch flow of ADR-OBS-003.

5. **Not in this decision.**
   - Creating new concepts from a source.
   - PDF and office formats.
   - Software-as-a-service connectors.
   - Replaying signed decisions on later runs. That needs the forum's decision record as an input and gets its own record.

## Consequences

- One port shape across the estate: VisionClaw's `CorpusSource` and `ontomark-source` can converge on the crate later without a semantic change.
- The plugin repins to `ontomark-ingest` 0.2.0. Its golden transcript changes only where the digit guard changes a grounding, and each such fixture is re-blessed deliberately.
- An ingest run is reproducible from its bundle: every quote can be re-checked against the source version it names.
- `vault propose` remains the only route to the public corpus (ADR-OM-005 decisions 5 and 6).

## Verification required before activation

1. **Grounding regression.** Every vocabulary term's own label and aliases still ground to itself, before and after the digit guard, over visionGraph.
2. **Digit-guard cases.** `ISO 9001`/`ISO 14001`, `Llama 3.1 8B`/`Llama 3.1 70B`, `Web 2.0`/`Web 3.0` and `IEEE 802.11`/`IEEE 802.3` are refused at every fuzzy tier and at `Initials`.
3. **Wire stability.** Note-only ingest output is byte-identical to 0.1.1 on the plugin fixtures, apart from re-blessed digit-guard cases.
4. **End to end.** Run `ontomark ingest` against a local stub model, then `vault propose --dry-run --diff <bundle>`, which must accept the manifest.
5. **Repeat run.** A second run with nothing changed makes zero model calls.
6. **Activation.** One real bundle is proposed, signed on the forum and applied.

## Implementation 2026-10-31

Decisions 3 and 4 landed as `ontomark ingest` on branch `feat/ontomark-ingest-cli` (crates/ontomark-cli: `ingest_cmd.rs`, `ingest_profile.rs`, `tests/ingest_cli.rs`).

- **Sources** read through `ontomark-source` (files, directories, URLs; ids are `file:<canonical path>` or the URL). A ledger of (source, version, parser profile) labels each source new, content changed, parser changed or unchanged. The parser profile hashes the model, endpoint, options and a fingerprint of the selected vaults. Unchanged sources still go through plan and accept so a bundle is a function of all its inputs; the response cache makes them cost no model call.
- **Targets** are explicit (page id or exact title) or `--discover`: a label, alias or note name that occurs in the source as whole normalised words and names exactly one entity, first mention first, capped by `--max-targets`.
- **Endpoint** policy is `ontomark-jev`'s, through a new `JevConfig::allows_endpoint`: allowed when `ONTOMARK_JEV_ALLOW_CLOUD=1`, declared local (`ONTOMARK_JEV_LOCAL`, `AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL`), or the host is a loopback, private or link-local IP literal, `localhost`, `.local` or a single-label name. Otherwise exit 2 before any request. Jev's own paths are unchanged.
- **Model calls**: one chat completion per chunk, `temperature` 0, `max_tokens` 1536, the plan's JSON schema as `response_format` (the plugin sends `json_object` and the schema in the prompt; the prompt layout is kept). Answers are cached under `<ledger dir>/cache/<sha256(body, model, endpoint)>.json`. Budgets are checked before each call; on exhaustion the run stops, the affected pair writes no page, and `evidence.json` lists what was skipped (exit 0, `budget_exhausted: true`).
- **Profile**: for an estate-shaped repository the model profile is derived from `ontology/vocabulary.yaml`: the `core` relations are the only fillable keys and `resource` is the identity. A plain vault reads as `generic/1`.
- **Bundle**: `manifest/<id>.md` (target page in full, patch applied; several sources accepted in id order against the progressively patched page) and `evidence.json` (no timestamps, so a no-change re-run is byte-identical). Quotes appear only there.
- **Vaults**: `--vault working|all` is refused (exit 2). `vault propose` reads the knowledge vault only: a working page in a manifest is proposed as a new public page (checked with `--dry-run`), which would publish private text.
- **Provenance keys**: none written. `sources` in the vocabulary fits only URL sources, and a local path must not enter the corpus; source references live in `evidence.json`.
- **Conflicts**: `review` in `evidence.json` lists `ConflictingValue` and `CompetingValues`. The engine raises those only for single-valued literal keys, and the vocabulary-derived profile offers relation keys only, so no ingest run can produce one yet.
- **Verification 4 and 5** are tests in `ingest_cli.rs`: a stub model server, `ontomark validate` on the patched copy, `vault propose --dry-run --diff <bundle>/manifest` accepted, and a re-run with zero model calls. Item 6 (activation) is not done and the status fields above are unchanged.

