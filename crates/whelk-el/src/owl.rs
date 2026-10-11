//! Adapter from `horned-owl` components to [`ElAxiom`]s (feature
//! `horned-owl`).
//!
//! Follows `whelk-rs` `src/whelk/owl.rs`, with one change of policy: upstream
//! silently dropped any axiom or expression it could not translate, which
//! makes "no entailment" indistinguishable from "not read". Here a logical
//! component outside the supported fragment is an [`Unsupported`] error naming
//! the construct, and only non-logical components (declarations of
//! properties and datatypes, annotations, imports, ontology metadata) translate
//! to nothing.

use horned_owl::model as hm;
use horned_owl::model::{ForIRI, Kinded};

use crate::input::{ClassExpr, ElAxiom};

/// A logical component or class expression the reasoner does not implement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    /// The `horned-owl` name of the construct, for example
    /// `ObjectAllValuesFrom` or `ObjectPropertyRange`.
    pub construct: String,
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is not supported by whelk-el", self.construct)
    }
}

impl std::error::Error for Unsupported {}

fn unsupported<T>(construct: impl Into<String>) -> Result<T, Unsupported> {
    Err(Unsupported {
        construct: construct.into(),
    })
}

fn object_property<A: ForIRI>(
    ope: &hm::ObjectPropertyExpression<A>,
) -> Result<String, Unsupported> {
    match ope {
        hm::ObjectPropertyExpression::ObjectProperty(hm::ObjectProperty(iri)) => {
            Ok(iri.to_string())
        }
        hm::ObjectPropertyExpression::InverseObjectProperty(_) => unsupported("ObjectInverseOf"),
    }
}

fn named_individual<A: ForIRI>(i: &hm::Individual<A>) -> Result<String, Unsupported> {
    match i {
        hm::Individual::Named(hm::NamedIndividual(iri)) => Ok(iri.to_string()),
        hm::Individual::Anonymous(_) => unsupported("AnonymousIndividual"),
    }
}

/// Convert one class expression.
///
/// # Errors
/// [`Unsupported`] naming the first construct outside the fragment.
pub fn class_expression<A: ForIRI>(ce: &hm::ClassExpression<A>) -> Result<ClassExpr, Unsupported> {
    match ce {
        hm::ClassExpression::Class(hm::Class(iri)) => Ok(ClassExpr::Named(iri.to_string())),
        hm::ClassExpression::ObjectIntersectionOf(operands) => Ok(ClassExpr::And(
            operands
                .iter()
                .map(class_expression)
                .collect::<Result<_, _>>()?,
        )),
        hm::ClassExpression::ObjectSomeValuesFrom { ope, bce } => Ok(ClassExpr::Some {
            property: object_property(ope)?,
            filler: Box::new(class_expression(bce)?),
        }),
        hm::ClassExpression::ObjectHasValue { ope, i } => Ok(ClassExpr::Some {
            property: object_property(ope)?,
            filler: Box::new(ClassExpr::Individual(named_individual(i)?)),
        }),
        hm::ClassExpression::ObjectOneOf(individuals) => match individuals.as_slice() {
            [only] => Ok(ClassExpr::Individual(named_individual(only)?)),
            _ => unsupported("ObjectOneOf with more than one individual"),
        },
        hm::ClassExpression::ObjectUnionOf(_) => unsupported("ObjectUnionOf"),
        hm::ClassExpression::ObjectComplementOf(_) => unsupported("ObjectComplementOf"),
        hm::ClassExpression::ObjectAllValuesFrom { .. } => unsupported("ObjectAllValuesFrom"),
        hm::ClassExpression::ObjectHasSelf(_) => unsupported("ObjectHasSelf"),
        hm::ClassExpression::ObjectMinCardinality { .. } => unsupported("ObjectMinCardinality"),
        hm::ClassExpression::ObjectMaxCardinality { .. } => unsupported("ObjectMaxCardinality"),
        hm::ClassExpression::ObjectExactCardinality { .. } => unsupported("ObjectExactCardinality"),
        hm::ClassExpression::DataSomeValuesFrom { .. } => unsupported("DataSomeValuesFrom"),
        hm::ClassExpression::DataAllValuesFrom { .. } => unsupported("DataAllValuesFrom"),
        hm::ClassExpression::DataHasValue { .. } => unsupported("DataHasValue"),
        hm::ClassExpression::DataMinCardinality { .. } => unsupported("DataMinCardinality"),
        hm::ClassExpression::DataMaxCardinality { .. } => unsupported("DataMaxCardinality"),
        hm::ClassExpression::DataExactCardinality { .. } => unsupported("DataExactCardinality"),
    }
}

