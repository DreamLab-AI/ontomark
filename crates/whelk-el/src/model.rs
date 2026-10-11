//! The reasoner's internal concept language: hash-consed EL concepts, roles and
//! the normalised axioms the saturation rules consume.
//!
//! Ported from `whelk-rs` `src/whelk/model.rs`. The disjunction, complement and
//! self-restriction variants of the upstream model are removed: no input in
//! this crate can produce them, the upstream rules for them were incomplete
//! (`rule_or_*`, `rule_*_self` are TODOs there), and the upstream complement
//! rule asserted `C ⊑ ⊥` for every `¬C` it saw, which is unsound. Inputs that
//! need those constructors are outside OWL 2 EL and are refused before they
//! reach this module (see [`crate::input`]).

use std::rc::Rc;

use im::HashSet;

/// IRI of `owl:Thing`, the top concept.
pub const TOP: &str = "http://www.w3.org/2002/07/owl#Thing";
/// IRI of `owl:Nothing`, the bottom concept.
pub const BOTTOM: &str = "http://www.w3.org/2002/07/owl#Nothing";
/// Prefix of the synthetic roles that binarise property chains longer than two.
/// The full id is this prefix, the first role's id, a space and the second
/// role's IRI.
pub const COMPOSITION_ROLE_PREFIX: &str = "urn:whelk:composition_role:";

/// Anything that mentions entities.
pub trait HasSignature {
    /// The roles, atomic concepts and individuals mentioned.
    fn signature(&self) -> HashSet<Rc<Entity>>;
}

/// An object property.
#[derive(Eq, PartialEq, Hash, Debug, Clone, PartialOrd, Ord)]
pub struct Role {
    /// The property IRI.
    pub id: String,
}

/// A named individual, used only inside a [`Nominal`].
#[derive(Eq, PartialEq, Hash, Debug, Clone, PartialOrd, Ord)]
pub struct Individual {
    /// The individual IRI.
    pub id: String,
}

/// A named class, including `owl:Thing` and `owl:Nothing`.
#[derive(Eq, PartialEq, Hash, Debug, Clone, PartialOrd, Ord)]
pub struct AtomicConcept {
    /// The class IRI.
    pub id: String,
}

/// `∃role.concept`.
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct ExistentialRestriction {
    /// The property restricted.
    pub role: Rc<Role>,
    /// The filler.
    pub concept: Rc<Concept>,
}

/// `left ⊓ right`. Larger intersections are nested binary conjunctions.
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct Conjunction {
    /// The left operand.
    pub left: Rc<Concept>,
    /// The right operand.
    pub right: Rc<Concept>,
}

/// `{individual}`: the class with exactly one member.
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct Nominal {
    /// The sole member.
    pub individual: Rc<Individual>,
}

/// An entity of the signature.
#[derive(Eq, PartialEq, Hash, Debug)]
pub enum Entity {
    /// An object property.
    Role(Rc<Role>),
    /// A named class.
    AtomicConcept(Rc<AtomicConcept>),
    /// A named individual.
    Individual(Rc<Individual>),
}

impl Entity {
    /// The entity's IRI.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Entity::Role(role) => &role.id,
            Entity::AtomicConcept(c) => &c.id,
            Entity::Individual(i) => &i.id,
        }
    }
}

/// An EL concept.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub enum Concept {
    /// A named class.
    AtomicConcept(Rc<AtomicConcept>),
    /// A binary intersection.
    Conjunction(Rc<Conjunction>),
    /// An existential restriction.
    ExistentialRestriction(Rc<ExistentialRestriction>),
    /// A singleton enumeration.
    Nominal(Rc<Nominal>),
}

