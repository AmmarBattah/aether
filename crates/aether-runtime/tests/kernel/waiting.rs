//! waiting scenarios and fixtures.
use super::*;

/// Bounds a poll loop by stall time (no observable progress), not total wall
/// time, while a `hard_ceiling` still fails a genuine hang.
///
/// A fixed wall-clock deadline treats CPU-starved scheduling delay the same
/// as a real hang. Tracking progress instead means a loaded host that keeps
/// making (slow) headway is tolerated, while true stalls still fail fast.
struct StallClock {
    start: tokio::time::Instant,
    hard_ceiling: Duration,
    stall_window: Duration,
    last_progress_at: tokio::time::Instant,
}

impl StallClock {
    pub(super) fn new(hard_ceiling: Duration, stall_window: Duration) -> Self {
        assert!(
            stall_window <= hard_ceiling,
            "stall window must not exceed the hard ceiling"
        );
        let start = tokio::time::Instant::now();
        Self {
            start,
            hard_ceiling,
            stall_window,
            last_progress_at: start,
        }
    }

    /// Reset the stall window when `changed` reports real progress.
    pub(super) fn observe(&mut self, changed: bool) {
        if changed {
            self.last_progress_at = tokio::time::Instant::now();
        }
    }

    /// `Some(reason)` once either bound is exceeded; `None` to keep polling.
    pub(super) fn expired(&self) -> Option<&'static str> {
        let now = tokio::time::Instant::now();
        if now.duration_since(self.start) >= self.hard_ceiling {
            return Some("hard ceiling exceeded");
        }
        if now.duration_since(self.last_progress_at) >= self.stall_window {
            return Some("no observable progress");
        }
        None
    }

    pub(super) fn elapsed(&self) -> Duration {
        tokio::time::Instant::now().duration_since(self.start)
    }
}

