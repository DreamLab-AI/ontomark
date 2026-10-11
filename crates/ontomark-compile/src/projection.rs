//! Pure RDF and logical projection of the normative typed model.
//!
//! This API performs no filesystem reads, identity minting or corpus mutation.
//! The caller supplies parsed documents, a model profile and an explicit scope.
//! It is separate from the rc1 legacy vault build; selecting this API adopts
//! typed identities and kinds rather than legacy phantom-parent conventions.
//!
//! ```
//! use ontomark_compile::projection::{project_model, Scope};
//! use ontomark_model::{DeclarativeProfile, DocId, Model};
//! use ontomark_parse::Document;
//! let doc = Document::parse("---\ntype: Class\niri: urn:A\n---\n");
//! let profile = DeclarativeProfile::generic();
//! let model = Model::build(&profile, [(DocId::new("A"), &doc)]);
//! let projected = project_model(&model, &profile, Scope::Authoring);
//! assert!(!projected.graph.is_empty());
//! assert!(projected.classify().is_complete());
//! ```

use crate::build::turtle::{Graph, Subject, Term};
use ontomark_model::{
    Axiom as ModelAxiom, ClassExpr, Disposition, DocDisposition, Kind, Model, Object, Profile,
    PropertyKind, Resolution, StatementForm,
};
use ontomark_reason::axiom::{Axiom, Characteristic, ClassExpression, Property};
use std::collections::BTreeSet;

const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const OWL: &str = "http://www.w3.org/2002/07/owl#";

/// Which declared entities and assertions a caller intends to inspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// All declared entities, including private and unpublished authoring data.
    Authoring,
    /// Public entities excluding unknown or explicitly unpublished lifecycle
    /// states. References and literals
    /// naming withheld entities are omitted before RDF and reasoning projection.
    Public,
}

/// Counts of omitted inputs, without private identifiers or source text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Omissions {
    /// Private or lifecycle-withheld entity declarations.
    pub withheld_entities: usize,
    /// Assertions, labels or aliases withheld for public privacy.
    pub withheld_values: usize,
    /// Ill-formed, ambiguous or unresolved assertions that yield no RDF edge.
    pub rejected_statements: usize,
    /// Typed ontology documents refused before obtaining an entity identity.
    pub refused_documents: usize,
    /// Logical assertions preserved in RDF but excluded by the model's reasoning
    /// disposition, including lifecycle exclusions. Annotations and SKOS are not
    /// logical assertions and do not increment this counter.
    pub logical_statements_not_reasoned: usize,
}

/// An in-memory asserted RDF graph and shared reasoner input for one scope.
#[derive(Debug, Clone)]
pub struct Projection {
    /// The explicit scope used for this graph and logical input.
    pub scope: Scope,
    /// RDF assertions; no private entity or target is retained in public scope.
    pub graph: Graph,
    /// Logical input, using the same axiom vocabulary as the shared classifier.
    pub axioms: Vec<Axiom>,
    /// Omitted inputs. Inspect these alongside classification refusals.
    pub omissions: Omissions,
}

/// Classification together with projection omissions that affect completeness.
#[derive(Debug, Clone)]
pub struct ProjectionClassification {
    /// Shared supported-profile classifier result, including explicit refusals.
    /// Its completeness covers projected axioms; [`Self::is_complete`] also
    /// accounts for source statements that could not be projected.
    pub classification: ontomark_reason::Classification,
    /// Projection omissions; public privacy withholding intentionally selects a
    /// smaller corpus and does not itself make that selected view incomplete.
    pub omissions: Omissions,
}

impl ProjectionClassification {
    /// Whether every logical statement of the selected view was admitted and
    /// saturated. This does not claim complete OWL 2 EL support or consistency.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.classification.is_complete()
            && self.omissions.rejected_statements == 0
            && self.omissions.refused_documents == 0
            && self.omissions.logical_statements_not_reasoned == 0
    }
    /// Whether the selected view is complete and its named classes are coherent.
    /// Incomplete classifications return false even if no clash was derived.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.is_complete() && self.classification.unsatisfiable.is_empty()
    }
}

