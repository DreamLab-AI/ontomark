//! The crate's input: owned, `Send`, string-IRI EL axioms, and their
//! normalisation into the reasoner's [`crate::model`].
//!
//! The translation follows `whelk-rs` `src/whelk/owl.rs` axiom for axiom. The
//! input type covers exactly what that translation implements; there is no
//! constructor for a construct the reasoner would silently ignore. Callers
//! holding a wider OWL model (for example `ontomark-reason`) refuse the rest
//! with a reason before building these values.

use std::rc::Rc;

use im::HashSet;

use crate::model::{
    AtomicConcept, Axiom, Concept, ConceptInclusion, Conjunction, ExistentialRestriction,
    Individual, Nominal, Role, RoleComposition, RoleInclusion, BOTTOM, COMPOSITION_ROLE_PREFIX,
    TOP,
};

/// An EL class expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClassExpr {
    /// `owl:Thing`.
    Thing,
    /// `owl:Nothing`.
    Nothing,
    /// A named class, by IRI. The IRIs of `owl:Thing` and `owl:Nothing` denote
    /// the same concepts as [`ClassExpr::Thing`] and [`ClassExpr::Nothing`].
    Named(String),
    /// `{individual}` (`ObjectOneOf` with one member), by IRI.
    Individual(String),
    /// `ObjectIntersectionOf`. Empty is `owl:Thing`; one operand is itself.
    And(Vec<ClassExpr>),
    /// `ObjectSomeValuesFrom(property, filler)`.
    Some {
        /// The object property IRI.
        property: String,
        /// The filler.
        filler: Box<ClassExpr>,
    },
}

impl ClassExpr {
    /// A named class.
    #[must_use]
    pub fn named(iri: impl Into<String>) -> ClassExpr {
        ClassExpr::Named(iri.into())
    }

    /// `∃property.filler`.
    #[must_use]
    pub fn some(property: impl Into<String>, filler: ClassExpr) -> ClassExpr {
        ClassExpr::Some {
            property: property.into(),
            filler: Box::new(filler),
        }
    }

    /// `true` when the expression denotes `owl:Thing`.
    #[must_use]
    pub fn is_thing(&self) -> bool {
        match self {
            ClassExpr::Thing => true,
            ClassExpr::Named(iri) => iri == TOP,
            ClassExpr::And(operands) => operands.iter().all(ClassExpr::is_thing),
            _ => false,
        }
    }

    /// `true` when the expression denotes `owl:Nothing` (including an
    /// intersection with `owl:Nothing`).
    #[must_use]
    pub fn is_nothing(&self) -> bool {
        match self {
            ClassExpr::Nothing => true,
            ClassExpr::Named(iri) => iri == BOTTOM,
            ClassExpr::And(operands) => operands.iter().any(ClassExpr::is_nothing),
            _ => false,
        }
    }

    /// Call `f` on every class, property and individual IRI mentioned, other
    /// than `owl:Thing` and `owl:Nothing`.
    pub fn for_each_entity<'a>(&'a self, f: &mut impl FnMut(&'a str)) {
        match self {
            ClassExpr::Thing | ClassExpr::Nothing => {}
            ClassExpr::Named(iri) => {
                if iri != TOP && iri != BOTTOM {
                    f(iri);
                }
            }
            ClassExpr::Individual(iri) => f(iri),
            ClassExpr::And(operands) => operands.iter().for_each(|o| o.for_each_entity(f)),
            ClassExpr::Some { property, filler } => {
                f(property);
                filler.for_each_entity(f);
            }
        }
    }
}

