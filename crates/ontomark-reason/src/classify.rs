//! Classification: the profile check, the existential policy and the
//! saturation, packaged as a [`Classification`] that says what was entailed,
//! what is unsatisfiable and why, and what was not reasoned over.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use whelk_el::reasoner::{Progress, Saturation};
use whelk_el::{ClassExpr, ElAxiom, BOTTOM, TOP};

use crate::axiom::Axiom;
use crate::profile::{self, Refusal};

/// The profile every [`Classification`] is computed under.
pub const PROFILE: &str = "owl2-el/whelk-el";

/// Which existential restrictions in the superclass position are given to
/// the saturation.
///
/// An axiom `C ⊑ ∃R.D` contributes to a *named* subsumption only when some
/// restriction over `R` or a property `R` reaches through the property
/// hierarchy, chains or transitivity occurs on the left of an inclusion (a
/// definition `A ≡ … ⊓ ∃S.E`, a GCI `∃S.E ⊑ B`, a property domain). On a
/// corpus whose restrictions are mostly descriptive, such axioms dominate the
/// cost of saturation and entail nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExistentialPolicy {
    /// Saturate every axiom. Complete.
    #[default]
    All,
    /// Drop `C ⊑ ∃R.D` axioms (with a named `C` and no `owl:Nothing` in the
    /// filler) whose properties cannot reach any left-position restriction.
    ///
    /// Named subsumptions between satisfiable classes are the same as under
    /// [`ExistentialPolicy::All`]. Unsatisfiability is not carried back from an
    /// unsatisfiable filler `D` to `C` through a dropped axiom; `D` itself is
    /// still reported, so a clash is never hidden, only its derived
    /// casualties. [`Classification::existentials_dropped`] counts the
    /// dropped axioms.
    Relevant,
}

/// Options for [`classify_with`] and [`Job::new`](crate::session::Job::new).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Options {
    /// The existential policy.
    pub existentials: ExistentialPolicy,
    /// Abandon the saturation after this many steps instead of running on.
    /// `None` runs to the fixpoint.
    pub max_steps: Option<u64>,
}

/// One unsatisfiable class explained by a disjointness axiom.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DisjointnessViolation {
    /// The unsatisfiable class.
    pub class: String,
    /// The two classes it falls under that are declared disjoint.
    pub disjoint: (String, String),
    /// Index of the `DisjointClasses` axiom in the input.
    pub axiom: usize,
}

/// What classification concluded.
///
/// Every list is sorted and deduplicated, so equal inputs give byte-equal
/// serialisations on every target.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Classification {
    /// The profile and backend, [`PROFILE`].
    pub profile: String,
    /// The existential policy the saturation ran under.
    pub existentials: ExistentialPolicy,
    /// Every entailed `(sub, sup)` between distinct named classes, excluding
    /// `owl:Thing` and `owl:Nothing` on either side.
    pub subsumptions: Vec<(String, String)>,
    /// The [`Self::subsumptions`] not asserted as `SubClassOf(Class, Class)`.
    pub inferred: Vec<(String, String)>,
    /// Named classes entailed to be `owl:Nothing`.
    pub unsatisfiable: Vec<String>,
    /// The unsatisfiable classes that are not explained by an unsatisfiable
    /// asserted superclass: the ones to fix.
    pub root_causes: Vec<String>,
    /// For each unsatisfiable class, each declared disjoint pair it falls
    /// under.
    pub disjointness_violations: Vec<DisjointnessViolation>,
    /// Axioms refused by the profile check and not reasoned over.
    pub refusals: Vec<Refusal>,
    /// Named classes classified (excluding `owl:Thing` and `owl:Nothing`).
    pub classes: usize,
    /// Superclass-position existential axioms dropped by the policy.
    pub existentials_dropped: usize,
    /// Saturation steps taken.
    pub steps: u64,
}

impl Classification {
    /// `true` when every axiom was reasoned over.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.refusals.is_empty()
    }

    /// `true` when no class is unsatisfiable.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.unsatisfiable.is_empty()
    }
}

/// Why a classification did not finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stopped {
    /// The step budget in [`Options::max_steps`] ran out.
    OverBudget {
        /// Steps taken before stopping.
        steps: u64,
    },
    /// The job's [`crate::session::CancelToken`] was cancelled.
    Cancelled {
        /// Steps taken before stopping.
        steps: u64,
    },
}

impl std::fmt::Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stopped::OverBudget { steps } => write!(
                f,
                "reasoning stopped at the step budget after {steps} steps"
            ),
            Stopped::Cancelled { steps } => write!(f, "reasoning cancelled after {steps} steps"),
        }
    }
}

impl std::error::Error for Stopped {}

/// Classify `axioms` with default [`Options`]: every axiom, to the fixpoint.
#[must_use]
pub fn classify(axioms: &[Axiom]) -> Classification {
    match classify_with(axioms, Options::default()) {
        Ok(c) => c,
        Err(_) => unreachable!("no budget and no cancellation"),
    }
}