fn class_expressions<A: ForIRI>(
    ces: &[hm::ClassExpression<A>],
) -> Result<Vec<ClassExpr>, Unsupported> {
    ces.iter().map(class_expression).collect()
}

/// Convert one component. Non-logical components convert to no axioms.
///
/// # Errors
/// [`Unsupported`] naming the component kind, or the first unsupported
/// construct inside it.
pub fn component<A: ForIRI>(c: &hm::Component<A>) -> Result<Vec<ElAxiom>, Unsupported> {
    use hm::Component as C;
    Ok(match c {
        C::DeclareClass(hm::DeclareClass(hm::Class(iri))) => {
            vec![ElAxiom::DeclareClass(iri.to_string())]
        }
        C::DeclareNamedIndividual(hm::DeclareNamedIndividual(hm::NamedIndividual(iri))) => {
            vec![ElAxiom::DeclareIndividual(iri.to_string())]
        }
        C::OntologyID(_)
        | C::DocIRI(_)
        | C::OntologyAnnotation(_)
        | C::Import(_)
        | C::DeclareObjectProperty(_)
        | C::DeclareAnnotationProperty(_)
        | C::DeclareDataProperty(_)
        | C::DeclareDatatype(_)
        | C::AnnotationAssertion(_)
        | C::SubAnnotationPropertyOf(_)
        | C::AnnotationPropertyDomain(_)
        | C::AnnotationPropertyRange(_) => Vec::new(),
        C::SubClassOf(ax) => vec![ElAxiom::SubClassOf {
            sub: class_expression(&ax.sub)?,
            sup: class_expression(&ax.sup)?,
        }],
        C::EquivalentClasses(hm::EquivalentClasses(ces)) => {
            vec![ElAxiom::EquivalentClasses(class_expressions(ces)?)]
        }
        C::DisjointClasses(hm::DisjointClasses(ces)) => {
            vec![ElAxiom::DisjointClasses(class_expressions(ces)?)]
        }
        C::SubObjectPropertyOf(hm::SubObjectPropertyOf { sub, sup }) => {
            let sup = object_property(sup)?;
            match sub {
                hm::SubObjectPropertyExpression::ObjectPropertyExpression(ope) => {
                    vec![ElAxiom::SubPropertyOf {
                        sub: object_property(ope)?,
                        sup,
                    }]
                }
                hm::SubObjectPropertyExpression::ObjectPropertyChain(chain) => {
                    vec![ElAxiom::PropertyChain {
                        chain: chain
                            .iter()
                            .map(object_property)
                            .collect::<Result<_, _>>()?,
                        sup,
                    }]
                }
            }
        }
        C::EquivalentObjectProperties(hm::EquivalentObjectProperties(ps)) => {
            vec![ElAxiom::EquivalentProperties(
                ps.iter().map(object_property).collect::<Result<_, _>>()?,
            )]
        }
        C::TransitiveObjectProperty(hm::TransitiveObjectProperty(p)) => {
            vec![ElAxiom::TransitiveProperty(object_property(p)?)]
        }
        C::ObjectPropertyDomain(hm::ObjectPropertyDomain { ope, ce }) => {
            vec![ElAxiom::PropertyDomain {
                property: object_property(ope)?,
                class: class_expression(ce)?,
            }]
        }
        C::ClassAssertion(hm::ClassAssertion { ce, i }) => vec![ElAxiom::ClassAssertion {
            individual: named_individual(i)?,
            class: class_expression(ce)?,
        }],
        C::ObjectPropertyAssertion(hm::ObjectPropertyAssertion { ope, from, to }) => {
            let (subject, target) = (named_individual(from)?, named_individual(to)?);
            match ope {
                hm::ObjectPropertyExpression::ObjectProperty(hm::ObjectProperty(p)) => {
                    vec![ElAxiom::PropertyAssertion {
                        subject,
                        property: p.to_string(),
                        target,
                    }]
                }
                // upstream: an inverse assertion is the forward one reversed.
                hm::ObjectPropertyExpression::InverseObjectProperty(hm::ObjectProperty(p)) => {
                    vec![ElAxiom::PropertyAssertion {
                        subject: target,
                        property: p.to_string(),
                        target: subject,
                    }]
                }
            }
        }
        other => return unsupported(format!("{:?}", other.kind())),
    })
}

/// Convert every component of `components`, collecting the unsupported ones
/// instead of stopping at the first.
pub fn components<'a, A: ForIRI + 'a>(
    components: impl IntoIterator<Item = &'a hm::Component<A>>,
) -> (Vec<ElAxiom>, Vec<Unsupported>) {
    let mut axioms = Vec::new();
    let mut refused = Vec::new();
    for c in components {
        match component(c) {
            Ok(mut converted) => axioms.append(&mut converted),
            Err(e) => refused.push(e),
        }
    }
    (axioms, refused)
}
