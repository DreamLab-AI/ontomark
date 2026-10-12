# ontomark-wasm

Portable ontology engine for vault hosts. It indexes supplied Markdown files,
classifies supported OWL 2 EL axioms, evaluates local read-only SPARQL and
proposes guarded frontmatter patches. The host supplies file contents and owns
approved writes; the engine makes no network requests.

Both entry points, native `engine::Engine::handle_json` and JavaScript `WasmEngine.handle`,
binding share the [JSON protocol](../../docs/api/WP-E.md). Requests carry a
generation; stale reads and patches are refused. Queries cannot use UPDATE,
SERVICE or external dataset clauses. Standard functions such as NOW, RAND and
UUID use runtime clock or entropy and can return different values across runs.

A crate-level [rustdoc example](src/lib.rs) loads two classes,
querying their relation and refusing a stale request. Generic is the default
profile; estate vocabulary is an explicit choice. Unsupported reasoner input
and budget fallbacks are reported as incomplete classification.

Assisted ingest (ADR-OM-003) is two further requests, `ingest_plan` and
`ingest_accept`. The engine never calls a model: `ingest_plan` returns the
request a host sends to a model it controls, and `ingest_accept` turns the
replies into evidence-bearing candidates, a withheld list and a proven patch
preview in the `propose_patch` shape. The transcript fixture exercises both,
so native and WASM answers are byte-equal here too.

For its own axioms the engine keeps its document-profile adapters.
Each engine profile also implements `ontomark_model::Profile`, the explicit
mapping EXP-10 asks for; ingest reads the vault through it, and
`tests/profile_parity.rs` proves both sides index the same entities. Adopting
the typed projection for the engine's axioms still needs whole-transcript
native/WASM wire parity.

## Build and use

From the workspace root:

```bash
cargo test --locked -p ontomark-wasm
RUSTDOCFLAGS='-D warnings' cargo doc --locked -p ontomark-wasm --no-deps
cargo build --locked -p ontomark-wasm --target wasm32-unknown-unknown --release
```

The [engine prerelease](https://github.com/DreamLab-AI/ontomark/releases/tag/v0.1.0-rc.1)
contains generated browser and Node glue, WASM and provenance receipts.
The [standalone Obsidian plugin](https://github.com/DreamLab-AI/ontomark-obsidian)
embeds that pinned engine; users install its ready ZIP without Rust or Node.

MIT OR Apache-2.0. Read the root [NOTICE](../../NOTICE),
[upstream sources](../../docs/UPSTREAMS.md) and
[third-party notices](../../THIRD-PARTY-NOTICES.md); release attribution is scoped
to the exact compiled asset.
