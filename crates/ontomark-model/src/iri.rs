//! Absolute IRIs, stored verbatim, and the prefix map that expands CURIEs.

use std::fmt;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Why a string is not an IRI, or a CURIE could not be expanded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IriError {
    /// No `scheme:` prefix, or the scheme is malformed.
    #[error("{0:?} is not an absolute IRI (no valid scheme)")]
    NoScheme(String),
    /// Whitespace or a character IRIs forbid (`<>"{}|\^` and backtick).
    #[error("{0:?} contains a character IRIs do not allow")]
    BadChar(String),
    /// A `prefix:local` whose prefix is not declared and which is not an
    /// absolute IRI either.
    #[error("prefix of {0:?} is not declared")]
    UndeclaredPrefix(String),
}

/// An absolute IRI (RFC 3987 shape check), held exactly as written.
///
/// No normalisation, no prefix insertion, no tail extraction: an IRI that
/// comes in is the IRI that goes out (ADR-2015, defect 3). Identity in the
/// model is an `Iri`, never a title.
///
/// ```
/// # use ontomark_model::Iri;
/// assert!(Iri::parse("urn:ngm:class:knowledge-graph").is_ok());
/// assert!(Iri::parse("https://example.org/ns#Thing").is_ok());
/// assert!(Iri::parse("did:nostr:abc").is_ok());
/// assert!(Iri::parse("Knowledge Graph").is_err());
/// assert!(Iri::parse("C:\\path").is_err()); // one-letter scheme: a drive, not an IRI
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Iri(String);

impl Iri {
    /// Validate `s` as an absolute IRI.
    ///
    /// The scheme must be two or more characters (`[A-Za-z][A-Za-z0-9+.-]+`),
    /// so a Windows drive letter is never mistaken for one, and something must
    /// follow the colon.
    ///
    /// # Errors
    /// [`IriError::NoScheme`] or [`IriError::BadChar`].
    pub fn parse(s: impl Into<String>) -> Result<Self, IriError> {
        let s = s.into();
        let Some(colon) = s.find(':') else {
            return Err(IriError::NoScheme(s));
        };
        let scheme = &s[..colon];
        let mut chars = scheme.chars();
        let ok_scheme = scheme.len() >= 2
            && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
        if !ok_scheme || colon + 1 == s.len() {
            return Err(IriError::NoScheme(s));
        }
        if s.chars().any(|c| {
            c.is_whitespace()
                || c.is_control()
                || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '\\' | '^' | '`')
        }) {
            return Err(IriError::BadChar(s));
        }
        Ok(Self(s))
    }

    /// The IRI text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The scheme (before the first `:`).
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.0.split_once(':').map_or("", |(s, _)| s)
    }
}

impl fmt::Display for Iri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Iri {
    type Error = IriError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<Iri> for String {
    fn from(i: Iri) -> Self {
        i.0
    }
}

/// Prefix → namespace IRI, for CURIE expansion and compaction.
///
/// ```
/// # use ontomark_model::{PrefixMap, Iri};
/// let p = PrefixMap::standard();
/// assert_eq!(p.expand("owl:Class").unwrap().as_str(), "http://www.w3.org/2002/07/owl#Class");
/// // An absolute IRI whose scheme is not a declared prefix is kept verbatim.
/// assert_eq!(p.expand("urn:isbn:0451450523").unwrap().as_str(), "urn:isbn:0451450523");
/// let skos = Iri::parse("http://www.w3.org/2004/02/skos/core#broader").unwrap();
/// assert_eq!(p.compact(&skos).as_deref(), Some("skos:broader"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrefixMap {
    map: IndexMap<String, Iri>,
}

impl PrefixMap {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `rdf`, `rdfs`, `owl`, `xsd`, `skos`, `dcterms` and `prov`.
    #[must_use]
    pub fn standard() -> Self {
        let mut p = Self::new();
        for (k, v) in [
            ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
            ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
            ("owl", "http://www.w3.org/2002/07/owl#"),
            ("xsd", "http://www.w3.org/2001/XMLSchema#"),
            ("skos", "http://www.w3.org/2004/02/skos/core#"),
            ("dcterms", "http://purl.org/dc/terms/"),
            ("prov", "http://www.w3.org/ns/prov#"),
        ] {
            p.map.insert(k.to_owned(), Iri(v.to_owned()));
        }
        p
    }

    /// Declare (or redeclare) `prefix`.
    pub fn insert(&mut self, prefix: impl Into<String>, namespace: Iri) {
        self.map.insert(prefix.into(), namespace);
    }

    /// The namespace for `prefix`.
    #[must_use]
    pub fn get(&self, prefix: &str) -> Option<&Iri> {
        self.map.get(prefix)
    }

    /// Declared prefixes in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Iri)> {
        self.map.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Expand `prefix:local` when `prefix` is declared; otherwise accept an
    /// absolute IRI unchanged. A declared prefix wins over a scheme of the
    /// same name.
    ///
    /// # Errors
    /// [`IriError::UndeclaredPrefix`] when `text` is neither, or the IRI
    /// errors when the expansion is not a valid IRI.
    pub fn expand(&self, text: &str) -> Result<Iri, IriError> {
        if let Some((prefix, local)) = text.split_once(':') {
            if let Some(ns) = self.map.get(prefix) {
                return Iri::parse(format!("{}{local}", ns.0));
            }
        }
        Iri::parse(text).map_err(|e| match e {
            IriError::NoScheme(s) => IriError::UndeclaredPrefix(s),
            other => other,
        })
    }

    /// The shortest `prefix:local` for `iri`, using the longest matching
    /// namespace. `None` when no namespace matches.
    #[must_use]
    pub fn compact(&self, iri: &Iri) -> Option<String> {
        self.map
            .iter()
            .filter(|(_, ns)| iri.0.starts_with(&ns.0) && iri.0.len() > ns.0.len())
            .max_by_key(|(_, ns)| ns.0.len())
            .map(|(p, ns)| format!("{p}:{}", &iri.0[ns.0.len()..]))
    }
}
