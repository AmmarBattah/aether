use super::*;

async fn reported_wait(h: &mut Harness) -> (AgentId, TurnStarted) {
    let parent = start_coordinator(h);
    let turn = turn_of(h, parent).await;
    turn.events
        .upgrade()
        .unwrap()
        .send(EngineEvent::Assistant {
            content: vec![ContentBlock::ToolUse {
                id: "wait-reply".into(),
                name: "mcp__aether__wait_for".into(),
                input: json!({}),
            }],
            usage: None,
            model: None,
        })
        .await
        .unwrap();
    wait_for_event(h.store.as_ref(), |event| {
        matches!(&event.kind, EventKind::ToolRequested { call, .. }
            if call.provider_call_id == "wait-reply")
    })
    .await;
    let (reply, error) = raw_call(h, &turn, "wait_for", json!({})).await;
    assert!(!error, "{reply}");
    (parent, turn)
}

#[tokio::test]
async fn parking_waits_for_the_native_tool_reply_before_interrupting() {
    let mut h = harness(Duration::from_secs(5)).await;
    let (parent, turn) = reported_wait(&mut h).await;
    turn.events
        .upgrade()
        .unwrap()
        .send(EngineEvent::ToolResult {
            tool_use_id: "unrelated".into(),
            content: "ok".into(),
            is_error: false,
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!turn.observed.stopped());
    assert!(!turn.observed.stdin_closed());
    turn.events
        .upgrade()
        .unwrap()
        .send(EngineEvent::ToolResult {
            tool_use_id: "wait-reply".into(),
            content: "Waiting for your next message.".into(),
            is_error: false,
        })
        .await
        .unwrap();
    wait_until_asked_to_stop(&turn.observed, Duration::from_secs(2)).await;
    assert!(events(h.store.as_ref()).iter().any(|event| {
        matches!(&event.kind, EventKind::ToolSucceeded { outcome, .. }
            if !outcome.is_error && outcome.content == "Waiting for your next message.")
    }));
    let _ = turn.end.send(());
    wait_for_park(&h, parent).await;
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn a_missing_native_reply_cannot_prevent_the_bounded_park_stop() {
    let mut h = harness(Duration::from_millis(100)).await;
    let (parent, turn) = reported_wait(&mut h).await;
    wait_until_stopped(&turn.observed, Duration::from_secs(2)).await;
    drop(turn.events);
    wait_for_park(&h, parent).await;
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn bridge_requests_can_arrive_before_their_native_stream_events() {
    let mut h = harness(Duration::from_secs(5)).await;
    let parent = start_coordinator(&h);
    let turn = turn_of(&mut h, parent).await;
    let (reply, error) = raw_call(&h, &turn, "wait_for", json!({})).await;
    assert!(!error, "{reply}");
    assert!(!turn.observed.stopped());
    let stream = turn.events.upgrade().unwrap();
    stream
        .send(EngineEvent::Assistant {
            content: vec![ContentBlock::ToolUse {
                id: "late-request".into(),
                name: "mcp__aether__wait_for".into(),
                input: json!({}),
            }],
            usage: None,
            model: None,
        })
        .await
        .unwrap();
    stream
        .send(EngineEvent::ToolResult {
            tool_use_id: "late-request".into(),
            content: reply.clone(),
            is_error: false,
        })
        .await
        .unwrap();
    wait_until_asked_to_stop(&turn.observed, Duration::from_secs(2)).await;
    drop(stream);
    let _ = turn.end.send(());
    wait_for_park(&h, parent).await;
    h.runtime.shutdown().await;
}
