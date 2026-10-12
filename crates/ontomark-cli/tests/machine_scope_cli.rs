//! `ontomark build --scope machine` through the real binary: the bundle is
//! written and stamped, and each option the machine scope cannot honour exits
//! 2 naming itself, with nothing written.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/estate-small")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create destination");
    for entry in std::fs::read_dir(from).expect("read the fixture") {
        let entry = entry.expect("directory entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// A scratch copy of the fixture: `(scratch, repo, out)`.
fn repo() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    copy_tree(&fixture(), &repo);
    let out = scratch.path().join("out");
    (scratch, repo, out)
}

fn build(repo: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(repo)
        .args(["--profile", "estate", "build", "--out"])
        .arg(out)
        .args([
            "--generated-at",
            "2026-09-22T00:00:00+00:00",
            "--source-commit",
            "0000000000000000000000000000000000000000",
        ])
        .args(extra)
        .output()
        .expect("the ontomark binary runs")
}

#[test]
fn scope_machine_writes_the_bundle_and_stamps_it() {
    let (_scratch, repo, out) = repo();
    let output = build(&repo, &out, &["--scope", "machine"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out
        .join("api/pages/working/fixture-public-note.json")
        .is_file());
    assert!(!out.join("publish").exists());
    let marker = std::fs::read_to_string(out.join(".generation.json")).unwrap();
    assert!(marker.contains("\"scope\": \"machine\""), "{marker}");
}

#[test]
fn scope_public_is_the_default_and_writes_no_scope_field() {
    let (_scratch, repo, out) = repo();
    let output = build(&repo, &out, &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.join("publish").is_dir());
    let marker = std::fs::read_to_string(out.join(".generation.json")).unwrap();
    assert!(!marker.contains("\"scope\""), "{marker}");
}

#[test]
fn scope_machine_refuses_what_it_cannot_honour_with_exit_2() {
    let elsewhere = tempfile::tempdir().unwrap();
    let publish_out = elsewhere.path().join("publish");
    let publish_out = publish_out.to_str().unwrap();
    let cases: [(&[&str], &str); 4] = [
        (&["--publish-out", publish_out], "--publish-out"),
        (&["--vault", "all"], "--vault"),
        (&["--vault", "working"], "--vault"),
        (&["--no-redact-private-links"], "--no-redact-private-links"),
    ];
    for (flags, named) in cases {
        let (_scratch, repo, out) = repo();
        let mut args = vec!["--scope", "machine"];
        args.extend_from_slice(flags);
        let output = build(&repo, &out, &args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{flags:?}: {stderr}");
        assert!(
            stderr.contains(named),
            "{flags:?} must name {named}: {stderr}"
        );
        assert!(!out.exists(), "{flags:?}: nothing written");
    }
}

#[test]
fn an_unknown_scope_is_an_invalid_argument() {
    let (_scratch, repo, out) = repo();
    let output = build(&repo, &out, &["--scope", "everything"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("everything"));
}
