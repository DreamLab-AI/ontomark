//! The reasoner seam: EL axioms in, a resumable run out.
//!
//! A host runs the engine inside a Web Worker. A synchronous call into wasm
//! cannot observe `postMessage`, so classification is a [`ReasonerRun`] that
//! advances in bounded [`ReasonerRun::step`]s; between steps the worker drains
//! its message queue, and an edit that arrives in the meantime makes the next
//! step stale (EXP-06). No step reads a clock.
//!
//! The reasoner is [`WhelkEl`]: `ontomark-reason`'s OWL 2 EL classifier over
//! `whelk-el` (WP-B), driven through its sliced `session::Job`. Axioms outside
//! the profile, or in EL but not implemented, are refused whole and reported
//! in [`Classification::unsupported`]; [`Classification::complete`] is `false`
//! whenever anything was refused or the complete policy ran out of budget.

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
    /// Existential policy actually used: `all` (complete) or `relevant`
    /// (named subsumptions complete; unsatisfiability through dropped
    /// existentials not propagated), the latter only after `all` ran out of budget.
    pub policy: String,
    /// `true` when nothing was refused and the complete policy finished.
    pub complete: bool,
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
    /// The run stopped without a result.
    Stopped {
        /// Stable code: `reasoning_budget_exceeded` or `cancelled`.
        code: String,
        /// Work done before stopping.
        steps: u64,
    },
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

/// `ontomark-reason`'s OWL 2 EL classifier, stepped. See the module documentation.
#[derive(Debug, Clone, Copy)]
pub struct WhelkEl {
    /// Saturation-step budget for the complete (`All`) policy.
    pub complete_budget: u64,
    /// Saturation-step budget for the `Relevant` fallback.
    pub relevant_budget: u64,
}

impl Default for WhelkEl {
    fn default() -> Self {
        // WP-B: a 20,000-step slice is ~25 ms natively; the estate corpus needs
        // ~0.3 M steps under Relevant and does not finish under All.
        Self {
            complete_budget: 5_000_000,
            relevant_budget: 20_000_000,
        }
    }
}

impl Reasoner for WhelkEl {
    fn info(&self) -> ReasonerInfo {
        ReasonerInfo {
            id: "ontomark-reason/whelk-el".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            el_complete: true,
            note: "OWL 2 EL classification (whelk-el). Refused constructs are listed, never \
                   weakened; individuals are typed from the class hierarchy."
                .into(),
        }
    }

    fn start(&self, axioms: &[ElAxiom]) -> Box<dyn ReasonerRun> {
        let input: Vec<ElAxiom> = axioms.to_vec();
        let converted: Vec<ra::Axiom> = input.iter().map(to_reason_axiom).collect();
        let job = new_job(&converted, ExistentialPolicy::All, self.complete_budget);
        Box::new(WhelkRun {
            info: self.info(),
            input,
            converted,
            job,
            relevant: false,
            relevant_budget: self.relevant_budget,
            done: None,
        })
    }
}

use ontomark_reason::axiom as ra;
use ontomark_reason::classify::Stopped;
use ontomark_reason::session::{Job, Session, Step};
use ontomark_reason::{ExistentialPolicy, Options};

fn new_job(axioms: &[ra::Axiom], policy: ExistentialPolicy, budget: u64) -> Job {
    let ticket = Session::new().begin();
    Job::new(
        ticket,
        axioms,
        Options {
            existentials: policy,
            max_steps: Some(budget),
        },
    )
}

fn to_class(c: &ClassExpr) -> ra::ClassExpression {
    match c {
        ClassExpr::Named { iri } => ra::ClassExpression::Class(iri.clone()),
        ClassExpr::Top => ra::ClassExpression::Thing,
        ClassExpr::Bottom => ra::ClassExpression::Nothing,
        ClassExpr::Some { property, filler } => ra::ClassExpression::Some {
            property: ra::Property::named(property.clone()),
            filler: Box::new(to_class(filler)),
        },
        ClassExpr::And { operands } => {
            ra::ClassExpression::And(operands.iter().map(to_class).collect())
        }
    }
}

fn to_reason_axiom(a: &ElAxiom) -> ra::Axiom {
    match a {
        ElAxiom::SubClassOf { sub, sup } => ra::Axiom::SubClassOf {
            sub: to_class(sub),
            sup: to_class(sup),
        },
        ElAxiom::EquivalentClasses { members } => {
            ra::Axiom::EquivalentClasses(members.iter().map(to_class).collect())
        }
        ElAxiom::DisjointClasses { members } => {
            ra::Axiom::DisjointClasses(members.iter().map(to_class).collect())
        }
        ElAxiom::SubPropertyOf { sub, sup } => ra::Axiom::SubPropertyOf {
            sub: ra::Property::named(sub.clone()),
            sup: ra::Property::named(sup.clone()),
        },
        ElAxiom::Domain { property, class } => ra::Axiom::PropertyDomain {
            property: ra::Property::named(property.clone()),
            class: to_class(class),
        },
        ElAxiom::Range { property, class } => ra::Axiom::PropertyRange {
            property: ra::Property::named(property.clone()),
            class: to_class(class),
        },
    }
}

