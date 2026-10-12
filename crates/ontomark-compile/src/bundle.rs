//! Typed readers for the public C3 bundle and its scaffold/prose indexes.
//!
//! [`BundleManifest`] is the emitter's existing generation stamp, not a second
//! envelope. [`BundleManifest::from_json`] checks identity and safe artefact
//! claims before a consumer opens files. File bytes, digests, staleness and
//! activation still have to be checked by that consumer; deserialisation alone
//! never attests a served generation.
//!
//! Scaffold v1 omitted fields remain optional for existing readers. Omission
//! does not invent an ontology assertion or an attestation. Consumers bind the
//! three trust parameters to their own governance types; the compiler owns the
//! wire field names, while a hosting application owns trust interpretation.
//!
//! ```
//! use ontomark_compile::bundle::ScaffoldIndex;
//! let index: ScaffoldIndex = serde_json::from_str(
//!     r#"{"version":1,"generated":"2026-10-11","classes":{"a":{"t":"A"}}}"#
//! )?;
//! assert_eq!(index.classes["a"].t.as_deref(), Some("A"));
//! assert!(index.classes["a"].sup.is_empty());
//! # Ok::<(), serde_json::Error>(())
//! ```

use std::collections::HashSet;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::build::generation::Artifact;
use crate::build::generation::{generation_id, Generation};

/// The public C3 manifest, identical to the build's [`Generation`] stamp.
/// Legacy markers whose `artifacts` is a map require an explicit legacy reader.
pub type BundleManifest = Generation;

/// A malformed or internally inconsistent manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// JSON did not have the typed C3 shape, including every required field.
    #[error("invalid C3 manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Identity, digest syntax, paths or artefact claims are inconsistent.
    #[error("invalid C3 manifest: {0}")]
    Invalid(String),
}

impl Generation {
    /// Read and validate the current C3 marker without field coercion/defaults
    /// (bar `scope`, which is absent from a public marker and defaults to public).
    ///
    /// Unknown extension fields are permitted. This does not read or hash any
    /// artefact, consult a clock, fetch a resource or activate a generation.
    ///
    /// # Errors
    /// [`ManifestError`] for a malformed shape or failed [`Self::validate`].
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_json::from_str(json)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Check C3 identity, digest syntax, safe relative paths and unique claims.
    /// Source identity may be `unknown` outside Git, as the emitter specifies.
    ///
    /// # Errors
    /// [`ManifestError::Invalid`] when a required invariant is broken.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let invalid = |reason: &str| ManifestError::Invalid(reason.to_owned());
        if self.commit.is_empty() || self.id != generation_id(&self.commit, self.dirty) {
            return Err(invalid("id does not identify commit and dirty state"));
        }
        if !lower_hex(&self.content_digest, 64) {
            return Err(invalid("content_digest is not lowercase SHA-256"));
        }
        if !self
            .ontology_digest
            .strip_prefix("sha256-12-")
            .is_some_and(|s| lower_hex(s, 12))
        {
            return Err(invalid(
                "ontology_digest is not a sha256-12 content address",
            ));
        }
        if self.generated_at.is_empty() {
            return Err(invalid("generated_at is empty"));
        }
        let mut names = HashSet::new();
        for artifact in &self.artifacts {
            let safe = !artifact.name.contains(['\\', '\0'])
                && artifact
                    .name
                    .split('/')
                    .all(|p| !matches!(p, "" | "." | ".."));
            if !safe {
                return Err(invalid("artefact name is not a safe relative POSIX path"));
            }
            if !names.insert(&artifact.name) {
                return Err(invalid("artefact path is claimed more than once"));
            }
            if !lower_hex(&artifact.sha256, 64) {
                return Err(invalid("artefact hash is not lowercase SHA-256"));
            }
        }
        Ok(())
    }
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// One scaffold-v1 class record. Keys retain the established compact spelling.
/// `A`, `M` and `C` are consumer-owned attestation, mapping and computation types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "A: Deserialize<'de>, M: Deserialize<'de>, C: Deserialize<'de>"))]
pub struct ClassEntry<A = Value, M = Value, C = Value> {
    /// Display title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
    /// Short definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<String>,
    /// Domain identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dom: Option<String>,
    /// Quality score. A producer's zero score is represented by JSON null.
    #[serde(default)]
    pub q: Option<f64>,
    /// Maturity identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<String>,
    /// Asserted superclass identifiers.
    #[serde(default)]
    pub sup: Vec<String>,
    /// Inferred superclass identifiers, kept separate from asserted ones.
    #[serde(default)]
    pub isup: Vec<String>,
    /// Typed relationship keys and targets in producer order.
    #[serde(default)]
    pub rel: IndexMap<String, Vec<String>>,
    /// Backlink page identifiers.
    #[serde(default)]
    pub bl: Vec<String>,
    /// Producer attestation when the source carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated: Option<A>,
    /// Verification attestations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verified: Vec<A>,
    /// Source mappings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub map: Vec<M>,
    /// Attested computations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ac: Vec<C>,
    /// Forward-compatible wire fields, preserved without interpreting them.
    #[serde(flatten)]
    pub extensions: IndexMap<String, Value>,
}

impl<A, M, C> Default for ClassEntry<A, M, C> {
    fn default() -> Self {
        Self {
            t: None,
            d: None,
            dom: None,
            q: None,
            m: None,
            sup: Vec::new(),
            isup: Vec::new(),
            rel: IndexMap::new(),
            bl: Vec::new(),
            generated: None,
            verified: Vec::new(),
            map: Vec::new(),
            ac: Vec::new(),
            extensions: IndexMap::new(),
        }
    }
}

/// The scaffold-v1 index consumed by Loom and other corpus readers.
/// Field omissions in legacy indexes remain distinguishable from a versioned
/// producer response through [`Self::version`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "A: Deserialize<'de>, M: Deserialize<'de>, C: Deserialize<'de>"))]
pub struct ScaffoldIndex<A = Value, M = Value, C = Value> {
    /// Declared wire version; absent on unversioned legacy fixtures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i64>,
    /// Producer's build instant; absent legacy values remain empty.
    #[serde(default)]
    pub generated: String,
    /// Producer counts, retaining key order and future counters.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub counts: IndexMap<String, usize>,
    /// Class records in producer order.
    #[serde(default)]
    pub classes: IndexMap<String, ClassEntry<A, M, C>>,
    /// Forward-compatible index fields.
    #[serde(flatten)]
    pub extensions: IndexMap<String, Value>,
}

/// One prose-index record, keyed by the same class identifier as the scaffold.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProseEntry {
    /// Definition before the scaffold's truncation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dfull: Option<String>,
    /// Extracted Current Landscape section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cl: Option<String>,
    /// Forward-compatible prose fields.
    #[serde(flatten)]
    pub extensions: IndexMap<String, Value>,
}

/// The public prose-v1 index. Empty or omitted legacy entries carry no prose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProseIndex {
    /// Declared wire version, absent for legacy fixtures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i64>,
    /// Producer's build instant.
    #[serde(default)]
    pub generated: String,
    /// Producer counts in wire order.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub counts: IndexMap<String, usize>,
    /// Prose by class identifier.
    #[serde(default)]
    pub pages: IndexMap<String, ProseEntry>,
    /// Forward-compatible index fields.
    #[serde(flatten)]
    pub extensions: IndexMap<String, Value>,
}