impl Projection {
    /// Classify the projected logical input with all existential restrictions.
    /// Source refusals are retained in the combined completeness verdict.
    #[must_use]
    pub fn classify(&self) -> ProjectionClassification {
        ProjectionClassification {
            classification: ontomark_reason::classify(&self.axioms),
            omissions: self.omissions.clone(),
        }
    }
    /// Classify with a caller's existential policy and step budget.
    ///
    /// # Errors
    /// [`ontomark_reason::Stopped`] when the requested budget is exhausted.
    pub fn classify_with(
        &self,
        options: ontomark_reason::Options,
    ) -> Result<ProjectionClassification, ontomark_reason::Stopped> {
        Ok(ProjectionClassification {
            classification: ontomark_reason::classify_with(&self.axioms, options)?,
            omissions: self.omissions.clone(),
        })
    }
}

/// Project declared kinds, labels and model statements without reading a vault.
///
/// Rejected/unresolved references never create RDF nodes. External annotation
/// or SKOS IRIs may be preserved without creating an ontology declaration.
/// Public literals containing a withheld entity's IRI, label, alias or document
/// identifier are conservatively withheld as a whole; source prose is not part
/// of this API. The returned counters disclose no withheld names.
#[must_use]
pub fn project_model(model: &Model, profile: &dyn Profile, scope: Scope) -> Projection {
    let hidden: BTreeSet<_> = model
        .entities
        .values()
        .filter(|e| {
            scope == Scope::Public
                && (!e.public
                    || match &e.lifecycle {
                        ontomark_model::Lifecycle::Declared(s) => {
                            profile.lifecycle_def(s).is_some_and(|l| !l.published)
                        }
                        ontomark_model::Lifecycle::Unknown(_) => true,
                        ontomark_model::Lifecycle::Absent => false,
                    })
        })
        .map(|e| e.iri.clone())
        .collect();
    let tokens: BTreeSet<_> = model
        .entities
        .values()
        .filter(|e| hidden.contains(&e.iri))
        .flat_map(|e| {
            std::iter::once(e.iri.as_str().to_owned())
                .chain(std::iter::once(e.label.clone()))
                .chain(e.aliases.clone())
                .chain(std::iter::once(e.document.as_str().to_owned()))
        })
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect();
    let private_text = |s: &str| {
        let text = s.to_lowercase();
        tokens.iter().any(|t| text.contains(t))
    };
    let mut graph = Graph::new();
    let mut kept = model.clone();
    let mut omissions = Omissions {
        withheld_entities: hidden.len(),
        ..Omissions::default()
    };
    // Public diagnostics must not disclose a refused document's name or body.
    omissions.refused_documents = model
        .documents
        .values()
        .filter(|d| {
            matches!(
                d.disposition,
                DocDisposition::NoIdentity
                    | DocDisposition::Refused(_)
                    | DocDisposition::IdentityConflict(_)
            )
        })
        .count();
    kept.entities.retain(|iri, _| !hidden.contains(iri));
    for entity in kept.entities.values_mut() {
        let iri = entity.iri.as_str();
        graph.add_iri(
            iri,
            &format!("{RDF}type"),
            Term::Iri(entity.kind.owl_iri().to_owned()),
        );
        for (predicate, text) in std::iter::once((format!("{RDFS}label"), &entity.label)).chain(
            entity
                .aliases
                .iter()
                .map(|a| ("http://www.w3.org/2004/02/skos/core#altLabel".into(), a)),
        ) {
            if private_text(text) {
                omissions.withheld_values += 1;
            } else {
                graph.add_iri(iri, &predicate, Term::plain(text));
            }
        }
        entity.statements.retain(|s| {
            if matches!(&s.object, Object::Reference(r) if private_text(&r.written)) {
                omissions.withheld_values += 1;
                return false;
            }
            if hidden.contains(&s.predicate) {
                omissions.withheld_values += 1;
                return false;
            }
            if matches!(s.disposition, Disposition::Rejected(_)) {
                omissions.rejected_statements += 1;
                return false;
            }
            let object = match &s.object {
                Object::Literal(l) => {
                    if private_text(&l.lexical) || hidden.contains(&l.datatype) {
                        omissions.withheld_values += 1;
                        return false;
                    }
                    Term::typed(&l.lexical, l.datatype.as_str())
                }
                Object::Reference(r) => match &r.state {
                    Resolution::Resolved(i) | Resolution::External(i) if !hidden.contains(i) => {
                        Term::Iri(i.as_str().to_owned())
                    }
                    Resolution::Resolved(_) | Resolution::External(_) => {
                        omissions.withheld_values += 1;
                        return false;
                    }
                    _ => {
                        omissions.rejected_statements += 1;
                        return false;
                    }
                },
            };
            if s.form == StatementForm::Existential {
                let node = graph.fresh_blank();
                graph.add_iri(iri, &format!("{RDFS}subClassOf"), Term::Blank(node.clone()));
                graph.add(
                    Subject::Blank(node.clone()),
                    format!("{RDF}type"),
                    Term::Iri(format!("{OWL}Restriction")),
                );
                graph.add(
                    Subject::Blank(node.clone()),
                    format!("{OWL}onProperty"),
                    Term::Iri(s.predicate.as_str().to_owned()),
                );
                graph.add(Subject::Blank(node), format!("{OWL}someValuesFrom"), object);
            } else {
                graph.add_iri(iri, s.predicate.as_str(), object);
            }
            if matches!(s.disposition, Disposition::PreservedNotReasoned(_))
                && !matches!(s.form, StatementForm::Annotation | StatementForm::Skos(_))
            {
                omissions.logical_statements_not_reasoned += 1;
            }
            true
        });
    }
    let mut axioms = Vec::new();
    for e in kept.entities.values() {
        let iri = e.iri.as_str().to_owned();
        match e.kind {
            Kind::Class => axioms.push(Axiom::DeclareClass(iri)),
            Kind::Individual => axioms.push(Axiom::DeclareIndividual(iri)),
            Kind::Property(PropertyKind::Object) => axioms.push(Axiom::DeclareProperty(iri)),
            Kind::Property(_) => {}
        }
    }
    for sourced in kept.axioms(profile) {
        if let ModelAxiom::Transitive { property } = &sourced.axiom {
            if hidden.contains(property) {
                continue;
            }
            graph.add_iri(
                property.as_str(),
                &format!("{RDF}type"),
                Term::Iri(format!("{OWL}TransitiveProperty")),
            );
        }
        axioms.push(reason_axiom(&sourced.axiom));
    }
    Projection {
        scope,
        graph,
        axioms,
        omissions,
    }
}

