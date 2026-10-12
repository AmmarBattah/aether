//! questions scenarios and fixtures.
use super::*;

/// `ask_user` must actually reach the operator: the question is recorded as
/// its own durable event (so a client can show it), the tool's reply is
/// truthful about that, and the operator's ordinary reply - the same
/// `send_message` path any message uses - wakes the agent with the answer
/// in its transcript.
#[tokio::test]
async fn ask_user_records_the_question_and_wakes_on_the_operators_reply() {
    let h = harness(vec![
        ScriptedClient::tool_call("ask_user", json!({"question": "which colour?"})),
        ScriptedClient::text("thanks - blue it is"),
    ])
    .await;
    let (agent, task) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                config: Some(AgentConfig {
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    assert!(task.is_none(), "no objective means no task was assigned");

    h.runtime
        .send_message(agent.id, vec![ContentBlock::text("go")], Actor::user())
        .expect("send");

    // Wait for the tool call to actually settle (both events recorded), not
    // just for a `Waiting` status: a fresh agent with no objective already
    // sits in `Waiting(Inbox)` before this message is even processed, so
    // that status alone cannot distinguish "hasn't started" from "asked and
    // parked again".
    let events = wait_until_events(h.store.as_ref(), agent.id, |events| {
        events.iter().any(|e| e.type_name() == "question.asked")
            && events.iter().any(|e| e.type_name() == "tool.succeeded")
    })
    .await;
    let question = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::QuestionAsked { question } => Some(question.clone()),
            _ => None,
        })
        .expect("question.asked recorded");
    assert_eq!(question, "which colour?");

    let call_id = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolRequested { call, .. } if call.name == "ask_user" => Some(call.id),
            _ => None,
        })
        .expect("ask_user was requested");
    let outcome = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolSucceeded { call, outcome } if *call == call_id => Some(outcome.clone()),
            _ => None,
        })
        .expect("ask_user settled");
    assert!(
        !outcome.content.to_lowercase().contains("delivered"),
        "the reply must not claim a delivery it never performed: {outcome:?}"
    );
    assert!(
        outcome.content.to_lowercase().contains("recorded")
            && outcome.content.to_lowercase().contains("operator"),
        "the reply should describe what actually happened: {outcome:?}"
    );

    // Only now - with the question and its tool outcome both confirmed
    // recorded - does a `Waiting(Inbox)` status actually mean the turn
    // parked *because of this `ask_user` call* rather than the agent's
    // pre-existing idle state.
    let waiting = h
        .wait_until(agent.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    assert!(
        matches!(
            waiting,
            AgentStatus::Waiting {
                condition: aether_core::agent::WaitCondition::Inbox
            }
        ),
        "{waiting:?}"
    );

    // The operator's reply is an ordinary message to this agent - the same
    // path that already satisfies `WaitCondition::Inbox` for anything else.
    h.runtime
        .send_message(agent.id, vec![ContentBlock::text("blue")], Actor::user())
        .expect("send");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let current = h.store.agent(agent.id).expect("agent").expect("exists");
        if current.stats.turns >= 2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the agent never woke on the operator's reply"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        h.client
            .requests()
            .iter()
            .any(|r| format!("{r:?}").contains("blue")),
        "the answer reached the model's context: {:?}",
        h.client.requests()
    );
}

