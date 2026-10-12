use super::*;
use reqwest::header::HeaderValue;

fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut h = HeaderMap::new();
    for (k, v) in pairs {
        h.insert(*k, HeaderValue::from_str(v).expect("header"));
    }
    h
}

#[test]
fn quota_is_not_a_transient_429_and_keeps_reset_and_redaction() {
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", "3600".parse().unwrap());
    let error = map_http_error(
        429,
        &headers,
        r#"{"error":{"type":"invalid_request_error","code":"insufficient_quota","message":"Credits exhausted; resets tomorrow 14:00; sk-123456789012345678901234"}}"#,
    );
    assert_eq!(error.class, ModelErrorClass::QuotaExhausted);
    assert_eq!(error.retry_after, Some(Duration::from_secs(3600)));
    assert!(error.message.contains("resets tomorrow 14:00"));
    assert!(!error.message.contains("sk-123456789012345678901234"));
    assert_eq!(error.provider_code.as_deref(), Some("insufficient_quota"));
    assert!(!error.is_retryable());
    let credits = map_http_error(
        400,
        &HeaderMap::new(),
        r#"{"error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the API"}}"#,
    );
    assert_eq!(credits.class, ModelErrorClass::QuotaExhausted);
    for (status, message, expected) in [
        (429, "rate limit", ModelErrorClass::RateLimit),
        (429, "quota", ModelErrorClass::RateLimit),
        (401, "insufficient_quota", ModelErrorClass::Auth),
        (503, "quota", ModelErrorClass::Server),
    ] {
        assert_eq!(
            map_http_error(status, &HeaderMap::new(), message).class,
            expected
        );
    }
}

#[test]
fn a_null_code_falls_back_to_the_error_type() {
    let error = map_http_error(
        500,
        &HeaderMap::new(),
        r#"{"error":{"type":"server_error","code":null,"message":"oops"}}"#,
    );
    assert_eq!(error.class, ModelErrorClass::Server);
    assert_eq!(error.provider_code.as_deref(), Some("server_error"));
}

/// The shapes providers actually send, and what each has to mean. The
/// `type` alone gets two of them wrong.
#[test]
fn in_stream_error_table() {
    let cases = [
        // A rejected key: OpenAI sends the auth signal in `code`, under
        // an `invalid_request_error` type.
        (
            "invalid_request_error",
            "Incorrect API key provided",
            Some("invalid_api_key"),
            ModelErrorClass::Auth,
        ),
        // Uppercased by the provider: the code is folded before matching.
        (
            "invalid_request_error",
            "rejected",
            Some("INVALID_API_KEY"),
            ModelErrorClass::Auth,
        ),
        // A real bad request keeps its class.
        (
            "invalid_request_error",
            "Unknown parameter: 'foo'",
            Some("unknown_parameter"),
            ModelErrorClass::InvalidRequest,
        ),
        // An exhausted balance is distinct from transient rate limiting.
        (
            "insufficient_quota",
            "You exceeded your current quota",
            Some("insufficient_quota"),
            ModelErrorClass::QuotaExhausted,
        ),
        (
            "rate_limit_error",
            "slow down",
            None,
            ModelErrorClass::RateLimit,
        ),
        // A throttle that names the key is still a throttle.
        (
            "rate_limit_error",
            "Rate limit exceeded for your API key",
            Some("rate_limit_exceeded"),
            ModelErrorClass::RateLimit,
        ),
        // A typed auth failure is not reread as quota from its wording.
        (
            "authentication_error",
            "insufficient_quota on this key",
            None,
            ModelErrorClass::Auth,
        ),
        ("server_error", "oops", None, ModelErrorClass::Server),
        ("api_error", "oops", None, ModelErrorClass::Server),
        (
            "overloaded_error",
            "overloaded",
            None,
            ModelErrorClass::Overloaded,
        ),
        (
            "invalid_request_error",
            "prompt is too long: 250000 tokens",
            None,
            ModelErrorClass::ContextLimit,
        ),
        // Nobody recognizes it: say so rather than guess. Terminal.
        (
            "weird_error",
            "something happened",
            None,
            ModelErrorClass::Unknown,
        ),
    ];
    for (kind, message, code, want) in cases {
        assert_eq!(
            classify_openai_error(kind, message, code),
            want,
            "{kind} / {message} / {code:?}"
        );
    }
    assert!(
        !ModelError::new(ModelErrorClass::Unknown, "x").is_retryable(),
        "Unknown is documented as terminal"
    );
}