fn expression(e: &ClassExpr) -> ClassExpression {
    match e {
        ClassExpr::Named(i) => ClassExpression::class(i.as_str()),
        ClassExpr::Thing => ClassExpression::Thing,
        ClassExpr::Nothing => ClassExpression::Nothing,
        ClassExpr::And(v) => ClassExpression::And(v.iter().map(expression).collect()),
        ClassExpr::Some { property, filler } => {
            ClassExpression::some(property.as_str(), expression(filler))
        }
    }
}

fn reason_axiom(a: &ModelAxiom) -> Axiom {
    match a {
        ModelAxiom::SubClassOf { sub, sup } => Axiom::SubClassOf {
            sub: expression(sub),
            sup: expression(sup),
        },
        ModelAxiom::EquivalentClasses { classes } => {
            Axiom::EquivalentClasses(classes.iter().map(expression).collect())
        }
        ModelAxiom::DisjointClasses { classes } => {
            Axiom::DisjointClasses(classes.iter().map(expression).collect())
        }
        ModelAxiom::SubPropertyOf { sub, sup } => Axiom::SubPropertyOf {
            sub: Property::named(sub.as_str()),
            sup: Property::named(sup.as_str()),
        },
        ModelAxiom::Transitive { property } => Axiom::Characteristic {
            property: Property::named(property.as_str()),
            characteristic: Characteristic::Transitive,
        },
        ModelAxiom::ClassAssertion { individual, class } => Axiom::ClassAssertion {
            individual: individual.as_str().into(),
            class: expression(class),
        },
        ModelAxiom::ObjectAssertion {
            subject,
            property,
            object,
        } => Axiom::PropertyAssertion {
            subject: subject.as_str().into(),
            property: Property::named(property.as_str()),
            target: object.as_str().into(),
        },
        ModelAxiom::SameIndividual { individuals } => {
            Axiom::SameIndividual(individuals.iter().map(|i| i.as_str().into()).collect())
        }
    }
}
