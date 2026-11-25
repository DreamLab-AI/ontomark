//! OKF v0.2 lifecycle and trust vocabulary (contract C1 `okf:`).
//!
//! Five frontmatter keys carry the whole governance model:
//!
//! | key | meaning |
//! |---|---|
//! | `status` | [`Status`] — `draft`, `stable` or `deprecated` |
//! | `stale_after` | the date past which the claim must be re-verified |
//! | `generated` | one [`Stamp`] — who produced the page and when |
//! | `verified` | a list of [`Stamp`]s — who attested it and when |
//! | `sources` | a list of [`Source`]s — where the content came from |
//!
//! An [`Actor`] is always prefix-qualified, so a human signature can never be
//! confused with a process stamp: `human:<npub>`, `process:<name>/<version>`,
//! `agent:<producer>/<version>`, and, from vocabulary version 3 (ADR-VG-011),
//! `did:nostr:<64 lowercase hex>` — the principal a stamp's `by` names.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_norway::Value as Yaml;

use crate::input::frontmatter::{Frontmatter, Wikilink};

/// The lifecycle state of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Authored or machine-proposed, not yet attested.
    Draft,
    /// Attested and current.
    Stable,
    /// Withdrawn; kept for provenance, never served as current.
    Deprecated,
}

impl Status {
    /// The frontmatter spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Stable => "stable",
            Self::Deprecated => "deprecated",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "stable" => Ok(Self::Stable),
            "deprecated" => Ok(Self::Deprecated),
            other => Err(format!(
                "unknown status {other:?}; expected draft, stable or deprecated"
            )),
        }
    }
}

/// Who did something: a person, a principal key, a deterministic process, or
/// an agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Actor {
    /// A person, identified by their Nostr `npub`.
    Human(String),
    /// A principal key: the 64 lowercase hex characters of a `did:nostr:` DID
    /// (ADR-VG-011 §1). Whether the principal is a person is a roster fact,
    /// not a spelling one, so [`Actor::is_human`] is `false` for it.
    Did(String),
    /// A deterministic program: name and version.
    Process {
        /// The program's name, e.g. `vault`.
        name: String,
        /// Its version, e.g. `1.0`.
        version: String,
    },
    /// A model-driven agent: producer and version.
    Agent {
        /// The producing system, e.g. `visionclaw`.
        producer: String,
        /// Its version.
        version: String,
    },
}

impl Actor {
    /// The canonical frontmatter spelling.
    #[must_use]
    pub fn as_string(&self) -> String {
        match self {
            Self::Human(npub) => format!("human:{npub}"),
            Self::Did(hex) => format!("{}{hex}", crate::input::principal::DID_NOSTR),
            Self::Process { name, version } => format!("process:{name}/{version}"),
            Self::Agent { producer, version } => format!("agent:{producer}/{version}"),
        }
    }

    /// `true` when this actor is a person — the only kind whose attestation
    /// satisfies the human gate (decision Q6).
    #[must_use]
    pub fn is_human(&self) -> bool {
        matches!(self, Self::Human(_))
    }

    /// The principal key this actor names as `did:nostr:<hex>`, when it names
    /// one: a [`Actor::Did`] directly, a [`Actor::Human`] whose `npub` decodes
    /// (the conversion is lossless, ADR-VG-011 §1). `None` for agents,
    /// processes and a `human:` value that is not an `npub`.
    ///
    /// ```
    /// use ontomark_compile::input::okf::Actor;
    /// let npub: Actor = "human:npub16kv2h3hdalphuyzj8tkycq8cm9n54ymer23558ds80afkc9e7frqwrawzj".parse().unwrap();
    /// let did: Actor = "did:nostr:d598abc6edefc37e10523aec4c00f8d9674a93791aa34a1db03bfa9b60b9f246".parse().unwrap();
    /// assert_eq!(npub.principal_did(), did.principal_did());
    /// assert_eq!("human:jjohare".parse::<Actor>().unwrap().principal_did(), None);
    /// ```
    #[must_use]
    pub fn principal_did(&self) -> Option<String> {
        match self {
            Self::Did(hex) => Some(format!("{}{hex}", crate::input::principal::DID_NOSTR)),
            Self::Human(npub) => crate::input::principal::npub_to_hex(npub)
                .ok()
                .map(|hex| format!("{}{hex}", crate::input::principal::DID_NOSTR)),
            Self::Process { .. } | Self::Agent { .. } => None,
        }
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_string())
    }
}

