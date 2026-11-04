//! Field-level certification read from a note's own `verified` stamps
//! (ADR-OM-012).
//!
//! The OKF trust block already records human review as a list of stamps:
//!
//! ```yaml
//! verified:
//!   - by: human:npub1...
//!     at: 2026-10-05T18:59:14Z          # no `keys`: the whole page
//!   - by: human:npub1...
//!     at: 2026-11-04T10:00:00Z
//!     keys: [is-a, definition]          # only these keys
//! ```
//!
//! A key is **certified** when a human stamp covers it: a stamp without
//! `keys` covers every key on the page, a stamp with `keys` covers those.
//! Ingest never adds a value to a certified key; the proposal is withheld
//! with [`crate::WithholdReason::Certified`], so a re-ingest cannot alter what a
//! reviewer approved. Adapted from NVIDIA auto-ontology, whose recompile
//! keeps any field marked certified.
//!
//! Only `human:<npub>` actors certify, the same rule as `ontomark-compile`'s
//! `Actor::is_human`: an agent or process stamp, or one whose actor does not
//! parse, certifies nothing. Certification is read from the note on every
//! call, never from the host's request, so a stale or edited request cannot
//! drop it.

use std::collections::BTreeSet;

use ontomark_parse::{Document, Value};
use serde::{Deserialize, Serialize};

/// The keys a note's human review covers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Certification {
    /// A human stamp without `keys` covers the whole page.
    pub whole_page: bool,
    /// Keys named by human stamps that list `keys`, sorted.
    pub keys: BTreeSet<String>,
}

impl Certification {
    /// Read the certification of `doc` from its `verified_key` list.
    ///
    /// ```
    /// use ontomark_ingest::Certification;
    /// use ontomark_parse::Document;
    ///
    /// let doc = Document::parse(
    ///     "---\nverified:\n  - by: human:npub1x\n    at: 2026-11-04T00:00:00Z\n    keys: [is-a]\n---\n",
    /// );
    /// let c = Certification::of(&doc, "verified");
    /// assert!(c.covers("is-a"));
    /// assert!(!c.covers("enables"));
    /// ```
    #[must_use]
    pub fn of(doc: &Document, verified_key: &str) -> Self {
        let mut out = Self::default();
        let Some(Value::Seq(stamps)) = doc.value(verified_key) else {
            return out;
        };
        for stamp in stamps {
            let Value::Map(m) = stamp else { continue };
            let human = matches!(m.get("by"), Some(Value::String(by)) if is_human_actor(by));
            let dated = matches!(m.get("at"), Some(Value::String(at)) if !at.trim().is_empty());
            if !(human && dated) {
                continue;
            }
            match m.get("keys") {
                None | Some(Value::Null) => out.whole_page = true,
                Some(v) => out.keys.extend(
                    v.texts()
                        .into_iter()
                        .map(|k| k.trim().to_owned())
                        .filter(|k| !k.is_empty()),
                ),
            }
        }
        out
    }

    /// `true` when a human stamp covers `key`.
    #[must_use]
    pub fn covers(&self, key: &str) -> bool {
        self.whole_page || self.keys.contains(key)
    }

    /// `true` when nothing on the page is certified.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.whole_page && self.keys.is_empty()
    }
}

/// `true` for a `human:<npub>` actor: the only actor that certifies.
///
/// Mirrors `ontomark-compile`'s `Actor::is_human` (the `human:` prefix with a
/// non-empty remainder, after trimming), which this crate cannot depend on
/// without losing its `wasm32` portability.
///
/// ```
/// use ontomark_ingest::is_human_actor;
/// assert!(is_human_actor("human:npub1abc"));
/// assert!(!is_human_actor("agent:ontology-mesh/1"));
/// assert!(!is_human_actor("did:nostr:jjohare"));
/// ```
#[must_use]
pub fn is_human_actor(actor: &str) -> bool {
    actor
        .trim()
        .strip_prefix("human:")
        .is_some_and(|npub| !npub.is_empty())
}
