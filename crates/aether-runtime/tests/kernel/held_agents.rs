//! held agents scenarios and fixtures.
use super::*;

/// The dependency gate clears the block it created and nothing else: an agent
/// parked for inspection is not put back to work by a task finishing.
#[tokio::test]
async fn a_dependency_finishing_leaves_an_inspection_alone() {
    let under_inspection = AgentId::new();
    let h = harness_with(vec![slow_turn("dependency done")], |store| {
        seed_interrupted(store, under_inspection, AgentStatus::Running, Some("bash"));
    })
    .await;
    let blocked = h.store.agent(under_inspection).expect("a").expect("exists");
    let AgentStatus::Blocked {
        reason: BlockReason::Inspection { .. },
    } = &blocked.status
    else {
        panic!("recovery must park a mutating call for a person: {blocked:?}");
    };

    let (_, dependency) = h
        .runtime
        .create_agent(spec("the thing it waits for"), Actor::user())
        .expect("owner");
    let dependency = dependency.expect("task");
    let gated = h
        .runtime
        .create_task(
            aether_runtime::CreateTaskSpec {
                objective: "run after it".into(),
                owner: Some(under_inspection),
                dependencies: vec![dependency.id],
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("gated task");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while !h
        .store
        .task(dependency.id)
        .expect("task")
        .expect("exists")
        .status
        .is_terminal()
    {
        assert!(tokio::time::Instant::now() < deadline, "dependency stalled");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // The gate has run at least once more since; give it room to get it wrong.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = h.store.agent(under_inspection).expect("a").expect("exists");
    assert_eq!(
        after.status, blocked.status,
        "an inspection is only cleared by a person"
    );
    assert_eq!(after.stats.turns, 0, "it must not have taken a turn");
    assert_eq!(
        h.store
            .task(gated.id)
            .expect("task")
            .expect("exists")
            .status
            .kind(),
        TaskStatusKind::Assigned,
        "the task is assigned; the agent still is not running it"
    );
}

/// VISION §3.1/§3.8: pausing an agent the gate is holding is a real outcome,
/// not a silent success, and it survives the dependency finishing.
#[tokio::test]
async fn pausing_a_gated_agent_holds_it_past_the_gate() {
    let h = harness(vec![
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
    h.runtime
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
    h.wait_until(second.id, |s| {
        matches!(
            s,
            AgentStatus::Blocked {
                reason: BlockReason::Dependency { .. }
            }
        )
    })
    .await;
    h.runtime
        .pause_agent(second.id, false, Actor::user())
        .expect("pausing a gated agent is accepted and real");
    assert!(
        h.store.agent(second.id).expect("a").expect("exists").paused,
        "the pause must be recorded"
    );

    h.wait_until(first.id, |s| s.is_terminal()).await;
    // The gate has cleared; only the pause is holding it now.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        let a = h.store.agent(second.id).expect("a").expect("exists");
        assert_eq!(a.stats.turns, 0, "a paused agent takes no turns");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(h.client.remaining(), 1, "its turn was never taken");

    h.runtime
        .resume_agent(second.id, false, Actor::user())
        .expect("resume");
    let status = h.wait_until(second.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    let t = types(h.store.as_ref(), second.id);
    assert!(t.contains(&"agent.paused".to_owned()), "{t:?}");
}

/// VISION §3.8: interrupting an agent that is only queued behind another
/// agent's file lock leaves it waiting for its next message, with the tool
/// calls it never started settled.
#[tokio::test]
async fn interrupting_a_lease_blocked_turn_leaves_the_agent_waiting() {
    let sleeping = || ScriptedClient::tool_call("bash", json!({"command": "sleep 0.6"}));
    let h = harness_configured(
        vec![
            sleeping(),
            sleeping(),
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
        ],
        |_| {},
        |c| c.max_concurrent_turns = 2,
    )
    .await;
    let (a, _) = h
        .runtime
        .create_agent(spec("touch the repo"), Actor::user())
        .expect("a");
    let (b, _) = h
        .runtime
        .create_agent(spec("touch it too"), Actor::user())
        .expect("b");

    let waiter = wait_for_with_budget(
        h.wait_ceiling,
        WAIT_UNTIL_STALL_WINDOW,
        || {
            [a.id, b.id].into_iter().find(|id| {
                matches!(
                    h.store.agent(*id).expect("a").expect("exists").status,
                    AgentStatus::Blocked {
                        reason: BlockReason::Resource { .. }
                    }
                )
            })
        },
        || {
            (
                activity_signature(&h.store.agent(a.id).expect("a").expect("exists")),
                activity_signature(&h.store.agent(b.id).expect("b").expect("exists")),
            )
        },
        || "no one blocked on the resource".to_owned(),
    )
    .await;
    assert!(
        h.runtime
            .interrupt_agent(waiter, Actor::user())
            .expect("interrupt"),
        "a lease-blocked turn is interruptible"
    );
    let status = h
        .wait_until(waiter, |s| {
            !matches!(
                s,
                AgentStatus::Blocked {
                    reason: BlockReason::Resource { .. }
                }
            )
        })
        .await;
    assert_eq!(
        status,
        AgentStatus::Waiting {
            condition: aether_core::agent::WaitCondition::Inbox
        },
        "an interrupted agent idles instead of dying"
    );
    let t = types(h.store.as_ref(), waiter);
    assert!(t.contains(&"agent.interrupted".to_owned()), "{t:?}");
    assert!(!t.contains(&"agent.failed".to_owned()), "{t:?}");
    assert!(
        t.contains(&"tool.cancelled".to_owned()),
        "the abandoned calls must be settled: {t:?}"
    );
    let other = if waiter == a.id { b.id } else { a.id };
    h.wait_until(other, |s| s.is_terminal()).await;
    assert!(
        h.runtime.resource_holdings().is_empty(),
        "keys outlived their turns: {:?}",
        h.runtime.resource_holdings()
    );

    // The abandoned call left a result behind, so the next turn is a message
    // the provider will accept rather than a tool_use with nothing answering it.
    h.runtime
        .send_message(waiter, vec![ContentBlock::text("carry on")], Actor::user())
        .expect("send");
    wait_for_with_budget(
        h.wait_ceiling,
        WAIT_UNTIL_STALL_WINDOW,
        || (h.client.remaining() == 0).then_some(()),
        || h.client.remaining(),
        || "the interrupted agent never resumed its turn".to_owned(),
    )
    .await;
    let last = h.client.requests().pop().expect("a request");
    let answered: Vec<&str> = last
        .messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        answered.contains(&"call_0"),
        "the abandoned tool call has no result: {last:#?}"
    );
}

/// VISION §3.9: the lease table dies with the daemon, so a turn that was
/// waiting for a key is re-classified like any other interrupted turn rather
/// than left blocked on a holder that no longer exists.
#[tokio::test]
async fn restart_recovers_a_turn_that_was_waiting_for_a_key() {
    let waiting_on_key = AgentId::new();
    let h = harness_with(vec![], |store| {
        seed_interrupted(
            store,
            waiting_on_key,
            AgentStatus::Blocked {
                reason: BlockReason::Resource {
                    holder: AgentId::new(),
                    key: aether_core::tool::ResourceKey::repo("/r"),
                },
            },
            None,
        );
    })
    .await;
    let agent = h.store.agent(waiting_on_key).expect("a").expect("exists");
    assert!(
        matches!(
            agent.status.kind(),
            AgentStatusKind::Ready | AgentStatusKind::Waiting
        ),
        "a lease wait cannot survive the table that backed it: {:?}",
        agent.status
    );
    let t = types(h.store.as_ref(), waiting_on_key);
    assert!(t.contains(&"recovery.classified".to_owned()), "{t:?}");
    assert!(h.store.lease(waiting_on_key).expect("lease").is_none());
}

/// VISION §3.6 limit 1: a person's message reaches an agent the dependency
/// gate is holding, ahead of the gate.
#[tokio::test]
async fn a_person_reaches_an_agent_the_gate_is_holding() {
    let h = harness(vec![
        turn_of_length(200, "first done"),
        ScriptedClient::text("I hear you."),
        ScriptedClient::tool_call("complete_task", json!({"summary": "second done"})),
    ])
    .await;
    h.client.set_delay(Duration::from_millis(20));
    let (_first, first_task) = h
        .runtime
        .create_agent(spec("do the first part"), Actor::user())
        .expect("first");
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
                objective: "do the second part".into(),
                owner: Some(second.id),
                dependencies: vec![first_task.id],
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("gated task");
    h.wait_until(second.id, |s| {
        matches!(
            s,
            AgentStatus::Blocked {
                reason: BlockReason::Dependency { .. }
            }
        )
    })
    .await;
    h.runtime
        .send_message(
            second.id,
            vec![ContentBlock::text("stop what you are doing")],
            Actor::user(),
        )
        .expect("send");
    // It answers while the dependency is still running: the gate does not
    // make it deaf.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let a = h.store.agent(second.id).expect("a").expect("exists");
        if a.stats.turns > 0 {
            break;
        }
        assert!(
            !h.store
                .task(first_task.id)
                .expect("task")
                .expect("exists")
                .status
                .is_terminal(),
            "the dependency finished before the person was answered"
        );
        assert!(tokio::time::Instant::now() < deadline, "never answered");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let transcript = h.store.transcript(second.id, 0).expect("transcript");
    assert!(
        transcript
            .iter()
            .any(|m| m.message.text().contains("I hear you")),
        "{transcript:#?}"
    );
    // The gate still holds the task itself until the dependency is done: the
    // agent answered the person, it did not start the work.
    assert_eq!(
        h.store
            .task(gated.id)
            .expect("task")
            .expect("exists")
            .status
            .kind(),
        TaskStatusKind::Queued
    );
    let status = h.wait_until(second.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
}
