//! The ADR-VG-013 lifecycle: every legal move, every illegal one, disputes,
//! approve-then-supersede and its completion.

mod common;

use common::*;
use ontomark_compile::input::page::VaultKind;
use ontomark_parse::{Document, Value};
use ontomark_write::{
    code, CompleteRequest, DisputeInput, Engine, Move, Plan, RecordDisputesRequest, Resolution,
    ReviewEventKind, SupersedeReason, TransitionRequest, Withdrawal,
};

const A_URN: &str = "urn:visionflow:page:a1";
const D_URN: &str = "urn:visionflow:page:d1";

fn tr(id: &str, mv: Move) -> TransitionRequest {
    TransitionRequest {
        id: id.into(),
        expect: None,
        mv,
        writer: human(),
        now: T2.into(),
    }
}

fn val(plan: &Plan, i: usize, key: &str) -> Option<Value> {
    Document::parse(&plan.writes[i].text).value(key).cloned()
}

fn page_in(status: &str) -> String {
    let extra = match status {
        "superseded" => {
            "superseded-by: ['urn:visionflow:page:zzz']\nvalid-to: '2026-11-05T00:00:00Z'\n"
                .to_owned()
        }
        "invalidated" => format!(
            "supersedes:\n  - {{ reason: withdrawn, at: '2026-11-02T00:00:00Z', by: {HUMAN} }}\n"
        ),
        _ => String::new(),
    };
    belief("p1", status, &extra)
}

#[test]
fn every_move_from_every_status_matches_the_table() {
    let moves = |name: &str| -> Move {
        match name {
            "approve" => Move::Approve {
                resolutions: vec![],
            },
            "reject" => Move::Reject {
                reason: Withdrawal::Withdrawn,
                note: None,
            },
            "confirm" => Move::Confirm,
            "send_back" => Move::SendBack {
                disputes: vec![],
                status_reason: Some("looks stale".into()),
            },
            "withdraw" => Move::Withdraw {
                reason: Withdrawal::Corrected,
                note: None,
            },
            "archive" => Move::Archive {
                valid_to: None,
                note: None,
            },
            _ => unreachable!(),
        }
    };
    let legal = [
        ("draft", "approve", "approved"),
        ("draft", "reject", "invalidated"),
        ("approved", "confirm", "approved"),
        ("approved", "send_back", "draft"),
        ("approved", "withdraw", "invalidated"),
        ("approved", "archive", "archived"),
    ];
    for from in ["draft", "approved", "superseded", "invalidated", "archived"] {
        for name in [
            "approve",
            "reject",
            "confirm",
            "send_back",
            "withdraw",
            "archive",
        ] {
            let src = page_in(from);
            let e = engine(&[("b/P", &src)]);
            let result = e.transition(&tr("b/P", moves(name)));
            match legal.iter().find(|(f, m, _)| *f == from && *m == name) {
                Some((_, _, to)) => {
                    let plan = result.unwrap_or_else(|r| panic!("{from} {name}: {r}"));
                    assert_eq!(
                        val(&plan, 0, "status"),
                        Some(Value::string(*to)),
                        "{from} {name}"
                    );
                }
                None => {
                    let err = result.unwrap_err();
                    assert_eq!(err.code, code::ILLEGAL_TRANSITION, "{from} {name}: {err}");
                }
            }
        }
    }
}

#[test]
fn approve_sets_the_review_clock_only_and_clears_the_reason() {
    let src = belief("p1", "draft", "status-reason: sent back\n");
    let e = engine(&[("b/P", &src)]);
    let plan = e
        .transition(&tr(
            "b/P",
            Move::Approve {
                resolutions: vec![],
            },
        ))
        .unwrap();
    assert_eq!(val(&plan, 0, "status"), Some(Value::string("approved")));
    assert_eq!(val(&plan, 0, "last-reviewed"), Some(Value::string(T2)));
    assert_eq!(
        val(&plan, 0, "content-updated"),
        Some(Value::string(T0)),
        "a review never moves the content clock"
    );
    assert!(val(&plan, 0, "status-reason").is_none());
    assert_eq!(plan.writes.len(), 1);
}

