//! The two configuration files `m0002-vocabulary-v3` edits: the vocabulary
//! and the attribution roster. Both are edited as text, so their comments and
//! layout survive; every edit names the text it expects, and a file that is
//! not in the expected shape is refused rather than guessed at.

use super::CANONICAL_PRINCIPAL;
use crate::migrate::Transform;

const MIGRATED_MARK: &str = "\nversion: 3\n";

/// `(what, anchor, replacement)` for every edit to `ontology/vocabulary.yaml`.
/// Raw strings, because a continuation backslash would eat the indentation
/// the anchors depend on.
const VOCABULARY_EDITS: &[(&str, &str, &str)] = &[
    (
        "the version",
        r#"version: 2
vocabulary_id: "urn:ngm:vocabulary:2"
"#,
        r#"version: 3
vocabulary_id: "urn:ngm:vocabulary:3"
"#,
    ),
    (
        "the v2 history comment",
        "# forum 31403 signature this Schema-level change would otherwise need.\n",
        r"# forum 31403 signature this Schema-level change would otherwise need.
# v3 (ADR-VG-010 to ADR-VG-013): stamps name a principal and the instrument it
# directed, pages carry time and supersession, sources are typed, `working/`
# has the memory kinds, `visibility` and `page_resource`. Applied to the pages
# by `ontomark migrate m0002-vocabulary-v3`.
",
    ),
    (
        "the JSON-LD context",
        "context: \"https://narrativegoldmine.com/context/v2.jsonld\"\n",
        "context: \"https://narrativegoldmine.com/context/v4.jsonld\"\n",
    ),
    (
        "the working types",
        "working_types: [Note, Episode, Transcript, Draft Concept, Journal, Canvas]\n",
        r"working_types:
  Note: {}
  Episode: { memory: true }
  Brief: {}
  Proposal: {}
  Belief: { memory: true }
  Procedure: { memory: true }
  Transcript: {}
  Journal: {}
  Canvas: {}
",
    ),
    (
        "the working extensions",
        r"  source:           { type: text }
  topic:            { type: text }
  episodes:         { type: number }
  assertions:       { type: number }
  promotion-status: { type: text, enum: [candidate, candidate_rejected, elevated] }
",
        r"  topic:            { type: text }
  promotion-status: { type: text, enum: [candidate, candidate_deferred, candidate_rejected, elevated] }
",
    ),
    (
        "the source item fields",
        "      resource: text      # a wikilink, a URL, or an IRI\n",
        r"      resource: text      # a wikilink, a URL, or an IRI
      rel: text           # origin | episode | evidence | derived-from | standard | reference (default)
      at: datetime        # claim or publication date
      locator: text       # fp:<assertion-fp> or sha256-v1:<digest>
",
    ),
    (
        "the did actor comment",
        r#"    did:     "did:<method>:<id>"
    # `did:` is an ADDITIVE extension of contract C1, because it is what the
    # corpus actually carries: did:nostr:ontology-mesh 3,819,
    # did:nostr:lcr-swarm 1,575, did:nostr:jjohare 1,077,
    # did:nostr:enrichment-swarm 225, did:nostr:visionclaw-server 1.
    # A DID is a stronger actor identifier than either alternative, and
    # did:nostr:jjohare is the identity the forum signing loop matches.
"#,
        r#"    did:     "did:nostr:<64 lowercase hex>"
    # Under v3 `by` is always a principal, a did:nostr key the roster resolves
    # (ADR-VG-011); `via` names the instrument it directed, an agent:<name>/<version>
    # or process:<name>/<version> actor.
"#,
    ),
];

/// `ontology/vocabulary.yaml`, version 2 to 3.
pub(crate) fn vocabulary(source: &str) -> Transform {
    if source.contains(MIGRATED_MARK) || source.starts_with("version: 3\n") {
        return Transform::Unchanged;
    }
    let mut out = source.to_owned();
    for (what, anchor, replacement) in VOCABULARY_EDITS {
        if out.matches(anchor).count() != 1 {
            return Transform::Refused(format!(
                "not in the expected vocabulary 2 shape: cannot find {what} exactly once"
            ));
        }
        out = out.replacen(anchor, replacement, 1);
    }
    Transform::Changed {
        source: out,
        summary: "vocabulary".to_owned(),
    }
}

/// `ontology/attribution.yaml`: a `principal:` on every generator and the
/// principal registry.
pub(crate) fn roster(source: &str) -> Transform {
    if source.lines().any(|l| l.starts_with("principals:")) {
        return Transform::Unchanged;
    }
    let parsed: serde_norway::Value = match serde_norway::from_str(source) {
        Ok(v) => v,
        Err(e) => return Transform::Refused(format!("does not parse: {e}")),
    };
    let generators = parsed
        .get("generators")
        .and_then(serde_norway::Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let text = |g: &serde_norway::Value, key: &str| {
        g.get(key)
            .and_then(serde_norway::Value::as_str)
            .map(str::to_owned)
    };
    let humans: Vec<(String, String)> = generators
        .iter()
        .filter(|g| text(g, "kind").as_deref() == Some("human"))
        .filter_map(|g| Some((text(g, "did")?, text(g, "label").unwrap_or_default())))
        .collect();
    let Some((_, label)) = humans.first() else {
        return Transform::Refused(
            "no generator of `kind: human` to name the principal".to_owned(),
        );
    };
    // One `principal:` line after each generator's `kind:` line.
    let mut out = String::with_capacity(source.len() + 1024);
    let mut inserted = 0usize;
    for line in source.split_inclusive('\n') {
        out.push_str(line);
        let body = line.trim_end_matches(['\n', '\r']);
        if body.starts_with("    kind: ") {
            let eol = &line[body.len()..];
            out.push_str(&format!("    principal: {CANONICAL_PRINCIPAL}{eol}"));
            inserted += 1;
        }
    }
    if inserted != generators.len() {
        return Transform::Refused(format!(
            "expected one `    kind:` line per generator ({}), found {inserted}",
            generators.len()
        ));
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    // One line per push: a string continuation would eat the indentation.
    for line in [
        String::new(),
        "# The principal registry (ADR-VG-011 §2, ADR-VG-012 §1): one canonical key per".to_owned(),
        "# person, other keys resolve to it. `kind` says who is human, which decides who"
            .to_owned(),
        "# may sign a `verified` entry. Each generator above names its principal.".to_owned(),
        "principals:".to_owned(),
        format!("  - did: {CANONICAL_PRINCIPAL}"),
        format!("    label: {label}"),
        "    kind: human".to_owned(),
        "    also:".to_owned(),
    ] {
        out.push_str(&line);
        out.push('\n');
    }
    for (did, _) in humans.iter().filter(|(did, _)| did != CANONICAL_PRINCIPAL) {
        out.push_str(&format!("      - {did}\n"));
    }
    Transform::Changed {
        source: out,
        summary: "roster".to_owned(),
    }
}
