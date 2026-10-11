# ADR-OM-001: Decisions taken in the first ontomark sprint

- Status: accepted
- Date: 2026-10-11
- Decides for: ontomark (this repository) and the consuming estate repos
- Amends: VisionFlow ADR-2014, ADR-2015, ADR-2016; visionGraph ADR-VG-007;
  VisionClaw ADR-2137; Agentbox ADR-2136; Loom ADR-142; vowl-wasm ADR-001
- Verified against: VisionClaw tag `ontomark-census-baseline-1` (`de33ca538`),
  ontomark `32a3de3`

## Context

The eight ADRs above were written as proposals on 2026-10-11 and left ten
decisions open. The owner resolved them in the same session and a sprint began
immediately. This record fixes those resolutions and the facts established
during the census, so the sprint can resume from any point without re-deriving
them.

## Decisions

1. **Name and home.** Repository `DreamLab-AI/ontomark`, private until the
   first release looks complete, then public. Crates `ontomark-model`,
   `ontomark-parse`, `ontomark-reason`, `ontomark-compile`, `ontomark-wasm`,
   `ontomark-cli`, `whelk-el`. All names were free on crates.io when chosen.
2. **Licence.** Dual `MIT OR Apache-2.0` for every crate here. The owner is the
   sole rights holder for the AGPL sources being ported (VisionClaw `vault`,
   `vault-core`, `visionclaw-domain::vault`; knowledgeGraph `pipeline/`, all
   authored by the owner and the owner's agents) and relicenses them for this
   repository. The grant is recorded in `NOTICE`. Excluded, and never to be
   copied here: code derived from the solid-pod or sidestr lineages, which in
   practice is `vault/src/nostr.rs` and governance posting. Those stay AGPL in
   VisionClaw.
3. **Whelk.** Forked into `whelk-el` from `jjohare/whelk-rs` at `79a1ee2` (MIT,
   itself a port of Jim Balhoff's Whelk). MIT permits forking, renaming and
   abandoning upstream; the obligation is attribution, satisfied by
   `LICENSE-WHELK`, `NOTICE` and a header in `lib.rs`. `rayon` becomes an
   optional feature; the CLI and all `std::time`/thread/fs use are stripped from
   library code so the crate compiles to `wasm32-unknown-unknown`.
4. **Governance verbs.** VisionClaw keeps a thin AGPL `vault` adapter owning
   `propose`, `apply` and panel publishing, built on ontomark crates. Agentbox
   bakes both the ontomark CLI and that adapter.
5. **Standards first, superset as a profile.** OWL 2 EL, RDF 1.1 Turtle,
   JSON-LD 1.1, SKOS, PROV-O, DCTERMS, SHACL. Estate rules (frontmatter grammar,
   lifecycle and governance vocabulary, `urn:ngm:class:` and `resource:` IRI
   rules, the 53-category taxonomy, the attribution roster) live in a versioned
   `profiles/estate`; a generic profile must accept a plain one-note vault.
6. **Signing.** A `Signer` trait with `did:nostr` as one implementation; the
   default surface is unsigned so non-estate users are not forced into Nostr.
7. **Bulk migration permitted.** Reversible, idempotent, with a page-level
   report. Golden fixtures are captured before any migration runs. Git is the
   undo.
8. **Deprecate as we go.** Every duplicate semantic implementation in the
   estate is deleted or repinned in the same sprint, gated by the consuming
   repo's own tests, with that repo's docs and diagrams-as-code corpus updated
   in the same change. Inventory: `docs/DEPRECATION.md`.
9. **Scope.** Six repos named in the plan plus `knowledgeGraph` (the deployed
   corpus; source of truth is visionGraph) and the standalone WasmVOWL repo
   (archive or repin).
10. **Ownership.** DreamLab-AI owns the repository. Secret scanning and push
    protection are on from the first commit.

### Decisions taken during the sprint

11. **Unresolved references are reported, never invented.** A reference that
    matches no page is `Unresolved`. The legacy exporter's phantom parents and
    its first-claimant choice for the nine IRIs claimed by several pages live
    only in named compatibility adapters in `ontomark-compile`, off by default,
    listed as divergences, used solely by the golden-parity test.
12. **One IRI minter.** `Model::mint_iri(profile, kind, title)` is the only
    entry point; building a model never mints. It returns
    `Result<Iri, MintRefusal>` with refusals `TitleTaken { existing }`,
    `NoRule` and `IriClaimed`, and refuses any title an entity already has
    (case-insensitive, aliases included), naming the existing IRI, so a fresh
    mint can never create a second IRI beside a legacy-rule one such as
    `3-d-animation`. The 418 legacy-rule pairs are the fixture.
13. **Private-link redaction is on by default.** Staged Markdown drops
    frontmatter relations whose target is non-public and renders such body
    wikilinks as unlinked display text; `--no-redact-private-links` exists for
    parity runs only. Divergence 1 from legacy output; a permanent canary
    greps every public artefact and the staging tree for every private title.
14. **NGG1 has a dependency-free owner.** `ontomark-ngg1` (zero dependencies)
    owns the format; `ontomark-compile` re-exports it; `vowl-wasm` depends on
    it directly so its bundle does not grow.
15. **No source fallback in deploys.** `publish.yml` fetches a release asset by
    tag and verifies its sha256; a source fallback would run code the digest
    does not cover.
16. **Governance verbs stay AGPL in VisionClaw** (restates 4 after the
    census): `propose`, `apply`, `panel` only; everything else ships here.
17. **Loom's reload path becomes a tested Rust binary**, not a deletion:
    `reload_published_generation` in loom-facade replaces the Python timer
    once a fixture-bundle test passes.
18. **Cross-vendor audit is a recorded gate.** Each slice records its Codex
    audit verdict; while Codex was unavailable the row read NOT RUN, never
    "passed". Same-family Claude reviews are labelled degraded.

## Facts established by the census (2026-10-11)

- `vault` 20.5k lines and `vault-core` 6.3k lines, both AGPL-3.0-only at the
  baseline; `vault-core` has only portable dependencies, `vault` pulls `rayon`,
  `ureq` and `nostr-bbs-core`.
- Two `vault` pins diverge: visionGraph `publish.yml` builds from `1044b0722`;
  Agentbox `flake.nix` pins `3213e314`. The planning README's cited sha
  (`2a2156db`) was wrong.
- NGG1 has four implementations: `vowl-wasm` (Rust), the visionGraph explorer
  (`ngg1.ts:139/386/488`, encoder and decoder), knowledgeGraph's explorer copy,
  and the vault build. `ngg1-3n2e.bin` golden fixture exists but no test loads
  it.
- Loom mints `urn:ngm:class:` IRIs in three places and checks the generation
  marker twice (Rust and Python).
- Agentbox `management-api` has its own JavaScript frontmatter parser, slugifier,
  IRI minter, page writer and subclass extractor; nothing tests the baked vault
  package.
- The visionGraph explorer copies the taxonomy five times; eleven Python
  hygiene scripts and `fmlib.py` parse and write frontmatter and mint IRIs with
  no tests.
- About 45 of 143 VisionFlow diagram topics describe shapes being replaced.
- A reported 48-vs-53 category-table drift in VisionClaw was a miscount
  (rustfmt-wrapped tuples); both tables hold 53. A unit test now asserts they
  are one set (tag `ontomark-census-baseline-1`).
- The legacy `vault build` is deterministic apart from `generated_at` and the
  git commit, so byte parity is testable.

## Consequences

- Golden artefacts are captured at `ontomark-census-baseline-1`; parity tests
  compare against that tag.
- The ontomark CLI must satisfy the full caller contract in
  `DEPRECATION.md` §2.1 (not only `build|validate|gate|conflicts`), and a
  migration guard fails CI while any estate caller still invokes the legacy
  binary.
- Every specialist works in its own git worktree; the shared checkout stays on
  `main`.

## Open

- First release version and the exact two-release compatibility window.
- Whether `whelk-el` is published to crates.io before or after the first
  ontomark release.
- Mobile Obsidian support (out of scope for this sprint; desktop only).
