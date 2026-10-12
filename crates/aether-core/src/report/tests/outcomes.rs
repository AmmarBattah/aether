use super::*;

#[test]
fn a_run_is_running_until_every_agent_is_terminal() {
    let mut log = Log::new();
    let (root, child) = (AgentId::new(), AgentId::new());
    log.push(created(root, None, "root"));
    log.push(created(child, Some(root), "child"));
    log.push(
        EventDraft::new(Actor::System, EventKind::AgentCompleted { summary: None }).agent(root),
    );
    assert_eq!(log.report().body.outcome, RunOutcome::Running);
    log.push(
        EventDraft::new(
            Actor::System,
            EventKind::AgentFailed {
                error: "boom".into(),
            },
        )
        .agent(child),
    );
    let report = log.report();
    // The root completed, so the run completed - a child's failure is a
    // failure line, not a verdict the root already gave.
    assert_eq!(report.body.outcome, RunOutcome::Completed);
    assert_eq!(report.body.failures.len(), 0, "agent.failed is an outcome");
    assert!(
        report.body.open_loops.is_empty(),
        "{:?}",
        report.body.open_loops
    );
}

#[test]
fn tool_calls_are_tallied_per_tool_and_failures_are_listed() {
    let mut log = Log::new();
    let agent = AgentId::new();
    log.push(created(agent, None, "root"));
    let (ok, bad) = (ToolCallId::new(), ToolCallId::new());
    for (id, name) in [(ok, "bash"), (bad, "bash")] {
        log.push(
            EventDraft::new(
                Actor::agent(agent),
                EventKind::ToolRequested {
                    call: ToolCall {
                        id,
                        provider_call_id: "p".into(),
                        name: name.into(),
                        input: serde_json::json!({}),
                        agent_id: agent,
                        task_id: None,
                        invocation_id: None,
                        resources: Vec::new(),
                        idempotency_key: None,
                        requested_at: log.at,
                    },
                    side_effect: SideEffectClass::ReadOnly,
                },
            )
            .agent(agent),
        );
    }
    log.push(
        EventDraft::new(
            Actor::agent(agent),
            EventKind::ToolSucceeded {
                call: ok,
                outcome: ToolOutcome {
                    duration_ms: 40,
                    ..ToolOutcome::ok("fine")
                },
            },
        )
        .agent(agent),
    );
    log.push(
        EventDraft::new(
            Actor::agent(agent),
            EventKind::ToolFailed {
                call: bad,
                error: "exit 1".into(),
                retryable: false,
                duration_ms: Some(250),
            },
        )
        .agent(agent),
    );
    let report = log.report();
    let bash = report
        .body
        .tools
        .iter()
        .find(|t| t.tool == "bash")
        .expect("bash");
    assert_eq!(bash.tally.requested, 2);
    assert_eq!(bash.tally.succeeded, 1);
    assert_eq!(bash.tally.failed, 1);
    // A tool that mostly fails should not report near-zero
    // time - the failed call's real duration counts too.
    assert_eq!(bash.duration_ms, 40 + 250);
    assert_eq!(report.body.failures.len(), 1);
    assert_eq!(report.body.failures[0].error, "exit 1");
    assert_eq!(report.body.agents[0].tool_calls, 2);
}

