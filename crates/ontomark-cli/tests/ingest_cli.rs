//! `ontomark ingest` through the real binary against a stub model server
//! (ADR-OM-010 verification items 4 and 5). No test touches the network: the
//! "model" is a `TcpListener` on loopback that counts the requests it gets.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use ontomark_source::{LocalFiles, Source};
use serde_json::Value;

const VOCABULARY: &str = r#"version: 1
namespace: "urn:ngm:class:"
prefixes:
  vc: "https://narrativegoldmine.com/ns/v1#"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
  requires: { owl: "vc:requires" }
"#;

fn page(title: &str, extra: &str) -> String {
    let slug = title.to_lowercase().replace(' ', "-");
    format!(
        "---\ntype: Class\ntitle: {title}\npublic: true\nresource: urn:ngm:class:{slug}\nstatus: stable\n{extra}---\nAbout {title}.\n"
    )
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn fixture(root: &Path, widget_extra: &str) {
    write(&root.join("ontology/vocabulary.yaml"), VOCABULARY);
    write(&root.join("knowledge/Alpha.md"), &page("Alpha", ""));
    write(&root.join("knowledge/Beta.md"), &page("Beta", ""));
    write(&root.join("knowledge/Gamma.md"), &page("Gamma", ""));
    write(
        &root.join("knowledge/Quantum Widget.md"),
        &page("Quantum Widget", widget_extra),
    );
}

const MD: &str = "# Widgets\n\nThe Quantum Widget requires the Gamma engine to run.\n";
const HTML: &str =
    "<html><body><h1>Specs</h1><p>The Quantum Widget is a kind of Beta device.</p></body></html>";

fn sources(dir: &Path) {
    write(&dir.join("widgets.md"), MD);
    write(&dir.join("specs.html"), HTML);
}

/// A loopback server that answers every chat completion from `answer(body)`.
struct Stub {
    url: String,
    count: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    addr: std::net::SocketAddr,
    thread: Option<JoinHandle<()>>,
}

impl Stub {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, s) = (count.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if s.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else { continue };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let body = loop {
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).into_owned();
                    if let Some((head, rest)) = text.split_once("\r\n\r\n") {
                        let want: usize = head
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap())
                            })
                            .unwrap_or(0);
                        if rest.len() >= want || n == 0 {
                            break rest.to_owned();
                        }
                    } else if n == 0 {
                        break String::new();
                    }
                };
                c.fetch_add(1, Ordering::SeqCst);
                let content = answer(&body);
                let reply = serde_json::json!({
                    "choices": [{ "message": { "role": "assistant", "content": content } }]
                })
                .to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
            }
        });
        Self {
            url: format!("http://{addr}/v1"),
            count,
            stop,
            addr,
            thread: Some(thread),
        }
    }

    fn calls(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

impl Drop for Stub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// What the "model" says, decided by the chunk it was shown.
fn answer(body: &str) -> String {
    let mut assertions = Vec::new();
    if body.contains("requires the Gamma engine") {
        assertions.push(serde_json::json!({
            "key": "requires", "value": "Gamma", "evidence": "requires the Gamma engine"
        }));
    }
    if body.contains("is a kind of Beta device") {
        assertions.push(serde_json::json!({
            "key": "is-a", "value": "Beta", "evidence": "is a kind of Beta"
        }));
    }
    serde_json::json!({ "chunk": 0, "assertions": assertions }).to_string()
}

struct Env {
    _dir: tempfile::TempDir,
    repo: PathBuf,
    work: PathBuf,
    src: PathBuf,
}

fn env(widget_extra: &str) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let work = dir.path().join("work");
    let src = dir.path().join("src");
    fixture(&repo, widget_extra);
    sources(&src);
    std::fs::create_dir_all(&work).unwrap();
    Env {
        _dir: dir,
        repo,
        work,
        src,
    }
}

