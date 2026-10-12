//! suspension scenarios and fixtures.
use super::*;

#[tokio::test]
async fn final_link_exhaustion_suspends_instead_of_failing() {
    for class in [ModelErrorClass::QuotaExhausted, ModelErrorClass::UsageLimit] {
        let h = harness_configured(
            vec![ScriptedClient::error(
                class,
                "Original provider message; resets tomorrow 14:00",
            )],
            |_| {},
            |config| config.exhaustion_probe_floor = NO_PROBE_IN_THIS_TEST,
        )
        .await;
        let (parent, child, task) =
            parent_and_suspending_child(&h, spec("suspend rather than fail")).await;
        let status = h.wait_until(child, blocked_on_exhaustion).await;
        let reason = exhausted_reason(&status);
        let BlockReason::Exhausted {
            model,
            class: parked,
            since,
            next_probe,
            probes,
            detail,
            ..
        } = &reason
        else {
            panic!("{reason:?}")
        };
        assert_eq!(model.provider, "anthropic");
        assert_eq!(parked.as_str(), class.as_str());
        assert_eq!(*probes, 0, "the first suspension has failed no probe yet");
        assert!(next_probe > since && *next_probe > now(), "{reason:?}");
        assert!(detail.contains("resets tomorrow 14:00"), "{detail}");

        // The work is not lost: the task stays open behind the suspension.
        assert_eq!(
            h.store
                .task(task)
                .expect("task")
                .expect("exists")
                .status
                .kind(),
            TaskStatusKind::Active
        );
        let suspended = exhaustion_records(h.store.as_ref(), child, "suspended");
        assert_eq!(suspended.len(), 1, "{suspended:?}");
        assert_eq!(suspended[0].1["source_agent"], json!(child));
        assert_eq!(suspended[0].1["source_task"], json!(task));
        assert_eq!(suspended[0].1["probes"], json!(0));
        let verdict = suspended[0].1["verdict"].as_str().expect("verdict");
        assert!(
            verdict.contains("Resume it sooner from your client"),
            "{verdict}"
        );
        assert!(!verdict.contains("`aether "), "{verdict}");
        assert!(
            exhaustion_records(h.store.as_ref(), child, "failed").is_empty(),
            "a suspension is not a terminal exhaustion"
        );
        let t = types(h.store.as_ref(), child);
        assert!(!t.contains(&"agent.failed".to_owned()), "{t:?}");
        assert!(t.contains(&"agent.blocked".to_owned()), "{t:?}");

        let told = child_suspensions(h.store.as_ref(), parent);
        assert_eq!(told.len(), 1, "{told:?}");
        assert!(told[0].contains(&child.to_string()), "{}", told[0]);
        assert!(told[0].contains(&task.to_string()), "{}", told[0]);
        assert!(child_failures(h.store.as_ref(), parent).is_empty());

        // The provider is held exactly until this agent probes it again, so a
        // Ready agent does not walk into the wall in the meantime.
        let (benched, _) = bench_history(h.store.as_ref());
        let benched = benched.expect("provider.backoff");
        assert!(
            (benched - *next_probe).num_seconds().abs() <= 1,
            "benched until {benched}, probing at {next_probe}"
        );
        assert_eq!(
            h.client.requests().len(),
            1,
            "a spent link is never retried"
        );
        h.runtime.shutdown().await;
    }
}

/// PR C surfaces `agents_suspended` on `RuntimeStatus`; it must not replace
/// `agents_blocked`, since existing readers sum that counter to the live
/// total (design §7).
#[tokio::test]
async fn a_suspended_agent_counts_as_both_blocked_and_suspended() {
    let h = harness_configured(
        vec![ScriptedClient::error(
            ModelErrorClass::UsageLimit,
            "5-hour limit reached",
        )],
        |_| {},
        |config| config.exhaustion_probe_floor = NO_PROBE_IN_THIS_TEST,
    )
    .await;
    let (_, child, _) = parent_and_suspending_child(&h, spec("suspend and stay counted")).await;
    h.wait_until(child, blocked_on_exhaustion).await;
    let status = h.runtime.status().expect("status while suspended");
    assert_eq!(status.agents_suspended, 1);
    assert_eq!(
        status.agents_blocked, 1,
        "a suspended agent must still count as blocked"
    );
    h.runtime.shutdown().await;
}

/// The probe is the interrupted turn itself: the model sees the same last user
/// message it never answered, and nothing is replayed or re-executed.
#[tokio::test]
async fn a_suspended_agent_resumes_when_the_probe_succeeds() {
    let h = harness_configured(
        vec![
            ScriptedClient::error(ModelErrorClass::UsageLimit, "5-hour limit reached"),
            ScriptedClient::tool_call(
                "complete_task",
                json!({"summary": "finished after the wait"}),
            ),
        ],
        |_| {},
        |config| config.exhaustion_probe_floor = Duration::from_millis(50),
    )
    .await;
    let (_, child, _) = parent_and_suspending_child(&h, spec("finish after the limit")).await;
    let status = h.wait_until(child, |s| s.is_terminal()).await;
    assert!(
        matches!(status, AgentStatus::Completed { .. }),
        "{status:?}"
    );
    let requests = h.client.requests();
    assert_eq!(requests.len(), 2, "one probe, one turn's worth of calls");
    assert_eq!(
        requests[0].messages.last(),
        requests[1].messages.last(),
        "the probe re-issues the turn the provider refused"
    );
}