/// A call that ran and answered with an error - an MCP server refusing it, a
/// command exiting non-zero - arrives as `tool.succeeded` flagged `is_error`.
/// It is a failed call, and the run's failures say so.
#[test]
fn a_tool_that_answered_with_an_error_is_a_failure() {
    let mut log = Log::new();
    let agent = AgentId::new();
    log.push(created(agent, None, "root"));
    let call = ToolCallId::new();
    let tool = "mcp__aether__mcp__mock_strict__read_note";
    log.push(
        EventDraft::new(
            Actor::agent(agent),
            EventKind::ToolRequested {
                call: ToolCall {
                    id: call,
                    provider_call_id: "p".into(),
                    name: tool.into(),
                    input: serde_json::json!({}),
                    agent_id: agent,
                    task_id: None,
                    invocation_id: None,
                    resources: Vec::new(),
                    idempotency_key: None,
                    requested_at: log.at,
                },
                side_effect: SideEffectClass::External,
            },
        )
        .agent(agent),
    );
    log.push(
        EventDraft::new(
            Actor::agent(agent),
            EventKind::ToolSucceeded {
                call,
                outcome: ToolOutcome::error(
                    "internal error: mcp server `mock-strict`: unknown param _meta\nmore detail",
                )
                .with_duration(6),
            },
        )
        .agent(agent),
    );
    let report = log.report();
    let line = report
        .body
        .tools
        .iter()
        .find(|t| t.tool == tool)
        .expect("the tool's line");
    assert_eq!(line.tally.succeeded, 0);
    assert_eq!(line.tally.failed, 1);
    assert_eq!(line.duration_ms, 6);
    assert_eq!(report.body.failures.len(), 1);
    assert_eq!(
        report.body.failures[0].error,
        format!(
            "{tool} answered with an error: internal error: mcp server `mock-strict`: unknown param _meta"
        )
    );
    assert_eq!(report.body.failures[0].agent, Some(agent));
}

/// A call that never actually ran (`duration_ms: None`, e.g. an
/// unresolvable tool name) has no real time to report, so it must not
/// be counted as a 0ms contribution.
#[test]
fn a_tool_failure_that_never_ran_contributes_no_duration() {
    let mut log = Log::new();
    let agent = AgentId::new();
    log.push(created(agent, None, "root"));
    let call = ToolCallId::new();
    log.push(
        EventDraft::new(
            Actor::agent(agent),
            EventKind::ToolRequested {
                call: ToolCall {
                    id: call,
                    provider_call_id: "p".into(),
                    name: "ghost_tool".into(),
                    input: serde_json::json!({}),
                    agent_id: agent,
                    task_id: None,
                    invocation_id: None,
                    resources: Vec::new(),
                    idempotency_key: None,
                    requested_at: log.at,
                },
                side_effect: SideEffectClass::ReadOnly,
            },
        )
        .agent(agent),
    );
    log.push(
        EventDraft::new(
            Actor::agent(agent),
            EventKind::ToolFailed {
                call,
                error: "tool no longer registered".into(),
                retryable: false,
                duration_ms: None,
            },
        )
        .agent(agent),
    );
    let report = log.report();
    let ghost = report
        .body
        .tools
        .iter()
        .find(|t| t.tool == "ghost_tool")
        .expect("ghost_tool");
    assert_eq!(ghost.tally.failed, 1);
    assert_eq!(ghost.duration_ms, 0, "a call that never ran has no time");
}

#[test]
fn an_unanswered_approval_and_an_open_task_are_open_loops() {
    let mut log = Log::new();
    let agent = AgentId::new();
    let task = TaskId::new();
    log.push(created(agent, None, "root"));
    log.push(
        EventDraft::new(
            Actor::System,
            EventKind::TaskCreated {
                objective: "ship it".into(),
                parent_task: None,
                dependencies: Vec::new(),
                priority: Priority::default(),
            },
        )
        .task(task),
    );
    log.push(
        EventDraft::new(
            Actor::agent(agent),
            EventKind::ApprovalRequested {
                approval: ApprovalId::new(),
                call: ToolCallId::new(),
                tool: "bash".into(),
                description: "rm -rf /".into(),
                argument: None,
                side_effect: SideEffectClass::Mutation,
            },
        )
        .agent(agent),
    );
    log.push(
        EventDraft::new(Actor::System, EventKind::AgentCompleted { summary: None }).agent(agent),
    );
    let report = log.report();
    assert_eq!(
        report.body.open_loops.len(),
        2,
        "{:?}",
        report.body.open_loops
    );
    assert!(
        report
            .body
            .open_loops
            .iter()
            .any(|l| matches!(l, OpenLoop::Task { objective, .. } if objective == "ship it"))
    );
    assert!(
        report
            .body
            .open_loops
            .iter()
            .any(|l| matches!(l, OpenLoop::Approval { .. }))
    );
    assert_eq!(report.body.approvals.len(), 1);
    assert!(report.body.approvals[0].decision.is_none());
}
