//! Acceptance tests for ADR-2015: the four reference states and the
//! baseline defects (planning/ontology-workspace README, "Baseline ontology
//! review"): 1, 2, 4, 5, 7 in full; the model half of 3 (Turtle is WP-C) and
//! of 6 (the reasoner is WP-B). Defect 8 is design-record drift, not code. Every fixture is synthesised inline.

use ontomark_model::{
    codes, CyclePolicy, DeclarativeProfile, Disposition, DocDisposition, DocId, Iri, Kind,
    Lifecycle, Model, Object, Profile, ProfileSpec, PropertyKind, Resolution, Severity,
    StatementForm,
};
use ontomark_parse::Document;

/// An estate-shaped profile, written as data: the rules are injected.
fn estate_like(cycles: &str) -> DeclarativeProfile {
    let json = format!(
        r#"{{
        "id": "test-estate/1",
        "prefixes": {{ "vc": "https://example.org/vc#", "ngm": "urn:ngm:class:" }},
        "kinds": {{ "Class": "class", "Individual": "individual", "ObjectProperty": "object_property",
                    "DatatypeProperty": "data_property", "AnnotationProperty": "annotation_property" }},
        "identity_key": "resource",
        "lenient_booleans": true,
        "lifecycle": [ {{ "id": "stable" }}, {{ "id": "draft", "reasoned": false }} ],
        "predicates": {{
            "is-a": {{ "iri": "rdfs:subClassOf", "form": "sub_class_of" }},
            "instance-of": {{ "iri": "rdf:type", "form": "class_assertion" }},
            "has-part": {{ "iri": "https://narrativegoldmine.com/ns/v1#hasPart", "form": "existential", "domain": "ngm:system" }},
            "related-to": {{ "iri": "vc:relatedTo", "form": "annotation" }},
            "broader": {{ "iri": "skos:broader", "form": {{ "skos": "broader" }} }},
            "measured-by": {{ "iri": "https://example.org/vocab#measuredBy", "form": "object_assertion" }},
            "mass-kg": {{ "iri": "vc:massKg", "form": "data_assertion", "datatype": "xsd:decimal" }}
        }},
        "reserved": ["generated", "links", "quality", "domain", "maturity"],
        "reference_namespaces": ["urn:ngm:class:"],
        "publication": {{ "namespace": "urn:ngm:class:", "base_url": "https://narrativegoldmine.com/", "suffix": ".html" }},
        "cycle_policy": "{cycles}"
    }}"#
    );
    let spec: ProfileSpec = serde_json::from_str(&json).expect("spec parses");
    DeclarativeProfile::new(spec).expect("spec compiles")
}

struct Vault(Vec<(DocId, Document)>);

impl Vault {
    fn new(pages: &[(&str, &str)]) -> Self {
        Self(
            pages
                .iter()
                .map(|(p, s)| (DocId::new(p), Document::parse(*s)))
                .collect(),
        )
    }
    fn model(&self, profile: &dyn Profile) -> Model {
        Model::build(profile, self.0.iter().map(|(i, d)| (i.clone(), d)))
    }
}

fn iri(s: &str) -> Iri {
    Iri::parse(s).unwrap()
}

fn page(ty: &str, res: &str, extra: &str) -> String {
    format!("---\ntype: {ty}\nresource: {res}\nstatus: stable\n{extra}---\nBody.\n")
}

// ---------- reference states ----------

