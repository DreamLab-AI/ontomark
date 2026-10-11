//! The SHACL shapes and the frontmatter data graph: structure, coverage and
//! agreement with the actor grammar. These tests need no SHACL engine; the
//! engine run (pyshacl) is `conformance/standards/shacl_check.py`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ontomark_compile::build::turtle::{self, Graph, Subject, Term};
use ontomark_compile::input::consistency::DISJOINT_NOT_SIBLINGS;
use ontomark_compile::input::definition::NON_EL_DEFINITION;
use ontomark_compile::input::okf::Actor;
use ontomark_compile::input::page::{Vault, VaultKind};
use ontomark_compile::input::vocabulary::{
    Vocabulary, DEFINITION_OVER_TRANSITIVE, NOT_AN_EXISTENTIAL,
};
use ontomark_compile::profile::Profile;
use ontomark_compile::shacl::{
    frontmatter_graph, key_iri, partial_rule_code_iri, profile_rule_iri, rule_code_iri, rule_codes,
    shape_iri, shapes, Coverage, RULES,
};

fn manifest(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn estate() -> Profile {
    Profile::from_yaml(
        &std::fs::read_to_string(manifest("../../profiles/estate/profile.yaml")).expect("profile"),
    )
    .expect("estate profile")
}

fn vocab() -> Vocabulary {
    Vocabulary::load(manifest(
        "../../conformance/fixtures/estate-small/ontology/vocabulary.yaml",
    ))
    .expect("vocabulary")
}

fn objects<'a>(graph: &'a Graph, subject: &str, predicate: &str) -> Vec<&'a Term> {
    graph
        .iter()
        .filter(|(s, p, _)| matches!(s, Subject::Iri(i) if i == subject) && *p == predicate)
        .map(|(_, _, o)| o)
        .collect()
}

fn plain(term: &Term) -> &str {
    match term {
        Term::Literal { value, .. } => value,
        other => panic!("not a literal: {other:?}"),
    }
}

