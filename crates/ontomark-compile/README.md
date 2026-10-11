# ontomark-compile

Compile Markdown ontology vaults into RDF 1.1 Turtle, JSON-LD 1.1, SHACL,
NGG1 graph tiers, page APIs and the public scaffold/prose indexes. A profile
supplies corpus-specific vocabulary, identity and publication rules.

The `bundle` module provides the shared C3 manifest and index reader types.
Loom and other readers can disable the classifier dependency:

```toml
[dependencies]
ontomark-compile = { version = "0.1.1", default-features = false }
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

The native build pipeline preserves the legacy estate compatibility path.
Publication redaction resolves private aliases, fragments and sibling-vault
references before reasoning and all public exports. `--no-redact-private-links`
exists for recorded legacy comparisons; it is not a public-export privacy gate.

The classifier currently uses a supported subset of OWL 2 EL. Capability and
per-run completeness must be reported by the reasoner; this crate does not
claim complete OWL 2 EL support. Authored Markdown remains the source of truth.

The extraction's relicensing grant and exclusions are recorded in the
repository's `NOTICE`. Licensed under MIT OR Apache-2.0; governance posting
and Nostr signing remain outside this crate.
