//! Emit one top-level `key: value` entry in the block's own style.
//!
//! The house style is the one the visionGraph corpus already uses (serde_yaml
//! 0.9's output): plain scalars where they read back unchanged, single quotes
//! otherwise, double quotes only for control characters and line breaks,
//! `[]` / `{}` for empty collections, and indentless `- item` sequences unless
//! the block indents them. Every emitted scalar is re-read through the same
//! resolver the parser uses; a scalar that would not read back identically is
//! double-quoted instead.

use crate::frontmatter::{resolve_plain, BlockStyle};
use crate::value::Value;

/// A value [`entry`] cannot write so that it reads back equal.
///
/// The payload is the text that does not survive: the raw text of a
/// [`Value::Float`] the reader would not type as a float (`1_0.0`, `nan`),
/// or the key whose emitted entry does not read back equal under the
/// caller's [`BlockStyle`](crate::BlockStyle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unrepresentable(pub String);

impl std::fmt::Display for Unrepresentable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}` does not read back as written", self.0)
    }
}

impl std::error::Error for Unrepresentable {}

/// Refuse any value (at any depth) that would not read back equal.
///
/// # Errors
/// [`Unrepresentable`] naming the first offending float text.
pub fn check(v: &Value) -> Result<(), Unrepresentable> {
    match v {
        Value::Float(raw) => match resolve_plain(raw) {
            Value::Float(r) if &r == raw => Ok(()),
            _ => Err(Unrepresentable(raw.clone())),
        },
        Value::Seq(items) => items.iter().try_for_each(check),
        Value::Map(m) => m.values().try_for_each(check),
        _ => Ok(()),
    }
}

/// Render `key: value` as one or more complete lines (final line break
/// included) using `style`.
///
/// # Errors
/// [`Unrepresentable`] when `value` holds a float the reader would not
/// type as one; nothing is emitted rather than text that reads back wrong.
pub fn entry(key: &str, value: &Value, style: &BlockStyle) -> Result<String, Unrepresentable> {
    check(value)?;
    let nl = if style.crlf { "\r\n" } else { "\n" };
    let mut out = String::new();
    out.push_str(&scalar_text(key));
    out.push(':');
    write_value_after_key(&mut out, value, 0, style, nl);
    // Prove the text reads back as the same key and value under a plain
    // block. A caller-supplied style (say `map_indent: 0`) can otherwise
    // change structure.
    let mut probe = format!("---{nl}{out}");
    if !out.ends_with('\n') {
        probe.push_str(nl);
    }
    probe.push_str("---");
    probe.push_str(nl);
    let back = crate::Document::parse(&probe);
    if back.value(key) != Some(value) || back.frontmatter().map_or(0, |f| f.entries.len()) != 1 {
        return Err(Unrepresentable(key.to_owned()));
    }
    Ok(out)
}

/// Write a value that follows `key:` at indentation `indent`.
fn write_value_after_key(
    out: &mut String,
    value: &Value,
    indent: usize,
    style: &BlockStyle,
    nl: &str,
) {
    match value {
        Value::Seq(items) if !items.is_empty() => {
            out.push_str(nl);
            let dash_indent = indent + style.seq_indent;
            for item in items {
                push_spaces(out, dash_indent);
                out.push('-');
                write_item(out, item, dash_indent + 2, style, nl);
            }
        }
        Value::Map(map) if !map.is_empty() => {
            out.push_str(nl);
            let child = indent + style.map_indent;
            for (k, v) in map {
                push_spaces(out, child);
                out.push_str(&scalar_text(k));
                out.push(':');
                write_value_after_key(out, v, child, style, nl);
            }
        }
        other => {
            out.push(' ');
            out.push_str(&inline(other));
            out.push_str(nl);
        }
    }
}

/// Write a sequence item after its `-`; `content_indent` is the column the
/// item's content starts at.
fn write_item(out: &mut String, item: &Value, content_indent: usize, style: &BlockStyle, nl: &str) {
    match item {
        Value::Map(map) if !map.is_empty() => {
            let mut first = true;
            for (k, v) in map {
                if first {
                    out.push(' ');
                    first = false;
                } else {
                    push_spaces(out, content_indent);
                }
                out.push_str(&scalar_text(k));
                out.push(':');
                write_value_after_key(out, v, content_indent, style, nl);
            }
        }
        Value::Seq(items) if !items.is_empty() => {
            let mut first = true;
            for inner in items {
                if first {
                    out.push(' ');
                    first = false;
                } else {
                    push_spaces(out, content_indent);
                }
                out.push('-');
                write_item(out, inner, content_indent + 2, style, nl);
            }
        }
        other => {
            out.push(' ');
            out.push_str(&inline(other));
            out.push_str(nl);
        }
    }
}

fn push_spaces(out: &mut String, n: usize) {
    out.extend(std::iter::repeat_n(' ', n));
}

/// A value that fits on one line: scalars and empty collections.
fn inline(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(raw) => {
            // Unreachable for patches: `Document::apply` refuses a float
            // whose text the reader would not type as a float.
            if matches!(resolve_plain(raw), Value::Float(_)) {
                raw.clone()
            } else {
                double_quoted(raw)
            }
        }
        Value::String(s) => scalar_text(s),
        Value::Seq(_) => "[]".into(),
        Value::Map(_) => "{}".into(),
    }
}

/// A string scalar in the cheapest style that reads back as the same string.
#[must_use]
pub fn scalar_text(s: &str) -> String {
    if plain_safe(s) {
        s.to_owned()
    } else if s.chars().any(|c| c.is_control() || c == '\u{feff}') {
        double_quoted(s)
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}

fn plain_safe(s: &str) -> bool {
    let Some(first) = s.chars().next() else {
        return false;
    };
    if s.trim() != s || s.chars().any(|c| c.is_control() || c == '\u{feff}') {
        return false;
    }
    if "-?:,[]{}#&*!|>'\"%@`".contains(first) {
        return false;
    }
    // Document markers: at column 0 (a key) they would end the document.
    if s.starts_with("---") || s.starts_with("...") {
        return false;
    }
    if s.contains(": ") || s.contains(" #") || s.ends_with(':') {
        return false;
    }
    // YAML 1.1 booleans: quote them so older readers agree.
    if matches!(
        s.to_ascii_lowercase().as_str(),
        "y" | "n" | "yes" | "no" | "on" | "off"
    ) {
        return false;
    }
    matches!(resolve_plain(s), Value::String(ref r) if r == s)
}

