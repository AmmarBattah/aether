//! completion artifacts scenarios and fixtures.
use super::*;

/// `complete_task.artifacts` must reject a dangling artifact id - one
/// that was never stored - not silently record it on the task result.
#[tokio::test]
async fn complete_task_rejects_a_dangling_artifact_id() {
    use aether_core::task::TaskStatus;

    let dangling = ArtifactId::new();
    let h = harness(vec![
        ScriptedClient::tool_call(
            "complete_task",
            json!({"summary": "done", "artifacts": [dangling.to_string()]}),
        ),
        // The rejection does not end the turn, so the agent gets another
        // chance - and a retry with no artifacts is valid, unaffected input.
        ScriptedClient::tool_call("complete_task", json!({"summary": "done, retried"})),
    ])
    .await;
    let (owner, task) = h
        .runtime
        .create_agent(spec("write something"), Actor::user())
        .expect("owner");
    let task = task.expect("objective creates a task");
    h.wait_until(owner.id, |s| s.is_terminal()).await;

    let rejection = first_artifact_rejection(&h, owner.id);
    let text = rejection.expect("a dangling artifact id was accepted, not rejected");
    assert!(text.contains(&dangling.to_string()), "{text}");

    let settled = h.store.task(task.id).expect("task").expect("exists");
    match settled.status {
        TaskStatus::Completed { result } => {
            assert_eq!(result.summary, "done, retried");
            assert!(
                result.artifacts.is_empty(),
                "the rejected artifact must never reach the settled task"
            );
        }
        other => panic!("the valid retry must still complete the task: {other:?}"),
    }
}

/// `complete_task.artifacts` must reject a *real*, stored artifact
/// that was produced by an unrelated agent outside the caller's subtree -
/// the ownership check, not just the "does not exist" fallback.
#[tokio::test]
async fn complete_task_rejects_an_artifact_outside_the_callers_scope() {
    use aether_core::artifact::{Artifact, ArtifactKind};
    use aether_core::task::TaskStatus;

    // Minted up front so the first scripted call can already name it.
    let foreign = ArtifactId::new();
    let h = harness(vec![
        ScriptedClient::tool_call(
            "complete_task",
            json!({"summary": "done", "artifacts": [foreign.to_string()]}),
        ),
        // The rejection does not end the turn, so the agent gets another
        // chance - and a retry with no artifacts is valid, unaffected input.
        ScriptedClient::tool_call("complete_task", json!({"summary": "done, retried"})),
    ])
    .await;

    // An unrelated agent that produces the artifact; the owner never touches it.
    let (stranger, _) = h
        .runtime
        .create_agent(
            {
                let mut s = spec("mind its own business");
                s.objective = None;
                s
            },
            Actor::user(),
        )
        .expect("stranger");
    h.store
        .commit(Commit::new().mutate(Mutation::PutArtifact(Artifact {
            id: foreign,
            kind: ArtifactKind::Other {
                label: "note".into(),
            },
            name: "stranger's note".into(),
            path: None,
            summary: "not the caller's to attach".into(),
            size_bytes: 0,
            content_type: None,
            digest: None,
            created_by: Actor::Agent { id: stranger.id },
            agent_id: Some(stranger.id),
            task_id: None,
            created_at: now(),
            evicted_at: None,
        })))
        .expect("store the foreign artifact");

    let (owner, task) = h
        .runtime
        .create_agent(spec("write something"), Actor::user())
        .expect("owner");
    let task = task.expect("objective creates a task");
    h.wait_until(owner.id, |s| s.is_terminal()).await;

    let rejection = first_artifact_rejection(&h, owner.id);
    let text = rejection.expect("a foreign artifact id was accepted, not rejected");
    assert!(text.contains(&foreign.to_string()), "{text}");

    let settled = h.store.task(task.id).expect("task").expect("exists");
    match settled.status {
        TaskStatus::Completed { result } => {
            assert_eq!(result.summary, "done, retried");
            assert!(
                result.artifacts.is_empty(),
                "the rejected artifact must never reach the settled task"
            );
        }
        other => panic!("the valid retry must still complete the task: {other:?}"),
    }
}

/// The first `complete_task.artifacts` rejection message in `agent`'s
/// transcript, if any.
fn first_artifact_rejection(h: &Harness, agent: AgentId) -> Option<String> {
    h.store
        .transcript(agent, 0)
        .expect("transcript")
        .iter()
        .find_map(|e| {
            e.message.content.iter().find_map(|b| match b {
                ContentBlock::ToolResult {
                    content,
                    is_error: true,
                    ..
                } => content.iter().find_map(|c| match c {
                    aether_core::model::ToolResultBlock::Text { text }
                        if text.contains("complete_task.artifacts") =>
                    {
                        Some(text.clone())
                    }
                    _ => None,
                }),
                _ => None,
            })
        })
}
