//! Native turns wait out network and other transient provider failures,
//! resuming the same session, inside a bounded window.
use super::*;
use aether_core::model::{ModelErrorClass, StreamEvent, Usage};

pub(super) const OFFLINE: &str =
    "API Error: Can't reach the API server — check your internet or DNS (EAI_AGAIN)";

pub(super) fn failure(error: &str) -> EngineEvent {
    EngineEvent::Done(EngineOutcome {
        is_error: true,
        error: Some(error.into()),
        ..Default::default()
    })
}

pub(super) fn session(id: &str) -> EngineEvent {
    EngineEvent::Session {
        id: id.into(),
        model: None,
        tools: vec![],
    }
}

/// A launch that opened its session, then could not reach the provider, as
/// claude-code reports an outage.
pub(super) fn offline() -> Vec<EngineEvent> {
    vec![session("s"), failure(OFFLINE)]
}

pub(super) fn partial(text: &str) -> EngineEvent {
    EngineEvent::Delta(StreamEvent::TextDelta {
        index: 0,
        text: text.into(),
    })
}

pub(super) async fn queued_with(
    runs: Vec<Vec<EngineEvent>>,
    configure: impl FnOnce(&mut RuntimeConfig),
) -> (Harness, Arc<QueuedEngine>) {
    let engine = QueuedEngine::new(runs);
    let h = harness_with_engine_config(engine.clone(), configure).await;
    (h, engine)
}

pub(super) fn failures(h: &Harness, id: AgentId) -> Vec<(ModelErrorClass, bool)> {
    h.store
        .events(0, 10_000, &EventFilter::own_events(id))
        .unwrap()
        .into_iter()
        .filter_map(|event| match event.kind {
            EventKind::InvocationFailed {
                class: Some(class),
                retryable,
                ..
            } => Some((class, retryable)),
            _ => None,
        })
        .collect()
}

pub(super) fn retries(h: &Harness, id: AgentId) -> Vec<serde_json::Value> {
    h.store
        .events(0, 10_000, &EventFilter::own_events(id))
        .unwrap()
        .into_iter()
        .filter_map(|event| match event.kind {
            EventKind::Custom { name, payload } if name == "engine.transient_retry" => {
                Some(payload)
            }
            _ => None,
        })
        .collect()
}

/// A chat agent whose first turn left the engine session `retained`.
pub(super) async fn resumed_chat(
    then: Vec<Vec<EngineEvent>>,
    configure: impl FnOnce(&mut RuntimeConfig),
) -> (Harness, Arc<QueuedEngine>, AgentId) {
    let mut first = vec![session("retained")];
    first.extend(answered("ready"));
    let mut runs = vec![first];
    runs.extend(then);
    let (h, engine) = queued_with(runs, configure).await;
    let id = chat_agent(&h);
    engine_turn(&h, id, "first").await;
    (h, engine, id)
}

#[tokio::test]
async fn an_outage_mid_turn_resumes_the_same_session_with_a_continuation_note() {
    for (error, class) in [
        (OFFLINE, ModelErrorClass::Network),
        (
            "API Error: 500 Internal server error",
            ModelErrorClass::Server,
        ),
    ] {
        let (h, engine, id) = resumed_chat(
            vec![
                // The model was already answering when the network went away.
                vec![session("retained"), partial("halfway"), failure(error)],
                // Still offline: nothing reaches the model this time.
                vec![failure(error)],
                answered("finished"),
            ],
            |_| {},
        )
        .await;
        engine_turn(&h, id, "do the work").await;
        assert_eq!(
            *engine.sessions.lock().unwrap(),
            [
                None,
                Some("retained".into()),
                Some("retained".into()),
                Some("retained".into())
            ],
            "{error}"
        );
        let prompts = engine.prompts.lock().unwrap().clone();
        assert!(prompts[1].contains("do the work"), "{error}");
        for resumed in &prompts[2..] {
            assert!(
                resumed.contains(&format!(
                    "interrupted by a transient {} error",
                    class.as_str()
                )),
                "{resumed}"
            );
            assert!(
                !resumed.contains("do the work"),
                "the prompt is never replayed"
            );
        }
        assert_eq!(failures(&h, id), [(class, true), (class, true)]);
        let waits = retries(&h, id);
        assert_eq!(waits.len(), 2);
        assert_eq!(waits[0]["class"], class.as_str());
        assert_eq!(waits[0]["resume"], "continue");
        assert_eq!(waits[0]["attempt"], 1);
        assert_eq!(waits[1]["attempt"], 2);
        assert!(waits[0]["next_at"].is_string());
        let agent = h.store.agent(id).unwrap().unwrap();
        assert_eq!(agent.status.kind(), AgentStatusKind::Waiting);
        assert_eq!(agent.engine_session.as_deref(), Some("retained"));
        assert!(
            spoken(h.store.as_ref(), id)
                .iter()
                .any(|(role, text)| *role == MessageRole::Assistant && text == "finished")
        );
        h.runtime.shutdown().await;
    }
}

