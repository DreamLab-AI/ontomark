//! Profiles: the rules a vault's frontmatter is read by, injected rather
//! than hard-coded.
//!
//! The model knows OWL/RDF/SKOS shapes; a [`Profile`] says which keys carry
//! identity, type, labels and visibility, which keys are assertions with
//! which full predicate IRI and form, which lifecycle values exist, and which
//! corpus policies apply. [`DeclarativeProfile`] implements the trait from a
//! serialisable [`ProfileSpec`]; [`DeclarativeProfile::generic`] is the
//! estate-free default for an ordinary vault. Estate rules belong in an
//! estate profile (`profiles/estate`), never in this crate.

use indexmap::IndexMap;
use ontomark_parse::{Document, Value};
use serde::{Deserialize, Serialize};

use crate::iri::{Iri, IriError, PrefixMap};
use crate::kind::{Kind, PropertyKind};
use crate::model::DocId;

/// How a predicate's assertions are to be read. Chosen by the profile per
/// key; one assertion has exactly one form (ADR-2015, defect 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateForm {
    /// `C rdfs:subClassOf D`.
    SubClassOf,
    /// `i rdf:type C`.
    ClassAssertion,
    /// On a class, `C rdfs:subClassOf [owl:onProperty P; owl:someValuesFrom D]`;
    /// on an individual, the object property assertion `i P j`.
    Existential,
    /// `i P j` between individuals.
    ObjectAssertion,
    /// `x P "literal"^^datatype`.
    DataAssertion,
    /// `x P v` as an annotation: never reasoned over.
    Annotation,
    /// A SKOS concept relation (not OWL subsumption).
    Skos(SkosRelation),
    /// `owl:equivalentClass`.
    EquivalentClass,
    /// `owl:disjointWith`.
    DisjointWith,
    /// `owl:sameAs` between individuals.
    SameAs,
    /// `rdfs:subPropertyOf`.
    SubPropertyOf,
}

/// SKOS mapping and semantic relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkosRelation {
    /// `skos:broader`.
    Broader,
    /// `skos:narrower`.
    Narrower,
    /// `skos:related`.
    Related,
    /// `skos:exactMatch`.
    ExactMatch,
    /// `skos:closeMatch`.
    CloseMatch,
}

/// A declared predicate: frontmatter key → full IRI and form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateDef {
    /// The frontmatter key.
    pub key: String,
    /// The full predicate IRI.
    pub iri: Iri,
    /// How its assertions are read.
    pub form: PredicateForm,
    /// Literal datatype for [`PredicateForm::DataAssertion`] (default `xsd:string`).
    pub datatype: Option<Iri>,
    /// Declared `rdfs:domain`. Informational for consumers: it is never a
    /// required-field rule (ADR-2015, defect 7).
    pub domain: Option<Iri>,
    /// Declared `rdfs:range`; same rule as [`Self::domain`].
    pub range: Option<Iri>,
    /// Declared transitive.
    pub transitive: bool,
}

/// What a page's declared type means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KindDecision {
    /// An ontology entity of this kind.
    Entity(Kind),
    /// An ordinary note: no entity, no assertions.
    NotOntology,
    /// The type is not one the profile accepts; never coerced.
    Refused(String),
}

/// What a page's identity header holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityDecision {
    /// A valid IRI, stored verbatim.
    Iri(Iri),
    /// No identity header. The model never mints one implicitly.
    Missing,
    /// A header that is not a valid IRI or expandable CURIE.
    Invalid(String),
}

/// A page's lifecycle status as the profile reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "snake_case")]
pub enum Lifecycle {
    /// A status the profile declares.
    Declared(String),
    /// A status value the profile does not declare (kept, reported).
    Unknown(String),
    /// No status header.
    Absent,
}

/// A lifecycle state the profile declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleDef {
    /// The header value, e.g. `stable`.
    pub id: String,
    /// Whether entities in this state are given to the reasoner.
    #[serde(default = "yes")]
    pub reasoned: bool,
    /// Whether entities in this state may be published.
    #[serde(default = "yes")]
    pub published: bool,
}

fn yes() -> bool {
    true
}

/// What a subclass cycle means for this corpus. A cycle is a policy concern,
/// not an OWL inconsistency (ADR-2015, defect 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CyclePolicy {
    /// Cycles are allowed (they make the members equivalent).
    Allow,
    /// Report cycles as warnings.
    #[default]
    Warn,
    /// Report cycles as errors.
    Refuse,
}

