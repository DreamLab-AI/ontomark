//! Shared helpers for the integration tests.
#![allow(dead_code)]

use ontomark_ngg1::{build_csr, Csr, GraphNode, CATEGORY_NONE, DOMAIN_NONE};
use std::path::PathBuf;

/// The in-repo golden graph directory.
pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/estate-small.golden/expected/out/data/graph")
}

/// A node with the given identity and defaults elsewhere.
pub fn node(id: u32, label: &str, iri: &str) -> GraphNode {
    GraphNode {
        id,
        x: 0.0,
        y: 0.0,
        domain: DOMAIN_NONE,
        category: CATEGORY_NONE,
        flags: 0,
        degree: 0,
        label: label.to_owned(),
        iri: iri.to_owned(),
    }
}

/// A deterministic xorshift generator, so fuzz loops reproduce.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// A pseudo-random graph with `n` nodes and `e` edges.
pub fn random_graph(seed: u64, n: usize, e: usize) -> (Vec<GraphNode>, Csr) {
    let mut rng = Rng(seed | 1);
    let nodes = (0..n)
        .map(|i| {
            let mut g = node(
                u32::try_from(i * 3 + 1).unwrap(),
                &format!("label {i}"),
                &format!("https://example.org/n/{i}"),
            );
            g.x = (rng.below(2000) as f32 - 1000.0) / 7.0;
            g.y = (rng.below(2000) as f32 - 1000.0) / 3.0;
            g.domain = rng.below(3) as u16;
            g.category = if i % 4 == 0 {
                CATEGORY_NONE
            } else {
                rng.below(9) as u16
            };
            g.flags = rng.below(256) as u8;
            g.degree = rng.below(500) as u32;
            g
        })
        .collect();
    let edges: Vec<(u32, u32, u8)> = if n == 0 {
        Vec::new()
    } else {
        (0..e)
            .map(|_| (rng.below(n) as u32, rng.below(n) as u32, rng.below(2) as u8))
            .collect()
    };
    (nodes, build_csr(n, &edges))
}
