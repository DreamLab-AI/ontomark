# ontomark-cli

Native publication CLI for Markdown ontology vaults. It implements the estate
publication contract through the shared compiler and a caller-selected profile.
`ontomark` is the binary, distributed as a GitHub release asset.

Released commands: `build`, `validate`, `gate`, `conflicts`, `repair`
and `migrate`. Use `ontomark --help` and each command's `--help` for flags.
`repair` has `bodies` and `fences` subcommands; `migrate` has `list`, `plan`,
`apply`, `rollback` and `status`. `attribute`, `assets` and `body-residue` remain
outside this CLI. Governance posting and graph retrieval stay with their hosts.

Global options include `--repo`, `--profile` and `--json`. The repository grammar
and default profile selection are described by the help output. Commands can
write publication outputs, repair source files or apply migrations; review
migration plans before applying them. Journals under `.ontomark/` preserve
original bytes, including private pages, and must stay outside publication.

Classification in the legacy estate projection is delegated to the shared reasoner.
It preserves recorded corpus compatibility; adopting the compiler's typed
in-memory API requires separate profile and identity migration.

## Build and install

From the workspace root:

```bash
cargo build --locked -p ontomark-cli --release
cargo test --locked -p ontomark-cli
```

The [Linux x86-64 prerelease](https://github.com/DreamLab-AI/ontomark/releases/tag/v0.1.0-rc.1)
ships an ordinary ELF binary verified on Debian 12 without networking.
Follow the root [download and checksum instructions](../../README.md#cli-prerelease).
Do not distribute a local binary that depends on Nix store paths as that asset.

MIT OR Apache-2.0. The root [NOTICE](../../NOTICE) records the source relocation
and relicensing grant. [Upstream sources](../../docs/UPSTREAMS.md) and
[full dependency notices](../../THIRD-PARTY-NOTICES.md) accompany the release.
