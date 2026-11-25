//! The attribution roster: `ontology/attribution.yaml` maps each generating actor
//! (`human:<npub>`, `agent:<label>/<version>`, `process:<name>/<version>`) to the
//! `did:nostr` it publishes under, so every class can carry `prov:wasAttributedTo`.
//! Public keys only; absent roster means no per-class attribution is emitted.

use std::collections::HashMap;
use std::path::Path;

use crate::input::okf::Actor;

/// One roster entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generator {
    /// `did:nostr:<hex x-only pubkey>`.
    pub did: String,
    /// Short label, e.g. `enrichment-swarm`.
    pub label: String,
    /// bech32 `npub` of the same key, the spelling `human:` actors use.
    pub npub: Option<String>,
}

/// What a roster principal is. Only a [`PrincipalKind::Human`] may appear in
/// `verified` (ADR-VG-011 §4, ruling B8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    /// A person.
    Human,
    /// An organisation answerable for work done under its key.
    Organisation,
    /// Any other `kind:` the roster states, for example a `process` swarm key.
    Other,
}

impl PrincipalKind {
    fn parse(s: &str) -> Self {
        match s.trim() {
            "human" => Self::Human,
            "organisation" | "organization" => Self::Organisation,
            _ => Self::Other,
        }
    }
}

/// A principal in the registry (ADR-VG-011 §2-§3): one canonical DID, the
/// other keys that resolve to it, and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// The canonical `did:nostr:<hex>`.
    pub did: String,
    /// Short label, e.g. `jjohare`.
    pub label: String,
    /// What the principal is.
    pub kind: PrincipalKind,
    /// Other `did:nostr:` keys that resolve to this principal.
    pub also: Vec<String>,
}

/// A principal DID resolved through the roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPrincipal {
    /// The canonical DID the key resolves to.
    pub canonical: String,
    /// What the principal is.
    pub kind: PrincipalKind,
}

/// The loaded roster, keyed by every actor spelling that resolves to a generator.
#[derive(Debug, Default, Clone)]
pub struct Roster {
    by_actor: HashMap<String, Generator>,
    principals: Vec<Principal>,
    /// Generator DID to (stated `kind`, stated `principal`).
    generator_facts: HashMap<String, (Option<String>, Option<String>)>,
}

/// The roster file as typed data, for the principal registry. The flat reader
/// below keeps the v2 generator behaviour; this reads only what v3 adds.
#[derive(Debug, Default, serde::Deserialize)]
struct RosterFile {
    #[serde(default)]
    generators: Vec<RawEntry>,
    #[serde(default)]
    principals: Vec<RawEntry>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct RawEntry {
    #[serde(default)]
    did: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    principal: Option<String>,
    #[serde(default)]
    also: Vec<String>,
}

impl Roster {
    /// Load `ontology/attribution.yaml` beneath `repo_root`; an absent file is an empty roster.
    #[must_use]
    pub fn discover(repo_root: &Path) -> Self {
        let path = repo_root.join("ontology").join("attribution.yaml");
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::from_yaml_str(&text),
            Err(_) => Self::default(),
        }
    }

