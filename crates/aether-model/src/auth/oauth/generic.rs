//! Configuration-driven OAuth: PKCE authorization code with a local
//! callback (or pasted code), RFC 8628 device code, and refresh. Provider
//! flows are values of [`OAuthFlowConfig`], so a new provider is data.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::callback::{CallbackServer, parse_manual_input};
use super::device_code::{DevicePoll, DevicePollOptions};
use super::pkce;
use crate::auth::interaction::{AuthEvent, AuthInteraction, AuthPrompt};
use crate::auth::types::{AuthError, Credential};
use crate::http::SHORT_TIMEOUT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenContentType {
    #[default]
    Form,
    Json,
}

/// RFC 8628 endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceEndpoints {
    pub device_authorization_url: String,
    /// Token endpoint to poll; defaults to the flow's `token_url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthFlowConfig {
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    pub authorize_url: String,
    pub token_url: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Fixed port when the provider registered an exact redirect; 0 for any free port.
    #[serde(default)]
    pub redirect_port: u16,
    #[serde(default = "default_redirect_path")]
    pub redirect_path: String,
    #[serde(default)]
    pub extra_authorize_params: BTreeMap<String, String>,
    #[serde(default)]
    pub extra_token_params: BTreeMap<String, String>,
    #[serde(default)]
    pub token_content_type: TokenContentType,
    /// Some servers require `state` echoed in the code exchange.
    #[serde(default)]
    pub send_state_in_exchange: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<DeviceEndpoints>,
}

impl std::fmt::Debug for OAuthFlowConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthFlowConfig")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "<redacted>"),
            )
            .field("authorize_url", &self.authorize_url)
            .field("token_url", &self.token_url)
            .field("scopes", &self.scopes)
            .field("redirect_port", &self.redirect_port)
            .field("redirect_path", &self.redirect_path)
            .field(
                "extra_authorize_params",
                &self.extra_authorize_params.keys().collect::<Vec<_>>(),
            )
            .field(
                "extra_token_params",
                &self.extra_token_params.keys().collect::<Vec<_>>(),
            )
            .field("token_content_type", &self.token_content_type)
            .field("send_state_in_exchange", &self.send_state_in_exchange)
            .field("device", &self.device)
            .finish()
    }
}

fn default_redirect_path() -> String {
    "/callback".into()
}

/// Tokens as returned by a token endpoint.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct OAuthTokens {
    pub access: String,
    pub refresh: Option<String>,
    pub expires_in: Option<u64>,
    /// Other string fields from the response (`id_token`, `scope`, ...).
    pub extra: BTreeMap<String, String>,
}

impl std::fmt::Debug for OAuthTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthTokens")
            .field("expires_in", &self.expires_in)
            .finish_non_exhaustive()
    }
}

impl OAuthTokens {
    /// Convert to a stored credential, expiring five minutes early so a
    /// token is never used at the edge of its validity.
    pub fn into_credential(self, extra: BTreeMap<String, String>) -> Result<Credential, AuthError> {
        // Bound untrusted lifetimes without discarding a newly rotated refresh token.
        // Ten years exceeds ordinary token lifetimes and fits every supported timestamp.
        let expires_at = self.expires_in.map(|secs| {
            aether_core::time::now()
                + chrono::Duration::seconds(secs.min(10 * 365 * 24 * 60 * 60) as i64)
                - chrono::Duration::minutes(5)
        });
        let mut all = self.extra;
        all.extend(extra);
        Ok(Credential::OAuth {
            access: self.access,
            refresh: self.refresh,
            expires_at,
            extra: all,
        })
    }
}

/// Parse a token endpoint response body.
pub fn parse_token_response(body: &str) -> Result<OAuthTokens, AuthError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|e| AuthError::Flow(format!("token response is not JSON: {e}")))?;
    if let Some(err) = value.get("error") {
        let desc = value
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or("");
        return Err(AuthError::Flow(format!(
            "token endpoint error: {err} {desc}"
        )));
    }
    let access = value
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| AuthError::Flow("token response has no access_token".into()))?
        .to_owned();
    let refresh = value
        .get("refresh_token")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let expires_in = match value.get("expires_in") {
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    };
    let extra = value
        .as_object()
        .map(|o| {
            o.iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "access_token" | "refresh_token" | "expires_in" | "token_type"
                    )
                })
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    Ok(OAuthTokens {
        access,
        refresh,
        expires_in,
        extra,
    })
}

