//! The typed model and its builder.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use indexmap::IndexMap;
use ontomark_parse::{Document, FrontmatterState, SourceDigest, Span, Value, Wikilink};
use serde::{Deserialize, Serialize};

use crate::iri::{Iri, PrefixMap};
use crate::kind::{Kind, PropertyKind};
use crate::policy;
use crate::profile::{
    CyclePolicy, IdentityDecision, KindDecision, Lifecycle, PredicateForm, Profile, SkosRelation,
};

/// A document's identity in the vault: its path under the pages root, `/`
/// separated, without `.md`. Distinct from the entity IRI.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocId(String);

impl DocId {
    /// From a vault-relative path; backslashes become `/` and a trailing
    /// `.md` is removed.
    ///
    /// ```
    /// # use ontomark_model::DocId;
    /// let id = DocId::new("space\\Launch Vehicle.md");
    /// assert_eq!(id.as_str(), "space/Launch Vehicle");
    /// assert_eq!(id.name(), "Launch Vehicle");
    /// ```
    #[must_use]
    pub fn new(path: impl AsRef<str>) -> Self {
        let p = path.as_ref().replace('\\', "/");
        let p = p.strip_prefix("./").unwrap_or(&p);
        Self(p.strip_suffix(".md").unwrap_or(p).to_owned())
    }

    /// The id text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The last path segment (the note name Obsidian shows).
    #[must_use]
    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The outcome of resolving a reference. Never a guess: more than one
/// candidate is [`Resolution::Ambiguous`], none is [`Resolution::Unresolved`]
/// (no target is synthesised), and an IRI the vault does not declare is
/// [`Resolution::External`] (ADR-2015, defect 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "iri", rename_all = "snake_case")]
pub enum Resolution {
    /// Exactly one entity.
    Resolved(Iri),
    /// Several entities match (sorted), or one IRI that several documents
    /// claim (a one-element list): either way, no single referent.
    Ambiguous(Vec<Iri>),
    /// Nothing matches.
    Unresolved,
    /// A full IRI that no page in the vault declares.
    External(Iri),
}

/// A reference written in a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// The text resolved: a wikilink target, or the plain value.
    pub written: String,
    /// The wikilink, when written as one.
    pub link: Option<Wikilink>,
    /// The resolution.
    pub state: Resolution,
    /// A document matched by path that has no entity identity, when the
    /// reference is unresolved for that reason.
    pub document: Option<DocId>,
}

/// A typed literal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Literal {
    /// Lexical form.
    pub lexical: String,
    /// Datatype IRI.
    pub datatype: Iri,
}

/// A statement's object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "snake_case")]
pub enum Object {
    /// An entity reference.
    Reference(Reference),
    /// A literal.
    Literal(Literal),
}

impl Object {
    /// The resolution, for a reference.
    #[must_use]
    pub fn resolution(&self) -> Option<&Resolution> {
        match self {
            Self::Reference(r) => Some(&r.state),
            Self::Literal(_) => None,
        }
    }
}

/// The logical shape of one statement, after the subject's kind is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementForm {
    /// `C rdfs:subClassOf D`.
    SubClassOf,
    /// `C rdfs:subClassOf ∃P.D`.
    Existential,
    /// `i rdf:type C`.
    ClassAssertion,
    /// `i P j`.
    ObjectAssertion,
    /// `x P "lit"`.
    DataAssertion,
    /// Annotation.
    Annotation,
    /// SKOS relation.
    Skos(SkosRelation),
    /// `owl:equivalentClass`.
    EquivalentClass,
    /// `owl:disjointWith`.
    DisjointWith,
    /// `owl:sameAs`.
    SameAs,
    /// `rdfs:subPropertyOf`.
    SubPropertyOf,
}