/// An EL axiom.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ElAxiom {
    /// `Declaration(Class(iri))`: the class is classified even if nothing
    /// else mentions it.
    DeclareClass(String),
    /// `Declaration(NamedIndividual(iri))`.
    DeclareIndividual(String),
    /// `SubClassOf(sub, sup)`.
    SubClassOf {
        /// The subclass expression.
        sub: ClassExpr,
        /// The superclass expression.
        sup: ClassExpr,
    },
    /// `EquivalentClasses(…)`: every pair is mutually subsumed.
    EquivalentClasses(Vec<ClassExpr>),
    /// `DisjointClasses(…)`: every pair has an empty intersection.
    DisjointClasses(Vec<ClassExpr>),
    /// `SubObjectPropertyOf(sub, sup)`.
    SubPropertyOf {
        /// The sub-property IRI.
        sub: String,
        /// The super-property IRI.
        sup: String,
    },
    /// `SubObjectPropertyOf(ObjectPropertyChain(chain), sup)`.
    PropertyChain {
        /// The chain, first property first.
        chain: Vec<String>,
        /// The implied property IRI.
        sup: String,
    },
    /// `EquivalentObjectProperties(…)`.
    EquivalentProperties(Vec<String>),
    /// `TransitiveObjectProperty(p)`.
    TransitiveProperty(String),
    /// `ObjectPropertyDomain(property, class)`.
    PropertyDomain {
        /// The property IRI.
        property: String,
        /// The domain.
        class: ClassExpr,
    },
    /// `ClassAssertion(class, individual)`.
    ClassAssertion {
        /// The individual IRI.
        individual: String,
        /// The asserted type.
        class: ClassExpr,
    },
    /// `ObjectPropertyAssertion(property, subject, target)`.
    PropertyAssertion {
        /// The subject individual IRI.
        subject: String,
        /// The property IRI.
        property: String,
        /// The target individual IRI.
        target: String,
    },
}

impl ElAxiom {
    /// `SubClassOf(Named(sub), Named(sup))`.
    #[must_use]
    pub fn sub_class(sub: impl Into<String>, sup: impl Into<String>) -> ElAxiom {
        ElAxiom::SubClassOf {
            sub: ClassExpr::Named(sub.into()),
            sup: ClassExpr::Named(sup.into()),
        }
    }

    /// Call `f` on every IRI mentioned, other than `owl:Thing` and
    /// `owl:Nothing`.
    pub fn for_each_entity<'a>(&'a self, f: &mut impl FnMut(&'a str)) {
        match self {
            ElAxiom::DeclareClass(iri) => {
                if iri != TOP && iri != BOTTOM {
                    f(iri);
                }
            }
            ElAxiom::DeclareIndividual(iri) | ElAxiom::TransitiveProperty(iri) => f(iri),
            ElAxiom::SubClassOf { sub, sup } => {
                sub.for_each_entity(f);
                sup.for_each_entity(f);
            }
            ElAxiom::EquivalentClasses(cs) | ElAxiom::DisjointClasses(cs) => {
                cs.iter().for_each(|c| c.for_each_entity(f));
            }
            ElAxiom::SubPropertyOf { sub, sup } => {
                f(sub);
                f(sup);
            }
            ElAxiom::PropertyChain { chain, sup } => {
                chain.iter().for_each(|p| f(p));
                f(sup);
            }
            ElAxiom::EquivalentProperties(ps) => ps.iter().for_each(|p| f(p)),
            ElAxiom::PropertyDomain { property, class } => {
                f(property);
                class.for_each_entity(f);
            }
            ElAxiom::ClassAssertion { individual, class } => {
                f(individual);
                class.for_each_entity(f);
            }
            ElAxiom::PropertyAssertion {
                subject,
                property,
                target,
            } => {
                f(subject);
                f(property);
                f(target);
            }
        }
    }

    /// `true` when the axiom relates classes that share no entity: `owl:Thing`
    /// on the left of an inclusion (`⊤ ⊑ C`, an equivalence with `⊤`, or a
    /// disjointness between two `⊤` operands) reaches every class, and
    /// `owl:Nothing` on the left (`⊥ ⊑ C`) reaches every unsatisfiable class.
    #[must_use]
    pub fn constrains_everything(&self) -> bool {
        match self {
            ElAxiom::SubClassOf { sub, .. } => sub.is_thing() || sub.is_nothing(),
            ElAxiom::EquivalentClasses(cs) => cs.iter().any(ClassExpr::is_thing),
            // (⊤ ⊓ ⊤) ⊑ ⊥ makes every class unsatisfiable.
            ElAxiom::DisjointClasses(cs) => cs.iter().filter(|c| c.is_thing()).count() >= 2,
            _ => false,
        }
    }
}

