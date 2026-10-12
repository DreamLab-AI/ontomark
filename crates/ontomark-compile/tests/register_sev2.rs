//! Reproduction tests for the SEV-2 anomalies of the ontomark register
//! (`VisionFlow/docs/diagrams/ontomark/00-anomaly-register-and-reconciliation.md`,
//! OM-00.2 / OM-00.3), S2.1, S2.2, S2.3 and S2.5. (S2.4 is CLI-level and lives in
//! `ontomark-cli/tests/register_sev2_cli.rs`.)
//!
//! Written red against ffe06e4 and green since 8d654d1: each asserts the corrected
//! behaviour and fails for the register's stated reason. No product code is
//! changed by this file.

use std::path::{Path, PathBuf};

use ontomark_compile::build;
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::profile::Profile;
use ontomark_compile::whelk::WhelkEl;

fn profile() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml"))
        .expect("estate profile")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/estate-small")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create destination");
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.expect("walk the fixture");
        let rel = entry.path().strip_prefix(from).expect("relative path");
        let target = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).expect("create directory");
        } else {
            std::fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// A scratch copy of the estate-small fixture: `(scratch, repo)`.
fn fixture_repo() -> (tempfile::TempDir, PathBuf) {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let repo = scratch.path().join("repo");
    copy_tree(&fixture(), &repo);
    (scratch, repo)
}

fn options(repo: &Path, out: &Path) -> build::Options {
    build::Options {
        vault_root: repo.join("knowledge"),
        out: out.to_path_buf(),
        repo_root: repo.to_path_buf(),
        with_markdown_mirror: false,
        generated_at: Some("2026-09-22T00:00:00+00:00".to_owned()),
        source: Some(("0000000000000000000000000000000000000000".to_owned(), false)),
        with_working_publish: false,
        redact_private_links: None,
        publish_out: None,
        stale_after: None,
        scope: build::Scope::Public,
    }
}

fn run(repo: &Path, opts: &build::Options) -> anyhow::Result<build::Report> {
    let vocab = Vocabulary::load(repo.join("ontology/vocabulary.yaml")).expect("vocabulary");
    build::run(opts, &profile(), &vocab, &WhelkEl)
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// S2.1 — `build/mod.rs:395-399` (`anyhow::ensure!` on `is_consistent`).
///
/// `P ⊑ A`, `P ⊑ B`, `A disjoint-with B` (siblings under `Parent`) makes `P`
/// unsatisfiable. Today the refusal is a bare `anyhow` message string, so the
/// CLI cannot tell it from any other failure and exits 1. The fix must return
/// a typed, downcastable error (the exit-2 mapping itself needs an ADR; the
/// typed error is the testable half). The typed error's name is not yet
/// decided, so this asserts the *negative*: the error must not be a bare
/// string message. That is the current, wrong behaviour the fix must flip.
#[test]
fn s2_1_unsatisfiable_corpus_is_a_typed_error_not_a_bare_string() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    write(
        &repo.join("ontology/vocabulary.yaml"),
        r#"version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
  disjoint-with: { owl: "owl:disjointWith" }
"#,
    );
    let class = |id: &str, extra: &str| {
        format!(
            "---\ntype: Class\npublic: true\nstatus: stable\nresource: urn:ngm:class:{id}\n{extra}---\nA page.\n"
        )
    };
    let pages = repo.join("knowledge/pages");
    write(&pages.join("Parent.md"), &class("parent", ""));
    write(
        &pages.join("A.md"),
        &class("a", "is-a: [\"[[Parent]]\"]\ndisjoint-with: [\"[[B]]\"]\n"),
    );
    write(&pages.join("B.md"), &class("b", "is-a: [\"[[Parent]]\"]\n"));
    write(
        &pages.join("P.md"),
        &class("p", "is-a: [\"[[A]]\", \"[[B]]\"]\n"),
    );

    let out = scratch.path().join("out");
    let error = run(&repo, &options(&repo, &out)).expect_err("an unsatisfiable corpus must refuse");

    // Fixture sanity: it failed for the unsatisfiable reason, not another.
    assert!(
        format!("{error:#}").contains("UNSATISFIABLE_CLASSES"),
        "fixture did not produce the unsatisfiable refusal: {error:#}"
    );
    assert!(!out.exists(), "nothing may be written");
    // Not a ReasoningError either: that is the cap/profile refusal family.
    assert!(
        error
            .downcast_ref::<ontomark_compile::reason::ReasoningError>()
            .is_none(),
        "unexpectedly a ReasoningError: {error:#}"
    );
    // The register's complaint: today this is `anyhow::ensure!` with a
    // formatted message, i.e. a bare `String` payload nobody can downcast to
    // a domain type. A typed variant is not a `String`.
    assert!(
        error.downcast_ref::<String>().is_none(),
        "S2.1: the unsatisfiable refusal is a bare anyhow string, not a typed error: {error:#}"
    );
}

/// S2.2(a) — `build/mod.rs:578-586`: the digest covers only the knowledge
/// vault's pages. With `with_working_publish`, a changed `working/` page is
/// published content (the C3 surface) but leaves `content_digest` unchanged.
#[test]
fn s2_2a_content_digest_covers_published_working_pages() {
    let digest_with_working_page = |body: &str| {
        let (scratch, repo) = fixture_repo();
        write(
            &repo.join("working/pages/podcast-evidence/Episode One.md"),
            &format!("---\ntype: Note\nstatus: draft\npublic: true\n---\n{body}\n"),
        );
        let mut opts = options(&repo, &scratch.path().join("out"));
        opts.with_working_publish = true;
        run(&repo, &opts).expect("builds").generation.content_digest
    };
    let first = digest_with_working_page("Working note, first revision.");
    let second = digest_with_working_page("Working note, second revision.");
    assert_ne!(
        first, second,
        "S2.2: a changed published working/ page did not change content_digest"
    );
}

/// S2.2(b) — `build/mod.rs:578-586`: `ontology/vocabulary.yaml` governs the
/// whole projection (validation, the Turtle) yet is not an input to
/// `content_digest`, so a vocabulary change with identical pages is invisible.
#[test]
fn s2_2b_content_digest_covers_the_vocabulary() {
    let digest = |extra_relation: bool| {
        let (scratch, repo) = fixture_repo();
        if extra_relation {
            let path = repo.join("ontology/vocabulary.yaml");
            let text = std::fs::read_to_string(&path).unwrap();
            let marker = "  # --- logical, provisional (ADR-2125)";
            assert!(text.contains(marker), "fixture vocabulary changed shape");
            let patched = text.replacen(
                marker,
                "  s2-probe: { owl: vc:s2Probe, emitted: false, status: provisional }\n\n  # --- logical, provisional (ADR-2125)",
                1,
            );
            std::fs::write(&path, patched).unwrap();
        }
        run(&repo, &options(&repo, &scratch.path().join("out")))
            .expect("builds with the vocabulary change")
            .generation
            .content_digest
    };
    assert_ne!(
        digest(false),
        digest(true),
        "S2.2: a vocabulary change with identical pages did not change content_digest"
    );
}

// S2.2(c) is deliberately not written: the digest re-reads each page with
// `unwrap_or_default()` (an unreadable page hashes as empty bytes), but making
// a file unreadable between load and digest cannot be timed deterministically.

/// A directory restored to writable on drop so the tempdir can be removed.
struct Unlock(PathBuf);
impl Drop for Unlock {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// S2.3 — `build/mod.rs:241-246` and `build/publish.rs:316-317`: `write_to`
/// does `remove_dir_all(dir)` BEFORE writing, after the bundle is already
/// promoted. A failure in that window destroys the previous `--publish-out`
/// and leaves no new one. Failure is injected by making the parent of
/// `publish_out` read-only: the children are removed, then removing the
/// directory itself fails. An atomic promote (stage, then swap) would leave
/// the previous publication untouched.
#[test]
fn s2_3_failed_publish_out_replacement_keeps_the_previous_publication() {
    use std::os::unix::fs::PermissionsExt;
    let (scratch, repo) = fixture_repo();
    let holder = scratch.path().join("holder");
    std::fs::create_dir_all(&holder).unwrap();
    let published = holder.join("published");

    let mut first = options(&repo, &scratch.path().join("out1"));
    first.publish_out = Some(published.clone());
    run(&repo, &first).expect("the first build publishes");
    let index = published.join("index.md");
    assert!(
        index.is_file(),
        "precondition: the first publication landed"
    );
    let previous = std::fs::read(&index).unwrap();

    let _unlock = Unlock(holder.clone());
    std::fs::set_permissions(&holder, std::fs::Permissions::from_mode(0o555)).unwrap();
    // Running as root ignores the mode, so the injection cannot bite.
    if std::fs::create_dir(holder.join("probe")).is_ok() {
        eprintln!("skipping the S2.3 injection: permissions are not enforced (root?)");
        return;
    }

    let mut second = options(&repo, &scratch.path().join("out2"));
    second.publish_out = Some(published.clone());
    let result = run(&repo, &second);
    assert!(result.is_err(), "the injected failure should surface");

    assert!(
        index.is_file(),
        "S2.3: the failed replacement destroyed the previous publish_out (remove_dir_all ran first)"
    );
    assert_eq!(std::fs::read(&index).unwrap(), previous);
}

/// S2.5(a) — `build/mod.rs:602-608`: `staged.artifacts()` is taken before the
/// two generation markers are added, so the markers are not in the artefact
/// list they stamp.
#[test]
fn s2_5a_generation_markers_are_listed_among_the_artifacts() {
    let (scratch, repo) = fixture_repo();
    let report = run(&repo, &options(&repo, &scratch.path().join("out"))).expect("builds");
    let names: Vec<&str> = report
        .generation
        .artifacts
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    // A marker cannot list its own hash, so the rule is: the root marker lists
    // the data marker (real hash), and no marker lists itself. The register's
    // "both markers" wording was corrected to this at fix time.
    let marker = "data/.generation.json";
    assert!(
        names.contains(&marker),
        "S2.5: {marker} is not in the artefact list it stamps"
    );
    assert!(
        !names.contains(&".generation.json"),
        "S2.5: the root marker must not list itself"
    );
}

/// S2.5(b) — `build/mod.rs:411`: when `publish_out` is set the `publish/`
/// files are not staged into the bundle, so they are never hashed into the
/// artefact list even though they are shipped.
#[test]
fn s2_5b_publish_files_are_hashed_when_publish_out_is_set() {
    let (scratch, repo) = fixture_repo();
    let mut opts = options(&repo, &scratch.path().join("out"));
    opts.publish_out = Some(scratch.path().join("published"));
    let report = run(&repo, &opts).expect("builds");
    assert!(
        report
            .generation
            .artifacts
            .iter()
            .any(|a| a.name.starts_with("publish/")),
        "S2.5: with publish_out set, no publish/ file is hashed into the artefact list"
    );
}
