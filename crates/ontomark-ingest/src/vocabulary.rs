//! The vocabulary a model may use, and grounding of names against it.

use std::collections::BTreeSet;

use ontomark_model::{DocId, Iri, Kind, Model, Profile, Resolution};
use serde::{Deserialize, Serialize};

use crate::normalize::normalize_label;
use crate::similarity::{
    labels_differ_only_by_initials, lexical_alias, AliasTier, LexicalThresholds, Names,
};

/// One entity the model may name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Term {
    /// Its IRI.
    pub iri: Iri,
    /// Its kind.
    pub kind: Kind,
    /// The note declaring it.
    pub document: DocId,
    /// Its label.
    pub label: String,
    /// Its aliases.
    pub aliases: Vec<String>,
    /// `true` when several notes claim the IRI.
    pub conflicted: bool,
    /// Normalised primary form (the label).
    #[serde(skip)]
    normal: String,
    /// Normalised label, aliases and note name, deduplicated and sorted.
    #[serde(skip)]
    names: Vec<String>,
}

impl Term {
    /// The Obsidian note name (file name without `.md`) a wikilink targets.
    #[must_use]
    pub fn note_name(&self) -> &str {
        note_name(&self.document)
    }
}

/// `DocId::new` already drops `.md`, so the name is the link target.
pub(crate) fn note_name(id: &DocId) -> &str {
    id.name()
}

/// A near miss offered when a name does not ground.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    /// The entity.
    pub iri: Iri,
    /// Its label.
    pub label: String,
    /// Jaro-Winkler similarity of the best name, in `0.0..=1.0`, rounded to
    /// four decimal places so it survives any host's JSON float parsing.
    pub score: f64,
}

/// The outcome of grounding one name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Grounding {
    /// Exactly one entity, and the tier that decided it.
    Resolved {
        /// The entity.
        iri: Iri,
        /// How it matched.
        tier: AliasTier,
    },
    /// More than one entity matched, or the one that did is claimed by
    /// several notes.
    Ambiguous {
        /// The candidates, sorted.
        candidates: Vec<Iri>,
    },
    /// The name is identical to an entity's except for a conflicting
    /// initial, which authors write to tell two things apart.
    InitialsConflict {
        /// The entity it must not be merged with.
        with: Iri,
    },
    /// Nothing matched.
    Ungrounded {
        /// The closest names, best first.
        suggestions: Vec<Suggestion>,
    },
}

/// Every entity of a model, indexed for grounding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Vocabulary {
    terms: Vec<Term>,
}

impl Vocabulary {
    /// Derive the vocabulary from a built model. Terms are sorted by IRI.
    #[must_use]
    pub fn from_model(_profile: &dyn Profile, model: &Model) -> Self {
        let mut terms: Vec<Term> = model
            .entities
            .values()
            .map(|e| {
                let mut names: BTreeSet<String> = BTreeSet::new();
                names.insert(normalize_label(&e.label));
                names.extend(e.aliases.iter().map(|a| normalize_label(a)));
                names.insert(normalize_label(note_name(&e.document)));
                names.retain(|n| !n.is_empty());
                Term {
                    iri: e.iri.clone(),
                    kind: e.kind,
                    document: e.document.clone(),
                    label: e.label.clone(),
                    aliases: e.aliases.clone(),
                    conflicted: model.conflicts.contains_key(&e.iri),
                    normal: normalize_label(&e.label),
                    names: names.into_iter().collect(),
                }
            })
            .collect();
        terms.sort_by(|a, b| a.iri.cmp(&b.iri));
        Self { terms }
    }

    /// The terms, sorted by IRI.
    #[must_use]
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// The term for an IRI.
    #[must_use]
    pub fn term(&self, iri: &Iri) -> Option<&Term> {
        self.terms
            .binary_search_by(|t| t.iri.cmp(iri))
            .ok()
            .map(|i| &self.terms[i])
    }

