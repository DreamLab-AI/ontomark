//! EXP-10 mapping check: the engine's own projection and the typed model read
//! through the engine profile's `ontomark_model::Profile` bridge must agree
//! on which pages are entities, under which IRIs, with which labels. Assisted
//! ingest grounds against the model, so a disagreement here would offer a
//! model names the engine does not index, or hide ones it does.

use std::collections::BTreeMap;
use std::path::Path;

use ontomark_model::{DocId, Model};
use ontomark_parse::Document;
use ontomark_wasm::profile::ProfileId;
use ontomark_wasm::projection;

fn vault() -> BTreeMap<String, Document> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault");
    let mut out = BTreeMap::new();
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Document>) {
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
                out.insert(rel, Document::parse(std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    walk(&root, &root, &mut out);
    out
}

/// `(IRI, label)` per entity, sorted by IRI.
type Labelled = Vec<(String, String)>;

/// The engine's entities and the typed model's, over the fixture vault.
fn both(profile: ProfileId) -> (Labelled, Labelled) {
    let docs = vault();
    let compiled = projection::compile(&docs, profile.profile());
    let mut engine: Labelled = compiled
        .entities
        .iter()
        .map(|e| (e.iri.clone(), e.label.clone()))
        .collect();
    engine.sort();

    let model = Model::build(
        profile.profile(),
        docs.iter().map(|(p, d)| (DocId::new(p), d)),
    );
    let mut typed: Labelled = model
        .entities
        .iter()
        .map(|(iri, e)| (iri.as_str().to_owned(), e.label.clone()))
        .collect();
    typed.sort();
    (engine, typed)
}

#[test]
fn generic_profile_indexes_the_same_entities_on_both_sides() {
    let (engine, typed) = both(ProfileId::Generic);
    assert!(!engine.is_empty(), "the fixture vault has typed pages");
    assert_eq!(engine, typed);
}

#[test]
fn estate_profile_requires_authored_iris_on_both_sides() {
    // No fixture page carries `resource:`, so neither side indexes anything.
    let (engine, typed) = both(ProfileId::Estate);
    assert_eq!(engine, typed);
    assert!(engine.is_empty());
}