struct WhelkRun {
    info: ReasonerInfo,
    input: Vec<ElAxiom>,
    converted: Vec<ra::Axiom>,
    job: Job,
    relevant: bool,
    relevant_budget: u64,
    done: Option<Classification>,
}

impl WhelkRun {
    fn finish(&self, c: &ontomark_reason::Classification) -> Classification {
        let inferred: BTreeSet<(&str, &str)> = c
            .inferred
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let all: BTreeSet<(&str, &str)> = c
            .subsumptions
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let mut subsumptions: Vec<Subsumption> = c
            .subsumptions
            .iter()
            .map(|(sub, sup)| Subsumption {
                sub: sub.clone(),
                sup: sup.clone(),
                told: !inferred.contains(&(sub.as_str(), sup.as_str())),
            })
            .collect();
        subsumptions.sort();
        let mut groups: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for &(a, b) in &all {
            if all.contains(&(b, a)) {
                groups.entry(a).or_default().insert(b);
            }
        }
        let equivalences: BTreeSet<Vec<String>> = groups
            .iter()
            .map(|(a, rest)| {
                let mut g: Vec<String> = rest.iter().map(|s| (*s).to_owned()).collect();
                g.push((*a).to_owned());
                g.sort();
                g
            })
            .collect();
        let mut unsupported: Vec<Unsupported> = c
            .refusals
            .iter()
            .filter_map(|r| {
                self.input.get(r.index).map(|axiom| Unsupported {
                    axiom: axiom.clone(),
                    reason: format!("{}: {} ({})", r.code(), r.reason, r.construct),
                })
            })
            .collect();
        unsupported.sort();
        let mut unsatisfiable = c.unsatisfiable.clone();
        unsatisfiable.sort();
        Classification {
            reasoner: self.info.clone(),
            subsumptions,
            equivalences: equivalences.into_iter().collect(),
            unsatisfiable,
            complete: unsupported.is_empty() && !self.relevant,
            unsupported,
            policy: if self.relevant { "relevant" } else { "all" }.into(),
        }
    }
}

impl ReasonerRun for WhelkRun {
    fn step(&mut self, budget: u64) -> Progress {
        if let Some(done) = &self.done {
            return Progress::Done(done.clone());
        }
        match self.job.step(budget.max(1)) {
            Step::Pending { steps } => Progress::Pending { steps, total: None },
            Step::Done(outcome) => {
                let done = self.finish(&outcome.classification);
                self.done = Some(done.clone());
                Progress::Done(done)
            }
            Step::Stopped(Stopped::OverBudget { steps }) if !self.relevant => {
                // The complete policy did not fit: classify named subsumptions with Relevant.
                self.relevant = true;
                self.job = new_job(
                    &self.converted,
                    ExistentialPolicy::Relevant,
                    self.relevant_budget,
                );
                Progress::Pending { steps, total: None }
            }
            Step::Stopped(Stopped::OverBudget { steps }) => Progress::Stopped {
                code: "reasoning_budget_exceeded".into(),
                steps,
            },
            Step::Stopped(Stopped::Cancelled { steps }) => Progress::Stopped {
                code: "cancelled".into(),
                steps,
            },
            Step::Spent => Progress::Stopped {
                code: "cancelled".into(),
                steps: 0,
            },
        }
    }
}

