//! Create and patch: positive and negative cases, including the version 2 pass-through.

mod common;

use common::*;
use ontomark_parse::{Document, SourceDigest, Value};
use ontomark_write::{code, BodyEdit, Change, CreateRequest, PatchRequest, Role};

fn create_req(id: &str, ty: &str) -> CreateRequest {
    CreateRequest {
        id: id.into(),
        page_type: ty.into(),
        fields: vec![
            ("about".into(), Value::string("https://example.org/tea")),
            ("confidence".into(), Value::Float("0.7".into())),
            (
                "sources".into(),
                Value::Seq(vec![Value::Map(
                    [
                        ("id".to_owned(), Value::string("a")),
                        ("resource".to_owned(), Value::string("https://x.example")),
                        ("rel".to_owned(), Value::string("evidence")),
                        ("locator".to_owned(), Value::string("p1")),
                    ]
                    .into_iter()
                    .collect(),
                )]),
            ),
        ],
        body: "Tea before coffee.\n".into(),
        visibility: None,
        writer: human(),
        now: T1.into(),
        entropy: [0xab; 32],
    }
}

// ---------------------------------------------------------------- create

#[test]
fn create_mints_identity_stamps_and_initial_status() {
    let e = engine(&[]);
    let plan = e
        .create(&create_req("beliefs/Tea first", "Belief"))
        .unwrap();
    assert_eq!(plan.writes.len(), 1);
    let w = &plan.writes[0];
    assert!(w.before_text.is_none());
    assert_eq!(w.rel_path.to_str(), Some("beliefs/Tea first.md"));
    let doc = Document::parse(&w.text);
    let get = |k: &str| doc.value(k).cloned();
    assert_eq!(get("type"), Some(Value::string("Belief")));
    assert_eq!(get("status"), Some(Value::string("draft")));
    assert_eq!(get("visibility"), Some(Value::string("shared")));
    // B4: content-independent hex id, never a title slug.
    let urn = get("page_resource").unwrap().scalar_text().unwrap();
    assert_eq!(urn, format!("urn:visionflow:page:{}", "ab".repeat(32)));
    assert!(!urn.to_lowercase().contains("tea"));
    // First generated stamp: principal, no via for a direct writer, at.
    let g = get("generated").unwrap();
    let g = g.as_map().unwrap();
    assert_eq!(g["by"], Value::string(HUMAN));
    assert_eq!(g["at"], Value::string(T1));
    assert!(!g.contains_key("via"));
    // ADR-VG-012 section 6: contributors mirror the origin; ADR-VG-013 section 7.
    assert_eq!(get("contributors").unwrap().as_seq().unwrap().len(), 1);
    assert_eq!(get("content-updated"), Some(Value::string(T1)));
    assert!(w.text.ends_with("Tea before coffee.\n"));
}

#[test]
fn create_by_an_agent_records_via_and_canonicalises_an_also_key() {
    let e = engine(&[]);
    let mut req = create_req("beliefs/A", "Belief");
    req.writer = ontomark_write::Writer::new(HUMAN_ALSO).via(AGENT);
    let plan = e.create(&req).unwrap();
    let doc = Document::parse(&plan.writes[0].text);
    let g = doc.value("generated").unwrap().as_map().unwrap().clone();
    assert_eq!(
        g["by"],
        Value::string(HUMAN),
        "the also key resolves to the canonical principal"
    );
    assert_eq!(g["via"], Value::string(AGENT));
}

#[test]
fn create_accepts_an_npub_and_writes_the_did() {
    let e = engine(&[]);
    let mut req = create_req("beliefs/N", "Belief");
    let npub =
        ontomark_compile::input::principal::hex_to_npub(&HUMAN["did:nostr:".len()..]).unwrap();
    req.writer = ontomark_write::Writer::new(format!("human:{npub}"));
    let plan = e.create(&req).unwrap();
    assert!(
        plan.writes[0].text.contains(&format!("by: {HUMAN}")),
        "{}",
        plan.writes[0].text
    );
    // An undecodable npub is refused.
    req.writer = ontomark_write::Writer::new("human:npub1notbech32");
    assert_eq!(e.create(&req).unwrap_err().code, code::INVALID_REQUEST);
}