#[test]
fn reference_states_resolved_ambiguous_unresolved_external() {
    let v = Vault::new(&[
        ("Animal.md", &page("Class", "urn:ngm:class:animal", "aliases:\n- Beast\n")),
        ("zoo/Mercury.md", &page("Class", "urn:ngm:class:mercury-planet", "title: Mercury\n")),
        ("chem/Mercury Element.md", &page("Class", "urn:ngm:class:mercury-element", "title: Mercury\n")),
        ("Plain note.md", "Just prose.\n"),
        ("Typed No Iri.md", "---\ntype: Class\n---\n"),
        (
            "Dog.md",
            &page(
                "Class",
                "urn:ngm:class:dog",
                "is-a:\n- '[[Animal]]'\n- '[[Beast]]'\n- '[[zoo/Mercury]]'\n- '[[Mercury]]'\n- '[[Nowhere]]'\n- '[[Plain note]]'\n- https://www.wikidata.org/wiki/Q144\n- ngm:animal\n- '[[animal]]'\n",
            ),
        ),
    ]);
    let p = estate_like("warn");
    let m = v.model(&p);
    let dog = &m.entities[&iri("urn:ngm:class:dog")];
    let states: Vec<&Resolution> = dog
        .statements
        .iter()
        .map(|s| s.object.resolution().unwrap())
        .collect();
    let animal = Resolution::Resolved(iri("urn:ngm:class:animal"));
    assert_eq!(states[0], &animal, "by note name");
    assert_eq!(states[1], &animal, "by alias");
    assert_eq!(
        states[2],
        &Resolution::Resolved(iri("urn:ngm:class:mercury-planet")),
        "by path"
    );
    assert_eq!(
        states[3],
        &Resolution::Ambiguous(vec![
            iri("urn:ngm:class:mercury-element"),
            iri("urn:ngm:class:mercury-planet")
        ]),
        "two titles"
    );
    assert_eq!(states[4], &Resolution::Unresolved, "no target synthesised");
    assert_eq!(
        states[5],
        &Resolution::Unresolved,
        "a note without identity is not an entity"
    );
    if let Object::Reference(r) = &dog.statements[5].object {
        assert_eq!(r.document, Some(DocId::new("Plain note")));
    }
    assert_eq!(
        states[6],
        &Resolution::External(iri("https://www.wikidata.org/wiki/Q144"))
    );
    assert_eq!(states[7], &animal, "CURIE of a declared IRI");
    assert_eq!(
        states[8], &animal,
        "profile namespace candidate (and case-folded name) agree"
    );
    assert!(m
        .diagnostics
        .iter()
        .any(|d| d.code == codes::REFERENCE_AMBIGUOUS));
    // Typed page without identity: no entity minted.
    assert_eq!(
        m.documents[&DocId::new("Typed No Iri")].disposition,
        DocDisposition::NoIdentity
    );
    assert_eq!(m.entities.len(), 4);
}

// ---------- defect 1: entity types ----------

#[test]
fn d1_property_pages_are_properties_and_unknown_types_are_refused() {
    let v = Vault::new(&[
        ("partOf.md", &page("ObjectProperty", "vc:partOf", "")),
        ("mass.md", &page("DatatypeProperty", "vc:mass", "")),
        ("note.md", &page("AnnotationProperty", "vc:note", "")),
        ("thing.md", &page("Property", "vc:thing", "")),
        ("ghost.md", &page("Concept", "vc:ghost", "")),
    ]);
    let m = v.model(&estate_like("warn"));
    assert_eq!(
        m.entities[&iri("https://example.org/vc#partOf")].kind,
        Kind::Property(PropertyKind::Object)
    );
    assert_eq!(
        m.entities[&iri("https://example.org/vc#mass")].kind,
        Kind::Property(PropertyKind::Data)
    );
    assert_eq!(
        m.entities[&iri("https://example.org/vc#note")].kind,
        Kind::Property(PropertyKind::Annotation)
    );
    for refused in ["thing", "ghost"] {
        assert!(
            matches!(
                m.documents[&DocId::new(refused)].disposition,
                DocDisposition::Refused(_)
            ),
            "{refused}"
        );
        assert!(
            !m.entities
                .values()
                .any(|e| e.document == DocId::new(refused)),
            "{refused} must not become a Class"
        );
    }
    assert_eq!(
        m.diagnostics
            .iter()
            .filter(|d| d.code == codes::KIND_REFUSED)
            .count(),
        2
    );
}