/// What a consumer may do with a statement (ADR-2015, defect 6). Unknown or
/// unsupported constructs are never silently downgraded to a generic edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "reason", rename_all = "snake_case")]
pub enum Disposition {
    /// Within the supported (OWL 2 EL) reasoning fragment.
    Supported,
    /// Kept and exported, not given to the reasoner, with the reason.
    PreservedNotReasoned(String),
    /// Ill-formed for its predicate (e.g. kind mismatch); kept for
    /// diagnostics only.
    Rejected(String),
}

/// Where a statement came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// Document.
    pub document: DocId,
    /// Frontmatter key.
    pub key: String,
    /// Sequence item index, for list values.
    pub item: Option<usize>,
    /// Source span of the item, when known.
    pub span: Option<Span>,
}

/// One assertion by an entity's page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statement {
    /// Full predicate IRI.
    pub predicate: Iri,
    /// Logical form.
    pub form: StatementForm,
    /// Object.
    pub object: Object,
    /// What a consumer may do with it.
    pub disposition: Disposition,
    /// Provenance in the source.
    pub origin: Origin,
}

/// An ontology entity declared by one page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// Identity.
    pub iri: Iri,
    /// Kind.
    pub kind: Kind,
    /// The declaring document.
    pub document: DocId,
    /// Display label.
    pub label: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Lifecycle status.
    pub lifecycle: Lifecycle,
    /// Visibility.
    pub public: bool,
    /// Explicitly mapped publication URL.
    pub publication_url: Option<String>,
    /// Assertions in source order.
    pub statements: Vec<Statement>,
    /// Keys neither reserved nor declared: preserved in the page, not
    /// interpreted.
    pub unmapped: Vec<String>,
}

/// What became of a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "snake_case")]
pub enum DocDisposition {
    /// Declares this entity.
    Entity(Iri),
    /// An ordinary note.
    NotOntology,
    /// Typed but has no stored identity; never minted implicitly.
    NoIdentity,
    /// Refused, with the reason (bad type, bad IRI, locked frontmatter).
    Refused(String),
    /// Claims an IRI another document also claims; neither is chosen.
    IdentityConflict(Iri),
}

/// A document the model saw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentRecord {
    /// Id.
    pub id: DocId,
    /// Digest of the source the model was built from.
    pub digest: SourceDigest,
    /// Disposition.
    pub disposition: DocDisposition,
    /// Visibility per profile.
    pub public: bool,
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational.
    Info,
    /// Worth fixing.
    Warning,
    /// Must be fixed for the affected document to be used.
    Error,
}

/// A finding from building the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity.
    pub severity: Severity,
    /// Stable code, e.g. `KIND_REFUSED`.
    pub code: String,
    /// Document, when the finding has one.
    pub document: Option<DocId>,
    /// Human-readable detail.
    pub message: String,
}

/// A corpus-policy finding: not a logical inconsistency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyFinding {
    /// Code, e.g. `SUBCLASS_CYCLE`.
    pub code: String,
    /// Severity from the profile's policy.
    pub severity: Severity,
    /// Entities involved, sorted.
    pub members: Vec<Iri>,
}

/// The typed model of a vault under one profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    /// The profile id it was built with.
    pub profile: String,
    /// Entities by IRI, in document order.
    pub entities: IndexMap<Iri, Entity>,
    /// Every document seen, in input order.
    pub documents: IndexMap<DocId, DocumentRecord>,
    /// IRIs claimed by more than one document.
    pub conflicts: BTreeMap<Iri, Vec<DocId>>,
    /// Build diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Corpus-policy findings.
    pub policy: Vec<PolicyFinding>,
    prefixes: PrefixMap,
    /// lower-cased label/alias/name → IRIs.
    names: BTreeMap<String, BTreeSet<Iri>>,
    /// lower-cased doc path → every document with that path (paths that
    /// differ only by case are distinct files on a case-sensitive disk).
    paths: BTreeMap<String, Vec<DocId>>,
    /// document → claimed IRI (unique or conflicted).
    claims: BTreeMap<DocId, Iri>,
    /// Every claimed IRI.
    claimed: BTreeSet<Iri>,
}

