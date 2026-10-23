//! Reproductions for the SEV-1 anomalies in the Phase 2 register
//! (`VisionFlow/docs/diagrams/ontomark/00-anomaly-register-and-reconciliation.md`,
//! OM-00.2 and OM-00.3). Written red against ffe06e4; green since 8d654d1, when the product was
//! fixed; the failure message names the register item. The CLI-side S1.5 lives
//! in `ontomark-cli/tests/register_sev1_cli.rs`.

#![cfg(feature = "whelk-el")]

use std::path::{Path, PathBuf};

use ontomark_compile::build;
use ontomark_compile::check::validate;
use ontomark_compile::input::page::{Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::model::Corpus;
use ontomark_compile::profile::Profile;
use ontomark_compile::whelk::WhelkEl;

const VOCABULARY: &str = r#"version: 1
namespace: "urn:ngm:class:"
types:
  Class: { owl: "owl:Class", required: [resource, status] }
relations:
  is-a: { owl: "rdfs:subClassOf" }
"#;

/// A synthetic OpenAI-shaped key. Not a real credential.
const SYNTHETIC_KEY: &str = "sk-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn profile() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml"))
        .expect("estate profile")
}

fn vocab() -> Vocabulary {
    Vocabulary::from_yaml_str(VOCABULARY).expect("vocabulary")
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A `Class` page. `extra` is spliced into the frontmatter, `body` follows it.
fn class(public: &str, resource: &str, extra: &str, body: &str) -> String {
    format!(
        "---\ntype: Class\npublic: {public}\nstatus: stable\nresource: urn:ngm:class:{resource}\n{extra}---\n{body}\n"
    )
}

/// A scratch repo (`<tmp>/repo/knowledge`) and its output directory.
fn repo() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let repo = scratch.path().join("repo");
    write(&repo.join("ontology/vocabulary.yaml"), VOCABULARY);
    let out = scratch.path().join("out");
    (scratch, repo, out)
}

fn options(repo: &Path, out: &Path) -> build::Options {
    build::Options {
        vault_root: repo.join("knowledge"),
        out: out.to_path_buf(),
        repo_root: repo.to_path_buf(),
        with_markdown_mirror: true,
        generated_at: Some("2026-09-22T00:00:00+00:00".to_owned()),
        source: Some(("0000000000000000000000000000000000000000".to_owned(), false)),
        with_working_publish: false,
        redact_private_links: None,
        publish_out: None,
        stale_after: None,
        scope: build::Scope::Public,
        certificate_out: None,
    }
}

fn run_build(repo: &Path, out: &Path) -> anyhow::Result<build::Report> {
    build::run(&options(repo, out), &profile(), &vocab(), &WhelkEl)
}

/// Validate the knowledge vault of `repo` exactly as `ontomark validate` does.
fn run_validate(repo: &Path) -> validate::Report {
    let vault = Vault::load(repo.join("knowledge"), VaultKind::Knowledge).expect("load the vault");
    let corpus = Corpus::build(&vault, &vocab());
    validate::validate(&profile(), &vault, &corpus, &vocab())
}

/// Control: the scaffold used below builds cleanly, so every failure further
/// down is the register item and not a fixture mistake.
#[test]
fn control_clean_vault_builds_and_validates() {
    let (_scratch, repo, out) = repo();
    let pages = repo.join("knowledge");
    write(
        &pages.join("Alpha.md"),
        &class("true", "alpha", "", "Alpha."),
    );
    write(&pages.join("Beta.md"), &class("false", "beta", "", "Beta."));
    run_build(&repo, &out).expect("a clean vault builds");
    let report = run_validate(&repo);
    let codes: Vec<String> = report.errors().iter().map(|i| i.to_string()).collect();
    assert!(
        codes.is_empty(),
        "control vault has validation errors: {codes:?}"
    );
}

/// S1.1 — the prose redactor must fail CLOSED.
///
/// Reproduced `ir/projection.rs:232` (`Regex::new(..).ok()` left `pattern: None`)
/// and `:243-245` (`redact` then returned the text unchanged). The limit is
/// unreachable through `build::run` at any feasible vault size (measured: 100k
/// titles still compile, literals take the aho-corasick path), so the refusal
/// is driven through the redactor's own API with an injected size limit: the
/// build path calls exactly this constructor and propagates its error as
/// `PublicationBlocked::Redactor`.
#[test]
fn s1_1_redactor_refuses_rather_than_fails_open() {
    use ontomark_compile::ir::projection::{ProseRedactor, PublicationBlocked};
    use std::collections::HashSet;
    let markers: HashSet<String> = (0..256)
        .map(|i| format!("Confidential Programme Codename {i:05}"))
        .collect();
    match ProseRedactor::with_size_limit(markers.clone(), 8) {
        Err(PublicationBlocked::Redactor(_)) => {}
        Err(other) => panic!("S1.1: refused, but not as a redactor failure: {other}"),
        Ok(_) => panic!("S1.1: an 8-byte program limit must not compile 256 markers"),
    }
    // And the default limit still redacts: fail-closed must not mean refuse-always.
    let redactor = ProseRedactor::new(markers).expect("the default limit compiles");
    assert_eq!(
        redactor.redact("about Confidential Programme Codename 00042, see"),
        "about [private], see"
    );
}

