//! Shared reader contracts: emitted C3 markers and Loom indexes have one owner.

use ontomark_compile::build::generation::{generation, Artifact};
use ontomark_compile::bundle::{BundleManifest, ClassEntry, ProseIndex, ScaffoldIndex};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

fn marker() -> Value {
    serde_json::to_value(generation(
        "visionGraph",
        "0123456789abcdef0123456789abcdef01234567".into(),
        false,
        "a".repeat(64),
        "sha256-12-0123456789ab".into(),
        "2026-10-11T00:00:00.000000+00:00".into(),
        1,
        1,
        1,
        None,
        vec![Artifact::of("data/ontology.ttl", b"ttl")],
    ))
    .unwrap()
}

#[test]
fn c3_reader_round_trips_the_existing_emitter_without_a_new_envelope() {
    let emitted = marker();
    let parsed = BundleManifest::from_json(&emitted.to_string()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), emitted);
    let scoped = parsed.scoped("data");
    assert_eq!(scoped.artifacts[0].name, "ontology.ttl");
    assert_eq!(scoped.content_digest, parsed.content_digest);
    assert_eq!(scoped.ontology_digest, parsed.ontology_digest);
    scoped.validate().unwrap();
}

#[test]
fn incomplete_or_mixed_markers_are_refused_without_defaults() {
    for field in [
        "id",
        "commit",
        "dirty",
        "content_digest",
        "ontology_digest",
        "generated_at",
        "class_count",
        "page_count",
        "vocabulary_version",
        "artifacts",
    ] {
        let mut bad = marker();
        bad.as_object_mut().unwrap().remove(field);
        assert!(
            BundleManifest::from_json(&bad.to_string()).is_err(),
            "missing {field}"
        );
    }
    let mut mixed = marker();
    mixed["commit"] = json!("f".repeat(40));
    assert!(BundleManifest::from_json(&mixed.to_string()).is_err());
    let mut dirty = marker();
    dirty["dirty"] = json!(true);
    assert!(BundleManifest::from_json(&dirty.to_string()).is_err());
    let mut legacy = marker();
    legacy["artifacts"] = json!({"ontology.ttl": "a".repeat(64)});
    assert!(BundleManifest::from_json(&legacy.to_string()).is_err());
}

#[test]
fn the_generation_prefix_is_read_from_the_id_not_assumed() {
    // A reader needs no profile: whatever producer spelled the prefix, the id
    // must be `<prefix>@<commit>[+dirty]` for the marker's own commit and flag.
    let commit = "0123456789abcdef0123456789abcdef01234567";
    for prefix in ["visionGraph", "Field Notes", "a@b"] {
        let mut m = marker();
        m["id"] = json!(format!("{prefix}@{commit}"));
        BundleManifest::from_json(&m.to_string()).unwrap_or_else(|e| panic!("{prefix}: {e}"));
        m["id"] = json!(format!("{prefix}@{commit}+dirty"));
        m["dirty"] = json!(true);
        BundleManifest::from_json(&m.to_string()).unwrap_or_else(|e| panic!("{prefix}: {e}"));
    }
    for bad_id in [
        format!("@{commit}"),                  // an empty prefix
        commit.to_owned(),                     // no prefix at all
        format!("visionGraph@{commit}x"),      // a different commit
        format!("visionGraph@{commit}+dirty"), // dirty flag disagrees
    ] {
        let mut m = marker();
        m["id"] = json!(bad_id);
        assert!(
            BundleManifest::from_json(&m.to_string()).is_err(),
            "{bad_id}"
        );
    }
}

#[test]
fn artifact_paths_hashes_and_claims_are_checked_before_a_consumer_opens_files() {
    for path in [
        "",
        "/etc/passwd",
        "../x",
        "data/../x",
        "./x",
        "data\\x",
        "x//y",
    ] {
        let mut bad = marker();
        bad["artifacts"][0]["name"] = json!(path);
        assert!(
            BundleManifest::from_json(&bad.to_string()).is_err(),
            "{path}"
        );
    }
    for hash in ["a", &"A".repeat(64), &"z".repeat(64)] {
        let mut bad = marker();
        bad["artifacts"][0]["sha256"] = json!(hash);
        assert!(
            BundleManifest::from_json(&bad.to_string()).is_err(),
            "{hash}"
        );
    }
    let mut duplicate = marker();
    let entry = duplicate["artifacts"][0].clone();
    duplicate["artifacts"].as_array_mut().unwrap().push(entry);
    assert!(BundleManifest::from_json(&duplicate.to_string()).is_err());
}

