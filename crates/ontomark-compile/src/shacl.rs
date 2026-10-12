//! SHACL Core shapes for the frontmatter contract, and the data graph they
//! validate.
//!
//! [`shapes`] projects the corpus rules that `ontomark validate` hard-codes
//! (`check/validate.rs`) and that a [`Profile`] and a [`Vocabulary`] describe
//! into a SHACL shapes graph. [`frontmatter_graph`] projects a loaded
//! [`Vault`] into the RDF data graph those shapes validate. Both are
//! [`Graph`]s, written with [`turtle::serialise`](crate::build::turtle::serialise)
//! and [`jsonld::to_jsonld`](crate::jsonld::to_jsonld).
//!
//! # Supported subset (ADR-2015)
//!
//! This module emits **SHACL Core** only, and a deliberately small part of it.
//! It does not claim complete SHACL support, and it uses no SHACL-SPARQL, no
//! SHACL-AF rules, no `sh:closed`, no
//! `owl:imports` and no entailment.
//!
//! | construct | used for |
//! |---|---|
//! | `sh:targetClass`, `sh:targetObjectsOf` | selecting pages, journals, link targets |
//! | `sh:path` (predicate, sequence, alternative, inverse) | frontmatter keys, link keys, duplicate identities |
//! | `sh:minCount`, `sh:maxCount` | required keys, single-valued keys, one owner per identity |
//! | `sh:datatype`, `sh:nodeKind` | numbers, booleans, IRIs, stamp objects |
//! | `sh:in`, `sh:hasValue` | status, type, enumerations, domains, known keys |
//! | `sh:pattern`, `sh:minLength` | `generated.by` actor grammar, IRI namespaces, journal dates |
//! | `sh:minInclusive`, `sh:maxInclusive` | numeric ranges |
//! | `sh:class` | link targets are declared pages |
//! | `sh:disjoint` | a class is not its own parent |
//! | `sh:node`, `sh:or`, `sh:not`, `sh:qualifiedValueShape` | stamp objects, "stable needs a human verifier", public-to-private links |
//! | `sh:severity`, `sh:message` | mapping onto validate's error, warning and info |
//!
//! Patterns are XPath/XSD-style regular expressions (no inline flags, no
//! look-around), so any conforming engine reads them the same way.
//!
//! # The data graph
//!
//! [`frontmatter_graph`] builds **one graph per vault**: link resolution and
//! identity uniqueness are per-vault in `validate`, so the graph is too. All
//! names live in the profile's vocabulary namespace, written `ns:` below.
//!
//! | node or property | meaning |
//! |---|---|
//! | `ns:page/<id>` | one node per page under `pages/` (the id percent-encoded) |
//! | `ns:journal/<id>` | one node per page under `journals/` |
//! | `ns:fm-<key>` | the frontmatter key `<key>`, one triple per value, nulls omitted |
//! | `ns:meta-key` | each top-level frontmatter key as a string, null-valued keys included |
//! | `ns:meta-id` | the page id (the path under `pages/` without `.md`) |
//! | `ns:meta-identity` | the effective identity IRI, `resource` or its minted fallback |
//! | `ns:meta-parent` | each `is-a` parent as the build resolves it, an IRI |
//! | `ns:meta-idlink` | a link whose target is exactly another page's id |
//! | `rdf:type ns:Page` | every page; `ns:OntologyPage` if it has `resource` or `type` |
//! | `rdf:type ns:page-in-<role>` | the vault role; `ns:ontology-page-in-<role>` likewise |
//! | `rdf:type ns:PublicPage` / `ns:PrivatePage` | the publish gate, fail-closed |
//! | `rdf:type ns:LinkTarget` | pages and `tail_iris` targets a link may name |
//! | `rdf:type ns:type/<Type>` | the declared `type`, so per-type rules can target it |
//!
//! Values are read through the accessors `validate` uses, so the graph holds
//! what `validate` sees rather than the raw YAML: a scalar of a declared
//! number or boolean type is a typed literal only when it parses as one
//! (`public: "true"` is a boolean, `public: "yes"` stays a string and fails the
//! datatype shape), `status` is trimmed and lower-cased when it is a known
//! state, and `generated` and `verified` are the parsed stamps. A relation key
//! becomes an edge to the page it resolves to under `validate`'s membership
//! test (exact id, lower-cased title or alias, or `tail_iris`), or to an
//! untyped `ns:unresolved/<target>` node when it resolves to nothing.
//!
//! # Rule coverage
//!
//! Every validation code `validate` can emit is listed in [`RULES`]. `shacl`
//! means a shape in [`shapes`] carries the code in its `ns:ruleCode`
//! annotation; `procedural` means SHACL Core cannot express it and
//! `ontomark validate` remains the only implementation.
//!
//! | code | status | shape (`ns:shape-…`) or reason |
//! |---|---|---|
//! | `UNKNOWN_KEY` | shacl | `UnknownKey-<role>` (`sh:in` over `ns:meta-key`) |
//! | `INVALID_NUMBER` | shacl | `InvalidNumber-<key>` |
//! | `INVALID_BOOLEAN` | shacl | `InvalidBoolean-<key>` |
//! | `OUT_OF_RANGE` | shacl | `OutOfRange-<key>` |
//! | `INVALID_ENUM` | shacl | `InvalidEnum-<key>` |
//! | `MISSING_TYPE` | shacl | `MissingType-<role>` |
//! | `INVALID_TYPE` | shacl | `InvalidType-<role>`, `JournalInvalidType` |
//! | `MISSING_REQUIRED` | shacl | `MissingRequired-<Type>-<key>` |
//! | `MISSING_CLASS_IRI` | shacl | `MissingRequired-<Type>-resource` (see note 1) |
//! | `MISSING_STATUS` | shacl | `MissingStatus-<role>` |
//! | `INVALID_STATUS` | shacl | `InvalidStatus` |
//! | `UNVERIFIED_STABLE` | shacl | `UnverifiedStable` |
//! | `MISSING_GENERATED` | shacl | `MissingGenerated-<role>`, `GeneratedStamp-<role>` (the `generated.by` `sh:pattern`) |
//! | `MISSING_DOMAIN` | shacl | `MissingDomain` |
//! | `INVALID_DOMAIN` | shacl | `InvalidDomain` |
//! | `SELF_REFERENCE` | shacl | `SelfReference` (`sh:disjoint`) |
//! | `MULTI_PARENT` | shacl | `MultiParent` (`sh:maxCount`, info) |
//! | `DANGLING_LINK` | shacl | `DanglingLink` (`sh:class`) |
//! | `PUBLIC_LINKS_PRIVATE` | shacl | `PublicLinksPrivate` (`sh:not`/`sh:class`) |
//! | `DUPLICATE_IRI` | shacl | `DuplicateIri` (inverse path, `sh:maxCount`) |
//! | `JOURNAL_NOT_DATED` | shacl | `JournalNotDated` (`sh:pattern` on the id) |
//! | `MISSING_LABEL` | procedural | `label` falls back to `title`, which falls back to the file stem; the stem is not frontmatter |
//! | `SLUG_MISMATCH` | procedural | compares `slugify` of a link label with the target's slug, a string function SHACL Core lacks |
//! | `DISJOINT_NOT_SIBLINGS` | procedural | two classes must share a direct parent, a join across two focus nodes SHACL Core cannot make; the target-is-a-page half is the shape `DisjointTargetIsClass`, annotated `ns:partialRuleCode` instead of `ns:ruleCode` |
//! | `NON_EL_DEFINITION` | procedural | grammar of nested `defines-as` maps, parsed by `input::definition` |
//! | `DEFINITION_OVER_TRANSITIVE` | procedural | needs the transitive closure of property characteristics in the vocabulary |
//! | `DEFINITION_NOT_EXISTENTIAL` | procedural | needs the vocabulary's `restriction` flag of the named relation |
//! | `FENCE_RESIDUE` | procedural | page body text, not frontmatter |
//! | `LOGSEQ_PROPERTY` | procedural | page body text |
//! | `EMBED_RESIDUE` | procedural | page body text |
//! | `BLOCK_REF_RESIDUE` | procedural | page body text |
//! | `UNTERMINATED_FENCE` | procedural | page body text |
//! | `SECRET_DETECTED` | procedural | page body and frontmatter text |
//! | `UNCLOSED_FRONTMATTER` | procedural | file structure: the fence never closes |
//! | `INVALID_PUBLIC_FLAG` | procedural | publication permission must be a YAML bool, not a string |
//!
//! Note 1: `MISSING_CLASS_IRI` and the `resource` half of `MISSING_REQUIRED`
//! cannot fire in `validate`, because a page without `resource` has an identity
//! minted from its slug. The shapes test the same minted identity
//! (`ns:meta-identity`), so they agree with `validate` by never firing; the
//! stricter profile rule "a knowledge page declares `resource`" is a separate
//! shape, `RequiredKey-<role>-resource`, that `validate` does not have.
//!
//! A shape carries one origin annotation: `ns:ruleCode` (all of a `validate`
//! rule), `ns:partialRuleCode` (the SHACL-expressible part of a procedural
//! rule) or `ns:profileRule` (a rule only the profile asks for).
//!
//! The profile also asks for things the legacy `validate` never enforced:
//! required `title` and `public`, the namespace of `resource` and
//! `page_resource`, a required `resource`, and the profile's lifecycle levels
//! for `maturity` (the vocabulary's own enumeration, which `validate` checks,
//! is wider). Those shapes carry no
//! `ns:ruleCode`, are marked `ns:profileRule true`, and are `sh:Warning`, so a
//! conforming corpus is not made non-conforming by rules `validate` never had.
//!
//! # Example
//!
//! ```
//! use ontomark_compile::build::turtle::serialise;
//! use ontomark_compile::input::vocabulary::Vocabulary;
//! use ontomark_compile::profile::Profile;
//! use ontomark_compile::shacl::shapes;
//!
//! let vocab = Vocabulary::from_yaml_str("version: 1\nnamespace: \"urn:ngm:class:\"\n").unwrap();
//! let profile = Profile::generic();
//! let ttl = serialise(&profile, &shapes(&profile, &vocab));
//! assert!(ttl.contains("http://www.w3.org/ns/shacl#"));
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::sync::OnceLock;

