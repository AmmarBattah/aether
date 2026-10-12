//! Preserve a complete SQLite snapshot before changing an existing schema.
use crate::{Result, StoreError, schema};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub(crate) fn before_migration(conn: &Connection, path: &Path) -> Result<Option<PathBuf>> {
    let Some(version) = schema::current_version(conn)? else {
        return Ok(None);
    };
    if version >= schema::SCHEMA_VERSION {
        return Ok(None);
    }
    let directory = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("migration-backups");
    std::fs::create_dir_all(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    }
    let destination = directory.join(format!(
        "schema-{version}-before-{}-{}.sqlite",
        schema::SCHEMA_VERSION,
        ulid::Ulid::generate()
    ));
    let temporary = tempfile::NamedTempFile::new_in(&directory)?;
    // SQLite's backup API includes committed WAL data and preserves row IDs,
    // which are referenced by the external-content memory search index.
    conn.backup(rusqlite::MAIN_DB, temporary.path(), None)?;
    temporary.as_file().sync_all()?;
    let file = temporary
        .persist_noclobber(&destination)
        .map_err(|error| StoreError::Io(error.error))?;
    file.sync_all()?;
    #[cfg(unix)]
    std::fs::File::open(&directory)?.sync_all()?;
    tracing::info!(from_version = version, to_version = schema::SCHEMA_VERSION, path = %destination.display(), "saved database before schema migration");
    Ok(Some(destination))
}
