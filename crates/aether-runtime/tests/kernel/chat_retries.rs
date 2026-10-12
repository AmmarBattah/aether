//! chat retries scenarios and fixtures.
use super::*;

#[tokio::test]
async fn rate_limit_is_retried_after_backoff() {
    let h = harness(vec![
        ScriptedClient::error(ModelErrorClass::RateLimit, "slow down"),
        ScriptedClient::tool_call("complete_task", json!({"summary": "second try worked"})),
    ])
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("retry me"), Actor::user())
        .expect("create");
    let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    let t = types(h.store.as_ref(), agent.id);
    assert!(t.contains(&"invocation.failed".to_string()));
    assert!(t.contains(&"invocation.completed".to_string()));
    let invocations = h.store.invocations_for_agent(agent.id).expect("inv");
    assert_eq!(invocations.len(), 2);
    assert!(invocations[0].error.is_some());
}

#[tokio::test]
async fn idle_chat_agent_waits_and_answers_messages() {
    let h = harness(vec![ScriptedClient::text("Hello! How can I help?")]).await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                config: Some(AgentConfig {
                    long_lived: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    // No task, no message: parked without spending a turn.
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    assert_eq!(h.client.remaining(), 1);
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("hi there")],
            Actor::user(),
        )
        .expect("send");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if h.client.remaining() == 0 {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    let transcript = h.store.transcript(agent.id, 0).expect("t");
    assert_eq!(transcript.len(), 2);
    assert!(transcript[0].message.text().contains("hi there"));
    assert_eq!(transcript[1].message.text(), "Hello! How can I help?");
    let requests = h.client.requests();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0]
            .system
            .iter()
            .any(|b| b.text.contains("Role: assistant"))
    );
}
