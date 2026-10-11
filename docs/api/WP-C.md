# WP-C — compiler, profiles and CLI

Status: the portable CLI prerelease is published from `21cd9ff`. The recovered
typed projection and shared native reasoner are integrated into public `main`;
current compiler 0.1.3 is published. Current receipts are in
`docs/evidence/resume/`; remaining consumer adoption is recorded in
[DEPRECATION](../DEPRECATION.md).

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
(networked embedder). Each relocated module names its origin. The sole rights
holder's relicensing grant and exclusions are recorded in `NOTICE` and
ADR-OM-001; this is resolved.

The NGG1 reader descends from `vowl-wasm` 0.1.2 (MIT); its notice is kept.

## Resume verification

The initial recovered source at `21cd9ff` passed `just check`: 746 tests, 7 ignored,
formatting and clippy with warnings denied. This includes the fixture parity
tests and ten public-export privacy canaries. Previously ignored Turtle writer
tests now run, including 400 random graph seeds per profile. Additional
regressions cover private aliases in display labels, cross-vault machine links
and quoted YAML definition keys. Definitions containing private conjuncts are
withheld as a whole.

`TMPDIR=/tmp ./conformance/parity.sh <release-binary> ../visionGraph
conformance/golden/corpus-visiongraph` reports zero differences across 32
artefact lines and 12 verb outputs for committed corpus `b5e36abd1`. This
compatibility check uses `--no-redact-private-links`; public redaction has
separate canaries. The first resumed full-corpus receipt used the local Nix
build, which cannot be shipped as an ordinary Linux asset. The clean Debian-built
portable asset also passes the same full corpus parity gate (see
`corpus-parity-portable.log`) and builds the synthetic fixture offline on Debian 12.

Compiler 0.1.1 now publishes the strict C3 marker and shared scaffold/prose
reader contracts. Current compiler 0.1.3 provides the opt-in typed/in-memory
projection API and shared native reasoner. Its initial 0.1.2 release is yanked
after correcting Unicode private-name filtering. See [typed API migration](WP-C-TYPED.md) for the explicit
identity, privacy and supported-profile semantics.

The shared native adapter at `6c040a5` also passes the full committed corpus
comparison: zero differences across the same 32 artefact lines and 12 command
outputs for visionGraph `b5e36abd1`. This receipt covers the native adapter;
the typed model route has separate synthetic migration tests.

Remaining estate migrations include applying the typed projection through a
normative estate profile, the native bundle's 690 SKOS-only class declarations,
and other estate consumer replacements. The typed API declares every model class
as `owl:Class`; this does not claim a completed full-corpus native migration.