    /// Parse the flat roster. Tolerant of comments and ordering; each `- did:` starts an entry.
    #[must_use]
    pub fn from_yaml_str(text: &str) -> Self {
        let mut gens: Vec<Generator> = Vec::new();
        let mut cur: Option<(String, String, Option<String>, Option<String>)> = None; // did,label,npub,process
        let flush = |cur: &mut Option<(String, String, Option<String>, Option<String>)>,
                     gens: &mut Vec<Generator>,
                     keys: &mut Vec<(String, usize)>| {
            if let Some((did, label, npub, process)) = cur.take() {
                let idx = gens.len();
                gens.push(Generator {
                    did,
                    label: label.clone(),
                    npub: npub.clone(),
                });
                keys.push((format!("agent:{label}/0"), idx));
                if let Some(n) = npub {
                    keys.push((format!("human:{n}"), idx));
                }
                if let Some(p) = process {
                    keys.push((p, idx));
                }
                keys.push((label, idx));
            }
        };
        let mut keys: Vec<(String, usize)> = Vec::new();
        // The `principals:` registry (v3) shares the `- did:` spelling with
        // generators and is read separately, so the flat reader skips it.
        let mut in_principals = false;
        for raw in text.lines() {
            let line = raw.trim_end();
            let t = line.trim_start();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if !line.starts_with([' ', '-', '\t']) && t.ends_with(':') {
                in_principals = t == "principals:";
                continue;
            }
            if in_principals {
                continue;
            }
            if let Some(rest) = t.strip_prefix("- did:") {
                flush(&mut cur, &mut gens, &mut keys);
                cur = Some((rest.trim().to_owned(), String::new(), None, None));
                continue;
            }
            let Some((k, v)) = t.split_once(':') else {
                continue;
            };
            let v = v.trim().trim_matches('"').to_owned();
            if let Some(c) = cur.as_mut() {
                match k.trim() {
                    "label" => c.1 = v,
                    "npub" => c.2 = Some(v),
                    "process" => c.3 = Some(v),
                    _ => {}
                }
            }
        }
        flush(&mut cur, &mut gens, &mut keys);
        let mut by_actor = HashMap::new();
        for (k, idx) in keys {
            by_actor.insert(k, gens[idx].clone());
        }
        for generator in &gens {
            by_actor.insert(generator.did.clone(), generator.clone());
        }
        let file: RosterFile = serde_norway::from_str(text).unwrap_or_default();
        let principals = file
            .principals
            .into_iter()
            .filter(|p| !p.did.is_empty())
            .map(|p| Principal {
                did: p.did,
                label: p.label,
                kind: PrincipalKind::parse(p.kind.as_deref().unwrap_or("")),
                also: p.also,
            })
            .collect();
        let generator_facts = file
            .generators
            .into_iter()
            .map(|g| (g.did, (g.kind, g.principal)))
            .collect();
        Self {
            by_actor,
            principals,
            generator_facts,
        }
    }

    /// The registry principals, in file order.
    #[must_use]
    pub fn principals(&self) -> &[Principal] {
        &self.principals
    }

    /// Resolve a `did:nostr:` DID to its canonical principal and kind
    /// (ADR-VG-011 §3). A registry entry, canonical or listed under `also:`,
    /// wins and carries the principal's `kind`: only a principal's own keys do.
    /// A generator entry is the fallback: its stated `principal:` gives the
    /// canonical DID it acts for, but its kind is its own stated `kind:`, so a
    /// swarm, generator or process key is never human whoever it is filed
    /// under. `None` when the roster knows nothing of the key. Every humanness
    /// decision (`verified`, certification, the write engine's approver check,
    /// ingest's key list) goes through this function.
    #[must_use]
    pub fn resolve_principal(&self, did: &str) -> Option<ResolvedPrincipal> {
        if let Some(p) = self
            .principals
            .iter()
            .find(|p| p.did == did || p.also.iter().any(|a| a == did))
        {
            return Some(ResolvedPrincipal {
                canonical: p.did.clone(),
                kind: p.kind,
            });
        }
        // A generator's `principal:` gives only the canonical DID it acts
        // for. Its kind is its OWN: an instrument's key never counts as a
        // person, whoever it is filed under (B1).
        let (kind, principal) = self.generator_facts.get(did)?;
        let canonical = principal.clone().unwrap_or_else(|| did.to_owned());
        let kind = PrincipalKind::parse(kind.as_deref().unwrap_or(""));
        Some(ResolvedPrincipal { canonical, kind })
    }

    /// Whether `actor` is a person under the roster: a `did:nostr:` key the
    /// roster resolves to a principal of `kind: human` (canonical or listed
    /// under `also`, B1). An organisation, an unknown key, an agent and the
    /// retired `human:<npub or label>` spelling are not: that prefix is a
    /// vocabulary 2 rule, applied only where the vocabulary is 2
    /// ([`crate::input::okf::OkfBlock::is_human_verified_under`]), never as a
    /// roster fact.
    #[must_use]
    pub fn is_human_actor(&self, actor: &Actor) -> bool {
        match actor {
            Actor::Human(_) => false,
            Actor::Did(_) => self
                .resolve_principal(&actor.as_string())
                .is_some_and(|p| p.kind == PrincipalKind::Human),
            _ => false,
        }
    }

