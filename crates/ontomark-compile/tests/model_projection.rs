//! Pure projection contract: model identities, public views and shared reasoning.
use ontomark_compile::build::turtle::{Subject, Term};
use ontomark_compile::projection::{project_model, Scope};
use ontomark_model::{DeclarativeProfile, DocId, Model};
use ontomark_parse::Document;

fn project(pages: &[(&str, &str)], scope: Scope) -> ontomark_compile::projection::Projection {
    let docs: Vec<_> = pages
        .iter()
        .map(|(id, s)| (DocId::new(*id), Document::parse(*s)))
        .collect();
    let p = DeclarativeProfile::generic();
    let m = Model::build(&p, docs.iter().map(|(id, d)| (id.clone(), d)));
    project_model(&m, &p, scope)
}

fn triples(p: &ontomark_compile::projection::Projection) -> String {
    p.graph
        .iter()
        .map(|(s, p, o)| format!("{s:?} {p} {o:?}\n"))
        .collect()
}

#[test]
fn a_skos_only_class_is_declared_as_a_class_without_minting_a_missing_target() {
    let p = project(
        &[(
            "A",
            "---\ntype: Class\niri: urn:A\nskos:broader: ['[[Missing]]']\n---\n",
        )],
        Scope::Authoring,
    );
    assert!(p
        .graph
        .iter()
        .any(|(s, p, o)| s == &Subject::Iri("urn:A".into())
            && p.ends_with("#type")
            && o == &Term::Iri("http://www.w3.org/2002/07/owl#Class".into())));
    assert!(!triples(&p).contains("Missing"));
    assert!(p.classify().classification.subsumptions.is_empty());
}

#[test]
fn undeclared_identity_and_duplicate_claimants_never_gain_phantom_classes() {
    let p = project(
        &[
            ("No IRI", "---\ntype: Class\n---\n"),
            ("A", "---\ntype: Class\niri: urn:Clash\n---\n"),
            ("B", "---\ntype: Class\niri: urn:Clash\n---\n"),
        ],
        Scope::Authoring,
    );
    assert_eq!(p.graph.len(), 0);
    assert!(p.axioms.is_empty());
}

#[test]
fn public_projection_filters_private_targets_before_classification() {
    let pages = [
        (
            "Secret",
            "---\ntype: Class\niri: urn:SecretCanary\npublic: false\n---\n",
        ),
        (
            "Open",
            "---\ntype: Class\niri: urn:Open\npublic: true\nrdfs:subClassOf: ['[[Secret]]']\n---\n",
        ),
    ];
    let public = project(&pages, Scope::Public);
    assert!(!triples(&public).contains("Secret"));
    assert!(!format!("{:?}", public.axioms).contains("Secret"));
    assert!(!format!("{:?}", public.classify()).contains("Secret"));
    let authoring = project(&pages, Scope::Authoring);
    assert!(triples(&authoring).contains("SecretCanary"));
}

#[test]
fn shared_classifier_derives_transitive_subsumption() {
    let p = project(
        &[
            (
                "A",
                "---\ntype: Class\niri: urn:A\nrdfs:subClassOf: ['[[B]]']\n---\n",
            ),
            (
                "B",
                "---\ntype: Class\niri: urn:B\nrdfs:subClassOf: ['[[C]]']\n---\n",
            ),
            ("C", "---\ntype: Class\niri: urn:C\n---\n"),
        ],
        Scope::Authoring,
    );
    let result = p.classify();
    assert!(result
        .classification
        .inferred
        .contains(&("urn:A".into(), "urn:C".into())));
    assert!(result.is_complete());
}

