//! Portable provenance wire types. Processing and review are separate claims.
/// Identity of the compiler that produced a bundle, never a content author.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuildProducer {
    /// Compiler package name.
    pub name: String,
    /// Compiler package version.
    pub version: String,
}

/// Source review binding counts. Absence on a legacy bundle means unknown.
/// These counts describe source content, not a redacted projection's bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReviewSummary {
    /// Human stamps bound to the source's current reviewed values.
    pub current: usize,
    /// Human stamps with changed content or invalid bindings.
    pub stale: usize,
    /// Historical human stamps without content bindings.
    pub unbound: usize,
}