#[tokio::test]
async fn a_first_turn_that_cannot_reach_the_provider_is_retried_with_its_prompt() {
    let (h, engine) = queued_with(
        vec![
            // Offline before the engine even reported a session, and the
            // adapter proves the prompt never left.
            vec![EngineEvent::Done(EngineOutcome {
                is_error: true,
                error: Some("workspace routing discovery failed".into()),
                prompt_not_submitted: true,
                ..Default::default()
            })],
            // The session started, but the API call behind it could not.
            vec![session("fresh"), failure(OFFLINE)],
            answered("online again"),
        ],
        |_| {},
    )
    .await;
    let id = chat_agent(&h);
    engine_turn(&h, id, "hello").await;
    assert_eq!(
        *engine.sessions.lock().unwrap(),
        [None, None, Some("fresh".into())]
    );
    let prompts = engine.prompts.lock().unwrap().clone();
    assert!(prompts[0].contains("hello"));
    assert_eq!(prompts[1], prompts[0], "proven unsent: replayed as it was");
    // The started session may hold the prompt: resumed with it, told to check.
    assert!(prompts[2].contains("interrupted delivery of the message below"));
    assert!(prompts[2].ends_with(&prompts[0]));
    assert_eq!(
        failures(&h, id),
        [
            (ModelErrorClass::Network, true),
            (ModelErrorClass::Network, true)
        ]
    );
    let resumes: Vec<_> = retries(&h, id)
        .iter()
        .map(|wait| wait["resume"].clone())
        .collect();
    assert_eq!(resumes, ["replay", "recheck"]);
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn a_continued_turn_bills_each_run_once() {
    let (h, engine, id) = resumed_chat(
        vec![
            vec![
                session("retained"),
                partial("halfway"),
                EngineEvent::Done(EngineOutcome {
                    is_error: true,
                    error: Some(OFFLINE.into()),
                    cost_usd: Some(0.5),
                    usage: Usage {
                        input_tokens: 100,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ],
            vec![EngineEvent::Done(EngineOutcome {
                text: Some("finished".into()),
                cost_usd: Some(0.25),
                usage: Usage {
                    input_tokens: 40,
                    ..Default::default()
                },
                ..Default::default()
            })],
        ],
        |_| {},
    )
    .await;
    let before = h.store.agent(id).unwrap().unwrap().stats.usage;
    engine_turn(&h, id, "do the work").await;
    assert_eq!(engine.prompts.lock().unwrap().len(), 3);
    let after = h.store.agent(id).unwrap().unwrap().stats.usage;
    assert_eq!(after.input_tokens - before.input_tokens, 140);
    assert!(
        (after.cost_usd - before.cost_usd - 0.75).abs() < 1e-9,
        "{after:?}"
    );
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn an_outage_longer_than_the_window_fails_the_agent_clearly() {
    let (h, engine) = queued_with((0..1_000).map(|_| offline()).collect(), |config| {
        config.engine_transient_retry_window = Duration::from_millis(300)
    })
    .await;
    let (id, task) = engine_worker(&h, "outlast the outage");
    let failed = wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
    let AgentStatus::Failed { error } = &failed.status else {
        panic!("{:?}", failed.status);
    };
    assert!(error.contains("EAI_AGAIN"), "{error}");
    assert!(
        error.contains("gave up on a transient network failure after retrying for"),
        "{error}"
    );
    assert!(matches!(
        h.store.task(task).unwrap().unwrap().status,
        aether_core::task::TaskStatus::Failed { .. }
    ));
    let launches = engine.prompts.lock().unwrap().len();
    assert!((2..1_000).contains(&launches), "{launches}");
    // Retryable per launch; the give-up is terminal, recorded on its own when
    // the window ran out during a wait rather than during the last launch.
    let recorded = failures(&h, id);
    assert!(
        [launches, launches + 1].contains(&recorded.len()),
        "{recorded:?}"
    );
    assert!(recorded.iter().all(|f| f.0 == ModelErrorClass::Network));
    let (last, retried) = recorded.split_last().unwrap();
    assert!(!last.1 && retried.iter().all(|f| f.1));
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn a_zero_window_fails_on_the_first_transient_error() {
    let (h, engine) = queued_with(vec![offline(), answered("never")], |config| {
        config.engine_transient_retry_window = Duration::ZERO;
    })
    .await;
    let (id, _) = engine_worker(&h, "no patience");
    wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
    assert_eq!(engine.prompts.lock().unwrap().len(), 1);
    assert!(retries(&h, id).is_empty());
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn cancelling_during_the_backoff_stops_promptly() {
    let (h, engine) = queued_with(vec![offline(), answered("never")], |config| {
        config.engine_transient_retry_floor = Duration::from_secs(3_600);
    })
    .await;
    let (id, _) = engine_worker(&h, "cancel me while waiting");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while retries(&h, id).is_empty() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "never started waiting"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let asked = tokio::time::Instant::now();
    h.runtime
        .cancel_agent(id, Some("stop".into()), Actor::user())
        .unwrap();
    let cancelled = wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
    assert!(asked.elapsed() < Duration::from_secs(5));
    assert!(matches!(cancelled.status, AgentStatus::Cancelled { .. }));
    assert_eq!(engine.prompts.lock().unwrap().len(), 1);
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn no_engine_is_launched_while_the_provider_host_is_unreachable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let target = listener.local_addr().unwrap();
    drop(listener);
    let (h, engine) = queued_with(vec![offline(), answered("back")], |config| {
        config.engine_probe_targets = Some(vec![target.to_string()]);
    })
    .await;
    let id = chat_agent(&h);
    h.runtime
        .send_message(id, vec![ContentBlock::text("go")], Actor::user())
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while retries(&h, id).len() < 3 {
        assert!(tokio::time::Instant::now() < deadline, "probe never failed");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        engine.prompts.lock().unwrap().len(),
        1,
        "launched while offline"
    );
    assert!(
        retries(&h, id)
            .iter()
            .all(|wait| wait["waiting_for"] == "network")
    );
    let _online = std::net::TcpListener::bind(target).unwrap();
    wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns > 0 && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    assert_eq!(engine.prompts.lock().unwrap().len(), 2);
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn quota_and_usage_limits_still_suspend_instead_of_waiting() {
    for (error, class) in [
        ("You've hit your usage limit", ExhaustionClass::UsageLimit),
        (
            "insufficient_quota (ECONNRESET)",
            ExhaustionClass::QuotaExhausted,
        ),
    ] {
        let (h, engine) = queued_with(vec![vec![failure(error)], answered("never")], |_| {}).await;
        let (id, _) = engine_worker(&h, "spend it all");
        let blocked = wait_for(h.store.as_ref(), id, |a| {
            matches!(a.status, AgentStatus::Blocked { .. }) || a.status.is_terminal()
        })
        .await;
        assert!(
            matches!(
                &blocked.status,
                AgentStatus::Blocked {
                    reason: BlockReason::Exhausted { class: c, .. }
                } if *c == class
            ),
            "{error}: {:?}",
            blocked.status
        );
        assert_eq!(engine.prompts.lock().unwrap().len(), 1);
        assert!(retries(&h, id).is_empty());
        h.runtime.shutdown().await;
    }
}

#[tokio::test]
async fn an_unknown_failure_still_fails_at_once() {
    let (h, engine) = queued_with(
        vec![vec![failure("process failed")], answered("never")],
        |_| {},
    )
    .await;
    let (id, _) = engine_worker(&h, "fail plainly");
    let failed = wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
    assert!(
        matches!(&failed.status, AgentStatus::Failed { error } if error == "claude-code failed: process failed"),
        "{:?}",
        failed.status
    );
    assert_eq!(engine.prompts.lock().unwrap().len(), 1);
    assert!(retries(&h, id).is_empty());
    assert_eq!(failures(&h, id), [(ModelErrorClass::Unknown, false)]);
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn activity_without_a_known_session_is_never_replayed() {
    let (h, engine) = queued_with(
        vec![
            vec![partial("halfway"), failure(OFFLINE)],
            answered("never"),
        ],
        |_| {},
    )
    .await;
    let (id, _) = engine_worker(&h, "nothing to resume");
    wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
    assert_eq!(engine.prompts.lock().unwrap().len(), 1);
    assert_eq!(failures(&h, id), [(ModelErrorClass::Network, false)]);
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn a_deadline_still_stops_an_agent_waiting_out_an_outage() {
    let (h, engine) = queued_with(vec![offline(), answered("never")], |config| {
        config.engine_transient_retry_floor = Duration::from_secs(3_600);
    })
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "worker".into(),
                start: true,
                objective: Some("run out of time offline".into()),
                config: Some(AgentConfig {
                    approval: ApprovalPolicy::none(),
                    deadline: Some(aether_core::time::now() + chrono::Duration::seconds(2)),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .unwrap();
    let stopped = wait_for(h.store.as_ref(), agent.id, |a| a.status.is_terminal()).await;
    assert!(
        format!("{:?}", stopped.status).contains("deadline"),
        "{:?}",
        stopped.status
    );
    assert_eq!(retries(&h, agent.id).len(), 1);
    assert_eq!(engine.prompts.lock().unwrap().len(), 1);
    h.runtime.shutdown().await;
}
