use super::*;

#[cfg(unix)]
#[test]
fn artifact_write_does_not_follow_a_stale_staging_symlink() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("aether.db")).unwrap();
    let id = ArtifactId::new();
    let path = store.artifact_path(id).unwrap();
    let victim = dir.path().join("unrelated.txt");
    std::fs::write(&victim, b"keep this").unwrap();
    let stale = path.with_extension("tmp");
    symlink(&victim, &stale).unwrap();

    store.write_artifact_bytes(id, b"artifact").unwrap();

    assert_eq!(std::fs::read(&victim).unwrap(), b"keep this");
    assert_eq!(std::fs::read(&path).unwrap(), b"artifact");
    assert!(stale.is_symlink());
}

#[test]
fn streamed_artifact_import_is_immutable_hashed_and_deduplicated() {
    use aether_core::artifact::ArtifactKind;
    use sha2::Digest;
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("artifacts.db")).unwrap();
    let source = dir.path().join("shell.log");
    let bytes = vec![b'x'; 2 << 20];
    std::fs::write(&source, &bytes).unwrap();
    let id = ArtifactId::new();
    let first = store.import_artifact_file(id, &source).unwrap();
    assert_eq!(first.size_bytes, bytes.len() as u64);
    assert_eq!(first.digest, hex::encode(sha2::Sha256::digest(&bytes)));
    assert!(first.shared_with.is_none());
    let record = Artifact {
        id,
        kind: ArtifactKind::CommandOutput,
        name: "log".into(),
        path: Some(first.path.clone()),
        summary: "test".into(),
        size_bytes: first.size_bytes,
        content_type: Some("text/plain".into()),
        digest: Some(first.digest.clone()),
        created_by: Actor::System,
        agent_id: None,
        task_id: None,
        created_at: now(),
        evicted_at: None,
    };
    store
        .commit(Commit::new().mutate(Mutation::PutArtifact(record)))
        .unwrap();
    store
        .finish_artifact_publication(first.publication.as_ref().unwrap())
        .unwrap();
    let second = store
        .import_artifact_file(ArtifactId::new(), &source)
        .unwrap();
    assert_eq!(second.shared_with, Some(id));
    assert_eq!(second.path, first.path);
    assert_eq!(
        std::fs::read_dir(first.path.parent().unwrap())
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().file_type().unwrap().is_file())
            .count(),
        1
    );
    std::fs::write(&source, b"changed").unwrap();
    assert_eq!(
        store
            .read_artifact_range(id, (bytes.len() - 4) as u64, 4)
            .unwrap(),
        Some(b"xxxx".to_vec())
    );
}

#[test]
fn artifact_ranges_seek_on_disk_and_handle_missing_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("range.db")).unwrap();
    let id = ArtifactId::new();
    let path = store.write_artifact_bytes(id, b"head").unwrap();
    // A sparse payload catches an implementation that reads the entire file
    // merely to return a page near its end.
    use std::io::{Seek, SeekFrom, Write};
    let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    let offset = 1 << 32;
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.write_all(b"tail").unwrap();
    assert_eq!(
        store.read_artifact_range(id, offset, 4).unwrap(),
        Some(b"tail".to_vec())
    );
    assert_eq!(
        store.read_artifact_range(id, offset + 4, 4).unwrap(),
        Some(vec![])
    );
    assert_eq!(
        store.read_artifact_range(id, 0, 2).unwrap(),
        Some(b"he".to_vec())
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(store.read_artifact_range(id, 0, 4).unwrap(), None);
}

