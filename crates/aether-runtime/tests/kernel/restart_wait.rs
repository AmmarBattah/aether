//! Persisted waits must not become model-driven polling after daemon recovery.
use super::*;
use aether_core::agent::WaitCondition;

async fn parked() -> (Harness, AgentId) {
    let h = harness(vec![ScriptedClient::tool_call(
        "wait_for",
        json!({"timeout_seconds": 3600}),
    )])
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("wait for a signal"), Actor::user())
        .unwrap();
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    h.runtime.shutdown().await;
    (h, agent.id)
}

#[tokio::test]
async fn restart_preserves_a_future_wait_without_invoking_the_model() {
    let (h, id) = parked().await;
    let before = h.store.agent(id).unwrap().unwrap();
    let AgentStatus::Waiting { condition } = &before.status else {
        panic!("not waiting")
    };
    assert!(condition.earliest_deadline().unwrap() > now());
    let h2 = restart_over(
        &h,
        vec![ScriptedClient::tool_call(
            "complete_task",
            json!({"summary": "signal received"}),
        )],
    )
    .await;
    // Allow scheduler sweeps, without spending the queued scripted response.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after = h2.store.agent(id).unwrap().unwrap();
    assert_eq!(after.status, before.status);
    assert_eq!(after.stats.turns, before.stats.turns);
    assert_eq!(after.stats.usage, before.stats.usage);
    assert!(h2.client.requests().is_empty());
    h2.runtime
        .send_message(id, vec![ContentBlock::text("continue")], Actor::user())
        .unwrap();
    h2.wait_until(id, |s| s.is_terminal()).await;
    h2.runtime.shutdown().await;
    assert_eq!(h2.client.requests().len(), 1);
    assert_eq!(
        h2.store.agent(id).unwrap().unwrap().status.kind(),
        AgentStatusKind::Completed
    );
    let h3 = restart_over(&h2, vec![]).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        h3.client.requests().is_empty(),
        "a settled wake must not replay"
    );
}

#[tokio::test]
async fn a_deadline_that_expired_offline_wakes_once_on_restart() {
    let (h, id) = parked().await;
    let mut agent = h.store.agent(id).unwrap().unwrap();
    let before_turns = agent.stats.turns;
    // Seed the durable state of a deadline that elapsed while no daemon ran.
    // This avoids a wall-clock race between stopping and starting runtimes.
    agent.status = AgentStatus::Waiting {
        condition: WaitCondition::Deadline {
            at: now() - chrono::Duration::seconds(1),
        },
    };
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(agent)))
        .unwrap();
    let h2 = restart_over(
        &h,
        vec![ScriptedClient::tool_call(
            "complete_task",
            json!({"summary": "deadline handled"}),
        )],
    )
    .await;
    h2.wait_until(id, |s| s.is_terminal()).await;
    h2.runtime.shutdown().await;
    let settled = h2.store.agent(id).unwrap().unwrap();
    assert_eq!(settled.status.kind(), AgentStatusKind::Completed);
    assert_eq!(settled.stats.turns, before_turns + 1);
    assert_eq!(h2.client.requests().len(), 1);
    let h3 = restart_over(&h2, vec![]).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(h3.client.requests().is_empty());
    assert_eq!(
        h3.store.agent(id).unwrap().unwrap().stats.turns,
        settled.stats.turns
    );
}
