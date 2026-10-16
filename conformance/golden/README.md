# Golden captures

Reference outputs of VisionClaw's `vault build` at tag `ontomark-census-baseline`
(`1044b0722`), the producer ontomark-compile replaces. Captured by
`capture.sh <vault-binary> <source> <out> [knowledge|working|all]`.

| Directory | Source | Stored |
|---|---|---|
| `corpus-visiongraph/` | visionGraph `b5e36abd1`, `--vault all` | `MANIFEST`, `out.sha256`, `publish.sha256`, `build-stats.json`. Hashes only; no corpus bytes. Bulk directories (`api/pages`, `api/markdown`, `okf/concepts`, staging `pages`/`working`) are one aggregate line: sha256 of the sorted `sha256  bytes  path` listing, and the file count. |
| `../fixtures/estate-small.golden/` | `../fixtures/estate-small` (50 public pages from VisionClaw's golden fixture, plus synthetic pages) | Hashes and the complete `expected/` outputs |

`corpus-visiongraph/` was captured on the Logseq-era `pages/` layout
(`knowledge/pages/`, `working/pages/`) and is re-captured when visionGraph
flattens its vaults to the root layout ontomark reads since 2026-10-16;
`estate-small.golden/` was re-captured on the flat layout that day.

`generated_at` and the snapshot commit are recorded in each `MANIFEST`. The
build is deterministic apart from those two values (two builds of the same
tree differ only in `.generation.json`, `data/.generation.json`,
`ontology-inferred.ttl`, `prose-index.json` and `scaffold-index.json`, all
through `generated_at`), so ontomark is run with the recorded clock and commit
and compared byte for byte.
