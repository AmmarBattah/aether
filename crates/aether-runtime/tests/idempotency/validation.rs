use super::*;

#[tokio::test]
async fn a_key_reused_for_different_work_is_refused_by_name() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    fx.runtime
        .send_message_keyed(
            agent,
            text("ship it"),
            Actor::user(),
            Some(message_submission(
                "outbox-3",
                &agent.to_string(),
                "ship it",
            )),
        )
        .expect("first delivery");

    let err = fx
        .runtime
        .send_message_keyed(
            agent,
            text("hold on"),
            Actor::user(),
            Some(message_submission(
                "outbox-3",
                &agent.to_string(),
                "hold on",
            )),
        )
        .expect_err("a key spent on other work is refused");
    let message = err.to_string();
    assert!(message.contains("outbox-3"), "{message}");
    assert!(message.contains("message"), "{message}");
    assert_eq!(
        fx.store.inbox(agent, 0).expect("inbox").len(),
        1,
        "the refused submission delivered nothing"
    );
}

/// A record written under a different fingerprint scheme cannot be compared
/// against this daemon's digest. Refused by name - neither replayed (the two
/// requests may differ) nor called a different request (they may not).
#[tokio::test]
async fn a_record_from_another_fingerprint_scheme_is_refused_not_replayed() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    let submission = message_submission("outbox-9", &agent.to_string(), "ship it");
    let mut record = submission.record(
        now(),
        aether_core::idempotency::SubmissionOutcome::Message {
            message: MessageId::new(),
        },
    );
    record.scheme = aether_core::idempotency::FINGERPRINT_SCHEME + 1;
    fx.store
        .commit(Commit::new().mutate(Mutation::RecordSubmission(record)))
        .expect("record from an older daemon");

    let err = fx
        .runtime
        .send_message_keyed(agent, text("ship it"), Actor::user(), Some(submission))
        .expect_err("a record this daemon cannot read is not a replay");
    let message = err.to_string();
    assert!(message.contains("outbox-9"), "{message}");
    assert!(message.contains("scheme"), "{message}");
    assert!(
        !message.contains("a different request"),
        "the client is not blamed for the daemon's upgrade: {message}"
    );
    assert!(
        fx.store.inbox(agent, 0).expect("inbox").is_empty(),
        "the refused submission delivered nothing"
    );
}

/// A submission that failed created nothing, so it recorded nothing and its
/// key is still spendable. Proven by spending it: the second call must
/// deliver, not replay and not conflict.
///
/// The two calls name different agents - the first has to be terminal to fail
/// and the second live to succeed - so their fingerprints differ. That makes
/// the assertion stronger, not weaker: a record left by the failure would
/// answer with a fingerprint conflict rather than a delivery.
#[tokio::test]
async fn a_key_a_failed_submission_used_is_still_spendable() {
    let fx = fixture().await;
    let terminal = an_agent(&fx).await;
    let live = an_agent(&fx).await;
    fx.runtime
        .cancel_agent(terminal, Some("done".into()), Actor::user())
        .expect("cancel");

    let refused = fx.runtime.send_message_keyed(
        terminal,
        text("ship it"),
        Actor::user(),
        Some(message_submission(
            "outbox-8",
            &terminal.to_string(),
            "ship it",
        )),
    );
    assert!(
        refused.is_err(),
        "a message to a terminal agent must be refused for this to mean anything"
    );
    let key = message_submission("outbox-8", &terminal.to_string(), "ship it").key;
    assert!(
        fx.store.submission(&key).expect("read").is_none(),
        "the refused submission recorded nothing"
    );

    fx.runtime
        .send_message_keyed(
            live,
            text("ship it"),
            Actor::user(),
            Some(message_submission("outbox-8", &live.to_string(), "ship it")),
        )
        .expect("the key was never spent, so it is not a retry");
    assert_eq!(
        fx.store.inbox(live, 0).expect("inbox").len(),
        1,
        "the second submission delivered rather than replayed"
    );
}

/// The same guarantee as above, for `create_agent`: a keyed creation refused
/// under a cancelled parent records nothing, so its key is still spendable -
/// and spending it creates fresh work rather than replaying the refusal.
#[tokio::test]
async fn a_key_a_failed_agent_creation_used_is_still_spendable() {
    let fx = fixture().await;
    let cancelled_parent = an_agent(&fx).await;
    fx.runtime
        .cancel_agent(cancelled_parent, Some("done".into()), Actor::user())
        .expect("cancel");

    let refused = fx.runtime.create_agent_keyed(
        CreateAgentSpec {
            parent: Some(cancelled_parent),
            ..agent_spec("worker")
        },
        Actor::user(),
        Some(agent_submission("outbox-agent-8", "worker")),
    );
    assert!(
        refused.is_err(),
        "creation under a cancelled parent must be refused for this to mean anything"
    );
    let key = agent_submission("outbox-agent-8", "worker").key;
    assert!(
        fx.store.submission(&key).expect("read").is_none(),
        "the refused submission recorded nothing"
    );

    let (agent, _) = fx
        .runtime
        .create_agent_keyed(
            agent_spec("worker"),
            Actor::user(),
            Some(agent_submission("outbox-agent-8", "worker")),
        )
        .expect("the key was never spent, so it is not a retry");
    assert_eq!(
        fx.store.agent(agent.id).expect("read").map(|a| a.id),
        Some(agent.id),
        "the second submission created fresh work rather than replaying the refusal"
    );
}

/// A key is opaque, but not unbounded and not something that can smuggle
/// whitespace or control characters into a log line.
#[test]
fn a_malformed_key_is_refused_before_any_work() {
    let request = json!({"cmd": "send_message"});
    for bad in ["", " ", "with space", "line\nbreak", &"x".repeat(201)] {
        assert!(
            Submission::new(bad, &request).is_err(),
            "{bad:?} must be refused"
        );
    }
    assert!(Submission::new(&"x".repeat(200), &request).is_ok());
}

/// The fingerprint is over the work asked for, not over the frame's byte
/// order and not over the key itself.
#[test]
fn the_fingerprint_normalizes_the_request_and_ignores_the_key() {
    let ordered = json!({"cmd": "send_message", "to": "a", "content": "hi"});
    let shuffled = json!({"content": "hi", "cmd": "send_message", "to": "a"});
    let keyed = json!({"cmd": "send_message", "to": "a", "content": "hi", "idempotency_key": "k"});
    let different = json!({"cmd": "send_message", "to": "a", "content": "ho"});

    let of = |v: &serde_json::Value| Submission::new("k", v).expect("key").fingerprint;
    assert_eq!(of(&ordered), of(&shuffled));
    assert_eq!(of(&ordered), of(&keyed));
    assert_ne!(of(&ordered), of(&different));
    // Two keys on one request are two submissions, not one.
    assert_ne!(
        Submission::new("k1", &ordered).expect("key").key,
        Submission::new("k2", &ordered).expect("key").key
    );
}
