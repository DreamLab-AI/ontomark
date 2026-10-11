//! Entailment tests that run unchanged on the host and on
//! `wasm32-unknown-unknown` (through `wasm-bindgen-test-runner` under Node).
//!
//! Each case is the smallest axiom set where the named entailment needs the
//! rule under test, with a control showing the entailment is absent without
//! the load-bearing axiom.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use whelk_el::reasoner::{Progress, Saturation};
use whelk_el::{
    assert, assert_append, classify, components, translate, ClassExpr, ElAxiom, BOTTOM,
};

fn iri(local: &str) -> String {
    format!("urn:t:{local}")
}

fn named(local: &str) -> ClassExpr {
    ClassExpr::Named(iri(local))
}

fn sub(a: &str, b: &str) -> ElAxiom {
    ElAxiom::sub_class(iri(a), iri(b))
}

fn some(p: &str, filler: ClassExpr) -> ClassExpr {
    ClassExpr::some(iri(p), filler)
}

fn entails(pairs: &[(String, String)], a: &str, b: &str) -> bool {
    pairs.binary_search(&(iri(a), iri(b))).is_ok()
}

fn unsatisfiable(pairs: &[(String, String)], a: &str) -> bool {
    pairs.binary_search(&(iri(a), BOTTOM.to_owned())).is_ok()
}

/// `GrippingRobot ≡ Robot ⊓ ∃hasPart.Gripper`; `Arm ⊑ Robot`,
/// `Arm ⊑ ∃hasPart.SmallGripper`, `SmallGripper ⊑ Gripper`.
fn defined_class(with_definition: bool) -> Vec<ElAxiom> {
    let mut axioms = vec![
        sub("Arm", "Robot"),
        ElAxiom::SubClassOf {
            sub: named("Arm"),
            sup: some("hasPart", named("SmallGripper")),
        },
        sub("SmallGripper", "Gripper"),
        ElAxiom::DeclareClass(iri("GrippingRobot")),
    ];
    if with_definition {
        axioms.push(ElAxiom::EquivalentClasses(vec![
            named("GrippingRobot"),
            ClassExpr::And(vec![named("Robot"), some("hasPart", named("Gripper"))]),
        ]));
    }
    axioms
}

#[test]
fn a_defined_class_classifies_a_class_nobody_placed_under_it() {
    let with = classify(&defined_class(true));
    assert!(entails(&with, "Arm", "GrippingRobot"));
    assert!(!entails(&with, "Robot", "GrippingRobot"));
    let without = classify(&defined_class(false));
    assert!(!entails(&without, "Arm", "GrippingRobot"));
}

#[test]
fn intersection_operand_order_and_nesting_do_not_matter() {
    let mut axioms = defined_class(false);
    axioms.push(ElAxiom::EquivalentClasses(vec![
        named("GrippingRobot"),
        ClassExpr::And(vec![
            ClassExpr::And(vec![some("hasPart", named("Gripper"))]),
            ClassExpr::Thing,
            named("Robot"),
        ]),
    ]));
    assert_eq!(classify(&axioms), classify(&defined_class(true)));
}

#[test]
fn disjoint_parents_make_a_common_subclass_unsatisfiable() {
    let mut axioms = vec![sub("P", "A"), sub("P", "B"), sub("Q", "P")];
    let consistent = classify(&axioms);
    assert!(!unsatisfiable(&consistent, "P"));
    axioms.push(ElAxiom::DisjointClasses(vec![named("A"), named("B")]));
    let clash = classify(&axioms);
    assert!(unsatisfiable(&clash, "P"));
    assert!(unsatisfiable(&clash, "Q"));
    assert!(!unsatisfiable(&clash, "A"));
}

