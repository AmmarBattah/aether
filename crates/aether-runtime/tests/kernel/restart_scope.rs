//! restart scope scenarios and fixtures.
use super::*;

/// A restart is the work being picked up again, so nothing a person said to
/// the old agent may be stranded on it, and the hard stop that ended it must
/// not end the successor before it takes a turn.
#[tokio::test]
async fn a_restart_carries_the_unread_inbox_and_drops_a_deadline_that_has_passed() {
    let h = harness(vec![]).await;
    let mut s = idle_spec();
    s.config.as_mut().expect("config").deadline = Some(now() + chrono::Duration::hours(1));
    let (old, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    h.wait_until(old.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    // A message lands, and the deadline passes before the agent reads it.
    h.runtime
        .send_message(
            old.id,
            vec![ContentBlock::text("stop after the second file")],
            Actor::user(),
        )
        .expect("message");
    let mut expired = h.store.agent(old.id).expect("a").expect("e");
    let passed = now() - chrono::Duration::seconds(1);
    expired.config.deadline = Some(passed);
    expired.inbox_cursor = 0;
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(expired)))
        .expect("deadline in the past");
    h.wait_until(old.id, |s| s.is_terminal()).await;
    let stopped = h.store.agent(old.id).expect("a").expect("e");
    assert!(
        h.store.inbox_last_seq(old.id).expect("seq") > stopped.inbox_cursor,
        "the message was never read"
    );

    let fresh = h
        .runtime
        .restart_agent(old.id, Actor::user())
        .expect("restart");
    assert_eq!(
        fresh.config.deadline, None,
        "a deadline that already fired is not inherited"
    );
    let carried: Vec<_> = h
        .store
        .inbox(fresh.id, 0)
        .expect("inbox")
        .into_iter()
        .map(|i| i.kind)
        .collect();
    assert_eq!(carried.len(), 1, "{carried:?}");
    assert!(
        matches!(&carried[0], aether_core::inbox::InboxKind::Message { content, .. }
            if content.iter().any(|c| matches!(c, ContentBlock::Text { text } if text.contains("second file")))),
        "the words a person sent reached the successor: {carried:?}"
    );
    // The restart says what it carried and what it let go of.
    let restarted = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(fresh.id))
        .expect("events")
        .into_iter()
        .find_map(|e| match e.kind {
            EventKind::AgentRestarted {
                inbox_carried,
                dropped_deadline,
                ..
            } => Some((inbox_carried, dropped_deadline)),
            _ => None,
        })
        .expect("agent.restarted");
    assert_eq!(restarted.0, 1);
    assert_eq!(restarted.1, Some(passed));

    // And the successor stays live rather than being stopped on the next tick.
    for _ in 0..3 {
        aether_runtime::scheduler::tick(&h.runtime).expect("tick");
    }
    assert!(
        !h.store
            .agent(fresh.id)
            .expect("a")
            .expect("e")
            .status
            .is_terminal()
    );
}

