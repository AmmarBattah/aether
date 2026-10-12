use super::*;
use crate::exhaustion::probing::{PROBE_CAP, REPORTED_RESET_CAP};

fn suspended_on(probes: u32) -> BlockReason {
    BlockReason::Exhausted {
        model: ModelRef::new("anthropic", "claude-sonnet-5-5"),
        class: ExhaustionClass::UsageLimit,
        since: now(),
        reported_reset: None,
        next_probe: now(),
        probes,
        detail: "spent".into(),
    }
}

/// How long the next probe is away, however the jitter fell.
fn interval_ms(
    previous: Option<&BlockReason>,
    retry_after: Option<Duration>,
    floor: Duration,
) -> i64 {
    let at = now();
    (next_probe(at, previous, retry_after, floor) - at).num_milliseconds()
}

fn assert_near(got: i64, want: i64, what: &str) {
    let spread = want as f64 * 0.1;
    assert!(
        ((want as f64 - spread)..=(want as f64 + spread)).contains(&(got as f64)),
        "{what}: wanted {want}ms ±10%, got {got}ms"
    );
}

#[test]
fn the_probe_schedule_starts_at_the_floor_and_doubles_to_the_cap() {
    let floor = Duration::from_secs(60);
    assert_near(interval_ms(None, None, floor), 60_000, "first suspension");
    for (probes, want) in [(0, 120_000), (1, 240_000), (3, 960_000)] {
        let previous = suspended_on(probes);
        assert_near(
            interval_ms(Some(&previous), None, floor),
            want,
            &format!("after {probes} failed probes"),
        );
    }
    // Far past the cap, including a doubling that would overflow.
    for probes in [10, u32::MAX] {
        let previous = suspended_on(probes);
        assert_near(
            interval_ms(Some(&previous), None, floor),
            PROBE_CAP.as_millis() as i64,
            "capped",
        );
    }
}

#[test]
fn a_reported_retry_after_wins_over_the_doubling_and_is_clamped() {
    let floor = Duration::from_millis(10);
    let previous = suspended_on(5);
    for (retry_after, want) in [
        (Duration::from_secs(2), 2_000),
        // A zero-second header must not spin; a month must not park.
        (Duration::ZERO, floor.as_millis() as i64),
        (
            Duration::from_secs(30 * 24 * 3600),
            REPORTED_RESET_CAP.as_millis() as i64,
        ),
    ] {
        assert_near(
            interval_ms(Some(&previous), Some(retry_after), floor),
            want,
            "reported retry-after",
        );
    }
}

/// The agent's own link, spent; `suspended_on` is the episode on it.
fn spent_link() -> ModelRef {
    ModelRef::new("anthropic", "claude-sonnet-5-5")
}

/// A later link of the chain, healthy while `spent_link` is out.
fn fallback_link() -> ModelRef {
    ModelRef::new("groq", "openai/gpt-oss-120b")
}

/// What `record` writes: the episode under `reason`, the spent link at the
/// payload root, which is what keys the episode.
fn suspend(store: &dyn Store, agent: &Agent, reason: &BlockReason) {
    let BlockReason::Exhausted { model, .. } = reason else {
        panic!("a suspension parks the agent on an exhausted block");
    };
    store
        .commit(Commit::new().event(custom(
            agent,
            &Actor::System,
            "exhaustion.suspended",
            json!({
                "reason": reason,
                "provider": model.provider,
                "model": model.model,
            }),
        )))
        .unwrap();
}

fn complete_invocation(store: &dyn Store, agent: &Agent, model: &ModelRef) {
    let completed = EventDraft::new(
        Actor::System,
        EventKind::InvocationCompleted {
            invocation: InvocationId::new(),
            snapshot: SnapshotId::new(),
            model: model.clone(),
            usage: Usage::default(),
            stop_reason: None,
            duration_ms: 1,
        },
    )
    .agent(agent.id)
    .run(agent.run);
    store.commit(Commit::new().event(completed)).unwrap();
}

/// Run `record_recovery` the way the turn does and return the provider it
/// released together with the `exhaustion.recovered` events it wrote.
fn recover(rt: &Runtime, agent: &Agent, answered: &ModelRef) -> (Option<String>, Vec<Value>) {
    let hold = rt.hold();
    let mut commit = Commit::new();
    let provider =
        record_recovery(rt, &hold, &mut commit, agent, answered, &Actor::System).unwrap();
    let written = commit
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom { name, payload } if name == "exhaustion.recovered" => {
                Some(payload.clone())
            }
            _ => None,
        })
        .collect();
    rt.apply_held(hold, commit).unwrap();
    (provider, written)
}

