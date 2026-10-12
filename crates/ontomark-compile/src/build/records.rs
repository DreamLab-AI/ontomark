//! `data/ontology-corpus.records.jsonl`: the portable vector records
//! (contract C3, `--with-rvdb`).
//!
//! **Division of labour.** The Loom owns the Postgres write channel: its
//! `build-concept-records` turns `scaffold-index.json` and `prose-index.json`
//! into concept records and `stage_corpus` embeds and stages them. This module
//! replaces neither. It produces the *portable* form, records with their
//! vectors inline, for a consumer that wants the bundle without a database.
//! The Loom's `promote_vault_build` turns them into its serving artefact
//! (`ontology-corpus.rvdb`) and writes that artefact's sidecar: only the
//! builder of a file can describe it.
//!
//! **This module is pure.** It never opens a socket: the host supplies an
//! [`Embedder`] (the `ontomark` CLI implements the Xinference endpoint) and
//! the build calls it through [`embed_all`]. That keeps `ontomark-compile`
//! free of transport code and makes the emitter testable with a fake.
//!
//! **The embedder lock is not a convention.** `bge-small-en-v1.5` at 384
//! dimensions, and only that: the whole namespace must be cosine-comparable,
//! so every vector is width-checked and a mismatch is an error, never a
//! quietly wrong answer. The HNSW rebuild after a bulk load is a separate,
//! non-concurrent step and is never folded in here.
//!
//! Ported from VisionClaw `crates/vault/src/build/rvdb.rs` (the estate's own
//! code; see `docs/adr/ADR-OM-005`), with the Xinference transport moved to
//! the CLI.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The locked embedding model.
pub const MODEL_ID: &str = "bge-small-en-v1.5";
/// The locked vector width.
pub const DIMENSIONS: usize = 384;
/// Where `--with-rvdb` writes the portable records, relative to the bundle.
pub const RECORDS_PATH: &str = "data/ontology-corpus.records.jsonl";
/// Records per embedder call.
pub const BATCH: usize = 96;

/// One concept record: the seed-finding surface a semantic query matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConceptRecord {
    /// `urn:ngm:class:<slug>`: the join key across Turtle, scaffold, prose
    /// and the HNSW index.
    pub key: String,
    /// `loom:ontology-corpus:<key>`: the primary key, so upserts are
    /// idempotent.
    pub id: String,
    /// The embedded text.
    pub text: String,
    /// slug, title, domain, maturity, quality, `has_prose`, generation.
    pub metadata: Value,
    /// The unit-norm vector, present only after embedding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

/// Anything that turns texts into vectors. Implemented by the host.
pub trait Embedder {
    /// Embed a batch, returning one vector per input, in order.
    ///
    /// # Errors
    /// Any transport or protocol failure. Width is checked by the caller
    /// ([`check_width`]), so an implementation need not.
    fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}

/// Refuse a vector that is not [`DIMENSIONS`] wide.
///
/// # Errors
/// When `v.len() != DIMENSIONS`.
pub fn check_width(v: &[f32]) -> anyhow::Result<()> {
    anyhow::ensure!(
        v.len() == DIMENSIONS,
        "embedder returned {} dimensions, not {DIMENSIONS}: the namespace is locked to \
         {MODEL_ID} and a mismatch silently invalidates the index",
        v.len()
    );
    Ok(())
}

