//! Task state without native instruction replacement.
use super::*;
/// Codex receives task state as input while retaining its native instructions.
#[tokio::test]
async fn codex_receives_task_state_without_replacing_native_instructions() {
    let h = harness(Reply::OneTurn).await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                objective: Some("audit the codex sentinel".into()),
                model: Some(ModelRef::new("codex", "gpt-6.1-sol")),
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
    let started = wait_for(h.store.as_ref(), agent.id, |a| a.stats.turns >= 1).await;
    let task = started.current_task.expect("the agent was given a task");

    let system = h
        .systems
        .lock()
        .unwrap()
        .first()
        .expect("the codex turn ran")
        .clone();
    assert!(system.is_empty(), "native instructions remain untouched");
    assert!(
        !system.contains("Report a completed or failed task exactly once"),
        "the reporting policy is Claude Code only"
    );
    assert!(
        !system.contains("Aether states your assignment"),
        "the provenance sentence is Claude Code only - it is \
         false on Codex, where the assignment stays in the system prompt"
    );

    // Task state travels with the input and keeps its provenance boundary.
    let prompt = h
        .prompts
        .lock()
        .unwrap()
        .first()
        .expect("the codex turn ran")
        .clone();
    assert!(
        prompt.contains("Aether runtime status"),
        "codex still gets its status hoisted into the user turn: {prompt}"
    );
    assert!(
        prompt.contains("--- Aether"),
        "task state must be fenced from the incoming message: {prompt}"
    );
    assert!(prompt.contains("Scope:"));
    assert!(prompt.contains(&task.to_string()));
    assert_eq!(prompt.matches("audit the codex sentinel").count(), 1);
    h.runtime.shutdown().await;
}
