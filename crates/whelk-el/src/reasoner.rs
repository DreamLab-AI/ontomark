//! EL++ saturation: the completion rules of Whelk over the [`crate::model`]
//! concept language.
//!
//! Ported from `whelk-rs` `src/whelk/reasoner.rs`. The rules, their indexes and
//! their join orders are unchanged. Six things differ:
//!
//! * the work loop is a resumable [`Saturation`], so a host can run it in
//!   bounded slices, observe progress and abandon it (no clocks or threads in
//!   this crate; the host owns time);
//! * `index_role_compositions` grouped chains with `Itertools::group_by` over a
//!   hash set, which only groups *adjacent* items, so a chain `r ∘ s` with two
//!   super-properties could lose one; it now groups with a map;
//! * `index_role_compositions` pruned a composition whenever another with a
//!   distinct super-role existed, so equivalent roles pruned each other;
//!   it now prunes only strictly more general ones;
//! * chains longer than two were binarised with ids joined by `:`, which
//!   IRIs contain, so unrelated chains could share an intermediate role
//!   (see [`crate::input`]);
//! * `rule_bottom_left` tested the subclass instead of the superclass for
//!   `⊥`, so unsatisfiability did not propagate back along an existing link;
//! * the disjunction, complement and self-restriction rules are gone with
//!   their model variants (see [`crate::model`]).

use std::ops::Deref;
use std::rc::Rc;

use im::{hashmap, hashset, vector, HashMap, HashSet, Vector};

use crate::model::{
    AtomicConcept, Axiom, Concept, ConceptInclusion, Conjunction, Entity, ExistentialRestriction,
    HasSignature, QueueExpression, Role, RoleComposition, RoleInclusion,
};

type RoleIndex<V> = HashMap<Rc<Role>, V>;
type ConceptIndex<V> = HashMap<Rc<Concept>, V>;

/// The saturated (or partially saturated) state of the reasoner.
///
/// Cloning is cheap: every index is a persistent `im` structure, which is what
/// lets [`Saturation::append`] extend a finished state without disturbing it.
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct ReasonerState {
    hier: RoleIndex<HashSet<Rc<Role>>>,
    hier_comps: RoleIndex<RoleIndex<Vector<Rc<Role>>>>,
    inits: HashSet<Rc<Concept>>,
    asserted_concept_inclusions_by_subclass: ConceptIndex<Vector<Rc<ConceptInclusion>>>,
    closure_subs_by_superclass: ConceptIndex<HashSet<Rc<Concept>>>,
    closure_subs_by_subclass: ConceptIndex<HashSet<Rc<Concept>>>,
    asserted_negative_conjunctions: HashSet<Rc<Conjunction>>,
    asserted_negative_conjunctions_by_right_operand: ConceptIndex<ConceptIndex<Rc<Conjunction>>>,
    asserted_negative_conjunctions_by_left_operand: ConceptIndex<ConceptIndex<Rc<Conjunction>>>,
    links_by_subject: ConceptIndex<RoleIndex<HashSet<Rc<Concept>>>>,
    links_by_target: ConceptIndex<RoleIndex<Vector<Rc<Concept>>>>,
    negative_existential_restrictions_by_concept: ConceptIndex<HashSet<Rc<ExistentialRestriction>>>,
    propagations: ConceptIndex<RoleIndex<Vector<Rc<ExistentialRestriction>>>>,
    top: Rc<Concept>,
    bottom: Rc<Concept>,
}

impl ReasonerState {
    fn empty() -> ReasonerState {
        ReasonerState {
            hier: HashMap::default(),
            hier_comps: HashMap::default(),
            inits: HashSet::default(),
            asserted_concept_inclusions_by_subclass: HashMap::default(),
            closure_subs_by_superclass: hashmap! {Rc::new(Concept::bottom()) => HashSet::new()},
            closure_subs_by_subclass: hashmap! {Rc::new(Concept::top()) => HashSet::new()},
            asserted_negative_conjunctions: HashSet::default(),
            asserted_negative_conjunctions_by_right_operand: HashMap::default(),
            asserted_negative_conjunctions_by_left_operand: HashMap::default(),
            links_by_subject: HashMap::default(),
            links_by_target: HashMap::default(),
            negative_existential_restrictions_by_concept: HashMap::default(),
            propagations: HashMap::default(),
            top: Rc::new(Concept::top()),
            bottom: Rc::new(Concept::bottom()),
        }
    }

