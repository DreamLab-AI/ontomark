//! Identity minting: the one place a new entity IRI is created.
//!
//! ADR-2015: an identity is minted once, when the author explicitly creates
//! an entity, and then stored in the page header. Opening, indexing or
//! renaming never mints. The model only *offers* an IRI through
//! [`crate::Profile::mint_iri`]; the caller writes it with a guarded patch.

use crate::iri::Iri;
use crate::kind::Kind;
use crate::model::Model;

/// What a minter may consult.
#[derive(Debug, Clone, Copy, Default)]
pub struct MintContext<'a> {
    /// The current model, so a minted IRI never collides with a claimed one.
    pub model: Option<&'a Model>,
}

impl MintContext<'_> {
    /// `true` when `iri` is already claimed in the context model.
    #[must_use]
    pub fn is_taken(&self, iri: &Iri) -> bool {
        self.model.is_some_and(|m| m.is_claimed(iri))
    }
}

/// Lower-case, collapse every run of characters outside `[a-z0-9]` to `-`,
/// trim `-` from both ends. Matches the estate's historic Python rule
/// `re.sub(r'[^a-z0-9]+', '-', s.lower()).strip('-')`.
///
/// ```
/// # use ontomark_model::slugify;
/// assert_eq!(slugify("Knowledge Graph"), "knowledge-graph");
/// assert_eq!(slugify("ISO/IEC 9075"), "iso-iec-9075");
/// assert_eq!(slugify("  Café — Naïve  "), "caf-na-ve");
/// assert_eq!(slugify("***"), "");
/// ```
#[must_use]
pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dash = false;
    for c in s.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c);
        } else {
            dash = true;
        }
    }
    out
}

/// The declarative minting rule: `namespace(kind) + slugify(title)`,
/// refused (`None`) for an empty slug, a kind without a namespace, or an IRI
/// already claimed in the context.
pub(crate) fn mint_slug(
    namespace: Option<&str>,
    title: &str,
    ctx: &MintContext<'_>,
) -> Option<Iri> {
    let ns = namespace?;
    let slug = slugify(title);
    if slug.is_empty() {
        return None;
    }
    let iri = Iri::parse(format!("{ns}{slug}")).ok()?;
    (!ctx.is_taken(&iri)).then_some(iri)
}

/// Namespaces per kind for the slug rule.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MintSpec {
    /// Namespace for classes.
    #[serde(default)]
    pub class: Option<String>,
    /// Namespace for individuals.
    #[serde(default)]
    pub individual: Option<String>,
    /// Namespace for properties.
    #[serde(default)]
    pub property: Option<String>,
}

impl MintSpec {
    pub(crate) fn namespace(&self, kind: Kind) -> Option<&str> {
        match kind {
            Kind::Class => self.class.as_deref(),
            Kind::Individual => self.individual.as_deref(),
            Kind::Property(_) => self.property.as_deref(),
        }
    }
}
