//! `ontomark build --scope machine` (ADR-OM-005 decision 4): the public
//! knowledge projection plus `working/`'s public pages as plain records, with no
//! OWL contribution, private-link redaction as for any bundle, and no Quartz
//! `publish/` tree.
//!
//! The invariants come from `docs/design/machine-bundle-brief.md`. Each test
//! builds from a scratch copy of the `estate-small` fixture.

#![cfg(feature = "whelk-el")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use ontomark_compile::build::{self, generation::Generation, Scope};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::projection::PublicationBlocked;
use ontomark_compile::profile::Profile;
use ontomark_compile::whelk::WhelkEl;
use serde_json::Value;

const PRIVATE_CANARY: &str = "PRIVATE-CANARY-NOTE-91c2";

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

fn options(repo: &Path, out: &Path, scope: Scope) -> build::Options {
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
        scope,
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

/// Build `scope` of `repo` into `<scratch>/<scope>`.
fn build_scope(
    scratch: &tempfile::TempDir,
    repo: &Path,
    scope: Scope,
) -> (PathBuf, anyhow::Result<build::Report>) {
    let out = scratch.path().join(scope.as_str());
    let report = run(repo, &options(repo, &out, scope));
    (out, report)
}

/// Every file under `dir`, relative path to bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .map(|e| e.expect("walk the bundle"))
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let rel = e
                .path()
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (rel, std::fs::read(e.path()).unwrap())
        })
        .collect()
}

fn json(out: &Path, name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(out.join(name)).unwrap()).unwrap()
}

/// A working note page: `type: Note`, a title, optional extra frontmatter.
fn note(title: &str, public: bool, extra: &str, body: &str) -> String {
    format!(
        "---\ntype: Note\ntitle: {title}\npublic: {public}\nstatus: stable\n{extra}---\n\n{body}\n"
    )
}

/// A knowledge class page, valid under the fixture's role.
fn class(title: &str, public: bool, slug: &str) -> String {
    format!(
        "---\ntype: Class\ntitle: {title}\nresource: urn:ngm:class:{slug}\npublic: {public}\nstatus: draft\ndomain: blockchain\ngenerated:\n  by: human:npub16kv2h3hdalphuyzj8tkycq8cm9n54ymer23558ds80afkc9e7frqwrawzj\n  at: 2026-10-11T00:00:00Z\n---\n\nSynthetic class {title}.\n"
    )
}

fn built(scratch: &tempfile::TempDir, repo: &Path, scope: Scope) -> (PathBuf, build::Report) {
    let (out, report) = build_scope(scratch, repo, scope);
    (out, report.expect("the build succeeds"))
}

fn blocked(error: &anyhow::Error) -> &PublicationBlocked {
    error
        .downcast_ref::<PublicationBlocked>()
        .unwrap_or_else(|| panic!("a typed PublicationBlocked, got: {error:#}"))
}

// ── the knowledge side is untouched ─────────────────────────────────────────

#[test]
fn machine_and_public_builds_share_every_ontology_artefact() {
    let (scratch, repo) = fixture_repo();
    let (public_out, public) = built(&scratch, &repo, Scope::Public);
    let (machine_out, machine) = built(&scratch, &repo, Scope::Machine);
    let (public_tree, machine_tree) = (tree(&public_out), tree(&machine_out));

    // D4: all of `data/` bar the markers, and `okf/`, in both directions, so a
    // file present in only one bundle fails too.
    let shared = |tree: &BTreeMap<String, Vec<u8>>| -> BTreeMap<String, Vec<u8>> {
        tree.iter()
            .filter(|(name, _)| {
                (name.starts_with("data/") && name.as_str() != "data/.generation.json")
                    || name.starts_with("okf/")
            })
            .map(|(name, bytes)| (name.clone(), bytes.clone()))
            .collect()
    };
    let (public_shared, machine_shared) = (shared(&public_tree), shared(&machine_tree));
    assert!(
        public_shared.len() > 8,
        "the comparison must cover the ontology artefacts, saw {:?}",
        public_shared.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        public_shared.keys().collect::<Vec<_>>(),
        machine_shared.keys().collect::<Vec<_>>(),
        "the two bundles carry the same ontology-side files"
    );
    for (name, bytes) in &public_shared {
        assert_eq!(
            Some(bytes),
            machine_shared.get(name),
            "{name} differs between scopes"
        );
    }
    assert_eq!(
        public.generation.ontology_digest, machine.generation.ontology_digest,
        "the ontology digest is a function of the asserted graph alone"
    );
    assert_eq!(
        public.generation.content_digest, machine.generation.content_digest,
        "both loaded working/, so both digests cover the same bytes"
    );
    assert_eq!(public.class_count, machine.class_count);
}

