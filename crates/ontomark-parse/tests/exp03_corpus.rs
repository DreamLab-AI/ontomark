//! EXP-03 over a real vault, read-only: every `public: true` page
//! round-trips byte-for-byte, matches an independent YAML load, and survives
//! representative guarded patches with every unnamed byte unchanged.
//!
//! Set `ONTOMARK_CORPUS` to a `pages/` directory to run it (`just exp03`).
//! Without it the test reports that it was skipped. Output is counts only:
//! no titles, paths or page text.

mod common;

use std::path::{Path, PathBuf};

use common::{check_lossless, check_patch};
use ontomark_parse::{Document, Patch, Value};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
}

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    public: usize,
    lossless_fail: usize,
    patch_quality_fail: usize,
    patch_aliases_fail: usize,
    patch_remove_fail: usize,
    patch_insert_fail: usize,
    body_patch_fail: usize,
    body_links: usize,
    footnote_refs: usize,
    footnote_defs: usize,
    recovered_links: usize,
    entries: usize,
    emit_fail: usize,
    emit_byte_identical: usize,
}

#[test]
fn exp03_every_public_page() {
    let Ok(root) = std::env::var("ONTOMARK_CORPUS") else {
        eprintln!("EXP-03 corpus: SKIPPED (ONTOMARK_CORPUS unset)");
        return;
    };
    let mut files = Vec::new();
    walk(Path::new(&root), &mut files);
    files.sort();
    let mut t = Tally::default();
    let mut first_failures: Vec<String> = Vec::new();
    let note = |kind: &str, i: usize, e: &str, failures: &mut Vec<String>| {
        if failures.len() < 10 {
            // Index and failure class only: never the page's path or text.
            failures.push(format!(
                "{kind} #{i}: {}",
                e.chars().take(120).collect::<String>()
            ));
        }
    };
    for (i, path) in files.iter().enumerate() {
        t.files += 1;
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let doc = Document::parse(src.as_str());
        if doc.value("public") != Some(&Value::Bool(true)) {
            continue;
        }
        t.public += 1;
        let fails = check_lossless(&src);
        if !fails.is_empty() {
            t.lossless_fail += 1;
            note("lossless", i, &fails.join("; "), &mut first_failures);
            continue;
        }
        let fm = doc.frontmatter().expect("public implies frontmatter");
        t.recovered_links += fm
            .entries
            .iter()
            .map(|e| e.wikilinks(&src).iter().filter(|l| l.recovered).count())
            .sum::<usize>();
        // Emit every real entry (maps, sequences of maps, flat lists) in the
        // block's style and read it back.
        for e in &fm.entries {
            t.entries += 1;
            let text = ontomark_parse::emit_entry(&e.key, &e.value, &fm.style)
                .expect("corpus values are representable");
            if text == e.raw(&src) {
                t.emit_byte_identical += 1;
            }
            let back = Document::parse(format!("---\n{text}---\n"));
            if back.value(&e.key) != Some(&e.value) {
                t.emit_fail += 1;
                note("emit", i, &e.key, &mut first_failures);
            }
        }
        let idx = doc.body_index();
        t.body_links += idx.links.len();
        t.footnote_refs += idx.footnote_refs.len();
        t.footnote_defs += idx.footnote_defs.len();

        // (1) rewrite a scalar in place
        let q = match doc.value("quality") {
            Some(Value::Float(f)) if f == "0.123" => Value::Float("0.124".into()),
            _ => Value::Float("0.123".into()),
        };
        if let Err(e) = check_patch(
            &doc,
            &Patch::new(doc.digest()).set("quality", q),
            &["quality"],
            false,
        ) {
            t.patch_quality_fail += 1;
            note("quality", i, &e, &mut first_failures);
        }
        // (2) rewrite a list, keeping its items and appending one
        let mut aliases = doc.value("aliases").map(Value::texts).unwrap_or_default();
        aliases.push("Synthetic alias ✓".into());
        if let Err(e) = check_patch(
            &doc,
            &Patch::new(doc.digest()).set("aliases", Value::strings(aliases)),
            &["aliases"],
            false,
        ) {
            t.patch_aliases_fail += 1;
            note("aliases", i, &e, &mut first_failures);
        }
        // (3) remove a key
        if let Err(e) = check_patch(
            &doc,
            &Patch::new(doc.digest()).remove("status"),
            &["status"],
            false,
        ) {
            t.patch_remove_fail += 1;
            note("remove", i, &e, &mut first_failures);
        }
        // (4) insert a new key after the identity key
        let p = Patch::new(doc.digest()).set_at(
            "ontomark-probe",
            Value::Bool(true),
            ontomark_parse::Placement::After("title".into()),
        );
        if let Err(e) = check_patch(&doc, &p, &[], false) {
            t.patch_insert_fail += 1;
            note("insert", i, &e, &mut first_failures);
        }
        // (5) retarget the first body link
        if let Some(l) = idx.links.first() {
            let old = l.span.slice(&src);
            let new = format!("[[ontomark probe|{}]]", l.link.label());
            let p = Patch::new(doc.digest()).replace_body(l.span, old, new);
            if let Err(e) = check_patch(&doc, &p, &[], true) {
                t.body_patch_fail += 1;
                note("body", i, &e, &mut first_failures);
            }
        }
    }
    eprintln!("EXP-03 corpus: {t:?}");
    for f in &first_failures {
        eprintln!("  {f}");
    }
    assert!(t.public > 0, "no public pages found under ONTOMARK_CORPUS");
    let total_fail = t.lossless_fail
        + t.patch_quality_fail
        + t.patch_aliases_fail
        + t.patch_remove_fail
        + t.patch_insert_fail
        + t.body_patch_fail;
    assert_eq!(total_fail, 0, "EXP-03 failures: {t:?}");
}
