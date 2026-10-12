//! Heading-aware chunking of a note body.
//!
//! Adapted from the documented behaviour of OntoCast's chunker
//! (`tool/chunk/{chunker,sections,sizing}.py`, growgraph/ontocast `abb8d7b`,
//! Apache-2.0): cuts fall on the document's headings; a heading with no
//! text of its own opens the chunk that follows it; neighbouring short
//! sections are joined until the chunk reaches the minimum size, so one
//! chunk may cover several sections but a section is never split across
//! chunks unless it alone exceeds the maximum, when it splits at paragraph,
//! then line, boundaries. `#` lines inside fenced code are not headings, and
//! fenced code is never cut: a split that would fall inside a fence moves to
//! the fence's edge, and a fence that alone exceeds the maximum is emitted
//! whole as one oversize chunk.
//!
//! Every chunk's `text` is the exact byte slice of the document named by its
//! `span`, so evidence found in a chunk maps back to the source without
//! arithmetic on trimmed text.

use ontomark_parse::{Document, Span};
use serde::{Deserialize, Serialize};

/// One unit of body text the model sees on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    /// Position in the request, from zero.
    pub index: usize,
    /// Byte span in the document source.
    pub span: Span,
    /// The headings enclosing this chunk, outermost first.
    pub heading_path: Vec<String>,
    /// The exact source text of `span`.
    pub text: String,
}

/// A run of body text under one heading path. Offsets are into the body.
struct Section {
    heading_path: Vec<String>,
    start: usize,
    end: usize,
    /// `true` when nothing but the heading line (and blank lines) is here.
    bare: bool,
}

/// Cut `doc`'s body into chunks of roughly `min_chars..=max_chars`
/// characters.
///
/// ```
/// # use ontomark_ingest::chunk::chunk_document;
/// # use ontomark_parse::Document;
/// let doc = Document::parse("---\ntype: Class\n---\n# A\nalpha\n\n# B\nbeta\n");
/// let chunks = chunk_document(&doc, 1, 100);
/// assert_eq!(chunks.len(), 2);
/// assert_eq!(chunks[1].heading_path, vec!["B"]);
/// assert_eq!(chunks[1].span.slice(doc.source()), chunks[1].text);
/// ```
#[must_use]
pub fn chunk_document(doc: &Document, min_chars: usize, max_chars: usize) -> Vec<Chunk> {
    let body = doc.body();
    let base = doc.body_span().start;
    if body.trim().is_empty() {
        return Vec::new();
    }
    let max_chars = max_chars.max(1);
    let units = merge(body, sections(body), min_chars, max_chars);
    let mut out = Vec::new();
    for (path, start, end) in units {
        for (s, e) in split_oversize(&body[start..end], max_chars) {
            let span = Span::new(base + start + s, base + start + e);
            out.push(Chunk {
                index: out.len(),
                span,
                heading_path: path.clone(),
                text: span.slice(doc.source()).to_owned(),
            });
        }
    }
    out
}

/// `(level, title)` for an ATX heading line.
fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    Some((hashes, rest.trim().trim_end_matches('#').trim()))
}

/// `(marker, length)` for a code fence line.
fn fence(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start();
    ['`', '~'].into_iter().find_map(|mark| {
        let n = t.chars().take_while(|c| *c == mark).count();
        (n >= 3).then_some((mark, n))
    })
}

