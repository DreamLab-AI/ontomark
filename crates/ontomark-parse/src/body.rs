//! The page body as opaque text plus an index of the spans tooling cares
//! about: wikilinks and embeds, footnote references and definitions, and the
//! code regions where neither is recognised.
//!
//! The body is never re-rendered. The index only records where things are, so
//! a patch can replace one span and leave every other byte alone.

use crate::fence::Fence;
use crate::span::Span;
use crate::value::Wikilink;

/// A wikilink or embed in the body.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BodyLink {
    /// Source span of the whole token, `!` included for an embed.
    pub span: Span,
    /// The parsed link.
    pub link: Wikilink,
}

/// A footnote reference `[^label]` in running text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FootnoteRef {
    /// Source span of `[^label]`.
    pub span: Span,
    /// The label.
    pub label: String,
}

/// A footnote definition `[^label]: text` at the start of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FootnoteDef {
    /// Source span of the definition's first line, line break excluded.
    pub span: Span,
    /// The label.
    pub label: String,
    /// Source span of the definition text after `]:` on its first line.
    pub text: Span,
}

/// An inline footnote `^[text]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InlineFootnote {
    /// Source span of `^[…]`.
    pub span: Span,
}

/// What the body index found, with spans in document-source coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BodyIndex {
    /// Wikilinks and embeds outside code, in source order.
    pub links: Vec<BodyLink>,
    /// Footnote references outside code.
    pub footnote_refs: Vec<FootnoteRef>,
    /// Footnote definitions outside code.
    pub footnote_defs: Vec<FootnoteDef>,
    /// Inline footnotes outside code.
    pub inline_footnotes: Vec<InlineFootnote>,
    /// Fenced code blocks (fence lines included) and inline code spans.
    pub code: Vec<Span>,
}

impl BodyIndex {
    /// Index `body`, which starts at byte `base` of the document source.
    ///
    /// Fenced blocks open with three or more backticks or tildes (any
    /// indentation, so fences nested in lists and callouts count) and close
    /// with a run of the same character at least as long. An unclosed fence
    /// runs to the end of the body, as in CommonMark. Inline code is a
    /// backtick run closed by a run of equal length on the same line.
    ///
    /// ```
    /// # use ontomark_parse::BodyIndex;
    /// let idx = BodyIndex::scan("See [[A|a]] and `[[not]]`.[^1]\n\n[^1]: Note.\n", 0);
    /// assert_eq!(idx.links.len(), 1);
    /// assert_eq!(idx.links[0].link.target, "A");
    /// assert_eq!(idx.footnote_refs[0].label, "1");
    /// assert_eq!(idx.footnote_defs[0].label, "1");
    /// ```
    #[must_use]
    pub fn scan(body: &str, base: usize) -> Self {
        let mut idx = Self::default();
        let mut fence: Option<(Fence, usize)> = None; // (opener, start)
        let mut line_start = 0;
        while line_start < body.len() {
            let rest = &body[line_start..];
            let (line, next) = match rest.find('\n') {
                Some(i) => (&rest[..i], line_start + i + 1),
                None => (rest, body.len()),
            };
            let content = line.trim_end_matches('\r');
            if let Some((open, start)) = fence {
                if open.closes(content) {
                    idx.code
                        .push(Span::new(base + start, base + line_start + content.len()));
                    fence = None;
                }
            } else if let Some(open) = Fence::open(content) {
                fence = Some((open, line_start));
            } else {
                scan_line(content, base + line_start, &mut idx);
            }
            line_start = next;
        }
        if let Some((_, start)) = fence {
            idx.code.push(Span::new(base + start, base + body.len()));
        }
        idx.code.sort();
        idx
    }
}

