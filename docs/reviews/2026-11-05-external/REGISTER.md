# External review, 5 November 2026: register

The review is kept verbatim in [REVIEW.md](REVIEW.md). It covered this repository and [ontomark-obsidian](https://github.com/DreamLab-AI/ontomark-obsidian). Each finding was treated as a hypothesis and checked against `main` (ontomark `ff6b271`, ontomark-obsidian `2a6c44e`, plugin 0.4.2 pinning engine v0.7.0) before any change.

**Verdicts.** *Source* means confirmed by reading the code; *runtime* means the defect was observed running. Only A1 (by the reviewer) and F (here) were observed before fixing; every fix below has a regression test that fails without it.

## Findings

| ID | Finding | Verdict | Outcome |
|---|---|---|---|
| A1 | SPARQL blocks in one note supersede each other (one guard channel per note) | Confirmed, runtime (reviewer) | Fixed in ontomark-obsidian `8dab879`: a channel per block; quiet rejections no longer render. The shared `api-entities` channel had the same flaw for `__THIS__` blocks and was fixed with it |
| A2 | Query view: an older superseded run overwrites a newer result | Confirmed, source | Fixed in `8dab879`: a per-run token; only the latest run renders |
| B | Jev fails open silently, on a timeout, a malformed reply, a `not-monotone` re-run and any thrown error (the review named the first two) | Confirmed, source; wider than reported | Fixed in `8dab879`: states `off` / `verified` / `not_verified` shown in the review dialog. With a withholding threshold above 0 the run fails closed and every unverified fact is withheld as `not_verified`; at 0 a banner says Jev could not verify. `disposed` is rethrown |
| C | Plain `http:` with a bearer key to any host; redirects cannot be refused | Confirmed, source; applies to the Jev endpoint too | Fixed in `8dab879`, `e05b9e4`: `http:` only for loopback unless the new *Allow plain http to other hosts* setting is on. Redirect behaviour remains documented, without installed-desktop acceptance |
| D | In-note SPARQL stops updating after a reindex | Confirmed, source | Fixed in `8dab879`: blocks subscribe through the persistent `plugin.subscribe()` relay |
| E | Abort not checked across the write lifecycle; disk write and engine update not atomic | Confirmed, source | Fixed in `8dab879`: abort checked after plan, accept, Jev, and after Apply is pressed. A disk write that succeeded is reported as applied even if the engine update fails, because the note on disk is authoritative and is reloaded |
| F | `is_markdown` slices a `&str` at a byte offset and panics on a non-ASCII path | Confirmed, runtime: `"éé"`, `"😊"` panic | Fixed in ontomark `5802c36`: byte-suffix comparison. Regression tests plus a seeded 2,000-round fuzz test over `load`/`upsert`/`rename` and mangled JSON, both failing on the old code. No other tail slice of a `&str` exists in `crates/` |
| G | Public projection suppresses public literals containing any private label as a substring | Behaviour confirmed; a deliberate conservative policy | **Not addressed.** Severity needs a corpus measurement of false suppression |
| H | Query results carry no inference-completeness status | Confirmed | Fixed in `8dab879`: a status line (dataset, generation, completeness), a warning when inferred triples may be missing, and an ASK-false note that "no match" is not "false" |
| I | "Accepted (n)" overstates what was checked | Confirmed (wording) | Fixed in `8dab879`: "Grounded, ready for review (n)" |
| J | No labelled extraction benchmark | Confirmed gap | Added in `a1424ef`: [`ontomark-ingest-bench`](../../../crates/ontomark-ingest-bench), 60 labelled cases, four arms, selective-risk curves. Results below |
| K | 64 chunks × 60 s with no overall deadline | Confirmed | Fixed in `8dab879`: a 10-minute run deadline; each call's timeout is capped at the time left; nothing partial is applied |
| L | The plugin pins engine v0.7.0 while core is v0.8.0 | Confirmed | Done: plugin [0.5.0](https://github.com/DreamLab-AI/ontomark-obsidian/releases/tag/0.5.0) pins v0.8.1 (`271c4e1`); every plugin gate passed against it, and the wasm transcript changed only in its version string |

Items in the review's gap analysis not addressed here: cross-pipeline equivalence fixtures, stable authored IRIs and migration maps, and SPARQL intermediate resource bounds.

## Benchmark results (J)

From [the bench README](../../../crates/ontomark-ingest-bench/README.md#results-2026-11-05):

| Arm | Precision | Recall | False acceptance |
|---|---|---|---|
| deterministic (grounding only) | 0.32 | 1.00 | 1.00 |
| deterministic + Jev at 0.1 | 0.37 | 1.00 | 0.78 |
| model | 0.45 | 1.00 | 0.56 |
| model + Jev at 0.8 | 0.70 | 0.74 | 0.15 |

This measures the review's central point: grounding is not support. Grounding accepts every adversarial fact. The extraction model declines negations and corrections but proposes hypothetical, historical, attributed and hedged claims as stated. Jev helps only when the quote carries its context.

## The review's release gates

| Gate | State |
|---|---|
| 1. Concurrent SPARQL rendering, superseded responses, reindex subscriptions | Done (A1, A2, D) |
| 2. Jev outcomes visible, withholding enforced | Done (B) |
| 3. HTTP/redirect trust boundary; lifecycle cancellation | Partly: HTTPS rule and unit-level cancellation done; redirect behaviour and cancellation not tested on an installed desktop |
| 4. UTF-8 panic and malformed-input fuzzing | Done (F) |
| 5. Privacy canaries and false-suppression measurement | **Not done** (G) |
| 6. Labelled extraction benchmark | Done (J) |
| 7. Exact-version acceptance against the pinned engine | Partly: every automated plugin gate (tsc, vitest, build, bundle check, wasm, offline Docker, ingest e2e) passed against the pinned v0.8.1 engine. Not cleared: no installed-desktop acceptance of 0.5.0 |
