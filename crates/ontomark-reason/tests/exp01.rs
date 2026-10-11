//! EXP-01: native and wasm32 agree on every supported axiom.
//!
//! Each fixture in `tests/fixtures/` is an axiom list plus hand-written
//! expectations (subsumptions that must and must not hold, the unsatisfiable
//! classes, the root causes, the refused constructs). The full
//! [`Classification`] is also compared byte-for-byte with
//! `tests/golden/<name>.json`. The same test binary runs on the host
//! (`cargo test`) and on `wasm32-unknown-unknown` under Node
//! (`wasm-bindgen-test-runner`), against the same `include_str!`'d goldens,
//! so both targets producing the golden bytes is the parity proof. Each run
//! prints one `EXP-01 <name> sha256=<digest>` line per fixture for the
//! evidence record.
//!
//! `steps` is zeroed before comparison: the saturation's step count depends on
//! hash iteration order, which is randomised per process on the host; the
//! entailments do not.
//!
//! Regenerate goldens on the host with `ONTOMARK_BLESS=1 cargo test -p
//! ontomark-reason --test exp01`, then review the diff.
//!
//! The negative control runs a closure-only substitute (the told
//! `SubClassOf` transitive closure, as `vault/src/closure.rs` computes) over
//! the same expectations and requires it to fail the definition,
//! restriction and disjointness fixtures.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ontomark_reason::axiom::{Axiom, ClassExpression};
use ontomark_reason::{classify_with, Classification, Options};
use serde::Deserialize;

macro_rules! fixtures {
    ($($name:literal),* $(,)?) => {
        &[$((
            $name,
            include_str!(concat!("fixtures/", $name, ".json")),
            include_str!(concat!("golden/", $name, ".json")),
        )),*]
    };
}

const FIXTURES: &[(&str, &str, &str)] = fixtures![
    "defined_class_intersection_of_named",
    "defined_classes_definition",
    "defined_classes_definition_relevant",
    "defined_classes_skip_equivalent",
    "defined_classes_without_definition",
    "disjoint_parents_control",
    "disjoint_parents_probe",
    "disjointness_through_existential_filler",
    "disjointness_through_existential_filler_relevant",
    "el_profile_refusals",
    "equivalent_named_and_chain",
    "existential_role_hierarchy",
    "existential_role_hierarchy_relevant",
    "existential_transitive_part_of",
    "existential_transitive_part_of_without_transitivity",
    "two_independent_clashes",
];

/// Fixtures whose expectations need more than the told closure: the
/// substitute must fail every one of them.
const BEYOND_CLOSURE: &[&str] = &[
    "defined_class_intersection_of_named",
    "defined_classes_definition",
    "defined_classes_definition_relevant",
    "disjoint_parents_probe",
    "disjointness_through_existential_filler",
    "disjointness_through_existential_filler_relevant",
    "el_profile_refusals",
    "equivalent_named_and_chain",
    "existential_role_hierarchy",
    "existential_role_hierarchy_relevant",
    "existential_transitive_part_of",
    "existential_transitive_part_of_without_transitivity",
    "two_independent_clashes",
];

#[derive(Debug, Deserialize)]
struct Fixture {
    name: String,
    options: Options,
    axioms: Vec<Axiom>,
    must_include: Vec<(String, String)>,
    must_exclude: Vec<(String, String)>,
    unsatisfiable: Vec<String>,
    root_causes: Option<Vec<String>>,
    refused: Vec<String>,
}

fn load(json: &str) -> Fixture {
    serde_json::from_str(json).expect("fixture parses")
}

fn canonical(c: &Classification) -> String {
    let mut c = c.clone();
    c.steps = 0;
    let mut s = serde_json::to_string_pretty(&c).expect("serialises");
    s.push('\n');
    s
}