// ── the working records ─────────────────────────────────────────────────────

#[test]
fn the_public_working_note_is_a_plain_record_and_the_private_one_is_nowhere() {
    let (scratch, repo) = fixture_repo();
    let (out, report) = built(&scratch, &repo, Scope::Machine);

    let page = json(&out, "api/pages/working/fixture-public-note.json");
    assert_eq!(page["title"], "Fixture Public Note");
    assert_eq!(page["slug"], "working/fixture-public-note");
    // D2: the page IRI is the namespace plus the slug path, so no spaces.
    assert_eq!(
        page["id"],
        "urn:visionflow:page:working/fixture-public-note"
    );
    assert!(
        page.get("classIri").is_none() && page.get("entityType").is_none(),
        "a plain record carries no ontology identity: {page}"
    );

    let search = json(&out, "api/search-index.json");
    let entry = search
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "working/fixture-public-note")
        .expect("a search-index entry for the working note");
    assert_eq!(entry["title"], "Fixture Public Note");
    assert!(entry.get("iri").is_none(), "{entry}");

    assert_eq!(
        report.page_count,
        json(&out, "api/search-index.json")
            .as_array()
            .unwrap()
            .len(),
        "page_count counts the plain working records too"
    );
    let summary = report.machine.as_ref().expect("a machine summary");
    assert_eq!(summary.working_records, 1);
    assert_eq!(summary.yielded, 0);

    for (name, bytes) in tree(&out) {
        let text = String::from_utf8_lossy(&bytes);
        for needle in [
            PRIVATE_CANARY,
            "Fixture Private Note",
            "fixture-private-note",
        ] {
            assert!(!text.contains(needle), "{name} names the private note");
        }
    }
}

#[test]
fn machine_scope_writes_no_publish_tree_and_stamps_its_scope() {
    let (scratch, repo) = fixture_repo();
    let (public_out, _) = built(&scratch, &repo, Scope::Public);
    let (out, report) = built(&scratch, &repo, Scope::Machine);

    assert!(
        !out.join("publish").exists(),
        "no publish/ in machine scope"
    );
    assert!(
        public_out.join("publish").is_dir(),
        "control: public has it"
    );
    assert!(
        tree(&out).keys().all(|n| !n.starts_with("publish/")),
        "nothing publish-shaped anywhere"
    );
    assert!(
        report
            .generation
            .artifacts
            .iter()
            .all(|a| !a.name.starts_with("publish/")),
        "the marker records no publish artefact"
    );
    assert_eq!(
        report.written,
        tree(&out).len(),
        "written counts the bundle and no staging"
    );

    for marker in [".generation.json", "data/.generation.json"] {
        let doc = json(&out, marker);
        assert_eq!(doc["scope"], "machine", "{marker}");
        let parsed = Generation::from_json(&std::fs::read_to_string(out.join(marker)).unwrap())
            .expect("the marker round-trips through the typed reader");
        assert_eq!(parsed.scope, Scope::Machine);
    }
    for marker in [".generation.json", "data/.generation.json"] {
        let text = std::fs::read_to_string(public_out.join(marker)).unwrap();
        assert!(
            !text.contains("\"scope\""),
            "public {marker} stays byte-compatible"
        );
        assert_eq!(Generation::from_json(&text).unwrap().scope, Scope::Public);
    }
}

#[test]
fn no_working_page_iri_reaches_a_public_bundle() {
    let (scratch, repo) = fixture_repo();
    let (public_out, _) = built(&scratch, &repo, Scope::Public);
    let (machine_out, _) = built(&scratch, &repo, Scope::Machine);
    let needle = "urn:visionflow:page:working/";
    for (name, bytes) in tree(&public_out) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains(needle),
            "public {name} carries a working page IRI"
        );
    }
    assert!(tree(&machine_out)
        .values()
        .any(|b| String::from_utf8_lossy(b).contains(needle)));
}

