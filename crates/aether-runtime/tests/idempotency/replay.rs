use super::*;

#[tokio::test]
async fn a_retried_message_is_delivered_once() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    let submission = message_submission("outbox-1", &agent.to_string(), "ship it");

    let first = fx
        .runtime
        .send_message_keyed(
            agent,
            text("ship it"),
            Actor::user(),
            Some(submission.clone()),
        )
        .expect("first delivery");
    // The client never saw that answer, so it sends the same frame again.
    let retry = fx
        .runtime
        .send_message_keyed(agent, text("ship it"), Actor::user(), Some(submission))
        .expect("retry");

    assert_eq!(first, retry, "a retry is answered with the first identity");
    assert_eq!(
        fx.store.inbox(agent, 0).expect("inbox").len(),
        1,
        "the message reached the inbox once"
    );
}

/// The case that makes the key necessary rather than merely convenient: by
/// the time the retry arrives the agent can have finished, and refusing it
/// then would fail a request that already succeeded.
#[tokio::test]
async fn a_retry_is_replayed_even_once_the_agent_is_terminal() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    let submission = message_submission("outbox-2", &agent.to_string(), "ship it");
    let first = fx
        .runtime
        .send_message_keyed(
            agent,
            text("ship it"),
            Actor::user(),
            Some(submission.clone()),
        )
        .expect("first delivery");

    fx.runtime
        .cancel_agent(agent, Some("done".into()), Actor::user())
        .expect("cancel");
    assert!(
        fx.store
            .agent(agent)
            .expect("agent")
            .expect("exists")
            .status
            .is_terminal(),
        "the fixture must leave the agent terminal for this to mean anything"
    );

    let retry = fx
        .runtime
        .send_message_keyed(agent, text("ship it"), Actor::user(), Some(submission))
        .expect("a retry of a landed submission is not refused");
    assert_eq!(first, retry);
    // An unkeyed send to the same agent is still refused: the replay answers
    // the recorded submission, it does not reopen the agent.
    assert!(
        fx.runtime
            .send_message(agent, text("again"), Actor::user())
            .is_err()
    );
}

#[tokio::test]
async fn a_retried_creation_returns_the_first_agent_and_its_task() {
    let fx = fixture().await;
    let spec = CreateAgentSpec {
        role: "worker".into(),
        objective: Some("ship it".into()),
        start: false,
        ..Default::default()
    };
    let submission = agent_submission("outbox-4", "worker");

    let (first, first_task) = fx
        .runtime
        .create_agent_keyed(spec.clone(), Actor::user(), Some(submission.clone()))
        .expect("first creation");
    let (retry, retry_task) = fx
        .runtime
        .create_agent_keyed(spec, Actor::user(), Some(submission))
        .expect("retry");

    assert_eq!(first.id, retry.id);
    assert_eq!(
        first_task.map(|t| t.id),
        retry_task.map(|t| t.id),
        "the retry names the task the submission created"
    );
    assert_eq!(
        fx.store
            .agents(&AgentFilter::default())
            .expect("agents")
            .len(),
        1,
        "one submission, one agent"
    );
}

#[tokio::test]
async fn a_retried_task_creation_lands_once() {
    let fx = fixture().await;
    let key = Submission::new("outbox-task", &json!({"cmd": "create_task"})).expect("key");

    let first = fx
        .runtime
        .create_task_keyed(task_spec(), Actor::user(), Some(key.clone()))
        .expect("task");
    let retry = fx
        .runtime
        .create_task_keyed(task_spec(), Actor::user(), Some(key))
        .expect("task retry");

    assert_eq!(first.id, retry.id);
    assert_eq!(
        fx.store.tasks(&TaskFilter::default()).expect("tasks").len(),
        1,
        "one submission, one task"
    );
}

#[tokio::test]
async fn a_retried_trigger_lands_once() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    let key = Submission::new("outbox-trig", &json!({"cmd": "trigger"})).expect("key");

    for _ in 0..2 {
        fx.runtime
            .trigger_agent_keyed(
                agent,
                "ci".into(),
                json!({"run": 1}),
                Actor::user(),
                Some(key.clone()),
            )
            .expect("trigger");
    }

    assert_eq!(
        fx.store.inbox(agent, 0).expect("inbox").len(),
        1,
        "the retried trigger woke the agent once"
    );
}

