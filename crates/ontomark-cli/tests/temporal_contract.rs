//! The estate temporal contract, executed (`conformance/temporal/`).
//!
//! Loads both vector files, checks their SHA-256 pins (and that the README
//! quotes the same pins), validates the closure records, and asserts every
//! case against a deliberately plain reference evaluator of the v2 state rule.
//! The evaluator is not a port of any consumer's SPARQL: it applies the rule
//! one clause at a time to each version in turn.
//!
//! A negative test shows the check can fail: the v1 rule, and a rule with a
//! mistaken boundary, both disagree with the v2 vectors.
//!
//! Timestamps in the vectors are canonical UTC (`YYYY-MM-DDTHH:MM:SSZ`), so
//! comparing them as strings orders them in time; the loader checks the shape.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const V1_SHA256: &str = "6973aeef35e538412375dd6e4a9d9539f12f88fa7513d237371098da3bc46f75";
const V2_SHA256: &str = "7b75b8b7f2aa3ec5de19841a1ff27cefd5d08db66490e0c2ad46525da4059d8f";

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/temporal")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Which rule an evaluation applies. `V2` is the contract; the others exist to
/// prove the check can fail.
#[derive(Clone, Copy, PartialEq)]
enum Rule {
    /// The contract (README, "The state rule").
    V2,
    /// The v1 rule: a closure ends the interval whenever it was recorded, and
    /// recorded-time closures do not exist.
    V1,
    /// V2 with a strict `<` where the contract says `<=` on recorded time.
    V2StrictRecorded,
}

struct Version {
    id: String,
    valid_from: String,
    generated_at: String,
}

/// A closure, with the inline `validTo` shorthand already expanded.
struct Closure {
    version: String,
    closes_valid: bool,
    closes_recorded: bool,
    valid_until: Option<String>,
    recorded_at: String,
}

struct Doc {
    versions: Vec<Version>,
    closures: Vec<Closure>,
    cases: Vec<Case>,
}

struct Case {
    name: String,
    t: String,
    recorded_as_of: Option<String>,
    expect: BTreeSet<String>,
}

fn text(v: &Value, key: &str) -> String {
    let s = v[key]
        .as_str()
        .unwrap_or_else(|| panic!("missing string `{key}` in {v}"))
        .to_string();
    assert_instant(&s);
    s
}

fn assert_instant(s: &str) {
    let b = s.as_bytes();
    let ok = b.len() == 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7 | 10 | 13 | 16 | 19) || c.is_ascii_digit());
    assert!(ok, "not a canonical UTC second-precision instant: {s}");
}

fn load(name: &str) -> Doc {
    let root: Value = serde_json::from_str(&read(name)).expect("valid JSON");
    let mut versions = Vec::new();
    let mut closures = Vec::new();
    for v in root["versions"].as_array().expect("versions") {
        let id = v["id"].as_str().expect("version id").to_string();
        assert!(v.get("closes").is_none(), "{id}: a version carries no end");
        versions.push(Version {
            id: id.clone(),
            valid_from: text(v, "validFrom"),
            generated_at: text(v, "generatedAt"),
        });
        if v.get("validTo").is_some() {
            // Shorthand: a valid-time closure recorded at generatedAt.
            closures.push(Closure {
                version: id,
                closes_valid: true,
                closes_recorded: false,
                valid_until: Some(text(v, "validTo")),
                recorded_at: text(v, "generatedAt"),
            });
        }
    }
    for c in root["closures"].as_array().unwrap_or(&Vec::new()) {
        let id = c["id"].as_str().expect("closure id");
        let clocks: Vec<&str> = c["closes"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: closes"))
            .iter()
            .map(|x| x.as_str().expect("clock name"))
            .collect();
        assert!(
            !clocks.is_empty(),
            "{id}: a closure closes at least one clock"
        );
        assert!(
            clocks.iter().all(|k| matches!(*k, "valid" | "recorded")),
            "{id}: unknown clock"
        );
        let closes_valid = clocks.contains(&"valid");
        // `validUntil` is present iff the closure closes valid time.
        assert_eq!(
            c.get("validUntil").is_some(),
            closes_valid,
            "{id}: validUntil iff valid time is closed"
        );
        assert!(
            c.get("subject").is_none() && c.get("predicate").is_none(),
            "{id}: a closure is not a version"
        );
        if let Some(r) = c.get("replacedBy") {
            let r = r.as_str().expect("replacedBy");
            assert!(versions.iter().any(|v| v.id == r), "{id}: replacedBy {r}");
        }
        closures.push(Closure {
            version: c["version"].as_str().expect("version").to_string(),
            closes_valid,
            closes_recorded: clocks.contains(&"recorded"),
            valid_until: c.get("validUntil").map(|_| text(c, "validUntil")),
            recorded_at: text(c, "recordedAt"),
        });
    }
    for c in &closures {
        assert!(
            versions.iter().any(|v| v.id == c.version),
            "closure names unknown version {}",
            c.version
        );
    }
    let cases = root["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|c| Case {
            name: c["name"].as_str().expect("case name").to_string(),
            t: text(c, "t"),
            recorded_as_of: c.get("recordedAsOf").map(|_| text(c, "recordedAsOf")),
            expect: c["expect"]
                .as_array()
                .expect("expect")
                .iter()
                .map(|x| x.as_str().expect("id").to_string())
                .collect(),
        })
        .collect();
    Doc {
        versions,
        closures,
        cases,
    }
}