#[test]
fn two_public_working_pages_with_one_file_name_do_not_collide() {
    let (scratch, repo) = fixture_repo();
    // Distinct `resource`s: `validate` already refuses two pages that derive
    // one IRI from one title (`DUPLICATE_IRI`), in `working/` as anywhere.
    write(
        &repo.join("working/a/Foo.md"),
        &note("Foo", true, "resource: urn:ngm:class:foo-a\n", "First."),
    );
    write(
        &repo.join("working/b/Foo.md"),
        &note("Foo", true, "resource: urn:ngm:class:foo-b\n", "Second."),
    );
    let (out, report) = built(&scratch, &repo, Scope::Machine);
    for slug in ["working/a/foo", "working/b/foo"] {
        assert!(
            out.join(format!("api/pages/{slug}.json")).is_file(),
            "missing {slug}"
        );
    }
    assert_eq!(report.machine.unwrap().working_records, 3);
}

#[test]
fn links_resolve_to_the_final_record_slugs() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/Linker.md"),
        &note(
            "Linker",
            true,
            "",
            "See [[Fixture Public Note]] and [[3D Model]] and [[Nowhere At All]].",
        ),
    );
    // A curated list pins what is a link, as it does in knowledge.
    let (out, _) = built(&scratch, &repo, Scope::Machine);
    let page = json(&out, "api/pages/working/linker.json");
    let links: Vec<(String, String)> = page["wikilinks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["slug"].as_str().unwrap().to_owned(),
                l["label"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let slugs: Vec<&str> = links.iter().map(|(s, _)| s.as_str()).collect();
    assert!(slugs.contains(&"working/fixture-public-note"), "{slugs:?}");
    assert!(slugs.contains(&"3-d-model"), "{slugs:?}");
    assert!(
        slugs.contains(&"nowhere-at-all"),
        "slug-verbatim fallback: {slugs:?}"
    );

    // The edge is visible from the knowledge side's page document …
    let target = json(&out, "api/pages/3-d-model.json");
    let backlinks: Vec<&str> = target["backlinks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["slug"].as_str().unwrap())
        .collect();
    assert!(backlinks.contains(&"working/linker"), "{backlinks:?}");
    // … and never in the scaffold index Loom reads.
    let scaffold = std::fs::read_to_string(out.join("data/scaffold-index.json")).unwrap();
    assert!(
        !scaffold.contains("working/"),
        "D4: data/ is knowledge-only"
    );
}

// ── identity collisions and redaction ───────────────────────────────────────

#[test]
fn a_public_working_twin_of_a_private_knowledge_page_is_refused() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("knowledge/Twin.md"),
        &class("Twin", false, "twin"),
    );
    write(
        &repo.join("working/Twin.md"),
        &note("Twin", true, "", "Mine."),
    );
    let (_, public) = build_scope(&scratch, &repo, Scope::Public);
    public.expect("control: the public build is not affected");
    let (out, machine) = build_scope(&scratch, &repo, Scope::Machine);
    let error = machine.expect_err("fail closed");
    assert!(
        matches!(blocked(&error), PublicationBlocked::Collision(_)),
        "{error:#}"
    );
    assert!(!out.exists(), "nothing is promoted");
}

#[test]
fn a_private_working_twin_of_a_public_working_page_is_refused() {
    // D1 compares un-prefixed identities, so folders do not separate them.
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/a/Foo.md"),
        &note("Foo", true, "resource: urn:ngm:class:foo-a\n", "Out."),
    );
    write(
        &repo.join("working/c/Foo.md"),
        &note("Foo", false, "resource: urn:ngm:class:foo-c\n", "In."),
    );
    let (_, machine) = build_scope(&scratch, &repo, Scope::Machine);
    let error = machine.expect_err("fail closed");
    assert!(
        matches!(blocked(&error), PublicationBlocked::Collision(_)),
        "{error:#}"
    );
}

