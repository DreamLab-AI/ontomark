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

## Known host sensitivity: trigonometry in the layout

"Deterministic" above holds on one host. The NGG1 layout
(`crates/ontomark-compile/src/build/ngg1.rs`) places nodes on rings with
`f64::sin` and `f64::cos`, which call the host's libm, and the coordinates are
rounded to three decimals before they are hashed. glibc releases differ in the
last bit, so a coordinate that lands on a rounding boundary changes a digit.
Measured 2026-10-19 on the dream annexe host (rustc 1.97.1, `en_GB.UTF-8`):
`build_is_byte_identical_to_the_legacy_golden` fails with
`out/data/graph/overview.json` differing in two values (`nodes[39].y` and
`categories[30].y`, `-291.395` locally and `-291.394` there); the other three
differences listed are the marker entries that carry that file's hash.
`verbs_print_the_same_bytes_and_exit_codes` passes on both.

The pure-Rust `libm` crate gives the same answer on every host, but it is not
bit-identical to the glibc that produced these goldens: swapping it in made the
local run fail the same test. So the options are to pin conformance to the
reference glibc (today's state; CI and the development host agree), or to move
the layout to `libm` and re-capture every golden, including the full-corpus
hashes, which changes published graph tiers. Neither is done; the second is a
deliberate change to output, not a fix.
