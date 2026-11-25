//! Typed `sources` in the published output (ADR-VG-010 §5, §7), end to end: a
//! public page's sources reach `ontology.ttl` and `api/v3/pages`, and a source
//! that names a non-public page leaks neither that page's title nor its
//! identity into any file of the build.

use std::fs;
use std::path::{Path, PathBuf};

use ontomark_compile::build::{self, Scope};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::profile::Profile;
use ontomark_compile::whelk::WhelkEl;

const B4: &str = "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";
const SECRET_TITLE: &str = "Zebra Hush Ledger";
const PIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SECRET_URN: &str = "urn:visionflow:page:5ec2e7c0ffee";

fn profile() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml")).unwrap()
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn ledger(title: &str, visibility: &str, urn: &str) -> String {
    format!(
        "---\ntype: Episode\ntitle: {title}\nstatus: approved\nvisibility: {visibility}\npage_resource: {urn}\nepisode-url: https://example.org/{urn}\nepisode-date: 2026-03-10\ngenerated:\n  by: {B4}\n  at: 2026-11-01T00:00:00Z\n---\n# {title}\n\n- A claim.\n  <!-- assertion-fp: 32ebb74ef2751e32 -->\n"
    )
}

/// A vocabulary 3 repository whose public class cites a public ledger, a
/// brief, an external standard and a private ledger.
fn repo(dir: &Path) -> PathBuf {
    let root = dir.join("repo");
    write(
        &root,
        "ontology/vocabulary.yaml",
        include_str!("../../ontomark-cli/tests/vocab-v3-vault/ontology/vocabulary.yaml"),
    );
    write(
        &root,
        "ontology/attribution.yaml",
        include_str!("../../ontomark-cli/tests/vocab-v3-vault/ontology/attribution.yaml"),
    );
    write(
        &root,
        "knowledge/Cites.md",
        &format!(
            "---\ntype: Class\ntitle: Cites\nresource: urn:ngm:class:cites\npublic: true\nstatus: stable\ngenerated:\n  by: {B4}\n  at: 2026-11-01T00:00:00Z\nsources:\n- {{ id: a, resource: '[[Open Ledger]]', rel: origin }}\n- {{ id: b, resource: '[[{SECRET_TITLE}]]', rel: origin, locator: 'fp:32ebb74ef2751e32' }}\n- {{ id: c, resource: '{SECRET_URN}', rel: reference }}\n- {{ id: d, resource: '[[Open Brief]]', rel: derived-from, locator: 'sha256-v1:{PIN}' }}\n- {{ id: e, resource: 'https://example.org/spec', rel: standard }}\n---\nBody.\n"
        ),
    );
    write(
        &root,
        "working/Open Ledger.md",
        &ledger("Open Ledger", "public", "urn:visionflow:page:0be7a11c0de0"),
    );
    write(
        &root,
        &format!("working/{SECRET_TITLE}.md"),
        &ledger(SECRET_TITLE, "shared", SECRET_URN),
    );
    write(
        &root,
        "working/Open Brief.md",
        &format!(
            "---\ntype: Brief\ntitle: Open Brief\nstatus: draft\nvisibility: public\npage_resource: urn:visionflow:page:b21ef0000001\nscope: topic\ngenerated:\n  by: {B4}\n  at: 2026-11-01T00:00:00Z\n---\n# Open Brief\n"
        ),
    );
    root
}

fn run_build(root: &Path, out: &Path) {
    let vocab = Vocabulary::load(root.join("ontology/vocabulary.yaml")).unwrap();
    let opts = build::Options {
        vault_root: root.join("knowledge"),
        out: out.to_path_buf(),
        repo_root: root.to_path_buf(),
        with_markdown_mirror: true,
        generated_at: Some("2026-11-25T10:00:00+00:00".to_owned()),
        source: Some(("0".repeat(40), false)),
        with_working_publish: true,
        redact_private_links: Some(true),
        publish_out: Some(out.join("publish")),
        stale_after: None,
        scope: Scope::Public,
        certificate_out: None,
    };
    build::run(&opts, &profile(), &vocab, &WhelkEl).unwrap();
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
fn sources_reach_the_published_ontology_and_page_api_without_leaking_a_private_target() {
    let dir = tempfile::tempdir().unwrap();
    let root = repo(dir.path());
    let out = dir.path().join("out");
    run_build(&root, &out);

    let ttl = fs::read_to_string(out.join("data/ontology.ttl")).unwrap();
    assert!(
        ttl.contains("prov:hadPrimarySource <urn:visionflow:page:0be7a11c0de0>"),
        "{ttl}"
    );
    assert!(
        ttl.contains("prov:wasDerivedFrom <urn:visionflow:page:b21ef0000001>"),
        "{ttl}"
    );
    assert!(
        ttl.contains("dc:source <https://example.org/spec>"),
        "{ttl}"
    );
    assert!(
        ttl.contains(&format!("vc:sourceLocator \"sha256-v1:{PIN}\"")),
        "{ttl}"
    );

    let page: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("api/v3/pages/cites.json")).unwrap()).unwrap();
    let ids: Vec<&str> = page["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["a", "d", "e"], "{page}");
    let legacy: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("api/pages/cites.json")).unwrap()).unwrap();
    assert!(legacy.get("sources").is_none(), "{legacy}");

    // The whole output tree: neither the private page's title nor its identity
    // appears anywhere, in any artefact or mirror.
    let mut all = Vec::new();
    files(&out, &mut all);
    assert!(all.len() > 20, "{} files", all.len());
    for path in all {
        let bytes = fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for needle in [SECRET_TITLE, "5ec2e7c0ffee", "zebra-hush-ledger"] {
            assert!(!text.contains(needle), "{} leaks {needle}", path.display());
        }
    }
}
