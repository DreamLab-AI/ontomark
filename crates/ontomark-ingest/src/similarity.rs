//! Deterministic lexical guards for deciding whether two names denote one
//! entity.
//!
//! Adapted from OntoCast `tool/agg/signatures.py` and
//! `tool/agg/aggregate.py` (growgraph/ontocast `abb8d7b`, Apache-2.0),
//! specified from its documented behaviour and re-implemented here; nothing
//! is transliterated. The guards are tiers, tried in order, and the tier that
//! fires is reported so a reviewer can see why a name grounded.
//!
//! [`sequence_ratio`] re-implements Python `difflib.SequenceMatcher.ratio()`
//! (Ratcliff/Obershelp) without the junk heuristics, which only engage on
//! strings of 200 or more characters. Labels are far shorter, so the values
//! agree with Python on the fixtures below, but byte equality with Python is
//! not assumed for arbitrary long input.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::normalize::label_tokens;

/// Thresholds for the lexical tiers. Defaults are OntoCast's
/// `AGG_LEXICAL_LABEL_JACCARD`, `AGG_LEXICAL_SEQUENCE_RATIO` and
/// `AGG_LEXICAL_TOKEN_JACCARD`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LexicalThresholds {
    /// Minimum token Jaccard between any label pair (default 0.5).
    pub label_jaccard: f64,
    /// Minimum sequence ratio between the normal forms (default 0.90).
    pub sequence_ratio: f64,
    /// Minimum token Jaccard between the normal forms when both have two or
    /// more tokens (default 0.75).
    pub token_jaccard: f64,
}

impl Default for LexicalThresholds {
    fn default() -> Self {
        Self {
            label_jaccard: 0.5,
            sequence_ratio: 0.90,
            token_jaccard: 0.75,
        }
    }
}

/// Which tier decided that two names are aliases. Tiers are declared from
/// strongest to weakest, and order the same way, so `Exact < Initials`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AliasTier {
    /// The normal forms are identical.
    Exact,
    /// Some label of one side equals some label of the other.
    SharedLabel,
    /// Every token of the shorter label matches a token of the longer,
    /// exactly or as a single-character initial, with one full word shared.
    Initials,
    /// Token Jaccard of some label pair met the threshold.
    LabelJaccard,
    /// Sequence ratio of the normal forms met the threshold.
    SequenceRatio,
    /// Token Jaccard of the normal forms met the threshold.
    TokenJaccard,
}

/// Jaccard similarity of two token sets. Two empty sets are identical (1.0).
#[must_use]
pub fn jaccard(left: &BTreeSet<&str>, right: &BTreeSet<&str>) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    let inter = left.intersection(right).count();
    let union = left.union(right).count();
    inter as f64 / union as f64
}

/// Python `difflib.SequenceMatcher(None, a, b).ratio()` over code points:
/// `2 * matches / (len(a) + len(b))`, where matches are found by recursively
/// taking the longest common block (earliest in `a`, then earliest in `b`).
/// Two empty strings have ratio 1.0.
///
/// ```
/// # use ontomark_ingest::similarity::sequence_ratio;
/// assert!((sequence_ratio("abcd", "bcde") - 0.75).abs() < 1e-12);
/// assert!((sequence_ratio("kitten", "sitting") - 8.0 / 13.0).abs() < 1e-12);
/// ```
#[must_use]
pub fn sequence_ratio(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let total = a.len() + b.len();
    if total == 0 {
        return 1.0;
    }
    2.0 * matching_blocks(&a, &b) as f64 / total as f64
}

fn matching_blocks(a: &[char], b: &[char]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let (i, j, k) = longest_match(a, b);
    if k == 0 {
        return 0;
    }
    k + matching_blocks(&a[..i], &b[..j]) + matching_blocks(&a[i + k..], &b[j + k..])
}

