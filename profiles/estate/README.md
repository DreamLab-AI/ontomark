# ontomark-profile-estate

DreamLab's estate vocabulary and publication policy as versioned data.
[profile.yaml](profile.yaml) supplies vault roles, lifecycle rules, identity
namespaces, public URLs, taxonomy and attribution conventions. The crate's
`profile()` function loads it into the compiler's `Profile` type.

The profile preserves eight domain positions and 53 category identities used
by the published NGG1 graphs. These positions are compatibility data; changing
them requires consumer migration. The [crate-level example](src/lib.rs) checks
the profile identity, category count, domain ID and class namespace.

This is the legacy estate compiler profile. The compiler's opt-in typed
projection takes a normative model and its own explicit profile mapping;
availability of that API does not migrate the estate corpus automatically.
Other applications should supply vocabulary appropriate to their vaults.

From the workspace root:

```bash
cargo test --locked -p ontomark-profile-estate
RUSTDOCFLAGS='-D warnings' cargo doc --locked -p ontomark-profile-estate --no-deps
```

MIT OR Apache-2.0; see the root [NOTICE](../../NOTICE) and
[compiler documentation](../../crates/ontomark-compile/README.md).
