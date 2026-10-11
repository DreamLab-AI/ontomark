//! The typed view of a frontmatter value, and the `[[wikilink]]` token.
//!
//! [`Value`] is a *read* view. The bytes the author wrote stay in the
//! document source; a value is only ever written back through a guarded
//! [`crate::Patch`], which re-emits the one key it was asked to change.

use std::fmt;

use indexmap::IndexMap;

/// A YAML value under the core schema, as the parser resolved it.
///
/// Floats keep their lexical form (`0.70` stays `0.70`), so equality is
/// lexical and a value read from a page compares equal to itself however the
/// author spelled the number. Mapping keys are the scalar text the author
/// wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "t", content = "v", rename_all = "lowercase")
)]
pub enum Value {
    /// `null`, `~` or an empty value.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// A 64-bit integer.
    Int(i64),
    /// A float, kept in the author's lexical form.
    Float(String),
    /// A string (plain, quoted or block scalar).
    String(String),
    /// A sequence.
    Seq(Vec<Value>),
    /// A mapping, in author order.
    Map(IndexMap<String, Value>),
}

impl Value {
    /// A string value.
    #[must_use]
    pub fn string(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }

    /// A sequence of string values.
    ///
    /// ```
    /// # use ontomark_parse::Value;
    /// let v = Value::strings(["a", "b"]);
    /// assert_eq!(v.as_seq().unwrap().len(), 2);
    /// ```
    #[must_use]
    pub fn strings<I, S>(items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::Seq(items.into_iter().map(|s| Self::String(s.into())).collect())
    }

    /// The string, when this is a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean, when this is a YAML boolean. No string coercion: a
    /// profile that accepts `"true"` does so explicitly.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The number as `f64`, for an integer or a float.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            #[allow(clippy::cast_precision_loss)]
            Self::Int(i) => Some(*i as f64),
            Self::Float(raw) => raw.parse().ok(),
            _ => None,
        }
    }

    /// The items, when this is a sequence.
    #[must_use]
    pub fn as_seq(&self) -> Option<&[Value]> {
        match self {
            Self::Seq(items) => Some(items),
            _ => None,
        }
    }

    /// The entries, when this is a mapping.
    #[must_use]
    pub fn as_map(&self) -> Option<&IndexMap<String, Value>> {
        match self {
            Self::Map(m) => Some(m),
            _ => None,
        }
    }

    /// The scalar rendered as text: strings as-is, numbers and booleans in
    /// their canonical spelling. `None` for null, sequences and mappings.
    #[must_use]
    pub fn scalar_text(&self) -> Option<String> {
        match self {
            Self::String(s) => Some(s.clone()),
            Self::Int(i) => Some(i.to_string()),
            Self::Float(raw) => Some(raw.clone()),
            Self::Bool(b) => Some(b.to_string()),
            Self::Null | Self::Seq(_) | Self::Map(_) => None,
        }
    }

    /// The value as a list of scalar texts. A bare scalar reads as a
    /// one-element list (Obsidian's list-or-scalar tolerance); non-scalar
    /// items are skipped.
    ///
    /// ```
    /// # use ontomark_parse::Value;
    /// assert_eq!(Value::string("x").texts(), vec!["x".to_owned()]);
    /// assert!(Value::Null.texts().is_empty());
    /// ```
    #[must_use]
    pub fn texts(&self) -> Vec<String> {
        match self {
            Self::Seq(items) => items.iter().filter_map(Self::scalar_text).collect(),
            other => other.scalar_text().into_iter().collect(),
        }
    }

    /// A short name for the variant, for diagnostics.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Int(_) => "integer",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Seq(_) => "sequence",
            Self::Map(_) => "mapping",
        }
    }
}

/// What follows `#` in a wikilink target.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "t", content = "v", rename_all = "lowercase")
)]
pub enum Subpath {
    /// `[[Page#Heading]]`.
    Heading(String),
    /// `[[Page#^block-id]]`.
    Block(String),
}

