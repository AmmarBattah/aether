//! Import tokens minted by an official CLI's supported sign-in flow.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helper::{CredentialHelper, HelperCache};
use super::types::AuthError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum CredentialImport {
    /// The `ant auth login` profile under `$ANTHROPIC_CONFIG_DIR`.
    AnthropicProfile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub struct ImportedToken {
    pub token: String,
    pub source: String,
}

impl std::fmt::Debug for ImportedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportedToken")
            .field("token", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

pub async fn import(
    kind: &CredentialImport,
    cache: &HelperCache,
    env: &(dyn Fn(&str) -> Option<String> + Send + Sync),
) -> Result<Option<ImportedToken>, AuthError> {
    match kind {
        CredentialImport::AnthropicProfile { profile } => {
            anthropic_profile(profile.as_deref(), cache, env).await
        }
    }
}

fn anthropic_config_dir(env: &(dyn Fn(&str) -> Option<String> + Send + Sync)) -> Option<PathBuf> {
    if let Some(dir) = env("ANTHROPIC_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    dirs::home_dir().map(|h| h.join(".config").join("anthropic"))
}

/// Prefer the `ant` CLI, which refreshes tokens itself; fall back to reading
/// the profile's credential file, which cannot refresh.
async fn anthropic_profile(
    profile: Option<&str>,
    cache: &HelperCache,
    env: &(dyn Fn(&str) -> Option<String> + Send + Sync),
) -> Result<Option<ImportedToken>, AuthError> {
    let profile_name = profile
        .map(str::to_owned)
        .or_else(|| env("ANTHROPIC_PROFILE"));
    let Some(dir) = anthropic_config_dir(env) else {
        return Ok(None);
    };
    let name = profile_name.clone().unwrap_or_else(|| "default".into());
    let file = dir.join("credentials").join(format!("{name}.json"));
    if !file.exists() {
        return Ok(None);
    }

    if ant_available().await {
        let mut cmd = vec![
            "ant".to_owned(),
            "auth".to_owned(),
            "print-credentials".to_owned(),
            "--access-token".to_owned(),
        ];
        if let Some(p) = &profile_name {
            cmd.push("--profile".to_owned());
            cmd.push(p.clone());
        }
        let token = cache
            .token(&CredentialHelper {
                command: cmd,
                ttl_secs: Some(20 * 60),
            })
            .await?;
        return Ok(Some(ImportedToken {
            token,
            source: format!("ant profile `{name}`"),
        }));
    }

    let bytes = std::fs::read(&file)
        .map_err(|e| AuthError::Store(format!("read {}: {e}", file.display())))?;
    let v: Value = serde_json::from_slice(&bytes)
        .map_err(|e| AuthError::Store(format!("parse {}: {e}", file.display())))?;
    let token = ["access_token", "accessToken", "access"]
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .ok_or_else(|| AuthError::Store(format!("{} has no access token", file.display())))?;
    if expiry_passed(&v) == Some(true) {
        return Err(AuthError::Refresh {
            provider: "anthropic".into(),
            reason: format!(
                "profile `{name}` token has expired and `ant` is not installed to refresh it; run `ant auth login`"
            ),
        });
    }
    Ok(Some(ImportedToken {
        token: token.to_owned(),
        source: format!("anthropic profile file `{name}`"),
    }))
}

fn expiry_passed(v: &Value) -> Option<bool> {
    let raw = ["expires_at", "expiresAt", "expires"]
        .iter()
        .find_map(|k| v.get(*k))?;
    let now = aether_core::time::now();
    match raw {
        Value::String(s) => chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|t| t.with_timezone(&chrono::Utc) <= now),
        Value::Number(n) => n.as_i64().map(|ms| {
            let ms = if ms > 10_000_000_000 { ms } else { ms * 1000 };
            chrono::DateTime::from_timestamp_millis(ms).is_some_and(|t| t <= now)
        }),
        _ => None,
    }
}

async fn ant_available() -> bool {
    tokio::process::Command::new("ant")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_formats() {
        let past = serde_json::json!({"expires_at": "2000-01-01T00:00:00Z"});
        assert_eq!(expiry_passed(&past), Some(true));
        let future = serde_json::json!({"expiresAt": "2999-01-01T00:00:00Z"});
        assert_eq!(expiry_passed(&future), Some(false));
        let epoch_secs = serde_json::json!({"expires": 946684800});
        assert_eq!(expiry_passed(&epoch_secs), Some(true));
        assert_eq!(expiry_passed(&serde_json::json!({})), None);
    }

    #[tokio::test]
    async fn missing_profile_is_not_configured() {
        let dir = tempfile::tempdir().expect("tempdir");
        let d = dir.path().to_string_lossy().to_string();
        let env = move |k: &str| {
            if k == "ANTHROPIC_CONFIG_DIR" {
                Some(d.clone())
            } else {
                None
            }
        };
        let cache = HelperCache::new();
        let r = import(
            &CredentialImport::AnthropicProfile { profile: None },
            &cache,
            &env,
        )
        .await
        .expect("import");
        assert_eq!(r, None);
    }

    #[test]
    fn debug_redacts_imported_token() {
        let imported = ImportedToken {
            token: "imported-secret".into(),
            source: "ant profile".into(),
        };
        let debug = format!("{imported:?}");
        assert!(!debug.contains("imported-secret"));
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("ant profile"));
    }
}
