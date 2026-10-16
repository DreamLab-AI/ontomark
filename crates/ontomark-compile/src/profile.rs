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
    /// What the raw-Markdown export surfaces do with private links.
    pub publication: Publication,
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
            publication: Publication::default(),
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
    /// The namespace individual identities live in.
    pub individual_namespace: String,
    /// The namespace page identities live in (`page_resource:`).
    pub page_namespace: String,
    /// Where a class identity is published; empty publishes it unchanged.
    pub public_class_base: String,
    /// Where an individual identity is published; empty publishes it unchanged.
    pub public_individual_base: String,
    /// Where a page identity is published; empty publishes it unchanged.
    pub public_page_base: String,
    /// Historical identity namespaces that still publish under a base, as
    /// ordered `prefix -> base` pairs (a parser's tolerance for legacy input).
    pub legacy_projections: Vec<IriMapping>,
    /// Turtle/JSON-LD prefix name of [`IriRules::vocabulary_namespace`].
    pub vocabulary_prefix: String,
    /// Turtle/JSON-LD prefix name of the published class base.
    pub class_prefix: String,
    /// Turtle/JSON-LD prefix name of the published individual base.
    pub individual_prefix: String,
    /// The corpus's own property namespace.
    pub vocabulary_namespace: String,
    /// Base of the published JSON-LD context documents.
    pub ns_base: String,
    /// The asserted ontology's IRI; `owl:versionIRI` appends the digest.
    pub ontology_iri: String,
    /// The asserted ontology's `rdfs:label` (also the WebVOWL title).
    pub ontology_label: String,
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
            individual_namespace: "urn:ontomark:individual:".to_owned(),
            page_namespace: "urn:ontomark:page:".to_owned(),
            public_class_base: String::new(),
            public_individual_base: String::new(),
            public_page_base: String::new(),
            legacy_projections: Vec::new(),
            vocabulary_prefix: "vc".to_owned(),
            class_prefix: "cls".to_owned(),
            individual_prefix: "ind".to_owned(),
            vocabulary_namespace: "urn:ontomark:vocab#".to_owned(),
            ns_base: String::new(),
            ontology_iri: "urn:ontomark:ontology".to_owned(),
            ontology_label: "Ontology".to_owned(),
            inferred_ontology_iri: "urn:ontomark:ontology:inferred".to_owned(),
            inferred_label: "Inferred axioms".to_owned(),
            version_info: "1.0.0".to_owned(),
        }
    }
}

/// One ordered `prefix -> base` publication mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IriMapping {
    /// The authored identity prefix.
    pub prefix: String,
    /// The base it is published under.
    pub base: String,
}

impl IriRules {
    /// Where classes are published: the public base, else the authored namespace.
    #[must_use]
    pub fn class_base(&self) -> &str {
        if self.public_class_base.is_empty() {
            &self.class_namespace
        } else {
            &self.public_class_base
        }
    }

    /// Where individuals are published: the public base, else the authored namespace.
    #[must_use]
    pub fn individual_base(&self) -> &str {
        if self.public_individual_base.is_empty() {
            &self.individual_namespace
        } else {
            &self.public_individual_base
        }
    }

    /// Project an authored identity onto its published form when one of the
    /// namespaces here claims it; `None` otherwise. Class, individual and
    /// legacy namespaces are tried before the page namespace.
    #[must_use]
    pub fn project(&self, iri: &str) -> Option<String> {
        let own = [
            (self.class_namespace.as_str(), self.class_base()),
            (self.individual_namespace.as_str(), self.individual_base()),
        ];
        let legacy = self
            .legacy_projections
            .iter()
            .map(|m| (m.prefix.as_str(), m.base.as_str()));
        let page = (!self.public_page_base.is_empty())
            .then_some((self.page_namespace.as_str(), self.public_page_base.as_str()));
        own.into_iter()
            .chain(legacy)
            .chain(page)
            .filter(|(prefix, _)| !prefix.is_empty())
            .find_map(|(prefix, base)| iri.strip_prefix(prefix).map(|tail| format!("{base}{tail}")))
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
    /// The short corpus classifier carried in `stats.json`.
    pub kind: String,
    /// Title of the OKF bundle index.
    pub bundle_title: String,
    /// Named authors: the first is the asserted ontology header's
    /// `dcterms:creator`; all are listed in the WebVOWL header. Empty omits both.
    pub authors: Vec<String>,
}

/// One domain: its position in [`Taxonomy::domains`] is its published id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Domain {
    /// Canonical slug.
    pub slug: String,
    /// Display label.
    pub label: String,
    /// Fill colour in the WebVOWL rendering; empty uses the default colour.
    #[serde(default)]
    pub colour: String,
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
    /// Domain values accepted when the vocabulary does not enumerate
    /// `domain`; empty accepts any.
    pub accepted_domains: Vec<String>,
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
        let canonical = self.short_names.get(domain).map_or(domain, String::as_str);
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
    /// The level an unknown or absent `maturity:` falls back to.
    pub default_maturity: String,
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

/// Publication behaviour of the raw-Markdown export surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Publication {
    /// Redact links that resolve to a page that is not public from the staged
    /// `publish/` Markdown and the `api/markdown` mirror (EXP-04): list
    /// entries and scalars are removed from frontmatter, body links become
    /// their display text or `(private link)`, embeds vanish. Links to pages
    /// that do not exist are left alone. `ontomark build
    /// --no-redact-private-links` overrides this for byte-parity runs.
    pub redact_private_links: bool,
}

