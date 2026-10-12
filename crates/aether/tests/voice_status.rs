#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::Duration;

use aether_tui::voice::control::{ControlAction, Listener, control_socket_path};
use tokio::process::Command;

#[tokio::test]
async fn voice_status_json_reports_snapshot_without_toggling_or_starting_daemon() {
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("a.sock");
    let mut listener = Listener::start(control_socket_path(&socket)).await.unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_aether"));
    command
        .args(["voice", "status", "--json"])
        .env("AETHER_SOCKET", &socket)
        .env("AETHER_CONFIG_DIR", root.path().join("config"))
        .env("AETHER_DATA_DIR", root.path().join("data"))
        .env_remove("AETHER_ASCII")
        .env_remove("AETHER_THEME");
    let child = tokio::spawn(async move { command.output().await.unwrap() });
    let request = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request.action(), ControlAction::Status);
    let message = "expired: 120s start window elapsed; last=sent (42s ago)";
    request.ack(true, message).unwrap();
    let output = child.await.unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json, serde_json::json!({"ok": true, "message": message}));
    assert!(!socket.exists(), "status must not start a daemon");
}
