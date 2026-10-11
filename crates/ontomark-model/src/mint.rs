//! Identity minting: the one place a new entity IRI is created.
//!
//! ADR-2015: an identity is minted once, when the author explicitly creates
//! an entity, and then stored in the page header. Opening, indexing or
//! renaming never mints. The model only *offers* an IRI through
//! [`crate::Profile::mint_iri`]; the caller writes it with a guarded patch.

use serde::{Deserialize, Serialize};

use crate::iri::Iri;
use crate::kind::Kind;
use crate::model::Model;
use crate::profile::Profile;

/// Why no IRI was minted. Each case means the caller must not write an
/// identity on its own: it either supplies one explicitly or links the
/// existing entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum MintRefusal {
    /// The profile has no minting rule for this kind (the generic profile
    /// never mints).
    #[error("the profile has no minting rule for this kind; supply an IRI")]
    NoRule,
    /// The title yields no identifier (`***`).
    #[error("the title yields no identifier")]
    EmptySlug,
    /// The rule produced text that is not an absolute IRI.
    #[error("the minting rule produced an invalid IRI: {0}")]
    InvalidIri(String),
    /// The candidate IRI is already claimed. There is no silent suffixing.
    #[error("{0} is already claimed")]
    IriTaken(Iri),
    /// An entity already carries this title (case-insensitive, among labels,
    /// aliases and note names). Titles, not slugs, key this check, so
    /// `3D Animation` never gets a second IRI beside `3-d-animation`.
    #[error("{title:?} is already the title of {existing:?}")]
    TitleTaken {
        /// The title as requested.
        title: String,
        /// Every entity carrying it, sorted.
        existing: Vec<Iri>,
    },
}

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
/// refused for a kind without a namespace, an empty slug, or an IRI already
/// claimed in the context.
pub(crate) fn mint_slug(
    namespace: Option<&str>,
    title: &str,
    ctx: &MintContext<'_>,
) -> Result<Iri, MintRefusal> {
    let ns = namespace.ok_or(MintRefusal::NoRule)?;
    let slug = slugify(title);
    if slug.is_empty() {
        return Err(MintRefusal::EmptySlug);
    }
    let text = format!("{ns}{slug}");
    let iri = Iri::parse(text.clone()).map_err(|_| MintRefusal::InvalidIri(text))?;
    if ctx.is_taken(&iri) {
        return Err(MintRefusal::IriTaken(iri));
    }
    Ok(iri)
}

impl Model {
    /// Mint an IRI for a new entity the author is explicitly creating. This
    /// is **the** entry point. It applies the guards every profile shares,
    /// then the profile's rule ([`Profile::mint_iri`]):
    ///
    /// 1. If any entity already carries `title` (trimmed, case-insensitive,
    ///    among labels, aliases and note names), refuse with
    ///    [`MintRefusal::TitleTaken`], naming it. The caller should link
    ///    that entity, not create a second one.
    /// 2. Ask the profile for a candidate.
    /// 3. If the candidate is already claimed, refuse with
    ///    [`MintRefusal::IriTaken`]. This is checked here too, so a custom
    ///    profile cannot skip it.
    ///
    /// Never called while building a model.
    ///
    /// # Errors
    /// Every [`MintRefusal`].
    pub fn mint_iri(
        &self,
        profile: &dyn Profile,
        kind: Kind,
        title: &str,
    ) -> Result<Iri, MintRefusal> {
        let existing = self.entities_named(title);
        if !existing.is_empty() {
            return Err(MintRefusal::TitleTaken {
                title: title.to_owned(),
                existing,
            });
        }
        let iri = profile.mint_iri(kind, title, &MintContext { model: Some(self) })?;
        if self.is_claimed(&iri) {
            return Err(MintRefusal::IriTaken(iri));
        }
        Ok(iri)
    }
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
