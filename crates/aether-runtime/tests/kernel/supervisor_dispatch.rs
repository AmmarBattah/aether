//! supervisor dispatch scenarios and fixtures.
use super::*;

#[tokio::test]
async fn a_supervisor_cap_runs_its_workers_one_at_a_time() {
    use aether_core::model::{BlockStart, StopReason, StreamEvent};
    // A worker turn that streams slowly, then closes its task.
    let slow_then_complete = || {
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
        events.extend((0..25).map(|i| StreamEvent::TextDelta {
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
            partial_json: json!({"summary": "part done"}).to_string(),
        });
        events.push(StreamEvent::BlockStop { index: 1 });
        events.push(StreamEvent::Stop {
            reason: StopReason::ToolUse,
        });
        events.push(StreamEvent::MessageStop);
        Ok(events)
    };
    let h = harness_configured(
        vec![
            // parent turn 1: two workers at once, wait for both
            ScriptedClient::tool_calls(
                &[
                    (
                        "delegate",
                        json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "part one", "wait": true}),
                    ),
                    (
                        "delegate",
                        json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "part two", "wait": true}),
                    ),
                ],
                None,
            ),
            slow_then_complete(),
            slow_then_complete(),
            // parent turn 2: both results in, finish
            ScriptedClient::tool_call("complete_task", json!({"summary": "parent done"})),
        ],
        |_| {},
        |c| c.max_concurrent_turns = 4,
    )
    .await;
    h.client.set_delay(Duration::from_millis(60));
    let mut s = spec("split the work");
    s.config.as_mut().expect("config").concurrency = Some(1);
    let (parent, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    let status = h.wait_until(parent.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);

    let children = h
        .store
        .agents(&AgentFilter {
            parent: Some(parent.id),
            ..Default::default()
        })
        .expect("kids");
    assert_eq!(children.len(), 2);
    assert!(
        children
            .iter()
            .all(|c| c.status.kind() == AgentStatusKind::Completed),
        "{children:?}"
    );
    // Each child's turns as [started, completed] event-sequence intervals.
    let intervals = |id: AgentId| -> Vec<(u64, u64)> {
        let events = h
            .store
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
    };
    let a = intervals(children[0].id);
    let b = intervals(children[1].id);
    assert!(!a.is_empty() && !b.is_empty(), "{a:?} {b:?}");
    for (s1, e1) in &a {
        for (s2, e2) in &b {
            assert!(
                e1 < s2 || e2 < s1,
                "worker turns overlapped under a cap of 1: {a:?} vs {b:?}"
            );
        }
    }
    assert_eq!(h.client.remaining(), 0);
}

#[tokio::test]
async fn an_external_trigger_wakes_a_waiting_parent() {
    let h = harness(vec![
        // parent turn 1: delegate and wait
        ScriptedClient::tool_call(
            "delegate",
            json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "do the sub task", "wait": true}),
        ),
        // child turn 1: park on a question so it stays alive without finishing
        ScriptedClient::tool_call("ask_user", json!({"question": "which colour?"})),
        // parent turn 2: woken by the trigger, only replies
        ScriptedClient::text("CI is green; still waiting on the worker."),
        // child turn 2: finish once the user answered
        ScriptedClient::tool_call("complete_task", json!({"summary": "child finished"})),
        // parent turn 3: finish with the result
        ScriptedClient::tool_call("complete_task", json!({"summary": "parent done"})),
    ])
    .await;
    let (parent, _) = h
        .runtime
        .create_agent(spec("split the work"), Actor::user())
        .expect("create");
    let parked = h
        .wait_until(parent.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    let child = h
        .store
        .agents(&AgentFilter {
            parent: Some(parent.id),
            ..Default::default()
        })
        .expect("agents")
        .pop()
        .expect("child");
    h.wait_until(child.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;

    h.runtime
        .trigger_agent(
            parent.id,
            "ci".into(),
            json!({"pipeline": "build", "status": "green"}),
            Actor::System,
        )
        .expect("trigger");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let restored = loop {
        let agent = h.store.agent(parent.id).expect("agent").expect("exists");
        if agent.stats.turns >= 2 && matches!(agent.status, AgentStatus::Waiting { .. }) {
            break agent;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "parent never woke for the trigger: {:?}",
            agent.status
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(restored.status, parked, "the interrupted wait is restored");
    let reasons: Vec<String> = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(parent.id))
        .expect("events")
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::AgentResumed { reason, .. } => Some(reason),
            _ => None,
        })
        .collect();
    assert_eq!(reasons, vec!["external trigger from ci".to_string()]);
    assert!(
        h.client
            .requests()
            .iter()
            .any(|r| format!("{r:?}").contains("External trigger from ci")
                && format!("{r:?}").contains("green")),
        "the trigger reached the model"
    );

    h.runtime
        .send_message(child.id, vec![ContentBlock::text("blue")], Actor::user())
        .expect("send");
    let status = h.wait_until(parent.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
}

#[tokio::test]
async fn two_children_complete_in_any_order_without_lost_delivery() {
    let h = harness(vec![
        ScriptedClient::tool_calls(
            &[
                (
                    "delegate",
                    json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "a", "objective": "task A", "wait": true}),
                ),
                (
                    "delegate",
                    json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "b", "objective": "task B", "wait": true}),
                ),
            ],
            None,
        ),
        ScriptedClient::tool_call("complete_task", json!({"summary": "A done"})),
        ScriptedClient::tool_call("complete_task", json!({"summary": "B done"})),
        ScriptedClient::tool_call("complete_task", json!({"summary": "both collected"})),
    ])
    .await;
    let (parent, _) = h
        .runtime
        .create_agent(spec("fan out"), Actor::user())
        .expect("create");
    let status = h.wait_until(parent.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    let t = types(h.store.as_ref(), parent.id);
    assert_eq!(t.iter().filter(|t| *t == "agent.resumed").count(), 1);
    let inbox = h.store.inbox(parent.id, 0).expect("inbox");
    let completed: Vec<_> = inbox
        .iter()
        .filter(|i| matches!(i.kind, aether_core::inbox::InboxKind::TaskCompleted { .. }))
        .collect();
    assert_eq!(completed.len(), 2, "both completions delivered: {inbox:?}");
    let turns = h.store.invocations_for_agent(parent.id).expect("inv");
    assert_eq!(turns.len(), 2);
    let snap = h
        .store
        .snapshot(turns[1].snapshot_id)
        .expect("snap")
        .expect("exists");
    let text = snap.messages.last().map(|m| m.text()).unwrap_or_default();
    assert!(text.contains("A done") && text.contains("B done"), "{text}");
}
