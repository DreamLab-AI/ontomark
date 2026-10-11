# WP-E API: `ontomark-wasm` and the plugin worker protocol

Owner: `spec-plugin`. Branch `wp-e-plugin`. Status: stable for the plugin; usable by any host (browser, Node, native Rust).

## One protocol, three surfaces

`ontomark_wasm::engine::Engine::handle_json(&str) -> String` (native),
`WasmEngine.handle(string): string` (wasm-bindgen, `--target web` or
`--target nodejs`) and the plugin worker all speak the same JSON. The native
transcript test and the wasm harness replay one script and must produce
identical bytes.

Requests are tagged by `kind`:

| kind | fields | mutation |
|---|---|---|
| `load` | `generation`, `profile` (`generic`\|`estate`), `files: [{path, text}]` | replaces everything; adopts `generation` |
| `upsert` / `remove` / `rename` / `set_profile` | `generation` (must be greater than the engine's) + `path`, `text` / `path` / `from`, `to` / `profile` | yes |
| `index` | `generation` | entities, diagnostics (UTF-8 and UTF-16 spans), profile `keys`, last `classification` |
| `describe` | `generation`, `iri` | incoming and outgoing asserted triples |
| `frontmatter` | `generation`, `path` | parsed entries in the `ontomark-parse` Value wire form, `digest` |
| `classify_start` / `classify_step` | `generation`, `budget` (saturation steps, default 20,000) | stepped run; `{status: "pending", steps}` or `{status: "done", classification}` |
| `classify` | `generation` | runs to completion |
| `query` | `generation`, `sparql`, `dataset` (`asserted`\|`inferred`\|`combined`), `limit` | read-only SPARQL |
| `propose_patch` | `generation`, `path`, `base_digest`, `ops` (`ontomark-parse` PatchOp wire form) | returns `new_text`, `diff` and digests; never writes |
| `version` | — | engine and reasoner identity |

Every response is `{ok, generation, result?, error?: {code, message}}`. Reads
whose `generation` is not the engine's are answered `stale` without running.
Error codes: `stale`, `generation_regressed`, `no_run`, `not_classified`,
`not_found`, `stale_base`, `patch_refused`, `reasoning_budget_exceeded`,
`cancelled`, `query_syntax`, `update_forbidden`, `service_forbidden`,
`dataset_clause`, `query_unsupported`, `query_evaluation`, `bad_request`.

`classification`: `reasoner {id, version, el_complete, note}`, `subsumptions [{sub, sup, told}]`, `equivalences`, `unsatisfiable`, `unsupported [{axiom, reason}]`, `policy` (`all`\|`relevant`), `complete`.

## Worker envelope (plugin)

Host → worker: `{v: 1, epoch, id, op}` with `op` one of `init` (transferred wasm `ArrayBuffer`), `engine` (`request`), `classify` (`generation`, `budget`) or `cancel` (`target`).
Worker → host: `reply` (`response`), `progress` (`generation`, `steps`) or `fatal` (`message`).

The host owns the generation and the epoch (`plugin/src/guard.ts`). It accepts a reply only from the live epoch, at the current generation, and, for requests sent on a named channel, only the latest on that channel. A respawn posts `init` and the replay `load` back to back.

## Seams for WP-A and WP-C

`profile` and `projection` (documents → entities, triples, EL axioms) are interim. They are deleted when `ontomark-model` and `ontomark-compile` publish an in-memory projection (EXP-10); the wire shapes above do not change.
