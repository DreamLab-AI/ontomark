//! The embedder transport for `--with-rvdb`: Xinference's OpenAI-compatible
//! `/v1/embeddings` endpoint. The record shape, batching, normalisation and the
//! width check live in `ontomark_compile::build::records`; this file only
//! moves bytes, which is why it is in the CLI and not the compile crate.

use ontomark_compile::build::records::{Embedder, MODEL_ID};
use serde_json::{json, Value};

/// The estate's default Xinference endpoint on the LAN.
pub const DEFAULT_ENDPOINT: &str = "http://192.168.2.132:9997/v1/embeddings";

/// Xinference over HTTP.
#[derive(Debug, Clone)]
pub struct Xinference {
    endpoint: String,
    timeout: std::time::Duration,
}

impl Xinference {
    /// Point at an endpoint.
    #[must_use]
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            timeout: std::time::Duration::from_secs(120),
        }
    }
}

impl Embedder for Xinference {
    fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let response: Value = ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .build()
            .post(&self.endpoint)
            .send_json(json!({ "model": MODEL_ID, "input": texts }))
            .map_err(|e| anyhow::anyhow!("embedding request to {} failed: {e}", self.endpoint))?
            .into_json()
            .map_err(|e| anyhow::anyhow!("embedding response was not JSON: {e}"))?;
        let data = response["data"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("embedding response has no `data` array"))?;
        data.iter()
            .map(|item| {
                item["embedding"]
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("embedding is not an array"))
                    .map(|a| {
                        a.iter()
                            .map(|n| {
                                #[allow(clippy::cast_possible_truncation)]
                                let v = n.as_f64().unwrap_or(0.0) as f32;
                                v
                            })
                            .collect()
                    })
            })
            .collect()
    }
}