#[test]
fn approvals_and_confirmations_carry_no_via_and_need_a_human_principal() {
    let draft = belief("p1", "draft", "");
    let approved = belief("p2", "approved", "");
    let e = engine(&[("b/D", &draft), ("b/A", &approved)]);
    let mut r = tr(
        "b/D",
        Move::Approve {
            resolutions: vec![],
        },
    );
    r.writer = agent();
    assert_eq!(e.transition(&r).unwrap_err().code, code::APPROVER);
    let mut r = tr("b/A", Move::Confirm);
    r.writer = agent();
    assert_eq!(e.transition(&r).unwrap_err().code, code::APPROVER);
    // An organisation principal is not a human approver when the roster says so.
    let mut r = tr(
        "b/D",
        Move::Approve {
            resolutions: vec![],
        },
    );
    r.writer = ontomark_write::Writer::new(ORG);
    assert_eq!(e.transition(&r).unwrap_err().code, code::APPROVER);
    // ... and an unknown key is not either.
    r.writer = ontomark_write::Writer::new(format!("did:nostr:{}", "9".repeat(64)));
    assert_eq!(e.transition(&r).unwrap_err().code, code::APPROVER);
    // The human, directly, may.
    assert!(e
        .transition(&tr(
            "b/D",
            Move::Approve {
                resolutions: vec![]
            }
        ))
        .is_ok());
}

#[test]
fn confirm_moves_only_last_reviewed() {
    let src = belief("p1", "approved", &format!("last-reviewed: '{T0}'\n"));
    let e = engine(&[("b/P", &src)]);
    let plan = e.transition(&tr("b/P", Move::Confirm)).unwrap();
    let (b, a) = (Document::parse(&src), Document::parse(&plan.writes[0].text));
    assert_eq!(a.value("last-reviewed"), Some(&Value::string(T2)));
    for key in b.frontmatter().unwrap().keys() {
        if key != "last-reviewed" {
            assert_eq!(
                a.value(key),
                b.value(key),
                "{key} must not move on a confirmation"
            );
        }
    }
    assert_eq!(a.body(), b.body());
}

#[test]
fn send_back_needs_disputes_or_a_reason_and_clears_nothing_it_should_keep() {
    let src = belief("p1", "approved", "");
    let e = engine(&[("b/P", &src)]);
    let err = e
        .transition(&tr(
            "b/P",
            Move::SendBack {
                disputes: vec![],
                status_reason: None,
            },
        ))
        .unwrap_err();
    assert_eq!(err.code, code::ILLEGAL_TRANSITION);
    let plan = e
        .transition(&tr(
            "b/P",
            Move::SendBack {
                disputes: vec![],
                status_reason: Some("revalidation found drift".into()),
            },
        ))
        .unwrap();
    assert_eq!(val(&plan, 0, "status"), Some(Value::string("draft")));
    assert_eq!(
        val(&plan, 0, "status-reason"),
        Some(Value::string("revalidation found drift"))
    );
    assert!(plan.events.is_empty(), "no disputes, no review item");
}

#[test]
fn send_back_with_disputes_opens_a_references_only_item() {
    let a = belief("a1", "approved", "");
    let p = belief("p1", "approved", "");
    let e = engine(&[("b/A", &a), ("b/P", &p)]);
    let plan = e
        .transition(&tr(
            "b/P",
            Move::SendBack {
                disputes: vec![DisputeInput {
                    page: A_URN.into(),
                    reason: Some("SECRET REASON TEXT".into()),
                }],
                status_reason: None,
            },
        ))
        .unwrap();
    assert_eq!(plan.events.len(), 1);
    let ev = &plan.events[0];
    assert_eq!(ev.kind, ReviewEventKind::Opened);
    assert_eq!(ev.disputes.len(), 1);
    assert_eq!(ev.disputes[0].page, A_URN);
    let json = serde_json::to_string(ev).unwrap();
    assert!(
        !json.contains("SECRET REASON TEXT") && !json.contains("body of"),
        "{json}"
    );
    // The reason lives on the page, with the page's visibility.
    assert!(plan.writes[0].text.contains("SECRET REASON TEXT"));
}