/// The rules the model reads a vault by.
pub trait Profile {
    /// Profile identity and version, e.g. `generic/1`.
    fn id(&self) -> &str;
    /// Prefixes for CURIEs in headers and keys.
    fn prefixes(&self) -> &PrefixMap;
    /// The page's kind.
    fn kind(&self, doc: &Document) -> KindDecision;
    /// The page's stored identity.
    fn identity(&self, doc: &Document) -> IdentityDecision;
    /// The page's display label.
    fn label(&self, id: &DocId, doc: &Document) -> String;
    /// The page's aliases.
    fn aliases(&self, doc: &Document) -> Vec<String>;
    /// Whether the page is public.
    fn is_public(&self, doc: &Document) -> bool;
    /// The page's lifecycle status.
    fn lifecycle(&self, doc: &Document) -> Lifecycle;
    /// The declared lifecycle state, when `status` names one.
    fn lifecycle_def(&self, status: &str) -> Option<&LifecycleDef>;
    /// The predicate a frontmatter key asserts, if any.
    fn predicate(&self, key: &str) -> Option<PredicateDef>;
    /// Every predicate the profile declares by key (not CURIE keys), in
    /// declaration order. Used for property axioms. Default: none.
    fn predicates(&self) -> Vec<PredicateDef> {
        Vec::new()
    }
    /// `true` for keys the profile consumes as metadata (identity, type,
    /// labels, provenance…) rather than as assertions.
    fn is_reserved(&self, key: &str) -> bool;
    /// Extra candidate IRIs for a reference target (e.g. an estate's
    /// `namespace + slug` convention). Only candidates that name an existing
    /// entity are used. Default: none.
    fn reference_candidates(&self, _target: &str) -> Vec<Iri> {
        Vec::new()
    }
    /// The profile's minting rule: a candidate IRI for a new entity.
    ///
    /// Callers go through [`crate::Model::mint_iri`], which applies the
    /// title and claimed-IRI guards around this rule. A rule refuses with
    /// [`crate::MintRefusal`] when it has nothing to offer. Never called when
    /// reading or indexing a vault. Default: [`crate::MintRefusal::NoRule`].
    ///
    /// # Errors
    /// [`crate::MintRefusal`].
    fn mint_iri(
        &self,
        _kind: Kind,
        _title: &str,
        _ctx: &crate::mint::MintContext<'_>,
    ) -> Result<Iri, crate::mint::MintRefusal> {
        Err(crate::mint::MintRefusal::NoRule)
    }
    /// The public URL for an entity, when the profile maps one explicitly.
    fn publication_url(&self, _iri: &Iri) -> Option<String> {
        None
    }
    /// Subclass-cycle policy.
    fn cycle_policy(&self) -> CyclePolicy {
        CyclePolicy::Warn
    }
}

/// The declared meaning of a type value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KindSpec {
    /// `owl:Class`.
    Class,
    /// `owl:NamedIndividual`.
    Individual,
    /// `owl:ObjectProperty`.
    ObjectProperty,
    /// `owl:DatatypeProperty`.
    DataProperty,
    /// `owl:AnnotationProperty`.
    AnnotationProperty,
    /// An ordinary note.
    NotOntology,
}

impl KindSpec {
    fn decision(self) -> KindDecision {
        match self {
            Self::Class => KindDecision::Entity(Kind::Class),
            Self::Individual => KindDecision::Entity(Kind::Individual),
            Self::ObjectProperty => KindDecision::Entity(Kind::Property(PropertyKind::Object)),
            Self::DataProperty => KindDecision::Entity(Kind::Property(PropertyKind::Data)),
            Self::AnnotationProperty => {
                KindDecision::Entity(Kind::Property(PropertyKind::Annotation))
            }
            Self::NotOntology => KindDecision::NotOntology,
        }
    }
}

/// A predicate declaration in a [`ProfileSpec`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateSpec {
    /// Predicate IRI or CURIE.
    pub iri: String,
    /// Form.
    pub form: PredicateForm,
    /// Literal datatype (IRI or CURIE) for data assertions.
    #[serde(default)]
    pub datatype: Option<String>,
    /// `rdfs:domain` (IRI or CURIE).
    #[serde(default)]
    pub domain: Option<String>,
    /// `rdfs:range` (IRI or CURIE).
    #[serde(default)]
    pub range: Option<String>,
    /// Transitive.
    #[serde(default)]
    pub transitive: bool,
}

/// An explicit public-URL mapping: entity IRIs starting with `namespace`
/// publish at `base_url + rest + suffix`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationSpec {
    /// The IRI namespace this mapping applies to.
    pub namespace: String,
    /// The URL prefix.
    pub base_url: String,
    /// Appended after the IRI tail.
    #[serde(default)]
    pub suffix: String,
}

