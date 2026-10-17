# ontomark-compile

Compile Markdown ontology vaults into RDF 1.1 Turtle, JSON-LD 1.1, SHACL,
NGG1 graph tiers, page APIs and the public scaffold/prose indexes. A profile
supplies corpus-specific vocabulary, identity and publication rules.

The `bundle` module provides the shared C3 manifest and index reader types.
Loom and other readers can disable the classifier dependency:

```toml
[dependencies]
ontomark-compile = { version = "0.4.0", default-features = false }
```

## Vault layout

A vault is an ordinary Obsidian vault ([ADR-OM-007](../../docs/adr/ADR-OM-007-vault-root-is-the-content-root.md)):
pages sit at the vault root, and a page's id is its root-relative path without
`.md` (`ETSI/Domain.md` is `ETSI/Domain`). Top-level folders holding Obsidian
furniture rather than pages are skipped by the page walk and reported as
`excluded_directory` skips, never silently dropped: the built-in list
`input::page::EXCLUDED_DIRS` (`templates`, `bases`, `assets`, `journals`), or
the profile's `layout.excluded` in its place. Dot-folders (`.obsidian/`) are
hidden regardless; `journals/` is loaded as its own tree.

Where the vaults sit under the repository root is the profile's
`profile::Layout` ([ADR-OM-008](../../docs/adr/ADR-OM-008-any-obsidian-vault-is-a-repository.md)):
`layout.knowledge` (default `knowledge`) and `layout.working` (default
`working`; `null` for a layout with no working vault). `knowledge: "."` means
the repository root *is* the vault — a plain Obsidian vault opened directly.
Code that needs a folder asks the layout (`Layout::root_of`); a role the layout
does not place is refused by name (`MissingVault`).

```yaml
# A vault that keeps notes in notes/, drafts in drafts/, and excludes scratch/.
layout:
  knowledge: notes
  working: drafts
  excluded: [templates, bases, assets, journals, scratch]
# A plain Obsidian vault: the root is the vault, there is no working vault.
# layout: {knowledge: ".", working: null}
```

The output contract does not move with the input layout: knowledge pages
stage under `publish/pages/`, working pages under `publish/working/`, and every
report, digest path and staging count is keyed by the role name
(`VaultKind::role_name()`: `knowledge`, `working`). A build's generation id is
`<prefix>@<commit>` (`+dirty` for an uncommitted build); the prefix is
`profile.generation.prefix`, pinned to `visionGraph` by the estate profile and
otherwise the repository root's directory name.

## Testing

`cargo test -p ontomark-compile` runs the default leg with the `whelk-el`
classifier; CI also runs `--no-default-features` and `--no-default-features
--features model-projection`. Integration tests that use the classifier are
declared in `Cargo.toml` with `required-features`, and
`tests/feature_gates.rs` fails on every leg if one is not.

```rust
use ontomark_compile::bundle::ScaffoldIndex;

let index: ScaffoldIndex = serde_json::from_str(
    r#"{"version":1,"classes":{"animal":{"t":"Animal"}}}"#,
)?;
assert_eq!(index.classes["animal"].t.as_deref(), Some("Animal"));
# Ok::<(), serde_json::Error>(())
```

`BundleManifest` is the existing generation stamp. Its `from_json` method
checks required C3 fields, identity consistency, digest syntax, relative
artifact paths and unique claims. A hosting application must still verify
file bytes, expiry and loaded identity before activating a bundle. Legacy
artifact-map markers require an explicit legacy reader.

Trust fields on the index are generic. Bind `ScaffoldIndex<A, M, C>` to the
hosting application's attestation, mapping and computation types; the compiler
does not interpret or grant those claims. Omitted legacy fields remain
optional and unknown extensions survive an index round-trip.

Enable `model-projection` for pure in-memory compilation of a normative
`ontomark-model::Model`. `projection::project_model` takes the model, its injected
profile and explicit `Scope::Authoring` or `Scope::Public`. It returns asserted
RDF, shared logical axioms and omission counts without reading files. Public
scope withholds private targets before RDF and reasoning, and conservatively
withholds literals naming private entities. This API does not project page prose.

Every typed class gets `owl:Class`, including classes with only SKOS relations.
Missing identities and duplicate IRI claims never obtain invented class nodes.
These are intentional differences from the rc1 legacy path: adopting this API
requires checking source diagnostics and explicit profile mapping. The native
legacy build remains the recorded compatibility route; the typed API has no
phantom-parent or first-claimant compatibility defaults.

Native builds keep the legacy estate compatibility path.
Publication redaction resolves private aliases, fragments and sibling-vault
references before reasoning and all public exports. `--no-redact-private-links`
exists for recorded legacy comparisons; it is not a public-export privacy gate.

Both compiler routes now use `ontomark-reason`'s supported fragment over Whelk.
On the native side, the class adapter keeps legacy class closure and refuses reported
unsupported input before saturation. The typed API reports shared refusals
(including individual assertions), and preserves data assertions in RDF with
an omission count for reasoning. Use `ProjectionClassification::is_complete`
and `is_consistent` to include those source omissions in the verdict. These are
named-class coherence checks, not full OWL 2 EL consistency claims. Authored
Markdown remains the source of truth.

Version 0.1.2 requires `ontomark-reason` 0.1.1 or later so new builds use the
Whelk package with corrected upstream attribution and its Scala BSD notice.

Relicensing grant and exclusions for the extraction are recorded in the
repository's `NOTICE`. Licensed under MIT OR Apache-2.0; governance posting
and Nostr signing remain outside this crate.

### Public text case comparison

Version 0.1.3 makes private-text filtering use the same Unicode case comparison as model reference resolution. Public literals containing `STRASSE` for a private `Straße` alias, or a final-sigma spelling of a private Greek alias, are withheld. Authoring projections retain those values; unrelated public values remain. The regression covers both controls and identifier-free omission counts. The legacy CLI path is unchanged.
