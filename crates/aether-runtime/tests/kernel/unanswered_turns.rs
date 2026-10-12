//! unanswered turns scenarios and fixtures.
use super::*;

/// A reply with no content at all: no text and no tool call.
fn empty_reply() -> aether_model::testing::ScriptedResponse {
    Ok(vec![
        StreamEvent::MessageStart {
            provider_message_id: Some("scripted".into()),
            model: Some("scripted".into()),
        },
        StreamEvent::Stop {
            reason: aether_core::model::StopReason::EndTurn,
        },
        StreamEvent::MessageStop,
    ])
}

fn last_request_mentions(h: &Harness, needle: &str) -> bool {
    h.client
        .requests()
        .last()
        .is_some_and(|r| r.messages.iter().any(|m| m.text().contains(needle)))
}

/// Nudged once, a worker's next turn said nothing. An empty reply is no answer:
/// the worker is woken again and closes the task with its own summary.
#[tokio::test]
async fn an_empty_reply_after_the_nudge_does_not_complete_the_task() {
    let h = harness(vec![
        ScriptedClient::text("I looked into it."),
        empty_reply(),
        ScriptedClient::tool_call("complete_task", json!({"summary": "the real summary"})),
    ])
    .await;
    let (agent, task) = h
        .runtime
        .create_agent(spec("investigate the flake"), Actor::user())
        .expect("create");
    let task = task.expect("task");
    let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert_eq!(
        status,
        AgentStatus::Completed {
            summary: Some("the real summary".into())
        }
    );
    let task = h.store.task(task.id).expect("task").expect("exists");
    assert_eq!(
        task.result().map(|r| r.summary.as_str()),
        Some("the real summary"),
        "the empty turn did not close the task first"
    );
    assert!(
        last_request_mentions(&h, "ended without an answer"),
        "the worker was told its turn said nothing"
    );
    assert_eq!(h.client.remaining(), 0);
}

/// Silent turns are bounded like failed ones: a run as long as the failure limit
/// fails the task, never completes it, and a real answer restarts the count.
#[tokio::test]
async fn silent_turns_in_a_row_fail_the_task_instead_of_completing_it() {
    let h = harness_configured(
        vec![
            empty_reply(),
            empty_reply(),
            ScriptedClient::text("Progress so far."),
            empty_reply(),
            empty_reply(),
            empty_reply(),
        ],
        |_| {},
        |c| c.max_consecutive_failures = 3,
    )
    .await;
    let (agent, task) = h
        .runtime
        .create_agent(spec("investigate the flake"), Actor::user())
        .expect("create");
    let task = task.expect("task");
    let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
    let AgentStatus::Failed { error: reason } = &status else {
        panic!("a run of silent turns fails the agent: {status:?}");
    };
    assert!(reason.contains("3 turns in a row"), "{reason}");
    assert!(reason.contains("without an answer"), "{reason}");
    let task = h.store.task(task.id).expect("task").expect("exists");
    assert_eq!(task.status.kind(), TaskStatusKind::Failed);
    assert_eq!(
        h.client.remaining(),
        0,
        "the answer in between reset the count, so all six turns ran"
    );
}

/// The nudge a worker got on an earlier task does not count for the next one:
/// a first prose reply on the new task is nudged, not taken as its result.
#[tokio::test]
async fn a_nudge_sent_for_an_earlier_task_does_not_count_for_the_next() {
    let h = harness(vec![
        ScriptedClient::text("I looked at the first task."),
        ScriptedClient::tool_call("fail_task", json!({"error": "the index is corrupt"})),
        ScriptedClient::text("Back, and waiting."),
        ScriptedClient::text("Here is what I found on the second task."),
        ScriptedClient::tool_call("complete_task", json!({"summary": "second task done"})),
    ])
    .await;
    let (agent, first) = h
        .runtime
        .create_agent(spec("index the repository"), Actor::user())
        .expect("create");
    let first = first.expect("task");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Failed)
        .await;
    h.runtime.wake_agent(agent.id, Actor::user()).expect("wake");
    h.wait_for("the woken agent took its wake turn", || {
        h.client.remaining() == 2
    })
    .await;
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let second = h
        .runtime
        .create_task(
            aether_runtime::CreateTaskSpec {
                objective: "summarise the second finding".into(),
                owner: Some(agent.id),
                parent_task: None,
                dependencies: Vec::new(),
                priority: Default::default(),
            },
            Actor::user(),
        )
        .expect("assign a second task");
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("please take the new task")],
            Actor::user(),
        )
        .expect("send");
    let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert_eq!(
        status,
        AgentStatus::Completed {
            summary: Some("second task done".into())
        }
    );
    let second = h.store.task(second.id).expect("task").expect("exists");
    assert_eq!(
        second.result().map(|r| r.summary.as_str()),
        Some("second task done"),
        "the first prose reply on the new task was nudged, not taken as its result"
    );
    assert!(last_request_mentions(
        &h,
        &format!("Your task {} is still active", second.id)
    ));
    assert_ne!(first.id, second.id);
}
