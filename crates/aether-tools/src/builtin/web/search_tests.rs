use super::*;
use crate::testing::ctx_in;
use aether_core::agent::Scope;

/// Answers one request with `body`, recording the headers it saw.
async fn one_shot(body: &'static str) -> (String, tokio::task::JoinHandle<String>) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    raw_shot(response).await
}

/// Answers one request with exactly `response`, returning what it read.
async fn raw_shot(response: String) -> (String, tokio::task::JoinHandle<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 8192];
        let n = socket.read(&mut buf).await.unwrap();
        let request = String::from_utf8_lossy(&buf[..n]).into_owned();
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
        request
    });
    (format!("http://{addr}/search"), handle)
}

fn provider(endpoint: String, api_key_env: Option<&str>) -> SearchProvider {
    SearchProvider {
        endpoint,
        query_param: "q".into(),
        results_pointer: "/web/results".into(),
        title_field: "title".into(),
        url_field: "url".into(),
        snippet_field: "description".into(),
        api_key_env: api_key_env.map(str::to_owned),
        api_key_header: "X-Test-Token".into(),
    }
}

fn networked_ctx(dir: &std::path::Path) -> ToolContext {
    let mut ctx = ctx_in(dir);
    ctx.scope = Scope {
        network: NetworkPolicy::Allowed,
        ..ctx.scope
    };
    ctx
}

