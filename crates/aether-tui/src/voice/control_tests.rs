//! Fake-socket tests: no real hotkey, desktop or second TUI process, just
//! this module's own listener and raw `UnixStream` peers in a tempdir.

use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use super::ownership::lock_path_for;
use super::*;

/// A control socket lives under its own dedicated `<name>.voice` directory,
/// never directly in the (possibly shared) directory the caller's daemon
/// socket lives in.
fn socket_in(dir: &tempfile::TempDir) -> PathBuf {
    control_socket_path(&dir.path().join("a.sock"))
}

/// Pre-creates the dedicated voice directory at the same mode `Listener`
/// would leave it in, simulating what a previous, correctly-behaved run left
/// behind (the directory itself is never removed, only the socket is).
fn precreate_voice_dir(socket: &Path) {
    let dir = socket.parent().expect("control socket has a parent");
    std::fs::create_dir_all(dir).expect("create dedicated voice dir");
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
}

#[tokio::test]
async fn control_socket_path_avoids_extension_collisions_via_a_dedicated_directory() {
    let daemon = PathBuf::from("/run/user/1000/aether/aetherd.sock");
    assert_eq!(
        control_socket_path(&daemon),
        PathBuf::from("/run/user/1000/aether/aetherd.sock.voice/control.sock")
    );
    // Appending just an extension would collide these two: both would lose
    // their own extension and become "aetherd.voice.sock".
    let other = PathBuf::from("/run/user/1000/aether/aetherd.other");
    assert_ne!(control_socket_path(&daemon), control_socket_path(&other));
}

/// The socket, its lock file and its own dedicated directory are all
/// owner-only; the (possibly shared, operator-chosen) directory the daemon
/// socket itself lives in is never touched.
#[tokio::test]
async fn starting_hardens_its_own_directory_without_touching_the_parent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let outer_mode_before = std::fs::metadata(dir.path())
        .expect("outer dir metadata")
        .permissions()
        .mode()
        & 0o777;

    let socket = socket_in(&dir);
    let listener = Listener::start(socket.clone()).await.expect("bind");

    let voice_dir = socket.parent().expect("dedicated voice directory");
    let dir_mode = std::fs::metadata(voice_dir)
        .expect("dir metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700, "{dir_mode:o}");

    let outer_mode_after = std::fs::metadata(dir.path())
        .expect("outer dir metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        outer_mode_before, outer_mode_after,
        "must never chmod the daemon socket's own parent directory"
    );

    let socket_meta = std::fs::metadata(&socket).expect("socket metadata");
    assert!(socket_meta.file_type().is_socket());
    assert_eq!(socket_meta.permissions().mode() & 0o777, 0o600);

    let lock_meta =
        std::fs::metadata(lock_path_for(&socket)).expect("lock metadata exists under the lock");
    assert_eq!(lock_meta.permissions().mode() & 0o777, 0o600);

    drop(listener);
}

/// The core safety property: two listeners never both believe they own the
/// endpoint. A second `start` while the first is alive fails clearly rather
/// than binding over or beside it.
#[tokio::test]
async fn a_second_listener_cannot_steal_a_live_endpoint() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let first = Listener::start(socket.clone()).await.expect("first binds");

    let refused = Listener::start(socket.clone())
        .await
        .expect_err("a second listener must not take the same endpoint");
    assert!(refused.to_string().contains("already holds"), "{refused:#}");

    drop(first);
}

/// Dropping a listener (an `off` toggle, or handing off between two TUI
/// windows on the same socket) frees the endpoint for the next `start`, even
/// though the release happens in the dropped listener's own background task
/// rather than synchronously in `drop`.
#[tokio::test]
async fn dropping_a_listener_frees_the_endpoint_for_a_handoff() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let first = Listener::start(socket.clone()).await.expect("first binds");
    drop(first);

    let second = Listener::start(socket.clone())
        .await
        .expect("the endpoint is free once the old listener is gone");
    drop(second);
}

