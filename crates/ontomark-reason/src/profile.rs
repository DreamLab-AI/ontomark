//! The OWL 2 EL profile check: which axioms the reasoner takes, and for the
//! rest, exactly why not.
//!
//! Two kinds of refusal are kept apart, because they call for different
//! fixes: [`RefusalKind::OutsideEl`] means the axiom is not OWL 2 EL at all
//! (OWL 2 Profiles, section 2.2) and an EL reasoner can never honour it;
//! [`RefusalKind::NotImplemented`] means the axiom is EL but `whelk-el` does
//! not implement it, so a different backend could. A refused axiom is left out
//! whole, never weakened, and the [`crate::Classification`] that follows says
//! it is incomplete.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use whelk_el::{ClassExpr, ElAxiom};

use crate::axiom::{Axiom, CardinalityBound, Characteristic, ClassExpression, Property};

const OWL_TOP_PROPERTY: &str = "http://www.w3.org/2002/07/owl#topObjectProperty";
const OWL_BOTTOM_PROPERTY: &str = "http://www.w3.org/2002/07/owl#bottomObjectProperty";

/// Why an axiom was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalKind {
    /// The construct is outside the OWL 2 EL profile.
    OutsideEl,
    /// The construct is in OWL 2 EL but `whelk-el` does not implement it.
    NotImplemented,
}

/// One refused axiom.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Refusal {
    /// Index of the axiom in the input slice.
    pub index: usize,
    /// Which kind of refusal.
    pub kind: RefusalKind,
    /// The OWL constructor responsible, for example `ObjectAllValuesFrom`.
    pub construct: String,
    /// A sentence a person can act on.
    pub reason: String,
}

impl Refusal {
    /// A stable code for diagnostics: `EL_OUTSIDE_PROFILE` or
    /// `EL_NOT_IMPLEMENTED`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self.kind {
            RefusalKind::OutsideEl => "EL_OUTSIDE_PROFILE",
            RefusalKind::NotImplemented => "EL_NOT_IMPLEMENTED",
        }
    }
}

/// The result of checking a slice of axioms against the profile.
#[derive(Debug, Clone, Default)]
pub struct ProfileReport {
    /// The accepted axioms, translated, with the index of their source axiom.
    pub accepted: Vec<(usize, ElAxiom)>,
    /// The refused axioms, in input order.
    pub refusals: Vec<Refusal>,
}

impl ProfileReport {
    /// `true` when nothing was refused.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.refusals.is_empty()
    }
}

/// A refused construct, before it is attached to an axiom index.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Refused {
    kind: RefusalKind,
    construct: Cow<'static, str>,
    reason: &'static str,
}

const fn outside(construct: &'static str, reason: &'static str) -> Refused {
    Refused {
        kind: RefusalKind::OutsideEl,
        construct: Cow::Borrowed(construct),
        reason,
    }
}

const fn unimplemented(construct: &'static str, reason: &'static str) -> Refused {
    Refused {
        kind: RefusalKind::NotImplemented,
        construct: Cow::Borrowed(construct),
        reason,
    }
}

/// Check every axiom; translate the accepted ones.
#[must_use]
pub fn check(axioms: &[Axiom]) -> ProfileReport {
    let mut report = ProfileReport::default();
    for (index, axiom) in axioms.iter().enumerate() {
        match translate(axiom) {
            Ok(translated) => report
                .accepted
                .extend(translated.into_iter().map(|a| (index, a))),
            Err(r) => report.refusals.push(Refusal {
                index,
                kind: r.kind,
                construct: r.construct.into_owned(),
                reason: r.reason.to_owned(),
            }),
        }
    }
    report
}

/// Check one axiom.
///
/// # Errors
/// The [`Refusal`] (with `index` 0) if the axiom is refused.
pub fn check_one(axiom: &Axiom) -> Result<Vec<ElAxiom>, Refusal> {
    translate(axiom).map_err(|r| Refusal {
        index: 0,
        kind: r.kind,
        construct: r.construct.into_owned(),
        reason: r.reason.to_owned(),
    })
}

