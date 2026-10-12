//! task lifecycle scenarios and fixtures.
use super::*;

#[tokio::test]
async fn status_since_creation_and_operator_updates_preserve_continuous_state() {
    let h = harness(vec![]).await;
    let (created, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                start: false,
                ..spec("stay idle")
            },
            Actor::user(),
        )
        .unwrap();
    assert_eq!(created.status.kind(), AgentStatusKind::Created);
    assert_eq!(created.status_since, Some(created.created_at));
    h.runtime
        .pause_agent(created.id, false, Actor::user())
        .unwrap();
    let updated = h
        .runtime
        .update_agent(
            created.id,
            AgentUpdate {
                name: Some("renamed".into()),
                token_budget: Some(50_000),
                ..Default::default()
            },
            Actor::user(),
        )
        .unwrap();
    assert_eq!(updated.status_since, created.status_since);
    assert!(updated.paused);
    assert_eq!(updated.name, "renamed");
    assert_eq!(
        h.store.agent(created.id).unwrap().unwrap().status_since,
        created.status_since
    );
}

#[tokio::test]
async fn agent_completes_its_task_through_the_runtime() {
    let h = harness(vec![ScriptedClient::tool_call(
        "complete_task",
        json!({"summary": "wrote the file"}),
    )])
    .await;
    let (agent, task) = h
        .runtime
        .create_agent(spec("write a file"), Actor::user())
        .expect("create");
    let task = task.expect("task");
    let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert_eq!(
        status,
        AgentStatus::Completed {
            summary: Some("wrote the file".into())
        }
    );
    let task = h.store.task(task.id).expect("task").expect("exists");
    assert_eq!(task.status.kind(), TaskStatusKind::Completed);
    assert_eq!(
        task.result().map(|r| r.summary.as_str()),
        Some("wrote the file")
    );

    let t = types(h.store.as_ref(), agent.id);
    for expected in [
        "agent.created",
        "agent.ready",
        "agent.started",
        "lease.granted",
        "task.started",
        "snapshot.created",
        "invocation.completed",
        "agent.message",
        "tool.requested",
        "tool.started",
        "tool.succeeded",
        "task.completed",
        "agent.completed",
        "lease.released",
    ] {
        assert!(
            t.contains(&expected.to_string()),
            "missing {expected} in {t:?}"
        );
    }
    assert_eq!(t.iter().filter(|t| *t == "invocation.completed").count(), 1);
    assert_eq!(h.client.remaining(), 0);
    let invocations = h.store.invocations_for_agent(agent.id).expect("inv");
    assert_eq!(invocations.len(), 1);
    let snapshot = h
        .store
        .snapshot(invocations[0].snapshot_id)
        .expect("snap")
        .expect("exists");
    assert!(snapshot.system_text().contains("write a file"));
    assert!(snapshot.tools.iter().any(|t| t.name == "complete_task"));
    assert!(h.store.lease(agent.id).expect("lease").is_none());
    // A root with a task must not be told it has nothing to complete.
    assert!(
        !snapshot
            .system_text()
            .contains("Nothing to `complete_task`"),
        "root owns a task, so the disclaimer must not be in its prompt: {}",
        snapshot.system_text()
    );
}

/// A real tool call that fails after measurable elapsed time must
/// carry that elapsed time on `EventKind::ToolFailed`, not a zeroed
/// placeholder. A short shell timeout against a longer sleep forces a real
/// `ToolError::Timeout` with a non-trivial duration.
#[tokio::test]
async fn a_timed_out_tool_call_reports_its_real_elapsed_duration() {
    let h = harness_configured(
        vec![ScriptedClient::tool_call(
            "bash",
            json!({"command": "sleep 5"}),
        )],
        |_| {},
        |c| {
            c.execution = aether_tools::ExecutionPolicy {
                default_timeout: Duration::from_millis(150),
                shell_timeout: Duration::from_millis(150),
            }
        },
    )
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("run a slow command"), Actor::user())
        .expect("create");
    let events = wait_until_events(h.store.as_ref(), agent.id, |events| {
        events.iter().any(|e| e.type_name() == "tool.failed")
    })
    .await;
    let (error, retryable, duration_ms) = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolFailed {
                error,
                retryable,
                duration_ms,
                ..
            } => Some((error.clone(), *retryable, *duration_ms)),
            _ => None,
        })
        .expect("bash call timed out");
    assert!(error.contains("timed out"), "{error}");
    assert!(retryable, "a timeout is retryable");
    let measured = duration_ms.expect("a call that actually ran must report a real duration");
    assert!(
        measured > 0,
        "a call that ran for the full 150ms timeout must not report 0ms: {measured}"
    );
}

