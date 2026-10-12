//! Mid-turn subtree budgets and native bridge enforcement.
use super::*;
/// Hands the running turn's bridge credentials to the test and then stays
/// alive the way a real CLI does while it works, so a bridge call can be put
/// to a live turn. Reports `spend` first when it has any, and records whether
/// its own cancel token fired - the runtime stopping the process, not just
/// dropping its stream.
struct BridgeEngine {
    ready: Arc<Mutex<Option<BridgeHandover>>>,
    spend: Option<aether_core::model::Usage>,
    /// A tool call announced before the handover, the way a real CLI reports
    /// the block it is about to run - so a refusal has a call to close.
    announce: Option<(String, String)>,
    /// Queued immediately behind `spend`, so a turn that reads it has kept
    /// working past the accounting that carried it over its grant.
    then_says: Option<String>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl BridgeEngine {
    fn new(
        spend: Option<aether_core::model::Usage>,
    ) -> (
        Arc<Self>,
        tokio::sync::oneshot::Receiver<(AgentId, String)>,
        Arc<std::sync::atomic::AtomicBool>,
    ) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let engine = Arc::new(Self {
            ready: Arc::new(Mutex::new(Some(tx))),
            spend,
            announce: None,
            then_says: None,
            cancelled: cancelled.clone(),
        });
        (engine, rx, cancelled)
    }
}

#[async_trait::async_trait]
impl Engine for BridgeEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }

    fn adapter(&self) -> String {
        "bridge/1".into()
    }

    async fn run(
        &self,
        turn: EngineTurn,
        cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        let args = &turn.mcp.as_ref().expect("the turn carries a bridge").args;
        let after = |flag: &str| -> String {
            let at = args.iter().position(|a| a == flag).expect("bridge flag");
            args[at + 1].clone()
        };
        let credentials = (
            after("--agent").parse::<AgentId>().expect("agent id"),
            after("--token"),
        );
        let ready = self.ready.lock().unwrap().take();
        let spend = self.spend;
        let announce = self.announce.clone();
        let then_says = self.then_says.clone();
        let cancelled = self.cancelled.clone();
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            if let Some(spend) = spend {
                let _ = tx.send(EngineEvent::UsageDelta { usage: spend }).await;
            }
            if let Some(text) = then_says {
                let _ = tx
                    .send(EngineEvent::Assistant {
                        content: vec![ContentBlock::text(&text)],
                        usage: None,
                        model: None,
                    })
                    .await;
            }
            if let Some((id, name)) = announce {
                let _ = tx
                    .send(EngineEvent::Assistant {
                        content: vec![ContentBlock::ToolUse {
                            id,
                            name,
                            input: serde_json::json!({}),
                        }],
                        usage: None,
                        model: None,
                    })
                    .await;
            }
            if let Some(ready) = ready {
                let _ = ready.send(credentials);
            }
            tokio::select! {
                () = cancel.cancelled() => {
                    cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                () = tokio::time::sleep(Duration::from_secs(120)) => {
                    let _ = tx
                        .send(EngineEvent::Done(EngineOutcome {
                            text: Some("late".into()),
                            ..Default::default()
                        }))
                        .await;
                }
            }
        });
        Ok(EngineRun {
            events: rx,
            input: None,
            decisions: None,
            stop: None,
        })
    }
}

