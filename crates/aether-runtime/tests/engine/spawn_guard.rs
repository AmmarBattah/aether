//! Reject native delegation that bypasses Aether ownership.
use super::*;
/// Advertises Claude Code's native spawn tool in its `Session` init despite
/// the `--disallowedTools Task` flag `claude_code.rs` always passes - a
/// renamed or rejected flag, simulated directly rather than by actually
/// evading the CLI (this must be caught, not just argv-tested).
struct NativeSpawnBypassEngine {
    /// The token the runtime handed this run, so the test can check the
    /// bypass actually cancelled the process, not just stdin.
    handed_cancel: Arc<Mutex<Option<CancellationToken>>>,
}

#[async_trait::async_trait]
impl Engine for NativeSpawnBypassEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }

    fn adapter(&self) -> String {
        "bypass/1".into()
    }

    async fn run(
        &self,
        _turn: EngineTurn,
        cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        *self.handed_cancel.lock().unwrap_or_else(|e| e.into_inner()) = Some(cancel);
        let (events, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            let _ = events
                .send(EngineEvent::Session {
                    id: "s1".into(),
                    model: None,
                    tools: vec!["Task".into()],
                })
                .await;
            // The turn must already be failing by the time this would ever
            // be read - the bypass is caught in the Session event itself.
            let _ = events
                .send(EngineEvent::Done(EngineOutcome {
                    text: Some("should never be reached".into()),
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

/// A kill-switch flag going stale (renamed, rejected)
/// must visibly interrupt the turn and retain the agent for resumption,
/// never leave the violation only in a log line.
#[tokio::test]
async fn a_native_spawn_bypass_interrupts_the_turn_visibly_without_ending_the_agent() {
    let handed_cancel = Arc::new(Mutex::new(None));
    let engine: Arc<dyn Engine> = Arc::new(NativeSpawnBypassEngine {
        handed_cancel: handed_cancel.clone(),
    });
    let h = harness_with_engine(
        engine,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let id = chat_agent(&h);
    wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    h.runtime
        .send_message(
            id,
            vec![ContentBlock::text("go spawn something")],
            Actor::user(),
        )
        .expect("send");

    let agent = wait_for(h.store.as_ref(), id, |a| {
        a.stats.turns > 0 && a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    assert!(!agent.status.is_terminal());
    let events = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events");
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, EventKind::AgentInterrupted { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, EventKind::AgentFailed { .. }))
    );

    let bypass_events: Vec<(String, serde_json::Value)> = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events")
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::Custom { name, payload } if name == "engine.kill_switch_bypassed" => {
                Some((name, payload))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        bypass_events.len(),
        1,
        "the bypass must be its own event, not just a log line"
    );
    assert_eq!(bypass_events[0].1["tool"], "Task");

    // The bypass must stop the process itself, the same path a
    // cancel uses - not just close stdin and hope it notices.
    let cancel = handed_cancel
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .expect("the engine was given a token");
    assert!(
        cancel.is_cancelled(),
        "a detected bypass must cancel the engine's process token"
    );
}
