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
    run_with(verified, Options::default())
}

fn run_with(verified: &str, options: Options) -> (Vec<String>, Vec<(String, String)>) {
    let profile = profile();
    let graph = Document::parse("---\ntype: Class\niri: urn:x:Graph\n---\n");
    let kg = subject(verified);
    let id = DocId::new("Knowledge graph.md");
    let model = Model::build(
        &profile,
        [(DocId::new("Graph.md"), &graph), (id.clone(), &kg)],
    );
    let ingest = Ingest::new(&profile, &model, options);
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

#[test]
fn bound_reviews_report_freshness_without_unlocking_ingest() {
    use ontomark_parse::{certification::content_digest, Document};
    let original = Document::parse("---\ntitle: A\nvalue: old\n---\nbody");
    let keys = vec!["value".to_owned()];
    let digest = content_digest(&original, "verified", Some(&keys)).unwrap();
    let note = |value: &str| {
        Document::parse(format!("---\ntitle: A\nvalue: {value}\nverified:\n  - by: human:npub1test\n    at: 2026-11-15\n    keys: [value]\n    content_digest: {digest}\n---\nbody"))
    };
    let current = ontomark_ingest::Certification::of(&note("old"), "verified");
    assert_eq!(current.current, 1);
    assert_eq!(current.stale, 0);
    let stale = ontomark_ingest::Certification::of(&note("new"), "verified");
    assert_eq!(stale.current, 0);
    assert_eq!(stale.stale, 1);
    assert!(stale.covers("value"));
}

/// Item 1: under vocabulary 3 the old `human:` spelling does not certify, and
/// only the roster's keys do; the default (vocabulary 2) keeps the prefix rule.
#[test]
fn the_old_human_spelling_certifies_only_when_the_host_allows_it() {
    let by_human = subject(
        "verified:\n  - by: human:npub1x\n    at: 2026-11-04T00:00:00Z\n    keys: [is-a]\n",
    );
    let by_did = subject(
        "verified:\n  - by: did:nostr:ab\n    at: 2026-11-04T00:00:00Z\n    keys: [is-a]\n",
    );
    let humans = ["did:nostr:ab".to_owned()];
    assert!(Certification::of_under(&by_human, "verified", &humans, true).covers("is-a"));
    assert!(!Certification::of_under(&by_human, "verified", &humans, false).covers("is-a"));
    assert!(Certification::of_under(&by_did, "verified", &humans, false).covers("is-a"));
    assert!(!Certification::of_under(&by_did, "verified", &[], false).covers("is-a"));
    assert!(Options::default().legacy_human_prefix);
    // End to end through `accept`: with the prefix off the whole-page `human:`
    // stamp protects nothing; with a roster key it protects everything.
    let whole = "verified:\n  - by: human:npub1x\n    at: 2026-11-04T00:00:00Z\n";
    let mut off = Options::default();
    off.legacy_human_prefix = false;
    let (accepted, withheld) = run_with(whole, off.clone());
    assert_eq!(accepted.len(), 2, "{withheld:?}");
    assert!(withheld.is_empty());
    let (accepted, withheld) = run(whole);
    assert!(accepted.is_empty(), "the default keeps the prefix rule");
    assert_eq!(withheld.len(), 2);
    let did_whole = "verified:\n  - by: did:nostr:ab\n    at: 2026-11-04T00:00:00Z\n";
    let mut with_roster = off;
    with_roster.human_dids = vec!["did:nostr:ab".to_owned()];
    let (accepted, withheld) = run_with(did_whole, with_roster);
    assert!(accepted.is_empty());
    assert_eq!(withheld.len(), 2);
}
