---
id: DEPRECATION-001
title: Estate deprecation inventory and removal plan (EXP-10)
status: phase-2-partial
date: 2026-10-11
owner: spec-estate (WP-D)
census_at: 2026-10-11T17:55:26Z
census_baseline: VisionClaw ontomark-census-baseline = 1044b0722e6166c8936caec7aacf0ed4c9f7647b; ontomark-census-baseline-1 = de33ca538289e3b1384a5a1e23ed0a8178f0b54d
---

# Estate deprecation inventory and removal plan

This inventory records the original EXP-10 census and acceptance criteria. Its tables cite the interrupted sprint’s source baseline; they are not a claim that released crates remain skeletons. Core parity and a first consumer release batch are now landed. The remaining estate replacements stay open. See [current hand-off](HANDOFF.md) and the dated continuation below.

## 1. Census basis

| Repo | Path | HEAD | Dirty | Notes |
|---|---|---|---|---|
| VisionClaw | `project/` | `30169a41694f23d4bdc4dca42b3a475974eb9fee` | 2 (submodule `agentbox`, untracked `GH`) | `git diff 1044b0722 HEAD -- crates/vault crates/vault-core` is empty, so citations below hold at the census baseline. `ontomark-census-baseline-1` (`de33ca538`) adds only 9 lines at the end of `graph_tiers.rs` (the category set guard), so all citations also hold there |
| visionGraph | `visionGraph/` | `b5e36abd172052cfb0d1dd8a904983f604e1b641` | 15 (vault pages and journals only) | No tooling file dirty |
| vowl-wasm | `vowl-wasm/` | `c662aaa5f98cee3b1468ccac1a67400f38c34ac7` | 0 | crate and npm 0.1.2 |
| Loom | `loom/` | `2f8866622bc230c8e3985cdb7f51ca4bbd67f259` | 2 (untracked local settings) | |
| Agentbox | `agentbox/` | `4f25bd01a9c209c9f5295d26328b7ec7d0f992bb` | 0 | |
| VisionFlow | `VisionFlow/` | `33ece3856b6f8f37d3379b512bb19bb9cb9d80a0` | 2 (untracked review ledger) | estate diagram corpus |
| knowledgeGraph | `knowledgeGraph/` | `4ed9ac159daf402b4fb252ce559dbeb894a8d91e` | 0 | **in scope (queen, 2026-10-11)**: the published corpus and deploy target. Its explorer copy (fourth NGG1 codec, `FORMAT-NGG1.md`, vowl-wasm 0.1.1 tarball pin) is retired by fixing the source in visionGraph (D3) |
| WasmVOWL (standalone) | `WasmVOWL/` | last commit 2026-09-07 | — | **not in the WP-D brief**; vowl-wasm 0.1.1 tarball pin at `modern/package.json:19`, no NGG1 code (decision D3) |

An estate-wide sweep of `/home/devuser/workspace/*` (excluding `node_modules`, `target`, `archive`, `dist`) for vowl-wasm manifest
dependencies, NGG1 magic (`NGG1_MAGIC`, `0x3147474E`, `b"NGG1"`), `vault-core`/`crates/vault` path dependencies and `VisionClaw/<sha>` pins
found nothing beyond the repos above. The other hits are VisionClaw worktrees and clones (`wt/*`, `project2..5`, `.ab-notes`), the cargo
registry cache, and `project4` (the archived `jjohare/logseq` tree with the old `rust-wasm` copy of vowl-wasm). `dreamlab-ai-website` has no
vowl-wasm or NGG1 dependency; its only match was an npm integrity hash.

Method: six read-only Haiku `Explore` census passes (VisionClaw vault crates; the rest of VisionClaw; visionGraph; vowl-wasm and Loom;
Agentbox and VisionFlow; VisionClaw docs), then a specialist pass. LOC is `wc -l`. Test counts are `grep -c '#\[test\]'` / `it(` /
`def test_` per file: these are attribute counts, not executed results. No test was run for this census. I re-checked 48 of the cited
`path:line` anchors against the files: all 48 resolve to the named symbol (command and output in §11). Paths are relative to
`/home/devuser/workspace`.

### Licence boundary (resolved)

`project/crates/vault-core/Cargo.toml:7` and `project/crates/vault/Cargo.toml:7` declare `AGPL-3.0-only`. Several build modules are ports of
the knowledgeGraph Python pipeline (`gate.rs:1`, `build/turtle.rs:1`, `build/webvowl.rs:1`, `build/graph_tiers.rs:1`), which is
`AGPL-3.0-or-later` (`knowledgeGraph/LICENSING.md:18`). Every author in both histories is the owner or the owner's agents
(`git shortlog -sne`; `git log --follow` on the key files), so these licences were chosen by the owner, not inherited from anyone else.

**Rights-holder decision, confirmed by the owner on 2026-10-11 and relayed by the queen:** `vault` and `vault-core`, together with their
`pipeline/*.py` lineage, are relicensed MIT OR Apache-2.0 for extraction into ontomark. The only AGPL carve-out is code descending from
solid-pod or sidestr, which in practice means `vault/src/nostr.rs` (659 LOC) and governance posting. That code stays in VisionClaw and calls
the ontomark crates. Phase 2 records the grant in ontomark `NOTICE` and in ADR-2014.

## 2. Responsibility map and removal gates

| Code | Responsibility | Replacing ontomark owner | Gate before any removal (all must pass against the released crate or CLI) |
|---|---|---|---|
| P | Markdown and frontmatter parsing | `ontomark-parse` | EXP-03 lossless round trip on every public visionGraph page; `vault-core` inline tests (≈126) and `vault-core/tests/real_vocabulary.rs` (6) ported; consumer suites listed per row |
| W | Writing and guarded patching | `ontomark-parse` (guarded patch) | EXP-03 and EXP-08; `vault/tests/{apply_cli,create_cli,propose_cli}.rs`; `visionclaw-domain` round-trip tests (`vault/mod.rs:793-897`) |
| R | Reasoning, closure, subsumption | `whelk-el` + `ontomark-reason` | EXP-01 native == wasm32; `vault/tests/{el_profile,defined_classes,probe_consistency,whelk_scaling}.rs`; `project/tests/inference/*` |
| N | NGG1 encode and decode | `ontomark-compile` (sole codec); vowl-wasm re-exports | EXP-02 and EXP-09; byte identity with `vault/tests/golden/python/graph/*.bin`; vowl-wasm `src/ngg1.rs` tests (9) and `tests/ngg1_explorer_test.rs` (4); one cross-language fixture read by the Rust decoder and by TS |
| T | Taxonomy, category and domain tables | `profiles/estate/` registry, read through `ontomark-model` | EXP-05 (plain vault works, estate schema enforced); golden parity; `check-space-domains.mjs`; `xr-client/rust/tests/domain_palette_parity.rs` (8) |
| I | IRI minting, slugging, namespace mapping | `ontomark-model` identity plus `profiles/estate` namespaces | EXP-02 (byte-identical `urn:ngm:class:` and `resource:` IRIs on the golden corpus and on the full public visionGraph build) |
| V | Validation, conflicts, gate, collisions | `ontomark-compile` validation over `profiles/estate`, surfaced by `ontomark-cli` | EXP-05; `vault/tests/publish_gate.rs` (6); `validate.rs` (27) and `conflicts.rs` (10) inline tests ported; `vault validate/conflicts/gate` JSON identical on the full corpus |
| A | Attribution constants and roster | `profiles/estate` attribution roster, consumed by `ontomark-compile` | EXP-02 and EXP-04; per-class `prov:wasAttributedTo` identical on the golden corpus and on the full build |
| C | RDF compile (Turtle, JSON-LD, SHACL, WebVOWL JSON) | `ontomark-compile` | EXP-02, 04, 05, 09; `vault/tests/golden_parity.rs` (15) |
| CLI | the `vault build/validate/gate/conflicts/repair` door | `ontomark-cli` (`ontomark` binary) | `vault/tests/build_cli.rs`; byte-identical `site-data/` tree from `vault build --vault all` versus `ontomark build` at the same visionGraph sha |

### 2.1 CLI compatibility contract (widens WP-C's drop-in scope)

WP-C's stated acceptance covers `vault build/validate/gate/conflicts`. The estate actually invokes the verbs and flags below. publish.yml
cannot switch to `ontomark` until every row in its group exists with the same flags, JSON shape and exit codes (0 ok, 1 findings,
2 refusal or usage). The agentbox callers cannot switch until their rows exist too, whether in `ontomark` or in the governance adapter (D4).

