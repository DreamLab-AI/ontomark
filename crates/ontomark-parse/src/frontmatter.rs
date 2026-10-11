//! The frontmatter block: fences, top-level entries as byte spans, and the
//! typed value of each entry.
//!
//! Entry boundaries come from a real YAML parser (yaml-rust2's event stream),
//! not from a line heuristic: the parser reports where each top-level key
//! starts, and an entry owns the bytes from its key's line to the next key's
//! line, less any trailing blank lines and column-0 comments, which are kept
//! as [`Frontmatter::gaps`]. The entries and gaps tile the block exactly, so
//! the block is reproduced byte-for-byte by construction.

use indexmap::IndexMap;
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};
use yaml_rust2::Yaml;

use crate::span::Span;
use crate::value::{Value, Wikilink};

/// Whether the block's YAML could be read, and whether it may be patched.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "t", content = "reason", rename_all = "lowercase")
)]
pub enum FrontmatterState {
    /// A mapping of plain keys; entries and values are available and the
    /// block may be patched.
    Parsed,
    /// Valid YAML using a construct this crate does not patch (anchors,
    /// aliases, tags, complex or flow-mapping keys, several documents). The
    /// bytes round-trip; patches touching the block are refused.
    Unsupported(String),
    /// Not valid YAML, or not a mapping, or a duplicated key. The bytes
    /// round-trip; patches touching the block are refused.
    Malformed(String),
}

/// One top-level `key: value` entry.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entry {
    /// The key as YAML reads it (quotes removed).
    pub key: String,
    /// The key token in the source, quotes included.
    pub key_span: Span,
    /// Whole lines from the key's line through the value's last line,
    /// including the final line break.
    pub span: Span,
    /// The typed value.
    pub value: Value,
    /// For a block or flow sequence value: the source span of each item's
    /// text (after `- `), trailing whitespace trimmed. Empty otherwise.
    pub items: Vec<Span>,
}

impl Entry {
    /// The exact source text of this entry.
    #[must_use]
    pub fn raw<'a>(&self, source: &'a str) -> &'a str {
        self.span.slice(source)
    }

    /// The wikilinks in this entry's value, one per sequence item (or the
    /// scalar) that is exactly one `[[…]]` token.
    ///
    /// An item written unquoted, `- [[A|B]]`, is a nested flow sequence to
    /// YAML, not a string; it is recovered from the item's source text and
    /// marked [`FrontmatterLink::recovered`], so it is neither dropped nor
    /// silently treated as well-formed.
    #[must_use]
    pub fn wikilinks(&self, source: &str) -> Vec<FrontmatterLink> {
        let mut out = Vec::new();
        match &self.value {
            Value::Seq(items) => {
                for (i, item) in items.iter().enumerate() {
                    let span = self.items.get(i).copied();
                    if let Some(link) = item.as_str().and_then(Wikilink::parse) {
                        out.push(FrontmatterLink {
                            link,
                            item: Some(i),
                            span,
                            recovered: false,
                        });
                    } else if matches!(item, Value::Seq(_)) {
                        if let Some(link) = span.and_then(|s| Wikilink::parse(s.slice(source))) {
                            out.push(FrontmatterLink {
                                link,
                                item: Some(i),
                                span,
                                recovered: true,
                            });
                        }
                    }
                }
            }
            Value::String(s) => {
                if let Some(link) = Wikilink::parse(s) {
                    out.push(FrontmatterLink {
                        link,
                        item: None,
                        span: None,
                        recovered: false,
                    });
                }
            }
            _ => {}
        }
        out
    }
}

/// A wikilink found in a frontmatter value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrontmatterLink {
    /// The link.
    pub link: Wikilink,
    /// Index of the sequence item it came from; `None` for a scalar value.
    pub item: Option<usize>,
    /// Source span of the item, when known.
    pub span: Option<Span>,
    /// `true` when the item was an unquoted `[[…]]` that YAML reads as a
    /// nested sequence and the link was recovered from its source text.
    pub recovered: bool,
}

