//! Explicit checkpoint lifecycle and accounting.
use super::*;

#[tokio::test]
async fn engine_compaction_replaces_session_only_after_valid_checkpoint() {
    for valid in [true, false] {
        let turns = Arc::new(Mutex::new(Vec::new()));
        let engine = Arc::new(CheckpointEngine {
            turns: turns.clone(),
            valid,
        });
        let h = harness_with_engine(engine, Arc::default(), Arc::default()).await;
        let id = chat_agent(&h);
        wait_for(h.store.as_ref(), id, |a| {
            a.status.kind() == AgentStatusKind::Waiting
        })
        .await;
        h.runtime
            .send_message(
                id,
                vec![ContentBlock::text("remember saffron")],
                Actor::user(),
            )
            .unwrap();
        let before = wait_for(h.store.as_ref(), id, |a| {
            a.stats.turns >= 1 && a.status.kind() == AgentStatusKind::Waiting
        })
        .await;
        assert!(before.engine_session.is_some());
        h.runtime.request_compaction(id, Actor::user()).unwrap();
        let after = wait_for(h.store.as_ref(), id, |a| {
            a.stats.turns > before.stats.turns && a.status.kind() == AgentStatusKind::Waiting
        })
        .await;
        let recorded = turns.lock().unwrap().clone();
        for turn in &recorded {
            assert!(
                turn.tool_hidden_env
                    .contains(&"ENGINE_TEST_PROVIDER_KEY".to_owned())
            );
            assert!(
                turn.tool_hidden_env
                    .contains(&"ENGINE_TEST_DAEMON_SECRET".to_owned())
            );
            assert!(
                turn.hidden_env
                    .contains(&"ENGINE_TEST_DAEMON_SECRET".to_owned())
            );
            assert!(
                !turn
                    .hidden_env
                    .contains(&"ENGINE_TEST_PROVIDER_KEY".to_owned())
            );
        }
        let summaries: Vec<_> = recorded.iter().filter(|t| t.summary_only).collect();
        assert!(!summaries.is_empty());
        let normal_turns = recorded.len() - summaries.len();
        assert_eq!(
            normal_turns, 1,
            "manual compaction must not invoke a reply turn"
        );
        assert_eq!(
            after.stats.usage.input_tokens,
            40 * summaries.len() as u64 + 10 * normal_turns as u64
        );
        assert_eq!(
            after.stats.usage.output_tokens,
            7 * summaries.len() as u64 + 3 * normal_turns as u64
        );
        assert_eq!(
            after.stats.usage.cost_usd,
            1.5 * summaries.len() as f64 + 0.25 * normal_turns as f64
        );
        assert!(after.stats.usage.cost_complete);
        let events = h
            .store
            .events(0, 1000, &EventFilter::for_agent(id))
            .unwrap();
        let (total, turn_usage) = events
            .iter()
            .rev()
            .find_map(|event| match event.kind {
                EventKind::AgentUsageUpdated { total, turn_usage } => Some((total, turn_usage)),
                _ => None,
            })
            .expect("final billing update");
        assert_eq!(total, after.stats.usage);
        let frames = aether_core::replay::build(id, &events);
        let mut replayed = aether_core::model::Usage::default();
        for frame in &frames {
            replayed += frame.usage;
        }
        assert_eq!(
            replayed, after.stats.usage,
            "replay includes compaction exactly once"
        );
        assert_eq!(frames.last().unwrap().usage, turn_usage);
        assert_eq!(
            turn_usage.cost_usd,
            1.5 * summaries.len() as f64,
            "manual compaction bills only the compactor"
        );
        let report = aether_core::report::build(after.run, &events, aether_core::time::now());
        assert_eq!(
            report
                .body
                .agents
                .iter()
                .find(|agent| agent.id == id)
                .unwrap()
                .usage,
            after.stats.usage
        );
        assert_eq!(summaries[0].model.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(
            summaries[0].effort,
            Some(aether_core::model::Effort::Medium)
        );
        assert!(
            summaries
                .iter()
                .all(|t| t.session.is_none() && t.mcp.is_none() && !t.bypass_permissions)
        );
        let last = recorded.last().unwrap();
        assert!(last.summary_only);
        if valid {
            assert!(after.checkpoint.is_some());
            assert!(after.engine_session.is_none());
        } else {
            assert_eq!(after.engine_session, before.engine_session);
            assert!(after.checkpoint.is_none());
            assert!(
                h.store
                    .events(0, 1000, &EventFilter::for_agent(id))
                    .unwrap()
                    .iter()
                    .any(|e| matches!(e.kind, EventKind::CompactionFailed { .. }))
            );
        }
        if valid {
            h.runtime
                .send_message(
                    id,
                    vec![ContentBlock::text("recall the word")],
                    Actor::user(),
                )
                .unwrap();
            wait_for(h.store.as_ref(), id, |a| {
                a.stats.turns > after.stats.turns && a.status.kind() == AgentStatusKind::Waiting
            })
            .await;
            let resumed = turns.lock().unwrap().last().unwrap().clone();
            assert!(resumed.session.is_none());
            assert!(
                resumed.prompt.contains("saffron"),
                "the first requested reply after compaction must carry the checkpoint into a fresh session"
            );
        }
        assert!(
            h.store
                .transcript(id, 0)
                .unwrap()
                .iter()
                .any(|e| e.message.text().contains("saffron"))
        );
    }
}

#[tokio::test]
async fn compaction_spend_can_exhaust_the_owning_agents_budget() {
    let engine = Arc::new(CheckpointEngine {
        turns: Arc::default(),
        valid: true,
    });
    let h = harness_with_engine(engine, Arc::default(), Arc::default()).await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    cost_budget: Some(1.0),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .unwrap();
    let id = agent.id;
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    h.runtime
        .send_message(
            id,
            vec![ContentBlock::text("remember saffron")],
            Actor::user(),
        )
        .unwrap();
    let before = wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns == 1 && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    assert_eq!(before.stats.usage.cost_usd, 0.25);
    h.runtime.request_compaction(id, Actor::user()).unwrap();
    let stopped = wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Failed
    })
    .await;
    assert!(
        stopped.stats.usage.cost_usd >= 1.75,
        "checkpoint spend is charged before enforcing the budget"
    );
    let events = h
        .store
        .events(0, 1000, &EventFilter::for_agent(id))
        .unwrap();
    let usage = events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::BudgetExceeded { usage, .. } => Some(usage),
            _ => None,
        })
        .expect("budget enforcement event");
    assert!(usage.cost_usd >= 1.75);
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn empty_engine_compaction_settles_the_request_visibly() {
    let h = harness(Reply::OneTurn).await;
    let id = chat_agent(&h);
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    h.runtime.request_compaction(id, Actor::user()).unwrap();
    wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns > 0 && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    let events = h
        .store
        .events(0, 1000, &EventFilter::for_agent(id))
        .unwrap();
    assert!(events.iter().any(|e| matches!(&e.kind, EventKind::CompactionFailed { error, models } if error.contains("Nothing to compact") && models.is_empty())));
    assert!(h.store.latest_checkpoint(id).unwrap().is_none());
}
