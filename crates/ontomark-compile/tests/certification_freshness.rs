//! Compiler/portable-parser content-binding conformance.
use ontomark_compile::input::{okf::Certification, page::Page};
use ontomark_parse::{certification::content_digest, Document};

#[test]
fn compiler_checks_the_same_binding_as_the_portable_parser() {
    let original = Document::parse("---\ntitle: A\nvalue: old\n---\nbody");
    let keys = vec!["value".to_owned()];
    let digest = content_digest(&original, "verified", Some(&keys)).unwrap();
    let page = |value: &str| {
        Page::parse("a.md", "a.md", "a", &format!("---\ntitle: A\nvalue: {value}\nverified:\n  - by: human:npub1test\n    at: 2026-11-15\n    keys: [value]\n    content_digest: {digest}\n---\nbody")).unwrap()
    };
    assert_eq!(Certification::for_page(&page("old")).current, 1);
    let changed = Certification::for_page(&page("new"));
    assert_eq!(changed.stale, 1);
    assert_eq!(changed.keys, ["value"]);
}

#[test]
fn compiler_identity_does_not_rewrite_legacy_provenance() {
    use ontomark_compile::build::generation::generation;
    let g = generation(
        "test",
        "unknown".into(),
        false,
        "0".repeat(64),
        "sha256-12-000000000000".into(),
        "2026-11-15T00:00:00Z".into(),
        0,
        0,
        1,
        None,
        vec![],
    );
    let mut json = serde_json::to_value(&g).unwrap();
    assert_eq!(json["built_by"]["name"], "ontomark-compile");
    json.as_object_mut().unwrap().remove("built_by");
    let legacy: ontomark_compile::build::generation::Generation =
        serde_json::from_value(json).unwrap();
    assert!(legacy.built_by.is_none());
    assert_eq!(legacy.id, g.id);
    assert_eq!(legacy.content_digest, g.content_digest);
}

#[test]
fn compiler_never_rounds_a_reviewed_float_before_checking_it() {
    let source = "---\nvalue: 1.00000000000000001\n---\nbody";
    let digest = content_digest(&Document::parse(source), "verified", None).unwrap();
    let stamped = source.replace("\n---\nbody", &format!("\nverified:\n  - by: human:test\n    at: 2026-11-15\n    content_digest: {digest}\n---\nbody"));
    let page = Page::parse("a.md", "a.md", "a", &stamped).unwrap();
    assert_eq!(Certification::for_page(&page).current, 1);
    let changed = Page::parse(
        "a.md",
        "a.md",
        "a",
        &stamped.replace("1.00000000000000001", "1.00000000000000002"),
    )
    .unwrap();
    assert_eq!(Certification::for_page(&changed).stale, 1);
}
