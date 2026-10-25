//! Fallback equivalence, wire shape and the accept path, over a mocked transport.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use ontomark_ingest::{
    HostCandidate, Ingest, JudgeOutcome, ModelResponse, Options, ProposedAssertion, WithholdReason,
};
use ontomark_jev::{
    JevClient, JevConfig, JevFactJudge, JevSelector, JevSuggester, Transport, TransportError,
    CLOUD_ENDPOINT,
};
use ontomark_model::{DeclarativeProfile, DocId, Model, ProfileSpec};
use ontomark_parse::Document;
use serde_json::{json, Value};

const NOTE: &str = "Knowledge graph.md";

// ---- fixtures --------------------------------------------------------------

fn profile() -> DeclarativeProfile {
    let spec: ProfileSpec = serde_json::from_str(
        r##"{
        "id": "jev-test/1",
        "kinds": { "Class": "class", "NamedIndividual": "individual" },
        "predicates": {
            "is-a": { "iri": "http://www.w3.org/2000/01/rdf-schema#subClassOf", "form": "sub_class_of" },
            "part-of": { "iri": "urn:x:partOf", "form": "existential" }
        },
        "reserved": ["tags", "public"]
    }"##,
    )
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
        page(
            "Knowledge graph",
            "type: Class\niri: urn:x:KG",
            "A knowledge graph is a directed graph. It is not a Thing in any sense.\n",
        ),
    ]
}

fn responses() -> Vec<ModelResponse> {
    let a = |k: &str, v: &str, e: &str| ProposedAssertion {
        key: k.into(),
        value: v.into(),
        evidence: e.into(),
    };
    vec![ModelResponse {
        chunk: 0,
        assertions: vec![
            a("is-a", "graph", "is a directed graph"),
            a("part-of", "Thing", "not a Thing"),
        ],
    }]
}

// ---- mock transport --------------------------------------------------------

type Reply = Result<(u16, String), TransportError>;
type Call = (String, Option<String>, Value, Duration);
type Responder = Rc<RefCell<Box<dyn FnMut(&Value) -> Reply>>>;

#[derive(Clone)]
struct Mock {
    reply: Responder,
    calls: Rc<RefCell<Vec<Call>>>,
}

impl Mock {
    fn new(f: impl FnMut(&Value) -> Reply + 'static) -> Self {
        Self {
            reply: Rc::new(RefCell::new(Box::new(f))),
            calls: Rc::default(),
        }
    }
    fn count(&self) -> usize {
        self.calls.borrow().len()
    }
}

impl Transport for Mock {
    fn post(&self, url: &str, bearer: Option<&str>, body: &Value, t: Duration) -> Reply {
        self.calls
            .borrow_mut()
            .push((url.into(), bearer.map(Into::into), body.clone(), t));
        (self.reply.borrow_mut())(body)
    }
}

fn ok(body: Value) -> Reply {
    Ok((200, body.to_string()))
}

fn enabled() -> JevConfig {
    JevConfig {
        enabled: true,
        endpoint: Some("http://jev.invalid/v1/systemone".into()),
        key_env: Some("K".into()),
        // Thresholds a deployment would fit; the defaults are inert.
        reject_at_or_below: 0.1,
        select_min_confidence: 0.8,
        ..JevConfig::default()
    }
}

fn client(config: JevConfig, t: Mock) -> (JevClient<Mock>, Rc<RefCell<Vec<String>>>) {
    let logs = Rc::new(RefCell::new(Vec::new()));
    let sink = logs.clone();
    let c = JevClient::with_env(
        config,
        t,
        Box::new(|k| (k == "K").then(|| "sekret".to_owned())),
        Box::new(move |l| sink.borrow_mut().push(l.to_owned())),
    );
    (c, logs)
}

/// A Noul reply for questions `c0..`: probabilities by index.
fn noul_reply(ps: &'static [f64]) -> impl FnMut(&Value) -> Reply {
    move |body| {
        let qs = body["questions"].as_object().unwrap();
        let answers: serde_json::Map<String, Value> = qs
            .keys()
            .map(|id| {
                let i: usize = id[1..].parse().unwrap();
                (id.clone(), json!({"type": "noul", "noul": ps[i]}))
            })
            .collect();
        ok(
            json!({"model": "jev-1", "answers": answers, "usage": {"input_tokens": 1, "output_tokens": 1}}),
        )
    }
}

struct World {
    profile: DeclarativeProfile,
    model: Model,
    note: Document,
}

