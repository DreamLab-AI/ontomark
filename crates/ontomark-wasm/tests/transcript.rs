//! Replays `fixtures/script.json` against the native engine over the
//! synthetic vault in `fixtures/vault` and compares every response with
//! `fixtures/transcript.golden.jsonl`. The wasm harness (`plugin/test/wasm-harness.test.mjs`)
//! replays the same script through the built `.wasm` and must produce the
//! same bytes (EXP-01-style native/wasm parity for the engine surface).
//!
//! Set `ONTOMARK_BLESS=1` to rewrite the golden after an intended change.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ontomark_parse::SourceDigest;
use ontomark_wasm::engine::Engine;
use serde_json::{json, Value};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_vault(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            read_vault(root, &p, out);
        } else {
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, std::fs::read_to_string(&p).unwrap());
        }
    }
}

/// Run the script; returns one response line per step.
fn replay() -> Vec<String> {
    let root = fixtures().join("vault");
    let mut files = BTreeMap::new();
    read_vault(&root, &root, &mut files);
    let script: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(fixtures().join("script.json")).unwrap())
            .unwrap();
    let mut engine = Engine::new();
    let mut lines = Vec::new();
    for mut step in script {
        if let Some(raw) = step.get("$raw").and_then(Value::as_str) {
            lines.push(engine.handle_json(raw));
            continue;
        }
        if step["files"] == "@vault" {
            step["files"] = files
                .iter()
                .map(|(p, t)| json!({"path": p, "text": t}))
                .collect();
        }
        if step["base_digest"] == "@digest" {
            let path = step["path"].as_str().unwrap();
            step["base_digest"] = json!(SourceDigest::of(&files[path]).to_string());
        }
        // `@plan:N` feeds step N's `ingest_plan` result back as the request.
        if let Some(n) = step["request"]
            .as_str()
            .and_then(|s| s.strip_prefix("@plan:"))
            .and_then(|n| n.parse::<usize>().ok())
        {
            let planned: Value = serde_json::from_str(&lines[n]).unwrap();
            step["request"] = planned["result"]["request"].clone();
        }
        let line = engine.handle_json(&step.to_string());
        let ok = serde_json::from_str::<Value>(&line).unwrap()["ok"] == true;
        if ok {
            match step["kind"].as_str() {
                Some("upsert") => {
                    files.insert(
                        step["path"].as_str().unwrap().into(),
                        step["text"].as_str().unwrap().into(),
                    );
                }
                Some("remove") => {
                    files.remove(step["path"].as_str().unwrap());
                }
                Some("rename") => {
                    let text = files.remove(step["from"].as_str().unwrap()).unwrap();
                    files.insert(step["to"].as_str().unwrap().into(), text);
                }
                _ => {}
            }
        }
        lines.push(line);
    }
    lines
}

#[test]
fn transcript_matches_golden() {
    let lines = replay();
    let golden_path = fixtures().join("transcript.golden.jsonl");
    let got = lines.join("\n") + "\n";
    if std::env::var_os("ONTOMARK_BLESS").is_some() {
        std::fs::write(&golden_path, &got).unwrap();
    }
    let want =
        std::fs::read_to_string(&golden_path).expect("golden missing: run with ONTOMARK_BLESS=1");
    assert_eq!(got, want, "native transcript drifted from the golden");
}

fn step(lines: &[String], i: usize) -> Value {
    serde_json::from_str(&lines[i]).unwrap()
}