#[test]
fn an_unsatisfiable_filler_makes_its_restricted_class_unsatisfiable() {
    let axioms = vec![
        ElAxiom::SubClassOf {
            sub: named("C"),
            sup: some("r", named("D")),
        },
        sub("D", "A"),
        sub("D", "B"),
        ElAxiom::DisjointClasses(vec![named("A"), named("B")]),
    ];
    let pairs = classify(&axioms);
    assert!(unsatisfiable(&pairs, "D"));
    assert!(unsatisfiable(&pairs, "C"));
}

#[test]
fn existentials_propagate_through_filler_subsumption() {
    // C ⊑ ∃r.D, D ⊑ E, ∃r.E ⊑ F ⇒ C ⊑ F
    let axioms = vec![
        ElAxiom::SubClassOf {
            sub: named("C"),
            sup: some("r", named("D")),
        },
        sub("D", "E"),
        ElAxiom::SubClassOf {
            sub: some("r", named("E")),
            sup: named("F"),
        },
    ];
    assert!(entails(&classify(&axioms), "C", "F"));
    assert!(!entails(&classify(&axioms[..2]), "C", "F"));
}

#[test]
fn existentials_propagate_through_the_role_hierarchy() {
    // C ⊑ ∃s.D, s ⊑ r, ∃r.D ⊑ F ⇒ C ⊑ F
    let base = vec![
        ElAxiom::SubClassOf {
            sub: named("C"),
            sup: some("s", named("D")),
        },
        ElAxiom::SubClassOf {
            sub: some("r", named("D")),
            sup: named("F"),
        },
    ];
    assert!(!entails(&classify(&base), "C", "F"));
    let mut with = base.clone();
    with.push(ElAxiom::SubPropertyOf {
        sub: iri("s"),
        sup: iri("r"),
    });
    assert!(entails(&classify(&with), "C", "F"));
}

#[test]
fn transitivity_composes_links() {
    // C ⊑ ∃r.D, D ⊑ ∃r.E, ∃r.E ⊑ G; r transitive ⇒ C ⊑ G
    let base = vec![
        ElAxiom::SubClassOf {
            sub: named("C"),
            sup: some("r", named("D")),
        },
        ElAxiom::SubClassOf {
            sub: named("D"),
            sup: some("r", named("E")),
        },
        ElAxiom::SubClassOf {
            sub: some("r", named("E")),
            sup: named("G"),
        },
    ];
    let without = classify(&base);
    assert!(entails(&without, "D", "G"));
    assert!(!entails(&without, "C", "G"));
    let mut with = base.clone();
    with.push(ElAxiom::TransitiveProperty(iri("r")));
    assert!(entails(&classify(&with), "C", "G"));
}

#[test]
fn a_three_link_chain_is_binarised() {
    // C ⊑ ∃p.D, D ⊑ ∃q.E, E ⊑ ∃s.F, p∘q∘s ⊑ t, ∃t.F ⊑ G ⇒ C ⊑ G
    let axioms = vec![
        ElAxiom::SubClassOf {
            sub: named("C"),
            sup: some("p", named("D")),
        },
        ElAxiom::SubClassOf {
            sub: named("D"),
            sup: some("q", named("E")),
        },
        ElAxiom::SubClassOf {
            sub: named("E"),
            sup: some("s", named("F")),
        },
        ElAxiom::PropertyChain {
            chain: vec![iri("p"), iri("q"), iri("s")],
            sup: iri("t"),
        },
        ElAxiom::SubClassOf {
            sub: some("t", named("F")),
            sup: named("G"),
        },
    ];
    assert!(entails(&classify(&axioms), "C", "G"));
}

