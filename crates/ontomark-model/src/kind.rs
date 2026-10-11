//! Entity kinds.

use serde::{Deserialize, Serialize};

/// What sort of OWL entity a page declares. Explicit and closed: a page
/// whose declared type the profile does not map is refused, never coerced to
/// [`Kind::Class`] (ADR-2015, defect 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// `owl:Class`.
    Class,
    /// `owl:NamedIndividual`.
    Individual,
    /// A property of the given kind.
    Property(PropertyKind),
}

/// The three OWL property kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyKind {
    /// `owl:ObjectProperty`.
    Object,
    /// `owl:DatatypeProperty`.
    Data,
    /// `owl:AnnotationProperty`.
    Annotation,
}

impl Kind {
    /// The OWL declaration IRI for this kind.
    #[must_use]
    pub fn owl_iri(self) -> &'static str {
        match self {
            Self::Class => "http://www.w3.org/2002/07/owl#Class",
            Self::Individual => "http://www.w3.org/2002/07/owl#NamedIndividual",
            Self::Property(PropertyKind::Object) => "http://www.w3.org/2002/07/owl#ObjectProperty",
            Self::Property(PropertyKind::Data) => "http://www.w3.org/2002/07/owl#DatatypeProperty",
            Self::Property(PropertyKind::Annotation) => {
                "http://www.w3.org/2002/07/owl#AnnotationProperty"
            }
        }
    }

    /// `true` for any property kind.
    #[must_use]
    pub fn is_property(self) -> bool {
        matches!(self, Self::Property(_))
    }
}
