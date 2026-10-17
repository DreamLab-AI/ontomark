# ADR-OM-007: the vault root is the content root; the `vault` verbs live in `ontomark`

- Status: Accepted (owner decisions of 16 October 2026)
- Implementation: active from v0.3.0. Layout: `crates/ontomark-compile/src/input/page.rs` (`Vault::load`, `EXCLUDED_DIRS`, `SkipReason::ExcludedDirectory`), `profile.rs` (`Layout`). Verbs: `crates/ontomark-compile/src/ir/graph.rs`, `src/write/`, `crates/ontomark-cli`.
- Related: ADR-OM-005 (one producer), ADR-OM-008 (any Obsidian vault is a repository: layout folders and generation prefixes from the profile), `docs/DEPRECATION.md` §2.1 and rows VC-29/VC-30, visionGraph commit `e98034e06` (the flatten), VisionClaw's matching change.

## Context

The visionGraph vaults kept their pages under a `pages/` folder, a Logseq convention that Obsidian does not share. Every tool that read them encoded it: the compiler walked `root/pages`, stripped the prefix in five places, and refused a vault without the folder; Bases filtered on it; the hygiene scripts, the transcript pipeline and VisionClaw's parser all spelled it out. Meanwhile Obsidian's own furniture (`templates/`, `bases/`, `assets/`, `.obsidian/`) sat at the root, where the Logseq layout had never expected notes to be.

Page identity never depended on the folder: it is the path under the content root without `.md`, and IRIs come from frontmatter. The published URL contract (`/notes/pages/<slug>`) is a name the compiler writes into its staging tree, not the source layout.

## Decisions

1. **The content root is the vault root.** `Vault::load(root, kind)` walks `root`. There is no `pages/` detection and no compatibility shim; git is the rollback. Ids and `rel_path` are root-relative, so a page that was `pages/Foo.md` is still `Foo`, and `podcast-evidence/x` keeps its prefix.
2. **Furniture is excluded by a built-in list** (`templates`, `bases`, `assets`, `journals`; dot-folders were already hidden) that the profile can extend through `layout.excluded`. An excluded Markdown file is recorded as a skip with a reason, never silently absent. Journals remain a sibling tree loaded separately.
3. **The output contract does not move.** The staging tree still lays knowledge out under `pages/` and working under `working/`; `api/pages/<slug>.json` keeps its path. Public URLs are unchanged.
4. **The `vault` CLI's read and write verbs move here.** `find`, `retrieve`, `tree`, `edit`, `create` and `apply` are ported onto the typed model with the same flags, JSON shapes and exit codes, and verified byte-identical against the `vault` binary on the full corpus. `propose` and its Nostr transport stay in VisionClaw (AGPL, deliberately not ported). This closes ADR-OM-005's remaining duplicate parser on the read side.
5. **Retired content is deleted, not archived in the vault.** The `.deleted/` tray goes; the commit that retired its pages is the archive.

## Evidence

A build of the flattened visionGraph with this compiler, against a baseline of the old layout with the previous release: the staged site (9,416 pages) byte-identical; all 28,072 site-data files identical except the generation stamps, timestamps, and the census and validation report no longer listing the 28 retired pages as skips. Verb parity: 23 of 23 queries byte-identical, including a 631 kB `tree`.

## Consequences

- v0.3.0 is a minor bump: the input layout contract changes for every CLI user. A vault still on `pages/` fails with "not a directory" only if `pages/` was the whole vault; otherwise it loads the furniture-free root and finds no pages, which `validate` reports.
- Three pins move together: visionGraph `publish.yml`, agentbox `lib/ontomark.nix`, and the plugin's engine pin. The agentbox derivation's `--version` install check turns a stale pin into a build failure.
- `conformance/golden/corpus-visiongraph` was captured by the legacy `vault` binary on the old layout and is re-captured by ontomark itself once visionGraph flattens; the lineage is the byte-identical staged content above.
- The ontology-augment skill routes `find`/`retrieve`/`tree`/`edit` to `ontomark`; `vault` is needed only for `propose`.