#[tokio::test(flavor = "multi_thread")]
async fn search_reads_the_configured_provider() {
    let body = r#"{"web":{"results":[
        {"title":"First","url":"https://example.com/1","description":"one"},
        {"title":"Second","url":"https://example.com/2","description":"two"}
    ]}}"#;
    let (endpoint, server) = one_shot(body).await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = networked_ctx(dir.path());
    let tool = WebSearchTool::new(provider(endpoint, None));
    let out = tool
        .execute(
            json!({"query": "rust harness", "limit": 5}),
            &ctx,
            CancellationToken::new(),
        )
        .await
        .expect("search");
    let request = server.await.expect("server");
    assert!(request.contains("q=rust+harness"), "{request}");
    match out {
        ToolExecution::Outcome(o) => {
            assert!(o.content.contains("First"), "{}", o.content);
            assert!(o.content.contains("https://example.com/2"), "{}", o.content);
            assert_eq!(o.structured.as_ref().unwrap()["results"], 2);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn web_tools_refuse_an_agent_whose_scope_denies_network() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_in(dir.path());
    assert_eq!(ctx.scope.network, NetworkPolicy::Denied);
    let fetch = WebFetchTool::default()
        .execute(
            json!({"url": "https://example.com"}),
            &ctx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(fetch, Err(ToolError::Denied(_))), "{fetch:?}");
    let search = WebSearchTool::new(provider("https://example.com".into(), None))
        .execute(json!({"query": "x"}), &ctx, CancellationToken::new())
        .await;
    assert!(matches!(search, Err(ToolError::Denied(_))), "{search:?}");
}

#[tokio::test]
async fn a_missing_credential_fails_by_name_and_never_reaches_the_network() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = networked_ctx(dir.path());
    // An endpoint that would hang if it were ever contacted.
    // TEST-NET-1 over https: it would hang if it were ever contacted,
    // and https is required of an endpoint carrying a credential.
    let tool = WebSearchTool::new(provider(
        "https://192.0.2.1/search".into(),
        Some("AETHER_NO_SUCH_SEARCH_KEY"),
    ));
    let err = tool
        .execute(json!({"query": "x"}), &ctx, CancellationToken::new())
        .await
        .expect_err("must refuse without the credential");
    let message = err.to_string();
    assert!(matches!(err, ToolError::Denied(_)), "{message}");
    assert!(
        message.contains("AETHER_NO_SUCH_SEARCH_KEY"),
        "the message must name the variable: {message}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn search_sends_the_credential_in_the_configured_header() {
    // Setting an environment variable is `unsafe` and the workspace
    // forbids it, so the test names one the process already has.
    let var = "PATH";
    let expected = std::env::var(var).expect("PATH is set");
    let body = r#"{"web":{"results":[]}}"#;
    let (endpoint, server) = one_shot(body).await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = networked_ctx(dir.path());
    let tool = WebSearchTool::new(provider(endpoint, Some(var)));
    tool.execute(json!({"query": "x"}), &ctx, CancellationToken::new())
        .await
        .expect("search");
    let request = server.await.expect("server");
    let header = request
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("x-test-token:"))
        .expect("the credential must ride in the configured header");
    assert_eq!(
        header.split_once(':').unwrap().1.trim(),
        expected,
        "the header must carry the variable's value"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn search_refuses_a_redirect_rather_than_forwarding_the_credential() {
    // reqwest strips Authorization and Cookie across hosts, but not a
    // custom header, so following this would hand the key to whoever the
    // Location names.
    let (endpoint, server) = raw_shot(
        "HTTP/1.1 302 Found\r\nLocation: http://elsewhere.example/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .into(),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = networked_ctx(dir.path());
    let tool = WebSearchTool::new(provider(endpoint, None));
    let err = tool
        .execute(json!({"query": "x"}), &ctx, CancellationToken::new())
        .await
        .expect_err("a redirect must be refused");
    let message = err.to_string();
    assert!(
        message.contains("redirect") && message.contains("302"),
        "the refusal must say why: {message}"
    );
    let request = server.await.expect("server");
    assert!(
        !request.contains("elsewhere.example"),
        "only the configured host was ever contacted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_body_past_the_cap_is_cut_off_instead_of_being_buffered_whole() {
    let oversized = MAX_BODY_BYTES + (128 << 10);
    // Chunked, so the cap has to bite while reading rather than being
    // refused up front on Content-Length.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = socket.read(&mut vec![0u8; 8192]).await;
        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
        if socket.write_all(head.as_bytes()).await.is_err() {
            return;
        }
        let chunk = vec![b'a'; 64 << 10];
        let mut sent = 0usize;
        while sent < oversized {
            let header = format!("{:x}\r\n", chunk.len());
            if socket.write_all(header.as_bytes()).await.is_err()
                || socket.write_all(&chunk).await.is_err()
                || socket.write_all(b"\r\n").await.is_err()
            {
                return;
            }
            sent += chunk.len();
        }
        let _ = socket.write_all(b"0\r\n\r\n").await;
    });
    let mut response = reqwest::Client::new()
        .get(format!("http://{addr}/big"))
        .send()
        .await
        .expect("connect");
    let (body, truncated) = read_capped(&mut response).await.expect("read");
    assert!(truncated, "the cap must be reported");
    assert_eq!(
        body.len(),
        MAX_BODY_BYTES,
        "nothing past the cap is kept in memory"
    );
}

#[test]
fn fetched_text_uses_the_declared_charset() {
    assert_eq!(
        decode_body(b"caf\xe9", "text/html; charset=windows-1252"),
        ("café".into(), vec![])
    );
    assert_eq!(
        decode_body(b"\x93\xfa\x96\x7b", "text/plain; charset=\"Shift_JIS\""),
        ("日本".into(), vec![])
    );
    for content_type in ["text/plain", "invalid"] {
        assert_eq!(
            decode_body("café".as_bytes(), content_type),
            ("café".into(), vec![])
        );
    }
}

#[test]
fn unsupported_charsets_and_replaced_bytes_are_reported() {
    for label in [
        "unknown",
        "replacement",
        "iso-2022-kr",
        "hz-gb-2312",
        "iso-2022-cn",
    ] {
        assert_eq!(
            decode_body("café".as_bytes(), &format!("text/plain; charset={label}")),
            (
                "café".into(),
                vec![format!("[charset {label} unsupported; decoded as UTF-8]")]
            )
        );
    }
    assert_eq!(
        decode_body(b"caf\xff", "text/plain; charset=UTF-8"),
        ("caf�".into(), vec!["[undecodable bytes replaced]".into()])
    );
}

#[test]
fn html_metadata_supplies_charset_only_without_a_header_and_within_the_prefix() {
    for tag in [
        "<meta charset=windows-1252>",
        "<META name='ignored' CHARSET=\"windows-1252\">",
        "<meta charset='windows-1252' />",
    ] {
        let mut body = tag.as_bytes().to_vec();
        body.extend_from_slice(b"<p>caf\xe9</p>");
        let (decoded, notes) = decode_body(&body, "text/html");
        assert_eq!(html_to_text(&decoded), "café");
        assert!(notes.is_empty());
        for content_type in ["text/html; charset=UTF-8", "text/plain"] {
            let (decoded, notes) = decode_body(&body, content_type);
            assert!(decoded.contains("caf�"));
            assert_eq!(notes, ["[undecodable bytes replaced]"]);
        }
        let mut late = vec![b' '; 1024];
        late.extend_from_slice(&body);
        assert!(decode_body(&late, "text/html").0.contains("caf�"));
    }
    assert_eq!(html_charset(b"<meta content='charset=windows-1252'>"), None);
    assert_eq!(html_charset(b"<meta-data charset=windows-1252>"), None);
    assert_eq!(html_charset(b"<!-- <meta charset=windows-1252> -->"), None);
    assert_eq!(
        html_charset(b"<!-- <meta charset=Shift_JIS> --><meta charset=windows-1252>"),
        Some("windows-1252".into())
    );
}

#[tokio::test]
async fn an_exact_cap_body_is_complete_with_or_without_content_length() {
    for chunked in [false, true] {
        let body = "a".repeat(MAX_BODY_BYTES);
        let response = if chunked {
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
                body.len()
            )
        } else {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
        };
        let (endpoint, server) = raw_shot(response).await;
        let mut response = reqwest::Client::new()
            .get(endpoint)
            .send()
            .await
            .expect("response");
        let (bytes, truncated) = read_capped(&mut response).await.expect("body");
        assert!(!truncated, "an exact fit is complete (chunked={chunked})");
        assert_eq!(bytes, body.as_bytes());
        server.await.expect("server");
    }
}

#[tokio::test]
async fn one_byte_past_the_body_cap_is_truncated() {
    let body = "a".repeat(MAX_BODY_BYTES + 1);
    let response = format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
        body.len()
    );
    let (endpoint, server) = raw_shot(response).await;
    let mut response = reqwest::Client::new()
        .get(endpoint)
        .send()
        .await
        .expect("response");
    let (bytes, truncated) = read_capped(&mut response).await.expect("body");
    assert!(truncated);
    assert_eq!(bytes.len(), MAX_BODY_BYTES);
    server.await.expect("server");
}

#[test]
fn a_redirect_is_reported_as_an_absolute_target_not_followed() {
    use reqwest::header::{HeaderMap, HeaderValue, LOCATION};
    let base = Url::parse("https://example.com/a/b").unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, HeaderValue::from_static("/c"));
    assert_eq!(
        redirect_target(reqwest::StatusCode::FOUND, &headers, &base),
        Some("https://example.com/c".to_owned()),
        "a relative Location is resolved so the next call can vet it"
    );
    headers.insert(
        LOCATION,
        HeaderValue::from_static("http://169.254.169.254/latest/meta-data"),
    );
    assert_eq!(
        redirect_target(reqwest::StatusCode::MOVED_PERMANENTLY, &headers, &base),
        Some("http://169.254.169.254/latest/meta-data".to_owned()),
        "the hop is handed back, never fetched: the next call refuses it"
    );
    // `only_public_http_targets_are_fetched` proves that hop is refused
    // when the model brings it back as a fresh call.
    assert_eq!(
        redirect_target(reqwest::StatusCode::OK, &headers, &base),
        None
    );
    assert_eq!(
        redirect_target(reqwest::StatusCode::FOUND, &HeaderMap::new(), &base),
        None
    );
}