/// Waits for the engine process to have been stopped, not just its stream.
async fn cancelled_within(flag: &std::sync::atomic::AtomicBool, within: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + within;
    while tokio::time::Instant::now() < deadline {
        if flag.load(std::sync::atomic::Ordering::SeqCst) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

fn remember_call() -> serde_json::Value {
    serde_json::json!({"text": "the publication went out", "kind": "fact", "scope": "agent"})
}

/// An accounting that carries the agent over its grant
/// stops the turn where it lands. This is the only gate a turn running
/// entirely in the engine's own tools ever reaches, and it must read as a
/// budget stop: the event, the reason, and a stopped engine process.
#[tokio::test]
async fn an_accounting_over_the_grant_stops_the_turn_and_the_engine() {
    let (mut engine, _ready, cancelled) = BridgeEngine::new(Some(aether_core::model::Usage {
        input_tokens: 41_000,
        ..Default::default()
    }));
    // Already queued behind the accounting: a turn that goes on to record it
    // kept working past its grant.
    Arc::get_mut(&mut engine).expect("sole owner").then_says = Some("published it".into());
    let h = harness_with_engine(
        engine,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let id = budgeted_agent(&h, 40_000);
    h.runtime
        .send_message(id, vec![ContentBlock::text("Publish it")], Actor::user())
        .expect("send");

    let stopped = wait_for(h.store.as_ref(), id, |a| !a.is_live()).await;
    // One outcome, whichever path saw the crossing first: both go through the
    // scheduler's budget stop, so neither an engine failure nor a bare
    // `Cancelled` is acceptable here.
    match &stopped.status {
        AgentStatus::Failed { error, .. } => assert!(
            error.contains("token budget of 40000"),
            "the stop names the budget: {error}"
        ),
        other => panic!("a budget stop, not {other:?}"),
    }
    let events = h
        .store
        .events(0, 1000, &EventFilter::for_agent(id))
        .expect("events");
    assert!(
        events.iter().any(
            |e| matches!(&e.kind, EventKind::BudgetExceeded { reason, .. }
                if reason.contains("token budget of 40000"))
        ),
        "a budget stop is on the record as one"
    );
    assert!(
        !events.iter().any(|e| matches!(&e.kind, EventKind::AgentMessage { content, .. }
            if content.iter().any(|b| matches!(b, ContentBlock::Text { text } if text == "published it")))),
        "the turn stopped at the accounting, not after the next thing the engine said"
    );
    assert!(
        cancelled_within(&cancelled, Duration::from_secs(10)).await,
        "the engine process is stopped, not left running past the grant"
    );
    h.runtime.shutdown().await;
}

/// A live turn whose subtree has crossed the agent's grant, with the bridge
/// credentials in hand. The crossing never reaches the agent's own
/// accounting, so the tool boundary is the gate that has to catch it.
async fn a_turn_its_subtree_put_over_budget(
    engine: Arc<BridgeEngine>,
    ready: tokio::sync::oneshot::Receiver<(AgentId, String)>,
) -> (Harness, AgentId, String) {
    let h = harness_with_engine(
        engine,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let id = budgeted_agent(&h, 40_000);
    let (child, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "worker".into(),
                parent: Some(id),
                config: Some(aether_core::agent::AgentConfig {
                    effort: Some(aether_core::model::Effort::Low),
                    ..Default::default()
                }),
                start: false,
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("child");
    h.runtime
        .send_message(id, vec![ContentBlock::text("Publish it")], Actor::user())
        .expect("send");
    let (agent, token) = tokio::time::timeout(Duration::from_secs(20), ready)
        .await
        .expect("the turn started")
        .expect("the turn handed over its bridge");
    assert_eq!(agent, id);

    // Straight to the store: an `apply` would wake the scheduler, and these
    // tests are about the gate inside the turn, not the sweep outside it.
    let mut row = h.store.agent(child.id).expect("read").expect("child");
    row.stats.usage.input_tokens = 41_000;
    h.store
        .commit(aether_store::Commit::new().mutate(aether_store::Mutation::UpsertAgent(row)))
        .expect("commit");
    (h, id, token)
}

/// The refused call must not have run, and the agent must
/// stop as a budget stop - taking the subtree the grant covers with it.
#[tokio::test]
async fn a_child_crossing_the_grant_refuses_the_next_bridge_call_mid_turn() {
    let (mut engine, ready, cancelled) = BridgeEngine::new(None);
    Arc::get_mut(&mut engine).expect("sole owner").announce =
        Some(("toolu_remember".into(), "remember".into()));
    let (h, id, token) = a_turn_its_subtree_put_over_budget(engine, ready).await;

    let (reply, is_error) = h
        .runtime
        .engine_tool(id, &token, "remember".into(), remember_call())
        .await
        .expect("the live turn answered");
    assert!(is_error, "{reply}");
    assert!(
        reply.contains("token budget of 40000") && reply.contains("41000 used"),
        "the refusal names the budget and the figure: {reply}"
    );
    assert!(
        memories(&h).is_empty(),
        "the call must not have run: {:?}",
        memories(&h)
    );

    let stopped = wait_for(h.store.as_ref(), id, |a| !a.is_live()).await;
    match &stopped.status {
        AgentStatus::Failed { error, .. } => assert!(
            error.contains("token budget of 40000"),
            "the stop names the budget: {error}"
        ),
        other => panic!("a budget stop, not {other:?}"),
    }
    let events = h
        .store
        .events(0, 1000, &EventFilter::for_agent(id))
        .expect("events");
    assert!(
        events.iter().any(
            |e| matches!(&e.kind, EventKind::BudgetExceeded { reason, .. }
                if reason.contains("token budget of 40000"))
        ),
        "a budget stop is on the record as one"
    );
    // The announced call is closed, so a handoff off this agent is not handed
    // a tool call that looks like it never ran.
    assert!(
        events.iter().any(|e| matches!(&e.kind, EventKind::AgentMessage { content, .. }
            if content.iter().any(|b| matches!(b, ContentBlock::ToolResult { tool_use_id, is_error, .. }
                if tool_use_id == "toolu_remember" && *is_error)))),
        "the refused call has a result in the transcript"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(&e.kind, EventKind::ToolDenied { reason, .. }
                if reason.contains("token budget of 40000"))),
        "a refusal is recorded as a refusal, not as a failed success"
    );
    // Refusing bridge calls alone leaves the engine's own tools running past
    // the grant, so the process is stopped too. Only the cancel is visible
    // here - these engines report no stop verb; the ask that precedes it and
    // its grace are asserted in `engine_wait.rs`.
    assert!(
        cancelled_within(&cancelled, Duration::from_secs(10)).await,
        "the engine process is stopped, not left running past the grant"
    );
    for agent in h
        .store
        .agents(&aether_store::AgentFilter::default())
        .expect("agents")
    {
        assert!(
            agent.status.is_terminal(),
            "the subtree the grant covers stops with it: {} {:?}",
            agent.id,
            agent.status
        );
    }
    h.runtime.shutdown().await;
}

/// The other side of the same race: the scheduler's sweep stops the agent
/// while the turn is still running. The turn's own finalize must not write its
/// live status back over the verdict and then re-end the agent as a bare
/// `Cancelled` - the operator would be shown a cancel with no budget in it.
#[tokio::test]
async fn a_sweep_stop_during_a_turn_keeps_the_budget_as_the_reason() {
    let (engine, ready, _cancelled) = BridgeEngine::new(None);
    let h = harness_with_engine(
        engine,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let id = budgeted_agent(&h, 40_000);
    let (child, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "worker".into(),
                parent: Some(id),
                config: Some(aether_core::agent::AgentConfig {
                    effort: Some(aether_core::model::Effort::Low),
                    ..Default::default()
                }),
                start: false,
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("child");
    h.runtime
        .send_message(id, vec![ContentBlock::text("Publish it")], Actor::user())
        .expect("send");
    tokio::time::timeout(Duration::from_secs(20), ready)
        .await
        .expect("the turn started")
        .expect("the turn handed over its bridge");

    // Through the runtime, so the scheduler is woken and the sweep - not the
    // turn - is what finds the crossing.
    let mut row = h.store.agent(child.id).expect("read").expect("child");
    row.stats.usage.input_tokens = 41_000;
    h.runtime
        .apply(aether_store::Commit::new().mutate(aether_store::Mutation::UpsertAgent(row)))
        .expect("commit");

    let stopped = wait_for(h.store.as_ref(), id, |a| !a.is_live()).await;
    match &stopped.status {
        AgentStatus::Failed { error, .. } => assert!(
            error.contains("token budget of 40000"),
            "the verdict stands: {error}"
        ),
        other => panic!("the budget verdict was overwritten: {other:?}"),
    }
    // And it stays that way once the turn has finished unwinding.
    let settled = wait_for(h.store.as_ref(), id, |a| a.stats.turns >= 1).await;
    assert_eq!(
        settled.status.kind(),
        AgentStatusKind::Failed,
        "{:?}",
        settled.status
    );
    h.runtime.shutdown().await;
}

/// A Claude Code permission prompt is a tool call with a protocol of its own.
/// Refusing it with a bare string hands the CLI a malformed decision on the
/// one call that is actually a safety question.
#[tokio::test]
async fn a_permission_prompt_over_the_grant_is_denied_in_its_own_protocol() {
    let (mut engine, ready, cancelled) = BridgeEngine::new(None);
    // The call the prompt is about, announced the way a real CLI reports it.
    Arc::get_mut(&mut engine).expect("sole owner").announce =
        Some(("toolu_bash".into(), "Bash".into()));
    let (h, id, token) = a_turn_its_subtree_put_over_budget(engine, ready).await;

    let (reply, is_error) = h
        .runtime
        .engine_tool(
            id,
            &token,
            "permission".into(),
            serde_json::json!({
                "tool_name": "Bash",
                "tool_use_id": "toolu_bash",
                "input": {"command": "publish"},
            }),
        )
        .await
        .expect("the live turn answered");
    assert!(!is_error, "a decision is not a tool error: {reply}");
    let decision: serde_json::Value = serde_json::from_str(&reply).expect("a decision body");
    assert_eq!(decision["behavior"], "deny", "{reply}");
    assert!(
        decision["message"]
            .as_str()
            .expect("a message")
            .contains("token budget of 40000"),
        "{reply}"
    );
    wait_for(h.store.as_ref(), id, |a| !a.is_live()).await;
    let events = h
        .store
        .events(0, 1000, &EventFilter::for_agent(id))
        .expect("events");
    // The gated call is named in the payload, not in `req.name`: matched on
    // the name the bridge used, the refusal closed nothing and `toolu_bash`
    // dangled into the next model's transcript.
    assert!(
        events.iter().any(|e| matches!(&e.kind, EventKind::AgentMessage { content, .. }
            if content.iter().any(|b| matches!(b, ContentBlock::ToolResult { tool_use_id, is_error, .. }
                if tool_use_id == "toolu_bash" && *is_error)))),
        "the call the prompt gated is closed"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(&e.kind, EventKind::ToolDenied { .. })),
        "and recorded as a refusal"
    );
    assert!(
        cancelled_within(&cancelled, Duration::from_secs(10)).await,
        "the engine process is stopped, not left running past the grant"
    );
    h.runtime.shutdown().await;
}

/// The other direction, and the one the gate must never get wrong: an agent
/// with a grant it has *not* crossed is served its bridge call as usual.
#[tokio::test]
async fn a_budgeted_agent_under_its_grant_is_served_its_bridge_call() {
    let (engine, ready, _cancelled) = BridgeEngine::new(Some(aether_core::model::Usage {
        input_tokens: 1_000,
        ..Default::default()
    }));
    let h = harness_with_engine(
        engine,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let id = budgeted_agent(&h, 40_000);
    h.runtime
        .send_message(id, vec![ContentBlock::text("Work")], Actor::user())
        .expect("send");
    let (agent, token) = tokio::time::timeout(Duration::from_secs(20), ready)
        .await
        .expect("the turn started")
        .expect("the turn handed over its bridge");

    let (reply, is_error) = h
        .runtime
        .engine_tool(agent, &token, "remember".into(), remember_call())
        .await
        .expect("answered");
    assert!(!is_error, "an affordable call must be served: {reply}");
    assert_eq!(
        memories(&h),
        vec!["the publication went out".to_owned()],
        "the call ran"
    );
    let agent = h.store.agent(id).expect("read").expect("agent");
    assert!(agent.is_live(), "still working: {:?}", agent.status);
    h.runtime.shutdown().await;
}