impl From<Actor> for String {
    fn from(a: Actor) -> Self {
        a.as_string()
    }
}

impl TryFrom<String> for Actor {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl FromStr for Actor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if let Some(npub) = s.strip_prefix("human:") {
            if npub.is_empty() {
                return Err("human actor has no npub".into());
            }
            return Ok(Self::Human(npub.to_owned()));
        }
        let split_versioned = |rest: &str, kind: &str| -> Result<(String, String), String> {
            rest.rsplit_once('/')
                .filter(|(n, v)| !n.is_empty() && !v.is_empty())
                .map(|(n, v)| (n.to_owned(), v.to_owned()))
                .ok_or_else(|| format!("{kind} actor {rest:?} must be <name>/<version>"))
        };
        if let Some(rest) = s.strip_prefix("did:") {
            let Some(body) = rest.strip_prefix("nostr:") else {
                return Err(format!("DID method of {s:?} is not nostr"));
            };
            if !crate::input::principal::is_hex64(body) {
                return Err(format!(
                    "{s:?} must be did:nostr: followed by 64 lowercase hex characters"
                ));
            }
            return Ok(Self::Did(body.to_owned()));
        }
        if let Some(rest) = s.strip_prefix("process:") {
            let (name, version) = split_versioned(rest, "process")?;
            return Ok(Self::Process { name, version });
        }
        if let Some(rest) = s.strip_prefix("agent:") {
            let (producer, version) = split_versioned(rest, "agent")?;
            return Ok(Self::Agent { producer, version });
        }
        Err(format!(
            "actor {s:?} must start with human:, did:nostr:, process: or agent:"
        ))
    }
}

/// A `{ by: <actor>, at: <datetime> }` pair — the shape of both `generated`
/// and each entry of `verified`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    /// Who.
    pub by: Actor,
    /// When, as an ISO-8601 instant.
    pub at: String,
    /// Review scope, content binding and future metadata preserved verbatim.
    #[serde(flatten)]
    pub extensions: std::collections::BTreeMap<String, Yaml>,
}

impl Stamp {
    /// Build a stamp.
    #[must_use]
    pub fn new(by: Actor, at: impl Into<String>) -> Self {
        Self {
            by,
            at: at.into(),
            extensions: Default::default(),
        }
    }

    /// The instrument that did the work (`via`, ADR-VG-011 §1), when the stamp
    /// names one and it parses as an `agent:` or `process:` actor.
    #[must_use]
    pub fn via(&self) -> Option<Actor> {
        self.extensions
            .get("via")
            .and_then(Yaml::as_str)
            .and_then(|s| s.parse().ok())
    }

    /// Render as an inline YAML mapping value.
    #[must_use]
    pub fn to_yaml(&self) -> Yaml {
        let mut m = serde_norway::Mapping::new();
        m.insert(Yaml::String("by".into()), Yaml::String(self.by.as_string()));
        m.insert(Yaml::String("at".into()), Yaml::String(self.at.clone()));
        for (key, value) in &self.extensions {
            if key != "by" && key != "at" {
                m.insert(Yaml::String(key.clone()), value.clone());
            }
        }
        Yaml::Mapping(m)
    }

    /// Read a stamp from a YAML mapping, returning `None` when either field is
    /// missing or the actor prefix is unrecognised.
    #[must_use]
    pub fn from_yaml(v: &Yaml) -> Option<Self> {
        let m = v.as_mapping()?;
        let by = m.get(Yaml::String("by".into()))?.as_str()?.parse().ok()?;
        let at = m.get(Yaml::String("at".into()))?.as_str()?.to_owned();
        let extensions = m
            .iter()
            .filter_map(|(key, value)| {
                let key = key.as_str()?;
                (!matches!(key, "by" | "at")).then(|| (key.to_owned(), value.clone()))
            })
            .collect();
        Some(Self { by, at, extensions })
    }
}