fn property(p: &Property) -> Result<String, Refused> {
    match p {
        Property::Named(iri) if iri == OWL_TOP_PROPERTY => Err(unimplemented(
            "owl:topObjectProperty",
            "the universal property has built-in semantics whelk-el does not implement",
        )),
        Property::Named(iri) if iri == OWL_BOTTOM_PROPERTY => Err(unimplemented(
            "owl:bottomObjectProperty",
            "the empty property has built-in semantics whelk-el does not implement",
        )),
        Property::Named(iri) => Ok(iri.clone()),
        Property::Inverse(_) => Err(outside(
            "ObjectInverseOf",
            "OWL 2 EL has no inverse properties; state the property in the forward direction",
        )),
    }
}

fn class(ce: &ClassExpression) -> Result<ClassExpr, Refused> {
    Ok(match ce {
        ClassExpression::Class(iri) => ClassExpr::Named(iri.clone()),
        ClassExpression::Thing => ClassExpr::Thing,
        ClassExpression::Nothing => ClassExpr::Nothing,
        ClassExpression::And(operands) => {
            ClassExpr::And(operands.iter().map(class).collect::<Result<_, _>>()?)
        }
        ClassExpression::Some { property: p, filler } => ClassExpr::Some {
            property: property(p)?,
            filler: Box::new(class(filler)?),
        },
        ClassExpression::HasValue { .. } => {
            return Err(unimplemented(
                "ObjectHasValue",
                "value restrictions are OWL 2 EL, but whelk-el's nominal reasoning is incomplete (no nominal merging), so they are not reasoned over",
            ))
        }
        ClassExpression::OneOf(individuals) => {
            return Err(if individuals.len() == 1 {
                unimplemented(
                    "ObjectOneOf",
                    "a one-individual enumeration is OWL 2 EL, but whelk-el's nominal reasoning is incomplete (no nominal merging)",
                )
            } else {
                outside(
                    "ObjectOneOf",
                    "OWL 2 EL allows an enumeration of exactly one individual",
                )
            })
        }
        ClassExpression::Or(_) => {
            return Err(outside(
                "ObjectUnionOf",
                "OWL 2 EL has no disjunction; give each alternative its own subclass axiom",
            ))
        }
        ClassExpression::Not(_) => {
            return Err(outside(
                "ObjectComplementOf",
                "OWL 2 EL has no negation; state disjointness with DisjointClasses instead",
            ))
        }
        ClassExpression::Only { .. } => {
            return Err(outside(
                "ObjectAllValuesFrom",
                "OWL 2 EL has no universal restriction; use an existential (ObjectSomeValuesFrom) if that is what is meant",
            ))
        }
        ClassExpression::Cardinality { bound, .. } => {
            return Err(outside(
                match bound {
                    CardinalityBound::Min => "ObjectMinCardinality",
                    CardinalityBound::Max => "ObjectMaxCardinality",
                    CardinalityBound::Exact => "ObjectExactCardinality",
                },
                "OWL 2 EL has no cardinality restrictions; a minimum of one is an existential (ObjectSomeValuesFrom)",
            ))
        }
        ClassExpression::HasSelf(_) => {
            return Err(unimplemented(
                "ObjectHasSelf",
                "self restrictions are OWL 2 EL but whelk-el does not implement them",
            ))
        }
        ClassExpression::Data { constructor, .. } => {
            return Err(Refused {
                construct: Cow::Owned(constructor.clone()),
                ..unimplemented("", "whelk-el reasons over object properties only; data-property class expressions are not implemented")
            })
        }
    })
}

fn classes(ces: &[ClassExpression]) -> Result<Vec<ClassExpr>, Refused> {
    ces.iter().map(class).collect()
}

