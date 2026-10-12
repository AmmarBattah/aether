//! cancellation races scenarios and fixtures.
use super::*;

/// Subscribers see the log in order. Publishing after the commit gate is
/// released let a small commit overtake a large one's publication, and the
/// daemon's subscription, which drops what it has already passed, lost events.
#[tokio::test]
async fn concurrent_commits_reach_a_subscriber_in_log_order() {
    const LARGE: usize = 4_000;
    const SMALL_WRITERS: usize = 2;
    const SMALL_COMMITS: usize = 1_000;
    let h = harness(vec![]).await;
    let mut bus = h.runtime.subscribe();
    let marker = |pid: u32| {
        EventDraft::new(
            Actor::System,
            EventKind::DaemonStarted {
                version: "ordering".into(),
                pid,
            },
        )
    };

    use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
    let (large_started, large_done) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    );
    // Small commits queue on the gate behind the large one and race its publication.
    let small: Vec<_> = (1..=SMALL_WRITERS as u32)
        .map(|pid| {
            let (runtime, started, done) =
                (h.runtime.clone(), large_started.clone(), large_done.clone());
            std::thread::spawn(move || {
                while !started.load(SeqCst) {
                    std::thread::yield_now();
                }
                let mut seqs = Vec::new();
                while seqs.len() < SMALL_COMMITS && !done.load(SeqCst) {
                    let events = runtime
                        .apply(Commit::new().event(marker(pid)))
                        .expect("a small commit");
                    seqs.push(events[0].seq);
                }
                seqs
            })
        })
        .collect();
    let large = {
        let (runtime, started, done) =
            (h.runtime.clone(), large_started.clone(), large_done.clone());
        std::thread::spawn(move || {
            let mut commit = Commit::new();
            for _ in 0..LARGE {
                commit.push_event(marker(0));
            }
            started.store(true, SeqCst);
            let events = runtime.apply(commit).expect("the large commit");
            done.store(true, SeqCst);
            events.last().expect("the large commit's events").seq
        })
    };
    let large_last = large.join().expect("the large writer");
    let small_seqs: Vec<u64> = small
        .into_iter()
        .flat_map(|w| w.join().expect("a small writer"))
        .collect();
    // Sequenced after the large commit by a writer that saw it unfinished:
    // almost surely one that queued on the gate behind it.
    assert!(
        small_seqs.iter().any(|&seq| seq > large_last),
        "no small commit queued behind the large one: the race was not run"
    );
    let applied = LARGE + small_seqs.len();

    let mut seqs = Vec::with_capacity(applied);
    loop {
        match bus.try_recv() {
            Ok(aether_runtime::BusMessage::Event(e)) => seqs.push(e.seq),
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
            Err(other) => panic!("the subscriber lost events: {other}"),
        }
    }
    assert_eq!(seqs.len(), applied, "every committed event was published");
    let out_of_order = seqs.windows(2).find(|w| w[1] != w[0] + 1);
    assert_eq!(out_of_order, None, "events published out of seq order");
}

