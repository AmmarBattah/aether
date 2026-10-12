//! artifact dedup scenarios and fixtures.
use super::*;

#[tokio::test]
async fn identical_outputs_share_bytes_but_keep_their_own_records() {
    // Two different commands whose output is byte-identical and large enough
    // to become an artifact.
    let big = "x".repeat(40_000);
    let h = harness_configured(
        vec![
            ScriptedClient::tool_call("bash", json!({"command": format!("printf '{big}'")})),
            ScriptedClient::tool_call(
                "bash",
                json!({"command": format!("printf '{big}' # second call")}),
            ),
            ScriptedClient::tool_call("complete_task", json!({"summary": "printed twice"})),
        ],
        |_| {},
        |config| {
            // Nothing is old enough to age out; the size limit does the work.
            config.retention = RetentionPolicy {
                max_age_days: None,
                max_total_bytes: Some(1),
            };
        },
    )
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("print a lot twice"), Actor::user())
        .expect("create");
    h.wait_until(agent.id, |s| s.is_terminal()).await;

    let events = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(agent.id))
        .expect("events");
    let created: Vec<_> = events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::ArtifactCreated { artifact } => Some(artifact.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(created.len(), 1, "identical bytes are stored once");
    let deduped: Vec<_> = events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::ArtifactDeduplicated {
                artifact,
                shared_with,
                ..
            } => Some((*artifact, *shared_with)),
            _ => None,
        })
        .collect();
    assert_eq!(
        deduped.len(),
        1,
        "the second output reused the stored bytes"
    );
    let (second, shared_with) = deduped[0];
    assert_eq!(
        shared_with,
        Some(created[0].id),
        "the log says what it shares"
    );

    // Bytes shared, records not: the second call owns its own record, with
    // its own agent, task and created_at, and its own readable payload.
    let first = h.store.artifact(created[0].id).expect("a").expect("record");
    let second = h.store.artifact(second).expect("b").expect("record");
    assert_ne!(first.id, second.id);
    assert_eq!(second.agent_id, Some(agent.id));
    assert!(second.created_at >= first.created_at);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.path, second.path, "one stored payload");
    for id in [first.id, second.id] {
        assert_eq!(
            h.store
                .read_artifact_bytes(id)
                .expect("bytes")
                .map(|b| b.len()),
            Some(first.size_bytes as usize),
            "both records read the shared payload"
        );
    }
    // Both are listed for the agent that made them, which is the point: a
    // task's report must name what that task produced.
    assert_eq!(
        h.store
            .artifacts(&ArtifactFilter {
                agent: Some(agent.id),
                ..Default::default()
            })
            .expect("list")
            .len(),
        2
    );

    // The size limit is one byte, so the sweep drops the one payload - and
    // both records with it, since there is only one payload to drop.
    assert_eq!(h.runtime.sweep_artifacts().expect("sweep"), 2);
    for id in [first.id, second.id] {
        let record = h.store.artifact(id).expect("artifact").expect("survives");
        assert!(
            record.evicted_at.is_some(),
            "the record says the bytes went"
        );
        assert_eq!(record.size_bytes, first.size_bytes, "history is intact");
        assert!(h.store.read_artifact_bytes(id).expect("bytes").is_none());
    }
    assert!(
        h.store.live_artifacts().expect("live").is_empty(),
        "an evicted artifact is not swept again"
    );
    assert_eq!(h.runtime.sweep_artifacts().expect("second sweep"), 0);
    assert!(
        h.store
            .artifacts(&ArtifactFilter {
                with_bytes_only: true,
                ..Default::default()
            })
            .expect("list")
            .is_empty()
    );
}