/// Indentation conventions observed in a block, used when emitting a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockStyle {
    /// Spaces before `-` in a top-level key's block sequence (`0` is the
    /// indentless `key:\n- item` form).
    pub seq_indent: usize,
    /// Spaces before a nested mapping key.
    pub map_indent: usize,
    /// `true` when the block's lines end in `\r\n`.
    pub crlf: bool,
}

impl Default for BlockStyle {
    fn default() -> Self {
        Self {
            seq_indent: 0,
            map_indent: 2,
            crlf: false,
        }
    }
}

/// A `---`-fenced YAML frontmatter block at the very start of a page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frontmatter {
    /// The opening `---` line, line break included.
    pub open_fence: Span,
    /// The YAML text between the fences.
    pub content: Span,
    /// The closing `---` line, line break included when present.
    pub close_fence: Span,
    /// Parse state.
    pub state: FrontmatterState,
    /// Top-level entries in source order (empty unless [`FrontmatterState::Parsed`]).
    pub entries: Vec<Entry>,
    /// Bytes inside [`Self::content`] that belong to no entry: blank lines
    /// and column-0 comments before, between and after entries.
    pub gaps: Vec<Span>,
    /// Observed indentation and line-ending conventions.
    pub style: BlockStyle,
}

impl Frontmatter {
    /// The entry for `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// The value under `key`.
    #[must_use]
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.get(key).map(|e| &e.value)
    }

    /// `true` when `key` is present.
    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Keys in author order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.key.as_str())
    }

    /// `true` when the block was parsed and can be patched.
    #[must_use]
    pub fn is_parsed(&self) -> bool {
        self.state == FrontmatterState::Parsed
    }
}

/// Locate the frontmatter fences. `offset` is where the page proper begins
/// (after a BOM). Returns `(open, content, close)` spans.
pub(crate) fn locate(source: &str, offset: usize) -> Option<(Span, Span, Span)> {
    let text = &source[offset..];
    let open_len = if text.starts_with("---\n") {
        4
    } else if text.starts_with("---\r\n") {
        5
    } else {
        return None;
    };
    let content_start = offset + open_len;
    let mut line_start = content_start;
    while line_start < source.len() {
        let rest = &source[line_start..];
        let (line, next) = match rest.find('\n') {
            Some(i) => (&rest[..i], line_start + i + 1),
            None => (rest, source.len()),
        };
        if line.trim_end_matches('\r') == "---" {
            return Some((
                Span::new(offset, content_start),
                Span::new(content_start, line_start),
                Span::new(line_start, next),
            ));
        }
        line_start = next;
    }
    None
}

/// Parse the block whose fences [`locate`] found.
pub(crate) fn parse_block(source: &str, open: Span, content: Span, close: Span) -> Frontmatter {
    let text = content.slice(source);
    let crlf = text.contains("\r\n") || source[open.start..open.end].ends_with("\r\n");
    let mut fm = Frontmatter {
        open_fence: open,
        content,
        close_fence: close,
        state: FrontmatterState::Parsed,
        entries: Vec::new(),
        gaps: Vec::new(),
        style: BlockStyle {
            crlf,
            ..BlockStyle::default()
        },
    };
    match read_events(text) {
        Ok(Read::Empty) => {
            if !text.is_empty() {
                fm.gaps.push(content);
            }
        }
        Ok(Read::Entries(raw)) => build_entries(&mut fm, source, content, raw),
        Err(Problem::Unsupported(r)) => fm.state = FrontmatterState::Unsupported(r),
        Err(Problem::Malformed(r)) => fm.state = FrontmatterState::Malformed(r),
    }
    fm
}

enum Problem {
    Unsupported(String),
    Malformed(String),
}

enum Read {
    Empty,
    Entries(Vec<RawEntry>),
}