fn translate(axiom: &Axiom) -> Result<Vec<ElAxiom>, Refused> {
    Ok(match axiom {
        Axiom::DeclareClass(iri) => vec![ElAxiom::DeclareClass(iri.clone())],
        Axiom::DeclareProperty(_) => Vec::new(),
        Axiom::DeclareIndividual(iri) => vec![ElAxiom::DeclareIndividual(iri.clone())],
        Axiom::SubClassOf { sub, sup } => vec![ElAxiom::SubClassOf {
            sub: class(sub)?,
            sup: class(sup)?,
        }],
        Axiom::EquivalentClasses(ces) => vec![ElAxiom::EquivalentClasses(classes(ces)?)],
        Axiom::DisjointClasses(ces) => vec![ElAxiom::DisjointClasses(classes(ces)?)],
        Axiom::DisjointUnion { .. } => {
            return Err(outside(
                "DisjointUnion",
                "OWL 2 EL has no disjunction, so no disjoint union; use subclass and DisjointClasses axioms",
            ))
        }
        Axiom::SubPropertyOf { sub, sup } => vec![ElAxiom::SubPropertyOf {
            sub: property(sub)?,
            sup: property(sup)?,
        }],
        Axiom::PropertyChain { chain, sup } => vec![ElAxiom::PropertyChain {
            chain: chain.iter().map(property).collect::<Result<_, _>>()?,
            sup: property(sup)?,
        }],
        Axiom::EquivalentProperties(ps) => vec![ElAxiom::EquivalentProperties(
            ps.iter().map(property).collect::<Result<_, _>>()?,
        )],
        Axiom::DisjointProperties(_) => {
            return Err(outside(
                "DisjointObjectProperties",
                "OWL 2 EL has no disjoint object properties",
            ))
        }
        Axiom::InverseProperties(..) => {
            return Err(outside(
                "InverseObjectProperties",
                "OWL 2 EL has no inverse properties; keep the inverse as an annotation hint",
            ))
        }
        Axiom::PropertyDomain { property: p, class: c } => vec![ElAxiom::PropertyDomain {
            property: property(p)?,
            class: class(c)?,
        }],
        Axiom::PropertyRange { .. } => {
            return Err(unimplemented(
                "ObjectPropertyRange",
                "ranges are OWL 2 EL but whelk-el does not implement them; the range is not used for classification",
            ))
        }
        Axiom::Characteristic {
            property: p,
            characteristic,
        } => match characteristic {
            Characteristic::Transitive => vec![ElAxiom::TransitiveProperty(property(p)?)],
            Characteristic::Reflexive => {
                return Err(unimplemented(
                    "ReflexiveObjectProperty",
                    "reflexivity is OWL 2 EL but whelk-el does not implement it",
                ))
            }
            Characteristic::Irreflexive => {
                return Err(outside("IrreflexiveObjectProperty", "OWL 2 EL has no irreflexive properties"))
            }
            Characteristic::Functional => {
                return Err(outside(
                    "FunctionalObjectProperty",
                    "OWL 2 EL has no functional object properties",
                ))
            }
            Characteristic::InverseFunctional => {
                return Err(outside(
                    "InverseFunctionalObjectProperty",
                    "OWL 2 EL has no inverse-functional object properties",
                ))
            }
            Characteristic::Symmetric => {
                return Err(outside(
                    "SymmetricObjectProperty",
                    "OWL 2 EL has no symmetric properties; assert both directions where both hold",
                ))
            }
            Characteristic::Asymmetric => {
                return Err(outside("AsymmetricObjectProperty", "OWL 2 EL has no asymmetric properties"))
            }
        },
        Axiom::ClassAssertion { .. } => {
            return Err(unimplemented(
                "ClassAssertion",
                "class assertions are OWL 2 EL, but the classification facade does not report contradictions involving individuals",
            ))
        }
        Axiom::PropertyAssertion { .. } => {
            return Err(unimplemented(
                "ObjectPropertyAssertion",
                "object property assertions are OWL 2 EL, but the classification facade does not report contradictions involving individuals",
            ))
        }
        Axiom::NegativePropertyAssertion { .. } => {
            return Err(unimplemented(
                "NegativeObjectPropertyAssertion",
                "negative property assertions are OWL 2 EL but whelk-el does not implement them",
            ))
        }
        Axiom::SameIndividual(_) => {
            return Err(unimplemented(
                "SameIndividual",
                "individual equality is OWL 2 EL but whelk-el does not implement it",
            ))
        }
        Axiom::DifferentIndividuals(_) => {
            return Err(unimplemented(
                "DifferentIndividuals",
                "individual inequality is OWL 2 EL but whelk-el does not implement it",
            ))
        }
        Axiom::HasKey { .. } => {
            return Err(unimplemented(
                "HasKey",
                "keys are OWL 2 EL but whelk-el does not implement them",
            ))
        }
        Axiom::Data { constructor } => {
            return Err(Refused {
                construct: Cow::Owned(constructor.clone()),
                ..unimplemented("", "whelk-el reasons over object properties only; data-property axioms are not implemented")
            })
        }
    })
}