#[test]
fn persists_across_reopen_and_stores_artifacts() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("aether.db");
    let a = agent();
    let art_id = ArtifactId::new();
    let instance;
    {
        let store = SqliteStore::open(&path).expect("open");
        instance = store.instance_id().expect("id");
        store
            .commit(
                Commit::new()
                    .event(EventDraft::new(Actor::System, EventKind::AgentReady).agent(a.id))
                    .mutate(Mutation::UpsertAgent(a.clone())),
            )
            .expect("commit");
        store.write_artifact_bytes(art_id, b"hello").expect("write");
    }
    let store = SqliteStore::open(&path).expect("reopen");
    assert_eq!(store.instance_id().expect("id"), instance);
    assert_eq!(store.last_seq().expect("seq"), 1);
    assert_eq!(store.agent(a.id).expect("agent"), Some(a));
    assert_eq!(
        store.read_artifact_bytes(art_id).expect("read"),
        Some(b"hello".to_vec())
    );
    assert_eq!(
        store
            .read_artifact_bytes(ArtifactId::new())
            .expect("missing"),
        None
    );
}

#[test]
fn an_output_matching_an_evicted_artifact_is_stored_afresh() {
    let store = SqliteStore::open_in_memory().expect("open");
    let mut art = artifact_named("first");
    art.digest = Some("abc123".into());
    store
        .commit(Commit::new().mutate(Mutation::PutArtifact(art.clone())))
        .expect("put");
    assert_eq!(
        store
            .artifact_by_digest("abc123")
            .expect("lookup")
            .map(|a| a.id),
        Some(art.id)
    );
    store.evict_artifact_bytes(art.id).expect("evict");
    assert!(
        store
            .artifact_by_digest("abc123")
            .expect("lookup")
            .is_none(),
        "evicted bytes cannot back a new reference"
    );
    assert!(
        store.live_artifacts().expect("live").is_empty(),
        "an evicted artifact is not live"
    );
    assert_eq!(
        store
            .artifact(art.id)
            .expect("record")
            .map(|a| a.evicted_at.is_some()),
        Some(true),
        "the record stays; only the payload is gone"
    );
}

#[test]
fn the_artifact_filter_narrows_by_agent_task_and_bytes() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = agent();
    let task = Task::new("t", Actor::user(), a.run, now());
    let mut mine = artifact_named("mine");
    mine.agent_id = Some(a.id);
    mine.task_id = Some(task.id);
    let mut theirs = artifact_named("theirs");
    theirs.agent_id = Some(AgentId::new());
    for art in [&mine, &theirs] {
        store
            .commit(Commit::new().mutate(Mutation::PutArtifact(art.clone())))
            .expect("put");
    }
    let by_agent = store
        .artifacts(&ArtifactFilter {
            agent: Some(a.id),
            ..Default::default()
        })
        .expect("by agent");
    assert_eq!(by_agent.len(), 1);
    assert_eq!(by_agent[0].id, mine.id);
    assert_eq!(
        store
            .artifacts(&ArtifactFilter {
                task: Some(task.id),
                ..Default::default()
            })
            .expect("by task")
            .len(),
        1
    );
    // The limit is clamped into range rather than trusted.
    assert_eq!(
        store
            .artifacts(&ArtifactFilter {
                limit: 0,
                ..Default::default()
            })
            .expect("clamped")
            .len(),
        1
    );
    store.evict_artifact_bytes(mine.id).expect("evict");
    assert_eq!(
        store
            .artifacts(&ArtifactFilter {
                with_bytes_only: true,
                ..Default::default()
            })
            .expect("live only")
            .len(),
        1,
        "only the artifact that still has bytes is listed"
    );
}

