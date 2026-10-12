//! A 1M Claude Code model added in `providers.toml` reaches the CLI by its
//! full id, and its base model keeps launching as before.
use super::*;
use aether_engine::claude_code::ClaudeCodeEngine;
use aether_model::config::PROVIDERS_CLAUDE_1M_EXAMPLE;

/// Records each turn it is handed, then answers it in one message.
struct RecordingEngine {
    turns: Arc<Mutex<Vec<EngineTurn>>>,
}

#[async_trait::async_trait]
impl Engine for RecordingEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }

    fn adapter(&self) -> String {
        "extended-context/1".into()
    }

    async fn run(
        &self,
        turn: EngineTurn,
        _cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        self.turns.lock().unwrap().push(turn);
        let (tx, rx) = mpsc::channel(4);
        for event in [
            EngineEvent::ContextUsage { tokens: 750_000 },
            EngineEvent::Assistant {
                content: vec![ContentBlock::text("done")],
                usage: None,
                model: None,
            },
            EngineEvent::Done(EngineOutcome {
                text: Some("done".into()),
                ..Default::default()
            }),
        ] {
            tx.send(event).await.expect("fixture channel capacity");
        }
        Ok(EngineRun {
            events: rx,
            input: None,
            decisions: None,
            stop: None,
        })
    }
}

/// The value the real Claude Code launch passes after `--model` for `turn`.
fn launched_model(turn: &EngineTurn) -> String {
    let args = ClaudeCodeEngine::new("claude", "2.1.296").args(turn);
    let flags: Vec<usize> = (0..args.len()).filter(|i| args[*i] == "--model").collect();
    assert_eq!(flags.len(), 1, "exactly one --model in {args:?}");
    args[flags[0] + 1].clone()
}

#[tokio::test]
async fn a_1m_model_added_in_providers_toml_launches_with_its_id_unchanged() {
    let turns = Arc::new(Mutex::new(Vec::new()));
    let h = harness_with_engine_and_providers_toml(
        Arc::new(RecordingEngine {
            turns: turns.clone(),
        }),
        Arc::default(),
        Arc::default(),
        Some(PROVIDERS_CLAUDE_1M_EXAMPLE),
    )
    .await;
    for (requested, launched) in [
        ("claude-code/claude-opus-5-5[1m]", "claude-opus-5-5[1m]"),
        ("claude-code/opus", "claude-opus-5-5"),
    ] {
        let model = ModelRef::parse(requested).expect("model reference");
        let (agent, _) = h
            .runtime
            .create_agent(
                CreateAgentSpec {
                    role: "assistant".into(),
                    start: true,
                    model: Some(model.clone()),
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
        h.runtime
            .send_message(agent.id, vec![ContentBlock::text("hello")], Actor::user())
            .expect("send");
        let agent = wait_for(h.store.as_ref(), agent.id, |a| {
            a.stats.turns == 1 && a.status.kind() == AgentStatusKind::Waiting
        })
        .await;
        assert_eq!(agent.model, model);
        assert_eq!(agent.stats.context_tokens, Some(750_000));

        let turn = turns.lock().unwrap().pop().expect("one launch per agent");
        assert_eq!(turn.model.as_deref(), Some(launched));
        assert_eq!(launched_model(&turn), launched);
    }
    h.runtime.shutdown().await;
}
