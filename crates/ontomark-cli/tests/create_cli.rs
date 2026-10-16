// Ported from VisionClaw crates/vault/tests/create_cli.rs (DEPRECATION VC-29,
// relicensing grant in NOTICE). The `vault propose --dry-run` steps are
// dropped: posting stays in VisionClaw; the creation contract is what this
// binary owns.
//! `ontomark create` through the real binary: the interface the agentbox
//! elevation path calls, `ontomark --repo <root> create <file> --expect docs=1
//! --json`.

use std::path::Path;
use std::process::{Command, Output};

const VOCABULARY: &str = r#"version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
"#;

const EXISTING: &str = "---\ntype: Class\ntitle: Alpha\npublic: true\nresource: urn:ngm:class:alpha\nstatus: stable\n---\nbody\n";
const STAGED: &str = "---\ntype: Class\ntitle: New Thing\npublic: true\nresource: urn:ngm:class:new-thing\nstatus: draft\nis-a:\n- '[[Alpha]]'\n---\nElevated from a working note.\n";

/// The knowledge vault: pages sit at its root, and a created page is written there.
const PAGES: &str = "knowledge";

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn ontomark(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(repo)
        .arg("--json")
        .args(args)
        .output()
        .expect("the ontomark binary runs")
}

fn json(output: &Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not one JSON document ({e}):\n{stdout}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn create_a_page_exactly_once() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    write(&repo.join("ontology/vocabulary.yaml"), VOCABULARY);
    write(&repo.join(PAGES).join("Alpha.md"), EXISTING);
    let staged = dir.path().join("staged.md");
    write(&staged, STAGED);
    let staged_arg = staged.to_str().unwrap();

    let target = repo.join("knowledge/New Thing.md");
    let create = [
        "create",
        staged_arg,
        "--expect",
        "docs=1",
        "--set",
        "status=stable",
        "--set",
        "verified+={by: 'human:npub1test', at: '2026-09-22T00:00:00Z'}",
    ];
    let dry = ontomark(&repo, &[&create[..], &["--dry-run"]].concat());
    assert!(dry.status.success(), "{dry:?}");
    assert_eq!(json(&dry)["dry_run"], true);
    assert!(!target.exists(), "a dry run writes nothing");

    let first = ontomark(&repo, &create);
    assert!(first.status.success(), "{first:?}");
    let outcome = json(&first);
    assert_eq!(outcome["created"], true);
    assert_eq!(outcome["dry_run"], false);
    assert_eq!(outcome["path"], "knowledge/New Thing.md");
    assert_eq!(outcome["iri"], "urn:ngm:class:new-thing");
    assert_eq!(outcome["page"], "New Thing");
    assert_eq!(outcome["docs"], 1);
    assert_eq!(outcome["blocks"], 2);
    let written = std::fs::read_to_string(&target).unwrap();
    assert!(written.contains("status: stable"), "{written}");
    assert!(written.contains("human:npub1test"), "{written}");
    assert!(written.ends_with("---\nElevated from a working note.\n"));

    let second = ontomark(&repo, &create);
    assert_eq!(second.status.code(), Some(2), "{second:?}");
    let refusal = json(&second);
    assert_eq!(refusal["created"], false);
    assert_eq!(refusal["code"], "EXISTS");
    assert!(refusal["message"].as_str().is_some_and(|m| !m.is_empty()));
    assert_eq!(refusal["blockers"], serde_json::json!([]));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), written);
}

#[test]
fn create_refuses_an_invalid_page_or_an_undeclared_radius_with_exit_2() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    write(&repo.join("ontology/vocabulary.yaml"), VOCABULARY);
    write(&repo.join(PAGES).join("Alpha.md"), EXISTING);
    let staged = dir.path().join("staged.md");
    write(&staged, &STAGED.replace("status: draft\n", ""));
    let staged_arg = staged.to_str().unwrap();

    let invalid = ontomark(&repo, &["create", staged_arg, "--expect", "docs=1"]);
    assert_eq!(invalid.status.code(), Some(2), "{invalid:?}");
    let refusal = json(&invalid);
    assert_eq!(refusal["code"], "BLOCKED");
    let blockers = refusal["blockers"].as_array().unwrap();
    assert!(!blockers.is_empty(), "{refusal}");
    assert!(blockers.iter().all(|b| b["code"].is_string()), "{refusal}");

    let undeclared = ontomark(&repo, &["create", staged_arg, "--set", "status=stable"]);
    assert_eq!(undeclared.status.code(), Some(2), "{undeclared:?}");
    assert_eq!(json(&undeclared)["code"], "UNDECLARED_BLAST_RADIUS");

    let clash = dir.path().join("clash.md");
    write(
        &clash,
        &STAGED.replace("urn:ngm:class:new-thing", "urn:ngm:class:alpha"),
    );
    let collided = ontomark(
        &repo,
        &["create", clash.to_str().unwrap(), "--expect", "docs=1"],
    );
    assert_eq!(collided.status.code(), Some(2), "{collided:?}");
    let refusal = json(&collided);
    assert_eq!(refusal["code"], "BLOCKED");
    assert!(
        refusal["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["code"] == "IRI_COLLISION"),
        "{refusal}"
    );

    assert!(!repo.join(PAGES).join("New Thing.md").exists());
}
