use super::*;

#[tokio::test]
async fn an_unknown_wait_task_names_the_field_and_returns_owned_task_ids() {
    let mut h = harness(10).await;
    let coordinator = start_coordinator(&h, None);
    let bridge = h.ready.recv().await.expect("coordinator started");
    let (child, reply) = delegate(&h, &bridge, "read docs").await;
    let task = h
        .store
        .tasks(&aether_store::TaskFilter {
            owner: Some(child),
            ..Default::default()
        })
        .unwrap()
        .remove(0);
    assert!(reply.contains("wait_for_args"), "{reply}");
    let fields: serde_json::Value = serde_json::from_str(reply.lines().last().unwrap()).unwrap();
    assert_eq!(fields["task_id"], json!(task.id));
    assert_eq!(fields["wait_for_args"], json!({"tasks": [task.id]}));
    let foreign = start_coordinator(&h, None);
    let foreign_task = h
        .store
        .tasks(&aether_store::TaskFilter {
            owner: Some(foreign),
            ..Default::default()
        })
        .unwrap()
        .remove(0);
    let wrong = child.to_string().replacen("agt_", "tsk_", 1);
    let (reply, is_error) = h
        .runtime
        .engine_tool(
            coordinator,
            &bridge.token,
            "wait_for".into(),
            json!({"tasks": [wrong]}),
        )
        .await
        .unwrap();
    assert!(is_error, "{reply}");
    assert!(reply.contains("tasks") && reply.contains(&wrong), "{reply}");
    assert!(reply.contains(&task.id.to_string()), "{reply}");
    assert!(!reply.contains(&foreign_task.id.to_string()), "{reply}");
    assert!(
        reply.contains("never change an agent ID's prefix"),
        "{reply}"
    );
    h.runtime.shutdown().await;
}

/// The subtree cap counts only descendants, never the
/// coordinator itself, and a delegate call past the cap says so - the cap,
/// the live count, and that the child queues.
#[tokio::test]
async fn a_delegate_past_the_cap_names_it_and_the_coordinator_is_never_counted() {
    let mut h = harness(10).await;
    let coordinator = start_coordinator(&h, Some(2));
    let bridge = h.ready.recv().await.expect("coordinator turn started");
    assert_eq!(bridge.agent, coordinator);

    // First child: nothing running yet under the cap, so no "queues" note.
    let (child1, reply1) = delegate(&h, &bridge, "one").await;
    assert!(
        !reply1.contains("queues"),
        "first child should not report queueing: {reply1}"
    );
    let b1 = h.ready.recv().await.expect("child 1 turn started");
    assert_eq!(
        b1.agent, child1,
        "the coordinator's own turn is not counted"
    );
    wait_until(h.store.as_ref(), child1, |s| {
        s.kind() == AgentStatusKind::Running
    })
    .await;

    // Second child: one sibling running, cap is 2, still room.
    let (child2, reply2) = delegate(&h, &bridge, "two").await;
    assert!(
        !reply2.contains("queues"),
        "second child should still fit under the cap: {reply2}"
    );
    let b2 = h.ready.recv().await.expect("child 2 turn started");
    assert_eq!(b2.agent, child2);
    wait_until(h.store.as_ref(), child2, |s| {
        s.kind() == AgentStatusKind::Running
    })
    .await;

    // Third child: both siblings now running, the cap of 2 is spent - the
    // delegate call still creates it (VISION §3.6: "the rest queue", never
    // refused), but says so, naming the cap and the count.
    let (child3, reply3) = delegate(&h, &bridge, "three").await;
    assert!(
        reply3.contains("2/2") && reply3.contains("queues"),
        "expected the cap and count in the queueing note: {reply3}"
    );
    assert_eq!(
        children_of(h.store.as_ref(), coordinator).len(),
        3,
        "the cap queues the third child, it does not refuse it"
    );
    let status3 = h.store.agent(child3).expect("agent").expect("exists");
    assert_eq!(
        status3.status.kind(),
        AgentStatusKind::Ready,
        "queued behind the cap: created, not yet dispatched"
    );
}

