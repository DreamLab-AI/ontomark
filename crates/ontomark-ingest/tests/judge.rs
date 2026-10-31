//! The Jev-option hooks: fallback equivalence, narrowing, and validation.

use std::cell::Cell;

use ontomark_ingest::{
    CandidateSelector, FactClaim, FactJudge, FieldSuggester, HostCandidate, Ingest, JudgeOutcome,
    ModelResponse, NoJudge, NoSelector, NoSuggester, Options, ProposedAssertion, RawSuggestion,
    Selection, SelectionQuestion, SuggestionQuestion, Unavailable, Verdict, WithholdReason,
};
use ontomark_model::{DeclarativeProfile, DocId, Iri, Model, ProfileSpec};
use ontomark_parse::Document;

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
const NOTE: &str = "Knowledge graph.md";

fn profile() -> DeclarativeProfile {
    let spec: ProfileSpec = serde_json::from_str(&format!(
        r##"{{
        "id": "judge-test/1",
        "kinds": {{ "Class": "class", "NamedIndividual": "individual" }},
        "predicates": {{
            "is-a": {{ "iri": "http://www.w3.org/2000/01/rdf-schema#subClassOf", "form": "sub_class_of" }},
            "part-of": {{ "iri": "urn:x:partOf", "form": "existential" }},
            "founded": {{ "iri": "urn:x:founded", "form": "data_assertion", "datatype": "{XSD}date" }}
        }},
        "reserved": ["tags", "public"]
    }}"##
    ))
    .unwrap();
    DeclarativeProfile::new(spec).unwrap()
}

fn docs() -> Vec<(DocId, Document)> {
    let page = |name: &str, front: &str, body: &str| {
        (
            DocId::new(format!("{name}.md")),
            Document::parse(format!("---\n{front}\n---\n{body}")),
        )
    };
    vec![
        page("Graph", "type: Class\niri: urn:x:Graph", ""),
        page("Thing", "type: Class\niri: urn:x:Thing", ""),
        page("Network", "type: Class\niri: urn:x:Network", ""),
        page(
            "Knowledge graph",
            "type: Class\niri: urn:x:KG\npublic: false",
            "A knowledge graph is a directed graph. It is not a Thing in any sense. \
             Founded on 2019-03-01. Sometimes drawn as a Network.\n",
        ),
    ]
}

fn setup() -> (DeclarativeProfile, Model, Document) {
    let profile = profile();
    let docs = docs();
    let model = Model::build(&profile, docs.iter().map(|(i, d)| (i.clone(), d)));
    let note = docs
        .iter()
        .find(|(i, _)| *i == DocId::new(NOTE))
        .unwrap()
        .1
        .clone();
    (profile, model, note)
}

fn assertion(key: &str, value: &str, evidence: &str) -> ProposedAssertion {
    ProposedAssertion::new(key, value, evidence)
}

fn responses() -> Vec<ModelResponse> {
    vec![ModelResponse::new(
        0,
        vec![
            assertion("is-a", "graph", "is a directed graph"),
            assertion("part-of", "Thing", "not a Thing"),
            assertion("founded", "2019-03-01", "Founded on 2019-03-01"),
        ],
    )]
}

struct Fixed(Result<Vec<Verdict>, Unavailable>);
impl FactJudge for Fixed {
    fn judge(&self, _c: &[FactClaim]) -> Result<Vec<Verdict>, Unavailable> {
        self.0.clone()
    }
}

/// Rejects any claim whose written value is `[[Thing]]`.
struct RejectThing;
impl FactJudge for RejectThing {
    fn judge(&self, claims: &[FactClaim]) -> Result<Vec<Verdict>, Unavailable> {
        Ok(claims
            .iter()
            .map(|c| {
                if c.written == "[[Thing]]" {
                    Verdict::Unsupported
                } else {
                    Verdict::Supported
                }
            })
            .collect())
    }
}

