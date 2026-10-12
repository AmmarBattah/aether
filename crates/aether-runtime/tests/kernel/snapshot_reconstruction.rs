//! snapshot reconstruction scenarios and fixtures.
use super::*;

// ----- VISION §6.2: snapshots reconstruct ---------------------------------

/// Every snapshot a run recorded, checked against the properties the
/// inspector depends on: it survives storage byte for byte, the event that
/// announced it never drifts from it, the system prompt is exactly its
/// layers, and the request it builds is a pure function of it.
fn assert_snapshots_reconstruct(store: &dyn Store, id: AgentId) {
    let announced: Vec<(
        aether_core::id::SnapshotId,
        u32,
        u32,
        Vec<aether_core::snapshot::ContextLayer>,
    )> = store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events")
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::SnapshotCreated {
                snapshot,
                turn,
                token_estimate,
                layers,
                ..
            } => Some((snapshot, turn, token_estimate, layers)),
            _ => None,
        })
        .collect();
    assert!(!announced.is_empty(), "the run recorded no snapshot");
    for (sid, turn, token_estimate, layers) in announced {
        let stored = store
            .snapshot(sid)
            .expect("snapshot")
            .expect("every announced snapshot is stored");
        let json = serde_json::to_string(&stored).expect("serialize");
        let back: aether_core::snapshot::InvocationSnapshot =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            serde_json::to_string(&back).expect("reserialize"),
            json,
            "snapshot {sid} does not survive storage byte for byte"
        );
        assert_eq!(stored.turn, turn);
        assert_eq!(stored.token_estimate, token_estimate);
        assert_eq!(
            stored.layers, layers,
            "the event announcing {sid} drifted from the snapshot it announced"
        );
        let rendered: Vec<&str> = stored
            .layers
            .iter()
            .filter_map(|l| l.text.as_deref())
            .collect();
        assert_eq!(
            stored.system_text(),
            rendered.join("\n\n"),
            "the system prompt is the layers and nothing else"
        );
        let once = aether_context::build_request(&stored);
        assert_eq!(
            once,
            aether_context::build_request(&stored),
            "building the request twice from one snapshot gives two requests"
        );
        assert_eq!(once.messages, stored.messages);
        assert_eq!(once.tools, stored.tools);
    }
}

#[tokio::test]
async fn every_snapshot_of_a_recorded_run_reconstructs() {
    let h = harness(vec![
        ScriptedClient::tool_call("write_file", json!({"path": "n.txt", "content": "hi"})),
        ScriptedClient::text("Written."),
        ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
    ])
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("write a file and finish"), Actor::user())
        .expect("create");
    h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert_snapshots_reconstruct(h.store.as_ref(), agent.id);
}
