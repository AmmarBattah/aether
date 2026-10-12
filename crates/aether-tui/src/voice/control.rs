//! A private, same-user Unix socket that lets a short desktop-hotkey process
//! (`aether voice reply`/`cancel`) ask an already-running TUI to act. Aether
//! installs no global key hook; the operator's desktop shortcut runs that
//! binary, which connects here.
//!
//! Only [`ControlAction`] crosses the wire, never audio, a transcript, or a
//! secret. Every connection must share this process's uid; the socket and
//! its directory are owner-only and only ever removed by whoever created them.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

mod ownership;
use ownership::{FileId, acquire_and_bind, file_identity};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// What a control client is asking the running TUI to do. No payload beyond
/// the action itself: this socket never carries audio, a transcript or a
/// secret in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlAction {
    /// Toggle the global voice reply: start recording, then finish it.
    Reply,
    /// Inspect eligibility and lifecycle without starting or stopping audio.
    Status,
    /// Discard the in-progress global voice reply.
    Cancel,
}

/// The result of handling a [`ControlAction`]; `message` is a short status,
/// never audio text.
#[derive(Debug)]
struct ControlAck {
    ok: bool,
    message: String,
}

/// A no-op "already gone" error to hand back from a `oneshot` send that
/// found the connection's own task gone.
#[derive(Debug)]
pub struct PeerGone;

impl std::fmt::Display for PeerGone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the voice control peer already gave up waiting for this ack")
    }
}

impl std::error::Error for PeerGone {}

/// One accepted, peer-verified request. The connection lives in a background
/// task, so `ack` is a plain synchronous channel send that never blocks.
#[derive(Debug)]
pub struct ControlRequest {
    action: ControlAction,
    ack: oneshot::Sender<ControlAck>,
}

impl ControlRequest {
    pub fn action(&self) -> ControlAction {
        self.action
    }

    /// True while the peer is still waiting for an ack. Check this right
    /// before acting (e.g. starting the microphone) to skip a stale request.
    pub fn is_live(&self) -> bool {
        !self.ack.is_closed()
    }

    /// Hands the result to the connection's own task, which writes it back
    /// with a bounded timeout. `Err(PeerGone)` means the peer already gave
    /// up or disconnected; there is nothing further to clean up.
    pub fn ack(self, ok: bool, message: impl Into<String>) -> Result<(), PeerGone> {
        let message = bound_message(message.into());
        self.ack
            .send(ControlAck { ok, message })
            .map_err(|_| PeerGone)
    }
}

/// The listener half: bind once, then pull requests out with [`Self::accept`].
/// Dropping this signals (never forces) the background task that owns the
/// raw socket and its lock to tear itself down (see `run`); `start` briefly
/// retries its own lock acquisition to smooth over a same-process `off` then
/// `on` racing that teardown.
#[derive(Debug)]
pub struct Listener {
    shutdown: CancellationToken,
    requests: mpsc::Receiver<ControlRequest>,
}

/// Longest accepted line in either direction. Requests are one word; acks
/// are one short status sentence - both fit with room to spare.
const MAX_LINE_BYTES: usize = 512;
/// Longest ack message kept after sanitizing; the rest is dropped, not an error.
const MAX_ACK_MESSAGE_BYTES: usize = 200;
/// Concurrent in-flight connections; a hotkey is one person pressing one
/// key, so this only bounds a flood, not normal use.
const MAX_INFLIGHT: usize = 4;
/// Requests queued for the caller to `accept`; kept small so a slow caller
/// naturally back-pressures new connections instead of piling up actions.
const REQUEST_QUEUE: usize = 4;
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const ACK_TIMEOUT: Duration = Duration::from_secs(5);