/// A socket left behind by a listener that never got to clean up (a crash,
/// a `kill -9`) is detected as stale - nothing is listening behind it - and
/// safely replaced, all under this endpoint's own lock.
#[tokio::test]
async fn a_stale_socket_from_a_crashed_listener_is_replaced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    precreate_voice_dir(&socket);
    {
        // Binds the path directly, bypassing our lock/lifecycle entirely -
        // exactly what a listener killed before it could clean up leaves.
        let _leftover = std::os::unix::net::UnixListener::bind(&socket).expect("leftover bind");
        // Dropped here without unlinking: `_leftover` goes out of scope,
        // closing the fd but leaving the socket file on disk.
    }
    assert!(socket.exists(), "the leftover socket file is still there");

    let started = Listener::start(socket.clone())
        .await
        .expect("stale socket is replaced");
    drop(started);
}

/// Something still genuinely answering behind the socket - not merely a
/// leftover file - is a foreign listener under this lock and must not be
/// torn down out from under it.
#[tokio::test]
async fn a_socket_someone_is_still_answering_on_is_never_removed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    precreate_voice_dir(&socket);
    let _live = std::os::unix::net::UnixListener::bind(&socket).expect("a live foreign listener");

    let refused = Listener::start(socket.clone())
        .await
        .expect_err("a socket something is still answering on must not be reused");
    assert!(
        refused.to_string().contains("already listening"),
        "{refused:#}"
    );
    assert!(socket.exists(), "left untouched");
}

/// A non-socket entry at the configured path (a stray file, or what a
/// symlink attack would leave) is refused outright, never removed or bound
/// over: `remove_file` would only ever delete that path's own directory
/// entry, but this rejects the whole situation before it matters.
#[tokio::test]
async fn a_non_socket_entry_at_the_path_is_refused_not_removed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    precreate_voice_dir(&socket);
    std::fs::write(&socket, b"not a socket").expect("plain file");

    let refused = Listener::start(socket.clone())
        .await
        .expect_err("a regular file must not be mistaken for a stale socket");
    assert!(refused.to_string().contains("not a socket"), "{refused:#}");
    assert!(socket.exists(), "left untouched");
    assert_eq!(
        std::fs::read(&socket).expect("still readable"),
        b"not a socket"
    );
}

async fn raw_connect(socket: &Path) -> UnixStream {
    UnixStream::connect(socket).await.expect("connect")
}

async fn read_line(stream: &mut UnixStream) -> String {
    read_bounded_line(stream, 4096).await.expect("a line")
}

/// The full happy path: a client's request becomes an accepted
/// `ControlRequest` with the right action, and the runner's `ack` reaches
/// the client verbatim.
#[tokio::test]
async fn a_request_is_delivered_and_the_ack_reaches_the_client() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let mut listener = Listener::start(socket.clone()).await.expect("bind");

    let mut client = raw_connect(&socket).await;
    client
        .write_all(b"{\"action\":\"reply\"}\n")
        .await
        .expect("send request");

    let request = listener.accept().await.expect("accepted");
    assert_eq!(request.action(), ControlAction::Reply);
    assert!(request.is_live(), "the client is still waiting");
    request
        .ack(true, "recording started")
        .expect("ack delivered");

    let reply = read_line(&mut client).await;
    let ack: serde_json::Value = serde_json::from_str(&reply).expect("json ack");
    assert_eq!(ack["ok"], true);
    assert_eq!(ack["message"], "recording started");
}

/// `Cancel` round-trips the same way, and a failure ack carries the
/// runner's reason back to the client instead of a bare `ok: false`.
#[tokio::test]
async fn a_failed_ack_carries_its_reason_to_the_client() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let mut listener = Listener::start(socket.clone()).await.expect("bind");

    let mut client = raw_connect(&socket).await;
    client
        .write_all(b"{\"action\":\"cancel\"}\n")
        .await
        .expect("send request");

    let request = listener.accept().await.expect("accepted");
    assert_eq!(request.action(), ControlAction::Cancel);
    request
        .ack(false, "no recording to cancel")
        .expect("ack delivered");

    let reply = read_line(&mut client).await;
    let ack: serde_json::Value = serde_json::from_str(&reply).expect("json ack");
    assert_eq!(ack["ok"], false);
    assert_eq!(ack["message"], "no recording to cancel");
}

