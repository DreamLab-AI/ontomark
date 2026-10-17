//! Every integration test that reaches for the optional classifier
//! (`ontomark_compile::whelk` or `ontomark_reason`) must be declared in this
//! crate's `Cargo.toml` with `required-features = ["whelk-el"]` (or
//! `["model-projection"]`, which also pulls `ontomark-reason` in), or the
//! `--no-default-features` legs fail to compile. This test needs no feature and
//! so runs on every leg; it scans `tests/` and the manifest as plain text.

use std::collections::BTreeSet;
use std::path::Path;

/// The features whose `[[test]]` gate makes the classifier available.
const CLASSIFIER_FEATURES: [&str; 2] = ["\"whelk-el\"", "\"model-projection\""];

/// The `[[test]]` names the manifest gates behind a classifier feature.
fn gated_tests(manifest: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut name: Option<String> = None;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            name = None;
            continue;
        }
        if let Some(rest) = line.strip_prefix("name") {
            if let Some(value) = rest.trim().strip_prefix('=') {
                name = Some(value.trim().trim_matches('"').to_owned());
            }
        } else if let Some(rest) = line.strip_prefix("required-features") {
            if CLASSIFIER_FEATURES.iter().any(|f| rest.contains(f)) {
                if let Some(n) = &name {
                    out.insert(n.clone());
                }
            }
        }
    }
    out
}

/// The test files whose code (comments aside) names the classifier crate or
/// module. This file names them only in string literals and comments, and is
/// skipped by name.
fn tests_needing_classifier(dir: &Path) -> BTreeSet<String> {
    let needles = ["ontomark_compile::whelk", "ontomark_reason"];
    let mut out = BTreeSet::new();
    for entry in std::fs::read_dir(dir).expect("tests dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .into_owned();
        if stem == "feature_gates" {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read test");
        let uses = text.lines().any(|l| {
            let l = l.trim_start();
            !l.starts_with("//") && needles.iter().any(|n| l.contains(n))
        });
        if uses {
            out.insert(stem);
        }
    }
    out
}

#[test]
fn classifier_tests_are_gated_behind_whelk_el() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(crate_dir.join("Cargo.toml")).expect("Cargo.toml");
    let gated = gated_tests(&manifest);
    let needing = tests_needing_classifier(&crate_dir.join("tests"));
    let missing: Vec<&String> = needing.difference(&gated).collect();
    assert!(
        missing.is_empty(),
        "these tests use the classifier but have no `[[test]] required-features = \
         [\"whelk-el\"]` entry in crates/ontomark-compile/Cargo.toml: {missing:?}"
    );
    assert!(
        needing.contains("publish_gate") && needing.contains("vault_layout"),
        "the scan must see the known classifier tests; got {needing:?}"
    );
}

#[test]
fn the_manifest_scan_reads_gated_entries() {
    let gated = gated_tests(
        "[[test]]\nname = \"a\"\nrequired-features = [\"whelk-el\"]\n\n[[test]]\nname = \"b\"\n\
         required-features = [\"model-projection\"]\n\n[[test]]\nname = \"c\"\n\
         required-features = [\"other\"]\n\n[dependencies]\nname = \"not-a-test\"\n",
    );
    assert_eq!(gated.into_iter().collect::<Vec<_>>(), ["a", "b"]);
}