/// Encode parameters as `application/x-www-form-urlencoded`.
pub fn form_encode(params: &BTreeMap<String, String>) -> String {
    let mut ser = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in params {
        ser.append_pair(k, v);
    }
    ser.finish()
}

/// Attach a body in the requested encoding.
pub fn with_body(
    req: reqwest::RequestBuilder,
    content_type: TokenContentType,
    params: &BTreeMap<String, String>,
) -> reqwest::RequestBuilder {
    match content_type {
        TokenContentType::Form => req
            .header("content-type", "application/x-www-form-urlencoded")
            .body(form_encode(params)),
        TokenContentType::Json => req.json(params),
    }
}

async fn post_token(
    http: &reqwest::Client,
    url: &str,
    content_type: TokenContentType,
    params: &BTreeMap<String, String>,
    cancel: &CancellationToken,
) -> Result<OAuthTokens, AuthError> {
    let req = http
        .post(url)
        .timeout(SHORT_TIMEOUT)
        .header("accept", "application/json");
    let req = with_body(req, content_type, params);
    let resp = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(AuthError::Cancelled),
        resp = req.send() => resp
            .map_err(|e| AuthError::Flow(format!("token request to {url} failed: {e}")))?,
    };
    let status = resp.status();
    let body = read_token_body(resp, cancel).await?;
    if !status.is_success() {
        return Err(AuthError::Flow(format!(
            "token endpoint {url} returned {status}: {}",
            crate::http::truncate(&body, 500)
        )));
    }
    parse_token_response(&body)
}

pub async fn exchange_code(
    cfg: &OAuthFlowConfig,
    http: &reqwest::Client,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    state: Option<&str>,
) -> Result<OAuthTokens, AuthError> {
    exchange_code_cancellable(
        cfg,
        http,
        code,
        verifier,
        redirect_uri,
        state,
        &CancellationToken::new(),
    )
    .await
}

pub(super) async fn exchange_code_cancellable(
    cfg: &OAuthFlowConfig,
    http: &reqwest::Client,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    state: Option<&str>,
    cancel: &CancellationToken,
) -> Result<OAuthTokens, AuthError> {
    let mut params = cfg.extra_token_params.clone();
    params.insert("grant_type".into(), "authorization_code".into());
    params.insert("client_id".into(), cfg.client_id.clone());
    params.insert("code".into(), code.to_owned());
    params.insert("code_verifier".into(), verifier.to_owned());
    params.insert("redirect_uri".into(), redirect_uri.to_owned());
    if let Some(secret) = &cfg.client_secret {
        params.insert("client_secret".into(), secret.clone());
    }
    if cfg.send_state_in_exchange
        && let Some(s) = state
    {
        params.insert("state".into(), s.to_owned());
    }
    post_token(
        http,
        &cfg.token_url,
        cfg.token_content_type,
        &params,
        cancel,
    )
    .await
}

pub async fn refresh(
    cfg: &OAuthFlowConfig,
    http: &reqwest::Client,
    refresh_token: &str,
) -> Result<OAuthTokens, AuthError> {
    let mut params = BTreeMap::new();
    params.insert("grant_type".into(), "refresh_token".into());
    params.insert("client_id".into(), cfg.client_id.clone());
    params.insert("refresh_token".into(), refresh_token.to_owned());
    if let Some(secret) = &cfg.client_secret {
        params.insert("client_secret".into(), secret.clone());
    }
    let mut tokens = post_token(
        http,
        &cfg.token_url,
        cfg.token_content_type,
        &params,
        &CancellationToken::new(),
    )
    .await?;
    // Servers that do not rotate refresh tokens omit it; keep the old one.
    if tokens.refresh.is_none() {
        tokens.refresh = Some(refresh_token.to_owned());
    }
    Ok(tokens)
}

