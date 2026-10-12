//! Replacing the running binary: only where `install.sh` put it, and only by
//! a rename, so a failed update leaves the old binary in place.

use std::fmt;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, ensure};

use super::release::Version;

/// The documented way to install a released binary.
pub(super) const INSTALL_COMMAND: &str =
    "curl -fsSL https://raw.githubusercontent.com/mengsig/aether/main/install.sh | sh";
const CARGO_COMMAND: &str = "cargo install --git https://github.com/mengsig/aether --locked aether";
/// Written by `install.sh` beside the binary it installs.
pub(super) const RECEIPT_FILE: &str = ".aether-receipt";
const RECEIPT_MARKER: &str = "installer=install.sh";
const MAX_RECEIPT_BYTES: u64 = 4 * 1024;
/// The binary's name inside a release archive.
const ARCHIVE_MEMBER: &str = "aether";
const UNPACK_TIMEOUT: Duration = Duration::from_secs(120);
const VERSION_TIMEOUT: Duration = Duration::from_secs(30);

/// Why `aether update` leaves a binary alone, and the command that updates it.
#[derive(Debug)]
pub(super) struct Refusal {
    reason: String,
    pub(super) command: &'static str,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:\n  {}", self.reason, self.command)
    }
}

impl std::error::Error for Refusal {}

/// A binary `aether update` may replace, and the directory it lives in.
#[derive(Debug)]
pub(super) struct Installation {
    binary: PathBuf,
    directory: PathBuf,
}

impl Installation {
    /// The running binary, or why it is not this command's to replace.
    pub(super) fn of_running_binary() -> anyhow::Result<Self> {
        let executable =
            std::env::current_exe().context("could not find the running aether binary")?;
        Self::at(&executable)
    }

    /// `executable`, if `install.sh` put it in a directory this user can
    /// write. Otherwise the error is a [`Refusal`] naming what does update it.
    pub(super) fn at(executable: &Path) -> anyhow::Result<Self> {
        let binary = std::fs::canonicalize(executable).with_context(|| {
            format!(
                "could not resolve {}; was it replaced or removed while running?",
                executable.display()
            )
        })?;
        let directory = binary
            .parent()
            .with_context(|| format!("{} has no parent directory", binary.display()))?
            .to_owned();
        if let Some(refusal) = refusal(&binary, &directory)? {
            return Err(refusal.into());
        }
        Ok(Self { binary, directory })
    }

    pub(super) fn binary(&self) -> &Path {
        &self.binary
    }

    /// A private directory beside the binary, so the final rename stays on
    /// one filesystem. It is removed when dropped, whatever happened.
    pub(super) fn staging(&self) -> anyhow::Result<tempfile::TempDir> {
        tempfile::Builder::new()
            .prefix(".aether-update-")
            .tempdir_in(&self.directory)
            .with_context(|| format!("could not stage in {}", self.directory.display()))
    }

    /// Move the staged binary over the installed one. A process already
    /// running the old file keeps it until it exits.
    pub(super) fn replace_with(&self, staged: &Path) -> anyhow::Result<()> {
        let metadata = std::fs::symlink_metadata(staged)
            .with_context(|| format!("could not inspect {}", staged.display()))?;
        ensure!(
            metadata.is_file(),
            "{} is not a regular file",
            staged.display()
        );
        ensure!(
            staged.parent().and_then(Path::parent) == Some(self.directory.as_path()),
            "{} is not staged beside {}",
            staged.display(),
            self.binary.display()
        );
        std::fs::File::open(staged)
            .and_then(|file| file.sync_all())
            .with_context(|| format!("could not flush {}", staged.display()))?;
        std::fs::rename(staged, &self.binary)
            .with_context(|| format!("could not replace {}", self.binary.display()))
    }
}