struct RawEntry {
    key: String,
    /// Byte offset of the key token within the block content.
    key_at: usize,
    value: Value,
    /// Byte offsets (within content) of each top-level sequence item's start.
    item_at: Vec<usize>,
    /// Byte offsets of the first nested mapping key, if the value is a mapping.
    child_key_at: Option<usize>,
}

struct Events<'a> {
    parser: Parser<std::str::Chars<'a>>,
    /// `char_to_byte[i]` = byte offset of char `i`; one extra entry for EOF.
    char_to_byte: Vec<usize>,
}

impl Events<'_> {
    fn next(&mut self) -> Result<(Event, usize), Problem> {
        let (ev, mark) = self
            .parser
            .next_token()
            .map_err(|e| Problem::Malformed(e.to_string()))?;
        Ok((ev, self.byte(mark)))
    }

    fn byte(&self, mark: Marker) -> usize {
        let i = mark.index();
        self.char_to_byte
            .get(i)
            .copied()
            .unwrap_or_else(|| *self.char_to_byte.last().unwrap_or(&0))
    }
}

fn read_events(text: &str) -> Result<Read, Problem> {
    let mut char_to_byte: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
    char_to_byte.push(text.len());
    let mut ev = Events {
        parser: Parser::new_from_str(text),
        char_to_byte,
    };
    let (first, _) = ev.next()?;
    if first != Event::StreamStart {
        return Err(Problem::Malformed("expected a YAML stream".into()));
    }
    let (doc, _) = ev.next()?;
    match doc {
        Event::StreamEnd => return Ok(Read::Empty),
        Event::DocumentStart => {}
        other => return Err(Problem::Malformed(format!("unexpected event {other:?}"))),
    }
    let (top, _) = ev.next()?;
    let entries = match top {
        Event::MappingStart(anchor, tag) => {
            if anchor != 0 || tag.is_some() {
                return Err(Problem::Unsupported(
                    "anchor or tag on the frontmatter mapping".into(),
                ));
            }
            read_top_mapping(&mut ev)?
        }
        Event::Scalar(ref s, TScalarStyle::Plain, 0, None)
            if s.is_empty() || s == "~" || s == "null" =>
        {
            // A block holding only comments or whitespace.
            let (end, _) = ev.next()?;
            if end != Event::DocumentEnd {
                return Err(Problem::Malformed(
                    "unexpected content after a null document".into(),
                ));
            }
            expect_stream_end(&mut ev)?;
            return Ok(Read::Empty);
        }
        other => {
            return Err(Problem::Malformed(format!(
                "frontmatter must be a mapping, found {}",
                event_kind(&other)
            )))
        }
    };
    let (end, _) = ev.next()?;
    if end != Event::DocumentEnd {
        return Err(Problem::Malformed(format!("unexpected event {end:?}")));
    }
    expect_stream_end(&mut ev)?;
    Ok(Read::Entries(entries))
}

fn expect_stream_end(ev: &mut Events<'_>) -> Result<(), Problem> {
    match ev.next()?.0 {
        Event::StreamEnd => Ok(()),
        Event::DocumentStart => Err(Problem::Unsupported("more than one YAML document".into())),
        other => Err(Problem::Malformed(format!("unexpected event {other:?}"))),
    }
}

fn event_kind(e: &Event) -> &'static str {
    match e {
        Event::Scalar(..) => "a scalar",
        Event::SequenceStart(..) => "a sequence",
        Event::MappingStart(..) => "a mapping",
        Event::Alias(_) => "an alias",
        _ => "an unexpected event",
    }
}