    /// Every entailed `sub ⊑ sup` between named classes, including the
    /// reflexive pairs, `⊑ owl:Thing` and, for an unsatisfiable class,
    /// `⊑ owl:Nothing`. Unordered.
    #[must_use]
    pub fn named_subsumptions(&self) -> Vec<(Rc<AtomicConcept>, Rc<AtomicConcept>)> {
        self.closure_subs_by_subclass
            .iter()
            .filter_map(|(sub, supers)| match sub.deref() {
                Concept::AtomicConcept(ac) => Some((ac, supers)),
                _ => None,
            })
            .flat_map(|(sub, supers)| {
                supers.iter().filter_map(move |sup| match sup.deref() {
                    Concept::AtomicConcept(ac) => Some((Rc::clone(sub), Rc::clone(ac))),
                    _ => None,
                })
            })
            .collect()
    }

    /// [`Self::named_subsumptions`] as owned IRI pairs, sorted and
    /// deduplicated, so the result is identical on every target.
    #[must_use]
    pub fn named_subsumption_ids(&self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = self
            .named_subsumptions()
            .into_iter()
            .map(|(sub, sup)| (sub.id.clone(), sup.id.clone()))
            .collect();
        pairs.sort_unstable();
        pairs.dedup();
        pairs
    }

    /// The entailed superclasses of `concept`, or `None` if the reasoner never
    /// saw it.
    #[must_use]
    pub fn subsumers(&self, concept: &Concept) -> Option<&HashSet<Rc<Concept>>> {
        self.closure_subs_by_subclass.get(concept)
    }
}

/// Whether a [`Saturation`] has reached its fixpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// The fixpoint is reached; [`Saturation::into_state`] returns the result.
    Done,
    /// Work remains; call [`Saturation::run`] again.
    Pending,
}

/// A saturation in progress: the state plus the work still queued.
///
/// One step processes one queued item. Steps are the unit of the budget given
/// to [`Saturation::run`], so a host can interleave slices of reasoning with
/// its own work and stop at any slice boundary; an abandoned saturation leaves
/// every state it was derived from untouched.
#[derive(Debug, Clone)]
pub struct Saturation {
    state: ReasonerState,
    assertions: Vec<Rc<ConceptInclusion>>,
    todo: Vec<QueueExpression>,
    steps: u64,
}

impl Saturation {
    /// Start saturating `axioms` from nothing.
    #[must_use]
    pub fn new(axioms: &HashSet<Rc<Axiom>>) -> Saturation {
        let all_roles: HashSet<Rc<Role>> = axioms
            .iter()
            .flat_map(|ax| ax.signature())
            .filter_map(|entity| match entity.deref() {
                Entity::Role(role) => Some(Rc::clone(role)),
                _ => None,
            })
            .collect();
        let mut role_inclusions = Vec::new();
        let mut chains = Vec::new();
        let mut concept_inclusions = HashSet::new();
        for ax in axioms {
            match ax.deref() {
                Axiom::RoleInclusion(ri) => role_inclusions.push(Rc::clone(ri)),
                Axiom::RoleComposition(rc) => chains.push(Rc::clone(rc)),
                Axiom::ConceptInclusion(ci) => {
                    concept_inclusions.insert(Rc::clone(ci));
                }
            }
        }
        let hier = saturate_roles(&role_inclusions, &all_roles);
        let hier_comps = index_role_compositions(&hier, &chains);
        let initial_state = ReasonerState {
            hier,
            hier_comps,
            ..ReasonerState::empty()
        };
        Saturation::append(&concept_inclusions, &initial_state)
    }

