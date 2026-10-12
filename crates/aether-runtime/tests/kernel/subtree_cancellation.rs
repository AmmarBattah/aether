//! subtree cancellation scenarios and fixtures.
use super::*;

#[tokio::test]
async fn cancellation_intent_survives_failed_walks_and_restart() {
    for hook in [Hook::CancelDescendantRead, Hook::CancelMutationFailure] {
        let inner: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
        let store = Arc::new(CommitsMidPass::new(inner, hook));
        let h = harness_full(
            vec![],
            |_| {},
            |_| {},
            None,
            None,
            Some(store.clone()),
            |_| {},
        )
        .await;
        h.runtime.shutdown().await;
        let mut parent = None;
        let mut ids = Vec::new();
        for name in ["root", "middle", "leaf"] {
            let mut requested = spec(name);
            requested.start = false;
            requested.parent = parent;
            let (agent, _) = h
                .runtime
                .create_agent(requested, Actor::user())
                .expect("create");
            parent = Some(agent.id);
            ids.push(agent.id);
        }
        let error = h
            .runtime
            .cancel_agent(ids[0], Some("stop this tree".into()), Actor::user())
            .expect_err("injected error");
        assert!(
            error.to_string().contains("simulated cancellation"),
            "{error}"
        );
        let journal = store
            .meta("runtime.pending_cancellations")
            .unwrap()
            .unwrap();
        assert!(
            journal.contains(&ids[0].to_string()),
            "intent must survive failure"
        );
        assert!(store.read_failed());
        let mut late_child = spec("late child");
        late_child.start = false;
        late_child.parent = Some(ids[1]);
        let error = h
            .runtime
            .create_agent(late_child, Actor::user())
            .expect_err("pending cancellation rejects late descendants");
        assert!(error.to_string().contains("being cancelled"), "{error}");
        let restarted = harness_full(
            vec![],
            |_| {},
            |_| {},
            None,
            None,
            Some(store.clone()),
            |_| {},
        )
        .await;
        for id in ids {
            let status = restarted
                .wait_until(id, |status| status.is_terminal())
                .await;
            assert_eq!(status.kind(), AgentStatusKind::Cancelled);
            let events = restarted
                .store
                .events(0, 10_000, &EventFilter::for_agent(id))
                .unwrap();
            let cancellation = events
                .iter()
                .find(|event| matches!(event.kind, EventKind::AgentCancelled { .. }))
                .unwrap();
            assert_eq!(cancellation.actor, Actor::user());
        }
        aether_runtime::scheduler::tick(&restarted.runtime).expect("finish journal");
        assert_eq!(
            store
                .meta("runtime.pending_cancellations")
                .unwrap()
                .as_deref(),
            Some("{}")
        );
        restarted.runtime.shutdown().await;
    }
}

#[tokio::test]
async fn a_budget_stop_retries_cancellation_through_a_completed_middle() {
    let inner: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let store = Arc::new(CommitsMidPass::new(inner, Hook::CancelMutationFailure));
    let h = harness_full(
        vec![],
        |_| {},
        |_| {},
        None,
        None,
        Some(store.clone()),
        |_| {},
    )
    .await;
    h.runtime.shutdown().await;
    let mut tree = Vec::new();
    let mut parent = None;
    for name in ["root", "middle", "leaf"] {
        let mut requested = spec(name);
        requested.start = false;
        requested.parent = parent;
        let (agent, _) = h.runtime.create_agent(requested, Actor::user()).unwrap();
        parent = Some(agent.id);
        tree.push(agent);
    }
    tree[0].config.token_budget = Some(1);
    tree[0].stats.usage.input_tokens = 2;
    tree[1].status = AgentStatus::Completed { summary: None };
    store
        .commit(
            Commit::new()
                .mutate(Mutation::UpsertAgent(tree[0].clone()))
                .mutate(Mutation::UpsertAgent(tree[1].clone())),
        )
        .unwrap();
    let error =
        aether_runtime::scheduler::tick(&h.runtime).expect_err("child cancellation fails once");
    assert!(
        error.to_string().contains("simulated cancellation"),
        "{error}"
    );
    assert_eq!(
        store.agent(tree[0].id).unwrap().unwrap().status.kind(),
        AgentStatusKind::Failed
    );
    assert!(
        store
            .meta("runtime.pending_cancellations")
            .unwrap()
            .unwrap()
            .contains(&tree[0].id.to_string())
    );
    aether_runtime::scheduler::tick(&h.runtime).expect("retry cancellation");
    assert_eq!(
        store.agent(tree[1].id).unwrap().unwrap().status.kind(),
        AgentStatusKind::Completed
    );
    assert_eq!(
        store.agent(tree[2].id).unwrap().unwrap().status.kind(),
        AgentStatusKind::Cancelled
    );
    assert_eq!(
        store
            .meta("runtime.pending_cancellations")
            .unwrap()
            .as_deref(),
        Some("{}")
    );
}