/// A data-only profile description (JSON/YAML/TOML via serde).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileSpec {
    /// Profile identity and version.
    pub id: String,
    /// Prefixes, merged over [`PrefixMap::standard`].
    #[serde(default)]
    pub prefixes: IndexMap<String, String>,
    /// Header naming the page's type.
    #[serde(default = "d_type")]
    pub type_key: String,
    /// Type values and their kinds. Any other value is refused.
    #[serde(default)]
    pub kinds: IndexMap<String, KindSpec>,
    /// Kind of a page with no type header.
    #[serde(default = "d_untyped")]
    pub untyped: KindSpec,
    /// Header holding the stored entity IRI.
    #[serde(default = "d_identity")]
    pub identity_key: String,
    /// Header holding the label; the file name when absent.
    #[serde(default = "d_title")]
    pub title_key: String,
    /// Header holding aliases.
    #[serde(default = "d_aliases")]
    pub aliases_key: String,
    /// Header holding visibility.
    #[serde(default = "d_public")]
    pub public_key: String,
    /// Accept `"true"`/`"false"` strings as booleans for `public_key`.
    #[serde(default)]
    pub lenient_booleans: bool,
    /// Header holding lifecycle status.
    #[serde(default = "d_status")]
    pub lifecycle_key: String,
    /// Declared lifecycle states. Empty means any value is accepted as declared.
    #[serde(default)]
    pub lifecycle: Vec<LifecycleDef>,
    /// Assertion keys.
    #[serde(default)]
    pub predicates: IndexMap<String, PredicateSpec>,
    /// Treat a key that is a CURIE of a well-known RDFS/OWL/SKOS predicate
    /// (`rdfs:subClassOf`, `skos:broader`, …) as that predicate.
    #[serde(default = "yes")]
    pub curie_keys: bool,
    /// Further metadata keys consumed without assertion.
    #[serde(default)]
    pub reserved: Vec<String>,
    /// Namespaces for `namespace + target` reference candidates.
    #[serde(default)]
    pub reference_namespaces: Vec<String>,
    /// Explicit public-URL mapping.
    #[serde(default)]
    pub publication: Option<PublicationSpec>,
    /// Subclass-cycle policy.
    #[serde(default)]
    pub cycle_policy: CyclePolicy,
    /// Minting rule (`namespace(kind) + slugify(title)`); absent = never mint.
    #[serde(default)]
    pub mint: Option<crate::mint::MintSpec>,
}

fn d_type() -> String {
    "type".into()
}
fn d_untyped() -> KindSpec {
    KindSpec::NotOntology
}
fn d_identity() -> String {
    "iri".into()
}
fn d_title() -> String {
    "title".into()
}
fn d_aliases() -> String {
    "aliases".into()
}
fn d_public() -> String {
    "public".into()
}
fn d_status() -> String {
    "status".into()
}

/// A profile spec that could not be compiled.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    /// A prefix namespace, predicate, datatype, domain or range is not an IRI.
    #[error("{context}: {source}")]
    Iri {
        /// Where.
        context: String,
        /// Why.
        source: IriError,
    },
    /// A data assertion predicate without a usable datatype, or similar.
    #[error("{0}")]
    Invalid(String),
}

/// A compiled [`ProfileSpec`].
#[derive(Debug, Clone)]
pub struct DeclarativeProfile {
    spec: ProfileSpec,
    prefixes: PrefixMap,
    predicates: IndexMap<String, PredicateDef>,
}

