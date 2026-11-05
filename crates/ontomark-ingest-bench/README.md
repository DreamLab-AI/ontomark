# ontomark-ingest-bench

A labelled extraction benchmark for [`ontomark-ingest`](../ontomark-ingest). It measures how far each layer of assisted ingest separates facts a note **states** from facts it only **mentions**. Not published to crates.io.

## Why it exists

Ingest checks three things mechanically: the quote occurs in the note, the value names an existing term, and the patch applies to the exact revision. That is *syntactic grounding*. It cannot see that "The Whale is not a Fish" does not make a Whale a Fish, because every word the fact needs is present. Whether a quote *supports* a fact is a separate property, and the truth of the note is a third. This crate measures the second.

## The cases

[`fixtures/cases.json`](fixtures/cases.json) holds 60 single-fact cases. Every quote occurs verbatim in its note and names a vocabulary term, so every fact is groundable. The label says whether the quote states it.

| Category | Cases | Supported | Example |
|---|---|---|---|
| affirmative | 10 | yes | "The Wolf is a Mammal that hunts in packs." |
| paraphrase | 6 | yes | "Every Frog counts as an Amphibian." |
| paired | 6 | 3 yes, 3 no | "The Lynx is a Mammal, not a Reptile." (Mammal yes, Reptile no) |
| negated | 8 | no | "The Whale is not a Fish, despite living in the sea." |
| hypothetical | 6 | no | "If it existed, the Dragon would be a Reptile." |
| historical | 6 | no | "From 1930 to 2006 Pluto was classed as a Planet." |
| attributed | 6 | no | "Its marketing says Ponzi Co is a Charity." |
| uncertain | 6 | no | "Specimen 7 may be a Reptile; the scales are inconclusive." |
| contradicted | 6 | no | "Coral is a Plant. Correction: Coral is an Animal, not a Plant." |

19 facts are supported and 41 are not. The adversarial share is deliberate: the set probes failure modes, so its precision figures are not estimates of precision on ordinary notes.

## Arms

| Arm | Proposals | Decision |
|---|---|---|
| `deterministic` | an oracle proposes each case's own fact, with its quote | `Ingest::accept` |
| `deterministic+jev` | as above | plus the Jev judge's support probability |
| `model` | a chat model, prompted exactly as the Obsidian plugin prompts it | `Ingest::accept` |
| `model+jev` | as above | plus Jev |

A fact counts as kept when it reaches the accepted set and, with Jev, when its support probability is above the withholding threshold. Jev runs through `Ingest::accept_judged` with a recording judge, so it sees exactly the claims production builds. Its probabilities are read once and swept over thresholds 0–0.9 (`risk_curve`). A fact Jev did not score is kept, because the judge fails open.

## Running

```sh
cargo test -p ontomark-ingest-bench                      # pins the deterministic arm
cargo run -p ontomark-ingest-bench --release -- --out det.json
ONTOMARK_JEV=1 cargo run -p ontomark-ingest-bench --release -- --jev --out det-jev.json
cargo run -p ontomark-ingest-bench --release -- \
  --extractor model --endpoint http://HOST:PORT/v1 --model NAME --jev --out model-jev.json
```

