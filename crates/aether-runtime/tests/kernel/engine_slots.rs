//! engine slots scenarios and fixtures.
use super::*;

/// An unknown dependency is refused at creation rather than blocking forever.
#[tokio::test]
async fn a_task_cannot_depend_on_a_task_that_does_not_exist() {
    let h = harness(vec![]).await;
    let err = h
        .runtime
        .create_task(
            aether_runtime::CreateTaskSpec {
                objective: "impossible".into(),
                dependencies: vec![aether_core::id::TaskId::new()],
                ..Default::default()
            },
            Actor::user(),
        )
        .expect_err("unknown dependency");
    assert!(err.to_string().contains("dependency"), "{err}");
}

/// Limit 2: the global limit caps turns in flight.
#[tokio::test]
async fn the_global_limit_caps_concurrent_turns() {
    let h = harness_configured(
        vec![slow_turn("one"), slow_turn("two")],
        |_| {},
        |c| c.max_concurrent_turns = 1,
    )
    .await;
    h.client.set_delay(Duration::from_millis(40));
    let (a, _) = h
        .runtime
        .create_agent(spec("part one"), Actor::user())
        .expect("a");
    let (b, _) = h
        .runtime
        .create_agent(spec("part two"), Actor::user())
        .expect("b");
    h.wait_until(a.id, |s| s.is_terminal()).await;
    h.wait_until(b.id, |s| s.is_terminal()).await;
    assert_never_overlapped(
        &turn_intervals(h.store.as_ref(), a.id),
        &turn_intervals(h.store.as_ref(), b.id),
        "turns overlapped under a global limit of 1",
    );
    assert_eq!(h.client.remaining(), 0);
}

/// A run fanned out to 13 concurrent Claude Code sessions, each
/// burning at a normal single-agent rate. The engine cap bounds how many
/// engine turns run together, whatever the global limit allows.
#[tokio::test]
async fn the_engine_cap_runs_engine_turns_one_at_a_time() {
    let h = harness_configured(
        vec![slow_turn("one"), slow_turn("two")],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(1);
        },
    )
    .await;
    h.client.set_delay(Duration::from_millis(40));
    let (a, _) = h
        .runtime
        .create_agent(spec_on("part one", "claude-code", "haiku"), Actor::user())
        .expect("a");
    let (b, _) = h
        .runtime
        .create_agent(spec_on("part two", "claude-code", "haiku"), Actor::user())
        .expect("b");
    h.wait_until(a.id, |s| s.is_terminal()).await;
    h.wait_until(b.id, |s| s.is_terminal()).await;
    assert_never_overlapped(
        &turn_intervals(h.store.as_ref(), a.id),
        &turn_intervals(h.store.as_ref(), b.id),
        "engine turns overlapped under an engine cap of 1",
    );
    assert_eq!(
        h.client.peak_in_flight("claude-code"),
        1,
        "more engine turns were open at once than the cap allows"
    );
    assert_eq!(h.client.remaining(), 0);
}

