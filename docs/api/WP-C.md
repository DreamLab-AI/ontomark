# WP-C — compiler, profiles and CLI

Status: in progress on branch `wp-c-compile`. This page is rewritten with
executed results when the slice closes; every claim below cites the command
that proved it.

## Crates

| Crate | Path | Owns |
|---|---|---|
| `ontomark-compile` | `crates/ontomark-compile` | The build pipeline (pages → one generation), RDF 1.1 Turtle, JSON-LD 1.1, SHACL shapes, page API, indexes, OKF bundle, graph tiers, checks (`validate`, `conflicts`, `gate`, body residue), EL axiom extraction and the asserted/inferred split, `migrate` |
| `ontomark-ngg1` | `crates/ontomark-ngg1` | The NGG1 graph-tier format: constants, encoder, zero-copy reader. Re-exported as `ontomark_compile::ngg1`; `vowl-wasm` re-exports it |
| `ontomark-profile-estate` | `profiles/estate` | The visionGraph corpus profile (`profile.yaml`) |
| `ontomark-cli` | `crates/ontomark-cli` | The `ontomark` binary |

## Provenance and licence

The pipeline is relocated from VisionClaw `crates/vault` and `crates/vault-core`
at tag `ontomark-census-baseline` (`1044b0722`). Those crates are declared
`AGPL-3.0-only` in VisionClaw; the relocation into this MIT OR Apache-2.0
workspace follows the sprint brief (non-negotiable 5: only code descending from
solid-pod or sidestr, in practice `vault/src/nostr.rs` and governance posting,
stays AGPL). Not relocated: `nostr.rs`, `propose.rs`, `apply.rs`, `edit.rs`,
`create.rs`, `diff.rs`, `vault-core/src/{graph,proposal}.rs`, `build/rvdb.rs`
(networked embedder). Each relocated module names its origin. Open question for
the owner: confirm the relicensing of the relocated files.

The NGG1 reader descends from `vowl-wasm` 0.1.2 (MIT); its notice is kept.