    /// Start saturating further concept inclusions on top of `state`, which is
    /// not modified. Role axioms cannot be appended: a new role hierarchy
    /// needs [`Saturation::new`].
    #[must_use]
    pub fn append(axioms: &HashSet<Rc<ConceptInclusion>>, state: &ReasonerState) -> Saturation {
        let distinct_concepts: HashSet<Rc<Concept>> = axioms
            .iter()
            .flat_map(|ci| {
                ci.subclass
                    .concept_signature()
                    .union(ci.superclass.concept_signature())
            })
            .collect();
        let mut assertions: Vec<Rc<ConceptInclusion>> = Vec::with_capacity(axioms.len());
        let mut todo: Vec<QueueExpression> = Vec::new();
        for ax in axioms {
            assertions.push(Rc::clone(ax));
            todo.push(QueueExpression::ConceptInclusion(Rc::clone(ax)));
        }
        for c in &distinct_concepts {
            if let Concept::AtomicConcept(_) = c.deref() {
                todo.push(QueueExpression::Concept(Rc::clone(c)));
            }
        }
        // Asserted inclusions are indexed before any queued item runs, in
        // input order, exactly as upstream; `run` pops from the end.
        assertions.reverse();
        Saturation {
            state: state.clone(),
            assertions,
            todo,
            steps: 0,
        }
    }

    /// Process at most `budget` items. Returns [`Progress::Done`] once the
    /// fixpoint is reached.
    pub fn run(&mut self, budget: u64) -> Progress {
        let mut left = budget;
        while left > 0 {
            if let Some(ci) = self.assertions.pop() {
                process_asserted_concept_inclusion(&ci, &mut self.state, &mut self.todo);
            } else if let Some(item) = self.todo.pop() {
                process(item, &mut self.state, &mut self.todo);
            } else {
                return Progress::Done;
            }
            left -= 1;
            self.steps += 1;
        }
        if self.is_done() {
            Progress::Done
        } else {
            Progress::Pending
        }
    }

    /// Run to the fixpoint.
    pub fn run_to_completion(&mut self) {
        while self.run(u64::MAX) == Progress::Pending {}
    }

    /// `true` when no work is queued.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.assertions.is_empty() && self.todo.is_empty()
    }

    /// Items processed so far.
    #[must_use]
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// Items currently queued. Not a bound on the remaining work: processing
    /// an item can queue more.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.assertions.len() + self.todo.len()
    }

    /// The finished state, or the saturation itself if work remains.
    ///
    /// # Errors
    /// Returns `self` unchanged when [`Self::is_done`] is false.
    pub fn into_state(self) -> Result<ReasonerState, Box<Saturation>> {
        if self.is_done() {
            Ok(self.state)
        } else {
            Err(Box::new(self))
        }
    }
}

/// Saturate `axioms` to the fixpoint.
#[must_use]
pub fn assert(axioms: &HashSet<Rc<Axiom>>) -> ReasonerState {
    let mut saturation = Saturation::new(axioms);
    saturation.run_to_completion();
    saturation.state
}

/// Saturate further concept inclusions on top of `state`, returning a new
/// state; `state` is not modified.
#[must_use]
pub fn assert_append(
    axioms: &HashSet<Rc<ConceptInclusion>>,
    state: &ReasonerState,
) -> ReasonerState {
    let mut saturation = Saturation::append(axioms, state);
    saturation.run_to_completion();
    saturation.state
}

fn process_asserted_concept_inclusion(
    ci: &Rc<ConceptInclusion>,
    state: &mut ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    match state
        .asserted_concept_inclusions_by_subclass
        .get_mut(&ci.subclass)
    {
        None => {
            state
                .asserted_concept_inclusions_by_subclass
                .insert(Rc::clone(&ci.subclass), vector![Rc::clone(ci)]);
        }
        Some(vec) => {
            vec.push_back(Rc::clone(ci));
        }
    }
    rule_subclass_left(ci, state, todo);
    rule_plus_and_a(ci, state, todo);
    rule_plus_some_a(ci, state, todo);
}

fn process(item: QueueExpression, state: &mut ReasonerState, todo: &mut Vec<QueueExpression>) {
    match item {
        QueueExpression::Link {
            subject,
            role,
            target,
        } => process_link(&subject, &role, &target, state, todo),
        QueueExpression::ConceptInclusion(ci) => {
            let seen = process_concept_inclusion(&ci, state, todo);
            if !seen {
                rule_minus_some(&ci, todo);
                rule_minus_and(&ci, todo);
            }
        }
        QueueExpression::SubPlus(ci) => {
            process_concept_inclusion(&ci, state, todo);
        }
        QueueExpression::Concept(concept) => process_concept(&concept, state, todo),
    }
}

