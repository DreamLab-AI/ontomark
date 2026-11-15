//! Provenance survives the compiler while legacy contexts remain frozen.
use ontomark_compile::input::okf::Stamp;

#[test]
fn stamp_round_trip_retains_scope_and_digest() {
    let yaml: serde_norway::Value = serde_norway::from_str(
        "by: human:reviewer\nat: 2026-11-15\nkeys: [definition]\ncontent_digest: sha256-v1:123\n",
    )
    .unwrap();
    assert_eq!(Stamp::from_yaml(&yaml).unwrap().to_yaml(), yaml);
}