pub fn authorize_url(
    cfg: &OAuthFlowConfig,
    challenge: &str,
    state: &str,
    redirect_uri: &str,
) -> Result<String, AuthError> {
    let mut url = url::Url::parse(&cfg.authorize_url)
        .map_err(|e| AuthError::Flow(format!("authorize_url: {e}")))?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("client_id", &cfg.client_id);
        q.append_pair("redirect_uri", redirect_uri);
        if !cfg.scopes.is_empty() {
            q.append_pair("scope", &cfg.scopes.join(" "));
        }
        q.append_pair("code_challenge", challenge);
        q.append_pair("code_challenge_method", "S256");
        q.append_pair("state", state);
        for (k, v) in &cfg.extra_authorize_params {
            q.append_pair(k, v);
        }
    }
    Ok(url.into())
}

/// Browser-based PKCE login. Races the local callback against a pasted code.
pub async fn login_browser(
    cfg: &OAuthFlowConfig,
    http: &reqwest::Client,
    interaction: &dyn AuthInteraction,
    cancel: &CancellationToken,
) -> Result<OAuthTokens, AuthError> {
    let pkce = pkce::generate();
    let state = pkce::random_state();
    let server = CallbackServer::bind(cfg.redirect_port, &cfg.redirect_path).await?;
    let redirect_uri = server.redirect_uri();
    let url = authorize_url(cfg, &pkce.challenge, &state, &redirect_uri)?;
    interaction.notify(AuthEvent::AuthUrl {
        url,
        instructions: "Complete the login in your browser. If the browser is on another machine, paste the final redirect URL below.".into(),
    });

    let manual = interaction.prompt(AuthPrompt::ManualCode {
        message: "Waiting for the browser; or paste the authorization code / redirect URL".into(),
        placeholder: Some(redirect_uri.clone()),
    });
    let result = tokio::select! {
        r = server.wait_for_code(Some(&state), cancel) => r?,
        m = manual => {
            let parsed = parse_manual_input(&m?);
            if let Some(s) = &parsed.state && s != &state {
                return Err(AuthError::Flow("OAuth state mismatch in pasted input".into()));
            }
            parsed
        }
        _ = cancel.cancelled() => return Err(AuthError::Cancelled),
    };
    if result.code.is_empty() {
        return Err(AuthError::Flow("no authorization code received".into()));
    }
    interaction.notify(AuthEvent::Progress {
        message: "Exchanging the authorization code for tokens…".into(),
    });
    exchange_code_cancellable(
        cfg,
        http,
        &result.code,
        &pkce.verifier,
        &redirect_uri,
        Some(&state),
        cancel,
    )
    .await
}