#[test]
fn create_refuses_under_vocabulary_2() {
    let e = engine_with(VOCAB_V2, true, &[], &[]);
    let err = e.create(&create_req("beliefs/Tea", "Belief")).unwrap_err();
    assert_eq!(err.code, code::VOCABULARY_V2);
}

#[test]
fn create_refuses_what_it_must() {
    let e = engine(&[("beliefs/Existing", &belief("e1", "draft", ""))]);
    let mut cases: Vec<(CreateRequest, &str)> = Vec::new();
    // an existing id, case-insensitively
    cases.push((create_req("beliefs/existing", "Belief"), code::EXISTS));
    // path escapes and non-page folders
    for bad in [
        "../x",
        "a/../b",
        ".hidden",
        "a/.b",
        "x.md",
        "journals/2026-01-01",
        "templates/t",
        "",
        " pad",
        "a\\b",
        "a//b",
    ] {
        cases.push((create_req(bad, "Belief"), code::PATH_ESCAPE));
    }
    // unknown type
    cases.push((create_req("beliefs/T", "Nonsense"), code::INVALID_REQUEST));
    // engine-owned keys supplied by the client
    for key in [
        "page_resource",
        "generated",
        "contributors",
        "status",
        "type",
        "content-updated",
        "last-reviewed",
        "supersedes",
        "superseded-by",
        "disputes",
        "valid-to",
        "status-reason",
    ] {
        let mut r = create_req("beliefs/K", "Belief");
        r.fields.push((key.into(), Value::string("x")));
        cases.push((r, code::ENGINE_OWNED_KEY));
    }
    // private has no store yet; unknown visibility
    let mut r = create_req("beliefs/P", "Belief");
    r.visibility = Some("private".into());
    cases.push((r, code::INVALID_REQUEST));
    let mut r = create_req("beliefs/Q", "Belief");
    r.visibility = Some("secret".into());
    cases.push((r, code::INVALID_REQUEST));
    // bad clock, bad writer
    let mut r = create_req("beliefs/C", "Belief");
    r.now = "2026-11-10".into();
    cases.push((r, code::INVALID_REQUEST));
    let mut r = create_req("beliefs/W", "Belief");
    r.writer = ontomark_write::Writer::new("agent:claude/1");
    cases.push((r, code::INVALID_REQUEST));
    let mut r = create_req("beliefs/V", "Belief");
    r.writer = ontomark_write::Writer::new(HUMAN).via("human:someone");
    cases.push((r, code::INVALID_REQUEST));
    // a type whose status set has no draft
    let nodraft = engine_with(
        &VOCAB_V3.replace(
            "Note: { status: [draft, final] }",
            "Note: { status: [final] }",
        ),
        true,
        &[],
        &[],
    );
    let err = nodraft.create(&create_req("n/N", "Note")).unwrap_err();
    assert_eq!(err.code, code::INVALID_REQUEST);
    for (req, want) in cases {
        let err = e.create(&req).unwrap_err();
        assert_eq!(err.code, want, "{:?}: {err}", req.id);
    }
}

#[test]
fn create_that_would_introduce_an_error_is_refused_and_quotes_it() {
    let e = engine(&[]);
    let mut req = create_req("beliefs/NoAbout", "Belief");
    req.fields.retain(|(k, _)| k != "about");
    let err = e.create(&req).unwrap_err();
    assert_eq!(err.code, code::WOULD_INTRODUCE_ERRORS);
    assert!(
        err.issues.iter().any(|i| i.code == "MISSING_KIND_KEY"),
        "{err:?}"
    );
}

#[test]
fn create_reuse_of_entropy_is_refused() {
    let first = engine(&[])
        .create(&create_req("beliefs/One", "Belief"))
        .unwrap();
    let pages = applied(&[], &first);
    let e = engine(&refs(&pages));
    let err = e.create(&create_req("beliefs/Two", "Belief")).unwrap_err();
    assert_eq!(err.code, code::EXISTS);
}

