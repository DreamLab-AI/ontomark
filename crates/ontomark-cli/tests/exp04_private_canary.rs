//! EXP-04 canary: a page that is not public must not be nameable from any
//! published artefact.
//!
//! The fixture (`conformance/fixtures/estate-small`) carries synthetic private
//! pages (a knowledge class with an alias, a working note, a journal) and
//! public pages that link to them from frontmatter relation lists, prose,
//! labelled links and embeds. The set of non-public pages is computed here from
//! the files themselves, independently of the compiler, and every file the
//! build writes (`out/` and `publish/`, binary included) is searched for each
//! page's title, aliases, identity and canary strings, case-insensitively.
//!
//! Code spans and fences are authored text, not links: they are never
//! rewritten, so the fixture's fenced example uses a string that names no page.
//!
//! The machine projection shares the cross-vault resolver with Markdown staging,
//! so private aliases, fragments and working-page relations are checked too.
//! Explicit compatibility mode remains byte-identical to the legacy golden.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const CLOCK: &str = "2026-10-11T00:00:00.000000+00:00";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
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

fn fixture_copy() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("scratch dir");
    copy_tree(
        &repo_root().join("conformance/fixtures/estate-small"),
        dir.path(),
    );
    dir
}

/// Every file under `root`, by relative path.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let rel = path.strip_prefix(base).expect("under base");
                out.insert(
                    rel.to_string_lossy().into_owned(),
                    std::fs::read(&path).expect("read"),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    if root.exists() {
        walk(root, root, &mut out);
    }
    out
}

struct Built {
    _scratch: tempfile::TempDir,
    out: PathBuf,
    publish: PathBuf,
    stdout: String,
}

fn build(vault: &Path, redact: bool) -> Built {
    let scratch = tempfile::tempdir().expect("out dir");
    let out = scratch.path().join("out");
    let publish = scratch.path().join("publish");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ontomark"));
    cmd.arg("--repo")
        .arg(vault)
        .args(["--json", "build", "--vault", "all", "--stats", "--out"])
        .arg(&out)
        .arg("--publish-out")
        .arg(&publish)
        .arg("--with-markdown-mirror")
        .args(["--generated-at", CLOCK, "--source-commit", COMMIT]);
    if !redact {
        cmd.arg("--no-redact-private-links");
    }
    let output = cmd.output().expect("run ontomark build");
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Built {
        _scratch: scratch,
        out,
        publish,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    }
}

/// One non-public page, described from its file alone.
#[derive(Debug)]
struct Private {
    file: String,
    needles: Vec<String>,
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches(|c| c == '\'' || c == '"').to_owned()
}

/// Title, aliases, canary strings and whether `public: true` is declared.
fn describe(text: &str) -> (Option<String>, Vec<String>, Vec<String>, bool) {
    let (head, body) = match text
        .strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---\n"))
    {
        Some((h, b)) => (h, b),
        None => ("", text),
    };
    let mut title = None;
    let mut aliases = Vec::new();
    let mut public = false;
    let mut in_aliases = false;
    for line in head.lines() {
        if let Some(rest) = line.strip_prefix("title:") {
            title = Some(unquote(rest));
        } else if line.trim() == "public: true" {
            public = true;
        } else if let Some(rest) = line.strip_prefix("aliases:") {
            in_aliases = true;
            let rest = rest.trim();
            if let Some(inline) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                aliases.extend(inline.split(',').map(unquote));
                in_aliases = false;
            }
        } else if in_aliases && line.starts_with("- ") {
            aliases.push(unquote(&line[2..]));
        } else {
            in_aliases = false;
        }
    }
    let canaries = body
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|w| w.starts_with("PRIVATE-CANARY"))
        .map(str::to_owned)
        .collect();
    (title, aliases, canaries, public)
}

/// Every non-public page in the fixture, computed without the compiler.
/// Journals contribute only their canary: their dates are in every timestamp.
fn private_pages(vault: &Path) -> Vec<Private> {
    let mut out = Vec::new();
    for kind in ["knowledge", "working"] {
        for (rel, bytes) in files(&vault.join(kind)) {
            if !Path::new(&rel).extension().is_some_and(|e| e == "md") {
                continue;
            }
            let text = String::from_utf8(bytes).expect("utf-8 page");
            let (title, aliases, canaries, public) = describe(&text);
            let journal = rel.starts_with("journals/");
            if public && !journal {
                continue;
            }
            let mut needles = canaries;
            if !journal {
                let id = rel
                    .strip_prefix("pages/")
                    .and_then(|r| r.strip_suffix(".md"))
                    .expect("a page path")
                    .to_owned();
                needles.push(id);
                // The hyphenated form is how a name appears inside a slug.
                needles.extend(title.iter().chain(&aliases).map(|n| n.replace(' ', "-")));
                needles.extend(title);
                needles.extend(aliases);
            }
            out.push(Private {
                file: format!("{kind}/{rel}"),
                needles,
            });
        }
    }
    out
}

