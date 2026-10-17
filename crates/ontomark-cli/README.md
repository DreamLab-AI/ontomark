# ontomark (command line)

`ontomark` turns a vault of Markdown notes into standard artefacts — RDF 1.1
Turtle (asserted and inferred), JSON-LD 1.1, SHACL, the NGG1 graph tiers, a
page API, search and scaffold indexes and a redacted `publish/` tree — and
gives a corpus workflow the verbs around that: validation, a quality gate,
conflict detection, graph queries, guarded edits and reversible migrations.
It is the installable form of the `ontomark-compile` crate, and a drop-in for
VisionClaw's `vault` on the verbs both have.

## Install

```bash
cargo install ontomark
```

or take the static Linux x86-64 binary from a
[GitHub release](https://github.com/DreamLab-AI/ontomark/releases):

```bash
V=0.4.0
curl -fLO https://github.com/DreamLab-AI/ontomark/releases/download/v$V/ontomark-x86_64-unknown-linux-gnu
curl -fLO https://github.com/DreamLab-AI/ontomark/releases/download/v$V/ontomark-x86_64-unknown-linux-gnu.sha256
sha256sum --check ontomark-x86_64-unknown-linux-gnu.sha256
chmod +x ontomark-x86_64-unknown-linux-gnu && ./ontomark-x86_64-unknown-linux-gnu --help
```

## Two kinds of repository

`--repo` (default: the current directory) names one of two shapes
([ADR-OM-008](../../docs/adr/ADR-OM-008-any-obsidian-vault-is-a-repository.md)):

- **A plain Obsidian vault.** Notes at the root, `.obsidian/`, `templates/`,
  `bases/` beside them, no `ontology/` folder anywhere above. The directory
  itself is the knowledge vault, there is no working vault, and the generic
  profile and the built-in vocabulary apply. Anything that needs a working
  vault (`--vault working|all`, `--scope machine`) is refused with exit 2 and
  a message naming the layout.

  ```bash
  ontomark --repo ~/Vaults/Notes validate
  ontomark --repo ~/Vaults/Notes build --out /tmp/notes-www
  ontomark --repo ~/Vaults/Notes find --query dog
  ```

- **An estate-shaped repository.** `ontology/vocabulary.yaml` beside the
  vaults the profile's `layout` places — `knowledge/` and `working/` by
  default. The nearest ancestor holding `ontology/vocabulary.yaml` is the root,
  and the estate profile is the default. `--profile generic` or `--profile
  path/to/profile.yaml` overrides it; a profile file is taken as written,
  including its `layout`.

## Vault layout

A vault is an ordinary Obsidian vault
([ADR-OM-007](../../docs/adr/ADR-OM-007-vault-root-is-the-content-root.md)):
pages at the vault root, ids are root-relative paths without `.md`, and the
furniture folders (`templates/`, `bases/`, `assets/`, `journals/`, plus a
profile's `layout.excluded`) are skipped and reported in the census and the
validation report, never silently dropped. The profile's `layout.knowledge`
and `layout.working` say where the two vault roles live under the repository
root; `"."` means the root itself. The output contract is the same whatever
the input layout: knowledge pages stage under `publish/pages/`, working pages
under `publish/working/`, and reports are keyed by role name.

Every build stamps a generation, `<prefix>@<commit>` (`+dirty` when the vault
paths had uncommitted changes), into `.generation.json`. The prefix is the
profile's pin (`visionGraph` for the estate) or the repository's directory
name, so a vault at `~/Vaults/Notes` produces `Notes@<sha>`.

## Verbs

| Command | What it does |
| --- | --- |
| `validate` | Frontmatter grammar, vocabulary agreement, link integrity, credential scan and the public gate for the selected vaults. |
| `gate --tier quick\|full` | The autonomous continuation gate: validation, and with `full` conflicts plus the OWL 2 EL classifier. |
| `conflicts` | Semantic conflict detection with a failing `--severity`. |
| `build --out DIR` | The publication build. `--scope machine` writes the machine bundle (knowledge plus the working vault's public notes as plain records, no `publish/`); `--with-rvdb` adds embedded records; `--publish-out` stages the Markdown elsewhere. |
| `find`, `retrieve`, `tree` | Title and alias search; expansion from seed pages along declared edge types; the taxonomy tree beneath a page. |
| `edit`, `create`, `apply` | Guarded writes: a frontmatter edit with a declared blast radius, a new page from a staged file, a human-approved `kind: amend` proposal. |
| `repair bodies\|fences` | Logseq outliner bodies to Obsidian Markdown; unmatched code fences. |
| `migrate list\|plan\|apply\|rollback\|status` | Registry-driven, journalled, reversible corpus migrations. |

Every verb takes `--json`; `--vault knowledge|working|all` selects the roles
(`all` means both, everywhere). `ontomark <verb> --help` lists the flags.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | A failed check (validation errors, a failing gate or conflict) or an execution error. |
| `2` | A refusal or an invalid request: a credential on a page selected for publication, two artefacts claiming one output path, an unsatisfiable class, a definition the reasoner would not finish on, a `--scope machine` build asked for with something it cannot honour, a vault role the layout does not have, a `create`/`apply` whose guards refuse the write, or an unknown `--vault`, `--scope`, `--tier` or `--severity` value. Refusals name what they refused and write nothing. |

Migration journals under `.ontomark/` keep the original bytes of migrated
pages, private ones included: keep that directory out of git and out of
publication.

## Build from source

```bash
cargo build --locked -p ontomark --release
cargo test --locked -p ontomark
```

MIT OR Apache-2.0. The root [NOTICE](../../NOTICE) records the source
relocation and relicensing grant; [upstream sources](../../docs/UPSTREAMS.md)
and [third-party notices](../../THIRD-PARTY-NOTICES.md) accompany each release.