#[test]
fn reopening_retries_interrupted_eviction_without_removing_shared_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("aether.db");
    let mut first = artifact_named("first");
    let mut twin = artifact_named("twin");
    let path;
    {
        let store = SqliteStore::open(&database).unwrap();
        path = store.write_artifact_bytes(first.id, b"shared").unwrap();
        first.path = Some(path.clone());
        first.digest = Some("shared".into());
        twin.path = Some(path.clone());
        // Cleanup must check actual paths, even if legacy metadata has a different digest.
        twin.digest = Some("different".into());
        store
            .commit(
                Commit::new()
                    .mutate(Mutation::PutArtifact(first.clone()))
                    .mutate(Mutation::PutArtifact(twin.clone())),
            )
            .unwrap();
        first.evicted_at = Some(now());
        store
            .commit(Commit::new().mutate(Mutation::PutArtifact(first.clone())))
            .unwrap();
    }
    {
        let store = SqliteStore::open(&database).unwrap();
        assert_eq!(
            store.read_artifact_bytes(twin.id).unwrap(),
            Some(b"shared".to_vec())
        );
        twin.evicted_at = Some(now());
        store
            .commit(Commit::new().mutate(Mutation::PutArtifact(twin)))
            .unwrap();
    }
    let store = SqliteStore::open(&database).unwrap();
    assert!(
        !path.exists(),
        "the interrupted unlink must be retried on open"
    );
    store.evict_artifact_bytes(first.id).unwrap();
    assert!(
        store
            .artifact(first.id)
            .unwrap()
            .unwrap()
            .evicted_at
            .is_some()
    );
}

#[test]
fn unpublished_blob_cleanup_preserves_every_live_path_reference() {
    for disk in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = if disk {
            SqliteStore::open(dir.path().join("aether.db")).unwrap()
        } else {
            SqliteStore::open_in_memory().unwrap()
        };
        let id = ArtifactId::new();
        store.write_artifact_bytes(id, b"unpublished").unwrap();
        store.remove_unreferenced_artifact_bytes(id).unwrap();
        assert!(store.read_artifact_bytes(id).unwrap().is_none());
        let path = store.write_artifact_bytes(id, b"referenced").unwrap();
        let mut twin = artifact_named("twin");
        twin.path = Some(path);
        store
            .commit(Commit::new().mutate(Mutation::PutArtifact(twin.clone())))
            .unwrap();
        store.remove_unreferenced_artifact_bytes(id).unwrap();
        assert_eq!(
            store.read_artifact_bytes(twin.id).unwrap(),
            Some(b"referenced".to_vec())
        );
    }
}

#[test]
fn startup_cleanup_leaves_unrecorded_and_external_files_alone() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("aether.db");
    let external = dir.path().join("external");
    std::fs::write(&external, b"external").unwrap();
    let unrecorded;
    {
        let store = SqliteStore::open(&database).unwrap();
        unrecorded = store
            .write_artifact_bytes(ArtifactId::new(), b"unrecorded")
            .unwrap();
        let mut artifact = artifact_named("external");
        artifact.path = Some(external.clone());
        artifact.evicted_at = Some(now());
        store
            .commit(Commit::new().mutate(Mutation::PutArtifact(artifact)))
            .unwrap();
    }
    let _store = SqliteStore::open(&database).unwrap();
    assert_eq!(std::fs::read(unrecorded).unwrap(), b"unrecorded");
    assert_eq!(std::fs::read(external).unwrap(), b"external");
}