/// A `sources:` entry — the provenance link that absorbed `elevatedFrom`.
///
/// Version 3 (ADR-VG-010 §5) adds the optional [`rel`](Self::rel),
/// [`at`](Self::at) and [`locator`](Self::locator). Any other field is kept in
/// [`extensions`](Self::extensions) and written back by
/// [`to_yaml`](Self::to_yaml), so a read-modify-write never drops a field this
/// type does not model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// A short local identifier, e.g. `origin`.
    pub id: String,
    /// The source, usually a wikilink into `working/`.
    pub resource: String,
    /// The relation type (`origin`, `episode`, `evidence`, `derived-from`,
    /// `standard`, `reference`), as written. Absent reads as `reference`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel: Option<String>,
    /// The claim or publication date, as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// What inside the target is cited, as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// Every other field, preserved verbatim.
    #[serde(flatten)]
    pub extensions: std::collections::BTreeMap<String, Yaml>,
}

impl Source {
    /// Build a source with the two original fields and nothing else.
    #[must_use]
    pub fn new(id: impl Into<String>, resource: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            resource: resource.into(),
            rel: None,
            at: None,
            locator: None,
            extensions: Default::default(),
        }
    }

    /// The wikilink target, when `resource` is a `[[…]]` reference.
    #[must_use]
    pub fn link(&self) -> Option<Wikilink> {
        Wikilink::parse(&self.resource)
    }

    /// Render as a YAML mapping: `id`, `resource`, then `rel`, `at`,
    /// `locator` when present, then every preserved extension.
    #[must_use]
    pub fn to_yaml(&self) -> Yaml {
        let mut m = serde_norway::Mapping::new();
        let mut put = |key: &str, value: &str| {
            m.insert(Yaml::String(key.into()), Yaml::String(value.to_owned()));
        };
        put("id", &self.id);
        put("resource", &self.resource);
        for (key, value) in [
            ("rel", &self.rel),
            ("at", &self.at),
            ("locator", &self.locator),
        ] {
            if let Some(value) = value {
                put(key, value);
            }
        }
        for (key, value) in &self.extensions {
            if !matches!(key.as_str(), "id" | "resource" | "rel" | "at" | "locator") {
                m.insert(Yaml::String(key.clone()), value.clone());
            }
        }
        Yaml::Mapping(m)
    }

    /// Read a source from a YAML mapping. Needs `id` and `resource` as
    /// strings; every other field is kept.
    #[must_use]
    pub fn from_yaml(v: &Yaml) -> Option<Self> {
        let m = v.as_mapping()?;
        let text = |key: &str| m.get(Yaml::String(key.into())).and_then(scalar_text);
        let extensions = m
            .iter()
            .filter_map(|(key, value)| {
                let key = key.as_str()?;
                (!matches!(key, "id" | "resource" | "rel" | "at" | "locator"))
                    .then(|| (key.to_owned(), value.clone()))
            })
            .collect();
        Some(Self {
            id: m.get(Yaml::String("id".into()))?.as_str()?.to_owned(),
            resource: m.get(Yaml::String("resource".into()))?.as_str()?.to_owned(),
            rel: text("rel"),
            at: text("at"),
            locator: text("locator"),
            extensions,
        })
    }
}