#[test]
fn judge_unavailable_in_every_way_equals_plain_accept() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let id = DocId::new(NOTE);
    let request = ingest.plan(&id, &note).unwrap();
    let plain = ingest.accept(&note, &request, &responses()).unwrap();
    assert_eq!(plain.accepted.len(), 3, "fixture must accept all three");

    let cases: Vec<(&str, Box<dyn FactJudge>)> = vec![
        ("flag-off", Box::new(NoJudge)),
        ("timeout", Box::new(Fixed(Err(Unavailable::new("timeout"))))),
        (
            "http-503",
            Box::new(Fixed(Err(Unavailable::new("http-503")))),
        ),
        ("malformed", Box::new(Fixed(Ok(vec![Verdict::Unsupported])))),
        (
            "undecided",
            Box::new(Fixed(Ok(vec![Verdict::Undecided; 3]))),
        ),
    ];
    for (why, judge) in cases {
        let out = ingest
            .accept_judged(&note, &request, &responses(), judge.as_ref())
            .unwrap();
        assert_eq!(out.candidates, plain, "{why}");
    }
    // All supported: applied, nothing changed, still identical.
    let out = ingest
        .accept_judged(
            &note,
            &request,
            &responses(),
            &Fixed(Ok(vec![Verdict::Supported; 3])),
        )
        .unwrap();
    assert_eq!(out.candidates, plain);
    assert_eq!(
        out.outcome,
        JudgeOutcome::Applied {
            checked: 3,
            changed: 0
        }
    );
    // The skip reasons are the log codes.
    let off = ingest
        .accept_judged(&note, &request, &responses(), &NoJudge)
        .unwrap();
    assert_eq!(
        off.outcome,
        JudgeOutcome::Skipped {
            reason: "flag-off".into()
        }
    );
}

#[test]
fn confident_unsupported_is_withheld_and_the_patch_is_rebuilt_by_accept() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &note).unwrap();
    let out = ingest
        .accept_judged(&note, &request, &responses(), &RejectThing)
        .unwrap();
    assert_eq!(
        out.outcome,
        JudgeOutcome::Applied {
            checked: 3,
            changed: 1
        }
    );
    let written: Vec<&str> = out
        .candidates
        .accepted
        .iter()
        .map(|a| a.written.as_str())
        .collect();
    assert!(!written.contains(&"[[Thing]]"));
    assert_eq!(written.len(), 2);
    let w = out
        .candidates
        .withheld
        .iter()
        .find(|w| w.reason == WithholdReason::JudgedUnsupported)
        .expect("a judged-unsupported record");
    assert_eq!(w.key, "part-of");
    assert_eq!(w.value, "Thing");
    assert!(w.evidence.is_some());
    // The patch is exactly what plain accept gives for the surviving proposals,
    // so the digest binding and the in-memory proof are unchanged.
    let mut kept = responses();
    kept[0].assertions.remove(1);
    let expected = ingest.accept(&note, &request, &kept).unwrap();
    assert_eq!(out.candidates.patch, expected.patch);
    assert_eq!(out.candidates.accepted, expected.accepted);
    assert_eq!(out.candidates.digest, expected.digest);
    let patched = note.apply(out.candidates.patch.as_ref().unwrap()).unwrap();
    assert!(!patched.document.source().contains("[[Thing]]"));
}

#[test]
fn a_judge_cannot_unlock_a_value_the_plain_run_withheld() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &note).unwrap();
    // Two differing values for a single-valued key: plain accept withholds both.
    let competing = vec![ModelResponse::new(
        0,
        vec![
            assertion("founded", "2019-03-01", "Founded on 2019-03-01"),
            assertion("founded", "2019-03-01", "Founded on 2019-03-01"),
            assertion("is-a", "graph", "is a directed graph"),
            assertion("part-of", "Thing", "not a Thing"),
        ],
    )];
    let plain = ingest.accept(&note, &request, &competing).unwrap();
    struct Counting(Cell<usize>);
    impl FactJudge for Counting {
        fn judge(&self, c: &[FactClaim]) -> Result<Vec<Verdict>, Unavailable> {
            self.0.set(self.0.get() + c.len());
            // Reject everything.
            Ok(vec![Verdict::Unsupported; c.len()])
        }
    }
    let j = Counting(Cell::new(0));
    let out = ingest
        .accept_judged(&note, &request, &competing, &j)
        .unwrap();
    // Only facts the plain run accepted were ever sent to the judge.
    assert_eq!(j.0.get(), plain.accepted.len());
    assert!(out.candidates.accepted.is_empty());
}