#[test]
fn create_note_gets_draft_and_no_content_clock() {
    let e = engine(&[]);
    let mut req = create_req("notes/N", "Note");
    req.fields.clear();
    let plan = e.create(&req).unwrap();
    let doc = Document::parse(&plan.writes[0].text);
    assert_eq!(doc.value("status"), Some(&Value::string("draft")));
    assert!(
        doc.value("content-updated").is_none(),
        "only memory kinds carry a content clock"
    );
}

// ---------------------------------------------------------------- patch

fn patch(id: &str, changes: Vec<Change>) -> PatchRequest {
    PatchRequest {
        role: Role::Working,
        id: id.into(),
        changes,
        body: vec![],
        expect: None,
        writer: Some(human()),
        now: Some(T2.into()),
    }
}

fn set(k: &str, v: Value) -> Change {
    Change::Set {
        key: k.into(),
        value: v,
    }
}

#[test]
fn patch_changes_only_the_asked_key_and_stamps() {
    let src = belief("b1", "draft", "# keep this comment\nzzz-unknown: [1, 2]\n");
    let e = engine(&[("b/B1", &src)]);
    let plan = e
        .patch(&patch(
            "b/B1",
            vec![set("confidence", Value::Float("0.9".into()))],
        ))
        .unwrap();
    assert_eq!(plan.requested_keys, ["confidence"]);
    let after = &plan.writes[0].text;
    assert!(after.contains("# keep this comment"));
    assert!(after.contains("zzz-unknown: [1, 2]"));
    let (b, a) = (Document::parse(&src), Document::parse(after));
    assert_eq!(a.value("confidence"), Some(&Value::Float("0.9".into())));
    assert_eq!(
        a.value("content-updated"),
        Some(&Value::string(T2)),
        "a content change moves the content clock"
    );
    assert_eq!(
        a.value("generated"),
        b.value("generated"),
        "the first stamp is kept forever"
    );
    assert_eq!(a.value("contributors").unwrap().as_seq().unwrap().len(), 1);
    assert_eq!(a.body(), b.body());
}

#[test]
fn patch_ignores_client_clocks_and_reports_them() {
    let src = belief("b1", "draft", "");
    let e = engine(&[("b/B1", &src)]);
    let plan = e
        .patch(&patch(
            "b/B1",
            vec![
                set("last-reviewed", Value::string("2000-01-01T00:00:00Z")),
                set("content-updated", Value::string("2000-01-01T00:00:00Z")),
                set("confidence", Value::Float("0.8".into())),
            ],
        ))
        .unwrap();
    assert_eq!(plan.ignored_keys, ["last-reviewed", "content-updated"]);
    assert!(!plan.writes[0].text.contains("2000-01-01"));
    assert!(
        plan.writes[0]
            .text
            .contains(&format!("content-updated: '{T2}'"))
            || plan.writes[0]
                .text
                .contains(&format!("content-updated: {T2}"))
    );
}

#[test]
fn patch_refuses_engine_owned_keys() {
    let src = belief("b1", "draft", "");
    let e = engine(&[("b/B1", &src)]);
    for key in [
        "status",
        "status-reason",
        "disputes",
        "superseded-by",
        "supersedes",
        "valid-to",
        "contributors",
        "generated",
        "page_resource",
    ] {
        let err = e
            .patch(&patch("b/B1", vec![set(key, Value::string("x"))]))
            .unwrap_err();
        assert_eq!(err.code, code::ENGINE_OWNED_KEY, "{key}");
        let err = e
            .patch(&patch("b/B1", vec![Change::Unset { key: key.into() }]))
            .unwrap_err();
        assert_eq!(err.code, code::ENGINE_OWNED_KEY, "unset {key}");
    }
}

