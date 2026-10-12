//! `aether voice reply`/`cancel`: a short-lived client for the control
//! socket in `aether_tui::voice::control`, meant to be bound to a desktop
//! global hotkey. It never talks to the daemon - only to an already-running
//! TUI that opted in with `/voice replies on`.

use std::path::PathBuf;
use std::time::Duration;

use aether_tui::voice::control::{ControlAction, control_socket_path, send_request};
use anyhow::Context as _;

use super::acted;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const ROUNDTRIP_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn status(daemon_socket: PathBuf, json: bool) -> anyhow::Result<()> {
    act(daemon_socket, ControlAction::Status, json).await
}

pub(crate) async fn reply(daemon_socket: PathBuf, json: bool) -> anyhow::Result<()> {
    act(daemon_socket, ControlAction::Reply, json).await
}

pub(crate) async fn cancel(daemon_socket: PathBuf, json: bool) -> anyhow::Result<()> {
    act(daemon_socket, ControlAction::Cancel, json).await
}

/// Send `action` to the control socket derived from `daemon_socket` and
/// report the running TUI's own ack. A `ok: false` ack (voice replies off,
/// nothing to cancel, a second F4 recording in the way, ...) is a real
/// failure, not a shape to print quietly - it fails this command the same
/// way any other rejected request would.
async fn act(daemon_socket: PathBuf, action: ControlAction, json: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        !daemon_socket.as_os_str().is_empty(),
        "the daemon socket path is empty"
    );
    let control_socket = control_socket_path(&daemon_socket);
    let (ok, message) = send_request(&control_socket, action, CONNECT_TIMEOUT, ROUNDTRIP_TIMEOUT)
        .await
        .with_context(|| format!("sending {action:?} to {}", control_socket.display()))?;
    anyhow::ensure!(ok, "{message}");
    acted(
        json,
        &message,
        serde_json::json!({"ok": true, "message": message}),
    )
}

#[cfg(test)]
#[path = "voice_tests.rs"]
mod tests;
