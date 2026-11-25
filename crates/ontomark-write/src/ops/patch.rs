//! Patch a page: change only the keys asked for.
//!
//! Under vocabulary 3 on a working page the engine also stamps what
//! ADR-VG-012 and ADR-VG-013 make it responsible for: `contributors`,
//! `content-updated` and, on durable memory pages, the review consequence of a
//! content edit. Every other byte of the page is identical before and after.

use ontomark_compile::input::page::VaultKind;
use ontomark_parse::{PatchOp, SourceDigest, Span, Value};
use serde::{Deserialize, Serialize};

use crate::actor::Writer;
use crate::edit::Edit;
use crate::engine::{Engine, Replacement};
use crate::error::{code, Refusal, Result};
use crate::keys::{is_content_key, CLOCK_KEYS, IMMUTABLE_KEYS, MEMORY_OWNED_KEYS};
use crate::ops::{append_contributor, check_base, is_memory, status_of, ts, TERMINAL};
use crate::plan::Plan;
use crate::Role;

/// One requested change to a top-level key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Change {
    /// Replace (or add) the value under `key`.
    Set {
        /// The key.
        key: String,
        /// The new value.
        value: Value,
    },
    /// Append to the list under `key`, creating it when absent.
    Append {
        /// The key.
        key: String,
        /// The value to append.
        value: Value,
    },
    /// Remove `key`.
    Unset {
        /// The key.
        key: String,
    },
}

impl Change {
    /// The key this change targets.
    #[must_use]
    pub fn key(&self) -> &str {
        match self {
            Self::Set { key, .. } | Self::Append { key, .. } | Self::Unset { key } => key,
        }
    }
}

/// A guarded replacement of a body range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyEdit {
    /// Start byte, in document-source coordinates.
    pub start: usize,
    /// End byte.
    pub end: usize,
    /// The text the range must currently hold.
    pub expected: String,
    /// The replacement.
    pub replacement: String,
}

/// A request to patch one page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchRequest {
    /// Which vault the page is in.
    #[serde(default)]
    pub role: Role,
    /// The page id.
    pub id: String,
    /// Key changes, applied in order.
    #[serde(default)]
    pub changes: Vec<Change>,
    /// Body edits.
    #[serde(default)]
    pub body: Vec<BodyEdit>,
    /// The digest the request was planned against; refuses when the page moved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<SourceDigest>,
    /// Who writes. Required under vocabulary 3 on a working page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writer: Option<Writer>,
    /// When, `YYYY-MM-DDTHH:MM:SSZ`. Required with a writer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
}

