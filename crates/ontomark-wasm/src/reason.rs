//! The reasoner seam: EL axioms in, a resumable run out.
//!
//! A host runs the engine inside a Web Worker. A synchronous call into wasm
//! cannot observe `postMessage`, so classification is a [`ReasonerRun`] that
//! advances in bounded [`ReasonerRun::step`]s; between steps the worker drains
//! its message queue, and an edit that arrives in the meantime makes the next
//! step stale (EXP-06). No step reads a clock.
//!
//! The production reasoner is `ontomark-reason` over `whelk-el` (WP-B).
//! [`ToldClosure`] is the **interim** implementation used until that crate
//! publishes a stepped API: it computes the reflexive-transitive closure of
//! named `SubClassOf`/`EquivalentClasses` and the disjointness clashes that
//! follow, reports every other axiom as unsupported, and declares itself
//! `el_complete: false`. It is never presented as EL classification (EXP-01).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// An EL class expression over IRIs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClassExpr {
    /// A named class.
    Named {
        /// Class IRI.
        iri: String,
    },
    /// `owl:Thing`.
    Top,
    /// `owl:Nothing`.
    Bottom,
    /// `ObjectSomeValuesFrom(property, filler)`.
    Some {
        /// Property IRI.
        property: String,
        /// Filler expression.
        filler: Box<ClassExpr>,
    },
    /// `ObjectIntersectionOf(..)`.
    And {
        /// Operands.
        operands: Vec<ClassExpr>,
    },
}

impl ClassExpr {
    /// A named class expression.
    #[must_use]
    pub fn named(iri: impl Into<String>) -> Self {
        Self::Named { iri: iri.into() }
    }

    fn as_named(&self) -> Option<&str> {
        match self {
            Self::Named { iri } => Some(iri),
            _ => None,
        }
    }
}

/// An EL axiom.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ElAxiom {
    /// `SubClassOf(sub, sup)`.
    SubClassOf {
        /// Subclass.
        sub: ClassExpr,
        /// Superclass.
        sup: ClassExpr,
    },
    /// `EquivalentClasses(..)`.
    EquivalentClasses {
        /// Members.
        members: Vec<ClassExpr>,
    },
    /// `DisjointClasses(..)`.
    DisjointClasses {
        /// Members.
        members: Vec<ClassExpr>,
    },
    /// `SubObjectPropertyOf(sub, sup)`.
    SubPropertyOf {
        /// Sub-property IRI.
        sub: String,
        /// Super-property IRI.
        sup: String,
    },
    /// `ObjectPropertyDomain(property, class)`.
    Domain {
        /// Property IRI.
        property: String,
        /// Domain.
        class: ClassExpr,
    },
    /// `ObjectPropertyRange(property, class)`.
    Range {
        /// Property IRI.
        property: String,
        /// Range.
        class: ClassExpr,
    },
}

/// Who produced a classification and what it can be trusted for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonerInfo {
    /// Stable identifier.
    pub id: String,
    /// Implementation version.
    pub version: String,
    /// `true` only for a reasoner that is complete for the OWL 2 EL profile.
    pub el_complete: bool,
    /// Plain statement of the limits, shown to the user.
    pub note: String,
}

/// One entailed subsumption between named classes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Subsumption {
    /// Subclass IRI.
    pub sub: String,
    /// Superclass IRI.
    pub sup: String,
    /// `true` when the subsumption was asserted directly.
    pub told: bool,
}

/// An axiom the reasoner did not use, and why.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Unsupported {
    /// The axiom.
    pub axiom: ElAxiom,
    /// Why it was not used.
    pub reason: String,
}

/// The result of a completed run. Every list is sorted, so the JSON form is
/// deterministic across native and wasm builds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Classification {
    /// Reasoner identity and limits.
    pub reasoner: ReasonerInfo,
    /// Entailed non-reflexive subsumptions between named classes (not `owl:Thing`).
    pub subsumptions: Vec<Subsumption>,
    /// Sets of mutually equivalent named classes (size ≥ 2).
    pub equivalences: Vec<Vec<String>>,
    /// Unsatisfiable named classes.
    pub unsatisfiable: Vec<String>,
    /// Axioms outside the reasoner's supported fragment.
    pub unsupported: Vec<Unsupported>,
}

/// Progress of a [`ReasonerRun`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// More work remains; `steps` units have been done so far.
    Pending {
        /// Units of work done so far.
        steps: u64,
        /// Units of work in total, when known.
        total: Option<u64>,
    },
    /// The run finished.
    Done(Classification),
}

/// A classification in progress.
pub trait ReasonerRun {
    /// Do at most `budget` units of work (`budget` 0 is treated as 1).
    fn step(&mut self, budget: u64) -> Progress;
}

/// A reasoner the engine can start runs on.
pub trait Reasoner {
    /// Identity and limits.
    fn info(&self) -> ReasonerInfo;
    /// Begin classifying `axioms`. No work happens until the first step.
    fn start(&self, axioms: &[ElAxiom]) -> Box<dyn ReasonerRun>;
}