fn sections(body: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut open_fence: Option<(char, usize)> = None;
    let mut start = 0usize;
    let mut has_content = false;
    let mut path: Vec<String> = Vec::new();
    let mut offset = 0usize;
    for line in body.split_inclusive('\n') {
        let raw = line.trim_end_matches(['\n', '\r']);
        match open_fence {
            Some((mark, n)) => {
                let t = raw.trim();
                if t.chars().count() >= n && t.chars().all(|c| c == mark) {
                    open_fence = None;
                }
                has_content = true;
            }
            None => {
                if let Some(f) = fence(raw) {
                    open_fence = Some(f);
                    has_content = true;
                } else if let Some((level, title)) = heading(raw) {
                    out.push(Section {
                        heading_path: path.clone(),
                        start,
                        end: offset,
                        bare: !has_content,
                    });
                    while stack.last().is_some_and(|(l, _)| *l >= level) {
                        stack.pop();
                    }
                    stack.push((level, title.to_owned()));
                    path = stack.iter().map(|(_, t)| t.clone()).collect();
                    start = offset;
                    has_content = false;
                } else if !raw.trim().is_empty() {
                    has_content = true;
                }
            }
        }
        offset += line.len();
    }
    out.push(Section {
        heading_path: path,
        start,
        end: offset,
        bare: !has_content,
    });
    out.retain(|s| s.end > s.start);
    out
}

/// Join neighbouring sections into units: a bare heading opens the next
/// section; a unit under `min` absorbs the next while the result stays
/// within `max`.
fn merge(
    body: &str,
    sections: Vec<Section>,
    min: usize,
    max: usize,
) -> Vec<(Vec<String>, usize, usize)> {
    // Limits are in characters, as the options say; spans are bytes.
    let chars = |a: usize, b: usize| body[a..b].chars().count();
    let mut out: Vec<(Vec<String>, usize, usize)> = Vec::new();
    // A bare heading waiting for content: (path, start, end).
    let mut carry: Option<(Vec<String>, usize, usize)> = None;
    for s in sections {
        let (path, start) = match carry.take() {
            Some((p, st, _)) => (p, st),
            None => (s.heading_path.clone(), s.start),
        };
        if s.bare {
            carry = Some((path, start, s.end));
            continue;
        }
        if let Some(last) = out.last_mut() {
            if chars(last.1, last.2) < min && chars(last.1, s.end) <= max {
                last.2 = s.end;
                continue;
            }
        }
        out.push((path, start, s.end));
    }
    if let Some((path, start, end)) = carry {
        // Trailing bare heading: keep its bytes with the last unit so the
        // chunks cover the body, or stand alone if it is the whole body.
        match out.last_mut() {
            Some(last) => last.2 = end,
            None => out.push((path, start, end)),
        }
    }
    out
}

/// Byte ranges of the fenced blocks in `text`, each from the opening fence
/// line through its closing line. An unterminated fence runs to the end.
fn fence_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut open: Option<(char, usize, usize)> = None;
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let raw = line.trim_end_matches(['\n', '\r']);
        match open {
            Some((mark, n, from)) => {
                let t = raw.trim();
                if t.chars().count() >= n && t.chars().all(|c| c == mark) {
                    out.push((from, offset + line.len()));
                    open = None;
                }
            }
            None => {
                if let Some((mark, n)) = fence(raw) {
                    open = Some((mark, n, offset));
                }
            }
        }
        offset += line.len();
    }
    if let Some((_, _, from)) = open {
        out.push((from, text.len()));
    }
    out
}