/// Run `reasoner` to completion on `axioms` (for hosts that do not need stepping).
///
/// # Errors
/// The stop code when the run stopped without a result.
pub fn classify_all(reasoner: &dyn Reasoner, axioms: &[ElAxiom]) -> Result<Classification, String> {
    let mut run = reasoner.start(axioms);
    loop {
        match run.step(u64::MAX) {
            Progress::Done(c) => return Ok(c),
            Progress::Stopped { code, .. } => return Err(code),
            Progress::Pending { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(a: &str) -> ClassExpr {
        ClassExpr::named(a)
    }

    fn sub(a: &str, b: &str) -> ElAxiom {
        ElAxiom::SubClassOf {
            sub: named(a),
            sup: named(b),
        }
    }

    fn some(p: &str, c: ClassExpr) -> ClassExpr {
        ClassExpr::Some {
            property: p.into(),
            filler: Box::new(c),
        }
    }

    #[test]
    fn subsumption_is_transitive_and_marks_told_edges() {
        let c = classify_all(
            &WhelkEl::default(),
            &[sub("urn:a", "urn:b"), sub("urn:b", "urn:c")],
        )
        .unwrap();
        let pairs: Vec<_> = c
            .subsumptions
            .iter()
            .map(|s| (s.sub.as_str(), s.sup.as_str(), s.told))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("urn:a", "urn:b", true),
                ("urn:a", "urn:c", false),
                ("urn:b", "urn:c", true)
            ]
        );
        assert!(c.reasoner.el_complete && c.complete);
        assert_eq!(c.policy, "all");
    }

    #[test]
    fn existential_reasoning_goes_beyond_a_told_closure() {
        // A ⊑ ∃p.B, B ⊑ C, D ≡ ∃p.C  ⊢  A ⊑ D (a closure over told edges cannot derive it).
        let axioms = [
            ElAxiom::SubClassOf {
                sub: named("urn:A"),
                sup: some("urn:p", named("urn:B")),
            },
            sub("urn:B", "urn:C"),
            ElAxiom::EquivalentClasses {
                members: vec![named("urn:D"), some("urn:p", named("urn:C"))],
            },
        ];
        let c = classify_all(&WhelkEl::default(), &axioms).unwrap();
        assert!(
            c.subsumptions
                .iter()
                .any(|s| s.sub == "urn:A" && s.sup == "urn:D" && !s.told),
            "{:?}",
            c.subsumptions
        );
    }

    #[test]
    fn disjoint_superclasses_make_a_class_unsatisfiable() {
        let disj = ElAxiom::DisjointClasses {
            members: vec![named("urn:b"), named("urn:c")],
        };
        let c = classify_all(
            &WhelkEl::default(),
            &[
                sub("urn:a", "urn:b"),
                sub("urn:a", "urn:c"),
                sub("urn:d", "urn:a"),
                disj,
            ],
        )
        .unwrap();
        assert_eq!(
            c.unsatisfiable,
            vec!["urn:a".to_owned(), "urn:d".to_owned()]
        );
    }

    #[test]
    fn cycles_are_equivalences() {
        let c = classify_all(
            &WhelkEl::default(),
            &[sub("urn:a", "urn:b"), sub("urn:b", "urn:a")],
        )
        .unwrap();
        assert_eq!(
            c.equivalences,
            vec![vec!["urn:a".to_owned(), "urn:b".to_owned()]]
        );
    }

    #[test]
    fn refused_axioms_are_reported_and_mark_the_result_incomplete() {
        let range = ElAxiom::Range {
            property: "urn:p".into(),
            class: named("urn:b"),
        };
        let c = classify_all(&WhelkEl::default(), &[sub("urn:a", "urn:b"), range.clone()]).unwrap();
        assert_eq!(c.unsupported.len(), 1);
        assert_eq!(c.unsupported[0].axiom, range);
        assert!(
            c.unsupported[0].reason.starts_with("EL_NOT_IMPLEMENTED"),
            "{}",
            c.unsupported[0].reason
        );
        assert!(!c.complete);
    }

    #[test]
    fn stepping_gives_the_same_result_as_one_shot() {
        let axioms: Vec<_> = (0..40)
            .map(|i| sub(&format!("urn:c{i}"), &format!("urn:c{}", i + 1)))
            .collect();
        let mut run = WhelkEl::default().start(&axioms);
        let mut pendings = 0;
        let stepped = loop {
            match run.step(3) {
                Progress::Pending { .. } => pendings += 1,
                Progress::Done(c) => break c,
                Progress::Stopped { code, .. } => panic!("{code}"),
            }
        };
        assert!(
            pendings >= 3,
            "a budget of 3 steps must take several slices"
        );
        assert_eq!(stepped, classify_all(&WhelkEl::default(), &axioms).unwrap());
    }

    #[test]
    fn complete_policy_over_budget_falls_back_to_relevant() {
        let axioms: Vec<_> = (0..40)
            .map(|i| sub(&format!("urn:c{i}"), &format!("urn:c{}", i + 1)))
            .collect();
        let tight = WhelkEl {
            complete_budget: 5,
            relevant_budget: 10_000_000,
        };
        let c = classify_all(&tight, &axioms).unwrap();
        assert_eq!(c.policy, "relevant");
        assert!(!c.complete);
        assert_eq!(
            c.subsumptions,
            classify_all(&WhelkEl::default(), &axioms)
                .unwrap()
                .subsumptions
        );
        let none = WhelkEl {
            complete_budget: 5,
            relevant_budget: 5,
        };
        assert_eq!(
            classify_all(&none, &axioms).unwrap_err(),
            "reasoning_budget_exceeded"
        );
    }
}
