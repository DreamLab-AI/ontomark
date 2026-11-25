#![allow(missing_docs)]
//! Shared fixtures: an in-memory vault, a vocabulary, a roster.
#![allow(dead_code)]

use ontomark_compile::build::attribution::Roster;
use ontomark_compile::input::page::{Page, Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::profile::Profile;
use ontomark_write::{Engine, MemStore, Plan, Writer};

pub const HUMAN: &str =
    "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";
pub const HUMAN_ALSO: &str =
    "did:nostr:d598abc6edefc37e10523aec4c00f8d9674a93791aa34a1db03bfa9b60b9f246";
pub const ORG: &str = "did:nostr:ea3d03d62a8eb3ce6364c0dd02f239a16fa22956a9f31626619f330361be9c00";
/// A process swarm's own key, filed under the human principal in the roster.
pub const SWARM: &str =
    "did:nostr:94773a699927da1ef879b90843890b1fe0e671ae898163f4337dcbe5f96992c0";
pub const AGENT: &str = "agent:claude/1";
pub const T0: &str = "2026-11-01T00:00:00Z";
pub const T1: &str = "2026-11-10T09:00:00Z";
pub const T2: &str = "2026-11-20T12:00:00Z";

pub const VOCAB_V3: &str = "version: 3
namespace: \"urn:ngm:class:\"
types:
  Class: { owl: \"owl:Class\", required: [resource, status] }
working_types:
  Note: { status: [draft, final] }
  Episode: { memory: true }
  Brief: {}
  Proposal: {}
  Belief: { memory: true }
  Procedure: { memory: true }
relations:
  is-a: { owl: \"rdfs:subClassOf\" }
working_extensions:
  promotion-status: { type: text, enum: [candidate, candidate_deferred, candidate_rejected, elevated] }
";

pub const VOCAB_V2: &str = "version: 2
namespace: \"urn:ngm:class:\"
types:
  Class: { owl: \"owl:Class\", required: [resource, status] }
working_types: [Note, Belief, Brief]
relations:
  is-a: { owl: \"rdfs:subClassOf\" }
";

pub const ROSTER: &str = "principals:
  - did: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a
    label: jjohare
    kind: human
    also: [did:nostr:d598abc6edefc37e10523aec4c00f8d9674a93791aa34a1db03bfa9b60b9f246]
  - did: did:nostr:ea3d03d62a8eb3ce6364c0dd02f239a16fa22956a9f31626619f330361be9c00
    label: acme
    kind: organisation
generators:
  - did: did:nostr:94773a699927da1ef879b90843890b1fe0e671ae898163f4337dcbe5f96992c0
    label: enrichment-swarm
    kind: process
    principal: did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a
";

pub fn vault(kind: VaultKind, pages: &[(&str, &str)]) -> Vault {
    let root = match kind {
        VaultKind::Working => "/v/working",
        VaultKind::Knowledge => "/v/knowledge",
    };
    let mut v = Vault {
        root: root.into(),
        kind,
        pages: Vec::new(),
        journals: Vec::new(),
        skipped: Vec::new(),
    };
    for (id, text) in pages {
        let page = Page::parse(format!("{root}/{id}.md"), format!("{id}.md"), *id, text).unwrap();
        if id.starts_with("journals/") {
            v.journals.push(page);
        } else {
            v.pages.push(page);
        }
    }
    v
}

pub fn engine_with(
    vocab: &str,
    roster: bool,
    working: &[(&str, &str)],
    knowledge: &[(&str, &str)],
) -> Engine {
    Engine::new(
        Profile::default(),
        Vocabulary::from_yaml_str(vocab).unwrap(),
        roster.then(|| Roster::from_yaml_str(ROSTER)),
        vault(VaultKind::Working, working),
        (!knowledge.is_empty()).then(|| vault(VaultKind::Knowledge, knowledge)),
    )
}

pub fn engine(working: &[(&str, &str)]) -> Engine {
    engine_with(VOCAB_V3, true, working, &[])
}

pub fn human() -> Writer {
    Writer::new(HUMAN)
}

pub fn agent() -> Writer {
    Writer::new(HUMAN).via(AGENT)
}

/// A store holding the engine's current working pages.
pub fn store_of(working: &[(&str, &str)]) -> MemStore {
    let mut s = MemStore::default();
    for (id, text) in working {
        s.insert(VaultKind::Working, format!("{id}.md"), text);
    }
    s
}

/// A valid approved Belief.
pub fn belief(urn_tail: &str, status: &str, extra: &str) -> String {
    format!(
        "---\ntype: Belief\nstatus: {status}\npage_resource: urn:visionflow:page:{urn_tail}\nvisibility: shared\ngenerated: {{ by: {HUMAN}, at: '{T0}' }}\ncontent-updated: '{T0}'\nabout: 'https://example.org/thing'\nconfidence: 0.5\nsources:\n  - {{ id: a, resource: 'https://x.example', rel: evidence, locator: p1 }}\n{extra}---\nbody of {urn_tail}\n"
    )
}

pub fn text_of(plan: &Plan, i: usize) -> &str {
    &plan.writes[i].text
}

/// Apply every write of a plan to `pages` and return the new page list.
pub fn applied(pages: &[(String, String)], plan: &Plan) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = pages.to_vec();
    for w in &plan.writes {
        match out.iter_mut().find(|(id, _)| *id == w.id) {
            Some(slot) => slot.1 = w.text.clone(),
            None => out.push((w.id.clone(), w.text.clone())),
        }
    }
    out
}

pub fn refs(pages: &[(String, String)]) -> Vec<(&str, &str)> {
    pages
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect()
}
