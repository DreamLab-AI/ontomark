//! Losslessness: a patch touching key K leaves every other byte alone.
//!
//! Three layers: a deterministic property test over generated pages (comments,
//! blank lines, flow and block styles, unknown keys, CRLF, BOM); the same over
//! real pages copied at test time from a vault named by `ONTOMARK_REAL_VAULT`
//! (skipped without it; the corpus is not committed here); and an ignored
//! test that walks a whole real vault, read-only, when `ONTOMARK_REAL_VAULT`
//! names a repository root.

mod common;

use std::collections::BTreeSet;

use common::*;
use ontomark_compile::input::page::VaultKind;
use ontomark_parse::{Document, Value};
use ontomark_write::{Change, PatchRequest, Role};

/// A tiny deterministic generator, so a failure names its seed.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n as u64)).unwrap()
    }
    fn chance(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }
}

fn gen_entry(r: &mut Rng, key: &str) -> String {
    match r.below(9) {
        0 => format!("{key}: plain value {}\n", r.below(1000)),
        1 => format!("{key}: \"double: quoted # {}\"\n", r.below(1000)),
        2 => format!("{key}: 'single quoted {}'\n", r.below(1000)),
        3 => format!("{key}: [a, b,   c{}]\n", r.below(9)),
        4 => format!("{key}:\n  - one\n  - two {}\n  - 'three'\n", r.below(9)),
        5 => format!("{key}: {{x: 1,   y: [2, 3]}}\n"),
        6 => format!("{key}:\n  nested: yes\n  deeper:\n    - a\n    - b\n"),
        7 => format!(
            "{key}: |\n  block line one\n  block line two {}\n",
            r.below(9)
        ),
        _ => format!("{key}: {}.{}0\n", r.below(9), r.below(9)),
    }
}

fn gen_page(seed: u64, tail: &str) -> (String, Vec<String>) {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut fm = String::new();
    let mut keys = Vec::new();
    for i in 0..(2 + r.below(8)) {
        if r.chance(4) {
            fm.push_str(&format!("# a comment {i}\n"));
        }
        if r.chance(6) {
            fm.push('\n');
        }
        let key = format!("k-{i}");
        fm.push_str(&gen_entry(&mut r, &key));
        keys.push(key);
    }
    fm.push_str(tail);
    let body = match r.below(3) {
        0 => "Body with [[a link]] and text.\n".to_owned(),
        1 => "\n\n  indented\n\ttabbed\n".to_owned(),
        _ => String::new(),
    };
    let mut text = format!("---\n{fm}---\n{body}");
    if r.chance(5) {
        text = text.replace('\n', "\r\n");
    }
    if r.chance(7) {
        text.insert(0, '\u{feff}');
    }
    (text, keys)
}

fn raw_of(doc: &Document, key: &str) -> Option<String> {
    doc.frontmatter()
        .and_then(|f| f.get(key))
        .map(|e| e.raw(doc.source()).to_owned())
}

