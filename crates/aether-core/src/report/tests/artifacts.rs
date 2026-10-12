use super::*;

#[test]
fn an_artifact_and_its_eviction_are_one_line() {
    let mut log = Log::new();
    let agent = AgentId::new();
    let id = ArtifactId::new();
    log.push(created(agent, None, "root"));
    log.push(
        EventDraft::new(
            Actor::agent(agent),
            EventKind::ArtifactCreated {
                artifact: crate::artifact::Artifact {
                    id,
                    kind: ArtifactKind::Diff,
                    name: "patch".into(),
                    path: None,
                    summary: "one hunk".into(),
                    size_bytes: 120,
                    content_type: None,
                    digest: None,
                    created_by: Actor::agent(agent),
                    agent_id: Some(agent),
                    task_id: None,
                    created_at: log.at,
                    evicted_at: None,
                },
            },
        )
        .agent(agent),
    );
    assert!(!log.report().body.artifacts[0].evicted);
    log.push(
        EventDraft::new(
            Actor::System,
            EventKind::ArtifactEvicted {
                artifact: id,
                size_bytes: 120,
                reason: "retention".into(),
            },
        )
        .agent(agent),
    );
    let report = log.report();
    assert_eq!(report.body.artifacts.len(), 1);
    assert!(report.body.artifacts[0].evicted);
    assert_eq!(report.body.artifacts[0].kind, "diff");
}

#[test]
fn the_timeline_holds_the_shape_not_every_event() {
    let mut log = Log::new();
    let agent = AgentId::new();
    log.push(created(agent, None, "root"));
    log.push(
        EventDraft::new(
            Actor::System,
            EventKind::AgentTurnCompleted {
                turn: 1,
                invocation: None,
                next: AgentStatusKind::Waiting,
            },
        )
        .agent(agent),
    );
    log.push(
        EventDraft::new(
            Actor::System,
            EventKind::TaskCompleted {
                result: TaskResult::summary("done"),
            },
        )
        .task(TaskId::new()),
    );
    let timeline = log.report().body.timeline;
    assert_eq!(timeline.len(), 2, "{timeline:?}");
    assert_eq!(timeline[0].kind, "agent.created");
    assert_eq!(timeline[1].kind, "task.completed");
    assert_eq!(timeline[1].detail, "done");
}

/// The lossier sibling of a compaction: what a handoff could not carry
/// belongs in the report next to `compaction.completed`, not only in the
/// log and the TUI.
#[test]
fn context_handoff_reaches_the_timeline_and_names_what_was_lost() {
    let mut log = Log::new();
    let agent = AgentId::new();
    log.push(created(agent, None, "root"));
    log.push(
        EventDraft::new(
            Actor::System,
            EventKind::ContextHandoff {
                from: ModelRef::new("anthropic", "claude-sonnet-5"),
                to: ModelRef::new("mock", "m"),
                checkpoint: None,
                carried: 12,
                lost: vec!["engine session s1".into()],
            },
        )
        .agent(agent),
    );
    let timeline = log.report().body.timeline;
    let handoff = timeline
        .iter()
        .find(|m| m.kind == "context.handoff")
        .expect("the handoff reaches the timeline");
    assert_eq!(
        handoff.detail,
        "anthropic/claude-sonnet-5 -> mock/m (lost: engine session s1)"
    );
}
