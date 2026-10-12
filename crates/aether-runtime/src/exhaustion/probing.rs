use aether_core::{agent::BlockReason, time::Timestamp};
use std::time::Duration;

/// Longest a suspended agent waits between probes, however long the limit
/// lasts: a week-long window still costs at most 48 probe turns a day.
pub(super) const PROBE_CAP: Duration = Duration::from_secs(30 * 60);

/// A provider may report any retry-after; beyond a day it is treated as a day,
/// so an outlier header cannot park an agent for a month.
pub(super) const REPORTED_RESET_CAP: Duration = Duration::from_secs(24 * 60 * 60);

/// Failed probes this episode after the one being recorded now.
pub(super) fn probes_after(previous: Option<&BlockReason>) -> u32 {
    match previous {
        Some(BlockReason::Exhausted { probes, .. }) => probes.saturating_add(1),
        _ => 0,
    }
}

/// When the agent should try its provider again. Pure: the turn writes it into
/// the row and the scheduler reads it back, so the plan survives a restart.
pub(crate) fn next_probe(
    at: Timestamp,
    previous: Option<&BlockReason>,
    retry_after: Option<Duration>,
    floor: Duration,
) -> Timestamp {
    let interval = match retry_after {
        Some(after) => reported_interval(after, floor),
        None => {
            let doublings = 2u32.checked_pow(probes_after(previous)).unwrap_or(u32::MAX);
            floor.saturating_mul(doublings).min(PROBE_CAP.max(floor))
        }
    };
    at + jitter(interval)
}

/// The provider's own number, bounded on both sides: a zero-second header must
/// not spin, a month-long one must not park for a month. What the row reports
/// as the reset and what it probes at are the same bound, so they agree.
pub(super) fn reported_interval(retry_after: Duration, floor: Duration) -> Duration {
    retry_after.clamp(floor, REPORTED_RESET_CAP.max(floor))
}

/// ±10%, so agents suspended by the same limit do not all probe at once.
fn jitter(interval: Duration) -> chrono::Duration {
    let spread: f64 = rand::random_range(0.9..=1.1);
    let millis = (interval.as_millis() as f64 * spread) as i64;
    chrono::Duration::milliseconds(millis)
}
