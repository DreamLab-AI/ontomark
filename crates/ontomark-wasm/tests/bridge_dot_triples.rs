//! The Dot Triples bridge (`bridges/dot-triples/`) produces, from Dot Triples'
//! reading of five notes, the same ontology triples the engine asserts when it
//! reads those notes itself. This is the parity the bridge README claims.

use std::collections::BTreeSet;

use ontomark_wasm::engine::{Engine, FileText, Request};
use ontomark_wasm::profile::ProfileId;
use ontomark_wasm::query::{evaluate, parse_checked, DatasetSelection, QueryOutput, WireTerm};
use oxrdf::{Dataset, GraphName};
use oxttl::NTriplesParser;

const BRIDGE: &str = include_str!("../../../bridges/dot-triples/dot-triples-to-ontomark.rq");
const FIXTURE: &str = include_str!("../../../bridges/dot-triples/fixtures/zoo.nt");
const NOTES: &[(&str, &str)] = &[
    (
        "Animal.md",
        include_str!("../../../bridges/dot-triples/fixtures/zoo-vault/Animal.md"),
    ),
    (
        "Dog.md",
        include_str!("../../../bridges/dot-triples/fixtures/zoo-vault/Dog.md"),
    ),
    (
        "Rex.md",
        include_str!("../../../bridges/dot-triples/fixtures/zoo-vault/Rex.md"),
    ),
    (
        "Cat.md",
        include_str!("../../../bridges/dot-triples/fixtures/zoo-vault/Cat.md"),
    ),
    (
        "eats.md",
        include_str!("../../../bridges/dot-triples/fixtures/zoo-vault/eats.md"),
    ),
];

/// The predicates the bridge is responsible for; the engine asserts more
/// (`urn:ontomark:sourcePath`, annotation keys) that Dot Triples has no token for.
const BRIDGED: &[&str] = &[
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
    "http://www.w3.org/2000/01/rdf-schema#subClassOf",
    "http://www.w3.org/2000/01/rdf-schema#label",
    "http://www.w3.org/2004/02/skos/core#altLabel",
    "http://www.w3.org/2000/01/rdf-schema#domain",
    "http://www.w3.org/2000/01/rdf-schema#range",
];

fn text(t: &WireTerm) -> String {
    match t {
        WireTerm::Iri { value } => format!("<{value}>"),
        WireTerm::Bnode { value } => format!("_:{value}"),
        WireTerm::Literal {
            value,
            lang: Some(l),
            ..
        } => format!("\"{value}\"@{l}"),
        WireTerm::Literal {
            value, datatype, ..
        } if datatype == "http://www.w3.org/2001/XMLSchema#string" => format!("\"{value}\""),
        WireTerm::Literal {
            value, datatype, ..
        } => format!("\"{value}\"^^<{datatype}>"),
    }
}

fn line(s: &WireTerm, p: &WireTerm, o: &WireTerm) -> String {
    format!("{} {} {}", text(s), text(p), text(o))
}

/// The bridge's output over the fixture, as N-Triples-shaped lines.
fn bridged() -> BTreeSet<String> {
    let mut ds = Dataset::new();
    for t in NTriplesParser::new().for_slice(FIXTURE.as_bytes()) {
        let t = t.expect("fixture is valid N-Triples");
        ds.insert(&t.in_graph(GraphName::DefaultGraph));
    }
    let query = parse_checked(BRIDGE).expect("the bridge passes the engine's own query checks");
    match evaluate(&query, &ds, 10_000).expect("bridge evaluates") {
        QueryOutput::Construct { triples, truncated } => {
            assert!(!truncated);
            triples.iter().map(|[s, p, o]| line(s, p, o)).collect()
        }
        other => panic!("CONSTRUCT expected, got {other:?}"),
    }
}

/// What the engine asserts for the same notes, restricted to the bridged predicates.
fn engine_asserted() -> BTreeSet<String> {
    let mut engine = Engine::new();
    let files = NOTES
        .iter()
        .map(|(path, text)| FileText {
            path: (*path).to_owned(),
            text: (*text).to_owned(),
        })
        .collect();
    let loaded = engine.handle(Request::Load {
        generation: 1,
        profile: ProfileId::Generic,
        files,
    });
    assert!(loaded.ok, "{:?}", loaded.error);
    let answer = engine.handle(Request::Query {
        generation: 1,
        sparql: "SELECT ?s ?p ?o WHERE { ?s ?p ?o }".into(),
        dataset: DatasetSelection::Asserted,
        limit: Some(10_000),
    });
    assert!(answer.ok, "{:?}", answer.error);
    let out: QueryOutput =
        serde_json::from_value(answer.result.expect("ok carries a result")).expect("query output");
    let QueryOutput::Select { rows, .. } = out else {
        panic!("SELECT expected")
    };
    rows.iter()
        .filter(|r| matches!(&r["p"], WireTerm::Iri { value } if BRIDGED.contains(&value.as_str())))
        .map(|r| line(&r["s"], &r["p"], &r["o"]))
        .collect()
}

#[test]
fn bridge_output_equals_the_engines_own_reading_of_the_notes() {
    let bridge = bridged();
    let engine = engine_asserted();
    let only_bridge: Vec<_> = bridge.difference(&engine).collect();
    let only_engine: Vec<_> = engine.difference(&bridge).collect();
    assert!(
        only_bridge.is_empty() && only_engine.is_empty(),
        "bridge and engine disagree.\nonly the bridge says:\n  {}\nonly the engine says:\n  {}",
        only_bridge
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  "),
        only_engine
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  "),
    );
    assert!(
        bridge.len() >= 12,
        "{} triples is too few for five notes",
        bridge.len()
    );
}

#[test]
fn bridge_keeps_an_authored_iri_and_resolves_names_through_it() {
    let bridge = bridged();
    // Cat carries `iri:`; the subject is that IRI, and Dog's parent name resolves to Ontomark's derived page.
    assert!(bridge.contains("<https://example.org/zoo#Cat> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Class>"));
    assert!(bridge.contains("<https://example.org/zoo#Cat> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <urn:ontomark:page:Animal>"));
    assert!(bridge.contains("<https://example.org/zoo#Cat> <http://www.w3.org/2000/01/rdf-schema#label> \"Domestic cat\""));
    assert!(bridge.contains(
        "<https://example.org/zoo#Cat> <http://www.w3.org/2004/02/skos/core#altLabel> \"moggy\""
    ));
    assert!(bridge.contains("<urn:ontomark:page:Rex> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <urn:ontomark:page:Dog>"));
    assert!(bridge.contains("<urn:ontomark:page:eats> <http://www.w3.org/2000/01/rdf-schema#domain> <urn:ontomark:page:Animal>"));
    // No Dot Triples name leaks through as a subject or object.
    assert!(
        bridge.iter().all(|l| !l.contains("<urn:name:")),
        "{bridge:#?}"
    );
}