/// A YAML scalar rendered as text (strings, numbers and booleans).
fn scalar_text(v: &Yaml) -> Option<String> {
    match v {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Number(n) => Some(n.to_string()),
        Yaml::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Field-level certification read from `verified` stamps (ADR-OM-012).
///
/// A human stamp (`by: human:<npub>` with an `at`) certifies the whole page
/// when it lists no `keys`, and only the keys it lists otherwise. Agent and
/// process stamps certify nothing. `ontomark-ingest`'s `Certification`
/// applies the same rule and never patches a certified key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Certification {
    /// A human stamp without `keys` covers the whole page.
    pub whole_page: bool,
    /// Keys named by human stamps that list `keys`, sorted and de-duplicated.
    pub keys: Vec<String>,
    /// Current content-bound human review stamps.
    #[serde(default)]
    pub current: usize,
    /// Changed or malformed content-bound human review stamps.
    #[serde(default)]
    pub stale: usize,
    /// Historical human review stamps lacking a content binding.
    #[serde(default)]
    pub unbound: usize,
}

impl Certification {
    /// Read the certification out of a frontmatter `verified` value.
    #[must_use]
    pub fn from_verified(verified: Option<&Yaml>) -> Self {
        let mut out = Self::default();
        let Some(stamps) = verified.and_then(Yaml::as_sequence) else {
            return out;
        };
        let mut keys = std::collections::BTreeSet::new();
        for v in stamps {
            let Some(stamp) = Stamp::from_yaml(v) else {
                continue;
            };
            if !stamp.by.is_human() || stamp.at.trim().is_empty() {
                continue;
            }
            match v
                .as_mapping()
                .and_then(|m| m.get(Yaml::String("keys".into())))
            {
                None | Some(Yaml::Null) => out.whole_page = true,
                Some(list) => keys.extend(
                    list.as_sequence()
                        .into_iter()
                        .flatten()
                        .filter_map(Yaml::as_str)
                        .map(|k| k.trim().to_owned())
                        .filter(|k| !k.is_empty()),
                ),
            }
        }
        out.keys = keys.into_iter().collect();
        out
    }

    /// Evaluate binding freshness against the current page (ADR-2024).
    #[must_use]
    pub fn for_page(page: &crate::input::page::Page) -> Self {
        let doc = ontomark_parse::Document::parse(&page.source);
        let assessment = ontomark_parse::certification::assess(&doc, "verified");
        Self {
            whole_page: assessment.whole_page,
            keys: assessment.keys.into_iter().collect(),
            current: assessment.current,
            stale: assessment.stale,
            unbound: assessment.unbound,
        }
    }

    /// `certified` (whole page), `partial` (some keys) or `pending` (none).
    #[must_use]
    pub fn state(&self) -> &'static str {
        if self.whole_page {
            "certified"
        } else if self.keys.is_empty() {
            "pending"
        } else {
            "partial"
        }
    }
}

/// The OKF block projected out of a page's frontmatter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OkfBlock {
    /// `type` — the OKF type name, e.g. `Class` or `Note`.
    pub type_name: Option<String>,
    /// `resource` — the stable IRI.
    pub resource: Option<String>,
    /// `status`, when it parses.
    pub status: Option<Status>,
    /// The raw `status` text, kept so validation can complain precisely.
    pub status_raw: Option<String>,
    /// `stale_after`, as written.
    pub stale_after: Option<String>,
    /// `generated`, when it parses.
    pub generated: Option<Stamp>,
    /// `verified` — zero or more attestations.
    pub verified: Vec<Stamp>,
    /// `sources` — zero or more provenance links.
    pub sources: Vec<Source>,
}

impl OkfBlock {
    /// Project the OKF keys out of a frontmatter mapping. Never fails: an
    /// unparseable value simply does not appear in the typed field, which is
    /// what lets `vault validate` report it rather than crash on it.
    #[must_use]
    pub fn from_frontmatter(fm: &Frontmatter) -> Self {
        let seq = |key: &str| -> Vec<Yaml> {
            match fm.get(key) {
                Some(Yaml::Sequence(items)) => items.clone(),
                Some(other) => vec![other.clone()],
                None => Vec::new(),
            }
        };
        let status_raw = fm.text("status");
        Self {
            type_name: fm.text("type"),
            resource: fm.text("resource"),
            status: status_raw.as_deref().and_then(|s| s.parse().ok()),
            status_raw,
            stale_after: fm.text("stale_after"),
            generated: fm.get("generated").and_then(Stamp::from_yaml),
            verified: seq("verified")
                .iter()
                .filter_map(Stamp::from_yaml)
                .collect(),
            sources: seq("sources")
                .iter()
                .filter_map(Source::from_yaml)
                .collect(),
        }
    }

