//! The flat Obsidian vault layout (owner decision 2026-10-16): pages sit at
//! the vault root, the root's furniture folders are excluded by name, and a
//! profile's `layout.excluded` overrides the built-in list for the build.
//!
//! The output contract is unchanged: knowledge pages still stage under
//! `publish/pages/<id>.md` and working pages under `publish/working/<id>.md`.

use std::path::{Path, PathBuf};

use ontomark_compile::build;
use ontomark_compile::input::page::{SkipReason, Vault, VaultKind, EXCLUDED_DIRS};
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
        generated_at: Some("2026-10-16T00:00:00+00:00".to_owned()),
        source: Some(("0000000000000000000000000000000000000000".to_owned(), false)),
        with_working_publish: false,
        redact_private_links: None,
        publish_out: None,
        stale_after: None,
        scope: build::Scope::Public,
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A public knowledge page valid under the estate grammar, titled `title`.
fn public_class(title: &str) -> String {
    let slug = title.to_lowercase().replace(' ', "-");
    format!(
        "---\ntype: Class\ntitle: {title}\nresource: urn:ngm:class:{slug}\npublic: true\n\
         status: draft\ndomain: spatial-computing\nmaturity: draft\ngenerated:\n  \
         by: agent:fixture-writer/1.0\n  process: process:space-corpus-research/0.1.0\n  \
         at: 2026-10-16T00:00:00Z\nis-a:\n- '[[3D Asset]]'\n---\n\nA synthetic page for \
         the layout test.\n"
    )
}

fn staged(out: &Path) -> Vec<String> {
    walkdir::WalkDir::new(out.join("publish"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(out.join("publish"))
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

fn census(out: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(out.join("api/census.json")).unwrap()).unwrap()
}

#[test]
fn obsidian_furniture_at_the_vault_root_is_not_corpus() {
    let (scratch, repo) = fixture_repo();
    let k = repo.join("knowledge");
    // Furniture an ordinary Obsidian vault carries at its root. Each holds a
    // page that *would* publish if it were read, and the template's `public`
    // is a string, which would refuse the whole build if the census parsed it.
    write(
        &k.join("templates/Class Template.md"),
        "---\ntype: Class\ntitle: Fixture Template Leak\npublic: \"true\"\n---\n",
    );
    write(
        &k.join("bases/Fixture Base Leak.md"),
        &public_class("Fixture Base Leak"),
    );
    write(
        &k.join("assets/Fixture Asset Leak.md"),
        &public_class("Fixture Asset Leak"),
    );
    write(
        &k.join(".obsidian/Fixture Editor Leak.md"),
        "# editor state\n",
    );
    // A nested page that is corpus: it keeps its folder in the id.
    write(
        &k.join("podcast-evidence/Fixture Nested Page.md"),
        &public_class("Fixture Nested Page"),
    );

    let vault = Vault::load(&k, VaultKind::Knowledge).expect("vault");
    assert!(vault.get("podcast-evidence/Fixture Nested Page").is_some());
    assert!(vault.get("templates/Class Template").is_none());
    let excluded: Vec<String> = vault
        .skipped
        .iter()
        .filter(|s| s.reason == SkipReason::ExcludedDirectory)
        .map(|s| s.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        excluded,
        [
            "assets/Fixture Asset Leak.md",
            "bases/Fixture Base Leak.md",
            "templates/Class Template.md"
        ]
    );
    assert!(vault
        .skipped
        .iter()
        .any(|s| s.reason == SkipReason::HiddenDirectory
            && s.path == Path::new(".obsidian/Fixture Editor Leak.md")));

    let out = scratch.path().join("out");
    let vocab = Vocabulary::load(repo.join("ontology/vocabulary.yaml")).expect("vocabulary");
    build::run(&options(&repo, &out), &profile(), &vocab, &WhelkEl).expect("builds");

    let files = staged(&out);
    // Output contract: knowledge pages publish under `pages/`, with the id.
    assert!(
        files.contains(&"pages/podcast-evidence/Fixture Nested Page.md".to_owned()),
        "{files:?}"
    );
    assert!(files.contains(&"pages/Fixture Public Child.md".to_owned()));
    for leak in ["Template", "Base", "Asset", "Editor"] {
        assert!(
            !files
                .iter()
                .any(|f| f.contains(&format!("Fixture {leak} Leak"))),
            "{leak} leaked: {files:?}"
        );
    }
    // The census accounts for every file: the four furniture pages are
    // `excluded_directory`, none is parsed, and the books balance.
    let c = census(&out);
    assert_eq!(c["counts"]["excluded_directory"], 4, "{c}");
    assert_eq!(c["balanced"], true, "{c}");
}

#[test]
fn a_profile_layout_override_is_honoured_by_the_build() {
    let (scratch, repo) = fixture_repo();
    let k = repo.join("knowledge");
    write(
        &k.join("scratch/Fixture Scratch Page.md"),
        &public_class("Fixture Scratch Page"),
    );
    let vocab = Vocabulary::load(repo.join("ontology/vocabulary.yaml")).expect("vocabulary");

    // Built-in list: `scratch/` is an ordinary folder, so the page publishes.
    let out = scratch.path().join("default");
    build::run(&options(&repo, &out), &profile(), &vocab, &WhelkEl).expect("builds");
    assert!(
        staged(&out).contains(&"pages/scratch/Fixture Scratch Page.md".to_owned()),
        "{:?}",
        staged(&out)
    );
    let before = census(&out)["counts"]["excluded_directory"]
        .as_u64()
        .unwrap_or(0);

    // The profile adds `scratch` to the excluded folders: same tree, the page
    // is now furniture, and the census says so.
    let mut custom = profile();
    assert_eq!(custom.layout.excluded, EXCLUDED_DIRS);
    custom.layout.excluded.push("scratch".to_owned());
    let out = scratch.path().join("custom");
    build::run(&options(&repo, &out), &custom, &vocab, &WhelkEl).expect("builds");
    let files = staged(&out);
    assert!(
        !files.iter().any(|f| f.contains("Fixture Scratch Page")),
        "{files:?}"
    );
    assert_eq!(
        census(&out)["counts"]["excluded_directory"],
        before + 1,
        "{}",
        census(&out)
    );
}

#[test]
fn the_layout_block_parses_and_refuses_unknown_keys() {
    let p = Profile::from_yaml("layout:\n  excluded: [journals, drafts]\n").unwrap();
    assert_eq!(p.layout.excluded, ["journals", "drafts"]);
    assert!(Profile::from_yaml("layout:\n  pages: pages\n").is_err());
}
