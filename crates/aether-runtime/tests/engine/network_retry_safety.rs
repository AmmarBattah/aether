//! Waiting out a transient native failure never races the failed process,
//! spends past a grant, outlives its window or drops a retained session.
use super::network_retry::{
    OFFLINE, failure, failures, partial, queued_with, resumed_chat, retries, session,
};
use super::*;
use aether_core::model::{ModelErrorClass, Usage};

const SERVER: &str = "API Error: 500 Internal server error";

#[tokio::test]
async fn late_exhaustion_after_a_network_or_server_failure_is_never_retried() {
    for error in [OFFLINE, SERVER] {
        let (h, engine, id) = resumed_chat(
            vec![
                vec![
                    session("retained"),
                    partial("halfway"),
                    failure(error),
                    failure("Weekly limit reached"),
                ],
                answered("must not run"),
            ],
            |_| {},
        )
        .await;
        h.runtime
            .send_message(id, vec![ContentBlock::text("go")], Actor::user())
            .unwrap();
        let blocked = wait_for(h.store.as_ref(), id, |a| {
            matches!(a.status, AgentStatus::Blocked { .. }) || a.status.is_terminal()
        })
        .await;
        assert!(
            matches!(
                &blocked.status,
                AgentStatus::Blocked {
                    reason: BlockReason::Exhausted {
                        class: ExhaustionClass::UsageLimit,
                        ..
                    }
                }
            ),
            "{error}: {:?}",
            blocked.status
        );
        assert_eq!(engine.prompts.lock().unwrap().len(), 2, "{error}");
        assert!(retries(&h, id).is_empty(), "{error}");
        h.runtime.shutdown().await;
    }
}

/// How the failed first launch's stream closes after the runtime stops it.
#[derive(Clone, Copy)]
enum Exit {
    /// Open for a minute, as a process that ignores its stop would.
    Never,
    /// Promptly, but only after a host stall that outlasts every grace.
    AfterAHostStall,
}

/// Reports a failure on a known session, then closes its stream as `exit`
/// says; later launches answer.
struct StalledEngine {
    launches: std::sync::atomic::AtomicUsize,
    exit: Exit,
}

impl StalledEngine {
    fn new(exit: Exit) -> Arc<Self> {
        Arc::new(Self {
            launches: std::sync::atomic::AtomicUsize::new(0),
            exit,
        })
    }
}