/// A malformed request never becomes a `ControlRequest` (nothing reaches
/// the runner, so nothing can start a microphone from garbage input), the
/// client gets a clear ack explaining why, and the listener keeps serving
/// later connections normally.
#[tokio::test]
async fn a_malformed_request_is_rejected_without_disturbing_the_listener() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let mut listener = Listener::start(socket.clone()).await.expect("bind");

    let mut bad = raw_connect(&socket).await;
    bad.write_all(b"not json at all\n")
        .await
        .expect("send garbage");
    let reply = read_line(&mut bad).await;
    let ack: serde_json::Value = serde_json::from_str(&reply).expect("json ack");
    assert_eq!(ack["ok"], false);

    let mut good = raw_connect(&socket).await;
    good.write_all(b"{\"action\":\"reply\"}\n")
        .await
        .expect("send a real request afterwards");
    let request = tokio::time::timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("did not hang")
        .expect("accepted");
    assert_eq!(request.action(), ControlAction::Reply);
    request.ack(true, "ok").expect("ack delivered");
}

/// A line with no newline that keeps growing past the bound is cut off
/// during the read, not after: the connection is rejected with a clear ack
/// instead of the server buffering an unbounded amount from one peer.
#[tokio::test]
async fn an_oversized_request_is_rejected_and_bounded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let mut listener = Listener::start(socket.clone()).await.expect("bind");

    let mut oversized = raw_connect(&socket).await;
    let payload = vec![b'a'; MAX_LINE_BYTES * 4];
    // The write may block on a full pipe before the server gives up reading;
    // a write error here is exactly the server closing early, which is fine.
    let _ = oversized.write_all(&payload).await;
    let reply = read_line(&mut oversized).await;
    let ack: serde_json::Value = serde_json::from_str(&reply).expect("json ack");
    assert_eq!(ack["ok"], false);
    assert!(
        ack["message"].as_str().unwrap_or_default().len() <= MAX_ACK_MESSAGE_BYTES,
        "{ack}"
    );

    let mut good = raw_connect(&socket).await;
    good.write_all(b"{\"action\":\"cancel\"}\n")
        .await
        .expect("send a real request afterwards");
    let request = tokio::time::timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("did not hang")
        .expect("accepted");
    assert_eq!(request.action(), ControlAction::Cancel);
    request.ack(true, "ok").expect("ack delivered");
}

/// A connection that never sends anything is dropped after its own read
/// timeout rather than tying up a permit (and the listener) forever.
#[tokio::test]
async fn a_silent_connection_times_out_instead_of_hanging_the_listener() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let mut listener = Listener::start(socket.clone()).await.expect("bind");

    let mut silent = raw_connect(&socket).await;
    // Held open, nothing sent; the peer must still get a bounded reply.
    let reply = tokio::time::timeout(
        READ_TIMEOUT + Duration::from_secs(1),
        read_line(&mut silent),
    )
    .await
    .expect("the server replies within its own read timeout, not never");
    let ack: serde_json::Value = serde_json::from_str(&reply).expect("json ack");
    assert_eq!(ack["ok"], false);

    let mut good = raw_connect(&socket).await;
    good.write_all(b"{\"action\":\"reply\"}\n")
        .await
        .expect("the listener still serves new connections");
    let request = tokio::time::timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("did not hang")
        .expect("accepted");
    request.ack(true, "ok").expect("ack delivered");
}