use serde_yaml::Value as Yaml;

use crate::build::turtle::{Graph, Subject, Term};
use crate::input::frontmatter::Wikilink;
use crate::input::okf::{OkfBlock, Status};
use crate::input::page::{Page, Vault, JOURNAL_TYPE};
use crate::input::vocabulary::{ScalarType, Vocabulary};
use crate::ir::model::Corpus;
use crate::profile::{Profile, Role, UnknownKeys};

/// The SHACL namespace.
pub const SH: &str = "http://www.w3.org/ns/shacl#";

const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// The actor spellings `validate` accepts (`input::okf::Actor`), used when a
/// profile lists none.
const DEFAULT_ACTORS: [&str; 3] = [
    "human:<npub>",
    "agent:<label>/<version>",
    "process:<name>/<version>",
];

/// The domain list `validate` falls back to when the vocabulary enumerates
/// none; mirrors `check::validate::FALLBACK_DOMAINS`.
const FALLBACK_DOMAINS: &[&str] = &[
    "artificial-intelligence",
    "spatial-computing",
    "blockchain",
    "infrastructure",
    "distributed-collaboration",
    "robotics",
    "ai",
    "supply-chain",
    "metaverse",
    "data",
    "governance",
    "security",
    "standards",
    "finance",
    "distributed-systems",
    "machine-learning",
];

/// Whether SHACL Core carries a validation rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// At least one shape annotated with the rule's code.
    Shacl,
    /// Not expressible in SHACL Core; only `ontomark validate` enforces it.
    Procedural,
}

/// One validation code and how this module covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// The code `ontomark validate` reports.
    pub code: &'static str,
    /// Whether a shape carries it.
    pub coverage: Coverage,
    /// The shape name pattern (after `ns:shape-`) for a covered rule, or the
    /// reason a procedural rule is outside SHACL Core.
    pub note: &'static str,
}

const fn shacl(code: &'static str, note: &'static str) -> Rule {
    Rule {
        code,
        coverage: Coverage::Shacl,
        note,
    }
}

const fn procedural(code: &'static str, note: &'static str) -> Rule {
    Rule {
        code,
        coverage: Coverage::Procedural,
        note,
    }
}

