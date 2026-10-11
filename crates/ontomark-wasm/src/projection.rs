//! Documents → entities, asserted triples and EL axioms.
//!
//! This is the engine's document adapter (see [`crate::profile`]). Compiler
//! 0.1.3 provides a separate typed in-memory projection. Adopting it here
//! requires explicit profile mapping and native/WASM wire parity (EXP-10);
//! the released engine's identity defaults remain unchanged.
//!
//! Rules that hold whatever replaces this module:
//!
//! - Only frontmatter is read. Prose wikilinks are document links, never
//!   axioms (EXP-05, ADR-2016).
//! - A page is an entity only when its profile's `type` key names a kind.
//! - A relation target resolves to exactly one page, or the reference is
//!   reported as unresolved or ambiguous and produces nothing.
//! - Authored IRIs are kept byte-for-byte; a minted IRI is used only when the
//!   profile allows pages without one.

use std::collections::{BTreeMap, BTreeSet};

use ontomark_parse::{Document, FrontmatterState, Value};
use oxrdf::vocab::{rdf, rdfs, xsd};
use oxrdf::{Literal, NamedNode, NamedOrBlankNode, Term, Triple};
use serde::{Deserialize, Serialize};

use crate::profile::{Kind, Profile};
use crate::reason::{ClassExpr, ElAxiom};
use crate::text::{pct_encode_path, HostSpan, TripleSet};

/// OWL vocabulary used by the projection.
pub mod owl {
    /// `owl:Class`.
    pub const CLASS: &str = "http://www.w3.org/2002/07/owl#Class";
    /// `owl:NamedIndividual`.
    pub const NAMED_INDIVIDUAL: &str = "http://www.w3.org/2002/07/owl#NamedIndividual";
    /// `owl:ObjectProperty`.
    pub const OBJECT_PROPERTY: &str = "http://www.w3.org/2002/07/owl#ObjectProperty";
    /// `owl:equivalentClass`.
    pub const EQUIVALENT_CLASS: &str = "http://www.w3.org/2002/07/owl#equivalentClass";
    /// `owl:disjointWith`.
    pub const DISJOINT_WITH: &str = "http://www.w3.org/2002/07/owl#disjointWith";
    /// `owl:Nothing`.
    pub const NOTHING: &str = "http://www.w3.org/2002/07/owl#Nothing";
}

/// `skos:altLabel`.
pub const SKOS_ALT_LABEL: &str = "http://www.w3.org/2004/02/skos/core#altLabel";
/// Prefix of minted page IRIs (generic profile, page without an `iri`).
pub const PAGE_IRI_PREFIX: &str = "urn:ontomark:page:";
/// Prefix of predicates minted for frontmatter keys without a profile meaning.
pub const KEY_IRI_PREFIX: &str = "urn:ontomark:key:";
/// Predicate linking an entity to the vault path of its page.
pub const SOURCE_PATH: &str = "urn:ontomark:sourcePath";

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The page or reference was refused.
    Error,
    /// Something was ignored.
    Warning,
}

/// A problem found while projecting a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Vault path of the page.
    pub path: String,
    /// Stable machine code.
    pub code: String,
    /// Severity.
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
    /// Where in the page, when known.
    pub span: Option<HostSpan>,
}

/// One ontology entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// IRI, authored or minted.
    pub iri: String,
    /// `true` when the IRI was authored in the page.
    pub iri_authored: bool,
    /// Kind.
    pub kind: Kind,
    /// Vault path of the page.
    pub path: String,
    /// Display label.
    pub label: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Told superclasses (classes), types (individuals) or super-properties (properties).
    pub parents: Vec<String>,
}

/// The projection of a whole vault.
#[derive(Debug, Clone, Default)]
pub struct Compiled {
    /// Entities sorted by IRI.
    pub entities: Vec<Entity>,
    /// Asserted triples, deduplicated and sorted.
    pub triples: Vec<Triple>,
    /// EL axioms for the reasoner, deduplicated and sorted.
    pub axioms: Vec<ElAxiom>,
    /// Told `rdf:type` of individuals: individual IRI → class IRIs.
    pub individual_types: BTreeMap<String, BTreeSet<String>>,
    /// Diagnostics in path order.
    pub diagnostics: Vec<Diagnostic>,
}