#[test]
fn data_assertions_are_preserved_without_claiming_reasoning_completeness() {
    let mut spec = DeclarativeProfile::generic().spec().clone();
    spec.predicates.insert(
        "age".into(),
        ontomark_model::PredicateSpec {
            iri: "urn:age".into(),
            form: ontomark_model::PredicateForm::DataAssertion,
            datatype: Some("xsd:integer".into()),
            domain: None,
            range: None,
            transitive: false,
        },
    );
    let profile = DeclarativeProfile::new(spec).unwrap();
    let doc = Document::parse("---\ntype: NamedIndividual\niri: urn:person\nage: 7\n---\n");
    let model = Model::build(&profile, [(DocId::new("person"), &doc)]);
    let p = project_model(&model, &profile, Scope::Authoring);
    assert!(p.graph.iter().any(|(_, pred, o)| pred == "urn:age"
        && o == &Term::typed("7", "http://www.w3.org/2001/XMLSchema#integer")));
    assert_eq!(p.omissions.logical_statements_not_reasoned, 1);
    assert!(!p.classify().is_complete());
}

#[test]
fn private_aliases_headings_and_literal_iris_are_withheld_without_diagnostic_leaks() {
    let mut spec = DeclarativeProfile::generic().spec().clone();
    spec.predicates.insert(
        "note".into(),
        ontomark_model::PredicateSpec {
            iri: "urn:description".into(),
            form: ontomark_model::PredicateForm::Annotation,
            datatype: None,
            domain: None,
            range: None,
            transitive: false,
        },
    );
    let profile = DeclarativeProfile::new(spec).unwrap();
    let hidden=Document::parse("---\ntype: Class\niri: urn:secret-canary\npublic: false\naliases: [HiddenAliasCanary]\n---\n");
    let public=Document::parse("---\ntype: Class\niri: urn:public\npublic: true\nnote: ['See [[HiddenAliasCanary#SensitiveHeading|caption]] and urn:secret-canary', 'See hiddenaliascanary again']\n---\n");
    let model = Model::build(
        &profile,
        [
            (DocId::new("restricted"), &hidden),
            (DocId::new("public"), &public),
        ],
    );
    let authoring = project_model(&model, &profile, Scope::Authoring);
    assert!(triples(&authoring).contains("SensitiveHeading"));
    let p = project_model(&model, &profile, Scope::Public);
    for canary in [
        "secret-canary",
        "HiddenAliasCanary",
        "SensitiveHeading",
        "hiddenaliascanary",
    ] {
        assert!(!format!("{p:?}").contains(canary), "{canary}");
        assert!(!format!("{:?}", p.classify()).contains(canary), "{canary}");
    }
    assert!(p.omissions.withheld_values > 0);
}

#[test]
fn public_scope_withholds_unpublished_and_unknown_lifecycle_states() {
    let mut spec = DeclarativeProfile::generic().spec().clone();
    spec.lifecycle = vec![
        ontomark_model::LifecycleDef {
            id: "stable".into(),
            published: true,
            reasoned: true,
        },
        ontomark_model::LifecycleDef {
            id: "working".into(),
            published: false,
            reasoned: false,
        },
    ];
    let profile = DeclarativeProfile::new(spec).unwrap();
    let docs = [
        (
            "Working",
            "---\ntype: Class\niri: urn:working-canary\npublic: true\nstatus: working\n---\n",
        ),
        (
            "Unknown",
            "---\ntype: Class\niri: urn:unknown-canary\npublic: true\nstatus: unrecognised\n---\n",
        ),
    ]
    .map(|(id, s)| (DocId::new(id), Document::parse(s)));
    let model = Model::build(&profile, docs.iter().map(|(id, d)| (id.clone(), d)));
    let p = project_model(&model, &profile, Scope::Public);
    assert_eq!(p.graph.len(), 0);
    assert_eq!(p.omissions.withheld_entities, 2);
    assert!(!project_model(&model, &profile, Scope::Authoring)
        .graph
        .is_empty());
}

