//! The full VisionGraph corpus tier, when it is present on this machine.

use ontomark_ngg1::Ngg1;
use std::path::Path;

const DIR: &str = "/home/devuser/workspace/.tmp/ontomark-golden/corpus-visiongraph/out/data/graph";

#[test]
fn the_corpus_tiers_decode_and_re_encode_byte_identically() {
    let dir = Path::new(DIR);
    let full = dir.join("full.bin");
    if !full.exists() {
        eprintln!(
            "note: {} absent; the corpus is not in the repository, skipping",
            full.display()
        );
        return;
    }
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("bin") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let graph = Ngg1::try_parse(&bytes)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .to_graph()
            .unwrap();
        assert_eq!(graph.encode().unwrap(), bytes, "{}", path.display());
        checked += 1;
    }
    eprintln!("corpus: {checked} tier(s) decoded and re-encoded byte-identically");
    assert!(checked >= 1);
}