impl Default for Publication {
    fn default() -> Self {
        Self {
            redact_private_links: true,
        }
    }
}

/// Governance kinds, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Governance {
    /// Name → event kind number.
    pub kinds: IndexMap<String, u32>,
}

/// The estate profile, for unit tests that exercise corpus-shaped data.
#[cfg(test)]
pub(crate) fn test_estate() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml"))
        .expect("the estate profile parses")
}

/// Shared estate fixtures for unit tests.
#[cfg(test)]
pub(crate) mod testing {
    use super::Profile;
    use std::sync::OnceLock;

    /// The estate profile, parsed once.
    pub(crate) fn est() -> &'static Profile {
        static P: OnceLock<Profile> = OnceLock::new();
        P.get_or_init(super::test_estate)
    }
    /// The estate vocabulary namespace.
    pub(crate) fn vc() -> &'static str {
        &est().iris.vocabulary_namespace
    }
    /// The estate published class base.
    pub(crate) fn ngm() -> &'static str {
        &est().iris.public_class_base
    }
    /// The estate published individual base.
    pub(crate) fn ngmi() -> &'static str {
        &est().iris.public_individual_base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_keep_their_domain_class_ids() {
        let t = test_estate().taxonomy;
        for (id, alias) in ["ai", "bc", "mv", "rb", "ngm", "tc"].iter().enumerate() {
            let canonical = t.short_names.get(*alias).map_or(*alias, String::as_str);
            assert_eq!(t.domain_class_id(alias), t.domain_class_id(canonical));
            assert_ne!(t.domain_class_id(alias), 0, "{alias} ({id})");
        }
        assert_eq!(t.domain_class_id("ai"), 1);
        assert_eq!(t.domain_class_id("bc"), 2);
        assert_eq!(t.domain_class_id("space-science-and-systems"), 7);
        assert_eq!(
            t.domain_class_id("earth-observation-and-geospatial-sensing"),
            8
        );
        assert_eq!(t.domain_class_id(""), 0);
        assert_eq!(t.domain_class_id("space-science-typo"), 0);
    }

    #[test]
    fn projection_prefers_class_then_individual_then_legacy_then_page() {
        let r = test_estate().iris;
        assert_eq!(
            r.project("urn:ngm:class:x").unwrap(),
            format!("{}x", r.public_class_base)
        );
        assert_eq!(
            r.project("urn:ngm:individual:y").unwrap(),
            format!("{}y", r.public_individual_base)
        );
        assert_eq!(
            r.project("urn:visionflow:linked:z").unwrap(),
            "https://narrativegoldmine.com/linked/z"
        );
        assert_eq!(
            r.project("urn:visionflow:page:p").unwrap(),
            "https://narrativegoldmine.com/page/p"
        );
        assert_eq!(r.project("bare"), None);
        assert_eq!(
            IriRules::default().project("urn:ontomark:class:a").unwrap(),
            "urn:ontomark:class:a"
        );
    }

    #[test]
    fn projection_breaks_prefix_ties_in_the_documented_order() {
        // Overlapping claims: the individual namespace is a strict prefix of
        // the class namespace, and a legacy projection claims the same prefix
        // as the page namespace. First match in the documented order - class,
        // individual, legacy, page - must win.
        let r = IriRules {
            class_namespace: "urn:x:class:".to_owned(),
            individual_namespace: "urn:x:".to_owned(),
            public_class_base: "https://example.org/c/".to_owned(),
            public_individual_base: "https://example.org/i/".to_owned(),
            page_namespace: "urn:p:".to_owned(),
            public_page_base: "https://example.org/p/".to_owned(),
            legacy_projections: vec![IriMapping {
                prefix: "urn:p:".to_owned(),
                base: "https://example.org/legacy/".to_owned(),
            }],
            ..IriRules::default()
        };
        // Class beats individual although the individual prefix also matches.
        assert_eq!(
            r.project("urn:x:class:a").unwrap(),
            "https://example.org/c/a"
        );
        // The individual namespace claims what the class namespace does not.
        assert_eq!(
            r.project("urn:x:other").unwrap(),
            "https://example.org/i/other"
        );
        // Legacy beats the page namespace on an identical prefix.
        assert_eq!(
            r.project("urn:p:q").unwrap(),
            "https://example.org/legacy/q"
        );
        // An IRI no namespace claims is never rewritten.
        assert_eq!(r.project("urn:unrelated:z"), None);
    }

    #[test]
    fn page_identities_project_only_when_a_public_page_base_exists() {
        // The generic profile publishes class and individual identities as
        // themselves, but its empty `public_page_base` means the page
        // namespace claims nothing at all.
        let generic = IriRules::default();
        assert_eq!(
            generic.project("urn:ontomark:individual:i").unwrap(),
            "urn:ontomark:individual:i"
        );
        assert_eq!(generic.project("urn:ontomark:page:p"), None);
    }
}