#[test]
fn claims_carry_the_located_quote_not_the_models_text() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &note).unwrap();
    struct Capture(std::cell::RefCell<Vec<FactClaim>>);
    impl FactJudge for Capture {
        fn judge(&self, c: &[FactClaim]) -> Result<Vec<Verdict>, Unavailable> {
            self.0.borrow_mut().extend_from_slice(c);
            Ok(vec![Verdict::Supported; c.len()])
        }
    }
    let cap = Capture(Default::default());
    ingest
        .accept_judged(&note, &request, &responses(), &cap)
        .unwrap();
    let claims = cap.0.borrow();
    assert_eq!(claims.len(), 3);
    let thing = claims.iter().find(|c| c.written == "[[Thing]]").unwrap();
    assert_eq!(thing.quote, "not a Thing");
    assert_eq!(thing.subject, "Knowledge graph");
    assert_eq!(thing.key, "part-of");
}

fn cands() -> Vec<HostCandidate> {
    let c = |k: &str, v: &str, e: &str| HostCandidate {
        chunk: 0,
        key: k.into(),
        value: v.into(),
        evidence: e.into(),
    };
    vec![
        c("is-a", "graph", "is a directed graph"),
        c("is-a", "Thing", "not a Thing"),
        c("founded", "2019-03-01", "Founded on 2019-03-01"),
    ]
}

struct Answers(Result<Vec<Selection>, Unavailable>);
impl CandidateSelector for Answers {
    fn select(&self, _q: &[SelectionQuestion]) -> Result<Vec<Selection>, Unavailable> {
        self.0.clone()
    }
}

fn all_as_responses() -> Vec<ModelResponse> {
    vec![ModelResponse::new(
        0,
        cands()
            .into_iter()
            .map(|c| assertion(&c.key, &c.value, &c.evidence))
            .collect(),
    )]
}

#[test]
fn selector_unavailable_keeps_every_candidate() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &note).unwrap();
    let off: Vec<(&str, Box<dyn CandidateSelector>)> = vec![
        ("flag-off", Box::new(NoSelector)),
        (
            "timeout",
            Box::new(Answers(Err(Unavailable::new("timeout")))),
        ),
        (
            "malformed",
            Box::new(Answers(Ok(vec![Selection::NotStated]))),
        ),
        (
            "undecided",
            Box::new(Answers(Ok(vec![Selection::Undecided; 2]))),
        ),
        (
            "pick out of range",
            Box::new(Answers(Ok(vec![Selection::Pick(9), Selection::Undecided]))),
        ),
    ];
    for (why, s) in off {
        let out = ingest
            .select_responses(&request, &cands(), s.as_ref())
            .unwrap();
        assert_eq!(out.responses, all_as_responses(), "{why}");
        // Feeding them to accept equals feeding the baseline.
        assert_eq!(
            ingest.accept(&note, &request, &out.responses).unwrap(),
            ingest.accept(&note, &request, &all_as_responses()).unwrap(),
            "{why}"
        );
    }
}

#[test]
fn selector_narrows_a_group_and_accept_still_validates() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &note).unwrap();
    // Groups in order: (0,is-a) with 2 candidates, (0,founded) with 1.
    let sel = Answers(Ok(vec![Selection::Pick(0), Selection::NotStated]));
    let out = ingest.select_responses(&request, &cands(), &sel).unwrap();
    assert_eq!(
        out.outcome,
        JudgeOutcome::Applied {
            checked: 2,
            changed: 2
        }
    );
    assert_eq!(out.responses.len(), 1);
    let kept: Vec<(&str, &str)> = out.responses[0]
        .assertions
        .iter()
        .map(|a| (a.key.as_str(), a.value.as_str()))
        .collect();
    assert_eq!(kept, vec![("is-a", "graph")]);
    let accepted = ingest.accept(&note, &request, &out.responses).unwrap();
    assert_eq!(accepted.accepted.len(), 1);
    // An unknown chunk is the same hard error accept gives.
    let mut bad = cands();
    bad[0].chunk = 7;
    assert!(ingest
        .select_responses(&request, &bad, &NoSelector)
        .is_err());
}

