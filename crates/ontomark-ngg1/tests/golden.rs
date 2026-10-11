//! Decode the in-repo golden graph tiers and check them against `stats.json`.

mod common;

use ontomark_ngg1::{Ngg1, EDGE_RELATION, EDGE_SUBCLASS};

/// The integer after `"key":` inside the object of `scope` in `stats.json`'s `scopes`.
fn stat(stats: &str, scope: &str, key: &str) -> usize {
    let scopes = stats.find("\"scopes\"").expect("scopes");
    let at = scopes
        + stats[scopes..]
            .find(&format!("\"{scope}\""))
            .expect("scope");
    let rest = &stats[at..];
    let end = rest.find('}').expect("scope object end");
    let obj = &rest[..end];
    let k = obj.find(&format!("\"{key}\":")).expect("key");
    let digits: String = obj[k + key.len() + 3..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().expect("integer")
}

fn scopes() -> Vec<(String, std::path::PathBuf)> {
    let dir = common::golden_dir();
    let mut out = vec![("full".to_owned(), dir.join("full.bin"))];
    let mut domains: Vec<_> = std::fs::read_dir(&dir)
        .expect("golden dir")
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("domain-") && n.ends_with(".bin"))
        })
        .collect();
    domains.sort();
    for p in domains {
        let name = p.file_stem().unwrap().to_str().unwrap().to_owned();
        out.push((name, p));
    }
    out
}

#[test]
fn every_golden_tier_decodes_and_matches_stats() {
    let stats = std::fs::read_to_string(common::golden_dir().join("stats.json")).unwrap();
    let tiers = scopes();
    assert!(tiers.len() >= 2, "expected full.bin and domain tiers");
    for (scope, path) in tiers {
        let bytes = std::fs::read(&path).unwrap();
        let g = Ngg1::try_parse(&bytes).unwrap_or_else(|e| panic!("{scope}: {e}"));
        assert_eq!(
            g.node_count(),
            stat(&stats, &scope, "nodes"),
            "{scope} nodes"
        );
        assert_eq!(
            g.edge_count(),
            stat(&stats, &scope, "shipped"),
            "{scope} edges"
        );
        assert_eq!(bytes.len(), stat(&stats, &scope, "bytes"), "{scope} bytes");

        let (src, tgt, ty) = g.edges();
        assert_eq!(src.len(), g.edge_count());
        let backbone = ty.iter().filter(|&&t| t == EDGE_SUBCLASS).count();
        let relations = ty.iter().filter(|&&t| t == EDGE_RELATION).count();
        assert_eq!(
            backbone + relations,
            g.edge_count(),
            "{scope}: only known edge types"
        );
        assert!(tgt.iter().all(|&t| (t as usize) < g.node_count()));
        for i in 0..g.node_count() {
            assert!(!g.label(i).is_empty(), "{scope}: node {i} label");
            assert!(g.iri(i).starts_with("http"), "{scope}: node {i} iri");
        }
    }
}

#[test]
fn the_full_tier_splits_into_the_stated_backbone_and_relations() {
    let stats = std::fs::read_to_string(common::golden_dir().join("stats.json")).unwrap();
    let bytes = std::fs::read(common::golden_dir().join("full.bin")).unwrap();
    let g = Ngg1::parse(&bytes).unwrap();
    let (_, _, ty) = g.edges();
    assert_eq!(
        ty.iter().filter(|&&t| t == EDGE_SUBCLASS).count(),
        stat(&stats, "full", "backbone")
    );
    assert_eq!(
        ty.iter().filter(|&&t| t == EDGE_RELATION).count(),
        stat(&stats, "full", "relations")
    );
}

#[test]
fn every_golden_tier_re_encodes_byte_identically() {
    for (scope, path) in scopes() {
        let bytes = std::fs::read(&path).unwrap();
        let graph = Ngg1::parse(&bytes).unwrap().to_graph().unwrap();
        assert_eq!(graph.encode().unwrap(), bytes, "{scope}");
    }
}
