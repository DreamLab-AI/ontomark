//! Shared ADR-2024 review binding conformance, independent of host adapters.
use ontomark_parse::{
    certification::{assess, content_digest, freshness, Freshness},
    Document,
};

fn document(value: &str, stamps: &str) -> Document {
    Document::parse(format!("---\nvalue: {value}\nverified:\n{stamps}---\nbody"))
}
fn stamp(actor: &str, keys: &str, binding: &str) -> String {
    format!("  - by: {actor}\n    at: 2026-11-15\n    keys: {keys}\n{binding}")
}
#[test]
fn mixed_stamps_keep_historical_protection_without_inventing_freshness() {
    let keys = vec!["value".to_owned()];
    let digest = content_digest(&document("old", ""), "verified", Some(&keys)).unwrap();
    let stamps = [
        stamp(
            "human:reviewer",
            "[value]",
            &format!("    content_digest: {digest}\n"),
        ),
        stamp(
            "human:reviewer",
            "[value]",
            "    content_digest: sha256-v1:bad\n",
        ),
        stamp("human:reviewer", "[other]", ""),
        stamp("agent:worker", "[secret]", ""),
        stamp("process:compiler", "[secret]", ""),
    ]
    .concat();
    let result = assess(&document("old", &stamps), "verified");
    assert_eq!((result.current, result.stale, result.unbound), (1, 1, 1));
    assert_eq!(
        result.keys.into_iter().collect::<Vec<_>>(),
        ["other", "value"]
    );
    let changed = assess(&document("changed", &stamps), "verified");
    assert_eq!((changed.current, changed.stale, changed.unbound), (0, 2, 1));
    assert!(changed.keys.contains("value"));
}
#[test]
fn malformed_scopes_and_digest_types_never_become_current() {
    for keys in ["[]", "[value, 1]", "[verified]", "wrong", "['']"] {
        let result = assess(
            &document("x", &stamp("human:reviewer", keys, "")),
            "verified",
        );
        assert_eq!(
            (result.current, result.stale, result.unbound),
            (0, 1, 0),
            "{keys}"
        );
        assert!(!result.whole_page);
        if keys == "[value, 1]" {
            assert!(result.keys.contains("value"));
        }
    }
    let result = assess(
        &document(
            "x",
            &stamp("human:reviewer", "[value]", "    content_digest: 12\n"),
        ),
        "verified",
    );
    assert_eq!(result.stale, 1);
    assert!(result.keys.contains("value"));
}
#[test]
fn precise_floats_types_missing_values_and_body_are_distinct() {
    let keys = vec!["value".to_owned()];
    let values = [
        "1.00000000000000001",
        "1.00000000000000002",
        "1",
        "'1'",
        "null",
        "[1, 2]",
        "[2, 1]",
    ];
    let digests: std::collections::BTreeSet<_> = values
        .iter()
        .map(|v| content_digest(&document(v, ""), "verified", Some(&keys)).unwrap())
        .collect();
    assert_eq!(digests.len(), values.len());
    let missing = Document::parse("---\nother: 1\n---\nbody");
    assert!(!digests.contains(&content_digest(&missing, "verified", Some(&keys)).unwrap()));
    let whole = content_digest(&missing, "verified", None).unwrap();
    assert_eq!(
        freshness(
            &Document::parse("---\nother: 1\n---\nedited"),
            "verified",
            None,
            Some(&whole)
        ),
        Freshness::Stale
    );
}
