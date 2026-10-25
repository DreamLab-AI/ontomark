//! The transport seam and the one request/response round trip.

use std::time::Duration;

use ontomark_ingest::Unavailable;
use serde_json::{json, Map, Value};

use crate::config::{JevConfig, Resolved};

/// A transport failure, before any HTTP status exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    /// The deadline passed.
    Timeout,
    /// Anything else (DNS, connection, TLS).
    Failed,
}

/// A blocking HTTP POST. The crate's only I/O boundary: tests mock it, and
/// without the `http` feature the crate ships no implementation.
pub trait Transport {
    /// POST `body` as JSON to `url`, with a bearer token when given, and
    /// return the status and body text. A non-2xx status is a normal return.
    ///
    /// # Errors
    /// [`TransportError`] when no HTTP response arrived.
    fn post(
        &self,
        url: &str,
        bearer: Option<&str>,
        body: &Value,
        timeout: Duration,
    ) -> Result<(u16, String), TransportError>;
}

/// `ureq` over HTTP(S).
#[cfg(feature = "http")]
#[derive(Debug, Clone, Copy, Default)]
pub struct UreqTransport;

#[cfg(feature = "http")]
impl Transport for UreqTransport {
    fn post(
        &self,
        url: &str,
        bearer: Option<&str>,
        body: &Value,
        timeout: Duration,
    ) -> Result<(u16, String), TransportError> {
        let agent = ureq::AgentBuilder::new().timeout(timeout).build();
        let mut req = agent.post(url);
        if let Some(b) = bearer {
            req = req.set("Authorization", &format!("Bearer {b}"));
        }
        match req.send_json(body.clone()) {
            Ok(r) => {
                let status = r.status();
                r.into_string()
                    .map(|t| (status, t))
                    .map_err(|_| TransportError::Failed)
            }
            Err(ureq::Error::Status(code, r)) => Ok((code, r.into_string().unwrap_or_default())),
            Err(ureq::Error::Transport(t)) => {
                if t.to_string().to_lowercase().contains("timed out") {
                    Err(TransportError::Timeout)
                } else {
                    Err(TransportError::Failed)
                }
            }
        }
    }
}

/// Where fallback reasons go. Receives one line, never request content.
pub type LogSink = Box<dyn Fn(&str)>;

/// Reads an environment variable (injectable so tests need no process state).
pub type EnvReader = Box<dyn Fn(&str) -> Option<String>>;

/// One configured client: settings, an environment reader, a transport and
/// a log sink.
pub struct JevClient<T: Transport> {
    pub(crate) config: JevConfig,
    transport: T,
    env: EnvReader,
    log: LogSink,
}

impl<T: Transport> std::fmt::Debug for JevClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JevClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<T: Transport> JevClient<T> {
    /// Build a client that reads `std::env` and logs to stderr.
    pub fn new(config: JevConfig, transport: T) -> Self {
        Self::with_env(
            config,
            transport,
            Box::new(|k| std::env::var(k).ok()),
            Box::new(|line| eprintln!("{line}")),
        )
    }

    /// Build a client with an explicit environment reader and log sink.
    pub fn with_env(config: JevConfig, transport: T, env: EnvReader, log: LogSink) -> Self {
        Self {
            config,
            transport,
            env,
            log,
        }
    }

    /// Log the one-line fallback reason (never request content) for a
    /// failed result and pass the result through. `flag-off` is silent.
    pub(crate) fn logged<R>(
        &self,
        set: &str,
        out: Result<R, Unavailable>,
    ) -> Result<R, Unavailable> {
        if let Err(u) = &out {
            if u.reason != "flag-off" {
                (self.log)(&format!(
                    "ontomark-jev: fallback reason={} question-set={set}",
                    u.reason
                ));
            }
        }
        out
    }

    /// Ask `questions` about `state`; returns the `answers` object, or an
    /// [`Unavailable`] carrying the one-word fallback reason.
    pub(crate) fn ask(
        &self,
        state: Value,
        questions: Map<String, Value>,
    ) -> Result<Map<String, Value>, Unavailable> {
        self.round_trip(state, questions)
    }

    fn round_trip(
        &self,
        state: Value,
        questions: Map<String, Value>,
    ) -> Result<Map<String, Value>, Unavailable> {
        let Resolved { url, model, bearer } = self.config.resolve(&*self.env)?;
        let body = json!({ "model": model, "state": state, "questions": questions });
        let (status, text) = self
            .transport
            .post(&url, bearer.as_deref(), &body, self.config.timeout)
            .map_err(|e| {
                Unavailable::new(match e {
                    TransportError::Timeout => "timeout",
                    TransportError::Failed => "transport",
                })
            })?;
        if !(200..300).contains(&status) {
            return Err(Unavailable::new(format!("http-{status}")));
        }
        let parsed: Value =
            serde_json::from_str(&text).map_err(|_| Unavailable::new("malformed"))?;
        let answers = parsed
            .get("answers")
            .and_then(Value::as_object)
            .ok_or_else(|| Unavailable::new("malformed"))?;
        // Every question must have an answer of its own type.
        for (id, q) in &questions {
            let want = q.get("type").and_then(Value::as_str).unwrap_or_default();
            let got = answers
                .get(id)
                .and_then(|a| a.get("type"))
                .and_then(Value::as_str);
            if got != Some(want) {
                return Err(Unavailable::new("malformed"));
            }
        }
        Ok(answers.clone())
    }
}
