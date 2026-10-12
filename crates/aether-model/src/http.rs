//! Shared HTTP plumbing: client construction, error mapping, bounded retry.

use std::future::Future;
use std::sync::LazyLock;
use std::time::Duration;

use aether_core::model::{ModelError, ModelErrorClass};
use aether_core::redact::Redactor;
use reqwest::header::HeaderMap;
use tokio_util::sync::CancellationToken;

/// Connect timeout for every request.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Idle read timeout for streaming responses. Long enough for slow thinking.
pub const READ_TIMEOUT: Duration = Duration::from_secs(300);
/// Total timeout for short non-streaming calls (token exchange, model lists).
pub const SHORT_TIMEOUT: Duration = Duration::from_secs(30);

/// Build the shared client. No total timeout: streams may legitimately run
/// for many minutes; idle reads are bounded instead.
pub fn build_client(user_agent: &str) -> Result<reqwest::Client, ModelError> {
    reqwest::Client::builder()
        .user_agent(user_agent)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|e| ModelError::new(ModelErrorClass::Unknown, format!("http client: {e}")))
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(20),
        }
    }
}

impl RetryPolicy {
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            ..Default::default()
        }
    }

    /// Delay before the next attempt: server hint if present, else
    /// exponential backoff with full jitter.
    pub fn delay_for(&self, attempt: u32, err: &ModelError) -> Duration {
        if let Some(after) = err.retry_after {
            return after.min(self.max_delay);
        }
        let exp = self
            .base_delay
            .saturating_mul(2u32.saturating_pow(attempt.saturating_sub(1)));
        let cap = exp.min(self.max_delay);
        let jitter: f64 = rand::random_range(0.0..=1.0);
        cap.mul_f64(jitter)
    }
}

