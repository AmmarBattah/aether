use anyhow::Context as _;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::net::{UnixListener, UnixStream};

const LOCK_RETRY_ATTEMPTS: u32 = 10;
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(20);

/// A file's device+inode: enough to tell "the socket I bound" from "whatever
/// now sits at that path", without trusting the path alone.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct FileId {
    dev: u64,
    ino: u64,
}

pub(super) fn file_identity(path: &Path) -> std::io::Result<FileId> {
    let meta = std::fs::symlink_metadata(path)?;
    Ok(FileId {
        dev: meta.dev(),
        ino: meta.ino(),
    })
}

enum LockOutcome {
    Taken(std::fs::File),
    Busy,
}

/// `<socket>.lock` beside the socket. Never removed (see
/// `aether-daemon::instance_lock` for why: unlinking it would let a second
/// process lock a fresh inode while this one still holds the old one).
pub(super) fn lock_path_for(socket: &Path) -> PathBuf {
    let mut name = socket.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

fn try_take_lock(socket: &Path) -> anyhow::Result<LockOutcome> {
    let path = lock_path_for(socket);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&path)
        .with_context(|| format!("opening the voice control lock {}", path.display()))?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "{} must be a regular file, not a link or directory",
        path.display()
    );
    match file.try_lock() {
        Ok(()) => Ok(LockOutcome::Taken(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(LockOutcome::Busy),
        Err(std::fs::TryLockError::Error(e)) => {
            Err(anyhow::Error::new(e).context(format!("locking {}", path.display())))
        }
    }
}

/// A brief bounded retry on `try_take_lock`: a same-process `off` then `on`
/// can trail the old task's lock release by a beat; a genuinely different,
/// still-live TUI stays busy well past this window either way.
async fn take_lock_with_retry(socket: &Path) -> anyhow::Result<std::fs::File> {
    for attempt in 0..LOCK_RETRY_ATTEMPTS {
        match try_take_lock(socket)? {
            LockOutcome::Taken(file) => return Ok(file),
            LockOutcome::Busy if attempt + 1 < LOCK_RETRY_ATTEMPTS => {
                tokio::time::sleep(LOCK_RETRY_DELAY).await;
            }
            LockOutcome::Busy => anyhow::bail!(
                "another TUI already holds the voice control endpoint at {}",
                socket.display()
            ),
        }
    }
    unreachable!("the loop above always returns on its last attempt")
}

/// Create `dir` (our own dedicated `.voice` directory, never a shared parent)
/// at 0700 if missing; otherwise only verify it - never follow a symlink,
/// touch another user's directory, or chmod a loose mode ourselves.
fn harden_dir(dir: &Path) -> anyhow::Result<()> {
    let ours = nix::unistd::getuid().as_raw();
    match std::fs::symlink_metadata(dir) {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // `mkdir` with the mode baked in, not a separate `chmod` after a
            // default-mode create: no window where the directory briefly
            // exists with a looser mode than what we're about to enforce.
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(dir)
                .with_context(|| format!("creating {}", dir.display()))
        }
        Err(e) => Err(anyhow::Error::new(e).context(format!("inspecting {}", dir.display()))),
        Ok(meta) => {
            anyhow::ensure!(
                meta.is_dir(),
                "{} exists and is not a directory (refusing to follow a symlink or use a non-directory path)",
                dir.display()
            );
            anyhow::ensure!(
                meta.uid() == ours,
                "{} is owned by another user; refusing to use it",
                dir.display()
            );
            anyhow::ensure!(
                meta.permissions().mode() & 0o077 == 0,
                "{} is not owner-only (refusing to reuse or chmod it)",
                dir.display()
            );
            Ok(())
        }
    }
}

/// `Some` type/ownership check for whatever currently sits at `socket`,
/// short-circuiting on "nothing there" - not an error, just nothing to probe.
fn existing_socket_is_stale_candidate(socket: &Path) -> anyhow::Result<bool> {
    match std::fs::symlink_metadata(socket) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(anyhow::Error::new(e).context(format!("inspecting {}", socket.display()))),
        Ok(meta) => {
            anyhow::ensure!(
                meta.file_type().is_socket(),
                "refusing to remove {} - it is not a socket (a symlink or unexpected file there is left untouched)",
                socket.display()
            );
            anyhow::ensure!(
                meta.uid() == nix::unistd::getuid().as_raw(),
                "{} is owned by another user; refusing to touch it",
                socket.display()
            );
            Ok(true)
        }
    }
}

/// Clear a socket whose owner is gone. Called only while holding this
/// endpoint's own lock, so a foreign listener still answering here is left
/// alone rather than torn down - it can never be a second legitimate owner.
async fn remove_stale_socket(socket: &Path) -> anyhow::Result<()> {
    if !existing_socket_is_stale_candidate(socket)? {
        return Ok(());
    }
    match UnixStream::connect(socket).await {
        Ok(_) => anyhow::bail!(
            "another process is already listening for voice control at {}",
            socket.display()
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {}
        Err(e) => {
            return Err(
                anyhow::Error::new(e).context(format!("probing the socket {}", socket.display()))
            );
        }
    }
    match std::fs::remove_file(socket) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(anyhow::Error::new(e)
            .context(format!("removing the stale socket {}", socket.display()))),
        _ => Ok(()),
    }
}

pub(super) async fn acquire_and_bind(
    socket: &Path,
) -> anyhow::Result<(std::fs::File, UnixListener)> {
    if let Some(dir) = socket.parent().filter(|d| !d.as_os_str().is_empty()) {
        harden_dir(dir)?;
    }
    let lock = take_lock_with_retry(socket).await?;
    remove_stale_socket(socket).await?;
    let listener = UnixListener::bind(socket).map_err(|e| {
        anyhow::Error::new(e).context(format!(
            "binding the voice control socket {}",
            socket.display()
        ))
    })?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("hardening {}", socket.display()))?;
    Ok((lock, listener))
}