#[test]
fn withdraw_and_reject_write_a_withdrawal_entry_that_closes_recorded_time() {
    for (status, mv_name) in [("approved", "withdraw"), ("draft", "reject")] {
        let src = belief("p1", status, "");
        let e = engine(&[("b/P", &src)]);
        let mv = if mv_name == "withdraw" {
            Move::Withdraw {
                reason: Withdrawal::Withdrawn,
                note: Some("recorded in error".into()),
            }
        } else {
            Move::Reject {
                reason: Withdrawal::Corrected,
                note: None,
            }
        };
        let plan = e.transition(&tr("b/P", mv)).unwrap();
        assert_eq!(val(&plan, 0, "status"), Some(Value::string("invalidated")));
        let entries = val(&plan, 0, "supersedes").unwrap();
        let entry = entries.as_seq().unwrap()[0].as_map().unwrap().clone();
        assert!(
            !entry.contains_key("resource"),
            "a withdrawal names no page"
        );
        assert!(["withdrawn", "corrected"].contains(&entry["reason"].as_str().unwrap()));
        assert_eq!(entry["at"], Value::string(T2));
        assert_eq!(entry["by"], Value::string(HUMAN));
        // and no valid-to: both reasons close recorded time, not valid time.
        assert!(val(&plan, 0, "valid-to").is_none());
    }
}

#[test]
fn an_agent_withdrawal_records_the_instrument() {
    let src = belief("p1", "approved", "");
    let e = engine(&[("b/P", &src)]);
    let mut r = tr(
        "b/P",
        Move::Withdraw {
            reason: Withdrawal::Withdrawn,
            note: None,
        },
    );
    r.writer = agent();
    let plan = e.transition(&r).unwrap();
    let entries = val(&plan, 0, "supersedes").unwrap();
    assert_eq!(
        entries.as_seq().unwrap()[0].as_map().unwrap()["via"],
        Value::string(AGENT)
    );
}

#[test]
fn archive_may_close_valid_time() {
    let src = belief("p1", "approved", "");
    let e = engine(&[("b/P", &src)]);
    let plan = e
        .transition(&tr(
            "b/P",
            Move::Archive {
                valid_to: Some("2026-11-15T00:00:00Z".into()),
                note: Some("no longer relevant".into()),
            },
        ))
        .unwrap();
    assert_eq!(val(&plan, 0, "status"), Some(Value::string("archived")));
    assert_eq!(
        val(&plan, 0, "valid-to"),
        Some(Value::string("2026-11-15T00:00:00Z"))
    );
    let bad = e.transition(&tr(
        "b/P",
        Move::Archive {
            valid_to: Some("yesterday".into()),
            note: None,
        },
    ));
    assert_eq!(bad.unwrap_err().code, code::INVALID_REQUEST);
}

#[test]
fn only_memory_kinds_have_a_lifecycle_to_transition() {
    let note = "---\ntype: Note\nstatus: draft\npage_resource: urn:visionflow:page:n1\nvisibility: shared\ngenerated: { by: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a, at: '2026-11-01T00:00:00Z' }\n---\nx\n";
    let e = engine(&[("n/N", note)]);
    assert_eq!(
        e.transition(&tr("n/N", Move::Confirm)).unwrap_err().code,
        code::ILLEGAL_TRANSITION
    );
}

#[test]
fn transitions_refuse_under_vocabulary_2() {
    let e = engine_with(VOCAB_V2, true, &[], &[]);
    assert_eq!(
        e.transition(&tr("x", Move::Confirm)).unwrap_err().code,
        code::VOCABULARY_V2
    );
}

// ------------------------------------------------------------- disputes

fn disputed_pair(a_extra: &str, d_detected: &str) -> (String, String) {
    let a = belief("a1", "approved", a_extra);
    let d = belief(
        "d1",
        "draft",
        &format!("disputes:\n  - {{ page: {A_URN}, detected: '{d_detected}', reason: why }}\n"),
    );
    (a, d)
}

fn res(reason: SupersedeReason) -> Move {
    Move::Approve {
        resolutions: vec![Resolution {
            page: A_URN.into(),
            reason,
            valid_to: None,
        }],
    }
}

