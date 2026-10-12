//! External credential helpers: a command whose stdout is a bearer token.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use super::types::AuthError;

const DEFAULT_TTL: Duration = Duration::from_secs(50 * 60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialHelper {
    /// Program and arguments, run without a shell.
    pub command: Vec<String>,
    /// Cache lifetime in seconds when the output does not say; default 50 minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_secs: Option<u64>,
}

impl CredentialHelper {
    pub fn new(command: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            command: command.into_iter().map(Into::into).collect(),
            ttl_secs: None,
        }
    }

    pub fn display(&self) -> String {
        self.command.join(" ")
    }
}

/// Process-wide cache so a helper is not executed on every request.
#[derive(Default)]
pub struct HelperCache {
    entries: Mutex<HashMap<String, (String, Instant)>>,
}

impl std::fmt::Debug for HelperCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelperCache")
            .field("entries", &"<redacted>")
            .finish()
    }
}

impl HelperCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn token(&self, helper: &CredentialHelper) -> Result<String, AuthError> {
        let key = helper.display();
        {
            let entries = self.entries.lock().await;
            if let Some((token, until)) = entries.get(&key)
                && Instant::now() < *until
            {
                return Ok(token.clone());
            }
        }
        let (token, ttl) = run(helper).await?;
        self.entries
            .lock()
            .await
            .insert(key, (token.clone(), Instant::now() + ttl));
        Ok(token)
    }
}

/// Run the helper. Output is either a bare token or JSON with
/// `access_token` and optional `expires_in`.
pub async fn run(helper: &CredentialHelper) -> Result<(String, Duration), AuthError> {
    let (program, args) = helper
        .command
        .split_first()
        .ok_or_else(|| AuthError::Helper {
            command: String::new(),
            reason: "empty command".into(),
        })?;
    let output = tokio::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| AuthError::Helper {
            command: helper.display(),
            reason: e.to_string(),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AuthError::Helper {
            command: helper.display(),
            reason: format!(
                "exit {}: {}",
                output.status,
                crate::http::truncate(stderr.trim(), 300)
            ),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if stdout.is_empty() {
        return Err(AuthError::Helper {
            command: helper.display(),
            reason: "printed nothing".into(),
        });
    }
    let default_ttl = helper
        .ttl_secs
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_TTL);
    if stdout.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<Value>(&stdout)
            && let Some(token) = v.get("access_token").and_then(Value::as_str)
        {
            let ttl = v
                .get("expires_in")
                .and_then(Value::as_u64)
                .map(Duration::from_secs)
                .unwrap_or(default_ttl);
            return Ok((token.to_owned(), ttl));
        }
        return Err(AuthError::Helper {
            command: helper.display(),
            reason: "JSON output has no access_token".into(),
        });
    }
    Ok((stdout, default_ttl))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bare_and_json_outputs() {
        let (tok, ttl) = run(&CredentialHelper::new(["echo", "  tok-1  "]))
            .await
            .expect("run");
        assert_eq!(tok, "tok-1");
        assert_eq!(ttl, DEFAULT_TTL);
        let (tok, ttl) = run(&CredentialHelper::new([
            "echo",
            r#"{"access_token":"j","expires_in":10}"#,
        ]))
        .await
        .expect("run");
        assert_eq!(tok, "j");
        assert_eq!(ttl, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn failures_are_visible_and_cache_reuses() {
        let err = run(&CredentialHelper::new(["false"])).await.unwrap_err();
        assert!(matches!(err, AuthError::Helper { .. }));
        let err = run(&CredentialHelper::new(["definitely-not-a-real-binary-xyz"]))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Helper { .. }));

        let cache = HelperCache::new();
        let helper = CredentialHelper::new(["echo", "cached"]);
        assert_eq!(cache.token(&helper).await.expect("t"), "cached");
        assert_eq!(cache.entries.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn debug_redacts_cached_tokens_without_locking() {
        let cache = HelperCache::new();
        let mut entries = cache.entries.lock().await;
        entries.insert(
            "helper command".into(),
            ("cached-secret".into(), Instant::now() + DEFAULT_TTL),
        );
        drop(entries);
        let debug = format!("{cache:?}");
        assert!(!debug.contains("cached-secret"));
        assert!(debug.contains("<redacted>"));
        let _entries = cache.entries.lock().await;
        assert_eq!(format!("{cache:?}"), debug);
    }
}
