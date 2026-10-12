//! recovery limits scenarios and fixtures.
use super::*;

/// The budget pass skips a fleet that has not changed, so it must not miss a
/// budget set on an agent that is already over it: the change is a commit, and
/// a commit is what un-skips the pass.
#[tokio::test]
async fn a_budget_set_after_the_spend_still_stops_the_agent() {
    let h = harness(vec![]).await;
    let mut s = spec("supervise");
    s.objective = None;
    s.config.as_mut().expect("config").long_lived = true;
    let (agent, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    // Spend recorded, then several ticks with nothing else happening: the pass
    // has settled and is skipping.
    let mut spent = h.store.agent(agent.id).expect("a").expect("e");
    spent.stats.usage.cost_usd = 2.0;
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(spent)))
        .expect("commit the spend");
    for _ in 0..5 {
        aether_runtime::scheduler::tick(&h.runtime).expect("tick");
    }
    assert!(
        !h.store
            .agent(agent.id)
            .expect("a")
            .expect("e")
            .status
            .is_terminal()
    );

    h.runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                cost_budget: Some(1.0),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("set a budget below what it already spent");
    let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Failed, "{status:?}");
}

/// A long turn is still a turn: a mutating call that was in flight when the
/// daemon died must be found however many lease renewals the heartbeat wrote
/// on top of it. Classifying from a fixed window of recent events loses the
/// turn start and reads the interrupted call as an idle agent to retry.
#[tokio::test]
async fn a_long_turn_is_classified_from_its_start_not_a_window_of_recent_events() {
    let id = AgentId::new();
    let key = aether_core::tool::ResourceKey::repo("/repo/aether");
    let h = harness_with(vec![], |store| {
        seed_crashed_turn(store, id, Some("bash"), vec![key.clone()], true);
        // The heartbeat renews the lease every half-TTL for as long as the
        // call runs; a turn held up for hours buries its own start.
        let mut commit = Commit::new();
        for _ in 0..250 {
            commit.push_event(
                EventDraft::new(
                    Actor::System,
                    EventKind::LeaseRenewed {
                        expires_at: now() + chrono::Duration::seconds(120),
                    },
                )
                .agent(id),
            );
        }
        store.commit(commit).expect("seed lease renewals");
    })
    .await;

    assert_eq!(
        recovery_class(h.store.as_ref(), id),
        Some(aether_core::event::RecoveryClass::Inspect),
        "a mutating call in flight is inspected, not retried: {:?}",
        types(h.store.as_ref(), id)
    );
    let agent = h.store.agent(id).expect("a").expect("exists");
    assert!(
        matches!(
            agent.status,
            AgentStatus::Blocked {
                reason: BlockReason::Inspection { .. }
            }
        ),
        "{:?}",
        agent.status
    );
    let held = h.runtime.resource_holdings();
    assert!(
        held.iter().any(|h| h.agent == id && h.keys.contains(&key)),
        "the buried call's keys are still fenced off: {held:?}"
    );
    let t = types(h.store.as_ref(), id);
    assert!(
        t.contains(&"tool.cancelled".to_owned()),
        "the dangling call is closed: {t:?}"
    );
    assert!(
        t.contains(&"invocation.failed".to_owned()),
        "the open invocation is closed: {t:?}"
    );
}