fn process_concept(
    concept: &Rc<Concept>,
    state: &mut ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    if !state.inits.contains(concept) {
        match state.closure_subs_by_subclass.get_mut(&state.bottom) {
            None => {
                state
                    .closure_subs_by_subclass
                    .insert(Rc::clone(&state.bottom), hashset![Rc::clone(concept)]);
            }
            Some(super_classes_of_bottom) => {
                super_classes_of_bottom.insert(Rc::clone(concept));
            }
        }
        state.inits.insert(Rc::clone(concept));
        // rule_0: C ⊑ C
        todo.push(QueueExpression::ConceptInclusion(Rc::new(
            ConceptInclusion {
                subclass: Rc::clone(concept),
                superclass: Rc::clone(concept),
            },
        )));
        // rule_top: C ⊑ ⊤
        todo.push(QueueExpression::ConceptInclusion(Rc::new(
            ConceptInclusion {
                subclass: Rc::clone(concept),
                superclass: Rc::clone(&state.top),
            },
        )));
    }
}

/// Record `ci` in the closure; `true` if it was already there.
fn process_concept_inclusion(
    ci: &Rc<ConceptInclusion>,
    state: &mut ReasonerState,
    todo: &mut Vec<QueueExpression>,
) -> bool {
    let seen = match state.closure_subs_by_superclass.get_mut(&ci.superclass) {
        None => {
            state
                .closure_subs_by_superclass
                .insert(Rc::clone(&ci.superclass), hashset![Rc::clone(&ci.subclass)]);
            false
        }
        Some(subs) => subs.insert(Rc::clone(&ci.subclass)).is_some(),
    };
    if !seen {
        match state.closure_subs_by_subclass.get_mut(&ci.subclass) {
            None => {
                state
                    .closure_subs_by_subclass
                    .insert(Rc::clone(&ci.subclass), hashset![Rc::clone(&ci.superclass)]);
            }
            Some(supers) => {
                supers.insert(Rc::clone(&ci.superclass));
            }
        }
        rule_bottom_left(ci, state, todo);
        rule_subclass_right(ci, state, todo);
        rule_plus_and_right(ci, state, todo);
        rule_plus_and_left(ci, state, todo);
        rule_plus_some_b_right(ci, state, todo);
    }
    seen
}

fn process_link(
    subject: &Rc<Concept>,
    role: &Rc<Role>,
    target: &Rc<Concept>,
    state: &mut ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    let seen = match state.links_by_subject.get_mut(subject) {
        Some(roles_to_targets) => match roles_to_targets.get_mut(role) {
            Some(targets) => targets.insert(Rc::clone(target)).is_some(),
            None => {
                roles_to_targets.insert(Rc::clone(role), hashset![Rc::clone(target)]);
                false
            }
        },
        None => {
            let roles_to_targets = hashmap! {Rc::clone(role) => hashset![Rc::clone(target)]};
            state
                .links_by_subject
                .insert(Rc::clone(subject), roles_to_targets);
            false
        }
    };
    if !seen {
        match state.links_by_target.get_mut(target) {
            Some(role_to_subjects) => match role_to_subjects.get_mut(role) {
                Some(subjects) => {
                    subjects.push_back(Rc::clone(subject));
                }
                None => {
                    role_to_subjects.insert(Rc::clone(role), vector![Rc::clone(subject)]);
                }
            },
            None => {
                let roles_to_subjects = hashmap! {Rc::clone(role) => vector![Rc::clone(subject)]};
                state
                    .links_by_target
                    .insert(Rc::clone(target), roles_to_subjects);
            }
        }
        rule_bottom_right(subject, target, state, todo);
        rule_plus_some_right(subject, role, target, state, todo);
        rule_ring_right(subject, role, target, state, todo);
        rule_ring_left(subject, role, target, state, todo);
        // rule_squiggle: a link target is a concept to initialise.
        todo.push(QueueExpression::Concept(Rc::clone(target)));
    }
}

fn inclusion(subclass: &Rc<Concept>, superclass: Rc<Concept>) -> Rc<ConceptInclusion> {
    Rc::new(ConceptInclusion {
        subclass: Rc::clone(subclass),
        superclass,
    })
}

