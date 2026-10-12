//! Credential vocabulary and the store contract.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use aether_core::model::{ModelError, ModelErrorClass};
use aether_core::time::Timestamp;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("provider `{provider}` is not configured: {hint}")]
    NotConfigured { provider: String, hint: String },
    #[error("credential store: {0}")]
    Store(String),
    #[error("token refresh failed for `{provider}`: {reason}")]
    Refresh { provider: String, reason: String },
    #[error("login failed: {0}")]
    Flow(String),
    #[error("credential helper `{command}` failed: {reason}")]
    Helper { command: String, reason: String },
    #[error("login cancelled")]
    Cancelled,
    #[error("unknown provider `{0}`")]
    UnknownProvider(String),
    #[error("provider `{provider}` does not support {what}")]
    Unsupported { provider: String, what: String },
}

impl From<AuthError> for ModelError {
    fn from(err: AuthError) -> Self {
        let class = match err {
            AuthError::Cancelled => ModelErrorClass::Cancelled,
            _ => ModelErrorClass::Auth,
        };
        ModelError::new(class, err.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    ApiKey,
    OAuth,
    Bearer,
    Helper,
    Import,
    /// Owned by an official CLI (Claude Code, Codex); inference runs through it.
    Cli,
    None,
}

impl AuthKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::OAuth => "oauth",
            Self::Bearer => "bearer",
            Self::Helper => "helper",
            Self::Import => "import",
            Self::Cli => "cli",
            Self::None => "none",
        }
    }
}

/// A stored secret. `Debug` never prints the secret.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Credential {
    ApiKey {
        key: String,
        /// Provider-scoped settings captured at login (account ids, regions).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        env: BTreeMap<String, String>,
    },
    OAuth {
        access: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refresh: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expires_at: Option<Timestamp>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        extra: BTreeMap<String, String>,
    },
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey { env, .. } => f
                .debug_struct("ApiKey")
                .field("key", &"<redacted>")
                .field("env", env)
                .finish(),
            Self::OAuth {
                expires_at,
                extra,
                refresh,
                ..
            } => f
                .debug_struct("OAuth")
                .field("access", &"<redacted>")
                .field("refresh", &refresh.as_ref().map(|_| "<redacted>"))
                .field("expires_at", expires_at)
                .field("extra", &extra.keys().collect::<Vec<_>>())
                .finish(),
        }
    }
}

impl Credential {
    pub fn kind(&self) -> AuthKind {
        match self {
            Self::ApiKey { .. } => AuthKind::ApiKey,
            Self::OAuth { .. } => AuthKind::OAuth,
        }
    }

    pub fn expires_at(&self) -> Option<Timestamp> {
        match self {
            Self::OAuth { expires_at, .. } => *expires_at,
            Self::ApiKey { .. } => None,
        }
    }

    /// True when the credential expires within `window` of `now` (or has expired).
    pub fn expires_within(&self, window: Duration, now: Timestamp) -> bool {
        let Some(at) = self.expires_at() else {
            return false;
        };
        let window = chrono::Duration::from_std(window).unwrap_or(chrono::Duration::zero());
        now + window >= at
    }
}

/// Non-secret metadata for status displays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialInfo {
    pub provider: String,
    pub kind: AuthKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
}

/// The read-modify-write callback. Returning `Ok(None)` leaves the entry unchanged.
pub type ModifyFn = Box<
    dyn FnOnce(Option<Credential>) -> BoxFuture<'static, Result<Option<Credential>, AuthError>>
        + Send,
>;

/// One credential per provider. `modify` is the only write path and is
/// serialized per store, across processes where the backing store allows,
/// so a token refresh can never race another refresh.
#[async_trait::async_trait]
pub trait CredentialStore: Send + Sync {
    async fn read(&self, provider: &str) -> Result<Option<Credential>, AuthError>;
    async fn list(&self) -> Result<Vec<CredentialInfo>, AuthError>;
    async fn modify(&self, provider: &str, f: ModifyFn) -> Result<Option<Credential>, AuthError>;
    async fn delete(&self, provider: &str) -> Result<(), AuthError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_secrets() {
        let c = Credential::ApiKey {
            key: "sk-secret".into(),
            env: BTreeMap::new(),
        };
        assert!(!format!("{c:?}").contains("sk-secret"));
        let o = Credential::OAuth {
            access: "tok".into(),
            refresh: Some("ref".into()),
            expires_at: None,
            extra: BTreeMap::new(),
        };
        let dbg = format!("{o:?}");
        assert!(!dbg.contains("tok") && !dbg.contains("ref\""));
    }

    #[test]
    fn expiry_window() {
        let now = aether_core::time::now();
        let soon = Credential::OAuth {
            access: "a".into(),
            refresh: None,
            expires_at: Some(now + chrono::Duration::minutes(2)),
            extra: BTreeMap::new(),
        };
        assert!(soon.expires_within(Duration::from_secs(300), now));
        assert!(!soon.expires_within(Duration::from_secs(60), now));
        let key = Credential::ApiKey {
            key: "k".into(),
            env: BTreeMap::new(),
        };
        assert!(!key.expires_within(Duration::from_secs(300), now));
    }

    #[test]
    fn oauth_debug_redacts_all_token_response_extra_values() {
        let tokens = crate::auth::oauth::generic::parse_token_response(
            r#"{"access_token":"access-secret","refresh_token":"refresh-secret","id_token":"id-secret","custom":"custom-secret"}"#,
        ).unwrap();
        let credential = tokens.into_credential(BTreeMap::new()).unwrap();
        let debug = format!("{credential:?}");
        for secret in [
            "access-secret",
            "refresh-secret",
            "id-secret",
            "custom-secret",
        ] {
            assert!(!debug.contains(secret));
        }
        assert!(debug.contains("id_token"));
    }
}
