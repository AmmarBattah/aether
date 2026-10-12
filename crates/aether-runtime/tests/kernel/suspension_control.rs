//! suspension control scenarios and fixtures.
use super::*;

/// A suspended parent holds nothing up: its children keep running on their own
/// providers, and their results are waiting for it when it resumes.
#[tokio::test]
async fn children_keep_running_while_the_parent_is_suspended() {
    let h = harness_configured(
        vec![
            ScriptedClient::tool_call(
                "delegate",
                json!({
                    "model": "groq/openai/gpt-oss-120b", "effort": "low", "role": "worker",
                    "objective": "do the sub task", "wait": false
                }),
            ),
            ScriptedClient::error(ModelErrorClass::UsageLimit, "5-hour limit reached"),
            ScriptedClient::tool_call(
                "complete_task",
                json!({"summary": "read the child's result"}),
            ),
        ],
        |_| {},
        |config| {
            config.exhaustion_probe_floor = NO_PROBE_IN_THIS_TEST;
            // The child's turn must not occupy the parent's only slot.
            config.max_concurrent_turns = 2;
        },
    )
    .await;
    // A bounded check-in keeps the child live while its parent suspends;
    // the person's message below wakes it before the deadline.
    h.client.push_for(
        "groq",
        ScriptedClient::tool_call("wait_for", json!({"timeout_seconds": 3600})),
    );
    h.client.push_for(
        "groq",
        ScriptedClient::tool_call("complete_task", json!({"summary": "sub task done"})),
    );
    let (parent, _) = h
        .runtime
        .create_agent(spec("delegate, then run out of credits"), Actor::user())
        .expect("parent");
    h.wait_until(parent.id, blocked_on_exhaustion).await;
    let children = h
        .store
        .agents(&AgentFilter {
            parent: Some(parent.id),
            ..Default::default()
        })
        .expect("children");
    let child = children.first().expect("a delegated child").id;
    let waiting = h
        .wait_until(child, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    assert!(!waiting.is_terminal(), "{waiting:?}");

    h.runtime
        .send_message(child, vec![ContentBlock::text("carry on")], Actor::user())
        .expect("message");
    h.wait_until(child, |s| s.is_terminal()).await;
    // The parent is still suspended; a person lets it read what came in.
    h.runtime
        .resume_agent(parent.id, false, Actor::user())
        .expect("resume");
    let status = h.wait_until(parent.id, |s| s.is_terminal()).await;
    assert!(
        matches!(status, AgentStatus::Completed { .. }),
        "{status:?}"
    );
    let completions = h
        .store
        .inbox(parent.id, 0)
        .expect("inbox")
        .into_iter()
        .filter(|item| {
            matches!(
                item.kind,
                aether_core::inbox::InboxKind::TaskCompleted { .. }
            )
        })
        .count();
    assert_eq!(completions, 1, "the parent must read its child's result");
}

/// A person may decide not to wait: `aether resume` probes at once and lets the
/// provider off the bench, while a message probes without freeing it for others.
#[tokio::test]
async fn operator_resume_probes_immediately() {
    let h = harness_configured(
        vec![
            ScriptedClient::error(ModelErrorClass::UsageLimit, "5-hour limit reached"),
            ScriptedClient::tool_call("complete_task", json!({"summary": "resumed by hand"})),
        ],
        |_| {},
        |config| config.exhaustion_probe_floor = NO_PROBE_IN_THIS_TEST,
    )
    .await;
    let (_, child, _) = parent_and_suspending_child(&h, spec("wait for a person")).await;
    h.wait_until(child, blocked_on_exhaustion).await;
    assert_eq!(
        bench_history(h.store.as_ref()).1,
        0,
        "benched, not recovered"
    );

    h.runtime
        .resume_agent(child, false, Actor::user())
        .expect("resume");
    let status = h.wait_until(child, |s| s.is_terminal()).await;
    assert!(
        matches!(status, AgentStatus::Completed { .. }),
        "{status:?}"
    );
    assert_eq!(
        bench_history(h.store.as_ref()).1,
        1,
        "a person's resume must let the provider off the bench"
    );
}

#[tokio::test]
async fn a_persons_message_wakes_a_suspended_agent() {
    let h = harness_configured(
        vec![
            ScriptedClient::error(ModelErrorClass::UsageLimit, "5-hour limit reached"),
            ScriptedClient::tool_call("complete_task", json!({"summary": "answered the person"})),
        ],
        |_| {},
        |config| config.exhaustion_probe_floor = NO_PROBE_IN_THIS_TEST,
    )
    .await;
    let (_, child, _) = parent_and_suspending_child(&h, spec("wait for a person")).await;
    h.wait_until(child, blocked_on_exhaustion).await;
    h.runtime
        .send_message(child, vec![ContentBlock::text("keep going")], Actor::user())
        .expect("message");
    let status = h.wait_until(child, |s| s.is_terminal()).await;
    assert!(
        matches!(status, AgentStatus::Completed { .. }),
        "{status:?}"
    );
    // An addressed turn bypasses the bench; it is not lifted for everyone else.
    assert_eq!(bench_history(h.store.as_ref()).1, 0);
}

/// Waiting for credits is not a licence to outlive a budget: the local stop
/// still ends the agent and takes its subtree with it.
#[tokio::test]
async fn budget_stop_on_a_suspended_agent_stays_terminal_and_cancels_the_subtree() {
    let h = harness_configured(
        vec![ScriptedClient::error(
            ModelErrorClass::UsageLimit,
            "5-hour limit reached",
        )],
        |_| {},
        |config| config.exhaustion_probe_floor = NO_PROBE_IN_THIS_TEST,
    )
    .await;
    let mut child_spec = spec("spend carefully, then run out of credits");
    child_spec.config.as_mut().expect("config").cost_budget = Some(0.01);
    let (_, child, _) = parent_and_suspending_child(&h, child_spec).await;
    h.wait_until(child, blocked_on_exhaustion).await;
    let (grandchild, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                parent: Some(child),
                start: false,
                ..spec("work under a suspended agent")
            },
            Actor::Agent { id: child },
        )
        .expect("grandchild");

    let mut suspended = h.store.agent(child).expect("a").expect("e");
    suspended.stats.usage += aether_core::model::Usage::cost_only(Some(1.0));
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(suspended)))
        .expect("spend");
    let status = h.wait_until(child, |s| s.is_terminal()).await;
    let AgentStatus::Failed { error } = &status else {
        panic!("a budget stop is terminal, got {status:?}")
    };
    assert!(error.contains("cost budget of $0.01"), "{error}");
    let status = h.wait_until(grandchild.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Cancelled, "{status:?}");
}