#[test]
fn approving_a_disputed_draft_supersedes_what_it_disputed_in_one_plan() {
    let (a, d) = disputed_pair("", T1);
    let e = engine(&[("b/A", &a), ("b/D", &d)]);
    let plan = e
        .transition(&tr("b/D", res(SupersedeReason::WorldChanged)))
        .unwrap();
    // Draft first, then the replaced page: an interruption leaves both approved.
    assert_eq!(plan.writes.len(), 2);
    assert_eq!(plan.writes[0].id, "b/D");
    assert_eq!(plan.writes[1].id, "b/A");
    // The draft: approved, reviewed, disputes gone, one supersedes entry.
    assert_eq!(val(&plan, 0, "status"), Some(Value::string("approved")));
    assert_eq!(val(&plan, 0, "last-reviewed"), Some(Value::string(T2)));
    assert!(val(&plan, 0, "disputes").is_none());
    let entries = val(&plan, 0, "supersedes").unwrap();
    let e0 = entries.as_seq().unwrap()[0].as_map().unwrap().clone();
    assert_eq!(e0["resource"], Value::string(A_URN));
    assert_eq!(e0["reason"], Value::string("world-changed"));
    assert_eq!(e0["at"], Value::string(T2));
    assert_eq!(e0["by"], Value::string(HUMAN));
    assert!(!e0.contains_key("via"), "approvals carry no via");
    // The old page: superseded, superseded-by names the draft, valid time closed (B1).
    assert_eq!(val(&plan, 1, "status"), Some(Value::string("superseded")));
    assert_eq!(
        val(&plan, 1, "superseded-by"),
        Some(Value::strings([D_URN]))
    );
    assert_eq!(val(&plan, 1, "valid-to"), Some(Value::string(T2)));
    // The review item closes.
    assert_eq!(plan.events.len(), 1);
    assert_eq!(plan.events[0].kind, ReviewEventKind::Closed);
    // The result is a valid vault: no pending state, and it re-validates.
    let pages = applied(
        &[("b/A".into(), a.clone()), ("b/D".into(), d.clone())],
        &plan,
    );
    let after = engine(&refs(&pages));
    assert!(after.pending_supersessions().is_empty());
}

#[test]
fn a_corrected_supersession_closes_recorded_time_and_sets_no_valid_to() {
    let (a, d) = disputed_pair("", T1);
    let e = engine(&[("b/A", &a), ("b/D", &d)]);
    let plan = e
        .transition(&tr("b/D", res(SupersedeReason::Corrected)))
        .unwrap();
    assert_eq!(val(&plan, 1, "status"), Some(Value::string("superseded")));
    assert!(val(&plan, 1, "valid-to").is_none());
    let entries = val(&plan, 0, "supersedes").unwrap();
    assert_eq!(
        entries.as_seq().unwrap()[0].as_map().unwrap()["reason"],
        Value::string("corrected")
    );
}

#[test]
fn a_world_changed_resolution_may_name_when_it_stopped_being_true() {
    let (a, d) = disputed_pair("", T1);
    let e = engine(&[("b/A", &a), ("b/D", &d)]);
    let mv = Move::Approve {
        resolutions: vec![Resolution {
            page: A_URN.into(),
            reason: SupersedeReason::WorldChanged,
            valid_to: Some("2026-11-12T00:00:00Z".into()),
        }],
    };
    let plan = e.transition(&tr("b/D", mv)).unwrap();
    assert_eq!(
        val(&plan, 1, "valid-to"),
        Some(Value::string("2026-11-12T00:00:00Z"))
    );
}

#[test]
fn approval_refuses_a_disputed_page_that_changed_after_detection() {
    // A's content moved at T2-ish, after the dispute was detected at T1.
    let (a, d) = disputed_pair("", T1);
    let a = a.replace(
        &format!("content-updated: '{T0}'"),
        "content-updated: '2026-11-15T00:00:00Z'",
    );
    let e = engine(&[("b/A", &a), ("b/D", &d)]);
    let err = e
        .transition(&tr("b/D", res(SupersedeReason::WorldChanged)))
        .unwrap_err();
    assert_eq!(err.code, code::STALE_DISPUTE);
}

