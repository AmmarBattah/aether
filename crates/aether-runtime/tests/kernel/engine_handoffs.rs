//! engine handoffs scenarios and fixtures.
use super::*;

#[tokio::test]
async fn changing_an_idle_agents_engine_waits_for_the_next_message() {
    let h = harness(vec![
        ScriptedClient::text(
            &json!({
                "checkpoint": {
                    "objective": "answer the operator",
                    "state": "waiting for a message",
                    "decisions": [],
                    "constraints": [],
                    "open_loops": []
                },
                "memory_candidates": []
            })
            .to_string(),
        ),
        ScriptedClient::text("ready"),
    ])
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                model: Some(ModelRef::new("claude-code", "haiku")),
                start: true,
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let next = ModelRef::new("anthropic", "claude-sonnet-5-5");
    h.runtime
        .update_agent(
            agent.id,
            AgentUpdate {
                model: Some(next.clone()),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("switch");
    assert_eq!(
        h.store.inbox_last_seq(agent.id).expect("inbox seq"),
        0,
        "a model selection must not give an idle agent work"
    );
    assert!(
        h.client.requests().is_empty(),
        "no model call before a message"
    );

    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("hello after switching")],
            Actor::user(),
        )
        .expect("send");
    h.wait_for("first turn after switching", || {
        h.store
            .agent(agent.id)
            .expect("agent")
            .is_some_and(|a| a.status.kind() == AgentStatusKind::Waiting && a.stats.turns >= 1)
    })
    .await;
    assert!(
        types(h.store.as_ref(), agent.id).contains(&"context.handoff".to_owned()),
        "the next turn still performs the handoff"
    );
    assert_eq!(
        h.store
            .agent(agent.id)
            .expect("agent")
            .expect("exists")
            .model,
        next
    );
}

