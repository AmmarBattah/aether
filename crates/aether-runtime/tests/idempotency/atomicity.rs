use super::*;

#[tokio::test]
async fn a_keyed_send_message_commits_its_record_with_the_delivery() {
    let (fx, log) = logged().await;
    let agent = an_agent(&fx).await;
    log.take();

    let message = fx
        .runtime
        .send_message_keyed(
            agent,
            text("ship it"),
            Actor::user(),
            Some(message_submission(
                "atomic-message",
                &agent.to_string(),
                "ship it",
            )),
        )
        .expect("delivery");

    assert_committed_together(&log.take(), Work::Delivery(message), "send_message");
}

#[tokio::test]
async fn a_keyed_fan_out_commits_its_record_with_the_deliveries() {
    let (fx, log) = logged().await;
    let agent = an_agent(&fx).await;
    log.take();

    let key = Submission::new("atomic-fan", &json!({"cmd": "fan_out_message"})).expect("key");
    let deliveries = fx
        .runtime
        .fan_out_message_keyed(agent, text("all hands"), Actor::user(), Some(key))
        .expect("fan out");

    assert_eq!(deliveries.len(), 1);
    assert_committed_together(
        &log.take(),
        Work::Delivery(deliveries[0].message),
        "fan_out_message",
    );
}

#[tokio::test]
async fn a_keyed_trigger_commits_its_record_with_the_inbox_item() {
    let (fx, log) = logged().await;
    let agent = an_agent(&fx).await;
    log.take();

    let key = Submission::new("atomic-trigger", &json!({"cmd": "trigger"})).expect("key");
    fx.runtime
        .trigger_agent_keyed(
            agent,
            "ci".into(),
            json!({"run": 7}),
            Actor::user(),
            Some(key),
        )
        .expect("trigger");

    assert_committed_together(&log.take(), Work::Trigger(agent), "trigger");
}

#[tokio::test]
async fn a_keyed_create_task_commits_its_record_with_the_task() {
    let (fx, log) = logged().await;
    log.take();

    let key = Submission::new("atomic-task", &json!({"cmd": "create_task"})).expect("key");
    let task = fx
        .runtime
        .create_task_keyed(task_spec(), Actor::user(), Some(key))
        .expect("task");

    assert_committed_together(&log.take(), Work::Task(task.id), "create_task");
}

#[tokio::test]
async fn a_keyed_create_agent_commits_its_record_with_the_agent() {
    let (fx, log) = logged().await;
    log.take();

    let (agent, _) = fx
        .runtime
        .create_agent_keyed(
            agent_spec("worker"),
            Actor::user(),
            Some(agent_submission("atomic-agent", "worker")),
        )
        .expect("creation");

    assert_committed_together(&log.take(), Work::Agent(agent.id), "create_agent");
}

#[tokio::test]
async fn a_keyed_create_schedule_commits_its_record_with_the_timer() {
    let (fx, log) = logged().await;
    let agent = an_agent(&fx).await;
    log.take();

    let key = Submission::new("atomic-sched", &json!({"cmd": "create_schedule"})).expect("key");
    let job = fx
        .runtime
        .create_schedule_keyed(schedule_spec(agent), Actor::user(), Some(key))
        .expect("schedule");

    assert_committed_together(&log.take(), Work::Schedule(job.id), "create_schedule");
}

/// The other direction of the same guarantee: the record cannot be dropped and
/// leave the work behind. With every commit carrying a record refused, no
/// keyed command may land anything at all.
#[tokio::test]
async fn work_whose_record_cannot_be_committed_does_not_land() {
    let log = CommitLog::open(Refusal::Records);
    let fx = over(log as Arc<dyn Store>).await;
    // Unkeyed, so it carries no record and the store still takes it.
    let agent = an_agent(&fx).await;

    let refused = fx.runtime.send_message_keyed(
        agent,
        text("ship it"),
        Actor::user(),
        Some(message_submission(
            "unrecordable",
            &agent.to_string(),
            "ship it",
        )),
    );
    assert!(refused.is_err(), "an unrecordable submission is refused");
    assert!(
        fx.store.inbox(agent, 0).expect("inbox").is_empty(),
        "the delivery landed without its record"
    );

    let refused = fx.runtime.create_agent_keyed(
        agent_spec("worker"),
        Actor::user(),
        Some(agent_submission("unrecordable-agent", "worker")),
    );
    assert!(
        refused.is_err(),
        "an unrecordable creation is refused, not replayed into existence"
    );
    assert_eq!(
        fx.store
            .agents(&AgentFilter::default())
            .expect("agents")
            .len(),
        1,
        "only the unkeyed agent exists: the creation landed without its record"
    );
}
