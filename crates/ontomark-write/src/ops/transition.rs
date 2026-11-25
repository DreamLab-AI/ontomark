//! Status transitions of a durable memory page (ADR-VG-013 sections 1 to 5).
//!
//! | move | from | to | notes |
//! |---|---|---|---|
//! | [`Move::Approve`] | draft | approved | no `via`; sets `last-reviewed`; a disputed draft supersedes what it disputed, in one ordered plan |
//! | [`Move::Reject`] | draft | invalidated | withdrawal entry |
//! | [`Move::Confirm`] | approved | approved | no `via`; only `last-reviewed` moves |
//! | [`Move::SendBack`] | approved | draft | needs disputes, a `status-reason`, or both |
//! | [`Move::Withdraw`] | approved | invalidated | withdrawal entry |
//! | [`Move::Archive`] | approved | archived | optional `valid-to` |
//!
//! `superseded`, `invalidated` and `archived` are terminal. `approved` to
//! `superseded` happens only inside an approval that replaces the page.

use ontomark_compile::input::page::VaultKind;
use ontomark_parse::{Document, SourceDigest, Value};
use serde::{Deserialize, Serialize};

use crate::actor::Writer;
use crate::edit::{map, Edit};
use crate::engine::{Engine, Replacement};
use crate::error::{code, Refusal, Result};
use crate::ops::disputes::{event, held, DisputeInput};
use crate::ops::supersede::Resolution;
use crate::ops::{append_contributor, check_base, content_clock, is_memory, status_of, ts};
use crate::plan::{Plan, ReviewEvent, ReviewEventKind};

/// Why a page is withdrawn. Both reasons close recorded time (ruling B1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Withdrawal {
    /// It was never true.
    Corrected,
    /// It should not have been recorded.
    Withdrawn,
}

impl Withdrawal {
    fn as_str(self) -> &'static str {
        match self {
            Self::Corrected => "corrected",
            Self::Withdrawn => "withdrawn",
        }
    }
}

/// The move a [`TransitionRequest`] asks for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "move", rename_all = "snake_case")]
pub enum Move {
    /// draft to approved. When the draft holds disputes, every disputed page
    /// needs a [`Resolution`], and the pages are superseded in the same
    /// operation.
    Approve {
        /// One entry per dispute.
        #[serde(default)]
        resolutions: Vec<Resolution>,
    },
    /// draft to invalidated.
    Reject {
        /// Why.
        reason: Withdrawal,
        /// Optional `status-reason`.
        #[serde(default)]
        note: Option<String>,
    },
    /// approved to approved: a review that finds the page still good.
    Confirm,
    /// approved to draft: a revalidation or the principal found a problem.
    SendBack {
        /// Approved pages this one now contradicts.
        #[serde(default)]
        disputes: Vec<DisputeInput>,
        /// Why it left approved.
        #[serde(default)]
        status_reason: Option<String>,
    },
    /// approved to invalidated.
    Withdraw {
        /// Why.
        reason: Withdrawal,
        /// Optional `status-reason`.
        #[serde(default)]
        note: Option<String>,
    },
    /// approved to archived.
    Archive {
        /// When the page stopped being relevant, if known.
        #[serde(default)]
        valid_to: Option<String>,
        /// Optional `status-reason`.
        #[serde(default)]
        note: Option<String>,
    },
}

impl Move {
    fn name(&self) -> &'static str {
        match self {
            Self::Approve { .. } => "approve",
            Self::Reject { .. } => "reject",
            Self::Confirm => "confirm",
            Self::SendBack { .. } => "send_back",
            Self::Withdraw { .. } => "withdraw",
            Self::Archive { .. } => "archive",
        }
    }

    fn needs_status(&self) -> &'static str {
        match self {
            Self::Approve { .. } | Self::Reject { .. } => "draft",
            _ => "approved",
        }
    }
}

/// A request to move one memory page through its lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionRequest {
    /// The page id.
    pub id: String,
    /// The digest the request was planned against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<SourceDigest>,
    /// The move.
    #[serde(flatten)]
    pub mv: Move,
    /// Who writes.
    pub writer: Writer,
    /// When. The engine stamps the clocks from this; client clock values are
    /// not accepted anywhere.
    pub now: String,
}