/// A handoff is owed whenever the model that keeps the history changes:
/// across the engine/API boundary in either direction, and between two
/// engines. Moving within one engine keeps its session and owes nothing.
#[tokio::test]
async fn a_session_never_survives_the_engine_that_owns_it() {
    let cases = [
        (
            ("claude-code", "haiku"),
            ("anthropic", "claude-sonnet-5-5"),
            true,
        ),
        (
            ("anthropic", "claude-sonnet-5-5"),
            ("claude-code", "haiku"),
            true,
        ),
        (("claude-code", "haiku"), ("codex", "gpt-6.1-sol"), true),
        (("claude-code", "haiku"), ("claude-code", "opus"), false),
        (("codex", "gpt-6.1-sol"), ("codex", "default"), true),
        (("codex", "gpt-6.1-sol"), ("codex", "gpt-6-luna"), false),
    ];
    for ((from_p, from_m), (to_p, to_m), expected) in cases {
        let h = harness(Vec::new()).await;
        let from = ModelRef::new(from_p, from_m);
        let to = ModelRef::new(to_p, to_m);
        let (agent, _) = h
            .runtime
            .create_agent(
                CreateAgentSpec {
                    role: "assistant".into(),
                    model: Some(from.clone()),
                    config: Some(AgentConfig {
                        long_lived: true,
                        approval: ApprovalPolicy::none(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Actor::user(),
            )
            .expect("create");
        // An engine turn would have left one; the API path never runs here.
        let mut row = h.store.agent(agent.id).expect("a").expect("exists");
        row.engine_session = Some("s1".into());
        h.store
            .commit(Commit::new().mutate(Mutation::UpsertAgent(row)))
            .expect("seed session");

        h.runtime
            .update_agent(
                agent.id,
                AgentUpdate {
                    model: Some(to.clone()),
                    ..Default::default()
                },
                Actor::user(),
            )
            .expect("switch");

        let stored = h.store.agent(agent.id).expect("a").expect("exists");
        let handed =
            types(h.store.as_ref(), agent.id).contains(&"compaction.requested".to_string());
        assert_eq!(handed, expected, "{from} -> {to}: handoff");
        assert_eq!(
            stored.engine_session.is_none(),
            expected,
            "{from} -> {to}: a session the new model cannot resume was kept"
        );
    }
}

#[tokio::test]
async fn rapid_selections_keep_the_original_handoff_source_and_session() {
    let h = harness(Vec::new()).await;
    let original = ModelRef::new("codex", "gpt-6.1-sol");
    let middle = ModelRef::new("anthropic", "claude-sonnet-5-5");
    let final_model = ModelRef::new("anthropic", "claude-haiku-5-5");
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                model: Some(original.clone()),
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    let mut row = h.store.agent(agent.id).expect("read").expect("agent");
    row.engine_session = Some("source-session".into());
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(row)))
        .expect("seed session");

    for model in [middle, final_model.clone()] {
        h.runtime
            .update_agent(
                agent.id,
                AgentUpdate {
                    model: Some(model),
                    ..Default::default()
                },
                Actor::user(),
            )
            .expect("select model");
    }
    let handoff = h
        .store
        .latest_events(
            1,
            &EventFilter {
                types: vec!["compaction.requested".into()],
                ..EventFilter::own_events(agent.id)
            },
        )
        .expect("requests")
        .pop()
        .expect("latest request");
    assert!(matches!(
        handoff.kind,
        EventKind::CompactionRequested {
            trigger: CompactionTrigger::Handoff { from, to, session }
        } if from == original && to == final_model && session.as_deref() == Some("source-session")
    ));
}

#[tokio::test]
async fn a_new_selection_while_compaction_waits_remains_pending() {
    let checkpoint = json!({
        "checkpoint": {
            "objective": "answer the operator",
            "state": "the old model answered",
            "decisions": [], "constraints": [], "open_loops": []
        },
        "memory_candidates": []
    })
    .to_string();
    let h = harness(vec![
        ScriptedClient::text("old model answered"),
        ScriptedClient::text(&checkpoint),
        ScriptedClient::text("middle model answered"),
    ])
    .await;
    let original = ModelRef::new("claude-code", "haiku");
    let middle = ModelRef::new("anthropic", "claude-sonnet-5-5");
    let final_model = ModelRef::new("anthropic", "claude-haiku-5-5");
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                model: Some(original.clone()),
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    assert_eq!(
        h.client.requests().len(),
        0,
        "an idle agent must not consume the old-turn response"
    );
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("old turn")],
            Actor::user(),
        )
        .expect("send");
    h.wait_for("old model reply", || !h.client.requests().is_empty())
        .await;
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    assert_eq!(
        h.client.remaining(),
        2,
        "the next scripted response must belong to the compactor"
    );
    let mut row = h.store.agent(agent.id).expect("read").expect("agent");
    row.engine_session = Some("old-session".into());
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(row)))
        .expect("seed session");

    let release = h.client.hold_next_response();
    h.runtime
        .update_agent(
            agent.id,
            AgentUpdate {
                model: Some(middle.clone()),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("first switch");
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("continue on the middle model")],
            Actor::user(),
        )
        .expect("send after switching");
    h.wait_for("compactor request", || h.client.requests().len() >= 2)
        .await;
    assert_eq!(h.client.remaining(), 1, "the compactor took the gate");
    assert_eq!(
        h.store
            .agent(agent.id)
            .expect("read")
            .expect("agent")
            .status
            .kind(),
        AgentStatusKind::Running
    );
    h.runtime
        .update_agent(
            agent.id,
            AgentUpdate {
                model: Some(final_model.clone()),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("second switch");
    release.cancel();
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let latest = h
        .store
        .latest_events(
            1,
            &EventFilter {
                types: vec![
                    "compaction.requested".into(),
                    "compaction.completed".into(),
                    "compaction.failed".into(),
                ],
                ..EventFilter::own_events(agent.id)
            },
        )
        .expect("compaction events")
        .pop()
        .expect("latest compaction event");
    assert!(matches!(
        latest.kind,
        EventKind::CompactionRequested {
            trigger: CompactionTrigger::Handoff { from, to, session }
        } if from == original && to == final_model && session.as_deref() == Some("old-session")
    ));
    assert_eq!(
        h.store.agent(agent.id).expect("read").expect("agent").model,
        final_model
    );
}

#[tokio::test]
async fn a_second_selection_before_compaction_does_not_skip_the_started_turns_handoff() {
    let inner: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let store = Arc::new(CommitsMidPass::new(inner, Hook::CompactionPending));
    let checkpoint = json!({
        "checkpoint": {
            "objective": "answer the operator",
            "state": "the original model answered",
            "decisions": [], "constraints": [], "open_loops": []
        },
        "memory_candidates": []
    })
    .to_string();
    let h = harness_full(
        vec![
            ScriptedClient::text("original model answered"),
            ScriptedClient::text(&checkpoint),
            ScriptedClient::text("middle model answered"),
        ],
        |_| {},
        |_| {},
        None,
        None,
        Some(store.clone()),
        |_| {},
    )
    .await;
    let original = ModelRef::new("claude-code", "haiku");
    let middle = ModelRef::new("anthropic", "claude-sonnet-5-5");
    let final_model = ModelRef::new("anthropic", "claude-haiku-5-5");
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                model: Some(original.clone()),
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("original turn")],
            Actor::user(),
        )
        .expect("send");
    h.wait_for("original model reply", || !h.client.requests().is_empty())
        .await;
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    assert_eq!(h.client.remaining(), 2);

    let id = agent.id;
    let source = original.clone();
    let destination = final_model.clone();
    store.arm(Box::new(move |inner| {
        let mut row = inner.agent(id)?.expect("agent");
        assert_eq!(row.status.kind(), AgentStatusKind::Running);
        let session = Some("original-session".into());
        row.model = destination.clone();
        inner.commit(
            Commit::new()
                .event(
                    EventDraft::new(
                        Actor::user(),
                        EventKind::CompactionRequested {
                            trigger: CompactionTrigger::Handoff {
                                from: source,
                                to: destination,
                                session,
                            },
                        },
                    )
                    .agent(id),
                )
                .mutate(Mutation::UpsertAgent(row)),
        )?;
        Ok(())
    }));
    h.runtime
        .update_agent(
            id,
            AgentUpdate {
                model: Some(middle.clone()),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("first switch");
    h.runtime
        .send_message(
            id,
            vec![ContentBlock::text("continue on the middle model")],
            Actor::user(),
        )
        .expect("send after switching");
    h.wait_for("the second selection", || store.fired()).await;
    h.wait_until(id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let events = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events");
    assert!(
        events.iter().any(|event| matches!(
            &event.kind,
            EventKind::ContextHandoff { from, to, checkpoint: Some(_), .. }
                if from == &original && to == &middle
        )),
        "the started middle turn lost the checkpoint from the original model"
    );
    let latest = h
        .store
        .latest_events(
            1,
            &EventFilter {
                types: vec![
                    "compaction.requested".into(),
                    "compaction.completed".into(),
                    "compaction.failed".into(),
                ],
                ..EventFilter::own_events(id)
            },
        )
        .expect("compaction events")
        .pop()
        .expect("latest compaction event");
    assert!(matches!(
        latest.kind,
        EventKind::CompactionRequested {
            trigger: CompactionTrigger::Handoff { to, .. }
        } if to == final_model
    ));
}

/// The way back onto an engine is a handoff too. The engine that produced the
/// earlier turns is not the one being resumed, so the checkpoint built for the
/// switch has to reach it - reporting it carried while dropping it is worse
/// than not building it.
#[tokio::test]
async fn a_return_handoff_to_an_engine_carries_the_checkpoint() {
    let checkpoint = |state: &str| {
        serde_json::json!({
            "checkpoint": {
                "objective": "answer the operator",
                "state": state,
                "decisions": [], "constraints": [], "open_loops": []
            },
            "memory_candidates": []
        })
        .to_string()
    };
    let h = harness(vec![
        ScriptedClient::text("Noted: Argos."),
        ScriptedClient::text("Noted: nine."),
        // handoff 1 (engine -> API), then the first turn on the API model.
        ScriptedClient::text(&checkpoint("the dog is Argos, aged nine")),
        ScriptedClient::text("Argos is nine."),
        // handoff 2 (API -> engine), then the first turn back on the engine.
        ScriptedClient::text(&checkpoint("the dog is Argos, aged nine, and eats at six")),
        ScriptedClient::text("Six o'clock."),
    ])
    .await;
    let engine = ModelRef::new("claude-code", "haiku");
    let api = ModelRef::new("anthropic", "claude-sonnet-5-5");
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                model: Some(engine.clone()),
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    let say = |text: &'static str| {
        let store = h.store.clone();
        let runtime = h.runtime.clone();
        let client = h.client.clone();
        let id = agent.id;
        let wait_ceiling = h.wait_ceiling;
        async move {
            wait_until_with_budget(
                store.as_ref(),
                id,
                |s| s.kind() == AgentStatusKind::Waiting,
                wait_ceiling,
                WAIT_UNTIL_STALL_WINDOW,
            )
            .await;
            let before = client.remaining();
            runtime
                .send_message(id, vec![ContentBlock::text(text)], Actor::user())
                .expect("send");
            let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
            while client.remaining() == before {
                assert!(tokio::time::Instant::now() < deadline, "turn did not run");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    };
    let switch = |to: ModelRef| {
        let runtime = h.runtime.clone();
        let store = h.store.clone();
        let id = agent.id;
        let wait_ceiling = h.wait_ceiling;
        async move {
            wait_until_with_budget(
                store.as_ref(),
                id,
                |s| s.kind() == AgentStatusKind::Waiting,
                wait_ceiling,
                WAIT_UNTIL_STALL_WINDOW,
            )
            .await;
            runtime
                .update_agent(
                    id,
                    AgentUpdate {
                        model: Some(to),
                        ..Default::default()
                    },
                    Actor::user(),
                )
                .expect("switch");
        }
    };

    say("My dog is called Argos.").await;
    say("He is nine.").await;
    // An engine turn would have left a session behind; the API path here
    // never does, so stand one in for it.
    let mut row = h.store.agent(agent.id).expect("a").expect("exists");
    row.engine_session = Some("s1".into());
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(row)))
        .expect("seed session");

    switch(api.clone()).await;
    assert!(
        h.store
            .agent(agent.id)
            .expect("a")
            .expect("exists")
            .engine_session
            .is_none(),
        "leaving the engine left its session id behind"
    );
    say("He eats at six.").await;
    switch(engine.clone()).await;
    say("When does he eat?").await;

    let handoffs: Vec<(
        ModelRef,
        ModelRef,
        Option<aether_core::id::CheckpointId>,
        u32,
    )> = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(agent.id))
        .expect("events")
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::ContextHandoff {
                from,
                to,
                checkpoint,
                carried,
                ..
            } => Some((from, to, checkpoint, carried)),
            _ => None,
        })
        .collect();
    assert_eq!(
        handoffs.len(),
        2,
        "both directions handed over: {handoffs:?}"
    );
    assert_eq!(handoffs[0].0, engine);
    assert_eq!(handoffs[0].1, api);
    let (_, to, checkpoint, carried) = &handoffs[1];
    assert_eq!(to, &engine);
    let checkpoint = checkpoint.expect("the return handoff built a checkpoint");
    assert!(*carried > 0, "it reported carrying nothing");

    // Reported carried is only true if the model was actually given it.
    let last = h
        .store
        .invocations_for_agent(agent.id)
        .expect("inv")
        .into_iter()
        .next_back()
        .expect("a turn ran on the engine model");
    let snapshot = h
        .store
        .snapshot(last.snapshot_id)
        .expect("snap")
        .expect("exists");
    let layer = snapshot
        .layers
        .iter()
        .find(|l| l.kind == aether_core::snapshot::LayerKind::Checkpoint)
        .and_then(|l| l.text.as_deref())
        .expect("the checkpoint reached the turn back on the engine");
    assert!(layer.contains("eats at six"), "{layer}");
    assert_eq!(
        h.store
            .latest_checkpoint(agent.id)
            .expect("cp")
            .expect("exists")
            .id,
        checkpoint
    );
}

