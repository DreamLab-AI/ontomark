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
const V2_SHA256: &str = "334425f34523d63224fff92cde1c4268c18cdb989b479c731d271516c905a873";

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

struct Version {
    id: String,
    subject: String,
    predicate: String,
    object: String,
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
    /// Pinned reported ends (`None` = no end), only for the listed ids.
    expect_valid_to: Vec<(String, Option<String>)>,
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
        let spo = |k: &str| v[k].as_str().unwrap_or_default().to_string();
        versions.push(Version {
            id: id.clone(),
            subject: spo("subject"),
            predicate: spo("predicate"),
            object: spo("object"),
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
        let recorded_at = text(c, "recordedAt");
        let target = versions
            .iter()
            .find(|v| Some(v.id.as_str()) == c["version"].as_str())
            .unwrap_or_else(|| panic!("{id}: unknown version"));
        assert!(
            recorded_at >= target.generated_at,
            "{id}: recorded before the version it closes"
        );
        if let Some(u) = c.get("validUntil") {
            assert!(
                u.as_str().unwrap() > target.valid_from.as_str(),
                "{id}: validUntil must follow validFrom"
            );
        }
        assert!(c["agent"].is_string(), "{id}: a closure names its agent");
        closures.push(Closure {
            version: c["version"].as_str().expect("version").to_string(),
            closes_valid,
            closes_recorded: clocks.contains(&"recorded"),
            valid_until: c.get("validUntil").map(|_| text(c, "validUntil")),
            recorded_at,
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
            expect_valid_to: c
                .get("expectValidTo")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| {
                            let end = v.as_str().map(|s| {
                                assert_instant(s);
                                s.to_string()
                            });
                            assert!(end.is_some() || v.is_null(), "expectValidTo: {k}");
                            (k.clone(), end)
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    Doc {
        versions,
        closures,
        cases,
    }
}

/// The contract: `state(t, r)` as the ids of the versions in state, testing
/// each version against each clause of README "The state rule".
fn state(doc: &Doc, t: &str, r: Option<&str>) -> BTreeSet<String> {
    // A closure applies at knowledge cut `r` iff it was recorded by then.
    let applies = |c: &Closure| r.is_none_or(|r| c.recorded_at.as_str() <= r);
    let mut out = BTreeSet::new();
    for v in &doc.versions {
        if r.is_some_and(|r| v.generated_at.as_str() > r) {
            continue; // clause 1: not yet recorded
        }
        if v.valid_from.as_str() > t {
            continue; // clause 2: not yet valid
        }
        let mine: Vec<&Closure> = doc.closures.iter().filter(|c| c.version == v.id).collect();
        // clause 3
        let recorded_out = mine.iter().any(|c| c.closes_recorded && applies(c));
        // clause 4
        let valid_out = mine.iter().any(|c| {
            c.closes_valid && applies(c) && c.valid_until.as_deref().is_some_and(|u| u <= t)
        });
        if !recorded_out && !valid_out {
            out.insert(v.id.clone());
        }
    }
    out
}

/// The `validTo` the contract reports for version `id` at cut `r`: the earliest
/// `validUntil` among the valid-time closures that apply, or none.
fn reported_end(doc: &Doc, id: &str, r: Option<&str>) -> Option<String> {
    doc.closures
        .iter()
        .filter(|c| {
            c.version == id && c.closes_valid && r.is_none_or(|r| c.recorded_at.as_str() <= r)
        })
        .filter_map(|c| c.valid_until.clone())
        .min()
}

/// Wrong rules, used only to show the check can fail.
enum Mutant {
    /// The v1 rule: a closure ends the interval whenever it was recorded, and
    /// recorded-time closures do not exist.
    V1,
    /// The contract with a strict `<` where it says `<=` on recorded time.
    StrictRecorded,
    /// "Latest version per (subject, predicate) wins": a key-value current
    /// value store on top of the closure rules.
    LatestPerSubjectPredicate,
    /// "Dedupe per (subject, predicate, object), keep the latest `generatedAt`".
    LatestPerTriple,
}

fn mutant_state(doc: &Doc, t: &str, r: Option<&str>, m: &Mutant) -> BTreeSet<String> {
    if matches!(
        m,
        Mutant::LatestPerSubjectPredicate | Mutant::LatestPerTriple
    ) {
        let by_triple = matches!(m, Mutant::LatestPerTriple);
        let key = |v: &Version| {
            (
                v.subject.clone(),
                v.predicate.clone(),
                if by_triple {
                    v.object.clone()
                } else {
                    String::new()
                },
            )
        };
        let mut best: std::collections::BTreeMap<(String, String, String), &Version> =
            std::collections::BTreeMap::new();
        let current = state(doc, t, r);
        for v in doc.versions.iter().filter(|v| current.contains(&v.id)) {
            let slot = best.entry(key(v)).or_insert(v);
            if (&v.valid_from, &v.generated_at) > (&slot.valid_from, &slot.generated_at) {
                *slot = v;
            }
        }
        return best.values().map(|v| v.id.clone()).collect();
    }
    let mut out = BTreeSet::new();
    for v in &doc.versions {
        if r.is_some_and(|r| v.generated_at.as_str() > r) || v.valid_from.as_str() > t {
            continue;
        }
        let mine: Vec<&Closure> = doc.closures.iter().filter(|c| c.version == v.id).collect();
        let gone = match m {
            Mutant::LatestPerSubjectPredicate | Mutant::LatestPerTriple => unreachable!(),
            Mutant::V1 => mine.iter().any(|c| {
                c.valid_until
                    .as_deref()
                    .is_some_and(|u| c.closes_valid && u <= t)
            }),
            Mutant::StrictRecorded => {
                let applies = |c: &Closure| r.is_none_or(|r| c.recorded_at.as_str() < r);
                mine.iter().any(|c| {
                    applies(c)
                        && (c.closes_recorded
                            || (c.closes_valid && c.valid_until.as_deref().is_some_and(|u| u <= t)))
                })
            }
        };
        if !gone {
            out.insert(v.id.clone());
        }
    }
    out
}

fn failing(doc: &Doc, eval: impl Fn(&str, Option<&str>) -> BTreeSet<String>) -> Vec<String> {
    doc.cases
        .iter()
        .filter(|c| eval(&c.t, c.recorded_as_of.as_deref()) != c.expect)
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
    assert!(doc.cases.len() >= 41);
    let bad = failing(&doc, |t, r| state(&doc, t, r));
    assert!(bad.is_empty(), "cases failing: {bad:#?}");
}

#[test]
fn v1_vectors_hold_under_both_rules() {
    let doc = load("vectors.json");
    assert_eq!(doc.cases.len(), 9);
    assert!(failing(&doc, |t, r| mutant_state(&doc, t, r, &Mutant::V1)).is_empty());
    assert!(failing(&doc, |t, r| state(&doc, t, r)).is_empty());
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
    let bad = failing(&doc, |t, r| mutant_state(&doc, t, r, &Mutant::V1));
    assert!(bad.iter().any(|n| n == name), "v1 rule passed {name}");
    // ...and it also cannot express a correction.
    assert!(bad
        .iter()
        .any(|n| n.starts_with("correction with replacement")));
    // A strict recorded-time comparison fails the inclusive-boundary cases.
    let strict = failing(&doc, |t, r| {
        mutant_state(&doc, t, r, &Mutant::StrictRecorded)
    });
    assert!(
        strict.iter().any(|n| n.contains("(<=)")),
        "no boundary case caught `<` for `<=`"
    );
}

/// Which reported-end rule a wrong implementation applies.
enum EndRule {
    /// The end of the closure recorded first.
    FirstRecorded,
    /// The end of the closure recorded last (a later closure replaces).
    LastRecorded,
}

fn mutant_end(doc: &Doc, id: &str, r: Option<&str>, rule: &EndRule) -> Option<String> {
    let mut mine: Vec<&Closure> = doc
        .closures
        .iter()
        .filter(|c| {
            c.version == id && c.closes_valid && r.is_none_or(|r| c.recorded_at.as_str() <= r)
        })
        .collect();
    mine.sort_by(|a, b| a.recorded_at.cmp(&b.recorded_at));
    match rule {
        EndRule::FirstRecorded => mine.first(),
        EndRule::LastRecorded => mine.last(),
    }
    .and_then(|c| c.valid_until.clone())
}

fn failing_ends(doc: &Doc, end: impl Fn(&str, Option<&str>) -> Option<String>) -> Vec<String> {
    doc.cases
        .iter()
        .filter(|c| {
            c.expect_valid_to
                .iter()
                .any(|(id, want)| end(id, c.recorded_as_of.as_deref()) != *want)
        })
        .map(|c| c.name.clone())
        .collect()
}

/// Cases added after the first review, each aimed at one wrong reading. A
/// wrong rule must fail its target case; the contract passes all of them.
const OVERLAP: &str = "two versions with the same subject and predicate are both current when their valid times overlap";
const REASSERT: &str = "a re-assertion of the same subject, predicate and object is a second version, and both are current";
const END_FIRST: &str =
    "reported end under full knowledge is the earliest validUntil, not the first-recorded closure";
const END_LAST: &str = "reported end is the earliest validUntil, not the last-recorded closure";

#[test]
fn the_added_vectors_kill_the_wrong_readings() {
    let doc = load("vectors-v2.json");
    assert!(failing(&doc, |t, r| state(&doc, t, r)).is_empty());
    for (name, _) in [
        (OVERLAP, ()),
        (REASSERT, ()),
        (END_FIRST, ()),
        (END_LAST, ()),
    ] {
        assert!(doc.cases.iter().any(|c| c.name == name), "case {name}");
    }
    // 1. "latest version per (subject, predicate) wins" drops one of two
    //    concurrently valid versions.
    let bad = failing(&doc, |t, r| {
        mutant_state(&doc, t, r, &Mutant::LatestPerSubjectPredicate)
    });
    assert!(bad.iter().any(|n| n == OVERLAP), "{bad:#?}");
    // 2. "dedupe per (s, p, o), keep the latest generatedAt" drops the first
    //    of two assertions of the same fact.
    let bad = failing(&doc, |t, r| {
        mutant_state(&doc, t, r, &Mutant::LatestPerTriple)
    });
    assert!(bad.iter().any(|n| n == REASSERT), "{bad:#?}");
    // The reported end is the earliest validUntil that applies: a first-closure
    // or last-closure rule each fails one case.
    let first = failing_ends(&doc, |id, r| {
        mutant_end(&doc, id, r, &EndRule::FirstRecorded)
    });
    assert!(first.iter().any(|n| n == END_FIRST), "{first:#?}");
    let last = failing_ends(&doc, |id, r| {
        mutant_end(&doc, id, r, &EndRule::LastRecorded)
    });
    assert!(last.iter().any(|n| n == END_LAST), "{last:#?}");
    // The contract itself reports every pinned end.
    assert!(failing_ends(&doc, |id, r| reported_end(&doc, id, r)).is_empty());
}

/// "A closure on both clocks is a recorded-only closure" cannot be told apart
/// by any state vector, and no vector is added for it: a closure that closes
/// recorded time removes the version at every cut that sees the closure
/// (`recordedAt <= r`), which is exactly when its valid-time part would apply,
/// and at every other cut neither part applies. So the version is out of
/// `state(t, r)` either way and the valid part's `validUntil` is never
/// reported. This test proves it over every instant the vectors mention.
#[test]
fn a_both_clocks_closure_is_observably_a_recorded_close() {
    let doc = load("vectors-v2.json");
    let mut instants: BTreeSet<String> = BTreeSet::new();
    for v in &doc.versions {
        instants.insert(v.valid_from.clone());
        instants.insert(v.generated_at.clone());
    }
    for c in &doc.closures {
        instants.insert(c.recorded_at.clone());
        instants.extend(c.valid_until.clone());
    }
    let shifted: Vec<String> = instants
        .iter()
        .flat_map(|i| [shift(i, -1), i.clone(), shift(i, 1)])
        .collect();
    let recorded_only = Doc {
        versions: doc
            .versions
            .iter()
            .map(|v| Version {
                id: v.id.clone(),
                subject: v.subject.clone(),
                predicate: v.predicate.clone(),
                object: v.object.clone(),
                valid_from: v.valid_from.clone(),
                generated_at: v.generated_at.clone(),
            })
            .collect(),
        closures: doc
            .closures
            .iter()
            .map(|c| Closure {
                version: c.version.clone(),
                closes_valid: c.closes_valid && !c.closes_recorded,
                closes_recorded: c.closes_recorded,
                valid_until: c.valid_until.clone(),
                recorded_at: c.recorded_at.clone(),
            })
            .collect(),
        cases: Vec::new(),
    };
    let cuts: Vec<Option<&str>> = std::iter::once(None)
        .chain(shifted.iter().map(|s| Some(s.as_str())))
        .collect();
    for t in &shifted {
        for r in &cuts {
            assert_eq!(
                state(&doc, t, *r),
                state(&recorded_only, t, *r),
                "{t} {r:?}"
            );
        }
    }
}

/// `instant` moved by `secs` seconds (canonical UTC, second precision).
fn shift(instant: &str, secs: i64) -> String {
    let (date, time) = instant.trim_end_matches('Z').split_once('T').unwrap();
    let d: Vec<i64> = date.split('-').map(|x| x.parse().unwrap()).collect();
    let tm: Vec<i64> = time.split(':').map(|x| x.parse().unwrap()).collect();
    let days = days_from_civil(d[0], d[1], d[2]);
    let total = days * 86_400 + tm[0] * 3600 + tm[1] * 60 + tm[2] + secs;
    let (days, rem) = (total.div_euclid(86_400), total.rem_euclid(86_400));
    let (y, m, dd) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{dd:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}
