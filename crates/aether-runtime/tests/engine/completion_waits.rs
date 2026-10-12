//! User questions suspend silent-turn recovery.
use super::*;
/// An `ask_user` turn can end with no text and no model output streamed - a
/// no-model turn's shape - yet it waits for the reply and fails nothing.
#[tokio::test]
async fn an_ask_user_turn_waits_for_the_reply_rather_than_counting_as_silent() {
    let (handover, ready) = tokio::sync::oneshot::channel();
    let (finish, finished) = tokio::sync::oneshot::channel();
    let engine = Arc::new(SilentBridgeEngine {
        outcome: EngineOutcome::default(),
        handover: Mutex::new(Some(handover)),
        finish: Mutex::new(Some(finished)),
    });
    let h = harness_with_engine(
        engine,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let (id, task) = engine_worker(&h, "pick a colour");
    let (agent, token) = tokio::time::timeout(Duration::from_secs(20), ready)
        .await
        .expect("the turn started")
        .expect("the turn handed over its bridge");
    let (reply, is_error) = h
        .runtime
        .engine_tool(
            agent,
            &token,
            "ask_user".into(),
            serde_json::json!({"question": "which colour?"}),
        )
        .await
        .expect("answered");
    assert!(!is_error, "{reply}");
    finish.send(()).expect("the engine is still running");
    let agent = wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    assert_eq!(
        agent.status,
        AgentStatus::Waiting {
            condition: aether_core::agent::WaitCondition::Inbox
        }
    );
    let task = h.store.task(task).expect("task").expect("exists");
    assert!(!task.status.is_terminal(), "{:?}", task.status);
    assert_eq!(count(h.store.as_ref(), id, "question.asked"), 1);
    assert_eq!(
        failed_invocation_errors(h.store.as_ref(), id),
        Vec::<String>::new()
    );
    let notices: Vec<_> = h
        .store
        .inbox(id, 0)
        .expect("inbox")
        .into_iter()
        .filter(|i| matches!(&i.kind, aether_core::inbox::InboxKind::Wake { .. }))
        .collect();
    assert!(notices.is_empty(), "{notices:?}");
    h.runtime.shutdown().await;
}