/// Many sequential connections in a row must all still be served correctly:
/// a regression check for the connection `JoinSet`'s own reaping branch,
/// which fires repeatedly here instead of only once at shutdown.
#[tokio::test]
async fn many_sequential_requests_are_all_served() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let mut listener = Listener::start(socket.clone()).await.expect("bind");

    for i in 0..20 {
        let mut client = raw_connect(&socket).await;
        client
            .write_all(b"{\"action\":\"reply\"}\n")
            .await
            .expect("send request");
        let request = tokio::time::timeout(Duration::from_secs(2), listener.accept())
            .await
            .expect("did not hang")
            .unwrap_or_else(|e| panic!("accepted request {i}: {e:#}"));
        assert_eq!(request.action(), ControlAction::Reply);
        request.ack(true, "ok").expect("ack delivered");
        let reply = read_line(&mut client).await;
        let ack: serde_json::Value = serde_json::from_str(&reply).expect("json ack");
        assert_eq!(ack["ok"], true, "request {i}");
    }
}

/// `send_request` end to end, exercising the client half of the wire format
/// this module also owns.
#[tokio::test]
async fn send_request_round_trips_through_the_real_listener() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let mut listener = Listener::start(socket.clone()).await.expect("bind");

    let socket_for_client = socket.clone();
    let client = tokio::spawn(async move {
        send_request(
            &socket_for_client,
            ControlAction::Reply,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .await
    });

    let request = listener.accept().await.expect("accepted");
    assert_eq!(request.action(), ControlAction::Reply);
    request
        .ack(true, "recording started")
        .expect("ack delivered");

    let (ok, message) = client.await.expect("client task").expect("no error");
    assert!(ok);
    assert_eq!(message, "recording started");
}

/// A clear, specific error - not a generic timeout - when nothing is
/// listening at all, matching what a hotkey pointed at a closed TUI needs.
#[tokio::test]
async fn send_request_reports_a_clear_error_with_no_listener() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);

    let failure = send_request(
        &socket,
        ControlAction::Cancel,
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .await
    .expect_err("nothing is listening");
    assert!(
        failure.to_string().contains("no running TUI"),
        "{failure:#}"
    );
}

/// Dropping the request without acking, then dropping the listener, races
/// its own connection task against `run`'s shutdown: the task may still win
/// and write a best-effort ack, or shutdown's `JoinSet::shutdown` may abort
/// it first and just close the socket. Both are fine and this must not flake
/// between them - only a hang would be a bug - so it asserts solely that the
/// client is closed within a bound, checking the ack's shape only if one
/// arrived. `current_thread` keeps that race on this one OS thread, so the
/// two outcomes are the only ones possible, not a third caused by real
/// parallelism.
#[tokio::test(flavor = "current_thread")]
async fn dropping_a_request_without_acking_still_closes_the_connection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = socket_in(&dir);
    let mut listener = Listener::start(socket.clone()).await.expect("bind");

    let mut client = raw_connect(&socket).await;
    client
        .write_all(b"{\"action\":\"reply\"}\n")
        .await
        .expect("send request");

    let request = listener.accept().await.expect("accepted");
    drop(request);
    drop(listener);

    let bytes = tokio::time::timeout(Duration::from_secs(2), read_to_eof(&mut client))
        .await
        .expect("closed within the shutdown timeout, not left open forever");
    if !bytes.is_empty() {
        let line = std::str::from_utf8(&bytes).expect("utf8").trim_end();
        let ack: serde_json::Value = serde_json::from_str(line).expect("json ack");
        assert_eq!(
            ack["ok"], false,
            "a delivered ack must still say it did not succeed"
        );
    }
}

/// Reads until the peer closes the connection, returning whatever bytes (if
/// any) arrived first - unlike `read_line`, EOF with no line is a valid
/// result here, not a failure.
async fn read_to_eof(stream: &mut UnixStream) -> Vec<u8> {
    let mut collected = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        let n = stream.read(&mut chunk).await.expect("read does not error");
        if n == 0 {
            return collected;
        }
        collected.extend_from_slice(&chunk[..n]);
    }
}