// ---------- defect 2: predicate coverage ----------

#[test]
fn d2_any_declared_predicate_survives_and_undeclared_keys_are_kept_not_dropped() {
    let v = Vault::new(&[
        ("Gauge.md", &page("Individual", "urn:x:gauge", "")),
        (
            "Probe.md",
            &page(
                "Individual",
                "urn:x:probe",
                "measured-by:\n- '[[Gauge]]'\nmass-kg: 12.50\nwibble: '[[Gauge]]'\n",
            ),
        ),
    ]);
    let m = v.model(&estate_like("warn"));
    let probe = &m.entities[&iri("urn:x:probe")];
    let measured = &probe.statements[0];
    assert_eq!(
        measured.predicate.as_str(),
        "https://example.org/vocab#measuredBy"
    );
    assert_eq!(measured.form, StatementForm::ObjectAssertion);
    assert_eq!(measured.disposition, Disposition::Supported);
    let mass = &probe.statements[1];
    assert_eq!(mass.predicate.as_str(), "https://example.org/vc#massKg");
    match &mass.object {
        Object::Literal(l) => {
            assert_eq!(l.lexical, "12.50", "lexical form kept");
            assert_eq!(
                l.datatype.as_str(),
                "http://www.w3.org/2001/XMLSchema#decimal"
            );
        }
        o => panic!("{o:?}"),
    }
    assert_eq!(
        probe.statements.len(),
        2,
        "no generic edge invented for `wibble`"
    );
    assert_eq!(probe.unmapped, vec!["wibble".to_owned()]);
    assert!(m
        .diagnostics
        .iter()
        .any(|d| d.code == codes::UNMAPPED_KEY && d.message.contains("wibble")));
}

// ---------- defect 3: IRI correctness ----------

#[test]
fn d3_full_iris_are_kept_verbatim_and_publication_is_explicit() {
    let ids = [
        "urn:isbn:0451450523",
        "https://example.org/onto#Widget",
        "did:nostr:npub1abc",
        "urn:ngm:class:widget-2",
    ];
    let pages: Vec<(String, String)> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (format!("p{i}.md"), page("Class", id, "")))
        .collect();
    let refs: Vec<(&str, &str)> = pages
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let m = Vault::new(&refs).model(&estate_like("warn"));
    for id in ids {
        let e = &m.entities[&iri(id)];
        assert_eq!(e.iri.as_str(), id, "identity is byte-identical");
        let expect = id
            .strip_prefix("urn:ngm:class:")
            .map(|t| format!("https://narrativegoldmine.com/{t}.html"));
        assert_eq!(
            e.publication_url, expect,
            "only the mapped namespace gets a URL"
        );
    }
    // An invalid identity is refused, not repaired.
    let m = Vault::new(&[("bad.md", &page("Class", "not an iri", ""))]).model(&estate_like("warn"));
    assert!(matches!(
        m.documents[&DocId::new("bad")].disposition,
        DocDisposition::Refused(_)
    ));
}

// ---------- defect 4: identity resolution ----------

