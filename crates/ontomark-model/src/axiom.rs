//! The logical view of a model: OWL 2 EL axioms a reasoner can take.
//!
//! [`Model::axioms`] projects only statements whose [`Disposition`] is
//! [`Disposition::Supported`], plus property characteristics the profile
//! declares. Subjects and objects of the axioms are always keys of
//! [`Model::entities`]. Property IRIs are either profile-declared predicates
//! that no page claims, or pages of kind object property: a predicate IRI
//! whose page is a data property or a class is rejected, never projected. Everything else stays in the statement layer with its reason.
//! The shapes here are the EL constructs Markdown pages can express; a
//! reasoner that also accepts other inputs owns any wider expression type.

use serde::{Deserialize, Serialize};

use crate::iri::Iri;
use crate::kind::{Kind, PropertyKind};
use crate::model::{Disposition, Model, Object, Origin, Resolution, StatementForm};
use crate::profile::{PredicateForm, Profile};

/// An EL class expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "snake_case")]
pub enum ClassExpr {
    /// A named class.
    Named(Iri),
    /// `owl:Thing`.
    Thing,
    /// `owl:Nothing`.
    Nothing,
    /// `ObjectIntersectionOf`.
    And(Vec<ClassExpr>),
    /// `ObjectSomeValuesFrom(property, filler)`.
    Some {
        /// Object property.
        property: Iri,
        /// Filler.
        filler: Box<ClassExpr>,
    },
}

/// An EL axiom.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Axiom {
    /// `SubClassOf(sub, sup)`.
    SubClassOf {
        /// Subclass.
        sub: ClassExpr,
        /// Superclass.
        sup: ClassExpr,
    },
    /// `EquivalentClasses(…)`.
    EquivalentClasses {
        /// Operands.
        classes: Vec<ClassExpr>,
    },
    /// `DisjointClasses(…)`.
    DisjointClasses {
        /// Operands.
        classes: Vec<ClassExpr>,
    },
    /// `SubObjectPropertyOf(sub, sup)`.
    SubPropertyOf {
        /// Sub-property.
        sub: Iri,
        /// Super-property.
        sup: Iri,
    },
    /// `TransitiveObjectProperty(p)`.
    Transitive {
        /// Property.
        property: Iri,
    },
    /// `ClassAssertion(class, individual)`.
    ClassAssertion {
        /// Individual.
        individual: Iri,
        /// Class.
        class: ClassExpr,
    },
    /// `ObjectPropertyAssertion(property, subject, object)`.
    ObjectAssertion {
        /// Subject individual.
        subject: Iri,
        /// Property.
        property: Iri,
        /// Object individual.
        object: Iri,
    },
    /// `SameIndividual(…)`.
    SameIndividual {
        /// Individuals.
        individuals: Vec<Iri>,
    },
}

/// An axiom and where it was authored (`None` for profile-declared axioms).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcedAxiom {
    /// The axiom.
    pub axiom: Axiom,
    /// Its source in the Markdown, when it came from a page.
    pub origin: Option<Origin>,
}

impl Model {
    /// The reasoner's input: one axiom per supported statement, in entity
    /// and source order, followed by `TransitiveObjectProperty` for every
    /// transitive object predicate the profile declares.
    ///
    /// ```
    /// # use ontomark_model::{Axiom, ClassExpr, DeclarativeProfile, DocId, Iri, Model};
    /// # use ontomark_parse::Document;
    /// let a = Document::parse("---\ntype: Class\niri: https://ex.org/A\n---\n");
    /// let b = Document::parse("---\ntype: Class\niri: https://ex.org/B\nrdfs:subClassOf:\n- '[[A]]'\n---\n");
    /// let p = DeclarativeProfile::generic();
    /// let m = Model::build(&p, [(DocId::new("A"), &a), (DocId::new("B"), &b)]);
    /// let ax = m.axioms(&p);
    /// assert_eq!(ax[0].axiom, Axiom::SubClassOf {
    ///     sub: ClassExpr::Named(Iri::parse("https://ex.org/B").unwrap()),
    ///     sup: ClassExpr::Named(Iri::parse("https://ex.org/A").unwrap()),
    /// });
    /// ```
    #[must_use]
    pub fn axioms(&self, profile: &dyn Profile) -> Vec<SourcedAxiom> {
        let mut out = Vec::new();
        for e in self.entities.values() {
            for s in &e.statements {
                if s.disposition != Disposition::Supported {
                    continue;
                }
                let Object::Reference(r) = &s.object else {
                    continue;
                };
                let Resolution::Resolved(o) = &r.state else {
                    continue;
                };
                let (subj, obj) = (e.iri.clone(), o.clone());
                let named = ClassExpr::Named;
                let axiom = match s.form {
                    StatementForm::SubClassOf => Axiom::SubClassOf {
                        sub: named(subj),
                        sup: named(obj),
                    },
                    StatementForm::Existential => Axiom::SubClassOf {
                        sub: named(subj),
                        sup: ClassExpr::Some {
                            property: s.predicate.clone(),
                            filler: Box::new(named(obj)),
                        },
                    },
                    StatementForm::EquivalentClass => Axiom::EquivalentClasses {
                        classes: vec![named(subj), named(obj)],
                    },
                    StatementForm::DisjointWith => Axiom::DisjointClasses {
                        classes: vec![named(subj), named(obj)],
                    },
                    StatementForm::ClassAssertion => Axiom::ClassAssertion {
                        individual: subj,
                        class: named(obj),
                    },
                    StatementForm::ObjectAssertion => Axiom::ObjectAssertion {
                        subject: subj,
                        property: s.predicate.clone(),
                        object: obj,
                    },
                    StatementForm::SameAs => Axiom::SameIndividual {
                        individuals: vec![subj, obj],
                    },
                    StatementForm::SubPropertyOf => Axiom::SubPropertyOf {
                        sub: subj,
                        sup: obj,
                    },
                    // Never Supported; listed for exhaustiveness.
                    StatementForm::DataAssertion
                    | StatementForm::Annotation
                    | StatementForm::Skos(_) => continue,
                };
                out.push(SourcedAxiom {
                    axiom,
                    origin: Some(s.origin.clone()),
                });
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for p in profile.predicates() {
            let object = matches!(
                p.form,
                PredicateForm::Existential | PredicateForm::ObjectAssertion
            );
            // A predicate IRI that is a page of another kind is not an
            // object property: no characteristic is asserted for it.
            let not_object = self
                .entities
                .get(&p.iri)
                .is_some_and(|e| e.kind != Kind::Property(PropertyKind::Object));
            if p.transitive && object && !not_object && seen.insert(p.iri.clone()) {
                out.push(SourcedAxiom {
                    axiom: Axiom::Transitive { property: p.iri },
                    origin: None,
                });
            }
        }
        out
    }
}