/// Normalise `axioms` into the reasoner's axiom set.
#[must_use]
pub fn translate(axioms: &[ElAxiom]) -> HashSet<Rc<Axiom>> {
    let mut out = HashSet::new();
    let top = Rc::new(Concept::top());
    let bottom = Rc::new(Concept::bottom());
    for axiom in axioms {
        translate_one(axiom, &top, &bottom, &mut out);
    }
    out
}

fn concept_inclusion(sub: &Rc<Concept>, sup: &Rc<Concept>) -> Rc<Axiom> {
    Rc::new(Axiom::ConceptInclusion(Rc::new(ConceptInclusion {
        subclass: Rc::clone(sub),
        superclass: Rc::clone(sup),
    })))
}

fn role(iri: &str) -> Rc<Role> {
    Rc::new(Role { id: iri.to_owned() })
}

fn nominal(iri: &str) -> Rc<Concept> {
    Rc::new(Concept::Nominal(Rc::new(Nominal {
        individual: Rc::new(Individual { id: iri.to_owned() }),
    })))
}

fn role_inclusion(sub: &str, sup: &str) -> Rc<Axiom> {
    Rc::new(Axiom::RoleInclusion(Rc::new(RoleInclusion {
        subproperty: role(sub),
        superproperty: role(sup),
    })))
}

/// Every unordered pair of `items`, in order of first appearance.
fn pairs<T>(items: &[T]) -> impl Iterator<Item = (&T, &T)> {
    items
        .iter()
        .enumerate()
        .flat_map(move |(i, a)| items[i + 1..].iter().map(move |b| (a, b)))
}

fn translate_one(
    axiom: &ElAxiom,
    top: &Rc<Concept>,
    bottom: &Rc<Concept>,
    out: &mut HashSet<Rc<Axiom>>,
) {
    match axiom {
        ElAxiom::DeclareClass(iri) => {
            out.insert(concept_inclusion(
                &Rc::new(Concept::atomic(iri.as_str())),
                top,
            ));
        }
        ElAxiom::DeclareIndividual(iri) => {
            out.insert(concept_inclusion(&nominal(iri), top));
        }
        ElAxiom::SubClassOf { sub, sup } => {
            out.insert(concept_inclusion(&convert(sub), &convert(sup)));
        }
        ElAxiom::EquivalentClasses(cs) => {
            let converted: Vec<Rc<Concept>> = cs.iter().map(convert).collect();
            for (first, second) in pairs(&converted) {
                if first != bottom {
                    out.insert(concept_inclusion(first, second));
                }
                if second != bottom {
                    out.insert(concept_inclusion(second, first));
                }
            }
        }
        ElAxiom::DisjointClasses(cs) => {
            let converted: Vec<Rc<Concept>> = cs.iter().map(convert).collect();
            for (first, second) in pairs(&converted) {
                let conjunction = Rc::new(Concept::Conjunction(Rc::new(Conjunction {
                    left: Rc::clone(first),
                    right: Rc::clone(second),
                })));
                out.insert(concept_inclusion(&conjunction, bottom));
            }
        }
        ElAxiom::SubPropertyOf { sub, sup } => {
            out.insert(role_inclusion(sub, sup));
        }
        ElAxiom::PropertyChain { chain, sup } => translate_chain(chain, sup, out),
        ElAxiom::EquivalentProperties(ps) => {
            for (first, second) in pairs(ps) {
                out.insert(role_inclusion(first, second));
                out.insert(role_inclusion(second, first));
            }
        }
        ElAxiom::TransitiveProperty(p) => {
            translate_chain(&[p.clone(), p.clone()], p, out);
        }
        ElAxiom::PropertyDomain { property, class } => {
            let restriction = Rc::new(Concept::ExistentialRestriction(Rc::new(
                ExistentialRestriction {
                    role: role(property),
                    concept: Rc::clone(top),
                },
            )));
            out.insert(concept_inclusion(&restriction, &convert(class)));
        }
        ElAxiom::ClassAssertion { individual, class } => {
            out.insert(concept_inclusion(&nominal(individual), &convert(class)));
        }
        ElAxiom::PropertyAssertion {
            subject,
            property,
            target,
        } => {
            let restriction = Rc::new(Concept::ExistentialRestriction(Rc::new(
                ExistentialRestriction {
                    role: role(property),
                    concept: nominal(target),
                },
            )));
            out.insert(concept_inclusion(&nominal(subject), &restriction));
        }
    }
}

