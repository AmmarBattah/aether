//! Fake-socket tests for `aether voice reply`/`cancel`: a real `Listener`
//! from `aether_tui::voice::control` in a tempdir, no real hotkey or TUI.

use aether_tui::voice::control::{ControlAction, Listener, control_socket_path};

use super::*;

fn daemon_socket_in(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("a.sock")
}

#[tokio::test]
async fn reply_reaches_a_running_tui_and_reports_its_ack() {
    let dir = tempfile::tempdir().expect("tempdir");
    let daemon_socket = daemon_socket_in(&dir);
    let mut listener = Listener::start(control_socket_path(&daemon_socket))
        .await
        .expect("bind");

    let client = tokio::spawn(reply(daemon_socket, false));

    let request = listener.accept().await.expect("accepted");
    assert_eq!(request.action(), ControlAction::Reply);
    request
        .ack(true, "recording started")
        .expect("ack delivered");

    client
        .await
        .expect("client task")
        .expect("a successful ack must not fail the command");
}

#[tokio::test]
async fn cancel_sends_the_cancel_action() {
    let dir = tempfile::tempdir().expect("tempdir");
    let daemon_socket = daemon_socket_in(&dir);
    let mut listener = Listener::start(control_socket_path(&daemon_socket))
        .await
        .expect("bind");

    let client = tokio::spawn(cancel(daemon_socket, false));

    let request = listener.accept().await.expect("accepted");
    assert_eq!(request.action(), ControlAction::Cancel);
    request.ack(true, "cancelled").expect("ack delivered");

    client.await.expect("client task").expect("no error");
}

/// A `ok: false` ack (voice replies off, nothing to cancel, ...) must fail
/// the command with that exact reason, not print it as if it succeeded.
#[tokio::test]
async fn a_failed_ack_fails_the_command_with_its_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let daemon_socket = daemon_socket_in(&dir);
    let mut listener = Listener::start(control_socket_path(&daemon_socket))
        .await
        .expect("bind");

    let client = tokio::spawn(cancel(daemon_socket, false));

    let request = listener.accept().await.expect("accepted");
    request
        .ack(false, "no recording to cancel")
        .expect("ack delivered");

    let error = client
        .await
        .expect("client task")
        .expect_err("a failed ack must fail the command");
    assert_eq!(error.to_string(), "no recording to cancel");
}

#[tokio::test]
async fn reply_reports_a_clear_error_with_no_running_tui() {
    let dir = tempfile::tempdir().expect("tempdir");
    let daemon_socket = daemon_socket_in(&dir);

    let error = reply(daemon_socket, false)
        .await
        .expect_err("nothing is listening");
    assert!(format!("{error:#}").contains("no running TUI"), "{error:#}");
}

#[tokio::test]
async fn an_empty_daemon_socket_path_is_rejected_before_connecting() {
    let error = reply(PathBuf::new(), false)
        .await
        .expect_err("an empty socket path is never valid");
    assert!(error.to_string().contains("empty"), "{error}");
}

#[tokio::test]
async fn status_sends_only_status_and_supports_json_for_ineligible_snapshot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let daemon_socket = daemon_socket_in(&dir);
    let mut listener = Listener::start(control_socket_path(&daemon_socket))
        .await
        .expect("bind");
    let client = tokio::spawn(status(daemon_socket, true));
    let request = listener.accept().await.expect("accepted");
    assert_eq!(request.action(), ControlAction::Status);
    request
        .ack(true, "no_pending_question: no pending ask_user")
        .expect("ack");
    client
        .await
        .expect("task")
        .expect("ineligible status is a successful query");
}

#[tokio::test]
async fn status_explains_older_tui_protocol_rejection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let daemon_socket = daemon_socket_in(&dir);
    let mut listener = Listener::start(control_socket_path(&daemon_socket))
        .await
        .expect("bind");
    let client = tokio::spawn(status(daemon_socket, false));
    let request = listener.accept().await.expect("accepted");
    request
        .ack(false, "malformed voice control request")
        .expect("ack");
    let error = client
        .await
        .expect("task")
        .expect_err("older server rejected action");
    assert!(format!("{error:#}").contains("restart it with the updated aether binary"));
}

#[tokio::test]
async fn status_without_listener_never_enables_voice() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = daemon_socket_in(&dir);
    let error = status(socket.clone(), false)
        .await
        .expect_err("no listener");
    assert!(format!("{error:#}").contains("inspect /voice status"));
    assert!(!control_socket_path(&socket).exists());
}
