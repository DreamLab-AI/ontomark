# ontomark-profile-estate

The DreamLab estate profile for [ontomark](https://github.com/DreamLab-AI/ontomark):
the rules of the [visionGraph](https://github.com/DreamLab-AI/visionGraph)
corpus as versioned data. [`profile.yaml`](profile.yaml) is loaded into the
compiler's `Profile` type by `ontomark_profile_estate::profile()`, and the
`ontomark` CLI selects it by default for any repository holding
`ontology/vocabulary.yaml` (`--profile estate` names it explicitly).

**Most users never depend on this crate.** It exists so the estate's
publication pipeline can be reproduced byte for byte and so the CLI can be
published with it. A plain Obsidian vault, or any corpus that is not
visionGraph, uses `ontomark`'s generic profile (`--profile generic`, the
default without a vocabulary file) or its own profile YAML; see
`ontomark_compile::profile::Profile`.

What the profile pins:

- the `urn:ngm:class:` / `urn:visionflow:page:` identity namespaces and their
  `narrativegoldmine.com` projection;
- the eight-domain, 53-category taxonomy whose positions are published NGG1
  identities (append-only: changing a position needs consumer migration);
- the frontmatter grammar per vault role (`knowledge` fails on unknown keys and
  requires `type`, `title`, `resource`, `public`, `status`, `generated`;
  `working` is tolerant), the maturity lifecycle, the acronym table;
- the attribution roster location and `generated.by` actor grammar, the
  governance event kinds;
- `generation.prefix: visionGraph`, so every estate bundle's generation id is
  `visionGraph@<sha>` whatever directory the checkout sits in
  ([ADR-OM-008](../../docs/adr/ADR-OM-008-any-obsidian-vault-is-a-repository.md));
- the vault layout by default: `knowledge/` and `working/` beside `ontology/`,
  with `templates/`, `bases/`, `assets/` and `journals/` excluded
  ([ADR-OM-007](../../docs/adr/ADR-OM-007-vault-root-is-the-content-root.md)).

```rust
let estate = ontomark_profile_estate::profile();
assert_eq!(estate.id, "estate");
assert_eq!(estate.taxonomy.categories.len(), 53);
assert_eq!(estate.generation.prefix.as_deref(), Some("visionGraph"));
```

From the workspace root:

```bash
cargo test --locked -p ontomark-profile-estate
RUSTDOCFLAGS='-D warnings' cargo doc --locked -p ontomark-profile-estate --no-deps
```

MIT OR Apache-2.0; see the root [NOTICE](../../NOTICE) and the
[compiler documentation](../../crates/ontomark-compile/README.md).
