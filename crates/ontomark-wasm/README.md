# ontomark-wasm

The portable ontology engine for vault hosts such as the
[Obsidian plugin](https://github.com/DreamLab-AI/ontomark-obsidian). Given the
text of a vault's Markdown files it builds the index, classifies the supported
OWL 2 EL axioms, answers local read-only SPARQL, proposes guarded frontmatter
patches and runs the assisted-ingest plan/accept loop. It makes no network
requests and never writes: the host owns the files and every approved write.

Both entry points — native `engine::Engine::handle_json` and the
`wasm-bindgen` `WasmEngine.handle` — speak the same
[JSON protocol](../../docs/api/WP-E.md), and the transcript test proves they
return the same bytes.

## How a host feeds it

The host reads the vault and sends `FileText` records — a vault-relative,
`/`-separated path with its extension, and the exact file contents:

```json
{"kind": "load", "files": [
  {"path": "Ontology/Animal.md", "text": "---\ntype: class\n---\n"},
  {"path": "Ontology/Dog.md", "text": "---\ntype: class\nsubclass-of: [\"[[Animal]]\"]\n---\n"}
]}
```

`load` replaces the whole index; `update` replaces or removes single files.
Non-Markdown files are ignored. Every answer carries a **generation** number,
and a request that names a stale generation is refused rather than answered
from the wrong index; patches are proposed against the generation they were
computed on.

### Excluding furniture is the host's job

A plain Obsidian vault carries folders that are not notes — `templates/`
(Templater files such as `type: <% tp.file.title %>`, which is valid YAML and
would index as a note of that literal type), `bases/`, `assets/`,
`.obsidian/`, `.trash/`. The engine does **not** filter them: it has no view
of the vault beyond the paths it is handed, and the right list is a per-vault
setting the host already holds (Obsidian's own "excluded files", the plugin's
settings, a profile's `layout.excluded` for the compiler). Filtering inside
the engine would mean a second, hidden exclusion list that could disagree with
the host's, and a note that vanished with no diagnostic. A file the host does
send is indexed, and anything it cannot parse is reported
(`frontmatter_unreadable`), never dropped silently. The compiler's walk and the
CLI apply the exclusion list on disk
([ADR-OM-007](../../docs/adr/ADR-OM-007-vault-root-is-the-content-root.md));
the engine sees only what the host chose to send.

## What it answers

- `index`: the typed entities, their IRIs, labels, aliases and parents, with
  diagnostics; the generic profile is the default and the estate vocabulary an
  explicit choice.
- `classify`: subsumptions, equivalences, unsatisfiable classes, and every
  axiom the reasoner could not take, named ([ADR-OM-004](../../docs/adr/ADR-OM-004-owl2-el-as-the-local-reasoning-profile.md)).
  An incomplete classification says so.
- `query`: read-only SPARQL over the asserted and inferred graph; `UPDATE`,
  `SERVICE` and external datasets are refused. `NOW`, `RAND` and `UUID` use the
  runtime clock and entropy.
- `propose_patch`: a frontmatter change as a unified diff the host previews and
  applies.
- `ingest_plan` / `ingest_accept` ([ADR-OM-003](../../docs/adr/ADR-OM-003-native-assisted-ingest.md)):
  the engine never calls a model. `ingest_plan` returns the request the host
  sends to a model it controls; `ingest_accept` grounds the replies against
  the vault, demands a located quote for each, and returns candidates, a
  withheld list and a proven patch preview.

Each engine profile also implements `ontomark_model::Profile`; ingest reads the
vault through it and `tests/profile_parity.rs` proves both sides index the
same entities.

## Build and use

From the workspace root:

```bash
cargo test --locked -p ontomark-wasm
RUSTDOCFLAGS='-D warnings' cargo doc --locked -p ontomark-wasm --no-deps
cargo build --locked -p ontomark-wasm --target wasm32-unknown-unknown --release
```

Each [release](https://github.com/DreamLab-AI/ontomark/releases) ships the
generated browser and Node glue, the `.wasm` and provenance receipts; the
plugin embeds that pinned engine, so its users install a ZIP without Rust or
Node.

MIT OR Apache-2.0. See the root [NOTICE](../../NOTICE),
[upstream sources](../../docs/UPSTREAMS.md) and
[third-party notices](../../THIRD-PARTY-NOTICES.md).