/// Every validation code `ontomark validate` can emit, and its coverage.
///
/// The table in the [module documentation](self) is this list; a test keeps
/// it complete against `check/validate.rs`.
pub const RULES: &[Rule] = &[
    shacl("UNKNOWN_KEY", "UnknownKey-<role>"),
    shacl("INVALID_NUMBER", "InvalidNumber-<key>"),
    shacl("INVALID_BOOLEAN", "InvalidBoolean-<key>"),
    shacl("OUT_OF_RANGE", "OutOfRange-<key>"),
    shacl("INVALID_ENUM", "InvalidEnum-<key>"),
    shacl("MISSING_TYPE", "MissingType-<role>"),
    shacl("INVALID_TYPE", "InvalidType-<role>, JournalInvalidType"),
    shacl("MISSING_REQUIRED", "MissingRequired-<Type>-<key>"),
    shacl("MISSING_CLASS_IRI", "MissingRequired-<Type>-resource"),
    shacl("MISSING_STATUS", "MissingStatus-<role>"),
    shacl("INVALID_STATUS", "InvalidStatus"),
    shacl("UNVERIFIED_STABLE", "UnverifiedStable"),
    shacl(
        "MISSING_GENERATED",
        "MissingGenerated-<role>, GeneratedStamp-<role>",
    ),
    shacl("MISSING_DOMAIN", "MissingDomain"),
    shacl("INVALID_DOMAIN", "InvalidDomain"),
    shacl("SELF_REFERENCE", "SelfReference"),
    shacl("MULTI_PARENT", "MultiParent"),
    shacl("DANGLING_LINK", "DanglingLink"),
    shacl("PUBLIC_LINKS_PRIVATE", "PublicLinksPrivate"),
    shacl("DUPLICATE_IRI", "DuplicateIri"),
    shacl("JOURNAL_NOT_DATED", "JournalNotDated"),
    procedural(
        "MISSING_LABEL",
        "label falls back to title, which falls back to the file stem; the stem is not frontmatter",
    ),
    procedural(
        "SLUG_MISMATCH",
        "compares slugify of a link label with the target slug; SHACL Core has no string function",
    ),
    procedural(
        "DISJOINT_NOT_SIBLINGS",
        "two classes must share a direct parent, a join across two focus nodes (target-is-a-class half: DisjointTargetIsClass)",
    ),
    procedural(
        "NON_EL_DEFINITION",
        "grammar of nested defines-as maps, parsed by input::definition",
    ),
    procedural(
        "DEFINITION_OVER_TRANSITIVE",
        "needs the transitive closure of property characteristics in the vocabulary",
    ),
    procedural(
        "DEFINITION_NOT_EXISTENTIAL",
        "needs the restriction flag of the relation the definition names",
    ),
    procedural("FENCE_RESIDUE", "page body text, not frontmatter"),
    procedural("LOGSEQ_PROPERTY", "page body text"),
    procedural("EMBED_RESIDUE", "page body text"),
    procedural("BLOCK_REF_RESIDUE", "page body text"),
    procedural("UNTERMINATED_FENCE", "page body text"),
    procedural("SECRET_DETECTED", "page body and frontmatter text"),
    procedural("UNCLOSED_FRONTMATTER", "file structure: the fence never closes"),
    procedural("INVALID_PUBLIC_FLAG", "publication permission must be a YAML bool, not a string"),
];

// ---------------------------------------------------------------------------
// names
// ---------------------------------------------------------------------------

/// Percent-encode everything outside RFC 3986 `unreserved`, so a page id or a
/// frontmatter key is always safe inside `<…>`.
#[must_use]
pub fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// The profile's vocabulary namespace, the root of every name here.
fn namespace(profile: &Profile) -> &str {
    &profile.iris.vocabulary_namespace
}

/// The property IRI that carries frontmatter key `key` in the data graph:
/// `<vocabulary namespace>fm-<key>`. Shapes and data share this one function.
#[must_use]
pub fn key_iri(profile: &Profile, key: &str) -> String {
    format!("{}fm-{}", namespace(profile), encode_component(key))
}

/// The IRI of the shape named `name`: `<vocabulary namespace>shape-<name>`.
#[must_use]
pub fn shape_iri(profile: &Profile, name: &str) -> String {
    format!("{}shape-{}", namespace(profile), encode_component(name))
}

/// The annotation property that names the validation code a shape implements.
#[must_use]
pub fn rule_code_iri(profile: &Profile) -> String {
    format!("{}ruleCode", namespace(profile))
}

/// The annotation property marking a shape that only the profile asks for.
#[must_use]
pub fn profile_rule_iri(profile: &Profile) -> String {
    format!("{}profileRule", namespace(profile))
}

/// The annotation property naming the validation code a shape implements only
/// in part (the remainder is procedural).
#[must_use]
pub fn partial_rule_code_iri(profile: &Profile) -> String {
    format!("{}partialRuleCode", namespace(profile))
}

fn meta(profile: &Profile, name: &str) -> String {
    format!("{}meta-{name}", namespace(profile))
}

fn class(profile: &Profile, name: &str) -> String {
    format!("{}{name}", namespace(profile))
}

fn role_class(profile: &Profile, role: &str, ontology: bool) -> String {
    let stem = if ontology {
        "ontology-page-in"
    } else {
        "page-in"
    };
    format!("{}{stem}-{}", namespace(profile), encode_component(role))
}

fn type_class(profile: &Profile, type_name: &str) -> String {
    format!("{}type/{}", namespace(profile), encode_component(type_name))
}

fn page_node(profile: &Profile, id: &str, journal: bool) -> String {
    let kind = if journal { "journal" } else { "page" };
    format!("{}{kind}/{}", namespace(profile), encode_component(id))
}

fn sh(local: &str) -> String {
    format!("{SH}{local}")
}

fn rdf(local: &str) -> String {
    format!("{RDF}{local}")
}

fn xsd(local: &str) -> String {
    format!("{XSD}{local}")
}

/// A role that governs ontology pages: its pages must declare a `type`, a
/// `status` and a `generated` stamp (what `validate` demands of `knowledge/`).
fn governed(role: &Role) -> bool {
    role.reasoned
}

// ---------------------------------------------------------------------------
// regular expressions (XPath / XSD flavour)
// ---------------------------------------------------------------------------

/// Escape `text` for an XSD regular expression.
fn regex_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(
            c,
            '.' | '\\'
                | '?'
                | '*'
                | '+'
                | '{'
                | '}'
                | '('
                | ')'
                | '|'
                | '['
                | ']'
                | '^'
                | '$'
                | '-'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Turn an actor template such as `agent:<label>/<version>` into a regular
/// expression body that matches what `Actor::from_str` accepts: a placeholder
/// after a `/` is one path segment, any other placeholder is non-empty text.
fn template_regex(template: &str) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('<') {
        let (literal, tail) = rest.split_at(open);
        out.push_str(&regex_escape(literal));
        match tail.find('>') {
            Some(close) => {
                out.push_str(if literal.ends_with('/') {
                    "[^/]+"
                } else {
                    ".+"
                });
                rest = &tail[close + 1..];
            }
            None => {
                out.push_str(&regex_escape(tail));
                rest = "";
            }
        }
    }
    out.push_str(&regex_escape(rest));
    out
}

/// The `sh:pattern` for any accepted `generated.by` actor.
fn actor_pattern(profile: &Profile) -> String {
    let templates: Vec<&str> = if profile.attribution.actors.is_empty() {
        DEFAULT_ACTORS.to_vec()
    } else {
        profile
            .attribution
            .actors
            .iter()
            .map(String::as_str)
            .collect()
    };
    let alternatives: Vec<String> = templates.iter().map(|t| template_regex(t)).collect();
    format!("^({})$", alternatives.join("|"))
}

/// The `sh:pattern` for a human actor.
fn human_pattern(profile: &Profile) -> String {
    let human: Vec<String> = profile
        .attribution
        .actors
        .iter()
        .filter(|t| t.starts_with("human:"))
        .map(|t| template_regex(t))
        .collect();
    if human.is_empty() {
        "^human:.+$".to_owned()
    } else {
        format!("^({})$", human.join("|"))
    }
}

