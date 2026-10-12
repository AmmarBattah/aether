//! parent waits scenarios and fixtures.
use super::*;

#[tokio::test]
async fn a_message_wakes_a_waiting_parent_and_its_wait_is_restored() {
    let h = harness(vec![
        // parent turn 1: delegate and wait
        ScriptedClient::tool_call(
            "delegate",
            json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "do the sub task", "wait": true}),
        ),
        // child turn 1: park on a question so it stays alive without finishing
        ScriptedClient::tool_call("ask_user", json!({"question": "which colour?"})),
        // parent turn 2: woken by the user, only replies
        ScriptedClient::text("Still waiting on the worker; I will report when it is done."),
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
    let AgentStatus::Waiting { condition } = parked else {
        unreachable!()
    };
    assert_eq!(condition.task_ids().len(), 1, "{condition:?}");
    assert!(
        condition.earliest_deadline().is_none(),
        "delegated work wakes its parent through events, not polling: {condition:?}"
    );
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

    let requests_while_idle = h.client.requests().len();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(h.client.requests().len(), requests_while_idle);

    // A person speaks: the parent wakes, answers, and goes back to the same wait.
    h.runtime
        .send_message(
            parent.id,
            vec![ContentBlock::text("are you there?")],
            Actor::user(),
        )
        .expect("send");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let restored = loop {
        let agent = h.store.agent(parent.id).expect("agent").expect("exists");
        if agent.stats.turns >= 2 && matches!(agent.status, AgentStatus::Waiting { .. }) {
            break agent;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "parent never returned to waiting: {:?}",
            agent.status
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(
        restored.status,
        AgentStatus::Waiting {
            condition: condition.clone()
        },
        "the interrupted wait is restored unchanged"
    );
    assert!(restored.resume_wait.is_none());
    let t = types(h.store.as_ref(), parent.id);
    assert_eq!(
        t.iter().filter(|t| *t == "agent.resumed").count(),
        1,
        "{t:?}"
    );
    assert!(
        h.client
            .requests()
            .iter()
            .any(|r| format!("{r:?}").contains("are you there?")),
        "the message reached the model"
    );

    // The child finishes after the user's answer; the restored wait fires.
    h.runtime
        .send_message(child.id, vec![ContentBlock::text("blue")], Actor::user())
        .expect("send");
    let status = h.wait_until(parent.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    let t = types(h.store.as_ref(), parent.id);
    assert_eq!(
        t.iter().filter(|t| *t == "agent.resumed").count(),
        2,
        "one wake for the person, one for the finished work: {t:?}"
    );
}

/// The three-level tree both parent-interrupt tests need: `top` delegates to
/// `middle`, which delegates to `leaf` and waits on that task; `leaf` parks on
/// a question so it never finishes on its own.
async fn waiting_middle_of_a_three_level_tree(h: &Harness) -> (AgentId, AgentId, AgentId) {
    let (top, _) = h
        .runtime
        .create_agent(spec("split the work"), Actor::user())
        .expect("create");
    let child_of = |parent: AgentId| {
        h.store
            .agents(&AgentFilter {
                parent: Some(parent),
                ..Default::default()
            })
            .expect("agents")
            .pop()
            .expect("child")
    };
    h.wait_until(top.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    let middle = child_of(top.id);
    h.wait_until(middle.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    let leaf = child_of(middle.id);
    h.wait_until(leaf.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    (top.id, middle.id, leaf.id)
}

fn resume_reasons(store: &dyn Store, id: AgentId) -> Vec<String> {
    store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events")
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::AgentResumed { reason, .. } => Some(reason),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_parents_message_wakes_a_child_waiting_on_its_own_delegation() {
    let h = harness(vec![
        // top turn 1: delegate and wait
        ScriptedClient::tool_call(
            "delegate",
            json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "run the fan-out", "wait": true}),
        ),
        // middle turn 1: delegate further and wait
        ScriptedClient::tool_call(
            "delegate",
            json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "do the sub task", "wait": true}),
        ),
        // leaf turn 1: park on a question so it stays alive without finishing
        ScriptedClient::tool_call("ask_user", json!({"question": "which colour?"})),
        // middle turn 2: woken by its parent, only replies
        ScriptedClient::text("Understood; still waiting on the worker."),
        // leaf turn 2: finish once the user answered
        ScriptedClient::tool_call("complete_task", json!({"summary": "leaf finished"})),
        // middle turn 3, then top turn 2: finish with the result
        ScriptedClient::tool_call("complete_task", json!({"summary": "middle done"})),
        ScriptedClient::tool_call("complete_task", json!({"summary": "top done"})),
    ])
    .await;
    let (top, middle, leaf) = waiting_middle_of_a_three_level_tree(&h).await;
    let waiting = h
        .store
        .agent(middle)
        .expect("agent")
        .expect("exists")
        .status;
    let AgentStatus::Waiting { condition } = waiting else {
        unreachable!()
    };
    assert_eq!(condition.task_ids().len(), 1, "{condition:?}");

    // The supervisor speaks: the child wakes mid fan-out and goes back to the
    // same wait afterwards.
    h.runtime
        .send_message(
            middle,
            vec![ContentBlock::text("change of plan, report early")],
            Actor::Agent { id: top },
        )
        .expect("send");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let restored = loop {
        let agent = h.store.agent(middle).expect("agent").expect("exists");
        if agent.stats.turns >= 2 && matches!(agent.status, AgentStatus::Waiting { .. }) {
            break agent;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "middle never returned to waiting: {:?}",
            agent.status
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(
        restored.status,
        AgentStatus::Waiting { condition },
        "the interrupted wait is restored unchanged"
    );
    assert!(restored.resume_wait.is_none());
    assert_eq!(
        resume_reasons(h.store.as_ref(), middle),
        vec!["message from your parent".to_string()],
        "the wake names the parent"
    );
    assert!(
        h.client
            .requests()
            .iter()
            .any(|r| format!("{r:?}").contains("change of plan")),
        "the message reached the model"
    );

    // The leaf finishes after the user's answer; the restored wait fires.
    h.runtime
        .send_message(leaf, vec![ContentBlock::text("blue")], Actor::user())
        .expect("send");
    let status = h.wait_until(top, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    assert_eq!(
        resume_reasons(h.store.as_ref(), middle).len(),
        2,
        "one wake for the parent, one for the finished work"
    );
    assert_eq!(h.client.remaining(), 0);
}

#[tokio::test]
async fn a_message_from_a_non_parent_agent_does_not_wake_a_waiting_agent() {
    let h = harness(vec![
        ScriptedClient::tool_call(
            "delegate",
            json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "run the fan-out", "wait": true}),
        ),
        ScriptedClient::tool_call(
            "delegate",
            json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "do the sub task", "wait": true}),
        ),
        ScriptedClient::tool_call("ask_user", json!({"question": "which colour?"})),
        // No wake-up turn for middle: neither message below is urgent.
        ScriptedClient::tool_call("complete_task", json!({"summary": "leaf finished"})),
        ScriptedClient::tool_call("complete_task", json!({"summary": "middle done"})),
        ScriptedClient::tool_call("complete_task", json!({"summary": "top done"})),
    ])
    .await;
    let (_top, middle, leaf) = waiting_middle_of_a_three_level_tree(&h).await;

    // Its own child and an unrelated agent both speak; neither is urgent.
    for from in [leaf, AgentId::new()] {
        h.runtime
            .send_message(
                middle,
                vec![ContentBlock::text("just so you know")],
                Actor::Agent { id: from },
            )
            .expect("send");
    }
    // Several scheduler ticks (notified plus the one-second timer) must pass
    // without the wait being interrupted.
    let until = tokio::time::Instant::now() + Duration::from_millis(1_500);
    while tokio::time::Instant::now() < until {
        let agent = h.store.agent(middle).expect("agent").expect("exists");
        assert!(
            matches!(agent.status, AgentStatus::Waiting { .. }),
            "a non-parent message must not wake the agent: {:?}",
            agent.status
        );
        assert_eq!(agent.stats.turns, 1, "no extra turn was taken");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(resume_reasons(h.store.as_ref(), middle).is_empty());

    // The awaited work still wakes it, and the messages ride along.
    h.runtime
        .send_message(leaf, vec![ContentBlock::text("blue")], Actor::user())
        .expect("send");
    let status = h.wait_until(middle, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    assert_eq!(
        resume_reasons(h.store.as_ref(), middle),
        vec!["awaited work finished".to_string()]
    );
}