/// The inspection hold is not a property of the daemon that took it. A crash
/// loop or an upgrade on the same host boot leaves the keys fenced off: the lease
/// table lives in memory and starts empty, so every restart re-takes them
/// from the log until a person clears the inspection.
#[tokio::test]
async fn an_inspected_agent_keeps_its_keys_across_repeated_restarts() {
    let id = AgentId::new();
    let key = aether_core::tool::ResourceKey::repo("/repo/aether");
    let h = harness_with(vec![], |store| {
        seed_crashed_turn(store, id, Some("bash"), vec![key.clone()], true);
    })
    .await;
    let held = |h: &Harness| h.runtime.resource_holdings();
    assert!(held(&h).iter().any(|x| x.agent == id));

    let h2 = restart_over(&h, vec![]).await;
    assert!(
        held(&h2)
            .iter()
            .any(|x| x.agent == id && x.keys.contains(&key)),
        "the keys are still held after one restart: {:?}",
        held(&h2)
    );
    let h3 = restart_over(&h2, vec![]).await;
    let after_two = held(&h3);
    assert!(
        after_two
            .iter()
            .any(|x| x.agent == id && x.keys.contains(&key)),
        "and after a second: {after_two:?}"
    );
    assert_eq!(
        after_two.iter().filter(|x| x.agent == id).count(),
        1,
        "re-taking the hold does not stack up: {after_two:?}"
    );
    assert_eq!(
        after_two.iter().find(|x| x.agent == id).expect("held").keys,
        vec![key.clone()],
        "the key recovery itself recorded is the same key, counted once"
    );
    // Nothing was re-decided: the agent was classified once, on the crash.
    let t = types(h3.store.as_ref(), id);
    assert_eq!(
        t.iter().filter(|k| *k == "recovery.classified").count(),
        1,
        "{t:?}"
    );
    assert_eq!(
        t.iter().filter(|k| *k == "agent.blocked").count(),
        1,
        "{t:?}"
    );

    h3.runtime
        .resume_agent(id, false, Actor::user())
        .expect("a person clears the inspection");
    assert!(h3.runtime.resource_holdings().is_empty());
    let h4 = restart_over(&h3, vec![]).await;
    assert!(
        h4.runtime.resource_holdings().is_empty(),
        "keys a person handed back are not re-taken on the next restart"
    );
}

/// A deadline that catches a parent and a child in the same pass. The pass
/// loops over a snapshot its own cascade invalidates, so it must re-read each
/// row: acting on the stale child would overwrite the Cancelled the cascade
/// already committed with a Failed of its own, and leave two terminal events
/// on one agent.
#[tokio::test]
async fn a_deadline_on_both_a_parent_and_a_child_ends_each_of_them_once() {
    let h = harness(vec![]).await;
    let mut s = spec("take your time");
    s.objective = None;
    s.config.as_mut().expect("config").long_lived = true;
    let (parent, _) = h
        .runtime
        .create_agent(s.clone(), Actor::user())
        .expect("parent");
    let mut child_spec = s;
    child_spec.parent = Some(parent.id);
    let (child, _) = h
        .runtime
        .create_agent(child_spec, Actor::user())
        .expect("child");
    for id in [parent.id, child.id] {
        h.wait_until(id, |s| s.kind() == AgentStatusKind::Waiting)
            .await;
    }

    // Both deadlines land in one commit, so one pass sees both rows live and
    // the parent's cascade runs inside it.
    let passed = now() - chrono::Duration::seconds(1);
    let mut commit = Commit::new();
    for id in [parent.id, child.id] {
        let mut agent = h.store.agent(id).expect("a").expect("e");
        agent.config.deadline = Some(passed);
        commit.push_mutation(Mutation::UpsertAgent(agent));
    }
    h.store.commit(commit).expect("set both deadlines");

    let child_status = h.wait_until(child.id, |s| s.is_terminal()).await;
    h.wait_until(parent.id, |s| s.is_terminal()).await;
    assert_eq!(
        child_status.kind(),
        AgentStatusKind::Cancelled,
        "the parent's cascade ended the child; its own deadline must not re-decide that"
    );
    // Several more ticks: a stale snapshot would still be acted on later.
    for _ in 0..3 {
        aether_runtime::scheduler::tick(&h.runtime).expect("tick");
    }
    // Strictly this agent's own rows: the filter also matches a child's.
    let terminal = |id| {
        h.store
            .events(0, 10_000, &EventFilter::for_agent(id))
            .expect("events")
            .into_iter()
            .filter(|e| e.agent_id == Some(id))
            .map(|e| e.type_name())
            .filter(|k| matches!(k.as_str(), "agent.failed" | "agent.cancelled"))
            .collect::<Vec<_>>()
    };
    assert_eq!(terminal(child.id), vec!["agent.cancelled".to_owned()]);
    assert_eq!(terminal(parent.id), vec!["agent.failed".to_owned()]);
    assert_eq!(
        h.store
            .agent(child.id)
            .expect("a")
            .expect("e")
            .status
            .kind(),
        AgentStatusKind::Cancelled,
        "and the stored row still says so"
    );
}