    /// `true` when at least one `verified` entry was signed by a human — the
    /// predicate the promotion state machine calls `human-reviewed`.
    #[must_use]
    pub fn is_human_verified(&self) -> bool {
        self.verified.iter().any(|s| s.by.is_human())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_round_trips_every_prefix() {
        for s in [
            "human:npub1abc",
            "process:vault/1.0",
            "agent:visionclaw/2.3",
        ] {
            let a: Actor = s.parse().unwrap();
            assert_eq!(a.as_string(), s);
        }
    }

    #[test]
    fn a_did_nostr_actor_needs_a_64_hex_body() {
        let hex = "b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";
        let a: Actor = format!("did:nostr:{hex}").parse().unwrap();
        assert_eq!(a, Actor::Did(hex.into()));
        assert_eq!(a.as_string(), format!("did:nostr:{hex}"));
        assert!(!a.is_human(), "humanness of a DID is a roster fact");
        for bad in [
            "did:nostr:not-hex",
            "did:nostr:",
            "did:nostr:B41654017F6850B13857D19D8AE0E3F88F1365600CAB0321E8101C9E92682F7A",
            "did:nostr:b416",
            "did:web:example.org",
            "did:nostr:ontology-mesh",
        ] {
            assert!(bad.parse::<Actor>().is_err(), "{bad}");
        }
    }

    #[test]
    fn a_stamp_carries_via_and_keeps_unknown_fields() {
        let y = serde_norway::from_str::<Yaml>(
            "{ by: 'did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a', via: 'agent:enrichment-swarm/6', at: '2026-09-22T12:41:39Z', process: 'process:vault-migrate/1.0', receipt: abc }",
        )
        .unwrap();
        let stamp = Stamp::from_yaml(&y).unwrap();
        assert_eq!(stamp.via().unwrap().as_string(), "agent:enrichment-swarm/6");
        let back = stamp.to_yaml();
        assert_eq!(
            back.get("process").and_then(Yaml::as_str),
            Some("process:vault-migrate/1.0")
        );
        assert_eq!(back.get("receipt").and_then(Yaml::as_str), Some("abc"));
        assert_eq!(Stamp::from_yaml(&back).unwrap(), stamp);
    }

    #[test]
    fn a_bare_actor_is_rejected() {
        assert!("vault/1.0".parse::<Actor>().is_err());
        assert!("process:novversion".parse::<Actor>().is_err());
        assert!("human:".parse::<Actor>().is_err());
    }

    #[test]
    fn only_human_actors_satisfy_the_gate() {
        assert!(Actor::Human("npub1".into()).is_human());
        assert!(!Actor::Process {
            name: "vault".into(),
            version: "1.0".into()
        }
        .is_human());
    }

    #[test]
    fn projects_the_okf_block_out_of_frontmatter() {
        let fm = Frontmatter::parse(concat!(
            "type: Class\n",
            "resource: urn:ngm:class:knowledge-graph\n",
            "status: stable\n",
            "stale_after: 2026-10-06\n",
            "generated: { by: process:vault/1.0, at: 2026-09-22T00:00:00Z }\n",
            "verified:\n",
            "  - { by: human:npub1abc, at: 2026-09-22T00:00:00Z }\n",
            "sources:\n",
            "  - { id: origin, resource: \"[[working/Notes]]\" }\n",
        ))
        .unwrap();
        let okf = OkfBlock::from_frontmatter(&fm);
        assert_eq!(okf.status, Some(Status::Stable));
        assert_eq!(okf.type_name.as_deref(), Some("Class"));
        assert!(okf.is_human_verified());
        assert_eq!(okf.sources[0].link().unwrap().target, "working/Notes");
        assert_eq!(okf.generated.unwrap().by.as_string(), "process:vault/1.0");
    }

    #[test]
    fn an_unparseable_status_is_kept_raw_for_reporting() {
        let fm = Frontmatter::parse("status: half-baked\n").unwrap();
        let okf = OkfBlock::from_frontmatter(&fm);
        assert!(okf.status.is_none());
        assert_eq!(okf.status_raw.as_deref(), Some("half-baked"));
    }

    #[test]
    fn certification_counts_only_dated_human_stamps() {
        let y = |s: &str| serde_norway::from_str::<Yaml>(s).unwrap();
        let c = |s: &str| Certification::from_verified(Some(&y(s)));
        assert_eq!(Certification::from_verified(None).state(), "pending");
        assert_eq!(
            c("[{by: 'agent:x/1', at: '2026-11-04'}]").state(),
            "pending"
        );
        assert_eq!(c("[{by: 'human:npub1'}]").state(), "pending");
        assert_eq!(
            c("[{by: 'human:npub1', at: '2026-11-04'}]").state(),
            "certified"
        );
        let partial = c("[{by: 'human:npub1', at: '2026-11-04', keys: [requires, is-a, is-a]}]");
        assert_eq!(partial.state(), "partial");
        assert_eq!(partial.keys, ["is-a", "requires"]);
    }

    #[test]
    fn stamp_yaml_round_trips() {
        let s = Stamp::new(Actor::Human("npub1".into()), "2026-09-22T00:00:00Z");
        assert_eq!(Stamp::from_yaml(&s.to_yaml()).unwrap(), s);
    }
}
