//! EXP-03 on synthesised fixtures: unknown metadata and prose survive
//! editing. Parse, patch and re-parse with comments, block scalars, quoting,
//! aliases, Unicode, CRLF, a BOM and a missing final newline; bytes the patch
//! did not name stay identical.

mod common;

use common::{check_lossless, check_patch};
use ontomark_parse::{Document, Patch, PatchError, Placement, Span, Value};

const FIXTURES: &[(&str, &str)] = &[
    (
        "comments-and-quoting",
        include_str!("fixtures/comments-and-quoting.md"),
    ),
    ("corpus-traps", include_str!("fixtures/corpus-traps.md")),
    ("crlf-bom", include_str!("fixtures/crlf-bom.md")),
    (
        "no-final-newline",
        include_str!("fixtures/no-final-newline.md"),
    ),
];

#[test]
fn fixtures_have_the_byte_shapes_they_claim() {
    let crlf = FIXTURES[2].1;
    assert!(
        crlf.starts_with('\u{feff}') && crlf.contains("\r\n"),
        "git must not normalise crlf-bom.md"
    );
    assert!(!FIXTURES[3].1.ends_with('\n'));
}

#[test]
fn every_fixture_round_trips_and_matches_an_independent_yaml_load() {
    for (name, src) in FIXTURES {
        assert_eq!(check_lossless(src), Vec::<String>::new(), "{name}");
    }
}

#[test]
fn every_key_can_be_rewritten_alone() {
    for (name, src) in FIXTURES {
        let doc = Document::parse(*src);
        let keys: Vec<String> = doc
            .frontmatter()
            .unwrap()
            .keys()
            .map(str::to_owned)
            .collect();
        let comments: Vec<&str> = src.lines().filter(|l| l.starts_with('#')).collect();
        for key in &keys {
            let patch = Patch::new(doc.digest())
                .set(key.as_str(), Value::strings(["rewritten ü", "[[X]]"]));
            let set = check_patch(&doc, &patch, &[key], false)
                .unwrap_or_else(|e| panic!("{name}/{key}: {e}"));
            let patch = Patch::new(doc.digest()).remove(key.as_str());
            let rm = check_patch(&doc, &patch, &[key], false)
                .unwrap_or_else(|e| panic!("{name}/-{key}: {e}"));
            // Column-0 comments belong to no key: no single-key edit removes one.
            for c in &comments {
                assert!(
                    set.source().lines().any(|l| l == *c),
                    "{name}/{key}: lost {c:?}"
                );
                assert!(
                    rm.source().lines().any(|l| l == *c),
                    "{name}/-{key}: lost {c:?}"
                );
            }
        }
    }
}

#[test]
fn new_keys_land_where_placed_and_nothing_else_moves() {
    let src = FIXTURES[0].1;
    let doc = Document::parse(src);
    let patch = Patch::new(doc.digest())
        .set_at("first", Value::Int(1), Placement::Start)
        .set_at(
            "after-title",
            Value::Bool(false),
            Placement::After("title".into()),
        )
        .set("last", Value::string("z"));
    let new = check_patch(&doc, &patch, &[], false).unwrap();
    let keys: Vec<&str> = new.frontmatter().unwrap().keys().collect();
    assert_eq!(keys.first(), Some(&"first"));
    assert_eq!(keys.last(), Some(&"last"));
    let t = keys.iter().position(|k| *k == "title").unwrap();
    assert_eq!(keys[t + 1], "after-title");
    // The trailing-comment line on `title` is untouched.
    assert!(new.source().contains(
        "title: \"Café — Naïve Bayes\"   # trailing comment on a value\nafter-title: false\n"
    ));
}

#[test]
fn crlf_pages_get_crlf_edits() {
    let src = FIXTURES[2].1;
    let doc = Document::parse(src);
    let new = check_patch(
        &doc,
        &Patch::new(doc.digest()).set("public", Value::Bool(true)),
        &[],
        false,
    )
    .unwrap();
    assert!(new.source().starts_with('\u{feff}'));
    assert!(new.source().contains("- One\r\npublic: true\r\n---\r\n"));
}

#[test]
fn a_page_without_frontmatter_gains_a_block_after_the_bom() {
    let doc = Document::parse("\u{feff}# Plain note\n\nText.\n");
    let out = doc
        .apply(&Patch::new(doc.digest()).set("type", Value::string("Class")))
        .unwrap();
    assert_eq!(
        out.document.source(),
        "\u{feff}---\ntype: Class\n---\n# Plain note\n\nText.\n"
    );
}

#[test]
fn unquoted_wikilink_item_is_recovered_not_dropped() {
    let src = FIXTURES[1].1;
    let doc = Document::parse(src);
    let fm = doc.frontmatter().unwrap();
    let rel = fm.get("related-to").unwrap();
    let links = rel.wikilinks(src);
    assert_eq!(links.len(), 4, "{links:?}");
    let rec: Vec<_> = links.iter().filter(|l| l.recovered).collect();
    assert_eq!(rec.len(), 1);
    assert_eq!(rec[0].link.target, "Presentation - Conclusion");
    assert_eq!(
        links[0].link.alias.as_deref(),
        Some("ETSI Domain: Creative Media")
    );
    assert_eq!(
        links[1].link.alias.as_deref(),
        Some("Fairness: users #1 priority")
    );
    assert_eq!(links[3].link.target, "Alzheimer's Disease");
}