/// Split one unit over `max` characters at blank lines, then line ends, then
/// a character boundary, never inside a fenced block. A cut that would land
/// in a fence moves back to the fence's start, or, when the piece already
/// begins at the fence, forward to its end, so a fence longer than `max`
/// stands whole. Offsets are relative to `text`.
fn split_oversize(text: &str, max: usize) -> Vec<(usize, usize)> {
    if text.chars().count() <= max {
        return vec![(0, text.len())];
    }
    let fences = fence_ranges(text);
    let mut pieces = Vec::new();
    let mut start = 0usize;
    let mut remaining = text.chars().count();
    while start < text.len() {
        let rest = &text[start..];
        if remaining <= max {
            pieces.push((start, text.len()));
            break;
        }
        let limit = byte_at_char(rest, max);
        let window = &rest[..limit];
        let cut = window
            .rfind("\n\n")
            .map(|i| i + 2)
            .or_else(|| window.rfind('\n').map(|i| i + 1))
            .unwrap_or(limit);
        let at = start + cut;
        let end = match fences.iter().find(|(f, e)| *f < at && at < *e) {
            Some(&(f, _)) if f > start => f,
            Some(&(_, e)) => e,
            None => at,
        };
        pieces.push((start, end));
        remaining -= text[start..end].chars().count();
        start = end;
    }
    // A piece that is only whitespace joins its neighbour rather than
    // standing as a chunk with nothing to quote.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    let mut pending_start: Option<usize> = None;
    for (s, e) in pieces {
        if text[s..e].trim().is_empty() {
            match merged.last_mut() {
                Some(last) => last.1 = e,
                None => {
                    pending_start.get_or_insert(s);
                }
            }
            continue;
        }
        merged.push((pending_start.take().unwrap_or(s), e));
    }
    if pending_start.is_some() {
        match merged.last_mut() {
            Some(last) => last.1 = text.len(),
            None => merged.push((0, text.len())),
        }
    }
    merged
}