#[tokio::test]
async fn publication_keeps_shared_bytes_when_retention_races_its_commit() {
    use aether_core::artifact::ArtifactKind;
    use aether_runtime::artifacts::{ArtifactPublication, StoreArtifactSink};
    use aether_tools::ArtifactSink;
    use std::sync::mpsc;

    for disk in [false, true] {
        for streamed in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let inner: Arc<dyn Store> = if disk {
                Arc::new(SqliteStore::open(dir.path().join("aether.db")).unwrap())
            } else {
                Arc::new(SqliteStore::open_in_memory().unwrap())
            };
            let store = Arc::new(CommitsMidPass::new(inner, Hook::ArtifactPublicationCommit));
            let h = fixture::harness_full(
                vec![],
                |_| {},
                |config| {
                    config.retention = RetentionPolicy {
                        max_age_days: Some(1),
                        max_total_bytes: None,
                    };
                },
                None,
                None,
                Some(store.clone()),
                |_| {},
            )
            .await;
            let mut agent_spec = spec("artifact race");
            agent_spec.start = false;
            let (agent, _) = h.runtime.create_agent(agent_spec, Actor::user()).unwrap();
            let sink = StoreArtifactSink {
                runtime: h.runtime.clone(),
            };
            let bytes = b"shared bytes".to_vec();
            let mut first = sink
                .store(
                    ArtifactKind::CommandOutput,
                    "first",
                    "",
                    None,
                    bytes.clone(),
                    agent.id,
                    None,
                )
                .await
                .unwrap();
            first.created_at = now() - chrono::Duration::days(2);
            store
                .commit(Commit::new().mutate(Mutation::PutArtifact(first.clone())))
                .unwrap();

            let (sweeper_tx, sweeper_rx) = mpsc::channel();
            let runtime = h.runtime.clone();
            store.arm(Box::new(move |_| {
                let (finished_tx, finished_rx) = mpsc::channel();
                let sweeper = std::thread::spawn(move || {
                    let result = runtime.sweep_artifacts();
                    finished_tx.send(()).unwrap();
                    result
                });
                // Give retention the pre-commit gap. A protected publication
                // keeps it waiting until the new reference lands.
                let finished = match finished_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(()) => true,
                    Err(mpsc::RecvTimeoutError::Timeout) => false,
                    Err(error) => panic!("sweeper disconnected: {error}"),
                };
                sweeper_tx.send((sweeper, finished, finished_rx)).unwrap();
                Ok(())
            }));
            let second = if streamed {
                let source = dir.path().join("source.txt");
                std::fs::write(&source, &bytes).unwrap();
                sink.store_file(
                    ArtifactKind::CommandOutput,
                    "second",
                    "",
                    None,
                    &source,
                    agent.id,
                    None,
                )
                .await
                .unwrap()
            } else {
                h.runtime
                    .publish_artifact(
                        ArtifactPublication {
                            kind: ArtifactKind::CommandOutput,
                            name: "second".into(),
                            summary: String::new(),
                            content_type: None,
                            bytes: bytes.clone(),
                            agent: agent.id,
                            task: None,
                        },
                        Actor::agent(agent.id),
                    )
                    .await
                    .unwrap()
            };
            let (sweeper, finished, finished_rx) =
                sweeper_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            if !finished {
                finished_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            }
            assert_eq!(sweeper.join().unwrap().unwrap(), 0);
            assert_eq!(first.path, second.path);
            assert!(
                store
                    .artifact(first.id)
                    .unwrap()
                    .unwrap()
                    .evicted_at
                    .is_none()
            );
            assert!(
                store
                    .artifact(second.id)
                    .unwrap()
                    .unwrap()
                    .evicted_at
                    .is_none()
            );
            assert_eq!(
                store.read_artifact_bytes(second.id).unwrap(),
                Some(bytes),
                "disk={disk}, streamed={streamed}"
            );
            h.runtime.shutdown().await;
        }
    }
}

