//! `create --suggest`: the CLI side of the Jev option for new concepts
//! (VG-01-S1-02). Advisory only: the result is printed beside the create
//! outcome and never reaches the page that is written.

use ontomark_compile::input::page::Vault;
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::graph::VaultGraph;
use ontomark_jev::{
    CreateQuestion, CreateSuggestion, JevClient, JevConfig, JevCreateSuggester, ParentCandidate,
    UreqTransport,
};
use serde_json::{json, Value};

/// Candidate parents retrieved for the title.
const PARENTS: usize = 8;

/// The text after the frontmatter block.
fn body_of(staged: &str) -> &str {
    staged
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(_, body)| body))
        .unwrap_or("")
}

/// Retrieve candidate parents deterministically: the title as a phrase, then
/// each distinct word of the title and the first 40 words of the body, merged
/// by page id with the best score kept.
fn candidate_parents(graph: &VaultGraph, title: &str, body: &str) -> Vec<ParentCandidate> {
    let mut queries = vec![title.to_owned()];
    let words = title
        .split_whitespace()
        .chain(body.split_whitespace().take(40))
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_owned())
        .filter(|w| w.chars().count() >= 4);
    for w in words {
        if !queries.iter().any(|q| q.eq_ignore_ascii_case(&w)) {
            queries.push(w);
        }
    }
    let mut best: Vec<ontomark_compile::ir::graph::Hit> = Vec::new();
    for q in &queries {
        for h in graph.find(q, None, PARENTS, true) {
            match best.iter_mut().find(|b| b.id == h.id) {
                Some(b) if b.score >= h.score => {}
                Some(b) => *b = h,
                None => best.push(h),
            }
        }
    }
    ontomark_compile::ir::graph::sort_hits(&mut best);
    best.truncate(PARENTS);
    best.into_iter()
        .map(|h| ParentCandidate {
            id: h.id,
            title: h.title,
            node_type: h.node_type,
        })
        .collect()
}

/// Build the question from the vocabulary and the vault; no model involved.
pub fn question(staged: &str, title: &str, vault: &Vault, vocab: &Vocabulary) -> CreateQuestion {
    let graph = VaultGraph::build(vault, vocab);
    CreateQuestion {
        title: title.to_owned(),
        excerpt: body_of(staged).trim().to_owned(),
        domains: vocab
            .scalars
            .get("domain")
            .map(|d| d.roots.clone())
            .unwrap_or_default(),
        types: vocab.types.keys().cloned().collect(),
        parents: candidate_parents(&graph, title, body_of(staged)),
    }
}

/// Ask Jev, if (and only if) the option is on. `None` on every fallback.
pub fn suggest(
    staged: &str,
    title: &str,
    vault: &Vault,
    vocab: &Vocabulary,
) -> Option<CreateSuggestion> {
    let env = |k: &str| std::env::var(k).ok();
    let config = JevConfig::from_env(&env);
    if !config.enabled {
        return None;
    }
    let q = question(staged, title, vault, vocab);
    JevCreateSuggester(JevClient::new(config, UreqTransport))
        .suggest(&q)
        .ok()
}

/// The `jev_suggestion` JSON object.
pub fn to_json(s: &CreateSuggestion) -> Value {
    json!({
        "advisory": true,
        "domains": s.domains,
        "type": s.node_type,
        "parents": s.parents,
        "ambiguous": s.ambiguous,
    })
}

/// Prose output.
pub fn print(s: &CreateSuggestion) {
    println!("jev suggestion (advisory, nothing written):");
    println!("  domains: {}", s.domains.join(", "));
    println!("  type:    {}", s.node_type.as_deref().unwrap_or("-"));
    println!("  parents: {}", s.parents.join(", "));
    if s.ambiguous {
        println!("  uncertain: review before using");
    }
}