impl Listener {
    /// Binds `socket`: an owner-only directory and socket file (0700/0600),
    /// an exclusive lock so a second TUI cannot steal the endpoint, and
    /// stale-socket cleanup that only ever removes a socket this same user
    /// owns, and only while holding that lock.
    pub async fn start(socket: PathBuf) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !socket.as_os_str().is_empty(),
            "the voice control socket path is empty"
        );
        let (lock, listener) = acquire_and_bind(&socket).await?;
        let bound =
            file_identity(&socket).context("inspecting the freshly bound voice control socket")?;
        let (tx, requests) = mpsc::channel(REQUEST_QUEUE);
        let shutdown = CancellationToken::new();
        // Not kept: nothing here ever awaits or aborts it (see `Drop`), so
        // holding the handle would only be an unused field.
        tokio::spawn(run(listener, lock, socket, bound, tx, shutdown.clone()));
        Ok(Self { shutdown, requests })
    }

    /// Cancellation-safe (`mpsc::Receiver::recv`), so it is safe as a
    /// `tokio::select!` branch. Malformed, oversized, wrong-uid and
    /// timed-out connections are handled internally and never reach here.
    pub async fn accept(&mut self) -> anyhow::Result<ControlRequest> {
        self.requests
            .recv()
            .await
            .context("the voice control listener has stopped accepting connections")
    }
}