    /// Every `did:nostr:` key that names a person: each principal of
    /// `kind: human` and its `also` keys, in file order, then any generator
    /// whose own `kind: human` and which no principal lists. Exactly the keys
    /// for which [`Roster::is_human_by`] holds. The ingest crate
    /// (which cannot depend on this one) takes this list as
    /// `Options::human_dids`.
    #[must_use]
    pub fn human_dids(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .principals
            .iter()
            .filter(|p| p.kind == PrincipalKind::Human)
            .flat_map(|p| std::iter::once(p.did.clone()).chain(p.also.iter().cloned()))
            .collect();
        let mut own: Vec<&String> = self
            .generator_facts
            .iter()
            .filter(|(did, (kind, _))| {
                PrincipalKind::parse(kind.as_deref().unwrap_or("")) == PrincipalKind::Human
                    && self.is_unlisted(did)
            })
            .map(|(did, _)| did)
            .collect();
        own.sort();
        out.extend(own.into_iter().cloned());
        out
    }

    fn is_unlisted(&self, did: &str) -> bool {
        !self
            .principals
            .iter()
            .any(|p| p.did == did || p.also.iter().any(|a| a == did))
    }

    /// [`Roster::is_human_actor`] for a stamp's `by` text.
    #[must_use]
    pub fn is_human_by(&self, by: &str) -> bool {
        by.trim()
            .parse::<Actor>()
            .is_ok_and(|a| self.is_human_actor(&a))
    }

    /// The generator an actor resolves to. `agent:<label>/<any-version>` matches on label;
    /// `human:<npub>` on the npub; `process:<name>/<version>` on an explicit `process:` line.
    #[must_use]
    pub fn resolve(&self, actor: &Actor) -> Option<&Generator> {
        match actor {
            Actor::Human(npub) => self.by_actor.get(&format!("human:{npub}")),
            Actor::Did(_) => self.by_actor.get(&actor.as_string()),
            Actor::Agent { producer, .. } => self
                .by_actor
                .get(&format!("agent:{producer}/0"))
                .or_else(|| self.by_actor.get(producer)),
            Actor::Process { name, version } => {
                self.by_actor.get(&format!("process:{name}/{version}"))
            }
        }
    }

    /// Whether the roster holds any generator.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_actor.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROSTER: &str = "generators:\n  - did: did:nostr:aa\n    label: jjohare\n    npub: npub1x\n    kind: human\n  - did: did:nostr:bb\n    label: enrichment-swarm\n    kind: process\n  - did: did:nostr:cc\n    label: space-corpus-research\n    process: process:space-corpus-research/0.1.0\n";

    const REGISTRY: &str = "principals:
  - did: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a
    label: jjohare
    kind: human
    also:
      - did:nostr:d598abc6edefc37e10523aec4c00f8d9674a93791aa34a1db03bfa9b60b9f246
  - did: did:nostr:ea3d03d62a8eb3ce6364c0dd02f239a16fa22956a9f31626619f330361be9c00
    label: acme
    kind: organisation
generators:
  - did: did:nostr:94773a699927da1ef879b90843890b1fe0e671ae898163f4337dcbe5f96992c0
    label: enrichment-swarm
    kind: process
    principal: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a
";

    #[test]
    fn the_principals_block_is_invisible_to_the_flat_generator_reader() {
        let r = Roster::from_yaml_str(REGISTRY);
        let swarm: Actor = "agent:enrichment-swarm/6".parse().unwrap();
        assert_eq!(r.resolve(&swarm).unwrap().label, "enrichment-swarm");
        assert!(
            r.resolve(&"agent:jjohare/1".parse().unwrap()).is_none(),
            "a principal is not a generator"
        );
        assert_eq!(r.principals().len(), 2);
    }

