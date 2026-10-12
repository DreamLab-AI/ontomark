//! Reproductions for the SEV-1 anomalies in the Phase 2 register
//! (`VisionFlow/docs/diagrams/ontomark/00-anomaly-register-and-reconciliation.md`,
//! OM-00.2 and OM-00.3). Each test is expected to FAIL until the product is
//! fixed; the failure message names the register item. The CLI-side S1.5 lives
//! in `ontomark-cli/tests/register_sev1_cli.rs`.

use std::path::{Path, PathBuf};

use ontomark_compile::build;
use ontomark_compile::check::validate;
use ontomark_compile::input::page::load_vault;
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

/// A scratch repo (`<tmp>/repo/knowledge/pages`) and its output directory.
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
    }
}

fn run_build(repo: &Path, out: &Path) -> anyhow::Result<build::Report> {
    build::run(&options(repo, out), &profile(), &vocab(), &WhelkEl)
}

/// Validate the knowledge vault of `repo` exactly as `ontomark validate` does.
fn run_validate(repo: &Path) -> validate::Report {
    let vault = load_vault(repo.join("knowledge")).expect("load the vault");
    let corpus = Corpus::build(&vault, &vocab());
    validate::validate(&profile(), &vault, &corpus, &vocab())
}

/// Control: the scaffold used below builds cleanly, so every failure further
/// down is the register item and not a fixture mistake.
#[test]
fn control_clean_vault_builds_and_validates() {
    let (_scratch, repo, out) = repo();
    let pages = repo.join("knowledge/pages");
    write(&pages.join("Alpha.md"), &class("true", "alpha", "", "Alpha."));
    write(&pages.join("Beta.md"), &class("false", "beta", "", "Beta."));
    run_build(&repo, &out).expect("a clean vault builds");
    let report = run_validate(&repo);
    let codes: Vec<String> = report.errors().iter().map(|i| i.to_string()).collect();
    assert!(codes.is_empty(), "control vault has validation errors: {codes:?}");
}

/// S1.1 — the prose redactor fails OPEN.
///
/// Reproduces `ir/projection.rs:232` (`Regex::new(..).ok()` leaves
/// `pattern: None`) and `ir/projection.rs:243-245` (`redact` then returns the
/// text unchanged). With enough private titles the escaped alternation exceeds
/// the regex size limit; the build must refuse instead of publishing prose
/// that still names private pages.
///
/// `Redactor` is private and `Regex::new` offers no hook, so the test drives
/// the real path (`build::run`) and first proves, with the same alternation,
/// that the regex really fails to compile. That precondition does not hold at
/// any feasible vault size today (hence `#[ignore]`); once the redactor can be
/// given a size limit, lower `PRIVATE`/title length to match and un-ignore.
#[test]
#[ignore = "needs ProseRedactor::new to be fallible or take a size limit: Redactor is private, \
Regex::new exposes no hook, and measured escaped-literal alternations of 100k titles x ~130 bytes \
still compile (literals take the aho-corasick path), so the limit is unreachable via build::run"]
fn s1_1_redactor_fails_open_when_the_alternation_exceeds_the_regex_limit() {
    let (_scratch, repo, out) = repo();
    let pages = repo.join("knowledge/pages");
    const PRIVATE: usize = 4000;
    // Long, distinct, word-shaped titles: the alternation is their escaped union.
    let titles: Vec<String> = (0..PRIVATE)
        .map(|i| format!("Confidential Programme Codename {i:05} {}", "x".repeat(900)))
        .collect();
    for (i, title) in titles.iter().enumerate() {
        write(
            &pages.join(format!("private-{i:05}.md")),
            &class("false", &format!("private-{i:05}"), &format!("title: {title}\n"), "Private."),
        );
    }
    write(
        &pages.join("Public.md"),
        &class("true", "public", "", &format!("This page mentions {}.", titles[0])),
    );

    // Precondition: the alternation the redactor builds really does not compile.
    let mut sorted: Vec<&String> = titles.iter().collect();
    sorted.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    let alternation: Vec<String> = sorted.iter().map(|m| regex::escape(m)).collect();
    assert!(
        regex::Regex::new(&alternation.join("|")).is_err(),
        "fixture too small: the alternation compiles, so S1.1 is not reached"
    );

    let result = run_build(&repo, &out);
    assert!(
        result.is_err(),
        "S1.1: build succeeded although the redactor could not compile; \
         unredacted prose was emitted (ir/projection.rs:232,243-245)"
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
    let pages = repo.join("knowledge/pages");
    write(
        &pages.join("Leaky.md"),
        &class("true", "leaky", &format!("sources:\n- {SYNTHETIC_KEY}\n"), "A clean body."),
    );
    let error = match run_build(&repo, &out) {
        Ok(_) => panic!(
            "S1.2: build accepted a credential in frontmatter `sources:` \
             (build/publish.rs:224 scans the body only)"
        ),
        Err(e) => e,
    };
    assert!(
        error.downcast_ref::<build::publish::SecretsFound>().is_some(),
        "S1.2: build refused, but not with the credential error: {error:#}"
    );
}

/// S1.2, validate twin: `validate` scans `page.body` only
/// (`check/validate.rs:305`), so the same frontmatter credential is not
/// reported as `SECRET_DETECTED`.
#[test]
fn s1_2_validate_reports_a_credential_in_frontmatter() {
    let (_scratch, repo, _out) = repo();
    let pages = repo.join("knowledge/pages");
    write(
        &pages.join("Leaky.md"),
        &class("true", "leaky", &format!("sources:\n- {SYNTHETIC_KEY}\n"), "A clean body."),
    );
    let report = run_validate(&repo);
    assert!(
        report.errors().iter().any(|i| i.code == "SECRET_DETECTED"),
        "S1.2: validate did not report SECRET_DETECTED for a frontmatter credential \
         (check/validate.rs:305 scans the body only); errors: {:?}",
        report.errors().iter().map(|i| i.to_string()).collect::<Vec<_>>()
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
    let pages = repo.join("knowledge/pages");
    write(&pages.join("Quoted.md"), &class("\"true\"", "quoted", "", "Quoted."));

    let build_refuses = run_build(&repo, &out).is_err();
    let validate_report = run_validate(&repo);
    let validate_refuses = !validate_report.errors().is_empty();

    assert!(build_refuses, "control: the build census must refuse `public: \"true\"`");
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
    let text = std::fs::read_to_string(&contract)
        .unwrap_or_else(|e| panic!("cannot read the contract {}: {e}", contract.display()));
    let name = text
        .split_whitespace()
        .find(|w| w.ends_with(".records.jsonl"))
        .map(|w| w.rsplit('/').next().unwrap().to_owned())
        .expect("the contract names a *.records.jsonl file");
    assert_eq!(name, "ontology-corpus.records.jsonl", "contract filename moved");

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