#[test]
fn startup_eviction_cleanup_reports_unlink_failures_and_retries_later() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("aether.db");
    let mut artifact = artifact_named("interrupted");
    let path;
    {
        let store = SqliteStore::open(&database).unwrap();
        path = store.artifact_path(artifact.id).unwrap();
        std::fs::create_dir(&path).unwrap();
        artifact.path = Some(path.clone());
        artifact.evicted_at = Some(now());
        store
            .commit(Commit::new().mutate(Mutation::PutArtifact(artifact.clone())))
            .unwrap();
    }
    {
        let store = SqliteStore::open(&database).unwrap();
        assert!(path.is_dir());
        let pending: bool = store
            .conn
            .lock()
            .query_row(
                "SELECT unlink_pending FROM artifacts WHERE id = ?1",
                [artifact.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert!(pending);
    }
    std::fs::remove_dir(&path).unwrap();
    std::fs::write(&path, b"retry").unwrap();
    {
        let store = SqliteStore::open(&database).unwrap();
        assert!(!path.exists());
        let pending: bool = store
            .conn
            .lock()
            .query_row(
                "SELECT unlink_pending FROM artifacts WHERE id = ?1",
                [artifact.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!pending);
    }
    // Confirmed unlinks must not revisit old paths on every startup.
    std::fs::write(&path, b"unrecorded replacement").unwrap();
    let _store = SqliteStore::open(&database).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"unrecorded replacement");
}

#[test]
fn eviction_tracks_paths_even_when_digest_metadata_disagrees() {
    for shared_path in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("aether.db")).unwrap();
        let mut first = artifact_named("first");
        let mut second = artifact_named("second");
        let first_path = store.write_artifact_bytes(first.id, b"bytes").unwrap();
        first.path = Some(first_path.clone());
        first.digest = Some("digest".into());
        if shared_path {
            second.path = Some(first_path.clone());
            second.digest = Some("different".into());
        } else {
            second.path = Some(store.write_artifact_bytes(second.id, b"bytes").unwrap());
            second.digest = first.digest.clone();
        }
        store
            .commit(
                Commit::new()
                    .mutate(Mutation::PutArtifact(first.clone()))
                    .mutate(Mutation::PutArtifact(second.clone())),
            )
            .unwrap();
        store.evict_artifact_bytes(first.id).unwrap();
        assert_eq!(first_path.exists(), shared_path);
        assert!(
            !store
                .pending_artifact_cleanup()
                .unwrap()
                .pending_unlinks
                .contains(&first.id)
        );
        assert_eq!(
            store.read_artifact_bytes(second.id).unwrap(),
            Some(b"bytes".to_vec())
        );
        store.evict_artifact_bytes(second.id).unwrap();
        assert!(!first_path.exists());
        assert!(!second.path.unwrap().exists());
        assert!(
            store
                .pending_artifact_cleanup()
                .unwrap()
                .pending_unlinks
                .is_empty()
        );
    }
}

#[test]
fn interrupted_publication_is_reclaimed_on_open() {
    for streamed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("aether.db");
        let store = SqliteStore::open(&database).unwrap();
        let id = ArtifactId::new();
        let publication = if streamed {
            let source = dir.path().join("source");
            std::fs::write(&source, b"uncommitted").unwrap();
            store
                .import_artifact_file(id, &source)
                .unwrap()
                .publication
                .unwrap()
        } else {
            store.stage_artifact_bytes(id, b"uncommitted").unwrap()
        };
        let staged = publication.staged.as_ref().unwrap().0.clone();
        assert!(staged.exists());
        assert!(!publication.path.exists());
        drop(publication);
        // Reopening is also safe while another handle to the database remains alive.
        let reopened = SqliteStore::open(&database).unwrap();
        assert!(!staged.exists());
        assert!(reopened.artifact(id).unwrap().is_none());
        assert!(reopened.read_artifact_bytes(id).unwrap().is_none());
    }
}

