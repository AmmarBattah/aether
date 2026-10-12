//! Provider quota windows and tool-duration events.
use super::*;
#[tokio::test]
async fn provider_usage_keeps_every_reported_window_in_health_and_events() {
    let h = harness(Reply::Quota).await;
    let id = chat_agent(&h);
    h.runtime
        .send_message(id, vec![ContentBlock::text("Report usage")], Actor::user())
        .unwrap();
    wait_for(h.store.as_ref(), id, |agent| {
        agent.stats.turns > 0 && agent.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    let health =
        serde_json::to_value(h.runtime.provider_health_for("claude-code").unwrap()).unwrap();
    assert_eq!(
        health["quota"]["used_percent"], 42,
        "the scheduling window stays unchanged"
    );
    assert_eq!(
        health["usage"]["windows"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0),
        2,
        "both subscription windows must be available to /usage"
    );
    assert!(health["usage"]["observed_at"].is_string());
    let events = h.store.events(0, 1000, &EventFilter::default()).unwrap();
    let event = events
        .iter()
        .find(|event| event.type_name() == "provider.quota")
        .unwrap();
    let event = serde_json::to_value(event).unwrap();
    assert_eq!(
        event["kind"]["usage"], health["usage"],
        "health and durable events retain the same report"
    );
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn native_tool_duration_measures_the_observed_execution_interval() {
    let gate = Arc::new(tokio::sync::Notify::new());
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let systems = Arc::new(Mutex::new(Vec::new()));
    let engine = Arc::new(ScriptedEngine {
        reply: Reply::ToolRun,
        prompts: prompts.clone(),
        systems: systems.clone(),
        costs: Arc::new(Mutex::new(Default::default())),
        tool_gate: Some(gate.clone()),
    });
    let h = harness_with_engine(engine, prompts, systems).await;
    let id = chat_agent(&h);
    h.runtime
        .send_message(id, vec![ContentBlock::text("Inspect files")], Actor::user())
        .unwrap();
    wait_for(h.store.as_ref(), id, |_| {
        h.store
            .events(0, 1000, &EventFilter::default())
            .unwrap()
            .iter()
            .any(|event| matches!(event.kind, EventKind::ToolStarted { .. }))
    })
    .await;
    // Release only after the runtime recorded the start, so queueing the
    // scripted events cannot erase the interval being measured.
    tokio::time::sleep(Duration::from_millis(30)).await;
    gate.notify_one();
    wait_for(h.store.as_ref(), id, |agent| {
        agent.stats.turns > 0 && agent.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    let events = h.store.events(0, 1000, &EventFilter::default()).unwrap();
    let duration = events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::ToolSucceeded { outcome, .. } => Some(outcome.duration_ms),
            _ => None,
        })
        .expect("native result");
    assert!(
        duration >= 20,
        "native execution was recorded as {duration}ms"
    );
    h.runtime.shutdown().await;
}
