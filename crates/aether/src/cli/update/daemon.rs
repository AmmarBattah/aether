//! The daemon an update leaves on the old build, and whether a build with
//! another protocol version could still stop it.

use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// What holds the selected socket, as far as stopping it later goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Daemon {
    /// Nothing listens there.
    Absent,
    /// `aether stop` from any build ends it: asked, or signalled if it refuses.
    Signallable,
    /// Only a build it accepts can ask it to stop; says why it cannot be signalled.
    AskOnly(String),
}

impl Daemon {
    pub(super) fn is_running(&self) -> bool {
        !matches!(self, Self::Absent)
    }
}

/// Look at `socket` without speaking to its daemon. Nothing leaves this machine.
/// Short of a clear "nothing listens here", it counts as one that cannot be signalled.
pub(super) async fn probe(socket: &Path) -> Daemon {
    let connecting = tokio::net::UnixStream::connect(socket);
    let stream = match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(error)) if nothing_listens(error.kind()) => return Daemon::Absent,
        Ok(Err(error)) => {
            return Daemon::AskOnly(format!("its socket could not be reached: {error}"));
        }
        Err(_) => {
            let seconds = CONNECT_TIMEOUT.as_secs();
            return Daemon::AskOnly(format!("its socket did not answer within {seconds}s"));
        }
    };
    // The same pinning `aether stop` needs before it signals a refusing daemon.
    match crate::daemon_stop::Peer::identify(&stream) {
        Ok(_) => Daemon::Signallable,
        Err(error) => Daemon::AskOnly(format!("{error:#}")),
    }
}

/// A missing socket, or a file no process listens on.
fn nothing_listens(kind: ErrorKind) -> bool {
    matches!(kind, ErrorKind::NotFound | ErrorKind::ConnectionRefused)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_socket_nothing_listens_on_has_no_daemon() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            probe(&dir.path().join("missing.sock")).await,
            Daemon::Absent
        );
        let stale = dir.path().join("stale.sock");
        drop(std::os::unix::net::UnixListener::bind(&stale).expect("bind"));
        assert!(stale.exists(), "the socket file outlives its listener");
        assert_eq!(probe(&stale).await, Daemon::Absent);
        assert!(!Daemon::Absent.is_running());
    }

    /// This process is the listener, which no platform lets it signal: the
    /// same answer macOS and Linux before 6.5 give for every daemon.
    #[tokio::test]
    async fn a_listener_that_cannot_be_pinned_is_ask_only_with_the_reason() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("d.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        let daemon = probe(&socket).await;
        let Daemon::AskOnly(reason) = &daemon else {
            panic!("{daemon:?} must not count as signallable");
        };
        assert!(!reason.is_empty() && !reason.contains('\n'), "{reason}");
        assert!(daemon.is_running());
    }

    #[tokio::test]
    async fn a_socket_that_cannot_be_reached_is_not_taken_for_no_daemon() {
        if nix::unistd::geteuid().is_root() {
            eprintln!("root reaches every socket: nothing to refuse");
            return;
        }
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).expect("directory");
        let socket = locked.join("d.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("lock");
        let daemon = probe(&socket).await;
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("unlock");
        let Daemon::AskOnly(reason) = daemon else {
            panic!("{daemon:?} must not count as absent");
        };
        assert!(reason.contains("could not be reached"), "{reason}");
    }
}
