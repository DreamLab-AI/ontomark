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
//! `agent:<producer>/<version>`.

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

/// Who did something: a human, a deterministic process, or an agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Actor {
    /// A person, identified by their Nostr `npub`.
    Human(String),
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
        if let Some(rest) = s.strip_prefix("process:") {
            let (name, version) = split_versioned(rest, "process")?;
            return Ok(Self::Process { name, version });
        }
        if let Some(rest) = s.strip_prefix("agent:") {
            let (producer, version) = split_versioned(rest, "agent")?;
            return Ok(Self::Agent { producer, version });
        }
        Err(format!(
            "actor {s:?} must start with human:, process: or agent:"
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
}

impl Stamp {
    /// Build a stamp.
    #[must_use]
    pub fn new(by: Actor, at: impl Into<String>) -> Self {
        Self { by, at: at.into() }
    }

    /// Render as an inline YAML mapping value.
    #[must_use]
    pub fn to_yaml(&self) -> Yaml {
        let mut m = serde_norway::Mapping::new();
        m.insert(Yaml::String("by".into()), Yaml::String(self.by.as_string()));
        m.insert(Yaml::String("at".into()), Yaml::String(self.at.clone()));
        Yaml::Mapping(m)
    }

    /// Read a stamp from a YAML mapping, returning `None` when either field is
    /// missing or the actor prefix is unrecognised.
    #[must_use]
    pub fn from_yaml(v: &Yaml) -> Option<Self> {
        let m = v.as_mapping()?;
        let by = m.get(Yaml::String("by".into()))?.as_str()?.parse().ok()?;
        let at = m.get(Yaml::String("at".into()))?.as_str()?.to_owned();
        Some(Self { by, at })
    }
}

/// A `sources:` entry — the provenance link that absorbed `elevatedFrom`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// A short local identifier, e.g. `origin`.
    pub id: String,
    /// The source, usually a wikilink into `working/`.
    pub resource: String,
}

impl Source {
    /// The wikilink target, when `resource` is a `[[…]]` reference.
    #[must_use]
    pub fn link(&self) -> Option<Wikilink> {
        Wikilink::parse(&self.resource)
    }

    /// Render as a YAML mapping.
    #[must_use]
    pub fn to_yaml(&self) -> Yaml {
        let mut m = serde_norway::Mapping::new();
        m.insert(Yaml::String("id".into()), Yaml::String(self.id.clone()));
        m.insert(
            Yaml::String("resource".into()),
            Yaml::String(self.resource.clone()),
        );
        Yaml::Mapping(m)
    }

    /// Read a source from a YAML mapping.
    #[must_use]
    pub fn from_yaml(v: &Yaml) -> Option<Self> {
        let m = v.as_mapping()?;
        Some(Self {
            id: m.get(Yaml::String("id".into()))?.as_str()?.to_owned(),
            resource: m.get(Yaml::String("resource".into()))?.as_str()?.to_owned(),
        })
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