/// A turn holds a snapshot of the agent taken when it started and writes the
/// row back after every model response. A model change that lands while it
/// runs must survive that, or the agent keeps running the old model while the
/// handoff it committed names the new one.
#[tokio::test]
async fn a_model_change_during_a_running_turn_is_not_written_back_away() {
    let h = harness(vec![
        ScriptedClient::text("working on it"),
        ScriptedClient::text("still here"),
    ])
    .await;
    h.client.set_delay(Duration::from_millis(200));
    let engine = ModelRef::new("claude-code", "haiku");
    let api = ModelRef::new("anthropic", "claude-sonnet-5-5");
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                model: Some(engine.clone()),
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("keep going")],
            Actor::user(),
        )
        .expect("send");
    // Catch the turn mid-flight: the row it will write back is already taken.
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Running)
        .await;
    h.runtime
        .update_agent(
            agent.id,
            AgentUpdate {
                model: Some(api.clone()),
                name: Some("renamed during turn".into()),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("switch model mid-turn");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let stored = h.store.agent(agent.id).expect("a").expect("exists");
        if stored.status.kind() != AgentStatusKind::Running {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "turn did not finish"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let stored = h.store.agent(agent.id).expect("a").expect("exists");
    assert_eq!(
        stored.model, api,
        "the turn wrote its stale snapshot over the operator's model change"
    );
    assert_eq!(stored.name, "renamed during turn");
    let requested = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(agent.id))
        .expect("events")
        .into_iter()
        .find_map(|e| match e.kind {
            EventKind::CompactionRequested {
                trigger: CompactionTrigger::Handoff { to, .. },
            } => Some(to),
            _ => None,
        })
        .expect("the switch asked for a handoff");
    assert_eq!(
        requested, stored.model,
        "the handoff names a model the agent is not running"
    );
}

/// Put the operator's write at the exact read where the outgoing turn decides
/// whether to compact. It must leave the handoff for the destination turn,
/// after its own response has reached the transcript.
#[tokio::test]
async fn a_handoff_requested_during_the_old_turn_runs_on_the_new_turn() {
    let inner: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let store = Arc::new(CommitsMidPass::new(inner, Hook::CompactionPending));
    let checkpoint = json!({
        "checkpoint": {
            "objective": "answer the operator",
            "state": "the old turn answered",
            "decisions": [], "constraints": [], "open_loops": []
        },
        "memory_candidates": []
    })
    .to_string();
    let h = harness_full(
        vec![
            ScriptedClient::text("old turn answered"),
            ScriptedClient::text(&checkpoint),
            ScriptedClient::text("new turn answered"),
        ],
        |_| {},
        |_| {},
        None,
        None,
        Some(store.clone()),
        |_| {},
    )
    .await;
    let old = ModelRef::new("claude-code", "haiku");
    let new = ModelRef::new("anthropic", "claude-sonnet-5-5");
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                model: Some(old.clone()),
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let id = agent.id;
    let old_for_write = old.clone();
    let new_for_write = new.clone();
    store.arm(Box::new(move |inner| {
        let mut row = inner.agent(id)?.expect("agent");
        assert_eq!(row.status.kind(), AgentStatusKind::Running);
        row.model = new_for_write.clone();
        let session = row.engine_session.take();
        inner.commit(
            Commit::new()
                .event(
                    EventDraft::new(
                        Actor::user(),
                        EventKind::CompactionRequested {
                            trigger: CompactionTrigger::Handoff {
                                from: old_for_write,
                                to: new_for_write,
                                session,
                            },
                        },
                    )
                    .agent(id),
                )
                .mutate(Mutation::UpsertAgent(row)),
        )?;
        Ok(())
    }));
    h.runtime
        .send_message(id, vec![ContentBlock::text("first")], Actor::user())
        .expect("first message");
    h.wait_for("the mid-turn switch", || store.fired()).await;
    h.wait_until(id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    let events = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, EventKind::ContextHandoff { .. })),
        "the old turn consumed the new turn's handoff"
    );
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::InvocationCompleted { model, .. } if model == &old
    )));

    h.runtime
        .send_message(id, vec![ContentBlock::text("second")], Actor::user())
        .expect("second message");
    h.wait_for("the destination model's reply", || {
        h.store
            .events(0, 10_000, &EventFilter::for_agent(id))
            .expect("events")
            .iter()
            .any(|event| {
                matches!(
                    &event.kind,
                    EventKind::InvocationCompleted { model, .. } if model == &new
                )
            })
    })
    .await;
    h.wait_until(id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    assert!(
        types(h.store.as_ref(), id).contains(&"context.handoff".to_owned()),
        "the destination turn did not process the handoff"
    );
}