impl Engine {
    /// Patch one page.
    ///
    /// * **Vocabulary 2, or a knowledge page:** a lossless key patch and
    ///   nothing else: no stamps, no contributors, no new validation. That is
    ///   the contract CLI `edit` has under 0.9.0 (it validates nothing), kept
    ///   so existing callers do not change behaviour, but byte-preserving
    ///   instead of re-serialised.
    /// * **Vocabulary 3, working page:** engine-owned keys are refused,
    ///   clocks a client sent are ignored, the writer is appended to
    ///   `contributors`, a content change moves `content-updated`, an edit to
    ///   an `approved` memory page keeps it `approved` and moves
    ///   `last-reviewed` only when the writer may approve and otherwise sends
    ///   it back to `draft` (ADR-VG-013 section 1), and the result is
    ///   validated: any error the page did not already carry refuses the write.
    ///
    /// # Errors
    /// A [`Refusal`]; nothing is written.
    pub fn patch(&self, req: &PatchRequest) -> Result<Plan> {
        let kind = req.role.kind();
        let page = self
            .find(kind, &req.id)
            .ok_or_else(|| Refusal::new(code::NOT_FOUND, format!("no page `{}`", req.id)))?;
        check_base(page, req.expect.as_ref())?;
        let stamping = self.is_v3() && kind == VaultKind::Working;
        let memory = stamping && is_memory(self, kind, page);
        if stamping {
            self.refuse_pending(&page.id)?;
        }

        let mut ignored = Vec::new();
        let mut kept: Vec<&Change> = Vec::new();
        let before = ontomark_parse::Document::parse(&page.source);
        for change in &req.changes {
            let key = change.key();
            if self.is_v3() && CLOCK_KEYS.contains(&key) && kind == VaultKind::Working {
                ignored.push(key.to_owned());
                continue;
            }
            if self.is_v3() {
                if IMMUTABLE_KEYS.contains(&key) && before.frontmatter().is_some_and(|f| f.has(key))
                {
                    return Err(Refusal::new(
                        code::ENGINE_OWNED_KEY,
                        format!("`{key}` is kept for the life of the page and cannot be patched"),
                    ));
                }
                if stamping && key == "contributors" {
                    return Err(Refusal::new(
                        code::ENGINE_OWNED_KEY,
                        "`contributors` is appended by the engine on every write",
                    ));
                }
                if memory && MEMORY_OWNED_KEYS.contains(&key) {
                    return Err(Refusal::new(
                        code::ENGINE_OWNED_KEY,
                        format!(
                            "`{key}` on a durable memory page is written only by a lifecycle operation (approve, reject, confirm, send back, withdraw, archive, record disputes)"
                        ),
                    ));
                }
            }
            kept.push(change);
        }

        // Plan the requested edit alone, to learn what actually changes.
        let mut requested = Edit::new(&page.source);
        for change in &kept {
            match change {
                Change::Set { key, value } => requested.set(key, value.clone()),
                Change::Unset { key } => requested.remove(key),
                Change::Append { key, value } => {
                    let mut items = match before.value(key) {
                        Some(Value::Seq(items)) => items.clone(),
                        Some(Value::Null) | None => Vec::new(),
                        Some(other) => vec![other.clone()],
                    };
                    items.push(value.clone());
                    requested.set(key, Value::Seq(items));
                }
            }
        }
        for b in &req.body {
            requested.replace_body(PatchOp::ReplaceBody {
                range: Span::new(b.start, b.end),
                expected: b.expected.clone(),
                replacement: b.replacement.clone(),
            });
        }
        let (probe_text, _) = requested.clone().finish()?;
        let after_probe = ontomark_parse::Document::parse(&probe_text);
        let mut requested_keys: Vec<String> = Vec::new();
        for change in &kept {
            let key = change.key();
            let moved = before.value(key) != after_probe.value(key)
                || before.frontmatter().is_some_and(|f| f.has(key))
                    != after_probe.frontmatter().is_some_and(|f| f.has(key));
            if moved && !requested_keys.iter().any(|k| k == key) {
                requested_keys.push(key.to_owned());
            }
        }
        let body_changed = before.body() != after_probe.body();
        if probe_text == page.source {
            return Ok(Plan {
                operation: "patch",
                writes: Vec::new(),
                requested_keys: Vec::new(),
                ignored_keys: ignored,
                events: Vec::new(),
            });
        }

        let mut edit = requested;
        if stamping {
            let (writer, now) = writer_and_now(req)?;
            let actor = self.actor(writer)?;
            let content_changed = body_changed || requested_keys.iter().any(|k| is_content_key(k));
            if memory {
                let status = status_of(page).unwrap_or_default();
                if content_changed && TERMINAL.contains(&status.as_str()) {
                    return Err(Refusal::new(
                        code::TERMINAL,
                        format!(
                            "`{}` is {status}, a terminal status; a claim that comes back is a new page",
                            page.id
                        ),
                    ));
                }
                if content_changed {
                    edit.set("content-updated", Value::string(now));
                    if status == "approved" {
                        if actor.may_approve(self.roster_known()) {
                            edit.set("last-reviewed", Value::string(now));
                        } else {
                            edit.set("status", Value::string("draft"));
                            edit.remove("status-reason");
                        }
                    }
                }
            }
            append_contributor(&mut edit, &actor, now);
        }
        let (text_after, receipt) = edit.finish()?;
        let new_page = self.reparse(page, &text_after)?;
        if self.is_v3() {
            self.gate(&[Replacement {
                kind,
                page: new_page,
            }])?;
        }
        let write = self.write_for(kind, page, text_after, receipt)?;
        Ok(Plan {
            operation: "patch",
            writes: vec![write],
            requested_keys,
            ignored_keys: ignored,
            events: Vec::new(),
        })
    }
}

fn writer_and_now(req: &PatchRequest) -> Result<(&Writer, &str)> {
    let writer = req.writer.as_ref().ok_or_else(|| {
        Refusal::new(
            code::WRITER_REQUIRED,
            "a version 3 working-page write must name its principal (`by`)",
        )
    })?;
    let now = req
        .now
        .as_deref()
        .ok_or_else(|| Refusal::new(code::WRITER_REQUIRED, "a stamped write needs `now`"))?;
    ts("now", now)?;
    Ok((writer, now))
}