/// Diagnostic codes.
pub mod codes {
    /// The type header is not one the profile declares.
    pub const KIND_REFUSED: &str = "KIND_REFUSED";
    /// A typed page has no identity header.
    pub const IDENTITY_MISSING: &str = "IDENTITY_MISSING";
    /// The identity header is not an IRI.
    pub const IDENTITY_INVALID: &str = "IDENTITY_INVALID";
    /// Two documents claim one IRI.
    pub const IDENTITY_CONFLICT: &str = "IDENTITY_CONFLICT";
    /// The frontmatter is unsupported or malformed YAML.
    pub const FRONTMATTER_LOCKED: &str = "FRONTMATTER_LOCKED";
    /// A key is neither reserved nor declared.
    pub const UNMAPPED_KEY: &str = "UNMAPPED_KEY";
    /// A lifecycle value the profile does not declare.
    pub const UNKNOWN_LIFECYCLE: &str = "UNKNOWN_LIFECYCLE";
    /// `- [[A]]` written unquoted (a YAML nested list), recovered.
    pub const UNQUOTED_WIKILINK: &str = "UNQUOTED_WIKILINK";
    /// A reference matches several entities.
    pub const REFERENCE_AMBIGUOUS: &str = "REFERENCE_AMBIGUOUS";
    /// A value that cannot be an object of its predicate.
    pub const BAD_OBJECT: &str = "BAD_OBJECT";
    /// A statement rejected for its predicate's form.
    pub const STATEMENT_REJECTED: &str = "STATEMENT_REJECTED";
}

struct Pending<'a> {
    id: DocId,
    doc: &'a Document,
    kind: Kind,
    iri: Iri,
}

