//! Vocabulary profiles: which frontmatter keys carry ontology meaning.
//!
//! The normative typed model belongs to `ontomark-model`; compiler 0.1.3
//! provides an opt-in `model-projection` API. This module retains the engine's
//! released document-profile semantics for the engine's own projection.
//!
//! The explicit mapping EXP-10 asks for exists here for one consumer: each
//! engine [`Profile`] also implements [`ontomark_model::Profile`], reading a
//! vault by exactly the engine's grammar (case-insensitive kinds, `owl:`
//! prefix stripping, page IRIs minted from the path when the profile allows a
//! page without one). Assisted ingest ([`ontomark_ingest`]) builds its
//! [`ontomark_model::Model`] through that bridge, so the vocabulary a model is
//! offered is the set of entities the engine itself indexes
//! (`tests/profile_parity.rs` proves the identity sets equal over the fixture
//! vault). The engine's own axioms still come from [`crate::projection`];
//! replacing that needs native/WASM wire parity over the whole transcript.
//!
//! Two profiles exist. [`ProfileId::Generic`] is the default and works on any
//! vault: a page is an ontology entity only when its `type` says so, and only
//! the keys listed here become axioms. [`ProfileId::Estate`] follows the
//! estate corpus grammar (`resource` IRIs, `public` gate, `is-a` relations)
//! and is selected explicitly (EXP-05).

use std::sync::OnceLock;

use ontomark_model::{
    DocId, IdentityDecision, Iri, KindDecision, Lifecycle, LifecycleDef, PredicateDef,
    PredicateForm, PrefixMap, PropertyKind,
};
use ontomark_parse::{Document, Value};
use serde::{Deserialize, Serialize};

use crate::projection::PAGE_IRI_PREFIX;
use crate::text::pct_encode_path;

/// A profile identifier as it appears on the wire and in plugin settings.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ProfileId {
    /// Any vault, no estate knowledge required. The default.
    #[default]
    Generic,
    /// The estate corpus grammar. Opt-in.
    Estate,
}

impl ProfileId {
    /// The profile's vocabulary.
    #[must_use]
    pub fn profile(self) -> &'static Profile {
        match self {
            Self::Generic => &GENERIC,
            Self::Estate => &ESTATE,
        }
    }
}

/// The kind of ontology entity a page declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// An OWL class.
    Class,
    /// An OWL named individual.
    Individual,
    /// An OWL object property.
    Property,
}

/// The keys a profile reads. All slices are matched case-sensitively against
/// top-level frontmatter keys.
#[derive(Debug)]
pub struct Profile {
    /// Identifier.
    pub id: ProfileId,
    /// Key whose value declares the entity kind.
    pub type_key: &'static str,
    /// Key holding an authored absolute IRI.
    pub iri_key: &'static str,
    /// `true` when an entity page without an IRI key is refused.
    pub require_iri: bool,
    /// Key that must be a YAML boolean when present (`None`: no gate).
    pub gate_key: Option<&'static str>,
    /// Display label key.
    pub label_key: &'static str,
    /// Alias list key.
    pub alias_key: &'static str,
    /// Keys whose wikilinks are `SubClassOf` (classes) or types (individuals).
    pub subclass_keys: &'static [&'static str],
    /// Keys whose wikilinks are `EquivalentClasses`.
    pub equivalent_keys: &'static [&'static str],
    /// Keys whose wikilinks are `DisjointClasses`.
    pub disjoint_keys: &'static [&'static str],
    /// Keys whose wikilinks are `rdf:type` assertions on individuals.
    pub instance_keys: &'static [&'static str],
    /// Property domain keys.
    pub domain_keys: &'static [&'static str],
    /// Property range keys.
    pub range_keys: &'static [&'static str],
    /// Sub-property keys.
    pub subproperty_keys: &'static [&'static str],
}

impl Profile {
    /// Read a `type` value as an entity kind, if it names one.
    #[must_use]
    pub fn kind_of(&self, value: &str) -> Option<Kind> {
        let v = value.trim();
        let v = v.strip_prefix("owl:").unwrap_or(v).to_ascii_lowercase();
        match v.as_str() {
            "class" => Some(Kind::Class),
            "individual" | "namedindividual" | "instance" => Some(Kind::Individual),
            "property" | "objectproperty" | "relation" => Some(Kind::Property),
            _ => None,
        }
    }