/// Longest common block, breaking ties on the earliest start in `a`, then
/// the earliest start in `b` (difflib's order).
fn longest_match(a: &[char], b: &[char]) -> (usize, usize, usize) {
    let mut best = (0, 0, 0);
    // prev[j] = length of the common suffix ending at a[i-1], b[j-1]. Two
    // rows, swapped, rather than one allocation per row of `a`.
    let mut prev = vec![0usize; b.len() + 1];
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        for (j, &cb) in b.iter().enumerate() {
            if ca == cb {
                let k = prev[j] + 1;
                cur[j + 1] = k;
                let (start_a, start_b) = (i + 1 - k, j + 1 - k);
                let better =
                    k > best.2 || (k == best.2 && k > 0 && (start_a, start_b) < (best.0, best.1));
                if better {
                    best = (start_a, start_b, k);
                }
            } else {
                cur[j + 1] = 0;
            }
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    best
}

/// difflib's `real_quick_ratio`: an upper bound on [`sequence_ratio`] from
/// the lengths alone, so the quadratic pass can be skipped when it cannot
/// reach a threshold.
#[must_use]
pub fn sequence_ratio_bound(a: &str, b: &str) -> f64 {
    let (n, m) = (a.chars().count(), b.chars().count());
    if n + m == 0 {
        return 1.0;
    }
    2.0 * n.min(m) as f64 / (n + m) as f64
}

/// Exact token match, or one side is a single character that starts the
/// other (`d` for `dmitry`, `u` for `us`).
#[must_use]
pub(crate) fn tokens_alias_compatible(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let (shorter, longer) = if left.chars().count() <= right.chars().count() {
        (left, right)
    } else {
        (right, left)
    };
    shorter.chars().count() == 1 && longer.starts_with(shorter)
}

/// The sorted multiset of tokens that contain a digit. `3.1`, `802.11`,
/// `2.0` and `8b` each count as one token, because [`label_tokens`] splits
/// on whitespace only.
fn numeric_tokens<'a>(tokens: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = tokens
        .into_iter()
        .filter(|t| t.chars().any(char::is_numeric))
        .collect();
    out.sort_unstable();
    out
}

/// The numeric-token guard (ADR-OM-010): two names whose numeric tokens
/// differ as multisets denote different things (`ISO 9001` and `ISO 14001`,
/// `Llama 3.1 8B` and `Llama 3.1 70B`, `Web 2.0` and `Web 3.0`), however
/// alike they look. A name with a number never aliases one without it at a
/// fuzzy tier. Applies below [`AliasTier::SharedLabel`] only.
#[must_use]
pub(crate) fn numeric_tokens_differ<'a>(
    left: impl IntoIterator<Item = &'a str>,
    right: impl IntoIterator<Item = &'a str>,
) -> bool {
    numeric_tokens(left) != numeric_tokens(right)
}

fn token_strs(tokens: &[String]) -> impl Iterator<Item = &str> {
    tokens.iter().map(String::as_str)
}

/// The tokens of each label, computed once so the tiers never re-tokenise.
#[must_use]
pub(crate) fn tokenize_labels(labels: &[String]) -> Vec<Vec<String>> {
    labels
        .iter()
        .map(|l| label_tokens(l).into_iter().map(str::to_owned).collect())
        .collect()
}

/// True when some label pair matches token-injectively allowing initials:
/// every token of the shorter label matches a distinct token of the longer,
/// exactly or as an initial, and at least one matched token is a full word
/// (more than two characters). `baranov d` aliases `dmitry baranov`. Both
/// sides are the token lists of their labels (see [`tokenize_labels`]).
#[must_use]
pub(crate) fn labels_alias_with_initials(left: &[Vec<String>], right: &[Vec<String>]) -> bool {
    for lt in left.iter().filter(|t| !t.is_empty()) {
        for rt in right.iter().filter(|t| !t.is_empty()) {
            // `4` is an initial of `40` to the token rule; the numeric guard
            // is what keeps `gpt 4` and `gpt 40` apart.
            if numeric_tokens_differ(token_strs(lt), token_strs(rt)) {
                continue;
            }
            let (shorter, longer) = if lt.len() <= rt.len() {
                (lt, rt)
            } else {
                (rt, lt)
            };
            let mut available: Vec<Option<&str>> =
                longer.iter().map(|t| Some(t.as_str())).collect();
            let mut shared_full = false;
            let mut matched_all = true;
            for token in shorter {
                let found = available
                    .iter()
                    .position(|c| c.is_some_and(|c| tokens_alias_compatible(token, c)));
                match found {
                    None => {
                        matched_all = false;
                        break;
                    }
                    Some(ix) => {
                        if available[ix] == Some(token.as_str()) && token.chars().count() > 2 {
                            shared_full = true;
                        }
                        available[ix] = None;
                    }
                }
            }
            if matched_all && shared_full {
                return true;
            }
        }
    }
    false
}