impl Compiled {
    /// The entity with `iri`.
    #[must_use]
    pub fn entity(&self, iri: &str) -> Option<&Entity> {
        self.entities
            .binary_search_by(|e| e.iri.as_str().cmp(iri))
            .ok()
            .map(|i| &self.entities[i])
    }
}

/// Page identity: the vault path without the `.md` extension.
#[must_use]
pub fn page_id(path: &str) -> &str {
    path.strip_suffix(".md").unwrap_or(path)
}

fn stem(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

struct Head<'a> {
    path: &'a str,
    doc: &'a Document,
    kind: Kind,
    iri: String,
    iri_authored: bool,
    label: String,
    aliases: Vec<String>,
}

/// Project `docs` (vault path → document) under `profile`.
#[must_use]
pub fn compile(docs: &BTreeMap<String, Document>, profile: &Profile) -> Compiled {
    let mut out = Compiled::default();
    let mut heads: Vec<Head<'_>> = Vec::new();
    let mut names: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();

    // Pass 1: names for resolution (every page, entity or not) and entity heads.
    for (path, doc) in docs {
        let id = page_id(path);
        names.entry(id.to_lowercase()).or_default().insert(path);
        names
            .entry(stem(id).to_lowercase())
            .or_default()
            .insert(path);
        let Some(fm) = doc.frontmatter() else {
            continue;
        };
        if let Some(t) = doc.value(profile.label_key).and_then(Value::as_str) {
            names.entry(t.to_lowercase()).or_default().insert(path);
        }
        for alias in texts(doc.value(profile.alias_key)) {
            names.entry(alias.to_lowercase()).or_default().insert(path);
        }
        match &fm.state {
            FrontmatterState::Parsed => {}
            FrontmatterState::Unsupported(why) | FrontmatterState::Malformed(why) => {
                out.diagnostics.push(diag(
                    path,
                    doc,
                    "frontmatter_unreadable",
                    Severity::Error,
                    why.clone(),
                    Some((fm.content.start, fm.content.end)),
                ));
                continue;
            }
        }
        let Some(type_value) = doc.value(profile.type_key).and_then(Value::as_str) else {
            continue;
        };
        let Some(kind) = profile.kind_of(type_value) else {
            continue;
        };
        if let Some(gate) = profile.gate_key {
            if let Some(v) = doc.value(gate) {
                if v.as_bool().is_none() {
                    out.diagnostics.push(entry_diag(
                        path,
                        doc,
                        gate,
                        "gate_not_boolean",
                        format!("`{gate}` must be a YAML boolean"),
                    ));
                    continue;
                }
            }
        }
        let authored = doc
            .value(profile.iri_key)
            .and_then(Value::as_str)
            .map(str::trim);
        let (iri, iri_authored) = match authored {
            Some(s) if is_absolute_iri(s) => (s.to_owned(), true),
            Some(s) => {
                out.diagnostics.push(entry_diag(
                    path,
                    doc,
                    profile.iri_key,
                    "iri_invalid",
                    format!("`{}` is not an absolute IRI: {s}", profile.iri_key),
                ));
                continue;
            }
            None if profile.require_iri => {
                out.diagnostics.push(diag(
                    path,
                    doc,
                    "iri_missing",
                    Severity::Error,
                    format!("entity pages need `{}` under this profile", profile.iri_key),
                    None,
                ));
                continue;
            }
            None => (
                format!("{PAGE_IRI_PREFIX}{}", pct_encode_path(page_id(path))),
                false,
            ),
        };
        let label = doc
            .value(profile.label_key)
            .and_then(Value::as_str)
            .map_or_else(|| stem(page_id(path)).to_owned(), str::to_owned);
        heads.push(Head {
            path,
            doc,
            kind,
            iri,
            iri_authored,
            label,
            aliases: texts(doc.value(profile.alias_key)),
        });
    }

    // Duplicate IRIs: the first page in path order keeps it, later claimants are refused.
    let mut by_iri: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_path: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kept = Vec::new();
    for head in heads {
        if let Some(&first) = by_iri.get(&head.iri) {
            let owner: &Head<'_> = &kept[first];
            out.diagnostics.push(diag(
                head.path,
                head.doc,
                "duplicate_iri",
                Severity::Error,
                format!("IRI {} is already claimed by {}", head.iri, owner.path),
                None,
            ));
            continue;
        }
        by_iri.insert(head.iri.clone(), kept.len());
        by_path.insert(head.path, kept.len());
        kept.push(head);
    }

    // Pass 2: relations.
    let mut triples = TripleSet::default();
    let mut axioms = BTreeSet::new();
    for head in &kept {
        let s = NamedNode::new_unchecked(head.iri.clone());
        let type_iri = match head.kind {
            Kind::Class => owl::CLASS,
            Kind::Individual => owl::NAMED_INDIVIDUAL,
            Kind::Property => owl::OBJECT_PROPERTY,
        };
        triples.insert(triple(
            &s,
            rdf::TYPE.into_owned(),
            NamedNode::new_unchecked(type_iri),
        ));
        triples.insert(triple(
            &s,
            rdfs::LABEL.into_owned(),
            Literal::new_simple_literal(&head.label),
        ));
        triples.insert(triple(
            &s,
            NamedNode::new_unchecked(SOURCE_PATH),
            Literal::new_simple_literal(head.path),
        ));
        for alias in &head.aliases {
            triples.insert(triple(
                &s,
                NamedNode::new_unchecked(SKOS_ALT_LABEL),
                Literal::new_simple_literal(alias),
            ));
        }
        let mut parents = BTreeSet::new();
        let resolve = |key: &str, out: &mut Compiled| -> Vec<&Head<'_>> {
            resolve_key(head, key, &names, &by_path, &kept, out)
        };
        let Some(fm) = head.doc.frontmatter() else {
            continue;
        };
        for entry in &fm.entries {
            let key = entry.key.as_str();
            let p = profile;
            if p.subclass_keys.contains(&key)
                || (head.kind == Kind::Individual && p.instance_keys.contains(&key))
            {
                for t in resolve(key, &mut out) {
                    match (head.kind, t.kind) {
                        (Kind::Class, Kind::Class) => {
                            triples.insert(triple(
                                &s,
                                rdfs::SUB_CLASS_OF.into_owned(),
                                node(&t.iri),
                            ));
                            axioms.insert(ElAxiom::SubClassOf {
                                sub: ClassExpr::named(&head.iri),
                                sup: ClassExpr::named(&t.iri),
                            });
                            parents.insert(t.iri.clone());
                        }
                        (Kind::Individual, Kind::Class) => {
                            triples.insert(triple(&s, rdf::TYPE.into_owned(), node(&t.iri)));
                            out.individual_types
                                .entry(head.iri.clone())
                                .or_default()
                                .insert(t.iri.clone());
                            parents.insert(t.iri.clone());
                        }
                        (Kind::Property, Kind::Property) => {
                            triples.insert(triple(
                                &s,
                                rdfs::SUB_PROPERTY_OF.into_owned(),
                                node(&t.iri),
                            ));
                            axioms.insert(ElAxiom::SubPropertyOf {
                                sub: head.iri.clone(),
                                sup: t.iri.clone(),
                            });
                            parents.insert(t.iri.clone());
                        }
                        _ => out.diagnostics.push(kind_mismatch(head, key, t)),
                    }
                }
            } else if p.subproperty_keys.contains(&key) && head.kind == Kind::Property {
                for t in resolve(key, &mut out) {
                    if t.kind == Kind::Property {
                        triples.insert(triple(
                            &s,
                            rdfs::SUB_PROPERTY_OF.into_owned(),
                            node(&t.iri),
                        ));
                        axioms.insert(ElAxiom::SubPropertyOf {
                            sub: head.iri.clone(),
                            sup: t.iri.clone(),
                        });
                        parents.insert(t.iri.clone());
                    } else {
                        out.diagnostics.push(kind_mismatch(head, key, t));
                    }
                }
            } else if (p.equivalent_keys.contains(&key) || p.disjoint_keys.contains(&key))
                && head.kind == Kind::Class
            {
                let equivalent = p.equivalent_keys.contains(&key);
                for t in resolve(key, &mut out) {
                    if t.kind != Kind::Class {
                        out.diagnostics.push(kind_mismatch(head, key, t));
                        continue;
                    }
                    let members = vec![ClassExpr::named(&head.iri), ClassExpr::named(&t.iri)];
                    if equivalent {
                        triples.insert(triple(&s, node(owl::EQUIVALENT_CLASS), node(&t.iri)));
                        axioms.insert(ElAxiom::EquivalentClasses { members });
                    } else {
                        triples.insert(triple(&s, node(owl::DISJOINT_WITH), node(&t.iri)));
                        axioms.insert(ElAxiom::DisjointClasses { members });
                    }
                }
            } else if (p.domain_keys.contains(&key) || p.range_keys.contains(&key))
                && head.kind == Kind::Property
            {
                let domain = p.domain_keys.contains(&key);
                for t in resolve(key, &mut out) {
                    if t.kind != Kind::Class {
                        out.diagnostics.push(kind_mismatch(head, key, t));
                        continue;
                    }
                    let pred = if domain { rdfs::DOMAIN } else { rdfs::RANGE };
                    triples.insert(triple(&s, pred.into_owned(), node(&t.iri)));
                    let class = ClassExpr::named(&t.iri);
                    axioms.insert(if domain {
                        ElAxiom::Domain {
                            property: head.iri.clone(),
                            class,
                        }
                    } else {
                        ElAxiom::Range {
                            property: head.iri.clone(),
                            class,
                        }
                    });
                }
            } else if !p.consumes(key) {
                // Annotation: literals stay literals; wikilinks to entities become IRIs.
                let pred = node(&format!("{KEY_IRI_PREFIX}{}", pct_encode_path(key)));
                let links = entry.wikilinks(head.doc.source());
                if links.is_empty() {
                    for lit in literals(&entry.value) {
                        triples.insert(triple(&s, pred.clone(), lit));
                    }
                } else {
                    for link in links {
                        match lookup(&link.link.target, &names) {
                            Lookup::One(path) => match by_path.get(path) {
                                Some(&i) => {
                                    triples.insert(triple(&s, pred.clone(), node(&kept[i].iri)));
                                }
                                None => {
                                    triples.insert(triple(
                                        &s,
                                        pred.clone(),
                                        Literal::new_simple_literal(&link.link.target),
                                    ));
                                }
                            },
                            _ => {
                                triples.insert(triple(
                                    &s,
                                    pred.clone(),
                                    Literal::new_simple_literal(&link.link.target),
                                ));
                            }
                        }
                    }
                }
            }
        }
        out.entities.push(Entity {
            iri: head.iri.clone(),
            iri_authored: head.iri_authored,
            kind: head.kind,
            path: head.path.to_owned(),
            label: head.label.clone(),
            aliases: head.aliases.clone(),
            parents: parents.into_iter().collect(),
        });
    }
    out.entities.sort_by(|a, b| a.iri.cmp(&b.iri));
    out.triples = triples.into_vec();
    out.axioms = axioms.into_iter().collect();
    out.diagnostics
        .sort_by(|a, b| (&a.path, &a.code, &a.message).cmp(&(&b.path, &b.code, &b.message)));
    out
}