/// `state(t, r)`: the ids of the versions in state, by testing each version
/// against each clause of the rule.
fn state(doc: &Doc, t: &str, r: Option<&str>, rule: Rule) -> BTreeSet<String> {
    // A closure applies at knowledge cut `r` iff it was recorded by then.
    let applies = |c: &Closure| match r {
        None => true,
        Some(r) if rule == Rule::V2StrictRecorded => c.recorded_at.as_str() < r,
        Some(r) => c.recorded_at.as_str() <= r,
    };
    let mut out = BTreeSet::new();
    for v in &doc.versions {
        if let Some(r) = r {
            if v.generated_at.as_str() > r {
                continue; // not yet recorded
            }
        }
        if v.valid_from.as_str() > t {
            continue; // not yet valid
        }
        let mine: Vec<&Closure> = doc.closures.iter().filter(|c| c.version == v.id).collect();
        let recorded_out = rule != Rule::V1 && mine.iter().any(|c| c.closes_recorded && applies(c));
        let valid_out = mine.iter().any(|c| {
            c.closes_valid
                && (rule == Rule::V1 || applies(c))
                && c.valid_until.as_deref().is_some_and(|u| u <= t)
        });
        if !recorded_out && !valid_out {
            out.insert(v.id.clone());
        }
    }
    out
}

fn failures(doc: &Doc, rule: Rule) -> Vec<String> {
    doc.cases
        .iter()
        .filter(|c| state(doc, &c.t, c.recorded_as_of.as_deref(), rule) != c.expect)
        .map(|c| c.name.clone())
        .collect()
}

#[test]
fn pins_match_files_and_readme() {
    assert_eq!(
        sha256_hex(&read("vectors.json")),
        V1_SHA256,
        "v1 file changed"
    );
    assert_eq!(
        sha256_hex(&read("vectors-v2.json")),
        V2_SHA256,
        "v2 file changed: bump the pin here and in the README, and every consumer"
    );
    let readme = read("README.md");
    for pin in [V1_SHA256, V2_SHA256] {
        assert!(readme.contains(pin), "README does not quote pin {pin}");
    }
}

#[test]
fn v2_vectors_hold() {
    let doc = load("vectors-v2.json");
    assert!(doc.cases.len() >= 30);
    let bad = failures(&doc, Rule::V2);
    assert!(bad.is_empty(), "cases failing: {bad:#?}");
}

#[test]
fn v1_vectors_hold_under_both_rules() {
    let doc = load("vectors.json");
    assert_eq!(doc.cases.len(), 9);
    assert!(failures(&doc, Rule::V1).is_empty());
    assert!(failures(&doc, Rule::V2).is_empty());
}

#[test]
fn v2_keeps_the_v1_versions_and_cases_unchanged() {
    let v1: Value = serde_json::from_str(&read("vectors.json")).unwrap();
    let v2: Value = serde_json::from_str(&read("vectors-v2.json")).unwrap();
    assert_eq!(v2["contract"], "estate-temporal-v2");
    assert_eq!(
        v1["versions"].as_array().unwrap()[..],
        v2["versions"].as_array().unwrap()[..3]
    );
    assert_eq!(
        v1["cases"].as_array().unwrap()[..],
        v2["cases"].as_array().unwrap()[..9]
    );
}

#[test]
fn the_check_can_fail() {
    let doc = load("vectors-v2.json");
    // The v1 rule applied to a v2 case: the backdated close was recorded after
    // this cut, so the old version is still believed; v1 ends it regardless.
    let name = "knowledge cut before the backdated close was recorded still sees the old version";
    let bad = failures(&doc, Rule::V1);
    assert!(bad.iter().any(|n| n == name), "v1 rule passed {name}");
    // ...and it also cannot express a correction.
    assert!(bad
        .iter()
        .any(|n| n.starts_with("correction with replacement")));
    // A strict recorded-time comparison fails the inclusive-boundary cases.
    let strict = failures(&doc, Rule::V2StrictRecorded);
    assert!(
        strict.iter().any(|n| n.contains("(<=)")),
        "no boundary case caught `<` for `<=`"
    );
}
