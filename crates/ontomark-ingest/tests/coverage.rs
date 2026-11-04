//! Answerability grading over a small synthetic vault (ADR-OM-012).

use ontomark_ingest::{AliasTier, Ingest, Options};
use ontomark_model::{DeclarativeProfile, DocId, Iri, Model, ProfileSpec};
use ontomark_parse::Document;

fn profile() -> DeclarativeProfile {
    let spec: ProfileSpec = serde_json::from_value(serde_json::json!({
        "id": "coverage-test/1",
        "kinds": { "Class": "class" },
        "predicates": { "is-a": { "iri": "rdfs:subClassOf", "form": "sub_class_of" } }
    }))
    .unwrap();
    DeclarativeProfile::new(spec).unwrap()
}

fn vault() -> Vec<(DocId, Document)> {
    let page = |name: &str, iri: &str, aliases: &str| {
        (
            DocId::new(format!("{name}.md")),
            Document::parse(format!("---\ntype: Class\niri: {iri}\n{aliases}---\n")),
        )
    };
    vec![
        page("Revenue", "urn:x:Revenue", ""),
        page(
            "Invoice Line",
            "urn:x:InvoiceLine",
            "aliases:\n- line item\n",
        ),
        page(
            "Mercury (planet)",
            "urn:x:MercuryPlanet",
            "aliases:\n- Mercury\n",
        ),
        page(
            "Mercury (element)",
            "urn:x:MercuryElement",
            "aliases:\n- Mercury\n",
        ),
    ]
}

fn with_ingest<R>(f: impl FnOnce(&Ingest<'_>) -> R) -> R {
    let profile = profile();
    let docs = vault();
    let model = Model::build(&profile, docs.iter().map(|(id, d)| (id.clone(), d)));
    let ingest = Ingest::new(&profile, &model, Options::default());
    f(&ingest)
}

#[test]
fn every_name_lands_in_exactly_one_bucket() {
    let c = with_ingest(|i| i.coverage(&["revenue", "Line Item", "Mercury", "ad spend"]));
    assert_eq!(c.grounded.len(), 2);
    assert_eq!(c.grounded[0].iri, Iri::parse("urn:x:Revenue").unwrap());
    assert_eq!(c.grounded[0].tier, AliasTier::Exact);
    assert_eq!(c.grounded[1].label, "Invoice Line");
    assert_eq!(c.unclear.len(), 1);
    assert_eq!(c.unclear[0].name, "Mercury");
    assert_eq!(
        c.unclear[0].candidates,
        vec![
            Iri::parse("urn:x:MercuryElement").unwrap(),
            Iri::parse("urn:x:MercuryPlanet").unwrap()
        ]
    );
    assert_eq!(c.uncovered.len(), 1);
    assert_eq!(c.uncovered[0].name, "ad spend");
    assert_eq!(c.coverage, Some(0.5));
    assert!(!c.is_complete());
}

#[test]
fn no_names_is_not_zero_coverage() {
    let c = with_ingest(|i| i.coverage::<&str>(&[]));
    assert_eq!(c.coverage, None);
    assert!(!c.is_complete());
    let blank = with_ingest(|i| i.coverage(&["", "   "]));
    assert_eq!(blank.coverage, None);
}

#[test]
fn duplicates_by_normal_form_count_once() {
    let c = with_ingest(|i| i.coverage(&["Revenue", "revenue", "REVENUE "]));
    assert_eq!(c.grounded.len(), 1);
    assert_eq!(c.grounded[0].name, "Revenue");
    assert_eq!(c.coverage, Some(1.0));
    assert!(c.is_complete());
}

#[test]
fn coverage_serialises_with_a_null_grade_when_empty() {
    let c = with_ingest(|i| i.coverage::<&str>(&[]));
    let v = serde_json::to_value(&c).unwrap();
    assert!(v["coverage"].is_null());
    assert_eq!(v["grounded"], serde_json::json!([]));
}
