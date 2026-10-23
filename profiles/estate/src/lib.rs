//! The DreamLab estate profile: the visionGraph corpus's rules as versioned
//! data (`profile.yaml`), loaded into an [`ontomark_compile::profile::Profile`].
//!
//! It carries the frontmatter grammar per vault role, the maturity lifecycle,
//! the governance event kinds, the `urn:ngm:class:` / `urn:visionflow:page:`
//! identity namespaces and their `narrativegoldmine.com` projection, the
//! nine-domain, 68-category taxonomy whose positions are published NGG1
//! identities, the domain aliases, and the attribution roster location and
//! `generated.by` actor grammar.
//!
//! ```
//! let estate = ontomark_profile_estate::profile();
//! assert_eq!(estate.id, "estate");
//! assert_eq!(estate.taxonomy.categories.len(), 68);
//! assert_eq!(estate.taxonomy.domain_class_id("ai"), 1);
//! assert_eq!(estate.iris.class_namespace, "urn:ngm:class:");
//! ```

use ontomark_compile::profile::Profile;

/// The profile document, as shipped.
pub const PROFILE_YAML: &str = include_str!("../profile.yaml");

/// The estate profile.
///
/// # Panics
/// Never for a released crate: `profile.yaml` is parsed by this crate's tests.
#[must_use]
pub fn profile() -> Profile {
    Profile::from_yaml(PROFILE_YAML).expect("profiles/estate/profile.yaml is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_declares_the_published_taxonomy() {
        let p = profile();
        assert_eq!(p.taxonomy.domains.len(), 9);
        assert_eq!(p.taxonomy.categories.len(), 68);
        assert!(p
            .taxonomy
            .categories
            .iter()
            .all(|c| usize::from(c.domain) < p.taxonomy.domains.len()));
        assert_eq!(p.frontmatter.roles.len(), 2);
        assert_eq!(p.governance.kinds["proposal"], 31402);
    }
}
