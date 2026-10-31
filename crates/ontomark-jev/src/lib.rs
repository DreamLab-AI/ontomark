//! The **Jev option** for Ontomark assisted ingest.
//!
//! `ontomark-ingest` is pure and never calls a model; it exposes three hooks
//! ([`ontomark_ingest::FactJudge`], [`ontomark_ingest::CandidateSelector`]
//! and [`ontomark_ingest::FieldSuggester`]) with deterministic defaults that
//! do nothing. This crate is the host side: it fills those hooks with typed
//! Choice and Noul decisions over `POST /v1/systemone`.
//!
//! | Item | Type | Hook |
//! |---|---|---|
//! | OM-06-S1-01 verify candidate facts before accept | [`JevFactJudge`] | `Ingest::accept_judged` |
//! | OM-06-S1-02 select candidate names and spans | [`JevSelector`] | `Ingest::select_responses` |
//! | OM-01-S1-01 suggest frontmatter type/relation fields | [`JevSuggester`] | `Ingest::suggest_fields` |
//!
//! Off by default ([`JevConfig::default`]); with the switch off no request is
//! built. Every failure (no endpoint, timeout, HTTP error, malformed body,
//! low confidence) returns [`ontomark_ingest::Unavailable`], and the ingest
//! engine then returns exactly what it returns without a hook. The crate has
//! no network code unless the `http` feature supplies `UreqTransport`;
//! otherwise a host implements [`Transport`].
//! (`UreqTransport` is that feature's type.)
//!
//! Authority stays in code: a hook can only withhold or narrow what the
//! ordinary `accept` would have produced, or propose a field for review.
//! Nothing here writes, signs or approves.
//!
//! Every item is a "proposed research candidate; no measured uplift". The
//! default thresholds withhold and narrow nothing until fitted on labelled
//! data.
//!
//! ```
//! use std::time::Duration;
//! use ontomark_jev::{JevClient, JevConfig, Transport, TransportError};
//! use serde_json::Value;
//!
//! // Off by default: nothing resolves, so no request is ever built.
//! let no_env = |_: &str| None;
//! assert_eq!(JevConfig::default().resolve(&no_env).unwrap_err().reason, "flag-off");
//!
//! // Switched on with an explicit endpoint, the bearer comes from the
//! // variable `key_env` names, never from the config itself.
//! let config = JevConfig {
//!     enabled: true,
//!     endpoint: Some("http://127.0.0.1:8080/v1/systemone".into()),
//!     key_env: Some("MY_JEV_KEY".into()),
//!     local: true,
//!     ..JevConfig::default()
//! };
//! let env = |k: &str| (k == "MY_JEV_KEY").then(|| "secret".to_owned());
//! let resolved = config.resolve(&env).unwrap();
//! assert_eq!(resolved.bearer.as_deref(), Some("secret"));
//!
//! // Without the `http` feature a host supplies the only I/O boundary.
//! struct Offline;
//! impl Transport for Offline {
//!     fn post(&self, _: &str, _: Option<&str>, _: &Value, _: Duration)
//!         -> Result<(u16, String), TransportError> {
//!         Err(TransportError::Failed)
//!     }
//! }
//! let _client = JevClient::new(config, Offline);
//! ```

mod client;
mod config;
mod create;
mod hooks;

#[cfg(feature = "http")]
pub use client::UreqTransport;
pub use client::{EnvReader, JevClient, LogSink, Transport, TransportError};
pub use config::{JevConfig, Resolved, CLOUD_ENDPOINT, DEFAULT_MODEL};
pub use create::{
    create_questions, create_state, CreateQuestion, CreateSuggestion, JevCreateSuggester,
    ParentCandidate, CREATE_SET,
};
pub use hooks::{
    fact_questions, fact_state, select_questions, select_state, suggest_questions, suggest_state,
    JevFactJudge, JevSelector, JevSuggester, FACT_SET, SELECT_SET, SUGGEST_SET,
};