// ---------------------------------------------------------------------------
// shapes
// ---------------------------------------------------------------------------

/// How serious a result is; maps onto `sh:severity`.
#[derive(Debug, Clone, Copy)]
enum Severity {
    Violation,
    Warning,
    Info,
}

impl Severity {
    fn iri(self) -> String {
        sh(match self {
            Self::Violation => "Violation",
            Self::Warning => "Warning",
            Self::Info => "Info",
        })
    }
}

/// Where a shape's requirement comes from.
#[derive(Debug, Clone, Copy)]
enum Origin<'a> {
    /// All of a validation rule.
    Rule(&'a str),
    /// The part of a validation rule SHACL Core can express.
    Partial(&'a str),
    /// Only the profile asks for it.
    Profile,
}

/// A SHACL property path.
enum Path {
    Predicate(String),
    Alternative(Vec<String>),
    Inverse(String),
}

struct Builder<'a> {
    graph: Graph,
    profile: &'a Profile,
}

impl<'a> Builder<'a> {
    fn new(profile: &'a Profile) -> Self {
        Self {
            graph: Graph::new(),
            profile,
        }
    }

    fn add(&mut self, subject: &str, predicate: &str, object: Term) {
        self.graph.add_iri(subject, predicate, object);
    }

    fn add_blank(&mut self, label: &str, predicate: &str, object: Term) {
        self.graph
            .add(Subject::Blank(label.to_owned()), predicate, object);
    }

    fn list(&mut self, items: Vec<Term>) -> Term {
        let mut next = Term::Iri(rdf("nil"));
        for item in items.into_iter().rev() {
            let cell = self.graph.fresh_blank();
            self.add_blank(&cell, &rdf("first"), item);
            self.add_blank(&cell, &rdf("rest"), next);
            next = Term::Blank(cell);
        }
        next
    }

    fn strings(&mut self, values: &[String]) -> Term {
        let items = values.iter().map(|v| Term::plain(v.clone())).collect();
        self.list(items)
    }

    fn path_term(&mut self, path: &Path) -> Term {
        match path {
            Path::Predicate(iri) => Term::Iri(iri.clone()),
            Path::Inverse(iri) => {
                let node = self.graph.fresh_blank();
                self.add_blank(&node, &sh("inversePath"), Term::Iri(iri.clone()));
                Term::Blank(node)
            }
            Path::Alternative(options) => {
                let items = options.iter().map(|s| Term::Iri(s.clone())).collect();
                let list = self.list(items);
                let node = self.graph.fresh_blank();
                self.add_blank(&node, &sh("alternativePath"), list);
                Term::Blank(node)
            }
        }
    }

    /// Open a named shape. `path` makes it a property shape.
    fn shape(
        &mut self,
        name: &str,
        severity: Severity,
        origin: Origin<'_>,
        message: &str,
        path: Option<&Path>,
    ) -> String {
        let iri = shape_iri(self.profile, name);
        let kind = if path.is_some() {
            "PropertyShape"
        } else {
            "NodeShape"
        };
        self.add(&iri, &rdf("type"), Term::Iri(sh(kind)));
        self.add(&iri, &sh("severity"), Term::Iri(severity.iri()));
        self.add(&iri, &sh("message"), Term::plain(message));
        match origin {
            Origin::Rule(code) => {
                let property = rule_code_iri(self.profile);
                self.add(&iri, &property, Term::plain(code));
            }
            Origin::Partial(code) => {
                let property = partial_rule_code_iri(self.profile);
                self.add(&iri, &property, Term::plain(code));
            }
            Origin::Profile => {
                let property = profile_rule_iri(self.profile);
                self.add(&iri, &property, Term::typed("true", &xsd("boolean")));
            }
        }
        if let Some(path) = path {
            let term = self.path_term(path);
            self.add(&iri, &sh("path"), term);
        }
        iri
    }

    fn target_class(&mut self, shape: &str, class: &str) {
        self.add(shape, &sh("targetClass"), Term::Iri(class.to_owned()));
    }

    fn constraint(&mut self, shape: &str, component: &str, value: Term) {
        self.add(shape, &sh(component), value);
    }

    fn min_count(&mut self, shape: &str, count: u32) {
        self.constraint(
            shape,
            "minCount",
            Term::typed(count.to_string(), &xsd("integer")),
        );
    }

    fn max_count(&mut self, shape: &str, count: u32) {
        self.constraint(
            shape,
            "maxCount",
            Term::typed(count.to_string(), &xsd("integer")),
        );
    }

    fn one_of(&mut self, shape: &str, values: &[String]) {
        let list = self.strings(values);
        self.constraint(shape, "in", list);
    }

    fn pattern(&mut self, shape: &str, regex: &str) {
        self.constraint(shape, "pattern", Term::plain(regex));
    }

    /// An anonymous blank-node shape with a path, used inside `sh:node`.
    fn inner_property(&mut self, path: &Path) -> String {
        let node = self.graph.fresh_blank();
        let term = self.path_term(path);
        self.add_blank(&node, &sh("path"), term);
        node
    }
}

