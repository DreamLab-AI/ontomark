//! Reproduction test for register item S2.4 (`check/gate.rs:42-48`,
//! `check/conflicts.rs:48-55`): `--tier` and `--severity` accept any string and
//! silently fall back to `quick` / `high`. A typo in a CI gate must be refused
//! (exit 2) with the bad value named, not run a weaker check and exit 0.
//!
//! Expected to FAIL today. No product code is changed by this file.

use std::path::Path;
use std::process::{Command, Output};

const VOCABULARY: &str = r#"version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
"#;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A minimal valid repo with one clean page.
fn repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    write(&repo.join("ontology/vocabulary.yaml"), VOCABULARY);
    write(
        &repo.join("knowledge/pages/Solo.md"),
        "---\ntype: Class\npublic: true\nstatus: stable\nresource: urn:ngm:class:solo\n---\nA page.\n",
    );
    (dir, repo)
}

fn ontomark(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(repo)
        .args(args)
        .output()
        .expect("the ontomark binary runs")
}

#[test]
fn s2_4a_gate_refuses_an_unknown_tier() {
    let (_dir, repo) = repo();
    let control = ontomark(&repo, &["gate", "--tier", "quick"]);
    assert_eq!(
        control.status.code(),
        Some(0),
        "fixture sanity: a valid tier on a clean repo passes: {}",
        String::from_utf8_lossy(&control.stderr)
    );

    let output = ontomark(&repo, &["gate", "--tier", "bogus"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "S2.4: `--tier bogus` must exit 2, not fall back to quick; stderr: {stderr}"
    );
    assert!(
        stderr.contains("bogus"),
        "stderr must name the value: {stderr}"
    );
}

#[test]
fn s2_4b_conflicts_refuses_an_unknown_severity() {
    let (_dir, repo) = repo();
    let control = ontomark(&repo, &["conflicts", "--severity", "high"]);
    assert_eq!(
        control.status.code(),
        Some(0),
        "fixture sanity: a valid severity on a clean repo passes: {}",
        String::from_utf8_lossy(&control.stderr)
    );

    let output = ontomark(&repo, &["conflicts", "--severity", "bogus"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "S2.4: `--severity bogus` must exit 2, not fall back to high; stderr: {stderr}"
    );
    assert!(
        stderr.contains("bogus"),
        "stderr must name the value: {stderr}"
    );
}
