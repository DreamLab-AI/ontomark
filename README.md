# Ontomark

Ontologies in Markdown: a lossless parser, typed model, portable reasoner and compiler for Markdown notes with YAML frontmatter. Use the Rust crates in your application, build publication artefacts with the CLI, or work inside Obsidian with the [separate plugin](https://github.com/DreamLab-AI/ontomark-obsidian).

[![CI](https://github.com/DreamLab-AI/ontomark/actions/workflows/ci.yml/badge.svg)](https://github.com/DreamLab-AI/ontomark/actions/workflows/ci.yml)
[![Licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue)](NOTICE)

## Start with two notes

`Animal.md`:

```markdown
---
type: class
title: Animal
---
```

`Dog.md`:

```markdown
---
type: class
subclass-of: ["[[Animal]]"]
---
```

Ontomark’s generic profile recognises these as classes and treats Dog as a subclass of Animal. Plain notes remain plain notes; links in prose are navigation, rather than ontology axioms. The Obsidian plugin indexes, classifies and queries the vault locally, and previews frontmatter edits before applying them.

## CLI prerelease

The [v0.1.0-rc.1 release](https://github.com/DreamLab-AI/ontomark/releases/tag/v0.1.0-rc.1) includes a Linux x86-64 binary, SHA-256 receipt and versioned WASM engine bundle. The Linux binary was built from clean source and verified on Debian 12 with networking disabled.

```bash
curl -fLO https://github.com/DreamLab-AI/ontomark/releases/download/v0.1.0-rc.1/ontomark-x86_64-unknown-linux-gnu
curl -fLO https://github.com/DreamLab-AI/ontomark/releases/download/v0.1.0-rc.1/ontomark-x86_64-unknown-linux-gnu.sha256
sha256sum --check ontomark-x86_64-unknown-linux-gnu.sha256
chmod +x ontomark-x86_64-unknown-linux-gnu
./ontomark-x86_64-unknown-linux-gnu --help
```

The CLI prerelease implements the estate publication contract. Its `build`, `validate`, `gate`, `conflicts`, `migrate`, `attribute`, `assets` and `body-residue` commands are documented by `--help`. For a general vault, start with the parser/model crates or the Obsidian plugin. The library's generic profile and the CLI's estate compiler have different entry points.

## Rust crates

Each crate has its own README and inline rustdoc, including executable examples.

| Crate | Purpose |
| --- | --- |
| [ontomark-parse](crates/ontomark-parse) | Lossless frontmatter, body spans, links, footnotes and guarded patches. |
| [ontomark-model](crates/ontomark-model) | Classes, individuals, properties, IRI resolution and pluggable profiles. |
| [whelk-el](crates/whelk-el) | Portable saturation engine derived from Whelk, with optional native parallelism. |
| [ontomark-reason](crates/ontomark-reason) | Supported-profile checking, explicit refusal reasons and cancellable classification. |
| [ontomark-compile](crates/ontomark-compile) | Turtle, JSON-LD, SHACL, PROV-O, public projections and typed bundle readers. |
| [ontomark-ngg1](crates/ontomark-ngg1) | Shared binary graph encoder and zero-copy reader. |
| [ontomark-wasm](crates/ontomark-wasm) | Worker-friendly index, update, classify, query and patch protocol. |
| [ontomark-cli](crates/ontomark-cli) | Native publication commands. |

Parser, model and reasoner crates support native and `wasm32` targets. The compiler's portable feature set disables its optional native backend. The [estate profile](profiles/estate) owns DreamLab vocabulary and policy; applications can provide their own profile.

## Build and check

Rust 1.89 or later is required by the locked workspace. Install `wasm32-unknown-unknown` to build the browser engine. The plugin repository builds with Node and a checksum-pinned engine release, independently of this Rust workspace.

```bash
cargo build --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
cargo build --locked -p ontomark-wasm --target wasm32-unknown-unknown --release
```

`just check`, `just wasm-test` and `just conformance` provide the repository gates. CI checks formatting, Clippy, native tests, WASM, strict rustdoc, plugin contracts and complete Git history for secrets. The plugin tests include actual WASM/native transcript parity and an offline Obsidian API shim; that shim does not substitute for installed Obsidian acceptance.

## Current limits

This is experimental software. The reasoner implements a subset of OWL 2 EL and reports unsupported constructs explicitly. Dropped existential restrictions make a classification incomplete. The publication compiler still uses its legacy classification path while adoption of the shared facade is developed. General in-memory projection and additional CLI commands remain in progress.

Turtle, JSON-LD, SHACL, SKOS, PROV-O and Dublin Core form the publication surface. Public projections omit private content and sanitise links to private targets. Conformance and privacy tests record the supported behaviour; no claim of complete OWL 2 EL or universal standards conformance is made.

## Project documentation

- [Current hand-off](docs/HANDOFF.md): verified results, outstanding work and integration boundaries.
- [API documentation](docs/api/): crate contracts and capabilities.
- [Design decisions](docs/adr/ADR-OM-001-sprint-decisions.md): profiles, licensing and reasoner choices.
- [Estate migration plan](docs/DEPRECATION.md) and [branch recovery](docs/BRANCH-RECOVERY.md): consumer replacements and preserved interrupted work.
- [Run log](docs/RUNLOG.md) and [evidence](docs/evidence/resume/): reproducible checks and historical receipts.

Maintained by [John O'Hare](https://github.com/jjohare) and [DreamLab AI](https://github.com/DreamLab-AI).

## Licence

MIT OR Apache-2.0. [NOTICE](NOTICE) records the rights holder's relicensing grant and exclusions. Whelk's inherited MIT attribution remains with the reasoner package; see [its README](crates/whelk-el/README.md).