#[test]
fn a_public_working_twin_of_a_public_knowledge_page_yields() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("knowledge/Twin.md"),
        &class("Twin", true, "twin"),
    );
    write(
        &repo.join("working/Twin.md"),
        &note("Twin", true, "", "Mine."),
    );
    let (out, report) = built(&scratch, &repo, Scope::Machine);
    assert!(
        !out.join("api/pages/working/twin.json").exists(),
        "the working twin yields"
    );
    assert!(out.join("api/pages/twin.json").is_file(), "knowledge wins");
    let summary = report.machine.unwrap();
    assert_eq!(summary.yielded, 1);
    assert_eq!(summary.working_records, 1, "only the fixture note remains");
}

#[test]
fn a_link_to_a_yielded_twin_points_at_the_knowledge_winner() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("knowledge/Twin.md"),
        &class("Twin", true, "twin"),
    );
    write(
        &repo.join("working/Twin.md"),
        &note("Twin", true, "", "Mine."),
    );
    write(
        &repo.join("working/Linker.md"),
        &note("Linker", true, "", "See [[Twin]]."),
    );
    let (out, _) = built(&scratch, &repo, Scope::Machine);
    let page = json(&out, "api/pages/working/linker.json");
    let slugs: Vec<&str> = page["wikilinks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["slug"].as_str().unwrap())
        .collect();
    assert_eq!(slugs, ["twin"], "no edge to a record that does not exist");
}

#[test]
fn public_working_text_naming_a_private_working_page_is_masked() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/Reading.md"),
        &note("Reading Fixture Private Note", true, "", "Prose."),
    );
    let (out, _) = built(&scratch, &repo, Scope::Machine);
    let page = json(&out, "api/pages/working/reading.json");
    assert_eq!(page["title"], "Reading [private]");
}

#[test]
fn a_held_back_working_page_is_neither_published_nor_a_collision() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/misc/Scratch.md"),
        &note("Scratch", true, "", "Held back by folder."),
    );
    let (out, report) = built(&scratch, &repo, Scope::Machine);
    assert!(!out.join("api/pages/working/misc/scratch.json").exists());
    assert_eq!(report.machine.unwrap().working_records, 1);
}

// ── the gates ───────────────────────────────────────────────────────────────

#[test]
fn a_dirty_working_tree_is_dirty_in_machine_scope() {
    let (scratch, repo) = fixture_repo();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "fixture"]);
    write(
        &repo.join("working/Fixture Public Note.md"),
        &note("Fixture Public Note", true, "", "Edited after the commit."),
    );

    let mut machine = options(&repo, &scratch.path().join("machine"), Scope::Machine);
    machine.source = None;
    let report = run(&repo, &machine).expect("builds");
    assert!(
        report.generation.dirty,
        "working/ is a read path in machine scope"
    );
    assert!(
        report.generation.id.ends_with("+dirty"),
        "{}",
        report.generation.id
    );
}

#[test]
fn a_string_public_flag_in_working_refuses_the_machine_build() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/Quoted.md"),
        "---\ntype: Note\ntitle: Quoted\npublic: \"true\"\n---\n\nBody.\n",
    );
    let (out, machine) = build_scope(&scratch, &repo, Scope::Machine);
    let error = machine.expect_err("never coerced into publication permission");
    assert!(
        matches!(blocked(&error), PublicationBlocked::Census(_)),
        "{error:#}"
    );
    assert!(!out.exists());
}

#[test]
fn a_credential_in_a_public_working_page_refuses_the_machine_build() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/Leaky.md"),
        &note(
            "Leaky",
            true,
            "",
            "key: sk-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ),
    );
    let (out, machine) = build_scope(&scratch, &repo, Scope::Machine);
    let error = machine.expect_err("the credential gate covers working/");
    assert!(
        error
            .downcast_ref::<build::publish::SecretsFound>()
            .is_some(),
        "{error:#}"
    );
    assert!(!out.exists());
}

#[test]
fn a_validation_error_in_working_refuses_the_machine_build() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/Untyped.md"),
        "---\ntype: NotAWorkingType\ntitle: Untyped\npublic: true\n---\n\nBody.\n",
    );
    let (out, machine) = build_scope(&scratch, &repo, Scope::Machine);
    let error = machine.expect_err("working is validated under its own role");
    assert!(
        format!("{error:#}").contains("validation rejected"),
        "{error:#}"
    );
    assert!(!out.exists());
}

