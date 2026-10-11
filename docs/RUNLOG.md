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