#[test]
fn approval_follows_a_superseded_target_to_its_successor_by_refusing_with_it() {
    let (_, d) = disputed_pair("", T1);
    let a = belief(
        "a1",
        "superseded",
        "superseded-by: ['urn:visionflow:page:n9']\nvalid-to: '2026-11-05T00:00:00Z'\n",
    );
    let n = belief("n9", "approved", "");
    let e = engine(&[("b/A", &a), ("b/D", &d), ("b/N", &n)]);
    let err = e
        .transition(&tr("b/D", res(SupersedeReason::WorldChanged)))
        .unwrap_err();
    assert_eq!(err.code, code::DISPUTE_TARGET_MOVED);
    assert!(err.message.contains("urn:visionflow:page:n9"), "{err}");
    // Retargeting is a record-disputes write that names the successor.
    let plan = e
        .record_disputes(&RecordDisputesRequest {
            id: "b/D".into(),
            expect: None,
            disputes: vec![DisputeInput {
                page: "urn:visionflow:page:n9".into(),
                reason: None,
            }],
            writer: human(),
            now: T2.into(),
        })
        .unwrap();
    assert_eq!(plan.events[0].kind, ReviewEventKind::Rewritten);
    assert!(plan.writes[0].text.contains("urn:visionflow:page:n9"));
}

#[test]
fn approval_needs_a_reason_for_every_dispute_and_no_extras() {
    let (a, d) = disputed_pair("", T1);
    let e = engine(&[("b/A", &a), ("b/D", &d)]);
    let err = e
        .transition(&tr(
            "b/D",
            Move::Approve {
                resolutions: vec![],
            },
        ))
        .unwrap_err();
    assert_eq!(err.code, code::MISSING_RESOLUTION);
    let extra = Move::Approve {
        resolutions: vec![
            Resolution {
                page: A_URN.into(),
                reason: SupersedeReason::Corrected,
                valid_to: None,
            },
            Resolution {
                page: "urn:visionflow:page:other".into(),
                reason: SupersedeReason::Corrected,
                valid_to: None,
            },
        ],
    };
    assert_eq!(
        e.transition(&tr("b/D", extra)).unwrap_err().code,
        code::INVALID_REQUEST
    );
    // Resolutions with nothing disputed are refused too.
    let plain = belief("p1", "draft", "");
    let e = engine(&[("b/P", &plain)]);
    assert_eq!(
        e.transition(&tr("b/P", res(SupersedeReason::Corrected)))
            .unwrap_err()
            .code,
        code::INVALID_REQUEST
    );
}

#[test]
fn rejecting_a_disputed_draft_leaves_the_disputed_page_alone() {
    let (a, d) = disputed_pair("", T1);
    let e = engine(&[("b/A", &a), ("b/D", &d)]);
    let plan = e
        .transition(&tr(
            "b/D",
            Move::Reject {
                reason: Withdrawal::Withdrawn,
                note: None,
            },
        ))
        .unwrap();
    assert_eq!(plan.writes.len(), 1);
    assert!(
        val(&plan, 0, "disputes").is_none(),
        "leaving draft clears disputes"
    );
    assert_eq!(plan.events[0].kind, ReviewEventKind::Closed);
}