    /// `true` when `key` is consumed by this profile (and so not emitted as a
    /// plain annotation).
    #[must_use]
    pub fn consumes(&self, key: &str) -> bool {
        key == self.type_key
            || key == self.iri_key
            || key == self.label_key
            || key == self.alias_key
            || self.gate_key == Some(key)
            || [
                self.subclass_keys,
                self.equivalent_keys,
                self.disjoint_keys,
                self.instance_keys,
                self.domain_keys,
                self.range_keys,
                self.subproperty_keys,
            ]
            .iter()
            .any(|keys| keys.contains(&key))
    }
}

static GENERIC: Profile = Profile {
    id: ProfileId::Generic,
    type_key: "type",
    iri_key: "iri",
    require_iri: false,
    gate_key: None,
    label_key: "title",
    alias_key: "aliases",
    subclass_keys: &["subclass-of", "is-a"],
    equivalent_keys: &["equivalent-to"],
    disjoint_keys: &["disjoint-with"],
    instance_keys: &["instance-of"],
    domain_keys: &["domain"],
    range_keys: &["range"],
    subproperty_keys: &["subproperty-of"],
};

static ESTATE: Profile = Profile {
    id: ProfileId::Estate,
    type_key: "type",
    iri_key: "resource",
    require_iri: true,
    gate_key: Some("public"),
    label_key: "title",
    alias_key: "aliases",
    subclass_keys: &["is-a"],
    equivalent_keys: &["same-as"],
    disjoint_keys: &["disjoint-with"],
    instance_keys: &["instance-of"],
    domain_keys: &["domain"],
    range_keys: &["range"],
    subproperty_keys: &["subproperty-of"],
};

const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const OWL: &str = "http://www.w3.org/2002/07/owl#";

static PREFIXES: OnceLock<PrefixMap> = OnceLock::new();

fn def(key: &str, iri: String, form: PredicateForm) -> PredicateDef {
    PredicateDef {
        key: key.to_owned(),
        iri: Iri::parse(iri).expect("constant predicate IRI"),
        form,
        datatype: None,
        domain: None,
        range: None,
        transitive: false,
    }
}

impl Profile {
    /// The relation keys this profile reads, each with the predicate it
    /// asserts, in a fixed order. `domain` and `range` are declared as
    /// annotations: the typed model has no property-axiom form for them, and
    /// the engine's projection reads them itself.
    fn relation_keys(&self) -> impl Iterator<Item = (&'static str, String, PredicateForm)> + '_ {
        let groups: [(&'static [&'static str], String, PredicateForm); 7] = [
            (
                self.subclass_keys,
                format!("{RDFS}subClassOf"),
                PredicateForm::SubClassOf,
            ),
            (
                self.equivalent_keys,
                format!("{OWL}equivalentClass"),
                PredicateForm::EquivalentClass,
            ),
            (
                self.disjoint_keys,
                format!("{OWL}disjointWith"),
                PredicateForm::DisjointWith,
            ),
            (
                self.instance_keys,
                RDF_TYPE.to_owned(),
                PredicateForm::ClassAssertion,
            ),
            (
                self.subproperty_keys,
                format!("{RDFS}subPropertyOf"),
                PredicateForm::SubPropertyOf,
            ),
            (
                self.domain_keys,
                format!("{RDFS}domain"),
                PredicateForm::Annotation,
            ),
            (
                self.range_keys,
                format!("{RDFS}range"),
                PredicateForm::Annotation,
            ),
        ];
        groups
            .into_iter()
            .flat_map(|(keys, iri, form)| keys.iter().map(move |k| (*k, iri.clone(), form)))
    }

    /// The engine's minted page IRI for a note without an authored one.
    fn minted(id: &DocId) -> IdentityDecision {
        let text = format!("{PAGE_IRI_PREFIX}{}", pct_encode_path(id.as_str()));
        match Iri::parse(text) {
            Ok(iri) => IdentityDecision::Iri(iri),
            Err(e) => IdentityDecision::Invalid(e.to_string()),
        }
    }
}

