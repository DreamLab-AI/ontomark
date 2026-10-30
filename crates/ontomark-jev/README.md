# ontomark-jev

Host-side **Jev option** for Ontomark assisted ingest: typed Choice and Noul
decisions (TypeSafe System One, `POST /v1/systemone`) behind the three hooks
of [`ontomark-ingest`](../ontomark-ingest). Not published (`publish = false`).

> Proposed research candidate; **no measured uplift**. The wiring and the
> fallback tests show that the option is safe to switch on, not that it helps.
> Evaluate each item against its baseline before relying on it.

## Jev option

| | |
|---|---|
| **What leaves** | Only when enabled and only for the call you make: the note's label, the accepted fact (`key`, written value, predicate IRI) and the located evidence quote (fact check); the chunk text plus candidate values and quotes (selection); the first 2,000 characters of the note body plus the labels and kinds of vocabulary terms it mentions (field suggestion). Nothing else, and no request content is ever logged. |
| **Destination** | The endpoint you configure. Order: explicit `ONTOMARK_JEV_ENDPOINT` (+ `ONTOMARK_JEV_MODEL`, `ONTOMARK_JEV_KEY_ENV` naming the variable that holds the bearer key); else the agentbox loopback proxy (`AGENTBOX_SYSTEM_ONE_PROXY=1`, `AGENTBOX_SYSTEM_ONE_ENDPOINT`, `AGENTBOX_SYSTEM_ONE_MODEL`, no Authorization header); else `https://api.typesafe.ai/v1/systemone` with `TYPESAFE_API_KEY`. A consumer is **local-only** unless `ONTOMARK_JEV_ALLOW_CLOUD=1` is set (the agentbox default): while local-only, the proxy and the cloud key are refused unless declared local (`ONTOMARK_JEV_LOCAL=1` or `AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL=1`), and the cloud endpoint is reachable only as an explicit endpoint you typed. With `ONTOMARK_JEV_ALLOW_CLOUD=1` the proxy and the TypeSafe step are both allowed. `JevConfig::default()` stays local-only for callers that build the option in code. |
| **Flag** | `ONTOMARK_JEV=1` / `JevConfig { enabled: true, .. }`. |
| **Default** | **Off.** With the flag off no request is built and nothing leaves the process. The default build of every other crate is unchanged: nothing depends on this crate, and without the `http` feature it contains no network code (a host supplies a `Transport`). |
| **Egress** | `http` feature: blocking `ureq`, hard 4 s deadline, no retries. |

### Items

| Item | Type | Engine hook | Question |
|---|---|---|---|
| OM-06-S1-01 verify candidate facts before `ingest_accept` | `JevFactJudge` | `Ingest::accept_judged` | One Noul per accepted fact: does the located quote explicitly state it? |
| OM-06-S1-02 select candidate names and evidence spans without generation | `JevSelector` | `Ingest::select_responses` | One Choice per (chunk, key) group of host-generated candidates, with a `none` option. |
| OM-01-S1-01 suggest explicit frontmatter type/relation fields | `JevSuggester` | `Ingest::suggest_fields` | One Choice per name-valued key over the vocabulary terms the note mentions, with `none`. |

| VG-01-S1-02 (relocated from visionGraph) suggest domain/type/parents for a new concept | `JevCreateSuggester` | `ontomark create --suggest` (CLI feature `jev`) | A Noul per declared domain root and per retrieved candidate parent (independent, so multi-domain and multi-parent are kept), one Choice over the type vocabulary with `none`. |

`create --suggest` is **advisory** and local-only unless `ONTOMARK_JEV_ALLOW_CLOUD=1`. Build with `--features jev`
(off by default; the default `ontomark` binary contains none of this code),
set `ONTOMARK_JEV=1`, and point it at an endpoint declared local
(`ONTOMARK_JEV_LOCAL=1`); a non-local endpoint is refused (`not-local`, no
request built) unless `ONTOMARK_JEV_ALLOW_CLOUD=1`. Candidates come only from
`scalars.domain.roots`, the vocabulary's `types`, and parents retrieved by
`find` over the knowledge vault; the result is printed (`jev_suggestion` under
`--json`) after `create` has validated the page and never alters the page
written or any other output. Any fallback prints exactly what `create` printed
before. A refused creation sends nothing. What leaves: the staged title, the
first 2,000 characters of its body, the domain names, type names and the
titles/types of candidate parents.

Question-set versions: `ontomark.fact-support/1`, `ontomark.candidate-select/1`,
`ontomark.field-suggest/1`, `ontomark.create-suggest/1`; the version is logged with every fallback.

### Authority stays in code

* The judge and selector can only **remove** values from what plain
  `accept` would have produced; `accept` re-runs on the survivors, so quote
  grounding, name grounding, kind and conflict checks, the digest guard and
  the proof that the patch applies all still run. If dropping a proposal
  would let `accept` write something the plain run did not (a competing
  value becoming the only one left), the plain result is returned.
* The suggester returns **proposals** for a reviewed edit, never a patch.
  Keys come from the profile's grantable keys (reserved keys such as
  `public` are never offered), targets only from vocabulary terms the note
  mentions, and nothing is minted.

### Fallback

Any of: flag off, no endpoint, transport error, timeout, any non-2xx
(including 429/529; there are no retries), a body that does not parse, a
missing or wrongly typed answer, a Choice key outside the offered set, or a
probability/confidence below the acceptance threshold ⇒ the engine returns
exactly what it returns without a hook. One line is logged with the reason
(`no-endpoint`, `timeout`, `transport`, `http-<code>`, `malformed`,
`too-large`) and the question-set version. Low confidence is not an error:
the verdict is simply "undecided".

### Thresholds

`JevConfig` thresholds that would change an outcome default to *accept
nothing automatically*: `reject_at_or_below = 0.0` (no fact is withheld) and
`select_min_confidence = 1.0` (no candidate is dropped). Suggestions only
annotate a reviewed edit, so `suggest_min_confidence = 0.5` and
`suggest_clear_confidence = 0.75` (below which a suggestion is flagged
ambiguous). None of these is fitted; fit them on labelled development data
and evaluate on held-out cases before enabling an action threshold.

## Use

```rust,ignore
use ontomark_jev::{JevClient, JevConfig, JevFactJudge, UreqTransport};

let env = |k: &str| std::env::var(k).ok();
let client = JevClient::new(JevConfig::from_env(&env), UreqTransport);
let judged = ingest.accept_judged(&doc, &request, &responses, &JevFactJudge(client))?;
// judged.candidates equals ingest.accept(..) unless Jev answered and was confident.
```

## Tests

```
cargo test -p ontomark-ingest -p ontomark-jev --features ontomark-jev/http
```

Covers fallback equivalence (asserted against `accept`'s own output), the
wire shape (Choice `criteria` is a map with `none`; Noul `criteria` is
`{true,false}`; question ids; state; auth header), the confident accept path
with downstream validation still running, endpoint resolution and the
local-only refusals. No live calls: the transport is mocked.
