//! Create a working page: mint its identity, write its first stamp, set its
//! visibility and initial status.

use ontomark_compile::input::page::{Page, VaultKind, EXCLUDED_DIRS};
use ontomark_parse::Value;
use serde::{Deserialize, Serialize};

use crate::actor::Writer;
use crate::edit::Edit;
use crate::engine::{Engine, Replacement};
use crate::error::{code, Refusal, Result};
use crate::ops::{is_memory_type, ts};
use crate::plan::{confine, Plan, PlannedWrite};

/// The namespace of every page identity (ADR-VG-012 section 8).
pub const PAGE_URN: &str = "urn:visionflow:page:";

/// Keys the engine alone sets on a new page; a request naming one is refused.
const ENGINE_KEYS: [&str; 10] = [
    "type",
    "status",
    "page_resource",
    "visibility",
    "generated",
    "contributors",
    "content-updated",
    "last-reviewed",
    "supersedes",
    "superseded-by",
];

/// A request to create one working page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateRequest {
    /// The page id: its vault-relative path without `.md`, for example
    /// `beliefs/Rust compiles fast`. Never a journal date.
    pub id: String,
    /// A working type from the vocabulary.
    pub page_type: String,
    /// The type's own keys and any others, in the order they are written.
    #[serde(default)]
    pub fields: Vec<(String, Value)>,
    /// The body, after the frontmatter.
    #[serde(default)]
    pub body: String,
    /// `shared` (the default) or `public`. `private` has no store yet
    /// (ADR-VG-012 section 5) and is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    /// Who writes: the first `generated` stamp and the first contributor.
    pub writer: Writer,
    /// When, `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
    /// 32 random bytes the page identity is made from, as 64 hex characters.
    /// The host draws them (see [`crate::system::entropy`]); the engine never
    /// does, so a plan is reproducible and the engine builds for wasm.
    #[serde(with = "crate::wire::hex32")]
    pub entropy: [u8; 32],
}

