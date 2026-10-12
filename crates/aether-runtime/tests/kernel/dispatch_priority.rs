//! dispatch priority scenarios and fixtures.
use super::*;

/// Limit 6: among what the limits above allow, priority decides, then
/// whoever has waited longest.
#[tokio::test]
async fn priority_then_longest_waiting_decides_who_runs_next() {
    use aether_core::task::Priority;
    let h = harness_configured(
        vec![
            turn_of_length(120, "blocker"),
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
        ],
        |_| {},
        |c| c.max_concurrent_turns = 1,
    )
    .await;
    h.client.set_delay(Duration::from_millis(20));
    let (blocker, _) = h
        .runtime
        .create_agent(spec("hold the only slot"), Actor::user())
        .expect("blocker");
    let queue = |objective: &str, priority: Priority| {
        let mut s = spec(objective);
        s.config.as_mut().expect("config").priority = priority;
        h.runtime.create_agent(s, Actor::user()).expect("create").0
    };
    // Queued behind the blocker, in an order the expected result reverses.
    let normal_first = queue("normal a", Priority::Normal);
    let low = queue("low", Priority::Low);
    let high = queue("high", Priority::High);
    let normal_second = queue("normal b", Priority::Normal);
    for id in [
        blocker.id,
        normal_first.id,
        low.id,
        high.id,
        normal_second.id,
    ] {
        h.wait_until(id, |s| s.is_terminal()).await;
    }
    let started = |id: AgentId| {
        h.store
            .events(0, 10_000, &EventFilter::for_agent(id))
            .expect("events")
            .into_iter()
            .find(|e| e.type_name() == "agent.started")
            .map(|e| e.seq)
            .expect("agent.started")
    };
    let order = [
        ("high", started(high.id)),
        ("normal a", started(normal_first.id)),
        ("normal b", started(normal_second.id)),
        ("low", started(low.id)),
    ];
    assert!(
        order.windows(2).all(|w| w[0].1 < w[1].1),
        "wrong dispatch order: {order:?}"
    );
    assert_eq!(h.client.remaining(), 0);
}