/// Run `op` until it succeeds, returns a non-retryable error, the attempt
/// budget is spent, or `cancel` fires. `op` receives the 1-based attempt.
pub async fn retry<T, F, Fut>(
    policy: &RetryPolicy,
    cancel: &CancellationToken,
    mut op: F,
) -> Result<T, ModelError>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<T, ModelError>>,
{
    let mut attempt = 1;
    loop {
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        match op(attempt).await {
            Ok(v) => return Ok(v),
            Err(err) if err.is_retryable() && attempt < policy.max_attempts.max(1) => {
                let delay = policy.delay_for(attempt, &err);
                tracing::warn!(attempt, ?delay, error = %err, "retrying model request");
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = cancel.cancelled() => return Err(cancelled()),
                }
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

/// A stream that started and then stopped without its protocol's terminal
/// event. The bytes ran out mid-answer, so the turn is interrupted, not
/// finished: retryable, and no fabricated stop reason.
pub fn interrupted_stream() -> ModelError {
    ModelError::new(
        ModelErrorClass::Network,
        "stream ended before the model finished its response",
    )
}

pub fn cancelled() -> ModelError {
    ModelError::new(ModelErrorClass::Cancelled, "request cancelled")
}

/// Map a transport-level failure.
pub fn map_transport_error(err: reqwest::Error) -> ModelError {
    let class = if err.is_timeout() {
        ModelErrorClass::Timeout
    } else if err.is_connect() || err.is_request() || err.is_body() || err.is_decode() {
        ModelErrorClass::Network
    } else {
        ModelErrorClass::Unknown
    };
    ModelError::new(class, err.to_string())
}

/// Provider-controlled text on its way into a durable record: recognizable
/// secrets masked, size bounded.
///
/// Everything an adapter learns from an HTTP body or a stream goes through
/// here before it can reach a `ModelError`, and from there the invocation
/// row, the `invocation.failed` event, the timeline and the log. OpenAI's
/// 401 body quotes the key it rejected, so without this the durable event
/// log holds a fragment of the credential (VISION §10).
pub fn safe_provider_text(text: &str, max: usize) -> String {
    static REDACTOR: LazyLock<Redactor> = LazyLock::new(Redactor::default);
    truncate(&REDACTOR.redact(text), max)
}

/// Map an HTTP error status plus its body to a normalized error.
pub fn map_http_error(status: u16, headers: &HeaderMap, body: &str) -> ModelError {
    let parsed = parse_error_body(body);
    let message = safe_provider_text(
        &parsed.message.clone().unwrap_or_else(|| body.to_owned()),
        2000,
    );
    let lower = format!(
        "{} {} {}",
        message.to_lowercase(),
        parsed.code.as_deref().unwrap_or("").to_lowercase(),
        parsed.kind.as_deref().unwrap_or("").to_lowercase()
    );
    let exhaustion = aether_core::model::provider_exhaustion(&lower);
    let class = if matches!(status, 400 | 402 | 422 | 429)
        && let Some(class) = exhaustion
    {
        class
    } else {
        match status {
            400 | 422 => {
                if is_context_limit_message(&lower) {
                    ModelErrorClass::ContextLimit
                } else {
                    ModelErrorClass::InvalidRequest
                }
            }
            401 => ModelErrorClass::Auth,
            403 => ModelErrorClass::Permission,
            404 => ModelErrorClass::NotFound,
            408 => ModelErrorClass::Timeout,
            413 => ModelErrorClass::ContextLimit,
            429 => ModelErrorClass::RateLimit,
            529 => ModelErrorClass::Overloaded,
            500 | 502 | 503 | 504 => ModelErrorClass::Server,
            _ => ModelErrorClass::Unknown,
        }
    };
    let mut err = ModelError::new(class, format!("HTTP {status}: {message}")).with_status(status);
    if let Some(after) = retry_after(headers) {
        err = err.with_retry_after(after);
    }
    if let Some(id) = request_id(headers) {
        err = err.with_request_id(id);
    }
    if let Some(code) = parsed.code {
        err = err.with_provider_code(safe_provider_text(&code, 200));
    }
    err
}

/// Classify an OpenAI-family error object from a stream. The provider's own
/// `type` decides first, then `code` and the message as a second chance -
/// except for the two signals a `type` gets wrong on its own:
///
/// - A rejected key arrives as `{"type":"invalid_request_error",
///   "code":"invalid_api_key"}`. Both classes are unretryable, so the turn
///   behaves the same either way, but the operator is told which one it is.
/// - Explicit quota/usage exhaustion is distinct from transient rate limiting.
///   The runtime may try a fallback without retrying the exhausted final link.
///
/// An error nobody recognizes is `Unknown`, never `InvalidRequest`: nothing
/// here is evidence the caller sent a bad request, and the operator should
/// read "we do not know" rather than a guess. `Unknown` is terminal like
/// `InvalidRequest` - deliberately: an unrecognized fault is no evidence that
/// another provider would do better, so spending every link of the chain on
/// it buys nothing. Shared by both OpenAI adapters so they cannot drift apart.
pub fn classify_openai_error(kind: &str, message: &str, code: Option<&str>) -> ModelErrorClass {
    let lower = format!(
        "{} {}",
        message.to_lowercase(),
        code.unwrap_or("").to_lowercase()
    );
    if is_context_limit_message(&lower) {
        return ModelErrorClass::ContextLimit;
    }
    // A typed auth failure outranks quota wording in its explanation.
    if kind.contains("authentication") {
        return ModelErrorClass::Auth;
    }
    if let Some(class) = aether_core::model::provider_exhaustion(&format!("{kind} {lower}")) {
        return class;
    }
    if kind.contains("rate_limit") {
        return ModelErrorClass::RateLimit;
    }
    // After `rate_limit`: a throttle message naming the API key is no auth failure.
    if is_auth_signal(&lower) {
        return ModelErrorClass::Auth;
    }
    if kind.contains("overloaded") {
        return ModelErrorClass::Overloaded;
    }
    if kind.contains("server") || kind == "api_error" {
        return ModelErrorClass::Server;
    }
    if kind.contains("invalid_request") {
        return ModelErrorClass::InvalidRequest;
    }
    if lower.contains("rate_limit") || lower.contains("rate limit") {
        ModelErrorClass::RateLimit
    } else if lower.contains("server_error") || lower.contains("overloaded") {
        ModelErrorClass::Server
    } else {
        ModelErrorClass::Unknown
    }
}

/// A rejected, missing or expired credential, however the provider words it.
fn is_auth_signal(lower: &str) -> bool {
    lower.contains("invalid_api_key")
        || lower.contains("invalid_token")
        || lower.contains("api key")
        || lower.contains("api_key")
}

/// Providers phrase context overflow differently; these substrings cover
/// Anthropic, OpenAI and the common local servers.
pub fn is_context_limit_message(lower: &str) -> bool {
    lower.contains("prompt is too long")
        || lower.contains("context length")
        || lower.contains("context window")
        || lower.contains("too many tokens")
        || lower.contains("maximum context")
        || lower.contains("exceeds the context")
        || (lower.contains("too long") && lower.contains("token"))
        || lower.contains("context_length_exceeded")
}

pub fn request_id(headers: &HeaderMap) -> Option<String> {
    ["request-id", "x-request-id", "cf-ray"]
        .iter()
        .find_map(|h| {
            headers
                .get(*h)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        })
}

/// `Retry-After` in seconds (an HTTP date is ignored; backoff covers it).
pub fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let raw = headers.get("retry-after")?.to_str().ok()?.trim();
    raw.parse::<f64>()
        .ok()
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
}

#[derive(Debug, Default)]
struct ParsedError {
    message: Option<String>,
    code: Option<String>,
    kind: Option<String>,
}

/// Best-effort extraction from `{"error": {...}}`, `{"error": "..."}`, or `{"message": "..."}`.
fn parse_error_body(body: &str) -> ParsedError {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return ParsedError::default();
    };
    let error = value.get("error").unwrap_or(&value);
    let message = error
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            error
                .get("message")
                .and_then(|m| m.as_str())
                .map(str::to_owned)
        })
        .or_else(|| {
            value
                .get("message")
                .and_then(|m| m.as_str())
                .map(str::to_owned)
        });
    // OpenAI sends `"code": null` on many errors; the `type` is the code then.
    let scalar = |c: &serde_json::Value| {
        c.as_str()
            .map(str::to_owned)
            .or_else(|| c.as_i64().map(|n| n.to_string()))
    };
    let code = error
        .get("code")
        .and_then(scalar)
        .or_else(|| error.get("type").and_then(scalar));
    let kind = error
        .get("type")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    ParsedError {
        message,
        code,
        kind,
    }
}