enum Lookup<'a> {
    None,
    One(&'a str),
    Many(Vec<&'a str>),
}

fn lookup<'a>(target: &str, names: &'a BTreeMap<String, BTreeSet<&'a str>>) -> Lookup<'a> {
    let key = page_id(target.trim()).to_lowercase();
    match names.get(&key) {
        None => Lookup::None,
        Some(set) if set.len() == 1 => Lookup::One(set.iter().next().copied().unwrap_or_default()),
        Some(set) => {
            // An exact path match wins over stem/title/alias matches.
            let exact: Vec<&str> = set
                .iter()
                .copied()
                .filter(|p| page_id(p).to_lowercase() == key)
                .collect();
            if exact.len() == 1 {
                Lookup::One(exact[0])
            } else {
                Lookup::Many(set.iter().copied().collect())
            }
        }
    }
}

fn resolve_key<'h>(
    head: &Head<'_>,
    key: &str,
    names: &BTreeMap<String, BTreeSet<&str>>,
    by_path: &BTreeMap<&str, usize>,
    kept: &'h [Head<'h>],
    out: &mut Compiled,
) -> Vec<&'h Head<'h>> {
    let Some(entry) = head.doc.frontmatter().and_then(|fm| fm.get(key)) else {
        return Vec::new();
    };
    let source = head.doc.source();
    let links = entry.wikilinks(source);
    let mut targets = Vec::new();
    if links.is_empty()
        && !matches!(entry.value, Value::Null)
        && entry.value.as_seq().is_none_or(|s| !s.is_empty())
    {
        out.diagnostics.push(diag(
            head.path,
            head.doc,
            "relation_not_wikilink",
            Severity::Warning,
            format!("`{key}` values must be [[wikilinks]]; nothing was asserted"),
            Some((entry.span.start, entry.span.end)),
        ));
    }
    for link in links {
        let at = link.span.map_or_else(
            || locate_link(source, entry.span, &link.link.target),
            |s| (s.start, s.end),
        );
        let at = Some(at);
        match lookup(&link.link.target, names) {
            Lookup::None => out.diagnostics.push(diag(
                head.path,
                head.doc,
                "unresolved_reference",
                Severity::Warning,
                format!("`{key}`: no page named [[{}]]", link.link.target),
                at,
            )),
            Lookup::Many(paths) => out.diagnostics.push(diag(
                head.path,
                head.doc,
                "ambiguous_reference",
                Severity::Warning,
                format!(
                    "`{key}`: [[{}]] matches {}",
                    link.link.target,
                    paths.join(", ")
                ),
                at,
            )),
            Lookup::One(path) => match by_path.get(path) {
                Some(&i) => targets.push(&kept[i]),
                None => out.diagnostics.push(diag(
                    head.path,
                    head.doc,
                    "target_not_entity",
                    Severity::Warning,
                    format!(
                        "`{key}`: [[{}]] is a page without an ontology type",
                        link.link.target
                    ),
                    at,
                )),
            },
        }
    }
    targets
}

