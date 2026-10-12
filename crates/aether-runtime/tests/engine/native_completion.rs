use super::*;

#[tokio::test]
async fn a_long_lived_native_agent_keeps_its_task_open_after_a_reply() {
    let h = harness(Reply::OneTurn).await;
    let (agent, task) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                objective: Some("Discuss the task".into()),
                start: true,
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    wait_for(h.store.as_ref(), agent.id, |a| {
        a.stats.turns == 1 && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    assert!(
        !h.store
            .task(task.expect("task").id)
            .unwrap()
            .unwrap()
            .status
            .is_terminal()
    );
    assert_eq!(h.prompts.lock().unwrap().len(), 1);
    assert_eq!(count(h.store.as_ref(), agent.id, "task.completed"), 0);
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn native_final_answer_completes_the_task_without_an_extra_model_turn() {
    for provider in ["claude-code", "codex"] {
        let h = harness(Reply::OneTurn).await;
        let (agent, _) = h
            .runtime
            .create_agent(
                CreateAgentSpec {
                    role: "assistant".into(),
                    model: Some(ModelRef::new(provider, "default")),
                    objective: Some("Finish the task".into()),
                    start: true,
                    config: Some(AgentConfig {
                        long_lived: false,
                        approval: ApprovalPolicy::none(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Actor::user(),
            )
            .expect("create");
        let completed = wait_for(h.store.as_ref(), agent.id, |a| {
            a.status.kind() == AgentStatusKind::Completed
        })
        .await;
        assert_eq!(completed.stats.turns, 1, "{provider}");
        assert_eq!(completed.stats.usage.input_tokens, 70);
        assert_eq!(completed.stats.usage.output_tokens, 17);
        assert_eq!(h.prompts.lock().unwrap().len(), 1);
        assert_eq!(count(h.store.as_ref(), agent.id, "task.completed"), 1);
        h.runtime.shutdown().await;
    }
}