fn entry(reason: &str, at: &str, actor: &crate::actor::Actor) -> Value {
    let mut pairs = vec![
        ("reason", Value::string(reason)),
        ("at", Value::string(at)),
        ("by", Value::string(actor.did.clone())),
    ];
    if let Some(via) = &actor.via {
        pairs.push(("via", Value::string(via.clone())));
    }
    Value::Map(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

impl Engine {
    /// Move a durable memory page through ADR-VG-013's lifecycle.
    ///
    /// # Errors
    /// [`code::ILLEGAL_TRANSITION`] for a move outside the table (including
    /// any move from a terminal status); [`code::APPROVER`] when an approval
    /// or confirmation carries `via` or the writer is not a human principal of
    /// the roster; the dispute refusals ([`code::STALE_DISPUTE`],
    /// [`code::DISPUTE_TARGET_MOVED`], ...); and
    /// [`code::WOULD_INTRODUCE_ERRORS`]. Nothing is written on a refusal.
    #[allow(clippy::too_many_lines)] // One arm per move; splitting hides the table.
    pub fn transition(&self, req: &TransitionRequest) -> Result<Plan> {
        self.require_v3("a status transition")?;
        ts("now", &req.now)?;
        let page = self.find(VaultKind::Working, &req.id).ok_or_else(|| {
            Refusal::new(code::NOT_FOUND, format!("no working page `{}`", req.id))
        })?;
        check_base(page, req.expect.as_ref())?;
        if !is_memory(self, VaultKind::Working, page) {
            return Err(Refusal::new(
                code::ILLEGAL_TRANSITION,
                format!(
                    "`{}` is not a durable memory page; its status changes through patch",
                    page.id
                ),
            ));
        }
        self.refuse_pending(&page.id)?;
        let status = status_of(page).unwrap_or_default();
        let mv = &req.mv;
        if status != mv.needs_status() {
            return Err(Refusal::new(
                code::ILLEGAL_TRANSITION,
                format!(
                    "{} needs a {} page; `{}` is {status}{}",
                    mv.name(),
                    mv.needs_status(),
                    page.id,
                    if matches!(status.as_str(), "superseded" | "invalidated" | "archived") {
                        " (terminal: a claim that comes back is a new page)"
                    } else {
                        ""
                    }
                ),
            ));
        }
        let actor = self.actor(&req.writer)?;
        if matches!(mv, Move::Approve { .. } | Move::Confirm)
            && !actor.may_approve(self.roster_known())
        {
            return Err(Refusal::new(
                code::APPROVER,
                if actor.via.is_some() {
                    "an approval or confirmation carries no `via`: the principal acts directly (ADR-VG-013 open item 2)"
                } else {
                    "an approval or confirmation needs a principal the roster lists as human"
                },
            ));
        }
        let urn = page.frontmatter.text("page_resource");
        let doc = Document::parse(&page.source);
        let had_disputes = !held(&doc).is_empty();
        let mut edit = Edit::new(&page.source);
        let mut others: Vec<(
            ontomark_compile::input::page::Page,
            crate::plan::PlannedWrite,
        )> = Vec::new();
        let mut events: Vec<ReviewEvent> = Vec::new();
        match mv {
            Move::Approve { resolutions } => {
                edit.set("status", Value::string("approved"));
                edit.set("last-reviewed", Value::string(req.now.clone()));
                edit.remove("status-reason");
                if had_disputes {
                    let draft_urn = urn.clone().ok_or_else(|| {
                        Refusal::new(
                            code::INVALID_REQUEST,
                            "the draft has no page_resource; adopt it first",
                        )
                    })?;
                    let mut entries = match doc.value("supersedes") {
                        Some(Value::Seq(items)) => items.clone(),
                        _ => Vec::new(),
                    };
                    let mut used = vec![false; resolutions.len()];
                    for d in held(&doc) {
                        let target = self.dispute_target(page, &d.reference)?;
                        let detected = d.detected.as_deref().ok_or_else(|| {
                            Refusal::new(
                                code::STALE_DISPUTE,
                                format!("the dispute on `{}` has no `detected` time", target.id),
                            )
                        })?;
                        let detected_s = ts("detected", detected)?;
                        if let Some(clock) = content_clock(target) {
                            if ts("content-updated", &clock)? > detected_s {
                                return Err(Refusal::new(
                                    code::STALE_DISPUTE,
                                    format!(
                                        "`{}` changed at {clock}, after the dispute was detected at {detected}; compare again",
                                        target.id
                                    ),
                                ));
                            }
                        }
                        let target_urn =
                            target.frontmatter.text("page_resource").unwrap_or_default();
                        let at = resolutions.iter().position(|r| {
                            self.find_reference(&r.page).map(|p| p.id.as_str())
                                == Some(target.id.as_str())
                        });
                        let Some(at) = at else {
                            return Err(Refusal::new(
                                code::MISSING_RESOLUTION,
                                format!(
                                    "say why `{}` is replaced: world-changed or corrected",
                                    target.id
                                ),
                            ));
                        };
                        used[at] = true;
                        let res = &resolutions[at];
                        entries.push(map([
                            ("resource", Value::string(target_urn)),
                            ("reason", Value::string(res.reason.as_str())),
                            ("at", Value::string(req.now.clone())),
                            ("by", Value::string(actor.did.clone())),
                        ]));
                        let (new_page, write) = self.retire(
                            target,
                            &draft_urn,
                            res.reason,
                            res.valid_to.as_deref(),
                            &actor,
                            &req.now,
                        )?;
                        others.push((new_page, write));
                    }
                    if let Some(i) = used.iter().position(|u| !u) {
                        return Err(Refusal::new(
                            code::INVALID_REQUEST,
                            format!(
                                "resolution for `{}` matches no dispute",
                                resolutions[i].page
                            ),
                        ));
                    }
                    edit.set("supersedes", Value::Seq(entries));
                    edit.remove("disputes");
                    events.push(event(ReviewEventKind::Closed, &draft_urn, &[], &req.now));
                } else if !resolutions.is_empty() {
                    return Err(Refusal::new(
                        code::INVALID_REQUEST,
                        "resolutions were given but the draft holds no disputes",
                    ));
                }
            }
            Move::Reject { reason, note } | Move::Withdraw { reason, note } => {
                edit.set("status", Value::string("invalidated"));
                let mut list = match doc.value("supersedes") {
                    Some(Value::Seq(items)) => items.clone(),
                    _ => Vec::new(),
                };
                list.push(entry(reason.as_str(), &req.now, &actor));
                edit.set("supersedes", Value::Seq(list));
                set_or_clear_reason(&mut edit, note.as_deref());
                if had_disputes {
                    edit.remove("disputes");
                    if let Some(u) = &urn {
                        events.push(event(ReviewEventKind::Closed, u, &[], &req.now));
                    }
                }
            }
            Move::Confirm => {
                edit.set("last-reviewed", Value::string(req.now.clone()));
            }
            Move::SendBack {
                disputes,
                status_reason,
            } => {
                let reason = status_reason.as_deref().filter(|r| !r.trim().is_empty());
                if disputes.is_empty() && reason.is_none() {
                    return Err(Refusal::new(
                        code::ILLEGAL_TRANSITION,
                        "sending an approved page back to draft needs disputes, a status-reason, or both (ADR-VG-013 section 5)",
                    ));
                }
                let entries = self.build_disputes(page, disputes, &req.now)?;
                edit.set("status", Value::string("draft"));
                if !entries.is_empty() {
                    edit.set("disputes", Value::Seq(entries.clone()));
                    if let Some(u) = &urn {
                        events.push(event(ReviewEventKind::Opened, u, &entries, &req.now));
                    }
                }
                set_or_clear_reason(&mut edit, reason);
            }
            Move::Archive { valid_to, note } => {
                edit.set("status", Value::string("archived"));
                if let Some(v) = valid_to {
                    ts("valid_to", v)?;
                    edit.set("valid-to", Value::string(v.clone()));
                }
                set_or_clear_reason(&mut edit, note.as_deref());
            }
        }
        append_contributor(&mut edit, &actor, &req.now);
        let (text, receipt) = edit.finish()?;
        let new_page = self.reparse(page, &text)?;
        let mut replacements = vec![Replacement {
            kind: VaultKind::Working,
            page: new_page,
        }];
        let mut writes = vec![self.write_for(VaultKind::Working, page, text, receipt)?];
        for (p, w) in others {
            replacements.push(Replacement {
                kind: VaultKind::Working,
                page: p,
            });
            writes.push(w);
        }
        self.gate(&replacements)?;
        Ok(Plan {
            operation: mv.name(),
            writes,
            requested_keys: vec!["status".to_owned()],
            ignored_keys: Vec::new(),
            events,
        })
    }
}

fn set_or_clear_reason(edit: &mut Edit, note: Option<&str>) {
    match note.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => edit.set("status-reason", Value::string(n)),
        None => edit.remove("status-reason"),
    }
}
