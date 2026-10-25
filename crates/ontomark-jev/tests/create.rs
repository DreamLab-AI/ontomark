//! VG-01-S1-02: advisory create suggestions — local-only, fallbacks, wire shape.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use ontomark_jev::{
    CreateQuestion, JevClient, JevConfig, JevCreateSuggester, ParentCandidate, Transport,
    TransportError,
};
use serde_json::{json, Value};

type Reply = Result<(u16, String), TransportError>;
type Case = (&'static str, JevConfig, Box<dyn Fn(&Value) -> Reply>);

#[derive(Clone)]
struct Mock {
    reply: Rc<dyn Fn(&Value) -> Reply>,
    bodies: Rc<RefCell<Vec<Value>>>,
}
impl Mock {
    fn new(f: impl Fn(&Value) -> Reply + 'static) -> Self {
        Self {
            reply: Rc::new(f),
            bodies: Rc::default(),
        }
    }
}
impl Transport for Mock {
    fn post(&self, _u: &str, _b: Option<&str>, body: &Value, _t: Duration) -> Reply {
        self.bodies.borrow_mut().push(body.clone());
        (self.reply)(body)
    }
}

fn cfg() -> JevConfig {
    JevConfig {
        enabled: true,
        endpoint: Some("http://127.0.0.1:1/v1/systemone".into()),
        local: true,
        ..JevConfig::default()
    }
}

fn client(c: JevConfig, m: Mock) -> JevCreateSuggester<Mock> {
    JevCreateSuggester(JevClient::with_env(
        c,
        m,
        Box::new(|_| None),
        Box::new(|_| {}),
    ))
}

fn question() -> CreateQuestion {
    CreateQuestion {
        title: "Orbital debris".into(),
        excerpt: "Fragments in low earth orbit.".into(),
        domains: vec!["space".into(), "earth".into(), "art".into()],
        types: vec!["Class".into(), "Individual".into()],
        parents: vec![
            ParentCandidate {
                id: "Debris".into(),
                title: "Debris".into(),
                node_type: Some("Class".into()),
            },
            ParentCandidate {
                id: "Orbit".into(),
                title: "Orbit".into(),
                node_type: None,
            },
        ],
    }
}

fn answers(f: impl Fn(&str) -> Value + 'static) -> impl Fn(&Value) -> Reply {
    move |b| {
        let a: serde_json::Map<String, Value> = b["questions"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| (k.clone(), f(k)))
            .collect();
        Ok((
            200,
            json!({"model": "m", "answers": a, "usage": {}}).to_string(),
        ))
    }
}

fn confident(k: &str) -> Value {
    match k {
        "d0" | "d1" | "p0" => json!({"type": "noul", "noul": 0.95}),
        "d2" | "p1" => json!({"type": "noul", "noul": 0.02}),
        _ => json!({"type": "choice", "choice": "t0", "confidence": 0.9, "probabilities": {}}),
    }
}

#[test]
fn confident_answer_selects_independent_domains_and_multiple_parents() {
    let m = Mock::new(answers(confident));
    let s = client(cfg(), m.clone()).suggest(&question()).unwrap();
    assert_eq!(s.domains, vec!["space", "earth"]);
    assert_eq!(s.node_type.as_deref(), Some("Class"));
    assert_eq!(s.parents, vec!["Debris"]);
    assert!(!s.ambiguous);
    // Wire shape: Noul per domain/parent, one Choice with a criteria MAP incl. none.
    let b = m.bodies.borrow()[0].clone();
    let q = b["questions"].as_object().unwrap();
    assert_eq!(q.keys().cloned().collect::<Vec<_>>().len(), 6);
    assert_eq!(q["d0"]["type"], "noul");
    assert_eq!(q["type"]["type"], "choice");
    let crit = q["type"]["criteria"].as_object().unwrap();
    assert!(crit.contains_key("none") && crit.contains_key("t0") && crit.contains_key("t1"));
    assert_eq!(b["state"]["domains"][2]["name"], "art");
    assert_eq!(b["state"]["candidate_parents"][1]["title"], "Orbit");
}

#[test]
fn borderline_or_none_is_flagged_ambiguous_and_picks_nothing_invented() {
    let m = Mock::new(answers(|k| match k {
        "d0" => json!({"type": "noul", "noul": 0.5}),
        "type" => {
            json!({"type": "choice", "choice": "none", "confidence": 0.9, "probabilities": {}})
        }
        _ => json!({"type": "noul", "noul": 0.0}),
    }));
    let s = client(cfg(), m).suggest(&question()).unwrap();
    assert!(s.ambiguous && s.domains.is_empty() && s.parents.is_empty() && s.node_type.is_none());
}

#[test]
fn every_fallback_is_unavailable() {
    let cases: Vec<Case> = vec![
        (
            "flag-off",
            JevConfig {
                enabled: false,
                ..cfg()
            },
            Box::new(|_| panic!("sent")),
        ),
        (
            "no-endpoint",
            JevConfig {
                endpoint: None,
                ..cfg()
            },
            Box::new(|_| panic!("sent")),
        ),
        ("timeout", cfg(), Box::new(|_| Err(TransportError::Timeout))),
        ("http-503", cfg(), Box::new(|_| Ok((503, String::new())))),
        ("malformed", cfg(), Box::new(|_| Ok((200, "x".into())))),
        (
            "malformed",
            cfg(),
            Box::new(answers(|k| {
                if k == "type" {
                    json!({"type": "choice", "choice": "t9", "confidence": 1.0, "probabilities": {}})
                } else {
                    confident(k)
                }
            })),
        ),
    ];
    for (why, c, f) in cases {
        let e = client(c, Mock::new(f)).suggest(&question()).unwrap_err();
        assert_eq!(e.reason, why);
    }
    let empty = CreateQuestion {
        domains: vec![],
        types: vec![],
        parents: vec![],
        ..question()
    };
    let m = Mock::new(|_| panic!("sent"));
    assert_eq!(
        client(cfg(), m).suggest(&empty).unwrap_err().reason,
        "nothing-to-suggest"
    );
}

#[test]
fn a_non_local_endpoint_is_refused_unless_allow_cloud() {
    let m = Mock::new(answers(confident));
    let remote = JevConfig {
        local: false,
        endpoint: Some("https://jev.example/v1/systemone".into()),
        ..cfg()
    };
    let e = client(remote.clone(), m.clone())
        .suggest(&question())
        .unwrap_err();
    assert_eq!(e.reason, "not-local");
    assert!(m.bodies.borrow().is_empty(), "no request was built");
    let allowed = JevConfig {
        allow_cloud: true,
        ..remote
    };
    assert!(client(allowed, m.clone()).suggest(&question()).is_ok());
    assert_eq!(m.bodies.borrow().len(), 1);
}