/// The cap is on engines, not on turns: an API-model agent runs while the
/// only engine slot is held. Without that, one knob would quietly halve the
/// fleet's throughput on work that costs a subscription window nothing.
#[tokio::test]
async fn an_api_turn_does_not_count_against_the_engine_cap() {
    let h = harness_configured(
        vec![],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(1);
        },
    )
    .await;
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("complete_task", json!({"summary": "engine done"})),
    );
    h.client.push_for(
        "anthropic",
        ScriptedClient::tool_call("complete_task", json!({"summary": "api done"})),
    );
    let release_engine = h.client.hold_next_response();
    let (engine, _) = h
        .runtime
        .create_agent(
            spec_on("engine work", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("engine agent");
    // The engine turn is open and holding the only engine slot.
    h.wait_until(engine.id, |s| matches!(s, AgentStatus::Running))
        .await;
    let (api, _) = h
        .runtime
        .create_agent(
            spec_on("api work", "anthropic", "claude-sonnet-5-5"),
            Actor::user(),
        )
        .expect("api agent");
    // It finishes without waiting for the engine slot to free.
    h.wait_until(api.id, |s| s.is_terminal()).await;
    release_engine.cancel();
    h.wait_until(engine.id, |s| s.is_terminal()).await;
}

/// A slot freed by the cap goes to whoever has waited longest among equals,
/// the same order every other limit is relieved in (VISION §3.6 limit 6).
#[tokio::test]
async fn the_engine_cap_admits_the_longest_waiting_agent_first() {
    let h = harness_configured(
        vec![
            turn_of_length(120, "blocker"),
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
        ],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(1);
        },
    )
    .await;
    h.client.set_delay(Duration::from_millis(20));
    let (blocker, _) = h
        .runtime
        .create_agent(
            spec_on("hold the only engine slot", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("blocker");
    let mut queued = Vec::new();
    for objective in ["first", "second", "third"] {
        // Distinct `updated_at`, so the tiebreak under test is the one the
        // assertion reads rather than store order.
        tokio::time::sleep(Duration::from_millis(15)).await;
        queued.push(
            h.runtime
                .create_agent(spec_on(objective, "claude-code", "haiku"), Actor::user())
                .expect("queued")
                .0,
        );
    }
    for id in std::iter::once(blocker.id).chain(queued.iter().map(|a| a.id)) {
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
    // The cap held: the queued turns ran one at a time...
    for pair in queued.windows(2) {
        assert_never_overlapped(
            &turn_intervals(h.store.as_ref(), pair[0].id),
            &turn_intervals(h.store.as_ref(), pair[1].id),
            "engine turns overlapped under an engine cap of 1",
        );
    }
    // ...and each slot went to whoever had waited longest.
    let order: Vec<u64> = queued.iter().map(|a| started(a.id)).collect();
    assert!(
        order.windows(2).all(|w| w[0] < w[1]),
        "the engine cap admitted out of order: {order:?}"
    );
}

/// No deadlock is possible: only a turn actually running on an engine holds
/// a slot, and a parent asleep waiting on its child holds none. With a cap
/// of one and both agents engine-backed, the child runs, releases the slot,
/// and the parent's wake-up gets it to record the result.
#[tokio::test]
async fn an_engine_capped_parent_still_wakes_to_record_its_child_result() {
    let h = harness_full_with_engines(
        vec![
            ScriptedClient::tool_call(
                "delegate",
                json!({"effort":"low",
                    "role": "worker",
                    "objective": "do the piece",
                    "model": "claude-code/haiku",
                    "wait": true,
                }),
            ),
            ScriptedClient::tool_call("complete_task", json!({"summary": "child done"})),
            ScriptedClient::tool_call("complete_task", json!({"summary": "parent done"})),
        ],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(1);
        },
        None,
        None,
        None,
        |_| {},
        declared_engines(),
    )
    .await;
    let (parent, _) = h
        .runtime
        .create_agent(
            spec_on("supervise the piece", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("parent");
    let status = h.wait_until(parent.id, |s| s.is_terminal()).await;
    assert_eq!(
        status.kind(),
        AgentStatusKind::Completed,
        "the parent never got a slot back from its child: {status:?}"
    );
    let children = h
        .store
        .agents(&AgentFilter {
            parent: Some(parent.id),
            ..Default::default()
        })
        .expect("children");
    assert_eq!(children.len(), 1, "expected exactly one child");
    assert!(
        children[0].status.is_terminal(),
        "the child never finished: {:?}",
        children[0].status
    );
    // Never two engine turns at once - but that is not this test's evidence
    // for the cap: `wait: true` parks the parent before the child can run, so
    // the two could not have overlapped in any case. This is the no-deadlock
    // test; the cap binding is proved by the tests above and by
    // `an_engine_turn_unparking_from_a_lease_queues_for_its_slot_again`.
    // (`turn_intervals` cannot be used here - an event filter by agent also
    // matches that agent's children.)
    assert_eq!(
        h.client.peak_in_flight("claude-code"),
        1,
        "more engine turns were open at once than the cap allows"
    );
    assert_eq!(h.client.remaining(), 0);
}
