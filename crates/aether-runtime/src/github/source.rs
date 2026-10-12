//! Bounded GitHub CLI transport. Arguments are never interpreted by a shell.
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(super) struct Error {
    pub message: String,
    pub retry_after: u64,
    pub status: Option<u16>,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retry_after: 0,
            status: None,
        }
    }
}

#[async_trait::async_trait]
pub(super) trait Source: Send + Sync {
    async fn get(&self, path: &str) -> Result<Value, Error>;
    /// One GraphQL request; `request` is the whole `{query, variables}` body.
    async fn graphql(&self, request: &Value) -> Result<Value, Error>;
}

pub(super) struct Gh;

#[async_trait::async_trait]
impl Source for Gh {
    async fn get(&self, path: &str) -> Result<Value, Error> {
        let (out, success) =
            run_gh(&["-H", "Accept: application/vnd.github+json", path], None).await?;
        parse_response(&out, success)
    }

    async fn graphql(&self, request: &Value) -> Result<Value, Error> {
        debug_assert!(request["query"].is_string(), "a GraphQL body has a query");
        let body = serde_json::to_vec(request)
            .map_err(|e| Error::new(format!("serialize GitHub GraphQL request: {e}")))?;
        let (out, _) = run_gh(&["graphql", "--input", "-"], Some(body)).await?;
        // gh exits non-zero when a 200 body lists GraphQL errors. The HTTP
        // status is checked here; the caller refuses a body with `errors`.
        parse_response(&out, true)
    }
}

/// Run `gh api` with bounded output and a deadline, feeding `input` on stdin.
/// Returns stdout (headers and body) and whether gh reported success.
async fn run_gh(tail: &[&str], input: Option<Vec<u8>>) -> Result<(Vec<u8>, bool), Error> {
    debug_assert!(!tail.is_empty(), "gh api needs an endpoint");
    let mut child = Command::new("gh")
        .args(["api", "--hostname", "github.com", "--include"])
        .args(tail)
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            Error::new(format!(
                "cannot start gh api: {e}; install gh and run gh auth login"
            ))
        })?;
    let stdin = child.stdin.take();
    debug_assert_eq!(
        stdin.is_some(),
        input.is_some(),
        "stdin is piped only for input"
    );
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::new("gh stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::new("gh stderr unavailable"))?;
    const LIMIT: u64 = 16 * 1024 * 1024;
    let operation = async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        tokio::try_join!(
            write_input(stdin, input),
            read_bounded(stdout, &mut out, LIMIT),
            read_bounded(stderr, &mut err, 65536)
        )?;
        let status = child
            .wait()
            .await
            .map_err(|e| Error::new(format!("waiting for gh: {e}")))?;
        Ok((out, status.success()))
    };
    tokio::time::timeout(Duration::from_secs(30), operation)
        .await
        .map_err(|_| Error::new("gh api timed out after 30 seconds"))?
}

/// Write the whole request body, then close stdin so gh sees its end.
async fn write_input(
    stdin: Option<tokio::process::ChildStdin>,
    input: Option<Vec<u8>>,
) -> Result<(), Error> {
    let (Some(mut stdin), Some(input)) = (stdin, input) else {
        return Ok(());
    };
    stdin
        .write_all(&input)
        .await
        .map_err(|e| Error::new(format!("writing gh input: {e}")))?;
    stdin
        .shutdown()
        .await
        .map_err(|e| Error::new(format!("closing gh input: {e}")))
}

async fn read_bounded(
    mut input: impl tokio::io::AsyncRead + Unpin,
    bytes: &mut Vec<u8>,
    limit: u64,
) -> Result<(), Error> {
    (&mut input)
        .take(limit + 1)
        .read_to_end(bytes)
        .await
        .map_err(|e| Error::new(format!("reading gh output: {e}")))?;
    if bytes.len() as u64 > limit {
        return Err(Error::new("gh response exceeds bounded output limit"));
    }
    Ok(())
}

fn parse_response(out: &[u8], success: bool) -> Result<Value, Error> {
    let text = std::str::from_utf8(out).map_err(|_| Error::new("gh returned non-UTF-8 output"))?;
    let (headers, body) = text
        .split_once("\r\n\r\n")
        .or_else(|| text.split_once("\n\n"))
        .ok_or_else(|| Error::new("gh returned no HTTP headers; verify gh auth login"))?;
    let header = |name: &str| -> Option<&str> {
        headers
            .lines()
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim())
    };
    let code = headers
        .lines()
        .next()
        .and_then(|s| s.split_whitespace().nth(1))
        .unwrap_or("unknown");
    let remaining = header("x-ratelimit-remaining").and_then(|s| s.parse::<u64>().ok());
    let limited = code == "429"
        || remaining.is_some_and(|n| n <= 10)
        || header("retry-after").is_some()
        || (code == "403" && body.to_ascii_lowercase().contains("rate limit"));
    if limited {
        let reset = header("x-ratelimit-reset")
            .and_then(|s| s.parse::<i64>().ok())
            .map(|t| t.saturating_sub(chrono::Utc::now().timestamp()).max(0) as u64 + 1)
            .unwrap_or(0);
        let retry = header("retry-after")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60);
        return Err(Error {
            message: format!(
                "GitHub HTTP {code}: rate limit or low remaining quota; polling paused"
            ),
            retry_after: reset.max(retry).min(86400),
            status: code.parse().ok(),
        });
    }
    if !success || code != "200" {
        // Do not persist arbitrary stderr/body: it can include credentials or hostile text.
        return Err(Error {
            message: format!(
                "GitHub HTTP {code}; verify gh auth login and repository/token access"
            ),
            retry_after: 0,
            status: code.parse().ok(),
        });
    }
    serde_json::from_str(body).map_err(|e| Error::new(format!("invalid GitHub JSON: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rate_headers_control_backoff_even_on_success() {
        let e = parse_response(
            b"HTTP/2.0 200 OK\r\nX-RateLimit-Remaining: 0\r\nRetry-After: 321\r\n\r\n[]",
            true,
        )
        .unwrap_err();
        assert!(e.retry_after >= 321);
    }
    #[test]
    fn forbidden_repository_is_not_a_global_rate_cooldown() {
        let error = parse_response(
            b"HTTP/2.0 403 Forbidden\n\n{\"message\":\"Resource not accessible by integration\"}",
            false,
        )
        .unwrap_err();
        assert_eq!(error.status, Some(403));
        assert_eq!(error.retry_after, 0);
        let limited = parse_response(
            b"HTTP/2.0 403 Forbidden\n\n{\"message\":\"API rate limit exceeded\"}",
            false,
        )
        .unwrap_err();
        assert!(limited.retry_after >= 60);
    }
    #[test]
    fn authentication_failure_is_not_an_empty_listing() {
        assert!(parse_response(b"HTTP/2.0 401 Unauthorized\n\n{}", false).is_err());
        assert_eq!(
            parse_response(b"HTTP/2.0 200 OK\n\n[]", true).ok(),
            Some(serde_json::json!([]))
        );
    }
}
