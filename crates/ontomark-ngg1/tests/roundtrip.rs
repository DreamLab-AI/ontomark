//! Encode then decode synthetic graphs, including non-estate shapes.

mod common;

use common::{node, random_graph};
use ontomark_ngg1::{
    build_csr, encode, Csr, Graph, Ngg1, CATEGORY_NONE, DOMAIN_NONE, EDGE_RELATION, EDGE_SUBCLASS,
    FLAG_BRIDGE, FLAG_HAS_PAGE, HEADER_SIZE, NODE_STRIDE,
};

fn round_trip(nodes: &[ontomark_ngg1::GraphNode], csr: &Csr) -> Vec<u8> {
    let bytes = encode(nodes, csr).expect("encode");
    let g = Ngg1::try_parse(&bytes).expect("parse");
    assert_eq!(g.node_count(), nodes.len());
    assert_eq!(g.edge_count(), csr.col_idx.len());
    let back = g.to_graph().expect("to_graph");
    assert_eq!(back.nodes, nodes);
    assert_eq!(back.csr, *csr);
    assert_eq!(
        back.encode().expect("re-encode"),
        bytes,
        "byte-identical re-encode"
    );
    bytes
}

#[test]
fn offsets_follow_the_documented_layout_for_every_padding_class() {
    for edges in 0..9usize {
        let n = 4usize;
        let list: Vec<(u32, u32, u8)> = (0..edges)
            .map(|k| ((k % n) as u32, ((k + 1) % n) as u32, (k % 2) as u8))
            .collect();
        let nodes: Vec<_> = (0..n).map(|i| node(i as u32, "ab", "u")).collect();
        let bytes = round_trip(&nodes, &build_csr(n, &list));
        let rd = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
        let pad = (4 - edges % 4) % 4;
        assert_eq!(rd(16), HEADER_SIZE);
        assert_eq!(rd(20), HEADER_SIZE + n * NODE_STRIDE);
        assert_eq!(rd(24), rd(20) + (n + 1) * 4 + edges * 4);
        assert_eq!(rd(28), rd(24) + edges + pad);
        assert!(
            bytes[rd(24) + edges..rd(28)].iter().all(|&b| b == 0),
            "zero pad"
        );
        let blob_len = rd(rd(28) + 4);
        assert_eq!(bytes.len(), rd(28) + 8 + n * 2 * 4 + blob_len);
    }
}

#[test]
fn three_custom_domains_without_categories() {
    let mut nodes = Vec::new();
    for i in 0..9u32 {
        let mut n = node(
            i + 100,
            &format!("Thing {i}"),
            &format!("https://x.example/t/{i}"),
        );
        n.domain = (i % 3) as u16;
        n.flags = FLAG_HAS_PAGE | if i == 0 { FLAG_BRIDGE } else { 0 };
        n.degree = i;
        n.x = i as f32 * 1.5;
        n.y = -(i as f32);
        nodes.push(n);
    }
    let edges = [
        (1, 0, EDGE_SUBCLASS),
        (2, 0, EDGE_SUBCLASS),
        (3, 4, EDGE_RELATION),
    ];
    let bytes = round_trip(&nodes, &build_csr(9, &edges));
    let g = Ngg1::parse(&bytes).unwrap();
    assert_eq!(g.node(4).domain, 1);
    assert_eq!(g.node(4).category, CATEGORY_NONE);
    assert_eq!(g.node(0).id, 100);
}

#[test]
fn no_domains_at_all() {
    let nodes: Vec<_> = (0..5)
        .map(|i| node(i, &format!("n{i}"), &format!("urn:{i}")))
        .collect();
    let bytes = round_trip(&nodes, &build_csr(5, &[(0, 1, 0), (1, 2, 0)]));
    let g = Ngg1::parse(&bytes).unwrap();
    assert!((0..5).all(|i| g.node(i).domain == DOMAIN_NONE));
}

#[test]
fn zero_edges_and_zero_nodes() {
    let nodes = vec![node(1, "Only", "urn:only")];
    let bytes = round_trip(&nodes, &build_csr(1, &[]));
    assert!(Ngg1::parse(&bytes).unwrap().edges().0.is_empty());

    let empty = round_trip(&[], &build_csr(0, &[]));
    assert_eq!(
        empty.len(),
        HEADER_SIZE + 4 + 8,
        "header, row_ptr[0], string header"
    );
    let g = Ngg1::parse(&empty).unwrap();
    assert_eq!((g.node_count(), g.edge_count()), (0, 0));
    assert!(g.seed_positions().is_empty());
}