/// Project the validation rules into a SHACL Core shapes graph.
///
/// Governed by `profile` (roles, admitted types, required keys, namespaces,
/// actor grammar, lifecycle) and `vocab` (relations, scalar types, enumerations
/// and ranges, per-type required keys). Shape names, the data graph they
/// validate and the table of rules covered are in the [module
/// documentation](self); every shape that implements a `validate` rule carries
/// its code in `ns:ruleCode`.
#[must_use]
#[allow(clippy::too_many_lines)] // One block per rule family; splitting hides the list.
pub fn shapes(profile: &Profile, vocab: &Vocabulary) -> Graph {
    let mut b = Builder::new(profile);
    let fm = |key: &str| key_iri(profile, key);
    let statuses: Vec<String> = [Status::Draft, Status::Stable, Status::Deprecated]
        .iter()
        .map(|s| s.as_str().to_owned())
        .collect();

    // --- per vault role ---------------------------------------------------
    for (role_name, role) in &profile.frontmatter.roles {
        let pages = role_class(profile, role_name, false);
        let ontology = role_class(profile, role_name, true);
        let r = encode_component(role_name);

        // INVALID_TYPE: `validate` admits the role's types, plus (outside a
        // governed role) the governed types too.
        let mut admitted: Vec<String> = role.types.clone();
        if !governed(role) {
            admitted.extend(vocab.types.keys().cloned());
        }
        admitted.sort();
        admitted.dedup();
        if !admitted.is_empty() {
            let s = b.shape(
                &format!("InvalidType-{r}"),
                Severity::Violation,
                Origin::Rule("INVALID_TYPE"),
                "`type` is not declared for this vault",
                Some(&Path::Predicate(fm("type"))),
            );
            b.target_class(&s, &pages);
            b.one_of(&s, &admitted);
        }

        if governed(role) {
            let s = b.shape(
                &format!("MissingType-{r}"),
                Severity::Violation,
                Origin::Rule("MISSING_TYPE"),
                "every governed page needs a `type`",
                Some(&Path::Predicate(fm("type"))),
            );
            b.target_class(&s, &pages);
            b.min_count(&s, 1);

            let s = b.shape(
                &format!("MissingStatus-{r}"),
                Severity::Violation,
                Origin::Rule("MISSING_STATUS"),
                "every governed ontology page needs a `status`",
                Some(&Path::Predicate(fm("status"))),
            );
            b.target_class(&s, &ontology);
            b.min_count(&s, 1);

            let s = b.shape(
                &format!("MissingGenerated-{r}"),
                Severity::Warning,
                Origin::Rule("MISSING_GENERATED"),
                "no `generated` stamp",
                Some(&Path::Predicate(fm("generated"))),
            );
            b.target_class(&s, &ontology);
            b.min_count(&s, 1);

            // The stamp object: a mapping with a string `by` that is a valid
            // actor and a string `at`.
            let s = b.shape(
                &format!("GeneratedStamp-{r}"),
                Severity::Warning,
                Origin::Rule("MISSING_GENERATED"),
                "`generated` must be a mapping of `by` (an actor) and `at` (a datetime)",
                Some(&Path::Predicate(fm("generated"))),
            );
            b.target_class(&s, &ontology);
            b.constraint(&s, "nodeKind", Term::Iri(sh("BlankNode")));
            let stamp = b.graph.fresh_blank();
            b.add_blank(&stamp, &rdf("type"), Term::Iri(sh("NodeShape")));
            let by = b.inner_property(&Path::Predicate(fm("by")));
            b.add_blank(&by, &sh("minCount"), Term::typed("1", &xsd("integer")));
            b.add_blank(&by, &sh("maxCount"), Term::typed("1", &xsd("integer")));
            b.add_blank(&by, &sh("datatype"), Term::Iri(xsd("string")));
            b.add_blank(&by, &sh("pattern"), Term::plain(actor_pattern(profile)));
            let at = b.inner_property(&Path::Predicate(fm("at")));
            b.add_blank(&at, &sh("minCount"), Term::typed("1", &xsd("integer")));
            b.add_blank(&at, &sh("maxCount"), Term::typed("1", &xsd("integer")));
            b.add_blank(&at, &sh("datatype"), Term::Iri(xsd("string")));
            b.add_blank(&stamp, &sh("property"), Term::Blank(by));
            b.add_blank(&stamp, &sh("property"), Term::Blank(at));
            b.constraint(&s, "node", Term::Blank(stamp));
        }

        // UNKNOWN_KEY: every top-level key, null-valued ones included, is a
        // `meta-key` literal; the shape admits the vocabulary's known keys.
        if role.unknown_keys == UnknownKeys::Fail {
            let s = b.shape(
                &format!("UnknownKey-{r}"),
                Severity::Violation,
                Origin::Rule("UNKNOWN_KEY"),
                "frontmatter key is not declared in the vocabulary",
                Some(&Path::Predicate(meta(profile, "key"))),
            );
            b.target_class(&s, &pages);
            let known: Vec<String> = vocab.known_keys().into_iter().map(str::to_owned).collect();
            b.one_of(&s, &known);
        }

        // Profile-only: the keys the role asks for beyond what validate checks.
        for key in &role.required_keys {
            if governed(role) && matches!(key.as_str(), "type" | "status" | "generated") {
                continue;
            }
            let s = b.shape(
                &format!("RequiredKey-{r}-{}", encode_component(key)),
                Severity::Warning,
                Origin::Profile,
                &format!("the profile requires `{key}` on every page of this role"),
                Some(&Path::Predicate(fm(key))),
            );
            b.target_class(&s, &pages);
            b.min_count(&s, 1);
        }
    }

    // --- status, verification ---------------------------------------------
    let s = b.shape(
        "InvalidStatus",
        Severity::Violation,
        Origin::Rule("INVALID_STATUS"),
        "`status` is not draft, stable or deprecated",
        Some(&Path::Predicate(fm("status"))),
    );
    b.target_class(&s, &class(profile, "Page"));
    b.one_of(&s, &statuses);

    let s = b.shape(
        "UnverifiedStable",
        Severity::Warning,
        Origin::Rule("UNVERIFIED_STABLE"),
        "`status: stable` with no human entry in `verified`",
        None,
    );
    b.target_class(&s, &class(profile, "Page"));
    // not(stable) or (some verifier is a human).
    let not_stable = {
        let outer = b.graph.fresh_blank();
        let inner = b.inner_property(&Path::Predicate(fm("status")));
        b.add_blank(&inner, &sh("hasValue"), Term::plain("stable"));
        let holder = b.graph.fresh_blank();
        b.add_blank(&holder, &sh("property"), Term::Blank(inner));
        b.add_blank(&outer, &sh("not"), Term::Blank(holder));
        outer
    };
    let has_human = {
        let outer = b.graph.fresh_blank();
        let verified = b.inner_property(&Path::Predicate(fm("verified")));
        let qualified = b.graph.fresh_blank();
        let by = b.inner_property(&Path::Predicate(fm("by")));
        b.add_blank(&by, &sh("minCount"), Term::typed("1", &xsd("integer")));
        b.add_blank(&by, &sh("pattern"), Term::plain(human_pattern(profile)));
        b.add_blank(&qualified, &sh("property"), Term::Blank(by));
        b.add_blank(
            &verified,
            &sh("qualifiedValueShape"),
            Term::Blank(qualified),
        );
        b.add_blank(
            &verified,
            &sh("qualifiedMinCount"),
            Term::typed("1", &xsd("integer")),
        );
        b.add_blank(&outer, &sh("property"), Term::Blank(verified));
        outer
    };
    let alternatives = b.list(vec![Term::Blank(not_stable), Term::Blank(has_human)]);
    b.constraint(&s, "or", alternatives);

    // --- scalars ------------------------------------------------------------
    for (key, def) in &vocab.scalars {
        let k = encode_component(key);
        let path = Path::Predicate(fm(key));
        let pages = class(profile, "Page");
        match def.value_type {
            ScalarType::Number => {
                let s = b.shape(
                    &format!("InvalidNumber-{k}"),
                    Severity::Violation,
                    Origin::Rule("INVALID_NUMBER"),
                    &format!("`{key}` is not a number"),
                    Some(&path),
                );
                b.target_class(&s, &pages);
                b.constraint(&s, "datatype", Term::Iri(xsd("double")));
                if def.min.is_some() || def.max.is_some() {
                    let s = b.shape(
                        &format!("OutOfRange-{k}"),
                        Severity::Warning,
                        Origin::Rule("OUT_OF_RANGE"),
                        &format!("`{key}` is outside the declared bounds"),
                        Some(&path),
                    );
                    b.target_class(&s, &pages);
                    // Only a value that parsed as a number has a range to violate.
                    let not_a_number = b.graph.fresh_blank();
                    let datatype = b.graph.fresh_blank();
                    b.add_blank(&datatype, &sh("datatype"), Term::Iri(xsd("double")));
                    b.add_blank(&not_a_number, &sh("not"), Term::Blank(datatype));
                    let in_range = b.graph.fresh_blank();
                    if let Some(min) = def.min {
                        b.add_blank(
                            &in_range,
                            &sh("minInclusive"),
                            Term::typed(min.to_string(), &xsd("double")),
                        );
                    }
                    if let Some(max) = def.max {
                        b.add_blank(
                            &in_range,
                            &sh("maxInclusive"),
                            Term::typed(max.to_string(), &xsd("double")),
                        );
                    }
                    let alternatives =
                        b.list(vec![Term::Blank(not_a_number), Term::Blank(in_range)]);
                    b.constraint(&s, "or", alternatives);
                }
            }
            ScalarType::Boolean => {
                let s = b.shape(
                    &format!("InvalidBoolean-{k}"),
                    Severity::Violation,
                    Origin::Rule("INVALID_BOOLEAN"),
                    &format!("`{key}` is not a boolean"),
                    Some(&path),
                );
                b.target_class(&s, &pages);
                b.constraint(&s, "datatype", Term::Iri(xsd("boolean")));
            }
            ScalarType::Text | ScalarType::Date | ScalarType::List => {}
        }
        if !def.r#enum.is_empty() {
            let s = b.shape(
                &format!("InvalidEnum-{k}"),
                Severity::Warning,
                Origin::Rule("INVALID_ENUM"),
                &format!("`{key}` is not one of the declared values"),
                Some(&path),
            );
            b.target_class(&s, &pages);
            b.one_of(&s, &def.r#enum);
        }
    }

    // Profile-only: the lifecycle levels the profile declares. `validate`
    // checks the vocabulary's own `maturity` enumeration instead (InvalidEnum).
    if !profile.lifecycle.maturity_levels.is_empty() {
        let s = b.shape(
            "ProfileMaturityLevel",
            Severity::Warning,
            Origin::Profile,
            "`maturity` is not one of the profile's lifecycle levels",
            Some(&Path::Predicate(fm("maturity"))),
        );
        b.target_class(&s, &class(profile, "Page"));
        let levels: Vec<String> = profile
            .lifecycle
            .maturity_levels
            .iter()
            .map(|t| t.slug.clone())
            .collect();
        b.one_of(&s, &levels);
    }

    // --- domain -------------------------------------------------------------
    let ontology_pages = class(profile, "OntologyPage");
    let s = b.shape(
        "MissingDomain",
        Severity::Warning,
        Origin::Rule("MISSING_DOMAIN"),
        "no domain",
        Some(&Path::Predicate(fm("domain"))),
    );
    b.target_class(&s, &ontology_pages);
    b.min_count(&s, 1);
    b.constraint(&s, "minLength", Term::typed("1", &xsd("integer")));

    let mut domains: Vec<String> = match vocab.scalars.get("domain") {
        Some(def) if !def.known_values().is_empty() => {
            def.known_values().into_iter().map(str::to_owned).collect()
        }
        _ => FALLBACK_DOMAINS.iter().map(|d| (*d).to_owned()).collect(),
    };
    domains.sort();
    domains.dedup();
    // An empty domain is MISSING_DOMAIN, not INVALID_DOMAIN.
    domains.insert(0, String::new());
    let s = b.shape(
        "InvalidDomain",
        Severity::Warning,
        Origin::Rule("INVALID_DOMAIN"),
        "domain is not in the valid set",
        Some(&Path::Predicate(fm("domain"))),
    );
    b.target_class(&s, &ontology_pages);
    b.one_of(&s, &domains);

    // --- per-type required keys ---------------------------------------------
    for (type_name, def) in &vocab.types {
        for required in &def.required {
            let path = if required == "resource" {
                Path::Predicate(meta(profile, "identity"))
            } else {
                Path::Predicate(fm(required))
            };
            let code = if required == "resource" {
                "MISSING_CLASS_IRI"
            } else {
                "MISSING_REQUIRED"
            };
            let s = b.shape(
                &format!(
                    "MissingRequired-{}-{}",
                    encode_component(type_name),
                    encode_component(required)
                ),
                Severity::Violation,
                Origin::Rule(code),
                &format!("`type: {type_name}` requires `{required}`"),
                Some(&path),
            );
            b.target_class(&s, &type_class(profile, type_name));
            b.min_count(&s, 1);
        }
    }

    // --- identity and the profile's IRI grammar -------------------------------
    let s = b.shape(
        "DuplicateIri",
        Severity::Violation,
        Origin::Rule("DUPLICATE_IRI"),
        "an identity IRI is claimed by more than one page",
        Some(&Path::Inverse(meta(profile, "identity"))),
    );
    b.add(
        &s,
        &sh("targetObjectsOf"),
        Term::Iri(meta(profile, "identity")),
    );
    b.max_count(&s, 1);

    let class_ns = &profile.iris.class_namespace;
    let individual_ns = &vocab.individual_namespace;
    let mut prefixes: BTreeSet<&str> = BTreeSet::new();
    prefixes.insert(class_ns.as_str());
    prefixes.insert(individual_ns.as_str());
    prefixes.remove("");
    if !prefixes.is_empty() {
        let regex = format!(
            "^({})",
            prefixes
                .iter()
                .map(|p| regex_escape(p))
                .collect::<Vec<_>>()
                .join("|")
        );
        let s = b.shape(
            "ResourceIri",
            Severity::Warning,
            Origin::Profile,
            "`resource` is not an IRI in the profile's class or individual namespace",
            Some(&Path::Predicate(fm("resource"))),
        );
        b.target_class(&s, &class(profile, "Page"));
        b.constraint(&s, "nodeKind", Term::Iri(sh("IRI")));
        b.pattern(&s, &regex);
    }
    if !profile.iris.page_namespace.is_empty() {
        let s = b.shape(
            "PageResourceIri",
            Severity::Warning,
            Origin::Profile,
            "`page_resource` is not an IRI in the profile's page namespace",
            Some(&Path::Predicate(fm("page_resource"))),
        );
        b.target_class(&s, &class(profile, "Page"));
        b.constraint(&s, "nodeKind", Term::Iri(sh("IRI")));
        b.pattern(
            &s,
            &format!("^{}", regex_escape(&profile.iris.page_namespace)),
        );
    }

    // --- taxonomy and links ---------------------------------------------------
    let s = b.shape(
        "SelfReference",
        Severity::Violation,
        Origin::Rule("SELF_REFERENCE"),
        "a class is its own parent",
        Some(&Path::Predicate(meta(profile, "parent"))),
    );
    b.target_class(&s, &ontology_pages);
    b.constraint(&s, "disjoint", Term::Iri(meta(profile, "identity")));

    let s = b.shape(
        "MultiParent",
        Severity::Info,
        Origin::Rule("MULTI_PARENT"),
        "bridging class, more than one parent",
        Some(&Path::Predicate(meta(profile, "parent"))),
    );
    b.target_class(&s, &ontology_pages);
    b.max_count(&s, 1);

    let relation_paths: Vec<String> = vocab.relations.keys().map(|k| fm(k)).collect();
    if !relation_paths.is_empty() {
        let path = if relation_paths.len() == 1 {
            Path::Predicate(relation_paths[0].clone())
        } else {
            Path::Alternative(relation_paths)
        };
        let s = b.shape(
            "DanglingLink",
            Severity::Warning,
            Origin::Rule("DANGLING_LINK"),
            "a relation points at something that is not a page",
            Some(&path),
        );
        b.target_class(&s, &ontology_pages);
        b.constraint(&s, "class", Term::Iri(class(profile, "LinkTarget")));
    }

    let s = b.shape(
        "PublicLinksPrivate",
        Severity::Warning,
        Origin::Rule("PUBLIC_LINKS_PRIVATE"),
        "a public page links to a private page; the build drops the edge",
        Some(&Path::Predicate(meta(profile, "idlink"))),
    );
    b.target_class(&s, &class(profile, "PublicPage"));
    let private = b.graph.fresh_blank();
    b.add_blank(
        &private,
        &sh("class"),
        Term::Iri(class(profile, "PrivatePage")),
    );
    b.constraint(&s, "not", Term::Blank(private));

    // The target-is-a-class half of DISJOINT_NOT_SIBLINGS: a `disjoint-with`
    // target must be a page; a taxonomy node never takes part.
    let disjoint_key = crate::input::consistency::DISJOINT_WITH_KEY;
    if vocab.is_relation(disjoint_key) {
        let s = b.shape(
            "DisjointTargetIsClass",
            Severity::Violation,
            Origin::Partial("DISJOINT_NOT_SIBLINGS"),
            "a `disjoint-with` target must be a page; the sibling join is procedural",
            Some(&Path::Predicate(fm(disjoint_key))),
        );
        b.target_class(&s, &ontology_pages);
        b.constraint(&s, "class", Term::Iri(class(profile, "LinkTarget")));
    }

    // --- journals -------------------------------------------------------------
    let journals = class(profile, "JournalPage");
    let mut journal_types: BTreeSet<String> = vocab.working_types.iter().cloned().collect();
    journal_types.extend(vocab.types.keys().cloned());
    journal_types.insert(JOURNAL_TYPE.to_owned());
    let s = b.shape(
        "JournalInvalidType",
        Severity::Violation,
        Origin::Rule("INVALID_TYPE"),
        "`type` is not a declared type",
        Some(&Path::Predicate(fm("type"))),
    );
    b.target_class(&s, &journals);
    b.one_of(&s, &journal_types.into_iter().collect::<Vec<_>>());

    let s = b.shape(
        "JournalNotDated",
        Severity::Warning,
        Origin::Rule("JOURNAL_NOT_DATED"),
        "a journal page's identity is its date, YYYY-MM-DD",
        Some(&Path::Predicate(meta(profile, "id"))),
    );
    b.target_class(&s, &journals);
    b.pattern(&s, "^[0-9]{4}-[0-9]{2}-[0-9]{2}$");

    b.graph
}