fn sha256(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn print(line: &str) {
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::console_log!("{line}");
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{line}");
}

/// The fixture's hand-written expectations, as a list of violations.
fn violations(f: &Fixture, c: &Classification) -> Vec<String> {
    let mut out = Vec::new();
    let subs: BTreeSet<&(String, String)> = c.subsumptions.iter().collect();
    for p in &f.must_include {
        if !subs.contains(p) {
            out.push(format!("missing {} ⊑ {}", p.0, p.1));
        }
    }
    for p in &f.must_exclude {
        if subs.contains(p) {
            out.push(format!("unexpected {} ⊑ {}", p.0, p.1));
        }
    }
    let mut unsat = f.unsatisfiable.clone();
    unsat.sort();
    if c.unsatisfiable != unsat {
        out.push(format!(
            "unsatisfiable {:?} != expected {:?}",
            c.unsatisfiable, unsat
        ));
    }
    if let Some(roots) = &f.root_causes {
        let mut roots = roots.clone();
        roots.sort();
        if c.root_causes != roots {
            out.push(format!(
                "root causes {:?} != expected {:?}",
                c.root_causes, roots
            ));
        }
    }
    let refused: Vec<&str> = c.refusals.iter().map(|r| r.construct.as_str()).collect();
    let expected: Vec<&str> = f.refused.iter().map(String::as_str).collect();
    if refused != expected {
        out.push(format!("refused {refused:?} != expected {expected:?}"));
    }
    out
}

#[test]
fn every_fixture_meets_its_expectations_and_matches_its_golden() {
    let mut failures = Vec::new();
    for (name, json, golden) in FIXTURES {
        let f = load(json);
        assert_eq!(&f.name, name);
        let c = classify_with(&f.axioms, f.options).expect("no budget set");
        let v = violations(&f, &c);
        if !v.is_empty() {
            failures.push(format!("{name}: {v:?}"));
        }
        let got = canonical(&c);
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var_os("ONTOMARK_BLESS").is_some() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/golden")
                .join(format!("{name}.json"));
            std::fs::write(path, &got).expect("write golden");
            continue;
        }
        if got != *golden {
            failures.push(format!("{name}: classification differs from its golden"));
        }
        print(&format!(
            "EXP-01 {name} sha256={} subsumptions={} inferred={} unsatisfiable={} refusals={}",
            sha256(&got),
            c.subsumptions.len(),
            c.inferred.len(),
            c.unsatisfiable.len(),
            c.refusals.len()
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_relevant_policy_changes_no_named_subsumption_on_these_fixtures() {
    for (name, json, _) in FIXTURES {
        let f = load(json);
        let all = classify_with(&f.axioms, Options::default()).expect("no budget");
        let relevant = classify_with(
            &f.axioms,
            Options {
                existentials: ontomark_reason::ExistentialPolicy::Relevant,
                max_steps: None,
            },
        )
        .expect("no budget");
        // Compare subsumptions among classes satisfiable under both.
        let sat = |c: &Classification| -> BTreeSet<(String, String)> {
            let unsat: BTreeSet<&String> =
                all.unsatisfiable.iter().chain(&c.unsatisfiable).collect();
            c.subsumptions
                .iter()
                .filter(|(a, b)| !unsat.contains(a) && !unsat.contains(b))
                .cloned()
                .collect()
        };
        assert_eq!(sat(&all), sat(&relevant), "{name}");
    }
}

/// The closure-only substitute: told `SubClassOf(Class, Class)` closed
/// transitively. No definitions, restrictions, disjointness or refusals.
fn closure_only(axioms: &[Axiom]) -> Classification {
    let mut parents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for a in axioms {
        if let Axiom::SubClassOf {
            sub: ClassExpression::Class(s),
            sup: ClassExpression::Class(p),
        } = a
        {
            parents.entry(s.clone()).or_default().insert(p.clone());
        }
    }
    let mut subsumptions = BTreeSet::new();
    for start in parents.keys() {
        let mut stack: Vec<&String> = parents[start].iter().collect();
        let mut seen = BTreeSet::new();
        while let Some(p) = stack.pop() {
            if p != start && seen.insert(p.clone()) {
                subsumptions.insert((start.clone(), p.clone()));
                if let Some(next) = parents.get(p) {
                    stack.extend(next);
                }
            }
        }
    }
    Classification {
        subsumptions: subsumptions.into_iter().collect(),
        ..Classification::default()
    }
}

#[test]
fn the_closure_only_substitute_fails_exp01() {
    let mut failed = BTreeSet::new();
    for (name, json, golden) in FIXTURES {
        let f = load(json);
        let c = closure_only(&f.axioms);
        let v = violations(&f, &c);
        let golden_differs = canonical(&c) != *golden;
        if !v.is_empty() {
            failed.insert(*name);
        }
        print(&format!(
            "EXP-01 negative {name}: substitute expectation violations={} golden_differs={golden_differs}",
            v.len()
        ));
    }
    for name in BEYOND_CLOSURE {
        assert!(
            failed.contains(name),
            "the closure-only substitute passed {name}"
        );
    }
    print(&format!(
        "EXP-01 negative: substitute fails {} of {} fixtures",
        failed.len(),
        FIXTURES.len()
    ));
}