/// Why `binary` is not ours to replace, if it is not. Cargo's record is read
/// first: a receipt in the same directory must not let us overwrite cargo's binary.
fn refusal(binary: &Path, directory: &Path) -> anyhow::Result<Option<Refusal>> {
    let binary = binary.display();
    if installed_by_cargo(directory)? {
        return Ok(Some(Refusal {
            reason: format!(
                "{binary} was installed by cargo, so `aether update` leaves it alone. Update \
                 it with (add `--tag v1.2.3` for one release)"
            ),
            command: CARGO_COMMAND,
        }));
    }
    if !has_receipt(directory)? {
        return Ok(Some(Refusal {
            reason: format!(
                "{binary} was not installed by Aether's install.sh (no {RECEIPT_FILE} beside \
                 it), so `aether update` leaves it alone. Update it the way it was installed, \
                 or install the released binary with"
            ),
            command: INSTALL_COMMAND,
        }));
    }
    if !is_writable(directory)? {
        return Ok(Some(Refusal {
            reason: format!(
                "{} is not writable by you, so `aether update` cannot replace {binary}. \
                 Install into a directory you own with",
                directory.display()
            ),
            command: INSTALL_COMMAND,
        }));
    }
    Ok(None)
}

/// Whether `install.sh` left its receipt in `directory`.
fn has_receipt(directory: &Path) -> anyhow::Result<bool> {
    let path = directory.join(RECEIPT_FILE);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| format!("could not inspect {}", path.display()));
        }
    };
    if !metadata.is_file() || metadata.len() > MAX_RECEIPT_BYTES {
        return Ok(false);
    }
    let receipt = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read {}", path.display()))?;
    Ok(receipt.lines().any(|line| line.trim() == RECEIPT_MARKER))
}

/// Whether cargo's install record, beside a `bin` directory, lists aether.
fn installed_by_cargo(directory: &Path) -> anyhow::Result<bool> {
    let Some(root) = directory.parent() else {
        return Ok(false);
    };
    let record = root.join(".crates.toml");
    match std::fs::read_to_string(&record) {
        Ok(text) => Ok(text.lines().any(|line| line.starts_with("\"aether "))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("could not read {}", record.display())),
    }
}

/// Asked of the OS (ACLs and read-only mounts included), not read off mode bits.
fn is_writable(directory: &Path) -> anyhow::Result<bool> {
    let access = rustix::fs::accessat(
        rustix::fs::CWD,
        directory,
        rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
        rustix::fs::AtFlags::EACCESS,
    );
    match access {
        Ok(()) => Ok(true),
        Err(rustix::io::Errno::ACCESS | rustix::io::Errno::ROFS | rustix::io::Errno::PERM) => {
            Ok(false)
        }
        Err(error) => Err(std::io::Error::from(error))
            .with_context(|| format!("could not check access to {}", directory.display())),
    }
}

/// Unpack the binary from `archive` into `staging` with the system `tar`,
/// the tool `install.sh` uses, and return where it is.
pub(super) async fn unpack(archive: &Path, staging: &Path) -> anyhow::Result<PathBuf> {
    let mut command = tokio::process::Command::new("tar");
    command
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(staging)
        .arg(ARCHIVE_MEMBER)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(UNPACK_TIMEOUT, command.output())
        .await
        .context("unpacking the release archive timed out")?
        .context("could not run `tar`, which unpacks the release archive")?;
    ensure!(
        output.status.success(),
        "`tar` could not unpack the release archive ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let binary = staging.join(ARCHIVE_MEMBER);
    let metadata = std::fs::symlink_metadata(&binary)
        .with_context(|| format!("the release archive holds no `{ARCHIVE_MEMBER}`"))?;
    ensure!(
        metadata.is_file() && metadata.len() > 0,
        "`{ARCHIVE_MEMBER}` in the release archive is not a regular, non-empty file"
    );
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("could not make {} executable", binary.display()))?;
    Ok(binary)
}

/// Run the unpacked binary and require the version that was asked for: a
/// build for another machine, or another release, never replaces this one.
pub(super) async fn check_runs_as(binary: &Path, expected: Version) -> anyhow::Result<()> {
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(VERSION_TIMEOUT, command.output())
        .await
        .context("the downloaded binary did not answer `--version` in time")?
        .context("the downloaded binary does not run on this machine")?;
    ensure!(
        output.status.success(),
        "the downloaded binary failed `--version` ({})",
        output.status
    );
    let reported = String::from_utf8_lossy(&output.stdout);
    let reported = reported.trim();
    ensure!(
        reported == format!("aether {expected}"),
        "the downloaded binary reports `{reported}`, not aether {expected}"
    );
    Ok(())
}
