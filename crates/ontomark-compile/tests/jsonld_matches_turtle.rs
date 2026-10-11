//! The JSON-LD serialisation says what the Turtle says.
//!
//! Both documents are parsed by independent processors from the oxigraph
//! project (`oxttl`, `oxjsonld`), canonicalised with RDFC-1.0 (`oxrdf`), and
//! compared: equal canonical datasets mean the two graphs are isomorphic under
//! blank-node relabelling. No code of this crate is used to read either side.

use std::path::{Path, PathBuf};

use ontomark_compile::build::{okf, turtle};
use ontomark_compile::input::page::{Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::{model::Corpus, projection};
use ontomark_compile::jsonld::to_jsonld_string;
use ontomark_compile::profile::Profile;
use ontomark_compile::shacl::{frontmatter_graph, shapes};
use oxjsonld::JsonLdParser;
use oxrdf::dataset::{CanonicalizationAlgorithm, CanonicalizationHashAlgorithm};
use oxrdf::Dataset;
use oxttl::TurtleParser;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/estate-small")
}

fn invalid_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/estate-invalid")
}

fn estate() -> Profile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../profiles/estate/profile.yaml");
    Profile::from_yaml(&std::fs::read_to_string(path).expect("profile")).expect("estate profile")
}

fn canonical(mut dataset: Dataset) -> Dataset {
    dataset.canonicalize(CanonicalizationAlgorithm::Rdfc10 {
        hash_algorithm: CanonicalizationHashAlgorithm::Sha256,
    });
    dataset
}

fn from_turtle(text: &str) -> Dataset {
    let mut dataset = Dataset::new();
    for triple in TurtleParser::new().for_slice(text) {
        dataset.insert(
            oxrdf::TripleRef::from(&triple.expect("Turtle parses"))
                .in_graph(oxrdf::GraphNameRef::DefaultGraph),
        );
    }
    canonical(dataset)
}

fn from_jsonld(text: &str) -> Dataset {
    let mut dataset = Dataset::new();
    for quad in JsonLdParser::new().for_slice(text) {
        dataset.insert(&quad.expect("JSON-LD parses"));
    }
    canonical(dataset)
}

fn assert_equivalent(name: &str, graph: &turtle::Graph, profile: &Profile, vocab: &Vocabulary) {
    let ttl = turtle::serialise(profile, graph);
    let json = to_jsonld_string(graph, &okf::context(profile, vocab));
    let (a, b) = (from_turtle(&ttl), from_jsonld(&json));
    assert!(!a.is_empty(), "{name}: empty graph proves nothing");
    assert_eq!(a.len(), b.len(), "{name}: triple counts differ");
    assert!(a == b, "{name}: Turtle and JSON-LD are not isomorphic");
}

fn load() -> (Profile, Vocabulary) {
    let vocab = Vocabulary::load(repo().join("ontology/vocabulary.yaml")).expect("vocabulary");
    (estate(), vocab)
}

#[test]
fn the_asserted_ontology_graph_is_the_same_in_both_syntaxes() {
    let (profile, vocab) = load();
    let vault = Vault::load(repo().join("knowledge"), VaultKind::Knowledge).expect("vault");
    let raw = Corpus::build(&vault, &vocab);
    let corpus = projection::project(&profile, &raw, &Default::default()).expect("projection");
    let roster = ontomark_compile::build::attribution::Roster::discover(&repo());
    let graph = turtle::build_graph_attributed(&profile, &corpus, &vocab, true, &roster);
    assert!(graph.len() > 1000, "a real fixture graph: {}", graph.len());
    assert_equivalent("ontology", &graph, &profile, &vocab);
}

#[test]
fn the_shapes_graph_is_the_same_in_both_syntaxes() {
    let (profile, vocab) = load();
    // The shapes graph is the one that holds rdf:first/rest chains (`sh:in`,
    // `sh:or`, `sh:ignoredProperties`), so it exercises `@list`.
    let graph = shapes(&profile, &vocab);
    assert!(
        to_jsonld_string(&graph, &serde_json::Value::Null).contains("@list"),
        "the shapes use lists"
    );
    assert_equivalent("shapes", &graph, &profile, &vocab);
}

#[test]
fn the_frontmatter_data_graphs_are_the_same_in_both_syntaxes() {
    let (profile, vocab) = load();
    for (name, kind) in [
        ("knowledge", VaultKind::Knowledge),
        ("working", VaultKind::Working),
    ] {
        let vault = Vault::load(repo().join(name), kind).expect("vault");
        let graph = frontmatter_graph(&vault, name, &profile, &vocab);
        assert_equivalent(&format!("{name} data"), &graph, &profile, &vocab);
    }
}

#[test]
fn the_mutation_fixture_data_graphs_are_the_same_in_both_syntaxes() {
    let (profile, vocab) = load();
    for (name, kind) in [
        ("knowledge", VaultKind::Knowledge),
        ("working", VaultKind::Working),
    ] {
        let vault = Vault::load(invalid_repo().join(name), kind).expect("vault");
        let graph = frontmatter_graph(&vault, name, &profile, &vocab);
        assert_equivalent(&format!("invalid {name} data"), &graph, &profile, &vocab);
    }
}

#[test]
fn the_comparison_can_fail() {
    // A control: drop one node from the JSON-LD and the datasets must differ.
    let (profile, vocab) = load();
    let graph = shapes(&profile, &vocab);
    let ttl = turtle::serialise(&profile, &graph);
    let mut doc = ontomark_compile::jsonld::to_jsonld(&graph, &okf::context(&profile, &vocab));
    doc["@graph"].as_array_mut().expect("graph array").pop();
    assert!(from_turtle(&ttl) != from_jsonld(&doc.to_string()));
}

#[test]
fn the_output_is_deterministic() {
    let (profile, vocab) = load();
    let context = okf::context(&profile, &vocab);
    let once = to_jsonld_string(&shapes(&profile, &vocab), &context);
    let twice = to_jsonld_string(&shapes(&profile, &vocab), &context);
    assert_eq!(once, twice);
}