/// Limit 1: a person's message is dispatched ahead of everything and past
/// the limits below it - the global limit, its supervisor's subtree cap, and
/// its provider's cap, all met at once by a sibling's running turn.
#[tokio::test]
async fn a_person_is_dispatched_past_every_limit_below() {
    let h = harness_full(
        vec![
            ScriptedClient::tool_call("complete_task", json!({"summary": "worker done"})),
            ScriptedClient::text("Still running; I will report when it finishes."),
        ],
        |_| {},
        |c| c.max_concurrent_turns = 1,
        Some("[providers.anthropic]\nmax_concurrent = 1\n"),
        None,
        None,
        |_| {},
    )
    .await;
    let release_worker = h.client.hold_next_response();
    let mut supervisor = spec("supervise");
    supervisor.objective = None;
    supervisor.config.as_mut().expect("config").concurrency = Some(1);
    supervisor.config.as_mut().expect("config").long_lived = true;
    let (parent, _) = h
        .runtime
        .create_agent(supervisor, Actor::user())
        .expect("parent");
    let (worker, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                parent: Some(parent.id),
                ..spec("the long job")
            },
            Actor::user(),
        )
        .expect("worker");
    let mut reporter_spec = spec("report");
    reporter_spec.objective = None;
    reporter_spec.parent = Some(parent.id);
    reporter_spec.config.as_mut().expect("config").long_lived = true;
    let (reporter, _) = h
        .runtime
        .create_agent(reporter_spec, Actor::user())
        .expect("reporter");

    // Wait until the worker holds the only slot, the parent's cap and the
    // provider's cap; then speak to its sibling.
    h.wait_until(worker.id, |s| s.kind() == AgentStatusKind::Running)
        .await;
    // Running is recorded before model invocation. Wait for the worker to
    // take its scripted response before the reporter can consume the next one.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while h.client.remaining() == 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "worker never requested its response: {}",
            timeout_details(h.store.as_ref(), worker.id)
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        h.client.remaining(),
        1,
        "only the worker response was consumed"
    );
    h.runtime
        .send_message(
            reporter.id,
            vec![ContentBlock::text("status?")],
            Actor::user(),
        )
        .expect("send");

    let seq_of = |id: AgentId, kind: &str| {
        h.store
            .events(0, 10_000, &EventFilter::for_agent(id))
            .expect("events")
            .into_iter()
            .find(|e| e.type_name() == kind)
            .map(|e| e.seq)
    };
    let answered = loop {
        if let Some(seq) = seq_of(reporter.id, "invocation.completed") {
            break seq;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the person was never answered"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    release_worker.cancel();
    let worker_done = loop {
        if let Some(seq) = seq_of(worker.id, "agent.turn_completed") {
            break seq;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "worker never ended: {}",
            timeout_details(h.store.as_ref(), worker.id)
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert!(
        answered < worker_done,
        "the person waited for the worker: answered at {answered}, worker ended at {worker_done}"
    );
    h.wait_until(worker.id, |s| s.is_terminal()).await;
    assert_eq!(h.client.remaining(), 0);
}

/// CLI engines can finish a waiting turn with assistant prose and no pending
/// tool result. A task owned elsewhere must still wake such a waiter.
#[tokio::test]
async fn a_task_only_wait_continues_without_a_direct_completion_message() {
    use aether_core::agent::WaitCondition;
    use aether_core::task::{Task, TaskResult, TaskStatus};
    for (already_finished, restart) in [(false, false), (true, false), (true, true)] {
        let mut awaited = Task::new("independent work", Actor::user(), Default::default(), now());
        let finished = TaskStatus::Completed {
            result: TaskResult::summary(
                "Independent evidence: the heading is Documentation fixture.",
            ),
        };
        if already_finished {
            awaited.status = finished.clone();
        }
        let h = harness_with(
            vec![ScriptedClient::tool_call(
                "complete_task",
                json!({"summary":"integrated evidence"}),
            )],
            |store| {
                store
                    .commit(Commit::new().mutate(Mutation::UpsertTask(awaited.clone())))
                    .unwrap();
            },
        )
        .await;
        if restart {
            h.runtime.shutdown().await;
        }
        let mut spec = spec("wait for independent work");
        spec.start = false;
        let (mut agent, _) = h.runtime.create_agent(spec, Actor::user()).unwrap();
        agent.transition(AgentStatus::Ready, now()).unwrap();
        agent.transition(AgentStatus::Running, now()).unwrap();
        agent
            .transition(
                AgentStatus::Waiting {
                    condition: WaitCondition::Task { id: awaited.id },
                },
                now(),
            )
            .unwrap();
        agent.stats.turns = 1;
        h.store
            .commit(
                Commit::new()
                    .event(
                        aether_core::event::EventDraft::new(
                            Actor::Agent { id: agent.id },
                            EventKind::AgentMessage {
                                role: aether_core::model::MessageRole::Assistant,
                                content: vec![ContentBlock::text(
                                    "Awaiting the independent result.",
                                )],
                                invocation: None,
                            },
                        )
                        .agent(agent.id)
                        .run(agent.run),
                    )
                    .mutate(Mutation::UpsertAgent(agent.clone())),
            )
            .unwrap();
        if !already_finished {
            awaited.status = finished;
            // No completion inbox item is addressed to the waiter.
            h.store
                .commit(Commit::new().mutate(Mutation::UpsertTask(awaited.clone())))
                .unwrap();
        }
        let restored = if restart {
            Some(
                harness_full(
                    vec![ScriptedClient::tool_call(
                        "complete_task",
                        json!({"summary":"integrated evidence"}),
                    )],
                    |_| {},
                    |_| {},
                    None,
                    None,
                    Some(h.store.clone()),
                    |_| {},
                )
                .await,
            )
        } else {
            None
        };
        let active = restored.as_ref().unwrap_or(&h);
        h.wait_until(agent.id, |state| state.is_terminal()).await;
        let agent = h.store.agent(agent.id).unwrap().unwrap();
        assert_eq!(agent.status.kind(), AgentStatusKind::Completed);
        assert_eq!(agent.stats.turns, 2);
        let wakes = h
            .store
            .inbox(agent.id, 0)
            .unwrap()
            .into_iter()
            .filter_map(|item| match item.kind {
                aether_core::inbox::InboxKind::Wake { reason } => Some(reason),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(wakes.len(), 1);
        assert!(wakes[0].contains(&awaited.id.to_string()));
        assert!(wakes[0].contains("completed"));
        assert!(!wakes[0].contains("Independent evidence"));
        assert!(
            active
                .client
                .requests()
                .iter()
                .flat_map(|request| &request.messages)
                .any(|message| message.text().contains(&awaited.id.to_string()))
        );
    }
}