/// The interim told-closure reasoner. See the module documentation.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToldClosure;

impl Reasoner for ToldClosure {
    fn info(&self) -> ReasonerInfo {
        ReasonerInfo {
            id: "interim-told-closure".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            el_complete: false,
            note: "Interim, not OWL 2 EL: transitive closure of named subclass and \
                   equivalence assertions plus disjointness clashes. Existential and \
                   intersection axioms are reported as unsupported. Replaced by \
                   ontomark-reason (whelk-el)."
                .into(),
        }
    }

    fn start(&self, axioms: &[ElAxiom]) -> Box<dyn ReasonerRun> {
        Box::new(ToldRun::new(self.info(), axioms))
    }
}

struct ToldRun {
    info: ReasonerInfo,
    classes: Vec<String>,
    told_up: BTreeMap<String, BTreeSet<String>>,
    disjoint: Vec<(String, String)>,
    unsupported: Vec<Unsupported>,
    ancestors: BTreeMap<String, BTreeSet<String>>,
    cursor: usize,
    done: Option<Classification>,
}

impl ToldRun {
    fn new(info: ReasonerInfo, axioms: &[ElAxiom]) -> Self {
        let mut told_up: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut classes = BTreeSet::new();
        let mut disjoint = Vec::new();
        let mut unsupported = Vec::new();
        let unsupported_reason = "outside the interim told-closure fragment";
        for axiom in axioms {
            match axiom {
                ElAxiom::SubClassOf { sub, sup } => match (sub.as_named(), sup) {
                    (Some(a), ClassExpr::Named { iri: b }) => {
                        classes.insert(a.to_owned());
                        classes.insert(b.clone());
                        told_up.entry(a.to_owned()).or_default().insert(b.clone());
                    }
                    (Some(a), ClassExpr::Bottom) => {
                        classes.insert(a.to_owned());
                        told_up
                            .entry(a.to_owned())
                            .or_default()
                            .insert(NOTHING.into());
                    }
                    (Some(a), ClassExpr::Top) => {
                        classes.insert(a.to_owned());
                    }
                    _ => unsupported.push(Unsupported {
                        axiom: axiom.clone(),
                        reason: unsupported_reason.into(),
                    }),
                },
                ElAxiom::EquivalentClasses { members } => {
                    let named: Option<Vec<&str>> =
                        members.iter().map(ClassExpr::as_named).collect();
                    match named {
                        Some(named) if named.len() >= 2 => {
                            for &a in &named {
                                classes.insert(a.to_owned());
                                for &b in &named {
                                    if a != b {
                                        told_up
                                            .entry(a.to_owned())
                                            .or_default()
                                            .insert(b.to_owned());
                                    }
                                }
                            }
                        }
                        _ => unsupported.push(Unsupported {
                            axiom: axiom.clone(),
                            reason: unsupported_reason.into(),
                        }),
                    }
                }
                ElAxiom::DisjointClasses { members } => {
                    let named: Option<Vec<&str>> =
                        members.iter().map(ClassExpr::as_named).collect();
                    match named {
                        Some(named) => {
                            for (i, a) in named.iter().enumerate() {
                                classes.insert((*a).to_owned());
                                for b in &named[i + 1..] {
                                    disjoint.push(((*a).to_owned(), (*b).to_owned()));
                                }
                            }
                        }
                        None => unsupported.push(Unsupported {
                            axiom: axiom.clone(),
                            reason: unsupported_reason.into(),
                        }),
                    }
                }
                ElAxiom::SubPropertyOf { .. } | ElAxiom::Domain { .. } | ElAxiom::Range { .. } => {
                    unsupported.push(Unsupported {
                        axiom: axiom.clone(),
                        reason: unsupported_reason.into(),
                    });
                }
            }
        }
        classes.remove(NOTHING);
        unsupported.sort();
        Self {
            info,
            classes: classes.into_iter().collect(),
            told_up,
            disjoint,
            unsupported,
            ancestors: BTreeMap::new(),
            cursor: 0,
            done: None,
        }
    }

    fn ancestors_of(&self, class: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![class.to_owned()];
        while let Some(c) = stack.pop() {
            if let Some(ups) = self.told_up.get(&c) {
                for up in ups {
                    if seen.insert(up.clone()) {
                        stack.push(up.clone());
                    }
                }
            }
        }
        seen.remove(class);
        seen
    }