pub fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Read a response body for error reporting, bounded in size.
pub async fn read_error_body(
    resp: reqwest::Response,
    cancel: &CancellationToken,
) -> Result<String, ModelError> {
    read_bounded_body(resp, cancel, 8000, true).await
}

/// Read a complete response under a byte limit, preserving read/decoding errors.
pub(crate) async fn read_response_body(
    resp: reqwest::Response,
    cancel: &CancellationToken,
    max_bytes: usize,
) -> Result<String, ModelError> {
    read_bounded_body(resp, cancel, max_bytes, false).await
}

async fn read_bounded_body(
    mut resp: reqwest::Response,
    cancel: &CancellationToken,
    max_bytes: usize,
    allow_excerpt: bool,
) -> Result<String, ModelError> {
    let mut bytes = Vec::with_capacity(max_bytes.min(8000) + 1);
    while bytes.len() <= max_bytes {
        let chunk = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(cancelled()),
            chunk = resp.chunk() => chunk,
        };
        match chunk {
            Ok(Some(chunk)) => {
                let remaining = max_bytes + 1 - bytes.len();
                bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
            }
            Ok(None) => break,
            Err(e) if allow_excerpt => return Ok(format!("<unreadable body: {e}>")),
            Err(e) => {
                return Err(ModelError::new(
                    ModelErrorClass::Unknown,
                    format!("reading response body: {e}"),
                ));
            }
        }
    }
    let truncated = bytes.len() > max_bytes;
    if truncated && !allow_excerpt {
        return Err(ModelError::new(
            ModelErrorClass::Unknown,
            format!("response body exceeds {max_bytes} byte limit"),
        ));
    }
    // Reuse reqwest's charset and BOM decoding on only the bounded prefix.
    let response: ::http::Response<reqwest::Body> = resp.into();
    let bounded = reqwest::Response::from(response.map(|_| bytes));
    Ok(match bounded.text().await {
        Ok(text) => {
            if !allow_excerpt {
                return Ok(text);
            }
            let mut excerpt = truncate(&text, max_bytes);
            if truncated && text.len() <= max_bytes {
                excerpt.push('…');
            }
            excerpt
        }
        Err(e) if allow_excerpt => format!("<unreadable body: {e}>"),
        Err(e) => {
            return Err(ModelError::new(
                ModelErrorClass::Unknown,
                format!("decoding response body: {e}"),
            ));
        }
    })
}

#[cfg(test)]
mod tests;