fn world() -> World {
    let profile = profile();
    let d = docs();
    let model = Model::build(&profile, d.iter().map(|(i, x)| (i.clone(), x)));
    let note = d[2].1.clone();
    World {
        profile,
        model,
        note,
    }
}

// ---- OM-06-S1-01 -----------------------------------------------------------

#[test]
fn fact_judge_every_fallback_equals_plain_accept() {
    let w = world();
    let ingest = Ingest::new(&w.profile, &w.model, Options::default());
    let id = DocId::new(NOTE);
    let request = ingest.plan(&id, &w.note).unwrap();
    let plain = ingest.accept(&w.note, &request, &responses()).unwrap();
    assert_eq!(plain.accepted.len(), 2);

    type Case = (&'static str, JevConfig, Box<dyn FnMut(&Value) -> Reply>);
    let cases: Vec<Case> = vec![
        (
            "flag-off",
            JevConfig {
                enabled: false,
                ..enabled()
            },
            Box::new(|_| panic!("a request was sent with the flag off")),
        ),
        (
            "no-endpoint",
            JevConfig {
                endpoint: None,
                ..enabled()
            },
            Box::new(|_| panic!("a request was sent with no endpoint")),
        ),
        (
            "timeout",
            enabled(),
            Box::new(|_| Err(TransportError::Timeout)),
        ),
        (
            "transport",
            enabled(),
            Box::new(|_| Err(TransportError::Failed)),
        ),
        ("http-429", enabled(), Box::new(|_| Ok((429, "{}".into())))),
        ("http-529", enabled(), Box::new(|_| Ok((529, "{}".into())))),
        (
            "http-500",
            enabled(),
            Box::new(|_| Ok((500, "oops".into()))),
        ),
        (
            "malformed",
            enabled(),
            Box::new(|_| Ok((200, "not json".into()))),
        ),
        (
            "missing answers",
            enabled(),
            Box::new(|_| ok(json!({"model": "m", "answers": {}, "usage": {}}))),
        ),
        (
            "wrong answer type",
            enabled(),
            Box::new(|b| {
                let ids: Vec<&String> = b["questions"].as_object().unwrap().keys().collect();
                let a: serde_json::Map<String, Value> = ids
                    .iter()
                    .map(|i| ((*i).clone(), json!({"type": "choice", "choice": "x", "confidence": 1.0, "probabilities": {}})))
                    .collect();
                ok(json!({"model": "m", "answers": a, "usage": {}}))
            }),
        ),
        (
            "probability out of range",
            enabled(),
            Box::new(noul_reply(&[7.0, 7.0])),
        ),
        // Low confidence: p(yes) = 0.3 for the weak fact is above the 0.1 reject line.
        (
            "low-confidence",
            enabled(),
            Box::new(noul_reply(&[0.3, 0.3])),
        ),
    ];
    for (why, cfg, f) in cases {
        let mock = Mock::new(f);
        let (c, logs) = client(cfg, mock.clone());
        let out = ingest
            .accept_judged(&w.note, &request, &responses(), &JevFactJudge(c))
            .unwrap();
        assert_eq!(out.candidates, plain, "{why}");
        if why == "flag-off" || why == "no-endpoint" {
            assert_eq!(mock.count(), 0, "{why}");
        }
        // One log line carries the reason and never the note text.
        for l in logs.borrow().iter() {
            assert!(!l.contains("directed graph"), "{why}: {l}");
        }
        if why != "flag-off" && why != "low-confidence" {
            assert_eq!(logs.borrow().len(), 1, "{why}");
        }
    }
}

#[test]
fn fact_judge_wire_shape_matches_the_contract() {
    let w = world();
    let ingest = Ingest::new(&w.profile, &w.model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &w.note).unwrap();
    let mock = Mock::new(noul_reply(&[0.9, 0.9]));
    let (c, _) = client(enabled(), mock.clone());
    ingest
        .accept_judged(&w.note, &request, &responses(), &JevFactJudge(c))
        .unwrap();
    let calls = mock.calls.borrow();
    assert_eq!(calls.len(), 1, "one batched request, zero retries");
    let (url, bearer, body, timeout) = &calls[0];
    assert_eq!(url, "http://jev.invalid/v1/systemone");
    assert_eq!(bearer.as_deref(), Some("sekret"));
    assert_eq!(*timeout, Duration::from_secs(4));
    assert_eq!(body["model"], "jev-latest");
    // SystemOneRequest: model, state (object), questions (non-empty map of Question).
    let obj = body.as_object().unwrap();
    assert_eq!(obj.len(), 3);
    assert_eq!(body["state"]["subject"], "Knowledge graph");
    let claims = body["state"]["claims"].as_array().unwrap();
    assert_eq!(claims.len(), 2);
    assert_eq!(claims[1]["quote"], "not a Thing");
    let qs = body["questions"].as_object().unwrap();
    assert_eq!(qs.len(), 2);
    for (id, q) in qs {
        assert!(id.starts_with('c'));
        assert_eq!(q["type"], "noul");
        assert!(q["instructions"].as_str().unwrap().contains("`claims`"));
        // NoulCriteria is {true, false}; there is no Choice-style map here.
        assert_eq!(q["criteria"].as_object().unwrap().len(), 2);
        assert!(q["criteria"]["true"].is_string() && q["criteria"]["false"].is_string());
        // Only the fields the schema allows.
        for k in q.as_object().unwrap().keys() {
            assert!(["type", "instructions", "criteria"].contains(&k.as_str()));
        }
    }
}

#[test]
fn fact_judge_confident_no_withholds_and_accept_still_runs() {
    let w = world();
    let ingest = Ingest::new(&w.profile, &w.model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &w.note).unwrap();
    // c0 = is-a graph (supported), c1 = part-of Thing (p = 0.02 <= 0.1).
    let (c, _) = client(enabled(), Mock::new(noul_reply(&[0.95, 0.02])));
    let out = ingest
        .accept_judged(&w.note, &request, &responses(), &JevFactJudge(c))
        .unwrap();
    assert_eq!(
        out.outcome,
        JudgeOutcome::Applied {
            checked: 2,
            changed: 1
        }
    );
    assert_eq!(out.candidates.accepted.len(), 1);
    assert_eq!(out.candidates.accepted[0].written, "[[Graph]]");
    assert!(out
        .candidates
        .withheld
        .iter()
        .any(|x| x.reason == WithholdReason::JudgedUnsupported && x.key == "part-of"));
    // The patch comes from accept and applies to the digest-bound note.
    let patch = out.candidates.patch.as_ref().unwrap();
    let applied = w.note.apply(patch).unwrap();
    assert!(applied.document.source().contains("[[Graph]]"));
    assert!(!applied.document.source().contains("[[Thing]]"));
}

#[test]
fn default_thresholds_are_inert() {
    let w = world();
    let ingest = Ingest::new(&w.profile, &w.model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &w.note).unwrap();
    let plain = ingest.accept(&w.note, &request, &responses()).unwrap();
    let cfg = JevConfig {
        reject_at_or_below: JevConfig::default().reject_at_or_below,
        ..enabled()
    };
    // Even a confident "no" cannot withhold anything until thresholds are fitted.
    let (c, _) = client(cfg, Mock::new(noul_reply(&[0.0, 0.0])));
    let out = ingest
        .accept_judged(&w.note, &request, &responses(), &JevFactJudge(c))
        .unwrap();
    assert_eq!(out.candidates, plain);
}

// ---- endpoint resolution ---------------------------------------------------

fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k| {
        pairs
            .iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| (*v).to_owned())
    }
}

