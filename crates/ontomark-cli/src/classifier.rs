//! The [`Classifier`] the CLI hands the compiler: `whelk-el` over the
//! compiler's extracted EL axioms.
//!
//! Temporary until `ontomark-reason` exports its own implementation (WP-B);
//! this is a one-to-one axiom mapping, not a second reasoner.

use ontomark_compile::reason::{Classifier, DefinitionTerm, ElInput};
use whelk_el::{ClassExpr, ElAxiom};

/// `whelk-el`, single-threaded.
pub struct WhelkEl;

fn operand(term: &DefinitionTerm) -> ClassExpr {
    match term {
        DefinitionTerm::Named(iri) => ClassExpr::named(iri.clone()),
        DefinitionTerm::Some(property, filler) => {
            ClassExpr::some(property.clone(), ClassExpr::named(filler.clone()))
        }
    }
}

impl Classifier for WhelkEl {
    fn named_subsumptions(&self, input: &ElInput) -> Vec<(String, String)> {
        let mut axioms = Vec::new();
        axioms.extend(input.classes.iter().cloned().map(ElAxiom::DeclareClass));
        axioms.extend(
            input
                .transitive
                .iter()
                .cloned()
                .map(ElAxiom::TransitiveProperty),
        );
        axioms.extend(
            input
                .sub_properties
                .iter()
                .map(|(sub, sup)| ElAxiom::SubPropertyOf {
                    sub: sub.clone(),
                    sup: sup.clone(),
                }),
        );
        axioms.extend(
            input
                .sub_classes
                .iter()
                .map(|(sub, sup)| ElAxiom::SubClassOf {
                    sub: ClassExpr::named(sub.clone()),
                    sup: ClassExpr::named(sup.clone()),
                }),
        );
        axioms.extend(input.disjoint.iter().map(|(a, b)| {
            ElAxiom::DisjointClasses(vec![
                ClassExpr::named(a.clone()),
                ClassExpr::named(b.clone()),
            ])
        }));
        axioms.extend(input.definitions.iter().map(|(class, terms)| {
            let definiens = if let [only] = terms.as_slice() {
                operand(only)
            } else {
                ClassExpr::And(terms.iter().map(operand).collect())
            };
            ElAxiom::EquivalentClasses(vec![ClassExpr::named(class.clone()), definiens])
        }));
        axioms.extend(input.existentials.iter().map(|(sub, property, filler)| {
            ElAxiom::SubClassOf {
                sub: ClassExpr::named(sub.clone()),
                sup: ClassExpr::some(property.clone(), ClassExpr::named(filler.clone())),
            }
        }));
        whelk_el::classify(&axioms)
    }
}
