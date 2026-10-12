use super::*;

/// Fails with a retryable error a fixed number of times, then succeeds.
struct Flaky {
    class: SideEffectClass,
    remaining_failures: AtomicUsize,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for Flaky {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            spec: ToolSpec {
                name: "flaky".into(),
                description: "flaky".into(),
                input_schema: json!({"type": "object"}),
                strict: false,
            },
            side_effect: self.class,
            needs_network: false,
        }
    }

    fn resources(&self, _input: &Value, _ctx: &ToolContext) -> Vec<ResourceKey> {
        Vec::new()
    }

    async fn execute(
        &self,
        _input: Value,
        _ctx: &ToolContext,
        _cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.remaining_failures.fetch_sub(1, Ordering::SeqCst) > 0 {
            return Err(ToolError::Timeout);
        }
        Ok(ToolExecution::Outcome(ToolOutcome::ok("settled")))
    }
}

async fn run_flaky(class: SideEffectClass, failures: usize) -> (usize, Vec<String>, bool) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Arc::new(ctx_in(dir.path()));
    let calls = Arc::new(AtomicUsize::new(0));
    let tool = Arc::new(Flaky {
        class,
        remaining_failures: AtomicUsize::new(failures),
        calls: calls.clone(),
    }) as Arc<dyn Tool>;
    let results = ToolRuntime::new()
        .execute_batch(
            vec![(call("flaky", json!({})), tool)],
            ctx,
            CancellationToken::new(),
            CancellationToken::new(),
            &ExecutionPolicy::default(),
        )
        .await;
    let ok = results[0].result.is_ok();
    (calls.load(Ordering::SeqCst), results[0].retried.clone(), ok)
}

#[tokio::test]
async fn the_side_effect_class_decides_how_often_a_call_is_retried() {
    // A read may be repeated, so a transient failure is tried again.
    let (calls, retried, ok) = run_flaky(SideEffectClass::ReadOnly, 2).await;
    assert_eq!(calls, 3);
    assert_eq!(retried.len(), 2);
    assert!(ok);

    // A call that can compound its effect gets exactly one attempt.
    let (calls, retried, ok) = run_flaky(SideEffectClass::Mutation, 1).await;
    assert_eq!(calls, 1, "a non-idempotent call is never retried");
    assert!(retried.is_empty());
    assert!(!ok);

    // Repeating an idempotent mutation is safe, so it is retried like a
    // read; anything reaching outside gets one second chance, not two.
    let (calls, retried, ok) = run_flaky(SideEffectClass::IdempotentMutation, 2).await;
    assert_eq!(calls, 3);
    assert_eq!(retried.len(), 2);
    assert!(ok);
    let (calls, retried, ok) = run_flaky(SideEffectClass::External, 1).await;
    assert_eq!(calls, 2, "external gets two attempts");
    assert_eq!(retried.len(), 1);
    assert!(ok);
    let (calls, retried, ok) = run_flaky(SideEffectClass::External, 2).await;
    assert_eq!(calls, 2, "and no more than two");
    assert_eq!(retried.len(), 1);
    assert!(!ok);
}

/// Never settles, so the only thing that can end it is the deadline.
struct Hangs;

#[async_trait]
impl Tool for Hangs {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            spec: ToolSpec {
                name: "hangs".into(),
                description: "never returns".into(),
                input_schema: json!({"type": "object"}),
                strict: false,
            },
            side_effect: SideEffectClass::ReadOnly,
            needs_network: false,
        }
    }

    fn resources(&self, _input: &Value, _ctx: &ToolContext) -> Vec<ResourceKey> {
        Vec::new()
    }

    async fn execute(
        &self,
        _input: Value,
        _ctx: &ToolContext,
        _cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolError> {
        std::future::pending().await
    }
}

#[tokio::test]
async fn retrying_does_not_multiply_the_call_s_time_budget() {
    // Three attempts on a 300ms timeout used to mean 900ms of wall
    // clock; the caller was promised 300.
    let dir = tempfile::tempdir().unwrap();
    let ctx = Arc::new(ctx_in(dir.path()));
    let policy = ExecutionPolicy {
        default_timeout: Duration::from_millis(300),
        shell_timeout: Duration::from_millis(300),
    };
    let started = std::time::Instant::now();
    let results = ToolRuntime::new()
        .execute_batch(
            vec![(call("hangs", json!({})), Arc::new(Hangs) as Arc<dyn Tool>)],
            ctx,
            CancellationToken::new(),
            CancellationToken::new(),
            &policy,
        )
        .await;
    let elapsed = started.elapsed();
    assert!(
        matches!(results[0].result, Err(ToolError::Timeout)),
        "{:?}",
        results[0].result
    );
    assert!(
        elapsed < Duration::from_millis(800),
        "one deadline covers every attempt, but the call took {elapsed:?}"
    );
    assert!(
        results[0].retried.is_empty(),
        "no budget was left, so nothing was tried again"
    );
    // The log says how many attempts the class allowed, not how many
    // happened to be made.
    assert_eq!(results[0].attempts_allowed, 3);
}