/// `(file, needle)` for every needle found in any file of any tree.
fn leaks(trees: &[&Path], private: &[Private]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for tree in trees {
        for (rel, bytes) in files(tree) {
            let haystack = String::from_utf8_lossy(&bytes).to_lowercase();
            for page in private {
                for needle in &page.needles {
                    if haystack.contains(&needle.to_lowercase()) {
                        found.push((format!("{}", Path::new(&rel).display()), needle.clone()));
                    }
                }
            }
        }
    }
    found
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_fixture_has_the_private_pages_the_canary_relies_on() {
    let vault = fixture_copy();
    let private = private_pages(vault.path());
    let files: Vec<&str> = private.iter().map(|p| p.file.as_str()).collect();
    for expected in [
        "knowledge/pages/Fixture Private Ancestor.md",
        "working/pages/Fixture Private Note.md",
        "working/journals/2026-10-11.md",
    ] {
        assert!(files.contains(&expected), "{files:?}");
    }
    let ancestor = private
        .iter()
        .find(|p| p.file.ends_with("Fixture Private Ancestor.md"))
        .expect("ancestor");
    assert!(ancestor
        .needles
        .iter()
        .any(|n| n == "Fixture Hidden Ancestor"));
    assert!(ancestor
        .needles
        .iter()
        .any(|n| n.starts_with("PRIVATE-CANARY-ANCESTOR")));
}

#[test]
fn no_published_file_names_a_non_public_page() {
    let vault = fixture_copy();
    let private = private_pages(vault.path());
    let built = build(vault.path(), true);
    let found = leaks(&[&built.out, &built.publish], &private);
    assert!(found.is_empty(), "private names leaked:\n{found:#?}");
}

#[test]
fn the_canary_fires_without_redaction() {
    let vault = fixture_copy();
    let private = private_pages(vault.path());
    let built = build(vault.path(), false);
    let found = leaks(&[&built.publish], &private);
    let in_files: Vec<&str> = found.iter().map(|(f, _)| f.as_str()).collect();
    for expected in [
        "pages/Fixture Link Probe.md",
        "pages/Fixture Public Child.md",
        "working/Fixture Public Note.md",
    ] {
        assert!(
            in_files.contains(&expected),
            "the negative control must see {expected}: {found:#?}"
        );
    }
    // The legacy mirror already masked a private *title* with `[private]`
    // (the projection's marker redactor), which leaves broken `[[[private]]]`
    // link syntax; it did not know aliases or `working/` titles. With
    // redaction the same link reads as plain text.
    let mirror = read(&built.out.join("api/markdown/Fixture Link Probe.md"));
    assert!(mirror.contains("[private]"), "{mirror}");
    assert!(!mirror.contains("(private link)"), "{mirror}");
}

#[test]
fn staged_pages_are_rewritten_by_the_rules() {
    let vault = fixture_copy();
    let built = build(vault.path(), true);

    let probe = read(&built.publish.join("pages/Fixture Link Probe.md"));
    // Frontmatter: the only entry under `related-to` was private, so the key goes.
    assert!(!probe.contains("related-to"), "{probe}");
    assert!(probe.contains("is-a:\n- '[[3D Asset]]'\n---"), "{probe}");
    // Body: marker, safe display text, removed embed, untouched public/unresolved.
    assert!(probe.contains("- Plain: (private link)."), "{probe}");
    assert!(probe.contains("- Piped: a neutral alias."), "{probe}");
    assert!(probe.contains("- Embed:  (removed)."), "{probe}");
    assert!(probe.contains("[[Fixture Missing Page]] stays"), "{probe}");
    assert!(probe.contains("[[3D Model]] stays"), "{probe}");
    // Code is authored text: the fence and the span are byte for byte.
    assert!(
        probe.contains("```text\n[[Example Target]] and ![[Example Embed]]\n```"),
        "{probe}"
    );
    assert!(
        probe.contains("inline `[[Example Target]]` likewise"),
        "{probe}"
    );

    let child = read(&built.publish.join("pages/Fixture Public Child.md"));
    assert!(
        child.contains("is-a:\n- '[[3D Asset]]'\nrelated-to:"),
        "{child}"
    );
    assert!(!child.contains("Private Ancestor"), "{child}");

    let note = read(&built.publish.join("working/Fixture Public Note.md"));
    assert!(!note.contains("see-also"), "{note}");
    assert!(note.contains("related:\n- '[[3D Model]]'\n"), "{note}");
    assert!(
        note.contains("links to [[3D Model]] and (private link)."),
        "{note}"
    );
    assert!(
        note.contains("Embedded:  and a labelled one, my reading list."),
        "{note}"
    );
    assert!(
        note.contains("By heading: (private link); by alias: (private link)."),
        "{note}"
    );

    // The mirror is rewritten by the same rules.
    let mirror = read(&built.out.join("api/markdown/Fixture Link Probe.md"));
    assert!(mirror.contains("- Plain: (private link)."), "{mirror}");
    assert!(mirror.contains("- Piped: a neutral alias."), "{mirror}");
    assert!(mirror.contains("```text\n[[Example Target]]"), "{mirror}");
}

#[test]
fn machine_records_use_the_same_public_link_boundary_as_markdown() {
    let vault = fixture_copy();
    let page = vault.path().join("knowledge/pages/Fixture Link Probe.md");
    let text = read(&page).replace(
        "Synthetic public class whose prose links to non-public pages, for the EXP-04 redaction tests.",
        "Synthetic definition points to [[Fixture Hidden Ancestor#Detail]].",
    );
    std::fs::write(&page, text).expect("write definition probe");
    let on = build(vault.path(), true);
    let off = build(vault.path(), false);
    // Safe machine prose uses the same link rewrite as the Markdown mirror;
    // explicit compatibility mode keeps the legacy projection behaviour.
    let legacy: serde_json::Value =
        serde_json::from_str(&read(&off.out.join("api/pages/fixture-link-probe.json")))
            .expect("legacy page JSON");
    assert!(legacy["definition"]
        .as_str()
        .expect("definition")
        .contains("Fixture Hidden Ancestor"));
    let api: serde_json::Value =
        serde_json::from_str(&read(&on.out.join("api/pages/fixture-link-probe.json")))
            .expect("page JSON");
    assert_eq!(
        api["definition"],
        "Synthetic definition points to (private link)."
    );
    let links = api["wikilinks"].to_string();
    assert!(links.contains("3d-model"), "{links}");
    assert!(links.contains("fixture-missing-page"), "{links}");
    assert!(!links.contains("private"), "{links}");

    // The published machine artefacts carry no private name (the scan in
    // `no_published_file_names_a_non_public_page` covers every file; this pins
    // the Turtle and the page API explicitly).
    let private = private_pages(vault.path());
    for artefact in [
        "data/ontology.ttl",
        "data/ontology-inferred.ttl",
        "api/pages/fixture-link-probe.json",
        "api/pages/fixture-public-child.json",
    ] {
        let text = read(&on.out.join(artefact)).to_lowercase();
        for page in &private {
            for needle in &page.needles {
                assert!(
                    !text.contains(&needle.to_lowercase()),
                    "{artefact} names {needle}"
                );
            }
        }
    }
}

#[test]
fn the_report_counts_what_was_changed() {
    let vault = fixture_copy();
    let on = build(vault.path(), true);
    let value: serde_json::Value = serde_json::from_str(&on.stdout).expect("json report");
    let red = &value["redaction"];
    assert_eq!(red["publish"]["knowledge"]["pages_changed"], 2, "{red}");
    assert_eq!(red["publish"]["knowledge"]["embeds_removed"], 1, "{red}");
    assert_eq!(red["publish"]["working"]["pages_changed"], 1, "{red}");
    assert_eq!(
        red["publish"]["working"]["frontmatter_keys_removed"], 1,
        "{red}"
    );
    assert_eq!(red["mirror"]["pages_changed"], 1, "{red}");

    let off = build(vault.path(), false);
    let value: serde_json::Value = serde_json::from_str(&off.stdout).expect("json report");
    assert!(
        value.get("redaction").is_none(),
        "no report when redaction is off"
    );
}

/// Private aliases and heading forms must never reach machine wikilink lists.
#[test]
fn machine_wikilinks_do_not_name_private_aliases_and_headings() {
    let vault = fixture_copy();
    let page = vault.path().join("knowledge/pages/Fixture Link Probe.md");
    let mut text = read(&page);
    text.push_str("\nBy heading: [[Fixture Private Ancestor#Detail]]; by alias: [[Fixture Hidden Ancestor]].\nLabel: [[Fixture Private Ancestor|the Fixture Hidden Ancestor documentation]].\n");
    std::fs::write(&page, text).expect("write probe");
    let private = private_pages(vault.path());
    let built = build(vault.path(), true);
    let found = leaks(&[&built.out, &built.publish], &private);
    assert!(found.is_empty(), "private names leaked:\n{found:#?}");
}

/// A private sibling-vault relation is removed before closure and all emitters.
#[test]
fn relations_to_private_working_pages_do_not_reach_machine_artefacts() {
    let vault = fixture_copy();
    let page = vault.path().join("knowledge/pages/Fixture Link Probe.md");
    let text = read(&page).replace(
        "- '[[Fixture Hidden Ancestor]]'\n",
        "- '[[Fixture Hidden Ancestor]]'\n- '[[Fixture Private Note]]'\n",
    );
    std::fs::write(&page, text).expect("write probe");
    let private = private_pages(vault.path());
    let built = build(vault.path(), true);
    let found = leaks(&[&built.out, &built.publish], &private);
    assert!(found.is_empty(), "private names leaked:\n{found:#?}");
}

#[test]
fn private_curated_targets_and_definition_conjuncts_are_withheld() {
    let vault = fixture_copy();
    let note = vault.path().join("working/pages/Fixture Private Note.md");
    let text = read(&note).replace(
        "type: Note\n",
        "type: Note\naliases: [Hidden Working Alias]\n",
    );
    std::fs::write(&note, text).expect("write private alias");
    let page = vault.path().join("knowledge/pages/Fixture Link Probe.md");
    let text = read(&page).replacen("\n---\n", "\nlinks:\n- '[[Hidden Working Alias.md#Detail|neutral label]]'\n- '[[3D Model]]'\ndefines-as:\n- '[[3D Asset]]'\n- '[[Fixture Private Ancestor]]'\n---\n", 1);
    let text = text.replace(
        "- '[[Fixture Hidden Ancestor]]'\n",
        "- '[[Fixture Hidden Ancestor]]'\n- '[[Hidden Working Alias.md#Detail|neutral label]]'\n",
    );
    std::fs::write(&page, text).expect("write curated probe");
    let built = build(vault.path(), true);
    let found = leaks(&[&built.out, &built.publish], &private_pages(vault.path()));
    assert!(found.is_empty(), "private names leaked:\n{found:#?}");
    let api: serde_json::Value =
        serde_json::from_str(&read(&built.out.join("api/pages/fixture-link-probe.json")))
            .expect("page JSON");
    assert_eq!(api["wikilinks"].as_array().expect("links").len(), 1);
    assert!(!read(&built.publish.join("pages/Fixture Link Probe.md")).contains("defines-as"));
    // Keep the asserted parent; never publish a weakened equivalent definition.
    let turtle = read(&built.out.join("data/ontology.ttl"));
    assert!(!turtle.contains("owl:equivalentClass"), "{turtle}");
}

#[test]
fn an_explicit_private_curated_set_stays_empty_after_projection() {
    let vault = fixture_copy();
    let page = vault.path().join("knowledge/pages/Fixture Link Probe.md");
    let text = read(&page).replacen(
        "\n---\n",
        "\nlinks:\n- '[[Fixture Hidden Ancestor#Detail|neutral label]]'\n---\n",
        1,
    );
    std::fs::write(&page, text).expect("write curated probe");
    let built = build(vault.path(), true);
    let api: serde_json::Value =
        serde_json::from_str(&read(&built.out.join("api/pages/fixture-link-probe.json")))
            .expect("page JSON");
    // The body contains public links, but a declared curated set must retain
    // its meaning when filtering removes its last private target.
    assert_eq!(api["wikilinks"], serde_json::json!([]));
    let found = leaks(&[&built.out, &built.publish], &private_pages(vault.path()));
    assert!(found.is_empty(), "private names leaked:\n{found:#?}");
}