#[test]
fn questions_put_the_chunk_and_candidates_in_order() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &note).unwrap();
    struct Capture(std::cell::RefCell<Vec<SelectionQuestion>>);
    impl CandidateSelector for Capture {
        fn select(&self, q: &[SelectionQuestion]) -> Result<Vec<Selection>, Unavailable> {
            self.0.borrow_mut().extend_from_slice(q);
            Ok(vec![Selection::Undecided; q.len()])
        }
    }
    let cap = Capture(Default::default());
    ingest.select_responses(&request, &cands(), &cap).unwrap();
    let qs = cap.0.borrow();
    assert_eq!(qs.len(), 2);
    assert_eq!(qs[0].key, "is-a");
    assert_eq!(qs[0].candidates.len(), 2);
    assert_eq!(qs[0].text, request.chunks[0].text);
}

struct Suggest(Result<Vec<RawSuggestion>, Unavailable>);
impl FieldSuggester for Suggest {
    fn suggest(&self, _q: &SuggestionQuestion) -> Result<Vec<RawSuggestion>, Unavailable> {
        self.0.clone()
    }
}

fn raw(key: &str, iri: &str, ambiguous: bool) -> RawSuggestion {
    RawSuggestion {
        key: key.into(),
        target: Iri::parse(iri).unwrap(),
        ambiguous,
    }
}

#[test]
fn suggester_unavailable_returns_nothing() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let id = DocId::new(NOTE);
    for s in [
        Box::new(NoSuggester) as Box<dyn FieldSuggester>,
        Box::new(Suggest(Err(Unavailable::new("http-500")))),
    ] {
        let out = ingest.suggest_fields(&id, &note, s.as_ref()).unwrap();
        assert!(out.proposals.is_empty() && out.ambiguous.is_empty());
        assert!(matches!(out.outcome, JudgeOutcome::Skipped { .. }));
    }
}

#[test]
fn suggestions_are_validated_against_keys_and_vocabulary() {
    let (profile, model, note) = setup();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let id = DocId::new(NOTE);
    let mentioned: Vec<String> = ingest
        .mentioned_terms(&id, &note)
        .iter()
        .map(|t| t.label.clone())
        .collect();
    assert_eq!(mentioned, vec!["Graph", "Network", "Thing"]);
    let out = ingest
        .suggest_fields(
            &id,
            &note,
            &Suggest(Ok(vec![
                raw("is-a", "urn:x:Graph", false),
                raw("part-of", "urn:x:Network", true),
                // A reserved key, an unknown key, a data key, a term nobody offered,
                // the note itself, and a duplicate: all dropped.
                raw("public", "urn:x:Graph", false),
                raw("colour", "urn:x:Graph", false),
                raw("founded", "urn:x:Graph", false),
                raw("is-a", "urn:x:Invented", false),
                raw("is-a", "urn:x:KG", false),
                raw("is-a", "urn:x:Graph", false),
            ])),
        )
        .unwrap();
    assert_eq!(out.proposals.len(), 1);
    assert_eq!(out.proposals[0].key, "is-a");
    assert_eq!(out.proposals[0].written, "[[Graph]]");
    assert_eq!(out.ambiguous.len(), 1);
    assert_eq!(out.ambiguous[0].written, "[[Network]]");
    assert_eq!(
        out.outcome,
        JudgeOutcome::Applied {
            checked: 8,
            changed: 2
        }
    );
}

#[test]
fn a_value_already_in_the_note_is_not_suggested_again() {
    let profile = profile();
    let mut d = docs();
    d[3].1 = Document::parse(
        "---\ntype: Class\niri: urn:x:KG\nis-a:\n- \"[[Graph]]\"\n---\nA directed graph, not a Thing.\n",
    );
    let model = Model::build(&profile, d.iter().map(|(i, d)| (i.clone(), d)));
    let note = d[3].1.clone();
    let ingest = Ingest::new(&profile, &model, Options::default());
    let out = ingest
        .suggest_fields(
            &DocId::new(NOTE),
            &note,
            &Suggest(Ok(vec![
                raw("is-a", "urn:x:Graph", false),
                raw("is-a", "urn:x:Thing", false),
            ])),
        )
        .unwrap();
    let w: Vec<&str> = out.proposals.iter().map(|s| s.written.as_str()).collect();
    assert_eq!(w, vec!["[[Thing]]"]);
}