/// Byte range of the `[[target…]]` token inside `within`, or `within` itself.
/// Flow sequences carry no per-item spans, so a diagnostic would otherwise
/// highlight the whole entry.
fn locate_link(source: &str, within: ontomark_parse::Span, target: &str) -> (usize, usize) {
    let hay = &source[within.start..within.end];
    let needle = format!("[[{target}");
    hay.find(&needle)
        .and_then(|i| {
            hay[i..]
                .find("]]")
                .map(|j| (within.start + i, within.start + i + j + 2))
        })
        .unwrap_or((within.start, within.end))
}

fn kind_mismatch(head: &Head<'_>, key: &str, target: &Head<'_>) -> Diagnostic {
    let mut d = entry_diag(
        head.path,
        head.doc,
        key,
        "kind_mismatch",
        format!(
            "`{key}` on a {:?} cannot point at the {:?} {}",
            head.kind, target.kind, target.path
        ),
    );
    d.severity = Severity::Warning;
    d
}

fn diag(
    path: &str,
    doc: &Document,
    code: &str,
    severity: Severity,
    message: String,
    at: Option<(usize, usize)>,
) -> Diagnostic {
    Diagnostic {
        path: path.to_owned(),
        code: code.to_owned(),
        severity,
        message,
        span: at.map(|(s, e)| HostSpan::of(doc.source(), s, e)),
    }
}