fn ingest(e: &Env, extra: &[&str], vars: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ontomark"));
    for k in [
        "ONTOMARK_JEV",
        "ONTOMARK_JEV_ALLOW_CLOUD",
        "ONTOMARK_JEV_LOCAL",
        "ONTOMARK_JEV_ENDPOINT",
        "AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL",
        "AGENTBOX_SYSTEM_ONE_PROXY",
        "AGENTBOX_SYSTEM_ONE_ENDPOINT",
        "VAULT_NOSTR_SECRET",
    ] {
        cmd.env_remove(k);
    }
    for (k, v) in vars {
        cmd.env(k, v);
    }
    cmd.current_dir(&e.work)
        .arg("--repo")
        .arg(&e.repo)
        .arg("--json")
        .arg("ingest")
        .args(extra)
        .output()
        .unwrap()
}

fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout is not JSON ({err}):\n{}\nstderr: {}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )
    })
}

fn run_model(e: &Env, stub: &Stub, extra: &[&str]) -> Output {
    let mut args = vec![
        "--source",
        e.src.to_str().unwrap(),
        "--target",
        "Quantum Widget",
        "--endpoint",
        &stub.url,
        "--model",
        "stub-1",
        "--out",
        "bundle",
    ];
    args.extend_from_slice(extra);
    ingest(e, &args, &[])
}

/// The rendered text of the source `id` (`file:<path>`), as ingest read it.
fn rendered(id: &str) -> String {
    let path = id.strip_prefix("file:").unwrap();
    let source = LocalFiles::new(path);
    let item = source.list().unwrap().remove(0);
    source.fetch(&item).unwrap().text
}

fn evidence(e: &Env) -> Value {
    serde_json::from_slice(&std::fs::read(e.work.join("bundle/evidence.json")).unwrap()).unwrap()
}