fn double_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() || c == '\u{feff}' => {
                out.push_str(&format!("\\u{:04x}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    fn st() -> BlockStyle {
        BlockStyle::default()
    }

    #[test]
    fn corpus_house_style() {
        let v = Value::strings(["[[Computer Vision Task]]", "plain"]);
        assert_eq!(
            entry("is-a", &v, &st()).expect("representable"),
            "is-a:\n- '[[Computer Vision Task]]'\n- plain\n"
        );
        assert_eq!(
            entry("links", &Value::Seq(vec![]), &st()).expect("representable"),
            "links: []\n"
        );
        assert_eq!(
            entry("quality", &Value::Float("0.70".into()), &st()).expect("representable"),
            "quality: 0.70\n"
        );
        assert_eq!(
            entry("public", &Value::Bool(true), &st()).expect("representable"),
            "public: true\n"
        );
    }

    #[test]
    fn strings_that_look_like_other_types_are_quoted() {
        for s in [
            "true", "1", "0.5", "null", "~", "yes", "", " lead", "a: b", "x #y", "#c", "- d",
        ] {
            let t = scalar_text(s);
            assert_ne!(t, s, "{s:?} must be quoted");
        }
        assert_eq!(scalar_text("it's"), "it's");
        assert_eq!(scalar_text("'q'"), "'''q'''");
        assert_eq!(scalar_text("line\nbreak"), "\"line\\nbreak\"");
    }

    #[test]
    fn nested_mapping_and_sequence_of_mappings() {
        let mut g = IndexMap::new();
        g.insert("by".to_owned(), Value::string("agent:x/1"));
        g.insert("at".to_owned(), Value::string("2026-09-22T12:41:39Z"));
        assert_eq!(
            entry("generated", &Value::Map(g.clone()), &st()).expect("representable"),
            "generated:\n  by: agent:x/1\n  at: 2026-09-22T12:41:39Z\n"
        );
        let seq = Value::Seq(vec![Value::Map(g)]);
        assert_eq!(
            entry("sources", &seq, &st()).expect("representable"),
            "sources:\n- by: agent:x/1\n  at: 2026-09-22T12:41:39Z\n"
        );
    }

    #[test]
    fn crlf_and_indented_sequences_follow_the_block() {
        let style = BlockStyle {
            seq_indent: 2,
            map_indent: 2,
            crlf: true,
        };
        assert_eq!(
            entry("a", &Value::strings(["x"]), &style).expect("representable"),
            "a:\r\n  - x\r\n"
        );
    }
}