/// True when some label pair is identical except for conflicting short
/// tokens: `french company s` against `french company t`. Authors write this
/// shape to *distinguish* entities, so it is evidence of distinctness. Both
/// sides are token lists (see [`tokenize_labels`]).
#[must_use]
pub(crate) fn labels_differ_only_by_initials(left: &[Vec<String>], right: &[Vec<String>]) -> bool {
    fn split(toks: &[String]) -> (BTreeSet<&str>, BTreeSet<&str>) {
        let long: BTreeSet<&str> = toks
            .iter()
            .map(String::as_str)
            .filter(|t| t.chars().count() > 2)
            .collect();
        let short: BTreeSet<&str> = toks
            .iter()
            .map(String::as_str)
            .filter(|t| t.chars().count() <= 2)
            .collect();
        (long, short)
    }
    for l in left {
        let (ll, ls) = split(l);
        if ll.is_empty() || ls.is_empty() {
            continue;
        }
        for r in right {
            let (rl, rs) = split(r);
            if rl.is_empty() || rs.is_empty() || ll != rl {
                continue;
            }
            let compatible = ls
                .iter()
                .any(|a| rs.iter().any(|b| tokens_alias_compatible(a, b)));
            if !compatible {
                return true;
            }
        }
    }
    false
}

/// One side of a lexical comparison: a normal form, the normalised labels
/// it is known by, and those labels' tokens (from [`tokenize_labels`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Names<'a> {
    /// The normalised primary form.
    pub normal: &'a str,
    /// Normalised labels, aliases and note names.
    pub labels: &'a [String],
    /// `tokens[i]` are the tokens of `labels[i]`.
    pub tokens: &'a [Vec<String>],
}