fn read_top_mapping(ev: &mut Events<'_>) -> Result<Vec<RawEntry>, Problem> {
    let mut out: Vec<RawEntry> = Vec::new();
    loop {
        let (key_ev, key_at) = ev.next()?;
        let key = match key_ev {
            Event::MappingEnd => return Ok(out),
            Event::Scalar(s, _, 0, None) => s,
            Event::Scalar(..) => return Err(Problem::Unsupported("anchor or tag on a key".into())),
            _ => return Err(Problem::Unsupported("complex (non-scalar) key".into())),
        };
        if out.iter().any(|e| e.key == key) {
            return Err(Problem::Malformed(format!("duplicate key `{key}`")));
        }
        let (val_ev, val_at) = ev.next()?;
        let mut item_at = Vec::new();
        let mut child_key_at = None;
        let value = match val_ev {
            Event::SequenceStart(0, None) => {
                let mut items = Vec::new();
                loop {
                    let (e, at) = ev.next()?;
                    if e == Event::SequenceEnd {
                        break;
                    }
                    item_at.push(at);
                    items.push(read_node(ev, e)?);
                }
                Value::Seq(items)
            }
            Event::MappingStart(0, None) => {
                let mut map = IndexMap::new();
                loop {
                    let (e, at) = ev.next()?;
                    if e == Event::MappingEnd {
                        break;
                    }
                    child_key_at.get_or_insert(at);
                    insert_pair(ev, &mut map, e)?;
                }
                Value::Map(map)
            }
            other => read_node(ev, other)?,
        };
        let _ = val_at;
        out.push(RawEntry {
            key,
            key_at,
            value,
            item_at,
            child_key_at,
        });
    }
}

fn insert_pair(
    ev: &mut Events<'_>,
    map: &mut IndexMap<String, Value>,
    key_ev: Event,
) -> Result<(), Problem> {
    let key = match key_ev {
        Event::Scalar(s, _, 0, None) => s,
        Event::Scalar(..) => return Err(Problem::Unsupported("anchor or tag on a key".into())),
        _ => return Err(Problem::Unsupported("complex (non-scalar) key".into())),
    };
    let (v, _) = ev.next()?;
    let value = read_node(ev, v)?;
    if map.insert(key.clone(), value).is_some() {
        return Err(Problem::Malformed(format!("duplicate key `{key}`")));
    }
    Ok(())
}

fn read_node(ev: &mut Events<'_>, first: Event) -> Result<Value, Problem> {
    match first {
        Event::Scalar(s, style, anchor, tag) => {
            if anchor != 0 || tag.is_some() {
                return Err(Problem::Unsupported("anchor or tag on a value".into()));
            }
            Ok(if style == TScalarStyle::Plain {
                resolve_plain(&s)
            } else {
                Value::String(s)
            })
        }
        Event::SequenceStart(anchor, tag) => {
            if anchor != 0 || tag.is_some() {
                return Err(Problem::Unsupported("anchor or tag on a sequence".into()));
            }
            let mut items = Vec::new();
            loop {
                let (e, _) = ev.next()?;
                if e == Event::SequenceEnd {
                    return Ok(Value::Seq(items));
                }
                items.push(read_node(ev, e)?);
            }
        }
        Event::MappingStart(anchor, tag) => {
            if anchor != 0 || tag.is_some() {
                return Err(Problem::Unsupported("anchor or tag on a mapping".into()));
            }
            let mut map = IndexMap::new();
            loop {
                let (e, _) = ev.next()?;
                if e == Event::MappingEnd {
                    return Ok(Value::Map(map));
                }
                insert_pair(ev, &mut map, e)?;
            }
        }
        Event::Alias(_) => Err(Problem::Unsupported("YAML alias".into())),
        other => Err(Problem::Malformed(format!("unexpected event {other:?}"))),
    }
}

/// Core-schema resolution of a plain scalar.
pub(crate) fn resolve_plain(s: &str) -> Value {
    match Yaml::from_str(s) {
        Yaml::Null => Value::Null,
        Yaml::Boolean(b) => Value::Bool(b),
        Yaml::Integer(i) => Value::Int(i),
        Yaml::Real(r) => Value::Float(r),
        _ => match s {
            // YAML 1.2 core schema spellings yaml-rust2 leaves as strings.
            "Null" | "NULL" => Value::Null,
            _ => Value::String(s.to_owned()),
        },
    }
}