#[tokio::test]
async fn a_harness_cancel_is_confined_to_descendants_and_preserves_the_caller() {
    let h = harness(vec![]).await;
    let mut root_spec = spec("supervise");
    root_spec.start = false;
    let (ancestor, _) = h
        .runtime
        .create_agent(root_spec.clone(), Actor::user())
        .unwrap();
    root_spec.parent = Some(ancestor.id);
    let (root, _) = h
        .runtime
        .create_agent(root_spec.clone(), Actor::user())
        .unwrap();
    root_spec.parent = None;
    let (unrelated, _) = h
        .runtime
        .create_agent(root_spec.clone(), Actor::user())
        .unwrap();
    root_spec.parent = Some(root.id);
    let (mut middle, _) = h
        .runtime
        .create_agent(root_spec.clone(), Actor::user())
        .unwrap();
    root_spec.parent = Some(middle.id);
    let (leaf, _) = h.runtime.create_agent(root_spec, Actor::user()).unwrap();
    middle.status = AgentStatus::Completed { summary: None };
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(middle.clone())))
        .unwrap();
    for target in [root.id, ancestor.id, unrelated.id, middle.id] {
        h.client.push(ScriptedClient::tool_call(
            "cancel_agent",
            json!({"agent":target,"reason":"review no longer needed"}),
        ));
    }
    h.client.push(ScriptedClient::tool_call(
        "complete_task",
        json!({"summary":"supervision finished"}),
    ));
    h.runtime
        .send_message(
            root.id,
            vec![ContentBlock::text("stop the obsolete work")],
            Actor::user(),
        )
        .unwrap();
    let mut ready = root.clone();
    ready.transition(AgentStatus::Ready, now()).unwrap();
    h.store
        .commit(
            Commit::new().mutate(Mutation::UpsertAgent(ready)).event(
                EventDraft::new(Actor::user(), EventKind::AgentReady)
                    .agent(root.id)
                    .run(root.run),
            ),
        )
        .unwrap();
    let status = h.wait_until(root.id, AgentStatus::is_terminal).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    assert_eq!(
        h.store.agent(leaf.id).unwrap().unwrap().status.kind(),
        AgentStatusKind::Cancelled
    );
    assert_eq!(
        h.store.agent(unrelated.id).unwrap().unwrap().status.kind(),
        unrelated.status.kind()
    );
    assert_eq!(
        h.store.agent(middle.id).unwrap().unwrap().status.kind(),
        AgentStatusKind::Completed
    );
    assert_eq!(
        h.store.agent(ancestor.id).unwrap().unwrap().status,
        ancestor.status
    );
    let events = h.store.events(0, 10000, &EventFilter::default()).unwrap();
    let requested: Vec<_> = events.iter().filter(|event| matches!(&event.kind, EventKind::Custom {name,..} if name == "cancellation.requested")).collect();
    assert_eq!(
        requested.len(),
        1,
        "denied calls must not write cancellation intent"
    );
    assert_eq!(requested[0].agent_id, Some(middle.id));
    assert_eq!(requested[0].actor, Actor::agent(root.id));
    let stopped = events
        .iter()
        .find(|event| {
            event.agent_id == Some(leaf.id)
                && matches!(event.kind, EventKind::AgentCancelled { .. })
        })
        .unwrap();
    assert_eq!(stopped.actor, Actor::agent(root.id));
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn fan_out_reaches_live_grandchildren_below_completed_parents() {
    let h = harness(vec![]).await;
    let create = |parent| CreateAgentSpec {
        role: "fanout".into(),
        config: Some(AgentConfig {
            effort: Some(aether_core::model::Effort::Low),
            ..Default::default()
        }),
        parent,
        start: false,
        ..Default::default()
    };
    let (root, _) = h
        .runtime
        .create_agent(create(None), Actor::System)
        .expect("root");
    let (mut middle, _) = h
        .runtime
        .create_agent(create(Some(root.id)), Actor::System)
        .expect("middle");
    let (leaf, _) = h
        .runtime
        .create_agent(create(Some(middle.id)), Actor::System)
        .expect("leaf");
    middle.status = AgentStatus::Completed {
        summary: Some("delegated work".into()),
    };
    h.runtime
        .apply(Commit::new().mutate(Mutation::UpsertAgent(middle.clone())))
        .expect("complete middle");
    let deliveries = h
        .runtime
        .fan_out_message(
            root.id,
            vec![ContentBlock::text("updated instructions")],
            Actor::System,
        )
        .expect("fanout");
    let recipients: std::collections::BTreeSet<_> =
        deliveries.iter().map(|delivery| delivery.agent).collect();
    assert_eq!(
        recipients,
        std::collections::BTreeSet::from([root.id, leaf.id])
    );
    assert!(!recipients.contains(&middle.id));
    h.runtime.shutdown().await;
}
