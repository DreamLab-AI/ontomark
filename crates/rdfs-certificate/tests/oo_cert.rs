//! The certificate this crate writes is the format the independent checker
//! reads, and the checker refuses a forged line.
//!
//! The checker is `oo-cert` (Open Ontologies, Lean 4, theorem
//! `OOCert.certificate_sound`). It shares no code with this crate, which is
//! the point, so it is not a build dependency: set `OO_CERT_BIN` to the
//! binary to run the two checks, otherwise they are skipped and say so.

use std::path::Path;
use std::process::Command;

use rdfs_certificate::{
    certify, nt, Certificate, RDFS_DOMAIN, RDFS_SUBCLASS_OF, RDFS_SUB_PROPERTY_OF, RDF_TYPE,
};

fn fixture() -> Certificate {
    let i = |l: &str| nt::iri(&format!("urn:ex:{l}"));
    let t = [
        (i("espresso"), nt::iri(RDFS_SUBCLASS_OF), i("coffee")),
        (i("coffee"), nt::iri(RDFS_SUBCLASS_OF), i("drink")),
        (i("cup"), nt::iri(RDF_TYPE), i("espresso")),
        (i("serves"), nt::iri(RDFS_DOMAIN), i("cafe")),
        (i("pours"), nt::iri(RDFS_SUB_PROPERTY_OF), i("serves")),
        (i("bar"), i("pours"), i("cup")),
        (
            i("cup"),
            nt::iri("http://www.w3.org/2000/01/rdf-schema#label"),
            nt::literal("my cup \"of\" joy\n", Some("en"), None),
        ),
        (i("drink"), nt::iri(RDFS_SUBCLASS_OF), nt::blank("r")),
        (
            nt::blank("r"),
            nt::iri("http://www.w3.org/2002/07/owl#someValuesFrom"),
            i("liquid"),
        ),
    ];
    certify(
        t.iter()
            .map(|(s, p, o)| (s.as_str(), p.as_str(), o.as_str())),
    )
}

fn checker() -> Option<String> {
    let bin = std::env::var("OO_CERT_BIN").ok()?;
    if Path::new(&bin).is_file() {
        Some(bin)
    } else {
        eprintln!("OO_CERT_BIN={bin} is not a file; skipping");
        None
    }
}

fn run(bin: &str, dir: &Path) -> (bool, String) {
    let out = Command::new(bin)
        .arg(dir.join("asserted.tsv"))
        .arg(dir.join("derivations.tsv"))
        .output()
        .expect("oo-cert runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn the_independent_checker_accepts_what_this_crate_writes() {
    let Some(bin) = checker() else {
        eprintln!("OO_CERT_BIN unset; skipping the independent check");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let cert = fixture();
    let summary = cert.write(dir.path(), "test").unwrap();
    assert!(summary.derivations > 0);
    let (ok, stdout) = run(&bin, dir.path());
    assert!(ok, "oo-cert refused a genuine certificate: {stdout}");
    assert!(stdout.contains("\"ok\":true"), "{stdout}");
    assert!(
        stdout.contains(&format!("\"derivations\":{}", summary.derivations)),
        "{stdout}"
    );
}

#[test]
fn the_independent_checker_refuses_a_forged_conclusion() {
    let Some(bin) = checker() else {
        eprintln!("OO_CERT_BIN unset; skipping the independent check");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    fixture().write(dir.path(), "test").unwrap();
    let path = dir.path().join("derivations.tsv");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut fields: Vec<String> = lines[0].split('\t').map(str::to_owned).collect();
    fields[3] = "<urn:ex:beer>".to_owned();
    lines[0] = fields.join("\t");
    std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
    let (ok, stdout) = run(&bin, dir.path());
    assert!(!ok, "oo-cert accepted a forged line: {stdout}");
    assert!(stdout.contains("\"ok\":false"), "{stdout}");
    assert!(stdout.contains("\"first_rejected\":0"), "{stdout}");
}
