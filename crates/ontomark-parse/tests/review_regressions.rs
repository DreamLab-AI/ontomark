//! Regressions from the 2026-10-11 adversarial review of WP-A. Each test
//! reproduces a defect the reviewer confirmed by running it.

use ontomark_parse::{Document, Patch, PatchError, Placement, Span, Value};

fn apply(src: &str, build: impl FnOnce(Patch) -> Patch) -> Result<String, PatchError> {
    let d = Document::parse(src);
    d.apply(&build(Patch::new(d.digest())))
        .map(|a| a.document.source().to_owned())
}

#[test]
fn set_anchored_on_a_key_the_same_patch_removes_is_refused_in_either_order() {
    let src = "---\na: 1\nb: 2\n---\n";
    let anchored = |p: Patch| p.set_at("n", Value::Int(3), Placement::After("a".into()));
    let r1 = apply(src, |p| anchored(p.remove("a")));
    let r2 = apply(src, |p| anchored(p).remove("a"));
    assert!(matches!(r1, Err(PatchError::Overlap(0, 1))), "{r1:?}");
    assert!(matches!(r2, Err(PatchError::Overlap(0, 1))), "{r2:?}");
}

#[test]
fn a_body_edit_cannot_forge_frontmatter_alongside_a_key_set() {
    // The reviewer's input: the fence-like body text must stay body text,
    // after the block the Set creates, never become (part of) frontmatter.
    let r = apply("Body\n", |p| {
        p.replace_body(Span::new(0, 0), "", "---\nk: v\n")
            .set("k", Value::String("v".into()))
    })
    .unwrap();
    assert_eq!(r, "---\nk: v\n---\n---\nk: v\nBody\n");
    let d = Document::parse(r.as_str());
    let fm = d.frontmatter().unwrap();
    assert_eq!(fm.keys().collect::<Vec<_>>(), ["k"]);
    assert_eq!(d.body(), "---\nk: v\nBody\n");
    // Without the Set, the same body edit is refused outright.
    let r = apply("Body\n", |p| {
        p.replace_body(Span::new(0, 0), "", "---\nk: v\n---\n")
    });
    assert!(matches!(r, Err(PatchError::Verification(_))), "{r:?}");
}

#[test]
fn a_body_prefix_and_a_new_frontmatter_block_compose() {
    let r = apply("Body\n", |p| {
        p.replace_body(Span::new(0, 0), "", "Intro\n")
            .set("k", Value::String("v".into()))
    });
    assert_eq!(r.unwrap(), "---\nk: v\n---\nIntro\nBody\n");
}

#[test]
fn end_and_after_last_insertions_agree_on_order() {
    let src = "---\na: 1\nd: 2\n---\n";
    for (e_first, want) in [
        (true, "---\na: 1\nd: 2\nf: 4\ne: 3\n---\n"),
        (false, "---\na: 1\nd: 2\nf: 4\ne: 3\n---\n"),
    ] {
        let r = apply(src, |p| {
            let e = |p: Patch| p.set("e", Value::Int(3));
            let f = |p: Patch| p.set_at("f", Value::Int(4), Placement::After("d".into()));
            if e_first {
                f(e(p))
            } else {
                e(f(p))
            }
        });
        assert_eq!(r.unwrap(), want, "e_first={e_first}");
    }
}

#[test]
fn keep_chomped_block_scalars_keep_their_trailing_blank_lines() {
    let src = "---\na: |+\n  x\n\n\nb: 1\n---\n";
    let r = apply(src, |p| {
        p.set_at("m", Value::Int(2), Placement::After("a".into()))
    });
    assert_eq!(r.unwrap(), "---\na: |+\n  x\n\n\nm: 2\nb: 1\n---\n");
}

#[test]
fn a_document_end_marker_locks_the_block() {
    let src = "---\na: 1\n...\n---\n";
    let d = Document::parse(src);
    assert_eq!(d.source(), src);
    let r = apply(src, |p| p.set("a", Value::Int(2)));
    assert!(matches!(r, Err(PatchError::FrontmatterLocked(_))), "{r:?}");
}

#[test]
fn document_marker_lookalike_keys_and_values_are_quoted() {
    let r = apply("---\na: 1\n---\n", |p| {
        p.set("... x", Value::String("--- y".into()))
    });
    assert_eq!(r.unwrap(), "---\na: 1\n'... x': '--- y'\n---\n");
}

#[test]
fn the_empty_key_can_be_patched() {
    let r = apply("---\n'': 1\nb: 2\n---\n", |p| p.set("", Value::Int(5)));
    assert_eq!(r.unwrap(), "---\n'': 5\nb: 2\n---\n");
    let r = apply("---\n'': 1\nb: 2\n---\n", |p| p.remove(""));
    assert_eq!(r.unwrap(), "---\nb: 2\n---\n");
}

#[test]
fn a_float_the_reader_would_not_type_as_a_float_is_refused_by_name() {
    for raw in ["1_0.0", "nan", "abc"] {
        let r = apply("---\na: 1\n---\n", |p| p.set("q", Value::Float(raw.into())));
        assert!(
            matches!(&r, Err(PatchError::Unrepresentable(t)) if t == raw),
            "{raw}: {r:?}"
        );
        // Nested values are checked too.
        let r = apply("---\na: 1\n---\n", |p| {
            p.set("q", Value::Seq(vec![Value::Float(raw.into())]))
        });
        assert!(
            matches!(r, Err(PatchError::Unrepresentable(_))),
            "{raw} nested: {r:?}"
        );
    }
    for raw in ["0.70", ".inf", "1e3"] {
        let r = apply("---\na: 1\n---\n", |p| p.set("q", Value::Float(raw.into())));
        assert_eq!(r.unwrap(), format!("---\na: 1\nq: {raw}\n---\n"));
    }
}

// ---------- second review: Utf16Index ----------

use ontomark_parse::Utf16Index;

#[test]
fn utf16_offsets_round_trip_both_ways_at_every_boundary() {
    for s in [
        "",
        "abc",
        "é",
        "€",
        "𝄞",
        "𝄞𝄞",
        "a\r\nb",
        "é𝄞x\r\n€",
        "\u{feff}k",
        "e\u{301}",
    ] {
        let ix = Utf16Index::new(s);
        let units: Vec<u16> = s.encode_utf16().collect();
        for b in 0..=s.len() + 2 {
            let want = s.is_char_boundary(b).then(|| s[..b].encode_utf16().count());
            let want = if b > s.len() { None } else { want };
            assert_eq!(ix.offset(b), want, "{s:?} byte {b}");
            if let Some(u) = want {
                assert_eq!(ix.byte(u), Some(b), "{s:?} unit {u}");
            }
        }
        for u in 0..=units.len() + 2 {
            // Inside a surrogate pair, or past the end: no byte offset.
            let inside = u > 0 && u < units.len() && (0xDC00..0xE000).contains(&units[u]);
            if inside || u > units.len() {
                assert_eq!(ix.byte(u), None, "{s:?} unit {u}");
            } else {
                assert!(ix.byte(u).is_some(), "{s:?} unit {u}");
            }
        }
    }
}

#[test]
fn a_reversed_span_has_no_utf16_range() {
    let ix = Utf16Index::new("abc");
    assert_eq!(ix.span(Span { start: 2, end: 0 }), None);
    assert_eq!(ix.span(Span::new(0, 2)), Some((0, 2)));
}