// ---------------------------------------------------------------------------
// data graph
// ---------------------------------------------------------------------------

fn iri_syntax() -> &'static regex::Regex {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    R.get_or_init(|| {
        regex::Regex::new(r#"^[A-Za-z][A-Za-z0-9+.\-]*:[^\s<>"{}|\\^`]*$"#)
            .expect("static IRI regex")
    })
}

/// The link-resolution tables `validate` builds for one vault.
struct Resolver<'a> {
    by_id: HashMap<&'a str, usize>,
    by_title: HashMap<String, usize>,
    by_alias: HashMap<String, usize>,
}

impl<'a> Resolver<'a> {
    fn new(pages: &'a [Page]) -> Self {
        let mut by_id = HashMap::new();
        let mut by_title = HashMap::new();
        let mut by_alias = HashMap::new();
        for (i, page) in pages.iter().enumerate() {
            by_id.entry(page.id.as_str()).or_insert(i);
            by_title.entry(page.title().to_lowercase()).or_insert(i);
            for alias in page.frontmatter.strings("aliases") {
                by_alias.entry(alias.to_lowercase()).or_insert(i);
            }
        }
        Self {
            by_id,
            by_title,
            by_alias,
        }
    }

    fn page(&self, target: &str) -> Option<usize> {
        let lower = target.to_lowercase();
        self.by_id
            .get(target)
            .or_else(|| self.by_title.get(&lower))
            .or_else(|| self.by_alias.get(&lower))
            .copied()
    }
}