#[test]
fn committed_publication_is_recovered_on_open_including_shared_references() {
    for linked in [false, true] {
        for shared in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let database = dir.path().join("aether.db");
            let store = SqliteStore::open(&database).unwrap();
            let mut artifact = artifact_named("interrupted");
            let publication = store
                .stage_artifact_bytes(artifact.id, b"committed")
                .unwrap();
            let staged = publication.staged.as_ref().unwrap().0.clone();
            artifact.path = Some(publication.path.clone());
            let reader_id = if shared {
                let mut twin = artifact_named("shared");
                twin.path = artifact.path.clone();
                let reader_id = twin.id;
                artifact.evicted_at = Some(now());
                store
                    .commit(Commit::new().mutate(Mutation::PutArtifact(twin)))
                    .unwrap();
                reader_id
            } else {
                artifact.id
            };
            store
                .commit(Commit::new().mutate(Mutation::PutArtifact(artifact)))
                .unwrap();
            assert_eq!(
                store.read_artifact_bytes(reader_id).unwrap(),
                Some(b"committed".to_vec())
            );
            assert_eq!(
                store.read_artifact_range(reader_id, 2, 3).unwrap(),
                Some(b"mmi".to_vec())
            );
            assert!(store.artifact_bytes_exist(reader_id).unwrap());
            if linked {
                // Interruption between the no-clobber link and removal of the staging name.
                std::fs::hard_link(&staged, &publication.path).unwrap();
            }
            let target = publication.path.clone();
            drop(publication);
            drop(store);
            let reopened = SqliteStore::open(&database).unwrap();
            assert!(!staged.exists());
            assert_eq!(std::fs::read(target).unwrap(), b"committed");
            assert_eq!(
                reopened.read_artifact_bytes(reader_id).unwrap(),
                Some(b"committed".to_vec())
            );
            reopened.evict_artifact_bytes(reader_id).unwrap();
            assert!(reopened.read_artifact_bytes(reader_id).unwrap().is_none());
        }
    }
}

#[test]
fn staging_recovery_preserves_foreign_and_live_files() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("aether.db");
    let store = SqliteStore::open(&database).unwrap();
    let foreign = SqliteStore::open(dir.path().join("other.db")).unwrap();
    let abandoned_foreign = foreign
        .stage_artifact_bytes(ArtifactId::new(), b"foreign staged")
        .unwrap();
    let foreign_staged = abandoned_foreign.staged.as_ref().unwrap().0.clone();
    drop(abandoned_foreign);
    let unrecorded = store
        .write_artifact_bytes(ArtifactId::new(), b"legacy unrecorded")
        .unwrap();
    let mut artifact = artifact_named("live");
    let live = store.write_artifact_bytes(artifact.id, b"live").unwrap();
    artifact.path = Some(live.clone());
    store
        .commit(Commit::new().mutate(Mutation::PutArtifact(artifact)))
        .unwrap();
    let mut foreign_artifact = artifact_named("foreign live");
    let foreign_live = foreign
        .write_artifact_bytes(foreign_artifact.id, b"foreign live")
        .unwrap();
    foreign_artifact.path = Some(foreign_live.clone());
    foreign
        .commit(Commit::new().mutate(Mutation::PutArtifact(foreign_artifact)))
        .unwrap();
    let publication = store
        .stage_artifact_bytes(ArtifactId::new(), b"live staging path")
        .unwrap();
    let live_staged = publication.staged.as_ref().unwrap().0.clone();
    let mut direct = artifact_named("direct staging reference");
    direct.path = Some(live_staged.clone());
    store
        .commit(Commit::new().mutate(Mutation::PutArtifact(direct)))
        .unwrap();
    drop(publication);
    let _reopened = SqliteStore::open(&database).unwrap();
    assert_eq!(std::fs::read(live_staged).unwrap(), b"live staging path");
    assert_eq!(std::fs::read(foreign_staged).unwrap(), b"foreign staged");
    assert_eq!(std::fs::read(foreign_live).unwrap(), b"foreign live");
    assert_eq!(std::fs::read(live).unwrap(), b"live");
    assert_eq!(std::fs::read(unrecorded).unwrap(), b"legacy unrecorded");
}