#[test]
fn record_disputes_rules() {
    let a = belief("a1", "approved", "");
    let a_private =
        belief("a2", "approved", "").replace("visibility: shared", "visibility: public");
    let ep = "---\ntype: Episode\nstatus: approved\npage_resource: urn:visionflow:page:e1\nvisibility: shared\ngenerated: { by: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a, at: '2026-11-01T00:00:00Z' }\nepisode-url: 'https://x.example/e'\nepisode-date: 2026-05-01\nsources:\n  - { id: o, resource: 'https://x.example/e', rel: origin }\n---\nx\n";
    let d = belief("d1", "draft", "");
    let e = engine(&[("b/A", &a), ("b/A2", &a_private), ("b/D", &d), ("e/E", ep)]);
    let rec = |id: &str, targets: &[&str]| RecordDisputesRequest {
        id: id.into(),
        expect: None,
        disputes: targets
            .iter()
            .map(|t| DisputeInput {
                page: (*t).into(),
                reason: None,
            })
            .collect(),
        writer: human(),
        now: T2.into(),
    };
    // Opens an item; detected is the engine's clock.
    let plan = e.record_disputes(&rec("b/D", &[A_URN])).unwrap();
    assert_eq!(plan.events[0].kind, ReviewEventKind::Opened);
    assert_eq!(plan.events[0].disputes[0].detected, T2);
    // Each target at most once.
    assert_eq!(
        e.record_disputes(&rec("b/D", &[A_URN, "b/A"]))
            .unwrap_err()
            .code,
        code::DISPUTE_TARGET
    );
    // Same visibility only.
    assert_eq!(
        e.record_disputes(&rec("b/D", &["urn:visionflow:page:a2"]))
            .unwrap_err()
            .code,
        code::DISPUTE_TARGET
    );
    // Not itself; not an Episode; not something that does not exist.
    assert_eq!(
        e.record_disputes(&rec("b/D", &[D_URN])).unwrap_err().code,
        code::DISPUTE_TARGET
    );
    assert_eq!(
        e.record_disputes(&rec("b/D", &["urn:visionflow:page:e1"]))
            .unwrap_err()
            .code,
        code::DISPUTE_TARGET
    );
    assert_eq!(
        e.record_disputes(&rec("b/D", &["urn:visionflow:page:nope"]))
            .unwrap_err()
            .code,
        code::DISPUTE_TARGET
    );
    // Only a draft holds disputes.
    assert_eq!(
        e.record_disputes(&rec("b/A", &["urn:visionflow:page:a2"]))
            .unwrap_err()
            .code,
        code::ILLEGAL_TRANSITION
    );
    // An Episode neither records nor receives them.
    let draft_ep = ep.replace("status: approved", "status: draft");
    let e2 = engine(&[("b/A", &a), ("e/E", &draft_ep)]);
    assert_eq!(
        e2.record_disputes(&rec("e/E", &[A_URN])).unwrap_err().code,
        code::DISPUTE_TARGET
    );
    // An unchanged list is a no-op; an emptied one closes the item.
    let pages = applied(
        &[("b/A".into(), a.clone()), ("b/D".into(), d.clone())],
        &plan,
    );
    let e3 = engine(&refs(&pages));
    let mut again = rec("b/D", &[A_URN]);
    again.now = "2026-11-21T00:00:00Z".into();
    assert!(
        e3.record_disputes(&again).unwrap().writes.is_empty(),
        "an existing entry keeps its detected time"
    );
    let cleared = e3.record_disputes(&rec("b/D", &[])).unwrap();
    assert_eq!(cleared.events[0].kind, ReviewEventKind::Closed);
    assert!(Document::parse(&cleared.writes[0].text)
        .value("disputes")
        .is_none());
}

// ------------------------------------------- interruption and completion

#[test]
fn an_interrupted_supersession_blocks_other_writes_and_the_next_write_completes_it() {
    let (a, d) = disputed_pair("", T1);
    let e = engine(&[("b/A", &a), ("b/D", &d)]);
    let plan = e
        .transition(&tr("b/D", res(SupersedeReason::WorldChanged)))
        .unwrap();
    // Crash after the first write: the draft is approved, A still approved.
    let crashed = vec![
        ("b/A".to_owned(), a.clone()),
        ("b/D".to_owned(), plan.writes[0].text.clone()),
    ];
    let e = engine(&refs(&crashed));
    let pending = e.pending_supersessions();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        (pending[0].replacing.as_str(), pending[0].replaced.as_str()),
        ("b/D", "b/A")
    );
    // Any other write to either page is refused until it is completed.
    let patch = ontomark_write::PatchRequest {
        role: ontomark_write::Role::Working,
        id: "b/A".into(),
        changes: vec![ontomark_write::Change::Set {
            key: "tags".into(),
            value: Value::strings(["x"]),
        }],
        body: vec![],
        expect: None,
        writer: Some(human()),
        now: Some(T2.into()),
    };
    assert_eq!(
        e.patch(&patch).unwrap_err().code,
        code::PENDING_SUPERSESSION
    );
    assert_eq!(
        e.transition(&tr("b/A", Move::Confirm)).unwrap_err().code,
        code::PENDING_SUPERSESSION
    );
    // Completing it from either side retires the old page.
    for id in ["b/D", "b/A"] {
        let done = e
            .complete_supersession(&CompleteRequest {
                id: id.into(),
                writer: human(),
                now: "2026-11-21T00:00:00Z".into(),
            })
            .unwrap();
        assert_eq!(done.writes.len(), 1);
        assert_eq!(done.writes[0].id, "b/A");
        assert_eq!(val(&done, 0, "status"), Some(Value::string("superseded")));
        assert_eq!(
            val(&done, 0, "superseded-by"),
            Some(Value::strings([D_URN]))
        );
        // valid time closes at the approval's `at`, not at completion.
        assert_eq!(val(&done, 0, "valid-to"), Some(Value::string(T2)));
        let after = applied(&crashed, &done);
        assert!(engine(&refs(&after)).pending_supersessions().is_empty());
    }
    // Nothing pending: nothing to complete.
    let clean = engine(&[("b/A", &a)]);
    assert_eq!(
        clean
            .complete_supersession(&CompleteRequest {
                id: "b/A".into(),
                writer: human(),
                now: T2.into()
            })
            .unwrap_err()
            .code,
        code::NOT_FOUND
    );
}

