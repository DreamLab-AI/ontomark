//! Profiles: a corpus's rules as versioned data.
//!
//! The compiler knows OWL, RDF, SKOS, PROV-O, DCTERMS and the artefact
//! formats; a [`Profile`] supplies everything a particular corpus decides:
//! identity namespaces and their public projection, the domain and category
//! taxonomy (whose positions are published NGG1 identities), dataset metadata,
//! lifecycle vocabulary, the frontmatter grammar per vault role, the
//! attribution roster location and actor grammar, and governance kinds.
//!
//! [`Profile::generic`] needs no configuration and accepts an ordinary vault.
//! The estate profile lives in `profiles/estate` (`ontomark-profile-estate`)
//! and is loaded from its `profile.yaml` with [`Profile::from_yaml`].
//!
//! ```
//! use ontomark_compile::profile::Profile;
//!
//! let generic = Profile::generic();
//! assert!(generic.taxonomy.domains.is_empty());
//! assert_eq!(generic.taxonomy.domain_class_id("anything"), 0);
//!
//! let custom = Profile::from_yaml(
//!     "id: mine\nversion: '1'\ntaxonomy:\n  domains:\n    - {slug: botany, label: Botany}\n",
//! )
//! .unwrap();
//! assert_eq!(custom.taxonomy.domain_class_id("botany"), 1);
//! ```

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A corpus profile. Every field has a generic default, so a profile file
/// states only what it changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    /// Short identifier, e.g. `estate` or `generic`.
    pub id: String,
    /// The profile's own version; it advances when any meaning here changes.
    pub version: String,
    /// Identity namespaces and their published projection.
    pub iris: IriRules,
    /// Dataset-level metadata carried into graph and catalogue artefacts.
    pub dataset: Dataset,
    /// Domains, categories and aliases.
    pub taxonomy: Taxonomy,
    /// Lifecycle vocabulary.
    pub lifecycle: Lifecycle,
    /// Label reconstruction rules.
    pub labels: Labels,
    /// Frontmatter grammar per vault role.
    pub frontmatter: FrontmatterGrammar,
    /// Where generators are listed and how `generated.by` names them.
    pub attribution: AttributionRules,
    /// Governance event kinds (data only; posting is a host adapter's job).
    pub governance: Governance,
}

impl Default for Profile {
    fn default() -> Self {
        Self::generic()
    }
}

impl Profile {
    /// The configuration-free profile: no taxonomy, no dataset claims,
    /// `urn:ontomark:` identities published as themselves.
    #[must_use]
    pub fn generic() -> Self {
        Self {
            id: "generic".to_owned(),
            version: "1.0.0".to_owned(),
            iris: IriRules::default(),
            dataset: Dataset::default(),
            taxonomy: Taxonomy::default(),
            lifecycle: Lifecycle::default(),
            labels: Labels::default(),
            frontmatter: FrontmatterGrammar::default(),
            attribution: AttributionRules::default(),
            governance: Governance::default(),
        }
    }

    /// Parse a profile document. Unknown keys are refused, so a typo cannot
    /// silently fall back to a generic default.
    ///
    /// # Errors
    /// The YAML is malformed or names a key this version does not know.
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text)
    }
}

/// Identity namespaces. Authored IRIs are never rewritten except by the
/// explicit public projection named here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IriRules {
    /// The namespace minted class identities live in (`resource:`).
    pub class_namespace: String,
    /// The namespace page identities live in (`page_resource:`).
    pub page_namespace: String,
    /// Where a class identity is published; empty publishes it unchanged.
    pub public_class_base: String,
    /// Where an individual identity is published; empty publishes it unchanged.
    pub public_individual_base: String,
    /// The corpus's own property namespace.
    pub vocabulary_namespace: String,
    /// Base of the published JSON-LD context documents.
    pub ns_base: String,
    /// The asserted ontology's IRI; `owl:versionIRI` appends the digest.
    pub ontology_iri: String,
    /// The inferred ontology's IRI.
    pub inferred_ontology_iri: String,
    /// The inferred ontology's `rdfs:label`.
    pub inferred_label: String,
    /// `owl:versionInfo`.
    pub version_info: String,
}

impl Default for IriRules {
    fn default() -> Self {
        Self {
            class_namespace: "urn:ontomark:class:".to_owned(),
            page_namespace: "urn:ontomark:page:".to_owned(),
            public_class_base: String::new(),
            public_individual_base: String::new(),
            vocabulary_namespace: "urn:ontomark:vocab#".to_owned(),
            ns_base: String::new(),
            ontology_iri: "urn:ontomark:ontology".to_owned(),
            inferred_ontology_iri: "urn:ontomark:ontology:inferred".to_owned(),
            inferred_label: "Inferred axioms".to_owned(),
            version_info: "1.0.0".to_owned(),
        }
    }
}