#[test]
fn the_rule_table_covers_every_code_validate_can_emit() {
    let source = include_str!("../src/check/validate.rs");
    let production = source.split("#[cfg(test)]").next().expect("source");
    let literal = regex::Regex::new(r#""([A-Z][A-Z_]{4,})""#).expect("regex");
    let mut emitted: BTreeSet<String> = literal
        .captures_iter(production)
        .map(|c| c[1].to_owned())
        .collect();
    // Codes that reach `validate` through other modules' constants.
    for code in [
        NON_EL_DEFINITION,
        DEFINITION_OVER_TRANSITIVE,
        NOT_AN_EXISTENTIAL,
        DISJOINT_NOT_SIBLINGS,
    ] {
        emitted.insert(code.to_owned());
    }
    let listed: BTreeSet<String> = RULES.iter().map(|r| r.code.to_owned()).collect();
    assert_eq!(
        emitted, listed,
        "RULES and check/validate.rs disagree (left: validate, right: RULES)"
    );
    assert_eq!(listed.len(), RULES.len(), "a code is listed twice");
}

#[test]
fn the_module_documentation_table_matches_the_rule_list() {
    let docs = include_str!("../src/shacl.rs");
    for rule in RULES {
        let status = match rule.coverage {
            Coverage::Shacl => "shacl",
            Coverage::Procedural => "procedural",
        };
        let row = format!("//! | `{}` | {status} |", rule.code);
        assert!(docs.contains(&row), "docs table lacks `{row}`");
    }
}

#[test]
fn every_covered_rule_has_a_shape_and_every_annotation_is_a_covered_rule() {
    let (profile, vocab) = (estate(), vocab());
    let graph = shapes(&profile, &vocab);
    let full: BTreeSet<String> = rule_codes(&graph, &profile).into_values().collect();
    for rule in RULES {
        match rule.coverage {
            Coverage::Shacl => assert!(
                full.contains(rule.code),
                "{} is marked shacl but no shape carries it",
                rule.code
            ),
            Coverage::Procedural => assert!(
                !full.contains(rule.code),
                "{} is marked procedural but a shape claims all of it",
                rule.code
            ),
        }
    }
    let known: BTreeSet<&str> = RULES.iter().map(|r| r.code).collect();
    assert!(full.iter().all(|c| known.contains(c.as_str())));
    // The partial annotation names only procedural codes.
    let partial = partial_rule_code_iri(&profile);
    for (_, p, o) in graph.iter() {
        if p == partial {
            let code = plain(o);
            assert!(RULES
                .iter()
                .any(|r| r.code == code && r.coverage == Coverage::Procedural));
        }
    }
}

#[test]
fn every_shape_says_where_it_comes_from_and_how_serious_it_is() {
    let (profile, vocab) = (estate(), vocab());
    let graph = shapes(&profile, &vocab);
    let shape_types = [
        "http://www.w3.org/ns/shacl#PropertyShape",
        "http://www.w3.org/ns/shacl#NodeShape",
    ];
    let named: BTreeSet<String> = graph
        .iter()
        .filter(|(s, p, o)| {
            matches!(s, Subject::Iri(_))
                && *p == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
                && matches!(o, Term::Iri(t) if shape_types.contains(&t.as_str()))
        })
        .map(|(s, _, _)| match s {
            Subject::Iri(i) => i.clone(),
            Subject::Blank(_) => unreachable!(),
        })
        .collect();
    assert!(named.len() > 30, "{} shapes", named.len());
    let origin = [
        rule_code_iri(&profile),
        partial_rule_code_iri(&profile),
        profile_rule_iri(&profile),
    ];
    for shape in &named {
        assert!(
            !objects(&graph, shape, "http://www.w3.org/ns/shacl#severity").is_empty(),
            "{shape} has no severity"
        );
        assert_eq!(
            origin
                .iter()
                .filter(|p| !objects(&graph, shape, p).is_empty())
                .count(),
            1,
            "{shape} must name exactly one origin"
        );
        let targets = ["targetClass", "targetObjectsOf"].iter().any(|t| {
            !objects(&graph, shape, &format!("http://www.w3.org/ns/shacl#{t}")).is_empty()
        });
        assert!(targets, "{shape} selects nothing");
    }
}

#[test]
fn severities_follow_validate() {
    let (profile, vocab) = (estate(), vocab());
    let graph = shapes(&profile, &vocab);
    let severity = |name: &str| -> String {
        match objects(
            &graph,
            &shape_iri(&profile, name),
            "http://www.w3.org/ns/shacl#severity",
        )[..]
        {
            [Term::Iri(iri)] => iri.rsplit('#').next().expect("local name").to_owned(),
            _ => panic!("{name}: one severity expected"),
        }
    };
    assert_eq!(severity("InvalidStatus"), "Violation");
    assert_eq!(severity("InvalidBoolean-public"), "Violation");
    assert_eq!(severity("OutOfRange-quality"), "Warning");
    assert_eq!(severity("InvalidEnum-maturity"), "Warning");
    assert_eq!(severity("MultiParent"), "Info");
    assert_eq!(severity("DuplicateIri"), "Violation");
    // Rules only the profile asks for must not be violations.
    assert_eq!(severity("RequiredKey-knowledge-title"), "Warning");
    assert_eq!(severity("ResourceIri"), "Warning");
}

#[test]
fn the_actor_pattern_accepts_exactly_what_the_actor_grammar_accepts() {
    let (profile, vocab) = (estate(), vocab());
    let graph = shapes(&profile, &vocab);
    // Reach the pattern through the stamp's inner `by` property shape.
    let patterns: Vec<String> = graph
        .iter()
        .filter(|(_, p, _)| *p == "http://www.w3.org/ns/shacl#pattern")
        .map(|(_, _, o)| plain(o).to_owned())
        .filter(|p| p.contains("human"))
        .collect();
    let actor = patterns
        .iter()
        .find(|p| p.contains("agent"))
        .expect("actor pattern");
    let re = regex::Regex::new(actor).expect("pattern compiles");
    for sample in [
        "human:npub1abc",
        "human:",
        "process:vault/1.0",
        "process:vault-migrate/1.0",
        "process:a/b/c",
        "process:novers",
        "process:/1",
        "process:x/",
        "agent:claude/4",
        "agent:claude",
        "robot:fixture",
        "did:nostr:abc",
        "",
        "process",
    ] {
        assert_eq!(
            re.is_match(sample),
            sample.parse::<Actor>().is_ok(),
            "pattern {actor} vs Actor::from_str on {sample:?}"
        );
    }
}

fn load(repo: &str, name: &str, kind: VaultKind) -> (Vault, Graph) {
    let vault = Vault::load(manifest(repo).join(name), kind).expect("vault");
    let graph = frontmatter_graph(&vault, name, &estate(), &vocab());
    (vault, graph)
}

#[test]
fn the_data_graph_has_one_node_per_page_with_its_id() {
    let (vault, graph) = load(
        "../../conformance/fixtures/estate-small",
        "knowledge",
        VaultKind::Knowledge,
    );
    let profile = estate();
    let ids: BTreeSet<String> = graph
        .iter()
        .filter(|(_, p, _)| *p == format!("{}meta-id", profile.iris.vocabulary_namespace))
        .map(|(_, _, o)| plain(o).to_owned())
        .collect();
    let expected: BTreeSet<String> = vault.pages.iter().map(|p| p.id.clone()).collect();
    assert_eq!(ids, expected);
    // "51% Attack" and names with spaces survive as legal IRIs.
    let node = format!(
        "{}page/{}",
        profile.iris.vocabulary_namespace, "51%25%20Attack"
    );
    assert!(!objects(&graph, &node, &key_iri(&profile, "status")).is_empty());
}

#[test]
fn the_data_graph_reads_values_the_way_validate_does() {
    let (_, graph) = load(
        "../../conformance/fixtures/estate-invalid",
        "knowledge",
        VaultKind::Knowledge,
    );
    let profile = estate();
    let node = |id: &str| {
        format!(
            "{}page/{}",
            profile.iris.vocabulary_namespace,
            ontomark_compile::shacl::encode_component(id)
        )
    };
    let one = |id: &str, key: &str| -> Term {
        let found = objects(&graph, &node(id), &key_iri(&profile, key));
        assert_eq!(found.len(), 1, "{id}.{key}: {found:?}");
        found[0].clone()
    };
    // A string "true" is a boolean to validate; "yes" is not.
    assert!(matches!(one("C03 String Boolean", "public"),
        Term::Literal { datatype: Some(d), value, .. } if d.ends_with("#boolean") && value == "true"));
    assert!(matches!(one("M02 Public Yes", "public"),
        Term::Literal { datatype: None, value, .. } if value == "yes"));
    // A status is trimmed and lower-cased when it is a known state.
    assert_eq!(
        plain(&one("C02 Status Case And Human Verified", "status")),
        "stable"
    );
    assert_eq!(plain(&one("M03 Unknown Status", "status")), "half-baked");
    // An unparsable number stays a string; a parsable one is a double.
    assert!(matches!(
        one("M10 Quality Not A Number", "quality"),
        Term::Literal { datatype: None, .. }
    ));
    assert!(matches!(one("C01 Clean", "quality"),
        Term::Literal { datatype: Some(d), .. } if d.ends_with("#double")));
    // A malformed actor survives into the data so the pattern can reject it.
    let stamp = one("M04 Malformed Generated By", "generated");
    let Term::Blank(label) = stamp else {
        panic!("generated is a node")
    };
    let by: Vec<_> = graph
        .iter()
        .filter(|(s, p, _)| {
            matches!(s, Subject::Blank(b) if *b == label) && *p == key_iri(&profile, "by")
        })
        .collect();
    assert_eq!(by.len(), 1);
    assert_eq!(plain(by[0].2), "robot:fixture");
    // The dangling link points at an untyped node.
    let target = one("M05 Dangling Relation", "enables");
    let Term::Iri(iri) = target else {
        panic!("link target is an IRI")
    };
    assert!(
        iri.contains("unresolved/No%20Such%20Page%20Anywhere"),
        "{iri}"
    );
    // A missing resource keeps a minted identity but no authored key.
    assert!(objects(
        &graph,
        &node("M01 Missing Resource"),
        &key_iri(&profile, "resource")
    )
    .is_empty());
    assert_eq!(
        objects(
            &graph,
            &node("M01 Missing Resource"),
            &format!("{}meta-identity", profile.iris.vocabulary_namespace)
        )
        .len(),
        1
    );
}

#[test]
fn shapes_and_data_serialise_as_turtle() {
    let (profile, vocab) = (estate(), vocab());
    for text in [
        turtle::serialise(&profile, &shapes(&profile, &vocab)),
        turtle::serialise(
            &profile,
            &load(
                "../../conformance/fixtures/estate-small",
                "working",
                VaultKind::Working,
            )
            .1,
        ),
    ] {
        let mut triples = 0;
        for triple in oxttl::TurtleParser::new().for_slice(&text) {
            triple.expect("legal Turtle");
            triples += 1;
        }
        assert!(triples > 20);
    }
}

#[test]
fn a_generic_profile_yields_shapes_too() {
    let profile = Profile::generic();
    let vocab = Vocabulary::from_yaml_str("version: 1\nnamespace: \"urn:ngm:class:\"\n")
        .expect("vocabulary");
    let graph = shapes(&profile, &vocab);
    assert!(!graph.is_empty());
}