/// §6.3: moving an agent between an engine and an API model hands its context
/// over as a checkpoint, and says what did not cross.
#[tokio::test]
async fn switching_across_the_engine_boundary_hands_the_context_over() {
    let checkpoint_json = serde_json::json!({
        "checkpoint": {
            "objective": "answer the operator",
            "state": "the operator's dog is called Argos and is nine",
            "decisions": [],
            "constraints": [],
            "open_loops": []
        },
        "memory_candidates": []
    })
    .to_string();
    let h = harness(vec![
        ScriptedClient::text("Noted: Argos."),
        ScriptedClient::text("Noted: nine."),
        // The handoff compaction, then the first turn on the new model.
        ScriptedClient::text(&checkpoint_json),
        ScriptedClient::text("Argos is nine."),
    ])
    .await;
    let engine = ModelRef::new("claude-code", "haiku");
    let api = ModelRef::new("anthropic", "claude-sonnet-5-5");
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                model: Some(engine.clone()),
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    for text in ["My dog is called Argos.", "He is nine."] {
        h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
            .await;
        let before = h.client.remaining();
        h.runtime
            .send_message(agent.id, vec![ContentBlock::text(text)], Actor::user())
            .expect("send");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while h.client.remaining() == before {
            assert!(tokio::time::Instant::now() < deadline, "turn did not run");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    h.runtime
        .update_agent(
            agent.id,
            AgentUpdate {
                model: Some(api.clone()),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("switch model");
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("How old is Argos?")],
            Actor::user(),
        )
        .expect("send after switching");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while h.client.remaining() > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the handoff turn did not run; {} responses left",
            h.client.remaining()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let checkpoint = h
        .store
        .latest_checkpoint(agent.id)
        .expect("cp")
        .expect("the handoff leaves a checkpoint");
    assert_eq!(
        checkpoint.provenance.trigger,
        Some(CompactionTrigger::Handoff {
            from: engine.clone(),
            to: api.clone(),
            // Driven through the scripted client, so this agent never had an
            // engine session to abandon - which is why nothing is lost below.
            session: None,
        })
    );
    let handoff = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(agent.id))
        .expect("events")
        .into_iter()
        .find_map(|e| match e.kind {
            EventKind::ContextHandoff {
                from,
                to,
                checkpoint,
                carried,
                lost,
            } => Some((from, to, checkpoint, carried, lost)),
            _ => None,
        })
        .expect("the handoff is an event");
    assert_eq!(handoff.0, engine);
    assert_eq!(handoff.1, api);
    assert_eq!(handoff.2, Some(checkpoint.id));
    assert!(
        handoff.3 >= 4,
        "four messages were folded, got {}",
        handoff.3
    );
    assert!(
        handoff.4.is_empty(),
        "nothing was lost here, so nothing may be claimed: {:?}",
        handoff.4
    );

    // The turn after the handoff runs on the checkpoint alone: none of the
    // pre-handoff turns are left in the raw tail.
    let turns = h.store.invocations_for_agent(agent.id).expect("inv");
    let last = h
        .store
        .snapshot(turns.last().expect("turn").snapshot_id)
        .expect("snap")
        .expect("exists");
    assert_eq!(last.model, api);
    assert!(
        !last
            .messages
            .iter()
            .any(|m| m.text().contains("My dog is called Argos")),
        "the folded turns must not also be in the tail"
    );
    assert!(
        last.system_text().contains("Argos"),
        "the checkpoint carries what the old model knew"
    );
}