#[test]
fn d4_equal_tails_in_different_namespaces_stay_distinct_and_duplicates_are_not_first_wins() {
    let v = Vault::new(&[
        (
            "a/Foo.md",
            &page("Class", "urn:ngm:class:foo", "title: Foo (estate)\n"),
        ),
        (
            "b/Foo.md",
            &page("Class", "https://ex.org/foo", "title: Foo (external)\n"),
        ),
        (
            "Bar.md",
            &page(
                "Class",
                "urn:ngm:class:bar",
                "is-a:\n- '[[Foo]]'\n- https://ex.org/foo\n- '[[foo]]'\n",
            ),
        ),
        ("c/One.md", &page("Class", "urn:ngm:class:dup", "")),
        ("d/Two.md", &page("Class", "urn:ngm:class:dup", "")),
    ]);
    let m = v.model(&estate_like("warn"));
    assert!(
        m.entities.contains_key(&iri("urn:ngm:class:foo"))
            && m.entities.contains_key(&iri("https://ex.org/foo"))
    );
    let bar = &m.entities[&iri("urn:ngm:class:bar")];
    let s: Vec<_> = bar
        .statements
        .iter()
        .map(|s| s.object.resolution().unwrap().clone())
        .collect();
    // `[[Foo]]` is the note name of both documents: ambiguous, not the first.
    assert_eq!(
        s[0],
        Resolution::Ambiguous(vec![iri("https://ex.org/foo"), iri("urn:ngm:class:foo")])
    );
    assert_eq!(
        s[1],
        Resolution::Resolved(iri("https://ex.org/foo")),
        "a full IRI picks exactly one"
    );
    // The profile's namespace convention adds urn:ngm:class:foo; the names add both.
    assert!(matches!(s[2], Resolution::Ambiguous(_)));
    // Duplicate claim: neither document wins.
    let dup = iri("urn:ngm:class:dup");
    assert!(!m.entities.contains_key(&dup));
    assert_eq!(
        m.conflicts[&dup],
        vec![DocId::new("c/One"), DocId::new("d/Two")]
    );
    for d in ["c/One", "d/Two"] {
        assert_eq!(
            m.documents[&DocId::new(d)].disposition,
            DocDisposition::IdentityConflict(dup.clone())
        );
    }
}

#[test]
fn d4_input_order_does_not_change_the_model() {
    let pages = [
        (
            "A.md",
            page("Class", "urn:ngm:class:a", "is-a:\n- '[[B]]'\n"),
        ),
        ("B.md", page("Class", "urn:ngm:class:b", "title: Shared\n")),
        (
            "C.md",
            page(
                "Class",
                "urn:ngm:class:c",
                "title: Shared\nis-a:\n- '[[Shared]]'\n",
            ),
        ),
    ];
    let p = estate_like("warn");
    let fwd: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (*a, b.as_str())).collect();
    let rev: Vec<(&str, &str)> = fwd.iter().rev().copied().collect();
    let (m1, m2) = (Vault::new(&fwd).model(&p), Vault::new(&rev).model(&p));
    for (k, e) in &m1.entities {
        assert_eq!(&m2.entities[k], e);
    }
}

// ---------- defect 5: meaning of edges ----------

#[test]
fn d5_each_assertion_has_exactly_one_form() {
    let v = Vault::new(&[
        ("Wheel.md", &page("Class", "urn:ngm:class:wheel", "")),
        (
            "Car.md",
            &page(
                "Class",
                "urn:ngm:class:car",
                "has-part:\n- '[[Wheel]]'\nrelated-to:\n- '[[Wheel]]'\nbroader:\n- '[[Wheel]]'\n",
            ),
        ),
        ("w1.md", &page("Individual", "urn:x:w1", "")),
        (
            "c1.md",
            &page("Individual", "urn:x:c1", "has-part:\n- '[[w1]]'\n"),
        ),
    ]);
    let m = v.model(&estate_like("warn"));
    let car = &m.entities[&iri("urn:ngm:class:car")];
    let forms: Vec<StatementForm> = car.statements.iter().map(|s| s.form).collect();
    assert_eq!(
        forms,
        vec![
            StatementForm::Existential,
            StatementForm::Annotation,
            StatementForm::Skos(ontomark_model::SkosRelation::Broader)
        ],
        "one statement per authored value, no duplicate direct triple"
    );
    assert_eq!(car.statements[0].disposition, Disposition::Supported);
    let c1 = &m.entities[&iri("urn:x:c1")];
    assert_eq!(
        c1.statements[0].form,
        StatementForm::ObjectAssertion,
        "same predicate on an individual is a fact"
    );
}

// ---------- defect 6: inference scope ----------

