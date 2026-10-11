# ontomark-wasm

Portable ontology engine for vault hosts. It indexes supplied Markdown files,
classifies supported OWL 2 EL axioms, evaluates local read-only SPARQL and
proposes guarded frontmatter patches. The host supplies file contents and owns
approved writes; the engine makes no network requests.

The native `engine::Engine::handle_json` API and JavaScript `WasmEngine.handle`
binding share the [JSON protocol](../../docs/api/WP-E.md). Requests carry a
generation; stale reads and patches are refused. Queries cannot use UPDATE,
SERVICE or external dataset clauses. Standard functions such as NOW, RAND and
UUID use runtime clock or entropy and can return different values across runs.

The crate-level [rustdoc example](src/lib.rs) demonstrates loading two classes,
querying their relation and refusing a stale request. Generic is the default
profile; estate vocabulary is an explicit choice. Unsupported reasoner input
and budget fallbacks are reported as incomplete classification.

The engine's document-profile adapters remain in place. Compiler 0.1.3 has a
separate typed `model-projection` API; adopting it here needs explicit profile
mapping and native/WASM wire parity.

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
