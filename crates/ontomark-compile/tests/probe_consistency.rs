// Ported from VisionClaw crates/vault/tests/probe_consistency.rs at 1044b0722 (ontomark-census-baseline).
//! ADR-2125 Decision item 3: the Whelk consistency gate is *provably armed*.
//!
//! The probe is the smallest EL++ inconsistency there is: `P ⊑ A`, `P ⊑ B`,
//! `A disjoint-with B`, with A and B siblings under one parent. Proposing the
//! second parent onto P must collapse P to `owl:Nothing`, and the assessment
//! the governance verbs run (`turtle::build_graph` then `reason::reason`, the
//! upstream `propose` step) must report P unsatisfiable, which the shared
//! [`whelk_inconsistent`] blocker turns into a `WHELK_INCONSISTENT` naming P.
//! The control is the same corpus without the disjointness: it must stay
//! clean, so the probe proves the axiom is what arms the gate.
//!
//! The `propose` verb itself stays in VisionClaw; this port drives the same
//! build-then-reason path directly.
//!
//! The probe was red until disjointness landed on this path: the emitter now
//! writes `owl:disjointWith` and `reason` extracts it into
//! `DisjointClasses`. A probe that passes cleanly (no blocker) fails CI.
//!
//! The fixture lives here, never in `knowledge/`.

use ontomark_compile::build::turtle::{build_graph, iri_to_uri};
use ontomark_compile::input::consistency::{whelk_inconsistent, WHELK_INCONSISTENT};
use ontomark_compile::input::page::{Page, Vault, VaultKind};
use ontomark_compile::input::promotion::Blocker;
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::model::Corpus;
use ontomark_compile::profile::Profile;
use ontomark_compile::reason;
use ontomark_compile::whelk::WhelkEl;

fn profile() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml"))
        .expect("estate profile")
}

/// The fixture vocabulary. `disjoint-with` is registered as the relation
/// ADR-2125 item 1 names; if the signed registration settles on a different
/// shape, this line follows it — the probe's assertions do not change.
const VOCABULARY: &str = r#"
version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
  disjoint-with: { owl: "owl:disjointWith" }
scalars:
  quality: { type: number, min: 0, max: 1 }
"#;

const P_IRI: &str = "urn:ngm:class:p";

fn vocab() -> Vocabulary {
    Vocabulary::from_yaml_str(VOCABULARY).expect("the fixture vocabulary parses")
}

fn class(id: &str, extra: &str) -> String {
    format!(
        "---\ntype: Class\npublic: true\nresource: urn:ngm:class:{}\nstatus: stable\nquality: 0.35\n{extra}---\nbody\n",
        id.to_lowercase()
    )
}

fn vault_of(pages: &[(&str, String)]) -> Vault {
    Vault {
        root: std::path::PathBuf::new(),
        kind: VaultKind::Knowledge,
        journals: Vec::new(),
        skipped: Vec::new(),
        pages: pages
            .iter()
            .map(|(id, text)| {
                Page::parse(format!("/v/{id}.md"), format!("{id}.md"), *id, text)
                    .expect("fixture page parses")
            })
            .collect(),
    }
}

/// The corpus: `Parent`, its children `A` and `B`, and `P`. `P ⊑ A` always;
/// with `second_parent` the proposal's `P ⊑ B` is applied too. When `disjoint`
/// is set, A carries `disjoint-with: [[B]]`.
fn corpus_vault(disjoint: bool, second_parent: bool) -> Vault {
    let a_extra = if disjoint {
        "is-a: [\"[[Parent]]\"]\ndisjoint-with: [\"[[B]]\"]\n"
    } else {
        "is-a: [\"[[Parent]]\"]\n"
    };
    let p_extra = if second_parent {
        "is-a: [\"[[A]]\", \"[[B]]\"]\n"
    } else {
        "is-a: [\"[[A]]\"]\n"
    };
    vault_of(&[
        ("Parent", class("Parent", "")),
        ("A", class("A", a_extra)),
        ("B", class("B", "is-a: [\"[[Parent]]\"]\n")),
        ("P", class("P", p_extra)),
    ])
}

/// The blockers the build-then-reason assessment reports for `P ⊑ A, P ⊑ B`:
/// one `WHELK_INCONSISTENT` per unsatisfiable class.
fn assess_second_parent(disjoint: bool) -> Vec<Blocker> {
    let vocab = vocab();
    let vault = corpus_vault(disjoint, true);
    let corpus = Corpus::build(&vault, &vocab);
    let graph = build_graph(&profile(), &corpus, &vocab, true);
    let reasoning = reason::reason(&graph, &WhelkEl).expect("within the Relevant cap");
    reasoning
        .unsatisfiable
        .iter()
        .map(|class| whelk_inconsistent(class))
        .collect()
}

#[test]
fn probe_disjoint_parents_block_with_whelk_inconsistent_naming_p() {
    let blockers = assess_second_parent(true);
    // Whelk names classes by their emitted URI; accept that or the IRI.
    let p_uri = iri_to_uri(&profile(), P_IRI);
    let whelk: Vec<_> = blockers
        .iter()
        .filter(|b| b.code == WHELK_INCONSISTENT)
        .collect();
    assert!(
        whelk
            .iter()
            .any(|b| b.detail.contains(P_IRI) || b.detail.contains(&p_uri)),
        "P ⊑ A, P ⊑ B, A disjoint-with B must block as WHELK_INCONSISTENT naming {P_IRI}; \
         blockers: {blockers:?}"
    );
}

/// The control: without the disjointness the same change is consistent, so a
/// green probe is caused by the axiom, not by something else in the fixture.
#[test]
fn control_without_disjointness_has_no_whelk_inconsistent() {
    let blockers = assess_second_parent(false);
    assert!(
        !blockers.iter().any(|b| b.code == WHELK_INCONSISTENT),
        "no disjointness ⇒ no WHELK_INCONSISTENT: {blockers:?}"
    );
}