#[test]
fn unicode_labels_survive() {
    let labels = [
        "Café résumé",
        "量子コンピュータ",
        "🚀 rocket 🛰️",
        "a\u{0301} (combining)",
        "",
        "tab\tand\nnewline",
        "Ελληνικά",
    ];
    let nodes: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| node(i as u32, l, &format!("https://例え.example/{l}")))
        .collect();
    let bytes = round_trip(&nodes, &build_csr(labels.len(), &[]));
    let g = Ngg1::parse(&bytes).unwrap();
    for (i, l) in labels.iter().enumerate() {
        assert_eq!(g.label(i), *l);
    }
}

#[test]
fn random_graphs_round_trip() {
    for (seed, n, e) in [
        (1u64, 1usize, 0usize),
        (2, 2, 1),
        (3, 17, 40),
        (4, 300, 1001),
        (5, 64, 3),
    ] {
        let (nodes, csr) = random_graph(seed, n, e);
        round_trip(&nodes, &csr);
    }
}

#[test]
fn worked_example_decodes_and_re_encodes() {
    let bytes = ontomark_ngg1::worked_example();
    let graph: Graph = Ngg1::parse(&bytes).unwrap().to_graph().unwrap();
    assert_eq!(graph.encode().unwrap(), bytes);
}

#[test]
fn the_encoder_rejects_inconsistent_input() {
    use ontomark_ngg1::EncodeError;
    let nodes = vec![node(0, "a", "b"), node(1, "c", "d")];
    let ok = build_csr(2, &[(0, 1, 0)]);
    assert!(encode(&nodes, &ok).is_ok());
    let mut c = ok.clone();
    c.row_ptr.pop();
    assert!(matches!(
        encode(&nodes, &c),
        Err(EncodeError::RowPtrLength { .. })
    ));
    let mut c = ok.clone();
    c.edge_type.push(0);
    assert!(matches!(
        encode(&nodes, &c),
        Err(EncodeError::EdgeTypeLength { .. })
    ));
    let mut c = ok.clone();
    c.row_ptr[2] = 5;
    assert!(matches!(
        encode(&nodes, &c),
        Err(EncodeError::RowPtr { .. })
    ));
    let mut c = ok.clone();
    c.col_idx[0] = 2;
    assert!(matches!(
        encode(&nodes, &c),
        Err(EncodeError::EdgeTarget { edge: 0, target: 2 })
    ));
    assert!(!EncodeError::TooLarge.to_string().is_empty());
}

#[test]
fn the_vowl_wasm_surface_is_unchanged() {
    // Names and signatures of vowl-wasm 0.1.2's `ngg1` module.
    use ontomark_ngg1::{
        Ngg1Error, NodeRecord, CATEGORY_NONE, EDGE_RELATION, EDGE_SUBCLASS, FLAG_CATEGORY_ROOT,
        FLAG_DOMAIN_ROOT, NGG1_HEADER_SIZE, NGG1_MAGIC, NGG1_NODE_STRIDE, NGG1_VERSION,
    };
    let bytes = ontomark_ngg1::worked_example();
    let g: Result<Ngg1<'_>, Ngg1Error> = Ngg1::parse(&bytes);
    let g = g.unwrap();
    let _: usize = g.node_count();
    let _: usize = g.edge_count();
    let _: NodeRecord = g.node(0);
    let _: u32 = g.row_ptr(0);
    let _: u32 = g.col_idx(0);
    let _: u8 = g.edge_type(0);
    let _: String = g.string(0);
    let _: String = g.label(0);
    let _: String = g.iri(0);
    let _: Vec<f32> = g.seed_positions();
    let _: (Vec<u32>, Vec<u32>, Vec<u8>) = g.edges();
    let _: Vec<u32> = g.degrees();
    let e = Ngg1Error("x".to_owned());
    assert_eq!(e.to_string(), "NGG1: x");
    assert_eq!(NGG1_MAGIC.to_le_bytes(), *ontomark_ngg1::MAGIC);
    assert_eq!(
        (NGG1_VERSION, NGG1_HEADER_SIZE, NGG1_NODE_STRIDE),
        (1, 32, 24)
    );
    assert_eq!(
        (
            EDGE_SUBCLASS,
            EDGE_RELATION,
            FLAG_DOMAIN_ROOT,
            FLAG_CATEGORY_ROOT
        ),
        (0, 1, 1, 2)
    );
    assert_eq!(CATEGORY_NONE, 0xFFFF);
}
