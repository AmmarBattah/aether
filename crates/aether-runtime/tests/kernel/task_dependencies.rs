//! task dependencies scenarios and fixtures.
use super::*;

/// VISION §3.2: a task with unmet dependencies is never dispatched, and the
/// dependency finishing wakes its dependant.
#[tokio::test]
async fn a_task_waits_for_its_dependency_and_wakes_when_it_finishes() {
    let h = harness(vec![
        // Slow enough that the gated agent is observably held while it runs.
        turn_of_length(60, "first done"),
        ScriptedClient::tool_call("complete_task", json!({"summary": "second done"})),
    ])
    .await;
    h.client.set_delay(Duration::from_millis(20));
    let (first, first_task) = h
        .runtime
        .create_agent(spec("do the first part"), Actor::user())
        .expect("first");
    let first_task = first_task.expect("task");
    // A second agent with no work of its own, then a task gated on the first.
    let (second, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                objective: None,
                ..spec("unused")
            },
            Actor::user(),
        )
        .expect("second");
    let gated = h
        .runtime
        .create_task(
            aether_runtime::CreateTaskSpec {
                objective: "do the second part".into(),
                owner: Some(second.id),
                dependencies: vec![first_task.id],
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("gated task");
    assert_eq!(
        gated.status.kind(),
        TaskStatusKind::Queued,
        "a gated task is not assigned"
    );

    // It is held, visibly, and never dispatched while the dependency runs.
    let blocked = h
        .wait_until(second.id, |s| matches!(s, AgentStatus::Blocked { .. }))
        .await;
    let AgentStatus::Blocked { reason } = &blocked else {
        panic!("{blocked:?}");
    };
    assert_eq!(
        *reason,
        BlockReason::Dependency { on: first_task.id },
        "the gate names why it holds the agent"
    );
    assert_eq!(
        h.store.agent(second.id).expect("a").expect("e").stats.turns,
        0,
        "the gated agent took a turn before its dependency finished"
    );

    h.wait_until(first.id, |s| s.is_terminal()).await;
    let status = h.wait_until(second.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    let t = types(h.store.as_ref(), second.id);
    assert!(t.contains(&"agent.blocked".to_owned()), "{t:?}");
    assert!(t.contains(&"task.unblocked".to_owned()), "{t:?}");
    assert!(t.contains(&"task.assigned".to_owned()), "{t:?}");
    // The wake says how the dependency ended, not just that it did: the owner
    // is dispatched onto dependent work either way.
    let woke = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(second.id))
        .expect("events")
        .into_iter()
        .find_map(|e| match e.kind {
            EventKind::AgentResumed { reason, .. } => Some(reason),
            _ => None,
        })
        .expect("agent.resumed");
    assert!(
        woke.contains(&format!("{} completed", first_task.id)),
        "the wake does not say how the dependency ended: {woke}"
    );
    assert_eq!(
        h.store
            .task(gated.id)
            .expect("task")
            .expect("exists")
            .status
            .kind(),
        TaskStatusKind::Completed
    );
    assert_eq!(h.client.remaining(), 0);
}

/// The task an agent created with `spec` carries, once that agent is cancelled.
fn cancelled_task(h: &Harness, objective: &str) -> aether_core::task::Task {
    let mut s = spec(objective);
    s.start = false;
    let (agent, task) = h.runtime.create_agent(s, Actor::user()).expect("agent");
    h.runtime
        .cancel_agent(agent.id, Some("not needed".into()), Actor::user())
        .expect("cancel");
    let task = h
        .store
        .task(task.expect("task").id)
        .expect("task")
        .expect("exists");
    assert_eq!(task.status.kind(), TaskStatusKind::Cancelled);
    task
}

/// A dependency that already ended without a result can never be waited for:
/// creating a task on it is refused, not gated on nothing.
#[tokio::test]
async fn a_task_cannot_depend_on_one_already_cancelled() {
    let h = harness(vec![]).await;
    let dependency = cancelled_task(&h, "abandoned work");
    let error = h
        .runtime
        .create_task(
            aether_runtime::CreateTaskSpec {
                objective: "build on it".into(),
                dependencies: vec![dependency.id],
                ..Default::default()
            },
            Actor::user(),
        )
        .expect_err("a cancelled dependency is refused");
    assert!(
        error
            .to_string()
            .contains(&format!("dependency {} is cancelled", dependency.id)),
        "{error}"
    );
    assert!(
        h.store
            .tasks(&TaskFilter::default())
            .expect("tasks")
            .iter()
            .all(|t| t.objective != "build on it"),
        "nothing was created"
    );
}

/// VISION §3.2 gates on a dependency being terminal, not on it succeeding: one
/// cancelled after the gate closed releases the task to its owner, told how
/// it ended, instead of leaving it queued forever.
#[tokio::test]
async fn a_dependency_cancelled_later_releases_the_task_to_its_owner() {
    let h = harness(vec![ScriptedClient::tool_call(
        "complete_task",
        json!({"summary": "handled the cancelled input"}),
    )])
    .await;
    let mut s = spec("the first part");
    s.start = false;
    let (first, first_task) = h.runtime.create_agent(s, Actor::user()).expect("first");
    let first_task = first_task.expect("task");
    let (second, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                objective: None,
                ..spec("unused")
            },
            Actor::user(),
        )
        .expect("second");
    let gated = h
        .runtime
        .create_task(
            aether_runtime::CreateTaskSpec {
                objective: "the second part".into(),
                owner: Some(second.id),
                dependencies: vec![first_task.id],
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("gated task");
    assert_eq!(gated.status.kind(), TaskStatusKind::Queued);
    h.wait_until(second.id, |s| matches!(s, AgentStatus::Blocked { .. }))
        .await;

    h.runtime
        .cancel_agent(first.id, Some("not needed".into()), Actor::user())
        .expect("cancel the dependency's owner");
    let status = h.wait_until(second.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    let woke = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(second.id))
        .expect("events")
        .into_iter()
        .find_map(|e| match e.kind {
            EventKind::AgentResumed { reason, .. } => Some(reason),
            _ => None,
        })
        .expect("agent.resumed");
    assert!(
        woke.contains(&format!("{} cancelled", first_task.id)),
        "the wake does not say the dependency was cancelled: {woke}"
    );
    assert_eq!(
        h.store
            .task(gated.id)
            .expect("task")
            .expect("exists")
            .status
            .kind(),
        TaskStatusKind::Completed
    );
}