#[test]
fn bundle_validates_and_quotes_sit_at_their_spans() {
    let e = env("");
    let stub = Stub::start();
    let out = run_model(&e, &stub, &[]);
    assert!(out.status.success(), "{out:?}");
    let summary = json(&out);
    assert_eq!(summary["model_calls"], 2);
    assert_eq!(stub.calls(), 2);
    assert_eq!(summary["complete"], true);
    let cmd = summary["propose_command"].as_str().unwrap();
    assert!(
        cmd.starts_with("vault propose 'Quantum Widget' --repo "),
        "{cmd}"
    );
    assert!(cmd.contains("/bundle/manifest' --title '"), "{cmd}");

    // The manifest page carries both accepted values and no quote.
    let page = std::fs::read_to_string(e.work.join("bundle/manifest/Quantum Widget.md")).unwrap();
    assert!(
        page.contains("[[Gamma]]") && page.contains("[[Beta]]"),
        "{page}"
    );
    assert!(!page.contains("requires the Gamma") && !page.contains("kind of Beta"));
    assert!(
        page.contains("About Quantum Widget."),
        "the body is untouched"
    );
    assert!(!e.work.join("bundle/manifest/Alpha.md").exists());

    // `validate` accepts a copy of the vault with the manifest applied.
    let copy = e.work.join("copy");
    copy_dir(&e.repo, &copy);
    std::fs::copy(
        e.work.join("bundle/manifest/Quantum Widget.md"),
        copy.join("knowledge/Quantum Widget.md"),
    )
    .unwrap();
    let v = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(&copy)
        .arg("validate")
        .output()
        .unwrap();
    assert!(v.status.success(), "{}", String::from_utf8_lossy(&v.stdout));

    // Every recorded quote is at its span in the source text it names.
    let ev = evidence(&e);
    let mut seen = 0;
    for step in ev["targets"][0]["steps"].as_array().unwrap() {
        let src = step["source"]["id"].as_str().unwrap();
        let text = rendered(src);
        assert_eq!(
            ontomark_source::version_of(&text),
            step["source"]["version"].as_str().unwrap()
        );
        for a in step["accepted"].as_array().unwrap() {
            for s in a["evidence"].as_array().unwrap() {
                let (from, to) = (
                    s["span"]["start"].as_u64().unwrap() as usize,
                    s["span"]["end"].as_u64().unwrap() as usize,
                );
                assert_eq!(&text[from..to], s["quote"].as_str().unwrap());
                assert_eq!(s["source"]["id"], src);
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 2);
    assert_eq!(ev["provenance_keys"].as_array().unwrap().len(), 0);
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::create_dir_all(to).unwrap();
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn an_unchanged_rerun_makes_no_model_call() {
    let e = env("");
    let stub = Stub::start();
    assert!(run_model(&e, &stub, &[]).status.success());
    let first = std::fs::read(e.work.join("bundle/evidence.json")).unwrap();
    assert_eq!(stub.calls(), 2);

    let again = run_model(&e, &stub, &[]);
    assert!(again.status.success(), "{again:?}");
    let s = json(&again);
    assert_eq!(s["model_calls"], 0);
    assert_eq!(s["cache_hits"], 2);
    assert_eq!(stub.calls(), 2, "the stub saw no new request");
    assert_eq!(
        first,
        std::fs::read(e.work.join("bundle/evidence.json")).unwrap()
    );
    assert!(s["sources"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["change"] == "unchanged"));
}

#[test]
fn a_changed_source_replans_only_that_source() {
    let e = env("");
    let stub = Stub::start();
    assert!(run_model(&e, &stub, &[]).status.success());
    assert_eq!(stub.calls(), 2);
    write(
        &e.src.join("specs.html"),
        "<p>The Quantum Widget is a kind of Beta device, and a sturdy one.</p>",
    );
    let out = run_model(&e, &stub, &[]);
    assert!(out.status.success(), "{out:?}");
    let s = json(&out);
    assert_eq!(s["model_calls"], 1);
    assert_eq!(stub.calls(), 3);
    let change = |suffix: &str| {
        s["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"].as_str().unwrap().ends_with(suffix))
            .unwrap()["change"]
            .clone()
    };
    assert_eq!(change("specs.html"), "content_changed");
    assert_eq!(change("widgets.md"), "unchanged");
}

#[test]
fn the_budget_stops_cleanly_and_records_what_was_skipped() {
    let e = env("");
    let stub = Stub::start();
    let out = run_model(&e, &stub, &["--budget-calls", "1"]);
    assert!(out.status.success(), "{out:?}");
    let s = json(&out);
    assert_eq!(s["budget_exhausted"], true);
    assert_eq!(s["complete"], false);
    assert_eq!(s["model_calls"], 1);
    assert_eq!(stub.calls(), 1);
    let skipped = s["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["reason"], "budget_calls");
    assert_eq!(evidence(&e)["complete"], false);

    // The character budget is checked before the call too.
    let e2 = env("");
    let out = run_model(&e2, &stub, &["--budget-chars", "10"]);
    let s = json(&out);
    assert_eq!(s["model_calls"], 0);
    assert_eq!(s["skipped"][0]["reason"], "budget_chars");
    assert_eq!(stub.calls(), 1, "no further request was sent");
}

#[test]
fn a_cloud_endpoint_is_refused_without_the_opt_in() {
    let e = env("");
    let args = [
        "--source",
        e.src.to_str().unwrap(),
        "--target",
        "Quantum Widget",
        "--endpoint",
        "https://api.example.com/v1",
        "--model",
        "m",
        "--budget-calls",
        "0",
    ];
    let out = ingest(&e, &args, &[]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(json(&out)["refused"]
        .as_str()
        .unwrap()
        .contains("not local"));
    assert!(!e.work.join(".ontomark").exists(), "nothing was written");

    // With the opt-in the endpoint is accepted; a zero call budget keeps the
    // run from sending anything anywhere.
    let out = ingest(&e, &args, &[("ONTOMARK_JEV_ALLOW_CLOUD", "1")]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(json(&out)["model_calls"], 0);
}

#[test]
fn values_extend_what_the_page_holds_and_a_repeat_is_not_proposed_twice() {
    // Relation keys are lists: a second parent joins the first, and a value
    // the page already holds is withheld as already present, not duplicated.
    let e = env("is-a:\n- '[[Alpha]]'\n- '[[Beta]]'\n");
    let stub = Stub::start();
    let out = run_model(&e, &stub, &[]);
    assert!(out.status.success(), "{out:?}");
    let ev = evidence(&e);
    assert_eq!(ev["review"].as_array().unwrap().len(), 0);
    let page = std::fs::read_to_string(e.work.join("bundle/manifest/Quantum Widget.md")).unwrap();
    assert_eq!(page.matches("[[Beta]]").count(), 1, "{page}");
    assert!(page.contains("[[Alpha]]") && page.contains("[[Gamma]]"));
    let withheld = ev.to_string();
    assert!(withheld.contains("already_present"), "{withheld}");
}

#[test]
fn discover_finds_exact_mentions_in_order_and_honours_the_cap() {
    let e = env("");
    let base = [
        "--source",
        e.src.to_str().unwrap(),
        "--discover",
        "--plan-only",
    ];
    let all = json(&ingest(&e, &base, &[]));
    let targets: Vec<&str> = all["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert_eq!(targets, ["Beta", "Gamma", "Quantum Widget"]);
    assert!(all["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["chunks"] == 1));

    let capped = json(&ingest(
        &e,
        &[&base[..], &["--max-targets", "1"]].concat(),
        &[],
    ));
    assert_eq!(
        capped["targets"][0], "Quantum Widget",
        "first mention wins the cap"
    );
    assert_eq!(capped["targets"].as_array().unwrap().len(), 1);
}

#[test]
fn bad_requests_exit_two() {
    let e = env("");
    let none = ingest(
        &e,
        &["--source", e.src.to_str().unwrap(), "--plan-only"],
        &[],
    );
    assert_eq!(none.status.code(), Some(2));
    let unknown = ingest(
        &e,
        &[
            "--source",
            e.src.to_str().unwrap(),
            "--target",
            "Nope",
            "--plan-only",
        ],
        &[],
    );
    assert_eq!(unknown.status.code(), Some(2));
    let working = ingest(
        &e,
        &[
            "--source",
            e.src.to_str().unwrap(),
            "--target",
            "Beta",
            "--plan-only",
            "--vault",
            "working",
        ],
        &[],
    );
    assert_eq!(working.status.code(), Some(2), "{working:?}");
    assert!(json(&working)["refused"]
        .as_str()
        .unwrap()
        .contains("private"));
}

/// The `vault` binary: `$VAULT_BIN`, else the first `vault` on `PATH`.
fn vault_bin() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VAULT_BIN") {
        return Some(PathBuf::from(p));
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join("vault"))
        .find(|p| p.is_file())
}

/// ADR-OM-010 verification 4: `vault propose --dry-run --diff <bundle>/manifest`
/// accepts what ingest wrote. Runs when a `vault` binary is available
/// (`VAULT_BIN` or `PATH`); signs with a throwaway key.
#[test]
fn vault_propose_dry_run_accepts_the_manifest() {
    let Some(vault) = vault_bin() else {
        eprintln!("skipped: no `vault` binary (set VAULT_BIN)");
        return;
    };
    let e = env("");
    let git = |args: &[&str]| {
        let s = Command::new("git")
            .current_dir(&e.repo)
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .output()
            .unwrap();
        assert!(s.status.success(), "{s:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-qm", "fixture"]);

    let stub = Stub::start();
    assert!(run_model(&e, &stub, &[]).status.success());
    let secret: String = (0..32)
        .map(|i| format!("{:02x}", (i * 7 + 3) as u8))
        .collect();
    let out = Command::new(vault)
        .current_dir(&e.work)
        .env_remove("VAULT_NOSTR_SECRET")
        .args(["propose", "Quantum Widget", "--repo"])
        .arg(&e.repo)
        .args([
            "--diff",
            "bundle/manifest",
            "--title",
            "ingest test",
            "--hypothesis",
            "test",
        ])
        .args(["--dry-run", "--json", "--secret", &secret])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["proposal"]["pages"][0], "Quantum Widget");
    let diff = v["proposal"]["diff"].as_str().unwrap();
    assert!(
        diff.contains("+requires:") && diff.contains("+- '[[Gamma]]'"),
        "{diff}"
    );
    assert!(v["proposal"]["blockers"].as_array().unwrap().is_empty());
    assert_eq!(v["proposal"]["kind"], "amend");
    let p = &v["proposal"];
    eprintln!(
        "propose dry-run: kind={} pages={} blockers={} consequences={}",
        p["kind"], p["pages"], p["blockers"], p["consequences"]["verdict"]
    );
    eprintln!("diff:\n{diff}");
}