/// An agent nobody delegated has no parent.
#[tokio::test]
async fn a_root_coordinator_knows_it_has_no_parent() {
    let h = harness(10).await;
    let coordinator = start_coordinator(&h, None);
    let agent = h.store.agent(coordinator).expect("agent").expect("exists");
    assert_eq!(agent.parent, None);
}

/// An interrupt never cancels, pauses or orphans
/// children a turn already created - only cancel ends a subtree - and each
/// child still knows its own parent.
#[tokio::test]
async fn an_interrupt_never_orphans_children_the_turn_already_created() {
    let mut h = harness(10).await;
    let coordinator = start_coordinator(&h, None);
    let bridge = h.ready.recv().await.expect("coordinator turn started");

    let (child1, _) = delegate(&h, &bridge, "one").await;
    let (child2, _) = delegate(&h, &bridge, "two").await;

    let interrupted = h
        .runtime
        .interrupt_agent(coordinator, Actor::user())
        .expect("interrupt");
    assert!(interrupted, "a turn was running to interrupt");

    // The coordinator's own turn ends (its engine "process" closes the
    // stream once the token it was handed is cancelled) and it goes back to
    // waiting on its inbox - never Cancelled or Failed by a mere interrupt.
    let coordinator_status = wait_until(h.store.as_ref(), coordinator, |s| {
        matches!(s, AgentStatus::Waiting { .. })
    })
    .await;
    assert!(
        matches!(coordinator_status, AgentStatus::Waiting { .. }),
        "{coordinator_status:?}"
    );

    for (child, label) in [(child1, "child1"), (child2, "child2")] {
        let agent = h.store.agent(child).expect("agent").expect("exists");
        assert_eq!(agent.parent, Some(coordinator), "{label} lost its parent");
        assert!(
            !matches!(
                agent.status,
                AgentStatus::Cancelled { .. } | AgentStatus::Failed { .. }
            ),
            "{label} was orphaned by the interrupt: {:?}",
            agent.status
        );
    }
}

/// A harness-created child is visible - in the tree, with the
/// right parent, model and role - the moment its `agent.created` commits,
/// with no wait for the turn to end; the tool call that made it also
/// returns its id.
#[tokio::test]
async fn a_delegated_child_is_visible_the_instant_it_commits() {
    let mut h = harness(10).await;
    let coordinator = start_coordinator(&h, None);
    let bridge = h.ready.recv().await.expect("coordinator turn started");

    let (child, reply) = delegate(&h, &bridge, "be visible").await;
    // `delegate` diffed the store right after the bridge call returned, so
    // reaching this line already proves the child committed before the
    // coordinator's turn (still open on `bridge`) had any chance to end.
    assert!(reply.contains(&child.to_string()), "{reply}");
    let agent = h.store.agent(child).expect("agent").expect("exists");
    assert_eq!(agent.parent, Some(coordinator));
    assert_eq!(agent.role.as_str(), "worker");
    assert_eq!(agent.model, ModelRef::new("claude-code", "haiku"));
}