fn line_start_of(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// Pull `end` back over trailing blank lines and column-0 comment lines that
/// sit between `start` and `end`, returning the new end.
fn trim_trailing_trivia(text: &str, start: usize, end: usize) -> usize {
    let mut cut = end;
    loop {
        if cut <= start {
            return cut;
        }
        // The line ending just before `cut`.
        let prev_line_start = text[..cut - 1].rfind('\n').map_or(0, |i| i + 1).max(start);
        if prev_line_start == start {
            // Never trim the key line itself.
            return cut;
        }
        let line = text[prev_line_start..cut].trim_end_matches(['\n', '\r']);
        if line.trim().is_empty() || line.starts_with('#') {
            cut = prev_line_start;
        } else {
            return cut;
        }
    }
}

fn build_entries(fm: &mut Frontmatter, source: &str, content: Span, raw: Vec<RawEntry>) {
    let text = content.slice(source);
    let base = content.start;
    // Every top-level key must start its own line.
    for r in &raw {
        if line_start_of(text, r.key_at) != r.key_at {
            fm.state = FrontmatterState::Unsupported(format!(
                "key `{}` does not start a line (flow mapping or indented block)",
                r.key
            ));
            return;
        }
    }
    let mut cursor = 0;
    let mut style_seq: Option<usize> = None;
    let mut style_map: Option<usize> = None;
    for (i, r) in raw.iter().enumerate() {
        let next_start = raw.get(i + 1).map_or(text.len(), |n| n.key_at);
        let end = trim_trailing_trivia(text, r.key_at, next_start);
        if r.key_at > cursor {
            fm.gaps.push(Span::new(base + cursor, base + r.key_at));
        }
        let key_end = key_token_end(text, r.key_at);
        // Item spans only for block sequences (first item on a later line);
        // flow sequences share a line and gain nothing from item spans.
        let block_seq = r
            .item_at
            .first()
            .is_some_and(|&first| line_start_of(text, first) > r.key_at);
        let items = if block_seq {
            item_spans(text, &r.item_at, end)
        } else {
            Vec::new()
        };
        if style_seq.is_none() {
            if let Some(&first) = r.item_at.first() {
                let ls = line_start_of(text, first);
                let dash = &text[ls..first];
                if let Some(pos) = dash.find('-') {
                    if dash[..pos].chars().all(|c| c == ' ') && ls > r.key_at {
                        style_seq = Some(pos);
                    }
                }
            }
        }
        if style_map.is_none() {
            if let Some(at) = r.child_key_at {
                let ls = line_start_of(text, at);
                if ls > r.key_at && text[ls..at].chars().all(|c| c == ' ') {
                    style_map = Some(at - ls);
                }
            }
        }
        fm.entries.push(Entry {
            key: r.key.clone(),
            key_span: Span::new(base + r.key_at, base + key_end),
            span: Span::new(base + r.key_at, base + end),
            value: r.value.clone(),
            items: items.into_iter().map(|s| s.shift(base)).collect(),
        });
        cursor = end;
    }
    if cursor < text.len() {
        fm.gaps.push(Span::new(base + cursor, base + text.len()));
    }
    if let Some(s) = style_seq {
        fm.style.seq_indent = s;
    }
    if let Some(m) = style_map {
        fm.style.map_indent = m.max(1);
    }
}

/// End of the key token starting at `at` (exclusive), before the `:`.
fn key_token_end(text: &str, at: usize) -> usize {
    let rest = &text[at..];
    let bytes = rest.as_bytes();
    match bytes.first() {
        Some(b'\'') => {
            let mut i = 1;
            while i < bytes.len() {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                        continue;
                    }
                    return at + i + 1;
                }
                i += 1;
            }
            at + rest.len()
        }
        Some(b'"') => {
            let mut i = 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i += 2,
                    b'"' => return at + i + 1,
                    _ => i += 1,
                }
            }
            at + rest.len()
        }
        _ => {
            let line_end = rest.find('\n').unwrap_or(rest.len());
            let line = &rest[..line_end];
            let mut search = 0;
            while let Some(p) = line[search..].find(':') {
                let idx = search + p;
                let after = line.as_bytes().get(idx + 1);
                if matches!(after, None | Some(b' ' | b'\t' | b'\r')) {
                    return at + line[..idx].trim_end().len();
                }
                search = idx + 1;
            }
            at + line.trim_end().len()
        }
    }
}

