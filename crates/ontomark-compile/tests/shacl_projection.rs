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
    // A quoted "true" is NOT a boolean: publication permission is never coerced
    // (S1.3, 2026-10-12), so it stays a plain literal exactly like "yes".
    assert!(matches!(one("C03 String Boolean", "public"),
        Term::Literal { datatype: None, value, .. } if value == "true"));
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

/// What `validate` says about a stamp's `by` under vocabulary 3: `true` when it
/// reports `INVALID_STAMP` as an error on `field` (`generated` or `verified`).
fn validate_rejects_by(by: &str, field: &str) -> bool {
    use ontomark_compile::check::validate::{validate_with, Severity, Siblings};
    use ontomark_compile::input::page::Page;
    use ontomark_compile::ir::model::Corpus;

    let stamp = format!("{{ by: '{by}', at: '2026-11-01T00:00:00Z' }}");
    let key = if field == "verified" {
        format!("verified:\n  - {stamp}\n")
    } else {
        format!("generated: {stamp}\n")
    };
    let text = format!(
        "---\ntype: Class\ntitle: A\nresource: urn:ngm:class:a\nstatus: draft\npublic: true\n{key}---\nBody.\n"
    );
    let page = Page::parse("/v/A.md", "A.md", "A", &text).unwrap();
    let vault = Vault {
        root: PathBuf::new(),
        kind: VaultKind::Knowledge,
        journals: Vec::new(),
        skipped: Vec::new(),
        pages: vec![page],
    };
    let mut v3 = vocab();
    v3.version = 3;
    let report = validate_with(
        &estate(),
        &vault,
        &Corpus::build(&vault, &v3),
        &v3,
        &Siblings::default(),
    );
    report
        .issues
        .iter()
        .any(|i| i.code == "INVALID_STAMP" && i.severity == Severity::Error)
}

/// Like with like (ruling 15): the version 3 `sh:pattern` on a stamp's `by` is
/// compared with the verdict of `validate` itself on a page carrying that `by`,
/// not with `Actor::from_str`, which still reads the version 2 spellings. The
/// two used to diverge (the pattern passed `human:` that `validate` rejects)
/// and this test compared the pattern with the wrong oracle, so it stayed green.
///
/// What the pattern cannot see is the roster's principal `kind`
/// (`VERIFIED_NOT_HUMAN`), so that rule is outside the comparison.
#[test]
fn under_version_3_the_stamp_patterns_reject_exactly_what_validate_rejects() {
    let profile = estate();
    let mut v3 = vocab();
    v3.version = 3;
    let patterns = |vocab: &Vocabulary| {
        let graph = shapes(&profile, vocab);
        graph
            .iter()
            .filter(|(_, p, _)| *p == "http://www.w3.org/ns/shacl#pattern")
            .map(|(_, _, o)| plain(o).to_owned())
            .filter(|p| p.contains("did:nostr") || p.contains("agent") || p.contains("human"))
            .collect::<BTreeSet<_>>()
    };
    let v3_patterns = patterns(&v3);
    // The stamp's `by` and the stable-needs-a-human `by` are both the principal grammar.
    assert_eq!(
        v3_patterns,
        BTreeSet::from(["^did:nostr:[0-9a-f]{64}$".to_owned()]),
        "{v3_patterns:?}"
    );
    let re = regex::Regex::new(v3_patterns.first().unwrap()).expect("pattern compiles");
    let hex = "b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";
    let samples = [
        format!("did:nostr:{hex}"),
        format!("did:nostr:{}", hex.to_uppercase()),
        format!("did:nostr:{}", &hex[..63]),
        format!("did:nostr:{hex}0"),
        "did:nostr:abc".to_owned(),
        "human:npub1kst9gqtldpgtzwzh6xwc4c8rlz83xetqpj4sxg0gzqwfayng9aaqlz0mmn".to_owned(),
        "human:npub1abc".to_owned(),
        "human:jjohare".to_owned(),
        "agent:claude/4".to_owned(),
        "process:vault-migrate/1.0".to_owned(),
        "robot:x".to_owned(),
        String::new(),
    ];
    for field in ["generated", "verified"] {
        for sample in &samples {
            assert_eq!(
                !re.is_match(sample),
                validate_rejects_by(sample, field),
                "v3 pattern vs validate on {field}.by = {sample:?}"
            );
        }
    }
    // Version 2 shapes are byte-for-byte what they were: the template alternatives, no did.
    let v2_re = regex::Regex::new(
        patterns(&vocab())
            .iter()
            .find(|p| p.contains("agent"))
            .expect("v2 actor pattern"),
    )
    .expect("pattern compiles");
    assert!(!v2_re.is_match(&format!("did:nostr:{hex}")));
    assert!(v2_re.is_match("human:npub1abc") && v2_re.is_match("agent:claude/4"));
}