/// §3.1 wants every knob changeable on a live agent, and a deadline that can
/// only ever be pushed further out is not one of them.
#[tokio::test]
async fn a_deadline_can_be_cleared_on_a_live_agent() {
    let h = harness(vec![]).await;
    let mut s = idle_spec();
    s.config.as_mut().expect("config").deadline = Some(now() + chrono::Duration::hours(1));
    let (agent, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    let cleared = h
        .runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                deadline: Some(DeadlineUpdate::Clear),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("clear the deadline");
    assert_eq!(cleared.config.deadline, None);
    assert_eq!(
        h.store
            .agent(agent.id)
            .expect("a")
            .expect("e")
            .config
            .deadline,
        None
    );
}

/// Confining a live agent has to reach what it already delegated: a
/// supervisor moved into a container while its workers still run on the host
/// is not confined at all. Refuse, naming the descendant.
#[tokio::test]
async fn confining_a_live_agent_is_refused_while_a_descendant_is_wider() {
    let h = harness(vec![]).await;
    let (root, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("root");
    let mut child_spec = idle_spec();
    child_spec.parent = Some(root.id);
    let (child, _) = h
        .runtime
        .create_agent(child_spec, Actor::user())
        .expect("child");
    assert_eq!(
        child.scope.backend,
        aether_core::agent::ExecutionBackend::Host
    );

    let err = h
        .runtime
        .update_agent(
            root.id,
            aether_runtime::AgentUpdate {
                backend: Some(aether_core::agent::ExecutionBackend::Container {
                    image: "rust:1".into(),
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect_err("a host-scoped worker would be left outside the container");
    assert!(
        err.to_string().contains(&child.id.to_string()),
        "the refusal names the descendant: {err}"
    );
    assert_eq!(
        h.store.agent(root.id).expect("a").expect("e").scope.backend,
        aether_core::agent::ExecutionBackend::Host,
        "nothing was written"
    );

    // Tighten the worker first and the same change goes through.
    h.runtime
        .update_agent(
            child.id,
            aether_runtime::AgentUpdate {
                backend: Some(aether_core::agent::ExecutionBackend::Container {
                    image: "rust:1".into(),
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("confine the worker");
    let confined = h
        .runtime
        .update_agent(
            root.id,
            aether_runtime::AgentUpdate {
                backend: Some(aether_core::agent::ExecutionBackend::Container {
                    image: "rust:1".into(),
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("now the subtree is inside it");
    assert_ne!(
        confined.scope.backend,
        aether_core::agent::ExecutionBackend::Host
    );
}

/// §3.5: everybody hears a fanned-out message or nobody does. The batch is
/// one commit, so a single inbox seq claimed underneath it fails the whole
/// delivery and drops a person's words with an error. The seqs are therefore
/// read under the same hold on the write path that commits them.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_fan_out_survives_a_concurrent_writer_to_the_same_inboxes() {
    let h = harness(vec![]).await;
    let (root, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("root");
    let mut ids = vec![root.id];
    let mut parent = root.id;
    for _ in 0..4 {
        let mut child = idle_spec();
        child.parent = Some(parent);
        let (agent, _) = h.runtime.create_agent(child, Actor::user()).expect("child");
        ids.push(agent.id);
        parent = agent.id;
    }
    for id in &ids {
        h.wait_until(*id, |s| s.kind() == AgentStatusKind::Waiting)
            .await;
    }

    // Held, so the inboxes fill without anyone taking a turn on them: this
    // drill is about the writes racing, not about what the agents do next.
    h.runtime
        .pause_agent(root.id, true, Actor::user())
        .expect("pause the subtree");

    // One thread fans out, another writes to the same inboxes throughout.
    let noisy = {
        let runtime = h.runtime.clone();
        let targets = ids.clone();
        std::thread::spawn(move || {
            for round in 0..40 {
                for id in &targets {
                    runtime
                        .trigger_agent(
                            *id,
                            "watchdog".into(),
                            json!({ "round": round }),
                            Actor::System,
                        )
                        .expect("a concurrent inbox write must not fail either");
                }
            }
        })
    };
    let mut fanned = 0;
    for round in 0..40 {
        let deliveries = h
            .runtime
            .fan_out_message(
                root.id,
                vec![ContentBlock::text(format!("stand down ({round})"))],
                Actor::user(),
            )
            .expect("a person's message reaches the whole subtree");
        assert_eq!(deliveries.len(), ids.len(), "everybody, every time");
        fanned += 1;
    }
    noisy.join().expect("the concurrent writer finished");
    assert_eq!(fanned, 40);

    // Every delivery landed, and the per-agent sequences are gapless.
    for id in &ids {
        let items = h.store.inbox(*id, 0).expect("inbox");
        assert_eq!(items.len(), 80, "40 fanned out plus 40 from the other side");
        let seqs: Vec<u64> = items.iter().map(|i| i.seq).collect();
        assert_eq!(seqs, (1..=80).collect::<Vec<_>>(), "gapless, for {id}");
    }
}
