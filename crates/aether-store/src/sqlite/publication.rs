use super::{SqliteStore, blobs::live_explicit_path_reference};
use crate::{Result, StagedArtifact, StoreError};
use aether_core::id::ArtifactId;
use rusqlite::{TransactionBehavior, params};
use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};

impl SqliteStore {
    pub(super) fn begin_blob(&self, id: ArtifactId) -> Result<(PathBuf, File)> {
        let dir = self.staging_dir.as_ref().ok_or(StoreError::NoArtifactDir)?;
        let path = self.artifact_path(id).ok_or(StoreError::NoArtifactDir)?;
        let staged = dir.join(id.to_string());
        let mut conn = self.conn.lock();
        // Startup cannot inspect the entry before its publication lease exists.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if live_explicit_path_reference(&tx, &path)? {
            return Err(StoreError::Corrupt(format!(
                "artifact destination is already live: {}",
                path.display()
            )));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&staged)?;
        let initialized = (|| -> Result<()> {
            file.lock()?;
            sync_directory(dir)?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = initialized {
            discard_uncommitted_blob(&staged);
            return Err(error);
        }
        Ok((staged, file))
    }

    pub(super) fn stage_blob(&self, id: ArtifactId, bytes: &[u8]) -> Result<StagedArtifact> {
        if self.staging_dir.is_none() {
            return Ok(StagedArtifact::ready(self.write_blob(id, bytes)?));
        }
        self.write_staged_blob(id, |file| {
            file.write_all(bytes)?;
            file.sync_all()?;
            Ok(())
        })
        .map(|(publication, ())| publication)
    }

    pub(super) fn write_staged_blob<T>(
        &self,
        id: ArtifactId,
        write: impl FnOnce(&mut File) -> Result<T>,
    ) -> Result<(StagedArtifact, T)> {
        let (staged, mut file) = self.begin_blob(id)?;
        let value = match write(&mut file) {
            Ok(value) => value,
            Err(error) => {
                discard_uncommitted_blob(&staged);
                return Err(error);
            }
        };
        Ok((
            StagedArtifact {
                path: self.artifact_path(id).ok_or(StoreError::NoArtifactDir)?,
                staged: Some((staged, file)),
            },
            value,
        ))
    }

    pub(super) fn finish_blob(&self, publication: &StagedArtifact) -> Result<()> {
        let Some((staged, _lease)) = &publication.staged else {
            return Ok(());
        };
        if staged.parent() != self.staging_dir.as_deref()
            || publication.path.parent() != self.artifact_dir.as_deref()
            || staged.file_name() != publication.path.file_name()
        {
            return Err(StoreError::Corrupt(
                "publication belongs to another store".into(),
            ));
        }
        let mut conn = self.conn.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let owner = staged
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| StoreError::Corrupt("staged payload has no artifact id".into()))?;
        // Legacy imports carry evicted bytes; their unlink must already be durably pending.
        let committed = live_explicit_path_reference(&tx, &publication.path)?
            || tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM artifacts WHERE id = ?1 AND evicted = 1
                 AND unlink_pending = 1 AND json_extract(body, '$.path') = ?2)",
                params![owner, publication.path.to_string_lossy()],
                |row| row.get::<_, bool>(0),
            )?;
        if !committed {
            if !live_explicit_path_reference(&tx, staged)? {
                discard_uncommitted_blob(staged);
            }
            return Err(StoreError::Corrupt(format!(
                "staged artifact has no committed reference requiring bytes: {}",
                publication.path.display()
            )));
        }
        if live_explicit_path_reference(&tx, staged)? {
            return Err(StoreError::Corrupt(
                "staging path has a live reference".into(),
            ));
        }
        promote(staged, &publication.path)?;
        tx.commit()?;
        Ok(())
    }

    pub(super) fn open_blob_file(&self, path: &Path, live: bool) -> Result<Option<File>> {
        let open = |path: &Path| match File::open(path) {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError::Io(error)),
        };
        if let Some(file) = open(path)? {
            return Ok(Some(file));
        }
        if live
            && path.parent() == self.artifact_dir.as_deref()
            && let Some(dir) = &self.staging_dir
            && let Some(name) = path.file_name()
            && let Some(file) = open(&dir.join(name))?
        {
            return Ok(Some(file));
        }
        // Promotion can move the payload between the two opens.
        open(path)
    }

    pub(super) fn recover_staged_blobs(&self) -> Result<()> {
        let Some(dir) = &self.staging_dir else {
            return Ok(());
        };
        let mut conn = self.conn.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(path = %dir.display(), %error, "artifact staging scan failed; will retry on next open");
                tx.commit()?;
                return Ok(());
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    tracing::warn!(path = %dir.display(), %error, "artifact staging entry unavailable; will retry on next open");
                    continue;
                }
            };
            let Some(id) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<ArtifactId>().ok())
            else {
                continue;
            };
            let staged = entry.path();
            if let Err(error) = self.recover_staged_blob(&tx, id, &staged) {
                tracing::warn!(path = %staged.display(), %error, "artifact staging recovery failed; will retry on next open");
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn recover_staged_blob(
        &self,
        conn: &rusqlite::Connection,
        id: ArtifactId,
        staged: &Path,
    ) -> Result<()> {
        if !std::fs::symlink_metadata(staged)?.file_type().is_file() {
            return Ok(());
        }
        let file = match OpenOptions::new().read(true).write(true).open(staged) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Ok(()),
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
        // A row pointing directly into staging is still a live file.
        if live_explicit_path_reference(conn, staged)? {
            return Ok(());
        }
        let target = self.artifact_path(id).ok_or(StoreError::NoArtifactDir)?;
        if live_explicit_path_reference(conn, &target)? {
            promote(staged, &target)?;
        } else {
            std::fs::remove_file(staged)?;
            sync_directory(staged.parent().ok_or(StoreError::NoArtifactDir)?)?;
        }
        Ok(())
    }
}

pub(super) fn discard_uncommitted_blob(staged: &Path) {
    match std::fs::remove_file(staged) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(path = %staged.display(), %error,
            "uncommitted artifact cleanup failed; will retry on next open"),
    }
}

fn promote(staged: &Path, target: &Path) -> Result<()> {
    // Linking before unlinking is a no-clobber rename with a recoverable intermediate state.
    match std::fs::hard_link(staged, target) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if target.symlink_metadata()?.file_type().is_symlink()
                || !same_file::is_same_file(staged, target)?
            {
                return Err(StoreError::Corrupt(format!(
                    "artifact destination already exists: {}",
                    target.display()
                )));
            }
        }
        Err(error) => return Err(error.into()),
    }
    sync_directory(target.parent().ok_or(StoreError::NoArtifactDir)?)?;
    std::fs::remove_file(staged)?;
    sync_directory(staged.parent().ok_or(StoreError::NoArtifactDir)?)?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
