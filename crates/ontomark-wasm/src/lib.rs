//! Portable ontomark engine for hosts such as the Obsidian plugin.
//!
//! The crate turns a set of Markdown files into an in-memory ontology index,
//! classifies it in resumable steps, answers local read-only SPARQL, and plans
//! guarded Markdown patches. It never reads a filesystem or makes network
//! requests: the host supplies file text and applies the patches it approves.
//! Standard SPARQL functions such as `NOW`, `RAND` and `UUID` can read the
//! runtime clock or entropy; their results are not deterministic fixtures.
//!
//! Everything goes through one JSON request/response protocol,
//! [`engine::Request`] → [`engine::Response`], so the native API, the
//! [`WasmEngine`] binding and the plugin's worker share a single schema and can
//! be compared byte-for-byte.
//!
//! ```
//! use ontomark_wasm::engine::Engine;
//!
//! let mut engine = Engine::new();
//! let load = r#"{"kind":"load","generation":1,"files":[
//!   {"path":"Animal.md","text":"---\ntype: class\n---\n"},
//!   {"path":"Dog.md","text":"---\ntype: class\nsubclass-of: [\"[[Animal]]\"]\n---\n"}]}"#;
//! assert!(engine.handle_json(load).contains(r#""ok":true"#));
//! let ask = r#"{"kind":"query","generation":1,"sparql":
//!   "ASK { ?d <http://www.w3.org/2000/01/rdf-schema#subClassOf> ?a }"}"#;
//! assert!(engine.handle_json(ask).contains(r#""boolean":true"#));
//! // A read against an older generation is refused, not answered.
//! let stale = r#"{"kind":"index","generation":0}"#;
//! assert!(engine.handle_json(stale).contains(r#""code":"stale""#));
//! ```
//!
//! Classification is `ontomark-reason` (OWL 2 EL over `whelk-el`) behind the
//! stepped [`reason::Reasoner`] seam. Modules [`profile`] and [`projection`]
//! retain the released engine's document semantics; adopting the compiler's
//! available typed projection requires the mapping and parity checks described
//! in their module docs.

pub mod engine;
pub mod profile;
pub mod projection;
pub mod query;
pub mod reason;
pub mod store;
pub mod text;

use wasm_bindgen::prelude::wasm_bindgen;

/// The engine as a JavaScript class.
///
/// ```js
/// import { initSync, WasmEngine } from "./ontomark_wasm.js";
/// initSync({ module: wasmBytes });
/// const engine = new WasmEngine();
/// const res = JSON.parse(engine.handle(JSON.stringify({ kind: "version" })));
/// ```
#[wasm_bindgen]
pub struct WasmEngine {
    inner: engine::Engine,
}

#[wasm_bindgen]
impl WasmEngine {
    /// An empty engine (generation 0, generic profile).
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: engine::Engine::new(),
        }
    }

    /// Handle one JSON request; returns the JSON response.
    #[must_use]
    pub fn handle(&mut self, request: &str) -> String {
        self.inner.handle_json(request)
    }

    /// The engine's current generation.
    #[must_use]
    pub fn generation(&self) -> f64 {
        // Generations are host counters well below 2^53.
        self.inner.generation() as f64
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// The crate version.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
