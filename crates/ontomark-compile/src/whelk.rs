//! [`WhelkEl`]: the compiler's [`Classifier`] over `whelk-el` (feature
//! `whelk-el`, on by default).
//!
//! Maps [`ElInput`] to the shared `ontomark-reason` facade. The full result
//! reports profile refusals; the legacy [`Classifier`] adapter retains its
//! subsumption-only contract for the compiler's supported class fragment.

use crate::reason::{Classifier, DefinitionTerm, ElInput};
use ontomark_reason::axiom::{
    Axiom as ElAxiom, Characteristic, ClassExpression as ClassExpr, Property,
};

/// `whelk-el`, single-threaded.
pub struct WhelkEl;

fn operand(term: &DefinitionTerm) -> ClassExpr {
    match term {
        DefinitionTerm::Named(iri) => ClassExpr::class(iri.clone()),
        DefinitionTerm::Some(property, filler) => {
            ClassExpr::some(property.clone(), ClassExpr::class(filler.clone()))
        }
    }
}

impl WhelkEl {
    /// Classify the already-selected legacy EL input using the shared supported
    /// fragment. Inspect refusals before treating the result as complete.
    #[must_use]
    pub fn classify(&self, input: &ElInput) -> ontomark_reason::Classification {
        ontomark_reason::classify(&axioms(input))
    }
}

fn axioms(input: &ElInput) -> Vec<ElAxiom> {
    let mut axioms = Vec::new();
    axioms.extend(input.classes.iter().cloned().map(ElAxiom::DeclareClass));
    axioms.extend(
        input
            .properties
            .iter()
            .cloned()
            .map(ElAxiom::DeclareProperty),
    );
    axioms.extend(
        input
            .transitive
            .iter()
            .cloned()
            .map(|p| ElAxiom::Characteristic {
                property: Property::named(p),
                characteristic: Characteristic::Transitive,
            }),
    );
    axioms.extend(
        input
            .sub_properties
            .iter()
            .map(|(sub, sup)| ElAxiom::SubPropertyOf {
                sub: Property::named(sub.clone()),
                sup: Property::named(sup.clone()),
            }),
    );
    axioms.extend(
        input
            .sub_classes
            .iter()
            .map(|(sub, sup)| ElAxiom::SubClassOf {
                sub: ClassExpr::class(sub.clone()),
                sup: ClassExpr::class(sup.clone()),
            }),
    );
    axioms.extend(input.disjoint.iter().map(|(a, b)| {
        ElAxiom::DisjointClasses(vec![
            ClassExpr::class(a.clone()),
            ClassExpr::class(b.clone()),
        ])
    }));
    axioms.extend(input.definitions.iter().map(|(class, terms)| {
        let definiens = if let [only] = terms.as_slice() {
            operand(only)
        } else {
            ClassExpr::And(terms.iter().map(operand).collect())
        };
        ElAxiom::EquivalentClasses(vec![ClassExpr::class(class.clone()), definiens])
    }));
    axioms.extend(
        input
            .existentials
            .iter()
            .map(|(sub, property, filler)| ElAxiom::SubClassOf {
                sub: ClassExpr::class(sub.clone()),
                sup: ClassExpr::some(property.clone(), ClassExpr::class(filler.clone())),
            }),
    );
    axioms
}

impl Classifier for WhelkEl {
    fn refusals(&self, input: &ElInput) -> Vec<String> {
        ontomark_reason::check(&axioms(input))
            .refusals
            .into_iter()
            .map(|r| format!("{}: {}", r.construct, r.reason))
            .collect()
    }
    fn named_subsumptions(&self, input: &ElInput) -> Vec<(String, String)> {
        let result = self.classify(input);
        let mut pairs: std::collections::BTreeSet<_> = result.subsumptions.into_iter().collect();
        pairs.extend(
            result
                .unsatisfiable
                .into_iter()
                .map(|c| (c, "http://www.w3.org/2002/07/owl#Nothing".to_owned())),
        );
        // The legacy trait includes these uninformative pairs; the compiler
        // removes them before emitting its asserted/inferred split.
        for class in &input.classes {
            pairs.insert((class.clone(), class.clone()));
            pairs.insert((
                class.clone(),
                "http://www.w3.org/2002/07/owl#Thing".to_owned(),
            ));
        }
        pairs.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_classifier_retains_definition_entailments_and_bottom_results() {
        let input = ElInput {
            classes: ["urn:A", "urn:B", "urn:P", "urn:Q"]
                .map(str::to_owned)
                .into(),
            sub_classes: [
                ("urn:P".into(), "urn:A".into()),
                ("urn:P".into(), "urn:B".into()),
            ]
            .into(),
            disjoint: [("urn:A".into(), "urn:B".into())].into(),
            definitions: [(
                "urn:Q".into(),
                vec![
                    DefinitionTerm::Named("urn:A".into()),
                    DefinitionTerm::Named("urn:B".into()),
                ],
            )]
            .into(),
            ..ElInput::default()
        };
        let result = WhelkEl.classify(&input);
        assert!(result.is_complete());
        assert_eq!(result.unsatisfiable, vec!["urn:P", "urn:Q"]);
        let old = whelk_el::classify(&[
            whelk_el::ElAxiom::sub_class("urn:P", "urn:A"),
            whelk_el::ElAxiom::sub_class("urn:P", "urn:B"),
            whelk_el::ElAxiom::DisjointClasses(vec![
                whelk_el::ClassExpr::named("urn:A"),
                whelk_el::ClassExpr::named("urn:B"),
            ]),
            whelk_el::ElAxiom::EquivalentClasses(vec![
                whelk_el::ClassExpr::named("urn:Q"),
                whelk_el::ClassExpr::And(vec![
                    whelk_el::ClassExpr::named("urn:A"),
                    whelk_el::ClassExpr::named("urn:B"),
                ]),
            ]),
        ]);
        let meaningful = |pairs: Vec<(String, String)>| -> std::collections::BTreeSet<_> {
            pairs
                .into_iter()
                .filter(|(s, o)| {
                    s != o
                        && s != "http://www.w3.org/2002/07/owl#Thing"
                        && o != "http://www.w3.org/2002/07/owl#Thing"
                        && s != "http://www.w3.org/2002/07/owl#Nothing"
                })
                .collect()
        };
        assert_eq!(
            meaningful(WhelkEl.named_subsumptions(&input)),
            meaningful(old)
        );
    }
}
