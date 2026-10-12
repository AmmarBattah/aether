#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::time::Duration;

use aether_model::auth::AuthError;
use aether_model::auth::interaction::ScriptedInteraction;
use aether_model::auth::oauth::generic::{
    OAuthFlowConfig, OAuthTokens, TokenContentType, login_browser,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

async fn browser_login_against(
    response: Option<&[u8]>,
    cancel_exchange: bool,
) -> Result<OAuthTokens, AuthError> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let cfg = OAuthFlowConfig {
        client_id: "test-client".into(),
        client_secret: None,
        authorize_url: format!("{origin}/authorize"),
        token_url: format!("{origin}/token"),
        scopes: vec![],
        redirect_port: 0,
        redirect_path: "/callback".into(),
        extra_authorize_params: BTreeMap::new(),
        extra_token_params: BTreeMap::new(),
        token_content_type: TokenContentType::Form,
        send_state_in_exchange: false,
        device: None,
    };
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let interaction = ScriptedInteraction::new(["test-code"]);
    let cancel = CancellationToken::new();
    let server = async {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = BufReader::new(socket);
        let mut line = String::new();
        socket.read_line(&mut line).await.unwrap();
        assert_eq!(line, "POST /token HTTP/1.1\r\n");
        let mut content_length = None;
        loop {
            line.clear();
            assert_ne!(socket.read_line(&mut line).await.unwrap(), 0);
            if line == "\r\n" {
                break;
            }
            if let Some(length) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = Some(length.trim().parse::<usize>().unwrap());
            }
        }
        let mut body = vec![0; content_length.expect("token request has a body")];
        socket.read_exact(&mut body).await.unwrap();
        let params: BTreeMap<_, _> = url::form_urlencoded::parse(&body).into_owned().collect();
        assert_eq!(params["grant_type"], "authorization_code");
        assert_eq!(params["code"], "test-code");
        assert_eq!(params["client_id"], "test-client");
        assert!(!params["code_verifier"].is_empty());

        if let Some(response) = response {
            socket.get_mut().write_all(response).await.unwrap();
            if cancel_exchange {
                // Let the login enter the body read before simulating a user cancellation.
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        if cancel_exchange {
            cancel.cancel();
            // Keep the incomplete response open until cancellation closes the client.
            let mut byte = [0];
            assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
        }
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(login_browser(&cfg, &http, &interaction, &cancel), server)
    })
    .await
    .expect("browser login must finish promptly without the HTTP timeout");
    result
}

#[tokio::test]
async fn browser_login_cancels_stalled_token_body_after_headers() {
    let result = browser_login_against(
        Some(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"access_token\":"),
        true,
    )
    .await;
    assert!(matches!(result, Err(AuthError::Cancelled)), "{result:?}");
}

#[tokio::test]
async fn browser_login_cancels_while_waiting_for_token_headers() {
    let result = browser_login_against(None, true).await;
    assert!(matches!(result, Err(AuthError::Cancelled)), "{result:?}");
}

#[tokio::test]
async fn browser_login_returns_tokens_when_not_cancelled() {
    let body = r#"{"access_token":"access","refresh_token":"refresh","expires_in":3600}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let tokens = browser_login_against(Some(response.as_bytes()), false)
        .await
        .unwrap();
    assert_eq!(tokens.access, "access");
    assert_eq!(tokens.refresh.as_deref(), Some("refresh"));
    assert_eq!(tokens.expires_in, Some(3600));
}
