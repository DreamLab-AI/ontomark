//! ADR-001 acceptance 1: the TypeScript reader and this crate decode the same
//! bytes to the same graph. The TypeScript side is the real `ngg1.ts`,
//! transpiled by `conformance/ngg1/ts-reader-check.mjs`; the test is skipped
//! with a printed note when `node` or the visionGraph project is unavailable.

mod common;

use ontomark_ngg1::Ngg1;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;

fn hex(s: &str) -> String {
    s.bytes().fold(String::new(), |mut acc, b| {
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// The same canonical text the script prints.
fn render(bytes: &[u8]) -> String {
    let g = Ngg1::parse(bytes).unwrap();
    let mut out = format!("nodes {} edges {}\n", g.node_count(), g.edge_count());
    for i in 0..g.node_count() {
        let n = g.node(i);
        let _ = writeln!(
            out,
            "n {i} {} {} {} {} {} {} {} {} {}",
            n.id,
            n.x.to_bits(),
            n.y.to_bits(),
            n.domain,
            n.category,
            n.flags,
            n.degree,
            hex(&g.label(i)),
            hex(&g.iri(i))
        );
    }
    let (src, tgt, ty) = g.edges();
    for k in 0..src.len() {
        let _ = writeln!(out, "e {} {} {}", src[k], tgt[k], ty[k]);
    }
    out
}

#[test]
fn the_typescript_reader_agrees_with_the_rust_reader() {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/ngg1/ts-reader-check.mjs");
    let mut files: Vec<PathBuf> = std::fs::read_dir(common::golden_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("bin"))
        .collect();
    files.sort();
    let mut compared = 0;
    for file in &files {
        let out = match Command::new("node").arg(&script).arg(file).output() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("note: node unavailable ({e}); TypeScript agreement check skipped");
                return;
            }
        };
        if out.status.code() == Some(3) {
            eprintln!(
                "note: the TypeScript reader could not be loaded ({}); agreement check skipped",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            return;
        }
        assert!(
            out.status.success(),
            "{}: {}",
            file.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        let ts = String::from_utf8(out.stdout).unwrap();
        let rs = render(&std::fs::read(file).unwrap());
        assert_eq!(
            ts,
            rs,
            "{}: TypeScript and Rust readers disagree",
            file.display()
        );
        compared += 1;
    }
    eprintln!("ts agreement: {compared} golden tier(s) identical in TypeScript and Rust");
}