#[tokio::test]
async fn a_retried_fan_out_lands_once() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    let key = Submission::new("outbox-fan", &json!({"cmd": "fan_out_message"})).expect("key");

    let fanned = fx
        .runtime
        .fan_out_message_keyed(agent, text("all hands"), Actor::user(), Some(key.clone()))
        .expect("fan out");
    let again = fx
        .runtime
        .fan_out_message_keyed(agent, text("all hands"), Actor::user(), Some(key))
        .expect("fan out retry");

    assert_eq!(fanned, again);
    assert_eq!(
        fx.store.inbox(agent, 0).expect("inbox").len(),
        1,
        "the retried fan-out delivered once"
    );
}

#[tokio::test]
async fn a_retried_schedule_creation_lands_once() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    let key = Submission::new("outbox-sched", &json!({"cmd": "create_schedule"})).expect("key");

    let job = fx
        .runtime
        .create_schedule_keyed(schedule_spec(agent), Actor::user(), Some(key.clone()))
        .expect("schedule");
    let again = fx
        .runtime
        .create_schedule_keyed(schedule_spec(agent), Actor::user(), Some(key))
        .expect("schedule retry");

    assert_eq!(job.id, again.id);
    assert_eq!(
        fx.store.schedules(None).expect("schedules").len(),
        1,
        "one submission, one timer"
    );
}

/// ARCHITECTURE §13.6: a retry reports the subtree the original fan-out
/// reached, not the subtree as it is now. A retry that re-walked the tree
/// would answer with a delivery the client's original never got.
#[tokio::test]
async fn a_fan_out_retry_reports_the_subtree_the_original_reached() {
    let fx = fixture().await;
    let root = an_agent(&fx).await;
    let key = Submission::new("outbox-9", &json!({"cmd": "fan_out_message"})).expect("key");

    let first = fx
        .runtime
        .fan_out_message_keyed(root, text("all hands"), Actor::user(), Some(key.clone()))
        .expect("fan out");
    assert_eq!(
        first.len(),
        1,
        "the subtree is the root alone at this point"
    );

    let child = fx
        .runtime
        .create_agent(
            CreateAgentSpec {
                parent: Some(root),
                ..agent_spec("child")
            },
            Actor::user(),
        )
        .expect("child")
        .0
        .id;

    let retry = fx
        .runtime
        .fan_out_message_keyed(root, text("all hands"), Actor::user(), Some(key))
        .expect("retry");
    assert_eq!(
        retry, first,
        "the retry replays the original deliveries rather than re-walking the tree"
    );
    assert!(
        fx.store.inbox(child, 0).expect("inbox").is_empty(),
        "an agent born after the fan-out heard nothing"
    );

    // The subtree really did grow, so the retry's answer was history and not
    // a fresh walk that happened to agree.
    let fresh = fx
        .runtime
        .fan_out_message(root, text("again"), Actor::user())
        .expect("fan out");
    assert_eq!(
        fresh.len(),
        2,
        "the child is a live target, so a re-walk would have reached it"
    );
}

/// The store is the record, not the process: a daemon restarted between the
/// submission and its retry answers the retry the same way.
#[tokio::test]
async fn a_retry_after_a_restart_still_replays() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("aether.sqlite3");

    let store: Arc<dyn Store> = Arc::new(SqliteStore::open(&path).expect("store"));
    let fx = over(store).await;
    let agent = an_agent(&fx).await;
    let submission = message_submission("outbox-5", &agent.to_string(), "ship it");
    let first = fx
        .runtime
        .send_message_keyed(
            agent,
            text("ship it"),
            Actor::user(),
            Some(submission.clone()),
        )
        .expect("first delivery");
    fx.runtime.shutdown().await;
    drop(fx);

    let reopened: Arc<dyn Store> = Arc::new(SqliteStore::open(&path).expect("reopen"));
    let restarted = over(reopened).await;
    let retry = restarted
        .runtime
        .send_message_keyed(agent, text("ship it"), Actor::user(), Some(submission))
        .expect("retry after restart");
    assert_eq!(first, retry);
    assert_eq!(
        restarted.store.inbox(agent, 0).expect("inbox").len(),
        1,
        "the restart did not deliver a second copy"
    );
}

#[tokio::test]
async fn now_is_recorded_with_the_submission() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    let before = now();
    let submission = message_submission("outbox-7", &agent.to_string(), "ship it");
    fx.runtime
        .send_message_keyed(
            agent,
            text("ship it"),
            Actor::user(),
            Some(submission.clone()),
        )
        .expect("delivery");
    let record = fx
        .store
        .submission(&submission.key)
        .expect("read")
        .expect("recorded");
    assert!(record.at >= before, "the record is stamped when it lands");
    assert_eq!(record.fingerprint, submission.fingerprint);
}
