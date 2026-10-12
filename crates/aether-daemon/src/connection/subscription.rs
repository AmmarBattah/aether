use crate::Daemon;
use aether_protocol::{Delta, ErrorReply, ServerFrame};
use aether_runtime::{BusMessage, DeltaKind};
use aether_store::EventFilter;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(super) struct SubscriptionControl {
    pub cancel: CancellationToken,
    pub failure: mpsc::Sender<ErrorReply>,
}

impl SubscriptionControl {
    async fn send(&self, out: &mpsc::Sender<ServerFrame>, frame: ServerFrame) -> bool {
        tokio::select! {
            biased;
            _ = self.cancel.cancelled() => false,
            sent = out.send(frame) => sent.is_ok(),
        }
    }

    fn failed(&self, error: aether_store::StoreError) {
        tracing::warn!(%error, "subscription replay failed; closing its connection");
        match self.failure.try_send(super::store_err(error)) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::debug!("an earlier replay failure is already closing this connection");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!("subscription connection already closed");
            }
        }
    }
}

/// Replay from the store, then follow the bus without gaps or duplicates.
pub(super) fn spawn_subscription(
    daemon: Arc<Daemon>,
    out: mpsc::Sender<ServerFrame>,
    sub: u64,
    after: Option<u64>,
    filter: EventFilter,
    deltas: bool,
    control: SubscriptionControl,
) -> aether_store::Result<u64> {
    let store = daemon.runtime.store().clone();
    let mut rx = daemon.runtime.subscribe();
    let mut last = after.unwrap_or(store.last_seq()?);
    let start_seq = last;
    tokio::spawn(async move {
        // Replay history in pages before switching to live delivery.
        let mut cursor = start_seq;
        loop {
            if control.cancel.is_cancelled() {
                return;
            }
            let page = match store.events(cursor, 500, &filter) {
                Ok(p) => p,
                Err(e) => {
                    control.failed(e);
                    return;
                }
            };
            if page.is_empty() {
                break;
            }
            for event in page {
                cursor = event.seq;
                last = event.seq;
                if !control.send(&out, ServerFrame::Event { sub, event }).await {
                    return;
                }
            }
        }
        loop {
            let msg = tokio::select! {
                m = rx.recv() => m,
                _ = control.cancel.cancelled() => return,
            };
            match msg {
                Ok(BusMessage::Event(event)) => {
                    if event.seq <= last || !filter.matches(&event) {
                        continue;
                    }
                    last = event.seq;
                    if !control
                        .send(
                            &out,
                            ServerFrame::Event {
                                sub,
                                event: (*event).clone(),
                            },
                        )
                        .await
                    {
                        return;
                    }
                }
                Ok(BusMessage::Delta {
                    agent,
                    invocation,
                    delta,
                }) => {
                    if !deltas || filter.agent.is_some_and(|a| a != agent) {
                        continue;
                    }
                    let delta = match delta {
                        DeltaKind::TurnStarted { turn } => Delta::TurnStarted { turn },
                        DeltaKind::Text { text } => Delta::Text { text },
                        DeltaKind::Thinking { text } => Delta::Thinking { text },
                        DeltaKind::ToolInput { name, partial_json } => {
                            Delta::ToolInput { name, partial_json }
                        }
                        DeltaKind::TurnFinished => Delta::TurnFinished,
                    };
                    if !control
                        .send(
                            &out,
                            ServerFrame::Delta {
                                sub,
                                agent,
                                invocation,
                                delta,
                            },
                        )
                        .await
                    {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(sub, lagged = n, "subscription lagged; replaying from store");
                    // Catch up from the store, then continue live.
                    let mut cursor = last;
                    loop {
                        if control.cancel.is_cancelled() {
                            return;
                        }
                        let page = match store.events(cursor, 500, &filter) {
                            Ok(p) => p,
                            Err(error) => {
                                control.failed(error);
                                return;
                            }
                        };
                        if page.is_empty() {
                            break;
                        }
                        for event in page {
                            cursor = event.seq;
                            last = event.seq;
                            if !control.send(&out, ServerFrame::Event { sub, event }).await {
                                return;
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });
    Ok(start_seq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_interrupts_blocked_delivery() {
        let (out, mut received) = mpsc::channel(1);
        let frame = || ServerFrame::Reply {
            id: 0,
            result: Ok(aether_protocol::Reply::Ok),
        };
        out.send(frame()).await.unwrap();
        let (failure, _failures) = mpsc::channel(1);
        let control = SubscriptionControl {
            cancel: CancellationToken::new(),
            failure,
        };
        let pending = control.send(&out, frame());
        tokio::pin!(pending);
        assert!(futures::poll!(&mut pending).is_pending());
        control.cancel.cancel();
        assert!(!pending.await);
        assert!(received.try_recv().is_ok());
        assert!(received.try_recv().is_err());
    }

    #[tokio::test]
    async fn cancellation_wins_over_ready_delivery() {
        let (out, mut received) = mpsc::channel(1);
        let (failure, _failures) = mpsc::channel(1);
        let control = SubscriptionControl {
            cancel: CancellationToken::new(),
            failure,
        };
        control.cancel.cancel();
        assert!(
            !control
                .send(
                    &out,
                    ServerFrame::Reply {
                        id: 0,
                        result: Ok(aether_protocol::Reply::Ok)
                    }
                )
                .await
        );
        assert!(received.try_recv().is_err());
    }

    #[test]
    fn simultaneous_replay_failures_do_not_block_connection_cleanup() {
        let (failure, mut received) = mpsc::channel(1);
        let control = SubscriptionControl {
            cancel: CancellationToken::new(),
            failure,
        };
        for _ in 0..2 {
            control.failed(aether_store::StoreError::HistoryExpired {
                first_seq: 1,
                last_seq: 3,
            });
        }
        assert!(
            received
                .try_recv()
                .unwrap()
                .message
                .contains("history expired")
        );
        assert!(received.try_recv().is_err());
        drop(received);
        control.failed(aether_store::StoreError::HistoryExpired {
            first_seq: 1,
            last_seq: 3,
        });
    }
}
