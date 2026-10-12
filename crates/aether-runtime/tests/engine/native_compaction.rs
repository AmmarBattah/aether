//! Native pressure and persisted automatic requests must not invoke a second compactor.

use super::*;

struct PressureEngine {
    inner: CheckpointEngine,
    context: Vec<u64>,
}

#[async_trait::async_trait]
impl Engine for PressureEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }
    fn adapter(&self) -> String {
        "pressure/1".into()
    }
    async fn run(
        &self,
        turn: EngineTurn,
        cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        let summary = turn.summary_only;
        let mut run = self.inner.run(turn, cancel).await?;
        let context = self.context.clone();
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            while let Some(event) = run.events.recv().await {
                if !summary && matches!(event, EngineEvent::Done(_)) {
                    for &tokens in &context {
                        if tx.send(EngineEvent::ContextUsage { tokens }).await.is_err() {
                            return;
                        }
                    }
                }
                if tx.send(event).await.is_err() {
                    return;
                }
            }
        });
        Ok(EngineRun {
            events: rx,
            input: run.input,
            decisions: run.decisions,
            stop: None,
        })
    }
}

#[tokio::test]
async fn native_context_pressure_does_not_trigger_aether_compaction() {
    use aether_core::event::{CompactionTrigger, EventDraft};
    for (final_context, legacy_trigger) in [
        (40_000, None),
        (180_000, None),
        (
            180_000,
            Some(CompactionTrigger::TokenThreshold {
                tokens: 180_000,
                threshold: 150_000,
            }),
        ),
        (
            180_000,
            Some(CompactionTrigger::TaskBoundary {
                task: aether_core::id::TaskId::new(),
            }),
        ),
        (180_000, Some(CompactionTrigger::LongWait)),
        (
            180_000,
            Some(CompactionTrigger::LargeToolOutput {
                call: aether_core::id::ToolCallId::new(),
            }),
        ),
    ] {
        let turns = Arc::new(Mutex::new(Vec::new()));
        let h = harness_with_engine(
            Arc::new(PressureEngine {
                inner: CheckpointEngine {
                    turns: turns.clone(),
                    valid: true,
                },
                context: vec![180_000, final_context],
            }),
            Arc::default(),
            Arc::default(),
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
                vec![ContentBlock::text("remember saffron")],
                Actor::user(),
            )
            .unwrap();
        let first = wait_for(h.store.as_ref(), id, |a| {
            a.stats.turns >= 1 && a.status.kind() == AgentStatusKind::Waiting
        })
        .await;
        assert_eq!(first.stats.context_tokens, Some(final_context));
        assert!(
            !h.store
                .events(0, 1000, &EventFilter::for_agent(id))
                .unwrap()
                .iter()
                .any(|e| matches!(e.kind, EventKind::CompactionRequested { .. }))
        );
        if let Some(trigger) = legacy_trigger {
            h.store
                .commit(
                    aether_store::Commit::new().event(
                        EventDraft::new(Actor::user(), EventKind::CompactionRequested { trigger })
                            .agent(id),
                    ),
                )
                .unwrap();
        }
        h.runtime
            .send_message(id, vec![ContentBlock::text("continue")], Actor::user())
            .unwrap();
        wait_for(h.store.as_ref(), id, |a| {
            a.stats.turns > first.stats.turns && a.status.kind() == AgentStatusKind::Waiting
        })
        .await;
        assert_eq!(
            turns
                .lock()
                .unwrap()
                .iter()
                .filter(|t| t.summary_only)
                .count(),
            0,
            "native context pressure must not create a second compaction loop"
        );
        h.runtime.request_compaction(id, Actor::user()).unwrap();
        wait_for(h.store.as_ref(), id, |a| {
            a.stats.turns > first.stats.turns + 1 && a.status.kind() == AgentStatusKind::Waiting
        })
        .await;
        assert!(
            turns
                .lock()
                .unwrap()
                .iter()
                .filter(|t| t.summary_only)
                .count()
                > 0,
            "explicit checkpoint still runs independently of native pressure"
        );
        h.runtime.shutdown().await;
    }
}