// ------------------------------------------------- atomic across files

fn two_page_setup() -> (Engine, MemStoreBox, Plan) {
    let (a, d) = disputed_pair("", T1);
    let e = engine(&[("b/A", &a), ("b/D", &d)]);
    let store = store_of(&[("b/A", &a), ("b/D", &d)]);
    let plan = e
        .transition(&tr("b/D", res(SupersedeReason::WorldChanged)))
        .unwrap();
    (e, MemStoreBox(store), plan)
}

struct MemStoreBox(ontomark_write::MemStore);

#[test]
fn supersede_commits_both_pages() {
    let (_, mut store, plan) = two_page_setup();
    let report = plan.commit(&mut store.0).unwrap();
    assert_eq!(report.written.len(), 2);
    assert!(store
        .0
        .text(VaultKind::Working, "b/D.md")
        .unwrap()
        .contains("status: approved"));
    assert!(store
        .0
        .text(VaultKind::Working, "b/A.md")
        .unwrap()
        .contains("status: superseded"));
}

#[test]
fn a_failure_on_the_second_write_leaves_neither_page_written() {
    let (_, mut store, plan) = two_page_setup();
    let before_d = store.0.text(VaultKind::Working, "b/D.md").unwrap();
    let before_a = store.0.text(VaultKind::Working, "b/A.md").unwrap();
    store.0.fail_write_number = Some(1);
    let err = plan.commit(&mut store.0).unwrap_err();
    match err {
        ontomark_write::CommitError::Write {
            failed,
            restored,
            unrestored,
            ..
        } => {
            assert_eq!(failed.to_str(), Some("b/A.md"));
            assert_eq!(restored.len(), 1, "the first write was put back");
            assert!(unrestored.is_empty());
        }
        other => panic!("{other}"),
    }
    // The draft the first write changed is byte-identical to before; nothing else exists.
    assert_eq!(
        store.0.text(VaultKind::Working, "b/D.md").unwrap(),
        before_d
    );
    assert_eq!(
        store.0.text(VaultKind::Working, "b/A.md").unwrap(),
        before_a
    );
    assert_eq!(store.0.len(), 2);
}

#[test]
fn a_failed_rollback_is_reported_not_hidden() {
    let (_, mut store, plan) = two_page_setup();
    store.0.fail_write_number = Some(1);
    store.0.fail_restores = true;
    // fail_restores only bites once the injected failure has fired.
    let first = plan.commit(&mut store.0);
    match first.unwrap_err() {
        ontomark_write::CommitError::Write {
            unrestored,
            restored,
            ..
        } => {
            assert_eq!(
                (unrestored.len(), restored.len()),
                (1, 0),
                "the draft could not be put back and the error says so"
            );
        }
        other => panic!("{other}"),
    }
}

#[test]
fn a_stale_target_refuses_the_whole_plan_before_writing_anything() {
    let (_, mut store, plan) = two_page_setup();
    store
        .0
        .insert(VaultKind::Working, "b/A.md", "someone else edited this");
    let before_d = store.0.text(VaultKind::Working, "b/D.md").unwrap();
    assert!(matches!(
        plan.commit(&mut store.0),
        Err(ontomark_write::CommitError::Stale(_))
    ));
    assert_eq!(
        store.0.text(VaultKind::Working, "b/D.md").unwrap(),
        before_d
    );
}

#[test]
fn a_creation_never_overwrites_at_commit_time() {
    let e = engine(&[]);
    let plan = e
        .create(&ontomark_write::CreateRequest {
            id: "notes/N".into(),
            page_type: "Note".into(),
            fields: vec![],
            body: "x\n".into(),
            visibility: None,
            writer: human(),
            now: T1.into(),
            entropy: [3; 32],
        })
        .unwrap();
    let mut store = ontomark_write::MemStore::default();
    store.insert(VaultKind::Working, "notes/N.md", "already here");
    assert!(matches!(
        plan.commit(&mut store),
        Err(ontomark_write::CommitError::Exists(_))
    ));
    assert_eq!(
        store.text(VaultKind::Working, "notes/N.md").unwrap(),
        "already here"
    );
}

