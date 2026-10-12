# Machine-scope bundle: implementation brief (2026-10-12)

Status: locked after advisor review (ADR-2134 checkpoint). Decisions D1–D4 below
are defaults taken by the implementer under the owner's standing instruction and
are recorded in ADR-OM-005; each is reversible.

## What it is

`ontomark build --scope machine` produces the bundle VisionClaw consumes
(ADR-OM-005 decision 4): the public knowledge projection **plus** `working/`'s
`public: true`, non-held pages as **plain records** (no OWL contribution), with
private-link redaction applied as to any bundle, and never the Quartz `publish/`
tree. `--scope public` (default) is today's build, byte for byte.

## Surface

- `build::Options.scope: Scope` where `enum Scope { Public, Machine }` with serde
  `rename_all = "lowercase"`; CLI `--scope public|machine` (clap `ValueEnum`,
  default `public`). The marker field uses the same spellings.
- `--scope machine` refuses (exit 2, typed error `ScopeConflict`): `--publish-out`;
  `--vault working|all` (the vault set is fixed to knowledge + working);
  `--no-redact-private-links`; a `vault_root` whose kind is not `knowledge`;
  an absent `working/pages`.
- `.generation.json` gains `scope: "machine"`, `#[serde(default, skip_serializing_if
  = "Scope::is_public")]` so public markers and the conformance golden are
  byte-identical and absence means public. `page_count` counts plain working
  records in machine scope. Fix every `Generation { .. }` literal.

## Projection (the part the first design got wrong)

1. Run the census (`inspect_walk`, S1.3 refusal) on `working/pages` too. Keep
   `api/census.json` knowledge-only.
2. Load working with `Vault::load_with_walk`; load it **unconditionally** in
   machine scope and add it to `git_dirty` read paths and to the digest
   `sources` (the digest already includes it via `working`/`other_vault`; make the
   machine path not depend on redaction being on).
3. `excluded_identities_of` over both vaults (held `_misc`/`misc` at any depth).
4. Collision gate and `ProseRedactor` marker set computed over the **union** of
   both vaults' public/private names and identities. D1 (twins): identity for
   the gate is the **un-prefixed** page identity, so a public working `X` beside
   a private knowledge `X` is a `PublicationBlocked::Collision` (fail closed);
   a public working page whose un-prefixed identity equals a public knowledge
   page's **yields** (is dropped from the machine bundle, with a count in the
   report), matching VisionClaw's §V1 merge where knowledge wins. **Amended
   after the first real-corpus build:** the rule is directional. A *private*
   working page whose identity a *public* knowledge page holds (the corpus's
   `Web3`) is harmless, is never published, adds no marker, and yields
   silently without a count; only a public working page beside a private page
   of either vault is refused.
5. `project_vault_with_links` generalised to take `&[(&Vault, VaultKind)]` and
   use each page's home kind for `links.is_private(home, …)`.
6. Plain override, applied after the rebuild and before `by_class_slug`:
   working records get `has_ontology = false`, `iri = ""`, `entity_type =
   Class` (there is no plain variant; page_api emits `entityType` only under
   `has_ontology`).
7. Identity of a working record (D2): `page_id = "working/<vault-relative id>"`,
   `slug = "working/" + id slugified per path segment` (subdirectories kept, so
   two `Foo.md` in different folders do not collide), `page_iri = profile
   page_namespace + slug` (**amended:** the slug path, not the raw id, so the
   IRI carries no spaces). Machine scope only; a test asserts no
   `urn:…:page:working/` IRI appears in a public bundle.
8. Links (D3): one resolver, extracted from `redact::PrivateLinks` (home vault
   first, then whole target, then strip `#fragment`, then strip `.md`, over the
   `NameIndex` of id/title/slug/aliases), returning `(VaultKind, page index)`;
   used for redaction and for plain records' `links`, mapped to the
   **post-disambiguation** record slug. Unresolved targets fall back to
   slug-verbatim as knowledge does. Knowledge records' links are **not**
   re-resolved (their bytes stay identical); edges to working are therefore
   one-directional in v1 and that is documented.
9. D4: `scaffold_index` receives a **knowledge-only** backlink map in machine
   scope so `data/` (bar the markers) is byte-identical across scopes; Loom
   reads `data/`. `api/pages/*.json` backlinks may include working sources.
10. The credential gate scans working's public pages (`stage_redacted` over
    both vaults) but the staging files are **discarded**: no `publish/` is
    written or recorded in machine scope.
11. `validate(profile, &vault, &raw, …)` is positional over `vault.pages`: do
    **not** append to `raw`. Validate working separately under its role, errors
    refuse the build as for knowledge.

## Invariants (tests)

- Machine vs public build of the same inputs: `data/ontology.ttl`,
  `data/ontology-inferred.ttl`, `data/graph/*`, `data/ontology.json`,
  `data/scaffold-index.json`, `data/prose-index.json`, `okf/*` byte-identical;
  `ontology_digest` equal; `content_digest` equal when both loaded working.
- `api/pages/working/<slug>.json` and a search-index entry exist for the public
  working fixture page; the private working page appears nowhere.
- `publish/` absent in machine scope; `.generation.json` has `scope: "machine"`;
  public marker bytes unchanged by the field (conformance golden still passes).
- Two public `working/{a,b}/Foo.md` build without a claim collision.
- Public working twin of a private knowledge page: refused (Collision).
- Public working twin of a public knowledge page: yields; count reported.
- Public working prose naming a private working page's title: masked.
- Dirty `working/` gives `+dirty` in machine scope with `source: None`.
- Working page with `public: "true"`: census refusal.
- `--scope machine --publish-out …`, `--vault all`, `--no-redact-private-links`:
  exit 2 with the conflict named.
- Credential in a public working page: `SecretsFound`, exit 2.

## Docs to touch in the same change

- C3 contract (`VisionFlow/docs/engineering/sovereign-corpus-contracts.md`):
  `scope` field, machine-scope artefact set, working page IRI convention,
  `page_count` meaning per scope.
- ADR-OM-005: record D1–D4 and the `--scope` surface; DEPRECATION §13 AB-P6
  names `--scope machine`; HANDOFF.
- Not verified by anyone yet: the Loom marker reader's tolerance of unknown
  fields (lenient readers elsewhere; `Generation` has no `deny_unknown_fields`),
  and whether the explorer URL-encodes `/` in search-index ids.
