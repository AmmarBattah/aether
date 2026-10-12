//! Mid-turn delivery and rejected-message recovery.
use super::*;
#[tokio::test]
async fn a_message_during_the_turn_reaches_the_engine_and_is_answered_in_it() {
    let h = harness(Reply::Accept).await;
    let id = chat_agent(&h);
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    h.runtime
        .send_message(id, vec![ContentBlock::text("hello")], Actor::user())
        .expect("send");
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Running
    })
    .await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    h.runtime
        .send_message(
            id,
            vec![ContentBlock::text("second thought")],
            Actor::user(),
        )
        .expect("send");

    let agent = wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting && a.stats.turns >= 1
    })
    .await;
    assert_eq!(count(h.store.as_ref(), id, "agent.started"), 1, "one turn");
    let inbox_last = h.store.inbox_last_seq(id).expect("seq");
    assert_eq!(agent.inbox_cursor, inbox_last, "everything consumed");
    assert_eq!(
        count(h.store.as_ref(), id, "inbox.consumed"),
        2,
        "the first message at the turn start, the second on acknowledgement"
    );
    let said = spoken(h.store.as_ref(), id);
    let steer = said
        .iter()
        .position(|(role, text)| *role == MessageRole::User && text.contains("second thought"))
        .expect("the steered message is in the transcript");
    let answer = said
        .iter()
        .position(|(role, text)| *role == MessageRole::Assistant && text.contains("heard:"))
        .expect("the engine answered it");
    assert!(steer < answer, "{said:?}");
    assert_eq!(h.prompts.lock().unwrap().len(), 1);
    assert!(
        !h.store
            .events(0, 10_000, &EventFilter::for_agent(id))
            .expect("events")
            .iter()
            .any(|e| matches!(&e.kind, EventKind::AgentFailed { .. })),
    );
}

#[tokio::test]
async fn a_refused_message_is_kept_for_the_next_turn() {
    let h = harness(Reply::Reject).await;
    let id = chat_agent(&h);
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    h.runtime
        .send_message(id, vec![ContentBlock::text("hello")], Actor::user())
        .expect("send");
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Running
    })
    .await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    h.runtime
        .send_message(
            id,
            vec![ContentBlock::text("second thought")],
            Actor::user(),
        )
        .expect("send");

    // The refusal ends the first turn; the unconsumed message starts another.
    let agent = wait_for(h.store.as_ref(), id, |a| {
        matches!(a.status, AgentStatus::Waiting { .. }) && a.stats.turns >= 2
    })
    .await;
    assert_eq!(count(h.store.as_ref(), id, "agent.started"), 2);
    assert_eq!(agent.inbox_cursor, h.store.inbox_last_seq(id).expect("seq"));
    let prompts = h.prompts.lock().unwrap().clone();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert!(prompts[0].contains("hello"), "{prompts:?}");
    assert!(
        prompts[1].contains("second thought"),
        "the refused message opened the next turn: {prompts:?}"
    );
}