impl Model {
    /// Build the model of `docs` under `profile`.
    ///
    /// Two passes: identities and the name index first, so every reference
    /// resolves against the whole vault regardless of input order; then
    /// statements. Input order only decides output order.
    ///
    /// ```
    /// # use ontomark_model::{DeclarativeProfile, DocId, Model, Resolution};
    /// # use ontomark_parse::Document;
    /// let a = Document::parse("---\ntype: Class\niri: https://ex.org/Animal\n---\n");
    /// let b = Document::parse("---\ntype: Class\niri: https://ex.org/Dog\nrdfs:subClassOf:\n- '[[Animal]]'\n---\n");
    /// let profile = DeclarativeProfile::generic();
    /// let m = Model::build(&profile, [(DocId::new("Animal.md"), &a), (DocId::new("Dog.md"), &b)]);
    /// let dog = &m.entities[&ontomark_model::Iri::parse("https://ex.org/Dog").unwrap()];
    /// let st = &dog.statements[0];
    /// assert_eq!(st.object.resolution(), Some(&Resolution::Resolved(ontomark_model::Iri::parse("https://ex.org/Animal").unwrap())));
    /// ```
    pub fn build<'a, I>(profile: &dyn Profile, docs: I) -> Self
    where
        I: IntoIterator<Item = (DocId, &'a Document)>,
    {
        let mut m = Self {
            profile: profile.id().to_owned(),
            entities: IndexMap::new(),
            documents: IndexMap::new(),
            conflicts: BTreeMap::new(),
            diagnostics: Vec::new(),
            policy: Vec::new(),
            prefixes: profile.prefixes().clone(),
            names: BTreeMap::new(),
            paths: BTreeMap::new(),
            claims: BTreeMap::new(),
            claimed: BTreeSet::new(),
        };
        let mut pending: Vec<Pending<'a>> = Vec::new();
        let mut claimants: BTreeMap<Iri, Vec<DocId>> = BTreeMap::new();
        for (id, doc) in docs {
            m.paths
                .entry(fold(id.as_str()))
                .or_default()
                .push(id.clone());
            let public = profile.is_public(doc);
            let disposition = m.classify(profile, &id, doc, &mut pending, &mut claimants);
            m.documents.insert(
                id.clone(),
                DocumentRecord {
                    id,
                    digest: doc.digest(),
                    disposition,
                    public,
                },
            );
        }
        for (iri, ids) in &claimants {
            m.claimed.insert(iri.clone());
            for id in ids {
                m.claims.insert(id.clone(), iri.clone());
            }
            if ids.len() > 1 {
                m.conflicts.insert(iri.clone(), ids.clone());
                for id in ids {
                    if let Some(r) = m.documents.get_mut(id) {
                        r.disposition = DocDisposition::IdentityConflict(iri.clone());
                    }
                    m.diag(
                        Severity::Error,
                        codes::IDENTITY_CONFLICT,
                        Some(id),
                        format!(
                            "{iri} is also claimed by {} other document(s)",
                            ids.len() - 1
                        ),
                    );
                }
            }
        }
        // Name index over every claim, conflicted or not.
        for p in &pending {
            let mut names = vec![profile.label(&p.id, p.doc), p.id.name().to_owned()];
            names.extend(profile.aliases(p.doc));
            for n in names {
                let n = fold(n.trim());
                if !n.is_empty() {
                    m.names.entry(n).or_default().insert(p.iri.clone());
                }
            }
        }
        let kinds: BTreeMap<Iri, Kind> = pending
            .iter()
            .filter(|p| !m.conflicts.contains_key(&p.iri))
            .map(|p| (p.iri.clone(), p.kind))
            .collect();
        for p in &pending {
            if m.conflicts.contains_key(&p.iri) {
                continue;
            }
            let entity = m.entity(profile, p, &kinds);
            m.entities.insert(p.iri.clone(), entity);
        }
        m.policy = policy::subclass_cycles(&m, profile.cycle_policy());
        m
    }

    fn diag(&mut self, severity: Severity, code: &str, doc: Option<&DocId>, message: String) {
        self.diagnostics.push(Diagnostic {
            severity,
            code: code.to_owned(),
            document: doc.cloned(),
            message,
        });
    }

    fn classify<'a>(
        &mut self,
        profile: &dyn Profile,
        id: &DocId,
        doc: &'a Document,
        pending: &mut Vec<Pending<'a>>,
        claimants: &mut BTreeMap<Iri, Vec<DocId>>,
    ) -> DocDisposition {
        if let Some(fm) = doc.frontmatter() {
            if let FrontmatterState::Unsupported(r) | FrontmatterState::Malformed(r) = &fm.state {
                self.diag(
                    Severity::Error,
                    codes::FRONTMATTER_LOCKED,
                    Some(id),
                    r.clone(),
                );
                return DocDisposition::Refused(format!("frontmatter: {r}"));
            }
        }
        let kind = match profile.kind(doc) {
            KindDecision::NotOntology => return DocDisposition::NotOntology,
            KindDecision::Refused(r) => {
                self.diag(Severity::Error, codes::KIND_REFUSED, Some(id), r.clone());
                return DocDisposition::Refused(r);
            }
            KindDecision::Entity(k) => k,
        };
        match profile.identity(doc) {
            IdentityDecision::Missing => {
                self.diag(
                    Severity::Warning,
                    codes::IDENTITY_MISSING,
                    Some(id),
                    "typed page without a stored IRI; none is minted".into(),
                );
                DocDisposition::NoIdentity
            }
            IdentityDecision::Invalid(r) => {
                self.diag(
                    Severity::Error,
                    codes::IDENTITY_INVALID,
                    Some(id),
                    r.clone(),
                );
                DocDisposition::Refused(r)
            }
            IdentityDecision::Iri(iri) => {
                claimants.entry(iri.clone()).or_default().push(id.clone());
                pending.push(Pending {
                    id: id.clone(),
                    doc,
                    kind,
                    iri: iri.clone(),
                });
                DocDisposition::Entity(iri)
            }
        }
    }

