//! Configuration and endpoint resolution for the Jev option.

use std::time::Duration;

use ontomark_ingest::Unavailable;

/// The public TypeSafe endpoint (contract step 3).
pub const CLOUD_ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
/// The model alias used when none is configured.
pub const DEFAULT_MODEL: &str = "jev-latest";

/// The Jev option's settings. [`JevConfig::default`] is **off**.
#[derive(Debug, Clone, PartialEq)]
pub struct JevConfig {
    /// Master switch. With `false` no request is built and nothing leaves
    /// the process (`ONTOMARK_JEV=1` turns it on in [`JevConfig::from_env`]).
    pub enabled: bool,
    /// Explicit `/v1/systemone` endpoint (contract step 1).
    pub endpoint: Option<String>,
    /// Explicit model name or alias.
    pub model: Option<String>,
    /// Name of the environment variable holding the bearer key for
    /// `endpoint`. The key itself is never stored here.
    pub key_env: Option<String>,
    /// The configured endpoint is declared local to this machine/estate.
    pub local: bool,
    /// Vault notes are private: refuse the agentbox proxy and the cloud
    /// endpoint unless declared local. [`JevConfig::default`] is `true` (a
    /// caller constructing the option in code stays private); [`JevConfig::from_env`]
    /// sets it to `false` when `ONTOMARK_JEV_ALLOW_CLOUD=1`.
    pub local_only: bool,
    /// `ontomark create --suggest` is local-only: it refuses an endpoint not
    /// declared local (`local`, or `AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL=1`)
    /// unless this is `true`, because the vault is private. Default `false`.
    pub allow_cloud: bool,
    /// Hard deadline for one request. Defaults to 4 s; there are no retries.
    pub timeout: Duration,
    /// [`crate::JevFactJudge`] withholds a fact only when the probability
    /// that its quote supports it is at or below this. The default `0.0`
    /// withholds nothing automatically; fit it on labelled data first.
    pub reject_at_or_below: f64,
    /// [`crate::JevSelector`] narrows candidates only at or above this
    /// Choice confidence. The default `1.0` narrows nothing automatically.
    pub select_min_confidence: f64,
    /// [`crate::JevSuggester`] proposes a field only at or above this Choice
    /// confidence (a proposal is only shown to a reviewer). Default `0.5`.
    pub suggest_min_confidence: f64,
    /// A suggestion below this confidence is flagged ambiguous. Default 0.75.
    pub suggest_clear_confidence: f64,
    /// [`crate::JevCreateSuggester`] includes a domain or parent only at or
    /// above this Noul probability. Default 0.7 (advisory output only).
    pub suggest_min_probability: f64,
}

impl Default for JevConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            model: None,
            key_env: None,
            local: false,
            local_only: true,
            allow_cloud: false,
            timeout: Duration::from_secs(4),
            reject_at_or_below: 0.0,
            select_min_confidence: 1.0,
            suggest_min_confidence: 0.5,
            suggest_clear_confidence: 0.75,
            suggest_min_probability: 0.7,
        }
    }
}

/// A resolved destination.
#[derive(Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The URL to POST to.
    pub url: String,
    /// The model name.
    pub model: String,
    /// The bearer token; `None` for the keyless loopback proxy.
    pub bearer: Option<String>,
}

