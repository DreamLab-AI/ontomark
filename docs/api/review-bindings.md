# Content-bound review and provenance

VisionFlow ADR-2024 separates content attribution, compilation, logical derivation,
human review and governance admission. Ontomark implements the portable contract in
`ontomark-parse`; compiler, ingest, CLI and WASM use that implementation.

`generated` and `verified` are source records. Missing attribution stays unknown.
The generation marker's optional `built_by: {name, version}` records the compiler,
not a content author, reviewer or governance signer. Legacy markers omit it.
Existing derivation certificates and admission receipts retain their own meanings.

Prepare an explicit review with:

```sh
ontomark --repo /path/to/vault --json review-binding Page --key definition --key is-a
```

Omit `--key` for the whole page. The command reads the exact source and writes
nothing. A human reviewer can add the returned digest to their dated stamp:

```yaml
verified:
  - by: human:npub1...
    at: 2026-11-15T12:00:00Z
    keys: [definition, is-a]
    content_digest: sha256-v1:<64 lowercase hexadecimal digits>
```

A digest is an integrity binding, not a signature or evidence of the actor's identity.
Build, ordinary editing and assisted ingest never attach or refresh these stamps.

`api/v3/pages/<slug>.json` adds `reviewBinding` with `current`, `stale` and `unbound`
stamp counts. It uses `context/v3.jsonld` (also `ns/v3.jsonld`). Legacy page APIs and
v1/v2 contexts retain their existing meanings. The scaffold index carries the same
counts as `review_binding`, plus explicit source attestations. The typed reader
preserves missing legacy metadata as unknown. Counts describe the original source,
not the bytes of a redacted public projection. They do not mean every page field was
reviewed: consult each stamp's scope. Public projection still excludes private pages
and redacts private material; it never exports whole source content as a digest input.

The legacy `certification` block describes historical ingest protection. Current,
stale and unbound human stamps all retain that protection. Malformed scopes cannot
claim current review, but identifiable historical keys remain protected. Agent and
process stamps do not grant human certification. Invalid bindings remain stale;
history is preserved for a reviewer to resolve.

## Digest v1

SHA-256 is provided by RustCrypto `sha2`, domain-separated by the bytes
`ontomark-review-v1\0`. Values use type tags and big-endian u64 length framing.
Null, boolean, i64, float spelling, string, sequence and map have distinct encodings.
Maps and selected keys are sorted; sequences preserve order. Missing keys differ
from explicit nulls. Floating-point spelling is retained so high-precision values
never collapse through an intermediate f64 conversion. A spelling-only float edit
therefore conservatively invalidates review.

Key-scoped digests cover exactly the selected values. Whole-page digests cover all
frontmatter except `verified`, and the exact body. Adding a review stamp does not
invalidate prior whole-page review. Unsupported or malformed frontmatter is refused.
An empty scope or a scope including `verified` is invalid. The `sha256-v1:` prefix
identifies this encoding; changing it requires a new version and conformance fixtures.

## Portable API and verification

Rust callers use `certification::{content_digest, assess}`. Browser callers use
`ontomark_wasm.review_binding(source, keys_json)`, with JSON `null` for whole-page
review or a non-empty array of keys. Both return information without changing source.

Native conformance tests cover precise floats, nested maps, scalar types, missing
values, sequence order, changed bodies, scoped changes, malformed stamps, actor
refusal, mixed review history and ingest protection. CLI tests exercise actual public
exports and private canaries. `scripts/test-review-binding-wasm.cjs` compares native
CLI digests against the built WebAssembly module and checks current/stale controls.
