//! Reproduction of register item S1.5 (OM-00.2): under `--vault all`, `gate`
//! and `conflicts` look only at the first vault (`ontomark-cli/src/main.rs:334-337`
//! and `369-372`, `.into_iter().next()`), so a finding that exists only in
//! `working/` is never reported. Written red against ffe06e4; green since 8d654d1, where both iterate the vaults.

use std::path::Path;
use std::process::{Command, Output};

const VOCABULARY: &str = r#"version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
"#;

const SYNTHETIC_KEY: &str = "sk-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn class(title: &str, resource: &str, body: &str) -> String {
    format!(
        "---\ntype: Class\ntitle: {title}\npublic: false\nstatus: stable\nresource: urn:ngm:class:{resource}\n---\n{body}\n"
    )
}

/// `knowledge/` holds one clean page; `working/` holds the page(s) that gate
/// and conflicts would report on.
fn repo(working: &[(&str, String)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("ontology/vocabulary.yaml"), VOCABULARY);
    write(
        &dir.path().join("knowledge/pages/Clean.md"),
        &class("Clean", "clean", "A clean page."),
    );
    for (name, text) in working {
        write(&dir.path().join("working/pages").join(name), text);
    }
    dir
}

fn ontomark(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(repo)
        .args(args)
        .output()
        .expect("the ontomark binary runs")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Control: the same working page IS reported when `--vault working` is asked
/// for, so the fixture is a real finding and only the `all` selector hides it.
#[test]
fn control_working_only_findings_are_reported_for_vault_working() {
    let repo = repo(&[
        (
            "Leaky.md",
            class("Leaky", "leaky", &format!("key {SYNTHETIC_KEY}")),
        ),
        ("Twin A.md", class("Twin Concept", "twin-a", "One.")),
        ("Twin B.md", class("Twin Concept", "twin-b", "Two.")),
    ]);
    let gate = text(&ontomark(repo.path(), &["gate", "--vault", "working"]));
    assert!(gate.contains("Leaky"), "{gate}");
    let conflicts = text(&ontomark(repo.path(), &["conflicts", "--vault", "working"]));
    assert!(conflicts.contains("Twin Concept"), "{conflicts}");
}

/// S1.5 (gate): a credential that only `working/` carries must appear in
/// `gate --vault all`, and the gate must not pass.
#[test]
fn s1_5_gate_all_reports_a_working_only_page() {
    let repo = repo(&[(
        "Leaky.md",
        class("Leaky", "leaky", &format!("key {SYNTHETIC_KEY}")),
    )]);
    let output = ontomark(repo.path(), &["gate", "--vault", "all"]);
    let out = text(&output);
    assert!(
        out.contains("Leaky") && output.status.code() != Some(0),
        "S1.5: `gate --vault all` ignored the working vault (main.rs:334-337); \
         exit {:?}, output: {out}",
        output.status.code()
    );
}

/// S1.5 (conflicts): a duplicate concept that exists only in `working/` must
/// appear in `conflicts --vault all`.
#[test]
fn s1_5_conflicts_all_reports_a_working_only_conflict() {
    let repo = repo(&[
        ("Twin A.md", class("Twin Concept", "twin-a", "One.")),
        ("Twin B.md", class("Twin Concept", "twin-b", "Two.")),
    ]);
    let output = ontomark(repo.path(), &["conflicts", "--vault", "all"]);
    let out = text(&output);
    assert!(
        out.contains("Twin Concept"),
        "S1.5: `conflicts --vault all` ignored the working vault (main.rs:369-372); \
         exit {:?}, output: {out}",
        output.status.code()
    );
}