#[test]
fn transcript_semantics() {
    let l = replay();
    // EXP-05: plain notes and prose links produce nothing; typed pages do.
    let index = step(&l, 2);
    let entities = index["result"]["entities"].as_array().unwrap();
    let paths: Vec<&str> = entities
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert!(!paths.contains(&"Welcome.md") && !paths.contains(&"Notes/Daily idea.md"));
    assert_eq!(entities.len(), 10, "{paths:?}");
    let codes: Vec<&str> = index["result"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"frontmatter_unreadable"), "{codes:?}");
    assert!(codes.contains(&"unresolved_reference"), "{codes:?}");
    // The non-ASCII unresolved link carries correct UTF-16 coordinates.
    let chien = index["result"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["path"] == "Ontology/Chien.md")
        .unwrap();
    let src = std::fs::read_to_string(fixtures().join("vault/Ontology/Chien.md")).unwrap();
    let utf16: Vec<u16> = src.encode_utf16().collect();
    let (a, b) = (
        chien["span"]["utf16_start"].as_u64().unwrap() as usize,
        chien["span"]["utf16_end"].as_u64().unwrap() as usize,
    );
    assert_eq!(String::from_utf16(&utf16[a..b]).unwrap(), "[[Nowhere Ü]]");
    // Inferred dataset refused before classification, available after.
    assert_eq!(step(&l, 4)["error"]["code"], "not_classified");
    let classified = step(&l, 5);
    let cls = &classified["result"]["classification"];
    assert_eq!(cls["reasoner"]["id"], "ontomark-reason/whelk-el");
    assert_eq!(cls["reasoner"]["el_complete"], false);
    assert_eq!(cls["policy"], "all");
    // The `range` on `has owner` is EL but not implemented by whelk-el: refused
    // whole, reported, and the result is marked incomplete.
    assert_eq!(cls["complete"], false);
    let refused = cls["unsupported"].as_array().unwrap();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["axiom"]["t"], "range");
    assert!(refused[0]["reason"]
        .as_str()
        .unwrap()
        .starts_with("EL_NOT_IMPLEMENTED"));
    let unsat = classified["result"]["classification"]["unsatisfiable"]
        .as_array()
        .unwrap();
    assert_eq!(unsat, &vec![json!("urn:ontomark:page:Ontology/Catdog")]);
    let rex_types = step(&l, 6)["result"]["rows"].as_array().unwrap().len();
    assert_eq!(
        rex_types, 5,
        "Rex is a NamedIndividual, Dog, Chien (≡ Dog), Mammal and Animal"
    );
    // EXP-12 refusals.
    assert_eq!(step(&l, 8)["error"]["code"], "service_forbidden");
    assert_eq!(step(&l, 9)["error"]["code"], "update_forbidden");
    // EXP-06: stepping, then an edit makes the old run stale and drops it.
    assert_eq!(step(&l, 10)["result"]["status"], "pending");
    assert_eq!(step(&l, 12)["error"]["code"], "stale");
    assert_eq!(step(&l, 13)["error"]["code"], "no_run");
    assert_eq!(step(&l, 14)["error"]["code"], "stale");
    assert_eq!(step(&l, 15)["error"]["code"], "generation_regressed");
    // Guarded patch: comment and untouched keys survive; stale base and malformed block refused.
    let patch = step(&l, 16);
    let new_text = patch["result"]["new_text"].as_str().unwrap();
    assert!(
        new_text.contains("# an author comment that must survive patches\n"),
        "{new_text}"
    );
    assert!(new_text.contains("aliases: [Creature]\n"), "{new_text}");
    assert!(patch["result"]["diff"]
        .as_str()
        .unwrap()
        .contains("+subclass-of:"));
    assert_eq!(step(&l, 17)["error"]["code"], "stale_base");
    assert_eq!(step(&l, 18)["error"]["code"], "patch_refused");
    // Delete: Rex's types vanish from every dataset.
    assert_eq!(step(&l, 23)["result"]["rows"].as_array().unwrap().len(), 0);
    // Rename keeps resolution by stem: Dog still reaches Animal through Mammal.
    let renamed = step(&l, 25);
    let subs = renamed["result"]["classification"]["subsumptions"]
        .as_array()
        .unwrap();
    assert!(subs
        .iter()
        .any(|s| s["sub"] == "urn:ontomark:page:Ontology/Dog"
            && s["sup"] == "urn:ontomark:page:Ontology/Animal"));
    // EXP-05: estate profile refuses pages without `resource`.
    let estate = step(&l, 27);
    assert_eq!(estate["result"]["entities"].as_array().unwrap().len(), 0);
    assert_eq!(step(&l, 28)["error"]["code"], "bad_request");
    // Frontmatter read: values in the parse crate's wire form; absent block reported.
    let fm = step(&l, 19);
    assert_eq!(fm["result"]["entries"][2]["key"], "aliases");
    assert_eq!(
        fm["result"]["entries"][2]["value"],
        json!({"t": "seq", "v": [{"t": "string", "v": "Creature"}]})
    );
    assert_eq!(step(&l, 20)["result"]["state"], "absent");
}

#[test]
fn reads_never_mutate_and_rebuild_is_identical() {
    // EXP-12: an index rebuilt from the same files answers identically, and
    // running queries leaves the documents byte-identical.
    let a = replay();
    let b = replay();
    assert_eq!(a, b);
}