#[test]
fn status_mapping_table() {
    let cases = [
        (401, "{}", ModelErrorClass::Auth),
        (403, "{}", ModelErrorClass::Permission),
        (404, "{}", ModelErrorClass::NotFound),
        (413, "{}", ModelErrorClass::ContextLimit),
        (429, "{}", ModelErrorClass::RateLimit),
        (500, "{}", ModelErrorClass::Server),
        (503, "{}", ModelErrorClass::Server),
        (529, "{}", ModelErrorClass::Overloaded),
        (
            400,
            r#"{"error":{"type":"invalid_request_error","message":"bad field"}}"#,
            ModelErrorClass::InvalidRequest,
        ),
        (
            400,
            r#"{"error":{"type":"invalid_request_error","message":"prompt is too long: 250000 tokens"}}"#,
            ModelErrorClass::ContextLimit,
        ),
        (
            400,
            r#"{"error":{"code":"context_length_exceeded","message":"x"}}"#,
            ModelErrorClass::ContextLimit,
        ),
        (418, "teapot", ModelErrorClass::Unknown),
    ];
    for (status, body, class) in cases {
        let err = map_http_error(status, &HeaderMap::new(), body);
        assert_eq!(err.class, class, "status {status} body {body}");
        assert_eq!(err.status, Some(status));
    }
}

#[test]
fn retry_after_and_request_id_are_captured() {
    let h = headers(&[("retry-after", "2.5"), ("request-id", "req_123")]);
    let err = map_http_error(
        429,
        &h,
        r#"{"error":{"type":"rate_limit_error","message":"slow"}}"#,
    );
    assert_eq!(err.retry_after, Some(Duration::from_millis(2500)));
    assert_eq!(err.request_id.as_deref(), Some("req_123"));
    assert_eq!(err.provider_code.as_deref(), Some("rate_limit_error"));
    assert!(err.message.contains("slow"));
}

#[test]
fn retry_after_ignores_unrepresentable_values() {
    for raw in [
        "1e300",
        "1e20",
        "18446744073709551615",
        "inf",
        "NaN",
        "-1",
        "-inf",
        "invalid",
    ] {
        let err = map_http_error(429, &headers(&[("retry-after", raw)]), "slow down");
        assert_eq!(err.retry_after, None, "{raw}");
    }
    for (raw, expected) in [
        ("0", Duration::ZERO),
        ("2.5", Duration::from_millis(2500)),
        ("3600", Duration::from_secs(3600)),
    ] {
        let err = map_http_error(429, &headers(&[("retry-after", raw)]), "slow down");
        assert_eq!(err.retry_after, Some(expected));
        assert!(RetryPolicy::default().delay_for(1, &err) <= RetryPolicy::default().max_delay);
    }
}

fn response_with_body(body: reqwest::Body) -> reqwest::Response {
    ::http::Response::builder().body(body).unwrap().into()
}

#[tokio::test]
async fn error_body_stops_reading_at_the_limit_without_waiting_for_eof() {
    use futures::{StreamExt, stream};

    let chunks = stream::iter([
        Ok::<_, std::io::Error>(bytes::Bytes::from("a".repeat(7999))),
        Ok(bytes::Bytes::from_static("€tail".as_bytes())),
    ])
    .chain(stream::pending());
    let response = response_with_body(reqwest::Body::wrap_stream(chunks));
    let text = tokio::time::timeout(
        Duration::from_secs(1),
        read_error_body(response, &CancellationToken::new()),
    )
    .await
    .expect("a bounded read must not wait for EOF")
    .expect("body read");
    assert_eq!(text, format!("{}…", "a".repeat(7999)));
}