/// An Obsidian `[[target#subpath|alias]]` reference, or a `![[…]]` embed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Wikilink {
    /// The link target as written (a note name or vault path, no `.md`
    /// required). Empty for a same-note link such as `[[#Heading]]`.
    pub target: String,
    /// The heading or block reference after `#`.
    pub subpath: Option<Subpath>,
    /// The display text after `|`.
    pub alias: Option<String>,
    /// `true` for `![[…]]`.
    pub embed: bool,
}

impl Wikilink {
    /// A plain link to `target`.
    #[must_use]
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            subpath: None,
            alias: None,
            embed: false,
        }
    }

    /// The text a reader sees: the alias if present, else the target.
    #[must_use]
    pub fn label(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.target)
    }

    /// Parse one whole `[[…]]` or `![[…]]` token (surrounding whitespace
    /// allowed). Returns `None` when `raw` is not exactly one wikilink.
    ///
    /// A `\|` separator (the escaped form Obsidian writes inside tables) is
    /// accepted as the alias separator.
    ///
    /// ```
    /// # use ontomark_parse::{Wikilink, Subpath};
    /// let w = Wikilink::parse("[[Ontology#Scope|the ontology]]").unwrap();
    /// assert_eq!(w.target, "Ontology");
    /// assert_eq!(w.subpath, Some(Subpath::Heading("Scope".into())));
    /// assert_eq!(w.label(), "the ontology");
    /// assert!(Wikilink::parse("Ontology").is_none());
    /// assert!(Wikilink::parse("[[a]] and [[b]]").is_none());
    /// ```
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let t = raw.trim();
        let (embed, t) = match t.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, t),
        };
        let inner = t.strip_prefix("[[")?.strip_suffix("]]")?;
        if inner.contains("[[") || inner.contains("]]") || inner.contains('\n') {
            return None;
        }
        let mut link = Self::from_inner(inner)?;
        link.embed = embed;
        Some(link)
    }

    /// Parse the text between `[[` and `]]`.
    pub(crate) fn from_inner(inner: &str) -> Option<Self> {
        let (target_part, alias) = match inner.find('|') {
            Some(i) => {
                let t = &inner[..i];
                let t = t.strip_suffix('\\').unwrap_or(t);
                (t, Some(inner[i + 1..].trim().to_owned()))
            }
            None => (inner, None),
        };
        let (target, subpath) = match target_part.find('#') {
            Some(i) => {
                let sub = target_part[i + 1..].trim();
                let sub = match sub.strip_prefix('^') {
                    Some(block) => Subpath::Block(block.to_owned()),
                    None => Subpath::Heading(sub.to_owned()),
                };
                (target_part[..i].trim(), Some(sub))
            }
            None => (target_part.trim(), None),
        };
        if target.is_empty() && subpath.is_none() {
            return None;
        }
        Some(Self {
            target: target.to_owned(),
            subpath,
            alias,
            embed: false,
        })
    }
}

impl fmt::Display for Wikilink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.embed {
            f.write_str("!")?;
        }
        write!(f, "[[{}", self.target)?;
        match &self.subpath {
            Some(Subpath::Heading(h)) => write!(f, "#{h}")?,
            Some(Subpath::Block(b)) => write!(f, "#^{b}")?,
            None => {}
        }
        if let Some(a) = &self.alias {
            write!(f, "|{a}")?;
        }
        f.write_str("]]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wikilink_display_round_trips() {
        for raw in [
            "[[A]]",
            "[[A|B]]",
            "[[A#H]]",
            "[[A#^b1|x]]",
            "![[img.png]]",
            "[[#Local]]",
        ] {
            assert_eq!(Wikilink::parse(raw).unwrap().to_string(), raw, "{raw}");
        }
    }

    #[test]
    fn table_escaped_pipe_is_the_alias_separator() {
        let w = Wikilink::parse(r"[[Target\|Shown]]").unwrap();
        assert_eq!(w.target, "Target");
        assert_eq!(w.alias.as_deref(), Some("Shown"));
    }

    #[test]
    fn empty_link_is_not_a_link() {
        assert!(Wikilink::parse("[[]]").is_none());
        assert!(Wikilink::parse("[[ | x]]").is_none());
    }
}
