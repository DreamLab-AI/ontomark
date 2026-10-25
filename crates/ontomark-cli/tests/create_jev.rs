//! `ontomark create --suggest` (VG-01-S1-02), through the real binary with a
//! loopback server standing in for System One. Built only with `--features jev`.
#![cfg(feature = "jev")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const VOCABULARY: &str = r#"version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
scalars:
  domain: { type: text, roots: [space, earth, art] }
"#;
const EXISTING: &str = "---\ntype: Class\ntitle: Alpha\npublic: true\nresource: urn:ngm:class:alpha\nstatus: stable\n---\nbody\n";
const STAGED: &str = "---\ntype: Class\ntitle: New Thing\npublic: true\nresource: urn:ngm:class:new-thing\nstatus: draft\nis-a:\n- '[[Alpha]]'\n---\nRelated to Alpha, in orbit.\n";

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn repo() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    write(&repo.join("ontology/vocabulary.yaml"), VOCABULARY);
    write(&repo.join("knowledge/Alpha.md"), EXISTING);
    let staged = dir.path().join("staged.md");
    write(&staged, STAGED);
    (dir, repo, staged)
}

fn run(repo: &Path, staged: &Path, suggest: bool, env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ontomark"));
    c.arg("--repo")
        .arg(repo)
        .arg("--json")
        .arg("create")
        .arg(staged)
        .args(["--expect", "docs=1"]);
    if suggest {
        c.arg("--suggest");
    }
    for k in [
        "ONTOMARK_JEV",
        "ONTOMARK_JEV_ENDPOINT",
        "ONTOMARK_JEV_LOCAL",
        "ONTOMARK_JEV_ALLOW_CLOUD",
        "AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL",
        "TYPESAFE_API_KEY",
    ] {
        c.env_remove(k);
    }
    c.envs(env.iter().copied());
    c.output().unwrap()
}

/// A one-shot-per-connection System One stand-in; counts requests.
fn server(reply: &'static str, status: u16) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/systemone", l.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut buf = vec![0u8; 65536];
            let mut got = 0;
            // Read until the body is complete (headers + content-length).
            loop {
                let n = s.read(&mut buf[got..]).unwrap_or(0);
                if n == 0 {
                    break;
                }
                got += n;
                let text = String::from_utf8_lossy(&buf[..got]).to_string();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let len = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if body.len() >= len {
                        break;
                    }
                }
            }
            h.fetch_add(1, Ordering::SeqCst);
            let resp = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
            let _ = s.write_all(resp.as_bytes());
        }
    });
    (url, hits)
}

const GOOD: &str = r#"{"model":"m","usage":{},"answers":{
 "d0":{"type":"noul","noul":0.95},"d1":{"type":"noul","noul":0.01},"d2":{"type":"noul","noul":0.9},
 "type":{"type":"choice","choice":"t0","confidence":0.95,"probabilities":{}},
 "p0":{"type":"noul","noul":0.9}}}"#;

fn stdout_json(o: &Output) -> serde_json::Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("{e}: {o:?}"))
}

#[test]
fn unavailable_means_create_is_exactly_as_before() {
    // Baseline: no flag at all.
    let (_d0, r0, s0) = repo();
    let base = run(&r0, &s0, false, &[]);
    assert!(base.status.success(), "{base:?}");
    let page0 = std::fs::read_to_string(r0.join("knowledge/New Thing.md")).unwrap();

    let (url, hits) = server(GOOD, 200);
    let failing = server("{}", 503);
    let cases: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "flag off",
            vec![("ONTOMARK_JEV_ENDPOINT", &url), ("ONTOMARK_JEV_LOCAL", "1")],
        ),
        ("no endpoint", vec![("ONTOMARK_JEV", "1")]),
        (
            "not local",
            vec![("ONTOMARK_JEV", "1"), ("ONTOMARK_JEV_ENDPOINT", &url)],
        ),
        (
            "http 503",
            vec![
                ("ONTOMARK_JEV", "1"),
                ("ONTOMARK_JEV_ENDPOINT", &failing.0),
                ("ONTOMARK_JEV_LOCAL", "1"),
            ],
        ),
        (
            "refused connection",
            vec![
                ("ONTOMARK_JEV", "1"),
                ("ONTOMARK_JEV_ENDPOINT", "http://127.0.0.1:1/v1/systemone"),
                ("ONTOMARK_JEV_LOCAL", "1"),
            ],
        ),
    ];
    for (why, env) in cases {
        let (_d, r, s) = repo();
        let out = run(&r, &s, true, &env);
        assert!(out.status.success(), "{why}: {out:?}");
        assert_eq!(
            out.stdout, base.stdout,
            "{why}: stdout identical to plain create"
        );
        assert_eq!(
            std::fs::read_to_string(r.join("knowledge/New Thing.md")).unwrap(),
            page0,
            "{why}"
        );
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "flag-off and not-local sent nothing"
    );
}

#[test]
fn confident_local_answer_is_printed_but_never_changes_the_page() {
    let (url, hits) = server(GOOD, 200);
    let (_d0, r0, s0) = repo();
    let base = run(&r0, &s0, false, &[]);
    let page0 = std::fs::read_to_string(r0.join("knowledge/New Thing.md")).unwrap();

    let (_d, r, s) = repo();
    let env = [
        ("ONTOMARK_JEV", "1"),
        ("ONTOMARK_JEV_ENDPOINT", url.as_str()),
        ("ONTOMARK_JEV_LOCAL", "1"),
    ];
    let out = run(&r, &s, true, &env);
    assert!(out.status.success(), "{out:?}");
    let mut v = stdout_json(&out);
    let sug = v
        .as_object_mut()
        .unwrap()
        .remove("jev_suggestion")
        .expect("a suggestion");
    assert_eq!(sug["advisory"], true);
    assert_eq!(sug["domains"], serde_json::json!(["space", "art"]));
    assert_eq!(sug["type"], "Class");
    assert_eq!(sug["parents"], serde_json::json!(["Alpha"]));
    // Everything else equals plain create, and the written page is byte-identical.
    assert_eq!(v, stdout_json(&base));
    assert_eq!(
        std::fs::read_to_string(r.join("knowledge/New Thing.md")).unwrap(),
        page0
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[test]
fn a_refused_creation_sends_nothing() {
    let (url, hits) = server(GOOD, 200);
    let (_d, r, s) = repo();
    write(&s, &STAGED.replace("status: draft\n", ""));
    let env = [
        ("ONTOMARK_JEV", "1"),
        ("ONTOMARK_JEV_ENDPOINT", url.as_str()),
        ("ONTOMARK_JEV_LOCAL", "1"),
    ];
    let out = run(&r, &s, true, &env);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}
