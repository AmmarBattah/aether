//! engine fallback slots scenarios and fixtures.
use super::*;

/// The leak the `claim_link` re-check exists to close: an agent admitted on
/// an API link, whose chain falls back onto an engine, would take an engine
/// slot the scheduler never admitted it for. It waits for a real one instead,
/// and the turn is handed back unspent rather than failed.
#[tokio::test]
async fn an_api_turn_falling_back_onto_an_engine_waits_for_a_real_slot() {
    let h = harness_configured(
        vec![],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(1);
        },
    )
    .await;
    // The only engine slot, held open until the test releases it.
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("complete_task", json!({"summary": "blocker done"})),
    );
    let release_blocker = h.client.hold_next_response();
    let (blocker, _) = h
        .runtime
        .create_agent(
            spec_on("hold the only engine slot", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("blocker");
    h.wait_for("the blocker never opened its stream", || {
        h.client.peak_in_flight("claude-code") >= 1
    })
    .await;

    // API primary, engine fallback. The primary fails retryably, so the turn
    // walks to the engine link while the cap is full.
    h.client.push_for(
        "anthropic",
        ScriptedClient::error(ModelErrorClass::RateLimit, "slow down"),
    );
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("complete_task", json!({"summary": "fell back"})),
    );
    let (falls_back, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                fallbacks: vec![ModelRef::new("claude-code", "haiku")],
                ..spec_on("start on the API", "anthropic", "claude-sonnet-5-5")
            },
            Actor::user(),
        )
        .expect("falls back");
    // It must not have jumped the queue while the slot was held. Without the
    // re-check the engine stream opens within a beat of this event, so a
    // short settle is enough to tell the two behaviours apart.
    h.wait_for("the chain never walked to its engine link", || {
        types(h.store.as_ref(), falls_back.id).contains(&"provider.fallback".to_owned())
    })
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        h.client.peak_in_flight("claude-code"),
        1,
        "the fallback took an engine slot it was never admitted for"
    );
    assert!(
        !h.store
            .agent(falls_back.id)
            .expect("read")
            .expect("exists")
            .status
            .is_terminal(),
        "the fallback ran its engine turn while the only slot was held"
    );

    release_blocker.cancel();
    for id in [blocker.id, falls_back.id] {
        let status = h.wait_until(id, |s| s.is_terminal()).await;
        assert_eq!(
            status.kind(),
            AgentStatusKind::Completed,
            "{id} did not finish once the slot freed: {status:?}"
        );
    }
    assert_eq!(
        h.client.peak_in_flight("claude-code"),
        1,
        "more engine turns were open at once than the cap allows"
    );
}

/// Under the cap every engine candidate competes for one pool, so ordering by
/// per-provider business starves the busy engine: while one slot of two is
/// held on E1, a fresh arrival on an idle E2 would beat an E1 candidate that
/// has waited arbitrarily long. Wait time decides between engines; provider
/// business still spreads API turns.
#[tokio::test]
async fn the_engine_cap_prefers_the_longest_waiting_across_engine_providers() {
    let h = harness_configured(
        vec![],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(2);
        },
    )
    .await;
    h.client.set_delay(Duration::from_millis(10));
    // Both engine slots start out held on claude-code; the shorter blocker
    // frees one while the longer keeps that provider visibly the busier.
    h.client
        .push_for("claude-code", turn_of_length(50, "short blocker"));
    h.client
        .push_for("claude-code", turn_of_length(400, "long blocker"));
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("complete_task", json!({"summary": "waited longest"})),
    );
    h.client.push_for(
        "codex",
        ScriptedClient::tool_call("complete_task", json!({"summary": "arrived later"})),
    );
    let engine_agent = |objective: &str, provider: &str, model: &str| {
        h.runtime
            .create_agent(spec_on(objective, provider, model), Actor::user())
            .expect("create")
            .0
    };
    let short = engine_agent("free a slot soon", "claude-code", "haiku");
    let long = engine_agent("stay busy", "claude-code", "haiku");
    h.wait_for("both engine slots were never held at once", || {
        h.client.peak_in_flight("claude-code") >= 2
    })
    .await;
    // Queued behind a full cap, the older one on the busier engine.
    let waited = engine_agent("queue on the busy engine", "claude-code", "haiku");
    tokio::time::sleep(Duration::from_millis(30)).await;
    let arrived = engine_agent("queue on the idle engine", "codex", "gpt-6.1-sol");
    for id in [short.id, long.id, waited.id, arrived.id] {
        h.wait_until(id, |s| s.is_terminal()).await;
    }
    assert!(
        first_event_seq(h.store.as_ref(), waited.id, "agent.started")
            < first_event_seq(h.store.as_ref(), arrived.id, "agent.started"),
        "the freed engine slot went to the newer candidate on the idler engine"
    );
    assert!(
        h.client.peak_in_flight("claude-code") <= 2,
        "more engine turns were open at once than the cap allows"
    );
}

/// What a run budget is for: a run that fanned out and finished its children
/// has still spent that money. Terminal agents count, and only this run's do.
#[tokio::test]
async fn a_run_budget_counts_a_finished_child_and_no_other_run() {
    let h = harness_configured(
        vec![ScriptedClient::tool_call(
            "delegate",
            json!({"model":"anthropic/claude-sonnet-5-5", "effort":"low", "role": "worker", "objective": "more work"}),
        )],
        |_| {},
        |c| c.run_budget_usd = Some(1.0),
    )
    .await;
    let (parent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                objective: None,
                ..spec("supervise")
            },
            Actor::user(),
        )
        .expect("parent");
    h.wait_until(parent.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    // The spend sits on a child that has already finished, in this run...
    let (child, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                parent: Some(parent.id),
                run: Some(parent.run),
                objective: None,
                start: false,
                ..spec("the work that cost the money")
            },
            Actor::user(),
        )
        .expect("child");
    let mut child = h.store.agent(child.id).expect("read").expect("exists");
    child.status = AgentStatus::Completed { summary: None };
    child.stats.usage.cost_usd = 2.5;
    child.stats.usage.cost_complete = true;
    // ...and a fat unrelated run must not be billed to it.
    let (other, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                run: Some(aether_core::id::RunId::new()),
                objective: None,
                start: false,
                ..spec("someone else's run")
            },
            Actor::user(),
        )
        .expect("other run");
    let mut other = h.store.agent(other.id).expect("read").expect("exists");
    other.stats.usage.cost_usd = 100.0;
    other.stats.usage.cost_complete = true;
    h.store
        .commit(
            Commit::new()
                .mutate(Mutation::UpsertAgent(child))
                .mutate(Mutation::UpsertAgent(other)),
        )
        .expect("seed spend");

    h.runtime
        .send_message(parent.id, vec![ContentBlock::text("go")], Actor::user())
        .expect("nudge");
    let told = wait_for_tool_answer(&h, parent.id).await;
    assert!(
        told.contains("delegation refused")
            && told.contains("of its $1 budget")
            // The finished child's $2.50 (plus the parent's own crumbs), and
            // emphatically not the $100 sitting in someone else's run.
            && told.contains("$2.50")
            && !told.contains("$102"),
        "the refusal did not read the finished child's spend, and only this run's: {told}"
    );
    let live = h
        .store
        .agents(&AgentFilter {
            parent: Some(parent.id),
            ..Default::default()
        })
        .expect("children");
    assert_eq!(
        live.len(),
        1,
        "a refused delegation left an agent behind: {live:?}"
    );
}
