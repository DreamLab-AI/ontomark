//! A plain Obsidian vault — notes at the root, Obsidian furniture beside them,
//! no `ontology/` anywhere — is a repository (ADR-OM-008). `--repo <vault>`
//! validates, builds and queries it under the generic profile; the template
//! is reported as an excluded skip; the generation id carries the vault's
//! directory name; and the roles the flat layout does not have are refused
//! with exit 2 naming the layout.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The vault: two classes (one nested), a prose-only note, a Templater
/// template, a Base, and the editor's own folder.
fn plain_vault() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path().join("Field Notes");
    write(
        &vault.join("Animal.md"),
        "---\ntype: class\ntitle: Animal\npublic: true\n---\nA living thing.\n",
    );
    write(
        &vault.join("Zoology/Dog.md"),
        "---\ntype: class\nsubclass-of: [\"[[Animal]]\"]\npublic: true\n---\nA dog.\n",
    );
    write(
        &vault.join("Scratch.md"),
        "# Scratch\n\nProse only, [[Animal]].\n",
    );
    write(
        &vault.join("templates/Note.md"),
        "---\ntype: <% tp.file.title %>\n---\n",
    );
    write(
        &vault.join("bases/x.base"),
        "filters:\n  and:\n    - file.ext == \"md\"\nviews:\n  - type: table\n    name: All\n",
    );
    write(
        &vault.join(".obsidian/app.json"),
        "{\"alwaysUpdateLinks\": true}\n",
    );
    (dir, vault)
}

fn ontomark(vault: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(vault)
        .args(args)
        .output()
        .expect("the ontomark binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn validate_passes_and_reports_the_template_as_an_excluded_skip() {
    let (_dir, vault) = plain_vault();
    let output = ontomark(&vault, &["--json", "validate"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    // The role name keys the report, whatever folder the vault was read from.
    let knowledge = &report["knowledge"];
    assert_eq!(knowledge["total_pages"], 3, "{knowledge}");
    assert_eq!(knowledge["errors"], 0, "{knowledge}");
    assert_eq!(
        knowledge["skipped"],
        serde_json::json!([
            {"path": ".obsidian/app.json", "reason": "hidden_directory"},
            {"path": "templates/Note.md", "reason": "excluded_directory"}
        ])
        .as_array()
        .map(|a| serde_json::Value::Array(
            a.iter()
                .filter(|s| s["path"] != ".obsidian/app.json")
                .cloned()
                .collect()
        ))
        .unwrap(),
        "{knowledge}"
    );

    let prose = ontomark(&vault, &["validate"]);
    assert_eq!(prose.status.code(), Some(0));
    assert!(
        stdout(&prose).starts_with("[knowledge] 3 pages, 2 with ontology, 2 public — 0 errors"),
        "{}",
        stdout(&prose)
    );
}

#[test]
fn build_succeeds_and_stamps_the_vault_name_into_the_generation() {
    let (dir, vault) = plain_vault();
    let out = dir.path().join("www");
    let output = ontomark(
        &vault,
        &[
            "--json",
            "build",
            "--out",
            out.to_str().unwrap(),
            "--source-commit",
            "abc1234",
            "--generated-at",
            "2026-10-17T00:00:00+00:00",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(
        report["generation"]["id"], "Field Notes@abc1234",
        "{report}"
    );
    assert_eq!(report["generation"]["commit"], "abc1234");
    assert!(report["written"].as_u64().unwrap() > 0);
    let marker: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join(".generation.json")).unwrap())
            .unwrap();
    assert_eq!(marker["id"], "Field Notes@abc1234");
    // The output contract is independent of the input layout: knowledge pages
    // stage under `publish/pages/`.
    assert!(out.join("publish/pages/Animal.md").is_file());
    assert!(out.join("data/ontology.ttl").is_file());
    assert!(
        !out.join("publish/pages/templates").exists(),
        "the template is furniture, not a page"
    );

    // A dirty build says so in its id, under the same prefix.
    let dirty = ontomark(
        &vault,
        &[
            "--json",
            "build",
            "--out",
            out.to_str().unwrap(),
            "--source-commit",
            "abc1234",
            "--source-dirty",
        ],
    );
    assert_eq!(dirty.status.code(), Some(0), "stderr: {}", stderr(&dirty));
    let report: serde_json::Value = serde_json::from_str(&stdout(&dirty)).unwrap();
    assert_eq!(report["generation"]["id"], "Field Notes@abc1234+dirty");
}

#[test]
fn find_queries_the_root_vault() {
    let (_dir, vault) = plain_vault();
    let output = ontomark(&vault, &["--json", "find", "--query", "dog"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let hits: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let ids: Vec<&str> = hits
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["Zoology/Dog"], "{hits}");
}

#[test]
fn roles_the_flat_layout_lacks_are_refused_with_exit_2() {
    let (dir, vault) = plain_vault();
    for args in [
        &["validate", "--vault", "working"][..],
        &["validate", "--vault", "all"],
        &["find", "--vault", "working", "--query", "dog"],
        &["repair", "fences", "--vault", "working", "--dry-run"],
    ] {
        let output = ontomark(&vault, args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains(
                "no working vault: this layout declares none (layout: knowledge = \".\", \
                 working = none)"
            ),
            "{args:?}: {}",
            stderr(&output)
        );
    }
    let out = dir.path().join("www");
    for args in [
        &[
            "build",
            "--vault",
            "working",
            "--out",
            out.to_str().unwrap(),
        ][..],
        &[
            "build",
            "--scope",
            "machine",
            "--out",
            out.to_str().unwrap(),
        ],
    ] {
        let output = ontomark(&vault, args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("layout: knowledge = \".\", working = none"),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(!out.exists(), "nothing is written");
    }
}

#[test]
fn a_profile_yaml_keeps_its_own_layout_on_a_plain_vault() {
    // A vault that keeps its notes in `notes/` and drafts in `drafts/` says so
    // in a profile; the command line does not second-guess a declared layout.
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    write(
        &repo.join("notes/Animal.md"),
        "---\ntype: class\ntitle: Animal\npublic: true\n---\nA living thing.\n",
    );
    write(
        &repo.join("drafts/Idea.md"),
        "---\ntype: Note\n---\nAn idea.\n",
    );
    let profile = dir.path().join("profile.yaml");
    write(
        &profile,
        "id: mine\nversion: '1'\nlayout:\n  knowledge: notes\n  working: drafts\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(&repo)
        .arg("--profile")
        .arg(&profile)
        .args(["--json", "validate", "--vault", "all"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["knowledge"]["total_pages"], 1, "{report}");
    assert_eq!(report["working"]["total_pages"], 1, "{report}");
}