/// Never gives up: each failed probe waits longer, and the agent stays alive
/// to take the turn whenever the provider finally answers.
#[tokio::test]
async fn failed_probes_back_off_and_never_fail_the_agent() {
    let spent = || ScriptedClient::error(ModelErrorClass::QuotaExhausted, "credit balance too low");
    let h = harness_configured(
        vec![
            spent(),
            spent(),
            spent(),
            ScriptedClient::tool_call("complete_task", json!({"summary": "the window reset"})),
        ],
        |_| {},
        |config| config.exhaustion_probe_floor = Duration::from_millis(40),
    )
    .await;
    let (parent, child, _) = parent_and_suspending_child(&h, spec("keep probing")).await;
    let status = h.wait_until(child, |s| s.is_terminal()).await;
    assert!(
        matches!(status, AgentStatus::Completed { .. }),
        "{status:?}"
    );

    let mut records = exhaustion_records(h.store.as_ref(), child, "suspended");
    assert_eq!(records.len(), 1, "one episode, one diagnostic: {records:?}");
    records.extend(exhaustion_records(h.store.as_ref(), child, "probe_failed"));
    let probes: Vec<u64> = records
        .iter()
        .map(|(_, payload)| payload["probes"].as_u64().expect("probes"))
        .collect();
    assert_eq!(probes, vec![0, 1, 2], "{records:?}");
    let intervals: Vec<i64> = records
        .iter()
        .map(|(at, payload)| {
            let next: Timestamp =
                serde_json::from_value(payload["next_probe"].clone()).expect("next_probe");
            (next - *at).num_milliseconds()
        })
        .collect();
    assert!(
        intervals[0] < intervals[1] && intervals[1] < intervals[2],
        "each failed probe must wait longer: {intervals:?}"
    );
    // One episode, so the parent was told once and never that it failed.
    assert_eq!(child_suspensions(h.store.as_ref(), parent).len(), 1);
    assert!(child_failures(h.store.as_ref(), parent).is_empty());
    assert!(!types(h.store.as_ref(), child).contains(&"agent.failed".to_owned()));
}

/// Probes are not the agent failing: a spent provider must not spend the
/// consecutive-failure budget that gives an agent up, or the first ordinary
/// error after a long limit would destroy work the suspension kept.
#[tokio::test]
async fn probes_do_not_spend_the_consecutive_failure_budget() {
    let spent = || ScriptedClient::error(ModelErrorClass::QuotaExhausted, "credit balance too low");
    let h = harness_configured(
        vec![
            spent(),
            spent(),
            spent(),
            // One ordinary retryable error on top of the probes, retried at
            // once so the test does not wait out the default rate-limit bench.
            Err(
                aether_core::model::ModelError::new(ModelErrorClass::RateLimit, "slow down")
                    .with_retry_after(Duration::from_millis(10)),
            ),
            ScriptedClient::tool_call("complete_task", json!({"summary": "survived the limit"})),
        ],
        |_| {},
        |config| {
            config.exhaustion_probe_floor = Duration::from_millis(40);
            config.max_consecutive_failures = 3;
        },
    )
    .await;
    let (parent, child, _) = parent_and_suspending_child(&h, spec("outlast the limit")).await;
    let status = h.wait_until(child, |s| s.is_terminal()).await;
    assert!(
        matches!(status, AgentStatus::Completed { .. }),
        "three probes and a rate limit must not give the agent up: {status:?}"
    );
    assert!(child_failures(h.store.as_ref(), parent).is_empty());
}

/// A provider that says when to come back is believed, within bounds; its
/// prose about resets never is.
#[tokio::test]
async fn retry_after_seeds_the_first_probe() {
    let h = harness_configured(
        vec![Err(aether_core::model::ModelError::new(
            ModelErrorClass::UsageLimit,
            "resets tomorrow 14:00",
        )
        .with_retry_after(Duration::from_secs(2)))],
        |_| {},
        |config| config.exhaustion_probe_floor = Duration::from_millis(10),
    )
    .await;
    let (_, child, _) = parent_and_suspending_child(&h, spec("wait as told")).await;
    let status = h.wait_until(child, blocked_on_exhaustion).await;
    let BlockReason::Exhausted {
        since,
        reported_reset,
        next_probe,
        ..
    } = exhausted_reason(&status)
    else {
        panic!("{status:?}")
    };
    let reported = reported_reset.expect("a reported reset");
    assert!(
        (reported - since).num_milliseconds() == 2_000,
        "reported reset {reported} is not the reported 2s after {since}"
    );
    // Jittered around the reported reset, and far past the 10ms floor.
    assert!(
        (next_probe - reported).num_milliseconds().abs() <= 200,
        "probing at {next_probe}, reported {reported}"
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        blocked_on_exhaustion(&h.store.agent(child).expect("a").expect("e").status),
        "the agent probed before the provider said to"
    );
    assert_eq!(h.client.requests().len(), 1);
}
