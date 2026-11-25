//! Approve-then-supersede, and finishing one that was interrupted
//! (ADR-VG-013 section 3).
//!
//! The approval writes the draft first: `approved`, with a `supersedes` entry
//! per disputed page. The pages it replaces are written after: `superseded`,
//! `superseded-by`, and `valid-to` when the reason is `world-changed`. Because
//! the draft goes first, an interruption leaves both pages approved, as before
//! the review, never a retired page with nothing in its place. The engine
//! refuses every other write to either page until [`Engine::complete_supersession`]
//! has finished the job.

use ontomark_compile::input::page::{Page, VaultKind};
use ontomark_parse::{Document, Value};
use serde::{Deserialize, Serialize};

use crate::actor::{Actor, Writer};
use crate::edit::{map_text, maps, Edit};
use crate::engine::{Engine, Replacement};
use crate::error::{code, Refusal, Result};
use crate::ops::{append_contributor, status_of, ts};
use crate::plan::Plan;

/// The two reasons an approval may give for replacing a page. `merged` and
/// `renamed` are identity curation whose clock is undecided for an Episode
/// (ADR-VG-013 open item 6), so they are not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupersedeReason {
    /// It was true and stopped being true: closes valid time (`valid-to`).
    WorldChanged,
    /// It was never true: closes recorded time.
    Corrected,
}

impl SupersedeReason {
    /// The value written in the `reason` key.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorldChanged => "world-changed",
            Self::Corrected => "corrected",
        }
    }
}

/// An approval's answer for one disputed page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    /// The disputed page: `page_resource`, id or title.
    pub page: String,
    /// Why it is replaced.
    pub reason: SupersedeReason,
    /// For `world-changed`, when it stopped being true; default is the
    /// approval's `now`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
}

/// An approved page whose replaced page has not yet been retired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pending {
    /// The approved page that replaces.
    pub replacing: String,
    /// The page that should now be superseded.
    pub replaced: String,
    /// The entry's reason.
    pub reason: String,
    /// The entry's `at`.
    pub at: String,
}

/// A request to finish an interrupted approve-then-supersede.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompleteRequest {
    /// Either page of the pair.
    pub id: String,
    /// Who finishes it.
    pub writer: Writer,
    /// When.
    pub now: String,
}

impl Engine {
    /// Every half-finished supersession in the working vault.
    #[must_use]
    pub fn pending_supersessions(&self) -> Vec<Pending> {
        let mut out = Vec::new();
        for p in &self.working.pages {
            if status_of(p).as_deref() != Some("approved") {
                continue;
            }
            let Some(urn) = p.frontmatter.text("page_resource") else {
                continue;
            };
            let doc = Document::parse(&p.source);
            for e in maps(&doc, "supersedes") {
                let (Some(res), Some(reason)) = (map_text(&e, "resource"), map_text(&e, "reason"))
                else {
                    continue;
                };
                if reason != "world-changed" && reason != "corrected" {
                    continue;
                }
                let Some(target) = self.find_resource(&res) else {
                    continue;
                };
                let done = target.frontmatter.strings("superseded-by").contains(&urn);
                if status_of(target).as_deref() == Some("approved") && !done {
                    out.push(Pending {
                        replacing: p.id.clone(),
                        replaced: target.id.clone(),
                        reason,
                        at: map_text(&e, "at").unwrap_or_default(),
                    });
                }
            }
        }
        out
    }

    /// Refuse a write to a page caught in an interrupted supersession.
    pub(crate) fn refuse_pending(&self, id: &str) -> Result<()> {
        if let Some(p) = self
            .pending_supersessions()
            .into_iter()
            .find(|p| p.replacing == id || p.replaced == id)
        {
            return Err(Refusal::new(
                code::PENDING_SUPERSESSION,
                format!(
                    "`{}` replaces `{}` but `{}` is still approved; complete that supersession first",
                    p.replacing, p.replaced, p.replaced
                ),
            ));
        }
        Ok(())
    }

    /// The write that retires `old` in favour of `new_urn`.
    pub(crate) fn retire(
        &self,
        old: &Page,
        new_urn: &str,
        reason: SupersedeReason,
        valid_to: Option<&str>,
        actor: &Actor,
        now: &str,
    ) -> Result<(Page, crate::plan::PlannedWrite)> {
        let doc = Document::parse(&old.source);
        let mut edit = Edit::new(&old.source);
        edit.set("status", Value::string("superseded"));
        let mut by = match doc.value("superseded-by") {
            Some(Value::Seq(items)) => items.clone(),
            Some(Value::Null) | None => Vec::new(),
            Some(other) => vec![other.clone()],
        };
        by.push(Value::string(new_urn));
        edit.set("superseded-by", Value::Seq(by));
        if reason == SupersedeReason::WorldChanged && doc.value("valid-to").is_none() {
            let at = valid_to.unwrap_or(now);
            ts("valid_to", at)?;
            edit.set("valid-to", Value::string(at));
        }
        edit.remove("status-reason");
        append_contributor(&mut edit, actor, now);
        let (text, receipt) = edit.finish()?;
        let page = self.reparse(old, &text)?;
        let write = self.write_for(VaultKind::Working, old, text, receipt)?;
        Ok((page, write))
    }

    /// Finish an interrupted approve-then-supersede that involves `id`.
    ///
    /// # Errors
    /// [`code::NOT_FOUND`] when nothing is pending for `id`; any other refusal
    /// as for the operation that was interrupted.
    pub fn complete_supersession(&self, req: &CompleteRequest) -> Result<Plan> {
        self.require_v3("completing a supersession")?;
        ts("now", &req.now)?;
        let actor = self.actor(&req.writer)?;
        let mine: Vec<Pending> = self
            .pending_supersessions()
            .into_iter()
            .filter(|p| p.replacing == req.id || p.replaced == req.id)
            .collect();
        if mine.is_empty() {
            return Err(Refusal::new(
                code::NOT_FOUND,
                format!("no interrupted supersession involves `{}`", req.id),
            ));
        }
        let mut pages = Vec::new();
        let mut writes = Vec::new();
        for p in &mine {
            let (Some(old), Some(new)) = (
                self.find(VaultKind::Working, &p.replaced),
                self.find(VaultKind::Working, &p.replacing),
            ) else {
                continue;
            };
            let Some(new_urn) = new.frontmatter.text("page_resource") else {
                continue;
            };
            let reason = if p.reason == "world-changed" {
                SupersedeReason::WorldChanged
            } else {
                SupersedeReason::Corrected
            };
            let (page, write) =
                self.retire(old, &new_urn, reason, Some(p.at.as_str()), &actor, &req.now)?;
            pages.push(Replacement {
                kind: VaultKind::Working,
                page,
            });
            writes.push(write);
        }
        self.gate(&pages)?;
        Ok(Plan {
            operation: "complete_supersession",
            writes,
            requested_keys: vec!["status".to_owned()],
            ignored_keys: Vec::new(),
            events: Vec::new(),
        })
    }
}
