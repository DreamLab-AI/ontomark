//! `ontomark edit` and `ontomark memory` through the real binary: the one
//! write engine behind both.

use std::path::Path;
use std::process::{Command, Output};

const HUMAN: &str = "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";
const NOW: &str = "2026-11-20T12:00:00Z";

const VOCAB_V2: &str = "version: 2\nnamespace: \"urn:ngm:class:\"\ntypes:\n  Class: { owl: \"owl:Class\", required: [resource, status] }\nrelations:\n  is-a: { owl: \"rdfs:subClassOf\" }\n";
const VOCAB_V3: &str = "version: 3\nnamespace: \"urn:ngm:class:\"\ntypes:\n  Class: { owl: \"owl:Class\", required: [resource, status] }\nworking_types:\n  Note: {}\n  Belief: { memory: true }\nrelations:\n  is-a: { owl: \"rdfs:subClassOf\" }\n";
const ROSTER: &str = "principals:\n  - did: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a\n    label: jjohare\n    kind: human\n";

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(repo)
        .arg("--json")
        .args(args)
        .env_remove("ONTOMARK_PRINCIPAL")
        .env_remove("ONTOMARK_INSTRUMENT")
        .output()
        .expect("the ontomark binary runs")
}

fn json(o: &Output) -> serde_json::Value {
    serde_json::from_str(&String::from_utf8_lossy(&o.stdout)).unwrap_or_else(|e| {
        panic!(
            "not JSON ({e}): {}\n{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )
    })
}

fn v2_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    write(&repo.join("ontology/vocabulary.yaml"), VOCAB_V2);
    write(
        &repo.join("knowledge/Alpha.md"),
        "---\ntype: Class\n# keep this comment\npublic: true\nresource: urn:ngm:class:alpha\nstatus: draft\nweird:   {a: 1,   b: [x]}\n---\nbody [[Alpha]]\n",
    );
    (dir, repo)
}

