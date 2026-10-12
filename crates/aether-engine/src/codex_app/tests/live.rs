use super::*;

/// Live, opt-in integration probe: drives the real `codex app-server`
/// binary on PATH (needs the host's own Codex auth, e.g. `~/.codex/auth.json`)
/// with web search enabled in the host's native configuration.
/// Asserts a real `web_search` tool item and
/// a real `web_fetch`-equivalent (item action fetching a URL) both fire.
/// `cargo test -p aether-engine --lib codex_app::tests::live::live_app_server_searches_and_fetches -- --ignored --nocapture`
#[tokio::test]
#[ignore = "needs network + a locally authenticated `codex` CLI on PATH"]
async fn live_app_server_searches_and_fetches() {
    let engine = CodexAppEngine::detect_verbose()
        .await
        .expect("a real `codex` CLI with app-server support must be on PATH for this probe");
    let cwd = tempfile::tempdir().expect("temp cwd");
    let mut t = turn(None, false);
    t.cwd = cwd.path().to_path_buf();
    t.mcp = None;
    t.prompt = "Use web search to find the official Rust programming language homepage, \
                then fetch that exact URL directly and quote its page title back to me."
        .into();
    let mut run = engine
        .run(t, CancellationToken::new())
        .await
        .expect("app-server must start");
    let mut saw_web_search_tool_use = false;
    let mut saw_web_search_result = false;
    let mut assistant_text = String::new();
    while let Some(event) = run.events.recv().await {
        match event {
            EngineEvent::Assistant { content, .. } => {
                for block in &content {
                    if let ContentBlock::ToolUse { name, .. } = block
                        && name == "web_search"
                    {
                        saw_web_search_tool_use = true;
                    }
                }
                assistant_text.push_str(&aether_core::model::text_of(&content));
            }
            EngineEvent::ToolResult { content, .. } if content.ends_with("results") => {
                saw_web_search_result = true;
            }
            _ => {}
        }
    }
    assert!(
        saw_web_search_tool_use,
        "expected a real `web_search` tool-use item from the live app-server run"
    );
    assert!(
        saw_web_search_result,
        "expected a completed web_search result item"
    );
    // The engine's own stderr/stdout was not captured here; a failure
    // here means the probe prompt or Codex's own web tooling changed
    // shape, not that this override regressed - rerun with --nocapture.
    assert!(
        assistant_text.to_lowercase().contains("rust"),
        "expected the final answer to reflect the live-fetched page, got: {assistant_text}"
    );
}

/// Live, opt-in probe for network-denied turns: fresh and resumed real turns must serve
/// HTTP on 127.0.0.1:4173 and reach ::1, while egress, DNS, vsock and host unix sockets fail.
/// `cargo test -p aether-engine --lib codex_app::tests::live::live_network_denied_turn_reaches_only_loopback -- --ignored --nocapture`
#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "needs an authenticated `codex` CLI plus bwrap and socat on PATH"]
async fn live_network_denied_turn_reaches_only_loopback() {
    const PROBE: &str = r#"import http.client, http.server, json, os, socket, threading
r = {}
try:
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 4173), http.server.SimpleHTTPRequestHandler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    c = http.client.HTTPConnection("127.0.0.1", 4173, timeout=5); c.request("GET", "/"); r["http"] = c.getresponse().status
except OSError as e: r["http"] = repr(e)
try:
    s = socket.socket(socket.AF_INET6); s.bind(("::1", 0)); s.listen(1)
    c6 = socket.create_connection(("::1", s.getsockname()[1]), timeout=3); a, _ = s.accept(); c6.sendall(b"x")
    r["ipv6"] = a.recv(1) == b"x"
except OSError as e: r["ipv6"] = repr(e)
try: socket.create_connection(("1.1.1.1", 443), timeout=5); r["egress"] = "connected"
except OSError as e: r["egress"] = repr(e)
try: r["dns"] = repr(socket.getaddrinfo("example.com", 443)[0][4])
except OSError as e: r["dns"] = repr(e)
try: socket.socket(socket.AF_UNIX).connect("/run/user/%d/bus" % os.getuid()); r["unix"] = "connected"
except OSError as e: r["unix"] = repr(e)
try:
    a, b = socket.socketpair(socket.AF_UNIX, socket.SOCK_DGRAM); a.bind("sandbox-dgram.sock")
    a.connect("host-proxy.sock"); a.send(b"proxy"); r["dgram_pair"] = "sent"