impl Drop for Listener {
    /// Cancellation only, never `abort`: every step `run`'s own teardown
    /// takes (draining connections, then dropping the listener before the
    /// lock) is itself bounded, and an abort could strike mid-teardown and
    /// release the lock before the socket - the exact ordering bug this
    /// avoids. `run`'s task (not tracked here, see `start`) keeps making
    /// progress on the runtime independently until that bounded teardown
    /// ends, whether or not this `Listener` still exists to observe it.
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

/// Where the control endpoint for a running TUI lives: a dedicated
/// `<daemon socket filename>.voice/control.sock` next to the (already
/// session/config-resolved) daemon socket, so a hotkey pointed at a specific
/// `--socket`/`AETHER_SOCKET` reaches that TUI and no other, with no separate
/// config knob. Using the whole filename (not just its extension) keeps two
/// differently-named daemon sockets in the same directory from colliding,
/// and the dedicated directory means we never touch permissions on the
/// daemon socket's own (possibly shared, operator-chosen) parent directory.
pub fn control_socket_path(daemon_socket: &Path) -> PathBuf {
    let mut dir_name = daemon_socket.file_name().unwrap_or_default().to_owned();
    dir_name.push(".voice");
    daemon_socket.with_file_name(dir_name).join("control.sock")
}

/// Send one `action` to the control endpoint at `socket` and wait for its
/// ack, each half bounded by its own timeout. Used by `aether voice
/// reply`/`cancel`; kept here so the wire format has one owner.
pub async fn send_request(
    socket: &Path,
    action: ControlAction,
    connect_timeout: Duration,
    roundtrip_timeout: Duration,
) -> anyhow::Result<(bool, String)> {
    let mut stream = tokio::time::timeout(connect_timeout, UnixStream::connect(socket))
        .await
        .with_context(|| format!("timed out connecting to {}", socket.display()))?
        .with_context(|| {
            format!(
                "no running TUI is listening for voice control at {} (TUI closed, replies disabled, or endpoint unavailable; inspect /voice status in the matching TUI)",
                socket.display()
            )
        })?;
    // Defense in depth: the socket's own permissions already keep out other
    // uids, but this confirms the process on the other end is genuinely ours.
    verify_peer(&stream).context("the voice control endpoint failed its peer check")?;
    tokio::time::timeout(roundtrip_timeout, async {
        let mut line = serde_json::to_string(&WireRequest { action })
            .context("encoding the voice control request")?;
        line.push('\n');
        stream
            .write_all(line.as_bytes())
            .await
            .context("sending the voice control request")?;
        let reply = read_bounded_line(&mut stream, MAX_LINE_BYTES)
            .await
            .context("reading the voice control response")?;
        let wire: WireAck =
            serde_json::from_str(&reply).context("malformed voice control response")?;
        if action == ControlAction::Status && !wire.ok && wire.message == "malformed voice control request" {
            anyhow::bail!("running TUI does not support voice status; restart it with the updated aether binary (nothing recorded or sent)");
        }
        Ok::<_, anyhow::Error>((wire.ok, bound_message(wire.message)))
    })
    .await
    .context("timed out waiting for the running TUI to respond")?
}

#[derive(Serialize, Deserialize)]
struct WireRequest {
    action: ControlAction,
}

#[derive(Serialize, Deserialize)]
struct WireAck {
    ok: bool,
    message: String,
}

/// Sanitized and length-bounded, so a runner cannot accidentally leak
/// terminal control bytes or an oversized status through the ack.
fn bound_message(text: String) -> String {
    let mut sanitized = aether_core::text::strip_controls(&text).into_owned();
    if sanitized.len() > MAX_ACK_MESSAGE_BYTES {
        let mut end = MAX_ACK_MESSAGE_BYTES;
        while !sanitized.is_char_boundary(end) {
            end -= 1;
        }
        sanitized.truncate(end);
    }
    sanitized
}

/// One line, byte-at-a-time so the bound is enforced during the read, not
/// after: a peer that never sends `\n` cannot grow this past `max`. Requests
/// and acks are tiny and rare (one hotkey press), so the extra syscalls cost
/// nothing that matters.
async fn read_bounded_line(stream: &mut UnixStream, max: usize) -> anyhow::Result<String> {
    let mut buf = Vec::with_capacity(64.min(max));
    let mut byte = [0u8; 1];
    loop {
        let n = stream
            .read(&mut byte)
            .await
            .context("reading a line from the voice control socket")?;
        anyhow::ensure!(n != 0, "connection closed before sending a full line");
        if byte[0] == b'\n' {
            break;
        }
        buf.push(byte[0]);
        anyhow::ensure!(buf.len() <= max, "line exceeds {max} bytes");
    }
    String::from_utf8(buf).context("line must be UTF-8")
}

fn verify_peer(stream: &UnixStream) -> anyhow::Result<()> {
    let cred = stream
        .peer_cred()
        .context("reading the voice control peer's credentials")?;
    let ours = nix::unistd::getuid().as_raw();
    anyhow::ensure!(
        cred.uid() == ours,
        "voice control peer uid {} does not match ours ({ours})",
        cred.uid()
    );
    Ok(())
}

async fn read_request(stream: &mut UnixStream) -> anyhow::Result<ControlAction> {
    verify_peer(stream)?;
    let line = read_bounded_line(stream, MAX_LINE_BYTES).await?;
    let wire: WireRequest =
        serde_json::from_str(&line).context("malformed voice control request")?;
    Ok(wire.action)
}

async fn write_ack(stream: &mut UnixStream, ack: &ControlAck) -> anyhow::Result<()> {
    let mut line = serde_json::to_string(&WireAck {
        ok: ack.ok,
        message: ack.message.clone(),
    })
    .context("encoding the voice control ack")?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .await
        .context("writing the voice control ack")?;
    stream
        .shutdown()
        .await
        .context("closing the voice control connection")
}

/// Handle exactly one connection: read its request, hand the action to the
/// listener's caller, then wait (bounded) for that caller's ack and write
/// it back. Every path out of this function is bounded by a timeout or by
/// `shutdown`, so one bad or silent peer can only ever hold one of the
/// [`MAX_INFLIGHT`] permits for a bounded time.
async fn serve_one(
    mut stream: UnixStream,
    tx: mpsc::Sender<ControlRequest>,
    shutdown: CancellationToken,
) {
    let action = tokio::select! {
        _ = shutdown.cancelled() => return,
        result = tokio::time::timeout(READ_TIMEOUT, read_request(&mut stream)) => match result {
            Ok(Ok(action)) => action,
            Ok(Err(reason)) => {
                tracing::debug!(error = %reason, "rejected a voice control connection");
                let ack = ControlAck { ok: false, message: bound_message(reason.to_string()) };
                let _ = tokio::time::timeout(WRITE_TIMEOUT, write_ack(&mut stream, &ack)).await;
                return;
            }
            Err(_) => {
                tracing::debug!("voice control connection timed out reading a request");
                let ack = ControlAck {
                    ok: false,
                    message: "timed out waiting for a request".into(),
                };
                let _ = tokio::time::timeout(WRITE_TIMEOUT, write_ack(&mut stream, &ack)).await;
                return;
            }
        },
    };
    let (ack_tx, ack_rx) = oneshot::channel();
    tokio::select! {
        _ = shutdown.cancelled() => return,
        result = tx.send(ControlRequest { action, ack: ack_tx }) => {
            // Either the listener side is gone, or - on the cancelled
            // branch above - shutdown fired while the queue was full.
            if result.is_err() {
                return;
            }
        }
    }
    let mut probe = [0u8; 1];
    let ack = tokio::select! {
        _ = shutdown.cancelled() => ControlAck { ok: false, message: "voice control is shutting down".into() },
        _ = tokio::time::sleep(ACK_TIMEOUT) => ControlAck { ok: false, message: "timed out waiting for the running TUI to act".into() },
        received = ack_rx => received.unwrap_or(ControlAck {
            ok: false,
            message: "voice control dropped the request".into(),
        }),
        // The peer hung up (or sent unexpected data) before an ack was
        // ready; dropping `ack_rx` here closes `ControlRequest::is_live`
        // immediately instead of the caller waiting out the full timeout.
        _ = stream.read(&mut probe) => return,
    };
    // Best-effort: a peer that already hung up loses nothing it needed.
    let _ = tokio::time::timeout(WRITE_TIMEOUT, write_ack(&mut stream, &ack)).await;
}

/// The background task that owns the raw listener and its exclusive lock
/// for its entire life, so releasing them is tied to this task's own frame
/// teardown - reached only by observing `shutdown`, never by an outside
/// `abort` - so a caller can never see it skipped. A racing `Listener::start`
/// only ever sees this endpoint as free once this exact task is gone.
///
/// The socket path is removed only here, and only after confirming (`bound`
/// vs. the path's live identity) nobody has replaced it; a task that never
/// reaches this teardown - the whole runtime exiting first - leaves the
/// socket for the next `start` to detect as stale and clean up itself.
async fn run(
    listener: UnixListener,
    lock: std::fs::File,
    socket: PathBuf,
    bound: FileId,
    tx: mpsc::Sender<ControlRequest>,
    shutdown: CancellationToken,
) {
    let permits = Arc::new(Semaphore::new(MAX_INFLIGHT));
    let mut connections = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                        // Already at MAX_INFLIGHT: drop rather than queue unbounded.
                        continue;
                    };
                    let tx = tx.clone();
                    let child_shutdown = shutdown.clone();
                    connections.spawn(async move {
                        let _permit = permit;
                        serve_one(stream, tx, child_shutdown).await;
                    });
                }
                Err(e) => {
                    // Not known to be transient on this platform: stop rather
                    // than spin. Dropping `tx` closes the request channel, so
                    // the caller's next `accept` sees the listener is gone.
                    tracing::error!(error = %e, "voice control listener failed; disabling it");
                    break;
                }
            },
            // Reaps finished connection tasks as they complete; without this
            // branch a long-lived TUI would grow the `JoinSet`'s bookkeeping
            // by one entry per connection for as long as it runs.
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(e)) = result {
                    tracing::debug!(error = %e, "a voice control connection task ended abnormally");
                }
            }
        }
    }
    drop(tx);
    // Aborts every still-running per-connection task and waits for it to
    // finish, so none can outlive this task or touch `socket` after us.
    connections.shutdown().await;
    if file_identity(&socket).ok() == Some(bound) {
        let _ = std::fs::remove_file(&socket);
    }
    // Close the socket fd before releasing the lock: a racing `start` must
    // never be able to acquire the lock while this endpoint could still look
    // bound (e.g. if the `remove_file` above raced or failed).
    drop(listener);
    drop(lock);
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