/// The cap is a WHOLE-SUBTREE cap
/// (VISION §3.6 limit 3), so a grandchild filling it must not be invisible
/// to the advisory just because it counted only direct children.
#[tokio::test]
async fn a_delegate_reply_counts_the_whole_subtree_not_just_direct_children() {
    let mut h = harness(10).await;
    let _root = start_coordinator(&h, Some(2));
    let root_bridge = h.ready.recv().await.expect("root turn started");

    let (child1, _) = delegate(&h, &root_bridge, "child one").await;
    let child1_bridge = h.ready.recv().await.expect("child1 turn started");
    wait_until(h.store.as_ref(), child1, |s| {
        s.kind() == AgentStatusKind::Running
    })
    .await;

    // A grandchild is still in root's subtree - the cap counts descendants
    // at any depth, never only the direct children.
    let (grandchild1, _) = delegate(&h, &child1_bridge, "grandchild one").await;
    h.ready.recv().await.expect("grandchild1 turn started");
    wait_until(h.store.as_ref(), grandchild1, |s| {
        s.kind() == AgentStatusKind::Running
    })
    .await;

    // Root's cap of 2 is now spent by child1 + grandchild1, even though
    // root has only one direct child running: the third delegate must say
    // so, naming descendants rather than children.
    let (_, reply2) = delegate(&h, &root_bridge, "child two").await;
    assert!(
        reply2.contains("2/2") && reply2.contains("queues") && reply2.contains("descendants"),
        "a grandchild filling the cap must not be invisible to the advisory: {reply2}"
    );
}

/// Daemon shutdown must never cascade the
/// delegate-race re-walk (`turn::finish`) into an idle subtree. Nor does it
/// end the turns it stopped: those are left as a crash leaves them, and the
/// next start recovers them alongside the child it merely left queued.
#[tokio::test]
async fn shutdown_ends_no_turn_it_stopped_and_never_a_queued_child() {
    let mut h = harness(10).await;
    let root = start_coordinator(&h, Some(1));
    let root_bridge = h.ready.recv().await.expect("root turn started");

    // Under a cap of 1: the first child is dispatched (mid-turn, held open
    // by the engine); the second is queued behind the cap and never
    // dispatched - the idle child shutdown must leave alone.
    let (running_child, _) = delegate(&h, &root_bridge, "runs").await;
    h.ready.recv().await.expect("running child turn started");
    wait_until(h.store.as_ref(), running_child, |s| {
        s.kind() == AgentStatusKind::Running
    })
    .await;
    let (idle_child, reply) = delegate(&h, &root_bridge, "queues").await;
    assert!(reply.contains("queues"), "{reply}");
    assert_eq!(
        h.store
            .agent(idle_child)
            .expect("agent")
            .expect("exists")
            .status
            .kind(),
        AgentStatusKind::Ready,
        "queued behind the cap, never dispatched"
    );

    h.runtime.shutdown().await;

    // The turns shutdown stopped (root and the child it had dispatched) stay
    // Running under their leases, for the next start to recover...
    for (id, label) in [(root, "root"), (running_child, "running_child")] {
        let status = h.store.agent(id).expect("agent").expect("exists").status;
        assert_eq!(
            status.kind(),
            AgentStatusKind::Running,
            "{label} was mid-turn when shutdown ran: {status:?}"
        );
    }
    // ...and the idle child, never dispatched, must not be swept along by
    // root's stopped-turn re-walk.
    let idle_status = h
        .store
        .agent(idle_child)
        .expect("agent")
        .expect("exists")
        .status;
    assert_eq!(
        idle_status.kind(),
        AgentStatusKind::Ready,
        "shutdown cascaded the cancel into an idle child: {idle_status:?}"
    );

    // A restart over the same store ends none of them, and retries the two
    // stopped turns.
    let h2 = second_daemon_over(&h, 10).await;
    for (id, label) in [
        (root, "root"),
        (running_child, "running_child"),
        (idle_child, "idle_child"),
    ] {
        let status = h2.store.agent(id).expect("agent").expect("exists").status;
        assert!(
            !status.is_terminal(),
            "a restart must not find {label} ended: {status:?}"
        );
    }
    for id in [root, running_child] {
        let classified = h2
            .store
            .events(0, 10_000, &aether_store::EventFilter::for_agent(id))
            .expect("events")
            .into_iter()
            .find_map(|e| match e.kind {
                aether_core::event::EventKind::RecoveryClassified { class, .. } => Some(class),
                _ => None,
            });
        assert_eq!(
            classified,
            Some(aether_core::event::RecoveryClass::Retry),
            "{id}"
        );
    }
}
