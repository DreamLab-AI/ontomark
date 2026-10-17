//! Corpus-scale model build, read-only and env-gated: set `ONTOMARK_CORPUS`
//! (a vault root: the directory whose Markdown notes are the pages) and
//! `ONTOMARK_PROFILE` (a JSON `ProfileSpec`).
//! Prints counts only: no titles, paths or page text.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ontomark_model::{
    slugify, DeclarativeProfile, DocDisposition, DocId, Model, Profile as _, ProfileSpec,
};
use ontomark_parse::{Document, Value};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
}

#[test]
fn model_builds_over_every_public_page() {
    let (Ok(root), Ok(spec)) = (
        std::env::var("ONTOMARK_CORPUS"),
        std::env::var("ONTOMARK_PROFILE"),
    ) else {
        eprintln!("model corpus: SKIPPED (ONTOMARK_CORPUS / ONTOMARK_PROFILE unset)");
        return;
    };
    let spec: ProfileSpec = serde_json::from_str(&std::fs::read_to_string(spec).unwrap()).unwrap();
    let profile = DeclarativeProfile::new(spec).unwrap();
    let mut files = Vec::new();
    walk(Path::new(&root), &mut files);
    files.sort();
    let docs: Vec<(DocId, Document)> = files
        .iter()
        .filter_map(|p| {
            let src = std::fs::read_to_string(p).ok()?;
            let doc = Document::parse(src);
            let all = std::env::var("ONTOMARK_ALL_PAGES").is_ok();
            (all || doc.value("public") == Some(&Value::Bool(true))).then(|| {
                (
                    DocId::new(p.strip_prefix(&root).unwrap().to_string_lossy()),
                    doc,
                )
            })
        })
        .collect();
    let t0 = std::time::Instant::now();
    let m = Model::build(&profile, docs.iter().map(|(i, d)| (i.clone(), d)));
    let elapsed = t0.elapsed();
    let mut disp: BTreeMap<&str, usize> = BTreeMap::new();
    for r in m.documents.values() {
        let k = match r.disposition {
            DocDisposition::Entity(_) => "entity",
            DocDisposition::NotOntology => "not_ontology",
            DocDisposition::NoIdentity => "no_identity",
            DocDisposition::Refused(_) => "refused",
            DocDisposition::IdentityConflict(_) => "identity_conflict",
        };
        *disp.entry(k).or_default() += 1;
    }
    let mut diags: BTreeMap<(String, String), usize> = BTreeMap::new();
    for d in &m.diagnostics {
        *diags
            .entry((format!("{:?}", d.severity), d.code.clone()))
            .or_default() += 1;
    }
    let mut forms: BTreeMap<String, usize> = BTreeMap::new();
    let mut dispositions: BTreeMap<String, usize> = BTreeMap::new();
    for s in m.entities.values().flat_map(|e| &e.statements) {
        *forms.entry(format!("{:?}", s.form)).or_default() += 1;
        let d = match &s.disposition {
            ontomark_model::Disposition::Supported => "supported".to_owned(),
            ontomark_model::Disposition::PreservedNotReasoned(r) => format!("preserved: {r}"),
            ontomark_model::Disposition::Rejected(r) => {
                format!("rejected: {}", r.split(':').next().unwrap_or(r))
            }
        };
        *dispositions.entry(d).or_default() += 1;
    }
    let (res, amb, unres, ext) = m.resolution_counts();
    eprintln!(
        "model corpus: docs {} (all pages: {}), built in {elapsed:?}",
        docs.len(),
        std::env::var("ONTOMARK_ALL_PAGES").is_ok()
    );
    eprintln!("  documents: {disp:?}");
    eprintln!("  references: resolved {res}, ambiguous {amb}, unresolved {unres}, external {ext}");
    eprintln!("  forms: {forms:?}");
    eprintln!("  dispositions: {dispositions:?}");
    eprintln!("  diagnostics: {diags:?}");
    eprintln!("  policy findings: {}", m.policy.len());
    // Slug parity: does the minter's rule reproduce the resources the estate
    // already minted? Compared against the profile's label (title, else note
    // name). Counts only; set ONTOMARK_SLUG_EXAMPLES=n to print n public
    // mismatches for triage.
    let examples: usize = std::env::var("ONTOMARK_SLUG_EXAMPLES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let (mut minted, mut agree, mut shown) = (0usize, 0usize, 0usize);
    for (id, d) in &docs {
        let Some(Value::String(res)) = d.value("resource") else {
            continue;
        };
        let Some(tail) = res.strip_prefix("urn:ngm:class:") else {
            continue;
        };
        minted += 1;
        let label = profile.label(id, d);
        if slugify(&label) == tail {
            agree += 1;
        } else if shown < examples && d.value("public") == Some(&Value::Bool(true)) {
            shown += 1;
            eprintln!(
                "    slug mismatch: {label:?} -> {:?} vs {tail:?}",
                slugify(&label)
            );
        }
    }
    eprintln!("  slug parity: {agree}/{minted} urn:ngm:class: resources equal slugify(label)");
    assert_eq!(m.documents.len(), docs.len());
    assert!(disp.get("entity").copied().unwrap_or(0) > 0);
}
