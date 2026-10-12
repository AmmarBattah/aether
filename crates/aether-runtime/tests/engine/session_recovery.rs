//! Lost sessions and context-collapse recovery.
use super::*;
/// Reports a session, then refuses to resume it once. The runtime retries
/// that turn on a fresh session - which retains nothing, so the layers the
/// gate left out have to go in after all.
struct LostSessionEngine {
    prompts: Arc<Mutex<Vec<String>>>,
    refusals: Arc<Mutex<u32>>,
}

#[async_trait::async_trait]
impl Engine for LostSessionEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }

    fn adapter(&self) -> String {
        "lost-session/1".into()
    }

    async fn run(
        &self,
        turn: EngineTurn,
        _cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        self.prompts.lock().unwrap().push(turn.prompt.clone());
        let mut refusals = self.refusals.lock().unwrap();
        let refuse = turn.session.is_some() && *refusals == 0;
        if refuse {
            *refusals += 1;
        }
        drop(refusals);
        let (events, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            if refuse {
                let _ = events
                    .send(EngineEvent::Done(EngineOutcome {
                        is_error: true,
                        error: Some("no conversation found".into()),
                        session_lost: true,
                        ..Default::default()
                    }))
                    .await;
                return;
            }
            let _ = events
                .send(EngineEvent::Session {
                    id: format!("s{}", ulid::Ulid::generate()),
                    model: None,
                    tools: vec![],
                })
                .await;
            let _ = events
                .send(EngineEvent::Done(EngineOutcome {
                    text: Some("done".into()),
                    ..Default::default()
                }))
                .await;
        });
        Ok(EngineRun {
            events: rx,
            input: None,
            decisions: None,
            stop: None,
        })
    }
}

/// The gate leaves a layer out because the session holds it. A session the
/// engine then refuses to resume holds nothing, so the retry must carry the
/// whole assignment - not the trimmed turn the gate built for the old one.
#[tokio::test]
async fn a_refused_session_is_re_sent_everything_the_gate_left_out() {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let engine: Arc<dyn Engine> = Arc::new(LostSessionEngine {
        prompts: prompts.clone(),
        refusals: Arc::new(Mutex::new(0)),
    });
    let h = harness_with_engine(engine, prompts.clone(), Arc::default()).await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                name: Some("delta-worker".into()),
                objective: Some("migrate the ledger sentinel".into()),
                model: Some(ModelRef::new("claude-code", "haiku")),
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
    let id = agent.id;
    let started = wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns >= 1 && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    assert!(started.engine_session.is_some());

    h.runtime
        .send_message(id, vec![ContentBlock::text("carry on")], Actor::user())
        .expect("send");
    wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns > started.stats.turns && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;

    let sent = prompts.lock().unwrap().clone();
    assert_eq!(
        sent.len(),
        3,
        "one fresh turn, one refused resume, one retry"
    );
    assert!(
        !sent[1].contains("migrate the ledger sentinel"),
        "the gate trimmed the resumed turn: {}",
        sent[1]
    );
    assert!(
        sent[2].contains("migrate the ledger sentinel") && sent[2].contains("Scope:"),
        "the retry runs on a session that retains nothing: {}",
        sent[2]
    );
    // A fresh turn hoists the role/task assignment, which
    // carries authority - it must be fenced, with the shortened markers.
    assert!(
        sent[0].contains("--- Aether [") && sent[0].contains("--- end Aether ["),
        "an authoritative hoist must be fenced: {}",
        sent[0]
    );
    let events = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events");
    let recorded: Vec<String> = events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::SnapshotCreated { snapshot, .. } => Some(*snapshot),
            _ => None,
        })
        .filter_map(|sid| h.store.snapshot(sid).expect("snapshot"))
        .filter_map(|s| s.messages.last().map(|m| m.text()))
        .collect();
    assert!(
        recorded
            .iter()
            .any(|text| text.contains("Scope:") && text.contains("carry on")),
        "the snapshot must record what was really sent, not the trimmed turn: {recorded:?}"
    );

    // The resend re-announces `SnapshotCreated` for the same
    // snapshot id, so the LAST announcement per id is what a store rebuilt
    // from the event log would end up with - it must match the stored row,
    // the same two invariants `an_engine_turns_snapshot_reconstructs_too`
    // checks on the non-refused path.
    let last_announced: std::collections::BTreeMap<
        aether_core::id::SnapshotId,
        (u32, Vec<aether_core::snapshot::ContextLayer>),
    > = events
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::SnapshotCreated {
                snapshot,
                token_estimate,
                layers,
                ..
            } => Some((snapshot, (token_estimate, layers))),
            _ => None,
        })
        .collect();
    assert!(
        !last_announced.is_empty(),
        "the refused-session turn recorded no snapshot"
    );
    for (sid, (token_estimate, layers)) in last_announced {
        let stored = h
            .store
            .snapshot(sid)
            .expect("snapshot")
            .expect("every announced snapshot is stored");
        assert_eq!(
            stored.layers, layers,
            "the stored snapshot must match the last announcement, not a pre-resend one"
        );
        assert_eq!(
            stored.token_estimate, token_estimate,
            "the announced token_estimate must match what is actually stored"
        );
        let tail = stored
            .layers
            .iter()
            .find(|layer| layer.kind == aether_core::snapshot::LayerKind::RecentTail)
            .expect("tail layer");
        assert_eq!(
            tail.tokens,
            aether_context::estimate::TokenEstimator.messages(&stored.messages),
            "the tail's token count must be recomputed over what was really sent, \
             not sized to the trimmed prompt the gate built before the refusal"
        );
    }
    h.runtime.shutdown().await;
}