#[async_trait::async_trait]
impl Engine for StalledEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }
    fn adapter(&self) -> String {
        "stalled/1".into()
    }
    async fn run(
        &self,
        _turn: EngineTurn,
        cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        let launch = self
            .launches
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(8);
        let exit = self.exit;
        tokio::spawn(async move {
            if launch > 0 {
                for event in answered("recovered") {
                    tx.send(event).await.unwrap();
                }
                return;
            }
            for event in [session("stalled"), partial("halfway"), failure(OFFLINE)] {
                tx.send(event).await.unwrap();
            }
            match exit {
                Exit::Never => tokio::time::sleep(Duration::from_secs(60)).await,
                Exit::AfterAHostStall => {
                    cancel.cancelled().await;
                    // The clock passes the adapter's TERM grace and the
                    // runtime's reap grace while nothing runs; the exit
                    // lands once this task runs again.
                    tokio::time::advance(Duration::from_secs(120)).await;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
            drop(tx);
        });
        Ok(EngineRun {
            events: rx,
            input: None,
            decisions: None,
            stop: None,
        })
    }
}

#[tokio::test]
async fn a_failed_engine_that_never_exits_is_not_resumed() {
    let engine = StalledEngine::new(Exit::Never);
    let h = harness_with_engine_config(engine.clone(), |_| {}).await;
    let (id, _) = engine_worker(&h, "outlive the cleanup grace");
    let failed = wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
    assert!(
        matches!(&failed.status, AgentStatus::Failed { error }
            if error.contains("EAI_AGAIN") && error.contains("did not exit")),
        "{:?}",
        failed.status
    );
    assert_eq!(engine.launches.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(retries(&h, id).is_empty());
    h.runtime.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn a_failed_engine_that_exits_after_a_host_stall_is_resumed() {
    let engine = StalledEngine::new(Exit::AfterAHostStall);
    let h = harness_with_engine_config(engine.clone(), |_| {}).await;
    let (id, _) = engine_worker(&h, "exit after the stall, then recover");
    wait_for(h.store.as_ref(), id, |_| {
        engine.launches.load(std::sync::atomic::Ordering::SeqCst) == 2
    })
    .await;
    let retried = retries(&h, id);
    assert_eq!(retried.len(), 1, "{retried:?}");
    assert_eq!(retried[0]["resume"], "continue");
    h.runtime.shutdown().await;
}

fn cost_budgeted_agent(h: &Harness, dollars: f64) -> AgentId {
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    cost_budget: Some(dollars),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .unwrap();
    agent.id
}

fn budget_stops(h: &Harness, id: AgentId) -> usize {
    h.store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .unwrap()
        .iter()
        .filter(|e| matches!(e.kind, EventKind::BudgetExceeded { .. }))
        .count()
}

#[tokio::test]
async fn a_closing_receipt_over_the_grant_stops_before_another_launch() {
    let over_tokens = EngineOutcome {
        usage: Usage {
            input_tokens: 100,
            ..Default::default()
        },
        ..Default::default()
    };
    let over_cost = EngineOutcome {
        cost_usd: Some(0.5),
        ..Default::default()
    };
    // A spent window gives up, but never ahead of the grant the receipt crossed.
    let windows = [None, Some(Duration::ZERO)];
    let cases = [(over_tokens, "token budget"), (over_cost, "cost budget")];
    for ((receipt, budget), window) in cases
        .into_iter()
        .flat_map(|case| windows.map(|window| (case.clone(), window)))
    {
        let closing = EngineEvent::Done(EngineOutcome {
            is_error: true,
            error: Some(OFFLINE.into()),
            ..receipt
        });
        let (h, engine) = queued_with(
            vec![vec![session("retained"), closing], answered("must not run")],
            |config| {
                if let Some(window) = window {
                    config.engine_transient_retry_window = window;
                }
            },
        )
        .await;
        let id = match budget {
            "token budget" => budgeted_agent(&h, 50),
            _ => cost_budgeted_agent(&h, 0.1),
        };
        h.runtime
            .send_message(id, vec![ContentBlock::text("go")], Actor::user())
            .unwrap();
        let stopped = wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
        assert!(
            format!("{:?}", stopped.status).contains(budget),
            "{window:?}: {:?}",
            stopped.status
        );
        assert_eq!(budget_stops(&h, id), 1, "{budget} {window:?}");
        assert_eq!(engine.prompts.lock().unwrap().len(), 1, "{budget}");
        assert!(retries(&h, id).is_empty(), "no wait once the grant is gone");
        h.runtime.shutdown().await;
    }
}

/// Plays one queued run per launch, holding each run's last event back by
/// `pause`, as an engine still closing down would.
struct SlowClosingEngine {
    runs: Mutex<std::collections::VecDeque<Vec<EngineEvent>>>,
    pause: Duration,
    launches: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl Engine for SlowClosingEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::CLAUDE_CODE
    }
    fn adapter(&self) -> String {
        "slow-closing/1".into()
    }
    async fn run(
        &self,
        _turn: EngineTurn,
        _cancel: CancellationToken,
    ) -> Result<EngineRun, EngineError> {
        self.launches
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut events = self.runs.lock().unwrap().pop_front().unwrap_or_default();
        let last = events.pop();
        let pause = self.pause;
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            for event in events {
                tx.send(event).await.unwrap();
            }
            tokio::time::sleep(pause).await;
            if let Some(last) = last {
                let _ = tx.send(last).await;
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

#[tokio::test]
async fn a_window_spent_while_the_last_run_closes_still_yields_to_the_grant() {
    let over_tokens = EngineOutcome {
        usage: Usage {
            input_tokens: 100,
            ..Default::default()
        },
        ..Default::default()
    };
    let over_cost = EngineOutcome {
        cost_usd: Some(0.5),
        ..Default::default()
    };
    for (receipt, budget) in [(over_tokens, "token budget"), (over_cost, "cost budget")] {
        let engine = Arc::new(SlowClosingEngine {
            runs: Mutex::new(
                vec![
                    vec![session("retained"), partial("halfway"), failure(OFFLINE)],
                    // The closing receipt lands after the window has run out.
                    vec![failure(OFFLINE), EngineEvent::Done(receipt)],
                    answered("must not run"),
                ]
                .into(),
            ),
            pause: Duration::from_millis(300),
            launches: std::sync::atomic::AtomicUsize::new(0),
        });
        let h = harness_with_engine_config(engine.clone(), |config| {
            config.engine_transient_retry_window = Duration::from_millis(100);
        })
        .await;
        let id = match budget {
            "token budget" => budgeted_agent(&h, 50),
            _ => cost_budgeted_agent(&h, 0.1),
        };
        h.runtime
            .send_message(id, vec![ContentBlock::text("go")], Actor::user())
            .unwrap();
        let stopped = wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
        assert!(
            matches!(&stopped.status, AgentStatus::Failed { error }
                if error.contains(budget) && !error.contains("gave up")),
            "{:?}",
            stopped.status
        );
        assert_eq!(budget_stops(&h, id), 1, "{budget}");
        assert_eq!(
            engine.launches.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "{budget}"
        );
        assert_eq!(retries(&h, id).len(), 1, "{budget}");
        h.runtime.shutdown().await;
    }
}

#[tokio::test]
async fn partial_progress_before_every_failure_cannot_extend_the_window() {
    let window = Duration::from_millis(150);
    let (h, engine) = queued_with(
        (0..1_000)
            .map(|_| vec![session("s"), partial("a little"), failure(OFFLINE)])
            .collect(),
        |config| config.engine_transient_retry_window = window,
    )
    .await;
    let started = tokio::time::Instant::now();
    let (id, _) = engine_worker(&h, "progress a little, then fail");
    let failed = wait_for(h.store.as_ref(), id, |a| a.status.is_terminal()).await;
    assert!(
        matches!(&failed.status, AgentStatus::Failed { error } if error.contains("gave up")),
        "{:?}",
        failed.status
    );
    // 10ms, 20ms, 40ms, 80ms, then cut to the window: never more launches.
    let launches = engine.prompts.lock().unwrap().len();
    assert!((2..=5).contains(&launches), "{launches} launches");
    assert!(started.elapsed() < Duration::from_secs(5));
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn a_submitted_but_unanswered_prompt_on_a_known_session_is_rechecked_not_replayed() {
    for (error, class) in [
        (
            "codex app-server did not start the turn in time",
            ModelErrorClass::Timeout,
        ),
        (
            "stream disconnected before completion",
            ModelErrorClass::Network,
        ),
        (SERVER, ModelErrorClass::Server),
    ] {
        check_recheck(error, class).await;
    }
}

async fn check_recheck(error: &str, class: ModelErrorClass) {
    let (h, engine, id) = resumed_chat(
        vec![
            vec![EngineEvent::Done(EngineOutcome {
                is_error: true,
                error: Some(error.into()),
                // Submitted: whether the session holds the prompt is unknown.
                prompt_not_submitted: false,
                ..Default::default()
            })],
            answered("recovered"),
        ],
        |_| {},
    )
    .await;
    engine_turn(&h, id, "do the work").await;
    assert_eq!(
        *engine.sessions.lock().unwrap(),
        [None, Some("retained".into()), Some("retained".into())]
    );
    let prompts = engine.prompts.lock().unwrap().clone();
    assert!(
        prompts[2].contains(&format!(
            "transient {} error interrupted delivery of the message below",
            class.as_str()
        )),
        "{}",
        prompts[2]
    );
    assert_ne!(prompts[2], prompts[1], "{error}: never a blind replay");
    assert!(
        prompts[2].ends_with(&prompts[1]),
        "the prompt is carried, not dropped"
    );
    assert_eq!(failures(&h, id), [(class, true)]);
    assert_eq!(retries(&h, id)[0]["resume"], "recheck", "{error}");
    assert!(
        spoken(h.store.as_ref(), id)
            .iter()
            .any(|(role, text)| *role == MessageRole::Assistant && text == "recovered")
    );
    h.runtime.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn a_host_stall_does_not_spend_a_wait() {
    let h = harness(Reply::Accept).await;
    let id = chat_agent(&h);
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    tokio::spawn({
        let done = done.clone();
        async move {
            // The clock jumps past any deadline while nothing runs; the
            // awaited change lands only once the runtime runs again.
            tokio::time::advance(Duration::from_secs(60)).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            done.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    wait_for(h.store.as_ref(), id, |_| {
        done.load(std::sync::atomic::Ordering::SeqCst)
    })
    .await;
    h.runtime.shutdown().await;
}

/// A fake `codex app-server` that plays one line of `plan` per launch and
/// logs each launch's requests to `log.<n>`. JSON-RPC ids are sequential:
/// initialize, config read, thread start/resume, MCP status, turn start.
const FAKE_APP_SERVER: &str = r#"#!/bin/sh
dir=$(dirname "$0")
n=$(($(cat "$dir/n" 2>/dev/null || echo 0) + 1))
echo "$n" > "$dir/n"
mode=$(sed -n "${n}p" "$dir/plan")
reply() { read -r line; printf '%s\n' "$line" >> "$dir/log.$n"; printf '%s\n' "$1"; }
reply '{"jsonrpc":"2.0","id":1,"result":{}}'
reply '{"jsonrpc":"2.0","id":2,"result":{"layers":[{"config":{}}]}}'
if [ "$mode" = resume_offline ]; then
  reply '{"jsonrpc":"2.0","id":3,"error":{"code":-32000,"message":"failed to load thread: getaddrinfo EAI_AGAIN chatgpt.com"}}'
  exit 0
fi
reply '{"jsonrpc":"2.0","id":3,"result":{"thread":{"id":"retained"},"model":"gpt-test"}}'
reply '{"jsonrpc":"2.0","id":4,"result":{"data":[{"name":"aether","runtimeStatus":"connected","tools":{"delegate":{}}}]}}'
reply '{"jsonrpc":"2.0","id":5,"result":{"turn":{"id":"turn"}}}'
if [ "$mode" = submitted_offline ]; then
  printf '%s\n' '{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"stream disconnected before completion"}}}}'
elif [ "$mode" = partial_offline ]; then
  printf '%s\n' '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"delta":"halfway"}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"stream disconnected before completion"}}}}'
else
  printf '%s\n' '{"jsonrpc":"2.0","method":"item/completed","params":{"item":{"type":"agentMessage","id":"m","text":"done"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"status":"completed"}}}'
fi
"#;

/// The fake server's directory, its `plan` written one mode per launch.
fn fake_app_server(plan: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("plan"), plan.join("\n") + "\n").unwrap();
    // Written by a child, so no fd this process holds can make a sibling
    // test's exec of it fail with ETXTBSY.
    let script = dir.path().join("codex");
    let status = std::process::Command::new("/bin/sh")
        .args(["-c", r#"printf '%s' "$1" > "$2" && chmod 700 "$2""#, "sh"])
        .arg(FAKE_APP_SERVER)
        .arg(&script)
        .status()
        .unwrap();
    assert!(status.success());
    dir
}

/// The requests the fake server logged on its `launch`th launch.
fn requests(dir: &tempfile::TempDir, launch: usize) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.path().join(format!("log.{launch}")))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn launches(dir: &tempfile::TempDir) -> usize {
    std::fs::read_to_string(dir.path().join("n"))
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn a_codex_thread_that_cannot_be_reopened_offline_keeps_its_context() {
    // Before any progress, and after a continued turn already made some.
    for plan in [
        &["ok", "resume_offline", "ok"][..],
        &["ok", "partial_offline", "resume_offline", "ok"][..],
        // Dispatched but unanswered: the thread may already hold the prompt.
        &["ok", "submitted_offline", "ok"][..],
    ] {
        let dir = fake_app_server(plan);
        let program = dir.path().join("codex");
        let engine = Arc::new(aether_engine::codex_app::CodexAppEngine::new(
            program.to_string_lossy(),
            "0.162.0",
        ));
        let h = harness_with_engine_config(engine, |_| {}).await;
        let id = chat_agent(&h);
        engine_turn(&h, id, "first").await;
        engine_turn(&h, id, "do the work").await;
        assert_eq!(launches(&dir), plan.len(), "{plan:?}");
        for launch in 2..=plan.len() {
            let thread = &requests(&dir, launch)[2];
            assert_eq!(thread["method"], "thread/resume", "{plan:?} {launch}");
            assert_eq!(thread["params"]["threadId"], "retained", "{plan:?}");
        }
        let prompt = |launch| {
            // The prompt is the last input; a network note may precede it.
            let input = requests(&dir, launch)[4]["params"]["input"].clone();
            input.as_array().unwrap().last().unwrap()["text"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let last = prompt(plan.len());
        if plan.contains(&"partial_offline") {
            assert!(prompt(2).contains("do the work"));
            assert!(
                last.contains("interrupted by a transient network error"),
                "{last}"
            );
            assert!(!last.contains("do the work"), "never replayed: {last}");
        } else if plan.contains(&"submitted_offline") {
            assert!(
                last.contains("interrupted delivery of the message below")
                    && last.ends_with(&prompt(2)),
                "rechecked, not replayed: {last}"
            );
        } else {
            assert!(
                !last.contains("interrupted delivery") && last.contains("do the work"),
                "replayed unsent: {last}"
            );
        }
        let agent = h.store.agent(id).unwrap().unwrap();
        assert_eq!(agent.engine_session.as_deref(), Some("retained"));
        assert_eq!(agent.status.kind(), AgentStatusKind::Waiting);
        let recorded = failures(&h, id);
        assert_eq!(recorded.len(), plan.len() - 2, "{plan:?}");
        assert!(
            recorded
                .iter()
                .all(|f| *f == (ModelErrorClass::Network, true))
        );
        h.runtime.shutdown().await;
    }
}