#[test]
fn d6_every_statement_declares_what_a_reasoner_may_do_with_it() {
    let v = Vault::new(&[
        ("Animal.md", &page("Class", "urn:ngm:class:animal", "")),
        ("rex.md", &page("Individual", "urn:x:rex", "is-a:\n- '[[Animal]]'\ninstance-of:\n- '[[Animal]]'\n")),
        (
            "Dog.md",
            &page("Class", "urn:ngm:class:dog", "is-a:\n- '[[Animal]]'\n- '[[rex]]'\n- '[[Missing]]'\n- https://ex.org/Mammal\nrelated-to:\n- '[[Animal]]'\n"),
        ),
    ]);
    let m = v.model(&estate_like("warn"));
    let disp = |iri_s: &str| -> Vec<Disposition> {
        m.entities[&iri(iri_s)]
            .statements
            .iter()
            .map(|s| s.disposition.clone())
            .collect()
    };
    let dog = disp("urn:ngm:class:dog");
    assert_eq!(dog[0], Disposition::Supported);
    assert!(
        matches!(dog[1], Disposition::Rejected(_)),
        "subClassOf an individual"
    );
    assert!(
        matches!(dog[2], Disposition::PreservedNotReasoned(_)),
        "unresolved"
    );
    assert!(
        matches!(dog[3], Disposition::PreservedNotReasoned(_)),
        "external"
    );
    assert!(
        matches!(dog[4], Disposition::PreservedNotReasoned(_)),
        "annotation"
    );
    let rex = disp("urn:x:rex");
    assert!(
        matches!(rex[0], Disposition::Rejected(_)),
        "is-a on an individual is not coerced to rdf:type"
    );
    assert_eq!(rex[1], Disposition::Supported);
}

// ---------- defect 7: policy versus logic ----------

#[test]
fn d7_cycles_are_policy_findings_with_profile_severity() {
    let pages = [
        (
            "A.md",
            page("Class", "urn:ngm:class:a", "is-a:\n- '[[B]]'\n"),
        ),
        (
            "B.md",
            page("Class", "urn:ngm:class:b", "is-a:\n- '[[A]]'\n"),
        ),
        (
            "S.md",
            page("Class", "urn:ngm:class:s", "is-a:\n- '[[S]]'\n"),
        ),
        (
            "T.md",
            page("Class", "urn:ngm:class:t", "is-a:\n- '[[A]]'\n"),
        ),
    ];
    let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (*a, b.as_str())).collect();
    let v = Vault::new(&refs);
    for (policy, sev) in [
        ("allow", None),
        ("warn", Some(Severity::Warning)),
        ("refuse", Some(Severity::Error)),
    ] {
        let m = v.model(&estate_like(policy));
        match sev {
            None => assert!(m.policy.is_empty()),
            Some(s) => {
                assert_eq!(m.policy.len(), 2, "{policy}: {:?}", m.policy);
                assert!(m
                    .policy
                    .iter()
                    .all(|f| f.severity == s && f.code == "SUBCLASS_CYCLE"));
                assert_eq!(
                    m.policy[0].members,
                    vec![iri("urn:ngm:class:a"), iri("urn:ngm:class:b")]
                );
            }
        }
        // The statements themselves stay supported: a cycle is not an inconsistency.
        assert!(m
            .entities
            .values()
            .flat_map(|e| &e.statements)
            .all(|s| s.disposition == Disposition::Supported));
        assert!(m.diagnostics.iter().all(|d| d.severity != Severity::Error));
    }
    assert_eq!(estate_like("refuse").cycle_policy(), CyclePolicy::Refuse);
}

#[test]
fn d7_domain_is_not_a_required_field_rule() {
    // `has-part` declares a domain; a subject outside it is not an error.
    let v = Vault::new(&[
        ("Wheel.md", &page("Class", "urn:ngm:class:wheel", "")),
        (
            "Cart.md",
            &page("Class", "urn:ngm:class:cart", "has-part:\n- '[[Wheel]]'\n"),
        ),
    ]);
    let m = v.model(&estate_like("warn"));
    assert!(m.diagnostics.is_empty(), "{:?}", m.diagnostics);
    assert_eq!(
        m.entities[&iri("urn:ngm:class:cart")].statements[0].disposition,
        Disposition::Supported
    );
}

