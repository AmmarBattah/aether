#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod support;

use aether_protocol::PROTOCOL_VERSION;
use serde_json::json;
use support::{RawConn, TestDaemon};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn daemon_concurrency_uses_cli_then_file_then_default() {
    for (configured, override_limit, expected) in [
        (None, None, 32),
        (Some(64), None, 64),
        (Some(64), Some(12), 12),
    ] {
        let daemon = TestDaemon::start_with_concurrency(configured, override_limit).await;
        let mut conn = RawConn::connect(&daemon.socket).await.expect("connect");
        conn.hello(PROTOCOL_VERSION).await.expect("hello");
        let status = conn.call(json!({"cmd": "status"})).await;
        assert_eq!(
            status["result"]["Ok"]["data"]["max_concurrent_turns"], expected,
            "{status}"
        );
        daemon.stop().await;
    }
}

/// The config.toml -> RuntimeConfig -> DaemonStatus seam for the two engine
/// knobs (crates/aether-daemon/src/lib.rs, the `max_concurrent_engine_turns`
/// and `run_budget_usd` reads before `Runtime::start`) has no assertion
/// anywhere else: a typo'd field name there would ship green.
#[tokio::test]
async fn scheduler_engine_cap_and_run_budget_reach_daemon_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let config_dir = root.join("config");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    std::fs::write(
        config_dir.join("config.toml"),
        "[scheduler]\nmax_concurrent_engine_turns = 3\nrun_budget_usd = 7.5\n",
    )
    .expect("write config");
    std::fs::create_dir_all(root.join("workspace")).expect("workspace");

    let socket = root.join("aetherd.sock");
    let config = aether_daemon::DaemonConfig {
        socket: socket.clone(),
        store_path: root.join("aether.db"),
        config_dir: Some(config_dir),
        workspace_root: Some(root.join("workspace")),
        max_concurrent_turns: None,
        yolo: false,
        tcp: None,
        allow_remote: false,
    };
    let shutdown = CancellationToken::new();
    let serving = tokio::spawn(aether_daemon::run(config, shutdown.clone()));

    let mut conn = await_answering(&socket).await;
    let status = conn.call(json!({"cmd": "status"})).await;
    let data = &status["result"]["Ok"]["data"];
    assert_eq!(data["max_concurrent_engine_turns"], 3, "{status}");
    assert_eq!(data["run_budget_usd"], 7.5, "{status}");

    shutdown.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), serving).await;
}

/// Poll for a daemon that answers, not merely for a socket path to exist
/// (mirrors `support::TestDaemon::await_answering`, which is private).
async fn await_answering(socket: &std::path::Path) -> RawConn {
    for _ in 0..200 {
        if let Ok(mut conn) = RawConn::connect(socket).await
            && conn.hello(PROTOCOL_VERSION).await.is_ok()
        {
            return conn;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("daemon never answered on {}", socket.display());
}
