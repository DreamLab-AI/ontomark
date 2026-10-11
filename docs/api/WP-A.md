# WP-A API: ontomark-parse and ontomark-model

Owner: `spec-model`. Branch `wp-a-model-parse`. Status per crate below; code
against these signatures. Full rustdoc: `cargo doc -p ontomark-parse -p ontomark-model --no-deps --open`.

## ontomark-parse — STABLE (commit 7176583)

Owns the Markdown format. Portable (no fs/clock/net/threads); builds for
`wasm32-unknown-unknown`. Feature `serde` derives Serialize/Deserialize on the
data types (enable it for wasm bindings).

```rust
// Reading
let doc = Document::parse(text);              // infallible, lossless
doc.source() -> &str                          // == input, always
doc.digest() -> SourceDigest                  // sha256 of source; Display "sha256:<hex>"
doc.frontmatter() -> Option<&Frontmatter>
doc.value(key) -> Option<&Value>              // top-level frontmatter value
doc.body() -> &str; doc.body_span() -> Span; doc.body_line() -> usize (1-based)
doc.body_index() -> &BodyIndex                // links, footnote_refs, footnote_defs, inline_footnotes, code
doc.line_col(byte_offset) -> (line, col)      // 1-based, col in chars

Frontmatter { open_fence, content, close_fence: Span, state: FrontmatterState,
              entries: Vec<Entry>, gaps: Vec<Span>, style: BlockStyle }
  .get(key) -> Option<&Entry>; .value(key); .has(key); .keys(); .is_parsed()
FrontmatterState::{Parsed, Unsupported(String), Malformed(String)}
Entry { key, key_span, span /* whole lines */, value: Value, items: Vec<Span> /* block-seq items */ }
  .raw(source) -> &str
  .wikilinks(source) -> Vec<FrontmatterLink { link, item, span, recovered }>
Value::{Null, Bool, Int(i64), Float(String /*lexical*/), String, Seq(Vec<Value>), Map(IndexMap<String,Value>)}
  .as_str/.as_bool/.as_f64/.as_seq/.as_map/.scalar_text/.texts/.type_name
Wikilink { target, subpath: Option<Subpath::{Heading,Block}>, alias, embed } ; Wikilink::parse("[[..]]")
BodyLink { span, link }; FootnoteRef { span, label }; FootnoteDef { span, label, text }
Span { start, end } (byte offsets into the document source)

// Writing — the only write path
let patch = Patch::new(doc.digest())
    .set("key", value)                        // in place if present, else Placement::End
    .set_at("key", value, Placement::After("title".into()))  // or ::Start / ::End
    .remove("key")
    .replace_body(span, expected_text, replacement);
let Applied { document, receipt } = doc.apply(&patch)?;  // Result<Applied, PatchError>
Receipt { before, after: SourceDigest, edits: Vec<Edit { op, before: Span, after: Span }>, preserved_bytes }
PatchError::{DigestMismatch, FrontmatterLocked, AnchorNotFound, InvalidKey, Overlap, BadRange, BodyGuard, Verification}
emit_entry(key, &value, &BlockStyle) -> String; emit_scalar(&str) -> String   // house-style YAML
```

Guarantees (tested): every byte outside the edits is identical; untouched keys
keep exact bytes and values; set keys read back equal; existing keys are never
reordered; column-0 comments belong to no key and survive single-key edits;
no-op sets/removes produce zero edits (idempotent); a body patch can neither
touch nor create frontmatter; Unsupported/Malformed blocks round-trip but
refuse key patches. Values are emitted in the corpus's serde_yaml style
(indentless `- '[[X]]'`, `[]`, single quotes), following the block's CRLF and
sequence indentation.

EXP-03 (2026-10-11, visionGraph b5e36abd1): 9,244/9,244 public pages
round-trip, match an independent yaml-rust2 load, and pass five guarded
patches each. Run: `just exp03 ../visionGraph/knowledge/pages`.

## ontomark-model — IN PROGRESS

Signatures land here when stable; see the next section revision.