    fn entity(
        &mut self,
        profile: &dyn Profile,
        p: &Pending<'_>,
        kinds: &BTreeMap<Iri, Kind>,
    ) -> Entity {
        let lifecycle = profile.lifecycle(p.doc);
        if let Lifecycle::Unknown(s) = &lifecycle {
            self.diag(
                Severity::Warning,
                codes::UNKNOWN_LIFECYCLE,
                Some(&p.id),
                format!("status `{s}` is not declared"),
            );
        }
        let reasoned = match &lifecycle {
            Lifecycle::Declared(s) => profile.lifecycle_def(s).is_none_or(|d| d.reasoned),
            _ => true,
        };
        let mut e = Entity {
            iri: p.iri.clone(),
            kind: p.kind,
            document: p.id.clone(),
            label: profile.label(&p.id, p.doc),
            aliases: profile.aliases(p.doc),
            lifecycle: lifecycle.clone(),
            public: profile.is_public(p.doc),
            publication_url: profile.publication_url(&p.iri),
            statements: Vec::new(),
            unmapped: Vec::new(),
        };
        let Some(fm) = p.doc.frontmatter() else {
            return e;
        };
        let src = p.doc.source();
        for entry in &fm.entries {
            if profile.is_reserved(&entry.key) {
                continue;
            }
            let Some(pred) = profile.predicate(&entry.key) else {
                e.unmapped.push(entry.key.clone());
                self.diag(
                    Severity::Info,
                    codes::UNMAPPED_KEY,
                    Some(&p.id),
                    format!("key `{}` is preserved, not interpreted", entry.key),
                );
                continue;
            };
            // Gather (item index, span, object) triples.
            let links = entry.wikilinks(src);
            let mut objects: Vec<(Option<usize>, Option<Span>, Object)> = Vec::new();
            let items: Vec<(Option<usize>, &Value)> = match &entry.value {
                Value::Seq(v) => v.iter().enumerate().map(|(i, x)| (Some(i), x)).collect(),
                Value::Null => Vec::new(),
                other => vec![(None, other)],
            };
            for (idx, item) in items {
                let span = idx.and_then(|i| entry.items.get(i).copied());
                if let Some(fl) = links.iter().find(|l| l.item == idx) {
                    if fl.recovered {
                        self.diag(
                            Severity::Warning,
                            codes::UNQUOTED_WIKILINK,
                            Some(&p.id),
                            format!(
                                "`{}` item {} is an unquoted wikilink",
                                entry.key,
                                idx.unwrap_or(0)
                            ),
                        );
                    }
                    if pred.form == PredicateForm::DataAssertion {
                        self.diag(
                            Severity::Warning,
                            codes::BAD_OBJECT,
                            Some(&p.id),
                            format!(
                                "`{}` is a data predicate; a wikilink is not a literal",
                                entry.key
                            ),
                        );
                        continue;
                    }
                    let r = self.resolve_with(profile, &fl.link.target);
                    objects.push((
                        idx,
                        span,
                        Object::Reference(Reference {
                            link: Some(fl.link.clone()),
                            ..r
                        }),
                    ));
                    continue;
                }
                let Some(text) = item.scalar_text() else {
                    self.diag(
                        Severity::Warning,
                        codes::BAD_OBJECT,
                        Some(&p.id),
                        format!(
                            "`{}` holds a {}, not a scalar or link",
                            entry.key,
                            item.type_name()
                        ),
                    );
                    continue;
                };
                let obj = match pred.form {
                    PredicateForm::DataAssertion => Object::Literal(Literal {
                        lexical: text,
                        datatype: pred.datatype.clone().unwrap_or_else(xsd_string),
                    }),
                    PredicateForm::Annotation => match self.prefixes.expand(text.trim()) {
                        Ok(_) if text.contains(':') && !text.contains(char::is_whitespace) => {
                            Object::Reference(self.resolve_with(profile, &text))
                        }
                        _ => Object::Literal(Literal {
                            lexical: text,
                            datatype: xsd_string(),
                        }),
                    },
                    _ => Object::Reference(self.resolve_with(profile, &text)),
                };
                objects.push((idx, span, obj));
            }
            for (item, span, object) in objects {
                if let Some(Resolution::Ambiguous(c)) = object.resolution() {
                    let n = c.len();
                    self.diag(
                        Severity::Warning,
                        codes::REFERENCE_AMBIGUOUS,
                        Some(&p.id),
                        format!("`{}` item {:?} matches {n} entities", entry.key, item),
                    );
                }
                let (form, mut disposition) = shape(pred.form, &pred.iri, p.kind, &object, kinds);
                if disposition == Disposition::Supported && self.conflicts.contains_key(&pred.iri) {
                    disposition = Disposition::PreservedNotReasoned(
                        "predicate IRI is claimed by several documents".into(),
                    );
                }
                if !reasoned && disposition == Disposition::Supported {
                    disposition =
                        Disposition::PreservedNotReasoned("lifecycle state is not reasoned".into());
                }
                if let Disposition::Rejected(r) = &disposition {
                    self.diag(
                        Severity::Warning,
                        codes::STATEMENT_REJECTED,
                        Some(&p.id),
                        format!("`{}`: {r}", entry.key),
                    );
                }
                e.statements.push(Statement {
                    predicate: pred.iri.clone(),
                    form,
                    object,
                    disposition,
                    origin: Origin {
                        document: p.id.clone(),
                        key: entry.key.clone(),
                        item,
                        span,
                    },
                });
            }
        }
        e
    }

