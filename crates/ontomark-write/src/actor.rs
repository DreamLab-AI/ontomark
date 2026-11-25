//! The principal and instrument a write is made under (ADR-VG-011).
//!
//! `by` is always a principal, `did:nostr:` followed by 64 lowercase hex
//! characters; `via` is the optional instrument, `agent:<name>/<version>` or
//! `process:<name>/<version>`. A `human:<npub>` key is read as the same key and
//! written back as its DID, and a key the roster lists under `also:` is written
//! as its canonical principal.

use indexmap::IndexMap;
use ontomark_compile::build::attribution::{PrincipalKind, Roster};
use ontomark_compile::input::okf::Actor as StampActor;
use ontomark_compile::input::principal::npub_to_hex;
use ontomark_compile::input::time::parse_utc_seconds;
use ontomark_parse::Value;
use serde::{Deserialize, Serialize};

use crate::error::{code, Refusal, Result};

/// A writer as a host names it: unvalidated text.
///
/// ```
/// use ontomark_write::Writer;
/// let w = Writer::new("did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a")
///     .via("agent:claude/1");
/// assert!(w.via.is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Writer {
    /// The principal: `did:nostr:<64 hex>` or `human:<npub>`.
    pub by: String,
    /// The instrument, when one directed the write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
}

impl Writer {
    /// A writer acting directly.
    #[must_use]
    pub fn new(by: impl Into<String>) -> Self {
        Self {
            by: by.into(),
            via: None,
        }
    }

    /// The same writer directing an instrument.
    #[must_use]
    pub fn via(mut self, via: impl Into<String>) -> Self {
        self.via = Some(via.into());
        self
    }
}

/// A validated writer: a canonical principal DID and an optional instrument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// `did:nostr:<64 hex>`, canonical under the roster when one was given.
    pub did: String,
    /// `agent:` or `process:` instrument.
    pub via: Option<String>,
    /// The roster's kind for the principal, when a roster knows it.
    pub kind: Option<PrincipalKind>,
}

impl Actor {
    /// Validate `writer` against the optional roster.
    ///
    /// # Errors
    /// [`code::INVALID_REQUEST`] when `by` is not a principal key or `via` is
    /// not an `agent:`/`process:` instrument.
    pub fn resolve(writer: &Writer, roster: Option<&Roster>) -> Result<Self> {
        let parsed = writer.by.parse::<StampActor>().map_err(|e| {
            Refusal::new(
                code::INVALID_REQUEST,
                format!("`by` must be a principal key: {e}"),
            )
        })?;
        let did = match parsed {
            StampActor::Did(hex) => format!("did:nostr:{hex}"),
            StampActor::Human(npub) => {
                format!(
                    "did:nostr:{}",
                    npub_to_hex(&npub).map_err(|e| {
                        Refusal::new(code::INVALID_REQUEST, format!("`by`: {e}"))
                    })?
                )
            }
            StampActor::Agent { .. } | StampActor::Process { .. } => {
                return Err(Refusal::new(
                    code::INVALID_REQUEST,
                    "`by` names an instrument; a principal goes in `by` and the instrument in `via`",
                ));
            }
        };
        let resolved = roster.and_then(|r| r.resolve_principal(&did));
        let (did, kind) = resolved.map_or((did, None), |p| (p.canonical, Some(p.kind)));
        let via = match &writer.via {
            None => None,
            Some(v) => match v.parse::<StampActor>() {
                Ok(StampActor::Agent { .. } | StampActor::Process { .. }) => {
                    Some(v.trim().to_owned())
                }
                _ => {
                    return Err(Refusal::new(
                        code::INVALID_REQUEST,
                        format!(
                        "`via` {v:?} must be agent:<name>/<version> or process:<name>/<version>"
                    ),
                    ))
                }
            },
        };
        Ok(Self { did, via, kind })
    }

    /// `true` when this writer may approve or confirm a memory page
    /// (ADR-VG-013 open item 2, default 2026-11-25): no instrument, and, when
    /// the roster is known, a principal it lists as a human.
    #[must_use]
    pub fn may_approve(&self, roster_known: bool) -> bool {
        self.via.is_none() && (!roster_known || self.kind == Some(PrincipalKind::Human))
    }

    /// The stamp `{by, via?, at}`; `with_via` is `false` for approvals,
    /// confirmations and the supersession entry an approval writes.
    #[must_use]
    pub fn stamp(&self, at: &str, with_via: bool) -> Value {
        let mut m = IndexMap::new();
        m.insert("by".to_owned(), Value::string(self.did.clone()));
        if with_via {
            if let Some(via) = &self.via {
                m.insert("via".to_owned(), Value::string(via.clone()));
            }
        }
        m.insert("at".to_owned(), Value::string(at));
        Value::Map(m)
    }
}

/// Check that `ts` is a strict `YYYY-MM-DDTHH:MM:SSZ` instant and return its
/// Unix seconds.
///
/// # Errors
/// [`code::INVALID_REQUEST`] otherwise.
pub fn check_timestamp(what: &str, ts: &str) -> Result<i64> {
    parse_utc_seconds(ts).ok_or_else(|| {
        Refusal::new(
            code::INVALID_REQUEST,
            format!("{what} {ts:?} must be YYYY-MM-DDTHH:MM:SSZ (UTC, second precision)"),
        )
    })
}
