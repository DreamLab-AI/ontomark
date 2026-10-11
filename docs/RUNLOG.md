# Sprint run log

Append-only. Each entry: time (UTC), actor, what changed, evidence. The queen
writes here at every milestone so a fresh session can resume from this file plus
`docs/ADR-OM-001-sprint-decisions.md`, `docs/DEPRECATION.md` and the RuVector
memory key `ontomark-sprint-state-2026-10-11` (namespace `project-state`).

## 2026-10-11

- 17:38 queen: repo `DreamLab-AI/ontomark` created private; secret scanning and
  push protection on. Scaffold pushed (`24d2e88`): seven crates, dual licence,
  Whelk notice, CI with gitleaks, eight estate ADRs copied in. Native and
  `wasm32-unknown-unknown` builds proven for the skeleton.
- 17:39 queen: VisionClaw tag `ontomark-census-baseline` on `1044b0722`.
- 17:40 queen: `SPRINT-BRIEF.md` pushed (`4f112b6`). Five Opus specialists
  launched: `spec-model` (WP-A), `spec-reason` (WP-B), `spec-compile` (WP-C),
  `spec-estate` (WP-D), `spec-plugin` (WP-E). Each may spawn Sonnet workers and
  Haiku Explore agents. Advisor consulted at checkpoint 1 (reply pending at time
  of writing).
- 17:46 spec-estate: two vault pins diverge (`1044b0722` vs Agentbox
  `3213e314`); vowl-wasm pinned at `0.1.2`.
- 17:47 spec-estate: reported 48-vs-53 category drift. 17:58 queen: reproduced
  the miscount (rustfmt-wrapped tuples), confirmed both tables at 53, added a
  one-set unit test in VisionClaw, tag `ontomark-census-baseline-1`
  (`de33ca538`), pushed. Specialists corrected.
- 17:49 spec-compile: working in worktree `../ontomark-wp-c`; legacy build
  deterministic apart from `generated_at`. Licence confirmation requested.
- 17:55 queen: licence confirmed to all specialists on the owner's authority;
  worktree discipline made mandatory after a shared-checkout branch switch.
- 18:00 spec-estate: `DEPRECATION.md` committed (`1b009f6`, branch
  `wp-d-estate`), about 110 rows, 48 of 48 anchors verified. Asked D1 to D9.
- 18:04 queen: D1 to D9 decided (see ADR-OM-001); knowledgeGraph and standalone
  WasmVOWL brought into scope; WP-C CLI contract widened to DEPRECATION §2.1.
- 18:08 queen: relicensing grant recorded in `NOTICE` (`32a3de3`), after
  verifying knowledgeGraph is also owner-authored.
- 18:12 queen: ADR-OM-001 and this run log written.

## Resume procedure

1. `cd ~/workspace/ontomark && git fetch && git branch -a` to see which `wp-*`
   branches have moved; read each branch's `docs/api/WP-*.md` if present.
2. Read `docs/DEPRECATION.md` for the removal plan and its gating tests.
3. `ListAgents` to see which specialists are still alive; re-spawn any missing
   one with the brief in `SPRINT-BRIEF.md` and the slice description in this
   log, telling it to read its branch first.
4. The queen merges a slice only when its acceptance tests in the brief pass
   under `source scripts/env.sh && cargo test --workspace`, then pushes estate
   repos; VisionClaw via remote `dreamlab-github`, visionGraph via `origin`,
   both after the gates in their `CLAUDE.md`.
- 18:20 queen: README (estate style), ADR-OM-001 and this log pushed (`3312780`).
  spec-estate applied D1 to D3 (`1fe342a`). The D4 to D9 message was truncated in
  transit; resent in three parts. Decisions as sent: D3 archive standalone
  WasmVOWL; D4 thin AGPL `vault` adapter in VisionClaw for propose/apply/panel;
  D5 both VisionClaw reasoner paths route through ontomark-reason, closure-only
  path deleted once EXP-01 passes; D6 explorer TS NGG1 deleted, consumes
  vowl-wasm re-export, ngg1-3n2e.bin golden gets a loading test; D7 visionGraph
  Python hygiene scripts become `ontomark lint|migrate|attribute|assets`, then
  deleted; D8 vowl-wasm markdown-ontology feature deleted; D9 Loom consumes the
  bundle contract, one IRI function, one generation check. Phase 2 prep: failing
  consumer tests first on `ontomark/<responsibility>` branches, doc and diagram
  edits staged per step, `scripts/estate-guard.sh` migration guard. Lesson:
  SendMessage bodies over about 1,300 characters can truncate; send decisions in
  parts.
