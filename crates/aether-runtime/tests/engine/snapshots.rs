//! Exact invocation inputs and bounded transcript storage.
use super::*;
/// §6.2 holds on the engine path too. The engine keeps its own history, so
/// the snapshot records what Aether sent it - which is exactly what has to
/// survive, because it is all Aether will ever be able to say about the turn.
#[tokio::test]
async fn an_engine_turns_snapshot_reconstructs_too() {
    use aether_core::snapshot::ContextLayer;

    let h = harness(Reply::ToolRun).await;
    let id = chat_agent(&h);
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    h.runtime
        .send_message(id, vec![ContentBlock::text("what is here?")], Actor::user())
        .expect("send");
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting && a.stats.turns >= 1
    })
    .await;

    let announced: Vec<(aether_core::id::SnapshotId, u32, Vec<ContextLayer>)> = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events")
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::SnapshotCreated {
                snapshot,
                token_estimate,
                layers,
                ..
            } => Some((snapshot, token_estimate, layers)),
            _ => None,
        })
        .collect();
    assert!(
        !announced.is_empty(),
        "the engine turn recorded no snapshot"
    );
    for (sid, token_estimate, layers) in announced {
        let stored = h
            .store
            .snapshot(sid)
            .expect("snapshot")
            .expect("every announced snapshot is stored");
        let json = serde_json::to_string(&stored).expect("serialize");
        let back: aether_core::snapshot::InvocationSnapshot =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(serde_json::to_string(&back).expect("reserialize"), json);
        assert_eq!(stored.token_estimate, token_estimate);
        assert_eq!(stored.layers, layers);
        assert!(
            h.systems.lock().unwrap().contains(&stored.system_text()),
            "the engine must receive the exact policy and state its snapshot records"
        );
        assert_eq!(
            stored.token_estimate,
            stored.layers.iter().map(|layer| layer.tokens).sum::<u32>()
        );
        let tail = stored
            .layers
            .iter()
            .find(|layer| layer.kind == aether_core::snapshot::LayerKind::RecentTail)
            .expect("tail layer");
        assert_eq!(
            tail.tokens,
            aether_context::estimate::TokenEstimator.messages(&stored.messages)
        );
        let recorded_user = stored
            .messages
            .last()
            .expect("recorded engine input")
            .text();
        assert!(
            h.prompts.lock().unwrap().contains(&recorded_user),
            "the snapshot must preserve the exact user input including runtime status"
        );
        assert_eq!(
            stored.adapter.as_deref(),
            Some("scripted/1"),
            "the snapshot names the engine adapter that shaped the turn"
        );
    }
}

/// Native snapshots store only sent input, with or without a checkpoint.
/// Durable transcript events remain available for recovery and handoff.
#[tokio::test]
async fn engine_snapshots_never_duplicate_the_durable_transcript() {
    use aether_core::checkpoint::{Checkpoint, CheckpointDraft, CheckpointProvenance};
    use aether_core::id::CheckpointId;
    use aether_store::{Commit, Mutation};

    let h = harness(Reply::Resumable).await;
    let id = chat_agent(&h);
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    let recorded = |snapshot: &aether_core::snapshot::InvocationSnapshot, text: &str| {
        snapshot.messages.iter().any(|m| m.text().contains(text))
    };

    engine_turn(&h, id, "the early question").await;
    engine_turn(&h, id, "the second question").await;
    let uncheckpointed = latest_snapshot(&h, id);
    assert!(
        !recorded(&uncheckpointed, "the early question"),
        "history must not be duplicated even before a checkpoint: {:?}",
        uncheckpointed.messages
    );
    assert_eq!(uncheckpointed.messages.len(), 1);
    assert!(recorded(&uncheckpointed, "the second question"));
    let transcript = h.store.transcript(id, 0).expect("durable transcript");
    assert!(
        transcript
            .iter()
            .any(|entry| entry.message.text().contains("the early question"))
    );

    let folded_up_to = h
        .store
        .transcript(id, 0)
        .expect("transcript")
        .last()
        .expect("two turns were recorded")
        .seq;
    h.store
        .commit(Commit::new().mutate(Mutation::PutCheckpoint(Checkpoint {
            id: CheckpointId::new(),
            agent_id: id,
            version: 1,
            draft: CheckpointDraft {
                objective: "answer the questions".into(),
                state: "two questions are answered".into(),
                ..Default::default()
            },
            provenance: CheckpointProvenance {
                from_seq: 0,
                to_seq: folded_up_to,
                source_snapshot: None,
                compactor_model: None,
                previous: None,
                trigger: None,
            },
            created_at: aether_core::time::now(),
        })))
        .expect("seed checkpoint");

    engine_turn(&h, id, "the third question").await;
    let checkpointed = latest_snapshot(&h, id);
    assert!(
        !recorded(&checkpointed, "the early question")
            && !recorded(&checkpointed, "the second question"),
        "messages the checkpoint folded must not be stored again: {:?}",
        checkpointed.messages
    );
    assert_eq!(
        checkpointed.messages.len(),
        1,
        "only this turn's input follows the checkpoint: {:?}",
        checkpointed.messages
    );
    let sent = h
        .prompts
        .lock()
        .unwrap()
        .last()
        .expect("a turn ran")
        .clone();
    assert_eq!(checkpointed.messages[0].text(), sent);
    assert!(
        sent.contains("the third question") && sent.contains("two questions are answered"),
        "the engine still gets the new message and the unseen checkpoint: {sent}"
    );
    h.runtime.shutdown().await;
}
