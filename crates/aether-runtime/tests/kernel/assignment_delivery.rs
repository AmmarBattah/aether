//! assignment delivery scenarios and fixtures.
use super::*;

/// A task handed to an agent waiting on its inbox is new work: the assignment
/// wakes it, rather than leaving it parked until something else does.
#[tokio::test]
async fn assigning_a_task_to_a_waiting_agent_wakes_it() {
    let h = harness(vec![ScriptedClient::tool_call(
        "complete_task",
        json!({"summary": "done"}),
    )])
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("agent");
    let status = h
        .wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    assert!(
        matches!(
            status,
            AgentStatus::Waiting {
                condition: aether_core::agent::WaitCondition::Inbox
            }
        ),
        "{status:?}"
    );

    let spec = aether_runtime::CreateTaskSpec {
        objective: "the work".into(),
        owner: Some(agent.id),
        ..Default::default()
    };
    let task = h.runtime.create_task(spec, Actor::user()).expect("task");
    h.wait_for("the woken agent to complete its task", || {
        h.store
            .task(task.id)
            .expect("task read")
            .is_some_and(|t| t.status.kind() == TaskStatusKind::Completed)
    })
    .await;
    assert_eq!(h.client.remaining(), 0, "the agent took a turn for it");
}

/// Two assignments to one agent at once: the second must be refused, not
/// overwrite the owner's link to the first from a row read before it landed.
#[tokio::test]
async fn concurrent_assignments_to_one_agent_refuse_the_second() {
    let inner: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let store = Arc::new(CommitsMidPass::new(inner, Hook::AgentRead));
    let h = harness_full(
        vec![],
        |_| {},
        |_| {},
        None,
        None,
        Some(store.clone()),
        |_| {},
    )
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("agent");
    let id = agent.id;
    h.wait_until(id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    // Paused, so neither assignment starts a turn while the test looks.
    h.runtime
        .pause_agent(id, false, Actor::user())
        .expect("pause");
    let task_for = |objective: &str| aether_runtime::CreateTaskSpec {
        objective: objective.into(),
        owner: Some(id),
        ..Default::default()
    };

    let second = Arc::new(std::sync::Mutex::new(None));
    let slot = second.clone();
    let second_spec = task_for("the second");
    let racer = race_through_runtime(&store, &h.runtime, move |runtime| {
        let outcome = runtime
            .create_task(second_spec, Actor::user())
            .map(|t| t.id)
            .map_err(|e| e.to_string());
        *slot.lock().expect("slot") = Some(outcome);
    });
    let first = h
        .runtime
        .create_task(task_for("the first"), Actor::user())
        .expect("the first assignment");
    join_racer(&racer);

    let second = second.lock().expect("slot").take().expect("the racer ran");
    let refused = second.expect_err("the agent already owned a task");
    assert!(
        refused.contains(&format!("already owns a task ({})", first.id)),
        "{refused}"
    );
    assert!(refused.contains("without an owner"), "{refused}");
    let owner = h.store.agent(id).expect("read").expect("agent");
    assert_eq!(owner.current_task, Some(first.id));
}

/// A parent's own conversation is its own events: a child speaking between the
/// parent's tool call and its result used to hide the call from the step that
/// feeds the result back, so the parent's next turn never saw it.
///
/// The gated call is `call_outside`, which touches no disk: a real file write
/// syncs, and a host that stalls the sync stalls the turn past the wait.
#[tokio::test]
async fn a_childs_message_between_a_tool_call_and_its_result_still_composes_the_result() {
    let h = harness_with_tools(
        vec![
            ScriptedClient::tool_call("call_outside", json!({})),
            ScriptedClient::tool_call("complete_task", json!({"summary": "called"})),
        ],
        |_| {},
        |_| {},
        |tools| tools.register(Arc::new(CallOutside)),
    )
    .await;
    let mut s = spec("call the outside service");
    s.approval = Some(ApprovalPolicy {
        require_for: [SideEffectClass::External].into(),
    });
    let (agent, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let child = AgentId::new();
    let said = EventKind::AgentMessage {
        role: aether_core::model::MessageRole::Assistant,
        content: vec![ContentBlock::text("the child reports progress")],
        invocation: None,
    };
    h.store
        .commit(
            Commit::new().event(
                EventDraft::new(Actor::agent(child), said)
                    .agent(child)
                    .parent(Some(agent.id))
                    .run(agent.run),
            ),
        )
        .expect("the child's message");

    let pending = h.store.pending_approvals().expect("pending");
    assert_eq!(pending.len(), 1);
    h.runtime
        .resolve_approval(
            pending[0].id,
            ApprovalDecision::Approved,
            Actor::user(),
            None,
            None,
        )
        .expect("approve");
    let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed, "{status:?}");
    // Right after the call, before the model is asked again: a later turn's
    // results can reuse the scripted provider id and hide a missing one.
    let transcript = h.store.transcript(agent.id, 0).expect("transcript");
    let asked = transcript
        .iter()
        .position(|e| {
            e.message
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { name, .. } if name == "call_outside"))
        })
        .expect("the parent's call_outside call");
    let answered = &transcript
        .get(asked + 1)
        .expect("a message after it")
        .message;
    assert!(
        answered.is_tool_results_only(),
        "the approved call's result must follow it: {answered:?}"
    );
}

/// The failure streak is the agent's own: a child's failed invocations are the
/// child's, and must not fail the parent on its first retryable error.
#[tokio::test]
async fn a_childs_failed_invocations_do_not_count_toward_its_parents_failure_streak() {
    let h = harness_full(
        vec![
            ScriptedClient::error(ModelErrorClass::RateLimit, "slow down"),
            ScriptedClient::text("recovered"),
        ],
        |_| {},
        |c| c.max_consecutive_failures = 2,
        None,
        None,
        None,
        |_| {},
    )
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("agent");
    let id = agent.id;
    h.wait_until(id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let child = AgentId::new();
    let mut commit = Commit::new();
    for _ in 0..2 {
        let failed = EventKind::InvocationFailed {
            class: None,
            invocation: aether_core::id::InvocationId::new(),
            snapshot: aether_core::id::SnapshotId::new(),
            error: "the child's provider failed".into(),
            retryable: true,
            cancelled: false,
            partial: vec![],
        };
        commit.push_event(
            EventDraft::new(Actor::agent(child), failed)
                .agent(child)
                .parent(Some(id))
                .run(agent.run),
        );
    }
    h.store.commit(commit).expect("the child's failures");

    h.runtime
        .send_message(id, vec![ContentBlock::text("go")], Actor::user())
        .expect("message");
    let client = h.client.clone();
    let status = h
        .wait_until(id, move |s| {
            s.is_terminal() || (s.kind() == AgentStatusKind::Waiting && client.remaining() == 0)
        })
        .await;
    assert_eq!(
        status.kind(),
        AgentStatusKind::Waiting,
        "one failure of its own must not end the parent: {status:?}"
    );
}
