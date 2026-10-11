// Ported from VisionClaw crates/vault/tests/defined_classes.rs at 1044b0722 (ontomark-census-baseline).
//! ADR-2124: a curated EL defined class, end to end through the real corpus
//! path — frontmatter, `Corpus::build`, `vault validate`, `turtle::build_graph`
//! and `reason::reason`.
//!
//! The fixture is the smallest corpus where a definition earns its keep:
//! `Gripping Robot` is *defined* (`defines-as`) as `Robot ⊓ ∃hasPart.Gripper`,
//! and `Arm` is authored as a `Robot` with a `Small Gripper` part. Nobody wrote
//! `Arm ⊑ Gripping Robot`; the reasoner must derive it. Forced to
//! `Restrictions::Skip` it must not — which is what proves the mode switch is
//! load-bearing rather than decorative.
//!
//! The fixture lives here, never in `knowledge/`: which classes get a
//! definition is the owner's call.

use ontomark_compile::build::turtle::{self, iri_to_uri};
use ontomark_compile::check::validate;
use ontomark_compile::input::page::{Page, Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::model::Corpus;
use ontomark_compile::profile::Profile;
use ontomark_compile::reason::{self, Restrictions};
use ontomark_compile::whelk::WhelkEl;

fn profile() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml"))
        .expect("estate profile")
}

const VOCABULARY: &str = r#"
version: 1
namespace: "urn:ngm:class:"
prefixes: { vc: 'https://narrativegoldmine.com/ns/v1#' }
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a:       { owl: "rdfs:subClassOf" }
  has-part:   { owl: "vc:hasPart", restriction: true }
  related-to: { owl: "vc:relatedTo" }
  defines-as: { owl: "owl:equivalentClass", status: provisional }
scalars:
  quality: { type: number, min: 0, max: 1 }
"#;

fn vocab() -> Vocabulary {
    Vocabulary::from_yaml_str(VOCABULARY).expect("the fixture vocabulary parses")
}

fn class(id: &str, extra: &str) -> String {
    format!(
        "---\ntype: Class\npublic: true\nresource: urn:ngm:class:{}\nstatus: stable\nquality: 0.35\n{extra}---\nbody\n",
        id.to_lowercase().replace(' ', "-")
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
                Page::parse(
                    format!("/v/pages/{id}.md"),
                    format!("pages/{id}.md"),
                    *id,
                    text,
                )
                .expect("fixture page parses")
            })
            .collect(),
    }
}

/// The documented frontmatter shape (ADR-2124 Verification): a list whose
/// items are a named class `"[[Slug]]"` or a one-entry map
/// `relation-key: "[[Slug]]"` meaning `∃relation.Slug`.
const DEFINITION: &str = "defines-as:\n  - \"[[Robot]]\"\n  - has-part: \"[[Gripper]]\"\n";

fn fixture(with_definition: bool) -> Vault {
    let defined = if with_definition { DEFINITION } else { "" };
    vault_of(&[
        ("Robot", class("Robot", "")),
        ("Gripper", class("Gripper", "")),
        (
            "Small Gripper",
            class("Small Gripper", "is-a: [\"[[Gripper]]\"]\n"),
        ),
        (
            "Arm",
            class(
                "Arm",
                "is-a: [\"[[Robot]]\"]\nhas-part: [\"[[Small Gripper]]\"]\nrelated-to: [\"[[Gripper]]\"]\n",
            ),
        ),
        ("Gripping Robot", class("Gripping Robot", defined)),
    ])
}

fn arm_under_gripping_robot(r: &reason::Reasoning) -> bool {
    r.inferred.contains(&(
        iri_to_uri(&profile(), "urn:ngm:class:arm"),
        iri_to_uri(&profile(), "urn:ngm:class:gripping-robot"),
    ))
}

#[test]
fn the_definition_fixture_validates_cleanly() {
    let vault = fixture(true);
    let vocab = vocab();
    let corpus = Corpus::build(&vault, &vocab);
    let report = validate::validate(&profile(), &vault, &corpus, &vocab);
    assert!(
        report.errors().is_empty(),
        "a well-formed EL definition must validate: {:?}",
        report.errors()
    );
}

#[test]
fn one_defines_as_page_yields_a_named_subsumption_absent_from_the_asserted_graph() {
    let vault = fixture(true);
    let vocab = vocab();
    let corpus = Corpus::build(&vault, &vocab);
    let graph = turtle::build_graph(&profile(), &corpus, &vocab, true);

    let ttl = turtle::serialise(&profile(), &graph);
    assert!(ttl.contains("owl:equivalentClass"), "{ttl}");
    assert!(ttl.contains("owl:intersectionOf"), "{ttl}");
    // Not asserted: the only subClassOf objects of Arm are Robot and a
    // restriction node.
    let arm = iri_to_uri(&profile(), "urn:ngm:class:arm");
    let gripping = iri_to_uri(&profile(), "urn:ngm:class:gripping-robot");
    assert!(!graph.iter().any(|(s, p, o)| {
        *s == turtle::Subject::Iri(arm.clone())
            && p.ends_with("#subClassOf")
            && *o == turtle::Term::Iri(gripping.clone())
    }));

    let r = reason::reason(&graph, &WhelkEl).expect("within the Relevant cap");
    assert_eq!(r.mode, Restrictions::Relevant);
    assert_eq!(r.defined_classes, 1);
    assert!(
        arm_under_gripping_robot(&r),
        "Arm ⊑ Robot ⊓ ∃hasPart.Gripper must be derived: {:?}",
        r.inferred
    );
    // Only the hasPart restriction feeds the definition; there is no other
    // restricted property in this vocabulary, so it is the only one.
    assert_eq!(r.restrictions_kept, 1);
}

#[test]
fn the_same_fixture_forced_to_skip_loses_the_subsumption() {
    let vault = fixture(true);
    let vocab = vocab();
    let corpus = Corpus::build(&vault, &vocab);
    let graph = turtle::build_graph(&profile(), &corpus, &vocab, true);
    let skip = reason::reason_with(&graph, Restrictions::Skip, &WhelkEl);
    assert!(!arm_under_gripping_robot(&skip), "{:?}", skip.inferred);
    let include = reason::reason_with(&graph, Restrictions::Include, &WhelkEl);
    assert_eq!(
        include.inferred,
        reason::reason(&graph, &WhelkEl)
            .expect("within the Relevant cap")
            .inferred
    );
}

#[test]
fn without_the_definition_the_corpus_reasons_with_skip_and_derives_nothing_new() {
    let vault = fixture(false);
    let vocab = vocab();
    let corpus = Corpus::build(&vault, &vocab);
    let graph = turtle::build_graph(&profile(), &corpus, &vocab, true);
    let r = reason::reason(&graph, &WhelkEl).expect("within the Relevant cap");
    assert_eq!(r.mode, Restrictions::Skip);
    assert_eq!(r.defined_classes, 0);
    assert!(!arm_under_gripping_robot(&r));
    assert!(!turtle::serialise(&profile(), &graph).contains("owl:equivalentClass"));
}