#[test]
fn endpoint_resolution_order_and_local_only() {
    let on = JevConfig {
        enabled: true,
        ..JevConfig::default()
    };
    // Off by default, whatever the environment says.
    let all: &[(&str, &str)] = &[
        ("TYPESAFE_API_KEY", "k"),
        ("AGENTBOX_SYSTEM_ONE_PROXY", "1"),
        (
            "AGENTBOX_SYSTEM_ONE_ENDPOINT",
            "http://127.0.0.1:1/v1/systemone",
        ),
        ("AGENTBOX_SYSTEM_ONE_MODEL", "m"),
    ];
    assert_eq!(
        JevConfig::default()
            .resolve(&env_of(all))
            .unwrap_err()
            .reason,
        "flag-off"
    );
    // Vault notes are local-only: neither the proxy nor the cloud is used undeclared.
    assert_eq!(on.resolve(&env_of(all)).unwrap_err().reason, "no-endpoint");
    // Declared local: the keyless proxy wins over the cloud key.
    let declared: &[(&str, &str)] = &[
        ("AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL", "1"),
        ("AGENTBOX_SYSTEM_ONE_PROXY", "1"),
        (
            "AGENTBOX_SYSTEM_ONE_ENDPOINT",
            "http://127.0.0.1:1/v1/systemone",
        ),
        ("AGENTBOX_SYSTEM_ONE_MODEL", "m"),
        ("TYPESAFE_API_KEY", "k"),
    ];
    let r = on.resolve(&env_of(declared)).unwrap();
    assert_eq!((r.model.as_str(), r.bearer.clone()), ("m", None));
    // Explicit config beats everything and may carry its own key.
    let explicit = JevConfig {
        endpoint: Some("http://x.invalid/s".into()),
        key_env: Some("TYPESAFE_API_KEY".into()),
        ..on.clone()
    };
    let r = explicit.resolve(&env_of(declared)).unwrap();
    assert_eq!(r.url, "http://x.invalid/s");
    assert_eq!(r.bearer.as_deref(), Some("k"));
    assert!(!format!("{r:?}").contains('k') || format!("{r:?}").contains("<redacted>"));
    // A consumer that is not local-only reaches the cloud with the key.
    let open = JevConfig {
        local_only: false,
        ..on.clone()
    };
    let r = open.resolve(&env_of(&[("TYPESAFE_API_KEY", "k")])).unwrap();
    assert_eq!(r.url, CLOUD_ENDPOINT);
    assert_eq!(r.model, "jev-latest");
    // From the environment.
    let cfg = JevConfig::from_env(&env_of(&[
        ("ONTOMARK_JEV", "1"),
        ("ONTOMARK_JEV_ENDPOINT", "http://e.invalid/s"),
    ]));
    assert!(cfg.enabled && cfg.endpoint.as_deref() == Some("http://e.invalid/s"));
    assert!(!JevConfig::from_env(&env_of(&[("ONTOMARK_JEV", "0")])).enabled);
}

