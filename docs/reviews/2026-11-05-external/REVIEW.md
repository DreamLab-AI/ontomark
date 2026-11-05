<!-- External adversarial review, pasted by the operator on 2026-11-05. Verbatim; not edited. -->

Ontomark — adversarial review
Credible engineering
Release hardening needed
Not cleared for autonomous ingestion

Verdict: This is real engineering, not AI slop. The architecture is coherent, the trust boundaries are largely sensible, and the code demonstrates considerable attention to semantic correctness, source preservation and failure reporting.

However, I found several concrete defects, including a reproducible concurrency problem, a verifier that silently falls back when its filtering fails, and a Unicode-triggered panic in the supplied Rust engine. There are also important gaps between syntactic grounding, logical entailment and established truth.

My recommendation is to continue towards a production release, but prioritise correctness and adversarial validation over additional features.
1. Findings ranked by severity
Priority    Finding    Assessment
P1    SPARQL requests interfere with one another    Confirmed; reproduced request-guard behaviour
P1    Jev filtering silently fails open    Confirmed by source
P1    Redirect and transport trust boundary    Confirmed limitation; exploitation not demonstrated
P2    In-note queries lose subscriptions after reindex    Confirmed by control flow
P2    Cancellation and write lifecycle race    Source-supported failure scenario
P2    UTF-8 filename can panic the Rust engine    Confirmed by source reasoning
P2    Privacy filtering can suppress unrelated public data    Confirmed by filtering logic
P2    Query results lack inference-completeness context    Confirmed design gap

A. Multiple SPARQL blocks can invalidate one another
ontomark-obsidian/src/sparql-block.ts:71

Every SPARQL block in the same Markdown note uses the channel:
`sparql-block:${this.sourcePath}`


But GenerationGuard interprets a channel as a single consumer, accepting only its latest request.

Consequently, if a note contains two SPARQL blocks, the second request can supersede the first.

I executed the supplied GenerationGuard implementation with two requests sharing a channel:
{
  "first": {
    "accept": false,
    "reason": "superseded"
  },
  "second": {
    "accept": true
  }
}


The first block's error handler will subsequently render an error.

There is a related defect in query-view.ts:121–145: an older superseded request can return after the newer successful one and replace its result with an error.

Fix: Give each block instance a unique channel, use per-view request tokens, and ignore superseded responses at the rendering boundary.

This needs a regression test containing multiple query blocks in one note, including overlapping refreshes.
B. Jev silently loses its veto authority
ontomark-obsidian/src/jev-ingest.ts:45–55, 99–102

This is subtler and potentially more consequential.

The user can configure Jev to withhold facts below a specified support threshold. Yet when verification fails, including because of a timeout or malformed response, the function returns the original unfiltered result.
const unchanged = (outcome: string): Judged =>
  ({ result: plain, notes: [], outcome });


The caller in main.ts discards outcome, so the review dialog does not show that the verifier failed.

Consider:
1. Extraction proposes a questionable fact.
2. The deterministic engine accepts its syntax and quote.
3. Jev is configured to withhold low-support facts.
4. Jev times out.
5. The original fact remains available for application, without a visible verification-failure warning.
This does not create an unauthorised write: the user still sees the diff and must approve it. And advisory fallback is a deliberate design choice.

The defect is that a configured filtering policy silently becomes advisory-only.

I would expose three explicit verification states: verified, rejected, and not_verified. With a nonzero withholding threshold, a failed verifier should either withhold pending retry or require a conspicuous manual override.
C. The network trust boundary is weaker than the privacy framing
src/ingest-transport.ts:85–105

The transport uses Obsidian's requestUrl with bearer credentials. The endpoint validator also accepts ordinary HTTP URLs.

The code is candid about the redirect limitation, and the public project acknowledges it. Obsidian's API offers HTTP requests without browser CORS restrictions, while the configured transport has no redirect-control mechanism exposed here. 
Developer Documentation+1