#[test]
fn actual_fixture_indexes_deserialize_with_legacy_omissions_and_keep_wire_fields() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/estate-small.golden/expected");
    // The fixture path is discovered by name because the golden bundles were
    // captured before this reader API existed.
    let scaffold_path = walkdir::WalkDir::new(&fixture)
        .into_iter()
        .flatten()
        .find(|e| e.file_name() == "scaffold-index.json")
        .unwrap()
        .into_path();
    let input = std::fs::read_to_string(scaffold_path).unwrap();
    let index: ScaffoldIndex = serde_json::from_str(&input).unwrap();
    assert_eq!(index.version, Some(1));
    assert!(!index.classes.is_empty());
    let roundtrip: Value = serde_json::to_value(&index).unwrap();
    assert_eq!(roundtrip, serde_json::from_str::<Value>(&input).unwrap());
    let legacy: ScaffoldIndex = serde_json::from_str(r#"{"classes":{"a":{"t":"A"}}}"#).unwrap();
    assert_eq!(legacy.classes["a"].t.as_deref(), Some("A"));
    assert!(legacy.classes["a"].sup.is_empty());
    assert!(legacy.classes["a"].rel.is_empty());
    let prose: ProseIndex =
        serde_json::from_str(r#"{"pages":{"a":{"dfull":"Long","cl":"Now"}}}"#).unwrap();
    assert_eq!(prose.pages["a"].cl.as_deref(), Some("Now"));
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ConsumerAttestation {
    by: String,
}

#[test]
fn index_trust_values_are_typed_by_the_consumer_without_duplicating_the_wire_schema() {
    // Empty records must not require fabricated defaults for governance types.
    let empty: ClassEntry<ConsumerAttestation, ConsumerAttestation, ConsumerAttestation> =
        ClassEntry::default();
    assert!(empty.generated.is_none());
    assert!(empty.verified.is_empty());
    let value = json!({"version":1,"generated":"t","classes":{"a":{
        "t":"A","generated":{"by":"process:ontomark/0.1.0"},
        "verified":[{"by":"human:reviewer"}],"map":[{"resource":"urn:a"}],
        "ac":[{"runtime":"urn:runtime"}]
    }}});
    let index: ScaffoldIndex<ConsumerAttestation, Value, Value> =
        serde_json::from_value(value).unwrap();
    assert_eq!(
        index.classes["a"].generated.as_ref().unwrap().by,
        "process:ontomark/0.1.0"
    );
    assert_eq!(index.classes["a"].verified[0].by, "human:reviewer");
    assert_eq!(index.classes["a"].map[0]["resource"], "urn:a");
}

#[test]
fn the_emitted_marker_pair_and_prose_index_round_trip_through_the_public_readers() {
    let expected = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/estate-small.golden/expected");
    let read = |rel: &str| std::fs::read_to_string(expected.join(rel)).unwrap();

    // The bundle carries its stamp twice: the root marker lists every artefact
    // and the one beside the served index is the same generation seen from
    // inside data/ (Generation::scoped).
    let top = BundleManifest::from_json(&read("out/.generation.json")).unwrap();
    let data = BundleManifest::from_json(&read("out/data/.generation.json")).unwrap();
    assert_eq!(data.id, top.id);
    assert_eq!(data.commit, top.commit);
    assert_eq!(data.content_digest, top.content_digest);
    assert_eq!(data.ontology_digest, top.ontology_digest);
    assert!(!data.artifacts.is_empty());
    for artifact in &data.artifacts {
        let claimed = top
            .artifacts
            .iter()
            .find(|a| a.name == format!("data/{}", artifact.name))
            .unwrap_or_else(|| panic!("{} is not claimed by the bundle marker", artifact.name));
        assert_eq!(claimed.sha256, artifact.sha256);
        assert_eq!(claimed.bytes, artifact.bytes);
    }

    // The prose index round-trips through its reader exactly like the scaffold
    // index does: no wire field is dropped or invented.
    let prose_input = read("out/data/prose-index.json");
    let prose: ProseIndex = serde_json::from_str(&prose_input).unwrap();
    assert!(!prose.pages.is_empty());
    let roundtrip: Value = serde_json::to_value(&prose).unwrap();
    assert_eq!(roundtrip, serde_json::from_str::<Value>(&prose_input).unwrap());
}