/// `/proc/loadavg` on Linux, `uptime` elsewhere — so a load-induced flake is
/// diagnosable from the failure message alone, without re-running under CI.
pub(super) fn system_load_snapshot() -> String {
    #[cfg(target_os = "linux")]
    {
        match std::fs::read_to_string("/proc/loadavg") {
            Ok(contents) => format!("loadavg: {}", contents.trim()),
            Err(err) => format!("loadavg unavailable: {err}"),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        match std::process::Command::new("uptime").output() {
            Ok(out) => format!("uptime: {}", String::from_utf8_lossy(&out.stdout).trim()),
            Err(err) => format!("uptime unavailable: {err}"),
        }
    }
}

/// The old fast-fail ceiling: what every `wait_until` once used, kept
/// as the default so a genuinely hung non-bash test (138 of 154) still
/// fails in tens of seconds, not two minutes.
pub(super) const WAIT_UNTIL_FAST_HARD_CEILING: Duration = Duration::from_secs(30);

/// The gated ceiling for a harness that scripted a real `bash` call, which
/// can be legitimately slow to schedule under host contention.
pub(super) const WAIT_UNTIL_BASH_HARD_CEILING: Duration = Duration::from_secs(120);

pub(super) const WAIT_UNTIL_STALL_WINDOW: Duration = Duration::from_secs(15);

/// Polls `probe` until it returns `Some`, using the same stall/ceiling
/// clock as `wait_until_with_budget` for a wait that has no single agent to
/// read progress from. `progress` is a snapshot that changes
/// when the wait is making headway; pass equal `hard_ceiling`/`stall_window`
/// when no such signal exists, which makes this a ceiling-only wait. The
/// panic is always labelled TIMEOUT so a slow-but-real wait under
/// contention never reads as a product-bug assertion failure.
pub(super) async fn wait_for_with_budget<T, P: PartialEq>(
    hard_ceiling: Duration,
    stall_window: Duration,
    mut probe: impl FnMut() -> Option<T>,
    mut progress: impl FnMut() -> P,
    describe: impl FnOnce() -> String,
) -> T {
    let mut clock = StallClock::new(hard_ceiling, stall_window);
    let mut last = progress();
    loop {
        if let Some(value) = probe() {
            return value;
        }
        let current = progress();
        clock.observe(last != current);
        last = current;
        if let Some(reason) = clock.expired() {
            panic!(
                "TIMEOUT: {} ({reason}) after {:?}\n{}",
                describe(),
                clock.elapsed(),
                system_load_snapshot()
            );
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// The activity signature used as `wait_until`'s progress signal: any change
/// in status, turn count, tool-call count, or last-turn time counts as
/// movement, even short of the predicate the caller is waiting for.
type ActivitySignature = (AgentStatusKind, u32, u32, Option<Timestamp>);

pub(super) fn activity_signature(agent: &aether_core::agent::Agent) -> ActivitySignature {
    (
        agent.status.kind(),
        agent.stats.turns,
        agent.stats.tool_calls,
        agent.stats.last_turn_at,
    )
}

pub(super) async fn wait_until_with_budget(
    store: &dyn Store,
    id: AgentId,
    pred: impl Fn(&AgentStatus) -> bool,
    hard_ceiling: Duration,
    stall_window: Duration,
) -> AgentStatus {
    let mut clock = StallClock::new(hard_ceiling, stall_window);
    let mut last_activity: Option<ActivitySignature> = None;
    loop {
        let agent = store.agent(id).expect("agent").expect("exists");
        if pred(&agent.status) {
            return agent.status;
        }
        let activity = activity_signature(&agent);
        clock.observe(last_activity != Some(activity));
        last_activity = Some(activity);
        if let Some(reason) = clock.expired() {
            panic!(
                "timed out waiting ({reason}) after {:?}: {}\n{}",
                clock.elapsed(),
                timeout_details(store, id),
                system_load_snapshot()
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub(super) fn timeout_details(store: &dyn Store, id: AgentId) -> String {
    let agent = store.agent(id).expect("timeout agent").expect("exists");
    let events = store
        .events(0, 10_000, &EventFilter::for_agent(id))
        .expect("timeout events");
    let recent: Vec<_> = events
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|event| {
            let detail = match &event.kind {
                EventKind::ToolRequested { call, .. } => format!(" {} {}", call.name, call.id),
                EventKind::ToolStarted { call } => format!(" {call}"),
                EventKind::ToolSucceeded { call, outcome } => {
                    format!(" {call} {}ms", outcome.duration_ms)
                }
                _ => String::new(),
            };
            format!("{} {} {}{}", event.seq, event.at, event.type_name(), detail)
        })
        .collect();
    format!(
        "status {:?}; turns {}; tool calls {}; recent events:\n{}",
        agent.status,
        agent.stats.turns,
        agent.stats.tool_calls,
        recent.join("\n")
    )
}

const MARKER_HARD_CEILING: Duration = Duration::from_secs(120);

const MARKER_STALL_WINDOW: Duration = Duration::from_secs(20);

/// A background process's PID marker file, written via `echo $$ > x.tmp &&
/// mv x.tmp x`. The directory listing is the progress signal: a real hang
/// never touches it either, so it still resets the stall clock honestly.
pub(super) async fn wait_for_marker(workspace_root: &std::path::Path, marker_name: &str) -> i32 {
    wait_for_marker_with_budget(
        workspace_root,
        marker_name,
        MARKER_HARD_CEILING,
        MARKER_STALL_WINDOW,
    )
    .await
}

async fn wait_for_marker_with_budget(
    workspace_root: &std::path::Path,
    marker_name: &str,
    hard_ceiling: Duration,
    stall_window: Duration,
) -> i32 {
    let marker = workspace_root.join(marker_name);
    let mut clock = StallClock::new(hard_ceiling, stall_window);
    let mut last_listing = dir_listing(workspace_root);
    loop {
        if marker.exists() {
            break;
        }
        let listing = dir_listing(workspace_root);
        clock.observe(listing != last_listing);
        last_listing = listing;
        if let Some(reason) = clock.expired() {
            panic!(
                "timed out waiting for pid marker {} ({reason}) after {:?}; workspace listing: {:?}\n{}",
                marker.display(),
                clock.elapsed(),
                last_listing,
                system_load_snapshot()
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let contents = std::fs::read_to_string(&marker).unwrap_or_else(|err| {
        panic!(
            "pid marker {} vanished after appearing: {err}",
            marker.display()
        )
    });
    contents.trim().parse().unwrap_or_else(|err| {
        panic!(
            "pid marker {} did not contain a pid: {err}",
            marker.display()
        )
    })
}

/// The directory's entry names, or the io error reading it — a wrong or
/// deleted workspace root must surface as that error, not a silently
/// stable empty listing that reads as "nothing is happening".
fn dir_listing(dir: &std::path::Path) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|err| err.to_string())?
        .filter_map(|entry| {
            entry
                .ok()
                .map(|e| e.file_name().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    Ok(names)
}

/// Reads a panic payload as text; a caught panic from an async fn is either
/// a `&'static str` (a `panic!("literal")`) or a `String` (`panic!("{x}")`).
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

/// A stalled fixture (no progress ever observed) fails once
/// the stall window elapses, well inside the hard ceiling.
#[tokio::test(start_paused = true)]
async fn stall_clock_fails_fast_on_a_genuine_stall() {
    let hard_ceiling = Duration::from_secs(120);
    let stall_window = Duration::from_secs(5);
    let mut clock = StallClock::new(hard_ceiling, stall_window);
    loop {
        clock.observe(false);
        if let Some(reason) = clock.expired() {
            assert_eq!(reason, "no observable progress");
            assert!(
                clock.elapsed() < hard_ceiling,
                "a stall must be caught long before the hard ceiling: {:?}",
                clock.elapsed()
            );
            return;
        }
        tokio::time::advance(Duration::from_millis(200)).await;
    }
}

/// A slow-but-progressing fixture never trips the stall
/// window, even after it runs well past what a flat deadline would allow.
#[tokio::test(start_paused = true)]
async fn stall_clock_does_not_false_time_out_on_steady_progress() {
    let hard_ceiling = Duration::from_secs(120);
    let stall_window = Duration::from_secs(5);
    let mut clock = StallClock::new(hard_ceiling, stall_window);
    let steps = 30; // 30 * 3s = 90s of progress: past the old 15s deadline, short of the 120s ceiling.
    for _ in 0..steps {
        tokio::time::advance(Duration::from_secs(3)).await;
        clock.observe(true);
        assert_eq!(
            clock.expired(),
            None,
            "progress every 3s must never trip a 5s stall window"
        );
    }
}

/// Intermittent progress still can't outrun the hard
/// ceiling — a real hang-with-occasional-twitches still fails.
#[tokio::test(start_paused = true)]
async fn stall_clock_still_enforces_the_hard_ceiling() {
    let hard_ceiling = Duration::from_secs(10);
    let stall_window = Duration::from_secs(5);
    let mut clock = StallClock::new(hard_ceiling, stall_window);
    // 3 ticks (12s) must trip the 10s ceiling; a generous cap so a
    // regression here fails the test instead of hanging the binary.
    const MAX_TICKS: usize = 10;
    for _ in 0..MAX_TICKS {
        tokio::time::advance(Duration::from_secs(4)).await;
        clock.observe(true); // progress every 4s, always under the 5s stall window
        if let Some(reason) = clock.expired() {
            assert_eq!(reason, "hard ceiling exceeded");
            return;
        }
    }
    panic!(
        "hard ceiling never tripped after {MAX_TICKS} ticks ({:?}): StallClock::expired regressed",
        clock.elapsed()
    );
}

/// The marker-wait masking bug. On timeout it must panic
/// with a timeout message naming the marker path, never a raw io::Error.
#[tokio::test(start_paused = true)]
async fn marker_wait_times_out_with_a_clear_message_not_a_missing_file_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let outcome = std::panic::AssertUnwindSafe(wait_for_marker_with_budget(
        dir.path(),
        "never-appears.pid",
        Duration::from_secs(1),
        Duration::from_millis(200),
    ))
    .catch_unwind()
    .await;
    let payload = outcome.expect_err("a marker that never appears must panic");
    let message = panic_message(payload.as_ref());
    assert!(
        message.contains("timed out waiting for pid marker"),
        "message must name a timeout, not mask it: {message}"
    );
    assert!(
        message.contains("never-appears.pid"),
        "message must name the marker path: {message}"
    );
    assert!(
        !message.contains("No such file or directory"),
        "must not fall through to a raw io::Error: {message}"
    );
}

/// The concurrency-gate detector must key off a real `bash` tool call, not
/// any string that happens to say "bash" (e.g. a seeded fixture's label).
#[test]
fn spawns_real_bash_detects_only_a_real_bash_tool_call() {
    assert!(spawns_real_bash(&[ScriptedClient::tool_call(
        "bash",
        json!({"command": "true"})
    )]));
    assert!(!spawns_real_bash(&[ScriptedClient::tool_call(
        "read_file",
        json!({"path": "x"})
    )]));
    assert!(!spawns_real_bash(&[ScriptedClient::text(
        "bash is mentioned but not called"
    )]));
    assert!(!spawns_real_bash(&[]));
    // The batch form used by two_identical_calls_in_one_batch_both_run.
    assert!(spawns_real_bash(&[ScriptedClient::tool_calls(
        &[
            ("bash", json!({"command": "true"})),
            ("bash", json!({"command": "true"})),
        ],
        None,
    )]));
    // Detection only inspects the responses given at construction time; a
    // test that later `client.push`es a bash response bypasses the gate.
}
