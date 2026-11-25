//! A JSON facade over the engine, for hosts that cannot call Rust directly
//! (the Obsidian plugin's wasm worker).
//!
//! A host builds the [`Engine`] once from its snapshot, then sends one
//! [`Request`] per write and receives one [`Response`]: a [`Plan`] to apply
//! (the new text of every file, in order, with the review events) or a
//! [`Refusal`]. The host applies the plan itself, for example with
//! `app.vault.process`, guarded by each write's `before_text`.

use serde::{Deserialize, Serialize};

use crate::engine::Engine;
use crate::error::{IssueRecord, Refusal};
use crate::ops::adopt::AdoptRequest;
use crate::ops::create::CreateRequest;
use crate::ops::disputes::RecordDisputesRequest;
use crate::ops::patch::PatchRequest;
use crate::ops::supersede::CompleteRequest;
use crate::ops::transition::TransitionRequest;
use crate::plan::Plan;

/// One engine request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    /// [`Engine::create`].
    Create(CreateRequest),
    /// [`Engine::patch`].
    Patch(PatchRequest),
    /// [`Engine::adopt`].
    Adopt(AdoptRequest),
    /// [`Engine::transition`].
    Transition(TransitionRequest),
    /// [`Engine::record_disputes`].
    RecordDisputes(RecordDisputesRequest),
    /// [`Engine::complete_supersession`].
    CompleteSupersession(CompleteRequest),
}

/// The answer to a [`Request`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    /// The operation is planned; nothing has been written.
    Planned {
        /// What to write.
        plan: Plan,
    },
    /// The operation was refused; nothing was written.
    Refused {
        /// The stable code.
        code: &'static str,
        /// One line a human can act on.
        message: String,
        /// Validation errors the write would have introduced.
        issues: Vec<IssueRecord>,
    },
}

/// Run one request.
///
/// ```
/// use ontomark_write::wire::{execute_json, Response};
/// # use ontomark_compile::input::page::{Vault, VaultKind};
/// # use ontomark_compile::input::vocabulary::Vocabulary;
/// # use ontomark_compile::profile::Profile;
/// # use ontomark_write::Engine;
/// # let working = Vault { root: "/v".into(), kind: VaultKind::Working, pages: vec![], journals: vec![], skipped: vec![] };
/// let engine = Engine::new(Profile::default(), Vocabulary::generic(), None, working, None);
/// let out = execute_json(&engine, r#"{"kind":"patch","id":"x","changes":[]}"#);
/// assert!(out.contains("\"status\":\"refused\""));
/// assert!(out.contains("NOT_FOUND"));
/// ```
#[must_use]
pub fn execute(engine: &Engine, request: &Request) -> Response {
    let result = match request {
        Request::Create(r) => engine.create(r),
        Request::Patch(r) => engine.patch(r),
        Request::Adopt(r) => engine.adopt(r),
        Request::Transition(r) => engine.transition(r),
        Request::RecordDisputes(r) => engine.record_disputes(r),
        Request::CompleteSupersession(r) => engine.complete_supersession(r),
    };
    match result {
        Ok(plan) => Response::Planned { plan },
        Err(Refusal {
            code,
            message,
            issues,
        }) => Response::Refused {
            code,
            message,
            issues,
        },
    }
}

/// [`execute`] over JSON text. A request that does not parse is a refusal
/// with code `INVALID_REQUEST`.
#[must_use]
pub fn execute_json(engine: &Engine, request: &str) -> String {
    let response = match serde_json::from_str::<Request>(request) {
        Ok(r) => execute(engine, &r),
        Err(e) => Response::Refused {
            code: crate::code::INVALID_REQUEST,
            message: format!("the request is not valid JSON for the engine: {e}"),
            issues: Vec::new(),
        },
    };
    serde_json::to_string(&response).unwrap_or_else(|e| {
        format!(r#"{{"status":"refused","code":"INVALID_REQUEST","message":"{e}","issues":[]}}"#)
    })
}

/// Serde adapter: 32 bytes as 64 lowercase hex characters.
pub mod hex32 {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Serialise as hex.
    ///
    /// # Errors
    /// Never, beyond the serializer's own.
    pub fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(bytes))
    }

    /// Deserialise from hex.
    ///
    /// # Errors
    /// When the text is not exactly 64 hex characters.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(d)?;
        let raw = hex::decode(&text).map_err(serde::de::Error::custom)?;
        <[u8; 32]>::try_from(raw.as_slice())
            .map_err(|_| serde::de::Error::custom("entropy must be 32 bytes (64 hex characters)"))
    }
}
