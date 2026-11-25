//! Disputes (ADR-VG-013 section 3): a draft Belief or Procedure names each
//! approved page it contradicts. Only the engine writes them; `detected` is
//! the engine's clock.

use ontomark_compile::input::page::{Page, VaultKind};
use ontomark_parse::{Document, SourceDigest, Value};
use serde::{Deserialize, Serialize};

use crate::actor::Writer;
use crate::edit::{map_text, Edit};
use crate::engine::{Engine, Replacement};
use crate::error::{code, Refusal, Result};
use crate::ops::{append_contributor, check_base, status_of, ts};
use crate::plan::{DisputeRef, Plan, ReviewEvent, ReviewEventKind};

/// One page a draft contradicts, as the caller names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisputeInput {
    /// The disputed page: its `page_resource`, or its id or title.
    pub page: String,
    /// Why, free text; it has the draft's visibility and never enters the review item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Replace the disputes of a draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordDisputesRequest {
    /// The draft's page id.
    pub id: String,
    /// The digest the request was planned against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<SourceDigest>,
    /// The complete list the draft should hold afterwards. An entry already
    /// present keeps its `detected`; a new one is stamped now. Empty clears
    /// the disputes. Naming a disputed page's successor in place of the old
    /// target is how a dispute is retargeted.
    pub disputes: Vec<DisputeInput>,
    /// Who writes.
    pub writer: Writer,
    /// When.
    pub now: String,
}

/// A dispute as it stands on a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Held {
    pub reference: String,
    pub detected: Option<String>,
    pub reason: Option<String>,
}

