//! Vocabulary profiles: which frontmatter keys carry ontology meaning.
//!
//! The normative typed model belongs to `ontomark-model`; compiler 0.1.3
//! provides an opt-in `model-projection` API. This module retains the engine's
//! released document-profile semantics. Replacing it requires explicit
//! generic/estate profile mapping and native/WASM wire parity (EXP-10);
//! availability of the compiler API alone does not complete that migration.
//!
//! Two profiles exist. [`ProfileId::Generic`] is the default and works on any
//! vault: a page is an ontology entity only when its `type` says so, and only
//! the keys listed here become axioms. [`ProfileId::Estate`] follows the
//! estate corpus grammar (`resource` IRIs, `public` gate, `is-a` relations)
//! and is selected explicitly (EXP-05).

use serde::{Deserialize, Serialize};

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