| Verb and flags | Callers |
|---|---|
| `validate --vault all` (text report; exit status gates) | visionGraph `publish.yml:160`; `CLAUDE.md:19` |
| `validate --vault all --json` | `finalise.sh:22` |
| `validate --strict --json` | agentbox `elevation-stage.js:200`; `ontology-augment.sh:487` (`report validate --json`) |
| `build --vault all --out site-data --publish-out quartz/content --with-markdown-mirror --stats`; output layout incl. `<out>/publish/` when `--publish-out` is absent (`publish.yml:195-200`) | `publish.yml:188-193` |
| `build --vault all --out <dir> --json`; `build --stats` | `finalise.sh:39`; `CLAUDE.md:48` |
| `--repo <path> build --vault knowledge --out <dir>` (global `--repo`) | VisionClaw `ontology-publish.yml:172-174` |
| `build --with-rvdb` (named in Loom's error text) | Loom `artifact.rs:184`, `docs/published-generation-reload.md:99-101` |
| `repair bodies --vault all --check`; `repair bodies --vault knowledge --check`; `repair bodies --vault all` (fix mode) | `publish.yml:226`; `CLAUDE.md:20`; `finalise.sh:40` |
| `conflicts --vault knowledge`; `conflicts --vault all --json`; `conflicts --severity all` | `CLAUDE.md:22`; `finalise.sh:23`; `docs/source-repair-plan-2026-09-22.md:37` |
| `gate --tier full --vault all [--json]` | `CLAUDE.md:24`; `finalise.sh:20,24` |
| `--version` | `publish.yml:170`; agentbox `entrypoint-unified.sh:1130-1169` |
| `find --query <q> --limit N --json`; `find --query <q> --type Class --limit N --json`; bare-array JSON (`ontology-augment.sh:385`) | `ontology-apply.js:221`; `submit-proposals.mjs:94`; `ontology-augment.sh:384,408,421` |
| `retrieve <id…> --expand is-a=N,requires=N,enables=N,part-of=N --max-documents N --json`; `retrieve <id> --json` | `ontology-augment.sh:398-400,415` |
| `tree <iri> --depth 1 --json` | `ontology-augment.sh:454` |
| `edit <page> --set k=v… --expect <expectation> --json` | `ontology-apply.js:419-424` |
| `create <file> --expect docs=1 --set …` | `ontology-apply.js:478`; `space-corpus/src/main.rs:210` |
| `apply <file> --expect docs=1 --set 'verified+={…}'` | `ontology-apply.js:668` |
| `propose <iri> --level L --hypothesis H [--diff D] [--title T] [--dry-run] --json` (posts to the forum: AGPL, D4) | `ontology-propose.js:239-245`; `submit-proposals.mjs:162`; `ontology-augment.sh:498` |
| `panel publish` (AGPL, D4) | VisionClaw `main.rs:63` (no estate caller found) |

Dispositions used below: **DELETE** (superseded; removed in Phase 2), **REPIN** (consumer stays; dependency or invocation changes),
**ADAPT** (code stays in its repo but calls ontomark instead of its own logic), **RETAIN** (not a duplicate: a serving, physics,
presentation or transport concern, recorded so nobody deletes it by mistake), **DECIDE** (needs a queen decision, §8).

## 3. VisionClaw (`project/`)

### 3.1 `crates/vault-core` (6,336 LOC, AGPL, `publish = false`)

The extraction source for `ontomark-parse` and `ontomark-model`. Root-crate consumers: `src/services/page_parser.rs:17,21,69,127,183`,
`src/services/corpus_source/{mod.rs:28,local.rs:9}`, `src/services/github_sync_service.rs:36,2241,2260`,
`src/actors/elevation_actor.rs:584-907`, `src/actors/gpu/force_compute_actor.rs:1046`, `src/actors/gpu/gpu_resource_actor.rs:228`;
also visionGraph `scripts/space-corpus` (§4.3). CI lists `-p vault-core` at `.github/workflows/ci.yml:90`.

| Id | path:line | LOC | Resp | What | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|---|
| VC-01 | `crates/vault-core/src/page.rs:150` `Page::parse`, `:557` `parse_page`, `:38` `UNPUBLISHED_DIRS` | 754 | P I T | page parse, slug and resource IRI, unpublished dirs | 14 inline | ontomark-parse, ontomark-model | DELETE |
| VC-02 | `crates/vault-core/src/frontmatter.rs:119` `split`, `:166` parse, `:186` render, `:326` `CANONICAL_ORDER` | 442 | P W | YAML frontmatter split, parse and render; canonical key order | 8 | ontomark-parse | DELETE |
| VC-03 | `crates/vault-core/src/vocabulary.rs:53` `BUILTIN_PREFIXES` (incl. narrativegoldmine `:62-64`), `:94` `LOGICAL_NAMESPACES`, `:107` `NON_EL_OWL_TERMS`, `:136,:140` `urn:ngm:` | 1251 | T I V | vocabulary model over `ontology/vocabulary.yaml` | 23 + `tests/real_vocabulary.rs` 6 | ontomark-model + profiles/estate | DELETE |
| VC-04 | `crates/vault-core/src/slug.rs:29` `slugify`, `:88` `iri_for` | 139 | I | the canonical slug rule (`[^a-z0-9]+`, `:17`) | 4 | ontomark-model | DELETE |
| VC-05 | `crates/vault-core/src/domains.rs:4` `DOMAIN_ROOTS` (8), `:19` `DOMAIN_SLUGS`, `:33` `domain_class_id` | 67 | T | domain registry; NGG1 domain ids | 1 | profiles/estate | DELETE |
| VC-06 | `crates/vault-core/src/consistency.rs:50` `DOMAIN_ROOT_SLUGS`, `:62` `CATEGORY_SLUGS` (53), `:164` `check_disjoint_pair`, `:207` `whelk_inconsistent` | 299 | V R T I | disjointness and Whelk-inconsistency helpers; the 53-category table | 7 | ontomark-reason (checks), profiles/estate (tables) | DELETE |
| VC-07 | `crates/vault-core/src/definition.rs:167` `parse`, `:78` `NON_EL_KEYWORDS` | 431 | P V | EL defined-class expression parser | 8 | ontomark-parse / ontomark-model | DELETE |
| VC-08 | `crates/vault-core/src/graph.rs:108` `VaultGraph`, `:22` `LINK_PREDICATE`, `:435` `urn:ngm:` | 558 | P I | link graph | 9 | ontomark-model | DELETE |
| VC-09 | `crates/vault-core/src/okf.rs:73` `Actor`, `:165` `Stamp`, `:240` `OkfBlock` | 367 | V A | OKF block and actor grammar | 6 | ontomark-model + profiles/estate | DELETE |
| VC-10 | `crates/vault-core/src/promotion.rs:30` `PromotionState`, `:219` `apply` | 435 | V | promotion state machine (lifecycle) | 9 | profiles/estate lifecycle in ontomark-model | DELETE |
| VC-11 | `crates/vault-core/src/proposal.rs:18` `urn:ngm:proposal:`, `:99` `PatchProposal` | 538 | V I | governance proposal model | 11 | ontomark-model (proposal type); posting stays AGPL (VC-36) | DELETE (type) |
| VC-12 | `crates/vault-core/src/secrets.rs:166` `scan` | 322 | V | credential scan before publication | 5 | ontomark-compile publication guard | DELETE |
| VC-13 | `crates/vault-core/src/{code.rs:42,json.rs:137,error.rs}` | 644 | P W | code-span map, deterministic JSON writers | 18 | ontomark-parse / ontomark-compile | DELETE |

### 3.2 `crates/vault` (20,472 LOC incl. `nostr.rs`; AGPL; binary `vault`)

No Rust consumer outside its own binary and tests. Consumers are the binary's callers (§3.4, §7.2). Integration tests:
`tests/golden_parity.rs` 15, `apply_cli.rs` 10, `publish_gate.rs` 6, `whelk_scaling.rs` 6, `defined_classes.rs` 4, `create_cli.rs` 2,
`el_profile.rs` 2, `probe_consistency.rs` 2, `build_cli.rs` 1, `propose_cli.rs` 1. Golden fixtures: `tests/golden/fixture/` (50 pages and a vocabulary)
and `tests/golden/python/` (immutable 2026-09-22 reference: `ontology.ttl` 110,172 B, `ontology.json`, three indexes, nine NGG1 tier files).
Dependency closure: whelk git `79a1ee2`, horned-owl 1.2.0, oxttl/oxrdf (tests only), `nostr-bbs-core =1.0.0-beta.15`, `tungstenite` (native-tls),
`ureq`, `similar`, `rayon` (**declared but unused**: no `rayon` reference in `src/` or `tests/`).

| Id | path:line | LOC | Resp | What | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|---|
| VC-14 | `crates/vault/src/model.rs:36` `RELATION_KEYS` (12, frozen), `:107` `EntityType`, `:200` `Corpus` | 678 | T P | corpus model; ADR-2015 defects 1, 2, 4 live here (`:105,:217,:335,:356,:367,:426`) | 10 | ontomark-model | DELETE |
| VC-15 | `crates/vault/src/whelk.rs:505` `reason`, `:514` `reason_capped`, `:432` `RELEVANT_TRANSITIVE_CAP`, `:742` `inferred_turtle`; native timing `:647,:667,:677,:682` | 1329 | R C | Whelk wrapper over horned-owl; inferred TTL | 20 + 4 integration files | whelk-el + ontomark-reason | DELETE |
| VC-16 | `crates/vault/src/closure.rs:87` `compute`, `:26` `INHERITED_RELATION_CAP` | 349 | R | navigation closure (distinct from OWL entailment; ADR-2137 §3) | 8 | ontomark-reason (closure as a named projection) | DELETE |
| VC-17 | `crates/vault/src/build/ngg1.rs:39-66` constants, `:113` `build_csr`, `:150` `pack`, `:328` `force_layout`, `:438` `bake_positions` | 716 | N | **the only NGG1 encoder in Rust** | 16 | ontomark-compile `ngg1` | DELETE |
| VC-18 | `crates/vault/src/build/graph_tiers.rs:87` `CATEGORY_ORDER` (53; position = NGG1 category id), `:44` `PIPELINE_VERSION`, `:49` `ATTRIBUTED_TO`, `:51` `ATTRIBUTED_LABEL`, `:52` `CORPUS_NATURE`, `:54` `NGM_NS_BASE` | 1297 | T I A | tier model, overview, bridges | 22 | ontomark-compile + profiles/estate | DELETE |
| VC-19 | `crates/vault/src/build/turtle.rs:37-55` `VC/NGM/NGMI/ONTOLOGY_IRI/VERSION_INFO`, `:65` `MATURITY_LEVELS`, `:209` `build_graph_attributed`, `:326` RELATION_KEYS loop, IRI prefixing `:824-849` | 1740 | C I A | Turtle emitter (ADR-2015 defects 3, 5) | 31 | ontomark-compile | DELETE |
| VC-20 | `crates/vault/src/build/webvowl.rs:39-41` base IRIs, `:67` `remap_iri`, `:92` `build` | 427 | C I | WebVOWL JSON | 11 | ontomark-compile | DELETE |
| VC-21 | `crates/vault/src/build/attribution.rs:24` `Roster`, `:31` `discover` (reads `ontology/attribution.yaml`), `:84` `resolve` | 111 | A | generator roster | 1 | profiles/estate roster | DELETE |
| VC-22 | `crates/vault/src/build/{indexes.rs:30,:96,:253,:308; page_api.rs:18,:55; okf.rs:57}` | 1282 | C W | scaffold, prose and search indexes; page API; OKF bundle (Loom inputs) | 27 | ontomark-compile | DELETE |
| VC-23 | `crates/vault/src/build/generation.rs:106` `generation_id`, `:118,:140` git | 447 | H A | generation identity | 13 | ontomark-cli (host) | DELETE |
| VC-24 | `crates/vault/src/build/{mod.rs:291,publish.rs:176,rvdb.rs:29,:89}` | 2063 | H V | build orchestrator, publish staging, rvdb embeddings (hard-coded `http://192.168.2.132:9997` at `rvdb.rs:29`) | 34 | ontomark-cli; rvdb behind an optional capability with configurable endpoint | DELETE |
| VC-25 | `crates/vault/src/validate.rs:345` `validate` | 1377 | V | OKF and vocabulary validation | 27 | ontomark-compile validation | DELETE |
| VC-26 | `crates/vault/src/conflicts.rs:312` `analyse` | 556 | V | conflict analysis | 10 | ontomark-compile | DELETE |
| VC-27 | `crates/vault/src/gate.rs:122` `run` | 326 | V | tiered quality gate | 8 + `publish_gate.rs` 6 | ontomark-cli `gate` | DELETE |
| VC-28 | `crates/vault/src/projection.rs:69,:166,:375`; IRI map `:347-362` | 624 | V I | public projection, exclusion before inference | 10 | ontomark-compile (EXP-04) | DELETE |
| VC-29 | `crates/vault/src/{edit.rs:193,create.rs:138,apply.rs:215,diff.rs:270}` | 1992 | W V | guarded edit, create, apply and patch | 40 + `apply_cli`/`create_cli` | ontomark-parse guarded patch | DELETE |
| VC-30 | `crates/vault/src/propose.rs:466` `build` | 1434 | W V P | proposal builder | 30 + `propose_cli` | ontomark-model proposal + parse patch; posting stays (VC-36) | ADAPT |
| VC-31 | `crates/vault/src/{repair.rs:334,bodies.rs:395}` | 1763 | W V | `repair fences` and `repair bodies` (Obsidian body hygiene) | 41 | ontomark-parse body module | DELETE |
| VC-32 | `crates/vault/src/main.rs` (subcommand enum `:44-106`) | 1223 | H | CLI | 8 | ontomark-cli for build/validate/gate/conflicts/repair; see D4 for governance verbs | ADAPT |
| VC-36 | `crates/vault/src/nostr.rs:60,:100,:181,:272`; call sites `main.rs:703,:749-752` | 659 | G | **AGPL** forum ActionRequest, NIP-42 auth, relay publish | 16 | stays in VisionClaw; re-wired to ontomark types | **RETAIN (AGPL)** |

### 3.3 Duplicates elsewhere in VisionClaw

| Id | path:line | LOC | Resp | What | Consumers | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|---|---|
| VC-40 | `crates/visionclaw-domain/src/vault/mod.rs:278` `parse`, `:288` `split`, `:378,:393` frontmatter, `:197` `to_frontmatter_yaml`, `:308` `render_page` | 1222 | P W | **second parser and writer** (the module doc and ADR-2096 call it the single entry point; ADR-2113 says `vault-core`) | `github_sync_service.rs:1723,2070`; `file_service.rs:766-786`; `knowledge_graph_parser.rs:146`; `decision_elevation.rs:322,365`; `ontology_mutation_service.rs:100,495,532`; `elevation_actor.rs:566,722`; `src/bin/load_ontology.rs:96` | 44 inline; `visionclaw-domain/tests/vault_fixtures.rs` 5; `tests/vault_gate_test.rs` 18 | ontomark-parse | DELETE; callers REPIN |
| VC-41 | `src/services/ontology_enrichment_service.rs:151` `extract_frontmatter` | 289 | P | hand-rolled `key: value` reader | `:69` (service constructed only in `src/test_helpers.rs:270`) | 2 | ontomark-parse | DELETE |
| VC-42 | `src/services/page_parser.rs:26,:30` `urn:ngm:` constants, `:66` wrapper | 430 | I P | wrapper over vault-core plus duplicate namespace constants | `load_ontology.rs`, `github_sync_service.rs` | 10 | ontomark-parse + model | REPIN |
| VC-43 | `src/services/ontology_mutation_service.rs:51` `generate_vault_markdown`, `:754` `generate_term_id` (legacy prefix map `:756-760`), `:792` `check_consistency` | 1276 | W T V | page generation and patch; second Whelk consistency gate | `:341` | 4 | ontomark-parse patch + ontomark-reason | ADAPT |
| VC-44 | `src/services/decision_elevation.rs:261` draft page, `:139` `slugify`, `:200` generator `process:visionclaw/{ver}` | 1015 | W I A | decision page writer, Unicode slug, generator stamp | `decision_elevation_actor.rs:325,605` | 14 | ontomark-parse + model; generator via profile | ADAPT |
| VC-45 | `src/actors/elevation_actor.rs:510` `draft_class_page`, `:481` `slugify` (copy of VC-44), `:791` disjointness gate | 2553 | W I V | class draft writer and gate | internal | 30 | ontomark-parse + model + reason | ADAPT |
| VC-46 | `crates/visionclaw-adapters/src/whelk_inference_engine.rs:73`, `:319` `check_axiom_set` | 906 | R | **second Whelk adapter** (horned-owl translation `:15-20`) | `src/main.rs:531`; `github_sync_service.rs:199,226,3024`; `elevation_actor.rs:881-885`; `ontology_mutation_service.rs:797`; `decision_service.rs:406,641` | 13 + `tests/inference/{classification,consistency,explanation}_tests.rs` | ontomark-reason behind the `InferenceEngine` port | DELETE impl; port REPIN |
| VC-47 | `crates/visionclaw-ontology/src/reasoning/custom_reasoner.rs:60` `compute_transitive_closure`, `:224` `is_subclass_of` | 433 | R | hand-rolled closure | `ontology_pipeline_service.rs:242`; `ontology_reasoning_service.rs:153` | 5 + `tests/integration/{semantic_physics_integration,pipeline}_test.rs` | ontomark-reason closure projection | DELETE (D5) |
| VC-48 | `src/services/ontology_reasoner.rs:72` `precompute_transitive_closure`, `:131` `get_ancestors` | 578 | R | third closure store; **test-only**: its sole user `OntologyEnrichmentService` (VC-41) is constructed only in `src/test_helpers.rs:270` | `ontology_enrichment_service.rs:11`; `test_helpers.rs:260` | 3 | none needed | DELETE (dead) |
| VC-49 | `src/services/inferred_edge_materialiser.rs:98,:181` | 446 | R | transitive reduction to immediate parents | not traced to production | 14 | ontomark-reason (asserted/inferred split) | DECIDE (D5) |
| VC-50 | `crates/visionclaw-ontology/src/open_world.rs:163` ancestors, `:451` version IRI from Turtle, `:541` hand-rolled Turtle tokenizer, `:37-47` graph IRIs | 1354 | R C I | open-world query over a supplied closure | `ontology_query_service.rs:22,125-201` | 22 | ontomark-compile reader + model | ADAPT |
| VC-51 | `crates/visionclaw-ontology/src/services/owl_validator.rs:357`, `:797` `validate_domain_range` | 1262 | V | OWL validation that enforces domain/range, contrary to ADR-2126/2128 (validation never infers) | `ontology/services/mod.rs:7`; `tests/ontology_actor_integration_test.rs:39` | 8 | ontomark-compile validation | DELETE |
| VC-52 | `crates/visionclaw-ontology/src/services/jsonld_validator/{shacl_lite.rs:18,owl_el_profile.rs:76,class_bit.rs:74,frame.rs:87,shacl.rs:222,mod.rs:260}` | 2233 | V | parallel JSON-LD/OWL-EL validator stack for retired fences (ADR-2112) | `jsonld_ingest/shacl_gate.rs:21` | `tests/jsonld_validator_test.rs` 4; `shacl.rs` 17 | ontomark-compile SHACL | DELETE |
| VC-53 | `crates/visionclaw-ontology/src/inference/owl_parser.rs:175` `parse_turtle` (stub) | 331 | C | stub returning UnsupportedFormat | internal | 4 | — | DELETE |
| VC-54 | slug variants: `src/uri/mod.rs:200`; `visionclaw-ontology/src/services/canonical_iri.rs:59` (NFKD); `jsonld_ingest/expander.rs:576` (a wrapper over `canonical_iri::slugify`, not a rule of its own); `src/services/parsers/knowledge_graph_parser.rs:386` (ASCII, diverges from `:380`); `visionclaw-adapters/src/oxigraph_ontology_repository.rs:140`; `client/src/features/graph/utils/pageLinks.ts:19` | — | I | **six slug rules** besides vault-core (corrected in Phase 2 prep: `expander.rs:576` wraps `canonical_iri.rs:59`) | per file | per file | ontomark-model slug (TS via ontomark-wasm) | DELETE all but one |
| VC-55 | IRI constants: `crates/visionclaw-domain/src/uri.rs:22,:31`; `src/uri/mod.rs:41,:45,:336`; `visionclaw-adapters/src/{rdf_serializer.rs:42,provenance_emitter.rs:45,sparql_migrations.rs:44}`; `client/src/services/solidPod/typeIndex.ts:30` (**`…/ontology#` differs from `VC_NS …/ns/v1#`**); `client/src/features/ontology/services/inferredAxiomsService.ts:39`; `scripts/ontology/pack-pod-resources.py:31`; PROV constant defined four times | — | I C | namespace and mint constants | per file | per file | profiles/estate namespaces via ontomark-model | REPIN; fix drift |
| VC-55a | PROV namespace constant defined **five** times, not four (Phase 2 prep, checked by symbol): `src/services/decision_service.rs:49`, `src/services/provenance_writer.rs:100`, `crates/visionclaw-ontology/src/services/jsonld_ingest/expander.rs:56`, `crates/visionclaw-adapters/src/provenance_emitter.rs:43`, `crates/vault/src/build/turtle.rs:35` | — | I | duplicate constant | REPIN to ontomark-model's namespace table (step 2; the step 2 test requires zero local definitions) | EXP-02 |
| VC-56 | `src/services/github_sync_service.rs:2409` `DOMAIN_TABLE`, `:2474` `derive_source_domain` | 3215 | T | keyword-to-domain classifier | `:1677,1875,2500` | 20 | profiles/estate registry | ADAPT |
| VC-57 | `src/services/parsers/knowledge_graph_parser.rs:145,253,263` | 501 | P | graph nodes from PageMeta | `github_sync_service.rs:28-223` | 7 | ontomark-model types | REPIN |
| VC-58 | `crates/visionclaw-contracts/src/github_adapter.rs:63` `ParsedMarkdown` (+ TS binding) | 152 | P | wire type still carrying `jsonld_blocks` | sdk bindings | 2 | ontomark-model | REPIN |
| VC-60 | `.github/workflows/ontology-publish.yml:168,172-174` (`cargo build -p vault`; `vault build --vault knowledge`), `:194` `pack-pod-resources.py` | — | CLI | VisionClaw-side publish | — | — | pod-resource pack produced from the shared build on the Agentbox side; VisionClaw pulls it | RETIRE (gated; was REPIN until 2026-10-12, see §13 and ADR-OM-005 decision 2; depends on VisionClaw ADR-2106) |

RETAIN (not duplicates, recorded so Phase 2 leaves them alone): domain colour palettes in `client/src/features/graph/utils/domainColors.ts:4`,
`xr-client/rust/src/domain_palette.rs:24`, `xr-client/scripts/hud.gd:262` and the cluster palettes (`hulls.rs:48`, `ClusterHulls.tsx:56`) are
presentation. They key on the domain slugs, so they REPIN their slug list to the profile but keep their colours. The 52-byte websocket node
protocol (`src/utils/binary_protocol.rs:571`, `client/src/store/websocket/binaryProtocol.ts`, `xr-client/rust/src/binary_protocol.rs:32`) is a
live transport, **not NGG1**. Oxigraph stores (`oxigraph_ontology_repository.rs`, `rdf_serializer.rs`) are rebuildable query projections
(ADR-2137 §1). Physics and constraint projections stay explicit (ADR-2137 §3).

### 3.4 VisionClaw consumers of the `vault` binary

`.github/workflows/ontology-publish.yml:168-174` (VC-60) and `.github/workflows/ci.yml:90-91` (`-p vault-core`, `-p vault`). No Dockerfile,
compose or supervisord file invokes it.

## 4. visionGraph (`visionGraph/`)

### 4.1 Producer and package pins

| Id | path:line | Value | Consumers | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|
| VG-P1 | `.github/workflows/publish.yml:50` `VAULT_GIT_REV` | `1044b0722e6166c8936caec7aacf0ed4c9f7647b` | checkout `:110-116`, cache key `:130`, build `:133-146`, summary `:494` | none (CI only) | `ONTOMARK_RELEASE_TAG` downloading the released `ontomark` binary, plus a pinned-tag source fallback from the same release; VisionClaw checkout removed | REPIN |
| VG-P2 | `.github/workflows/publish.yml:43-45` | `VAULT_RELEASE_REPO=DreamLab-AI/VisionClaw`, `VAULT_RELEASE_TAG=''`, `VAULT_ASSET=vault-x86_64-unknown-linux-gnu` | `:83-97` | none (CI only) | ontomark release repo, tag and asset | REPIN |
| VG-P3 | `publishing-tools/WasmVOWL/modern/package.json:20`; lock `package-lock.json:595-599` | `@dreamlab-ai/vowl-wasm` `0.1.2` exact, registry integrity | `src/workers/physics.worker.ts:99,101`; `vite.config.ts:22` | modern `vitest run` does not load the WASM binary; none exercise the pinned package | vowl-wasm release that re-exports ontomark NGG1 | REPIN |
| VG-P4 | `scripts/space-corpus/Cargo.toml:18` | `vault-core = { path = "../../../project/crates/vault-core" }` (sibling path, no sha; `Cargo.lock:710` has no source) | `scripts/space-corpus/src/main.rs:20` | space-corpus `#[test]` 4 | `ontomark-parse`/`ontomark-model` by version | REPIN |
| VG-P5 | unversioned `vault` on PATH: `scripts/space-corpus/src/main.rs:210,:528`; `transcripts/test_weekly_ingest.py:96`; `planning/space-science-and-geo-sensing/research/tooling/finalise.sh:20-40`; `docs/migration-2026-09-22/proposals/regenerate.sh:10,41` | — | `transcripts/test_weekly_ingest.py` 23 (skips when `vault` is absent, `:96`) | — | `ontomark` CLI (governance verbs per D4) | REPIN |
| VG-P6 | `publishing-tools/WasmVOWL/scripts/check-versions.sh:38` | reads `publishing-tools/WasmVOWL/Cargo.toml`, which does not exist | — | none | delete or point at the vowl-wasm pin | DELETE |

### 4.2 Explorer (TypeScript)

| Id | path:line | LOC | Resp | What | Consumers | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|---|---|
| VG-01 | `publishing-tools/WasmVOWL/modern/src/lib/ngg1.ts:21-49` constants, `:64` header, `:139` `parseNgg1`, `:386` `serializeEgoNgg1`, `:488` `serializeNgg1`, `:567` CSR helpers | 624 | N | **third NGG1 codec, encoder and decoder** | `GraphPage.tsx:223`; `scopeStore.ts:441-494`; `palette.ts:10` | `ngg1-serialize.test.ts` 3; `focus-from-collapsed.test.ts` 4 | vowl-wasm re-export of ontomark NGG1 (decode and ego encode in WASM); TS keeps typed views over **generated** constants (ADR-001) | DELETE codec (D6) |
| VG-02 | `publishing-tools/WasmVOWL/modern/src/lib/__fixtures__/ngg1-3n2e.bin` | — | N | golden reader fixture **referenced by no test** | — | 0 | becomes the shared cross-language fixture in `conformance/fixtures` | ADAPT |
| VG-03 | `src/types/scope.ts:64-72` `DomainSlug`, `:75` `DOMAIN_SLUGS` (8), `:86` `DOMAIN_LABELS` | 246 | T | domain registry | scopeStore, GraphPage, LabelsTopN, EdgeLegend, `domains.ts:10`, `pageMeta.ts` | `spaceDomains.test.ts` 2; `focus-from-collapsed.test.ts` 4 | generated from profiles/estate | REPIN (generated) |
| VG-04 | `src/site/domains.ts:24,36,57`; `src/components/Canvas/palette.ts:50-57` | 226 | T | icons, blurbs, colours keyed by domain id | site views | `spaceDomains.test.ts` | presentation; keyed by generated registry | RETAIN, REPIN keys |
| VG-05 | `scripts/check-space-domains.mjs:17-27` | 55 | V T | CI bundle gate with a second hard-coded domain list (`publish.yml:297`) | publish.yml | self | read the generated registry | ADAPT |
| VG-06 | `src/site/pageMeta.ts:27` `AUTHOR_DID = 'did:nostr:jjohare'`, `:76` `slugFromClassIri` | 178 | A I | author constant **differs from the roster key** in `graph_tiers.rs:49` (`did:nostr:d598abc6…`) | PageView, SearchView, HomePage | 0 | profile roster via page API | DELETE |
| VG-07 | `src/api/pageService.ts:121` `quartzPageUrl`, `:193-216` lookup | 292 | I | Quartz slug mirror; id/IRI/slug lookup | `pageService.ts:246`; `hooks/usePage.ts` | 0 for these | RETAIN (Quartz URL is a host concern); id lookup REPIN to model IRIs | ADAPT |
| VG-08 | `src/lib/vaultMarkdown.ts:13,16,21,36,51` | 64 | P | frontmatter strip and wikilink rewrite for display | PageView, MarkdownRenderer, pageService | `vaultMarkdown.test.ts` 7 | ontomark-wasm body view | DECIDE (D6) |
| VG-09 | `src/pages/PageView.tsx:144-150` JSON-LD emitter; `static/ns/v2.jsonld` (143) | — | C | per-page JSON-LD and the published context | browser | 0 | ontomark-compile JSON-LD (page API carries it) | DELETE emitter; context generated |

Quartz (`quartz/quartz/plugins/transformers/frontmatter.ts:55-126`, `util/path.ts:57`, `components/Properties.tsx:48-70`,
`quartz.config.ts:126` `ExplicitPublish`) is the site renderer's own frontmatter handling: **RETAIN**, upstream Quartz code. The publication rule
duplication (`ExplicitPublish({key:"public"})` against the producer's `public: true` gate) is guarded by publish staging, which only stages
public pages (`vault build --publish-out`, `publish.yml:167-217`).

### 4.3 Corpus maintenance tooling (Python and Rust)

None of the eleven `scripts/ontology-hygiene/*.py` has a test, yet several mutate the corpus.

| Id | path:line | LOC | Resp | What | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|---|
| VG-20 | `scripts/ontology-hygiene/fmlib.py:6` `split`, `:14` `join`, `:32` `set_list`, `:49` `slug_title_map` | 58 | P W | hand-rolled block-YAML reader and writer shared by the scripts below | 0 | `ontomark` CLI JSON (parse) and guarded patch | DELETE |
| VG-21 | `scripts/ontology-hygiene/collision_lint.py:14` `UK_US`, `:19` `norm`, `:37` `CATEGORY_ROOTS` (6), `:42` | 87 | V I T | collision lint; a CLAUDE.md gate (must exit 0) | 0 | `ontomark lint collisions` over profiles/estate + `ontology/collision-allowlist.yaml` | DELETE (D7) |
| VG-22 | `scripts/ontology-hygiene/merge_plan.py:11-13,94,129` | 170 | V T | merge planner | 0 | `ontomark` decisions planner | DELETE (D7) |
| VG-23 | `scripts/ontology-hygiene/apply_decisions.py:21,30-36,50-122` | 126 | W V | applies decisions, regenerates `docs/ontology/collision-register.md` and the allowlist | 0 | `ontomark migrate`/guarded patch (EXP-08) | DELETE (D7) |
| VG-24 | `scripts/ontology-hygiene/assign_homes.py:20-32` hubs and roots, `:34` own `set_list`, `:68` `resource: urn:ngm:class:` rewrite | 70 | W I T | home assignment; **rewrites resource IRIs** (identity risk under non-negotiable 4) | 0 | ontomark guarded patch; IRIs immutable | DELETE (D7) |
| VG-25 | `scripts/ontology-hygiene/last_mile.py:12-64` (`DOMAIN_ROOTS` `:35`) | 65 | W T R | one-off category and is-a repair | 0 | — (one-off; already applied) | DELETE |
| VG-26 | `scripts/ontology-hygiene/resolve_conflicts.py:11,39,65,121,140` | 144 | R W V | hand-listed cycle drops and orphan parents | 0 | ontomark-reason + conflicts | DELETE |
| VG-27 | `scripts/ontology-hygiene/apply_bridges.py:26` `closure()` (reads gitignored `.research-scratch/closure-supers.json`), `bridge_candidates.py:52` | 122 | R W T | bridge application against a cached closure | 0 | ontomark-reason closure | DELETE (D7) |
| VG-28 | `scripts/ontology-hygiene/attribute.py:12` roster reader, `:53-57` actor grammar | 62 | A P W | attribution writer | 0 | profiles/estate roster + guarded patch | DELETE |
| VG-29 | `scripts/space-corpus/src/main.rs:249` `reaches` (is-a DFS), `:388` `header()` writer (`process:space-corpus/0.1.0`), `:404` `validate_catalogue` | 681 | R W A V | catalogue importer (ADR-VG-006) | 4 | ontomark-parse/model by version; reachability via ontomark-reason | ADAPT |
| VG-30 | `transcripts/weekly_ingest.py:117` `split_frontmatter`, `:165` `render_page`, `:62` `PROCESS_ACTOR` | 920 | P W A | podcast ingest page writer (PyYAML) | `test_weekly_ingest.py` 23 | `ontomark` CLI create/patch | ADAPT (D7) |
| VG-31 | `planning/space-science-and-geo-sensing/research/audit-eo-staged.py:14,46`; `research/tooling/check_pages.py:23-30` | 155 | P R V | research audits | 0 | — (planning evidence) | RETAIN as historical; do not maintain |

`.research-scratch/*.py` (20 files) is gitignored and out of scope.

## 5. vowl-wasm (`vowl-wasm/`, MIT, crate and npm 0.1.2)

Frozen JS API (keep): `init`, `version` (`src/lib.rs:170,181`); `WebVowl` methods (`src/bindings/mod.rs:45-769`); `NggExplorer`
(`src/bindings/explorer.rs:30-109`, feature `ngg1`). Published bundle features: `ngg1,interaction` (README.md:189). Tests: 214 `#[test]`
(165 in `src/`, 49 in `tests/`), 8 browser-only `wasm_bindgen_test`. No JS tests.

| Id | path:line | LOC | Resp | What | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|---|
| VW-01 | `src/ngg1.rs:19-49` constants, `:128` `Ngg1::parse` and accessors `:217-324` | 499 | N | second NGG1 decoder (Rust); cites a `FORMAT-NGG1.md` absent from this repo (`:3`) | 9 + `tests/ngg1_explorer_test.rs` 4 | `pub use` of a dependency-light NGG1 codec (its own crate or a no-default-features `ontomark-compile` module) behind the unchanged `vowl_wasm::ngg1` path and `NggExplorer` API. The published bundle (232,052 B, README.md:191) must not gain reasoner, RDF or YAML dependencies: a bundle-size check joins the gate | DELETE body, keep re-export |
| VW-02 | `src/ontology/markdown_parser.rs:56` `parse`, `:149`, `:171` `build_full_iri` | 386 | P I | Markdown ontology-block parser (feature `markdown-ontology`, **not in the published bundle**) | 5 + `tests/markdown_parser_test.rs` 10 | ontomark-parse (behind the same feature) or retire `parseMarkdownOntology` | DECIDE (D8) |
| VW-03 | `src/ontology/mod.rs:216` `Domain` enum, `:244` `from_prefix`, `:269` `base_uri` | 403 | T I | domain enum with base URIs | 4 | profiles/estate registry | DELETE |
| VW-04 | `src/ontology/owl2_validator.rs` | 790 | V | OWL 2 validator (feature `markdown-ontology`) | 14 + `tests/owl2_validation_test.rs` 10 | ontomark-compile validation | DECIDE (D8) |
| VW-05 | `src/graph/mod.rs:481` `apply_hierarchy_folding`, root detection `:491-512` | 939 | R (view) | presentation folding over SubClass edges | 14 | — | RETAIN (presentation) |
| VW-06 | `src/layout/*`, `src/render`, `src/interaction`, `src/graph/{builder,node,edge,pinning,statistics}.rs` | ≈5,800 | — | layout engine and render data | ≈120 | — | RETAIN (ADR-001: layout stays here) |
| VW-07 | `src/ontology/{parser.rs,loader.rs}` | 1222 | P | WebVOWL JSON and filesystem loaders | 12 | — | RETAIN (WebVOWL JSON is the crate's input contract, not frontmatter) |

## 6. Loom (`loom/`, MIT OR Apache-2.0)

Loom consumes artefacts and never parses Markdown (no YAML or Markdown crate in any manifest). Its duplicates are identity and contract logic.

| Id | path:line | LOC | Resp | What | Consumers | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|---|---|
| LM-01 | `crates/loom-domain/src/model.rs:32-34` `Iri::from_slug` | 531 | I | canonical `urn:ngm:class:` mint | `loom-scaffold/src/{grounding.rs:32,lib.rs:489-495,manifest.rs}` | `model_tests.rs` 12 | ontomark-model identity (MIT/Apache, licence-compatible) | REPIN |
| LM-02 | `crates/loom-vector-ruvector/src/bin/stage_corpus.rs:101`; `src/hnsw.rs:446` | 348 / 808 | I | two more `urn:ngm:class:{}` literals bypassing LM-01 | — | 5 / 15 | LM-01 path (then ontomark-model) | DELETE literals |
| LM-03 | `crates/loom-scaffold/src/index.rs:122` `slugify`, `:130` `ref_to_slug` | 293 | I | slug and ref parsing | scaffold | `tests.rs` 29 | ontomark-model | REPIN |
| LM-04 | `crates/loom-scaffold/src/index.rs:15,61,198,239`; `prose.rs:32`; `bin/build_concept_records.rs:269-324` | 835 | P (artefact) | readers of `scaffold-index.json` and `prose-index.json` | `loom-facade/src/config.rs:27,154` | 29 + 6 | typed artefact structs exported by ontomark-compile (ADR-142 §2: only after API review) | DECIDE (D9) |
| LM-05 | `crates/loom-graph-oxigraph/src/lib.rs:37-38` `ALLOWLIST`, `:171-205` load | 593 | C (consumer) | Turtle allowlist (`ontology.ttl`, `ontology-inferred.ttl`) | `loom-facade/src/lib.rs:103` | 17 + `tests/integration.rs` 6 | — | RETAIN; filenames come from ontomark's bundle manifest |
| LM-06 | `crates/loom-facade/src/mirror.rs:89-144`; `scripts/reload-published-generation.py:28-43` | 338 / 196 | V | generation-marker check in Rust **and** Python | bundle activation; deploy timer | `mirror_generation.rs` 4, `adr141_*` 7; `test_reload_published_generation.py` 16 | marker schema from ontomark-compile; Python copy calls the Rust reader or is deleted | ADAPT |
| LM-07 | `crates/loom-domain/src/bundle.rs:82` `digest_input`; `loom-facade/src/bundle.rs:100,142` | 765 | V | stage, verify and activate | facade | 5 + `exp013_bundle.rs` 13, `served_identity.rs` 2 | — | RETAIN (ADR-142 §4) |
| LM-08 | `crates/loom-domain/src/okf.rs:143` `vault_generated` (`process:vault`) | 311 | A | provenance default naming the producer | `loom-scaffold/src/lib.rs:504` | 5 | `process:ontomark/<ver>` from the bundle manifest | REPIN |
| LM-09 | `crates/loom-vector-ruvector/src/artifact.rs:149-184` `promote_vault_build` (error text `vault build --with-rvdb` `:184`) | 472 | CLI (consumer) | promotes a `vault build` rvdb | `bin/promote_vault_build.rs` | 6 + `tests/promote_vault_build.rs` 2 | `ontomark build` output | REPIN (rename text) |

## 7. Agentbox (`agentbox/`)

### 7.1 Pins

| Id | path:line | Value | Tests | Replaced by | Disp |
|---|---|---|---|---|---|
| AB-P1 | `flake.nix:41-42` `vaultSrc` | `github:DreamLab-AI/VisionClaw/3213e314f82491f338660c79b77332da6610ed9c` (**differs from publish.yml's `1044b0722`**) | none (no flake `checks`) | `ontomarkSrc` input at the released tag | REPIN |
| AB-P2 | `flake.lock:263-277` | rev `3213e314…`, narHash `sha256-FP99FAX3…` | none | relocked | REPIN |
| AB-P3 | `lib/vault.nix:75` lockfile, `:77` whelk outputHash, `:80` `buildAndTestSubdir = "crates/vault"`, `:85` `doCheck = true`; licence note `:58` | — | the derivation runs the vault crate suite (`doCheck = true`) | `lib/ontomark.nix` building `ontomark-cli` from its own lock; keep `doCheck` | REPIN |
| AB-P4 | `flake.nix:924-930` (`vaultCliActive`), `:1944-1945`, `:2002-2005` symlink `/opt/agentbox/bin/vault` | — | none | bake `ontomark`, keep a `vault` symlink for the transition window (ADR-2136 §1) | REPIN |
| AB-P5 | `config/entrypoint-unified.sh:1130-1169` Phase 5d(ii) `vault --version` gate | — | boot gate only | probe `ontomark --version` (and the adapter, per D4) | REPIN |

No vowl-wasm pin in agentbox. No flake `checks` attribute for vault. `package.json:10` `test:vault` globs `mcp/servers/lib/__tests__/*.test.js`,
a directory that does not exist (DELETE or fix).

### 7.2 Callers of the binary (keep working through the transition)

`management-api/lib/elevation-stage.js:200` (`validate --strict --json`); `management-api/lib/ontology-apply.js:221` (`find`), `:419-420` (`edit`),
`:478` (`create`), `:668` (`apply`); `management-api/lib/ontology-propose.js:239-276` (`propose --dry-run --json`);
`skills/podcast-knowledge-ingest/submit-proposals.mjs:162` (`propose`); `skills/ontology-augment/scripts/ontology-augment.sh:354-498`
(`retrieve`, `find`, `validate`, `propose`). Tests: `management-api/tests/{ontology-apply,ontology-propose-vault,elevation-stage}.test.js`,
`tests/sovereign/ontology-propose*.test.js`, `tests/skills/ontology-augment-grounding.test.sh`. The JSON output and exit codes of these verbs
are the compatibility contract (ADR-2136 verification).

### 7.3 Duplicate semantics

| Id | path:line | Lang | Resp | What | Tests | Replaced by | Disp |
|---|---|---|---|---|---|---|---|
| AB-01 | `management-api/lib/ontology-apply.js:320` frontmatter regex, `:176` `slugify` (reused `ontology-propose.js:173`, `elevation-stage.js:136`) | JS | P I | parse and slug | `management-api/tests/ontology-apply.test.js` 67, `ontology-propose-vault.test.js` 14, `elevation-stage.test.js` 14 | `ontomark` CLI JSON (`retrieve`, `slug`) | DELETE |
| AB-02 | `management-api/lib/ontology-propose.js:64,:173-177`; `skills/podcast-knowledge-ingest/submit-proposals.mjs:100` | JS | I | `urn:ngm:class:` minting | `ontology-propose-vault.test.js` 14; `tests/sovereign/ontology-propose.test.js` | CLI-reported IRI | DELETE |
| AB-03 | `management-api/lib/elevation-stage.js:136-174` page writer, `:80-94` relation resolution | JS | W | page writer | `elevation-stage.test.js` 14 | `ontomark create` (guarded) | DELETE |
| AB-04 | `management-api/lib/kg-proposal-extractor.js:156-181` | JS | P | is-subclass-of extraction | `tests/sovereign/kg-proposal-extractor.test.js` 18 | CLI JSON | DELETE |
| AB-05 | `mcp/servers/lib/ontology-index-build.js:49-56,126-127`; `mcp/servers/lib/ontology-local.js:104-327` (self-described interim, `:4-20`) | JS | P V | JSON-LD class index and dangling-link check | none (`package.json:10` `test:vault` globs a missing dir); `tests/config/vault-consumer-fallback.test.mjs` touches the loader | `ontomark` CLI (`find`, `validate`) | DELETE |
| AB-06 | `services/podcast-ingest/src/common/yaml_scalar.rs:33`; `src/promote/working_page.rs:56` | Rust | W | frontmatter scalar quoting and page writer | `yaml_scalar.rs` 7, `working_page.rs` 4 | `ontomark-parse` crate dependency | REPIN |

`management-api/lib/bc20-provenance-bridge.js:137` mints `urn:visionclaw:concept:` (VisionClaw's own identifier scheme, not ontology identity): RETAIN.
`ontology/{decision-layer,code-harness}.{ttl,ofn}` are a separate agentbox ontology: RETAIN.

## 8. Decisions needed from the queen

| # | Decision | Recommendation | Blocks |
|---|---|---|---|
| D1 | Relicensing of extracted `vault`, `vault-core` and `pipeline/*.py` code | **Resolved**: owner decision, MIT OR Apache-2.0; `nostr.rs` and governance posting stay AGPL (§1) | NOTICE and ADR-2014 entry in Phase 2 |
| D2 | Two category tables, one truth (`graph_tiers.rs:87` `CATEGORY_ORDER`, `consistency.rs:62` `CATEGORY_SLUGS`): both hold the same 53 slugs. Only `CATEGORY_ORDER`'s order is semantic (position = NGG1 category id); `CATEGORY_SLUGS` is used for membership (a `BTreeSet` at `turtle.rs:243-247`). Set equality is now asserted at `graph_tiers.rs:1303-1304` (`de33ca538`, tag `ontomark-census-baseline-1`). This is the drift risk the single-owner rule removes: a census pass and the queen both miscounted `CATEGORY_ORDER` as 48 with a single-line regex, because rustfmt wraps five tuples | **Resolved by design**: `profiles/estate` holds **one** ordered table, and membership is derived from it. Golden `python/graph/overview.json` encodes the legacy 34 and `golden_parity.rs:508-509,545-556` truncates to 34 while asserting 53, so the space categories are covered by those assertions and the full-corpus tier diff | step 3 |
| D3 | Repos outside the original brief. **knowledgeGraph** is in scope (queen): it is the published corpus and deploy target, and its explorer is a copy of visionGraph `publishing-tools/WasmVOWL/modern` (fourth NGG1 codec at `explorer/modern/src/lib/ngg1.ts`, `explorer/FORMAT-NGG1.md`, `scope.ts`, vowl-wasm `v0.1.1` tarball at `explorer/modern/package.json:20`). **WasmVOWL** (standalone, last commit 2026-09-07) pins the same tarball at `modern/package.json:19` | knowledgeGraph: fix the source in visionGraph, then redeploy so the copy is replaced and `FORMAT-NGG1.md` points at the ontomark spec. WasmVOWL: **decided (queen)**: archive the repo once knowledgeGraph is repinned; it has no consumers of its own | step 5 |
| D4 | The `vault` binary is AGPL only because of `nostr.rs`. `ontomark-cli` is a drop-in for build, validate, gate, conflicts and repair. The governance verbs (propose, apply, panel publish) post to the forum | **Decided (queen)**: VisionClaw keeps a thin AGPL `vault` adapter, built on the ontomark crates, that owns only propose, apply and panel publish. agentbox bakes both the ontomark CLI and that adapter | step 9 |
| D5 | VisionClaw server reasoners: VC-47 (`custom_reasoner.rs`) and VC-49 (`inferred_edge_materialiser.rs`) feed physics and the inferred-edge view; VC-48 (`ontology_reasoner.rs`) is a second closure store | **Decided (queen)**: both reasoning paths route through ontomark-reason. Once EXP-01 passes natively, delete the closure-only path (VC-47, VC-48 with VC-41), because it is exactly the substitute EXP-01 forbids. VC-49 becomes a projection over ontomark-reason's inferred set, and physics constraints stay an explicit projection (ADR-2137 §3) | EXP-01 native pass |
| D6 | Explorer TypeScript NGG1 codec (VG-01) and display markdown (VG-08) | **Decided (queen)**: delete the TS codec. The explorer consumes vowl-wasm's re-export of ontomark-compile's NGG1 (one owner, EXP-09). The unused `__fixtures__/ngg1-3n2e.bin` golden becomes a test that is actually loaded. TS keeps typed views over generated constants; VG-08 stays display-only until ontomark-wasm ships a body view | step 5 |
| D7 | visionGraph Python hygiene scripts (VG-21 to VG-27, VG-30) and `fmlib.py` | **Decided (queen)**: port to tested `ontomark` subcommands: `ontomark lint` (collision lint), `ontomark migrate` (assign-homes, merge-plan, apply-decisions), `ontomark attribute`, `ontomark assets` (asset audit). Then delete the scripts; the CLAUDE.md gate calls the CLI. `weekly_ingest.py` keeps PyYAML I/O but writes through `ontomark create` | WP-C CLI scope, step 7 |
| D8 | vowl-wasm `markdown-ontology` feature (VW-02, VW-04): own parser, `Domain` enum, OWL 2 validator | **Decided (queen)**: delete. vowl-wasm becomes rendering plus the NGG1 re-export only; the published JS bundle API is unaffected (the feature was never in it) | step 5 |
| D9 | Loom artefact readers (LM-04), IRI minting in three places (LM-07..09), duplicate generation-marker check (Rust `mirror.rs`, Python `reload-published-generation.py`) | **Decided (queen)**: Loom consumes the versioned bundle contract (ADR-142) and takes IRI minting from ontomark-model's rules through one function. The marker check collapses to the Rust one; the Python copy is deleted | step 10 |

## 9. Removal plan (Phase 2)

Each step runs only after `spec-compile` signals parity for that responsibility. Every step lands on a branch `ontomark/<responsibility>` in each
touched repo, never on main, and is reported to team-lead with the diff summary and executed test evidence. The queen pushes.

| Step | Responsibility (branch) | Repos and rows | Gate (all executed and green) |
|---|---|---|---|
| 1 | `ontomark/parse` | VisionClaw VC-01, 02, 07, 08, 13, 40, 41, 42, 57, 58; visionGraph VG-P4, VG-20; agentbox AB-01, 04, 06. **Ordering:** 17 files in `crates/vault/src` import `vault_core::page` and the other parse modules, so step 1 repins `crates/vault` onto ontomark-parse in the same change (the crate itself is deleted in step 10) | EXP-03; vault-core inline and `real_vocabulary`; `vault_fixtures.rs`, `tests/vault_gate_test.rs`; `cargo test --workspace` in VisionClaw; space-corpus tests; agentbox management-api tests |
| 2 | `ontomark/iri` | VC-04, 54, 55; LM-01, 02, 03; AB-02; VG-06, VG-24 identity rewrite removed | EXP-02 on golden and full public build (IRI multiset diff = ∅); Loom `model_tests`, scaffold `tests.rs` |
| 3 | `ontomark/taxonomy` | VC-05, 06 (tables), 18 (tables), 56; VW-03; VG-03, 04, 05; colour tables repinned to generated slugs | EXP-05; golden tiers byte-identical (one ordered 53-slug registry; position = category id); `check-space-domains.mjs`; `domain_palette_parity.rs`; `spaceDomains.test.ts` |
| 4 | `ontomark/reason` | VC-06 (checks), 15, 16, 46, 47, 48, 49 (D5); VG-26, 27 | EXP-01; `el_profile`, `defined_classes`, `probe_consistency`, `whelk_scaling`; `tests/inference/*`; `semantic_physics_integration_test`, `pipeline_test` |
| 5 | `ontomark/ngg1` | VC-17; VW-01 (re-export, API frozen); VG-01, 02 (D6); knowledgeGraph redeployed from the fixed visionGraph source (D3) | golden `graph/*.bin` byte-identical; vowl-wasm 214 tests plus wasm-pack bundle check; TS reads the shared fixture; explorer `vitest run` |
| 6 | `ontomark/rdf` | VC-19, 20, 22, 28, 50, 52, 53; VG-09; LM-08 | EXP-02, 04, 09; `golden_parity.rs` 15; Loom `exp013_bundle`, `served_identity`, graph integration |
| 7 | `ontomark/validate` | VC-09, 10, 12, 25, 26, 27, 51; VG-21, 22, 23 (D7); AB-05 | EXP-05; `publish_gate.rs`; `vault validate/conflicts/gate --json` identical on the full corpus at one visionGraph sha |
| 8 | `ontomark/attribution` | VC-21, VC-18 (constants), VC-44 (generator); VG-28 | per-class `prov:wasAttributedTo` identical (EXP-02/04) |
| 9 | `ontomark/write` | VC-29, 31, 43, 44, 45; VC-30 and VC-36 re-wired (AGPL stays); AB-03 | EXP-03, EXP-08; `apply_cli`, `create_cli`, `propose_cli`; `vault/mod.rs:793-897` round trips; elevation actor tests |
| 10 | `ontomark/cli-publish` | VG-P1, P2, P5, P6; VC-23, 24, 32, 60; AB-P1 to P5; LM-09; D4 adapter | byte-identical `site-data/` from `vault build --vault all` and `ontomark build`; publish.yml green on a branch run against a **released tag**; agentbox `nix build` with `doCheck`; agentbox caller tests |
| 11 | `ontomark/reload-generation` (Loom, on top of `ontomark/cli-publish`) | LM-06: Rust `reload_published_generation` in loom-facade replaces `scripts/reload-published-generation.py` (bundle hash, receipt, served-identity proof, compose restart); the systemd unit runs the binary; Python deleted only once the Rust test passes. Owner: WP-D (queen) | fixture-bundle integration test covering the 16 Python behaviours; LM-06 migration test |
| 12 | `ontomark/docs` | §10 docs and diagrams in every repo | VisionFlow generator `--check --strict-citations` green; topics restamped |

Final deletion of `crates/vault-core` and of everything in `crates/vault` except the governance adapter happens in step 10. Removing the crates
before then would break the agentbox pin and publish.yml.


### 9.1 Corrections found while writing the step tests (Phase 2 prep)

- **VC-29 vs D4:** `crates/vault/src/apply.rs` survives in the D4 adapter, thinned to a wrapper over ontomark-parse; it is not deleted in step 9. The adapter itself lands in step 10, not step 9 as D4's gate column said.
- **VC-11 and VC-14** (`vault-core/proposal.rs`, `vault/model.rs`) were in no step: both go in step 10 (crate deletion and the adapter shape check).
- **VC-52** names 6 of the 9 files under `jsonld_validator/`; `iri.rs`, `errors.rs` and `context-v1.jsonld` also go in step 6.
- **VC-51:** a second `crates/visionclaw-ontology/src/ontology/services/owl_validator.rs` exists beside the cited one; both go.
- **VG-05** lives at `publishing-tools/WasmVOWL/modern/scripts/check-space-domains.mjs` (called from `publish.yml:297`).
- **VG-20 ordering:** `fmlib.py` is imported by eight hygiene scripts, so it is deleted in step 7 with them (D7), not in step 1.
- **Pin convention (estate-wide):** every consumer pins the CLI as `ONTOMARK_RELEASE_TAG: vX.Y.Z` plus `ONTOMARK_SHA256` and verifies with `sha256sum --check`. The visionGraph step tests that need "consumer on ontomark" read that pin, so they go green only after `ontomark/publish-cli` lands.
- **LM-02:** four `urn:ngm:class:` minting sites, not two or three: `loom-domain/src/model.rs:33`, `loom-vector-ruvector/src/bin/stage_corpus.rs:101`, `src/artifact.rs:366`, and `src/hnsw.rs:446` (a test helper).
- **LM-06 widens scope:** `loom/scripts/reload-published-generation.py` is the whole reload timer (bundle hashing, receipt, served-identity proof, compose restart), not only a marker check. Deleting it needs a Rust replacement binary (proposed: `reload_published_generation` in loom-facade) driven by `deploy/loom-generation-reload.service`. Its runbook also claimed a mismatched `content_digest` is refused; the script discards it (`del declared_digest`).
- **VW-04/VW-07:** `owl2_validator` is declared without a feature gate (`src/ontology/mod.rs:13`), and `loader.rs` (marked RETAIN) is built on `markdown_parser` and `Domain`, so D8 deletes both. Deleting `Domain` (step 3) breaks the parser and loader at compile time unless step 5's D8 deletion lands first or with it.
- **vowl-wasm NGG1 crate (decided):** NGG1 gets its own dependency-light crate `ontomark-ngg1` (spec-compile, before rc.1); vowl-wasm depends on it directly, and the step-5 test accepts only that. `ontomark-compile` depends unconditionally on `serde_yaml` and `regex`, which the 232,052-byte bundle cannot carry.
- **vowl-wasm test count:** 173 lib tests with all features plus the integration suites, not 214.
- **AB-05 split:** `ontology-local.js`'s json-ld parsing goes in step 1 and its validator in step 7.
- **agentbox CI gap:** CI runs none of `management-api/tests/*.test.js` (jest roots are `tests/contract|sovereign|integration`); the step tests live there beside the ontology-apply tests and must be wired into `test:node` when each step lands.
- **Unassigned rows:** VC-03 goes in step 1, VC-11 and VC-14 in step 10, and VW-02 and VW-04 in step 5 (D8).
- **§10.1 additions:** knowledgeGraph `README.md` and `docs/architecture/explorer.md`; VisionFlow ADR-2013 §3 and `BASELINE-visionflow.md` invariants 8 and 9, which VF-01, KG-01, KG-04 and KG-05 quote.
- **§10.2 counts:** the generator parses 139 topic files. The migration plan (`VisionFlow/docs/engineering/ontomark-diagram-migration.md`, branch `ontomark/docs`) covers 55 topics: 48 change, 7 reviewed with no change. VC-01, VC-26, ES-01, VF-03 and VF-07 make no vault claim. AB-01's `vaultSrc` block is at `flake.nix:40-43`.
- **Citations wrong today, despite a green `--cite-check`:** VC-24 (`elevation_actor.rs` gate lines), VW-01, VW-03 and VW-04 line numbers, and AB-02 (`entrypoint-unified.sh:1159`, actually `:1222`). They are fixed at restamp; a green strict run does not prove the prose.
- **Ordering:** visionGraph `ontomark/publish-cli` (the queen's Phase 2 step 1) is this plan's step 10 for visionGraph only, and lands first. VisionClaw crate deletion and the agentbox flake stay on their step-10 branches.

## 10. Documentation and diagrams

### 10.1 Documents and what each must say after

| Doc | Says now | Must say after |
|---|---|---|
| visionGraph `CLAUDE.md:5-7,19-24,41-48` | spec in VisionClaw `VAULT-corpus-format.md`; gate runs `vault validate/repair/conflicts/gate` and `collision_lint.py`; "53 categories" | gate commands as `ontomark …` (pinned release); collision lint via `ontomark lint collisions` (D7); category count read from the estate profile version |
| visionGraph `README.md:31-35,62-66,125-131` | `vault` from VisionClaw is the only producer; "no Python ontology pipeline" (contradicted by 11 scripts) | released `ontomark` CLI by tag; maintenance scripts named or removed |
| visionGraph `docs/adr/ADR-VG-007` | proposed, inactive | implementation and activation status with evidence links; publish.yml consumes `ontomark` tag |
| visionGraph `docs/adr/ADR-VG-004:41` | cites knowledgeGraph scope.ts and vowl-wasm 0.1.1 | current visionGraph paths and pins, or a historical-note marker |
| visionGraph `planning/ontology-workspace/README.md:57,61` | TS NGG1 at `:144,375,483`; says publish.yml pins `2a2156db` (**wrong: it pins `1044b0722`**) | corrected anchors (`:139,:386,:488`); divergence stated as `1044b0722` vs agentbox `3213e314`; then superseded by this inventory |
| visionGraph `publishing-tools/WasmVOWL/FORMAT-NGG1.md:4,141,244` | writer `pipeline/emit_graph_tiers.py`; 34 categories; fixtures under `pipeline/tests` | single spec owned by ontomark (`ontomark-compile` docs); this file points there |
| visionGraph `publishing-tools/WasmVOWL/README.md:92-102,183-198`; `quartz/README.md:70-106` | `vault build` producer; vowl-wasm 0.1.2 | `ontomark build`; vowl-wasm version that re-exports ontomark NGG1 |
| VisionClaw `docs/VAULT-corpus-format.md` | normative spec v2.2.0; ADR-2124 to 2128 listed as proposed (they are accepted) | the estate profile in ontomark is the machine contract; this doc keeps the corpus rules and links the profile version; status drift fixed |
| VisionClaw `docs/adr/ADR-2113` | `vault-core`/`vault` are the single parser, reasoner and build; lists retired `migrate` | superseded by ADR-2137 (implemented) |
| VisionClaw `docs/adr/ADR-2096` | `visionclaw_domain::vault::parse` is the single entry point | superseded; entry point is ontomark-parse |
| VisionClaw `docs/adr/ADR-2137` | proposed, none, inactive | implementation evidence per step; `verified_commit` set |
| VisionClaw `crates/vault/README.md`, `crates/vault-core/README.md` | the CLI and library (README omits `apply`, `panel publish`, `consistency.rs`, `definition.rs`) | vault-core README deleted with the crate; vault README describes the AGPL governance adapter only (D4) |
| VisionClaw `README.md` (mermaid shows JSON-LD fences, GitHub sync), `AGENTS.md` (`neo4rs`; `propose` tool; omits vault-core), `docs/BASELINE-architecture.md:87` (`vault-migrate`), `docs/IDENTIFIER-taxonomy.md`, `docs/explanation/ontology-pipeline.md`, `docs/how-to/operations/pipeline-operator-runbook.md:34` (CustomReasoner stage), `pipeline-admin-api.md:358`, `docs/tutorials/promote-note-to-ontology.md:31` | stale shapes | ontomark crates as the semantic layer; CustomReasoner removed; Oxigraph as query projection |
| VisionClaw `docs/diagrams/README.md` | pointer to VisionFlow corpus | unchanged |
| vowl-wasm `README.md:9-229`, `CHANGELOG.md`, `docs/adr/ADR-001` | standalone NGG1 reader; ADR proposed | NGG1 is a re-export of `ontomark-compile`; JS API unchanged; ADR-001 implemented, owner of NGG1 = ontomark |
| Loom `docs/design/ADR-141`, `ADR-142`, `ONTOLOGY-LOOM-PIPELINE.md:7,106-107,122,186`, `README.md:14,50,121,205,365`, `docs/README.md:81-87`, `docs/published-generation-reload.md:90-114` | `vault build` is the producer; pipeline doc still says `urn:ngm:generation:<sha>` and describes `pipeline/manifest.py` (**drift: code uses `visionGraph@<sha>`**) | producer `ontomark build`; contract version from the ontomark bundle manifest; identity drift fixed |
| Agentbox `AGENTS.md:42,57` | `[vault]` corpus authority; URN scheme `urn:agentbox:` (no ontology or Loom section) | `ontomark` baked tool plus the governance adapter; `urn:ngm:` identity named |
| Agentbox `docs/adr/ADR-2136` | proposed; cites `lib/vault.nix:74` (actual `:75`), inspected at `b9991c37` | implemented with Nix evidence; anchors fixed |
| Agentbox `docs/GOVERNANCE-capabilities.md:202-279`, `docs/BASELINE-container.md:127-171,456-458` | vault toolchain; ADR-2136 unimplemented | ontomark toolchain and status |
| Agentbox skills: `skills/ontology-{augment,core,enrich}/SKILL.md`, `agents/ontology-curator.md:38-84`, `skills/podcast-knowledge-ingest/{SKILL.md,references/*}`, `skills/podcast-bulk-ingest/SKILL.md` | instruct `vault` subcommands | instruct `ontomark` for read, validate and build verbs; the governance adapter for propose and apply (D4) |
| VisionFlow `docs/adr/ADR-2014/2015/2016` | proposed | status and evidence |
| VisionFlow `docs/{PRD-sovereign-corpus.md,engineering/sovereign-corpus-contracts.md,estate-closeout/2026-09-06/pin-ledger.md,terminology.md}` | vault as canon producer; pin ledger | ontomark; the pin ledger gains the ontomark release row |

### 10.2 VisionFlow diagram corpus (`VisionFlow/docs/diagrams`, 143 topics + README, COVERAGE, VERIFICATION)

These topics describe the shape being replaced and must be re-read, redrawn and restamped (generator `scripts/diagram-index-gen.cjs`,
`--check --strict-citations`) in step 11. Topic ids with their current claim:

| Area | Topics | Current claim to change |
|---|---|---|
| visionclaw | VC-11, VC-16 (`vault-core/src/domains.rs` as domain source); VC-20 (Whelk adapter, `visionclaw_domain::vault::parse` `:58`); VC-21 (domain vault parser); VC-23 (`urn:ngm:class` mint in `visionclaw-domain/src/uri.rs`); VC-24, VC-05, VC-07 (Whelk gate and adapter); VC-25 (vault-core Vocabulary); VC-01, VC-26 (vault CLI, ontology-publish) | ontomark crates as parse, model, reason and IRI owners; adapters only |
| visiongraph | VG-01 (`vault.toml` read by crates/vault), VG-03 (publish.yml builds vault; vowl-wasm pinned), VG-04 (pull model), VG-06 (explorer duplication; NGG1 domain id) | released `ontomark` tag; single NGG1 owner |
| knowledgegraph | KG-01, KG-04, KG-05, KG-06, KG-07 (NGG1 reader, vowl-wasm 0.1.1, vault build) | deploy target of the visionGraph explorer; NGG1 owned by ontomark (D3) |
| vowl-wasm | VW-01 to VW-05 (`src/ngg1.rs` reader, OWL parse, bundle features) | NGG1 re-exported from ontomark; parser per D8 |
| agentbox | AB-01 (vault pin, **anchor drift: cites `flake.nix:26-29`, actual `:41-42`**), AB-02 (5d(ii) gate), AB-03, AB-04, AB-14, AB-17, AB-22, AB-24, AB-25, AB-28 | ontomark package and the governance adapter |
| estate | ES-01, ES-05, ES-06, ES-09 (`cargo build -p vault`), ES-11 (vault sole producer; two VOWL bundle pins), ES-90 | ontomark as producer; one vowl-wasm pin |
| visionflow | VF-01 (D3 "one parser/reasoner/build" = crates/vault), VF-03, VF-07, VF-08 (`crates/vault/src/main.rs`) | ontomark as the canon's semantic owner; compatibility matrix gains ontomark |

`COVERAGE.md:2804-2805` maps `crates/vault-core/src/domains.rs` and `crates/vault/src/main.rs`. It is generated, so it regenerates in step 11.

## 11. Evidence

Citation spot check, run 2026-10-11 from `/home/devuser/workspace` (shell function `chk file line pattern` prints OK when line `n` of
`file` contains `pattern`). Output: 48 OK, 0 BAD. Anchors checked: `vault/src/{model.rs:36,build/ngg1.rs:150,whelk.rs:505,closure.rs:87,
build/attribution.rs:24,build/turtle.rs:209,validate.rs:345,conflicts.rs:312,gate.rs:122}`; `vault-core/src/{page.rs:557,slug.rs:29,domains.rs:4}`;
`visionclaw-domain/src/vault/mod.rs:278,308`; `custom_reasoner.rs:60`; `ontology_reasoner.rs:72`; `whelk_inference_engine.rs:319`;
`src/uri/mod.rs:200`; `visionclaw-domain/src/uri.rs:22`; `canonical_iri.rs:59`; `decision_elevation.rs:139`; `elevation_actor.rs:481`;
`github_sync_service.rs:2409`; `domainColors.ts:4`; `owl_validator.rs:357`; vowl-wasm `ngg1.rs:128`, `markdown_parser.rs:56`, `ontology/mod.rs:216`;
Loom `model.rs:32`, `stage_corpus.rs:101`, `hnsw.rs:446`, `loom-graph-oxigraph/src/lib.rs:38`, `reload-published-generation.py:40`; visionGraph
`ngg1.ts:139,386,488`, `scope.ts:75`, `collision_lint.py:37`, `assign_homes.py:68`, `fmlib.py:6`, `pageMeta.ts:27`; agentbox
`ontology-apply.js:176,320`, `ontology-propose.js:64`, `elevation-stage.js:136`, `ontology-local.js:4`, `yaml_scalar.rs:33`; VisionClaw
`pack-pod-resources.py:31`.

Category tables, executed: a Python regex over both declarations (multi-line tuples included) gives 53 and 53, equal as sets, with positions
6-33 ordered differently. A first single-line `awk` count gave 48 because it missed wrapped tuples (see D2). Rule for every later count: parse the declaration, never count lines. The fixture page
`3D Scene Exchange Protocol (SXP).md` is the only golden page naming one of the five long-slug categories (`sc-standards-and-interop`).

Limits: responsibility labels for some `crates/vault/src/build/*` modules came from symbol names, not full reads. Test counts are attribute
counts, not executed passes. Consumer lists for Loom LM-02 and VisionClaw VC-49 are incomplete ("not traced"). This is a same-family census;
the Codex evidence audit applies to the Phase 2 removals, not to this inventory.

## 12. Release continuation: 2026-10-11

The recovered core is implemented and tested. Its native legacy adapter delegates to `ontomark-reason`, with zero differences against the complete committed corpus over 32 artefact lines and 12 command outputs. The opt-in typed projection is published in `ontomark-compile` 0.1.3; its initial 0.1.2 release is yanked after correcting Unicode private-name filtering. It does not apply a corpus-wide vocabulary/schema migration to legacy CLI callers.

- **visionGraph producer**: runtime 9cf6cb8 consumes the checksum-pinned `v0.1.0-rc.1` Linux CLI. All 23 publishing contracts, explorer build and browser smoke checks pass; publisher run 38179004619 and downstream Pages run 38180896856 succeeded. Public graph/About routes return HTTP 200.
- **NGG1**: the reusable codec is `ontomark-ngg1` 0.1.0. Published `vowl-wasm` 0.2.0 exposes the browser compatibility API and preserves physics/layout ownership. visionGraph deletes its independent JavaScript encoder/decoder and uses the immutable GitHub browser bundle because npm trusted-publisher authentication remains unresolved.
- **Graph view**: [ADR-OM-002](adr/ADR-OM-002-shared-graph-and-obsidian-boundary.md) places framework-neutral rendering, traversal and interaction in Ontomark `packages/graph`. Narrative Goldmine’s old node/edge/label renderers are removed; its store, palette, navigation and worker layout remain host-owned. The Obsidian plugin consumes the same released package.
- **Obsidian distribution**: [standalone public repo](https://github.com/DreamLab-AI/ontomark-obsidian), released offline Linux ZIP and real Obsidian 1.14.4 acceptance; mobile remains unsupported.
- **Loom contracts**: main eef1bec consumes shared typed C3 readers and registry compiler 0.1.1. Real contracts, reload/receipt races and linked-worktree build identity pass. The runtime ingestion/first-served activation gate remains partial/inactive.
- **Attribution**: corrected crate versions and complete frozen WASM/native CLI notices are published with source links; see [UPSTREAMS](UPSTREAMS.md). The live publisher also serves the full VOWL, shared renderer and 99-package JavaScript runtime notices, with the exact inventory digest verified. Historical release assets and licence texts are preserved.

This batch does not close the original removal inventory or declare its 876 baseline residues eliminated. The 690 SKOS-only class declaration rule has synthetic typed-model coverage; applying it to the normative corpus needs a distinct reversible migration and consumer evidence. Other VisionClaw, knowledgeGraph, taxonomy and Agentbox producer replacements retain their recorded acceptance gates.

## 13. Alignment: 2026-10-12

Owner decisions recorded in [ADR-OM-005](adr/ADR-OM-005-estate-alignment-one-producer-explicit-projections.md); this section carries the inventory consequences.

- **VisionClaw is a consumer.** Its force superset is an explicit projection over the shared bundle (ADR-2137 §3). The gap check (ADR-OM-005 §Evidence) found the bundle carries every corpus fact the projection reads; what it lacks is VisionClaw-owned (spring weights, class charges, population scales, seeds) or derived presentation. Two duplicate derivations collapse onto the bundle: the inferred closure (VisionClaw's own Whelk run yields a different set; route through `ontomark-reason`, D5) and the domain registry (`vault-core/src/domains.rs` vs `profile.taxonomy.domain_aliases`).
- **VC-60: REPIN → RETIRE (gated).** Gates: (i) a pod-resource producer outside VisionClaw, output byte-equal to `pack-pod-resources.py` for a dated window; (ii) `src/services/ontology_pull.rs:42` repointed; (iii) VisionClaw ADR-2106 re-verified or amended (its `verified_paths` list the workflow, the pack script and the pull module); (iv) the input set below. The job runs until all four pass. New row: **AB-P6** — Agentbox produces the pod-resource pack from the released `ontomark` CLI (`ontomark build --scope machine`; `--vault all` is the public publish's value) and serves it to VisionClaw.
- **OM-00 register reconciled at `8d654d1` (same day).** VisionFlow `docs/diagrams/ontomark/00` found 14 inherited faults in the relocated pipeline; all RECONCILE items are fixed in ontomark. VC-60's `vault build` still ships S1.1, S1.2, S1.5, S2.3 and S3.1 until retirement, which is one more reason the window should be short. The `estate-small` parity receipt now reads: every artefact byte-identical except the two generation markers, compared structurally.
- **Input set.** VisionClaw's live graph ingests `working/pages` (kept, owner decision); the published machine artefacts are `knowledge/` only. Step 10's gate gains: `ontomark build --vault all --private-machine-bundle` (name provisional) emits unredacted TTL/NGG1/`api/*` including `working/`, never published to the site; VisionClaw's node set over that bundle equals its current in-tree parse (plain nodes, no classes from `working/`).
- **Generation identity.** `visionGraph@<sha>` stays the identity; distribution (Agentbox-served or release-tagged, sha256-pinned) is separate. Identifiers need not be stable over time at this stage; a re-mint ships ADR-2137 §4's mapping, diff and rollback artefacts. Loom ADR-141 amended (proposed).
- **Licence facts.** `crates/vault` + `vault-core`: 24 commits, 29,383 lines, 100% owner agent identities; only port marker is own `pipeline/gate.py`. Extraction is a port under the existing grant (§1). `nostr-bbs-core` AGPL-3.0-only (owner jjohare) stays the AGPL transport. `horned-owl` LGPL-3.0: opt-in `whelk-el` feature, off by default; notice added to UPSTREAMS. Rule: dependencies flow permissive → AGPL, never the reverse.
- **Agentbox ADR-2108** ("vault baked by Nix") is the record ADR-2136 overturns; mapped in ADR-2136 today, superseded on acceptance.
- **Snapshots.** The estate ADR copies under `docs/adr/` are replaced by `docs/adr/ESTATE-ADRS.md` (pointer index). §10.1's "ADR copies" rows now mean the sources.
- **Not decided** (left open, see ADR-OM-005): proposal-not-Apply for estate vaults in the plugin; per-edge property IRI in NGG1; dual-licensing `nostr-bbs-core`.