fn spent(model: ModelRef) -> Exhausted {
    Exhausted {
        model,
        class: ExhaustionClass::UsageLimit,
        detail: "spent".into(),
        retry_after: None,
    }
}

/// The row loses the episode the moment the agent is released to probe, so
/// it is read back from the events - and only until that link answers.
#[tokio::test]
async fn an_episode_is_open_until_an_invocation_completes_after_it() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let rt = runtime(store.clone(), dir.path()).await;
    let agent = create(&rt, None);
    let reason = suspended_on(2);
    suspend(store.as_ref(), &agent, &reason);
    assert_eq!(
        open_episode(&rt, &agent, &spent_link()).unwrap(),
        Some(reason)
    );

    complete_invocation(store.as_ref(), &agent, &spent_link());
    assert_eq!(open_episode(&rt, &agent, &spent_link()).unwrap(), None);
    rt.shutdown().await;
}

/// The episode belongs to the link whose quota was spent. A fallback
/// carrying the turn is not that link's recovery, so nothing is announced
/// and the probe count keeps climbing towards the cap.
#[tokio::test]
async fn a_fallback_answering_leaves_the_spent_links_episode_open() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let rt = runtime(store.clone(), dir.path()).await;
    let agent = create(&rt, None);
    let reason = suspended_on(2);
    suspend(store.as_ref(), &agent, &reason);

    for _ in 0..3 {
        complete_invocation(store.as_ref(), &agent, &fallback_link());
        let (provider, written) = recover(&rt, &agent, &fallback_link());
        assert_eq!(provider, None, "a fallback recovered nothing");
        assert!(written.is_empty(), "{written:?}");
    }
    assert_eq!(
        open_episode(&rt, &agent, &spent_link()).unwrap(),
        Some(reason.clone()),
        "the spent link's episode survives every fallback answer"
    );

    // The next suspension on the spent link continues that episode.
    let next = plan(&rt, &agent, &spent(spent_link())).unwrap();
    assert!(!next.new_episode, "the ancestors were told once already");
    let (BlockReason::Exhausted { probes, since, .. }, BlockReason::Exhausted { since: was, .. }) =
        (&next.reason, &reason)
    else {
        panic!("a suspension parks the agent on an exhausted block");
    };
    assert_eq!(*probes, 3, "the probe count kept climbing");
    assert_eq!(since, was, "one episode, not a new one");
    rt.shutdown().await;
}

/// The spent link itself answering is the recovery, and the event
/// names it - never the fallback that was carrying the turn.
#[tokio::test]
async fn the_spent_link_answering_closes_the_episode_and_names_it() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let rt = runtime(store.clone(), dir.path()).await;
    let agent = create(&rt, None);
    suspend(store.as_ref(), &agent, &suspended_on(2));
    complete_invocation(store.as_ref(), &agent, &fallback_link());

    let (provider, written) = recover(&rt, &agent, &spent_link());
    assert_eq!(provider.as_deref(), Some(spent_link().provider.as_str()));
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(written[0]["provider"], json!(spent_link().provider));
    assert_eq!(written[0]["model"], json!(spent_link().model));
    assert_eq!(written[0]["probes"], json!(2));

    // Closed: a second answer finds nothing, and the next suspension on
    // that link starts a fresh episode at the floor.
    assert_eq!(open_episode(&rt, &agent, &spent_link()).unwrap(), None);
    assert_eq!(recover(&rt, &agent, &spent_link()).0, None);
    let next = plan(&rt, &agent, &spent(spent_link())).unwrap();
    assert!(next.new_episode, "a recovered link starts over");
    rt.shutdown().await;
}

#[tokio::test]
async fn incident_notices_do_not_close_an_existing_suspension() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let rt = runtime(store.clone(), dir.path()).await;
    let agent = create(&rt, None);
    let reason = suspended_on(2);
    suspend(store.as_ref(), &agent, &reason);
    let hold = rt.hold();
    let mut commit = Commit::new();
    record_incident(
        &rt,
        &hold,
        &mut commit,
        &agent,
        &spent_link(),
        &aether_core::model::ModelError::new(ModelErrorClass::UsageLimit, "spent"),
        &Actor::System,
    )
    .unwrap();
    rt.apply_held(hold, commit).unwrap();
    assert_eq!(
        open_episode(&rt, &agent, &spent_link()).unwrap(),
        Some(reason)
    );
    rt.shutdown().await;
}
