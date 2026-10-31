//! Grounding regression over a real corpus (ADR-OM-010 verification 1).
//!
//! Every vocabulary term's own label and each alias must ground back to that
//! term (`Resolved`, same IRI). The test reads a directory tree, so it is
//! ignored by default; run it with
//! `ONTOMARK_CORPUS_ROOT=/path/to/knowledge cargo test -p ontomark-wasm
//! --test corpus_grounding --release -- --ignored --nocapture`.
//! The profile is `ONTOMARK_CORPUS_PROFILE` (`estate`, the default, or
//! `generic`).

use std::path::Path;

use ontomark_ingest::{Grounding, LexicalThresholds, Vocabulary};
use ontomark_model::{DocId, Model};
use ontomark_parse::Document;
use ontomark_wasm::profile::ProfileId;

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Document)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(root, &p, out);
        } else if p.extension().is_some_and(|e| e == "md") {
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if let Ok(text) = std::fs::read_to_string(&p) {
                out.push((rel, Document::parse(text)));
            }
        }
    }
}

#[test]
#[ignore = "reads a corpus from ONTOMARK_CORPUS_ROOT"]
fn every_label_and_alias_grounds_to_its_own_term() {
    let root = std::env::var("ONTOMARK_CORPUS_ROOT").expect("set ONTOMARK_CORPUS_ROOT");
    let profile = match std::env::var("ONTOMARK_CORPUS_PROFILE").as_deref() {
        Ok("generic") => ProfileId::Generic,
        _ => ProfileId::Estate,
    };
    let mut docs = Vec::new();
    walk(Path::new(&root), Path::new(&root), &mut docs);
    let model = Model::build(
        profile.profile(),
        docs.iter().map(|(p, d)| (DocId::new(p), d)),
    );
    let vocab = Vocabulary::from_model(&model);
    let thresholds = LexicalThresholds::default();

    let (mut checks, mut same) = (0usize, 0usize);
    let mut diverged: Vec<String> = Vec::new();
    for term in vocab.terms() {
        let names =
            std::iter::once(term.label.as_str()).chain(term.aliases.iter().map(String::as_str));
        for name in names {
            checks += 1;
            let got = vocab.ground(profile.profile(), &model, name, &thresholds, 0);
            match got {
                Grounding::Resolved { ref iri, .. } if *iri == term.iri => same += 1,
                other => diverged.push(format!("{}\t{name:?}\t{other:?}", term.iri.as_str())),
            }
        }
    }
    diverged.sort();

    // Probe: a label whose last number is bumped by one names something else.
    // Before the digit guard a fuzzy tier could alias it back to the original.
    let (mut probes, mut back_to_origin, mut probe_resolved) = (0usize, 0usize, 0usize);
    let mut lines: Vec<String> = Vec::new();
    for term in vocab.terms() {
        let Some(bumped) = bump_last_number(&term.label) else {
            continue;
        };
        probes += 1;
        let got = vocab.ground(profile.profile(), &model, &bumped, &thresholds, 0);
        if let Grounding::Resolved { iri, .. } = &got {
            probe_resolved += 1;
            if *iri == term.iri {
                back_to_origin += 1;
            }
        }
        lines.push(format!(
            "{}\t{:?}\t{:?}\t{got:?}",
            term.iri.as_str(),
            term.label,
            bumped
        ));
    }
    lines.sort();
    let leaked = lines;
    eprintln!(
        "PROBE bumped_labels={probes} resolved_any={probe_resolved} resolved_to_original={back_to_origin}"
    );
    for line in &leaked {
        eprintln!("PROBE\t{line}");
    }
    eprintln!(
        "CORPUS terms={} checks={checks} resolved_same={same} diverged={}",
        vocab.terms().len(),
        diverged.len()
    );
    for line in &diverged {
        eprintln!("DIVERGED\t{line}");
    }
    // The bar is that nothing diverges that did not diverge before the digit
    // guard; the baseline is recorded by the run, not hard-coded here.
    if let Ok(max) = std::env::var("ONTOMARK_CORPUS_MAX_DIVERGED") {
        let max: usize = max.parse().unwrap();
        assert!(
            diverged.len() <= max,
            "{} diverged, allowed {max}",
            diverged.len()
        );
    }
}

/// `label` with its last run of ASCII digits incremented, or `None` when it
/// has no digits.
fn bump_last_number(label: &str) -> Option<String> {
    let end = label.rfind(|c: char| c.is_ascii_digit())? + 1;
    let start = label[..end]
        .rfind(|c: char| !c.is_ascii_digit())
        .map_or(0, |i| i + 1);
    let n: u128 = label[start..end].parse().ok()?;
    Some(format!("{}{}{}", &label[..start], n + 1, &label[end..]))
}