#[test]
fn patch_requires_a_writer_under_v3_working() {
    let src = belief("b1", "draft", "");
    let e = engine(&[("b/B1", &src)]);
    let mut r = patch("b/B1", vec![set("confidence", Value::Float("0.8".into()))]);
    r.writer = None;
    assert_eq!(e.patch(&r).unwrap_err().code, code::WRITER_REQUIRED);
    r.writer = Some(human());
    r.now = None;
    assert_eq!(e.patch(&r).unwrap_err().code, code::WRITER_REQUIRED);
}

#[test]
fn a_content_edit_of_an_approved_page_by_the_principal_keeps_it_approved_and_reviews_it() {
    let src = belief("b1", "approved", &format!("last-reviewed: '{T0}'\n"));
    let e = engine(&[("b/B1", &src)]);
    let plan = e
        .patch(&patch(
            "b/B1",
            vec![set("confidence", Value::Float("0.9".into()))],
        ))
        .unwrap();
    let a = Document::parse(&plan.writes[0].text);
    assert_eq!(a.value("status"), Some(&Value::string("approved")));
    assert_eq!(a.value("last-reviewed"), Some(&Value::string(T2)));
    assert_eq!(a.value("content-updated"), Some(&Value::string(T2)));
}

#[test]
fn a_content_edit_of_an_approved_page_through_an_agent_returns_it_to_draft() {
    let src = belief(
        "b1",
        "approved",
        &format!("last-reviewed: '{T0}'\nstatus-reason: earlier\n"),
    );
    let e = engine(&[("b/B1", &src)]);
    let mut r = patch("b/B1", vec![set("confidence", Value::Float("0.9".into()))]);
    r.writer = Some(agent());
    let plan = e.patch(&r).unwrap();
    let a = Document::parse(&plan.writes[0].text);
    assert_eq!(a.value("status"), Some(&Value::string("draft")));
    assert_eq!(
        a.value("last-reviewed"),
        Some(&Value::string(T0)),
        "an agent edit does not review"
    );
    assert!(
        a.value("status-reason").is_none(),
        "a status change with no reason of its own clears it"
    );
}

#[test]
fn an_organisation_principal_may_not_keep_an_approved_page_approved() {
    let src = belief("b1", "approved", "");
    let e = engine(&[("b/B1", &src)]);
    let mut r = patch("b/B1", vec![set("confidence", Value::Float("0.9".into()))]);
    r.writer = Some(ontomark_write::Writer::new(ORG));
    let plan = e.patch(&r).unwrap();
    assert_eq!(
        Document::parse(&plan.writes[0].text).value("status"),
        Some(&Value::string("draft"))
    );
}

#[test]
fn a_non_content_patch_moves_no_content_clock_and_no_review() {
    let src = belief("b1", "approved", "");
    let e = engine(&[("b/B1", &src)]);
    let plan = e
        .patch(&patch(
            "b/B1",
            vec![set("visibility", Value::string("public"))],
        ))
        .unwrap();
    let a = Document::parse(&plan.writes[0].text);
    assert_eq!(a.value("status"), Some(&Value::string("approved")));
    assert_eq!(a.value("content-updated"), Some(&Value::string(T0)));
    assert!(a.value("last-reviewed").is_none());
}

#[test]
fn a_content_edit_of_a_terminal_page_is_refused() {
    for status in ["superseded", "invalidated", "archived"] {
        let extra = match status {
            "superseded" => "superseded-by: ['urn:visionflow:page:zzz']\n",
            "invalidated" => "supersedes:\n  - { reason: withdrawn, at: '2026-11-02T00:00:00Z', by: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a }\n",
            _ => "",
        };
        let src = belief("b1", status, extra);
        let e = engine(&[("b/B1", &src)]);
        let err = e
            .patch(&patch(
                "b/B1",
                vec![set("confidence", Value::Float("0.9".into()))],
            ))
            .unwrap_err();
        assert_eq!(err.code, code::TERMINAL, "{status}");
    }
}

