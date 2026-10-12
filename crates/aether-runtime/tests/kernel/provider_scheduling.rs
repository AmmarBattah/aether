//! provider scheduling scenarios and fixtures.
use super::*;

/// Limit 4: two agents on a provider capped at one turn never overlap, even
/// with global slots to spare.
#[tokio::test]
async fn a_provider_limit_runs_its_agents_one_at_a_time() {
    let h = harness_full(
        vec![slow_turn("one"), slow_turn("two")],
        |_| {},
        |c| c.max_concurrent_turns = 4,
        Some("[providers.anthropic]\nmax_concurrent = 1\n"),
        None,
        None,
        |_| {},
    )
    .await;
    h.client.set_delay(Duration::from_millis(60));
    let (a, _) = h
        .runtime
        .create_agent(
            spec_on("part one", "anthropic", "claude-sonnet-5-5"),
            Actor::user(),
        )
        .expect("a");
    let (b, _) = h
        .runtime
        .create_agent(
            spec_on("part two", "anthropic", "claude-sonnet-5-5"),
            Actor::user(),
        )
        .expect("b");
    h.wait_until(a.id, |s| s.is_terminal()).await;
    h.wait_until(b.id, |s| s.is_terminal()).await;
    assert_never_overlapped(
        &turn_intervals(h.store.as_ref(), a.id),
        &turn_intervals(h.store.as_ref(), b.id),
        "turns overlapped under a provider cap of 1",
    );
    assert_eq!(h.client.remaining(), 0);
}

/// Limit 4, the other half: when a slot frees and several agents are queued,
/// it goes to the provider carrying the least, not to whoever queued first.
#[tokio::test]
async fn the_scheduler_spreads_turns_across_healthy_providers() {
    let h = harness_configured(
        vec![
            turn_of_length(200, "long"),
            turn_of_length(40, "medium"),
            turn_of_length(2, "short"),
            turn_of_length(2, "short"),
        ],
        |_| {},
        |c| c.max_concurrent_turns = 2,
    )
    .await;
    h.client.set_delay(Duration::from_millis(20));
    let create = |objective: &str, provider: &str, model: &str| {
        h.runtime
            .create_agent(spec_on(objective, provider, model), Actor::user())
            .expect("create")
            .0
    };
    // Both slots go to anthropic first: one long turn, one that frees a slot.
    let long = create("long", "anthropic", "claude-sonnet-5-5");
    let medium = create("medium", "anthropic", "claude-sonnet-5-5");
    // Queued behind the full global limit, the older one on the busy provider.
    let queued_anthropic = create("queued a", "anthropic", "claude-sonnet-5-5");
    let queued_openai = create("queued b", "openai", "gpt-6.1-sol");
    for id in [long.id, medium.id, queued_anthropic.id, queued_openai.id] {
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
    assert!(
        started(queued_openai.id) < started(queued_anthropic.id),
        "the freed slot went to the busy provider: openai at {}, anthropic at {}",
        started(queued_openai.id),
        started(queued_anthropic.id)
    );
    assert_eq!(h.client.remaining(), 0);
}

/// A 429 backs the provider off with a recorded reason, and the scheduler
/// says so again when the provider is usable.
#[tokio::test]
async fn a_rate_limit_backs_the_provider_off_and_recovery_is_recorded() {
    let h = harness(vec![
        ScriptedClient::error(ModelErrorClass::RateLimit, "slow down"),
        ScriptedClient::tool_call("complete_task", json!({"summary": "second try"})),
    ])
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("retry me"), Actor::user())
        .expect("create");
    h.wait_until(agent.id, |s| s.is_terminal()).await;
    let events = h
        .store
        .events(0, 10_000, &EventFilter::all())
        .expect("events");
    let backoff = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ProviderBackoff {
                provider, reason, ..
            } => Some((provider.clone(), reason.clone())),
            _ => None,
        })
        .expect("provider.backoff");
    assert_eq!(backoff.0, "anthropic");
    assert_eq!(backoff.1, "rate limited");
    assert!(
        events
            .iter()
            .any(|e| matches!(&e.kind, EventKind::ProviderRecovered { provider } if provider == "anthropic")),
        "no provider.recovered after the backoff expired"
    );
    // The health projection is clean once the provider is usable again.
    assert!(
        h.runtime
            .provider_health()
            .expect("provider health")
            .iter()
            .all(|p| p.backoff.is_none()),
        "{:?}",
        h.runtime.provider_health()
    );
}