/// Scale a vector to unit length; a zero vector is returned unchanged.
#[must_use]
pub fn normalise(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

/// Build the concept records from the scaffold and prose indexes.
///
/// The embedded text is the class's *header*: title, definition, domain,
/// taxonomy and verbalised typed relations, plus the prose index's full
/// definition when there is one, so a vector query lands on the right IRI
/// rather than on a paragraph that merely mentions it.
#[must_use]
pub fn concept_records(scaffold: &Value, prose: &Value, generation: &str) -> Vec<ConceptRecord> {
    let Some(classes) = scaffold["classes"].as_object() else {
        return Vec::new();
    };
    let prose_pages = prose["pages"].as_object();

    classes
        .iter()
        .map(|(slug, entry)| {
            let key = format!("urn:ngm:class:{slug}");
            let title = entry["t"].as_str().unwrap_or(slug);
            let prose_entry = prose_pages.and_then(|p| p.get(slug));
            let dfull = prose_entry.and_then(|p| p["dfull"].as_str());
            let definition = dfull.or_else(|| entry["d"].as_str()).unwrap_or_default();

            let text = concept_text(title, definition, entry);
            ConceptRecord {
                id: format!("loom:ontology-corpus:{key}"),
                key,
                text,
                metadata: json!({
                    "slug": slug,
                    "title": title,
                    "domain": entry["dom"].as_str().unwrap_or_default(),
                    "maturity": entry["m"].as_str().unwrap_or_default(),
                    "quality": entry["q"].clone(),
                    "has_prose": prose_entry.is_some(),
                    "generation": generation,
                }),
                embedding: None,
            }
        })
        .collect()
}

/// The embedded header text for one class.
fn concept_text(title: &str, definition: &str, entry: &Value) -> String {
    let mut text = format!("{title}\n{definition}");
    if let Some(dom) = entry["dom"].as_str().filter(|d| !d.is_empty()) {
        let _ = write!(text, "\nDomain: {dom}");
    }
    if let Some(sup) = entry["sup"].as_array().filter(|a| !a.is_empty()) {
        let _ = write!(text, "\nIs a: {}", join_slugs(sup));
    }
    if let Some(rel) = entry["rel"].as_object() {
        for (predicate, targets) in rel {
            if let Some(list) = targets.as_array().filter(|a| !a.is_empty()) {
                let _ = write!(text, "\n{predicate}: {}", join_slugs(list));
            }
        }
    }
    text
}

fn join_slugs(items: &[Value]) -> String {
    items
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.replace('-', " "))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Embed every record in batches of [`BATCH`], in place, normalising and
/// width-checking each vector.
///
/// # Errors
/// The embedder's failure, a batch of the wrong length, or a vector of the
/// wrong width. A partial batch is never kept: the caller sees an error rather
/// than a half-embedded bundle.
pub fn embed_all(records: &mut [ConceptRecord], embedder: &dyn Embedder) -> anyhow::Result<()> {
    for chunk in records.chunks_mut(BATCH) {
        let texts: Vec<String> = chunk.iter().map(|r| r.text.clone()).collect();
        let vectors = embedder.embed(&texts)?;
        anyhow::ensure!(
            vectors.len() == texts.len(),
            "embedder returned {} vectors for {} inputs",
            vectors.len(),
            texts.len()
        );
        for (record, vector) in chunk.iter_mut().zip(vectors) {
            check_width(&vector)?;
            record.embedding = Some(normalise(vector));
        }
    }
    Ok(())
}

/// Serialise the records as JSONL, one record per line.
///
/// # Errors
/// A `serde_json` failure.
pub fn to_jsonl(records: &[ConceptRecord]) -> serde_json::Result<String> {
    let mut out = String::with_capacity(records.len() * 512);
    for r in records {
        out.push_str(&serde_json::to_string(r)?);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake that returns `width`-wide vectors with the input index in slot 0.
    struct Fake(usize);

    impl Embedder for Fake {
        fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let mut v = vec![0.0f32; self.0];
                    #[allow(clippy::cast_precision_loss)]
                    {
                        v[0] = (i + 1) as f32;
                    }
                    v
                })
                .collect())
        }
    }

    fn scaffold() -> Value {
        json!({"classes": {
            "knowledge-graph": {"t": "Knowledge Graph", "d": "A graph.", "dom": "ai",
                                "m": "established", "q": 0.8, "sup": ["graph"],
                                "rel": {"requires": ["ontology"]}},
            "graph": {"t": "Graph", "d": "Nodes and edges.", "dom": "", "m": "", "q": null}
        }})
    }

    #[test]
    fn records_carry_key_text_and_metadata() {
        let prose = json!({"pages": {"knowledge-graph": {"dfull": "A graph of typed entities."}}});
        let records = concept_records(&scaffold(), &prose, "visionGraph@abc");
        assert_eq!(records.len(), 2);
        let kg = records
            .iter()
            .find(|r| r.key == "urn:ngm:class:knowledge-graph")
            .unwrap();
        assert_eq!(kg.id, "loom:ontology-corpus:urn:ngm:class:knowledge-graph");
        assert_eq!(
            kg.text,
            "Knowledge Graph\nA graph of typed entities.\nDomain: ai\nIs a: graph\nrequires: ontology"
        );
        assert_eq!(kg.metadata["has_prose"], json!(true));
        assert_eq!(kg.metadata["generation"], json!("visionGraph@abc"));
        assert!(kg.embedding.is_none());
    }

    #[test]
    fn embedding_normalises_and_checks_width() {
        let mut records = concept_records(&scaffold(), &json!({}), "g");
        embed_all(&mut records, &Fake(DIMENSIONS)).unwrap();
        for r in &records {
            let v = r.embedding.as_ref().unwrap();
            assert_eq!(v.len(), DIMENSIONS);
            assert!((v[0] - 1.0).abs() < 1e-6, "unit norm");
        }
        let err = embed_all(&mut records, &Fake(3)).unwrap_err();
        assert!(err.to_string().contains("3 dimensions, not 384"), "{err}");
    }

    #[test]
    fn jsonl_is_one_record_per_line() {
        let records = concept_records(&scaffold(), &json!({}), "g");
        let text = to_jsonl(&records).unwrap();
        assert_eq!(text.lines().count(), 2);
        let back: ConceptRecord = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert!(back.key.starts_with("urn:ngm:class:"));
    }

    #[test]
    fn zero_vector_is_left_alone() {
        assert_eq!(normalise(vec![0.0, 0.0]), vec![0.0, 0.0]);
    }
}
