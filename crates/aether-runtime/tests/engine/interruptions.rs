//! Bounded diagnostics for interrupted engines.
use super::*;
/// Ends the turn with `interrupted: true` and whatever error text the test
/// hands it - an adapter reporting it stopped itself for a harness boundary,
/// unrelated to a forced wind-down (`Park`).
struct InterruptingEngine {
    error: String,
}

#[async_trait::async_trait]
impl Engine for InterruptingEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }
    fn adapter(&self) -> String {
        "interrupting/1".into()
    }
    async fn run(
        &self,
        _turn: EngineTurn,
        _cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        let (tx, rx) = mpsc::channel(1);
        tx.send(EngineEvent::Done(EngineOutcome {
            interrupted: true,
            error: Some(self.error.clone()),
            ..Default::default()
        }))
        .await
        .expect("fixture channel capacity");
        Ok(EngineRun {
            events: rx,
            input: None,
            decisions: None,
            stop: None,
        })
    }
}

/// `engine.interrupted`'s `reason` is an adapter error string
/// with no length contract of its own - it must reach the durable event
/// clamped, the same as its sibling `engine.parked`, not verbatim.
#[tokio::test]
async fn engine_interrupted_clamps_an_over_long_adapter_error() {
    // Longer than turn_engine's private MAX_ENGINE_ERROR (1024 chars); this
    // test pins the clamp at the emit site, not the pre-existing helper.
    let long_error = "x".repeat(1024 + 500);
    let h = harness_with_engine(
        Arc::new(InterruptingEngine {
            error: long_error.clone(),
        }),
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let id = chat_agent(&h);
    h.runtime
        .send_message(id, vec![ContentBlock::text("go")], Actor::user())
        .expect("send");
    let agent = wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    assert!(
        !agent.status.is_terminal(),
        "an interruption is not a failure"
    );

    let events = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("events");
    let reason = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::Custom { name, payload } if name == "engine.interrupted" => Some(
                payload["reason"]
                    .as_str()
                    .expect("reason is a string")
                    .to_owned(),
            ),
            _ => None,
        })
        .expect("the interruption left its trace");
    assert_eq!(
        reason.chars().count(),
        1024,
        "an over-long adapter error reached the event unclamped: {} chars",
        reason.chars().count()
    );
    assert!(
        long_error.starts_with(&reason),
        "the clamp must keep the error's own text, not replace it"
    );
    h.runtime.shutdown().await;
}