/// `D ⊑ ⊥` and a link `C →r D` give `C ⊑ ⊥`.
///
/// Upstream tested `ci.subclass == ⊥` here, which never fires usefully, so an
/// unsatisfiable filler derived *after* its link left the subject satisfiable
/// (Whelk's ⊥-left rule tests the superclass). Pinned by
/// `an_unsatisfiable_filler_makes_its_restricted_class_unsatisfiable`.
fn rule_bottom_left(
    ci: &Rc<ConceptInclusion>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    if ci.superclass == state.bottom {
        if let Some(roles_to_subjects) = state.links_by_target.get(&ci.subclass) {
            for subjects in roles_to_subjects.values() {
                for subject in subjects {
                    todo.push(QueueExpression::ConceptInclusion(inclusion(
                        subject,
                        Rc::clone(&state.bottom),
                    )));
                }
            }
        }
    }
}

fn rule_bottom_right(
    subject: &Rc<Concept>,
    target: &Rc<Concept>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    if let Some(subs) = state.closure_subs_by_superclass.get(&state.bottom) {
        if subs.contains(target) {
            todo.push(QueueExpression::ConceptInclusion(inclusion(
                subject,
                Rc::clone(&state.bottom),
            )));
        }
    }
}

fn rule_subclass_left(
    ci: &ConceptInclusion,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    if let Some(others) = state.closure_subs_by_superclass.get(&ci.subclass) {
        for other in others {
            todo.push(QueueExpression::ConceptInclusion(inclusion(
                other,
                Rc::clone(&ci.superclass),
            )));
        }
    }
}

fn rule_subclass_right(
    ci: &ConceptInclusion,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    if let Some(others) = state
        .asserted_concept_inclusions_by_subclass
        .get(&ci.superclass)
    {
        for other in others {
            todo.push(QueueExpression::ConceptInclusion(inclusion(
                &ci.subclass,
                Rc::clone(&other.superclass),
            )));
        }
    }
}

fn rule_minus_and(ci: &Rc<ConceptInclusion>, todo: &mut Vec<QueueExpression>) {
    if let Concept::Conjunction(conjunction) = ci.superclass.deref() {
        todo.push(QueueExpression::ConceptInclusion(inclusion(
            &ci.subclass,
            Rc::clone(&conjunction.left),
        )));
        todo.push(QueueExpression::ConceptInclusion(inclusion(
            &ci.subclass,
            Rc::clone(&conjunction.right),
        )));
    }
}

fn rule_plus_and_a(
    ci: &Rc<ConceptInclusion>,
    state: &mut ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    let mut new_negative_conjunctions = Vec::new();
    for c in &ci.subclass.concept_signature() {
        if let Concept::Conjunction(conjunction) = c.deref() {
            state
                .asserted_negative_conjunctions
                .insert(Rc::clone(conjunction));
            match state
                .asserted_negative_conjunctions_by_left_operand
                .get_mut(&conjunction.left)
            {
                None => {
                    let by_right_for_left =
                        hashmap! {Rc::clone(&conjunction.right) => Rc::clone(conjunction)};
                    state
                        .asserted_negative_conjunctions_by_left_operand
                        .insert(Rc::clone(&conjunction.left), by_right_for_left);
                }
                Some(by_right_for_left) => {
                    by_right_for_left.insert(Rc::clone(&conjunction.right), Rc::clone(conjunction));
                }
            }
            match state
                .asserted_negative_conjunctions_by_right_operand
                .get_mut(&conjunction.right)
            {
                None => {
                    let by_left_for_right =
                        hashmap! {Rc::clone(&conjunction.left) => Rc::clone(conjunction)};
                    state
                        .asserted_negative_conjunctions_by_right_operand
                        .insert(Rc::clone(&conjunction.right), by_left_for_right);
                }
                Some(by_left_for_right) => {
                    by_left_for_right.insert(Rc::clone(&conjunction.left), Rc::clone(conjunction));
                }
            }
            new_negative_conjunctions.push(Rc::clone(conjunction));
        }
    }
    rule_plus_and_b(new_negative_conjunctions, state, todo);
}