/// S1.2 — the credential scan covers the body only, but `render` publishes the
/// re-rendered frontmatter.
///
/// Reproduces `build/publish.rs:224` (`secrets::scan(&page.body)`) against
/// `build/publish.rs:164` (`page.frontmatter.render()` goes into the staged
/// bytes). A key in `sources:` reaches `publish/` unscanned.
#[test]
fn s1_2_build_refuses_a_credential_in_published_frontmatter() {
    let (_scratch, repo, out) = repo();
    let pages = repo.join("knowledge");
    write(
        &pages.join("Leaky.md"),
        &class(
            "true",
            "leaky",
            &format!("sources:\n- {SYNTHETIC_KEY}\n"),
            "A clean body.",
        ),
    );
    let error = match run_build(&repo, &out) {
        Ok(_) => panic!(
            "S1.2: build accepted a credential in frontmatter `sources:` \
             (build/publish.rs:224 scans the body only)"
        ),
        Err(e) => e,
    };
    assert!(
        error
            .downcast_ref::<build::publish::SecretsFound>()
            .is_some(),
        "S1.2: build refused, but not with the credential error: {error:#}"
    );
}

/// S1.2, validate twin: `validate` scans `page.body` only
/// (`check/validate.rs:305`), so the same frontmatter credential is not
/// reported as `SECRET_DETECTED`.
#[test]
fn s1_2_validate_reports_a_credential_in_frontmatter() {
    let (_scratch, repo, _out) = repo();
    let pages = repo.join("knowledge");
    write(
        &pages.join("Leaky.md"),
        &class(
            "true",
            "leaky",
            &format!("sources:\n- {SYNTHETIC_KEY}\n"),
            "A clean body.",
        ),
    );
    let report = run_validate(&repo);
    assert!(
        report.errors().iter().any(|i| i.code == "SECRET_DETECTED"),
        "S1.2: validate did not report SECRET_DETECTED for a frontmatter credential \
         (check/validate.rs:305 scans the body only); errors: {:?}",
        report
            .errors()
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
    );
}

/// S1.3 — `public: "true"` (a quoted string) is accepted by validate and
/// refused by the build census.
///
/// Reproduces `input/page.rs:211-213` (lenient `boolean("public")`) against
/// `ir/projection.rs:110-116` (strict: only a YAML bool counts) and
/// `build/mod.rs:309`. The strict rule is canonical, so both must refuse.
#[test]
fn s1_3_validate_and_build_agree_on_a_string_public_flag() {
    let (_scratch, repo, out) = repo();
    let pages = repo.join("knowledge");
    write(
        &pages.join("Quoted.md"),
        &class("\"true\"", "quoted", "", "Quoted."),
    );

    let build_refuses = run_build(&repo, &out).is_err();
    let validate_report = run_validate(&repo);
    let validate_refuses = !validate_report.errors().is_empty();

    assert!(
        build_refuses,
        "control: the build census must refuse `public: \"true\"`"
    );
    assert_eq!(
        validate_refuses, build_refuses,
        "S1.3: validate and build disagree on `public: \"true\"` \
         (validate refuses: {validate_refuses}, build refuses: {build_refuses}; \
         input/page.rs:211-213 vs ir/projection.rs:110-116)"
    );
}

/// S1.4 — contract C3 names `data/ontology-corpus.records.jsonl`, but no code
/// in `ontomark-compile` writes it.
///
/// Reproduces `sovereign-corpus-contracts.md:115` against
/// `build/mod.rs:48-82` (no `Options` field, no emitter). The test reads the
/// filename from the contract text and greps the crate's `src/` for it, so the
/// gap is an executable check that turns green when an emitter lands.
#[test]
fn s1_4_c3_records_jsonl_has_an_emitter() {
    let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../VisionFlow/docs/engineering/sovereign-corpus-contracts.md");
    // The contract lives in the VisionFlow checkout beside this one. A lone
    // checkout (CI) has no sibling, and that is not a defect in this crate.
    let Ok(text) = std::fs::read_to_string(&contract) else {
        eprintln!(
            "skipping: no sibling VisionFlow checkout at {}",
            contract.display()
        );
        return;
    };
    let name = text
        .split_whitespace()
        .find(|w| w.ends_with(".records.jsonl"))
        .map(|w| w.rsplit('/').next().unwrap().to_owned())
        .expect("the contract names a *.records.jsonl file");
    assert_eq!(
        name, "ontology-corpus.records.jsonl",
        "contract filename moved"
    );

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let emitters: Vec<String> = walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .filter(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t.contains(&name)))
        .map(|e| e.path().display().to_string())
        .collect();
    assert!(
        !emitters.is_empty(),
        "S1.4: contract C3 (sovereign-corpus-contracts.md:115) names `data/{name}` \
         but nothing under ontomark-compile/src writes it (build/mod.rs:48-82)"
    );
}
