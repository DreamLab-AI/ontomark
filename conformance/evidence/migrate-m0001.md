# Evidence: `m0001-obsidian-bodies` against the visionGraph corpus (EXP-08)

Executed 2026-10-11 (UTC), on a **temporary copy**; the corpus itself was never
written. Counts and `knowledge/` ids only: no page bodies and no `working/`
paths are recorded here.

## Setup

```
git -C visionGraph archive HEAD knowledge working ontology vault.toml ':(exclude)working/assets' | tar -x -C $TMP/corpus
```

Source commit `b5e36abd172052cfb0d1dd8a904983f604e1b641`
(`docs(ontology): plan shared Rust/WASM core and local Obsidian tooling`),
11,314 files in the snapshot, 11,278 pages considered (`pages/` and `journals/`
of both vaults). Binary: `ontomark migrate` (commit `f8b7a84`) release build; profile
`profiles/estate/profile.yaml`.

## 1. Plan (dry run), 2026-10-11T18:30:32Z, 11 s

```
migrate plan m0001-obsidian-bodies --profile profiles/estate/profile.yaml --out plan1.json
```

| Measure | Value |
|---|---|
| pages considered | 11,278 |
| pages to change | 53 (52 in `knowledge/`, 1 in `working/`) |
| unchanged / refused | 11,225 / 0 |
| input digest | `dbffbf9bc416ae6dc352b86defa385cb79b49feb31054444045d8d712da51249` |
| status | READY (Syntax) |
| asserted triples (knowledge, public graph) | +0 / -0 |
| inferred subsumptions (Whelk) | +0 / -0 |
| visibility changes | 0 |
| newly unsatisfiable classes | 0 |
| identity changes (resource, slug, title) | 0; resources added 0, removed 0 |

53 matches the 53 pages `ontomark repair bodies --vault all --check` reports on
the same snapshot. The knowledge vault was compiled twice in memory (asserted
graph, then Whelk classification); both compiles succeeded.

## 2. Apply, 2026-10-11T18:30:50Z

```
migrate apply --plan plan1.json
```

53 applied, 0 already applied, 0 conflicts; journal 324 KB under
`.ontomark/migrations/m0001-obsidian-bodies/dbffbf9b...1249/` (originals plus
`journal.jsonl`, state `complete`; 53 intents, 53 applied).

After apply, with the real `ontomark` binary on the migrated copy:

- `repair bodies --vault all --check`: 11278 page(s) examined, 0 need converting.
- `validate --vault all`: knowledge 0 errors, working 0 errors.
- `gate --tier full --vault knowledge`: PASS (validate 0 errors, conflicts 0
  blocking, Whelk 9,950 classes, 81,342 inferred subsumptions, 0 unsatisfiable).

## 3. Idempotence

Re-plan after apply: 11,278 considered, **0 to change**, 11,278 unchanged,
READY. Re-applying the old plan: 0 applied, 53 already applied.

## 4. Rollback

```
migrate rollback m0001-obsidian-bodies dbffbf9b...1249
```

53 restored, 0 already original, 0 conflicts. The SHA-256 of the sorted
per-file SHA-256 listing of the whole copy (excluding `.ontomark/`) is
`c5f43d1740641b8f5e64ceac9f7c14ec90170f7ffa611bdb3f82c6991be54216` before
apply and after rollback: byte-identical. Journal state `rolled-back`
(53 restored). Apply then rollback was run twice on the copy with the same result.

## Reading the zero deltas

The conversion rewrites outline syntax in page bodies. On this corpus the
compiled graph did not move (observed, not assumed: both compiles ran), which
is what a Syntax migration promises. The delta machinery is checked by
`the_delta_machinery_reports_zero_for_a_no_op_edit`. The same
machinery reports non-zero deltas for a visibility or resource edit
(`tests/migrate.rs`).