fn scan_line(line: &str, base: usize, idx: &mut BodyIndex) {
    let bytes = line.as_bytes();
    // Footnote definition at the start of the line (up to 3 spaces).
    let indent = bytes.iter().take_while(|&&b| b == b' ').count();
    let mut i = 0;
    if indent <= 3 && line[indent..].starts_with("[^") {
        if let Some(close) = line[indent + 2..].find(']') {
            let label = &line[indent + 2..indent + 2 + close];
            let after = indent + 2 + close + 1;
            if valid_label(label) && bytes.get(after) == Some(&b':') {
                let text_start = after + 1;
                idx.footnote_defs.push(FootnoteDef {
                    span: Span::new(base, base + line.len()),
                    label: label.to_owned(),
                    text: Span::new(base + text_start, base + line.len()),
                });
                i = text_start;
            }
        }
    }
    while i < bytes.len() {
        match bytes[i] {
            b'`' => {
                let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
                if let Some(close) = find_backtick_run(&bytes[i + run..], run) {
                    let end = i + run + close + run;
                    idx.code.push(Span::new(base + i, base + end));
                    i = end;
                } else {
                    i += run;
                }
            }
            b'\\' => i += 1 + line[i + 1..].chars().next().map_or(0, char::len_utf8),
            b'!' if line[i..].starts_with("![[") => {
                if let Some(len) = link_len(&line[i + 1..]) {
                    let tok = &line[i..i + 1 + len];
                    if let Some(link) = Wikilink::parse(tok) {
                        idx.links.push(BodyLink {
                            span: Span::new(base + i, base + i + 1 + len),
                            link,
                        });
                    }
                    i += 1 + len;
                } else {
                    i += 1;
                }
            }
            b'[' if line[i..].starts_with("[[") => {
                if let Some(len) = link_len(&line[i..]) {
                    if let Some(link) = Wikilink::parse(&line[i..i + len]) {
                        idx.links.push(BodyLink {
                            span: Span::new(base + i, base + i + len),
                            link,
                        });
                    }
                    i += len;
                } else {
                    i += 2;
                }
            }
            b'[' if line[i..].starts_with("[^") => {
                if let Some(close) = line[i + 2..].find(']') {
                    let label = &line[i + 2..i + 2 + close];
                    if valid_label(label) {
                        let end = i + 2 + close + 1;
                        idx.footnote_refs.push(FootnoteRef {
                            span: Span::new(base + i, base + end),
                            label: label.to_owned(),
                        });
                        i = end;
                        continue;
                    }
                }
                i += 1;
            }
            b'^' if line[i..].starts_with("^[") && !line[i..].starts_with("^[[") => {
                if let Some(close) = matching_bracket(&line[i + 1..]) {
                    let end = i + 1 + close + 1;
                    idx.inline_footnotes.push(InlineFootnote {
                        span: Span::new(base + i, base + end),
                    });
                    i = end;
                } else {
                    i += 1;
                }
            }
            _ => i += line[i..].chars().next().map_or(1, char::len_utf8),
        }
    }
}

fn valid_label(label: &str) -> bool {
    !label.is_empty() && !label.contains(char::is_whitespace) && !label.contains('[')
}

/// Length of the `[[…]]` token at the start of `s`, if it closes on this line.
fn link_len(s: &str) -> Option<usize> {
    let close = s[2..].find("]]")?;
    let inner = &s[2..2 + close];
    if inner.contains("[[") {
        return None;
    }
    Some(2 + close + 2)
}

/// Offset of the `]` matching the `[` at the start of `s`.
fn matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Offset in `s` of a backtick run of exactly `run` characters.
fn find_backtick_run(s: &[u8], run: usize) -> Option<usize> {
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'`' {
            let n = s[i..].iter().take_while(|&&b| b == b'`').count();
            if n == run {
                return Some(i);
            }
            i += n;
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Subpath;

    #[test]
    fn links_in_fences_and_inline_code_are_ignored() {
        let body = "a [[X]]\n```rust\nlet v = \"[[Y]]\";\n```\n~~~~\n[[Z]]\n~~~~\nb `[[W]]` ``[[V]] ` x`` [[U]]\n";
        let idx = BodyIndex::scan(body, 0);
        let targets: Vec<_> = idx.links.iter().map(|l| l.link.target.as_str()).collect();
        assert_eq!(targets, vec!["X", "U"]);
        assert_eq!(idx.code.len(), 4);
    }

    #[test]
    fn subpaths_embeds_and_escaped_pipes() {
        let body = "![[diagram.png]] [[A#Heading]] [[B#^blk|b]] | [[C\\|c]] |";
        let idx = BodyIndex::scan(body, 10);
        assert_eq!(idx.links.len(), 4);
        assert!(idx.links[0].link.embed);
        assert_eq!(idx.links[0].span, Span::new(10, 26));
        assert_eq!(
            idx.links[1].link.subpath,
            Some(Subpath::Heading("Heading".into()))
        );
        assert_eq!(
            idx.links[2].link.subpath,
            Some(Subpath::Block("blk".into()))
        );
        assert_eq!(idx.links[3].link.target, "C");
        assert_eq!(idx.links[3].link.alias.as_deref(), Some("c"));
    }

    #[test]
    fn footnotes_refs_defs_and_inline() {
        let body = "Claim.[^a] More^[inline [nested]] text.[^b2]\n\n[^a]: First.\n   [^b2]: Second [[L]].\n";
        let idx = BodyIndex::scan(body, 0);
        assert_eq!(
            idx.footnote_refs
                .iter()
                .map(|r| r.label.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b2"]
        );
        assert_eq!(
            idx.footnote_defs
                .iter()
                .map(|d| d.label.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b2"]
        );
        assert_eq!(idx.inline_footnotes.len(), 1);
        assert_eq!(
            idx.links.len(),
            1,
            "a link inside a definition is still a link"
        );
    }

    #[test]
    fn unclosed_fence_runs_to_end() {
        let idx = BodyIndex::scan("x\n```\n[[A]]\n", 0);
        assert!(idx.links.is_empty());
        assert_eq!(idx.code, vec![Span::new(2, 12)]);
    }

    #[test]
    fn spans_slice_back_to_tokens_with_unicode() {
        let body = "Café → [[Naïve Bayes|NB]] ok";
        let idx = BodyIndex::scan(body, 0);
        assert_eq!(idx.links[0].span.slice(body), "[[Naïve Bayes|NB]]");
    }
}