impl std::fmt::Debug for Resolved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolved")
            .field("url", &self.url)
            .field("model", &self.model)
            .field("bearer", &self.bearer.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

fn flag(v: Option<String>) -> bool {
    v.is_some_and(|v| v.trim() == "1")
}

fn nonempty(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

impl JevConfig {
    /// Read the option from the environment: `ONTOMARK_JEV=1` enables it;
    /// `ONTOMARK_JEV_ENDPOINT`, `ONTOMARK_JEV_MODEL`, `ONTOMARK_JEV_KEY_ENV`
    /// and `ONTOMARK_JEV_LOCAL=1` fill the explicit step.
    ///
    /// `ONTOMARK_JEV_ALLOW_CLOUD=1` sets [`allow_cloud`](Self::allow_cloud)
    /// and lifts [`local_only`](Self::local_only), so the agentbox proxy and
    /// the `TYPESAFE_API_KEY` step are allowed. Absent (or any other value) the
    /// consumer stays local-only and refuses both unless declared local.
    pub fn from_env(env: &dyn Fn(&str) -> Option<String>) -> Self {
        let allow_cloud = flag(env("ONTOMARK_JEV_ALLOW_CLOUD"));
        Self {
            enabled: flag(env("ONTOMARK_JEV")),
            endpoint: nonempty(env("ONTOMARK_JEV_ENDPOINT")),
            model: nonempty(env("ONTOMARK_JEV_MODEL")),
            key_env: nonempty(env("ONTOMARK_JEV_KEY_ENV")),
            local: flag(env("ONTOMARK_JEV_LOCAL")),
            allow_cloud,
            local_only: !allow_cloud,
            ..Self::default()
        }
    }

    /// The local-only gate for a caller that already holds an explicit
    /// `endpoint` (an ingest host, say), without Jev's `enabled` switch.
    ///
    /// The endpoint is allowed when the consumer is not `local_only`
    /// (`ONTOMARK_JEV_ALLOW_CLOUD=1`), the environment declares it local
    /// (`ONTOMARK_JEV_LOCAL=1` or `AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL=1`), or its
    /// host is evidently on this machine or LAN: a loopback, private or
    /// link-local IP literal, `localhost`, a `.local` name or a single-label
    /// name such as a container alias. No DNS lookup is made, so a name that
    /// resolves elsewhere is the operator's declaration to make.
    ///
    /// # Errors
    /// [`Unavailable`] with `bad-endpoint` for a URL that is not `http(s)` or
    /// embeds credentials, and `not-local` for any other endpoint.
    pub fn allows_endpoint(
        &self,
        endpoint: &str,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<(), Unavailable> {
        let host = endpoint_host(endpoint).ok_or_else(|| Unavailable::new("bad-endpoint"))?;
        if self.allow_cloud
            || !self.local_only
            || self.local
            || flag(env("AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL"))
            || host_is_local(&host)
        {
            Ok(())
        } else {
            Err(Unavailable::new("not-local"))
        }
    }

    /// Resolve the destination, first match wins:
    ///
    /// 1. the explicit `endpoint` (key from `key_env`, if set and present);
    /// 2. `AGENTBOX_SYSTEM_ONE_ENDPOINT` + `AGENTBOX_SYSTEM_ONE_MODEL` when
    ///    `AGENTBOX_SYSTEM_ONE_PROXY=1`, with no Authorization header;
    /// 3. [`CLOUD_ENDPOINT`] with `TYPESAFE_API_KEY`, when that is set.
    ///
    /// Steps 2 and 3 are refused for a `local_only` consumer unless declared
    /// local (`local`, or `AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL=1`). The cloud
    /// endpoint can never be local, so a vault with `local_only` reaches it
    /// only through an explicit step-1 endpoint the user typed.
    ///
    /// # Errors
    /// [`Unavailable`] with `flag-off` or `no-endpoint`.
    pub fn resolve(&self, env: &dyn Fn(&str) -> Option<String>) -> Result<Resolved, Unavailable> {
        if !self.enabled {
            return Err(Unavailable::new("flag-off"));
        }
        if let Some(url) = nonempty(self.endpoint.clone()) {
            let bearer = self.key_env.as_deref().and_then(|name| nonempty(env(name)));
            return Ok(Resolved {
                url,
                model: self.model.clone().unwrap_or_else(|| DEFAULT_MODEL.into()),
                bearer,
            });
        }
        let declared_local = self.local || flag(env("AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL"));
        let may_use_shared = !self.local_only || declared_local;
        if may_use_shared && flag(env("AGENTBOX_SYSTEM_ONE_PROXY")) {
            if let (Some(url), Some(model)) = (
                nonempty(env("AGENTBOX_SYSTEM_ONE_ENDPOINT")),
                nonempty(env("AGENTBOX_SYSTEM_ONE_MODEL")),
            ) {
                return Ok(Resolved {
                    url,
                    model,
                    bearer: None,
                });
            }
        }
        if may_use_shared {
            if let Some(key) = nonempty(env("TYPESAFE_API_KEY")) {
                return Ok(Resolved {
                    url: CLOUD_ENDPOINT.into(),
                    model: self.model.clone().unwrap_or_else(|| DEFAULT_MODEL.into()),
                    bearer: Some(key),
                });
            }
        }
        Err(Unavailable::new("no-endpoint"))
    }
}

/// The host of an `http(s)` URL, lower-cased and without brackets or port;
/// `None` for another scheme, embedded credentials or no host.
fn endpoint_host(endpoint: &str) -> Option<String> {
    let rest = endpoint
        .strip_prefix("http://")
        .or_else(|| endpoint.strip_prefix("https://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.contains('@') {
        return None;
    }
    let host = if let Some(v6) = authority.strip_prefix('[') {
        v6.split(']').next()?
    } else {
        authority.split(':').next()?
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Whether `host` is evidently local: see [`JevConfig::allows_endpoint`].
fn host_is_local(host: &str) -> bool {
    use std::net::IpAddr;
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => {
            let first = ip.segments()[0];
            ip.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
        Err(_) => {
            host == "localhost"
                || host.ends_with(".localhost")
                || host.ends_with(".local")
                || !host.contains('.')
        }
    }
}

#[cfg(test)]
mod endpoint_tests {
    use super::*;

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn local_hosts_pass_and_cloud_needs_the_opt_in() {
        let c = JevConfig::default();
        for ok in [
            "http://127.0.0.1:8080/v1",
            "http://192.168.2.132:8084/v1",
            "http://10.0.0.5/v1",
            "http://[::1]:9/v1",
            "http://localhost:1/v1",
            "http://loom:8080/v1",
            "https://box.local/v1",
        ] {
            assert!(c.allows_endpoint(ok, &none).is_ok(), "{ok}");
        }
        for no in [
            "https://api.example.com/v1",
            "http://8.8.8.8/v1",
            "http://172.32.0.1/v1",
        ] {
            assert_eq!(
                c.allows_endpoint(no, &none).unwrap_err().reason,
                "not-local"
            );
        }
        for bad in ["ftp://x/v1", "http://u:p@host/v1", "http:///v1"] {
            assert_eq!(
                c.allows_endpoint(bad, &none).unwrap_err().reason,
                "bad-endpoint"
            );
        }
    }

    #[test]
    fn the_opt_in_and_the_declarations_lift_the_refusal() {
        let cloud = "https://api.example.com/v1";
        let allow = JevConfig::from_env(&|k| (k == "ONTOMARK_JEV_ALLOW_CLOUD").then(|| "1".into()));
        assert!(allow.allows_endpoint(cloud, &none).is_ok());
        let declared = JevConfig::from_env(&|k| (k == "ONTOMARK_JEV_LOCAL").then(|| "1".into()));
        assert!(declared.allows_endpoint(cloud, &none).is_ok());
        let backend = |k: &str| (k == "AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL").then(|| "1".to_owned());
        assert!(JevConfig::default()
            .allows_endpoint(cloud, &backend)
            .is_ok());
    }
}