    fn finish(&mut self) -> Classification {
        let mut unsatisfiable = BTreeSet::new();
        for class in &self.classes {
            let ups = &self.ancestors[class];
            let in_self = |x: &String| x == class || ups.contains(x);
            let clash = ups.contains(NOTHING)
                || self.disjoint.iter().any(|(a, b)| in_self(a) && in_self(b));
            if clash {
                unsatisfiable.insert(class.clone());
            }
        }
        let mut subsumptions = Vec::new();
        let mut equivalences: BTreeSet<Vec<String>> = BTreeSet::new();
        for class in &self.classes {
            let told = self.told_up.get(class);
            for sup in &self.ancestors[class] {
                if sup == NOTHING {
                    continue;
                }
                subsumptions.push(Subsumption {
                    sub: class.clone(),
                    sup: sup.clone(),
                    told: told.is_some_and(|t| t.contains(sup)),
                });
            }
            let mut group: Vec<String> = self.ancestors[class]
                .iter()
                .filter(|sup| {
                    *sup != NOTHING && self.ancestors.get(*sup).is_some_and(|a| a.contains(class))
                })
                .cloned()
                .collect();
            if !group.is_empty() {
                group.push(class.clone());
                group.sort();
                equivalences.insert(group);
            }
        }
        subsumptions.sort();
        Classification {
            reasoner: self.info.clone(),
            subsumptions,
            equivalences: equivalences.into_iter().collect(),
            unsatisfiable: unsatisfiable.into_iter().collect(),
            unsupported: self.unsupported.clone(),
        }
    }
}

const NOTHING: &str = "http://www.w3.org/2002/07/owl#Nothing";

impl ReasonerRun for ToldRun {
    fn step(&mut self, budget: u64) -> Progress {
        if let Some(done) = &self.done {
            return Progress::Done(done.clone());
        }
        let budget = usize::try_from(budget.max(1)).unwrap_or(usize::MAX);
        let end = self.cursor.saturating_add(budget).min(self.classes.len());
        while self.cursor < end {
            let class = self.classes[self.cursor].clone();
            let ups = self.ancestors_of(&class);
            self.ancestors.insert(class, ups);
            self.cursor += 1;
        }
        if self.cursor < self.classes.len() {
            return Progress::Pending {
                steps: self.cursor as u64,
                total: Some(self.classes.len() as u64),
            };
        }
        let done = self.finish();
        self.done = Some(done.clone());
        Progress::Done(done)
    }
}

/// Run `reasoner` to completion on `axioms` (for hosts that do not need stepping).
#[must_use]
pub fn classify_all(reasoner: &dyn Reasoner, axioms: &[ElAxiom]) -> Classification {
    let mut run = reasoner.start(axioms);
    loop {
        if let Progress::Done(c) = run.step(u64::MAX) {
            return c;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(a: &str, b: &str) -> ElAxiom {
        ElAxiom::SubClassOf {
            sub: ClassExpr::named(a),
            sup: ClassExpr::named(b),
        }
    }

    #[test]
    fn closure_is_transitive_and_marks_told_edges() {
        let c = classify_all(&ToldClosure, &[sub("a", "b"), sub("b", "c")]);
        let pairs: Vec<_> = c
            .subsumptions
            .iter()
            .map(|s| (s.sub.as_str(), s.sup.as_str(), s.told))
            .collect();
        assert_eq!(
            pairs,
            vec![("a", "b", true), ("a", "c", false), ("b", "c", true)]
        );
        assert!(!c.reasoner.el_complete);
    }

    #[test]
    fn disjoint_ancestors_make_a_class_unsatisfiable() {
        let disj = ElAxiom::DisjointClasses {
            members: vec![ClassExpr::named("b"), ClassExpr::named("c")],
        };
        let c = classify_all(
            &ToldClosure,
            &[sub("a", "b"), sub("a", "c"), sub("d", "a"), disj],
        );
        assert_eq!(c.unsatisfiable, vec!["a".to_owned(), "d".to_owned()]);
    }

    #[test]
    fn cycles_are_equivalences() {
        let c = classify_all(&ToldClosure, &[sub("a", "b"), sub("b", "a")]);
        assert_eq!(c.equivalences, vec![vec!["a".to_owned(), "b".to_owned()]]);
    }

    #[test]
    fn existentials_are_reported_unsupported_not_dropped() {
        let ex = ElAxiom::SubClassOf {
            sub: ClassExpr::named("a"),
            sup: ClassExpr::Some {
                property: "p".into(),
                filler: Box::new(ClassExpr::named("b")),
            },
        };
        let c = classify_all(&ToldClosure, std::slice::from_ref(&ex));
        assert_eq!(c.unsupported.len(), 1);
        assert_eq!(c.unsupported[0].axiom, ex);
    }

    #[test]
    fn stepping_reports_progress_then_the_same_result_as_one_shot() {
        let axioms: Vec<_> = (0..10)
            .map(|i| sub(&format!("c{i}"), &format!("c{}", i + 1)))
            .collect();
        let mut run = ToldClosure.start(&axioms);
        let mut pendings = 0;
        let stepped = loop {
            match run.step(3) {
                Progress::Pending { .. } => pendings += 1,
                Progress::Done(c) => break c,
            }
        };
        assert!(
            pendings >= 3,
            "budget 3 over 11 classes must yield pending steps"
        );
        assert_eq!(stepped, classify_all(&ToldClosure, &axioms));
    }
}