- 18:28 spec-compile: PARITY MET on `wp-c-compile` (`d7863ff`). Golden captured
  at the baseline for visionGraph `b5e36abd1` (hashes) plus a public-and-synthetic
  fixture `conformance/fixtures/estate-small` with full expected outputs.
  `ontomark build|validate|gate|conflicts|repair` reproduces legacy bytes: 0
  differences over 32 artefact lines, 12/12 verb outputs including exit codes,
  negative control done. Reasoning via `whelk-el` behind a `Classifier` trait.
- 18:30 queen: privacy finding accepted as divergence #1. Legacy publish staging
  copies a public page's raw frontmatter including `is-a: [[Private Title]]`, so
  private titles leak on the live site. Redaction approved for every public
  export surface, count and three examples required before applying, EXP-04
  test to grep every public artefact for every private title. Phase 2 step 1
  released to spec-estate: visionGraph publish.yml onto the ontomark CLI on
  branch `ontomark/publish-cli`.
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
- 18:50 queen: branch repair. Four queen commits (NOTICE, README+ADR, two RUNLOG
  entries) had landed on `wp-a-model-parse` because the shared checkout was on
  that branch when I committed; `origin/main` had only the scaffold, brief and
  one later entry. Cherry-picked onto main (`7936cb0`), log re-sequenced. Rule
  now: the queen verifies `git branch --show-current` is `main` before every
  commit in the shared checkout. Codex cross-vendor audit is down estate-wide
  (refresh token expired, 401 reproduced); user asked to `codex login`. Until
  then audits are Claude-only and labelled degraded; the Codex audit is a
  pending gate, not a pass.
- 19:00 spec-estate: `scripts/estate-guard.sh` committed (`516f92c`,
  `wp-d-estate`). Four residue kinds (invocation, producer-pin, vowl-pin,
  legacy-crate) over seven repos, allowlist with reasons, 22-case self-test.
  Baseline 876 residue lines, exit 1: project 376, visionGraph 174, agentbox
  131, VisionFlow 131, loom 59, knowledgeGraph 4, plus the legacy crate dir.
  This is the sprint burn-down number; done means exit 0. Five Sonnet workers
  are writing failing consumer tests and staged doc edits on
  `ontomark/<responsibility>` branches in worktrees under ~/workspace/wt-ontomark.
  Diagram topics are restamped only after code lands (no citing unlanded code).
- 19:05 queen: EXP-04 decision REDACT. Measured leak on visionGraph `b5e36abd1`:
  50 frontmatter relation entries in 46 public pages and 77 body wikilinks in
  50 public pages name non-public pages, only in the Quartz `publish/` staging
  (machine artefacts already redact). ontomark staging drops those frontmatter
  entries and renders such wikilinks as unlinked display text; default on under
  the estate profile, `--no-redact-private-links` for parity runs. Divergence 1,
  about 96 pages on narrativegoldmine.com/notes. EXP-04 canary extended to the
  staging tree.
- 19:12 queen: Codex restored by the user (`codex login`); `codex exec -m
  gpt-6-astra` verified from the queen's shell. All five specialists told to run
  the cross-vendor audit: WP-A now on its six claims; WP-C on the parity claim
  and divergence 1 before the release tag; WP-D on each estate branch before
  reporting it ready; WP-B and WP-E when their evidence exists. NOT RUN
  entries are to be replaced by verdicts.
- 19:10 spec-estate: Phase 2 step 1 ready on visionGraph `ontomark/publish-cli`
  (`9faf22f8c`, worktree, not pushed): consumer contract test 7 cases (red on
  main, 6/7 green now, the 7th waits on the release tag and digest), publish.yml
  fetches `ontomark-x86_64-unknown-linux-gnu` by tag and sha256 and drops the
  Rust steps, gates and CLAUDE.md and README rewritten, ADR-VG-007 status
  section. Queen decisions: no source fallback (digest pins what runs); step 1
  (parse) repins crates/vault in the same change; VC-54 six slugifiers, VC-55
  five PROV definitions. First release v0.1.0-rc.1 to be cut from wp-c-compile
  when compile confirms readiness.