    /// Resolve a reference target (a wikilink target, CURIE, IRI or name)
    /// against this model.
    ///
    /// Candidates are every entity whose label, alias or note name equals the
    /// target (case-insensitive), whose document path equals it, whose IRI
    /// is one of the profile's candidates for it, or (when the target reads
    /// as a declared-prefix CURIE or absolute IRI) whose IRI it is.
    ///
    /// - one candidate claimed by one document: [`Resolution::Resolved`];
    /// - several candidates, or one IRI several documents claim:
    ///   [`Resolution::Ambiguous`];
    /// - none, and the target reads as an IRI: [`Resolution::External`];
    /// - none otherwise: [`Resolution::Unresolved`].
    ///
    /// So `Resolved` always names a key of [`Model::entities`].
    ///
    /// IRI tails are never indexed: `urn:a:x` and `https://b/x` do not meet
    /// on `x`.
    #[must_use]
    pub fn resolve_with(&self, profile: &dyn Profile, target: &str) -> Reference {
        let t = target.trim();
        let mk = |state, document| Reference {
            written: t.to_owned(),
            link: None,
            state,
            document,
        };
        // A target that reads as a CURIE or IRI is one candidate among the
        // names, not a shortcut past them: `[[Foo:Bar]]` may be an alias.
        let as_iri = (t.contains(':') && !t.contains(char::is_whitespace))
            .then(|| self.prefixes.expand(t).ok())
            .flatten();
        let key = fold(t);
        let mut cands: BTreeSet<Iri> = self.names.get(&key).cloned().unwrap_or_default();
        if let Some(iri) = &as_iri {
            if self.claimed.contains(iri) {
                cands.insert(iri.clone());
            }
        }
        let mut doc_without_identity = None;
        for id in self.paths.get(&key).into_iter().flatten() {
            match self.claims.get(id) {
                Some(iri) => {
                    cands.insert(iri.clone());
                }
                None => {
                    doc_without_identity.get_or_insert_with(|| id.clone());
                }
            }
        }
        for c in profile.reference_candidates(t) {
            if self.claimed.contains(&c) {
                cands.insert(c);
            }
        }
        match cands.len() {
            0 => match as_iri {
                Some(iri) => mk(Resolution::External(iri), None),
                None => mk(Resolution::Unresolved, doc_without_identity),
            },
            1 => {
                let iri = cands.into_iter().next().expect("one");
                // Several documents claim it: no single entity to resolve to.
                if self.conflicts.contains_key(&iri) {
                    mk(Resolution::Ambiguous(vec![iri]), None)
                } else {
                    mk(Resolution::Resolved(iri), None)
                }
            }
            _ => mk(Resolution::Ambiguous(cands.into_iter().collect()), None),
        }
    }