struct DataBuilder<'a> {
    graph: Graph,
    profile: &'a Profile,
}

impl DataBuilder<'_> {
    fn fm(&self, key: &str) -> String {
        key_iri(self.profile, key)
    }

    fn type_triple(&mut self, subject: &str, class_iri: String) {
        self.graph
            .add_iri(subject, &rdf("type"), Term::Iri(class_iri));
    }

    /// Nodes for a YAML value (a mapping becomes a blank node).
    fn terms(&mut self, value: &Yaml) -> Vec<Term> {
        match value {
            Yaml::Null | Yaml::Tagged(_) => Vec::new(),
            Yaml::Bool(b) => vec![Term::typed(b.to_string(), &xsd("boolean"))],
            Yaml::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    vec![Term::typed(n.to_string(), &xsd("integer"))]
                } else {
                    vec![Term::typed(n.to_string(), &xsd("double"))]
                }
            }
            Yaml::String(s) => vec![Term::plain(s.clone())],
            Yaml::Sequence(items) => items.iter().flat_map(|i| self.terms(i)).collect(),
            Yaml::Mapping(map) => {
                let node = self.graph.fresh_blank();
                for (k, v) in map {
                    if let Some(key) = k.as_str() {
                        let predicate = self.fm(key);
                        for term in self.terms(v) {
                            self.graph
                                .add(Subject::Blank(node.clone()), &predicate, term);
                        }
                    }
                }
                vec![Term::Blank(node)]
            }
        }
    }

    fn stamp_node(&mut self, by: &str, at: &str) -> Term {
        let node = self.graph.fresh_blank();
        let (by_key, at_key) = (self.fm("by"), self.fm("at"));
        self.graph
            .add(Subject::Blank(node.clone()), &by_key, Term::plain(by));
        self.graph
            .add(Subject::Blank(node.clone()), &at_key, Term::plain(at));
        Term::Blank(node)
    }
}