/// The engine grammar as a typed-model profile (EXP-10 mapping). See the
/// module documentation.
impl ontomark_model::Profile for Profile {
    fn id(&self) -> &str {
        match self.id {
            ProfileId::Generic => "engine-generic/1",
            ProfileId::Estate => "engine-estate/1",
        }
    }

    fn prefixes(&self) -> &PrefixMap {
        PREFIXES.get_or_init(PrefixMap::standard)
    }

    /// A page is an entity only when its type names a kind; any other type
    /// value is an ordinary note, as the engine treats it (never refused).
    fn kind(&self, doc: &Document) -> KindDecision {
        let Some(value) = doc.value(self.type_key) else {
            return KindDecision::NotOntology;
        };
        match value.scalar_text().and_then(|t| self.kind_of(&t)) {
            Some(Kind::Class) => KindDecision::Entity(ontomark_model::Kind::Class),
            Some(Kind::Individual) => KindDecision::Entity(ontomark_model::Kind::Individual),
            Some(Kind::Property) => {
                KindDecision::Entity(ontomark_model::Kind::Property(PropertyKind::Object))
            }
            None => KindDecision::NotOntology,
        }
    }

    fn identity(&self, doc: &Document) -> IdentityDecision {
        match doc.value(self.iri_key) {
            None | Some(Value::Null) => IdentityDecision::Missing,
            Some(Value::String(s)) => match Iri::parse(s.trim()) {
                Ok(iri) => IdentityDecision::Iri(iri),
                Err(e) => IdentityDecision::Invalid(e.to_string()),
            },
            Some(v) => IdentityDecision::Invalid(format!("identity is a {}", v.type_name())),
        }
    }

    /// Authored IRIs verbatim; otherwise the engine's `urn:ontomark:page:`
    /// IRI, unless the profile requires an authored one (estate).
    fn identity_for(&self, id: &DocId, doc: &Document) -> IdentityDecision {
        match self.identity(doc) {
            IdentityDecision::Missing if !self.require_iri => Self::minted(id),
            other => other,
        }
    }

    fn label(&self, id: &DocId, doc: &Document) -> String {
        doc.value(self.label_key)
            .and_then(Value::scalar_text)
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| id.name().to_owned())
    }

    fn aliases(&self, doc: &Document) -> Vec<String> {
        doc.value(self.alias_key)
            .map(Value::texts)
            .unwrap_or_default()
    }

    fn is_public(&self, doc: &Document) -> bool {
        match self.gate_key {
            None => true,
            Some(key) => matches!(doc.value(key), Some(Value::Bool(true))),
        }
    }

    fn lifecycle(&self, _doc: &Document) -> Lifecycle {
        Lifecycle::Absent
    }

    fn lifecycle_def(&self, _status: &str) -> Option<&LifecycleDef> {
        None
    }

    fn predicate(&self, key: &str) -> Option<PredicateDef> {
        self.relation_keys()
            .find(|(k, _, _)| *k == key)
            .map(|(k, iri, form)| def(k, iri, form))
    }

    fn predicates(&self) -> Vec<PredicateDef> {
        self.relation_keys()
            .map(|(k, iri, form)| def(k, iri, form))
            .collect()
    }

    fn is_reserved(&self, key: &str) -> bool {
        key == self.type_key
            || key == self.iri_key
            || key == self.label_key
            || key == self.alias_key
            || self.gate_key == Some(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_parse_case_and_prefix_insensitively() {
        let p = ProfileId::Generic.profile();
        assert_eq!(p.kind_of("Class"), Some(Kind::Class));
        assert_eq!(p.kind_of("owl:NamedIndividual"), Some(Kind::Individual));
        assert_eq!(p.kind_of("ObjectProperty"), Some(Kind::Property));
        assert_eq!(p.kind_of("note"), None);
    }

    #[test]
    fn default_profile_is_generic() {
        assert_eq!(ProfileId::default(), ProfileId::Generic);
        assert!(!ProfileId::Generic.profile().require_iri);
        assert!(ProfileId::Estate.profile().require_iri);
    }
}
