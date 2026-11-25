//! Give an existing working page the identity it lacks.
//!
//! A daily note Obsidian made, or a page written before vocabulary 3, has no
//! `page_resource`, `visibility` or `generated`. `adopt` adds exactly the
//! missing ones: the first `generated` stamp (which, once present, no write
//! may change), a minted `page_resource` (journals included, ruling B4) and
//! `visibility: shared`. A page that has all three is left alone.

use ontomark_compile::input::page::VaultKind;
use ontomark_parse::Value;
use serde::{Deserialize, Serialize};

use crate::actor::Writer;
use crate::edit::Edit;
use crate::engine::{Engine, Replacement};
use crate::error::{code, Refusal, Result};
use crate::ops::create::PAGE_URN;
use crate::ops::{append_contributor, check_base, ts};
use crate::plan::Plan;
use ontomark_parse::SourceDigest;

/// A request to adopt one working page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdoptRequest {
    /// The page id (a journal is `journals/YYYY-MM-DD`).
    pub id: String,
    /// The digest the request was planned against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<SourceDigest>,
    /// Who adopts: the first `generated` stamp and a contributor.
    pub writer: Writer,
    /// When.
    pub now: String,
    /// Entropy for the identity, as for [`crate::CreateRequest`].
    #[serde(with = "crate::wire::hex32")]
    pub entropy: [u8; 32],
}

impl Engine {
    /// Add whichever of `page_resource`, `visibility` and `generated` a
    /// working page lacks (vocabulary 3 only).
    ///
    /// # Errors
    /// A [`Refusal`]; nothing is written.
    pub fn adopt(&self, req: &AdoptRequest) -> Result<Plan> {
        self.require_v3("adopting a working page")?;
        ts("now", &req.now)?;
        let page = self.find(VaultKind::Working, &req.id).ok_or_else(|| {
            Refusal::new(code::NOT_FOUND, format!("no working page `{}`", req.id))
        })?;
        check_base(page, req.expect.as_ref())?;
        let actor = self.actor(&req.writer)?;
        let mut edit = Edit::new(&page.source);
        let has = |k: &str| page.frontmatter.get(k).is_some();
        let mut added = Vec::new();
        if !has("page_resource") {
            let urn = format!("{PAGE_URN}{}", hex::encode(req.entropy));
            if self.find_resource(&urn).is_some() {
                return Err(Refusal::new(
                    code::EXISTS,
                    "that entropy already minted an identity",
                ));
            }
            edit.set("page_resource", Value::string(urn));
            added.push("page_resource".to_owned());
        }
        if !has("visibility") {
            edit.set("visibility", Value::string("shared"));
            added.push("visibility".to_owned());
        }
        if !has("generated") {
            edit.set("generated", actor.stamp(&req.now, true));
            added.push("generated".to_owned());
        }
        if added.is_empty() {
            return Ok(Plan {
                operation: "adopt",
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
        let write = self.write_for(VaultKind::Working, page, text, receipt)?;
        Ok(Plan {
            operation: "adopt",
            writes: vec![write],
            requested_keys: added,
            ignored_keys: Vec::new(),
            events: Vec::new(),
        })
    }
}
