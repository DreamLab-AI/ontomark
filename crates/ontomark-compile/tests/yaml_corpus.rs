//! YAML backend contract over the repository's own corpus.
//!
//! Every frontmatter block, vocabulary and profile the repository ships is
//! read through the compiler's YAML entry points, and the result is pinned in
//! `tests/golden/yaml_corpus.tsv`:
//!
//! * a type-tagged canonical dump of each parsed frontmatter mapping, so a
//!   backend that reads `1` as a string or `1.0` as an integer is caught;
//! * the bytes `Frontmatter::render` writes, so the house emit style is held;
//! * parse → render → parse identity and render idempotence;
//! * the parsed vocabulary and profile (as JSON) and the vocabulary's YAML
//!   serialisation, whose digest feeds the build's content digest.
//!
//! The golden was captured on serde_yaml 0.9.34 before the backend moved off
//! it, so a backend swap must reproduce serde_yaml's readings and bytes.
//! Set `ONTOMARK_BLESS=1` only when a reading is deliberately changed.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ontomark_compile::input::frontmatter::{split, Frontmatter};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::profile::Profile;
use serde_norway::Value as Yaml;
use sha2::{Digest, Sha256};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sha(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// A canonical, type-tagged dump that does not depend on the backend's
/// `Debug` output.
fn dump(v: &Yaml, out: &mut String) {
    match v {
        Yaml::Null => out.push_str("null"),
        Yaml::Bool(b) => write!(out, "bool:{b}").unwrap(),
        Yaml::Number(n) => {
            let kind = if n.is_u64() {
                "u64"
            } else if n.is_i64() {
                "i64"
            } else {
                "f64"
            };
            write!(out, "{kind}:{n}").unwrap();
        }
        Yaml::String(s) => write!(out, "str:{s:?}").unwrap(),
        Yaml::Sequence(items) => {
            out.push('[');
            for item in items {
                dump(item, out);
                out.push(',');
            }
            out.push(']');
        }
        Yaml::Mapping(map) => {
            out.push('{');
            for (k, v) in map {
                dump(k, out);
                out.push_str(": ");
                dump(v, out);
                out.push(',');
            }
            out.push('}');
        }
        Yaml::Tagged(t) => {
            write!(out, "!{}(", t.tag).unwrap();
            dump(&t.value, out);
            out.push(')');
        }
    }
}

fn markdown_files() -> Vec<PathBuf> {
    let root = root();
    let mut files: Vec<PathBuf> = ["conformance/fixtures", "crates"]
        .iter()
        .flat_map(|dir| walkdir::WalkDir::new(root.join(dir)))
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .filter(|p| {
            let rel = p.strip_prefix(&root).unwrap().to_string_lossy();
            rel.starts_with("conformance/") || rel.contains("/tests/fixtures/")
        })
        .collect();
    files.sort();
    files
}

fn rel(p: &Path) -> String {
    p.strip_prefix(root())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// One golden line per page: path, then `none`, `err`, or the dump digest,
/// the render digest and the round-trip verdict.
fn page_line(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let Some(yaml) = split(&text).yaml else {
        return format!("{}\tnone", rel(path));
    };
    let Ok(fm) = Frontmatter::parse(yaml) else {
        return format!("{}\terr", rel(path));
    };
    let mut d = String::new();
    dump(&Yaml::Mapping(fm.map.clone()), &mut d);
    let rendered = fm.render().expect("a parsed mapping renders");
    let again = Frontmatter::parse(split(&rendered).yaml.unwrap_or("")).expect("render re-parses");
    let identical = again.map == fm.map;
    let idempotent = again.render().unwrap() == rendered;
    format!(
        "{}\tpage\t{}\t{}\troundtrip={identical}\tidempotent={idempotent}",
        rel(path),
        sha(&d),
        sha(&rendered)
    )
}

fn yaml_lines() -> Vec<String> {
    let root = root();
    let mut lines = Vec::new();
    for vocab in [
        "conformance/fixtures/estate-small/ontology/vocabulary.yaml",
        "conformance/fixtures/estate-invalid/ontology/vocabulary.yaml",
    ] {
        let text = std::fs::read_to_string(root.join(vocab)).unwrap();
        let v = Vocabulary::from_yaml_str(&text).expect("shipped vocabulary parses");
        let json = serde_json::to_string(&v).unwrap();
        let yaml = serde_norway::to_string(&v).unwrap();
        lines.push(format!(
            "{vocab}\tvocabulary\t{}\t{}",
            sha(&json),
            sha(&yaml)
        ));
    }
    let profile = "profiles/estate/profile.yaml";
    let text = std::fs::read_to_string(root.join(profile)).unwrap();
    let p = Profile::from_yaml(&text).expect("shipped profile parses");
    lines.push(format!(
        "{profile}\tprofile\t{}",
        sha(&serde_json::to_string(&p).unwrap())
    ));
    lines
}

#[test]
fn yaml_backend_reproduces_the_pinned_corpus_readings() {
    let mut actual: Vec<String> = markdown_files().iter().map(|p| page_line(p)).collect();
    actual.extend(yaml_lines());
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/yaml_corpus.tsv");
    let body = actual.join("\n") + "\n";
    if std::env::var_os("ONTOMARK_BLESS").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, &body).unwrap();
    }
    let expected = std::fs::read_to_string(&golden).expect("golden present");
    let pages = actual.iter().filter(|l| l.contains("\tpage\t")).count();
    assert!(pages >= 200, "corpus shrank to {pages} parsed pages");
    let diffs: Vec<String> = expected
        .lines()
        .zip(body.lines())
        .filter(|(e, a)| e != a)
        .map(|(e, a)| format!("- {e}\n+ {a}"))
        .collect();
    assert!(
        diffs.is_empty() && expected.lines().count() == body.lines().count(),
        "{} corpus readings changed:\n{}",
        diffs.len(),
        diffs.join("\n")
    );
    assert!(
        actual
            .iter()
            .filter(|l| l.contains("\tpage\t"))
            .all(|l| l.ends_with("roundtrip=true\tidempotent=true")),
        "a page failed parse -> render -> parse"
    );
}