    #[test]
    fn principals_resolve_through_also_and_carry_a_kind() {
        let r = Roster::from_yaml_str(REGISTRY);
        let canonical =
            "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";
        let via_also = r
            .resolve_principal(
                "did:nostr:d598abc6edefc37e10523aec4c00f8d9674a93791aa34a1db03bfa9b60b9f246",
            )
            .unwrap();
        assert_eq!(
            (via_also.canonical.as_str(), via_also.kind),
            (canonical, PrincipalKind::Human)
        );
        let org = r
            .resolve_principal(
                "did:nostr:ea3d03d62a8eb3ce6364c0dd02f239a16fa22956a9f31626619f330361be9c00",
            )
            .unwrap();
        assert_eq!(org.kind, PrincipalKind::Organisation);
        // A generator filed under a registry principal canonicalises to it but
        // keeps its OWN kind: an instrument's key is never human (B1).
        let swarm = r
            .resolve_principal(
                "did:nostr:94773a699927da1ef879b90843890b1fe0e671ae898163f4337dcbe5f96992c0",
            )
            .unwrap();
        assert_eq!(
            (swarm.canonical.as_str(), swarm.kind),
            (canonical, PrincipalKind::Other)
        );
        let swarm_actor: Actor =
            "did:nostr:94773a699927da1ef879b90843890b1fe0e671ae898163f4337dcbe5f96992c0"
                .parse()
                .unwrap();
        assert!(!r.is_human_actor(&swarm_actor));
        assert!(r.is_human_actor(
            &"did:nostr:d598abc6edefc37e10523aec4c00f8d9674a93791aa34a1db03bfa9b60b9f246"
                .parse()
                .unwrap()
        ));
        // `human_dids` agrees with `is_human_actor` on every key it can name.
        for did in r.human_dids() {
            assert!(r.is_human_by(&did), "{did}");
        }
        assert!(!r.human_dids().iter().any(|d| d.contains("94773a")));
        assert!(r
            .resolve_principal(
                "did:nostr:0000000000000000000000000000000000000000000000000000000000000000"
            )
            .is_none());
    }

    #[test]
    fn a_v2_roster_resolves_generators_by_their_stated_kind() {
        let r = Roster::from_yaml_str("generators:\n  - did: did:nostr:aa\n    label: jjohare\n    kind: human\n  - did: did:nostr:bb\n    label: swarm\n    kind: process\n");
        assert_eq!(r.human_dids(), vec!["did:nostr:aa".to_owned()]);
        assert_eq!(
            r.resolve_principal("did:nostr:aa").unwrap().kind,
            PrincipalKind::Human
        );
        assert_eq!(
            r.resolve_principal("did:nostr:bb").unwrap().kind,
            PrincipalKind::Other
        );
        assert!(r.principals().is_empty());
    }

    #[test]
    fn a_did_actor_resolves_to_its_generator() {
        let did = "did:nostr:94773a699927da1ef879b90843890b1fe0e671ae898163f4337dcbe5f96992c0";
        let r = Roster::from_yaml_str(REGISTRY);
        assert_eq!(
            r.resolve(&did.parse().unwrap()).unwrap().label,
            "enrichment-swarm"
        );
    }

    #[test]
    fn resolves_each_actor_spelling() {
        let r = Roster::from_yaml_str(ROSTER);
        assert_eq!(
            r.resolve(&"human:npub1x".parse().unwrap()).unwrap().did,
            "did:nostr:aa"
        );
        assert_eq!(
            r.resolve(&"agent:enrichment-swarm/1".parse().unwrap())
                .unwrap()
                .did,
            "did:nostr:bb"
        );
        assert_eq!(
            r.resolve(&"process:space-corpus-research/0.1.0".parse().unwrap())
                .unwrap()
                .did,
            "did:nostr:cc"
        );
        assert!(r
            .resolve(&"process:vault-migrate/1.0".parse().unwrap())
            .is_none());
    }
}