/// Classify `axioms` under `options`.
///
/// # Errors
/// [`Stopped::OverBudget`] when [`Options::max_steps`] runs out first.
pub fn classify_with(axioms: &[Axiom], options: Options) -> Result<Classification, Stopped> {
    let mut job = Prepared::new(axioms, options);
    let budget = options.max_steps.unwrap_or(u64::MAX);
    match job.saturation.run(budget) {
        Progress::Done => Ok(job.finish()),
        Progress::Pending => Err(Stopped::OverBudget {
            steps: job.saturation.steps(),
        }),
    }
}

/// The checked input and the saturation over it.
pub(crate) struct Prepared {
    pub(crate) saturation: Saturation,
    options: Options,
    refusals: Vec<Refusal>,
    asserted: BTreeSet<(String, String)>,
    disjoint: Vec<(usize, Vec<String>)>,
    classes: BTreeSet<String>,
    existentials_dropped: usize,
}

impl Prepared {
    pub(crate) fn new(axioms: &[Axiom], options: Options) -> Prepared {
        let report = profile::check(axioms);
        let mut asserted = BTreeSet::new();
        let mut disjoint = Vec::new();
        let mut classes = BTreeSet::new();
        for (index, axiom) in &report.accepted {
            collect_classes(axiom, &mut classes);
            match axiom {
                ElAxiom::SubClassOf {
                    sub: ClassExpr::Named(a),
                    sup: ClassExpr::Named(b),
                } => {
                    asserted.insert((a.clone(), b.clone()));
                }
                ElAxiom::DisjointClasses(cs) => {
                    let named: Vec<String> = cs
                        .iter()
                        .filter_map(|c| match c {
                            ClassExpr::Named(iri) => Some(iri.clone()),
                            _ => None,
                        })
                        .collect();
                    disjoint.push((*index, named));
                }
                _ => {}
            }
        }
        let mut el: Vec<ElAxiom> = report.accepted.into_iter().map(|(_, a)| a).collect();
        let before = el.len();
        el = apply_policy(el, options.existentials);
        let existentials_dropped = before - el.len();
        Prepared {
            saturation: Saturation::new(&whelk_el::translate(&el)),
            options,
            refusals: report.refusals,
            asserted,
            disjoint,
            classes,
            existentials_dropped,
        }
    }

    pub(crate) fn finish(self) -> Classification {
        let steps = self.saturation.steps();
        let state = match self.saturation.into_state() {
            Ok(state) => state,
            Err(_) => unreachable!("finish is only called on a finished saturation"),
        };
        let mut subsumptions = Vec::new();
        let mut unsatisfiable = BTreeSet::new();
        for (sub, sup) in state.named_subsumption_ids() {
            if sub == BOTTOM || sub == TOP {
                continue;
            }
            if sup == BOTTOM {
                unsatisfiable.insert(sub);
                continue;
            }
            if sub == sup || sup == TOP {
                continue;
            }
            subsumptions.push((sub, sup));
        }
        let inferred = subsumptions
            .iter()
            .filter(|pair| !self.asserted.contains(*pair))
            .cloned()
            .collect();
        let root_causes = unsatisfiable
            .iter()
            .filter(|c| {
                !self
                    .asserted
                    .range(((*c).clone(), String::new())..)
                    .take_while(|(sub, _)| sub == *c)
                    .any(|(_, sup)| unsatisfiable.contains(sup))
            })
            .cloned()
            .collect();
        let mut disjointness_violations = Vec::new();
        for class in &unsatisfiable {
            // `subsumptions` is sorted by subclass: this class's superclasses
            // are one contiguous run.
            let start = subsumptions.partition_point(|(sub, _)| sub < class);
            let above: BTreeSet<&String> = subsumptions[start..]
                .iter()
                .take_while(|(sub, _)| sub == class)
                .map(|(_, sup)| sup)
                .collect();
            let falls_under = |x: &String| x == class || above.contains(x);
            for (axiom, operands) in &self.disjoint {
                for (i, a) in operands.iter().enumerate() {
                    for b in &operands[i + 1..] {
                        if falls_under(a) && falls_under(b) {
                            let pair = if a <= b {
                                (a.clone(), b.clone())
                            } else {
                                (b.clone(), a.clone())
                            };
                            disjointness_violations.push(DisjointnessViolation {
                                class: class.clone(),
                                disjoint: pair,
                                axiom: *axiom,
                            });
                        }
                    }
                }
            }
        }
        disjointness_violations.sort();
        disjointness_violations.dedup();
        Classification {
            profile: PROFILE.to_owned(),
            existentials: self.options.existentials,
            subsumptions,
            inferred,
            unsatisfiable: unsatisfiable.into_iter().collect(),
            root_causes,
            disjointness_violations,
            refusals: self.refusals,
            classes: self.classes.len(),
            existentials_dropped: self.existentials_dropped,
            steps,
        }
    }
}

