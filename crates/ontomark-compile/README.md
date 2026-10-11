# ontomark-compile

Compile Markdown ontology vaults into RDF 1.1 Turtle, JSON-LD 1.1, SHACL,
NGG1 graph tiers, page APIs and the public scaffold/prose indexes. A profile
supplies corpus-specific vocabulary, identity and publication rules.

The `bundle` module provides the shared C3 manifest and index reader types.
Loom and other readers can disable the classifier dependency:

```toml
[dependencies]
ontomark-compile = { version = "0.1.2", default-features = false }
```

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

The index's trust fields are generic. Bind `ScaffoldIndex<A, M, C>` to the
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

The native build pipeline preserves the legacy estate compatibility path.
Publication redaction resolves private aliases, fragments and sibling-vault
references before reasoning and all public exports. `--no-redact-private-links`
exists for recorded legacy comparisons; it is not a public-export privacy gate.

Both compiler routes now use `ontomark-reason`'s supported fragment over Whelk.
The native class adapter preserves legacy class closure and refuses reported
unsupported input before saturation. The typed API reports shared refusals
(including individual assertions), and preserves data assertions in RDF with
an omission count for reasoning. Use `ProjectionClassification::is_complete`
and `is_consistent` to include those source omissions in the verdict. These are
named-class coherence checks, not full OWL 2 EL consistency claims. Authored
Markdown remains the source of truth.

The extraction's relicensing grant and exclusions are recorded in the
repository's `NOTICE`. Licensed under MIT OR Apache-2.0; governance posting
and Nostr signing remain outside this crate.