/// Dataset-level metadata. Empty strings are omitted from artefacts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dataset {
    /// Producer pipeline identity.
    pub pipeline_version: String,
    /// The dataset-level `prov:wasAttributedTo` fallback.
    pub attributed_to: String,
    /// Its human-readable label.
    pub attributed_label: String,
    /// A short classifier of the corpus's provenance.
    pub nature: String,
    /// The dataset description.
    pub description: String,
}

/// One domain: its position in [`Taxonomy::domains`] is its published id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Domain {
    /// Canonical slug.
    pub slug: String,
    /// Display label.
    pub label: String,
}

/// One category: its position in [`Taxonomy::categories`] is its id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    /// The category root page's slug.
    pub slug: String,
    /// Display label.
    pub label: String,
    /// The owning domain's position.
    pub domain: u16,
}

/// The taxonomy registry. Empty means no taxonomy at all, which every
/// artefact accepts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Taxonomy {
    /// Domains in publication order (append-only).
    pub domains: Vec<Domain>,
    /// Historical short names that keep a domain's class id.
    pub short_names: IndexMap<String, String>,
    /// Rendering aliases folded onto a canonical domain.
    pub domain_aliases: IndexMap<String, String>,
    /// Categories in id order (append-only).
    pub categories: Vec<Category>,
}

impl Taxonomy {
    /// The position of a canonical domain slug.
    #[must_use]
    pub fn domain_index(&self, slug: &str) -> Option<usize> {
        self.domains.iter().position(|d| d.slug == slug)
    }

    /// GPU clustering class: `0` for unknown, the domain's position plus one
    /// otherwise. Short names resolve to their canonical domain first.
    #[must_use]
    pub fn domain_class_id(&self, domain: &str) -> i32 {
        let canonical = self
            .short_names
            .get(domain)
            .map_or(domain, String::as_str);
        self.domain_index(canonical)
            .and_then(|id| i32::try_from(id + 1).ok())
            .unwrap_or(0)
    }

    /// `true` for a domain root or category slug: a taxonomy node never takes
    /// part in a disjointness axiom (ADR-2125).
    #[must_use]
    pub fn is_taxonomy_node(&self, slug: &str) -> bool {
        self.domains.iter().any(|d| d.slug == slug)
            || self.categories.iter().any(|c| c.slug == slug)
    }
}

/// One `(slug, label)` pair of a controlled list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Term {
    /// Slug, as authored.
    pub slug: String,
    /// Display label.
    pub label: String,
}

/// Lifecycle vocabulary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Lifecycle {
    /// Maturity levels, declared as named individuals in the ontology.
    pub maturity_levels: Vec<Term>,
}

/// Label reconstruction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Labels {
    /// Lower-case slug segment → display spelling.
    pub acronyms: IndexMap<String, String>,
}

/// Whether an undeclared frontmatter key is accepted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnknownKeys {
    /// Accept and preserve it.
    #[default]
    Allow,
    /// Refuse the page.
    Fail,
}

/// One vault role's grammar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Role {
    /// Admitted `type:` values; empty admits any.
    pub types: Vec<String>,
    /// Keys every page must carry.
    pub required_keys: Vec<String>,
    /// Undeclared keys.
    pub unknown_keys: UnknownKeys,
    /// Pages here may be published.
    pub publishable: bool,
    /// Pages here enter the reasoner.
    pub reasoned: bool,
}

/// The frontmatter grammar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FrontmatterGrammar {
    /// Role name → grammar. A generic vault is one permissive role.
    pub roles: IndexMap<String, Role>,
    /// The fail-closed publication key: only a literal boolean `true` publishes.
    pub public_gate: String,
}

impl Default for FrontmatterGrammar {
    fn default() -> Self {
        let mut roles = IndexMap::new();
        roles.insert(
            "knowledge".to_owned(),
            Role {
                publishable: true,
                reasoned: true,
                ..Role::default()
            },
        );
        Self {
            roles,
            public_gate: "public".to_owned(),
        }
    }
}

/// Attribution: where the roster is and how actors are spelled.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AttributionRules {
    /// Repository-relative roster path; empty means no per-class attribution.
    pub roster: String,
    /// The accepted `generated.by` spellings, for documentation and SHACL.
    pub actors: Vec<String>,
}

/// Governance kinds, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Governance {
    /// Name → event kind number.
    pub kinds: IndexMap<String, u32>,
}