#[tokio::test]
async fn error_body_preserves_small_bodies_and_reports_read_failures() {
    for body in ["", "short error", &"x".repeat(8000)] {
        assert_eq!(
            read_error_body(
                response_with_body(body.to_owned().into()),
                &CancellationToken::new()
            )
            .await
            .unwrap(),
            body
        );
    }
    let chunks =
        futures::stream::iter([Err::<bytes::Bytes, _>(std::io::Error::other("broken body"))]);
    let text = read_error_body(
        response_with_body(reqwest::Body::wrap_stream(chunks)),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(text.starts_with("<unreadable body:"));
}

#[tokio::test]
async fn error_body_preserves_charset_and_bom_decoding() {
    let response = ::http::Response::builder()
        .header("content-type", "text/plain; charset=windows-1252")
        .body(vec![b'c', b'a', b'f', 0xe9])
        .unwrap()
        .into();
    assert_eq!(
        read_error_body(response, &CancellationToken::new())
            .await
            .unwrap(),
        "café"
    );
    let response = response_with_body(vec![0xef, 0xbb, 0xbf, b'x'].into());
    assert_eq!(
        read_error_body(response, &CancellationToken::new())
            .await
            .unwrap(),
        "x"
    );
}

#[test]
fn string_error_bodies_are_used_as_messages() {
    let err = map_http_error(500, &HeaderMap::new(), r#"{"error":"boom"}"#);
    assert!(err.message.contains("boom"));
    let err = map_http_error(500, &HeaderMap::new(), "plain text failure");
    assert!(err.message.contains("plain text failure"));
}

#[test]
fn delay_honors_retry_after_and_caps() {
    let policy = RetryPolicy {
        max_attempts: 3,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(1),
    };
    let hinted =
        ModelError::new(ModelErrorClass::RateLimit, "x").with_retry_after(Duration::from_secs(30));
    assert_eq!(policy.delay_for(1, &hinted), Duration::from_secs(1));
    let plain = ModelError::new(ModelErrorClass::Server, "x");
    for attempt in 1..6 {
        assert!(policy.delay_for(attempt, &plain) <= Duration::from_secs(1));
    }
}

#[tokio::test]
async fn retry_stops_on_non_retryable_and_respects_budget() {
    let policy = RetryPolicy {
        max_attempts: 3,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(2),
    };
    let cancel = CancellationToken::new();
    let mut calls = 0;
    let res: Result<(), _> = retry(&policy, &cancel, |_| {
        calls += 1;
        async { Err(ModelError::new(ModelErrorClass::Auth, "nope")) }
    })
    .await;
    assert_eq!(res.unwrap_err().class, ModelErrorClass::Auth);
    assert_eq!(calls, 1);

    let mut calls = 0;
    let res: Result<(), _> = retry(&policy, &cancel, |_| {
        calls += 1;
        async { Err(ModelError::new(ModelErrorClass::Server, "down")) }
    })
    .await;
    assert_eq!(res.unwrap_err().class, ModelErrorClass::Server);
    assert_eq!(calls, 3);

    let mut calls = 0;
    let res = retry(&policy, &cancel, |attempt| {
        calls += 1;
        async move {
            if attempt < 2 {
                Err(ModelError::new(ModelErrorClass::Overloaded, "busy"))
            } else {
                Ok(attempt)
            }
        }
    })
    .await;
    assert_eq!(res.expect("ok"), 2);
    assert_eq!(calls, 2);
}

#[tokio::test]
async fn retry_observes_cancellation() {
    let policy = RetryPolicy {
        max_attempts: 5,
        base_delay: Duration::from_secs(5),
        max_delay: Duration::from_secs(5),
    };
    let cancel = CancellationToken::new();
    cancel.cancel();
    let res: Result<(), _> = retry(&policy, &cancel, |_| async {
        Err(ModelError::new(ModelErrorClass::Server, "x"))
    })
    .await;
    assert_eq!(res.unwrap_err().class, ModelErrorClass::Cancelled);
}

#[test]
fn truncate_respects_char_boundaries() {
    let s = "héllo";
    assert_eq!(truncate(s, 2), "h…");
    assert_eq!(truncate(s, 10), "héllo");
}

#[tokio::test]
async fn stalled_error_body_read_is_cancellable_after_headers() {
    use futures::{StreamExt, stream};
    let cancel = CancellationToken::new();
    let body_cancel = cancel.clone();
    let chunks = stream::once(async move {
        body_cancel.cancel();
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"partial error"))
    })
    .chain(stream::pending());
    let response = response_with_body(reqwest::Body::wrap_stream(chunks));
    let err = tokio::time::timeout(Duration::from_secs(1), read_error_body(response, &cancel))
        .await
        .expect("cancellation must not await the read timeout")
        .unwrap_err();
    assert_eq!(err.class, ModelErrorClass::Cancelled);
}
