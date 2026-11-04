//! Certified keys are never patched (ADR-OM-012).

use ontomark_ingest::{
    Certification, Ingest, ModelResponse, Options, ProposedAssertion, WithholdReason,
};
use ontomark_model::{DeclarativeProfile, DocId, Model, ProfileSpec};
use ontomark_parse::Document;

fn profile() -> DeclarativeProfile {
    let spec: ProfileSpec = serde_json::from_value(serde_json::json!({
        "id": "certification-test/1",
        "kinds": { "Class": "class" },
        "predicates": {
            "is-a": { "iri": "rdfs:subClassOf", "form": "sub_class_of" },
            "note": { "iri": "urn:x:note", "form": "annotation" }
        },
        "reserved": ["verified"]
    }))
    .unwrap();
    DeclarativeProfile::new(spec).unwrap()
}

const BODY: &str = "A knowledge graph is a graph. Note: widely used.\n";

fn subject(verified: &str) -> Document {
    Document::parse(format!(
        "---\ntype: Class\niri: urn:x:KG\n{verified}---\n{BODY}"
    ))
}

/// Accept one `is-a` and one `note` proposal against a note with `verified`.
fn run(verified: &str) -> (Vec<String>, Vec<(String, String)>) {
    let profile = profile();
    let graph = Document::parse("---\ntype: Class\niri: urn:x:Graph\n---\n");
    let kg = subject(verified);
    let id = DocId::new("Knowledge graph.md");
    let model = Model::build(
        &profile,
        [(DocId::new("Graph.md"), &graph), (id.clone(), &kg)],
    );
    let ingest = Ingest::new(&profile, &model, Options::default());
    let request = ingest.plan(&id, &kg).unwrap();
    let response = ModelResponse::new(
        0,
        vec![
            ProposedAssertion::new("is-a", "graph", "is a graph"),
            ProposedAssertion::new("note", "widely used", "widely used"),
        ],
    );
    let out = ingest.accept(&kg, &request, &[response]).unwrap();
    let accepted = out.accepted.iter().map(|a| a.key.clone()).collect();
    let withheld = out
        .withheld
        .iter()
        .map(|w| (w.key.clone(), w.reason.category().to_owned()))
        .collect();
    (accepted, withheld)
}

#[test]
fn uncertified_note_accepts_both() {
    let (accepted, withheld) = run("");
    assert_eq!(accepted, ["is-a", "note"]);
    assert!(withheld.is_empty());
}

#[test]
fn keyed_human_stamp_blocks_only_its_keys() {
    let (accepted, withheld) =
        run("verified:\n  - by: human:npub1x\n    at: 2026-11-04T00:00:00Z\n    keys: [is-a]\n");
    assert_eq!(accepted, ["note"]);
    assert_eq!(withheld, [("is-a".to_owned(), "certified".to_owned())]);
}

#[test]
fn whole_page_human_stamp_blocks_every_key() {
    let (accepted, withheld) =
        run("verified:\n  - by: human:npub1x\n    at: 2026-11-04T00:00:00Z\n");
    assert!(accepted.is_empty());
    assert_eq!(withheld.len(), 2);
    assert!(withheld.iter().all(|(_, r)| r == "certified"));
}

#[test]
fn non_human_or_undated_stamps_certify_nothing() {
    for v in [
        "verified:\n  - by: agent:ontology-mesh/1\n    at: 2026-11-04T00:00:00Z\n",
        "verified:\n  - by: process:vault/1.0\n    at: 2026-11-04T00:00:00Z\n",
        "verified:\n  - by: did:nostr:jjohare\n    at: 2026-11-04T00:00:00Z\n",
        "verified:\n  - by: human:npub1x\n",
        "verified:\n  - by: 'human:'\n    at: 2026-11-04T00:00:00Z\n",
    ] {
        let (accepted, _) = run(v);
        assert_eq!(accepted, ["is-a", "note"], "{v}");
    }
}

#[test]
fn certification_is_read_from_the_note_not_the_request() {
    // The request was planned when the note was uncertified; certifying the
    // note changes its digest, so the old request is refused outright rather
    // than slipping a write past the stamp.
    let profile = profile();
    let graph = Document::parse("---\ntype: Class\niri: urn:x:Graph\n---\n");
    let before = subject("");
    let after = subject("verified:\n  - by: human:npub1x\n    at: 2026-11-04T00:00:00Z\n");
    let id = DocId::new("Knowledge graph.md");
    let model = Model::build(
        &profile,
        [(DocId::new("Graph.md"), &graph), (id.clone(), &before)],
    );
    let ingest = Ingest::new(&profile, &model, Options::default());
    let stale = ingest.plan(&id, &before).unwrap();
    assert!(ingest.accept(&after, &stale, &[]).is_err());
}

#[test]
fn certification_reports_its_keys() {
    let c = Certification::of(
        &subject("verified:\n  - by: human:npub1x\n    at: 2026-11-04T00:00:00Z\n    keys: [is-a, ' ', note]\n"),
        "verified",
    );
    assert!(!c.whole_page);
    assert_eq!(
        c.keys.iter().map(String::as_str).collect::<Vec<_>>(),
        ["is-a", "note"]
    );
    assert!(Certification::of(&subject(""), "verified").is_empty());
}

#[test]
fn withheld_reason_serialises_as_certified() {
    let v = serde_json::to_value(WithholdReason::Certified).unwrap();
    assert_eq!(v, serde_json::json!({ "reason": "certified" }));
}

/// A suggester that records the keys it was offered and proposes `is-a`.
struct Recording(std::cell::RefCell<Vec<String>>);

impl ontomark_ingest::FieldSuggester for Recording {
    fn suggest(
        &self,
        q: &ontomark_ingest::SuggestionQuestion,
    ) -> Result<Vec<ontomark_ingest::RawSuggestion>, ontomark_ingest::Unavailable> {
        self.0
            .borrow_mut()
            .extend(q.keys.iter().map(|k| k.key.clone()));
        Ok(vec![ontomark_ingest::RawSuggestion {
            key: "is-a".into(),
            target: ontomark_model::Iri::parse("urn:x:Graph").unwrap(),
            ambiguous: false,
        }])
    }
}

#[test]
fn suggester_is_never_offered_a_certified_key() {
    let profile = profile();
    let graph = Document::parse("---\ntype: Class\niri: urn:x:Graph\n---\n");
    let id = DocId::new("Knowledge graph.md");
    for (verified, offered) in [
        ("", vec!["is-a".to_owned()]),
        (
            "verified:\n  - by: human:npub1x\n    at: 2026-11-04T00:00:00Z\n    keys: [is-a]\n",
            vec![],
        ),
    ] {
        let kg = subject(verified);
        let model = Model::build(
            &profile,
            [(DocId::new("Graph.md"), &graph), (id.clone(), &kg)],
        );
        let ingest = Ingest::new(&profile, &model, Options::default());
        let rec = Recording(std::cell::RefCell::new(Vec::new()));
        let out = ingest.suggest_fields(&id, &kg, &rec).unwrap();
        assert_eq!(*rec.0.borrow(), offered, "{verified}");
        assert_eq!(out.proposals.len(), offered.len(), "{verified}");
    }
}