#[test]
fn concurrent_publications_are_not_swept_before_or_after_commit() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("aether.db");
    let store = SqliteStore::open(&database).unwrap();
    let mut publications = Vec::new();
    for committed in [false, true] {
        let mut artifact = artifact_named("in flight");
        let publication = store
            .stage_artifact_bytes(artifact.id, b"in flight")
            .unwrap();
        if committed {
            artifact.path = Some(publication.path.clone());
            store
                .commit(Commit::new().mutate(Mutation::PutArtifact(artifact.clone())))
                .unwrap();
        }
        publications.push((artifact, publication, committed));
    }
    let reopened = std::thread::spawn(move || SqliteStore::open(database).unwrap())
        .join()
        .unwrap();
    for (artifact, publication, committed) in publications {
        assert!(publication.staged.as_ref().unwrap().0.exists());
        assert!(!publication.path.exists());
        if !committed {
            let mut artifact = artifact;
            artifact.path = Some(publication.path.clone());
            store
                .commit(Commit::new().mutate(Mutation::PutArtifact(artifact)))
                .unwrap();
        }
        store.finish_artifact_publication(&publication).unwrap();
        assert_eq!(std::fs::read(&publication.path).unwrap(), b"in flight");
    }
    drop(reopened);
}

#[test]
fn promotion_conflict_preserves_live_destination_and_recoverable_staging() {
    for existing in [b"existing".as_slice(), b"new".as_slice()] {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("aether.db");
        let store = SqliteStore::open(&database).unwrap();
        let mut artifact = artifact_named("conflict");
        let publication = store.stage_artifact_bytes(artifact.id, b"new").unwrap();
        let staged = publication.staged.as_ref().unwrap().0.clone();
        std::fs::write(&publication.path, existing).unwrap();
        artifact.path = Some(publication.path.clone());
        store
            .commit(Commit::new().mutate(Mutation::PutArtifact(artifact)))
            .unwrap();
        assert!(
            store
                .finish_artifact_publication(&publication)
                .unwrap_err()
                .to_string()
                .contains("destination already exists")
        );
        assert_eq!(std::fs::read(&publication.path).unwrap(), existing);
        let target = publication.path.clone();
        drop(publication);
        let abandoned = store
            .stage_artifact_bytes(ArtifactId::new(), b"abandoned")
            .unwrap();
        let orphan = abandoned.staged.as_ref().unwrap().0.clone();
        drop(abandoned);
        let reopened = SqliteStore::open(&database).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), existing);
        assert_eq!(std::fs::read(&staged).unwrap(), b"new");
        assert!(!orphan.exists());
        drop(reopened);
        std::fs::remove_file(&target).unwrap();
        let _reopened = SqliteStore::open(database).unwrap();
        assert!(!staged.exists());
        assert_eq!(std::fs::read(target).unwrap(), b"new");
    }
}

#[test]
fn transfer_placeholder_does_not_commit_staged_payload_or_its_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("aether.db");
    let store = SqliteStore::open(&database).unwrap();
    let mut artifact = artifact_named("imported placeholder");
    artifact.path = None;
    store
        .commit(Commit::new().mutate(Mutation::PutArtifact(artifact.clone())))
        .unwrap();
    let publication = store
        .stage_artifact_bytes(artifact.id, b"metadata not committed")
        .unwrap();
    let staged = publication.staged.as_ref().unwrap().0.clone();
    assert!(store.read_artifact_bytes(artifact.id).unwrap().is_none());
    drop(publication);
    let reopened = SqliteStore::open(database).unwrap();
    assert!(!staged.exists());
    assert_eq!(reopened.artifact(artifact.id).unwrap(), Some(artifact));
}

#[cfg(unix)]
#[test]
fn database_aliases_share_the_same_staging_namespace() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("aether.db");
    let alias = dir.path().join("alias.db");
    let store = SqliteStore::open(&database).unwrap();
    symlink(&database, &alias).unwrap();
    let publication = store
        .stage_artifact_bytes(ArtifactId::new(), b"abandoned")
        .unwrap();
    let staged = publication.staged.as_ref().unwrap().0.clone();
    drop(publication);
    let reopened = SqliteStore::open(alias).unwrap();
    assert_eq!(store.staging_dir, reopened.staging_dir);
    assert!(!staged.exists());
}

