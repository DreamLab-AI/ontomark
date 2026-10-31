---
id: ADR-OM-005
title: "one producer, explicit consumer projections, and the generation loop"
date: 2026-10-12
decision_status: accepted
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit: "d9084dc8c5049c0fb18dcbcc6513f3af8cdbd670"
verified_paths: []
owner: jjohare
review_trigger: "a DEPRECATION.md step changes a producer, a consumer projection or the generation identity; VisionClaw ADR-2137 or Agentbox ADR-2136 is accepted or rejected"
repo: ontomark
---

# ADR-OM-005: one producer, explicit consumer projections, and the generation loop

- Status: Accepted (owner decisions of 12 October 2026, recorded here; implementation none; activation inactive)
- Scope: the estate-wide shape that the Ontomark workspace serves. It records decisions, evidence and the migration consequences. It changes no code, workflow or Nix pin; each of those remains a `DEPRECATION.md` step with its own gate.
- Related: ADR-OM-001 (#2, #4 licence), ADR-OM-003 (ingest), ADR-OM-004 (EL profile); VisionFlow ADR-2014 (accepted) and its 2026-10-12 alignment section; VisionClaw ADR-2137 (proposed), ADR-2106 (accepted, live); Agentbox ADR-2136 (proposed), ADR-2108 (accepted); Loom ADR-141 (proposed), ADR-142 (proposed); visionGraph ADR-VG-007.

Verification 2026-10-31: `verified_commit` is this record's own last-modified commit, recorded when the record was moved onto the estate ledger format. It is not a re-verification of the code the record describes, and `verified_paths` is empty, so the staleness gate is not yet armed for this record.

## Decisions (owner, 2026-10-12)

1. **VisionClaw is a consumer of the shared build, not a producer.** The source corpus and its semantics are shared. VisionClaw's GPU "force superset" (constraint, spring, charge and population projections) is its own explicit projection over the shared bundle, as ADR-2137 §3 already requires ("preserve downstream physics constraints as an explicit projection"). VisionClaw does not re-parse the corpus to build the ontology.
2. **The VisionClaw publish build job (DEPRECATION VC-60) is retired, not repinned.** VisionClaw reaches Agentbox directly in this estate, so the pod-resource pack is produced from the shared build on the Agentbox side and consumed by VisionClaw. This overturns the previous REPIN disposition and is **gated**: (i) a pod-resource producer outside VisionClaw exists and its output is byte-equal to the current pack for a dated window; (ii) `src/services/ontology_pull.rs:42` is repointed; (iii) VisionClaw ADR-2106 is re-verified or amended (its `verified_paths` include `ontology-publish.yml`, `pack-pod-resources.py` and `ontology_pull.rs`, so any such edit arms its staleness check); (iv) the input set is settled by decision 4. Until all four pass, the job keeps running.
3. **Generation identity is separable from distribution.** The identity stays `visionGraph@<sha>` (Loom ADR-141). A distribution channel (an Agentbox-served or release-tagged bundle carrying that identity and a sha256) is permitted. The owner does not require identifiers to be stable over time at this stage: identities across the synthetic corpus may be re-minted to best practice without compatibility shims, provided each re-mint ships the identity mapping, visibility diff and rollback artefact ADR-2137 §4 requires. Loom ADR-141 is amended accordingly (proposed on the Loom side).
4. **`working/` stays in the GPU view.** The live VisionClaw graph ingests `knowledge/pages` and `working/pages` by default; every machine artefact in the published bundle is built from `knowledge/` only (`--vault all` widens only the Quartz staging). To keep `working/` without a second parser, `ontomark build` gains a **private machine-bundle mode** (its own flag — `--vault all` is already the public publish's value — including `working/`'s public pages in the machine artefacts as plain records, private-link redaction applied as to any bundle, never published to the site) produced by the same binary, and that is what VisionClaw consumes. (Wording corrected 2026-10-12 by the OM-00 register; the first draft said "`--vault all`, unredacted".) `working/` is the source of daily promotion candidates; the view is acknowledged as clunky and the decision is to keep it visible rather than lose it.
5. **The AGPL boundary is not a provenance boundary for `crates/vault` and `vault-core`.** Evidence below. The owner's relicensing grant (ontomark `NOTICE`, DEPRECATION §1) already covers them, so moving their read, edit, gate and conflict verbs into Ontomark is a **port** onto the typed model, not a clean-room rewrite. The governance transport (`vault/src/nostr.rs`, forum 31402 posting, solid-pod/sidestr lineages) stays AGPL in VisionClaw, as DEPRECATION D4 and ADR-OM-001 #4 say.
6. **Dependency direction rule.** Dependencies flow permissive → AGPL, never the reverse. Ontomark holds semantics (parse, model, reason, compile, ingest, read and edit verbs); VisionClaw and the Nostr transport hold servers and signing; Agentbox, visionGraph and the Loom pin released artefacts. This is the purpose of the multi-repo layout and is restated so it stops being rediscovered.
7. **The generation loop is the target shape.** Author (plugin, `vault edit`, agents) → `ontomark validate` → `ontomark build` → generation `visionGraph@<sha>` → publish (site; public bundle; private machine bundle) → consumers (Loom closure and SPARQL; VisionClaw force projection; Agentbox skills; RuVector memory stamped with the generation; optional server-side full-OWL audit, ADR-OM-004) → enrichment (`ontomark-ingest` `Candidates` from any host) → `vault propose` (Whelk and conflict blockers, forum 31402, human signature) → commit → next generation. Nothing is deleted before its replacement passes a parity gate (DEPRECATION §9; ADR-2137 §5 compatibility windows). Whether the plugin's ingest action emits a proposal rather than a local Apply for estate vaults is **not** decided here (ADR-OBS-003 open item).

## Evidence

### Provenance of `crates/vault` and `crates/vault-core` (VisionClaw at `3c3f4120`)

- `git shortlog -s -e -- crates/vault crates/vault-core`: 24 commits, all under the owner's agent identities; `authors = ["jjohare"]` in both `Cargo.toml` files; first commit 2026-09-22.
- `git blame` line census over 29,383 current lines: 100% those identities.
- The only "ported from" marker is `crates/vault/src/gate.rs`, from the owner's own `pipeline/gate.py` (prime-agent). No copied-snippet attributions, no `Copyright` lines naming third parties, no vendored code.

### Dependency licences that shape the split (`cargo metadata` on VisionClaw)

| Dependency | Licence | Consequence |
|---|---|---|
| `nostr-bbs-core` 1.0.0-beta.15 | AGPL-3.0-only; crates.io owner `jjohare` | Signing transport stays AGPL (owner may dual-license later). Proposal *content* (patch, evidence, blockers) is permissive and built in Ontomark. |
| `horned-owl` 0.11.0 (via the whelk fork) and 1.2.0 (direct) | LGPL-3.0 | Not in Ontomark's default tree. `whelk-el` exposes an **opt-in** `horned-owl` feature, off by default and absent from the CLI build; it appears under `--all-features`. Neither `THIRD-PARTY-NOTICES.md` nor `UPSTREAMS.md` mentioned it before today; `UPSTREAMS.md` now does. Restated as "optional native adapter, documented", not "must never enter". |
| `whelk` git fork | MIT (`LICENSE-WHELK`, ADR-OM-001 #3); the fork's `Cargo.toml` lacks a `license` field | Metadata gap only; already replaced by the published `whelk-el`. |
| everything else | MIT / Apache-2.0 / BSD | no constraint |

### Force-projection gap check (VisionClaw at `3c3f4120`, read-only, 2026-10-12)

VisionClaw's layout and projection read **no** Ontomark bundle artefact; every input is re-parsed in-tree by `crates/vault-core` or `visionclaw_domain::vault`. The only bundle read is the generation id (`crates/visionclaw-ontology/src/open_world.rs:720-797`).

| Input | Where VisionClaw reads it | In the bundle? |
|---|---|---|
| asserted `is-a` → `rdfs:subClassOf` | `page_parser.rs:163-250`, `github_sync_service.rs:1762-1815` | yes (`ontology.ttl`, scaffold `sup`, `api/pages`, NGG1 type 0); the kernel also wants the property IRI, which NGG1's type byte lacks |
| typed relations and their spring weight | `github_sync_service.rs:1784-1788`, `:2363-2395` (`SEMANTIC_TYPE_REGISTRY`) | relations yes (`ontology.ttl`, `api/pages`, scaffold `rel`); NGG1 collapses them to one type; **weights are VisionClaw's** |
| untyped wikilink edges (`explicit_link`) | `knowledge_graph_parser.rs:~322-338` | `api/pages` `wikilinks`/`backlinks` only; not NGG1 or TTL |
| `elevated_from` edge | `github_sync_service.rs:1722-1737` | no (OKF `sources:` absorbed it, `input/okf.rs:199`) |
| inferred closure | `github_sync_service.rs:1183-1300`, `inferred_edge_materialiser.rs` (own Whelk run, own property axioms, reduction, cap 8 per child) | `ontology-inferred.ttl`, scaffold `isup`, `api/pages` `inferredSuperClasses`: **a different set** |
| domain roots and `domain_member` spokes | `github_sync_service.rs:2233-2360`, `vault-core/src/domains.rs` | `overview.json` has 8 roots / 53 categories; VisionClaw mints its own `urn:ngm:domain:` |
| `source_domain` → class id / charge; population → spring scale | `gpu_resource_actor.rs:214-232`, `force_compute_actor.rs:859-879, 1034-1085` | domain yes; **charges, scales are VisionClaw's** |
| quality / maturity, title, definition, IRI, `public` gate, domain | various | yes |
| keyword domain fallback, `ln(bytes)` node size, `wikilink_count`, z position seed, `status` | `github_sync_service.rs:2398-2505`, `knowledge_graph_parser.rs` | no (derived presentation or VisionClaw-side) |

Conclusion: the bundle carries the corpus facts; what it lacks is either VisionClaw-owned (weights, charges, scales, seeds) or derived presentation. Two duplicate derivations should collapse onto the bundle: the inferred closure (route through `ontomark-reason`, per ADR-OM-001 D5) and the domain registry (`profile.taxonomy.domain_aliases`). One bundle addition would make the projection a pure function of it: a per-predicate relation type in the binary tiers (a wire-contract change; a spec-compile item, not decided here).

`working/`: `src/services/corpus_source/local.rs:20` and `docker-compose.unified.yml:102` set `VAULT_BASE_PATHS=knowledge/pages,working/pages`; public `working/` pages enter as plain nodes (`github_sync_service.rs:1829`) and never contribute classes (`page_parser.rs:~87`). visionGraph `publish.yml` runs `ontomark build --vault all` (explicit; the CLI defaults to `knowledge`, `main.rs:118/187`), which widens only `publish/` → `quartz/content/working` (`main.rs:392-404`, `publish.rs:1-15`); `ontology.ttl`, scaffold, search indexes, `api/*` and NGG1 are built from `knowledge/` only.

## Consequences

- `DEPRECATION.md` VC-60 changes disposition from REPIN to **RETIRE (gated)** and §13 records this alignment. Step 10's gate gains the private machine bundle; a new step covers the pod-resource producer on the Agentbox side.
- VisionClaw ADR-2106 stays accepted and live until gate (iii); its review trigger ("the release channel moving") is what fires.
- Agentbox ADR-2108 ("the vault binary is baked by Nix") is what ADR-2136's repin overturns; ADR-2136 now maps to it section by section and will supersede it on acceptance, not before.
- The seven estate ADR snapshots previously copied into `docs/adr/` are replaced by [`ESTATE-ADRS.md`](ESTATE-ADRS.md), a pointer index; four copies had already drifted from their sources and nothing checked them.
- The Loom's ADR-141 identity rule is amended (proposed) to separate identity from distribution; ADR-142 notes that the producer is Ontomark and the private bundle exists.
- No code changes. Each gate is a `DEPRECATION.md` row.

## Deviations and open items

- Not decided: proposal-not-Apply for estate vaults in the plugin (ADR-OBS-003); per-edge property IRI in NGG1; whether `nostr-bbs-core` is dual-licensed; full generation-loop activation order beyond DEPRECATION §9.
- Advisor review (Opus, 2026-10-12) corrected an earlier draft of this record: retirement had been written as decided before the owner decided it; "same source data" had been asserted while VisionClaw built `--vault knowledge` and visionGraph `--vault all`; `horned-owl` had been overstated as "must not enter". All three are corrected above.

## Amendment 2026-10-12 (same day): the OM-00 register

The ontomark compile/build pipeline was mapped into the VisionFlow diagrams-as-code corpus (`docs/diagrams/ontomark/`, OM-01..05, 36 diagrams, 967 verified citations) and its anomaly register OM-00 reconciled at `8d654d1`. Consequences for this record:

- **Exit-code contract.** An unsatisfiable corpus now exits **2** (typed `build::Unsatisfiable`), beside the credential, collision and reasoner-cap refusals; the CLI crate doc is amended. Consumers keying on exit 1 for this case see 2.
- **`content_digest` is redefined** over every byte the build read (pages and journals of every loaded vault plus the vocabulary, hashed at parse). The legacy value is not reproducible; `conformance.rs` compares the markers structurally and every other artefact byte for byte.
- **`--with-rvdb` exists in ontomark** (`build::records`, pure; transport in the CLI). The C3 line that called it VisionClaw-only is withdrawn.
- **VC-60 still ships** S1.1 (fail-open redactor), S1.2 (frontmatter unscanned), S1.5 (`--vault all` = knowledge only), S2.3 (non-atomic publish-out) and S3.1 until it retires under decision 2's gates; the fixes are in ontomark only.
- **Step 10's private machine bundle** is now specified against the reconciled `run()` (OM-00.4) and is the next code item.

## Amendment 2026-10-12 (same day): the machine-scope bundle design, locked

Design brief: `docs/design/machine-bundle-brief.md` (advisor-reviewed; the first draft's hook point and identity scheme were wrong and are replaced). Surface: `ontomark build --scope public|machine` (clap enum, default `public`; the marker gains `scope`, omitted when public so public markers stay byte-identical). Defaults taken under the owner's standing instruction, each reversible:

- **D1 twins.** The rule is directional. A public `working/` page whose un-prefixed identity matches a *private* page of either vault is a publication collision (fail closed); a public working page matching a *public* knowledge page yields to knowledge (dropped, counted), as VisionClaw's §V1 merge does; a *private* working page matching a *public* knowledge page is harmless and yields silently (never published, no marker, not counted). The first real-corpus build found this last case (`Web3`) and forced the amendment.
- **D2 identity.** Working records are keyed on the vault-relative path: `page_id = working/<id>`, slug per path segment, `page_iri = <page_namespace><slug>` (the slug path, so no spaces). Machine scope only; a public bundle never carries one.
- **D3 one resolver.** The `PrivateLinks` lookup (home vault, whole target, `#fragment` stripped, `.md` stripped) serves both redaction and plain-record links, mapped to post-disambiguation slugs; knowledge links are not re-resolved, so working edges are one-directional in v1.
- **D4 Loom's `data/` is scope-invariant.** Scaffold gets a knowledge-only backlink map, so every `data/` artefact but the markers is byte-identical across scopes; `api/pages` backlinks may include working sources.

Implementation status: in progress on `main` after this record; the C3 contract is amended when the field list ships.

### First real-corpus machine build (12 October 2026, evening)

`ontomark build --scope machine` on visionGraph `d9fc5bec4` succeeds: 120 s, 9,244 classes plus 83 plain `working/` records, 88 public working pages yielded to their public knowledge twins (the promotion staging ground beside its promoted page, as expected), `ontology_digest` equal to the public build's, every `data/` artefact content-identical to the public bundle, no `publish/` written. Two corpus faults the gate surfaced were fixed at source rather than in the build: 12 `working/pages/misc/` converter duplicates from the 2026-09-22 conversion (visionGraph `f21dab90e`), and one true public-working/private-knowledge slug collision, resolved by renaming the working note to the canonical `ComfyUI` form and marking it private (visionGraph `d9fc5bec4`). Eight agent-residue files remain under `working/pages/misc/` (manifests and analysis reports); held back by path, harmless, left for the owner.
