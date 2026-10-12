//! Offline conformance: `ontomark` against the legacy golden for the
//! `estate-small` fixture (`conformance/fixtures/estate-small.golden`).
//!
//! Byte parity holds for every artefact except the two generation markers,
//! which are compared structurally (see `marker_diff`): their `content_digest`
//! and `artifacts` fields changed by decision on 2026-10-12 (OM-00 S2.2, S2.5).
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
///
/// Every artefact is compared byte for byte except the two generation markers,
/// which are compared structurally ([`marker_diff`]): since 2026-10-12 ontomark
/// defines `content_digest` over every byte the build read (OM-00 S2.2) and
/// lists the data marker and `--publish-out` files among the artefacts (S2.5),
/// so those two fields deliberately differ from the legacy `vault`. Everything
/// else in the marker, and every legacy artefact's hash, must still agree.
fn tree_diff(label: &str, want: &Path, got: &Path) -> Vec<String> {
    let (want, got) = (files(want), files(got));
    let mut diffs = Vec::new();
    for (path, bytes) in &want {
        match got.get(path) {
            None => diffs.push(format!("{label}/{path}: missing")),
            Some(b) if path.ends_with(".generation.json") => {
                diffs.extend(marker_diff(&format!("{label}/{path}"), bytes, b));
            }
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

/// The legacy and current markers must agree on identity (`id`, `commit`,
/// `dirty`, `ontology_digest`, `generated_at`, the counts, the vocabulary
/// version, `stale_after`), and every artefact the legacy marker lists must
/// appear in the current one with the same hash and size. `content_digest` is a
/// different digest by definition (S2.2) and the current list may be longer
/// (S2.5); both are reported only when they fail to be exactly that.
fn marker_diff(label: &str, want: &[u8], got: &[u8]) -> Vec<String> {
    let parse = |b: &[u8]| -> serde_json::Value {
        serde_json::from_slice(b).expect("a generation marker is JSON")
    };
    let (mut want, mut got) = (parse(want), parse(got));
    let mut diffs = Vec::new();
    let take = |v: &mut serde_json::Value, key: &str| v.as_object_mut().and_then(|o| o.remove(key));
    let want_digest = take(&mut want, "content_digest");
    let got_digest = take(&mut got, "content_digest");
    if want_digest.is_none() || got_digest.is_none() {
        diffs.push(format!("{label}: a marker has no content_digest"));
    }
    let want_artifacts = take(&mut want, "artifacts").unwrap_or_default();
    let got_artifacts = take(&mut got, "artifacts").unwrap_or_default();
    if want != got {
        diffs.push(format!(
            "{label}: identity fields differ\n  legacy: {want}\n  current: {got}"
        ));
    }
    let got_set: std::collections::BTreeSet<String> = got_artifacts
        .as_array()
        .map(|a| a.iter().map(ToString::to_string).collect())
        .unwrap_or_default();
    for artifact in want_artifacts.as_array().into_iter().flatten() {
        if !got_set.contains(&artifact.to_string()) {
            diffs.push(format!(
                "{label}: legacy artefact missing or changed in the current marker: {artifact}"
            ));
        }
    }
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
        // The legacy build keeps links to non-public pages; redaction is
        // ontomark's own behaviour (EXP-04) and is tested in exp04_private_canary.
        .arg("--no-redact-private-links")
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