Two risks remain:
- A configured endpoint could redirect note content to another destination. Whether bearer credentials follow depends on transport behaviour, which has not been demonstrated here.
- A remote http:// endpoint can receive the bearer token without transport encryption.
For a privacy-oriented product, I would require HTTPS except for explicitly recognised loopback endpoints, test cross-origin 301/302/307/308 behaviour, and avoid advertising destination confinement until it is enforceable.
D. Reindexing disconnects in-note SPARQL updates
src/sparql-block.ts:40 subscribes directly to plugin.api.onChange().

But src/main.ts:180–184 disposes that API and creates another during reindexing.

The ordinary graph and entity views subscribe through the plugin's persistent event relay. In-note SPARQL blocks do not.

A block can therefore remain displayed while no longer receiving updates.

Fix: Subscribe through plugin.subscribe(), as the other views do.
E. Cancellation does not cover the full write lifecycle
src/main.ts:488–501 checks the ingest cancellation token after the model call, but not after Jev verification or before applying the final patch.

Furthermore, src/api.ts:237–254 writes through vault.process() before updating the engine. If the engine has been disposed in the meantime, the disk write can succeed and the subsequent engine update can fail.

The byte-for-byte source guard is good and prevents overwriting intervening edits. It does not make the disk write and engine update an atomic transaction.

I would test unloading, disabling Jev, reindexing and closing Obsidian at every asynchronous boundary.
F. A Unicode path can panic the supplied Rust engine
ontomark-wasm/src/engine.rs:681–683
fn is_markdown(path: &str) -> bool {
    path.len() > 3 &&
        path[path.len() - 3..].eq_ignore_ascii_case(".md")
}


Rust string slices require UTF-8 character boundaries. The byte offset len() - 3 is not necessarily one.

For example, a load request containing a non-Markdown path consisting of the four-byte character 😊 reaches an invalid slice boundary.

The declared handle_json() contract says bad input does not panic. This violates that contract.

Use:
path.to_ascii_lowercase().ends_with(".md")


or an ASCII suffix comparison on the byte slice.

This finding applies to the supplied core snapshot; I have not verified whether the plugin's newer pinned engine already fixes it.
G. Public projection over-redacts
ontomark-compile/src/projection.rs:153–171

The public projection gathers hidden entity labels and aliases, then removes public literal values whenever they contain any such string.

Suppose a private entity has the label AI.

A completely unrelated public label such as chair contains ai and will be suppressed.

This is a privacy-first conservative policy, but the false-positive rate could become substantial on a large mixed corpus.

It needs corpus-level measurements: withheld values by reason, attributable private references, and unrelated-value suppression. Graph identity filtering and lexical disclosure screening should be distinguished.
2. The deeper gap: grounded does not mean supported
This is the most important conceptual issue.

Ontomark correctly separates model proposals, deterministic checking and human approval. But there are still three distinct properties:
Property    What it establishes
Syntactic grounding    The quoted text exists, the value is mentioned, and the target resolves
Semantic support    The quoted text actually supports the proposed relationship, including negation and context
Truth    The underlying statement is correct in the relevant world or domain

The first can be tested mechanically.

The second is what Jev proposes to evaluate, but its incremental value over the base model has not yet been measured.

The third is outside both mechanisms and requires evidence, provenance or authoritative validation.

For example, a note stating “Rex is not a dog” contains the relevant words. A lexical checker alone cannot establish the instance-of: Dog assertion as correct.

The review dialog currently calls structurally accepted candidates Accepted. I would consider Grounded candidates or Ready for review more precise.

Given your earlier Loom results around retrieval completeness and the copy ceiling, this distinction is particularly relevant: successful delivery or recovery of evidence does not establish correct interpretation of that evidence.
3. Gap analysis
High
Actual ingest quality has not been established

Build a labelled benchmark with affirmative, negated, hypothetical, historical, quoted and contradictory statements. Measure precision and recall for the deterministic engine alone, with the base extraction model, and with Jev. Include selective-risk curves for each withholding threshold.



High
Inference completeness is not carried into query results