// ----------------------------------------------------------------- adopt

#[test]
fn adopt_mints_identity_for_a_journal_and_leaves_complete_pages_alone() {
    let journal = "---\ntags: [daily]\n---\nToday I wrote tests.\n";
    let complete = belief("c1", "draft", "");
    let note = "---\ntype: Note\nstatus: draft\n---\nx\n";
    let e = engine(&[
        ("journals/2026-11-20", journal),
        ("b/C", &complete),
        ("n/N", note),
    ]);
    let req = |id: &str| ontomark_write::AdoptRequest {
        id: id.into(),
        expect: None,
        writer: human(),
        now: T2.into(),
        entropy: [5; 32],
    };
    let plan = e.adopt(&req("journals/2026-11-20")).unwrap();
    let a = Document::parse(&plan.writes[0].text);
    // A journal's identity is its own date: the same urn whatever the entropy
    // or the day it is adopted (what m0002 mints, via `derived_page_resource`).
    assert_eq!(
        a.value("page_resource"),
        Some(&Value::string(
            ontomark_compile::check::v3::state::derived_page_resource("journals/2026-11-20")
        ))
    );
    let mut other = req("journals/2026-11-20");
    other.entropy = [9; 32];
    other.now = "2026-12-31T00:00:00Z".into();
    assert_eq!(
        Document::parse(&e.adopt(&other).unwrap().writes[0].text).value("page_resource"),
        a.value("page_resource")
    );
    // Any other page is minted from the caller's entropy.
    let n = e.adopt(&req("n/N")).unwrap();
    assert_eq!(
        Document::parse(&n.writes[0].text).value("page_resource"),
        Some(&Value::string(format!(
            "urn:visionflow:page:{}",
            "05".repeat(32)
        )))
    );
    assert_eq!(a.value("visibility"), Some(&Value::string("shared")));
    assert_eq!(
        a.value("generated").unwrap().as_map().unwrap()["at"],
        Value::string(T2)
    );
    assert_eq!(
        a.value("tags"),
        Some(&Value::strings(["daily"])),
        "existing keys stay"
    );
    assert_eq!(a.body(), "Today I wrote tests.\n");
    assert!(
        e.adopt(&req("b/C")).unwrap().writes.is_empty(),
        "nothing missing, nothing written"
    );
    assert_eq!(e.adopt(&req("nope")).unwrap_err().code, code::NOT_FOUND);
}

/// B1: an instrument's own key never approves, even when the roster files it
/// under the operator; the refusal writes nothing, so the approval is not
/// re-stamped as the operator's.
#[test]
fn a_swarm_key_filed_under_the_operator_cannot_approve_or_confirm() {
    let draft = belief("p1", "draft", "");
    let approved = belief("p2", "approved", "");
    let e = engine(&[("b/D", &draft), ("b/A", &approved)]);
    let mut r = tr(
        "b/D",
        Move::Approve {
            resolutions: vec![],
        },
    );
    r.writer = ontomark_write::Writer::new(SWARM);
    assert_eq!(e.transition(&r).unwrap_err().code, code::APPROVER);
    let mut r = tr("b/A", Move::Confirm);
    r.writer = ontomark_write::Writer::new(SWARM);
    assert_eq!(e.transition(&r).unwrap_err().code, code::APPROVER);
    // Not even with a `via` that would be stripped: still the swarm's key.
    let mut r = tr(
        "b/D",
        Move::Approve {
            resolutions: vec![],
        },
    );
    r.writer = ontomark_write::Writer::new(SWARM).via(AGENT);
    assert_eq!(e.transition(&r).unwrap_err().code, code::APPROVER);
    // The operator's own `also` key still may.
    let mut r = tr(
        "b/D",
        Move::Approve {
            resolutions: vec![],
        },
    );
    r.writer = ontomark_write::Writer::new(HUMAN_ALSO);
    assert!(e.transition(&r).is_ok());
}