fn byte_at_char(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map_or(s.len(), |(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(body: &str) -> Document {
        Document::parse(format!("---\ntype: Class\n---\n{body}"))
    }

    fn check_invariants(d: &Document, chunks: &[Chunk]) {
        let mut last_end = 0;
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.index, i);
            assert_eq!(c.span.slice(d.source()), c.text);
            assert!(c.span.start >= last_end, "chunks overlap");
            assert!(!c.text.trim().is_empty(), "empty chunk");
            last_end = c.span.end;
        }
    }

    #[test]
    fn empty_body_has_no_chunks() {
        assert!(chunk_document(&doc(""), 10, 100).is_empty());
        assert!(chunk_document(&doc("\n\n  \n"), 10, 100).is_empty());
    }

    #[test]
    fn short_note_is_one_chunk() {
        let d = doc("Just a sentence.\n\nAnd another.\n");
        let c = chunk_document(&d, 3000, 12000);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].heading_path, Vec::<String>::new());
        assert_eq!(c[0].text, d.body());
    }

    #[test]
    fn headings_cut_and_nest() {
        let d = doc("intro\n\n# One\nalpha\n\n## One.A\nbeta\n\n# Two\ngamma\n");
        let c = chunk_document(&d, 1, 1000);
        check_invariants(&d, &c);
        let paths: Vec<Vec<String>> = c.iter().map(|c| c.heading_path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                vec![],
                vec!["One".to_owned()],
                vec!["One".to_owned(), "One.A".to_owned()],
                vec!["Two".to_owned()],
            ]
        );
        assert!(c[1].text.starts_with("# One\n"));
    }

    #[test]
    fn bare_heading_opens_the_following_section() {
        let d = doc("# Empty\n## Full\ntext\n");
        let c = chunk_document(&d, 1, 1000);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].heading_path, vec!["Empty".to_owned()]);
        assert!(c[0].text.starts_with("# Empty\n## Full\n"));
    }

    #[test]
    fn trailing_bare_heading_is_kept() {
        let d = doc("text\n# End\n");
        let c = chunk_document(&d, 1, 1000);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, d.body());
        let d = doc("# Only a heading\n");
        let c = chunk_document(&d, 1, 1000);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn fenced_code_hashes_are_not_headings() {
        let d = doc("# H\n```\n# not a heading\n```\nafter\n~~~md\n## nor this\n~~~\n# Real\nx\n");
        let c = chunk_document(&d, 1, 1000);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].heading_path, vec!["Real".to_owned()]);
    }

    #[test]
    fn short_sections_merge_up_to_min() {
        let d = doc("# A\na\n# B\nb\n# C\nc\n");
        let c = chunk_document(&d, 100, 1000);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].heading_path, vec!["A".to_owned()]);
        assert_eq!(c[0].text, d.body());
    }

    #[test]
    fn oversize_splits_at_paragraphs_then_lines_then_chars() {
        let para = "word ".repeat(10).trim_end().to_owned(); // 49 chars
        let d = doc(&format!("{para}\n\n{para}\n\n{para}\n"));
        let c = chunk_document(&d, 1, 60);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 3);
        assert!(c.iter().all(|ch| ch.text.chars().count() <= 60));

        let line = "x".repeat(40);
        let d = doc(&format!("{line}\n{line}\n{line}\n"));
        let c = chunk_document(&d, 1, 50);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 3);

        let d = doc(&"é".repeat(25));
        let c = chunk_document(&d, 1, 10);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 3);
        assert_eq!(c.iter().map(|c| c.text.chars().count()).sum::<usize>(), 25);
    }

    fn assert_no_cut_fence(_d: &Document, c: &[Chunk]) {
        for ch in c {
            let ticks = ch
                .text
                .lines()
                .filter(|l| l.trim_start().starts_with("```"))
                .count();
            assert_eq!(ticks % 2, 0, "fence cut in {:?}", ch.text);
        }
    }

    #[test]
    fn short_fence_straddling_a_cut_is_kept_whole() {
        let code = "```\nlet a = 1;\n\nlet b = 2;\n```\n"; // 31 chars
        let d = doc(&format!("{}\n\n{code}tail\n", "p".repeat(30)));
        let c = chunk_document(&d, 1, 40);
        check_invariants(&d, &c);
        assert_no_cut_fence(&d, &c);
        assert!(c.iter().any(|ch| ch.text.contains(code)), "{c:#?}");
        assert!(c.len() >= 2);
    }

    #[test]
    fn fence_longer_than_limit_is_one_oversize_chunk() {
        let code = format!("```\n{}\n\n{}\n```\n", "x".repeat(40), "y".repeat(40));
        let d = doc(&format!("before\n\n{code}after\n"));
        let c = chunk_document(&d, 1, 30);
        check_invariants(&d, &c);
        assert_no_cut_fence(&d, &c);
        let big: Vec<_> = c.iter().filter(|ch| ch.text.contains("```")).collect();
        assert_eq!(big.len(), 1);
        assert!(big[0].text.contains(&code));
        assert!(big[0].text.chars().count() > 30);
        assert_eq!(c.first().unwrap().text, "before\n\n");
        assert_eq!(
            c.iter().map(|ch| ch.text.len()).sum::<usize>(),
            d.body().len()
        );
    }

    #[test]
    fn tilde_lines_inside_a_backtick_fence_do_not_close_it() {
        let code = "```\n~~~\nnot closed\n\n~~~\n```\n";
        let d = doc(&format!("{code}{}\n", "z".repeat(20)));
        let c = chunk_document(&d, 1, 20);
        check_invariants(&d, &c);
        assert_eq!(c[0].text, code);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn unterminated_fence_runs_to_end_of_text() {
        let code = format!("```\n{}\n\n{}\n", "a".repeat(30), "b".repeat(30));
        let d = doc(&format!("intro\n\n{code}"));
        let c = chunk_document(&d, 1, 25);
        check_invariants(&d, &c);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].text, "intro\n\n");
        assert_eq!(c[1].text, code);
    }

    #[test]
    fn fence_aware_split_is_deterministic_and_counts_characters() {
        let code = "```\né é é\n\né é é\n```\n";
        let d = doc(&format!("{}\n\n{code}", "é".repeat(20)));
        assert_eq!(chunk_document(&d, 1, 25), chunk_document(&d, 1, 25));
        let c = chunk_document(&d, 1, 25);
        check_invariants(&d, &c);
        assert!(c.iter().any(|ch| ch.text == code));
    }

    #[test]
    fn deterministic() {
        let d = doc("# A\nalpha\n\n# B\nbeta\n");
        assert_eq!(chunk_document(&d, 1, 100), chunk_document(&d, 1, 100));
    }
}