The engine tracks incomplete classification, but the SPARQL result renderer has no corresponding warning or status. An ASK result of false means the graph pattern did not match, not that the corresponding real-world proposition is false. Incomplete inference makes that distinction even more important. 
w3c.github.io


Add a query envelope recording source generation, dataset, reasoner policy and completeness.



Medium
Consumer semantics remain divided

The typed model projection, estate compiler and plugin-facing WASM projection do not yet constitute one fully migrated semantic pipeline. Compatibility is an explicit project goal, but each path needs cross-implementation fixtures before equivalence can be assumed.



Medium
Stable identity and provenance require stronger defaults

Path-derived IRIs are convenient for generic vaults but change when notes move. Durable knowledge-graph consumers need stable authored identities, migration maps and provenance for material assertions.



Medium
Worst-case resource behaviour needs measurement

SPARQL result row limits do not bound intermediate joins, sorts or memory. Worker timeouts offer recovery, but do not prove predictable latency or resource consumption. Assisted ingest permits up to 64 sequential calls of 60 seconds each without an overall run deadline. Test adversarial queries and large notes.



4. What survives the adversarial review
A great deal, actually.
- The Markdown-as-authority architecture is defensible and useful. Parsed state can be regenerated rather than silently drifting in a database.
- The separation of Rust semantic processing from Obsidian presentation is sensible.
- Guarded, exact-source patching is a strong implementation decision.
- Explicit refusals for unsupported reasoning constructs are considerably better than pretending to offer complete OWL behaviour.
- Native/WASM protocol parity, deterministic serialization and reproducible build intentions are meaningful engineering commitments.
- Local-first processing, session-only keys and user-initiated model calls form a sound starting privacy posture.
- Source attribution and licensing receive unusually substantial attention.
The code also makes distinctions between logical consistency, ontology coverage and source correctness that many superficially similar projects never manage to articulate.

Those are architectural strengths, not merely documentation polish.
5. What I would require before calling it production-ready
Release gates
0 / 7 cleared



[ ] Fix concurrent SPARQL rendering, superseded responses and reindex subscriptions
[ ] Make Jev verification outcomes visible and enforce the chosen withholding policy
[ ] Close the HTTP/redirect trust boundary and test lifecycle cancellation
[ ] Fix the Rust UTF-8 panic and add malformed-input fuzzing
[ ] Run privacy canaries and quantify false suppression on a realistic corpus
[ ] Publish a labelled extraction benchmark comparing the deterministic and Jev-assisted paths
[ ] Run one exact-version release acceptance suite against the pinned engine and plugin
 Copy checklist

6. Scope and confidence of this review
I inspected the two supplied source exports, reconstructed their file structures and exercised the plugin's request-generation guard directly.

I did not execute the full Rust or Obsidian test suites. The supplied exports omit some build and test assets, and the environment does not have a Rust toolchain installed.

More importantly, the plugin pins Ontomark engine v0.7.0, whereas the supplied core snapshot does not contain the ingest operations now used by the plugin. I therefore cannot independently certify the actual bundled engine's most sensitive grounding logic from these attachments.

That is a limitation of this review's evidence, not proof of a release defect.
Final verdict
Architecture
Strong

Implementation maturity
Promising

Adversarial assurance
Incomplete

Autonomous-use readiness
Not yet



I would be comfortable treating Ontomark as a serious experimental ontology authoring and reasoning system, used locally with human-reviewed changes and sensible backups.

I would not yet put it in charge of unattended semantic promotion, sensitive remote ingestion or authoritative publication without additional gates.

The most encouraging thing is that I did not find an incoherent architecture hidden under elaborate terminology. I found fairly conventional, identifiable engineering problems at the edges of an otherwise well-designed system.

My blunt conclusion: stop expanding the feature surface for a sprint. Fix the request-lifecycle defects, establish an adversarial semantic-ingestion benchmark, and tie the release evidence to the exact bundled engine. That would improve the project's credibility more than another substantial capability.