/// Decide whether two names are lexical aliases, returning the tier that
/// fired. The tiers run in OntoCast's order: exact, shared label, initials,
/// label Jaccard, sequence ratio, token Jaccard. A pair where one normal form
/// is a strict word-prefix of the other (`graph` against `graph theory`)
/// never passes the ratio and token tiers.
#[must_use]
pub(crate) fn lexical_alias(
    left: &Names<'_>,
    right: &Names<'_>,
    t: &LexicalThresholds,
) -> Option<AliasTier> {
    if left.normal == right.normal {
        return Some(AliasTier::Exact);
    }
    // Labels are sorted and deduplicated by the caller, so a merge walk
    // finds a shared one without building sets.
    if left
        .labels
        .iter()
        .any(|l| right.labels.binary_search(l).is_ok())
    {
        return Some(AliasTier::SharedLabel);
    }
    if labels_alias_with_initials(left.tokens, right.tokens) {
        return Some(AliasTier::Initials);
    }
    if !left.labels.is_empty() && !right.labels.is_empty() {
        let mut best = f64::NEG_INFINITY;
        for at_tokens in left.tokens {
            let at: BTreeSet<&str> = token_strs(at_tokens).collect();
            for bt_tokens in right.tokens {
                if numeric_tokens_differ(token_strs(at_tokens), token_strs(bt_tokens)) {
                    continue;
                }
                let bt: BTreeSet<&str> = token_strs(bt_tokens).collect();
                best = best.max(jaccard(&at, &bt));
            }
        }
        if best >= t.label_jaccard {
            return Some(AliasTier::LabelJaccard);
        }
    }
    let (a, b) = (left.normal.trim(), right.normal.trim());
    if !a.is_empty()
        && !b.is_empty()
        && (a.starts_with(&format!("{b} ")) || b.starts_with(&format!("{a} ")))
    {
        return None;
    }
    // The ratio tiers compare whole normal forms; they too must not alias
    // names that differ in a number.
    if numeric_tokens_differ(label_tokens(left.normal), label_tokens(right.normal)) {
        return None;
    }
    if sequence_ratio_bound(left.normal, right.normal) >= t.sequence_ratio
        && sequence_ratio(left.normal, right.normal) >= t.sequence_ratio
    {
        return Some(AliasTier::SequenceRatio);
    }
    let at: BTreeSet<&str> = label_tokens(left.normal).into_iter().collect();
    let bt: BTreeSet<&str> = label_tokens(right.normal).into_iter().collect();
    if at.len() >= 2 && bt.len() >= 2 && jaccard(&at, &bt) >= t.token_jaccard {
        return Some(AliasTier::TokenJaccard);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set<'a>(items: &[&'a str]) -> BTreeSet<&'a str> {
        items.iter().copied().collect()
    }

    #[test]
    fn jaccard_edges() {
        assert_eq!(jaccard(&set(&[]), &set(&[])), 1.0);
        assert_eq!(jaccard(&set(&["a"]), &set(&[])), 0.0);
        assert_eq!(jaccard(&set(&["a", "b"]), &set(&["b", "c"])), 1.0 / 3.0);
    }

    #[test]
    fn sequence_ratio_matches_difflib_fixtures() {
        // Values produced by CPython 3.12 difflib.SequenceMatcher(None, a, b).ratio().
        let cases = [
            ("abcd", "bcde", 0.75),
            ("kitten", "sitting", 8.0 / 13.0),
            ("", "", 1.0),
            ("abc", "", 0.0),
            ("same", "same", 1.0),
            ("abc", "xyz", 0.0),
            ("knowledge graph", "knowledge graphs", 30.0 / 31.0),
            ("dvorak", "dvorak quartet", 12.0 / 20.0),
        ];
        for (a, b, want) in cases {
            let got = sequence_ratio(a, b);
            assert!(
                (got - want).abs() < 1e-12,
                "{a:?} vs {b:?}: {got} != {want}"
            );
        }
    }

    fn toks(labels: &[&str]) -> Vec<Vec<String>> {
        tokenize_labels(&labels.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn initials_alias_and_distinctness() {
        assert!(labels_alias_with_initials(
            &toks(&["baranov d"]),
            &toks(&["dmitry baranov"])
        ));
        assert!(labels_alias_with_initials(
            &toks(&["j. o'hare"]),
            &toks(&["john o'hare"])
        ));
        // Two initials only: no full word shared.
        assert!(!labels_alias_with_initials(
            &toks(&["j d"]),
            &toks(&["john doe"])
        ));
        assert!(!labels_alias_with_initials(
            &toks(&["baranov"]),
            &toks(&["ivanov"])
        ));

        assert!(labels_differ_only_by_initials(
            &toks(&["french company s"]),
            &toks(&["french company t"])
        ));
        // `u` is an initial of `us`: spelling variance, not a distinguishing mark.
        assert!(!labels_differ_only_by_initials(
            &toks(&["company u"]),
            &toks(&["company us"])
        ));
        assert!(!labels_differ_only_by_initials(
            &toks(&["company"]),
            &toks(&["company t"])
        ));
    }

    #[test]
    fn quick_bound_never_undercuts_the_ratio() {
        for (a, b) in [
            ("abcd", "bcde"),
            ("kitten", "sitting"),
            ("", ""),
            ("abc", ""),
            ("knowledge graph", "knowledge graphs"),
            ("dvorak", "dvorak quartet"),
        ] {
            assert!(sequence_ratio_bound(a, b) >= sequence_ratio(a, b) - 1e-12);
        }
    }

    /// Labels sorted as `Vocabulary` stores them, with their tokens.
    struct Side {
        labels: Vec<String>,
        tokens: Vec<Vec<String>>,
    }

    impl Side {
        fn new(labels: &[&str]) -> Self {
            let mut labels: Vec<String> = labels.iter().map(|s| (*s).to_owned()).collect();
            labels.sort();
            labels.dedup();
            let tokens = tokenize_labels(&labels);
            Self { labels, tokens }
        }
        fn names<'a>(&'a self, normal: &'a str) -> Names<'a> {
            Names {
                normal,
                labels: &self.labels,
                tokens: &self.tokens,
            }
        }
    }

    #[test]
    fn alias_tiers_fire_in_order() {
        let t = LexicalThresholds::default();
        let kg = Side::new(&["knowledge graph", "kg"]);
        let left = kg.names("knowledge graph");
        let none = Side::new(&[]);
        assert_eq!(
            lexical_alias(&left, &none.names("knowledge graph"), &t),
            Some(AliasTier::Exact)
        );

        let shared = Side::new(&["kg"]);
        assert_eq!(
            lexical_alias(&left, &shared.names("k g"), &t),
            Some(AliasTier::SharedLabel)
        );

        // Label Jaccard of {knowledge, graph} vs {knowledge, graphs} is 1/3 < 0.5,
        // so the ratio tier (30/31) decides.
        let plural = Side::new(&["knowledge graphs"]);
        assert_eq!(
            lexical_alias(&left, &plural.names("knowledge graphs"), &t),
            Some(AliasTier::SequenceRatio)
        );

        // Upstream behaviour: a one-word label that is a full word of a
        // longer label aliases it through the initials tier.
        let prefix = Side::new(&["graph"]);
        let graph_theory = Side::new(&["graph theory"]);
        assert_eq!(
            lexical_alias(
                &graph_theory.names("graph theory"),
                &prefix.names("graph"),
                &t
            ),
            Some(AliasTier::Initials)
        );
        // Without labels the initials tier is skipped and the strict
        // word-prefix guard refuses the ratio and token tiers.
        assert_eq!(
            lexical_alias(&none.names("graph theory"), &none.names("graph"), &t),
            None
        );

        let other = Side::new(&["blockchain"]);
        assert_eq!(lexical_alias(&left, &other.names("blockchain"), &t), None);
    }

    /// Permissive thresholds so a refusal is the guard's doing, not the
    /// default cut-offs'.
    fn loose() -> LexicalThresholds {
        LexicalThresholds {
            label_jaccard: 0.3,
            sequence_ratio: 0.5,
            token_jaccard: 0.3,
        }
    }

    /// The tier `lexical_alias` reports for two names, each with only its own
    /// label (so the label tiers run) or with none (so the normal-form tiers
    /// run).
    fn tier_of(a: &str, b: &str, with_labels: bool, t: &LexicalThresholds) -> Option<AliasTier> {
        let (na, nb) = (
            crate::normalize::normalize_label(a),
            crate::normalize::normalize_label(b),
        );
        let (sa, sb) = if with_labels {
            (Side::new(&[&na]), Side::new(&[&nb]))
        } else {
            (Side::new(&[]), Side::new(&[]))
        };
        lexical_alias(&sa.names(&na), &sb.names(&nb), t)
    }

    #[test]
    fn digit_guard_refuses_numeric_siblings_at_every_fuzzy_tier() {
        let pairs = [
            ("ISO 9001", "ISO 14001"),
            ("Llama 3.1 8B", "Llama 3.1 70B"),
            ("Web 2.0", "Web 3.0"),
            ("IEEE 802.11", "IEEE 802.3"),
            ("GPT-4", "GPT-5"),
            ("GPT-4", "GPT-40"),
            ("Web 2.0", "Web"),
        ];
        for (a, b) in pairs {
            for t in [LexicalThresholds::default(), loose()] {
                for with_labels in [true, false] {
                    assert_eq!(tier_of(a, b, with_labels, &t), None, "{a} / {b}");
                    assert_eq!(tier_of(b, a, with_labels, &t), None, "{b} / {a}");
                }
            }
        }
    }

    #[test]
    fn the_guard_is_not_what_stops_the_controls() {
        // The same shapes without a differing number do fire each fuzzy tier,
        // so the refusals above are the guard's.
        let t = loose();
        assert_eq!(
            tier_of("Llama 3.1 Base Chat", "Llama 3.1 Base Tuned", true, &t),
            Some(AliasTier::LabelJaccard)
        );
        assert_eq!(
            tier_of("Knowledge Graph 2.0", "Knowledge Graphs 2.0", false, &t),
            Some(AliasTier::SequenceRatio)
        );
        assert_eq!(
            tier_of(
                "Graph Theory Primer",
                "Graph Theory Basics",
                false,
                &LexicalThresholds {
                    sequence_ratio: 0.99,
                    ..t
                }
            ),
            Some(AliasTier::TokenJaccard)
        );
        // `Initials` with a shared, identical number.
        assert_eq!(
            tier_of("Gpt 4 Turbo", "Gpt 4", true, &LexicalThresholds::default()),
            Some(AliasTier::Initials)
        );
    }

    #[test]
    fn exact_and_shared_label_stay_unguarded() {
        let t = LexicalThresholds::default();
        assert_eq!(
            tier_of("ISO 9001", "iso 9001", false, &t),
            Some(AliasTier::Exact)
        );
        // An author who lists `ISO 14001` as an alias of `ISO 9001` has said
        // so; the guard does not overrule an assertion.
        let left = Side::new(&["iso 9001", "iso 14001"]);
        let right = Side::new(&["iso 14001"]);
        assert_eq!(
            lexical_alias(&left.names("iso 9001"), &right.names("iso 14001"), &t),
            Some(AliasTier::SharedLabel)
        );
    }

    #[test]
    fn dotted_numbers_stay_whole_tokens() {
        fn one(s: &str) -> Vec<&str> {
            numeric_tokens(label_tokens(s))
        }
        assert_eq!(one("llama 3.1 8b"), vec!["3.1", "8b"]);
        assert_eq!(one("ieee 802.11"), vec!["802.11"]);
        assert_eq!(one("web 2.0."), vec!["2.0"]);
        assert!(!numeric_tokens_differ(
            label_tokens("a 3 1"),
            label_tokens("1 a 3")
        ));
        assert!(numeric_tokens_differ(
            label_tokens("v 3.1"),
            label_tokens("v 3 1")
        ));
    }

    #[test]
    fn names_differing_only_in_words_are_unchanged() {
        let t = LexicalThresholds::default();
        let plural = tier_of("Knowledge Graph", "Knowledge Graphs", false, &t);
        assert_eq!(plural, Some(AliasTier::SequenceRatio));
    }
}
