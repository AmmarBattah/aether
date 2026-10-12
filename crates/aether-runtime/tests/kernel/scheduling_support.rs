//! scheduling support scenarios and fixtures.
use super::*;

// ----- VISION §3.6: the six scheduling limits, in order --------------------

/// A turn that streams for a while, then closes its task. Long enough that
/// two overlapping turns are visible as overlapping event intervals; the
/// delta count sets how long it holds its slot (the client delays per event).
pub(super) fn slow_turn(summary: &str) -> aether_model::testing::ScriptedResponse {
    turn_of_length(25, summary)
}

pub(super) fn turn_of_length(
    deltas: usize,
    summary: &str,
) -> aether_model::testing::ScriptedResponse {
    use aether_core::model::{BlockStart, StopReason, StreamEvent};
    let mut events = vec![
        StreamEvent::MessageStart {
            provider_message_id: Some("scripted".into()),
            model: Some("scripted".into()),
        },
        StreamEvent::BlockStart {
            index: 0,
            block: BlockStart::Text {
                text: String::new(),
            },
        },
    ];
    events.extend((0..deltas).map(|i| StreamEvent::TextDelta {
        index: 0,
        text: format!("w{i} "),
    }));
    events.push(StreamEvent::BlockStop { index: 0 });
    events.push(StreamEvent::BlockStart {
        index: 1,
        block: BlockStart::ToolUse {
            id: "call_done".into(),
            name: "complete_task".into(),
            input: None,
        },
    });
    events.push(StreamEvent::ToolInputDelta {
        index: 1,
        partial_json: json!({ "summary": summary }).to_string(),
    });
    events.push(StreamEvent::BlockStop { index: 1 });
    events.push(StreamEvent::Stop {
        reason: StopReason::ToolUse,
    });
    events.push(StreamEvent::MessageStop);
    Ok(events)
}

/// Each agent's turns as [started, completed] event-sequence intervals.
pub(super) fn turn_intervals(store: &dyn Store, id: AgentId) -> Vec<(u64, u64)> {
    let events = store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events");
    let mut out = Vec::new();
    let mut open = None;
    for e in events {
        match e.type_name().as_str() {
            "agent.started" => open = Some(e.seq),
            "agent.turn_completed" => {
                if let Some(start) = open.take() {
                    out.push((start, e.seq));
                }
            }
            _ => {}
        }
    }
    out
}

pub(super) fn assert_never_overlapped(a: &[(u64, u64)], b: &[(u64, u64)], what: &str) {
    assert!(!a.is_empty() && !b.is_empty(), "{what}: {a:?} {b:?}");
    for (s1, e1) in a {
        for (s2, e2) in b {
            assert!(e1 < s2 || e2 < s1, "{what}: {a:?} vs {b:?}");
        }
    }
}

pub(super) fn spec_on(objective: &str, provider: &str, model: &str) -> CreateAgentSpec {
    CreateAgentSpec {
        model: Some(ModelRef::new(provider, model)),
        ..spec(objective)
    }
}

/// The event-sequence window of an agent's first shell call.
pub(super) fn shell_window(store: &dyn Store, id: AgentId) -> (u64, u64) {
    let events = store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events");
    let start = events
        .iter()
        .find(|e| e.type_name() == "tool.started")
        .map(|e| e.seq)
        .expect("tool.started");
    let end = events
        .iter()
        .find(|e| e.type_name() == "tool.succeeded" && e.seq > start)
        .map(|e| e.seq)
        .expect("tool.succeeded");
    (start, end)
}

/// The first thing a tool call told the agent.
pub(super) async fn wait_for_tool_answer(h: &Harness, id: AgentId) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let told = h
            .store
            .events(0, 10_000, &EventFilter::for_agent(id))
            .expect("events")
            .into_iter()
            .find_map(|e| match e.kind {
                EventKind::ToolSucceeded { outcome, .. } => Some(outcome.content),
                EventKind::ToolFailed { error, .. } => Some(error),
                _ => None,
            });
        if let Some(told) = told {
            return told;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("no tool answer: {}", timeout_details(h.store.as_ref(), id));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// The seq of an agent's first event of this type.
pub(super) fn first_event_seq(store: &dyn Store, id: AgentId, type_name: &str) -> u64 {
    store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events")
        .into_iter()
        .find(|e| e.type_name() == type_name)
        .unwrap_or_else(|| panic!("no {type_name} for {id}"))
        .seq
}