/// Binarise `p₁ ∘ p₂ ∘ … ∘ pₙ ⊑ sup` with synthetic composition roles, as
/// upstream does: `p₁ ∘ p₂ ⊑ c`, then `c ∘ p₃ ∘ … ⊑ sup`.
fn translate_chain(chain: &[String], sup: &str, out: &mut HashSet<Rc<Axiom>>) {
    match chain {
        [] => {}
        [only] => {
            out.insert(role_inclusion(only, sup));
        }
        [first, second] => {
            out.insert(Rc::new(Axiom::RoleComposition(Rc::new(RoleComposition {
                first: role(first),
                second: role(second),
                superproperty: role(sup),
            }))));
        }
        [first, second, rest @ ..] => {
            // A space cannot occur in an IRI and only `first` can itself be
            // synthetic, so this id is unambiguous. Upstream joined with `:`,
            // which IRIs contain, so `[p:q, r, …]` and `[p, q:r, …]` shared an
            // intermediate role and leaked links between unrelated chains.
            let composition = format!("{COMPOSITION_ROLE_PREFIX}{first} {second}");
            translate_chain(&[first.clone(), second.clone()], &composition, out);
            let mut shorter = Vec::with_capacity(rest.len() + 1);
            shorter.push(composition);
            shorter.extend(rest.iter().cloned());
            translate_chain(&shorter, sup, out);
        }
    }
}

/// Convert a class expression to a concept. Intersections are flattened,
/// sorted and deduplicated first, so the same intersection written in any
/// order or nesting is one concept to the reasoner.
#[must_use]
pub fn convert(expression: &ClassExpr) -> Rc<Concept> {
    match expression {
        ClassExpr::Thing => Rc::new(Concept::top()),
        ClassExpr::Nothing => Rc::new(Concept::bottom()),
        ClassExpr::Named(iri) => Rc::new(Concept::AtomicConcept(Rc::new(AtomicConcept {
            id: iri.clone(),
        }))),
        ClassExpr::Individual(iri) => nominal(iri),
        ClassExpr::Some { property, filler } => Rc::new(Concept::ExistentialRestriction(Rc::new(
            ExistentialRestriction {
                role: role(property),
                concept: convert(filler),
            },
        ))),
        ClassExpr::And(_) => {
            let mut operands = Vec::new();
            flatten(expression, &mut operands);
            // An intersection with ⊥ is ⊥, whichever spelling of ⊥ it uses;
            // left as a conjunction, `⊥ ⊓ owl:Nothing` would survive
            // deduplication as a distinct concept.
            if operands.iter().any(|o| o.is_nothing()) {
                return Rc::new(Concept::bottom());
            }
            operands.sort();
            operands.dedup();
            // An intersection with ⊤ is the other operands.
            if operands.len() > 1 {
                operands.retain(|o| !o.is_thing());
            }
            let mut converted = operands.iter().rev().map(|o| convert(o));
            match converted.next() {
                None => Rc::new(Concept::top()),
                Some(last) => converted.fold(last, |right, left| {
                    Rc::new(Concept::Conjunction(Rc::new(Conjunction { left, right })))
                }),
            }
        }
    }
}

fn flatten<'a>(expression: &'a ClassExpr, out: &mut Vec<&'a ClassExpr>) {
    match expression {
        ClassExpr::And(operands) => operands.iter().for_each(|o| flatten(o, out)),
        other => out.push(other),
    }
}