// ---- OM-06-S1-02 -----------------------------------------------------------

fn host_candidates() -> Vec<HostCandidate> {
    let c = |v: &str, e: &str| HostCandidate {
        chunk: 0,
        key: "is-a".into(),
        value: v.into(),
        evidence: e.into(),
    };
    vec![c("graph", "is a directed graph"), c("Thing", "not a Thing")]
}

fn choice_reply(choice: &'static str, confidence: f64) -> impl FnMut(&Value) -> Reply {
    move |body| {
        let answers: serde_json::Map<String, Value> = body["questions"]
            .as_object()
            .unwrap()
            .keys()
            .map(|id| {
                (
                    id.clone(),
                    json!({"type": "choice", "choice": choice, "confidence": confidence, "probabilities": {}}),
                )
            })
            .collect();
        ok(json!({"model": "m", "answers": answers, "usage": {}}))
    }
}

#[test]
fn selector_fallbacks_equal_the_all_candidates_baseline() {
    let w = world();
    let ingest = Ingest::new(&w.profile, &w.model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &w.note).unwrap();
    let baseline = ingest
        .select_responses(&request, &host_candidates(), &ontomark_ingest::NoSelector)
        .unwrap()
        .responses;
    type Case = (&'static str, JevConfig, Box<dyn FnMut(&Value) -> Reply>);
    let cases: Vec<Case> = vec![
        (
            "flag-off",
            JevConfig {
                enabled: false,
                ..enabled()
            },
            Box::new(|_| panic!("sent")),
        ),
        (
            "timeout",
            enabled(),
            Box::new(|_| Err(TransportError::Timeout)),
        ),
        (
            "http-503",
            enabled(),
            Box::new(|_| Ok((503, String::new()))),
        ),
        ("malformed", enabled(), Box::new(|_| Ok((200, "[".into())))),
        (
            "choice outside the offered set",
            enabled(),
            Box::new(choice_reply("c9", 1.0)),
        ),
        (
            "low-confidence",
            enabled(),
            Box::new(choice_reply("c0", 0.5)),
        ),
    ];
    for (why, cfg, f) in cases {
        let (c, _) = client(cfg, Mock::new(f));
        let out = ingest
            .select_responses(&request, &host_candidates(), &JevSelector(c))
            .unwrap();
        assert_eq!(out.responses, baseline, "{why}");
        assert_eq!(
            ingest.accept(&w.note, &request, &out.responses).unwrap(),
            ingest.accept(&w.note, &request, &baseline).unwrap(),
            "{why}"
        );
    }
}

#[test]
fn selector_wire_shape_and_confident_pick() {
    let w = world();
    let ingest = Ingest::new(&w.profile, &w.model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &w.note).unwrap();
    let mock = Mock::new(choice_reply("c0", 0.95));
    let (c, _) = client(enabled(), mock.clone());
    let out = ingest
        .select_responses(&request, &host_candidates(), &JevSelector(c))
        .unwrap();
    assert_eq!(out.responses[0].assertions.len(), 1);
    assert_eq!(out.responses[0].assertions[0].value, "graph");
    let accepted = ingest.accept(&w.note, &request, &out.responses).unwrap();
    assert_eq!(accepted.accepted.len(), 1);

    let body = mock.calls.borrow()[0].2.clone();
    let q = &body["questions"]["g0"];
    assert_eq!(q["type"], "choice");
    // Choice criteria is a MAP of key -> description, including `none`.
    let crit = q["criteria"].as_object().unwrap();
    assert_eq!(
        crit.keys().cloned().collect::<Vec<_>>(),
        vec!["c0", "c1", "none"]
    );
    assert!(crit["c1"].as_str().unwrap().contains("not a Thing"));
    assert_eq!(body["state"]["chunks"][0]["chunk"], 0);
    assert!(body["state"]["chunks"][0]["text"]
        .as_str()
        .unwrap()
        .contains("directed graph"));
    // The chosen "none" with confidence drops the group; accept then has nothing.
    let (c, _) = client(enabled(), Mock::new(choice_reply("none", 0.95)));
    let out = ingest
        .select_responses(&request, &host_candidates(), &JevSelector(c))
        .unwrap();
    assert!(out.responses.is_empty());
}

#[test]
fn default_selector_threshold_is_inert() {
    let w = world();
    let ingest = Ingest::new(&w.profile, &w.model, Options::default());
    let request = ingest.plan(&DocId::new(NOTE), &w.note).unwrap();
    let cfg = JevConfig {
        select_min_confidence: JevConfig::default().select_min_confidence,
        ..enabled()
    };
    let (c, _) = client(cfg, Mock::new(choice_reply("none", 1.0)));
    let out = ingest
        .select_responses(&request, &host_candidates(), &JevSelector(c))
        .unwrap();
    assert_eq!(out.responses[0].assertions.len(), 2);
}

// ---- OM-01-S1-01 -----------------------------------------------------------

#[test]
fn suggester_fallbacks_return_nothing_and_a_confident_choice_is_validated() {
    let w = world();
    let ingest = Ingest::new(&w.profile, &w.model, Options::default());
    let id = DocId::new(NOTE);
    let ran = Rc::new(Cell::new(0));
    // Fallbacks → empty, same as the engine before the hook existed.
    for (why, f) in [
        (
            "timeout",
            Box::new(|_: &Value| Err(TransportError::Timeout)) as Box<dyn FnMut(&Value) -> Reply>,
        ),
        ("http-500", Box::new(|_| Ok((500, String::new())))),
        ("malformed", Box::new(|_| Ok((200, "{".into())))),
        ("outside offered set", Box::new(choice_reply("t99", 1.0))),
        ("low-confidence", Box::new(choice_reply("t0", 0.2))),
    ] {
        let (c, _) = client(enabled(), Mock::new(f));
        let out = ingest
            .suggest_fields(&id, &w.note, &JevSuggester(c))
            .unwrap();
        assert!(
            out.proposals.is_empty() && out.ambiguous.is_empty(),
            "{why}"
        );
        ran.set(ran.get() + 1);
    }
    let off = JevConfig {
        enabled: false,
        ..enabled()
    };
    let (c, _) = client(off, Mock::new(|_| panic!("sent")));
    assert!(ingest
        .suggest_fields(&id, &w.note, &JevSuggester(c))
        .unwrap()
        .proposals
        .is_empty());

    // Confident: t0 is "Graph" (terms sorted by IRI: Graph, Thing).
    let mock = Mock::new(choice_reply("t0", 0.95));
    let (c, _) = client(enabled(), mock.clone());
    let out = ingest
        .suggest_fields(&id, &w.note, &JevSuggester(c))
        .unwrap();
    // Both name keys (is-a, part-of) pick Graph.
    let keys: Vec<&str> = out.proposals.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(keys, vec!["is-a", "part-of"]);
    assert!(out.proposals.iter().all(|p| p.written == "[[Graph]]"));
    // Choice map with `none`; the state names the terms and keys.
    let body = mock.calls.borrow()[0].2.clone();
    let crit = body["questions"]["k0"]["criteria"].as_object().unwrap();
    assert!(crit.contains_key("none") && crit.contains_key("t0") && crit.contains_key("t1"));
    assert_eq!(body["state"]["keys"][1]["relation_key"], "part-of");
    // A mid-confidence pick is flagged ambiguous, not proposed.
    let (c, _) = client(enabled(), Mock::new(choice_reply("t0", 0.6)));
    let out = ingest
        .suggest_fields(&id, &w.note, &JevSuggester(c))
        .unwrap();
    assert!(out.proposals.is_empty() && out.ambiguous.len() == 2);
}