impl Engine {
    /// Create a working page (vocabulary 3 only).
    ///
    /// The page gets: `type`; `status: draft` (a durable memory page, or any
    /// type whose status set has `draft`); `page_resource`, a
    /// content-independent identity `urn:visionflow:page:<64 hex>` made from
    /// the entropy, never a title slug (ruling B4); `visibility`; the first
    /// `generated` stamp `{by, via, at}`, which no later write may change; a
    /// first `contributors` entry mirroring it (the shape of ADR-VG-012
    /// section 6); and, on a memory kind, `content-updated` equal to
    /// `generated.at` (ADR-VG-013 section 7). Creating a page already
    /// `approved` is an approval and is refused: create a draft, then approve.
    ///
    /// # Errors
    /// A [`Refusal`]; nothing is written. Under vocabulary 2:
    /// [`code::VOCABULARY_V2`].
    pub fn create(&self, req: &CreateRequest) -> Result<Plan> {
        self.require_v3("creating a working page")?;
        ts("now", &req.now)?;
        check_id(&req.id)?;
        if !self.vocab.working_types.contains(&req.page_type) {
            return Err(Refusal::new(
                code::INVALID_REQUEST,
                format!(
                    "`{}` is not a working type of this vocabulary",
                    req.page_type
                ),
            ));
        }
        let lower = req.id.to_lowercase();
        if self
            .working
            .pages
            .iter()
            .chain(self.working.journals.iter())
            .any(|p| p.id.to_lowercase() == lower)
        {
            return Err(Refusal::new(
                code::EXISTS,
                format!("`{}` already exists; a creation never overwrites", req.id),
            ));
        }
        for (key, _) in &req.fields {
            if ENGINE_KEYS.contains(&key.as_str()) {
                return Err(Refusal::new(
                    code::ENGINE_OWNED_KEY,
                    format!("`{key}` is set by the engine on creation and cannot be supplied"),
                ));
            }
            if key == "disputes" || key == "status-reason" || key == "valid-to" {
                return Err(Refusal::new(
                    code::ENGINE_OWNED_KEY,
                    format!("`{key}` is written only by a lifecycle operation"),
                ));
            }
        }
        let visibility = req.visibility.as_deref().unwrap_or("shared");
        match visibility {
            "shared" | "public" => {}
            "private" => {
                return Err(Refusal::new(
                    code::INVALID_REQUEST,
                    "`visibility: private` has no store yet (ADR-VG-012 section 5)",
                ))
            }
            other => {
                return Err(Refusal::new(
                    code::INVALID_REQUEST,
                    format!("`visibility` {other:?} is not shared or public"),
                ))
            }
        }
        let def = self.vocab.working_type(&req.page_type);
        if !def.status.is_empty() && !def.status.iter().any(|s| s == "draft") {
            return Err(Refusal::new(
                code::INVALID_REQUEST,
                format!(
                    "type `{}` has no `draft` status ({}); its first status is not the engine's to choose",
                    req.page_type,
                    def.status.join(", ")
                ),
            ));
        }
        let actor = self.actor(&req.writer)?;
        let urn = format!("{PAGE_URN}{}", hex::encode(req.entropy));
        if self.find_resource(&urn).is_some() {
            return Err(Refusal::new(
                code::EXISTS,
                "that entropy already minted a page identity; draw fresh bytes",
            ));
        }

        let mut edit = Edit::new(&req.body);
        edit.set("type", Value::string(req.page_type.clone()));
        edit.set("status", Value::string("draft"));
        edit.set("page_resource", Value::string(urn));
        edit.set("visibility", Value::string(visibility));
        edit.set("generated", actor.stamp(&req.now, true));
        edit.set(
            "contributors",
            Value::Seq(vec![actor.stamp(&req.now, true)]),
        );
        if is_memory_type(self, &req.page_type) {
            edit.set("content-updated", Value::string(req.now.clone()));
        }
        for (key, value) in &req.fields {
            edit.set(key, value.clone());
        }
        let (text, _) = edit.finish()?;

        let (path, rel_path) = self.path_for(VaultKind::Working, &req.id)?;
        confine(&rel_path)?;
        let page = Page::parse(path, rel_path.clone(), req.id.clone(), &text).map_err(|e| {
            Refusal::new(
                code::UNPATCHABLE,
                format!("the new page does not parse: {e}"),
            )
        })?;
        self.gate(&[Replacement {
            kind: VaultKind::Working,
            page,
        }])?;
        Ok(Plan {
            operation: "create",
            writes: vec![PlannedWrite {
                vault: VaultKind::Working,
                id: req.id.clone(),
                rel_path,
                before_text: None,
                text,
                receipt: None,
            }],
            requested_keys: req.fields.iter().map(|(k, _)| k.clone()).collect(),
            ignored_keys: Vec::new(),
            events: Vec::new(),
        })
    }
}

/// A page id is a vault-relative path without `.md`: no empty, dot, hidden or
/// control components, no backslash, and not under a folder the vault walk
/// does not treat as pages.
fn check_id(id: &str) -> Result<()> {
    let bad = |why: &str| Refusal::new(code::PATH_ESCAPE, format!("page id {id:?}: {why}"));
    if id.is_empty() || id.trim() != id {
        return Err(bad("empty or padded"));
    }
    if id.ends_with(".md") {
        return Err(bad("give the id without `.md`"));
    }
    if id.contains('\\') || id.chars().any(char::is_control) {
        return Err(bad("backslash or control character"));
    }
    for (i, part) in id.split('/').enumerate() {
        if part.is_empty() || part == "." || part == ".." || part.starts_with('.') {
            return Err(bad("empty, dot or hidden component"));
        }
        if i == 0 && id.contains('/') && EXCLUDED_DIRS.contains(&part) {
            return Err(bad("a folder the vault does not load as pages"));
        }
    }
    if EXCLUDED_DIRS.contains(&id) {
        return Err(bad("a folder name"));
    }
    Ok(())
}