/// Cancelling a supervisor must end its whole subtree even when a
/// non-interruptible `delegate` call is still in flight. Creation and durable
/// cancellation intent serialize at the write gate; the created child must
/// be cancelled with the operator attribution whichever caller gets there first.
///
/// The child's first turn shares the same scripted client as the
/// coordinator. With only one response queued, an empty pop returns "no
/// responses left" (`ScriptedClient::stream`) unconditionally, without ever
/// consulting the cancellation token - so a live child can fail with a model
/// error instead of observing the cancel. Queuing a second response for the
/// child means its turn always gets a real stream to cancel out of.
///
/// That stream used to need an artificial per-event delay to
/// leave a real window for the cancel to land in (later widened to
/// ~350 ms). `hold_response_at` targets the child's turn by call order instead
/// (call #2, since a delegated child shares its parent's provider and so
/// cannot be targeted by `push_for`): if the child's turn does reach the
/// model, the gate blocks it until released or its own cancellation token
/// fires, with no sleep. Most runs cancel the child before its turn ever
/// dispatches to the model at all - a terminal-without-a-turn path this
/// test asserts against too - so the gate is a deterministic replacement
/// for the old delay either way, not a guarantee it is always consumed.
#[tokio::test]
async fn cancelling_a_supervisor_mid_delegate_leaves_no_live_child() {
    let inner: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let store = Arc::new(CommitsMidPass::new(inner, Hook::ChildCreatedCommit));
    let h = harness_full(
        vec![
            ScriptedClient::tool_call(
                "delegate",
                json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "do the sub task", "wait": false}),
            ),
            // The child's own first turn, held below until the cancel lands.
            ScriptedClient::text("sub task done"),
        ],
        |_| {},
        |_| {},
        None,
        None,
        Some(store.clone()),
        |_| {},
    )
    .await;
    // Armed before either turn runs: it targets the child's turn by call
    // order (call #2), not by racing a timer against the coordinator's.
    let _child_gate = h.client.hold_response_at(2);
    let (coordinator, _) = h
        .runtime
        .create_agent(spec("spawn a worker"), Actor::user())
        .expect("create");
    let id = coordinator.id;

    // The cancellation races the creation commit on another caller thread.
    // It must acquire the same write gate rather than reenter a held gate.
    let rt = h.runtime.clone();
    let caller = Arc::new(std::sync::Mutex::new(None));
    let caller_slot = caller.clone();
    store.arm(Box::new(move |_inner| {
        let started = Arc::new(std::sync::Barrier::new(2));
        let worker_started = started.clone();
        *caller_slot.lock().unwrap() = Some(std::thread::spawn(move || {
            worker_started.wait();
            rt.cancel_agent(id, Some("operator cancel".into()), Actor::user())
                .expect("cancel");
        }));
        started.wait();
        Ok(())
    }));

    let status = h.wait_until(id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Cancelled, "{status:?}");
    assert!(store.fired(), "the cancel landed mid-delegate as armed");
    caller
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .join()
        .expect("cancel caller");

    let children = h
        .store
        .agents(&AgentFilter {
            parent: Some(id),
            ..Default::default()
        })
        .expect("agents");
    assert!(
        !children.is_empty(),
        "the race must actually create a child for this test to prove anything"
    );
    for child in &children {
        let status = h.wait_until(child.id, |status| status.is_terminal()).await;
        assert_eq!(
            status.kind(),
            AgentStatusKind::Cancelled,
            "the child a cancelled supervisor delegated must not survive it: {child:?}"
        );
    }
    let live = h
        .store
        .agents(&AgentFilter {
            parent: Some(id),
            live_only: true,
            ..Default::default()
        })
        .expect("agents");
    assert!(
        live.is_empty(),
        "no live descendant may remain under a cancelled supervisor: {live:?}"
    );
    assert_eq!(
        status,
        AgentStatus::Cancelled {
            reason: Some("operator cancel".into())
        }
    );
    let events = h
        .store
        .events(0, 10_000, &EventFilter::for_run(coordinator.run))
        .expect("events");
    let cancelled: Vec<_> = events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::AgentCancelled { .. }))
        .collect();
    assert_eq!(cancelled.len(), children.len() + 1);
    for event in cancelled {
        assert_eq!(event.actor, Actor::user(), "cancel attribution: {event:?}");
    }
}