/// RFC 8628 device-code login.
pub async fn login_device(
    cfg: &OAuthFlowConfig,
    http: &reqwest::Client,
    interaction: &dyn AuthInteraction,
    cancel: &CancellationToken,
) -> Result<OAuthTokens, AuthError> {
    let device = cfg
        .device
        .as_ref()
        .ok_or_else(|| AuthError::Flow("this provider has no device-code endpoint".into()))?;
    let mut params = BTreeMap::new();
    params.insert("client_id".to_owned(), cfg.client_id.clone());
    if !cfg.scopes.is_empty() {
        params.insert("scope".to_owned(), cfg.scopes.join(" "));
    }
    let resp = with_body(
        http.post(&device.device_authorization_url)
            .timeout(SHORT_TIMEOUT)
            .header("accept", "application/json"),
        TokenContentType::Form,
        &params,
    )
    .send()
    .await
    .map_err(|e| AuthError::Flow(format!("device authorization request failed: {e}")))?;
    let status = resp.status();
    let body = read_token_body(resp, cancel).await?;
    if !status.is_success() {
        return Err(AuthError::Flow(format!(
            "device authorization returned {status}: {}",
            crate::http::truncate(&body, 500)
        )));
    }
    let v: Value = serde_json::from_str(&body)
        .map_err(|e| AuthError::Flow(format!("device response: {e}")))?;
    let device_code = v["device_code"]
        .as_str()
        .ok_or_else(|| AuthError::Flow("device response has no device_code".into()))?
        .to_owned();
    let user_code = v["user_code"].as_str().unwrap_or("").to_owned();
    let verification_uri = v
        .get("verification_uri_complete")
        .or_else(|| v.get("verification_uri"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let interval_secs = v["interval"].as_u64().unwrap_or(5);
    let expires_in_secs = v["expires_in"].as_u64().unwrap_or(900);
    interaction.notify(AuthEvent::DeviceCode {
        user_code,
        verification_uri,
        interval_secs,
        expires_in_secs,
    });

    let token_url = device
        .token_url
        .clone()
        .unwrap_or_else(|| cfg.token_url.clone());
    let mut poll_params = BTreeMap::new();
    poll_params.insert(
        "grant_type".to_owned(),
        "urn:ietf:params:oauth:grant-type:device_code".to_owned(),
    );
    poll_params.insert("client_id".to_owned(), cfg.client_id.clone());
    poll_params.insert("device_code".to_owned(), device_code);
    let content_type = cfg.token_content_type;

    super::device_code::poll(
        DevicePollOptions {
            interval: Duration::from_secs(interval_secs),
            expires_in: Duration::from_secs(expires_in_secs),
            wait_before_first: true,
        },
        cancel,
        || {
            let http = http.clone();
            let url = token_url.clone();
            let params = poll_params.clone();
            async move {
                let req = http
                    .post(&url)
                    .timeout(SHORT_TIMEOUT)
                    .header("accept", "application/json");
                let req = with_body(req, content_type, &params);
                let resp = req
                    .send()
                    .await
                    .map_err(|e| AuthError::Flow(format!("device poll failed: {e}")))?;
                let body = read_token_body(resp, cancel).await?;
                let v: Value = match serde_json::from_str(&body) {
                    Ok(v) => v,
                    Err(_) => {
                        return Ok(DevicePoll::Failed(format!(
                            "device poll returned non-JSON: {}",
                            crate::http::truncate(&body, 200)
                        )));
                    }
                };
                match v.get("error").and_then(Value::as_str) {
                    None => Ok(DevicePoll::Complete(parse_token_response(&body)?)),
                    Some("authorization_pending") => Ok(DevicePoll::Pending),
                    Some("slow_down") => Ok(DevicePoll::SlowDown {
                        interval: v
                            .get("interval")
                            .and_then(Value::as_u64)
                            .map(Duration::from_secs),
                    }),
                    Some(other) => Ok(DevicePoll::Failed(format!(
                        "device login failed: {other} {}",
                        v.get("error_description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                    ))),
                }
            }
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> OAuthFlowConfig {
        OAuthFlowConfig {
            client_id: "cid".into(),
            client_secret: None,
            authorize_url: "https://example.com/oauth/authorize".into(),
            token_url: "https://example.com/oauth/token".into(),
            scopes: vec!["openid".into(), "email".into()],
            redirect_port: 0,
            redirect_path: "/callback".into(),
            extra_authorize_params: BTreeMap::from([("originator".into(), "aether".into())]),
            extra_token_params: BTreeMap::new(),
            token_content_type: TokenContentType::Form,
            send_state_in_exchange: false,
            device: None,
        }
    }

    #[test]
    fn authorize_url_has_pkce_and_extras() {
        let url = authorize_url(&cfg(), "chal", "st", "http://localhost:1/cb").expect("url");
        let parsed = url::Url::parse(&url).expect("parse");
        let q: BTreeMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["client_id"], "cid");
        assert_eq!(q["scope"], "openid email");
        assert_eq!(q["code_challenge"], "chal");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "st");
        assert_eq!(q["originator"], "aether");
        assert_eq!(q["redirect_uri"], "http://localhost:1/cb");
    }

    #[test]
    fn token_response_parsing() {
        let t = parse_token_response(r#"{"access_token":"a","refresh_token":"r","expires_in":"3600","token_type":"bearer","id_token":"jwt","scope":"x"}"#)
            .expect("parse");
        assert_eq!(t.access, "a");
        assert_eq!(t.refresh.as_deref(), Some("r"));
        assert_eq!(t.expires_in, Some(3600));
        assert_eq!(t.extra.get("id_token").map(String::as_str), Some("jwt"));
        assert!(!t.extra.contains_key("token_type"));

        let err =
            parse_token_response(r#"{"error":"invalid_grant","error_description":"expired"}"#)
                .unwrap_err();
        assert!(matches!(err, AuthError::Flow(m) if m.contains("invalid_grant")));
        assert!(parse_token_response("{}").is_err());
    }

    #[test]
    fn credential_expiry_is_conservative() {
        let t = OAuthTokens {
            access: "a".into(),
            refresh: None,
            expires_in: Some(3600),
            extra: BTreeMap::new(),
        };
        let now = aether_core::time::now();
        match t.into_credential(BTreeMap::new()).expect("valid expiry") {
            Credential::OAuth {
                expires_at: Some(at),
                ..
            } => {
                let delta = at - now;
                assert!(
                    delta > chrono::Duration::minutes(54) && delta < chrono::Duration::minutes(56)
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn enormous_expiry_is_clamped_without_discarding_rotated_refresh_tokens() {
        for seconds in [u64::MAX, i64::MAX as u64, 9_000_000_000_000] {
            let body = format!(
                r#"{{"access_token":"a","refresh_token":"rotated","expires_in":{seconds}}}"#
            );
            let tokens = parse_token_response(&body).expect("valid JSON");
            let credential = tokens.into_credential(BTreeMap::new()).unwrap();
            let Credential::OAuth {
                refresh,
                expires_at,
                ..
            } = credential
            else {
                panic!("OAuth credential")
            };
            assert_eq!(refresh.as_deref(), Some("rotated"));
            let remaining = expires_at.unwrap() - aether_core::time::now();
            assert!(remaining <= chrono::Duration::days(3650));
            assert!(remaining >= chrono::Duration::days(3649));
        }
        for seconds in [0, 1, 299, 300, 3600] {
            let tokens = OAuthTokens {
                expires_in: Some(seconds),
                ..Default::default()
            };
            assert!(tokens.into_credential(BTreeMap::new()).is_ok());
        }
        assert!(
            OAuthTokens::default()
                .into_credential(BTreeMap::new())
                .unwrap()
                .expires_at()
                .is_none()
        );
    }

    #[test]
    fn flow_debug_redacts_secrets_through_auth_methods() {
        let mut flow = cfg();
        flow.client_secret = Some("private-client-secret".into());
        flow.extra_token_params
            .insert("secret".into(), "private-param".into());
        let methods = crate::auth::methods::AuthMethods::default()
            .with_oauth(crate::auth::methods::OAuthKind::Generic(Box::new(flow)));
        let debug = format!("{methods:?}");
        assert!(!debug.contains("private-client-secret"));
        assert!(!debug.contains("private-param"));
        assert!(debug.contains("<redacted>"));
    }
}

/// Token endpoints share the bounded HTTP reader; cancellation and read failures
/// are surfaced instead of treating a missing response as empty credentials.
pub(super) async fn read_token_body(
    resp: reqwest::Response,
    cancel: &CancellationToken,
) -> Result<String, AuthError> {
    crate::http::read_response_body(resp, cancel, 1024 * 1024)
        .await
        .map_err(|error| {
            if error.class == aether_core::model::ModelErrorClass::Cancelled {
                AuthError::Cancelled
            } else {
                AuthError::Flow(format!("reading token response: {error}"))
            }
        })
}

#[cfg(test)]
mod response_body_tests {
    use super::*;
    use futures::{StreamExt, stream};
    fn response(body: reqwest::Body) -> reqwest::Response {
        ::http::Response::builder().body(body).unwrap().into()
    }
    #[tokio::test]
    async fn token_body_is_bounded_without_waiting_for_eof() {
        let chunks = stream::once(async {
            Ok::<_, std::io::Error>(bytes::Bytes::from(vec![b'x'; 2 * 1024 * 1024]))
        })
        .chain(stream::pending());
        let error = tokio::time::timeout(
            Duration::from_secs(1),
            read_token_body(
                response(reqwest::Body::wrap_stream(chunks)),
                &CancellationToken::new(),
            ),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(
            error.to_string().contains("exceeds 1048576 byte limit"),
            "{error}"
        );
    }
    #[tokio::test]
    async fn token_body_remains_cancellable_after_headers() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let body =
            reqwest::Body::wrap_stream(stream::pending::<Result<bytes::Bytes, std::io::Error>>());
        assert!(matches!(
            read_token_body(response(body), &cancel).await,
            Err(AuthError::Cancelled)
        ));
    }
}
