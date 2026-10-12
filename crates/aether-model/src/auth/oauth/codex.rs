//! OpenAI Codex sign-in with a ChatGPT subscription, following the flow the
//! open-source Codex CLI uses. Aether identifies itself as the originator.

use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::device_code::{DevicePoll, DevicePollOptions};
use super::generic::{self, OAuthFlowConfig, TokenContentType};
use crate::auth::interaction::{AuthEvent, AuthInteraction, AuthPrompt, SelectOption};
use crate::auth::types::{AuthError, Credential};
use crate::http::SHORT_TIMEOUT;

pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTH_BASE: &str = "https://auth.openai.com";
const REDIRECT_PORT: u16 = 1455;
const REDIRECT_PATH: &str = "/auth/callback";
const DEVICE_USER_CODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
const DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
const DEVICE_VERIFICATION_URI: &str = "https://auth.openai.com/codex/device";
const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";
const DEVICE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// JWT claim namespace that carries the ChatGPT account id.
const JWT_CLAIM: &str = "https://api.openai.com/auth";
/// Header the Codex backend expects alongside the bearer token.
pub const ACCOUNT_HEADER: &str = "chatgpt-account-id";
pub const ACCOUNT_EXTRA_KEY: &str = "chatgpt_account_id";

pub fn flow_config() -> OAuthFlowConfig {
    OAuthFlowConfig {
        client_id: CLIENT_ID.into(),
        client_secret: None,
        authorize_url: format!("{AUTH_BASE}/oauth/authorize"),
        token_url: format!("{AUTH_BASE}/oauth/token"),
        scopes: vec![
            "openid".into(),
            "profile".into(),
            "email".into(),
            "offline_access".into(),
        ],
        redirect_port: REDIRECT_PORT,
        redirect_path: REDIRECT_PATH.into(),
        extra_authorize_params: BTreeMap::from([
            ("id_token_add_organizations".into(), "true".into()),
            ("codex_cli_simplified_flow".into(), "true".into()),
            ("originator".into(), "aether".into()),
        ]),
        extra_token_params: BTreeMap::new(),
        token_content_type: TokenContentType::Form,
        send_state_in_exchange: false,
        device: None,
    }
}

/// Extract `chatgpt_account_id` from the access token's JWT payload.
pub fn account_id(access_token: &str) -> Option<String> {
    let payload = access_token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get(JWT_CLAIM)?
        .get("chatgpt_account_id")?
        .as_str()
        .map(str::to_owned)
}

fn credential_from(tokens: generic::OAuthTokens) -> Result<Credential, AuthError> {
    let account = account_id(&tokens.access).ok_or_else(|| {
        AuthError::Flow(
            "access token carries no chatgpt_account_id; is this a ChatGPT account?".into(),
        )
    })?;
    tokens.into_credential(BTreeMap::from([(ACCOUNT_EXTRA_KEY.to_owned(), account)]))
}

/// Request headers derived from a stored credential.
pub fn headers(credential: &Credential) -> BTreeMap<String, String> {
    match credential {
        Credential::OAuth { extra, .. } => extra
            .get(ACCOUNT_EXTRA_KEY)
            .map(|id| BTreeMap::from([(ACCOUNT_HEADER.to_owned(), id.clone())]))
            .unwrap_or_default(),
        Credential::ApiKey { .. } => BTreeMap::new(),
    }
}

pub async fn login(
    http: &reqwest::Client,
    interaction: &dyn AuthInteraction,
    cancel: &CancellationToken,
    method: Option<&str>,
) -> Result<Credential, AuthError> {
    let method = match method {
        Some(m) => m.to_owned(),
        None => {
            interaction
                .prompt(AuthPrompt::Select {
                    message: "How do you want to sign in to ChatGPT?".into(),
                    options: vec![
                        SelectOption {
                            id: "browser".into(),
                            label: "Browser login (default)".into(),
                        },
                        SelectOption {
                            id: "device".into(),
                            label: "Device code (headless)".into(),
                        },
                    ],
                })
                .await?
        }
    };
    let tokens = match method.as_str() {
        "browser" => generic::login_browser(&flow_config(), http, interaction, cancel).await?,
        "device" | "device_code" => login_device(http, interaction, cancel).await?,
        other => {
            return Err(AuthError::Flow(format!(
                "unknown Codex login method `{other}`"
            )));
        }
    };
    credential_from(tokens)
}

pub async fn refresh(http: &reqwest::Client, refresh_token: &str) -> Result<Credential, AuthError> {
    credential_from(generic::refresh(&flow_config(), http, refresh_token).await?)
}