/// Regression for the upstream `group_by` defect: one chain pair with two
/// super-properties keeps both.
#[test]
fn a_chain_with_two_super_properties_implies_both() {
    let mut axioms = vec![
        ElAxiom::SubClassOf {
            sub: named("C"),
            sup: some("p", named("D")),
        },
        ElAxiom::SubClassOf {
            sub: named("D"),
            sup: some("q", named("E")),
        },
        ElAxiom::SubClassOf {
            sub: some("t1", named("E")),
            sup: named("G1"),
        },
        ElAxiom::SubClassOf {
            sub: some("t2", named("E")),
            sup: named("G2"),
        },
    ];
    // Interleave unrelated chains so the two chains for (p, q) are unlikely
    // to be adjacent in any iteration order.
    for i in 0..16 {
        axioms.push(ElAxiom::PropertyChain {
            chain: vec![iri(&format!("x{i}")), iri(&format!("y{i}"))],
            sup: iri(&format!("z{i}")),
        });
    }
    axioms.push(ElAxiom::PropertyChain {
        chain: vec![iri("p"), iri("q")],
        sup: iri("t1"),
    });
    axioms.push(ElAxiom::PropertyChain {
        chain: vec![iri("p"), iri("q")],
        sup: iri("t2"),
    });
    let pairs = classify(&axioms);
    assert!(entails(&pairs, "C", "G1"));
    assert!(entails(&pairs, "C", "G2"));
}

#[test]
fn a_domain_classifies_the_subject() {
    let axioms = vec![
        ElAxiom::SubClassOf {
            sub: named("C"),
            sup: some("drives", named("Car")),
        },
        ElAxiom::PropertyDomain {
            property: iri("drives"),
            class: named("Driver"),
        },
    ];
    assert!(entails(&classify(&axioms), "C", "Driver"));
}

#[test]
fn a_nominal_filler_and_assertions_classify() {
    // ∃livesIn.{london} ⊑ Londoner; C ⊑ ∃livesIn.{london}
    let axioms = vec![
        ElAxiom::SubClassOf {
            sub: named("C"),
            sup: some("livesIn", ClassExpr::Individual(iri("london"))),
        },
        ElAxiom::SubClassOf {
            sub: some("livesIn", ClassExpr::Individual(iri("london"))),
            sup: named("Londoner"),
        },
        ElAxiom::ClassAssertion {
            individual: iri("london"),
            class: named("City"),
        },
    ];
    assert!(entails(&classify(&axioms), "C", "Londoner"));
}

#[test]
fn equivalent_named_classes_subsume_each_other() {
    let axioms = vec![
        ElAxiom::EquivalentClasses(vec![named("Car"), named("Automobile")]),
        sub("Saloon", "Car"),
    ];
    let pairs = classify(&axioms);
    assert!(entails(&pairs, "Car", "Automobile"));
    assert!(entails(&pairs, "Automobile", "Car"));
    assert!(entails(&pairs, "Saloon", "Automobile"));
}

fn mixed_fixture() -> Vec<ElAxiom> {
    let mut axioms = defined_class(true);
    axioms.extend([
        sub("P", "A"),
        sub("P", "B"),
        ElAxiom::DisjointClasses(vec![named("A"), named("B")]),
        ElAxiom::TransitiveProperty(iri("partOf")),
        ElAxiom::SubClassOf {
            sub: named("Finger"),
            sup: some("partOf", named("Hand")),
        },
        ElAxiom::SubClassOf {
            sub: named("Hand"),
            sup: some("partOf", named("Body")),
        },
        ElAxiom::EquivalentClasses(vec![named("BodyPart"), some("partOf", named("Body"))]),
    ]);
    for i in 0..50 {
        axioms.push(sub(&format!("L{i}"), &format!("L{}", i + 1)));
    }
    axioms
}

#[test]
fn sliced_saturation_equals_one_shot_saturation() {
    let axioms = translate(&mixed_fixture());
    let whole = assert(&axioms).named_subsumption_ids();
    for budget in [1, 7, 100] {
        let mut s = Saturation::new(&axioms);
        let mut slices = 0;
        while s.run(budget) == Progress::Pending {
            slices += 1;
        }
        assert!(slices > 0 || budget == 100);
        let state = s.into_state().expect("done");
        assert_eq!(state.named_subsumption_ids(), whole, "budget {budget}");
    }
}

