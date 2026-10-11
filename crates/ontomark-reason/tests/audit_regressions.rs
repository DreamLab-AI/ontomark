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
