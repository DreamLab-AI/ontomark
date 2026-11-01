//! Regression cases for shared vault-layout and role contracts.
use ontomark_compile::input::page::{Vault, VaultKind};
use ontomark_compile::profile::Profile;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
fn note(name: &str) -> String {
    format!("---\ntype: class\ntitle: {name}\npublic: true\n---\n{name} description.\n")
}
fn fixture(layout: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    let profile = temp.path().join("profile.yaml");
    write(
        &profile,
        &format!("id: audit\nversion: '1'\nlayout:\n{layout}"),
    );
    (temp, root, profile)
}
fn cli(root: &Path, profile: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(root)
        .arg("--profile")
        .arg(profile)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn build_preserves_the_explicit_knowledge_role() {
    let (temp, root, profile) = fixture("  knowledge: nested/drafts\n  working: drafts\n");
    write(&root.join("nested/drafts/Alpha.md"), &note("Alpha"));
    let mut config = std::fs::read_to_string(&profile).unwrap();
    config.push_str("frontmatter:\n  roles:\n    knowledge:\n      required_keys: [status]\n      unknown_keys: allow\n    working:\n      unknown_keys: allow\n");
    write(&profile, &config);
    let validate = cli(&root, &profile, &["--json", "validate"]);
    assert_eq!(validate.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&validate.stdout).contains("MISSING_STATUS"));
    let out = temp.path().join("out");
    let build = cli(
        &root,
        &profile,
        &["--json", "build", "--out", out.to_str().unwrap()],
    );
    assert_eq!(
        build.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(String::from_utf8_lossy(&build.stderr).contains("MISSING_STATUS"));
    assert!(!out.exists());
}

#[test]
fn overlapping_knowledge_and_working_roots_are_refused() {
    let (temp, root, profile) = fixture("  knowledge: '.'\n  working: drafts\n");
    write(&root.join("Alpha.md"), &note("Alpha"));
    write(&root.join("drafts/DraftOnly.md"), &note("DraftOnly"));
    let output = cli(&root, &profile, &["--json", "validate", "--vault", "all"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("overlap"));
    for scope in ["public", "machine"] {
        let out = temp.path().join(scope);
        let output = cli(
            &root,
            &profile,
            &[
                "--json",
                "build",
                "--scope",
                scope,
                "--out",
                out.to_str().unwrap(),
            ],
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(!out.exists());
    }
}

#[test]
fn equivalent_folder_spellings_cannot_create_two_roles() {
    for working in ["./notes", "notes/../notes"] {
        let (_temp, root, profile) =
            fixture(&format!("  knowledge: notes\n  working: {working}\n"));
        write(&root.join("notes/Alpha.md"), &note("Alpha"));
        let p = Profile::from_yaml(&std::fs::read_to_string(&profile).unwrap()).unwrap();
        let k = p
            .layout
            .root_of(&root, VaultKind::Knowledge)
            .unwrap()
            .canonicalize()
            .unwrap();
        let w = p
            .layout
            .root_of(&root, VaultKind::Working)
            .unwrap()
            .canonicalize()
            .unwrap();
        assert_eq!(k, w);
        let output = cli(&root, &profile, &["--json", "validate", "--vault", "all"]);
        assert_eq!(output.status.code(), Some(2));
    }
}

#[test]
fn relative_and_implicit_starts_find_the_same_ancestor() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("estate");
    let child = root.join("nested");
    write(
        &root.join("ontology/vocabulary.yaml"),
        "[invalid vocabulary\n",
    );
    write(&child.join("Alpha.md"), &note("Alpha"));
    let implicit = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .current_dir(&child)
        .args(["--json", "validate"])
        .output()
        .unwrap();
    assert_eq!(implicit.status.code(), Some(1));
    let explicit = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .current_dir(&child)
        .args(["--repo", ".", "--json", "validate"])
        .output()
        .unwrap();
    assert_eq!(explicit.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&explicit.stderr).contains("vocabulary.yaml"));
}

#[test]
fn caller_exclusions_preserve_furniture_and_journal_separation() {
    let (_temp, root, profile) = fixture("  knowledge: '.'\n  working: null\n  excluded: []\n");
    write(&root.join("Alpha.md"), &note("Alpha"));
    write(
        &root.join("journals/2026_10_17.md"),
        "# Journal\n\nAn observation.\n",
    );
    write(&root.join("templates/Template.md"), "A template.\n");
    let p = Profile::from_yaml(&std::fs::read_to_string(&profile).unwrap()).unwrap();
    let v = Vault::load_excluding(&root, VaultKind::Knowledge, &p.layout.excluded_dirs()).unwrap();
    assert_eq!(v.pages.len(), 1);
    assert_eq!(v.journals.len(), 1);
    assert_eq!(v.pages[0].id, "Alpha");
    assert_eq!(v.journals[0].id, "2026_10_17");
}

#[test]
fn layout_cannot_select_a_sibling_outside_the_repository() {
    let (temp, root, profile) = fixture("  knowledge: ../outside\n  working: null\n");
    write(&temp.path().join("outside/Outside.md"), &note("Outside"));
    let output = cli(&root, &profile, &["--json", "validate"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
#[cfg(unix)]
fn symlink_role_aliases_are_refused() {
    let (_temp, root, profile) = fixture("  knowledge: notes\n  working: drafts\n");
    write(&root.join("notes/Alpha.md"), &note("Alpha"));
    std::os::unix::fs::symlink(root.join("notes"), root.join("drafts")).unwrap();
    let output = cli(&root, &profile, &["--json", "validate", "--vault", "all"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn migration_plan_refuses_an_undeclared_working_role() {
    let (_temp, root, profile) = fixture("  knowledge: '.'\n  working: null\n");
    write(&root.join("Alpha.md"), &note("Alpha"));
    let output = cli(
        &root,
        &profile,
        &[
            "--json",
            "migrate",
            "plan",
            "m0001-obsidian-bodies",
            "--vault",
            "working",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("no working vault"));
}

#[test]
fn rollback_refuses_a_parent_component_as_the_migration_name() {
    let dir = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(dir.path())
        .args(["migrate", "rollback", "..", "abc"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("must be plain names"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