/// The row is the whole plan: a daemon that restarts over a suspended agent
/// leaves it alone, tells no one twice, and releases it when its probe is due.
#[tokio::test]
async fn suspension_survives_a_restart_without_reclassification() {
    let h = harness_configured(
        vec![ScriptedClient::error(
            ModelErrorClass::UsageLimit,
            "5-hour limit reached",
        )],
        |_| {},
        |config| config.exhaustion_probe_floor = Duration::from_millis(400),
    )
    .await;
    let (parent, child, _) = parent_and_suspending_child(&h, spec("survive a restart")).await;
    let parked = h.wait_until(child, blocked_on_exhaustion).await;
    h.runtime.shutdown().await;

    let restarted = harness_full(
        vec![ScriptedClient::tool_call(
            "complete_task",
            json!({"summary": "finished after the restart"}),
        )],
        |_| {},
        |config| config.exhaustion_probe_floor = Duration::from_millis(400),
        None,
        None,
        Some(h.store.clone()),
        |_| {},
    )
    .await;
    let (next_probe, probes) = schedule_of(&exhausted_reason(
        &restarted.store.agent(child).expect("a").expect("e").status,
    ));
    let (was_next, was_probes) = schedule_of(&exhausted_reason(&parked));
    assert_eq!(
        (next_probe, probes),
        (was_next, was_probes),
        "the row moved"
    );
    let t = types(restarted.store.as_ref(), child);
    assert!(
        !t.contains(&"recovery.classified".to_owned()),
        "recovery must leave a suspended row alone: {t:?}"
    );

    let status = restarted.wait_until(child, |s| s.is_terminal()).await;
    assert!(
        matches!(status, AgentStatus::Completed { .. }),
        "{status:?}"
    );
    assert_eq!(
        child_suspensions(restarted.store.as_ref(), parent).len(),
        1,
        "the ancestors were told once, before the restart"
    );
    restarted.runtime.shutdown().await;
}