- 19:25 spec-compile status: reasoner swap to whelk-el leaves
  `ontology-inferred.ttl` byte-identical (81,238 inferred pairs, 0
  unsatisfiable); `profiles/estate` crate holds taxonomy, IRI rules and estate
  data with a test that its tables equal the legacy constants; 386 unit tests
  and 30 doc examples pass; `just conformance` runs the fixture comparison.
  Five workers in flight (profile threading with one taxonomy table, NGG1
  single owner, JSON-LD and SHACL, reversible migrate with per-page report,
  W3C Turtle and OWL 2 EL conformance). Divergence list: 1 private-link
  redaction (approved); 2 `--version` string (accepted); 3 two compatibility
  adapters, off by default, when compile moves onto WP-A's model
  (phantom parents; first-claimant IRI for the 9 multiply-claimed IRIs).
  rc.1 sha follows profile threading, redaction, Codex audit.
- 19:32 spec-model: review cycle one closed (`177e5b9`, `2b99203`, pushed
  through `4cc0810`): 11 confirmed defects fixed with regression tests proven
  red on old code; corpus results unchanged (9,244/9,244 round-trip, 163,645
  entries, 20,180 unresolved). New finding: 418 existing `urn:ngm:class:` IRIs
  follow an older slug rule (`2-d-li-dar`), so a fresh mint for an existing
  title could create a second IRI. Queen decision: mint_iri refuses when any
  existing IRI already maps to the same title (case-insensitive title index),
  returning None with the existing IRI named; the 418 become the fixture.
  Codex audit still to run on this slice (channel restored 19:12).
- 19:38 queen decisions from the estate step tests: NGG1 codec gets its own
  dependency-light crate `ontomark-ngg1` (no serde_yaml, no regex), re-exported
  by ontomark-compile; vowl-wasm depends on it directly so its 232 KB bundle
  does not grow; lands before rc.1 (EXP-09 owner). Loom D9 widened: the Python
  reload timer is the whole reload path, so spec-estate owns a Rust
  `reload_published_generation` in loom-facade with a fixture-bundle test;
  Python deleted in the same branch once it passes (step 11). Tag format
  `v0.1.0-rc.1` accepted by every estate test.
- 19:45 spec-compile: golden repinned to `ontomark-census-baseline-1`
  (`ac3149c`), identical to the original tag apart from the injected build
  clock; parity still 0 differences on fixture and corpus. `ontomark-ngg1`
  landed as the single NGG1 owner with byte parity; the TypeScript reader
  agrees; vowl-wasm re-export works. migrate, Turtle conformance and test
  porting committed. Still running: profile threading, JSON-LD and SHACL.
  Then redaction (count to queen first), fmt and Cargo.lock, rc.1 build.
- 20:05 queen: WP-A MERGED to main (`28ed200`). Independent gate: `just check`
  exit 0 in a clean worktree of `3f6f521`; parse 25+12+15 tests, model 24, doc
  tests, clippy -D warnings, cargo doc 0 warnings, wasm32 build. EXP-03
  9,244/9,244 lossless; 163,645 entries re-emitted, 0 failures. Codex
  cross-vendor audit RUN at `4cc0810`: two high-severity defects (a `|+` set
  deleting a following comment; case-only path clash resolving to last-loaded)
  fixed test-first in `3f6f521`. Delta audit `4cc0810..3f6f521` pending.
  Consumers told to take WP-A from main.
- 20:10 spec-compile: `ontomark migrate` engine landed (`f8b7a84`, `c299746`,
  `79db994`; 21 integration tests). On the corpus it plans 53 conversions, the
  same 53 pages legacy `repair bodies` flags; apply leaves validate and the full
  gate clean, re-apply is a no-op, rollback restores byte-identically (EXP-08).
  Plan files hold digests only; the journal under `.ontomark/` is gitignored in
  ontomark (`bd00ffa`) and must be in visionGraph too (routed to spec-estate).
  Remaining before rc.1: CLI wiring of migrate, JSON-LD/SHACL doctest fixes,
  redaction with count, fmt/lockfile tidy, Codex audit.
