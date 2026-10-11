---
slice: WP-E (ontomark-wasm, plugin/)
git_sha: 03d7187
produced_by: agent:claude-opus (spec-plugin), UI layer by a Sonnet worker
produced_at: 2026-10-11T18:33Z
audited_by: none yet (cross-vendor Codex audit pending; any Claude audit is same-family and degraded)
---

# WP-E evidence

Every log in this directory starts with the command, the commit and a UTC
timestamp, and ends with the exit code. They were produced from a clean
rebuild (`plugin/dist`, `plugin/src/generated` and `plugin/.offline` deleted
first) at `03d7187`.

| Log | What ran | Result |
|---|---|---|
| `01-cargo-test.log` | `cargo test -p ontomark-wasm` | 18 unit + 3 transcript + 1 doctest passed |
| `02-clippy.log` | `cargo clippy -p ontomark-wasm --all-targets -- -D warnings` | clean |
| `03-rustdoc.log` | `RUSTDOCFLAGS=-D warnings cargo doc -p ontomark-wasm --no-deps` | clean |
| `04-build.log` | `npm run build` (wasm32 release, wasm-bindgen web+nodejs, wasm-opt -Oz, tsc, esbuild) | wasm 4,479,577 → 2,332,819 bytes; dist/main.js 3.0 MB |
| `05-vitest.log` | `npx vitest run` | 15 passed |
| `06-wasm-harness.log` | `node --test test/wasm-harness.test.mjs` | 3 passed |
| `07-check-bundle.log` | `node scripts/check-bundle.mjs dist/main.js` | 0 findings; wasm and worker embedded |
| `08-offline-docker.log` | `node scripts/offline-test.mjs` → `docker run --rm -i --network none node:24-bookworm-slim …` | 38/38 checks; interfaces `["lo"]`; outbound connect `ENETUNREACH`; 0 network attempts |

The offline run was repeated five times at `03d7187` (all 38/38). The
edit-during-classify race resolved as `stale` (engine generation check) in two
runs and `cancelled` (cooperative cancel between steps) in three, so both
mechanisms were exercised on real threads.

## Expectations

| Expectation | Evidence | Stabilised by |
|---|---|---|
| EXP-05 generic default, estate opt-in, prose never axioms | 01 (`projection::tests::plain_notes_and_prose_links_yield_nothing`, `estate_profile_requires_resource_and_keeps_it_verbatim`, `transcript_semantics`); 08 (index, settings, estate checks) | `crates/ontomark-wasm/src/projection.rs::tests`, `tests/transcript.rs::transcript_semantics`, `plugin/test/e2e/offline.e2e.cjs` |
| EXP-06 results belong to the current source | 01 (stale/no_run/generation_regressed steps); 05 (edit, delete, kill-and-respawn with a forged old-epoch reply, out-of-order, stale read, timeout, dispose, shared same-generation run); 08 (edit during classify through vault events) | `plugin/test/client.test.ts`, `plugin/test/guard.test.ts`, `tests/transcript.rs` |
| EXP-11 installed plugin works offline | 07, 08 | `plugin/scripts/check-bundle.mjs` (with negative control in `wasm-harness.test.mjs`), `plugin/scripts/offline-test.mjs` |
| EXP-12 read-only, bounded, derived from Markdown | 01 (`query::tests::*`, `reads_never_mutate_and_rebuild_is_identical`); 06 (wasm executes SPARQL); 08 (refusals, hash of vault unchanged by queries) | `crates/ontomark-wasm/src/query.rs::tests`, `tests/transcript.rs`, offline driver |
| Native/wasm parity of the engine surface | 06: both wasm-bindgen targets, including the bytes embedded in main.js, replay the 29-step script byte-identically to `tests/fixtures/transcript.golden.jsonl` | `plugin/test/wasm-harness.test.mjs` |

## Not automated (and why)

1. **Real Obsidian/Electron.** No Obsidian binary exists in the build
   container and downloading one is out of bounds. The offline driver loads
   the installed `main.js` against `plugin/test/e2e/obsidian-shim.cjs` (a
   filesystem-backed stand-in for the exact API subset the plugin uses) and a
   `worker_threads` Worker that accepts the plugin's Blob URL. The following
   remain to be checked by hand in Obsidian desktop:
   - the Blob worker starts in the renderer, and `initSync` compiles the
     embedded module there;
   - `vault.process` writes nothing when the callback throws;
   - the views, the diff modal and settings render and behave.
2. **Mobile.** Out of scope for this sprint; the manifest says `isDesktopOnly: true`.
3. **CI job `plugin`.** It was written but not executed here (GitHub Actions
   is not reachable from this container). Its commands are the ones above.
4. **Reasoning semantics.** Classification uses the interim told-closure
   reasoner (`el_complete: false`), so EXP-01 parity is not claimed. It
   switches to `ontomark-reason` (whelk-el) behind the `Reasoner` seam when
   WP-B publishes a stepped API.