#[test]
fn body_link_retarget_touches_only_that_span() {
    let src = FIXTURES[1].1;
    let doc = Document::parse(src);
    let link = doc.body_index().links[0].clone();
    assert_eq!(link.span.slice(src), "[[Cell\\|shown]]");
    let patch = Patch::new(doc.digest()).replace_body(
        link.span,
        link.span.slice(src),
        "[[New Cell\\|shown]]",
    );
    let new = check_patch(&doc, &patch, &[], true).unwrap();
    assert_eq!(
        new.source(),
        src.replace("[[Cell\\|shown]]", "[[New Cell\\|shown]]")
    );
    assert_eq!(
        new.frontmatter().unwrap().content.slice(new.source()),
        doc.frontmatter().unwrap().content.slice(src)
    );
}

#[test]
fn guards_refuse_stale_or_unsafe_patches() {
    let src = FIXTURES[0].1;
    let doc = Document::parse(src);
    // Stale digest.
    let other = Document::parse("x");
    assert!(matches!(
        doc.apply(&Patch::new(other.digest()).remove("type")),
        Err(PatchError::DigestMismatch { .. })
    ));
    // Body guard.
    let body = doc.body_span();
    let r = Span::new(body.start, body.start + 3);
    assert!(matches!(
        doc.apply(&Patch::new(doc.digest()).replace_body(r, "nope", "x")),
        Err(PatchError::BodyGuard { .. })
    ));
    // Range in the frontmatter.
    let r = Span::new(0, 3);
    assert!(matches!(
        doc.apply(&Patch::new(doc.digest()).replace_body(r, "---", "+++")),
        Err(PatchError::BadRange(_))
    ));
    // Two ops on one key.
    let p = Patch::new(doc.digest())
        .set("type", Value::Null)
        .remove("type");
    assert!(matches!(doc.apply(&p), Err(PatchError::Overlap(0, 1))));
    // Unknown anchor.
    let p = Patch::new(doc.digest()).set_at("k", Value::Null, Placement::After("missing".into()));
    assert!(matches!(doc.apply(&p), Err(PatchError::AnchorNotFound(_))));
    // A body edit may not fabricate a frontmatter block.
    let plain = Document::parse("hello\n");
    let p = Patch::new(plain.digest()).replace_body(Span::new(0, 5), "hello", "---\na: 1\n---");
    assert!(matches!(plain.apply(&p), Err(PatchError::Verification(_))));
}

#[test]
fn locked_frontmatter_round_trips_but_refuses_key_patches() {
    for src in [
        "---\na: &x 1\nb: *x\n---\nbody\n",
        "---\na: [oops\n---\nbody\n",
        "---\na: 1\na: 2\n---\n",
    ] {
        let doc = Document::parse(src);
        assert_eq!(doc.source(), src);
        let p = Patch::new(doc.digest()).set("c", Value::Null);
        assert!(
            matches!(doc.apply(&p), Err(PatchError::FrontmatterLocked(_))),
            "{src}"
        );
    }
    // Body patches on a locked page still work and leave the block alone.
    let doc = Document::parse("---\na: &x 1\n---\nbody\n");
    let b = doc.body_span();
    let out = doc
        .apply(&Patch::new(doc.digest()).replace_body(b, "body\n", "BODY\n"))
        .unwrap();
    assert_eq!(out.document.source(), "---\na: &x 1\n---\nBODY\n");
}

#[test]
fn no_op_patches_are_idempotent() {
    let src = FIXTURES[0].1;
    let doc = Document::parse(src);
    let current = doc.value("quality").unwrap().clone();
    let p = Patch::new(doc.digest())
        .set("quality", current)
        .remove("absent");
    let out = doc.apply(&p).unwrap();
    assert_eq!(out.document.source(), src);
    assert!(out.receipt.edits.is_empty());
    assert_eq!(out.receipt.before, out.receipt.after);
    // Applying a real change twice: the second plan (against the new digest) is a no-op.
    let p1 = Patch::new(doc.digest()).set("quality", Value::Float("0.9".into()));
    let once = doc.apply(&p1).unwrap().document;
    let p2 = Patch::new(once.digest()).set("quality", Value::Float("0.9".into()));
    let twice = once.apply(&p2).unwrap();
    assert!(twice.receipt.edits.is_empty());
}

#[test]
fn strings_needing_escapes_survive_a_round_trip() {
    let doc = Document::parse("---\na: 1\n---\n");
    for s in [
        "line\nbreak",
        "tab\there",
        "quote ' and \"",
        "--- fence",
        "#hash",
        "trailing ",
        "ünï ✓",
        "\u{7f}del",
    ] {
        let p = Patch::new(doc.digest()).set("v", Value::string(s));
        let out = doc.apply(&p).unwrap_or_else(|e| panic!("{s:?}: {e}"));
        assert_eq!(out.document.value("v"), Some(&Value::string(s)));
    }
}