#[test]
fn another_store_cannot_promote_a_publication_lease() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("aether.db")).unwrap();
    let foreign = SqliteStore::open(dir.path().join("other.db")).unwrap();
    let mut artifact = artifact_named("owned");
    let publication = store.stage_artifact_bytes(artifact.id, b"owned").unwrap();
    artifact.path = Some(publication.path.clone());
    foreign
        .commit(Commit::new().mutate(Mutation::PutArtifact(artifact)))
        .unwrap();
    assert!(
        foreign
            .finish_artifact_publication(&publication)
            .unwrap_err()
            .to_string()
            .contains("another store")
    );
    assert!(publication.staged.as_ref().unwrap().0.exists());
    assert!(!publication.path.exists());
}

#[cfg(unix)]
#[test]
fn staging_never_follows_or_sweeps_a_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("aether.db");
    let store = SqliteStore::open(&database).unwrap();
    let id = ArtifactId::new();
    let victim = dir.path().join("victim");
    std::fs::write(&victim, b"preserve").unwrap();
    let staged = store.staging_dir.as_ref().unwrap().join(id.to_string());
    symlink(&victim, &staged).unwrap();
    assert!(store.stage_artifact_bytes(id, b"replace").is_err());
    let _reopened = SqliteStore::open(database).unwrap();
    assert!(staged.is_symlink());
    assert_eq!(std::fs::read(victim).unwrap(), b"preserve");
}

#[test]
fn publication_process_exit_is_recovered_without_running_cleanup() {
    const DATABASE: &str = "AETHER_TEST_PUBLICATION_DATABASE";
    const COMMITTED: &str = "AETHER_TEST_PUBLICATION_COMMITTED";
    if let Some(database) = std::env::var_os(DATABASE) {
        let store = SqliteStore::open(database).unwrap();
        let mut artifact = artifact_named("process exit");
        let publication = store
            .stage_artifact_bytes(artifact.id, b"process exit")
            .unwrap();
        if std::env::var(COMMITTED).unwrap() == "true" {
            artifact.path = Some(publication.path.clone());
            store
                .commit(Commit::new().mutate(Mutation::PutArtifact(artifact)))
                .unwrap();
        }
        // Bypass every destructor, like a crashed publisher.
        std::process::exit(0);
    }
    for committed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("aether.db");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "sqlite::tests::artifacts::publication_process_exit_is_recovered_without_running_cleanup"])
            .env(DATABASE, &database)
            .env(COMMITTED, committed.to_string())
            .status().unwrap();
        assert!(status.success());
        let namespaces: Vec<_> = std::fs::read_dir(dir.path().join("artifacts/.staging"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(namespaces.len(), 1);
        let stages: Vec<_> = std::fs::read_dir(&namespaces[0])
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(stages.len(), 1);
        let id: ArtifactId = stages[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        let reopened = SqliteStore::open(database).unwrap();
        assert!(!stages[0].exists());
        assert_eq!(
            reopened.read_artifact_bytes(id).unwrap(),
            committed.then(|| b"process exit".to_vec())
        );
        assert_eq!(reopened.artifact(id).unwrap().is_some(), committed);
    }
}

#[cfg(unix)]
#[test]
fn staging_namespace_symlinks_cannot_redirect_cleanup_to_foreign_files() {
    use std::os::unix::fs::symlink;
    for redirect_root in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("aether.db");
        let store = SqliteStore::open(&database).unwrap();
        let namespace = store.staging_dir.clone().unwrap();
        let root = namespace.parent().unwrap().to_path_buf();
        let foreign = dir.path().join("foreign");
        std::fs::create_dir(&foreign).unwrap();
        let foreign_file = foreign.join(ArtifactId::new().to_string());
        std::fs::write(&foreign_file, b"foreign").unwrap();
        std::fs::remove_dir(&namespace).unwrap();
        let redirect = if redirect_root {
            std::fs::remove_dir(&root).unwrap();
            root
        } else {
            namespace
        };
        symlink(&foreign, &redirect).unwrap();
        assert!(SqliteStore::open(database).is_err());
        assert_eq!(std::fs::read(foreign_file).unwrap(), b"foreign");
    }
}