#[tokio::test]
async fn failed_publication_cleans_new_bytes_and_preserves_shared_bytes() {
    use aether_core::artifact::ArtifactKind;
    use aether_runtime::artifacts::StoreArtifactSink;
    use aether_tools::ArtifactSink;

    for disk in [false, true] {
        for streamed in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let inner: Arc<dyn Store> = if disk {
                Arc::new(SqliteStore::open(dir.path().join("aether.db")).unwrap())
            } else {
                Arc::new(SqliteStore::open_in_memory().unwrap())
            };
            let store = Arc::new(CommitsMidPass::new(inner, Hook::ArtifactPublicationCommit));
            let h = fixture::harness_full(
                vec![],
                |_| {},
                |_| {},
                None,
                None,
                Some(store.clone()),
                |_| {},
            )
            .await;
            let mut agent_spec = spec("failed publication");
            agent_spec.start = false;
            let (agent, _) = h.runtime.create_agent(agent_spec, Actor::user()).unwrap();
            let sink = StoreArtifactSink {
                runtime: h.runtime.clone(),
            };
            let source = dir.path().join("source");
            std::fs::write(&source, b"content").unwrap();
            for shared in [false, true] {
                let twin = if shared {
                    Some(
                        sink.store(
                            ArtifactKind::Result,
                            "first",
                            "",
                            None,
                            b"content".to_vec(),
                            agent.id,
                            None,
                        )
                        .await
                        .unwrap(),
                    )
                } else {
                    None
                };
                let (id_tx, id_rx) = std::sync::mpsc::channel();
                let artifact_dir = dir.path().join("artifacts");
                store.arm(Box::new(move |_| {
                    // Capture both staged and promoted payloads before rejecting the commit.
                    if disk {
                        let mut paths: Vec<_> = std::fs::read_dir(&artifact_dir)
                            .unwrap()
                            .map(|entry| entry.unwrap().path())
                            .filter(|path| path.is_file())
                            .collect();
                        for namespace in std::fs::read_dir(artifact_dir.join(".staging")).unwrap() {
                            paths.extend(
                                std::fs::read_dir(namespace.unwrap().path())
                                    .unwrap()
                                    .map(|entry| entry.unwrap().path()),
                            );
                        }
                        id_tx.send(paths).unwrap();
                    }
                    Err(aether_store::StoreError::Corrupt(
                        "publication rejected".into(),
                    ))
                }));
                let source = dir.path().join("source");
                let error = if streamed {
                    sink.store_file(
                        ArtifactKind::Result,
                        "failed",
                        "",
                        None,
                        &source,
                        agent.id,
                        None,
                    )
                    .await
                    .unwrap_err()
                } else {
                    sink.store(
                        ArtifactKind::Result,
                        "failed",
                        "",
                        None,
                        b"content".to_vec(),
                        agent.id,
                        None,
                    )
                    .await
                    .unwrap_err()
                };
                assert!(error.to_string().contains("publication rejected"));
                if disk {
                    let paths = id_rx.recv().unwrap();
                    assert_eq!(paths.len(), 1);
                    assert_eq!(paths[0].exists(), shared);
                }
                if let Some(twin) = twin {
                    assert_eq!(
                        store.read_artifact_bytes(twin.id).unwrap(),
                        Some(b"content".to_vec())
                    );
                }
            }
            h.runtime.shutdown().await;
        }
    }
}

#[tokio::test]
async fn committed_publication_succeeds_when_promotion_needs_startup_retry() {
    use aether_core::artifact::ArtifactKind;
    use aether_runtime::artifacts::StoreArtifactSink;
    use aether_tools::ArtifactSink;
    for streamed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("aether.db");
        let inner: Arc<dyn Store> = Arc::new(SqliteStore::open(&database).unwrap());
        let store = Arc::new(CommitsMidPass::new(inner, Hook::ArtifactPromotion));
        let h = fixture::harness_full(
            vec![],
            |_| {},
            |_| {},
            None,
            None,
            Some(store.clone()),
            |_| {},
        )
        .await;
        let mut agent_spec = spec("deferred artifact promotion");
        agent_spec.start = false;
        let (agent, _) = h.runtime.create_agent(agent_spec, Actor::user()).unwrap();
        let sink = StoreArtifactSink {
            runtime: h.runtime.clone(),
        };
        store.arm(Box::new(|_| {
            Err(std::io::Error::new(std::io::ErrorKind::StorageFull, "promotion rejected").into())
        }));
        let bytes = b"committed payload";
        let artifact = if streamed {
            let source = dir.path().join("source");
            std::fs::write(&source, bytes).unwrap();
            sink.store_file(
                ArtifactKind::Result,
                "result",
                "",
                None,
                &source,
                agent.id,
                None,
            )
            .await
            .unwrap()
        } else {
            sink.store(
                ArtifactKind::Result,
                "result",
                "",
                None,
                bytes.to_vec(),
                agent.id,
                None,
            )
            .await
            .unwrap()
        };
        assert!(store.fired());
        assert!(!artifact.path.as_ref().unwrap().exists());
        assert_eq!(
            store.read_artifact_bytes(artifact.id).unwrap(),
            Some(bytes.to_vec())
        );
        assert_eq!(
            store.artifacts(&ArtifactFilter::default()).unwrap().len(),
            1
        );
        let events = store
            .events(0, 100, &EventFilter::for_agent(agent.id))
            .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, EventKind::ArtifactCreated { .. }))
                .count(),
            1
        );
        let reopened = SqliteStore::open(&database).unwrap();
        assert!(artifact.path.as_ref().unwrap().exists());
        assert_eq!(
            reopened.read_artifact_bytes(artifact.id).unwrap(),
            Some(bytes.to_vec())
        );
        assert_eq!(
            reopened
                .artifacts(&ArtifactFilter::default())
                .unwrap()
                .len(),
            1
        );
        h.runtime.shutdown().await;
    }
}