fn rule_plus_and_b(
    new_negative_conjunctions: Vec<Rc<Conjunction>>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    for conjunction in new_negative_conjunctions {
        if let Some(left_subclasses) = state.closure_subs_by_superclass.get(&conjunction.left) {
            if let Some(right_subclasses) = state.closure_subs_by_superclass.get(&conjunction.right)
            {
                let common = left_subclasses
                    .clone()
                    .intersection(right_subclasses.clone());
                for c in common {
                    todo.push(QueueExpression::SubPlus(inclusion(
                        &c,
                        Rc::new(Concept::Conjunction(Rc::clone(&conjunction))),
                    )));
                }
            }
        }
    }
}

fn rule_plus_and_left(
    ci: &Rc<ConceptInclusion>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    let d1 = &ci.superclass;
    let c = &ci.subclass;
    if let Some(d2s) = state.closure_subs_by_subclass.get(c) {
        if let Some(conjunctions_matching_left) =
            state.asserted_negative_conjunctions_by_left_operand.get(d1)
        {
            // Join order matters a great deal for performance.
            if d2s.len() < conjunctions_matching_left.len() {
                for d2 in d2s {
                    if let Some(conjunction) = conjunctions_matching_left.get(d2) {
                        todo.push(QueueExpression::SubPlus(inclusion(
                            c,
                            Rc::new(Concept::Conjunction(Rc::clone(conjunction))),
                        )));
                    }
                }
            } else {
                for (right, conjunction) in conjunctions_matching_left {
                    if d2s.contains(right) {
                        todo.push(QueueExpression::SubPlus(inclusion(
                            c,
                            Rc::new(Concept::Conjunction(Rc::clone(conjunction))),
                        )));
                    }
                }
            }
        }
    }
}

fn rule_plus_and_right(
    ci: &Rc<ConceptInclusion>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    let d2 = &ci.superclass;
    let c = &ci.subclass;
    if let Some(d1s) = state.closure_subs_by_subclass.get(c) {
        if let Some(conjunctions_matching_right) = state
            .asserted_negative_conjunctions_by_right_operand
            .get(d2)
        {
            if d1s.len() < conjunctions_matching_right.len() {
                for d1 in d1s {
                    if let Some(conjunction) = conjunctions_matching_right.get(d1) {
                        todo.push(QueueExpression::SubPlus(inclusion(
                            c,
                            Rc::new(Concept::Conjunction(Rc::clone(conjunction))),
                        )));
                    }
                }
            } else {
                for (left, conjunction) in conjunctions_matching_right {
                    if d1s.contains(left) {
                        todo.push(QueueExpression::SubPlus(inclusion(
                            c,
                            Rc::new(Concept::Conjunction(Rc::clone(conjunction))),
                        )));
                    }
                }
            }
        }
    }
}

fn rule_minus_some(ci: &ConceptInclusion, todo: &mut Vec<QueueExpression>) {
    if let Concept::ExistentialRestriction(er) = ci.superclass.deref() {
        todo.push(QueueExpression::Link {
            subject: Rc::clone(&ci.subclass),
            role: Rc::clone(&er.role),
            target: Rc::clone(&er.concept),
        });
    }
}

fn add_propagation(
    state: &mut ReasonerState,
    concept: &Rc<Concept>,
    er: &Rc<ExistentialRestriction>,
) {
    match state.propagations.get_mut(concept) {
        Some(roles_to_ers) => match roles_to_ers.get_mut(&er.role) {
            Some(ers) => {
                ers.push_back(Rc::clone(er));
            }
            None => {
                roles_to_ers.insert(Rc::clone(&er.role), vector![Rc::clone(er)]);
            }
        },
        None => {
            state.propagations.insert(
                Rc::clone(concept),
                hashmap! {Rc::clone(&er.role) => vector![Rc::clone(er)]},
            );
        }
    }
}

