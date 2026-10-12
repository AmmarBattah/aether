use super::*;

/// Two retries racing: the key is checked and recorded inside one hold on the
/// commit lock, so exactly one of them does the work.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_identical_submissions_create_one() {
    let fx = fixture().await;
    let agent = an_agent(&fx).await;
    let submission = message_submission("outbox-6", &agent.to_string(), "ship it");

    let start = Arc::new(std::sync::Barrier::new(8));
    let mut threads = Vec::new();
    for _ in 0..8 {
        let runtime = fx.runtime.clone();
        let submission = submission.clone();
        let start = start.clone();
        threads.push(std::thread::spawn(move || {
            start.wait();
            runtime.send_message_keyed(agent, text("ship it"), Actor::user(), Some(submission))
        }));
    }
    let answers: Vec<_> = threads
        .into_iter()
        .map(|t| t.join().expect("thread").expect("delivery"))
        .collect();

    let first = answers[0];
    assert!(
        answers.iter().all(|id| *id == first),
        "every racing submission is answered with one identity: {answers:?}"
    );
    assert_eq!(
        fx.store.inbox(agent, 0).expect("inbox").len(),
        1,
        "the race delivered one message"
    );
}

/// `create_agent` is the only keyed command that checks the key twice: an
/// unheld preflight, so deriving the repository id does not run `git` under
/// the commit lock, and an authoritative re-check under the hold. Only the
/// held one decides a race - eight retries all pass the preflight together.
///
/// Every answer must be `Ok`. A racer that got as far as the store and was
/// stopped there by the primary key would surface a raw sqlite constraint
/// failure to a client that did nothing wrong, so a rejected commit is a
/// failure of this guarantee even when the agent count comes out right.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_identical_creations_all_answer_with_one_agent() {
    let log = CommitLog::open(Refusal::None);
    let fx = over(log.clone() as Arc<dyn Store>).await;
    let submission = agent_submission("outbox-race", "worker");

    let start = Arc::new(std::sync::Barrier::new(8));
    let mut threads = Vec::new();
    for _ in 0..8 {
        let runtime = fx.runtime.clone();
        let submission = submission.clone();
        let start = start.clone();
        threads.push(std::thread::spawn(move || {
            start.wait();
            runtime.create_agent_keyed(agent_spec("worker"), Actor::user(), Some(submission))
        }));
    }
    let answers: Vec<_> = threads
        .into_iter()
        .map(|t| t.join().expect("thread"))
        .collect();

    for answer in &answers {
        assert!(
            answer.is_ok(),
            "a racing retry is replayed, never refused: {:?}",
            answer.as_ref().err()
        );
    }
    let ids: Vec<_> = answers
        .iter()
        .map(|a| a.as_ref().expect("ok").0.id)
        .collect();
    assert!(
        ids.iter().all(|id| *id == ids[0]),
        "every racing creation names one agent: {ids:?}"
    );
    assert_eq!(
        fx.store
            .agents(&AgentFilter::default())
            .expect("agents")
            .len(),
        1,
        "the race created one agent"
    );
    assert_eq!(
        log.rejected(),
        0,
        "a racer reached the store and was stopped by the primary key instead of by the held \
         re-check"
    );
}