except OSError as e: r["dgram_pair"] = repr(e)
try: socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM); r["vsock"] = "created"
except OSError as e: r["vsock"] = repr(e)
json.dump(r, open("loopback_probe.json", "w"))
"#;
    let engine = CodexAppEngine::detect_verbose()
        .await
        .expect("a real `codex` CLI with app-server support must be on PATH for this probe");
    let cwd = tempfile::tempdir().expect("temp cwd");
    std::fs::write(cwd.path().join("loopback_probe.py"), PROBE).unwrap();
    // A stand-in for a host proxy that a datagram pair could reconnect to.
    let proxy = std::os::unix::net::UnixDatagram::bind(cwd.path().join("host-proxy.sock")).unwrap();
    proxy.set_nonblocking(true).unwrap();
    // A fresh thread confirms the sandbox at thread/start; a resumed one via thread/read.
    let mut session = None;
    for round in ["fresh", "resumed"] {
        let report = cwd.path().join("loopback_probe.json");
        let _ = std::fs::remove_file(&report);
        session = Some(run_offline_probe(&engine, cwd.path(), session).await);
        let report = std::fs::read_to_string(&report)
            .unwrap_or_else(|e| panic!("{round}: the probe must have written its report: {e}"));
        println!("{round} loopback probe: {report}");
        let report: Value = serde_json::from_str(&report).unwrap();
        assert_eq!(report["http"], 200, "{round}: {report}");
        assert_eq!(report["ipv6"], true, "{round}: {report}");
        let egress = report["egress"].as_str().unwrap();
        assert!(
            egress.contains("Error"),
            "{round}: egress must fail: {report}"
        );
        let dns = report["dns"].as_str().unwrap();
        assert!(dns.contains("gaierror"), "{round}: DNS must fail: {report}");
        for probe in ["unix", "dgram_pair", "vsock"] {
            let outcome = report[probe].as_str().unwrap();
            assert!(
                outcome.contains("PermissionError"),
                "{round}: {probe} must fail: {report}"
            );
        }
        let received = proxy.recv(&mut [0; 64]);
        assert!(
            received
                .as_ref()
                .is_err_and(|e| e.kind() == std::io::ErrorKind::WouldBlock),
            "{round}: the host proxy received {received:?}"
        );
    }
}

/// One network-denied turn that runs the probe; returns its thread id.
#[cfg(target_os = "linux")]
async fn run_offline_probe(
    engine: &CodexAppEngine,
    cwd: &std::path::Path,
    session: Option<String>,
) -> String {
    let mut t = turn(None, false);
    t.session = session;
    t.cwd = cwd.to_path_buf();
    t.extra_dirs.clear();
    t.mcp = None;
    t.network_access = false;
    t.model = None;
    t.prompt = "Run exactly `python3 loopback_probe.py` once with your shell tool, without \
                requesting escalation or extra permissions, then reply DONE."
        .into();
    let mut run = engine
        .run(t, CancellationToken::new())
        .await
        .expect("the loopback sandbox and app-server must start");
    let decisions = run.decisions.take().unwrap();
    let (mut outcome, mut thread) = (None, None);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(300);
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, run.events.recv()).await {
        match event {
            EngineEvent::Permission { request_id, .. } => {
                let deny = PermissionDecision {
                    request_id,
                    allow: false,
                    message: Some("stay sandboxed".into()),
                    answer: None,
                };
                decisions.send(deny).await.unwrap();
            }
            EngineEvent::Session { id, .. } => thread = Some(id),
            EngineEvent::Done(done) => outcome = Some(done),
            _ => {}
        }
    }
    let outcome = outcome.expect("the turn must finish within the deadline");
    assert!(!outcome.is_error, "turn failed: {:?}", outcome.error);
    thread.expect("the turn must report its thread")
}

/// Live, opt-in probe: a thread started network-denied and resumed network-allowed must
/// not keep believing it is offline, and a later denied turn must believe it again.
/// `cargo test -p aether-engine --lib codex_app::tests::live::live_network_note_follows_each_turn -- --ignored --nocapture`
#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "needs an authenticated `codex` CLI plus bwrap and socat on PATH"]
async fn live_network_note_follows_each_turn() {
    let engine = CodexAppEngine::detect_verbose()
        .await
        .expect("a real `codex` CLI with app-server support must be on PATH for this probe");
    let cwd = tempfile::tempdir().expect("temp cwd");
    let mut session = None;
    for (network_access, expected) in [(false, "NO"), (true, "YES"), (true, "YES"), (false, "NO")] {
        let mut t = turn(None, false);
        t.session = session.clone();
        t.cwd = cwd.path().to_path_buf();
        t.extra_dirs.clear();
        t.mcp = None;
        t.network_access = network_access;
        t.model = None;
        t.prompt = "Without running any command, answer from your instructions only: can \
                    commands in this turn reach external hosts such as example.com? Reply \
                    with exactly YES or NO."
            .into();
        let mut run = engine
            .run(t, CancellationToken::new())
            .await
            .expect("the app-server must start");
        let (mut outcome, mut thread) = (None, None);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(300);
        while let Ok(Some(event)) = tokio::time::timeout_at(deadline, run.events.recv()).await {
            match event {
                EngineEvent::Session { id, .. } => thread = Some(id),
                EngineEvent::Done(done) => outcome = Some(done),
                _ => {}
            }
        }
        let outcome = outcome.expect("the turn must finish within the deadline");
        assert!(!outcome.is_error, "turn failed: {:?}", outcome.error);
        let answer = outcome.text.unwrap_or_default();
        println!("network_access={network_access}: {answer}");
        assert_eq!(
            answer.trim().trim_matches(['`', '*', '.']),
            expected,
            "network_access={network_access}"
        );
        assert!(
            session.is_none() || session == thread,
            "the thread must be resumed"
        );
        session = thread;
    }
}