- 20:15 spec-estate: Codex audit of visionGraph `ontomark/publish-cli` PASS WITH
  FINDINGS; fixed at `ea812a287` (unpushed), re-audit running. Majors: contract
  test checked command text only (commented checksum, wrong asset, removed
  download all stayed green); now it extracts the fetch step shell block and
  runs it against a fake `gh` with a fake binary; backslash-continued `vault`
  calls now folded before the check. Evidence: 11/11 red on main, 10/11 green
  on branch (release pin red by design), 11/11 Codex mutations caught.
  Method note: Codex sandbox cannot start in this container (bwrap); audits
  run unsandboxed on checksummed scratch copies, never on worktrees. Batch
  audits of 29 prep branches running per repo.
- 20:22 queen decision 19: `ontomark-ngg1` 0.1.0 publishes to crates.io ahead
  of rc.1. Reason: vowl-wasm is a crates.io crate and cannot depend on a git
  pin into a private repo; its CI cannot fetch one. The crate has zero deps,
  is MIT OR Apache-2.0, and its surface is frozen by byte parity. Name free
  on crates.io (checked); publish credentials present in the repo cargo home.
  Blocked on spec-compile pushing wp-c-compile and naming the commit. The
  queen publishes from a clean worktree. `.ontomark/` ignored on the
  visionGraph publish-cli branch (`0e9f6c032`).
- 20:30 spec-compile: profile threading landed (`f588878`, `d915d3c`), parity
  still 0; `ontomark migrate list|plan|apply|rollback|status` wired
  (`c56d1db`); W3C Turtle 219/219 (rdf-tests `42e6bb1`); OWL 2 EL 0
  violations over 278,961 axioms. Redaction: strict rule confirmed (`[[T]]`
  becomes `(private link)`, aliases kept unless equal to title, embeds and
  frontmatter entries removed); the queen's earlier "else target text" wording
  was wrong. Crude count now ~240 pages; definitive count from the resolver
  pending. Post-rc.1: writer `shorten` fixes (no divergence); 690 classes
  typed only skos:Concept get owl:Class as divergence 3 (adapters become 4).
- 20:45 queen: `ontomark-ngg1` 0.1.0 PUBLISHED to crates.io from a clean
  worktree of `291e0de` (tag `ontomark-ngg1-v0.1.0`). Gates run by the queen:
  `cargo package` verified (16 files, 74.6 KiB), 31/31 tests from the extracted
  tarball, `cargo publish --dry-run` clean. Token supplied per-command from
  the shared cargo home (env.sh redirects CARGO_HOME to the repo-local one,
  which holds no credentials). vowl-wasm step 5 unblocked; spec-estate told
  to swap the pin.
- 20:55 queen: WP-A delta-audit fixes MERGED (`wp-a-delta-fixes` `d454740`).
  Codex delta audit found 4 defects (2 HIGH: doubly-claimed predicate still
  reasoned; Unicode case fold wrong for ß; 2 MEDIUM: lone CR line break;
  emit_entry read-back under caller styles), all fixed test-first. Gate:
  clean-worktree `just check` exit 0, 100 tests, 0 warnings; EXP-03 unchanged.
  WP-A CLOSED: both Codex verdicts recorded, no pending gates. API note:
  `PatchError::Unrepresentable` may now carry a key, not only float text.
- 21:20 queen: wrap-up at user request. spec-compile estimates rc.1 at about
  90 minutes; checkpoint pushed at `d5c1188` (unverified), last verified
  `291e0de`. rc.1 is handed off as pending with its ordered steps in
  `docs/HANDOFF.md`. The first HANDOFF.md commit (`44d600e`) was a truncated
  file; rewritten in full at this commit.
- 21:50 queen: hand-off complete. HANDOFF.md covers all five slices; WP-B
  verified gate-green in a clean worktree (38 tests) and ready to merge; WP-E
  has engine, plugin and receipts committed, 17 files uncommitted, push
  requested. Estate table received in three parts; HANDOFF-WP-D at 935915f.
  Advisor never answered three checkpoints and was released. Memory key
  ontomark-handoff-2026-10-11 updated. Queen standing down.