fn rule_plus_some_a(
    ci: &ConceptInclusion,
    state: &mut ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    let mut new_negative_existentials = Vec::new();
    for c in &ci.subclass.concept_signature() {
        if let Concept::ExistentialRestriction(er) = c.deref() {
            match state
                .negative_existential_restrictions_by_concept
                .get_mut(&er.concept)
            {
                Some(ers) => {
                    ers.insert(Rc::clone(er));
                }
                None => {
                    state
                        .negative_existential_restrictions_by_concept
                        .insert(Rc::clone(&er.concept), hashset![Rc::clone(er)]);
                }
            }
            new_negative_existentials.push(Rc::clone(er));
        }
    }
    // rule_plus_some_b_left
    let mut new_propagations = Vec::new();
    for er in new_negative_existentials {
        let subclasses: Vec<Rc<Concept>> = state
            .closure_subs_by_superclass
            .get(&er.concept)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        for subclass in subclasses {
            add_propagation(state, &subclass, &er);
            new_propagations.push((subclass, Rc::clone(&er)));
        }
    }
    rule_plus_some_left(new_propagations, state, todo);
}

fn rule_plus_some_b_right(
    ci: &ConceptInclusion,
    state: &mut ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    let mut new_propagations = Vec::new();
    let ers: Vec<Rc<ExistentialRestriction>> = state
        .negative_existential_restrictions_by_concept
        .get(&ci.superclass)
        .map(|s| s.iter().cloned().collect())
        .unwrap_or_default();
    for er in ers {
        add_propagation(state, &ci.subclass, &er);
        new_propagations.push((Rc::clone(&ci.subclass), er));
    }
    rule_plus_some_left(new_propagations, state, todo);
}

fn rule_plus_some_left(
    new_propagations: Vec<(Rc<Concept>, Rc<ExistentialRestriction>)>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    for (concept, er) in new_propagations {
        if let Some(links_with_target) = state.links_by_target.get(&concept) {
            for (role, subjects) in links_with_target {
                if let Some(super_roles) = state.hier.get(role) {
                    if super_roles.contains(&er.role) {
                        for subject in subjects {
                            todo.push(QueueExpression::SubPlus(inclusion(
                                subject,
                                Rc::new(Concept::ExistentialRestriction(Rc::clone(&er))),
                            )));
                        }
                    }
                }
            }
        }
    }
}

fn rule_plus_some_right(
    subject: &Rc<Concept>,
    role: &Rc<Role>,
    target: &Rc<Concept>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    if let Some(role_to_er) = state.propagations.get(target) {
        if let Some(ss) = state.hier.get(role) {
            for s in ss {
                if let Some(fs) = role_to_er.get(s) {
                    for f in fs {
                        todo.push(QueueExpression::SubPlus(inclusion(
                            subject,
                            Rc::new(Concept::ExistentialRestriction(Rc::clone(f))),
                        )));
                    }
                }
            }
        }
    }
}

fn rule_ring_left(
    subject: &Rc<Concept>,
    role: &Rc<Role>,
    target: &Rc<Concept>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    if let Some(links_by_target_for_subject) = state.links_by_target.get(subject) {
        for (r1, es) in links_by_target_for_subject {
            if let Some(r1s) = state.hier_comps.get(r1) {
                if let Some(ss) = r1s.get(role) {
                    for s in ss {
                        for e in es {
                            todo.push(QueueExpression::Link {
                                subject: Rc::clone(e),
                                role: Rc::clone(s),
                                target: Rc::clone(target),
                            });
                        }
                    }
                }
            }
        }
    }
}

