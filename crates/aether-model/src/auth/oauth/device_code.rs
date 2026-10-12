//! RFC 8628 device-code polling with `slow_down` handling and a deadline.

use std::future::Future;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::auth::types::AuthError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePoll<T> {
    Pending,
    /// The server asked us to back off; it may name the new interval.
    SlowDown {
        interval: Option<Duration>,
    },
    Complete(T),
    Failed(String),
}

#[derive(Debug, Clone, Copy)]
pub struct DevicePollOptions {
    pub interval: Duration,
    pub expires_in: Duration,
    pub wait_before_first: bool,
}

const MIN_INTERVAL: Duration = Duration::from_secs(1);
/// RFC 8628 section 3.5: `slow_down` adds five seconds when no interval is given.
const SLOW_DOWN_INCREMENT: Duration = Duration::from_secs(5);

pub async fn poll<T, F, Fut>(
    opts: DevicePollOptions,
    cancel: &CancellationToken,
    mut poll_once: F,
) -> Result<T, AuthError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<DevicePoll<T>, AuthError>>,
{
    let deadline = tokio::time::Instant::now() + opts.expires_in;
    let mut interval = opts.interval.max(MIN_INTERVAL);
    let mut slowed = false;

    if opts.wait_before_first {
        sleep_until_or_cancel(interval, deadline, cancel).await?;
    }
    while tokio::time::Instant::now() < deadline {
        if cancel.is_cancelled() {
            return Err(AuthError::Cancelled);
        }
        match poll_once().await? {
            DevicePoll::Complete(v) => return Ok(v),
            DevicePoll::Failed(msg) => return Err(AuthError::Flow(msg)),
            DevicePoll::Pending => {}
            DevicePoll::SlowDown { interval: hinted } => {
                slowed = true;
                interval = hinted
                    .unwrap_or(interval + SLOW_DOWN_INCREMENT)
                    .max(MIN_INTERVAL);
            }
        }
        sleep_until_or_cancel(interval, deadline, cancel).await?;
    }
    Err(AuthError::Flow(if slowed {
        "device login timed out after slow_down responses; check the system clock and retry".into()
    } else {
        "device login timed out".into()
    }))
}

async fn sleep_until_or_cancel(
    interval: Duration,
    deadline: tokio::time::Instant,
    cancel: &CancellationToken,
) -> Result<(), AuthError> {
    let now = tokio::time::Instant::now();
    if now >= deadline {
        return Ok(());
    }
    let wake = (now + interval).min(deadline);
    tokio::select! {
        _ = tokio::time::sleep_until(wake) => Ok(()),
        _ = cancel.cancelled() => Err(AuthError::Cancelled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn opts(interval_ms: u64, expires_ms: u64) -> DevicePollOptions {
        DevicePollOptions {
            interval: Duration::from_millis(interval_ms),
            expires_in: Duration::from_millis(expires_ms),
            wait_before_first: false,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn completes_after_pending_and_slow_down() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let cancel = CancellationToken::new();
        let result = poll(opts(1000, 60_000), &cancel, move || {
            let n = c.fetch_add(1, Ordering::SeqCst);
            async move {
                Ok(match n {
                    0 => DevicePoll::Pending,
                    1 => DevicePoll::SlowDown { interval: None },
                    2 => DevicePoll::Pending,
                    _ => DevicePoll::Complete("token"),
                })
            }
        })
        .await
        .expect("poll");
        assert_eq!(result, "token");
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn times_out_with_slow_down_hint() {
        let cancel = CancellationToken::new();
        let err = poll::<(), _, _>(opts(1000, 3500), &cancel, || async {
            Ok(DevicePoll::SlowDown { interval: None })
        })
        .await
        .unwrap_err();
        assert!(matches!(err, AuthError::Flow(m) if m.contains("slow_down")));
    }

    #[tokio::test(start_paused = true)]
    async fn failure_and_cancellation_propagate() {
        let cancel = CancellationToken::new();
        let err = poll::<(), _, _>(opts(1000, 10_000), &cancel, || async {
            Ok(DevicePoll::Failed("denied".into()))
        })
        .await
        .unwrap_err();
        assert_eq!(err, AuthError::Flow("denied".into()));

        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = poll::<(), _, _>(opts(1000, 10_000), &cancel, || async {
            Ok(DevicePoll::Pending)
        })
        .await
        .unwrap_err();
        assert_eq!(err, AuthError::Cancelled);
    }
}