// ── what machine scope refuses ──────────────────────────────────────────────

fn conflict_of(error: &anyhow::Error) -> &build::ScopeConflict {
    error
        .downcast_ref::<build::ScopeConflict>()
        .unwrap_or_else(|| panic!("a typed ScopeConflict, got: {error:#}"))
}

#[test]
fn machine_scope_refuses_options_it_cannot_honour() {
    let (scratch, repo) = fixture_repo();
    let out = scratch.path().join("machine");
    type Tweak = fn(&mut build::Options);
    let cases: [(&str, Tweak); 3] = [
        ("--publish-out", |o| {
            o.publish_out = Some(PathBuf::from("elsewhere"))
        }),
        ("--vault", |o| o.with_working_publish = true),
        ("--no-redact-private-links", |o| {
            o.redact_private_links = Some(false)
        }),
    ];
    for (named, tweak) in cases {
        let mut opts = options(&repo, &out, Scope::Machine);
        tweak(&mut opts);
        let error = run(&repo, &opts).expect_err(named);
        let conflict = conflict_of(&error);
        assert!(
            conflict.conflicts.iter().any(|c| c.contains(named)),
            "{named} must be named: {conflict}"
        );
        assert!(!out.exists(), "{named}: nothing written");
    }
}

#[test]
fn machine_scope_refuses_a_profile_that_turns_redaction_off() {
    let (scratch, repo) = fixture_repo();
    let mut lax = profile();
    lax.publication.redact_private_links = false;
    let vocab = Vocabulary::load(repo.join("ontology/vocabulary.yaml")).unwrap();
    let opts = options(&repo, &scratch.path().join("machine"), Scope::Machine);
    let error = build::run(&opts, &lax, &vocab, &WhelkEl).expect_err("fail closed");
    assert!(conflict_of(&error)
        .conflicts
        .iter()
        .any(|c| c.contains("redact")));
}

#[test]
fn machine_scope_refuses_a_non_knowledge_root_and_a_missing_working_vault() {
    let (scratch, repo) = fixture_repo();
    let out = scratch.path().join("machine");

    let mut opts = options(&repo, &out, Scope::Machine);
    opts.vault_root = repo.join("working");
    let error = run(&repo, &opts).expect_err("working is not the root");
    assert!(conflict_of(&error)
        .conflicts
        .iter()
        .any(|c| c.contains("knowledge")));

    std::fs::remove_dir_all(repo.join("working")).unwrap();
    let error = run(&repo, &options(&repo, &out, Scope::Machine)).expect_err("no working/ vault");
    assert!(conflict_of(&error)
        .conflicts
        .iter()
        .any(|c| c.contains("working/ is absent")));
    assert!(!out.exists());
}

#[test]
fn a_private_working_twin_of_a_public_knowledge_page_yields_silently() {
    // The real corpus has this shape (`Web3`): a private working note beside a
    // public knowledge class of the same name. Nothing of the note is
    // published, so it is harmless and must not refuse the build.
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("knowledge/Twin.md"),
        &class("Twin", true, "twin"),
    );
    write(
        &repo.join("working/Twin.md"),
        &note("Twin", false, "", "Private draft."),
    );
    let (out, report) = built(&scratch, &repo, Scope::Machine);
    assert!(
        out.join("api/pages/twin.json").is_file(),
        "knowledge is published"
    );
    assert!(
        !out.join("api/pages/working/twin.json").exists(),
        "the private note is nowhere"
    );
    let summary = report.machine.unwrap();
    assert_eq!(summary.yielded, 0, "a private twin is not a yield");
    // The public name is not redacted out of the knowledge page by the twin.
    let page = std::fs::read_to_string(out.join("api/pages/twin.json")).unwrap();
    assert!(page.contains("Twin"), "{page}");
}

