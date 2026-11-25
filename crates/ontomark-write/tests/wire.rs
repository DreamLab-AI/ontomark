//! The JSON facade the plugin's worker calls.

mod common;

use common::*;
use ontomark_write::wire::{execute_json, hex32};

fn field(v: &serde_json::Value, k: &str) -> serde_json::Value {
    v.get(k).cloned().unwrap_or(serde_json::Value::Null)
}

#[test]
fn create_over_json_returns_a_plan_the_host_can_apply() {
    let e = engine(&[]);
    let req = serde_json::json!({
        "kind": "create",
        "id": "notes/Wire",
        "page_type": "Note",
        "fields": [["tags", {"t": "seq", "v": [{"t": "string", "v": "x"}]}]],
        "body": "hello\n",
        "writer": {"by": HUMAN, "via": AGENT},
        "now": T1,
        "entropy": "11".repeat(32),
    });
    let out: serde_json::Value = serde_json::from_str(&execute_json(&e, &req.to_string())).unwrap();
    assert_eq!(field(&out, "status"), "planned", "{out}");
    let w = &out["plan"]["writes"][0];
    assert_eq!(w["vault"], "working");
    assert_eq!(w["rel_path"], "notes/Wire.md");
    assert!(w["before_text"].is_null());
    assert!(w["text"].as_str().unwrap().contains("status: draft"));
}

#[test]
fn transition_over_json_uses_the_flattened_move() {
    let src = belief("p1", "approved", "");
    let e = engine(&[("b/P", &src)]);
    let req = serde_json::json!({
        "kind": "transition", "id": "b/P", "move": "confirm",
        "writer": {"by": HUMAN}, "now": T2,
    });
    let out: serde_json::Value = serde_json::from_str(&execute_json(&e, &req.to_string())).unwrap();
    assert_eq!(out["status"], "planned", "{out}");
    assert!(out["plan"]["writes"][0]["text"]
        .as_str()
        .unwrap()
        .contains("last-reviewed"));
}

#[test]
fn refusals_and_bad_json_are_refusals_not_panics() {
    let e = engine(&[]);
    let out: serde_json::Value = serde_json::from_str(&execute_json(&e, "not json")).unwrap();
    assert_eq!(out["status"], "refused");
    assert_eq!(out["code"], "INVALID_REQUEST");
    let req = serde_json::json!({"kind": "transition", "id": "nope", "move": "confirm", "writer": {"by": HUMAN}, "now": T2});
    let out: serde_json::Value = serde_json::from_str(&execute_json(&e, &req.to_string())).unwrap();
    assert_eq!(out["code"], "NOT_FOUND");
    // A vault with no vocabulary version 3 refuses writes rather than guessing.
    let v2 = engine_with(VOCAB_V2, false, &[], &[]);
    let req = serde_json::json!({"kind": "adopt", "id": "x", "writer": {"by": HUMAN}, "now": T2, "entropy": "00".repeat(32)});
    let out: serde_json::Value =
        serde_json::from_str(&execute_json(&v2, &req.to_string())).unwrap();
    assert_eq!(out["code"], "VOCABULARY_V2");
}

#[test]
fn entropy_must_be_exactly_32_bytes_of_hex() {
    #[derive(serde::Deserialize)]
    struct W {
        #[serde(with = "hex32")]
        e: [u8; 32],
    }
    assert!(serde_json::from_str::<W>(&format!("{{\"e\":\"{}\"}}", "ab".repeat(32))).is_ok());
    assert!(serde_json::from_str::<W>("{\"e\":\"abcd\"}").is_err());
    assert!(serde_json::from_str::<W>("{\"e\":\"zz\"}").is_err());
}
