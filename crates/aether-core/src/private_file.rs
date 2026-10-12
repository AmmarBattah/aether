//! Files that carry a run's contents are created private to their owner.
//!
//! An export is the highest-value file this system produces: every model
//! message, every tool input, every invocation error and every artifact
//! payload inline. A written report carries the same material. Both go to a
//! path the caller chose, so the mode cannot be left to where they point it
//! or to whatever umask the shell had - `VISION.md` §10 specifies the socket
//! as 0600 for the same reason.

use std::fs::File;
use std::io;
use std::path::Path;

/// Create (or truncate) `path` readable and writable by its owner only.
pub fn create(path: &Path) -> io::Result<File> {
    open(path, false)
}

/// Create a new private file, refusing any existing entry, including symlinks.
pub fn create_new(path: &Path) -> io::Result<File> {
    open(path, true)
}

fn open(path: &Path, exclusive: bool) -> io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.create(!exclusive).create_new(exclusive).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
        );
    }
    let file = options.open(path).map_err(|error| {
        #[cfg(unix)]
        if error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) {
            return io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refusing to follow symlink: {}", path.display()),
            );
        }
        error
    })?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a regular file", path.display()),
        ));
    }
    // Check the opened object before changing its permissions or contents.
    restrict(&file)?;
    file.set_len(0)?;
    Ok(file)
}

/// Write `bytes` to `path`, readable by its owner only.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use io::Write as _;
    let original = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing to follow symlink",
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination is not a regular file",
            ));
        }
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let temporary = path.with_file_name(format!(".aether-private-{}", ulid::Ulid::generate()));
    let mut file = create_new(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        #[cfg(unix)]
        if let Some(metadata) = original {
            use std::os::unix::fs::MetadataExt;
            rustix::fs::fchown(
                &file,
                Some(rustix::fs::Uid::from_raw(metadata.uid())),
                Some(rustix::fs::Gid::from_raw(metadata.gid())),
            )?;
            restrict(&file)?;
        }
        #[cfg(not(unix))]
        let _ = original;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if let Err(error) = result {
        if let Err(cleanup) = std::fs::remove_file(&temporary) {
            return Err(io::Error::new(
                error.kind(),
                format!("{error}; removing temporary file failed: {cleanup}"),
            ));
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(unix)]
fn restrict(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
}

/// Windows has no mode to set; the file inherits the directory's ACL.
#[cfg(not(unix))]
fn restrict(_file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::FileTypeExt as _;
    use std::os::unix::fs::PermissionsExt as _;

    /// The mode bits, or the io error that stopped us reading them.
    fn mode(path: &std::path::Path) -> std::io::Result<u32> {
        Ok(std::fs::metadata(path)?.permissions().mode() & 0o777)
    }

    #[test]
    fn symlinks_are_refused_without_truncating_or_chmodding_the_target() {
        let dir = tempfile::tempdir().expect("dir");
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        std::fs::write(&target, b"keep").expect("seed");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        assert!(
            super::create(&link)
                .unwrap_err()
                .to_string()
                .contains("refusing to follow symlink")
        );
        assert!(
            super::write(&link, b"replace")
                .unwrap_err()
                .to_string()
                .contains("refusing to follow symlink")
        );
        assert_eq!(std::fs::read(&target).expect("read"), b"keep");
        assert_eq!(mode(&target).expect("mode"), 0o644);
        std::fs::remove_file(&target).expect("remove target");
        assert!(super::write(&link, b"replace").is_err());
        assert!(!target.exists());
    }

    #[test]
    fn nonregular_files_are_refused_without_blocking() {
        let dir = tempfile::tempdir().expect("dir");
        assert!(super::create(dir.path()).is_err());
        let fifo = dir.path().join("fifo");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &fifo,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::from_bits_truncate(0o644),
            0,
        )
        .expect("fifo");
        assert!(super::create(&fifo).is_err());
        assert!(
            std::fs::symlink_metadata(&fifo)
                .expect("metadata")
                .file_type()
                .is_fifo()
        );
    }

    #[test]
    fn exclusive_creation_refuses_existing_entries() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("partial");
        drop(super::create_new(&path).expect("create"));
        assert_eq!(mode(&path).expect("mode"), 0o600);
        std::fs::write(&path, b"keep").expect("seed");
        assert_eq!(
            super::create_new(&path).expect_err("existing file").kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&path).expect("read"), b"keep");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&path, &link).expect("symlink");
        assert_eq!(
            super::create_new(&link).expect_err("symlink").kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&path).expect("read"), b"keep");
    }

    #[test]
    fn a_written_file_is_readable_by_its_owner_only() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("run.aether");
        super::write(&path, b"secrets").expect("write");
        assert_eq!(mode(&path).expect("mode"), 0o600);
        assert_eq!(std::fs::read(&path).expect("read"), b"secrets");
    }

    #[test]
    fn atomic_write_leaves_existing_open_file_and_hard_link_intact() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report");
        std::fs::write(&path, b"original").unwrap();
        let alias = dir.path().join("alias");
        std::fs::hard_link(&path, &alias).unwrap();
        let mut reader = std::fs::File::open(&path).unwrap();
        super::write(&path, b"replacement").unwrap();
        let mut old = String::new();
        reader.read_to_string(&mut old).unwrap();
        assert_eq!(old, "original");
        assert_eq!(std::fs::read(&alias).unwrap(), b"original");
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
    }

    #[test]
    fn an_existing_world_readable_file_is_narrowed() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("run.aether");
        std::fs::write(&path, b"old").expect("seed");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        super::write(&path, b"new").expect("write");
        assert_eq!(mode(&path).expect("mode"), 0o600);
    }
}
