//! Malformed input is a typed error, never a panic.

mod common;

use common::{random_graph, Rng};
use ontomark_ngg1::{encode, worked_example, Ngg1, Ngg1Fault};

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn fault(bytes: &[u8]) -> Ngg1Fault {
    match Ngg1::try_parse(bytes) {
        Ok(_) => panic!("expected a fault"),
        Err(f) => f,
    }
}

#[test]
fn truncated_header() {
    let b = worked_example();
    for len in 0..32 {
        assert_eq!(fault(&b[..len]), Ngg1Fault::TooSmall { len });
    }
}

#[test]
fn truncated_sections() {
    let b = worked_example();
    // A 100-byte cut: the string header at offset 132 is the first thing missing.
    assert_eq!(fault(&b[..100]), Ngg1Fault::StringHeaderOutOfBounds);
    // Cut inside the nodes, adjacency, edge types, string table and blob.
    for len in [40, 103, 110, 127, 131, 133, 139, 150, 182] {
        let f = fault(&b[..len]);
        assert!(
            matches!(
                f,
                Ngg1Fault::SectionOutOfBounds
                    | Ngg1Fault::StringHeaderOutOfBounds
                    | Ngg1Fault::BlobOutOfBounds
            ),
            "len {len}: {f:?}"
        );
    }
}

#[test]
fn out_of_range_offsets() {
    let base = worked_example();
    let mut b = base.clone();
    put_u32(&mut b, 20, 0xFFFF_FFF0);
    assert!(matches!(
        fault(&b),
        Ngg1Fault::SectionOutOfBounds | Ngg1Fault::SectionOverflow
    ));
    let mut b = base.clone();
    put_u32(&mut b, 28, 0x0100_0000);
    assert_eq!(fault(&b), Ngg1Fault::StringHeaderOutOfBounds);
    let mut b = base.clone();
    put_u32(&mut b, 24, 0xFFFF_FFFC);
    assert!(matches!(
        fault(&b),
        Ngg1Fault::SectionOutOfBounds | Ngg1Fault::SectionOverflow
    ));
    let mut b = base.clone();
    put_u32(&mut b, 20, 105);
    assert_eq!(fault(&b), Ngg1Fault::Misaligned);
    let mut b = base.clone();
    put_u32(&mut b, 16, 0);
    assert_eq!(fault(&b), Ngg1Fault::OffsetInHeader { section: "nodes" });
    let mut b = base;
    put_u32(&mut b, 8, 0x7FFF_FFFF); // node count far beyond the buffer
    assert!(matches!(
        fault(&b),
        Ngg1Fault::SectionOutOfBounds | Ngg1Fault::SectionOverflow
    ));
}

#[test]
fn wrong_magic_and_unknown_version() {
    let mut b = worked_example();
    b[0] = b'X';
    assert!(matches!(fault(&b), Ngg1Fault::BadMagic { .. }));
    let mut b = worked_example();
    b[4] = 2;
    assert_eq!(fault(&b), Ngg1Fault::UnsupportedVersion { found: 2 });
    // The flattened error keeps the vowl-wasm wording.
    let err = Ngg1::parse(&b).err().unwrap();
    assert_eq!(
        err.to_string(),
        "NGG1: unsupported version 2 (reader supports 1)"
    );
}

#[test]
fn edge_endpoint_out_of_range() {
    let mut b = worked_example();
    put_u32(&mut b, 120, 3); // col_idx[0] = 3 with only 3 nodes
    assert_eq!(
        fault(&b),
        Ngg1Fault::EdgeTargetOutOfRange {
            edge: 0,
            target: 3,
            nodes: 3
        }
    );
    let mut b = worked_example();
    put_u32(&mut b, 124, u32::MAX);
    assert!(matches!(
        fault(&b),
        Ngg1Fault::EdgeTargetOutOfRange { edge: 1, .. }
    ));
}