/// A turn that asks the operator something and completes the task in
/// the same batch used to record a question nobody would ever wait on - the
/// agent ended, the TUI still showed the question as open, and the reply was
/// refused by `send_message`. The completion is now refused instead, so the
/// question keeps its promise: the agent parks on the answer.
#[tokio::test]
async fn completing_a_task_after_asking_in_the_same_turn_is_refused() {
    let h = harness(vec![ScriptedClient::tool_calls(
        &[
            ("ask_user", json!({"question": "which colour?"})),
            ("complete_task", json!({"summary": "picked one anyway"})),
        ],
        None,
    )])
    .await;
    let (agent, task) = h
        .runtime
        .create_agent(spec("paint it"), Actor::user())
        .expect("create");
    let task = task.expect("task");

    let events = wait_until_events(h.store.as_ref(), agent.id, |events| {
        events
            .iter()
            .filter(|e| e.type_name() == "tool.succeeded")
            .count()
            >= 2
    })
    .await;

    let outcome = settled_outcome(&events, "complete_task");
    assert!(
        outcome.is_error,
        "completing while a question is outstanding must be refused: {outcome:?}"
    );
    assert!(
        outcome.content.contains("which colour?"),
        "the refusal names the question it is protecting: {outcome:?}"
    );
    assert!(
        !events.iter().any(|e| e.type_name() == "task.completed"),
        "the task must not have ended under the open question"
    );
    assert_eq!(
        h.store
            .task(task.id)
            .expect("task")
            .expect("exists")
            .status
            .kind(),
        TaskStatusKind::Active,
        "the task is still the one the answer will finish"
    );

    // The question stands, and so does the park it promised: the operator's
    // reply reaches an agent that is still waiting for it.
    assert!(
        events.iter().any(|e| e.type_name() == "question.asked"),
        "the question itself is untouched"
    );
    let waiting = h
        .wait_until(agent.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    assert!(
        matches!(
            waiting,
            AgentStatus::Waiting {
                condition: aether_core::agent::WaitCondition::Inbox
            }
        ),
        "{waiting:?}"
    );
    h.runtime
        .send_message(agent.id, vec![ContentBlock::text("blue")], Actor::user())
        .expect("the operator's reply still reaches the asker");
}

/// A `long_lived` agent's settle parks it on `Inbox` instead of ending
/// it, so the question's own park is not clobbered - the completion, the
/// question and the wait all stand together.
#[tokio::test]
async fn completing_a_task_after_asking_is_allowed_for_a_long_lived_agent() {
    let h = harness(vec![ScriptedClient::tool_calls(
        &[
            ("ask_user", json!({"question": "which colour?"})),
            ("complete_task", json!({"summary": "picked blue"})),
        ],
        None,
    )])
    .await;
    let mut s = spec("paint it");
    s.config.as_mut().expect("config").long_lived = true;
    let (agent, task) = h.runtime.create_agent(s, Actor::user()).expect("create");
    let task = task.expect("task");

    let events = wait_until_events(h.store.as_ref(), agent.id, |events| {
        events
            .iter()
            .filter(|e| e.type_name() == "tool.succeeded")
            .count()
            >= 2
    })
    .await;

    let outcome = settled_outcome(&events, "complete_task");
    assert!(
        !outcome.is_error,
        "a long_lived agent's settle does not end it, so the question's park \
         still holds and the completion is not refused: {outcome:?}"
    );
    assert!(
        events.iter().any(|e| e.type_name() == "task.completed"),
        "the task really settled"
    );
    assert_eq!(
        h.store
            .task(task.id)
            .expect("task")
            .expect("exists")
            .status
            .kind(),
        TaskStatusKind::Completed,
        "unlike the non-long-lived case, the task itself is done"
    );
    assert!(
        events.iter().any(|e| e.type_name() == "question.asked"),
        "the question survives the settle"
    );

    let waiting = h
        .wait_until(agent.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    let AgentStatus::Waiting { condition } = &waiting else {
        unreachable!("{waiting:?}")
    };
    assert!(
        waits_on_inbox(condition),
        "the agent parks on the reply rather than ending: {waiting:?}"
    );
    h.runtime
        .send_message(agent.id, vec![ContentBlock::text("blue")], Actor::user())
        .expect("the question is still answerable after the settle");
}

/// The mirror of the completion case: `fail_task` gets the same merge.
#[tokio::test]
async fn failing_a_task_after_asking_is_allowed_for_a_long_lived_agent() {
    let h = harness(vec![ScriptedClient::tool_calls(
        &[
            ("ask_user", json!({"question": "retry or give up?"})),
            ("fail_task", json!({"error": "could not paint it"})),
        ],
        None,
    )])
    .await;
    let mut s = spec("paint it");
    s.config.as_mut().expect("config").long_lived = true;
    let (agent, task) = h.runtime.create_agent(s, Actor::user()).expect("create");
    let task = task.expect("task");

    let events = wait_until_events(h.store.as_ref(), agent.id, |events| {
        events
            .iter()
            .filter(|e| e.type_name() == "tool.succeeded")
            .count()
            >= 2
    })
    .await;

    let outcome = settled_outcome(&events, "fail_task");
    assert!(
        !outcome.is_error,
        "the settle is not refused for a long_lived agent: {outcome:?}"
    );
    assert_eq!(
        h.store
            .task(task.id)
            .expect("task")
            .expect("exists")
            .status
            .kind(),
        TaskStatusKind::Failed,
        "the task really failed"
    );
    let waiting = h
        .wait_until(agent.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    let AgentStatus::Waiting { condition } = &waiting else {
        unreachable!("{waiting:?}")
    };
    assert!(
        waits_on_inbox(condition),
        "the agent parks on the reply rather than ending: {waiting:?}"
    );
}

/// The reverse order of the tests above - a `long_lived` settle
/// runs *first* in the batch, then a question. `apply_action`'s own
/// `AskUser` guard already admits this (it only refuses a genuine terminal
/// `Completed`/`Failed`, which a long_lived settle never records), but the
/// engine-bridge park gate refused every question once anything had parked
/// the turn; this pins the batch path's own correct behaviour so the two
/// stay consistent.
#[tokio::test]
async fn asking_after_completing_in_the_same_batch_is_allowed_for_a_long_lived_agent() {
    let h = harness(vec![ScriptedClient::tool_calls(
        &[
            ("complete_task", json!({"summary": "picked blue"})),
            ("ask_user", json!({"question": "which shade next?"})),
        ],
        None,
    )])
    .await;
    let mut s = spec("paint it");
    s.config.as_mut().expect("config").long_lived = true;
    let (agent, task) = h.runtime.create_agent(s, Actor::user()).expect("create");
    let task = task.expect("task");

    let events = wait_until_events(h.store.as_ref(), agent.id, |events| {
        events
            .iter()
            .filter(|e| e.type_name() == "tool.succeeded")
            .count()
            >= 2
    })
    .await;

    let outcome = settled_outcome(&events, "ask_user");
    assert!(
        !outcome.is_error,
        "the first question after a long_lived settle must be admitted, not treated \
         as a second decision: {outcome:?}"
    );
    assert_eq!(
        h.store
            .task(task.id)
            .expect("task")
            .expect("exists")
            .status
            .kind(),
        TaskStatusKind::Completed,
        "the settle really landed"
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| e.type_name() == "question.asked")
            .count(),
        1,
        "the question was actually recorded"
    );

    let waiting = h
        .wait_until(agent.id, |s| matches!(s, AgentStatus::Waiting { .. }))
        .await;
    let AgentStatus::Waiting { condition } = &waiting else {
        unreachable!("{waiting:?}")
    };
    assert!(
        waits_on_inbox(condition),
        "the agent parks on the reply rather than ending: {waiting:?}"
    );
    h.runtime
        .send_message(agent.id, vec![ContentBlock::text("blue")], Actor::user())
        .expect("the question is still answerable after the settle");
}