fn rule_ring_right(
    subject: &Rc<Concept>,
    role: &Rc<Role>,
    target: &Rc<Concept>,
    state: &ReasonerState,
    todo: &mut Vec<QueueExpression>,
) {
    let links_by_link_subject = state.links_by_subject.get(subject);
    if let Some(r2s) = state.hier_comps.get(role) {
        if let Some(r2_to_targets) = state.links_by_subject.get(target) {
            for (r2, targets) in r2_to_targets {
                if let Some(ss) = r2s.get(r2) {
                    for s in ss {
                        let links_with_s = links_by_link_subject.and_then(|x| x.get(s));
                        for d in targets {
                            // Skips links already present: the queue for this
                            // rule can otherwise grow very large.
                            if links_with_s.is_none_or(|l| !l.contains(d)) {
                                todo.push(QueueExpression::Link {
                                    subject: Rc::clone(subject),
                                    role: Rc::clone(s),
                                    target: Rc::clone(d),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn saturate_roles(
    role_inclusions: &[Rc<RoleInclusion>],
    all_roles: &HashSet<Rc<Role>>,
) -> RoleIndex<HashSet<Rc<Role>>> {
    let mut sub_to_super: RoleIndex<HashSet<Rc<Role>>> = HashMap::new();
    for ri in role_inclusions {
        sub_to_super
            .entry(Rc::clone(&ri.subproperty))
            .or_default()
            .insert(Rc::clone(&ri.superproperty));
    }
    let mut result: RoleIndex<HashSet<Rc<Role>>> = HashMap::new();
    for role in sub_to_super.keys() {
        let all_supers = all_super_roles(role, &HashSet::new(), &sub_to_super);
        result.insert(Rc::clone(role), all_supers);
    }
    // Every role is its own super-role.
    for role in all_roles {
        result
            .entry(Rc::clone(role))
            .or_default()
            .insert(Rc::clone(role));
    }
    result
}

fn all_super_roles(
    role: &Rc<Role>,
    exclude: &HashSet<Rc<Role>>,
    sub_to_super: &RoleIndex<HashSet<Rc<Role>>>,
) -> HashSet<Rc<Role>> {
    let current_exclude = exclude.update(Rc::clone(role));
    let mut result = HashSet::new();
    if let Some(supers) = sub_to_super.get(role) {
        for super_prop in supers.iter().filter(|s| !current_exclude.contains(*s)) {
            let all_supers_reflexive = all_super_roles(super_prop, &current_exclude, sub_to_super)
                .update(Rc::clone(super_prop));
            for super_super_prop in all_supers_reflexive {
                result.insert(super_super_prop);
            }
        }
    }
    result
}

type RoleTriple = (Rc<Role>, Rc<Role>, Rc<Role>);
type RolePair = (Rc<Role>, Rc<Role>);

fn index_role_compositions(
    hier: &RoleIndex<HashSet<Rc<Role>>>,
    chains: &[Rc<RoleComposition>],
) -> RoleIndex<RoleIndex<Vector<Rc<Role>>>> {
    let mut role_comps: HashMap<RolePair, HashSet<Rc<Role>>> = HashMap::new();
    for rc in chains {
        role_comps
            .entry((Rc::clone(&rc.first), Rc::clone(&rc.second)))
            .or_default()
            .insert(Rc::clone(&rc.superproperty));
    }
    let mut hier_comps_tuples: HashSet<RoleTriple> = HashSet::new();
    for (r1, s1s) in hier {
        for s1 in s1s {
            for (r2, s2s) in hier {
                for s2 in s2s {
                    if let Some(ss) = role_comps.get(&(Rc::clone(s1), Rc::clone(s2))) {
                        for s in ss {
                            hier_comps_tuples.insert((Rc::clone(r1), Rc::clone(r2), Rc::clone(s)));
                        }
                    }
                }
            }
        }
    }
    // Drop a composition when a strictly more specific one with the same pair
    // exists. The super-role link itself is then never materialised, but no
    // consequence is lost: existential matching and further compositions
    // consult the role hierarchy (`rule_plus_some_*`, the `hier` expansion
    // above). Upstream dropped on any distinct super-role, so two equivalent
    // roles (t ⊑ u, u ⊑ t) removed each other and the chain was lost.
    let mut hier_comps_remove: HashSet<RoleTriple> = HashSet::new();
    for (r1, r2, s) in &hier_comps_tuples {
        if let Some(super_ss) = hier.get(s) {
            let strictly_above = |super_s: &&Rc<Role>| {
                *super_s != s && hier.get(*super_s).is_none_or(|ss| !ss.contains(s))
            };
            for super_s in super_ss.iter().filter(strictly_above) {
                let candidate = (Rc::clone(r1), Rc::clone(r2), Rc::clone(super_s));
                if hier_comps_tuples.contains(&candidate) {
                    hier_comps_remove.insert(candidate);
                }
            }
        }
    }
    let mut hier_comps: RoleIndex<RoleIndex<Vector<Rc<Role>>>> = HashMap::new();
    for (r1, r2, s) in hier_comps_tuples.relative_complement(hier_comps_remove) {
        hier_comps
            .entry(r1)
            .or_default()
            .entry(r2)
            .or_default()
            .push_back(s);
    }
    hier_comps
}
