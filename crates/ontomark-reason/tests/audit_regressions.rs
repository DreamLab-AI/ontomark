//! Counterexamples from the cross-vendor (Codex, gpt-6-astra) audit of WP-B,
//! kept as regressions. Each held against an earlier revision. Runs on the
//! host and on wasm32.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ontomark_reason::axiom::{Axiom, ClassExpression as C};
use ontomark_reason::{classify_with, Classification, ExistentialPolicy, Options};

fn both(axioms: &[Axiom]) -> (Classification, Classification) {
    let run = |existentials| {
        classify_with(
            axioms,
            Options {
                existentials,
                max_steps: None,
            },
        )
        .expect("no budget")
    };
    (
        run(ExistentialPolicy::All),
        run(ExistentialPolicy::Relevant),
    )
}

fn has(c: &Classification, a: &str, b: &str) -> bool {
    c.subsumptions
        .binary_search(&(a.to_owned(), b.to_owned()))
        .is_ok()
}

/// `A ⊑ B ⊓ ∃r.C` with `r` irrelevant: Relevant dropped the whole axiom and
/// lost `A ⊑ B`. It must drop only the existential conjunct.
#[test]
fn relevant_keeps_named_conjuncts_of_a_reduced_axiom() {
    let axioms = vec![
        Axiom::DeclareClass("urn:A".into()),
        Axiom::DeclareClass("urn:B".into()),
        Axiom::DeclareClass("urn:C".into()),
        Axiom::SubClassOf {
            sub: C::class("urn:A"),
            sup: C::And(vec![C::class("urn:B"), C::some("urn:r", C::class("urn:C"))]),
        },
    ];
    let (all, relevant) = both(&axioms);
    assert!(has(&all, "urn:A", "urn:B"));
    assert!(has(&relevant, "urn:A", "urn:B"));
    assert_eq!(relevant.existentials_dropped, 1);
    assert_eq!(all.subsumptions, relevant.subsumptions);
}

/// `⊤ ⊑ B`, `A ⊑ ∃r.C`: under All, A and C are initialised and fall under B.
/// Relevant dropped the only axiom mentioning them and lost both.
#[test]
fn relevant_still_initialises_classes_of_a_dropped_restriction() {
    let axioms = vec![
        Axiom::SubClassOf {
            sub: C::Thing,
            sup: C::class("urn:B"),
        },
        Axiom::SubClassOf {
            sub: C::class("urn:A"),
            sup: C::some("urn:r", C::class("urn:C")),
        },
    ];
    let (all, relevant) = both(&axioms);
    assert!(has(&all, "urn:A", "urn:B") && has(&all, "urn:C", "urn:B"));
    assert_eq!(all.subsumptions, relevant.subsumptions);
}

/// Round 3: `A ⊑ ∃r.(B ⊓ C)` with B, C disjoint makes A unsatisfiable; with
/// `r` irrelevant, Relevant used to drop the restriction and report a
/// consistent ontology. An anonymous filler is never dropped.
#[test]
fn relevant_never_hides_a_clash_inside_an_anonymous_filler() {
    let axioms = vec![
        Axiom::SubClassOf {
            sub: C::class("urn:A"),
            sup: C::some("urn:r", C::And(vec![C::class("urn:B"), C::class("urn:C")])),
        },
        Axiom::DisjointClasses(vec![C::class("urn:B"), C::class("urn:C")]),
    ];
    let (all, relevant) = both(&axioms);
    assert_eq!(all.unsatisfiable, vec!["urn:A".to_owned()]);
    assert_eq!(relevant.unsatisfiable, all.unsatisfiable);
    assert_eq!(relevant.existentials_dropped, 0);
}

/// Round 3: built-in properties and nominal class expressions are refused,
/// not reasoned over with ordinary semantics.
#[test]
fn built_in_properties_and_nominals_are_refused() {
    let axioms = vec![
        Axiom::SubClassOf {
            sub: C::class("urn:A"),
            sup: C::some(
                "http://www.w3.org/2002/07/owl#bottomObjectProperty",
                C::Thing,
            ),
        },
        Axiom::SubClassOf {
            sub: C::class("urn:A"),
            sup: C::OneOf(vec!["urn:a".into()]),
        },
    ];
    let c = ontomark_reason::classify(&axioms);
    let refused: Vec<&str> = c.refusals.iter().map(|r| r.construct.as_str()).collect();
    assert_eq!(refused, ["owl:bottomObjectProperty", "ObjectOneOf"]);
    assert!(c.refusals.iter().all(|r| r.code() == "EL_NOT_IMPLEMENTED"));
}

#[test]
fn a_dropped_existential_does_not_claim_completeness_or_consistency() {
    let axioms = vec![
        Axiom::SubClassOf {
            sub: C::class("urn:A"),
            sup: C::some("urn:r", C::class("urn:C")),
        },
        Axiom::SubClassOf {
            sub: C::class("urn:C"),
            sup: C::Nothing,
        },
    ];
    let (all, relevant) = both(&axioms);
    assert!(all.is_complete());
    assert_eq!(all.unsatisfiable, ["urn:A", "urn:C"]);
    assert_eq!(relevant.existentials_dropped, 1);
    assert!(!relevant.is_complete());
    assert!(!relevant.is_consistent());
}

#[test]
fn individual_assertions_are_refused_and_cannot_claim_consistency() {
    use ontomark_reason::axiom::Property;

    for asserted_class in [
        C::Nothing,
        C::And(vec![C::class("urn:A"), C::class("urn:B")]),
    ] {
        let axioms = vec![
            Axiom::ClassAssertion {
                individual: "urn:a".into(),
                class: asserted_class,
            },
            Axiom::PropertyAssertion {
                subject: "urn:a".into(),
                property: Property::named("urn:r"),
                target: "urn:b".into(),
            },
            Axiom::DisjointClasses(vec![C::class("urn:A"), C::class("urn:B")]),
        ];
        let c = ontomark_reason::classify(&axioms);
        assert_eq!(c.refusals.len(), 2);
        assert_eq!(c.refusals[0].construct, "ClassAssertion");
        assert_eq!(c.refusals[1].construct, "ObjectPropertyAssertion");
        assert!(c.refusals.iter().all(|r| r.code() == "EL_NOT_IMPLEMENTED"));
        assert!(!c.is_complete());
        assert!(!c.is_consistent());
    }
}

#[test]
fn consistency_is_not_claimed_for_an_incomplete_relevant_result_without_clashes() {
    let axioms = vec![Axiom::SubClassOf {
        sub: C::class("urn:A"),
        sup: C::some("urn:r", C::class("urn:B")),
    }];
    let (all, relevant) = both(&axioms);
    assert!(all.is_consistent());
    assert!(relevant.unsatisfiable.is_empty());
    assert!(!relevant.is_consistent());
}

#[test]
fn an_unsatisfiable_thing_is_reported_even_without_named_classes() {
    let axioms = [Axiom::SubClassOf {
        sub: C::Thing,
        sup: C::Nothing,
    }];
    let (all, relevant) = both(&axioms);
    for c in [all, relevant] {
        assert!(
            c.is_complete(),
            "contradiction does not imply an unsupported axiom"
        );
        assert_eq!(c.classes, 0);
        assert_eq!(c.unsatisfiable, [whelk_el::TOP]);
        assert!(!c.is_consistent());
    }
}