    /// Every entity IRI carrying `name` as a label, alias or note name
    /// (trimmed, case-insensitive), sorted. Conflicted claims are included.
    #[must_use]
    pub fn entities_named(&self, name: &str) -> Vec<Iri> {
        self.names
            .get(&fold(name.trim()))
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// `true` when some document claims `iri` (uniquely or in conflict).
    #[must_use]
    pub fn is_claimed(&self, iri: &Iri) -> bool {
        self.claimed.contains(iri)
    }

    /// The entity a document declares.
    #[must_use]
    pub fn entity_of(&self, doc: &DocId) -> Option<&Entity> {
        match &self.documents.get(doc)?.disposition {
            DocDisposition::Entity(iri) => self.entities.get(iri),
            _ => None,
        }
    }

    /// Count of every statement's resolution state, for reports:
    /// `(resolved, ambiguous, unresolved, external)`.
    #[must_use]
    pub fn resolution_counts(&self) -> (usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0);
        for s in self.entities.values().flat_map(|e| &e.statements) {
            match s.object.resolution() {
                Some(Resolution::Resolved(_)) => c.0 += 1,
                Some(Resolution::Ambiguous(_)) => c.1 += 1,
                Some(Resolution::Unresolved) => c.2 += 1,
                Some(Resolution::External(_)) => c.3 += 1,
                None => {}
            }
        }
        c
    }
}

fn xsd_string() -> Iri {
    Iri::parse("http://www.w3.org/2001/XMLSchema#string").expect("static IRI")
}

type KindTest = fn(Kind) -> bool;