#[test]
fn promotion_preserves_pending_eviction_without_resurrecting_completed_unlinks() {
    for pending in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("aether.db");
        let store = SqliteStore::open(&database).unwrap();
        let mut artifact = artifact_named("evicted publication");
        let publication = store.stage_artifact_bytes(artifact.id, b"evicted").unwrap();
        let staged = publication.staged.as_ref().unwrap().0.clone();
        artifact.path = Some(publication.path.clone());
        artifact.evicted_at = Some(now());
        store
            .commit(Commit::new().mutate(Mutation::PutArtifact(artifact)))
            .unwrap();
        if !pending {
            store
                .conn
                .lock()
                .execute("UPDATE artifacts SET unlink_pending = 0", [])
                .unwrap();
        }
        assert_eq!(
            store.finish_artifact_publication(&publication).is_ok(),
            pending
        );
        assert_eq!(publication.path.exists(), pending);
        assert!(
            !staged.exists(),
            "completed eviction must reclaim the staging name immediately"
        );
        let target = publication.path.clone();
        drop(publication);
        drop(store);
        let _reopened = SqliteStore::open(database).unwrap();
        assert!(!staged.exists());
        assert!(!target.exists());
    }
}

#[test]
fn failed_staged_writes_reclaim_partial_bytes_and_preserve_the_original_error() {
    use std::io::Write;
    for cleanup_fails in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("aether.db")).unwrap();
        let id = ArtifactId::new();
        let staged = store.staging_dir.as_ref().unwrap().join(id.to_string());
        let error = store
            .write_staged_blob(id, |file| {
                file.write_all(b"partial")?;
                if cleanup_fails {
                    std::fs::remove_file(&staged)?;
                    std::fs::create_dir(&staged)?;
                }
                Err::<(), _>(std::io::Error::from(std::io::ErrorKind::StorageFull).into())
            })
            .unwrap_err();
        assert!(
            matches!(error, StoreError::Io(error) if error.kind() == std::io::ErrorKind::StorageFull)
        );
        assert_eq!(staged.exists(), cleanup_fails);
        assert!(!store.artifact_path(id).unwrap().exists());
    }
}

#[test]
fn failed_streamed_source_reads_remove_the_uncommitted_staging_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("aether.db")).unwrap();
    let id = ArtifactId::new();
    assert!(store.import_artifact_file(id, dir.path()).is_err());
    assert!(
        !store
            .staging_dir
            .as_ref()
            .unwrap()
            .join(id.to_string())
            .exists()
    );
}

#[test]
fn cleanup_preview_keeps_pending_unlinks_and_staged_publications_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("preview.db")).unwrap();
    let mut artifact = artifact_named("pending");
    artifact.evicted_at = Some(now());
    let path = store
        .write_artifact_bytes(artifact.id, b"pending bytes")
        .unwrap();
    artifact.path = Some(path.clone());
    store
        .commit(Commit::new().mutate(Mutation::PutArtifact(artifact.clone())))
        .unwrap();
    let staged_id = ArtifactId::new();
    let publication = store.stage_artifact_bytes(staged_id, b"in flight").unwrap();
    let staged = publication.staged.as_ref().unwrap().0.clone();
    let changes = store.conn.lock().total_changes();
    let preview = store.pending_artifact_cleanup().unwrap();
    assert_eq!(preview.pending_unlinks, vec![artifact.id]);
    assert_eq!(preview.staged_publications, vec![staged_id]);
    assert_eq!(std::fs::read(path).unwrap(), b"pending bytes");
    assert_eq!(std::fs::read(staged).unwrap(), b"in flight");
    assert_eq!(store.conn.lock().total_changes(), changes);
    assert_eq!(store.pending_artifact_cleanup().unwrap(), preview);
}
