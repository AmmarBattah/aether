//! OpenRouter's PKCE flow mints a user-controlled API key; there is nothing
//! to refresh afterwards.

use std::collections::BTreeMap;

use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::callback::{CallbackServer, parse_manual_input};
use super::pkce;
use crate::auth::interaction::{AuthEvent, AuthInteraction, AuthPrompt};
use crate::auth::types::{AuthError, Credential};
use crate::http::SHORT_TIMEOUT;

const AUTH_URL: &str = "https://openrouter.ai/auth";
const KEYS_URL: &str = "https://openrouter.ai/api/v1/auth/keys";
const CALLBACK_PATH: &str = "/callback";

pub async fn login(
    http: &reqwest::Client,
    interaction: &dyn AuthInteraction,
    cancel: &CancellationToken,
) -> Result<Credential, AuthError> {
    let pkce = pkce::generate();
    let server = CallbackServer::bind(0, CALLBACK_PATH).await?;
    let redirect = server.redirect_uri();
    let mut url = url::Url::parse(AUTH_URL).map_err(|e| AuthError::Flow(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("callback_url", &redirect)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256");
    interaction.notify(AuthEvent::AuthUrl {
        url: url.into(),
        instructions:
            "Authorize Aether on OpenRouter in your browser, or paste the redirect URL below."
                .into(),
    });
    let manual = interaction.prompt(AuthPrompt::ManualCode {
        message: "Waiting for the browser; or paste the code / redirect URL".into(),
        placeholder: Some(redirect.clone()),
    });
    let code = tokio::select! {
        r = server.wait_for_code(None, cancel) => r?.code,
        m = manual => parse_manual_input(&m?).code,
        _ = cancel.cancelled() => return Err(AuthError::Cancelled),
    };
    interaction.notify(AuthEvent::Progress {
        message: "Exchanging the code for an API key…".into(),
    });
    let resp = http
        .post(KEYS_URL)
        .timeout(SHORT_TIMEOUT)
        .json(&serde_json::json!({
            "code": code,
            "code_verifier": pkce.verifier,
            "code_challenge_method": "S256",
        }))
        .send()
        .await
        .map_err(|e| AuthError::Flow(format!("key exchange failed: {e}")))?;
    let status = resp.status();
    let body = super::generic::read_token_body(resp, cancel).await?;
    if !status.is_success() {
        return Err(AuthError::Flow(format!(
            "OpenRouter key exchange returned {status}: {}",
            crate::http::truncate(&body, 300)
        )));
    }
    let v: Value = serde_json::from_str(&body)
        .map_err(|e| AuthError::Flow(format!("key exchange response: {e}")))?;
    let key = v["key"]
        .as_str()
        .ok_or_else(|| AuthError::Flow("key exchange response has no key".into()))?;
    Ok(Credential::ApiKey {
        key: key.to_owned(),
        env: BTreeMap::new(),
    })
}