// ---------- lifecycle is profile data (defect 8 is ADR status drift: not WP-A-owned) ----------

#[test]
fn lifecycle_states_are_profile_data() {
    let v = Vault::new(&[
        ("Animal.md", &page("Class", "urn:ngm:class:animal", "")),
        ("Draft.md", "---\ntype: Class\nresource: urn:ngm:class:draft\nstatus: draft\nis-a:\n- '[[Animal]]'\n---\n"),
        ("Odd.md", "---\ntype: Class\nresource: urn:ngm:class:odd\nstatus: ratified-ish\n---\n"),
    ]);
    let m = v.model(&estate_like("warn"));
    let draft = &m.entities[&iri("urn:ngm:class:draft")];
    assert_eq!(draft.lifecycle, Lifecycle::Declared("draft".into()));
    assert!(matches!(
        draft.statements[0].disposition,
        Disposition::PreservedNotReasoned(_)
    ));
    assert_eq!(
        m.entities[&iri("urn:ngm:class:odd")].lifecycle,
        Lifecycle::Unknown("ratified-ish".into())
    );
    assert!(m
        .diagnostics
        .iter()
        .any(|d| d.code == codes::UNKNOWN_LIFECYCLE));
}

// ---------- generic profile, prose links, recovered links ----------

#[test]
fn generic_profile_accepts_a_plain_vault_and_body_links_are_not_axioms() {
    let v = Vault::new(&[
        (
            "Only note.md",
            "# Hello\n\nSee [[Other]] and [[Only note]].\n",
        ),
        (
            "Typed.md",
            "---\ntype: Class\niri: https://ex.org/T\n---\nIt is a [[Only note]].\n",
        ),
    ]);
    let m = v.model(&DeclarativeProfile::generic());
    assert_eq!(
        m.documents[&DocId::new("Only note")].disposition,
        DocDisposition::NotOntology
    );
    assert!(m.entities[&iri("https://ex.org/T")].statements.is_empty());
    assert!(m.diagnostics.is_empty(), "{:?}", m.diagnostics);
}

#[test]
fn unquoted_wikilink_items_become_statements_with_a_warning() {
    let v = Vault::new(&[
        (
            "Presentation - Conclusion.md",
            &page("Class", "urn:ngm:class:pc", ""),
        ),
        (
            "Talk.md",
            &page(
                "Class",
                "urn:ngm:class:talk",
                "is-a:\n- [[Presentation - Conclusion|Conclusion]]\n",
            ),
        ),
    ]);
    let m = v.model(&estate_like("warn"));
    let talk = &m.entities[&iri("urn:ngm:class:talk")];
    assert_eq!(
        talk.statements[0].object.resolution(),
        Some(&Resolution::Resolved(iri("urn:ngm:class:pc")))
    );
    assert!(m
        .diagnostics
        .iter()
        .any(|d| d.code == codes::UNQUOTED_WIKILINK));
}

#[test]
fn locked_frontmatter_is_refused_with_a_diagnostic() {
    let m = Vault::new(&[("x.md", "---\na: &x 1\n---\n")]).model(&DeclarativeProfile::generic());
    assert!(matches!(
        m.documents[&DocId::new("x")].disposition,
        DocDisposition::Refused(_)
    ));
    assert!(m
        .diagnostics
        .iter()
        .any(|d| d.code == codes::FRONTMATTER_LOCKED));
}

#[test]
fn the_model_serialises() {
    let m =
        Vault::new(&[("A.md", &page("Class", "urn:ngm:class:a", ""))]).model(&estate_like("warn"));
    let json = serde_json::to_string(&m.entities).unwrap();
    assert!(json.contains("\"urn:ngm:class:a\""));
}