impl DeclarativeProfile {
    /// Compile and validate `spec`.
    ///
    /// # Errors
    /// [`ProfileError`] when an IRI or CURIE in the spec does not expand.
    pub fn new(spec: ProfileSpec) -> Result<Self, ProfileError> {
        let mut prefixes = PrefixMap::standard();
        for (p, ns) in &spec.prefixes {
            let iri = Iri::parse(ns.clone()).map_err(|source| ProfileError::Iri {
                context: format!("prefix `{p}`"),
                source,
            })?;
            prefixes.insert(p.clone(), iri);
        }
        let exp = |ctx: String, s: &str| {
            prefixes.expand(s).map_err(|source| ProfileError::Iri {
                context: ctx,
                source,
            })
        };
        let mut predicates = IndexMap::new();
        for (key, p) in &spec.predicates {
            let iri = exp(format!("predicate `{key}`"), &p.iri)?;
            let opt = |what: &str, v: &Option<String>| {
                v.as_deref()
                    .map(|s| exp(format!("{what} of `{key}`"), s))
                    .transpose()
            };
            let mut datatype = opt("datatype", &p.datatype)?;
            if p.form == PredicateForm::DataAssertion && datatype.is_none() {
                datatype = Some(exp("xsd:string".into(), "xsd:string")?);
            }
            predicates.insert(
                key.clone(),
                PredicateDef {
                    key: key.clone(),
                    iri,
                    form: p.form,
                    datatype,
                    domain: opt("domain", &p.domain)?,
                    range: opt("range", &p.range)?,
                    transitive: p.transitive,
                },
            );
        }
        if let Some(m) = &spec.mint {
            for ns in [&m.class, &m.individual, &m.property].into_iter().flatten() {
                Iri::parse(format!("{ns}x")).map_err(|source| ProfileError::Iri {
                    context: "mint namespace".into(),
                    source,
                })?;
            }
        }
        for ns in &spec.reference_namespaces {
            Iri::parse(format!("{ns}x")).map_err(|source| ProfileError::Iri {
                context: "reference namespace".into(),
                source,
            })?;
        }
        Ok(Self {
            spec,
            prefixes,
            predicates,
        })
    }

    /// The estate-free profile for an ordinary vault (`generic/1`).
    ///
    /// * identity header `iri`; type header `type` with the OWL names
    ///   `Class`, `NamedIndividual`, `ObjectProperty`, `DatatypeProperty`,
    ///   `AnnotationProperty`; an untyped page is an ordinary note;
    /// * no assertion keys except CURIE keys of well-known predicates
    ///   (`rdfs:subClassOf`, `rdf:type`, `owl:equivalentClass`,
    ///   `skos:broader`, …), which the author writes explicitly;
    /// * no reference conventions, no publication mapping, cycles warned.
    ///
    /// ```
    /// # use ontomark_model::{DeclarativeProfile, Profile};
    /// let p = DeclarativeProfile::generic();
    /// assert_eq!(p.id(), "generic/1");
    /// assert!(p.predicate("is-a").is_none());
    /// assert!(p.predicate("rdfs:subClassOf").is_some());
    /// ```
    #[must_use]
    pub fn generic() -> Self {
        let kinds = [
            ("Class", KindSpec::Class),
            ("NamedIndividual", KindSpec::Individual),
            ("ObjectProperty", KindSpec::ObjectProperty),
            ("DatatypeProperty", KindSpec::DataProperty),
            ("AnnotationProperty", KindSpec::AnnotationProperty),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let spec = ProfileSpec {
            id: "generic/1".into(),
            prefixes: IndexMap::new(),
            type_key: d_type(),
            kinds,
            untyped: KindSpec::NotOntology,
            identity_key: d_identity(),
            title_key: d_title(),
            aliases_key: d_aliases(),
            public_key: d_public(),
            lenient_booleans: false,
            lifecycle_key: d_status(),
            lifecycle: Vec::new(),
            predicates: IndexMap::new(),
            curie_keys: true,
            reserved: ["tags", "cssclasses", "description", "created", "updated"]
                .map(str::to_owned)
                .to_vec(),
            reference_namespaces: Vec::new(),
            publication: None,
            cycle_policy: CyclePolicy::Warn,
            mint: None,
        };
        Self::new(spec).expect("the generic profile is valid")
    }

    /// The spec this profile was compiled from.
    #[must_use]
    pub fn spec(&self) -> &ProfileSpec {
        &self.spec
    }
}

/// Well-known predicates a CURIE key may name.
fn well_known(prefixes: &PrefixMap, key: &str) -> Option<PredicateDef> {
    let form = match key {
        "rdfs:subClassOf" => PredicateForm::SubClassOf,
        "rdf:type" => PredicateForm::ClassAssertion,
        "owl:equivalentClass" => PredicateForm::EquivalentClass,
        "owl:disjointWith" => PredicateForm::DisjointWith,
        "owl:sameAs" => PredicateForm::SameAs,
        "rdfs:subPropertyOf" => PredicateForm::SubPropertyOf,
        "skos:broader" => PredicateForm::Skos(SkosRelation::Broader),
        "skos:narrower" => PredicateForm::Skos(SkosRelation::Narrower),
        "skos:related" => PredicateForm::Skos(SkosRelation::Related),
        "skos:exactMatch" => PredicateForm::Skos(SkosRelation::ExactMatch),
        "skos:closeMatch" => PredicateForm::Skos(SkosRelation::CloseMatch),
        "rdfs:label" | "rdfs:comment" | "rdfs:seeAlso" | "skos:definition" | "skos:prefLabel"
        | "skos:altLabel" => PredicateForm::Annotation,
        k if k.starts_with("dcterms:") => PredicateForm::Annotation,
        _ => return None,
    };
    Some(PredicateDef {
        key: key.to_owned(),
        iri: prefixes.expand(key).ok()?,
        form,
        datatype: None,
        domain: None,
        range: None,
        transitive: false,
    })
}

impl Profile for DeclarativeProfile {
    fn id(&self) -> &str {
        &self.spec.id
    }