#[test]
fn machine_scope_page_iris_carry_no_spaces_and_equal_the_slug_path() {
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/Sub Dir/Two Words.md"),
        &note("Two Words", true, "", "Body."),
    );
    let (out, _) = built(&scratch, &repo, Scope::Machine);
    let page = json(&out, "api/pages/working/sub-dir/two-words.json");
    let iri = page["id"].as_str().unwrap_or("");
    assert!(!iri.is_empty(), "{page}");
    assert!(!iri.contains(' '), "D2: no spaces in a page IRI: {iri}");
    assert!(iri.ends_with("working/sub-dir/two-words"), "{iri}");
}

#[test]
fn a_non_ascii_working_folder_keeps_its_own_slug_segment() {
    // Review finding 1: `slugify("東京")` is empty, which collapsed
    // `a/東京/Foo` onto `a//foo` and let two records claim one output file.
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/a/Foo.md"),
        &note("Foo", true, "resource: urn:ngm:class:foo-a\n", "First."),
    );
    write(
        &repo.join("working/a/東京/Foo.md"),
        &note(
            "Foo Tokyo",
            true,
            "resource: urn:ngm:class:foo-tokyo\n",
            "Second.",
        ),
    );
    let (out, _report) = built(&scratch, &repo, Scope::Machine);
    let names: Vec<String> = tree(&out)
        .into_keys()
        .filter(|p| p.starts_with("api/pages/working/a/"))
        .collect();
    assert!(
        names.iter().any(|p| p == "api/pages/working/a/foo.json"),
        "{names:?}"
    );
    assert!(
        names
            .iter()
            .any(|p| p.starts_with("api/pages/working/a/x-") && p.ends_with("/foo.json")),
        "the non-ASCII folder must keep a non-empty segment: {names:?}"
    );
    assert!(
        !names.iter().any(|p| p.contains("//")),
        "no empty segment: {names:?}"
    );
}

#[test]
fn two_working_pages_with_one_slug_are_refused_before_staging() {
    // Two folders that slugify to one segment, with distinct titles and
    // resources so DUPLICATE_IRI does not fire first: a Collision, not a late
    // output clash.
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("working/A B/Foo.md"),
        &note(
            "Foo One",
            true,
            "resource: urn:ngm:class:foo-one\n",
            "First.",
        ),
    );
    write(
        &repo.join("working/A-B/Foo.md"),
        &note(
            "Foo Two",
            true,
            "resource: urn:ngm:class:foo-two\n",
            "Second.",
        ),
    );
    let (_out, report) = build_scope(&scratch, &repo, Scope::Machine);
    let err = report.expect_err("one slug for two pages is refused");
    match blocked(&err) {
        PublicationBlocked::Collision(msg) => {
            assert!(
                msg.contains("working slug") && msg.contains("claimed by two"),
                "{msg}"
            );
        }
        other => panic!("expected Collision, got {other:?}"),
    }
}

#[test]
fn a_private_working_twins_title_is_still_masked_in_public_working_prose() {
    // Review finding 2: the private twin shares `page_id` with a public
    // knowledge page, but its TITLE is its own and private. Dropping the whole
    // record let that title ship unmasked in another working page's prose.
    let (scratch, repo) = fixture_repo();
    write(
        &repo.join("knowledge/Nightjar.md"),
        &class("Nightjar", true, "nightjar"),
    );
    write(
        &repo.join("working/Nightjar.md"),
        &note(
            "Project Nightjar Alpha",
            false,
            "resource: urn:ngm:class:nightjar-private\n",
            "Private notes.",
        ),
    );
    write(
        &repo.join("working/Diary.md"),
        // A plain record publishes only its title, so the title is the prose
        // surface a private name could escape through.
        &note(
            "Diary of Project Nightjar Alpha",
            true,
            "resource: urn:ngm:class:diary\n",
            "Body.",
        ),
    );
    let (out, _report) = built(&scratch, &repo, Scope::Machine);
    let files = tree(&out);
    let diary_path = files
        .keys()
        .find(|p| p.starts_with("api/pages/working/diary"))
        .expect("the diary record is published");
    let diary = String::from_utf8(files[diary_path].clone()).unwrap();
    assert!(
        !diary.contains("Project Nightjar Alpha"),
        "private twin title leaked: {diary}"
    );
    assert!(
        diary.contains("Diary of"),
        "the public part of the title stays: {diary}"
    );
}