/// The disputes a page holds, in either the ADR's mapping form or a bare
/// reference.
pub(crate) fn held(doc: &Document) -> Vec<Held> {
    match doc.value("disputes") {
        Some(Value::Seq(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::Map(m) => map_text(m, "page").map(|reference| Held {
                    reference,
                    detected: map_text(m, "detected"),
                    reason: map_text(m, "reason"),
                }),
                other => other.scalar_text().map(|reference| Held {
                    reference,
                    detected: None,
                    reason: None,
                }),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn visibility(page: &Page) -> String {
    page.frontmatter
        .text("visibility")
        .unwrap_or_else(|| "shared".to_owned())
}

impl Engine {
    /// Check that `target` may be disputed by `draft`: an `approved` Belief or
    /// Procedure of the same visibility, other than the draft itself.
    pub(crate) fn dispute_target<'a>(&'a self, draft: &Page, reference: &str) -> Result<&'a Page> {
        let target = self.find_reference(reference).ok_or_else(|| {
            Refusal::new(
                code::DISPUTE_TARGET,
                format!("`{reference}` names no working page"),
            )
        })?;
        let ty = target.frontmatter.text("type").unwrap_or_default();
        if target.id == draft.id {
            return Err(Refusal::new(
                code::DISPUTE_TARGET,
                "a page cannot dispute itself",
            ));
        }
        if !matches!(ty.as_str(), "Belief" | "Procedure") {
            return Err(Refusal::new(
                code::DISPUTE_TARGET,
                format!(
                    "`{}` is a {ty}; only a Belief or Procedure can be disputed",
                    target.id
                ),
            ));
        }
        let status = status_of(target).unwrap_or_default();
        if status != "approved" {
            let successor = target.frontmatter.strings("superseded-by");
            if status == "superseded" && !successor.is_empty() {
                return Err(Refusal::new(
                    code::DISPUTE_TARGET_MOVED,
                    format!(
                        "`{}` was superseded by {}; dispute that page instead",
                        target.id,
                        successor.join(", ")
                    ),
                ));
            }
            return Err(Refusal::new(
                code::DISPUTE_TARGET,
                format!("`{}` is {status}, not approved", target.id),
            ));
        }
        if visibility(target) != visibility(draft) {
            return Err(Refusal::new(
                code::DISPUTE_TARGET,
                format!(
                    "`{}` has visibility {} and the draft {}; a dispute needs the same visibility",
                    target.id,
                    visibility(target),
                    visibility(draft)
                ),
            ));
        }
        Ok(target)
    }

    /// Build the `disputes` value for `draft` from `inputs`, keeping the
    /// `detected` of entries already held.
    pub(crate) fn build_disputes(
        &self,
        draft: &Page,
        inputs: &[DisputeInput],
        now: &str,
    ) -> Result<Vec<Value>> {
        let ty = draft.frontmatter.text("type").unwrap_or_default();
        if !inputs.is_empty() && !matches!(ty.as_str(), "Belief" | "Procedure") {
            return Err(Refusal::new(
                code::DISPUTE_TARGET,
                format!("a {ty} neither records nor receives disputes"),
            ));
        }
        let already = held(&ontomark_parse::Document::parse(&draft.source));
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for input in inputs {
            let target = self.dispute_target(draft, &input.page)?;
            let urn = target.frontmatter.text("page_resource").ok_or_else(|| {
                Refusal::new(
                    code::DISPUTE_TARGET,
                    format!("`{}` has no page_resource; adopt it first", target.id),
                )
            })?;
            if seen.contains(&urn) {
                return Err(Refusal::new(
                    code::DISPUTE_TARGET,
                    format!("`{}` is named twice", target.id),
                ));
            }
            seen.push(urn.clone());
            let detected = already
                .iter()
                .find(|h| {
                    self.find_reference(&h.reference)
                        .and_then(|p| p.frontmatter.text("page_resource"))
                        .as_deref()
                        == Some(urn.as_str())
                })
                .and_then(|h| h.detected.clone())
                .unwrap_or_else(|| now.to_owned());
            let mut entry = vec![
                ("page", Value::string(urn)),
                ("detected", Value::string(detected)),
            ];
            if let Some(r) = input.reason.as_ref().filter(|r| !r.trim().is_empty()) {
                entry.push(("reason", Value::string(r.clone())));
            }
            out.push(map_vec(entry));
        }
        Ok(out)
    }

    /// Record, replace or clear the disputes of a draft Belief or Procedure.
    ///
    /// The review event is emitted when the write opens the item
    /// ([`ReviewEventKind::Opened`]), changes it (`Rewritten`) or empties it
    /// (`Closed`).
    ///
    /// # Errors
    /// A [`Refusal`]; nothing is written.
    pub fn record_disputes(&self, req: &RecordDisputesRequest) -> Result<Plan> {
        self.require_v3("recording disputes")?;
        ts("now", &req.now)?;
        let page = self.find(VaultKind::Working, &req.id).ok_or_else(|| {
            Refusal::new(code::NOT_FOUND, format!("no working page `{}`", req.id))
        })?;
        check_base(page, req.expect.as_ref())?;
        self.refuse_pending(&page.id)?;
        if status_of(page).as_deref() != Some("draft") {
            return Err(Refusal::new(
                code::ILLEGAL_TRANSITION,
                "disputes belong on a draft; send an approved page back to draft with its disputes instead",
            ));
        }
        let actor = self.actor(&req.writer)?;
        let before = ontomark_parse::Document::parse(&page.source);
        let had = held(&before);
        let entries = self.build_disputes(page, &req.disputes, &req.now)?;
        let mut edit = Edit::new(&page.source);
        if entries.is_empty() {
            edit.remove("disputes");
        } else {
            edit.set("disputes", Value::Seq(entries.clone()));
        }
        let (probe, _) = edit.clone().finish()?;
        if probe == page.source {
            return Ok(Plan {
                operation: "record_disputes",
                writes: Vec::new(),
                requested_keys: Vec::new(),
                ignored_keys: Vec::new(),
                events: Vec::new(),
            });
        }
        append_contributor(&mut edit, &actor, &req.now);
        let (text, receipt) = edit.finish()?;
        let new_page = self.reparse(page, &text)?;
        self.gate(&[Replacement {
            kind: VaultKind::Working,
            page: new_page,
        }])?;
        let kind = match (had.is_empty(), entries.is_empty()) {
            (true, false) => Some(ReviewEventKind::Opened),
            (false, true) => Some(ReviewEventKind::Closed),
            (false, false) => Some(ReviewEventKind::Rewritten),
            (true, true) => None,
        };
        let events = match (kind, page.frontmatter.text("page_resource")) {
            (Some(kind), Some(draft)) => vec![event(kind, &draft, &entries, &req.now)],
            _ => Vec::new(),
        };
        Ok(Plan {
            operation: "record_disputes",
            writes: vec![self.write_for(VaultKind::Working, page, text, receipt)?],
            requested_keys: vec!["disputes".to_owned()],
            ignored_keys: Vec::new(),
            events,
        })
    }
}

fn map_vec(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

/// A references-only event for `draft`.
pub(crate) fn event(
    kind: ReviewEventKind,
    draft: &str,
    disputes: &[Value],
    at: &str,
) -> ReviewEvent {
    let refs = disputes
        .iter()
        .filter_map(|v| match v {
            Value::Map(m) => Some(DisputeRef {
                page: map_text(m, "page")?,
                detected: map_text(m, "detected")?,
            }),
            _ => None,
        })
        .collect();
    ReviewEvent {
        kind,
        draft: draft.to_owned(),
        disputes: if kind == ReviewEventKind::Closed {
            Vec::new()
        } else {
            refs
        },
        at: at.to_owned(),
    }
}
