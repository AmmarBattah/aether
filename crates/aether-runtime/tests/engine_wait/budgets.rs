use super::*;

/// A bridge call over the grant is refused, and the process behind it is
/// wound down the way a park is: the engine is asked to stop in its own verb,
/// with the grace the refusal needs to reach it, before it is killed. The
/// engines in `tests/engine.rs` report no stop verb, so deleting the whole
/// wind-down left every test there green.
#[tokio::test]
async fn a_bridge_call_over_the_grant_asks_the_engine_to_stop_before_killing_it() {
    let mut h = harness(Duration::from_secs(3600)).await;
    let coordinator = start_agent(&h, budgeted_spec(40_000));
    let child = idle_child(&h, coordinator);
    let first = turn_of(&mut h, coordinator).await;
    spend_over_budget(&h, child, 41_000);

    let (reply, is_error) = call(&h, &first, "list_tasks", json!({})).await;
    assert!(is_error, "a call past the grant was served: {reply}");
    assert!(reply.contains("token budget of 40000"), "{reply}");

    wait_until_asked_to_stop(&first.observed, Duration::from_secs(5)).await;
    assert_eq!(
        first.observed.stop_grace(),
        Some(Duration::from_millis(500)),
        "the wind-down ran on the park grace, not the refusal's own"
    );
    // The operator's park grace is an hour here: the stop that follows is the
    // refusal's, and it has to arrive anyway.
    wait_until_stopped(&first.observed, Duration::from_secs(5)).await;

    let stopped = wait_until(h.store.as_ref(), coordinator, |a| !a.is_live()).await;
    assert!(
        failed_with(&stopped).contains("token budget of 40000"),
        "{:?}",
        stopped.status
    );
    assert!(
        custom(h.store.as_ref(), coordinator, "engine.parked").is_some(),
        "the refusal-created wind-down left no engine.parked"
    );
    h.runtime.shutdown().await;
}

/// A straggler bridge call from a park that is already being killed still
/// runs the budget check - the grant binds whatever else is happening - and
/// the wind-down it lands in is still recorded. Without the record the only
/// durable trace of an engine that ignored the stop is lost to the crossing.
#[tokio::test]
async fn a_straggler_call_crossing_the_grant_still_records_the_wind_down() {
    let mut h = harness_full(
        Duration::from_millis(200),
        OnKill::Report(vec![cancelled_with(Usage::default())]),
        Script {
            linger_after_stop: Some(Duration::from_secs(3)),
            ..Default::default()
        },
    )
    .await;
    let coordinator = start_agent(&h, budgeted_spec(40_000));
    let child = idle_child(&h, coordinator);
    let first = turn_of(&mut h, coordinator).await;

    let (reply, is_error) = call(&h, &first, "wait_for", json!({"timeout_seconds": 3600})).await;
    assert!(!is_error, "{reply}");
    wait_until_stopped(&first.observed, Duration::from_secs(2)).await;
    spend_over_budget(&h, child, 41_000);

    let (reply, is_error) = call(
        &h,
        &first,
        "remember",
        json!({"text": "one last thought", "kind": "lesson"}),
    )
    .await;
    assert!(is_error, "{reply}");
    assert!(
        reply.contains("token budget of 40000"),
        "the grant is not checked once the park is being killed: {reply}"
    );
    // Let the engine report and the turn finalize: the scheduler's own sweep
    // is a second away, and this is about what the turn itself records.
    let _ = first.end.send(());

    let stopped = wait_until(h.store.as_ref(), coordinator, |a| !a.is_live()).await;
    assert!(
        failed_with(&stopped).contains("token budget of 40000"),
        "{:?}",
        stopped.status
    );
    let park = custom(h.store.as_ref(), coordinator, "engine.parked")
        .expect("the wind-down the crossing landed in left no trace");
    assert_eq!(park["stopped_by"], json!("runtime"));
    h.runtime.shutdown().await;
}

/// The accounting path's own crossing arm has to route through the park the
/// same way the kill-switch-bypass arm does: a park already open when the
/// engine's own event pushes it over the grant is a runtime stop, not the
/// engine stopping itself, and the record must say so.
#[tokio::test]
async fn a_crossing_found_while_a_park_is_open_blames_the_runtime_not_the_engine() {
    let mut h = harness_full(
        Duration::from_secs(5),
        OnKill::Report(vec![cancelled_with(Usage::default())]),
        Script {
            crossing_after_stop: Some(Usage {
                input_tokens: 41_000,
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await;
    let coordinator = start_agent(&h, budgeted_spec(40_000));
    let first = turn_of(&mut h, coordinator).await;

    let (reply, is_error) = call(&h, &first, "wait_for", json!({"timeout_seconds": 3600})).await;
    assert!(!is_error, "{reply}");
    wait_until_asked_to_stop(&first.observed, Duration::from_secs(5)).await;

    let stopped = wait_until(h.store.as_ref(), coordinator, |a| !a.is_live()).await;
    assert!(
        failed_with(&stopped).contains("token budget of 40000"),
        "{:?}",
        stopped.status
    );
    let park = custom(h.store.as_ref(), coordinator, "engine.parked")
        .expect("the crossing found inside the park left no record");
    assert_eq!(
        park["stopped_by"],
        json!("runtime"),
        "the runtime already asked this park to stop; the record blames the engine instead"
    );
    h.runtime.shutdown().await;
}

/// A session lost because the grant stopped the process must not be retried:
/// the spend that crossed the grant is gone either way, and the retry would
/// run the whole turn again on a budget the agent no longer has.
#[tokio::test]
async fn a_turn_stopped_by_its_grant_is_not_retried_when_the_stop_loses_the_session() {
    let held = EngineOutcome {
        usage: Usage {
            input_tokens: 45_000,
            ..Default::default()
        },
        session_lost: true,
        is_error: true,
        error: Some("cancelled".into()),
        ..Default::default()
    };
    let mut h = harness_full(
        Duration::from_millis(200),
        OnKill::Report(vec![cancelled_with(Usage::default())]),
        Script {
            // Held back by the tool it says is still running, so the crossing
            // that follows is the accounting's, not this result's.
            background: Some("build".into()),
            crossing_on_resume: Some((
                held,
                Usage {
                    input_tokens: 41_000,
                    ..Default::default()
                },
            )),
            ..Default::default()
        },
    )
    .await;
    let coordinator = start_agent(&h, budgeted_spec(40_000));
    // The retry only applies to a turn that asked to resume a session, so the
    // crossing has to be on the second one.
    let first = turn_of(&mut h, coordinator).await;
    let _ = first.end.send(());
    wait_for_park(&h, coordinator).await;
    h.runtime
        .send_message(
            coordinator,
            vec![ContentBlock::text("carry on")],
            Actor::user(),
        )
        .expect("message");

    let second = turn_of(&mut h, coordinator).await;
    assert!(
        second.resumed.is_some(),
        "the second turn resumes a session"
    );

    let stopped = wait_until(h.store.as_ref(), coordinator, |a| !a.is_live()).await;
    assert!(
        failed_with(&stopped).contains("token budget of 40000"),
        "{:?}",
        stopped.status
    );
    assert_eq!(
        stopped.stats.usage.total_tokens(),
        45_000,
        "the result the crossing landed on was dropped; the turn books what \
         the stream happened to have reported instead of what was measured"
    );
    assert!(
        h.started.try_recv().is_err(),
        "the crossed turn was run again with a fresh session"
    );
    assert_eq!(turns_started(h.store.as_ref(), coordinator), 2);
    h.runtime.shutdown().await;
}