/// Cancelling a supervisor ends its whole
/// subtree at ANY depth, even past a child that already finished on its own
/// - `cancel_live_children` must not stop enumerating past a terminal
/// intermediate, or a live grandchild beneath it survives the cancel.
#[tokio::test]
async fn cancel_descends_past_a_terminal_intermediate_to_a_live_grandchild() {
    let h = harness(vec![]).await;
    let mut root_spec = spec("supervise");
    root_spec.start = false;
    let (root, _) = h
        .runtime
        .create_agent(root_spec, Actor::user())
        .expect("root");

    let mut child_spec = spec("finish quickly");
    child_spec.start = false;
    child_spec.parent = Some(root.id);
    let (child, _) = h
        .runtime
        .create_agent(child_spec, Actor::user())
        .expect("child");

    let mut grandchild_spec = spec("still running");
    grandchild_spec.start = false;
    grandchild_spec.parent = Some(child.id);
    let (grandchild, _) = h
        .runtime
        .create_agent(grandchild_spec, Actor::user())
        .expect("grandchild");

    // The child finished on its own, by hand - no turn ran, so nothing else
    // about the tree changes.
    let mut completed = h.store.agent(child.id).expect("a").expect("e");
    completed.status = AgentStatus::Completed {
        summary: Some("done".into()),
    };
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(completed)))
        .expect("commit");

    h.runtime
        .cancel_agent(root.id, Some("operator cancel".into()), Actor::user())
        .expect("cancel");

    let root_status = h.store.agent(root.id).expect("a").expect("e").status;
    assert_eq!(
        root_status.kind(),
        AgentStatusKind::Cancelled,
        "{root_status:?}"
    );
    let child_status = h.store.agent(child.id).expect("a").expect("e").status;
    assert_eq!(
        child_status.kind(),
        AgentStatusKind::Completed,
        "a cancel must not touch an already-terminal child: {child_status:?}"
    );
    let grandchild_status = h.store.agent(grandchild.id).expect("a").expect("e").status;
    assert_eq!(
        grandchild_status.kind(),
        AgentStatusKind::Cancelled,
        "a live grandchild past a finished intermediate must not survive the cancel: {grandchild_status:?}"
    );
}

/// A subtree pause holds what the subtree becomes, not only what it was: the
/// held supervisor's own in-flight turn can still delegate, and a child that
/// runs while its parent is held is not paused in any useful sense.
#[tokio::test]
async fn a_child_created_under_a_paused_parent_inherits_the_pause() {
    let h = harness(vec![ScriptedClient::text("should never be asked for")]).await;
    let (root, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("root");
    h.wait_until(root.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    h.runtime
        .pause_agent(root.id, true, Actor::user())
        .expect("pause subtree");

    let mut child_spec = idle_spec();
    child_spec.parent = Some(root.id);
    let (child, _) = h
        .runtime
        .create_agent(child_spec, Actor::user())
        .expect("child");
    assert!(
        paused(h.store.as_ref(), child.id),
        "a child of a held supervisor is held with it"
    );
    assert_eq!(
        h.store
            .agent(child.id)
            .expect("child")
            .expect("exists")
            .paused_via,
        Some(root.id),
        "the hold names the subtree pause it came from"
    );
    h.runtime
        .send_message(child.id, vec![ContentBlock::text("go")], Actor::user())
        .expect("message");
    for _ in 0..5 {
        aether_runtime::scheduler::tick(&h.runtime).expect("tick");
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(h.client.remaining(), 1, "it took no turn");

    // And the same subtree resume lifts it.
    let resumed = h
        .runtime
        .resume_agent(root.id, true, Actor::user())
        .expect("resume subtree");
    let ids: Vec<_> = resumed.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![root.id, child.id]);
    assert!(!paused(h.store.as_ref(), child.id));
}

/// Pausing what is already paused still has to say whether a turn is in
/// flight. Reporting only the agents the call moved answers "no turn" for an
/// agent that is mid-turn, which reads as the pause already having taken hold.
#[tokio::test]
async fn re_pausing_an_agent_that_is_mid_turn_still_reports_the_turn() {
    let h = harness(vec![
        ScriptedClient::tool_call("bash", json!({"command": "sleep 0.4"})),
        ScriptedClient::text("that is done"),
    ])
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("keep working"), Actor::user())
        .expect("agent");
    h.wait_until(agent.id, |s| matches!(s, AgentStatus::Running))
        .await;
    let first = h
        .runtime
        .pause_agent(agent.id, false, Actor::user())
        .expect("pause");
    assert_eq!(first.mid_turn, vec![agent.id]);

    let again = h
        .runtime
        .pause_agent(agent.id, false, Actor::user())
        .expect("pause again");
    assert!(again.changed.is_empty(), "nothing moved the second time");
    assert_eq!(
        again.mid_turn,
        vec![agent.id],
        "the turn is still running, whoever asks"
    );
}