/// Codex's device flow is not RFC 8628: it hands back an authorization code
/// and verifier to exchange at the regular token endpoint.
async fn login_device(
    http: &reqwest::Client,
    interaction: &dyn AuthInteraction,
    cancel: &CancellationToken,
) -> Result<generic::OAuthTokens, AuthError> {
    let resp = http
        .post(DEVICE_USER_CODE_URL)
        .timeout(SHORT_TIMEOUT)
        .json(&serde_json::json!({"client_id": CLIENT_ID}))
        .send()
        .await
        .map_err(|e| AuthError::Flow(format!("device code request failed: {e}")))?;
    let status = resp.status();
    let body = generic::read_token_body(resp, cancel).await?;
    if status.as_u16() == 404 {
        return Err(AuthError::Flow(
            "device code login is not enabled; use browser login".into(),
        ));
    }
    if !status.is_success() {
        return Err(AuthError::Flow(format!(
            "device code request returned {status}: {}",
            crate::http::truncate(&body, 300)
        )));
    }
    let v: Value = serde_json::from_str(&body)
        .map_err(|e| AuthError::Flow(format!("device code response: {e}")))?;
    let device_auth_id = v["device_auth_id"]
        .as_str()
        .ok_or_else(|| AuthError::Flow("no device_auth_id".into()))?
        .to_owned();
    let user_code = v["user_code"]
        .as_str()
        .ok_or_else(|| AuthError::Flow("no user_code".into()))?
        .to_owned();
    let interval = match &v["interval"] {
        Value::Number(n) => n.as_u64().unwrap_or(5),
        Value::String(s) => s.trim().parse().unwrap_or(5),
        _ => 5,
    };
    interaction.notify(AuthEvent::DeviceCode {
        user_code: user_code.clone(),
        verification_uri: DEVICE_VERIFICATION_URI.into(),
        interval_secs: interval,
        expires_in_secs: DEVICE_TIMEOUT.as_secs(),
    });

    let (code, verifier) = super::device_code::poll(
        DevicePollOptions {
            interval: Duration::from_secs(interval),
            expires_in: DEVICE_TIMEOUT,
            wait_before_first: false,
        },
        cancel,
        || {
            let http = http.clone();
            let body =
                serde_json::json!({"device_auth_id": device_auth_id, "user_code": user_code});
            async move {
                let resp = http
                    .post(DEVICE_TOKEN_URL)
                    .timeout(SHORT_TIMEOUT)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| AuthError::Flow(format!("device poll failed: {e}")))?;
                let status = resp.status().as_u16();
                let text = generic::read_token_body(resp, cancel).await?;
                if status == 200 {
                    let v: Value = serde_json::from_str(&text)
                        .map_err(|e| AuthError::Flow(format!("device token response: {e}")))?;
                    return match (
                        v["authorization_code"].as_str(),
                        v["code_verifier"].as_str(),
                    ) {
                        (Some(c), Some(ver)) => {
                            Ok(DevicePoll::Complete((c.to_owned(), ver.to_owned())))
                        }
                        _ => Ok(DevicePoll::Failed(format!(
                            "device token response missing fields: {text}"
                        ))),
                    };
                }
                if status == 403 || status == 404 {
                    return Ok(DevicePoll::Pending);
                }
                let code = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v.get("error").cloned())
                    .and_then(|e| {
                        e.as_str()
                            .map(str::to_owned)
                            .or_else(|| e.get("code").and_then(Value::as_str).map(str::to_owned))
                    });
                Ok(match code.as_deref() {
                    Some("deviceauth_authorization_pending") => DevicePoll::Pending,
                    Some("slow_down") => DevicePoll::SlowDown { interval: None },
                    _ => DevicePoll::Failed(format!(
                        "device login failed with status {status}: {}",
                        crate::http::truncate(&text, 300)
                    )),
                })
            }
        },
    )
    .await?;

    generic::exchange_code_cancellable(
        &flow_config(),
        http,
        &code,
        &verifier,
        DEVICE_REDIRECT_URI,
        None,
        cancel,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_account_id_from_jwt() {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD
            .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acct_1"}}"#);
        let token = format!("{header}.{payload}.sig");
        assert_eq!(account_id(&token).as_deref(), Some("acct_1"));
        assert_eq!(account_id("not.a.jwt"), None);
        assert_eq!(account_id("x"), None);
    }

    #[test]
    fn headers_from_credential() {
        let cred = Credential::OAuth {
            access: "a".into(),
            refresh: None,
            expires_at: None,
            extra: BTreeMap::from([(ACCOUNT_EXTRA_KEY.to_owned(), "acct".to_owned())]),
        };
        assert_eq!(
            headers(&cred).get(ACCOUNT_HEADER).map(String::as_str),
            Some("acct")
        );
    }

    #[test]
    fn flow_config_identifies_aether() {
        let cfg = flow_config();
        assert_eq!(
            cfg.extra_authorize_params
                .get("originator")
                .map(String::as_str),
            Some("aether")
        );
        assert_eq!(cfg.redirect_port, 1455);
    }
}