fn item_spans(text: &str, item_at: &[usize], entry_end: usize) -> Vec<Span> {
    let mut out = Vec::with_capacity(item_at.len());
    for (i, &at) in item_at.iter().enumerate() {
        let limit = item_at
            .get(i + 1)
            .map_or(entry_end, |&next| line_start_of(text, next).max(at));
        let raw = &text[at..limit.max(at)];
        // A flow-sequence item ends at its `,` or `]`; a block item at its
        // line. Keep only the item's own line(s), trimmed.
        let trimmed = raw.trim_end();
        out.push(Span::new(at, at + trimmed.len()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(src: &str) -> Frontmatter {
        let (o, c, cl) = locate(src, 0).expect("fences");
        parse_block(src, o, c, cl)
    }

    #[test]
    fn entries_and_gaps_tile_the_content() {
        let src = "---\n# lead\na: 1\n\nb:\n- x\n# between\nc: [1, 2]\n# tail\n---\nbody\n";
        let f = fm(src);
        assert_eq!(f.state, FrontmatterState::Parsed);
        let mut spans: Vec<Span> = f
            .entries
            .iter()
            .map(|e| e.span)
            .chain(f.gaps.iter().copied())
            .collect();
        spans.sort();
        let mut at = f.content.start;
        for s in &spans {
            assert_eq!(s.start, at, "gap in tiling at {at}");
            at = s.end;
        }
        assert_eq!(at, f.content.end);
        assert_eq!(f.get("b").unwrap().raw(src), "b:\n- x\n");
        assert_eq!(
            f.gaps.iter().map(|g| g.slice(src)).collect::<Vec<_>>(),
            vec!["# lead\n", "\n", "# between\n", "# tail\n"]
        );
    }

    #[test]
    fn duplicate_keys_are_malformed() {
        assert!(matches!(
            fm("---\na: 1\na: 2\n---\n").state,
            FrontmatterState::Malformed(_)
        ));
    }

    #[test]
    fn anchors_are_unsupported() {
        assert!(matches!(
            fm("---\na: &x 1\nb: *x\n---\n").state,
            FrontmatterState::Unsupported(_)
        ));
    }

    #[test]
    fn flow_mapping_block_is_unsupported() {
        assert!(matches!(
            fm("---\n{a: 1, b: 2}\n---\n").state,
            FrontmatterState::Unsupported(_)
        ));
    }

    #[test]
    fn non_mapping_is_malformed() {
        assert!(matches!(
            fm("---\n- a\n---\n").state,
            FrontmatterState::Malformed(_)
        ));
    }

    #[test]
    fn comment_only_block_is_empty_and_parsed() {
        let f = fm("---\n# nothing\n---\n");
        assert_eq!(f.state, FrontmatterState::Parsed);
        assert!(f.entries.is_empty());
        assert_eq!(f.gaps.len(), 1);
    }

    #[test]
    fn unicode_keys_and_values_get_byte_spans() {
        let src = "---\ntítulo: «Café»\nnext: ok\n---\n";
        let f = fm(src);
        assert_eq!(f.get("next").unwrap().raw(src), "next: ok\n");
        assert_eq!(f.get("título").unwrap().key_span.slice(src), "título");
    }

    #[test]
    fn style_is_observed() {
        let f = fm("---\na:\n  - x\nm:\n    k: v\n---\n");
        assert_eq!(f.style.seq_indent, 2);
        assert_eq!(f.style.map_indent, 4);
    }
}