#[test]
fn edit_is_lossless_and_keeps_its_contract() {
    let (_d, repo) = v2_repo();
    let page = repo.join("knowledge/Alpha.md");
    // Without --expect: refused, exit 1, nothing written.
    let before = std::fs::read_to_string(&page).unwrap();
    let o = run(&repo, &["edit", "Alpha", "--set", "status=stable"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("--expect must declare docs and blocks"));
    assert_eq!(std::fs::read_to_string(&page).unwrap(), before);
    // A wrong blast radius: refused.
    let o = run(
        &repo,
        &[
            "edit",
            "Alpha",
            "--set",
            "status=stable",
            "--expect",
            "docs=1,blocks=2",
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("--expect blocks=2"));
    // Dry run writes nothing but reports the same outcome shape.
    let o = run(
        &repo,
        &[
            "edit",
            "Alpha",
            "--set",
            "status=stable",
            "--expect",
            "docs=1,blocks=1",
            "--dry-run",
        ],
    );
    assert!(o.status.success());
    let v = json(&o);
    assert_eq!(
        (v["page"].as_str(), v["docs"].as_u64(), v["blocks"].as_u64()),
        (Some("Alpha"), Some(1), Some(1))
    );
    assert!(v["changes"]["status"].is_array());
    assert_eq!(std::fs::read_to_string(&page).unwrap(), before);
    // The real edit: only the status line changes; comment, odd flow map, body stay.
    let o = run(
        &repo,
        &[
            "edit",
            "Alpha",
            "--set",
            "status=stable",
            "--expect",
            "docs=1,blocks=1",
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        std::fs::read_to_string(&page).unwrap(),
        before.replace("status: draft", "status: stable")
    );
    // A no-op edit declares zero and writes nothing.
    let o = run(
        &repo,
        &[
            "edit",
            "Alpha",
            "--set",
            "status=stable",
            "--expect",
            "docs=0,blocks=0",
        ],
    );
    assert!(o.status.success());
    // Append and unset.
    let o = run(
        &repo,
        &[
            "edit",
            "Alpha",
            "--set",
            "tags+=x",
            "--expect",
            "docs=1,blocks=1",
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(std::fs::read_to_string(&page).unwrap().contains("tags:"));
}

fn v3_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    write(&repo.join("ontology/vocabulary.yaml"), VOCAB_V3);
    write(&repo.join("ontology/attribution.yaml"), ROSTER);
    write(&repo.join("knowledge/Alpha.md"), "---\ntype: Class\npublic: true\nresource: urn:ngm:class:alpha\nstatus: stable\n---\nbody\n");
    write(
        &repo.join("working/Note One.md"),
        &format!("---\ntype: Note\nstatus: draft\npage_resource: urn:visionflow:page:n1\nvisibility: shared\ngenerated: {{ by: {HUMAN}, at: '2026-11-01T00:00:00Z' }}\n---\nnote\n"),
    );
    (dir, repo)
}

#[test]
fn a_v3_working_edit_is_stamped_and_the_stamps_do_not_count_toward_blocks() {
    let (_d, repo) = v3_repo();
    let page = repo.join("working/Note One.md");
    let o = run(
        &repo,
        &[
            "edit",
            "--vault",
            "working",
            "Note One",
            "--set",
            "tags+=x",
            "--expect",
            "docs=1,blocks=1",
        ],
    );
    assert_eq!(
        o.status.code(),
        Some(1),
        "an unstamped v3 working write is refused"
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("must name its principal"));
    let o = run(
        &repo,
        &[
            "edit",
            "--vault",
            "working",
            "Note One",
            "--set",
            "tags+=x",
            "--expect",
            "docs=1,blocks=1",
            "--by",
            HUMAN,
            "--now",
            NOW,
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = std::fs::read_to_string(&page).unwrap();
    assert!(
        text.contains("contributors:") && text.contains(&format!("by: {HUMAN}")),
        "{text}"
    );
    assert!(text.contains("tags:"));
    // An engine-owned key is refused.
    let o = run(
        &repo,
        &[
            "edit",
            "--vault",
            "working",
            "Note One",
            "--set",
            "generated=x",
            "--expect",
            "docs=1,blocks=1",
            "--by",
            HUMAN,
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("ENGINE_OWNED_KEY"));
}

#[test]
fn memory_create_approve_and_the_refusals_exit_two() {
    let (_d, repo) = v3_repo();
    let create = |extra: &[&str]| {
        let mut a = vec![
            "memory",
            "create",
            "beliefs/Tea",
            "--type",
            "Belief",
            "--set",
            "about=https://example.org/tea",
            "--set",
            "confidence=0.7",
            "--set",
            "sources=[{id: a, resource: 'https://x.example', rel: evidence, locator: p1}]",
            "--body",
            "Tea first.\n",
            "--by",
            HUMAN,
            "--now",
            NOW,
        ];
        a.extend_from_slice(extra);
        run(&repo, &a)
    };
    let o = create(&["--dry-run"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!repo.join("working/beliefs/Tea.md").exists());
    let o = create(&[]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(json(&o)["written"], true);
    let text = std::fs::read_to_string(repo.join("working/beliefs/Tea.md")).unwrap();
    assert!(
        text.contains("status: draft") && text.contains("page_resource: urn:visionflow:page:"),
        "{text}"
    );
    // A second creation never overwrites: refusal, exit 2, JSON names the code.
    let o = create(&[]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(json(&o)["code"], "EXISTS");
    // An approval through an instrument is refused.
    let o = run(
        &repo,
        &[
            "memory",
            "approve",
            "beliefs/Tea",
            "--by",
            HUMAN,
            "--via",
            "agent:claude/1",
            "--now",
            NOW,
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(json(&o)["code"], "APPROVER");
    // The principal approves.
    let o = run(
        &repo,
        &[
            "memory",
            "approve",
            "beliefs/Tea",
            "--by",
            HUMAN,
            "--now",
            NOW,
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = std::fs::read_to_string(repo.join("working/beliefs/Tea.md")).unwrap();
    assert!(
        text.contains("status: approved") && text.contains("last-reviewed:"),
        "{text}"
    );
    // approved -> approved is a confirmation, not an approval.
    let o = run(
        &repo,
        &[
            "memory",
            "approve",
            "beliefs/Tea",
            "--by",
            HUMAN,
            "--now",
            NOW,
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(json(&o)["code"], "ILLEGAL_TRANSITION");
    // The validator is satisfied with what the engine wrote.
    let v = run(&repo, &["validate", "--vault", "all"]);
    assert!(
        v.status.success(),
        "{}{}",
        String::from_utf8_lossy(&v.stdout),
        String::from_utf8_lossy(&v.stderr)
    );
}

#[test]
fn memory_supersede_writes_both_pages_and_nothing_else() {
    let (_d, repo) = v3_repo();
    let belief = |tail: &str, status: &str, extra: &str| {
        format!("---\ntype: Belief\nstatus: {status}\npage_resource: urn:visionflow:page:{tail}\nvisibility: shared\ngenerated: {{ by: {HUMAN}, at: '2026-11-01T00:00:00Z' }}\ncontent-updated: '2026-11-01T00:00:00Z'\nabout: 'https://example.org/t'\nconfidence: 0.5\nsources:\n  - {{ id: a, resource: 'https://x.example', rel: evidence, locator: p1 }}\n{extra}---\nbody {tail}\n")
    };
    write(
        &repo.join("working/Old.md"),
        &belief("old1", "approved", ""),
    );
    write(
        &repo.join("working/New.md"),
        &belief(
            "new1",
            "draft",
            "disputes:\n  - { page: urn:visionflow:page:old1, detected: '2026-11-10T00:00:00Z' }\n",
        ),
    );
    let o = run(
        &repo,
        &[
            "memory",
            "approve",
            "New",
            "--resolve",
            "urn:visionflow:page:old1=world-changed",
            "--by",
            HUMAN,
            "--now",
            NOW,
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v = json(&o);
    assert_eq!(v["writes"].as_array().unwrap().len(), 2);
    let old = std::fs::read_to_string(repo.join("working/Old.md")).unwrap();
    let new = std::fs::read_to_string(repo.join("working/New.md")).unwrap();
    assert!(
        old.contains("status: superseded")
            && old.contains("superseded-by:")
            && old.contains("valid-to:"),
        "{old}"
    );
    assert!(
        new.contains("status: approved")
            && new.contains("supersedes:")
            && !new.contains("disputes:"),
        "{new}"
    );
    let val = run(&repo, &["validate", "--vault", "all"]);
    assert!(
        val.status.success(),
        "{}{}",
        String::from_utf8_lossy(&val.stdout),
        String::from_utf8_lossy(&val.stderr)
    );
}
