//! Bounded native interruption before the process-group termination fallback.

use tokio::io::AsyncRead;
use tokio::sync::mpsc;

use crate::{EngineEvent, EngineOutcome};

// Leave room for the two-second TERM ladder inside the runtime's five-second
// reap window. A silent or incompatible CLI must not postpone cancellation.
const DRAIN_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

pub(crate) async fn drain<R, P>(
    input: &mpsc::WeakSender<String>,
    request: String,
    lines: &mut crate::output::BoundedLines<R>,
    parser: &mut P,
    events: &mpsc::Sender<EngineEvent>,
) -> Option<EngineOutcome>
where
    R: AsyncRead + Unpin,
    P: FnMut(&str) -> Vec<EngineEvent>,
{
    let Some(input) = input.upgrade() else {
        tracing::debug!("native interrupt unavailable after stdin closed; terminating group");
        return None;
    };
    if let Err(error) = input.try_send(request) {
        tracing::warn!(%error, "native interrupt could not be queued; terminating group");
        return None;
    }
    let result = tokio::time::timeout(DRAIN_GRACE, async {
        while let Some(line) = lines.next_line().await? {
            for event in parser(&line) {
                if let EngineEvent::Done(outcome) = event {
                    return Ok::<_, std::io::Error>(Some(outcome));
                }
                if events.send(event).await.is_err() {
                    return Ok(None);
                }
            }
        }
        Ok(None)
    })
    .await;
    match result {
        Ok(Ok(outcome)) => return outcome,
        Ok(Err(error)) => {
            tracing::warn!(%error, "native interrupt output failed; terminating group")
        }
        Err(_) => tracing::warn!("native interrupt drain timed out; terminating group"),
    }
    None
}

#[cfg(test)]
mod tests;
