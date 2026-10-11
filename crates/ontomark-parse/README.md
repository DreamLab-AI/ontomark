# ontomark-parse

Lossless Markdown parsing for Obsidian documents. The document retains original
bytes, frontmatter spans, body links and footnotes; guarded patches apply only
when the source still matches their expected digest.

```rust
use ontomark_parse::Document;
let document = Document::parse("---\ntype: Class\n---\nA [[linked]] page.\n");
assert_eq!(document.source(), "---\ntype: Class\n---\nA [[linked]] page.\n");
```

The parser does not assign ontology identity, entity kinds or predicate meaning.
Use `ontomark-model` with an explicit profile for that layer. The optional
`serde` feature derives serialisation for the public document types.

Licensed under MIT OR Apache-2.0. See `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
