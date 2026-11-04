//! Answerability: how much of a question the vault vocabulary covers
//! (ADR-OM-012).
//!
//! Ingest grounds the names a model proposes for a note. The same grounder,
//! pointed at the names a question mentions, tells an agent before it
//! answers whether the corpus can support the answer, which names need
//! clarifying, and which the corpus has nothing on. Adapted from NVIDIA
//! auto-ontology's `check_answerable` (entity coverage = covered / named),
//! with two differences: the grade is lexical and deterministic (the
//! vocabulary's own tiers, no embeddings or distance threshold), and an
//! ambiguous name is reported apart from an absent one, because "which do you
//! mean?" and "the corpus has nothing on this" are different answers.
//!
//! Extracting the names from the question is the host's job: an agent
//! already reads the question, and this crate calls no model.

use std::collections::BTreeSet;

use ontomark_model::{Iri, Model, Profile};
use serde::{Deserialize, Serialize};

use crate::normalize::normalize_label;
use crate::similarity::{AliasTier, LexicalThresholds};
use crate::vocabulary::{Grounding, Suggestion, Vocabulary};

/// A name that grounded to exactly one entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GroundedName {
    /// The name as the host passed it.
    pub name: String,
    /// The entity.
    pub iri: Iri,
    /// Its label.
    pub label: String,
    /// The tier that decided it.
    pub tier: AliasTier,
}

/// A name the corpus can resolve only with the asker's help.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UnclearName {
    /// The name as the host passed it.
    pub name: String,
    /// The entities it could mean, sorted. For an initials conflict, the
    /// one entity it must not silently be taken for.
    pub candidates: Vec<Iri>,
}

/// A name nothing in the vocabulary matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UncoveredName {
    /// The name as the host passed it.
    pub name: String,
    /// The closest names, best first.
    pub suggestions: Vec<Suggestion>,
}

/// How much of a question the vocabulary covers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Coverage {
    /// Grounded names over distinct names, in `0.0..=1.0`, rounded to four
    /// places. `None` when no names were given: an empty question is not
    /// an unanswerable one, and `0.0` would read as "the corpus has nothing".
    pub coverage: Option<f64>,
    /// Names that resolved to one entity, in the order given.
    pub grounded: Vec<GroundedName>,
    /// Names that matched several entities or an initials conflict.
    pub unclear: Vec<UnclearName>,
    /// Names that matched nothing.
    pub uncovered: Vec<UncoveredName>,
}

impl Coverage {
    /// `true` when at least one name was given and every name grounded.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.coverage.is_some() && self.unclear.is_empty() && self.uncovered.is_empty()
    }
}

impl Vocabulary {
    /// Ground each of a question's names and grade the coverage.
    ///
    /// Names are de-duplicated by their normal form, keeping the first
    /// spelling; empty names are ignored. Each distinct name lands in
    /// exactly one of `grounded`, `unclear` or `uncovered`.
    #[must_use]
    pub fn coverage<S: AsRef<str>>(
        &self,
        profile: &dyn Profile,
        model: &Model,
        names: &[S],
        thresholds: &LexicalThresholds,
        suggestions: usize,
    ) -> Coverage {
        let mut seen = BTreeSet::new();
        let mut out = Coverage {
            coverage: None,
            grounded: Vec::new(),
            unclear: Vec::new(),
            uncovered: Vec::new(),
        };
        for name in names {
            let name = name.as_ref().trim();
            let normal = normalize_label(name);
            if normal.is_empty() || !seen.insert(normal) {
                continue;
            }
            let name = name.to_owned();
            match self.ground(profile, model, &name, thresholds, suggestions) {
                Grounding::Resolved { iri, tier } => {
                    let label = self.term(&iri).map(|t| t.label.clone()).unwrap_or_default();
                    out.grounded.push(GroundedName {
                        name,
                        iri,
                        label,
                        tier,
                    });
                }
                Grounding::Ambiguous { candidates } => {
                    out.unclear.push(UnclearName { name, candidates });
                }
                Grounding::InitialsConflict { with } => {
                    out.unclear.push(UnclearName {
                        name,
                        candidates: vec![with],
                    });
                }
                Grounding::Ungrounded { suggestions } => {
                    out.uncovered.push(UncoveredName { name, suggestions });
                }
            }
        }
        if !seen.is_empty() {
            #[allow(clippy::cast_precision_loss)]
            let ratio = out.grounded.len() as f64 / seen.len() as f64;
            out.coverage = Some((ratio * 10_000.0).round() / 10_000.0);
        }
        out
    }
}