#[test]
fn an_unfinished_saturation_refuses_to_yield_a_state() {
    let mut s = Saturation::new(&translate(&mixed_fixture()));
    assert_eq!(s.run(3), Progress::Pending);
    assert_eq!(s.steps(), 3);
    assert!(s.into_state().is_err());
}

#[test]
fn appending_equals_asserting_together() {
    let all = mixed_fixture();
    let (first, second) = all.split_at(all.len() / 2);
    // Role axioms must be present from the start: append takes concept
    // inclusions only.
    let mut base: Vec<ElAxiom> = first.to_vec();
    base.push(ElAxiom::TransitiveProperty(iri("partOf")));
    let state = assert(&translate(&base));
    let extra: im::HashSet<_> = translate(second)
        .into_iter()
        .filter_map(|ax| match &*ax {
            whelk_el::model::Axiom::ConceptInclusion(ci) => Some(std::rc::Rc::clone(ci)),
            _ => None,
        })
        .collect();
    let before = state.named_subsumption_ids();
    let appended = assert_append(&extra, &state);
    assert_eq!(
        state.named_subsumption_ids(),
        before,
        "append must not modify its input"
    );
    let mut together = base.clone();
    together.extend_from_slice(second);
    assert_eq!(appended.named_subsumption_ids(), classify(&together));
}

#[test]
fn components_reason_independently_to_the_same_result() {
    let axioms = mixed_fixture();
    let groups = components(&axioms);
    assert!(groups.len() > 1, "fixture should have several components");
    let mut merged: Vec<(String, String)> = groups
        .iter()
        .flat_map(|g| {
            let subset: Vec<ElAxiom> = g.iter().map(|&i| axioms[i].clone()).collect();
            classify(&subset)
        })
        .collect();
    merged.sort_unstable();
    merged.dedup();
    assert_eq!(merged, classify(&axioms));
}

#[test]
fn thing_on_the_left_forces_one_component() {
    let mut axioms = mixed_fixture();
    axioms.push(ElAxiom::SubClassOf {
        sub: ClassExpr::Thing,
        sup: named("Everything"),
    });
    assert_eq!(components(&axioms).len(), 1);
    assert!(entails(&classify(&axioms), "L0", "Everything"));
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_classification_equals_sequential() {
    let axioms = mixed_fixture();
    assert_eq!(whelk_el::classify_parallel(&axioms), classify(&axioms));
}

/// Cross-vendor audit counterexample: a chain whose two super-properties are
/// equivalent roles must survive composition pruning.
#[test]
fn a_chain_into_equivalent_roles_survives_pruning() {
    let axioms = vec![
        ElAxiom::PropertyChain {
            chain: vec![iri("r"), iri("s")],
            sup: iri("t"),
        },
        ElAxiom::PropertyChain {
            chain: vec![iri("r"), iri("s")],
            sup: iri("u"),
        },
        ElAxiom::SubPropertyOf {
            sub: iri("t"),
            sup: iri("u"),
        },
        ElAxiom::SubPropertyOf {
            sub: iri("u"),
            sup: iri("t"),
        },
        ElAxiom::SubClassOf {
            sub: named("A"),
            sup: some("r", named("B")),
        },
        ElAxiom::SubClassOf {
            sub: named("B"),
            sup: some("s", named("C")),
        },
        ElAxiom::SubClassOf {
            sub: some("t", named("C")),
            sup: named("D"),
        },
    ];
    assert!(entails(&classify(&axioms), "A", "D"));
}

/// Cross-vendor audit counterexample: `⊥ ⊑ B` joins every unsatisfiable
/// class to `B`, so it must not be split into its own component.
#[test]
fn nothing_on_the_left_forces_one_component() {
    let axioms = vec![
        ElAxiom::SubClassOf {
            sub: named("A"),
            sup: ClassExpr::Nothing,
        },
        ElAxiom::SubClassOf {
            sub: ClassExpr::Nothing,
            sup: named("B"),
        },
    ];
    assert_eq!(components(&axioms).len(), 1);
    assert!(entails(&classify(&axioms), "A", "B"));
}