/// A resumed session, reporting a shrinking context on every ordinary turn.
struct ContextCollapseEngine {
    prompts: Arc<Mutex<Vec<String>>>,
    /// Reported context size for call N (1-indexed); the last value repeats
    /// for any call past the end.
    reported_context: Vec<u64>,
    call: Arc<Mutex<usize>>,
}

#[async_trait::async_trait]
impl Engine for ContextCollapseEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }

    fn adapter(&self) -> String {
        "context-collapse/1".into()
    }

    async fn run(
        &self,
        turn: EngineTurn,
        _cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        use aether_core::model::Usage;
        self.prompts.lock().unwrap().push(turn.prompt.clone());
        let mut call = self.call.lock().unwrap();
        let index = *call;
        *call += 1;
        drop(call);
        let context_tokens = self
            .reported_context
            .get(index)
            .or_else(|| self.reported_context.last())
            .copied()
            .expect("at least one reported context figure");
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            // Same id every turn: an ordinary resumed session, no refusal.
            let _ = tx
                .send(EngineEvent::Session {
                    id: "sess_never_refused".into(),
                    model: None,
                    tools: vec![],
                })
                .await;
            let _ = tx
                .send(EngineEvent::Assistant {
                    content: vec![ContentBlock::text("done")],
                    usage: Some(Usage {
                        input_tokens: context_tokens,
                        ..Default::default()
                    }),
                    model: None,
                })
                .await;
            let _ = tx
                .send(EngineEvent::Done(EngineOutcome {
                    text: Some("done".into()),
                    ..Default::default()
                }))
                .await;
        });
        Ok(EngineRun {
            events: rx,
            input: None,
            decisions: None,
            stop: None,
        })
    }
}

/// `engine_sent` promises the session still holds what it was
/// sent, but Claude Code's own compaction can replace a retained turn with a
/// summary - invisibly to Aether. A sharp drop in the engine's reported
/// context between two ordinary (non-refused) turns is the cheap signal that
/// this happened, and must force the very next turn to resend everything,
/// exactly as a refused session would.
#[tokio::test]
async fn a_sharp_context_drop_forces_the_next_turn_to_resend_everything() {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let engine: Arc<dyn Engine> = Arc::new(ContextCollapseEngine {
        prompts: prompts.clone(),
        // Turn 2 reports under half of turn 1's figure - the drop.
        reported_context: vec![20_000, 8_000, 8_000],
        call: Arc::new(Mutex::new(0)),
    });
    let h = harness_with_engine(engine, prompts.clone(), Arc::default()).await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                name: Some("echo-worker".into()),
                objective: Some("reconcile the audit ledger".into()),
                model: Some(ModelRef::new("claude-code", "haiku")),
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
    let id = agent.id;
    let turn1 = wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns >= 1 && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;

    h.runtime
        .send_message(id, vec![ContentBlock::text("keep going")], Actor::user())
        .expect("send turn 2");
    let turn2 = wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns > turn1.stats.turns && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;

    h.runtime
        .send_message(id, vec![ContentBlock::text("still going")], Actor::user())
        .expect("send turn 3");
    wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns > turn2.stats.turns && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;

    let sent = prompts.lock().unwrap().clone();
    assert_eq!(
        sent.len(),
        3,
        "one fresh turn, one ordinary turn, one retry"
    );
    assert!(
        sent[0].contains("reconcile the audit ledger"),
        "turn 1 is fresh: the whole assignment goes out: {}",
        sent[0]
    );
    assert!(
        !sent[1].contains("reconcile the audit ledger"),
        "turn 2's task text is unchanged and the session holds it, so the gate \
         must leave it out: {}",
        sent[1]
    );
    assert!(
        sent[2].contains("reconcile the audit ledger"),
        "turn 2 reported a context collapse, so turn 3 must resend everything \
         rather than trust a session that may have compacted it away: {}",
        sent[2]
    );
    h.runtime.shutdown().await;
}