impl Concept {
    /// The concept and every sub-concept it is built from.
    #[must_use]
    pub fn concept_signature(&self) -> HashSet<Rc<Concept>> {
        match self {
            Concept::AtomicConcept(_) | Concept::Nominal(_) => HashSet::unit(Rc::new(self.clone())),
            Concept::Conjunction(conjunction) => {
                let mut sig = conjunction
                    .left
                    .concept_signature()
                    .union(conjunction.right.concept_signature());
                sig.insert(Rc::new(self.clone()));
                sig
            }
            Concept::ExistentialRestriction(er) => {
                let mut sig = er.concept.concept_signature();
                sig.insert(Rc::new(self.clone()));
                sig
            }
        }
    }

    /// A named class.
    #[must_use]
    pub fn atomic(id: impl Into<String>) -> Concept {
        Concept::AtomicConcept(Rc::new(AtomicConcept { id: id.into() }))
    }

    /// `owl:Thing`.
    #[must_use]
    pub fn top() -> Concept {
        Concept::atomic(TOP)
    }

    /// `owl:Nothing`.
    #[must_use]
    pub fn bottom() -> Concept {
        Concept::atomic(BOTTOM)
    }
}

impl HasSignature for Concept {
    fn signature(&self) -> HashSet<Rc<Entity>> {
        match self {
            Concept::AtomicConcept(ac) => {
                HashSet::unit(Rc::new(Entity::AtomicConcept(Rc::clone(ac))))
            }
            Concept::Conjunction(conjunction) => conjunction
                .left
                .signature()
                .union(conjunction.right.signature()),
            Concept::ExistentialRestriction(er) => {
                let mut sig = er.concept.signature();
                sig.insert(Rc::new(Entity::Role(Rc::clone(&er.role))));
                sig
            }
            Concept::Nominal(nominal) => {
                HashSet::unit(Rc::new(Entity::Individual(Rc::clone(&nominal.individual))))
            }
        }
    }
}

/// `subclass ⊑ superclass`.
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct ConceptInclusion {
    /// The left-hand side.
    pub subclass: Rc<Concept>,
    /// The right-hand side.
    pub superclass: Rc<Concept>,
}

/// `subproperty ⊑ superproperty`.
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct RoleInclusion {
    /// The sub-property.
    pub subproperty: Rc<Role>,
    /// The super-property.
    pub superproperty: Rc<Role>,
}

/// `first ∘ second ⊑ superproperty`.
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct RoleComposition {
    /// The first property of the chain.
    pub first: Rc<Role>,
    /// The second property of the chain.
    pub second: Rc<Role>,
    /// The implied property.
    pub superproperty: Rc<Role>,
}

/// A normalised axiom.
#[derive(Eq, PartialEq, Hash, Debug)]
pub enum Axiom {
    /// A general concept inclusion.
    ConceptInclusion(Rc<ConceptInclusion>),
    /// A role inclusion.
    RoleInclusion(Rc<RoleInclusion>),
    /// A binary role composition.
    RoleComposition(Rc<RoleComposition>),
}

impl HasSignature for Axiom {
    fn signature(&self) -> HashSet<Rc<Entity>> {
        match self {
            Axiom::ConceptInclusion(ci) => ci.subclass.signature().union(ci.superclass.signature()),
            Axiom::RoleInclusion(ri) => [
                Rc::new(Entity::Role(Rc::clone(&ri.subproperty))),
                Rc::new(Entity::Role(Rc::clone(&ri.superproperty))),
            ]
            .into_iter()
            .collect(),
            Axiom::RoleComposition(rc) => [
                Rc::new(Entity::Role(Rc::clone(&rc.first))),
                Rc::new(Entity::Role(Rc::clone(&rc.second))),
                Rc::new(Entity::Role(Rc::clone(&rc.superproperty))),
            ]
            .into_iter()
            .collect(),
        }
    }
}

/// A unit of saturation work.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub(crate) enum QueueExpression {
    Concept(Rc<Concept>),
    ConceptInclusion(Rc<ConceptInclusion>),
    SubPlus(Rc<ConceptInclusion>),
    Link {
        subject: Rc<Concept>,
        role: Rc<Role>,
        target: Rc<Concept>,
    },
}
