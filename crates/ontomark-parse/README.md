# ontomark-parse

Lossless Markdown document model for Obsidian pages: frontmatter spans, body
links and footnotes, and guarded byte-preserving patches.

This crate owns the Markdown *format* in the ontomark workspace and nothing
else. It does not know what a `type:` key means, does not assign identity and
does not reason. That layer is [`ontomark-model`](https://crates.io/crates/ontomark-model);
keeping it out of the parser is what makes the parser reusable for any vault.

## What a `Document` is

A `Document` keeps the page's exact bytes and indexes them:

- the YAML frontmatter block, split into top-level entries with typed `Value`s
  (string, int, float, bool, null, sequence, map), with comments and blank
  lines kept as gaps so nothing is lost on the way back out;
- the body as opaque text, with an index of wikilinks, embeds, footnote
  references and definitions, and the fenced-code regions where those are
  *not* recognised;
- a content digest, so any later write can prove what it was planned against.

```rust
use ontomark_parse::Document;

let src = "---\ntype: Class\n# reviewed\naliases:\n- KG\n---\nA [[Graph]] page.\n";
let doc = Document::parse(src);
assert_eq!(doc.source(), src);                   // lossless, always
assert_eq!(doc.body_index().links.len(), 1);     // body index: one wikilink
assert!(doc.value("aliases").is_some());         // typed frontmatter value
```

## One write path: `Patch`

Nothing is ever re-rendered from the typed view. The only way to change a
document is a `Patch`, and a patch is a contract:

- it is bound to the digest of the document it was planned against, and refuses
  to apply to anything else (`stale_base`);
- it names exactly the keys or body ranges it changes (`set`, `remove`,
  `replace_body`);
- after applying, it proves that every byte it did *not* name is unchanged and
  returns a `Receipt` saying so.

```rust
use ontomark_parse::{Document, Patch, Value};

let doc = Document::parse("---\ntype: Class\naliases:\n- KG\n---\nBody.\n");
let patch = Patch::new(doc.digest())
    .set("aliases", Value::strings(["KG", "Knowledge graph"]));
let out = doc.apply(&patch).unwrap();
assert_eq!(
    out.document.source(),
    "---\ntype: Class\naliases:\n- KG\n- Knowledge graph\n---\nBody.\n"
);
assert!(out.receipt.preserved_bytes > 0);
```

This is why the Obsidian plugin can show a diff and apply it with confidence:
the write happens only if the file still holds the bytes the diff was computed
from, and the receipt is checkable.

## Spans in two coordinate systems

Every span carries byte offsets and UTF-16 offsets (`Utf16Index`), because the
Rust side thinks in bytes and editors like Obsidian think in UTF-16 code units.
A caller that highlights evidence in an editor uses the UTF-16 pair; a caller
that slices the source uses the byte pair; neither has to convert.

## Features

- `serde` (off by default): `Serialize`/`Deserialize` on the public document
  types, used by the wasm engine's wire protocol.

Builds for native and `wasm32-unknown-unknown`; no I/O, no allocation beyond
the document itself.

Licensed under MIT OR Apache-2.0. See `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
