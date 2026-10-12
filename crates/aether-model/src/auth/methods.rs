//! What a provider's auth looks like, as data, plus the flows behind it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::helper::CredentialHelper;
use super::import::CredentialImport;
use super::interaction::AuthInteraction;
use super::oauth::{self, OAuthFlowConfig};
use super::types::{AuthError, Credential};

/// Interactive OAuth flows a provider may offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "flow", rename_all = "kebab-case")]
pub enum OAuthKind {
    /// ChatGPT subscription via the open Codex CLI flow.
    OpenaiCodex,
    /// Mints an OpenRouter API key through PKCE.
    Openrouter,
    /// Any standard PKCE / device-code server, configured in TOML.
    Generic(Box<OAuthFlowConfig>),
}

impl OAuthKind {
    pub fn label(&self) -> String {
        match self {
            Self::OpenaiCodex => "ChatGPT subscription (Codex)".into(),
            Self::Openrouter => "OpenRouter OAuth".into(),
            Self::Generic(cfg) => format!("OAuth ({})", cfg.authorize_url),
        }
    }

    pub fn is_subscription(&self) -> bool {
        matches!(self, Self::OpenaiCodex)
    }

    pub async fn login(
        &self,
        http: &reqwest::Client,
        interaction: &dyn AuthInteraction,
        cancel: &CancellationToken,
        method: Option<&str>,
    ) -> Result<Credential, AuthError> {
        match self {
            Self::OpenaiCodex => oauth::codex::login(http, interaction, cancel, method).await,
            Self::Openrouter => oauth::openrouter::login(http, interaction, cancel).await,
            Self::Generic(cfg) => {
                let tokens = match method {
                    Some("device") | Some("device_code") => {
                        oauth::generic::login_device(cfg, http, interaction, cancel).await?
                    }
                    _ => oauth::generic::login_browser(cfg, http, interaction, cancel).await?,
                };
                tokens.into_credential(BTreeMap::new())
            }
        }
    }

    pub async fn refresh(
        &self,
        http: &reqwest::Client,
        credential: &Credential,
    ) -> Result<Credential, AuthError> {
        let Credential::OAuth {
            refresh: Some(refresh),
            ..
        } = credential
        else {
            return Err(AuthError::Flow(
                "credential has no refresh token; sign in again".into(),
            ));
        };
        match self {
            Self::OpenaiCodex => oauth::codex::refresh(http, refresh).await,
            Self::Openrouter => Err(AuthError::Flow("OpenRouter keys do not refresh".into())),
            Self::Generic(cfg) => {
                let tokens = oauth::generic::refresh(cfg, http, refresh).await?;
                let extra = match credential {
                    Credential::OAuth { extra, .. } => extra.clone(),
                    Credential::ApiKey { .. } => BTreeMap::new(),
                };
                tokens.into_credential(extra)
            }
        }
    }

    /// Extra request headers implied by a stored credential.
    pub fn headers(&self, credential: &Credential) -> BTreeMap<String, String> {
        match self {
            Self::OpenaiCodex => oauth::codex::headers(credential),
            Self::Openrouter | Self::Generic(_) => BTreeMap::new(),
        }
    }
}

/// Every way a provider can be authenticated, in resolution order after a
/// stored credential: helper, import, bearer env, API-key env, or none.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthMethods {
    /// Environment variables holding an API key, first match wins.
    pub api_key_env: Vec<String>,
    /// Environment variables holding a bearer token.
    pub bearer_env: Vec<String>,
    pub oauth: Option<OAuthKind>,
    pub helper: Option<CredentialHelper>,
    pub import: Option<CredentialImport>,
    /// An official CLI owns the login; see [`crate::auth::cli`].
    pub cli: Option<crate::auth::cli::CliAuth>,
    /// The provider needs no credentials (local servers).
    pub none: bool,
}

impl AuthMethods {
    pub fn api_key(envs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            api_key_env: envs.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    pub fn none() -> Self {
        Self {
            none: true,
            ..Default::default()
        }
    }

    pub fn with_oauth(mut self, kind: OAuthKind) -> Self {
        self.oauth = Some(kind);
        self
    }

    pub fn with_bearer_env(mut self, envs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.bearer_env = envs.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_import(mut self, import: CredentialImport) -> Self {
        self.import = Some(import);
        self
    }

    /// Whether an interactive API-key login makes sense.
    pub fn accepts_api_key(&self) -> bool {
        !self.none && self.cli.is_none()
    }

    pub fn cli(cli: crate::auth::cli::CliAuth) -> Self {
        Self {
            cli: Some(cli),
            ..Default::default()
        }
    }

    /// Human-readable hint for "not configured" errors.
    pub fn hint(&self, provider: &str) -> String {
        let mut parts = Vec::new();
        if !self.api_key_env.is_empty() {
            parts.push(format!("set {}", self.api_key_env.join(" or ")));
        }
        if !self.bearer_env.is_empty() {
            parts.push(format!("set {}", self.bearer_env.join(" or ")));
        }
        if !self.none {
            parts.push(format!("run `aether login {provider}`"));
        }
        if parts.is_empty() {
            "no auth method available".into()
        } else {
            parts.join(", or ")
        }
    }
}