    /// Ground a name: the model's own resolution first (label, alias or
    /// note name, case-folded), then the lexical tiers, then suggestions.
    ///
    /// Where OntoCast merges every pair a tier accepts into one cluster,
    /// grounding must choose one target, so the terms matching at the
    /// strongest tier win and only a tie at that tier is ambiguous: `kg`
    /// resolves to *Knowledge graph* (shared label) although *Graph* would
    /// also alias it through the weaker initials tier.
    #[must_use]
    pub fn ground(
        &self,
        profile: &dyn Profile,
        model: &Model,
        name: &str,
        thresholds: &LexicalThresholds,
        suggestions: usize,
    ) -> Grounding {
        match model.resolve_with(profile, name).state {
            Resolution::Resolved(iri) => {
                return Grounding::Resolved {
                    iri,
                    tier: AliasTier::Exact,
                }
            }
            Resolution::Ambiguous(c) => {
                let mut candidates = c;
                candidates.sort();
                return Grounding::Ambiguous { candidates };
            }
            // An unclaimed IRI or CURIE from the model is an invented
            // identifier; the name tiers below treat it as text.
            Resolution::External(_) | Resolution::Unresolved => {}
        }
        let normal = normalize_label(name);
        if normal.is_empty() {
            return Grounding::Ungrounded {
                suggestions: Vec::new(),
            };
        }
        let left_labels = [normal.clone()];
        let left = Names {
            normal: &normal,
            labels: &left_labels,
        };
        let mut matches: Vec<(&Term, AliasTier)> = Vec::new();
        let mut initials_conflict: Option<&Term> = None;
        for term in &self.terms {
            // A term the name matches outright is never an initials
            // conflict, whatever else its aliases look like.
            let outright = term.normal == normal || term.names.binary_search(&normal).is_ok();
            if !outright
                && labels_differ_only_by_initials(
                    left_labels.iter().map(String::as_str),
                    term.names.iter().map(String::as_str),
                )
            {
                initials_conflict.get_or_insert(term);
                continue;
            }
            let right = Names {
                normal: &term.normal,
                labels: &term.names,
            };
            if let Some(tier) = lexical_alias(&left, &right, thresholds) {
                matches.push((term, tier));
            }
        }
        if let Some(best) = matches.iter().map(|(_, t)| *t).min() {
            matches.retain(|(_, t)| *t == best);
            // A weak match does not outrank a conflicting initial: `Company
            // U` against `Company S` is withheld even if some other term
            // shares enough tokens to pass a similarity tier.
            if best > AliasTier::SharedLabel {
                if let Some(term) = initials_conflict {
                    return Grounding::InitialsConflict {
                        with: term.iri.clone(),
                    };
                }
            }
        }
        match matches.len() {
            1 => {
                let (term, tier) = matches[0];
                if term.conflicted {
                    Grounding::Ambiguous {
                        candidates: vec![term.iri.clone()],
                    }
                } else {
                    Grounding::Resolved {
                        iri: term.iri.clone(),
                        tier,
                    }
                }
            }
            0 => match initials_conflict {
                Some(term) => Grounding::InitialsConflict {
                    with: term.iri.clone(),
                },
                None => Grounding::Ungrounded {
                    suggestions: self.suggest(&normal, suggestions),
                },
            },
            _ => Grounding::Ambiguous {
                candidates: matches.iter().map(|(t, _)| t.iri.clone()).collect(),
            },
        }
    }

    fn suggest(&self, normal: &str, n: usize) -> Vec<Suggestion> {
        if n == 0 {
            return Vec::new();
        }
        let mut scored: Vec<Suggestion> = self
            .terms
            .iter()
            .filter_map(|t| {
                let score = t
                    .names
                    .iter()
                    .map(|name| strsim::jaro_winkler(normal, name))
                    .fold(0.0f64, f64::max);
                (score > 0.0).then(|| Suggestion {
                    iri: t.iri.clone(),
                    label: t.label.clone(),
                    score: (score * 10_000.0).round() / 10_000.0,
                })
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.iri.cmp(&b.iri))
        });
        scored.truncate(n);
        scored
    }
}