#[test]
fn object_properties_restrictions_and_individual_types_keep_their_distinct_shapes() {
    let mut spec = DeclarativeProfile::generic().spec().clone();
    spec.predicates.insert(
        "part".into(),
        ontomark_model::PredicateSpec {
            iri: "urn:part".into(),
            form: ontomark_model::PredicateForm::Existential,
            datatype: None,
            domain: None,
            range: None,
            transitive: true,
        },
    );
    let profile = DeclarativeProfile::new(spec).unwrap();
    let docs = [
        ("A", "---\ntype: Class\niri: urn:A\npart: ['[[B]]']\n---\n"),
        ("B", "---\ntype: Class\niri: urn:B\n---\n"),
        (
            "i",
            "---\ntype: NamedIndividual\niri: urn:i\nrdf:type: ['[[A]]']\n---\n",
        ),
        ("part", "---\ntype: ObjectProperty\niri: urn:part\n---\n"),
    ]
    .map(|(id, s)| (DocId::new(id), Document::parse(s)));
    let model = Model::build(&profile, docs.iter().map(|(id, d)| (id.clone(), d)));
    let p = project_model(&model, &profile, Scope::Authoring);
    assert!(p
        .graph
        .iter()
        .any(|(s, p, o)| s == &Subject::Iri("urn:i".into())
            && p.ends_with("#type")
            && o == &Term::Iri("urn:A".into())));
    assert!(p
        .graph
        .iter()
        .any(|(_, p, _)| p.ends_with("#someValuesFrom")));
    assert!(p
        .graph
        .iter()
        .any(|(s, p, o)| s == &Subject::Iri("urn:part".into())
            && p.ends_with("#type")
            && o == &Term::Iri("http://www.w3.org/2002/07/owl#ObjectProperty".into())));
    assert!(!p.classify().is_complete());
    assert_eq!(
        p.classify().classification.refusals[0].construct,
        "ClassAssertion"
    );
    assert!(matches!(
        p.classify_with(ontomark_reason::Options {
            max_steps: Some(0),
            ..Default::default()
        }),
        Err(ontomark_reason::Stopped::OverBudget { .. })
    ));
}

#[test]
fn rejected_and_unresolved_assertions_do_not_become_an_unsupported_success() {
    let p = project(
        &[(
            "A",
            "---\ntype: Class\niri: urn:A\nrdfs:subClassOf: ['[[Missing]]']\n---\n",
        )],
        Scope::Authoring,
    );
    assert!(p
        .axioms
        .iter()
        .all(|a| !format!("{a:?}").contains("Missing")));
    assert!(!p.classify().is_complete());
    assert_eq!(p.omissions.rejected_statements, 1);
}

#[test]
fn public_literals_withhold_private_unicode_case_variants_and_keep_controls() {
    let mut spec = DeclarativeProfile::generic().spec().clone();
    spec.predicates.insert(
        "note".into(),
        ontomark_model::PredicateSpec {
            iri: "urn:description".into(),
            form: ontomark_model::PredicateForm::Annotation,
            datatype: None,
            domain: None,
            range: None,
            transitive: false,
        },
    );
    let profile = DeclarativeProfile::new(spec).unwrap();
    let hidden = Document::parse(
        "---\ntype: Class\niri: urn:hidden\npublic: false\naliases: [StraßeCanary, ΟΣCanary]\n---\n",
    );
    let visible = Document::parse(
        "---\ntype: Class\niri: urn:visible\npublic: true\nnote: ['Meet STRASSECanary', 'Meet οςcanary', 'Public control remains']\n---\n",
    );
    let model = Model::build(
        &profile,
        [
            (DocId::new("restricted"), &hidden),
            (DocId::new("visible"), &visible),
        ],
    );
    let authoring = triples(&project_model(&model, &profile, Scope::Authoring));
    assert!(authoring.contains("STRASSECanary"));
    assert!(authoring.contains("οςcanary"));
    let public = project_model(&model, &profile, Scope::Public);
    let text = triples(&public);
    assert!(
        !text.contains("STRASSECanary"),
        "private sharp-s case variant escaped"
    );
    assert!(
        !text.contains("οςcanary"),
        "private final-sigma case variant escaped"
    );
    assert!(text.contains("Public control remains"));
    assert_eq!(public.omissions.withheld_values, 2);
}
