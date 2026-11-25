//! Vocabulary version 3 through the real binary, over a small fixture vault
//! (`tests/vocab-v3-vault`) built to trip every code the v3 rules add.
//!
//! The expected list below is explicit, so a new code that no page exercises,
//! or a code that stops firing, fails here. The same vault under a version 2
//! vocabulary must produce none of them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Every code vocabulary version 3 adds, by stage.
const V3_CODES: &[&str] = &[
    // S0
    "INVALID_WORKING_ENUM",
    // S1
    "INVALID_STAMP",
    "NON_CANONICAL_PRINCIPAL",
    "VERIFIED_NOT_HUMAN",
    "VERIFIED_PRINCIPAL_UNKNOWN",
    // S2
    "INVALID_DATETIME",
    "VALID_RANGE",
    "VALID_TO_STATUS",
    "INVALID_SUPERSEDES",
    "DANGLING_SUPERSESSION",
    "SUPERSESSION_CLOCK",
    "DEPRECATED_NO_SUCCESSOR",
    "SUPERSESSION_CYCLE",
    "SUPERSEDED_TARGET",
    "INVALID_STATEMENT",
    "STATEMENT_MISMATCH",
    "UNKNOWN_SOURCE_ID",
    // S3
    "INVALID_SOURCE",
    "INVALID_SOURCE_RESOURCE",
    "DANGLING_SOURCE",
    "BRIEF_AS_EVIDENCE",
    "MISSING_LOCATOR",
    "DANGLING_LOCATOR",
    "INVALID_PIN",
    "STALE_PIN",
    "MISSING_KIND_KEY",
    "MISSING_EVIDENCE",
    "MISSING_ORIGIN",
    "INVALID_KIND_VALUE",
    // S4
    "MISSING_WORKING_KEY",
    "INVALID_VISIBILITY",
    "PRIVATE_UNSUPPORTED",
    "INVALID_PAGE_RESOURCE",
    "DUPLICATE_PAGE_RESOURCE",
    "MEMORY_STATUS",
    "INVALID_TYPE_STATUS",
    "SUPERSEDED_BY_MISMATCH",
    "WITHDRAWAL_MISMATCH",
    "DISPUTE_NOT_DRAFT",
    "DISPUTE_TARGET",
    "EVIDENCE_NOT_APPROVED",
    "INVALID_ABOUT",
    "UNRESOLVED_ABOUT",
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vocab-v3-vault")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The codes `validate --vault all --json` reports for `repo`, and its exit code.
fn codes_of(repo: &Path) -> (BTreeSet<String>, Option<i32>) {
    let out = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(repo)
        .args(["validate", "--vault", "all", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stderr)));
    let mut codes = BTreeSet::new();
    for vault in json.as_object().unwrap().values() {
        for issue in vault["issues"].as_array().unwrap() {
            codes.insert(issue["code"].as_str().unwrap().to_owned());
        }
    }
    (codes, out.status.code())
}

#[test]
fn the_fixture_vault_exercises_every_v3_code() {
    let (codes, exit) = codes_of(&fixture());
    let expected: BTreeSet<String> = V3_CODES.iter().map(|c| (*c).to_owned()).collect();
    let missing: Vec<_> = expected.difference(&codes).collect();
    assert!(
        missing.is_empty(),
        "codes no fixture page trips: {missing:?}"
    );
    assert_eq!(exit, Some(1), "version 3 errors fail validation");
}

#[test]
fn the_same_vault_under_version_2_reports_none_of_them() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    copy_tree(&fixture(), &repo);
    let vocab = repo.join("ontology/vocabulary.yaml");
    let text = std::fs::read_to_string(&vocab).unwrap();
    std::fs::write(&vocab, text.replacen("version: 3", "version: 2", 1)).unwrap();
    let (codes, _) = codes_of(&repo);
    // Version 2 warns about unreadable stamps; that is the one code it shares.
    let leaked: Vec<_> = V3_CODES
        .iter()
        .filter(|c| **c != "INVALID_STAMP" && codes.contains(**c))
        .collect();
    assert!(leaked.is_empty(), "v3 codes under version 2: {leaked:?}");
}

/// B1 and S12 on the fixture: the swarm key does not certify, the operator's own
/// key does, and an empty `generated:` is an error on a knowledge page.
#[test]
fn an_instrument_key_does_not_certify_and_an_empty_stamp_is_an_error() {
    let out = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(fixture())
        .args(["validate", "--vault", "knowledge", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let on = |page: &str, code: &str| {
        json["knowledge"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["path"].as_str().is_some_and(|p| p.contains(page)) && i["code"] == code)
    };
    assert!(on("SwarmVerified", "VERIFIED_NOT_HUMAN"));
    assert!(!on("OperatorVerified", "VERIFIED_NOT_HUMAN"));
    assert!(on("EmptyStamp", "INVALID_STAMP"));
}

/// Item 1 on the fixture: under vocabulary 3 the old `human:<npub>` spelling is
/// an `INVALID_STAMP` error and no longer certifies (the stable page then has
/// no human entry: `UNVERIFIED_STABLE`), while the operator's own key still
/// does. Under vocabulary 2 the prefix rule is as released: no error, and it
/// certifies.
#[test]
fn the_old_human_spelling_does_not_certify_under_version_3() {
    let run = |repo: &Path| {
        let out = Command::new(env!("CARGO_BIN_EXE_ontomark"))
            .arg("--repo")
            .arg(repo)
            .args(["validate", "--vault", "knowledge", "--json"])
            .output()
            .unwrap();
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()
    };
    let on = |json: &serde_json::Value, page: &str, code: &str| {
        json["knowledge"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["path"].as_str().is_some_and(|p| p.contains(page)) && i["code"] == code)
    };
    let v3 = run(&fixture());
    assert!(on(&v3, "HumanPrefixVerified", "INVALID_STAMP"));
    assert!(on(&v3, "HumanPrefixVerified", "UNVERIFIED_STABLE"));
    assert!(!on(&v3, "OperatorVerified", "UNVERIFIED_STABLE"));
    assert!(on(&v3, "SwarmVerified", "UNVERIFIED_STABLE"));

    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    copy_tree(&fixture(), &repo);
    let vocab = repo.join("ontology/vocabulary.yaml");
    let text = std::fs::read_to_string(&vocab).unwrap();
    std::fs::write(&vocab, text.replacen("version: 3", "version: 2", 1)).unwrap();
    let v2 = run(&repo);
    assert!(!on(&v2, "HumanPrefixVerified", "INVALID_STAMP"));
    assert!(
        !on(&v2, "HumanPrefixVerified", "UNVERIFIED_STABLE"),
        "under version 2 the human: prefix still certifies"
    );
}
