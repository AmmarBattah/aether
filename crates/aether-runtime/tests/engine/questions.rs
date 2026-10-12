//! Native permission/question reply channel semantics.
use super::*;
#[derive(Clone, Copy, Debug)]
enum QuestionChannel {
    Open,
    Closed,
    Missing,
}

struct QuestionEngine(QuestionChannel);

#[async_trait::async_trait]
impl Engine for QuestionEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }

    fn adapter(&self) -> String {
        "question-test/1".into()
    }

    async fn run(
        &self,
        _turn: EngineTurn,
        cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        let (events, rx) = mpsc::channel(8);
        let (decisions, answers) = mpsc::channel::<aether_engine::PermissionDecision>(1);
        let mut answers = if matches!(self.0, QuestionChannel::Open) {
            Some(answers)
        } else {
            drop(answers);
            None
        };
        tokio::spawn(async move {
            events
                .send(EngineEvent::Question {
                    request_id: "question-1".into(),
                    prompt: "Which colour?".into(),
                })
                .await
                .expect("question delivered");
            if let Some(answers) = answers.as_mut() {
                let answer = tokio::select! {
                    answer = answers.recv() => answer.expect("answer delivered"),
                    () = cancel.cancelled() => return,
                };
                assert_eq!(answer.request_id, "question-1");
                let text = format!("Selected {}", answer.answer.expect("answer text"));
                events
                    .send(EngineEvent::Assistant {
                        content: vec![ContentBlock::text(&text)],
                        usage: None,
                        model: None,
                    })
                    .await
                    .expect("assistant delivered");
                events
                    .send(EngineEvent::Done(EngineOutcome {
                        text: Some(text),
                        ..Default::default()
                    }))
                    .await
                    .expect("outcome delivered");
            } else {
                cancel.cancelled().await;
            }
        });
        Ok(EngineRun {
            events: rx,
            input: None,
            decisions: (!matches!(self.0, QuestionChannel::Missing)).then_some(decisions),
            stop: None,
        })
    }
}

#[tokio::test]
async fn question_answers_without_steering_are_delivered_or_retained_on_channel_failure() {
    for channel in [
        QuestionChannel::Open,
        QuestionChannel::Closed,
        QuestionChannel::Missing,
    ] {
        let h = harness_with_engine(
            Arc::new(QuestionEngine(channel)),
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(Mutex::new(Vec::new())),
        )
        .await;
        let id = chat_agent(&h);
        h.runtime
            .send_message(
                id,
                vec![ContentBlock::text("Choose a colour")],
                Actor::user(),
            )
            .expect("start");
        wait_for(h.store.as_ref(), id, |_| {
            spoken(h.store.as_ref(), id)
                .iter()
                .any(|(role, text)| *role == MessageRole::Assistant && text == "Which colour?")
        })
        .await;
        h.runtime
            .send_message(id, vec![ContentBlock::text("blue")], Actor::user())
            .expect("answer");
        let agent = wait_for(h.store.as_ref(), id, |a| match channel {
            QuestionChannel::Open => {
                a.status.kind() == AgentStatusKind::Waiting && a.stats.turns == 1
            }
            _ => a.status.kind() == AgentStatusKind::Failed,
        })
        .await;
        let last = h.store.inbox_last_seq(id).expect("last inbox seq");
        match channel {
            QuestionChannel::Open => {
                assert_eq!(agent.inbox_cursor, last);
                assert!(spoken(h.store.as_ref(), id).iter().any(|(role, text)| {
                    *role == MessageRole::Assistant && text.contains("blue")
                }));
            }
            _ => {
                assert!(agent.inbox_cursor < last, "{channel:?}: answer was lost");
                assert!(format!("{:?}", agent.status).contains("answer remains queued"));
                assert_eq!(
                    h.store
                        .inbox(id, agent.inbox_cursor)
                        .expect("pending")
                        .len(),
                    1
                );
            }
        }
        h.runtime.shutdown().await;
    }
}