fn collect_classes(axiom: &ElAxiom, out: &mut BTreeSet<String>) {
    fn walk(c: &ClassExpr, out: &mut BTreeSet<String>) {
        match c {
            ClassExpr::Named(iri) if iri != TOP && iri != BOTTOM => {
                out.insert(iri.clone());
            }
            ClassExpr::And(ops) => ops.iter().for_each(|o| walk(o, out)),
            ClassExpr::Some { filler, .. } => walk(filler, out),
            _ => {}
        }
    }
    match axiom {
        ElAxiom::DeclareClass(iri) if iri != TOP && iri != BOTTOM => {
            out.insert(iri.clone());
        }
        ElAxiom::SubClassOf { sub, sup } => {
            walk(sub, out);
            walk(sup, out);
        }
        ElAxiom::EquivalentClasses(cs) | ElAxiom::DisjointClasses(cs) => {
            cs.iter().for_each(|c| walk(c, out))
        }
        ElAxiom::PropertyDomain { class, .. } | ElAxiom::ClassAssertion { class, .. } => {
            walk(class, out)
        }
        _ => {}
    }
}

/// Properties mentioned anywhere inside `c`.
fn properties_in(c: &ClassExpr, out: &mut BTreeSet<String>) {
    match c {
        ClassExpr::Some { property, filler } => {
            out.insert(property.clone());
            properties_in(filler, out);
        }
        ClassExpr::And(ops) => ops.iter().for_each(|o| properties_in(o, out)),
        _ => {}
    }
}

fn mentions_nothing(c: &ClassExpr) -> bool {
    match c {
        ClassExpr::Nothing => true,
        ClassExpr::Named(iri) => iri == BOTTOM,
        ClassExpr::And(ops) => ops.iter().any(mentions_nothing),
        ClassExpr::Some { filler, .. } => mentions_nothing(filler),
        _ => false,
    }
}

/// A superclass-position existential `Named ⊑ (… ∃R.D …)`: its properties.
fn droppable(axiom: &ElAxiom) -> Option<BTreeSet<String>> {
    match axiom {
        ElAxiom::SubClassOf {
            sub: ClassExpr::Named(_),
            sup,
        } if !mentions_nothing(sup) => {
            let mut ps = BTreeSet::new();
            properties_in(sup, &mut ps);
            (!ps.is_empty()).then_some(ps)
        }
        _ => None,
    }
}

/// Filter checked axioms by `policy`: unchanged for
/// [`ExistentialPolicy::All`]; see [`ExistentialPolicy::Relevant`] for the
/// other.
#[must_use]
pub fn apply_policy(axioms: Vec<ElAxiom>, policy: ExistentialPolicy) -> Vec<ElAxiom> {
    match policy {
        ExistentialPolicy::All => axioms,
        ExistentialPolicy::Relevant => relevant_only(axioms),
    }
}

/// [`ExistentialPolicy::Relevant`]: keep every axiom except superclass-position
/// existentials none of whose properties is relevant.
///
/// A property is relevant when it occurs in a left-position restriction (a
/// GCI's subclass, any operand of an equivalence or disjointness, a domain,
/// an assertion), or reaches a relevant property through sub-property
/// inclusions, a chain or transitivity, in either direction (the
/// super-property direction is a safety margin, as in the estate reasoner).
fn relevant_only(axioms: Vec<ElAxiom>) -> Vec<ElAxiom> {
    let mut relevant: BTreeSet<String> = BTreeSet::new();
    // Property graph edges, undirected: sub/super, chain member/super.
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut link = |a: &str, b: &str| {
        edges.entry(a.to_owned()).or_default().insert(b.to_owned());
        edges.entry(b.to_owned()).or_default().insert(a.to_owned());
    };
    for axiom in &axioms {
        match axiom {
            ElAxiom::SubClassOf { sub, .. } => properties_in(sub, &mut relevant),
            ElAxiom::EquivalentClasses(cs) | ElAxiom::DisjointClasses(cs) => {
                cs.iter().for_each(|c| properties_in(c, &mut relevant));
            }
            ElAxiom::PropertyDomain { property, class } => {
                relevant.insert(property.clone());
                properties_in(class, &mut relevant);
            }
            ElAxiom::ClassAssertion { class, .. } => properties_in(class, &mut relevant),
            ElAxiom::PropertyAssertion { property, .. } => {
                relevant.insert(property.clone());
            }
            ElAxiom::SubPropertyOf { sub, sup } => link(sub, sup),
            ElAxiom::PropertyChain { chain, sup } => chain.iter().for_each(|p| link(p, sup)),
            ElAxiom::EquivalentProperties(ps) => {
                for w in ps.windows(2) {
                    link(&w[0], &w[1]);
                }
            }
            _ => {}
        }
    }
    let mut frontier: Vec<String> = relevant.iter().cloned().collect();
    while let Some(p) = frontier.pop() {
        if let Some(next) = edges.get(&p) {
            for q in next {
                if relevant.insert(q.clone()) {
                    frontier.push(q.clone());
                }
            }
        }
    }
    axioms
        .into_iter()
        .filter(|axiom| match droppable(axiom) {
            Some(ps) => ps.iter().any(|p| relevant.contains(p)),
            None => true,
        })
        .collect()
}