/// A root (or any agent between tasks) calling `complete_task` has nothing
/// to complete - this must read as information, not a failure, and name
/// the alternative.
#[tokio::test]
async fn complete_task_on_a_task_less_agent_is_informational_not_an_error() {
    let h = harness(vec![
        ScriptedClient::tool_call("complete_task", json!({"summary": "nothing to close"})),
        ScriptedClient::text("noted, waiting for instructions"),
    ])
    .await;
    let (agent, task) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                config: Some(AgentConfig {
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    assert!(task.is_none(), "no objective means no task was assigned");
    // Idle with no task and no inbox item goes straight to waiting; a
    // message is what gives it a turn to act on.
    h.runtime
        .send_message(agent.id, vec![ContentBlock::text("go")], Actor::user())
        .expect("send");
    let events = wait_until_events(h.store.as_ref(), agent.id, |events| {
        events.iter().any(|e| e.type_name() == "tool.succeeded")
    })
    .await;
    let call_id = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolRequested { call, .. } if call.name == "complete_task" => Some(call.id),
            _ => None,
        })
        .expect("complete_task was requested");
    let outcome = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolSucceeded { call, outcome } if *call == call_id => Some(outcome.clone()),
            _ => None,
        })
        .expect("complete_task settled");
    assert!(
        !outcome.is_error,
        "a task-less agent's complete_task is informational, not an error: {outcome:?}"
    );
    assert!(!outcome.content.contains("coordinator"), "{outcome:?}");
    assert!(
        outcome.content.contains("ask_user") && outcome.content.contains("send_message"),
        "names the alternative: {outcome:?}"
    );
    assert_eq!(
        outcome.structured,
        Some(json!({"task_less": true})),
        "the TUI's system line reads this marker off the runtime's outcome: {outcome:?}"
    );
    assert!(
        !events.iter().any(|e| e.type_name() == "task.completed"),
        "there is no task to complete"
    );
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
}

/// Same as above for `fail_task`.
#[tokio::test]
async fn fail_task_on_a_task_less_agent_is_informational_not_an_error() {
    let h = harness(vec![
        ScriptedClient::tool_call("fail_task", json!({"error": "nothing to fail"})),
        ScriptedClient::text("noted, waiting for instructions"),
    ])
    .await;
    let (agent, task) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                config: Some(AgentConfig {
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    assert!(task.is_none(), "no objective means no task was assigned");
    // Idle with no task and no inbox item goes straight to waiting; a
    // message is what gives it a turn to act on.
    h.runtime
        .send_message(agent.id, vec![ContentBlock::text("go")], Actor::user())
        .expect("send");
    let events = wait_until_events(h.store.as_ref(), agent.id, |events| {
        events.iter().any(|e| e.type_name() == "tool.succeeded")
    })
    .await;
    let call_id = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolRequested { call, .. } if call.name == "fail_task" => Some(call.id),
            _ => None,
        })
        .expect("fail_task was requested");
    let outcome = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolSucceeded { call, outcome } if *call == call_id => Some(outcome.clone()),
            _ => None,
        })
        .expect("fail_task settled");
    assert!(
        !outcome.is_error,
        "a task-less agent's fail_task is informational, not an error: {outcome:?}"
    );
    assert!(!outcome.content.contains("coordinator"), "{outcome:?}");
    assert!(
        outcome.content.contains("ask_user") && outcome.content.contains("send_message"),
        "names the alternative: {outcome:?}"
    );
    assert_eq!(
        outcome.structured,
        Some(json!({"task_less": true})),
        "the TUI's system line reads this marker off the runtime's outcome: {outcome:?}"
    );
    assert!(
        !events.iter().any(|e| e.type_name() == "task.failed"),
        "there is no task to fail"
    );
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
}

/// Completing a task a second time is a genuinely wrong call, not the
/// task-less case above - it must stay a real, named error.
#[tokio::test]
async fn completing_an_already_terminal_task_is_a_real_error() {
    let h = harness(vec![ScriptedClient::tool_calls(
        &[
            ("complete_task", json!({"summary": "first pass"})),
            ("complete_task", json!({"summary": "trying again"})),
        ],
        None,
    )])
    .await;
    let (agent, task) = h
        .runtime
        .create_agent(spec("finish exactly once"), Actor::user())
        .expect("create");
    let task = task.expect("task");
    h.wait_until(agent.id, |s| s.is_terminal()).await;
    let events = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(agent.id))
        .expect("events");
    let call_ids: Vec<_> = events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::ToolRequested { call, .. } if call.name == "complete_task" => Some(call.id),
            _ => None,
        })
        .collect();
    assert_eq!(
        call_ids.len(),
        2,
        "both complete_task calls were requested: {call_ids:?}"
    );
    let outcomes: Vec<_> = call_ids
        .iter()
        .map(|id| {
            events
                .iter()
                .find_map(|e| match &e.kind {
                    EventKind::ToolSucceeded { call, outcome } if call == id => {
                        Some(outcome.clone())
                    }
                    _ => None,
                })
                .expect("settled")
        })
        .collect();
    assert!(
        !outcomes[0].is_error,
        "the first, real completion succeeds: {:?}",
        outcomes[0]
    );
    assert!(
        outcomes[1].is_error,
        "completing an already-terminal task is a real error: {:?}",
        outcomes[1]
    );
    assert!(
        outcomes[1].content.contains(&task.id.to_string())
            && outcomes[1].content.contains("already"),
        "names the task and its state: {:?}",
        outcomes[1]
    );
}
