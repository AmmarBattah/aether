use super::*;
use std::time::Duration;

fn bulky_event(bytes: usize) -> Commit {
    Commit::new().event(EventDraft::new(
        Actor::System,
        EventKind::Custom {
            name: "test.bulk".into(),
            payload: serde_json::json!({ "body": "x".repeat(bytes) }),
        },
    ))
}

/// SQLite answers a write that would grow the database past `max_page_count`
/// exactly as it answers a full disk: SQLITE_FULL, from the real pager. The
/// commit must read as unwritable and leave nothing behind, and the same
/// connection must take writes again once there is room, without a reopen.
#[test]
fn a_full_disk_refuses_the_commit_and_the_store_writes_again_once_freed() {
    let dir = tempfile::tempdir().expect("tmp");
    let store =
        SqliteStore::open_checkpointing(&dir.path().join("aether.db"), Duration::from_secs(3600))
            .expect("open");
    store.commit(bulky_event(16)).expect("room at first");
    let before = store.last_seq().expect("seq");
    {
        let conn = store.conn.lock();
        let pages: i64 = conn
            .query_row("PRAGMA page_count", [], |r| r.get(0))
            .expect("page count");
        conn.execute_batch(&format!("PRAGMA max_page_count = {pages}"))
            .expect("cap");
    }

    let error = store
        .commit(bulky_event(1 << 20))
        .expect_err("a megabyte does not fit in the pages already there");
    assert!(error.is_unwritable(), "{error}");
    assert_eq!(
        store.last_seq().expect("seq"),
        before,
        "a refused commit moved the seq"
    );

    store
        .conn
        .lock()
        .execute_batch("PRAGMA max_page_count = 1073741823")
        .expect("room again");
    let landed = store
        .commit(bulky_event(1 << 20))
        .expect("the disk has room again");
    assert_eq!(landed[0].seq, before + 1, "the refused commit left a gap");
    assert!(
        store.event_by_seq(before + 1).expect("read").is_some(),
        "the reader sees the commit"
    );
}

#[test]
fn only_a_disk_that_refuses_writes_reads_as_unwritable() {
    let sqlite = |code| {
        StoreError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(code),
            None,
        ))
    };
    let os = |errno| StoreError::Io(std::io::Error::from_raw_os_error(errno));
    for unwritable in [
        sqlite(rusqlite::ffi::SQLITE_FULL),
        sqlite(rusqlite::ffi::SQLITE_IOERR_WRITE),
        sqlite(rusqlite::ffi::SQLITE_IOERR_FSYNC),
        os(28),  // ENOSPC
        os(122), // EDQUOT
    ] {
        assert!(unwritable.is_unwritable(), "{unwritable}");
    }
    for writable in [
        sqlite(rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE),
        sqlite(rusqlite::ffi::SQLITE_BUSY),
        os(2), // ENOENT
        StoreError::Corrupt("bad row".into()),
        StoreError::NoArtifactDir,
    ] {
        assert!(!writable.is_unwritable(), "{writable}");
    }
}