#[test]
fn csr_and_string_table_corruption() {
    let mut b = worked_example();
    put_u32(&mut b, 108, 1); // row_ptr[1] = 1, then row_ptr[2] = 2 ... still monotone
    assert!(Ngg1::try_parse(&b).is_ok());
    put_u32(&mut b, 112, 0); // row_ptr[2] = 0 < row_ptr[1]
    assert_eq!(fault(&b), Ngg1Fault::BadRowPtr { index: 2 });
    let mut b = worked_example();
    put_u32(&mut b, 104, 1);
    assert_eq!(fault(&b), Ngg1Fault::BadRowPtr { index: 0 });
    let mut b = worked_example();
    put_u32(&mut b, 116, 1);
    assert_eq!(fault(&b), Ngg1Fault::BadRowPtr { index: 3 });
    let mut b = worked_example();
    put_u32(&mut b, 132, 4);
    assert_eq!(
        fault(&b),
        Ngg1Fault::StringCountMismatch { count: 4, nodes: 3 }
    );
    let mut b = worked_example();
    put_u32(&mut b, 136, 0xFFFF_FF00);
    assert_eq!(fault(&b), Ngg1Fault::BlobOutOfBounds);
    let mut b = worked_example();
    put_u32(&mut b, 144, 0xFF); // offset[1]
    assert_eq!(fault(&b), Ngg1Fault::BadStringOffset { index: 1 });
}

#[test]
fn invalid_utf8_is_lossy_for_accessors_and_strict_for_to_graph() {
    let mut b = worked_example();
    b[164] = 0xFF; // first blob byte
    let g = Ngg1::parse(&b).unwrap();
    assert!(g.label(0).contains('\u{FFFD}'));
    assert_eq!(
        g.to_graph().unwrap_err(),
        Ngg1Fault::InvalidUtf8 { index: 0 }
    );
}

/// Touch every accessor of a successfully parsed buffer; none may panic.
fn exercise(bytes: &[u8]) {
    if let Ok(g) = Ngg1::try_parse(bytes) {
        for i in 0..g.node_count().min(10_000) {
            let _ = g.node(i);
            let _ = g.label(i);
            let _ = g.iri(i);
        }
        for e in 0..g.edge_count().min(10_000) {
            let _ = g.col_idx(e);
            let _ = g.edge_type(e);
        }
        let _ = g.edges();
        let _ = g.seed_positions();
        let _ = g.degrees();
        if let Ok(graph) = g.to_graph() {
            assert!(graph.encode().is_ok(), "a decodable tier re-encodes");
        }
    }
}

fn corpus() -> Vec<Vec<u8>> {
    let (nodes, csr) = random_graph(9, 12, 30);
    vec![worked_example(), encode(&nodes, &csr).unwrap()]
}

#[test]
fn truncating_at_every_length_never_panics_and_always_errors() {
    for valid in corpus() {
        for len in 0..valid.len() {
            assert!(
                Ngg1::try_parse(&valid[..len]).is_err(),
                "len {len} of {}",
                valid.len()
            );
            exercise(&valid[..len]);
        }
        assert!(Ngg1::try_parse(&valid).is_ok());
    }
}

#[test]
fn flipping_bytes_never_panics() {
    for valid in corpus() {
        for pos in 0..valid.len() {
            for mask in [0x01u8, 0x80, 0xFF, 0x5A] {
                let mut b = valid.clone();
                b[pos] ^= mask;
                exercise(&b);
            }
        }
    }
}

#[test]
fn random_multi_byte_corruption_never_panics() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for valid in corpus() {
        for _ in 0..20_000 {
            let mut b = valid.clone();
            for _ in 0..=rng.below(4) {
                let pos = rng.below(b.len());
                b[pos] = rng.next() as u8;
            }
            if rng.below(8) == 0 {
                b.truncate(rng.below(b.len() + 1));
            }
            exercise(&b);
        }
    }
}