fn entry_diag(path: &str, doc: &Document, key: &str, code: &str, message: String) -> Diagnostic {
    let at = doc
        .frontmatter()
        .and_then(|fm| fm.get(key))
        .map(|e| (e.span.start, e.span.end));
    diag(path, doc, code, Severity::Error, message, at)
}

fn texts(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Seq(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn literals(v: &Value) -> Vec<Literal> {
    match v {
        Value::Null | Value::Map(_) => Vec::new(),
        Value::Bool(b) => vec![Literal::new_typed_literal(b.to_string(), xsd::BOOLEAN)],
        Value::Int(i) => vec![Literal::new_typed_literal(i.to_string(), xsd::INTEGER)],
        Value::Float(lex) => {
            let lower = lex.to_ascii_lowercase();
            let dt = if lower.contains('e') || lower.contains("inf") || lower.contains("nan") {
                xsd::DOUBLE
            } else {
                xsd::DECIMAL
            };
            vec![Literal::new_typed_literal(lex.clone(), dt)]
        }
        Value::String(s) => vec![Literal::new_simple_literal(s)],
        Value::Seq(items) => items.iter().flat_map(literals).collect(),
    }
}

fn is_absolute_iri(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    !rest.is_empty()
        && scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && NamedNode::new(s).is_ok()
}

fn node(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

fn triple(s: &NamedNode, p: NamedNode, o: impl Into<Term>) -> Triple {
    Triple::new(NamedOrBlankNode::NamedNode(s.clone()), p, o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ProfileId;

    fn vault(pages: &[(&str, &str)]) -> BTreeMap<String, Document> {
        pages
            .iter()
            .map(|(p, t)| ((*p).to_owned(), Document::parse(*t)))
            .collect()
    }

    #[test]
    fn plain_notes_and_prose_links_yield_nothing() {
        let docs = vault(&[
            ("A.md", "A note linking [[B]] and [[C]].\n"),
            (
                "B.md",
                "---\ntags: [x]\nrelated: [\"[[A]]\"]\n---\nSee [[A]].\n",
            ),
            (
                "C.md",
                "---\ntype: class\n---\nProse says [[A]] is a kind of [[B]].\n",
            ),
        ]);
        let c = compile(&docs, ProfileId::Generic.profile());
        assert_eq!(c.entities.len(), 1, "only the typed page is an entity");
        assert!(
            c.axioms.is_empty(),
            "prose links must never become axioms: {:?}",
            c.axioms
        );
        assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    }

    #[test]
    fn generic_typed_pages_give_axioms_and_unresolved_links_diagnose() {
        let docs = vault(&[
            ("Animal.md", "---\ntype: class\n---\n"),
            (
                "Dog.md",
                "---\ntype: class\nsubclass-of: [\"[[Animal]]\", \"[[Missing]]\"]\n---\n",
            ),
            (
                "Rex.md",
                "---\ntype: individual\ninstance-of: [\"[[Dog]]\"]\n---\n",
            ),
        ]);
        let c = compile(&docs, ProfileId::Generic.profile());
        assert_eq!(c.axioms.len(), 1);
        assert_eq!(c.diagnostics.len(), 1);
        assert_eq!(c.diagnostics[0].code, "unresolved_reference");
        let span = c.diagnostics[0].span.unwrap();
        assert_eq!(
            &docs["Dog.md"].source()[span.byte_start..span.byte_end],
            "[[Missing]]"
        );
        assert_eq!(c.individual_types["urn:ontomark:page:Rex"].len(), 1);
    }

    #[test]
    fn estate_profile_requires_resource_and_keeps_it_verbatim() {
        let docs = vault(&[
            ("KG.md", "---\ntype: Class\nresource: urn:ngm:class:knowledge-graph\npublic: true\nis-a: [\"[[Content]]\"]\n---\n"),
            ("Content.md", "---\ntype: Class\nresource: urn:ngm:class:content\npublic: true\n---\n"),
            ("Bad.md", "---\ntype: Class\npublic: \"true\"\nresource: urn:ngm:class:bad\n---\n"),
            ("NoIri.md", "---\ntype: Class\n---\n"),
        ]);
        let estate = compile(&docs, ProfileId::Estate.profile());
        let iris: Vec<_> = estate.entities.iter().map(|e| e.iri.as_str()).collect();
        assert_eq!(
            iris,
            vec!["urn:ngm:class:content", "urn:ngm:class:knowledge-graph"]
        );
        let codes: Vec<_> = estate.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, vec!["gate_not_boolean", "iri_missing"]);
        // The same vault under the generic profile: no `is-a` refusal, minted IRIs.
        let generic = compile(&docs, ProfileId::Generic.profile());
        assert_eq!(generic.entities.len(), 4);
    }

    #[test]
    fn ambiguous_titles_are_reported_not_guessed() {
        let docs = vault(&[
            ("x/Bank.md", "---\ntype: class\n---\n"),
            ("y/Bank.md", "---\ntype: class\n---\n"),
            (
                "Loan.md",
                "---\ntype: class\nsubclass-of: [\"[[Bank]]\"]\n---\n",
            ),
        ]);
        let c = compile(&docs, ProfileId::Generic.profile());
        assert!(c.axioms.is_empty());
        assert_eq!(c.diagnostics[0].code, "ambiguous_reference");
    }
}
