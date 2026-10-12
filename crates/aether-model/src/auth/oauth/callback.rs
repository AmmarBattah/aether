//! A one-shot local HTTP listener that receives the OAuth redirect, plus
//! parsing for a pasted redirect URL or bare code.

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::auth::types::AuthError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackResult {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug)]
pub struct CallbackServer {
    listener: TcpListener,
    path: String,
    port: u16,
}

impl CallbackServer {
    /// Bind `127.0.0.1:port`. Port 0 picks a free port (only usable when the
    /// provider accepts any localhost redirect).
    pub async fn bind(port: u16, path: &str) -> Result<Self, AuthError> {
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            AuthError::Flow(format!(
                "cannot listen on {addr} for the OAuth redirect: {e}"
            ))
        })?;
        let port = listener
            .local_addr()
            .map_err(|e| AuthError::Flow(e.to_string()))?
            .port();
        Ok(Self {
            listener,
            path: path.to_owned(),
            port,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://localhost:{}{}", self.port, self.path)
    }

    /// Serve until a request on the callback path carries a code. Requests on
    /// other paths get a 404 and are ignored; an `error` parameter or a state
    /// mismatch ends the wait with an error.
    pub async fn wait_for_code(
        &self,
        expected_state: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<CallbackResult, AuthError> {
        loop {
            let (stream, _) = tokio::select! {
                accepted = self.listener.accept() => accepted.map_err(|e| AuthError::Flow(format!("accept: {e}")))?,
                _ = cancel.cancelled() => return Err(AuthError::Cancelled),
            };
            match self.handle(stream, expected_state).await {
                Ok(Some(result)) => return Ok(result),
                Ok(None) => continue,
                Err(e) => return Err(e),
            }
        }
    }

    async fn handle(
        &self,
        mut stream: TcpStream,
        expected_state: Option<&str>,
    ) -> Result<Option<CallbackResult>, AuthError> {
        let mut buf = vec![0u8; 16 * 1024];
        let mut read = 0;
        // Only the request line matters; read until the headers end or the buffer fills.
        loop {
            let n = stream
                .read(&mut buf[read..])
                .await
                .map_err(|e| AuthError::Flow(format!("read: {e}")))?;
            if n == 0 {
                break;
            }
            read += n;
            if buf[..read].windows(4).any(|w| w == b"\r\n\r\n") || read == buf.len() {
                break;
            }
        }
        let request = String::from_utf8_lossy(&buf[..read]);
        let target = request
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("/");
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if path != self.path {
            respond(
                &mut stream,
                404,
                &page("Not found", "This is not the callback route."),
            )
            .await;
            return Ok(None);
        }
        let params: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
        let get = |k: &str| {
            params
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        if let Some(error) = get("error") {
            let desc = get("error_description").unwrap_or_default();
            respond(
                &mut stream,
                400,
                &page("Login failed", &format!("{error} {desc}")),
            )
            .await;
            return Err(AuthError::Flow(format!(
                "authorization server returned {error} {desc}"
            )));
        }
        let Some(code) = get("code") else {
            respond(
                &mut stream,
                400,
                &page("Missing code", "The redirect did not include a code."),
            )
            .await;
            return Ok(None);
        };
        let state = get("state");
        if let Some(expected) = expected_state
            && state.as_deref() != Some(expected)
        {
            respond(
                &mut stream,
                400,
                &page("State mismatch", "Please restart the login."),
            )
            .await;
            return Err(AuthError::Flow("OAuth state mismatch".into()));
        }
        respond(
            &mut stream,
            200,
            &page(
                "Signed in",
                "Aether received the authorization. You can close this tab.",
            ),
        )
        .await;
        Ok(Some(CallbackResult { code, state }))
    }
}

async fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn page(title: &str, message: &str) -> String {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    };
    let title = escape(title);
    let message = escape(message);
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{font-family:-apple-system,system-ui,sans-serif;background:#0b0c10;color:#e6e6e6;\
         display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
         main{{text-align:center}}h1{{font-weight:500}}p{{color:#9aa0a6}}</style></head>\
         <body><main><h1>{title}</h1><p>{message}</p></main></body></html>"
    )
}

/// Accept a full redirect URL, `code#state`, `code=...&state=...`, or a bare code.
pub fn parse_manual_input(input: &str) -> CallbackResult {
    let value = input.trim();
    if let Ok(url) = url::Url::parse(value) {
        let mut code = None;
        let mut state = None;
        for (k, v) in url.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(v.into_owned()),
                "state" => state = Some(v.into_owned()),
                _ => {}
            }
        }
        if let Some(code) = code {
            return CallbackResult { code, state };
        }
    }
    if let Some((code, state)) = value.split_once('#') {
        return CallbackResult {
            code: code.to_owned(),
            state: Some(state.to_owned()),
        };
    }
    if value.contains("code=") {
        let mut code = None;
        let mut state = None;
        for (k, v) in url::form_urlencoded::parse(value.as_bytes()) {
            match k.as_ref() {
                "code" => code = Some(v.into_owned()),
                "state" => state = Some(v.into_owned()),
                _ => {}
            }
        }
        if let Some(code) = code {
            return CallbackResult { code, state };
        }
    }
    CallbackResult {
        code: value.to_owned(),
        state: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_input_forms() {
        let r = parse_manual_input("http://localhost:1455/auth/callback?code=abc&state=xyz");
        assert_eq!(
            r,
            CallbackResult {
                code: "abc".into(),
                state: Some("xyz".into())
            }
        );
        assert_eq!(
            parse_manual_input("abc#xyz"),
            CallbackResult {
                code: "abc".into(),
                state: Some("xyz".into())
            }
        );
        assert_eq!(
            parse_manual_input("code=abc&state=s"),
            CallbackResult {
                code: "abc".into(),
                state: Some("s".into())
            }
        );
        assert_eq!(
            parse_manual_input("  plaincode "),
            CallbackResult {
                code: "plaincode".into(),
                state: None
            }
        );
    }

    #[tokio::test]
    async fn receives_code_and_ignores_other_paths() {
        let server = CallbackServer::bind(0, "/cb").await.expect("bind");
        let port = server.port();
        let cancel = CancellationToken::new();
        let client = tokio::spawn(async move {
            let mut s = TcpStream::connect(("127.0.0.1", port))
                .await
                .expect("connect");
            s.write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n")
                .await
                .expect("write");
            let mut out = Vec::new();
            s.read_to_end(&mut out).await.expect("read");
            assert!(String::from_utf8_lossy(&out).starts_with("HTTP/1.1 404"));

            let mut s = TcpStream::connect(("127.0.0.1", port))
                .await
                .expect("connect");
            s.write_all(b"GET /cb?code=c0de&state=st HTTP/1.1\r\nHost: x\r\n\r\n")
                .await
                .expect("write");
            let mut out = Vec::new();
            s.read_to_end(&mut out).await.expect("read");
            assert!(String::from_utf8_lossy(&out).starts_with("HTTP/1.1 200"));
        });
        let result = server
            .wait_for_code(Some("st"), &cancel)
            .await
            .expect("code");
        assert_eq!(
            result,
            CallbackResult {
                code: "c0de".into(),
                state: Some("st".into())
            }
        );
        client.await.expect("client");
    }

    #[tokio::test]
    async fn state_mismatch_is_an_error() {
        let server = CallbackServer::bind(0, "/cb").await.expect("bind");
        let port = server.port();
        let cancel = CancellationToken::new();
        tokio::spawn(async move {
            let mut s = TcpStream::connect(("127.0.0.1", port))
                .await
                .expect("connect");
            s.write_all(b"GET /cb?code=c&state=wrong HTTP/1.1\r\n\r\n")
                .await
                .expect("write");
            let mut out = Vec::new();
            let _ = s.read_to_end(&mut out).await;
        });
        let err = server
            .wait_for_code(Some("right"), &cancel)
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Flow(m) if m.contains("state")));
    }

    #[test]
    fn page_escapes_title_and_provider_error_details() {
        let html = page("<title>&", r#"<script>alert("x")</script>&'"#);
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<title><title>"));
        assert!(html.contains("&lt;title&gt;&amp;"));
        assert!(html.contains("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;&amp;&#39;"));
    }
}