#[test]
fn a_no_op_patch_plans_no_write() {
    let src = belief("b1", "draft", "");
    let e = engine(&[("b/B1", &src)]);
    let plan = e
        .patch(&patch(
            "b/B1",
            vec![set("confidence", Value::Float("0.5".into()))],
        ))
        .unwrap();
    assert!(plan.writes.is_empty() && plan.requested_keys.is_empty());
    assert!(plan.is_noop());
}

#[test]
fn a_stale_base_is_refused() {
    let src = belief("b1", "draft", "");
    let e = engine(&[("b/B1", &src)]);
    let mut r = patch("b/B1", vec![set("confidence", Value::Float("0.9".into()))]);
    r.expect = Some(SourceDigest::of("something else"));
    assert_eq!(e.patch(&r).unwrap_err().code, code::STALE_BASE);
    r.expect = Some(SourceDigest::of(&src));
    assert!(e.patch(&r).is_ok());
}

#[test]
fn a_patch_that_introduces_an_error_is_refused_but_an_existing_error_does_not_block_others() {
    let src = belief("b1", "draft", "");
    let e = engine(&[("b/B1", &src)]);
    let err = e
        .patch(&patch("b/B1", vec![set("confidence", Value::Int(3))]))
        .unwrap_err();
    assert_eq!(err.code, code::WOULD_INTRODUCE_ERRORS);
    assert!(err.issues.iter().any(|i| i.code == "INVALID_KIND_VALUE"));

    // The page already carries an error (confidence out of range): an unrelated
    // patch to it must still go through, because it introduces nothing.
    let broken = belief("b2", "draft", "").replace("confidence: 0.5", "confidence: 3");
    let e = engine(&[("b/B2", &broken)]);
    let plan = e
        .patch(&patch("b/B2", vec![set("tags", Value::strings(["x"]))]))
        .unwrap();
    assert_eq!(plan.requested_keys, ["tags"]);
}

#[test]
fn append_and_unset_and_body_edit() {
    let src = belief("b1", "draft", "tags: [a]\n");
    let e = engine(&[("b/B1", &src)]);
    let doc = Document::parse(&src);
    let body = doc.body_span();
    let mut r = patch(
        "b/B1",
        vec![
            Change::Append {
                key: "tags".into(),
                value: Value::string("b"),
            },
            Change::Unset {
                key: "confidence".into(),
            },
        ],
    );
    r.body = vec![BodyEdit {
        start: body.start,
        end: body.end,
        expected: doc.body().to_owned(),
        replacement: "new body\n".into(),
    }];
    // Dropping confidence breaks the Belief's required key: refused; keep it.
    r.changes.pop();
    let plan = e.patch(&r).unwrap();
    let a = Document::parse(&plan.writes[0].text);
    assert_eq!(a.value("tags"), Some(&Value::strings(["a", "b"])));
    assert_eq!(a.body(), "new body\n");
    assert!(plan.requested_keys.contains(&"tags".to_owned()));
}

#[test]
fn patch_under_v2_is_a_lossless_pass_through_without_stamps() {
    let src = "---\ntype: Note\n# a comment\nstatus: draft\nweird:   {a: 1,   b: [x]}\n---\nBody\n";
    let e = engine_with(VOCAB_V2, false, &[("n/N", src)], &[]);
    let mut r = patch("n/N", vec![set("status", Value::string("stable"))]);
    r.writer = None;
    r.now = None;
    let plan = e.patch(&r).unwrap();
    assert_eq!(
        plan.writes[0].text,
        "---\ntype: Note\n# a comment\nstatus: stable\nweird:   {a: 1,   b: [x]}\n---\nBody\n"
    );
    assert!(!plan.writes[0].text.contains("contributors"));
}

#[test]
fn patch_under_v2_does_not_validate() {
    // 0.9.0's edit validated nothing; the pass-through keeps that contract.
    let src = "---\ntype: Note\nstatus: draft\n---\nBody\n";
    let e = engine_with(VOCAB_V2, false, &[("n/N", src)], &[]);
    let mut r = patch("n/N", vec![set("status", Value::string("nonsense"))]);
    r.writer = None;
    r.now = None;
    assert!(e.patch(&r).is_ok());
}

