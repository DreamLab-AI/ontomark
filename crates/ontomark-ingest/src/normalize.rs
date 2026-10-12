//! Label normalisation shared by grounding and the lexical guards.
//!
//! Adapted from OntoCast `tool/representation_text.py` and
//! `tool/agg/signatures.py` (growgraph/ontocast `abb8d7b`, Apache-2.0). The
//! two sides of every comparison must agree on what counts as "the same
//! string", so this module is the single source for that rule.

use unicode_general_category::{get_general_category, GeneralCategory};
use unicode_normalization::UnicodeNormalization;

/// Normalise free text for matching: NFD, non-spacing marks (category `Mn`)
/// dropped, `_` and `-` read as spaces, lower-cased, whitespace collapsed
/// and trimmed.
///
/// Only `Mn` is dropped, as upstream does: spacing marks (`Mc`) such as
/// Indic vowel signs carry meaning and stay, so `किताब` and `कताब` remain
/// distinct.
///
/// ```
/// # use ontomark_ingest::normalize::normalize_text;
/// assert_eq!(normalize_text("  Dvořák_String-Quartet "), "dvorak string quartet");
/// ```
#[must_use]
pub fn normalize_text(text: &str) -> String {
    let folded: String = text
        .nfd()
        .filter(|c| get_general_category(*c) != GeneralCategory::NonspacingMark)
        .map(|c| if c == '_' || c == '-' { ' ' } else { c })
        .collect();
    let lower = folded.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut pending_space = false;
    for c in lower.chars() {
        // Python's `\s` also covers U+001C..U+001F.
        if c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c) {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(c);
        }
    }
    out
}

/// [`normalize_text`] after splitting CamelCase, so `KnowledgeGraph`,
/// `knowledge_graph` and `knowledge-graph` all yield `knowledge graph`.
///
/// ```
/// # use ontomark_ingest::normalize::normalize_label;
/// assert_eq!(normalize_label("KnowledgeGraph"), "knowledge graph");
/// assert_eq!(normalize_label("knowledge_graph"), "knowledge graph");
/// ```
#[must_use]
pub fn normalize_label(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut split = String::with_capacity(text.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        // Upstream's `(?=[A-Z][a-z])` is ASCII-only; so is this.
        let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
        if i > 0 && c.is_ascii_uppercase() && next_lower {
            split.push(' ');
        }
        split.push(c);
    }
    normalize_text(&split)
}

/// Strip non-word characters from both ends of a token: `baranov,` and `d.`
/// become `baranov` and `d`. An initial written `D.` is two characters
/// lexically and one semantically.
#[must_use]
pub fn clean_token(token: &str) -> &str {
    token.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_'))
}

/// Split a normalised label into cleaned, non-empty tokens. Short tokens
/// stay: initials and single-letter identifiers are often the only mark that
/// tells `company S.` from `company T.`.
///
/// ```
/// # use ontomark_ingest::normalize::label_tokens;
/// assert_eq!(label_tokens("baranov, d."), vec!["baranov", "d"]);
/// ```
#[must_use]
pub fn label_tokens(label: &str) -> Vec<&str> {
    label
        .split_whitespace()
        .map(clean_token)
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_nonspacing_marks_are_dropped() {
        // Mn: the acute on é goes.
        assert_eq!(normalize_text("Café"), "cafe");
        // Mc: the Devanagari vowel sign i (U+093F) is a spacing mark and
        // stays, so these two words do not collapse into one.
        assert_ne!(normalize_text("किताब"), normalize_text("कताब"));
        // Mn: the virama (U+094D) goes, as upstream.
        assert_eq!(normalize_text("क्"), "क");
    }

    #[test]
    fn diacritics_case_and_separators_fold() {
        assert_eq!(normalize_text("Émile  Zola"), "emile zola");
        assert_eq!(normalize_text("snake_case-kebab"), "snake case kebab");
        assert_eq!(normalize_text("   "), "");
        // Lower-casing, not case-folding, as in Python `str.lower()`.
        assert_eq!(normalize_text("ß"), "ß");
    }

    #[test]
    fn camel_split_matches_snake() {
        assert_eq!(normalize_label("knowledgeGraph"), "knowledge graph");
        // The upstream regex splits before an upper-then-lower pair only.
        assert_eq!(normalize_label("HTTPServer"), "http server");
        assert_eq!(normalize_label("Graph"), "graph");
    }

    #[test]
    fn tokens_shed_edge_punctuation_only() {
        assert_eq!(label_tokens("o'hare, j."), vec!["o'hare", "j"]);
        assert_eq!(label_tokens("--- ..."), Vec::<&str>::new());
    }
}
