//! The upstream whelk-rs inference test, through the `horned-owl` adapter.
//!
//! `tests/data/skeletons` is copied from whelk-rs `src/data/inference-tests`
//! (MIT, see NOTICE). Every subsumption in `*-entailed.owx` must be entailed and
//! no named subsumption in `*-invalid.owx` may be. The 1.5 MB `go-extract` case
//! is not vendored; set `WHELK_RS_INFERENCE_TESTS` to whelk-rs's
//! `src/data/inference-tests` directory to run it too.
//!
//! Two allowances, both about the fixtures rather than the reasoner:
//!
//! * `horned-owl` resolves an abbreviated `#local` IRI against the empty
//!   `Prefix` in one skeletons file and against `xml:base` in another, so pairs
//!   are compared by IRI fragment;
//! * `go-extract` carries non-EL axioms (ranges, inverses, universals,
//!   cardinalities, complements) that upstream dropped silently; here they
//!   are refused by the adapter, counted and printed, and the rest reasoned.
#![cfg(feature = "horned-owl")]

use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use horned_owl::io::ParserConfiguration;
use horned_owl::model::RcStr;
use horned_owl::ontology::set::SetOntology;
use whelk_el::{classify, ClassExpr, ElAxiom, TOP};

fn read(path: &Path) -> SetOntology<RcStr> {
    let mut reader = BufReader::new(File::open(path).expect("fixture file"));
    horned_owl::io::owx::reader::read(&mut reader, ParserConfiguration::default())
        .expect("valid OWL/XML")
        .0
}

fn axioms(path: &Path) -> Vec<ElAxiom> {
    let ontology = read(path);
    let (axioms, refused) = whelk_el::owl::components(ontology.iter().map(|a| &a.component));
    if !refused.is_empty() {
        let mut by_construct: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for r in &refused {
            *by_construct.entry(r.construct.as_str()).or_default() += 1;
        }
        eprintln!("{}: refused {by_construct:?}", path.display());
    }
    axioms
}

fn fragment(iri: &str) -> String {
    iri.rsplit('#').next().unwrap_or(iri).to_owned()
}

fn key((a, b): &(String, String)) -> (String, String) {
    (fragment(a), fragment(b))
}

fn named_pairs(axioms: &[ElAxiom]) -> Vec<(String, String)> {
    axioms
        .iter()
        .filter_map(|a| match a {
            ElAxiom::SubClassOf {
                sub: ClassExpr::Named(a),
                sup: ClassExpr::Named(b),
            } => Some((a.clone(), b.clone())),
            _ => None,
        })
        .collect()
}

/// Returns (entailed checked, invalid checked).
fn run_case(dir: &Path) -> (usize, usize) {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let entailed_path = dir.join(format!("{name}-entailed.owx"));
    let invalid_path = dir.join(format!("{name}-invalid.owx"));
    let pairs: BTreeSet<(String, String)> =
        classify(&axioms(&dir.join(format!("{name}-asserted.owx"))))
            .iter()
            .map(key)
            .collect();
    let mut checked = (0, 0);
    if entailed_path.exists() {
        for pair in named_pairs(&axioms(&entailed_path)) {
            assert!(
                pairs.contains(&key(&pair)),
                "{name}: {pair:?} must be entailed"
            );
            checked.0 += 1;
        }
    }
    if invalid_path.exists() {
        for pair in named_pairs(&axioms(&invalid_path)) {
            if pair.1 != TOP {
                assert!(
                    !pairs.contains(&key(&pair)),
                    "{name}: {pair:?} must not be entailed"
                );
                checked.1 += 1;
            }
        }
    }
    checked
}

#[test]
fn skeletons() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/skeletons");
    let (entailed, invalid) = run_case(&dir);
    eprintln!("skeletons: {entailed} entailed and {invalid} invalid subsumptions checked");
    assert!(entailed > 0 && invalid > 0);
}

#[test]
fn every_upstream_case_when_available() {
    let Some(root) = std::env::var_os("WHELK_RS_INFERENCE_TESTS") else {
        eprintln!("SKIP: WHELK_RS_INFERENCE_TESTS unset");
        return;
    };
    for entry in std::fs::read_dir(root).expect("inference-tests dir") {
        let dir = entry.expect("entry").path();
        if dir.is_dir() {
            let (entailed, invalid) = run_case(&dir);
            eprintln!(
                "{}: {entailed} entailed, {invalid} invalid checked",
                dir.display()
            );
        }
    }
}
