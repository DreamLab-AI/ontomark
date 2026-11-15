//! ADR-2024: explicit review preparation never writes or grants certification.
use ontomark_parse::{certification::content_digest, Document};
use std::process::Command;

#[test]
fn review_binding_reads_exact_source_and_never_writes_a_stamp() {
    let dir = tempfile::tempdir().unwrap();
    let source = "---\ntitle: Review\nvalue: 1.00000000000000001\n---\nprivate body canary";
    let file = dir.path().join("Review.md");
    std::fs::write(&file, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(dir.path())
        .args(["--json", "review-binding", "Review", "--key", "value"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let keys = vec!["value".to_owned()];
    assert_eq!(
        result["content_digest"],
        content_digest(&Document::parse(source), "verified", Some(&keys)).unwrap()
    );
    assert_eq!(result["writes_stamp"], false);
    assert_eq!(std::fs::read_to_string(file).unwrap(), source);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private body canary"));
}

#[test]
fn built_artifacts_preserve_review_without_leaking_private_pages() {
    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path().join("vault");
    let out = dir.path().join("output");
    std::fs::create_dir(&vault).unwrap();
    let source = "---\ntitle: Public\ntype: class\npublic: true\ndefinition: reviewed\ngenerated:\n  by: agent:writer/1\n  at: 2026-11-15\n---\nbody";
    let keys = vec!["definition".to_owned()];
    let digest = content_digest(&Document::parse(source), "verified", Some(&keys)).unwrap();
    let source = source.replace("\n---\nbody", &format!("\nverified:\n  - by: human:reviewer\n    at: 2026-11-15\n    keys: [definition]\n    content_digest: {digest}\n---\nbody"));
    std::fs::write(vault.join("Public.md"), source).unwrap();
    std::fs::write(
        vault.join("Secret.md"),
        "---\ntitle: SECRET_CANARY\ntype: class\npublic: false\n---\nprivate body",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ontomark"))
        .arg("--repo")
        .arg(&vault)
        .args(["build", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let read = |path: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(out.join(path)).unwrap()).unwrap()
    };
    let page = read("api/v3/pages/public.json");
    assert_eq!(page["reviewBinding"]["current"], 1);
    assert_eq!(page["reviewBinding"]["stale"], 0);
    assert!(read("api/pages/public.json").get("reviewBinding").is_none());
    let scaffold = read("data/scaffold-index.json");
    assert_eq!(
        scaffold["classes"]["public"]["verified"][0]["content_digest"],
        digest
    );
    assert_eq!(
        scaffold["classes"]["public"]["generated"]["by"],
        "agent:writer/1"
    );
    assert_eq!(
        scaffold["classes"]["public"]["review_binding"]["current"],
        1
    );
    assert!(!scaffold.to_string().contains("SECRET_CANARY"));
    assert!(!out.join("api/v3/pages/secret.json").exists());
    let context = read("context/v3.jsonld");
    assert!(context["@context"]["reviewBinding"].is_object());
    assert!(read("context/v2.jsonld")["@context"]
        .get("reviewBinding")
        .is_none());
    assert_eq!(
        read(".generation.json")["built_by"]["name"],
        "ontomark-compile"
    );
}
