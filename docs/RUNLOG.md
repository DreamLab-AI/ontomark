- 18:40 spec-model: WP-A crates landed on `wp-a-model-parse` (local worktree,
  not yet on origin). EXP-03 round-trip 9,244/9,244 public pages, each also
  passing five guarded patches; 163,645 frontmatter entries re-written, 0
  failures; parse 25 unit + 12 fixture + 8 doc tests; model 15 acceptance + 6
  doc tests; whole-corpus model build 9,244 entities, 0 identity conflicts,
  0.9 s; wasm32 clean. `docs/api/WP-A.md` published. Cross-vendor Codex audit
  not yet run (transient login failure; retry ordered, Codex is logged in).
- 18:42 queen decisions: unresolved references (about 20k) are reported as
  Unresolved, never invented; the legacy exporter's phantom parents are a defect,
  reproducible only via a named off-by-default `legacy-phantom-parents` adapter
  for the parity test, listed as a divergence. `Profile::mint_iri` hook added,
  default off, one minter owned by the model crate. Defect 8 (ADR status labels
  vs corpus-format doc) owned by spec-estate as a doc fix. All specialists told
  to push their branches after each milestone; only `wp-d-estate` is on origin.