Jev is configured from the environment exactly as `ontomark ingest` configures it (endpoint resolution and local-only policy are `ontomark-jev`'s). A bearer key for `--endpoint` is read from `ONTOMARK_BENCH_KEY`.

## Results (2026-11-05)

Raw reports are in [`results/`](results/). Model: `hemmingway-1.5-q8_0` behind the estate's model door, direct (no scaffold). Jev: `jev-latest` through the agentbox proxy.

### Deterministic engine alone

| | TP | FP | FN | TN | Precision | Recall | False acceptance |
|---|---|---|---|---|---|---|---|
| all | 19 | 41 | 0 | 0 | 0.32 | 1.00 | 1.00 |

As designed: grounding accepts every groundable fact, so every adversarial fact reaches the review dialog. This row is pinned by `tests/deterministic.rs`.

### Deterministic + Jev, selective-risk curve

| Withhold at or below | Kept | TP | FP | FN | TN | Precision | Recall | False acceptance |
|---|---|---|---|---|---|---|---|---|
| 0 (off) | 60 | 19 | 41 | 0 | 0 | 0.32 | 1.00 | 1.00 |
| 0.1 | 51 | 19 | 32 | 0 | 9 | 0.37 | 1.00 | 0.78 |
| 0.2 | 47 | 18 | 29 | 1 | 12 | 0.38 | 0.95 | 0.71 |
| 0.3 | 45 | 18 | 27 | 1 | 14 | 0.40 | 0.95 | 0.66 |
| 0.4 | 40 | 17 | 23 | 2 | 18 | 0.42 | 0.89 | 0.56 |
| 0.5 | 35 | 15 | 20 | 4 | 21 | 0.43 | 0.79 | 0.49 |
| 0.6 | 26 | 11 | 15 | 8 | 26 | 0.42 | 0.58 | 0.37 |
| 0.7 | 18 | 7 | 11 | 12 | 30 | 0.39 | 0.37 | 0.27 |
| 0.8 | 7 | 1 | 6 | 18 | 35 | 0.14 | 0.05 | 0.15 |
| 0.9 | 1 | 0 | 1 | 19 | 40 | 0.00 | 0.00 | 0.02 |

At 0.1 Jev withholds nine false facts, mostly negations whose quote carries the "not", with no loss of recall. Above that, precision stays near 0.4 while recall falls, so Jev's discrimination on these cases is weak. The reason is visible per case: Jev judges the **quote**, and an oracle quote is minimal. "founded on 2001-01-01" omits the "not" that precedes it (`negated-06`, p = 0.88), and a correction, attribution or "would" outside the quote is invisible to it.

### Model and model + Jev

One call per case, no errors. "Kept" counts only each case's labelled fact; the model also proposed 10 other facts across the run that ingest accepted (`extra_accepted` per case in the report; several are the correct reading, such as "Coral is an Animal" in a contradicted case). Those are not scored.

**Model alone, by category** (the model+jev run at threshold 0, where Jev withholds nothing):

| Category | TP | FP | FN | TN |
|---|---|---|---|---|
| affirmative | 10 | 0 | 0 | 0 |
| paraphrase | 6 | 0 | 0 | 0 |
| paired | 3 | 0 | 0 | 3 |
| negated | 0 | 0 | 0 | 8 |
| contradicted | 0 | 1 | 0 | 5 |
| hypothetical | 0 | 5 | 0 | 1 |
| historical | 0 | 5 | 0 | 1 |
| attributed | 0 | 6 | 0 | 0 |
| uncertain | 0 | 6 | 0 | 0 |
| **all** | **19** | **23** | **0** | **18** |

Precision 0.45, recall 1.00, false acceptance 0.56. The model handles negation, the "not X" half of a pair and corrections well, and does not propose those facts at all. It fails on modality: it proposes almost every hypothetical, historical, attributed and hedged fact as if stated.

**Model + Jev, selective-risk curve:**

| Withhold at or below | Kept | TP | FP | FN | TN | Precision | Recall | False acceptance |
|---|---|---|---|---|---|---|---|---|
| 0 (off) | 42 | 19 | 23 | 0 | 18 | 0.45 | 1.00 | 0.56 |
| 0.1 | 42 | 19 | 23 | 0 | 18 | 0.45 | 1.00 | 0.56 |
| 0.2 | 40 | 18 | 22 | 1 | 19 | 0.45 | 0.95 | 0.54 |
| 0.3 | 40 | 18 | 22 | 1 | 19 | 0.45 | 0.95 | 0.54 |
| 0.4 | 38 | 17 | 21 | 2 | 20 | 0.45 | 0.89 | 0.51 |
| 0.5 | 36 | 17 | 19 | 2 | 22 | 0.47 | 0.89 | 0.46 |
| 0.6 | 28 | 14 | 14 | 5 | 27 | 0.50 | 0.74 | 0.34 |
| 0.7 | 23 | 14 | 9 | 5 | 32 | 0.61 | 0.74 | 0.22 |
| 0.8 | 20 | 14 | 6 | 5 | 35 | 0.70 | 0.74 | 0.15 |
| 0.9 | 10 | 7 | 3 | 12 | 38 | 0.70 | 0.37 | 0.07 |

Here the model's quotes are longer than the oracle's, so Jev sees more context, and the curve has a usable region: at 0.7–0.8 it removes about three quarters of the false facts the model let through at a cost of 5 of 19 true ones. Attributed claims are the stubborn case (support 0.66–0.98 for all six).

### Summary

| Arm | Precision | Recall | False acceptance |
|---|---|---|---|
| deterministic | 0.32 | 1.00 | 1.00 |
| deterministic + Jev at 0.1 | 0.37 | 1.00 | 0.78 |
| model | 0.45 | 1.00 | 0.56 |
| model + Jev at 0.8 | 0.70 | 0.74 | 0.15 |

Grounding is a necessary filter, not a semantic one. On these cases the extraction model does most of the semantic work, and Jev adds a useful but costly withholding margin only when quotes carry their context. Neither replaces the review dialog.

## What this does and does not show

- It shows which statement types each layer lets through, on a small synthetic set with one profile.
- It does not estimate precision on real notes, where adversarial statements are rarer.
- It does not test truth: a supported fact can still be false.
- 60 cases give wide intervals; differences of a few cases between arms are not significant.
