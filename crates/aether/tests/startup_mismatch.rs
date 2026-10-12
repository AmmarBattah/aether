//! Real CLI entry points against an isolated, deliberately incompatible peer.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

struct Fixture {
    child: Child,
    socket: PathBuf,
    _dir: tempfile::TempDir,
}

impl Fixture {
    fn new(mode: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("skew-")
            .tempdir()
            .expect("fixture dir");
        let socket = dir.path().join("s");
        let child = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--ignored", "--exact", "fixture_peer", "--nocapture"])
            .env("AETHER_SKEW_FIXTURE_SOCKET", &socket)
            .env("AETHER_SKEW_FIXTURE_MODE", mode)
            .spawn()
            .expect("fixture process");
        let mut fixture = Self {
            child,
            socket,
            _dir: dir,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while !fixture.socket.exists() {
            assert!(
                fixture.child.try_wait().expect("fixture status").is_none(),
                "fixture exited before binding"
            );
            assert!(Instant::now() < deadline, "fixture failed to bind");
            std::thread::sleep(Duration::from_millis(10));
        }
        fixture
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aether"))
            .arg("--socket")
            .arg(&self.socket)
            .args(args)
            .env("AETHER_CONFIG_DIR", self._dir.path().join("config"))
            .env("AETHER_DATA_DIR", self._dir.path().join("data"))
            .output()
            .expect("CLI output")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.child.try_wait().expect("fixture status").is_none() {
            self.child.kill().expect("kill owned fixture only");
        }
        self.child.wait().expect("reap fixture");
    }
}

#[test]
fn stale_daemon_is_diagnosed_without_starting_or_removing_it() {
    let mut fixture = Fixture::new("refuse");
    for args in [&["status"][..], &[][..]] {
        let output = fixture.cli(args);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{output:?}");
        assert!(error.contains("from another build"), "{error}");
        assert!(
            error.contains(&format!(
                "aether --socket '{}' stop",
                fixture.socket.display()
            )),
            "{error}"
        );
        assert!(error.contains("fixture speaks protocol 1"), "{error}");
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert!(fixture.socket.exists());
    }
    let output = fixture.cli(&["doctor", "--json"]);
    let report = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(report.contains("refused this build"), "{report}");
    assert!(!report.contains("nothing answers"), "{report}");
}

#[cfg(target_os = "linux")]
#[test]
fn stop_uses_the_refusing_socket_peer_not_a_stale_pid_file() {
    let mut fixture = Fixture::new("refuse");
    std::fs::write(
        fixture.socket.with_extension("pid"),
        std::process::id().to_string(),
    )
    .unwrap();
    let output = fixture.cli(&["stop"]);
    assert!(output.status.success(), "{output:?}");
    assert!(
        fixture.child.wait().unwrap().success(),
        "SIGTERM handler exited cleanly"
    );
    assert!(
        !fixture.socket.exists(),
        "fixture cleaned up its own socket"
    );
}

#[test]
fn invalid_reply_never_signals_the_peer() {
    let mut fixture = Fixture::new("invalid");
    let output = fixture.cli(&["stop"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(fixture.child.try_wait().unwrap().is_none());
    assert!(fixture.socket.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn ignored_sigterm_times_out_without_killing_or_unlinking() {
    let mut fixture = Fixture::new("ignore-term");
    let output = fixture.cli(&["stop"]);
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{output:?}");
    assert!(error.contains("SIGTERM was sent"), "{error}");
    assert!(
        error.contains("fixture speaks protocol 1"),
        "original refusal survives: {error}"
    );
    assert!(fixture.child.try_wait().unwrap().is_none());
    assert!(fixture.socket.exists());
}

#[test]
fn silent_peer_times_out_without_being_signalled() {
    let mut fixture = Fixture::new("silent");
    let start = Instant::now();
    let output = fixture.cli(&["stop"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("timed out stopping"));
    assert!(start.elapsed() < Duration::from_secs(15));
    assert!(fixture.child.try_wait().unwrap().is_none());
}

#[tokio::test]
#[ignore = "subprocess fixture, launched only by the tests above"]
async fn fixture_peer() {
    use aether_protocol::{ErrorCode, ErrorReply, ServerFrame};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let socket = std::env::var_os("AETHER_SKEW_FIXTURE_SOCKET").expect("fixture socket");
    let mode = std::env::var("AETHER_SKEW_FIXTURE_MODE").expect("fixture mode");
    let mut term =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind fixture");
    loop {
        tokio::select! {
            _ = term.recv() => {
                if mode != "ignore-term" {
                    std::fs::remove_file(&socket).expect("fixture removes its own socket");
                    return;
                }
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted.expect("accept");
                let mode = mode.clone();
                tokio::spawn(async move {
                    let (read, mut write) = stream.into_split();
                    let mut lines = BufReader::new(read).lines();
                    let line = lines.next_line().await.unwrap().expect("hello");
                    let frame: aether_protocol::ClientFrame = serde_json::from_str(&line).unwrap();
                    if mode == "silent" {
                        std::future::pending::<()>().await;
                    }
                    let reply = if mode == "invalid" {
                        "invalid JSON\n".to_owned()
                    } else {
                        let reply = ServerFrame::Reply { id: frame.id, result: Err(ErrorReply::new(
                            ErrorCode::Incompatible, "fixture speaks protocol 1",
                        )) };
                        format!("{}\n", serde_json::to_string(&reply).unwrap())
                    };
                    write.write_all(reply.as_bytes()).await.expect("reply");
                });
            }
        }
    }
}
