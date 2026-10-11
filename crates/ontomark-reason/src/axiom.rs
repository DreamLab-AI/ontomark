//! The OWL axioms the facade accepts: the logical vocabulary of OWL 2 DL, so
//! that constructs outside the EL profile can be named when they are refused.
//!
//! These types stand in for `ontomark-model`'s axioms until that crate
//! publishes them; an adapter will then convert one into the other. IRIs are
//! plain strings and are never rewritten here.

use serde::{Deserialize, Serialize};

/// An object property expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Property {
    /// A named object property.
    Named(String),
    /// `ObjectInverseOf(p)`.
    Inverse(String),
}

impl Property {
    /// A named object property.
    #[must_use]
    pub fn named(iri: impl Into<String>) -> Property {
        Property::Named(iri.into())
    }
}

/// A class expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassExpression {
    /// A named class, by IRI.
    Class(String),
    /// `owl:Thing`.
    Thing,
    /// `owl:Nothing`.
    Nothing,
    /// `ObjectIntersectionOf`.
    And(Vec<ClassExpression>),
    /// `ObjectUnionOf`.
    Or(Vec<ClassExpression>),
    /// `ObjectComplementOf`.
    Not(Box<ClassExpression>),
    /// `ObjectSomeValuesFrom`.
    Some {
        /// The restricted property.
        property: Property,
        /// The filler.
        filler: Box<ClassExpression>,
    },
    /// `ObjectAllValuesFrom`.
    Only {
        /// The restricted property.
        property: Property,
        /// The filler.
        filler: Box<ClassExpression>,
    },
    /// `ObjectHasValue`.
    HasValue {
        /// The restricted property.
        property: Property,
        /// The individual IRI.
        individual: String,
    },
    /// `ObjectHasSelf`.
    HasSelf(Property),
    /// `ObjectOneOf`, by individual IRIs.
    OneOf(Vec<String>),
    /// `ObjectMinCardinality`, `ObjectMaxCardinality` or
    /// `ObjectExactCardinality`.
    Cardinality {
        /// Which bound.
        bound: CardinalityBound,
        /// The number.
        n: u32,
        /// The restricted property.
        property: Property,
        /// The qualifying filler, if any.
        filler: Option<Box<ClassExpression>>,
    },
    /// Any class expression over a data property (`DataSomeValuesFrom`,
    /// `DataHasValue`, `DataAllValuesFrom`, data cardinalities), named by
    /// its OWL constructor.
    Data {
        /// The OWL constructor name, for example `DataSomeValuesFrom`.
        constructor: String,
        /// The data property IRI.
        property: String,
    },
}

impl ClassExpression {
    /// A named class.
    #[must_use]
    pub fn class(iri: impl Into<String>) -> ClassExpression {
        ClassExpression::Class(iri.into())
    }

    /// `∃property.filler` over a named property.
    #[must_use]
    pub fn some(property: impl Into<String>, filler: ClassExpression) -> ClassExpression {
        ClassExpression::Some {
            property: Property::named(property),
            filler: Box::new(filler),
        }
    }
}

/// The bound of a cardinality restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardinalityBound {
    /// `ObjectMinCardinality`.
    Min,
    /// `ObjectMaxCardinality`.
    Max,
    /// `ObjectExactCardinality`.
    Exact,
}

/// An object property characteristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Characteristic {
    /// `TransitiveObjectProperty`.
    Transitive,
    /// `ReflexiveObjectProperty`.
    Reflexive,
    /// `IrreflexiveObjectProperty`.
    Irreflexive,
    /// `FunctionalObjectProperty`.
    Functional,
    /// `InverseFunctionalObjectProperty`.
    InverseFunctional,
    /// `SymmetricObjectProperty`.
    Symmetric,
    /// `AsymmetricObjectProperty`.
    Asymmetric,
}

/// A logical axiom (plus the declarations that give a class a place in the
/// taxonomy).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axiom {
    /// `Declaration(Class(iri))`.
    DeclareClass(String),
    /// `Declaration(ObjectProperty(iri))`. No logical content.
    DeclareProperty(String),
    /// `Declaration(NamedIndividual(iri))`.
    DeclareIndividual(String),
    /// `SubClassOf(sub, sup)`.
    SubClassOf {
        /// The subclass expression.
        sub: ClassExpression,
        /// The superclass expression.
        sup: ClassExpression,
    },
    /// `EquivalentClasses(…)`.
    EquivalentClasses(Vec<ClassExpression>),
    /// `DisjointClasses(…)`.
    DisjointClasses(Vec<ClassExpression>),
    /// `DisjointUnion(class, operands)`.
    DisjointUnion {
        /// The class defined as the union.
        class: String,
        /// The pairwise disjoint operands.
        operands: Vec<ClassExpression>,
    },
    /// `SubObjectPropertyOf(sub, sup)`.
    SubPropertyOf {
        /// The sub-property.
        sub: Property,
        /// The super-property.
        sup: Property,
    },
    /// `SubObjectPropertyOf(ObjectPropertyChain(chain), sup)`.
    PropertyChain {
        /// The chain, first property first.
        chain: Vec<Property>,
        /// The implied property.
        sup: Property,
    },
    /// `EquivalentObjectProperties(…)`.
    EquivalentProperties(Vec<Property>),
    /// `DisjointObjectProperties(…)`.
    DisjointProperties(Vec<Property>),
    /// `InverseObjectProperties(a, b)`.
    InverseProperties(Property, Property),
    /// `ObjectPropertyDomain(property, class)`.
    PropertyDomain {
        /// The property.
        property: Property,
        /// The domain.
        class: ClassExpression,
    },
    /// `ObjectPropertyRange(property, class)`.
    PropertyRange {
        /// The property.
        property: Property,
        /// The range.
        class: ClassExpression,
    },
    /// A property characteristic axiom.
    Characteristic {
        /// The property.
        property: Property,
        /// The characteristic.
        characteristic: Characteristic,
    },
    /// `ClassAssertion(class, individual)`.
    ClassAssertion {
        /// The individual IRI.
        individual: String,
        /// The asserted type.
        class: ClassExpression,
    },
    /// `ObjectPropertyAssertion(property, subject, target)`.
    PropertyAssertion {
        /// The subject individual IRI.
        subject: String,
        /// The property.
        property: Property,
        /// The target individual IRI.
        target: String,
    },
    /// `NegativeObjectPropertyAssertion(property, subject, target)`.
    NegativePropertyAssertion {
        /// The subject individual IRI.
        subject: String,
        /// The property.
        property: Property,
        /// The target individual IRI.
        target: String,
    },
    /// `SameIndividual(…)`.
    SameIndividual(Vec<String>),
    /// `DifferentIndividuals(…)`.
    DifferentIndividuals(Vec<String>),
    /// `HasKey(class, properties)`.
    HasKey {
        /// The keyed class.
        class: ClassExpression,
        /// The key properties.
        properties: Vec<Property>,
    },
    /// Any axiom about data properties or datatypes, named by its OWL
    /// constructor (for example `DataPropertyRange`).
    Data {
        /// The OWL constructor name.
        constructor: String,
    },
}

impl Axiom {
    /// `SubClassOf(Class(sub), Class(sup))`.
    #[must_use]
    pub fn sub_class(sub: impl Into<String>, sup: impl Into<String>) -> Axiom {
        Axiom::SubClassOf {
            sub: ClassExpression::class(sub),
            sup: ClassExpression::class(sup),
        }
    }
}
