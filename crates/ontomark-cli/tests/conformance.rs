//! Offline conformance: `ontomark` against the legacy golden for the
//! `estate-small` fixture (`conformance/fixtures/estate-small.golden`).
//!
//! The golden was produced by VisionClaw's `vault` at tag
//! `ontomark-census-baseline` (`1044b0722`) by `conformance/golden/capture.sh`.
//! Every artefact of the build (`out/` and the `publish/` staging) must be
//! byte-identical when ontomark is given the recorded clock and source commit,
//! and every verb (`validate`, `conflicts`, `gate`, `repair bodies --check`, in
//! prose and JSON) must print the same bytes and exit with the same code.
//!
//! The full-corpus comparison needs the visionGraph checkout and runs through
//! `conformance/parity.sh`; this test needs nothing but the repository.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn golden() -> PathBuf {
    repo_root().join("conformance/fixtures/estate-small.golden")
}

fn manifest() -> BTreeMap<String, String> {
    std::fs::read_to_string(golden().join("MANIFEST"))
        .expect("golden MANIFEST")
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// Copy the fixture vault to a scratch directory: the verbs and the build
/// must not depend on where the vault lives.
fn fixture_copy() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("scratch dir");
    copy_tree(
        &repo_root().join("conformance/fixtures/estate-small"),
        dir.path(),
    );
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

/// Every file under `root`, by relative path.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if entry.file_type().expect("file type").is_dir() {
                walk(base, &path, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("under base")
                    .to_string_lossy()
                    .into_owned();
                out.insert(rel, std::fs::read(&path).expect("read"));
            }
        }
    }
    let mut out = BTreeMap::new();
    if root.exists() {
        walk(root, root, &mut out);
    }
    out
}

fn ontomark() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ontomark"))
}

/// Compare two trees, returning one line per difference.
fn tree_diff(label: &str, want: &Path, got: &Path) -> Vec<String> {
    let (want, got) = (files(want), files(got));
    let mut diffs = Vec::new();
    for (path, bytes) in &want {
        match got.get(path) {
            None => diffs.push(format!("{label}/{path}: missing")),
            Some(b) if b != bytes => diffs.push(format!("{label}/{path}: bytes differ")),
            Some(_) => {}
        }
    }
    diffs.extend(
        got.keys()
            .filter(|p| !want.contains_key(*p))
            .map(|p| format!("{label}/{p}: unexpected")),
    );
    diffs
}

#[test]
fn build_is_byte_identical_to_the_legacy_golden() {
    let m = manifest();
    let vault = fixture_copy();
    let out = tempfile::tempdir().expect("out dir");
    let status = ontomark()
        .arg("--repo")
        .arg(vault.path())
        .args(["--json", "build", "--vault", &m["vault_kind"], "--stats"])
        .arg("--out")
        .arg(out.path().join("out"))
        .arg("--publish-out")
        .arg(out.path().join("publish"))
        .arg("--with-markdown-mirror")
        .args(["--generated-at", &m["generated_at"]])
        .args(["--source-commit", &m["snapshot_commit"]])
        .output()
        .expect("run ontomark build");
    assert!(
        status.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let expected = golden().join("expected");
    let mut diffs = tree_diff("out", &expected.join("out"), &out.path().join("out"));
    diffs.extend(tree_diff(
        "publish",
        &expected.join("publish"),
        &out.path().join("publish"),
    ));
    assert!(
        diffs.is_empty(),
        "{} difference(s):\n{}",
        diffs.len(),
        diffs.join("\n")
    );
}

#[test]
fn verbs_print_the_same_bytes_and_exit_codes() {
    let vault = fixture_copy();
    let table = std::fs::read_to_string(golden().join("verbs.tsv")).expect("verbs.tsv");
    let mut diffs = Vec::new();
    let mut checked = 0;
    for line in table.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        let (name, code, command) = (cols[0], cols[1], cols[3]);
        let args: Vec<&str> = command.split_whitespace().collect();
        let output = ontomark()
            .current_dir(vault.path())
            .args(&args)
            .output()
            .expect("run verb");
        let want = std::fs::read(golden().join("expected/verbs").join(name)).expect("golden verb");
        let got_code = output.status.code().unwrap_or(-1).to_string();
        if got_code != code {
            diffs.push(format!("{name}: exit {got_code}, golden {code}"));
        }
        if output.stdout != want {
            diffs.push(format!("{name}: stdout differs"));
        }
        checked += 1;
    }
    assert_eq!(checked, 12, "the golden records twelve verb outputs");
    assert!(diffs.is_empty(), "{}", diffs.join("\n"));
}