fn key_order(doc: &Document) -> Vec<String> {
    doc.frontmatter()
        .map(|f| f.keys().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Assert that only `changed` differ between `before` and `after`.
fn assert_only(before: &str, after: &str, changed: &BTreeSet<&str>, context: &str) {
    let (b, a) = (Document::parse(before), Document::parse(after));
    assert_eq!(a.body(), b.body(), "{context}: body changed");
    let mut all: BTreeSet<String> = key_order(&b).into_iter().collect();
    all.extend(key_order(&a));
    for key in &all {
        if changed.contains(key.as_str()) {
            continue;
        }
        assert_eq!(
            raw_of(&a, key),
            raw_of(&b, key),
            "{context}: `{key}` bytes changed"
        );
    }
    // Surviving keys keep their order.
    let order_b: Vec<String> = key_order(&b)
        .into_iter()
        .filter(|k| !changed.contains(k.as_str()))
        .collect();
    let order_a: Vec<String> = key_order(&a)
        .into_iter()
        .filter(|k| !changed.contains(k.as_str()))
        .collect();
    assert_eq!(order_a, order_b, "{context}: key order changed");
}

fn note_tail(v3: bool) -> &'static str {
    if v3 {
        "type: Note\nstatus: draft\npage_resource: urn:visionflow:page:p1\nvisibility: shared\ngenerated: { by: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a, at: '2026-11-01T00:00:00Z' }\n"
    } else {
        "type: Note\nstatus: draft\n"
    }
}

fn patch_of(role: Role, id: &str, key: &str, value: Value, stamped: bool) -> PatchRequest {
    PatchRequest {
        role,
        id: id.into(),
        changes: vec![Change::Set {
            key: key.into(),
            value,
        }],
        body: vec![],
        expect: None,
        writer: stamped.then(human),
        now: stamped.then(|| T2.to_owned()),
    }
}

fn random_value(r: &mut Rng) -> Value {
    match r.below(4) {
        0 => Value::string(format!("v{}", r.below(99))),
        1 => Value::Int(i64::try_from(r.below(500)).unwrap()),
        2 => Value::strings(["p", "q"]),
        _ => Value::Map([("m".to_owned(), Value::string("n"))].into_iter().collect()),
    }
}

#[test]
fn a_patch_touching_k_changes_exactly_k_under_v2() {
    let mut touched = 0;
    for seed in 1..=300u64 {
        let (text, keys) = gen_page(seed, note_tail(false));
        let id = "n/N";
        let e = engine_with(VOCAB_V2, false, &[(id, &text)], &[]);
        let mut r = Rng(seed * 31 + 7);
        let key = if r.chance(3) {
            "k-new".to_owned()
        } else {
            keys[r.below(keys.len())].clone()
        };
        let value = random_value(&mut r);
        let plan = e
            .patch(&patch_of(Role::Working, id, &key, value, false))
            .unwrap();
        if plan.writes.is_empty() {
            continue;
        }
        touched += 1;
        let after = &plan.writes[0].text;
        assert_only(
            &text,
            after,
            &BTreeSet::from([key.as_str()]),
            &format!("seed {seed}, key {key}"),
        );
        // The proof the parse crate returns agrees: the receipt names edits.
        assert!(!plan.writes[0].receipt.as_ref().unwrap().edits.is_empty());
    }
    assert!(
        touched > 200,
        "the generator must exercise real edits, got {touched}"
    );
}

#[test]
fn a_patch_touching_k_changes_k_and_the_stamped_set_under_v3() {
    let mut touched = 0;
    for seed in 1..=300u64 {
        let (text, keys) = gen_page(seed, note_tail(true));
        let id = "n/N";
        let e = engine_with(VOCAB_V3, true, &[(id, &text)], &[]);
        let mut r = Rng(seed * 17 + 3);
        let key = if r.chance(3) {
            "k-new".to_owned()
        } else {
            keys[r.below(keys.len())].clone()
        };
        let value = random_value(&mut r);
        let plan = e
            .patch(&patch_of(Role::Working, id, &key, value, true))
            .unwrap();
        if plan.writes.is_empty() {
            continue;
        }
        touched += 1;
        // A Note is not a memory kind: the only stamp is `contributors`.
        let allowed = BTreeSet::from([key.as_str(), "contributors"]);
        assert_only(
            &text,
            &plan.writes[0].text,
            &allowed,
            &format!("seed {seed}, key {key}"),
        );
        // The first stamp and identity are untouched, byte for byte.
        let (b, a) = (
            Document::parse(&text),
            Document::parse(&plan.writes[0].text),
        );
        for fixed in ["generated", "page_resource"] {
            assert_eq!(raw_of(&a, fixed), raw_of(&b, fixed), "seed {seed}: {fixed}");
        }
    }
    assert!(touched > 200, "got {touched}");
}

#[test]
fn removing_a_key_changes_only_that_key() {
    for seed in 1..=100u64 {
        let (text, keys) = gen_page(seed, note_tail(false));
        let e = engine_with(VOCAB_V2, false, &[("n/N", &text)], &[]);
        let key = &keys[seed as usize % keys.len()];
        let mut req = patch_of(Role::Working, "n/N", key, Value::Null, false);
        req.changes = vec![Change::Unset { key: key.clone() }];
        let plan = e.patch(&req).unwrap();
        assert_only(
            &text,
            &plan.writes[0].text,
            &BTreeSet::from([key.as_str()]),
            &format!("seed {seed}"),
        );
        assert!(raw_of(&Document::parse(&plan.writes[0].text), key).is_none());
    }
}

// ------------------------------------------------------------ real pages

/// Copy a deterministic sample of public knowledge pages from a real vault
/// into `dir` (the source is only read) and return `(id, text)`. `None` when
/// `ONTOMARK_REAL_VAULT` is not set: the corpus is not redistributed with
/// this repository (its licensed sources carry notices of their own), so the
/// real-page tests run where the vault is, not in CI.
fn samples(dir: &std::path::Path) -> Option<Vec<(String, String)>> {
    let root = std::env::var("ONTOMARK_REAL_VAULT").ok()?;
    let base = std::path::Path::new(&root).join("knowledge");
    let mut found: Vec<std::path::PathBuf> = std::fs::read_dir(&base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    found.sort();
    let mut out = Vec::new();
    for (i, path) in found.iter().enumerate() {
        if i % 97 != 0 || out.len() >= 60 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if !text.contains("\npublic: true") {
            continue;
        }
        let id = format!("s{i:05}");
        std::fs::write(dir.join(format!("{id}.md")), &text).unwrap();
        out.push((id, text));
    }
    Some(out)
}

#[test]
fn real_pages_round_trip_byte_for_byte_and_patch_losslessly() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(pages) = samples(tmp.path()) else {
        eprintln!(
            "skipped: set ONTOMARK_REAL_VAULT to a visionGraph checkout (read-only) to run this"
        );
        return;
    };
    assert!(
        pages.len() >= 20,
        "expected a sample of real pages, got {}",
        pages.len()
    );
    for vocab in [VOCAB_V2, VOCAB_V3] {
        let refs_: Vec<(&str, &str)> = pages
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let e = engine_with(vocab, true, &[], &refs_);
        for (id, text) in &pages {
            assert_eq!(
                Document::parse(text).source(),
                text,
                "{id}: parse is lossless"
            );
            let req = patch_of(
                Role::Knowledge,
                id,
                "tags",
                Value::strings(["zz-sample"]),
                false,
            );
            let plan = e.patch(&req).unwrap_or_else(|r| panic!("{id}: {r}"));
            assert_only(text, &plan.writes[0].text, &BTreeSet::from(["tags"]), id);
            // A patch that sets an existing key to its own value plans nothing.
            let doc = Document::parse(text);
            let first = doc.frontmatter().unwrap().keys().next().unwrap().to_owned();
            let same = doc.value(&first).unwrap().clone();
            let noop = e
                .patch(&patch_of(Role::Knowledge, id, &first, same, false))
                .unwrap();
            assert!(
                noop.writes.is_empty(),
                "{id}: setting `{first}` to itself must not write"
            );
        }
    }
}

/// Walk a whole real repository, read-only, and report. Run with
/// `ONTOMARK_REAL_VAULT=/path/to/repo cargo test -p ontomark-write --test lossless -- --ignored --nocapture`.
#[test]
#[ignore = "reads a real vault; set ONTOMARK_REAL_VAULT"]
fn every_page_of_a_real_vault_round_trips() {
    let Ok(root) = std::env::var("ONTOMARK_REAL_VAULT") else {
        eprintln!("ONTOMARK_REAL_VAULT not set");
        return;
    };
    let (mut pages, mut unparsed, mut unpatchable, mut lossy) = (0, 0, 0, 0);
    for vault in ["knowledge", "working"] {
        let base = std::path::Path::new(&root).join(vault);
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in rd.flatten() {
                let p = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || name == "assets" {
                    continue;
                }
                let Ok(meta) = std::fs::symlink_metadata(&p) else {
                    continue;
                };
                if meta.is_dir() {
                    stack.push(p);
                } else if name.ends_with(".md") {
                    let Ok(text) = std::fs::read_to_string(&p) else {
                        continue;
                    };
                    pages += 1;
                    let doc = Document::parse(&text);
                    if doc.source() != text {
                        lossy += 1;
                    }
                    if !doc
                        .frontmatter()
                        .is_some_and(ontomark_parse::Frontmatter::is_parsed)
                    {
                        if text.starts_with("---") {
                            unparsed += 1;
                        }
                        continue;
                    }
                    let patch = ontomark_parse::Patch::new(doc.digest())
                        .set("zz-note", Value::string("added"));
                    match doc.apply(&patch) {
                        Ok(applied) => {
                            let after = applied.document;
                            let ok = doc
                                .frontmatter()
                                .unwrap()
                                .keys()
                                .all(|k| raw_of(&after, k) == raw_of(&doc, k))
                                && after.body() == doc.body();
                            if !ok {
                                lossy += 1;
                            }
                        }
                        Err(_) => unpatchable += 1,
                    }
                }
            }
        }
    }
    eprintln!(
        "pages={pages} frontmatter-not-parsed={unparsed} unpatchable={unpatchable} lossy={lossy}"
    );
    assert_eq!(lossy, 0);
    let _ = VaultKind::Working;
}