/// Decide the statement form and disposition from the predicate's form, the
/// subject's kind and the object.
fn shape(
    form: PredicateForm,
    predicate: &Iri,
    subject: Kind,
    object: &Object,
    kinds: &BTreeMap<Iri, Kind>,
) -> (StatementForm, Disposition) {
    use Disposition::{PreservedNotReasoned as Keep, Rejected as Reject, Supported};
    let class = Kind::Class;
    let ind = Kind::Individual;
    // (statement form, required subject kind, required object kind)
    let (sf, subj_ok, obj_need): (StatementForm, bool, Option<KindTest>) = match form {
        PredicateForm::Annotation => return (StatementForm::Annotation, Keep("annotation".into())),
        PredicateForm::Skos(r) => {
            return (
                StatementForm::Skos(r),
                Keep("SKOS relations are not OWL axioms".into()),
            )
        }
        PredicateForm::DataAssertion => {
            let st = StatementForm::DataAssertion;
            return match (subject, object) {
                (_, Object::Reference(_)) => (
                    st,
                    Reject("data predicate with a non-literal object".into()),
                ),
                (Kind::Individual, Object::Literal(_)) => {
                    (st, Keep("data assertions are not classified".into()))
                }
                _ => (
                    st,
                    Keep("data assertion on a non-individual (annotation-like)".into()),
                ),
            };
        }
        PredicateForm::SubClassOf => (
            StatementForm::SubClassOf,
            subject == class,
            Some(|k| k == Kind::Class),
        ),
        PredicateForm::EquivalentClass => (
            StatementForm::EquivalentClass,
            subject == class,
            Some(|k| k == Kind::Class),
        ),
        PredicateForm::DisjointWith => (
            StatementForm::DisjointWith,
            subject == class,
            Some(|k| k == Kind::Class),
        ),
        PredicateForm::ClassAssertion => (
            StatementForm::ClassAssertion,
            subject == ind,
            Some(|k| k == Kind::Class),
        ),
        PredicateForm::SameAs => (
            StatementForm::SameAs,
            subject == ind,
            Some(|k| k == Kind::Individual),
        ),
        PredicateForm::SubPropertyOf => (
            StatementForm::SubPropertyOf,
            subject.is_property(),
            Some(Kind::is_property),
        ),
        PredicateForm::Existential => match subject {
            Kind::Class => (StatementForm::Existential, true, Some(|k| k == Kind::Class)),
            Kind::Individual => (
                StatementForm::ObjectAssertion,
                true,
                Some(|k| k == Kind::Individual),
            ),
            Kind::Property(_) => (StatementForm::Existential, false, None),
        },
        PredicateForm::ObjectAssertion => {
            if subject == class {
                return (
                    StatementForm::ObjectAssertion,
                    Keep("object assertion on a class would need punning".into()),
                );
            }
            (
                StatementForm::ObjectAssertion,
                subject == ind,
                Some(|k| k == Kind::Individual),
            )
        }
    };
    if !subj_ok {
        return (
            sf,
            Reject(format!("{sf:?} does not apply to a {subject:?} subject")),
        );
    }
    // An object predicate whose IRI is itself a page must be an object
    // property: one IRI cannot be both a data property (or a class) and an
    // object property.
    if matches!(
        sf,
        StatementForm::Existential | StatementForm::ObjectAssertion
    ) {
        if let Some(k) = kinds.get(predicate) {
            if *k != Kind::Property(PropertyKind::Object) {
                return (
                    sf,
                    Reject(format!(
                        "predicate {predicate} is a {k:?}, not an object property"
                    )),
                );
            }
        }
    }
    let Object::Reference(r) = object else {
        return (
            sf,
            Reject("a literal cannot be the object of a logical axiom".into()),
        );
    };
    if sf == StatementForm::SubPropertyOf && subject == Kind::Property(PropertyKind::Annotation) {
        return (
            sf,
            Keep("annotation sub-properties are not logical axioms".into()),
        );
    }
    match &r.state {
        Resolution::Resolved(iri) => match (kinds.get(iri), obj_need) {
            (None, _) => (sf, Keep("target identity is conflicted".into())),
            (Some(k), _) if sf == StatementForm::SubPropertyOf && *k != subject => (
                sf,
                Reject(format!("a {subject:?} cannot be a sub-property of a {k:?}")),
            ),
            // The EL projection has object-property hierarchies only.
            (Some(_), _)
                if sf == StatementForm::SubPropertyOf
                    && subject == Kind::Property(PropertyKind::Data) =>
            {
                (
                    sf,
                    Keep(
                        "data sub-property hierarchies are not projected to the EL reasoner".into(),
                    ),
                )
            }
            (Some(k), Some(ok)) if !ok(*k) => (sf, Reject(format!("{sf:?} target is a {k:?}"))),
            _ => (sf, Supported),
        },
        Resolution::External(_) => (sf, Keep("target IRI is not declared in this vault".into())),
        Resolution::Ambiguous(_) => (sf, Keep("target is ambiguous".into())),
        Resolution::Unresolved => (sf, Keep("target is unresolved".into())),
    }
}

impl CyclePolicy {
    /// The severity a finding under this policy carries, if any.
    #[must_use]
    pub fn severity(self) -> Option<Severity> {
        match self {
            Self::Allow => None,
            Self::Warn => Some(Severity::Warning),
            Self::Refuse => Some(Severity::Error),
        }
    }
}
/// Caseless key for titles, aliases, note names and paths.
///
/// Uppercasing first applies the full Unicode mappings that lowercasing
/// alone skips (`ß` → `SS`), so `Straße`, `STRASSE` and `strasse` share one
/// key. Identity checks (minting, resolution) must all go through this.
pub(crate) fn fold(s: &str) -> String {
    s.to_uppercase().to_lowercase()
}