#[test]
fn patch_of_a_knowledge_page_is_lossless_and_unstamped() {
    let page = "---\ntype: Class\nstatus: draft\nresource: urn:ngm:class:a\ngenerated: { by: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a, at: '2026-11-01T00:00:00Z' }\n---\nBody\n";
    let e = engine_with(VOCAB_V3, true, &[], &[("A", page)]);
    let mut r = patch("A", vec![set("status", Value::string("stable"))]);
    r.role = Role::Knowledge;
    r.writer = None;
    r.now = None;
    let plan = e.patch(&r).unwrap();
    assert!(!plan.writes[0].text.contains("contributors"));
    assert!(plan.writes[0].text.contains("status: stable"));
    // The first stamp is kept on a knowledge page too.
    r.changes = vec![set("generated", Value::string("x"))];
    assert_eq!(e.patch(&r).unwrap_err().code, code::ENGINE_OWNED_KEY);
}

#[test]
fn missing_page_is_not_found() {
    let e = engine(&[]);
    assert_eq!(
        e.patch(&patch("nope", vec![])).unwrap_err().code,
        code::NOT_FOUND
    );
}

/// ADR-VG-012 section 6 ("every change, oldest first") and ADR-VG-013 section 7
/// ("`contributors` is the audit trail"): one entry per WRITE, not per writer.
#[test]
fn contributors_gain_one_entry_per_write_oldest_first() {
    let src = belief("b1", "draft", "");
    let e = engine(&[("b/B1", &src)]);
    let first = e
        .patch(&patch(
            "b/B1",
            vec![set("confidence", Value::Float("0.6".into()))],
        ))
        .unwrap();
    let entries = |text: &str| -> Vec<(String, Option<String>, String)> {
        Document::parse(text)
            .value("contributors")
            .unwrap()
            .as_seq()
            .unwrap()
            .iter()
            .map(|v| {
                let m = v.as_map().unwrap();
                let get = |k: &str| {
                    m.iter()
                        .find(|(key, _)| key.as_str() == k)
                        .and_then(|(_, v)| v.as_str().map(str::to_owned))
                };
                (get("by").unwrap(), get("via"), get("at").unwrap())
            })
            .collect()
    };
    assert_eq!(entries(&first.writes[0].text).len(), 1);
    // The same writer again: the trail grows.
    let pages = applied(&[("b/B1".into(), src.clone())], &first);
    let e2 = engine(&refs(&pages));
    let mut r = patch("b/B1", vec![set("confidence", Value::Float("0.7".into()))]);
    r.now = Some("2026-11-21T00:00:00Z".into());
    let second = e2.patch(&r).unwrap();
    assert_eq!(entries(&second.writes[0].text).len(), 2);
    // The same writer in the same second is still a second write.
    let pages = applied(&refs_owned(&pages), &second);
    let e3 = engine(&refs(&pages));
    let mut r = patch("b/B1", vec![set("confidence", Value::Float("0.75".into()))]);
    r.now = Some("2026-11-21T00:00:00Z".into());
    let third = e3.patch(&r).unwrap();
    assert_eq!(entries(&third.writes[0].text).len(), 3);
    // A different writer (the agent under the same principal) is added too.
    let pages = applied(&refs_owned(&pages), &third);
    let e4 = engine(&refs(&pages));
    let mut r = patch("b/B1", vec![set("confidence", Value::Float("0.8".into()))]);
    r.writer = Some(agent());
    r.now = Some("2026-11-22T00:00:00Z".into());
    let fourth = e4.patch(&r).unwrap();
    let all = entries(&fourth.writes[0].text);
    assert_eq!(all.len(), 4);
    let ats: Vec<&str> = all.iter().map(|e| e.2.as_str()).collect();
    let mut sorted = ats.clone();
    sorted.sort_unstable();
    assert_eq!(ats, sorted, "oldest first");
    assert!(all[3].1.is_some(), "the agent's entry carries its via");
}

fn refs_owned(p: &[(String, String)]) -> Vec<(String, String)> {
    p.to_vec()
}