/// Project a loaded vault's frontmatter into the RDF data graph the [`shapes`]
/// validate.
///
/// `role` names the vault's role in `profile.frontmatter.roles` (for example
/// `"knowledge"`); it types every page `ns:page-in-<role>` so the role's shapes
/// select it. The graph covers `pages/` and `journals/`; see the [module
/// documentation](self) for the names and for what is read how.
#[must_use]
#[allow(clippy::too_many_lines)] // One pass over a page's keys; splitting it hides the order.
pub fn frontmatter_graph(
    vault: &Vault,
    role: &str,
    profile: &Profile,
    vocab: &Vocabulary,
) -> Graph {
    let mut d = DataBuilder {
        graph: Graph::new(),
        profile,
    };
    let resolver = Resolver::new(&vault.pages);
    let corpus = Corpus::build(vault, vocab);
    let page_nodes: Vec<String> = vault
        .pages
        .iter()
        .map(|p| page_node(profile, &p.id, false))
        .collect();
    let link_target = class(profile, "LinkTarget");
    for node in &page_nodes {
        d.type_triple(node, link_target.clone());
    }

    for (index, (page, record)) in vault.pages.iter().zip(&corpus.records).enumerate() {
        let node = &page_nodes[index];
        let fm = &page.frontmatter;
        d.type_triple(node, class(profile, "Page"));
        d.type_triple(node, role_class(profile, role, false));
        let has_ontology = record.has_ontology;
        if has_ontology {
            d.type_triple(node, class(profile, "OntologyPage"));
            d.type_triple(node, role_class(profile, role, true));
        }
        d.type_triple(
            node,
            class(
                profile,
                if page.is_public() {
                    "PublicPage"
                } else {
                    "PrivatePage"
                },
            ),
        );
        if let Some(type_name) = fm.text("type") {
            d.type_triple(node, type_class(profile, &type_name));
        }
        d.graph
            .add_iri(node, &meta(profile, "id"), Term::plain(page.id.clone()));
        if has_ontology && !record.iri.is_empty() {
            d.graph.add_iri(
                node,
                &meta(profile, "identity"),
                Term::Iri(page.resource(&vocab.namespace)),
            );
            for parent in &record.sub_class_of {
                d.graph.add_iri(
                    node,
                    &meta(profile, "parent"),
                    Term::Iri(parent.iri.clone()),
                );
            }
        }

        let okf: OkfBlock = page.okf();
        for (key_value, value) in &fm.map {
            let Some(key) = key_value.as_str() else {
                continue;
            };
            d.graph
                .add_iri(node, &meta(profile, "key"), Term::plain(key));
            if value.is_null() {
                continue;
            }
            let predicate = d.fm(key);
            if vocab.relations.contains_key(key) {
                for link in fm.wikilinks(key) {
                    let target = link_object(&mut d, &resolver, &page_nodes, &link, vocab);
                    d.graph.add_iri(node, &predicate, Term::Iri(target));
                    if has_ontology {
                        if let Some(&i) = resolver.by_id.get(link.target.as_str()) {
                            d.graph.add_iri(
                                node,
                                &meta(profile, "idlink"),
                                Term::Iri(page_nodes[i].clone()),
                            );
                        }
                    }
                }
                continue;
            }
            let terms: Vec<Term> = match key {
                "status" => fm
                    .text("status")
                    .map(|raw| match raw.parse::<Status>() {
                        Ok(status) => Term::plain(status.as_str()),
                        Err(_) => Term::plain(raw),
                    })
                    .into_iter()
                    .collect(),
                "generated" => match &okf.generated {
                    Some(stamp) => vec![d.stamp_node(&stamp.by.as_string(), &stamp.at)],
                    None => d.terms(value),
                },
                "verified" => okf
                    .verified
                    .iter()
                    .map(|stamp| d.stamp_node(&stamp.by.as_string(), &stamp.at))
                    .collect(),
                "resource" | "page_resource" => fm
                    .text(key)
                    .map(|text| {
                        if iri_syntax().is_match(&text) {
                            Term::Iri(text)
                        } else {
                            Term::plain(text)
                        }
                    })
                    .into_iter()
                    .collect(),
                _ => match vocab.scalars.get(key) {
                    Some(def) if fm.text(key).is_some() => {
                        let raw = fm.text(key).unwrap_or_default();
                        vec![match def.value_type {
                            ScalarType::Number => match fm.number(key) {
                                Some(v) => Term::typed(v.to_string(), &xsd("double")),
                                None => Term::plain(raw),
                            },
                            ScalarType::Boolean => match fm.boolean(key) {
                                Some(v) => Term::typed(v.to_string(), &xsd("boolean")),
                                None => Term::plain(raw),
                            },
                            _ => Term::plain(raw),
                        }]
                    }
                    _ => d.terms(value),
                },
            };
            for term in terms {
                d.graph.add_iri(node, &predicate, term);
            }
        }
    }

    // Journals: identity is the date; only the type and the id are checked.
    for journal in &vault.journals {
        let node = page_node(profile, &journal.id, true);
        d.type_triple(&node, class(profile, "JournalPage"));
        d.graph
            .add_iri(&node, &meta(profile, "id"), Term::plain(journal.id.clone()));
        if let Some(type_name) = journal.frontmatter.text("type") {
            d.graph
                .add_iri(&node, &d.fm("type"), Term::plain(type_name));
        }
    }
    d.graph
}

/// The node a wikilink points at under `validate`'s membership test.
fn link_object(
    d: &mut DataBuilder<'_>,
    resolver: &Resolver<'_>,
    page_nodes: &[String],
    link: &Wikilink,
    vocab: &Vocabulary,
) -> String {
    if let Some(i) = resolver.page(&link.target) {
        return page_nodes[i].clone();
    }
    if let Some(tail) = vocab.tail_iris.get(&link.target) {
        d.type_triple(tail, class(d.profile, "LinkTarget"));
        return tail.clone();
    }
    format!(
        "{}unresolved/{}",
        namespace(d.profile),
        encode_component(&link.target)
    )
}

/// Shape IRIs and their `ns:ruleCode` annotation, as read from a shapes graph.
///
/// A convenience for harnesses that map a validation result's
/// `sh:sourceShape` back to the code `ontomark validate` reports.
#[must_use]
pub fn rule_codes(shapes_graph: &Graph, profile: &Profile) -> BTreeMap<String, String> {
    let property = rule_code_iri(profile);
    shapes_graph
        .iter()
        .filter(|(_, predicate, _)| *predicate == property)
        .filter_map(|(subject, _, object)| match (subject, object) {
            (Subject::Iri(shape), Term::Literal { value, .. }) => {
                Some((shape.clone(), value.clone()))
            }
            _ => None,
        })
        .collect()
}
