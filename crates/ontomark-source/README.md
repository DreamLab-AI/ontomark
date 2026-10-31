# ontomark-source

The Source port for Ontomark ingest, specified by
[ADR-OM-010](../../docs/adr/ADR-OM-010-external-source-ingest-lands-as-proposals.md).
It mirrors VisionClaw's `CorpusSource` (`describe`, `list`, `fetch`, a change
marker per item) so one port shape serves the estate.

- `LocalFiles`: a directory or one file. Markdown and plain text verbatim (CRLF
  to LF, BOM stripped); HTML rendered to Markdown so headings become `#`
  headings (script, style, nav and similar chrome dropped, entities decoded).
  Dot-files and dot-directories (`.obsidian/`, `.trash/`) and symlinks are skipped.
- `Url` (cargo feature `http`, off by default): blocking `ureq` GET with a
  timeout and a body limit (5 MiB by default); `Content-Type` picks the
  renderer; `ETag` or `Last-Modified` is the change marker. No cookies, no
  credentials. Without the feature the crate has no network code.
- Anything else (PDF, office documents, images) is refused with
  `SourceError::UnsupportedMediaType`, never guessed.
- `Fetched::version` is the lowercase hex SHA-256 of the rendered text, so a
  rename or touch is not a re-read.
- `Ledger` and `classify` report `New`, `ContentChanged`, `ParserChanged`,
  `MarkerOnly`, `Unchanged`, `Removed` (and `NeedsFetch` when a fetch is needed
  to decide). `Ledger::can_skip` is the cheap pre-check.

```rust
use ontomark_source::{LocalFiles, Source};

let source = LocalFiles::new("notes");
for item in source.list()? {
    let fetched = source.fetch(&item)?;
    println!("{} {}", fetched.item_id, fetched.version);
}
# Ok::<(), ontomark_source::SourceError>(())
```

Licensed under MIT OR Apache-2.0.