    fn prefixes(&self) -> &PrefixMap {
        &self.prefixes
    }

    fn kind(&self, doc: &Document) -> KindDecision {
        match doc.value(&self.spec.type_key) {
            None | Some(Value::Null) => self.spec.untyped.decision(),
            Some(v) => match v.scalar_text() {
                Some(t) => match self.spec.kinds.get(&t) {
                    Some(k) => k.decision(),
                    None => KindDecision::Refused(format!(
                        "type `{t}` is not declared by profile {}",
                        self.spec.id
                    )),
                },
                None => KindDecision::Refused(format!("type is a {}, not a scalar", v.type_name())),
            },
        }
    }

    fn identity(&self, doc: &Document) -> IdentityDecision {
        match doc.value(&self.spec.identity_key) {
            None | Some(Value::Null) => IdentityDecision::Missing,
            Some(Value::String(s)) => match self.prefixes.expand(s.trim()) {
                Ok(iri) => IdentityDecision::Iri(iri),
                Err(e) => IdentityDecision::Invalid(e.to_string()),
            },
            Some(v) => IdentityDecision::Invalid(format!("identity is a {}", v.type_name())),
        }
    }

    fn label(&self, id: &DocId, doc: &Document) -> String {
        doc.value(&self.spec.title_key)
            .and_then(Value::scalar_text)
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| id.name().to_owned())
    }

    fn aliases(&self, doc: &Document) -> Vec<String> {
        doc.value(&self.spec.aliases_key)
            .map(Value::texts)
            .unwrap_or_default()
    }

    fn is_public(&self, doc: &Document) -> bool {
        match doc.value(&self.spec.public_key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::String(s)) if self.spec.lenient_booleans => {
                s.trim().eq_ignore_ascii_case("true")
            }
            _ => false,
        }
    }

    fn lifecycle(&self, doc: &Document) -> Lifecycle {
        match doc
            .value(&self.spec.lifecycle_key)
            .and_then(Value::scalar_text)
        {
            None => Lifecycle::Absent,
            Some(s) if self.spec.lifecycle.is_empty() || self.lifecycle_def(&s).is_some() => {
                Lifecycle::Declared(s)
            }
            Some(s) => Lifecycle::Unknown(s),
        }
    }

    fn lifecycle_def(&self, status: &str) -> Option<&LifecycleDef> {
        self.spec.lifecycle.iter().find(|l| l.id == status)
    }

    fn predicate(&self, key: &str) -> Option<PredicateDef> {
        if let Some(p) = self.predicates.get(key) {
            return Some(p.clone());
        }
        if self.spec.curie_keys {
            return well_known(&self.prefixes, key);
        }
        None
    }

    fn predicates(&self) -> Vec<PredicateDef> {
        self.predicates.values().cloned().collect()
    }

    fn is_reserved(&self, key: &str) -> bool {
        let s = &self.spec;
        [
            &s.type_key,
            &s.identity_key,
            &s.title_key,
            &s.aliases_key,
            &s.public_key,
            &s.lifecycle_key,
        ]
        .iter()
        .any(|k| k.as_str() == key)
            || s.reserved.iter().any(|k| k == key)
    }

    fn reference_candidates(&self, target: &str) -> Vec<Iri> {
        self.spec
            .reference_namespaces
            .iter()
            .filter_map(|ns| Iri::parse(format!("{ns}{target}")).ok())
            .collect()
    }

    fn mint_iri(
        &self,
        kind: Kind,
        title: &str,
        ctx: &crate::mint::MintContext<'_>,
    ) -> Result<Iri, crate::mint::MintRefusal> {
        let m = self
            .spec
            .mint
            .as_ref()
            .ok_or(crate::mint::MintRefusal::NoRule)?;
        crate::mint::mint_slug(m.namespace(kind), title, ctx)
    }

    fn publication_url(&self, iri: &Iri) -> Option<String> {
        let p = self.spec.publication.as_ref()?;
        let tail = iri.as_str().strip_prefix(p.namespace.as_str())?;
        Some(format!("{}{tail}{}", p.base_url, p.suffix))
    }

    fn cycle_policy(&self) -> CyclePolicy {
        self.spec.cycle_policy
    }
}
